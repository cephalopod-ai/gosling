//! Provider, model, mode, thinking-effort, and credential-scoped transitions.
//!
//! Maintainers: preserve persist-before-live-swap and rollback ordering here.
//! Clients: provider restoration, fallback, and mode behavior remain stable.

use super::*;

async fn deliver_bootstrap_handoff(
    candidate: &Arc<dyn Provider>,
    model_config: &gosling_providers::model::ModelConfig,
    session_id: &str,
    snapshot: &gosling_sdk_types::session_handoff::SessionHandoffSnapshotV1Dto,
) -> Result<()> {
    let bootstrap_messages = vec![
        crate::session::handoff::handoff_bootstrap_message(snapshot)?,
        Message::user()
            .with_text("Acknowledge this checkpoint by stating only the objective, current state, and next safe action. Do not use tools or perform any action.")
            .with_visibility(false, true),
    ];
    let acknowledgement = tokio::time::timeout(
        Duration::from_secs(30),
        crate::session_context::with_session_id(
            Some(session_id.to_string()),
            candidate.complete(
                model_config,
                "You are receiving a session handoff checkpoint. Acknowledge it without using tools or taking action.",
                &bootstrap_messages,
                &[],
            ),
        ),
    )
    .await;
    let acknowledgement = match acknowledgement {
        Ok(Ok((message, _))) => message,
        Ok(Err(error)) => anyhow::bail!("Checkpoint acknowledgement failed: {error}"),
        Err(_) => anyhow::bail!("Checkpoint acknowledgement timed out after 30 seconds"),
    };
    let acknowledgement_is_safe = acknowledgement.content.iter().all(|content| {
        matches!(
            content,
            MessageContent::Text(_)
                | MessageContent::Thinking(_)
                | MessageContent::RedactedThinking(_)
        )
    }) && !acknowledgement.as_concat_text().trim().is_empty();
    anyhow::ensure!(
        acknowledgement_is_safe,
        "Checkpoint acknowledgement was empty or attempted an action"
    );
    Ok(())
}

impl Agent {
    pub(super) async fn persist_provider_failure_checkpoint(
        &self,
        session_id: &str,
        provider: &Arc<dyn Provider>,
        model_config: &gosling_providers::model::ModelConfig,
        failure_message: &Message,
    ) -> Result<()> {
        self.config
            .session_manager
            .upsert_message(session_id, failure_message)
            .await?;
        let mut snapshot =
            crate::session::handoff::SessionHandoffBuilder::new(&self.config.session_manager)
                .build(
                    session_id,
                    provider.get_name(),
                    &model_config.model_name,
                    model_config.context_limit(),
                    provider.capabilities(),
                    gosling_sdk_types::session_handoff::SessionHandoffTriggerDto::ProviderFailure,
                )
                .await?;
        if let Some(failure) = failure_message.metadata.terminal_error.as_deref() {
            crate::session::handoff::set_redacted_failure(&mut snapshot, failure);
        }
        self.config
            .session_manager
            .prepare_handoff_snapshot(snapshot, None)
            .await?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn transition_provider(
        &self,
        session_id: &str,
        target_provider_name: &str,
        target_model_config: gosling_providers::model::ModelConfig,
        trigger: gosling_sdk_types::session_handoff::SessionHandoffTriggerDto,
        expected_current_generation: Option<u64>,
        expected_source_hash: Option<&str>,
        confirm_new_context: bool,
    ) -> Result<gosling_sdk_types::session_handoff::SessionHandoffSnapshotV1Dto> {
        use gosling_sdk_types::session_handoff::{
            SessionContinuityClassDto, SessionHandoffStatusDto,
        };

        let _transition = self.state_transition.lock().await;
        let session = self
            .config
            .session_manager
            .get_session(session_id, false)
            .await?;
        self.validate_session_provider_scope(&session, target_provider_name)?;
        let target_entry = crate::providers::get_from_registry(target_provider_name).await?;
        let target_model_config = target_entry.normalize_model_config(target_model_config)?;
        let snapshot =
            crate::session::handoff::SessionHandoffBuilder::new(&self.config.session_manager)
                .build(
                    session_id,
                    target_provider_name,
                    &target_model_config.model_name,
                    target_model_config.context_limit(),
                    target_entry.capabilities(),
                    trigger,
                )
                .await?;
        if let Some(expected_generation) = expected_current_generation {
            let current_generation = self
                .config
                .session_manager
                .latest_handoff_generation(session_id)
                .await?;
            anyhow::ensure!(
                current_generation == expected_generation,
                "stale handoff generation: expected {expected_generation}, current {current_generation}"
            );
        }
        if let Some(expected_source_hash) = expected_source_hash {
            anyhow::ensure!(
                snapshot.coverage.source_hash == expected_source_hash,
                "session changed after the handoff checkpoint was previewed"
            );
        }
        anyhow::ensure!(
            !snapshot
                .active_or_interrupted_operations
                .iter()
                .any(|operation| operation.state == "started"),
            "Cannot switch providers while a tool operation is active"
        );
        anyhow::ensure!(
            snapshot.pending_approvals.is_empty(),
            "Cannot switch providers while an approval is pending"
        );
        anyhow::ensure!(
            snapshot.continuity_class != SessionContinuityClassDto::NewContextOnly
                || confirm_new_context,
            "The selected provider supports new context only; explicit confirmation is required"
        );
        let snapshot = self
            .config
            .session_manager
            .prepare_handoff_snapshot(snapshot, expected_current_generation)
            .await?;
        let mut snapshot = self
            .config
            .session_manager
            .update_handoff_status(
                &snapshot.snapshot_id,
                SessionHandoffStatusDto::Activating,
                None,
            )
            .await?;

        let extensions = EnabledExtensionsState::extensions_or_default(
            Some(&session.extension_data),
            Config::global(),
        );
        let previous_provider = self.provider().await?;
        let reuses_provider = previous_provider.get_name() == target_provider_name
            && target_entry.capabilities().in_place_model_change
                != crate::providers::base::CapabilitySupport::Unsupported;
        let candidate = if reuses_provider {
            previous_provider
        } else {
            match self
                .create_provider_with_session_scope(&session, target_provider_name, extensions)
                .await
            {
                Ok(candidate) => candidate,
                Err(error) => {
                    let _ = self
                        .config
                        .session_manager
                        .update_handoff_status(
                            &snapshot.snapshot_id,
                            SessionHandoffStatusDto::Failed,
                            Some(&error.to_string()),
                        )
                        .await;
                    return Err(error);
                }
            }
        };
        let mode = session.gosling_mode;
        if !reuses_provider {
            if let Err(error) = candidate.update_mode(session_id, mode).await {
                let message = format!("Provider rejected mode update: {error}");
                let _ = self
                    .config
                    .session_manager
                    .update_handoff_status(
                        &snapshot.snapshot_id,
                        SessionHandoffStatusDto::Failed,
                        Some(&message),
                    )
                    .await;
                anyhow::bail!(message);
            }
        }

        use gosling_sdk_types::session_handoff::{
            HandoffDeliveryStrategyDto, HandoffEvidenceClassDto, HandoffEvidenceItemDto,
        };
        let native_kind = match snapshot.delivery_strategy {
            HandoffDeliveryStrategyDto::NativeResume => {
                Some(crate::providers::base::NativeHandoffKind::Resume)
            }
            HandoffDeliveryStrategyDto::HistoryImport => {
                Some(crate::providers::base::NativeHandoffKind::HistoryImport)
            }
            _ => None,
        };
        if let Some(native_kind) = native_kind {
            let snapshot_json = serde_json::to_string(&snapshot)?;
            match candidate
                .deliver_native_handoff(native_kind, session_id, &snapshot_json)
                .await
            {
                Ok(receipt) => {
                    let provider_session_id = receipt
                        .provider_session_id
                        .filter(|value| !value.trim().is_empty());
                    let acknowledged = receipt
                        .acknowledgement
                        .is_some_and(|value| !value.trim().is_empty());
                    if provider_session_id.is_none() && !acknowledged {
                        let message =
                            "Native handoff returned no provider session identity or acknowledgement";
                        let _ = self
                            .config
                            .session_manager
                            .update_handoff_status(
                                &snapshot.snapshot_id,
                                SessionHandoffStatusDto::Failed,
                                Some(message),
                            )
                            .await;
                        anyhow::bail!(message);
                    }
                    snapshot.target.provider_session_id = provider_session_id;
                    snapshot.acknowledged_at = Some(chrono::Utc::now().to_rfc3339());
                    self.config
                        .session_manager
                        .update_handoff_snapshot(&snapshot)
                        .await?;
                }
                Err(error)
                    if target_entry.capabilities().bootstrap_handoff
                        != crate::providers::base::CapabilitySupport::Unsupported =>
                {
                    snapshot.continuity_class = SessionContinuityClassDto::SummarizedHandoff;
                    snapshot.delivery_strategy = HandoffDeliveryStrategyDto::Bootstrap;
                    snapshot.attempted_mitigations.push(HandoffEvidenceItemDto {
                        content:
                            "Native handoff failed; Gosling used the bounded bootstrap fallback."
                                .to_string(),
                        evidence: HandoffEvidenceClassDto::Observed,
                        source_message_id: None,
                        source_row_id: None,
                        timestamp: Some(chrono::Utc::now().timestamp()),
                    });
                    self.config
                        .session_manager
                        .update_handoff_snapshot(&snapshot)
                        .await?;
                    if let Err(bootstrap_error) = deliver_bootstrap_handoff(
                        &candidate,
                        &target_model_config,
                        session_id,
                        &snapshot,
                    )
                    .await
                    {
                        let message = format!(
                            "Native handoff failed ({error}); bootstrap fallback failed: {bootstrap_error}"
                        );
                        let _ = self
                            .config
                            .session_manager
                            .update_handoff_status(
                                &snapshot.snapshot_id,
                                SessionHandoffStatusDto::Failed,
                                Some(&message),
                            )
                            .await;
                        anyhow::bail!(message);
                    }
                    self.config
                        .session_manager
                        .acknowledge_handoff_snapshot(&snapshot.snapshot_id)
                        .await?;
                }
                Err(error) => {
                    let message = format!("Native handoff failed: {error}");
                    let _ = self
                        .config
                        .session_manager
                        .update_handoff_status(
                            &snapshot.snapshot_id,
                            SessionHandoffStatusDto::Failed,
                            Some(&message),
                        )
                        .await;
                    anyhow::bail!(message);
                }
            }
        } else if snapshot.delivery_strategy == HandoffDeliveryStrategyDto::Bootstrap {
            if let Err(error) =
                deliver_bootstrap_handoff(&candidate, &target_model_config, session_id, &snapshot)
                    .await
            {
                let message = error.to_string();
                let _ = self
                    .config
                    .session_manager
                    .update_handoff_status(
                        &snapshot.snapshot_id,
                        SessionHandoffStatusDto::Failed,
                        Some(&message),
                    )
                    .await;
                return Err(error);
            }
            self.config
                .session_manager
                .acknowledge_handoff_snapshot(&snapshot.snapshot_id)
                .await?;
        }

        let mut current_provider = self.provider.lock().await;
        let activated = match self
            .config
            .session_manager
            .commit_provider_transition(
                &snapshot.snapshot_id,
                target_provider_name,
                target_model_config,
                mode,
            )
            .await
        {
            Ok(snapshot) => snapshot,
            Err(error) => {
                let _ = self
                    .config
                    .session_manager
                    .update_handoff_status(
                        &snapshot.snapshot_id,
                        SessionHandoffStatusDto::Failed,
                        Some(&error.to_string()),
                    )
                    .await;
                return Err(error);
            }
        };
        *current_provider = Some(candidate);
        *self.current_gosling_mode.lock().await = mode;
        let _ = self.gosling_mode_changes.send(mode);
        Ok(activated)
    }

    pub async fn update_provider(
        &self,
        provider: Arc<dyn Provider>,
        model_config: gosling_providers::model::ModelConfig,
        session_id: &str,
    ) -> Result<()> {
        let _transition = self.state_transition.lock().await;
        let mode = self.gosling_mode().await;
        self.apply_provider_transition(provider, model_config, session_id, mode)
            .await
    }

    async fn apply_provider_transition(
        &self,
        provider: Arc<dyn Provider>,
        model_config: gosling_providers::model::ModelConfig,
        session_id: &str,
        mode: GoslingMode,
    ) -> Result<()> {
        let provider_name = provider.get_name().to_string();

        // Normalize against the provider entry so custom/declarative providers
        // backfill `context_limit` from their known models before the config is
        // persisted as the session source of truth; otherwise auto-compaction
        // would fall back to DEFAULT_CONTEXT_LIMIT.
        let model_config = match crate::providers::get_from_registry(&provider_name).await {
            Ok(entry) => entry
                .normalize_model_config(model_config.clone())
                .unwrap_or(model_config),
            Err(_) => model_config,
        };

        provider
            .update_mode(session_id, mode)
            .await
            .map_err(|e| anyhow::anyhow!("Provider rejected mode update: {e}"))?;

        let mut current_provider = self.provider.lock().await;
        self.config
            .session_manager
            .clone()
            .update(session_id)
            .provider_name(&provider_name)
            .model_config(model_config)
            .apply()
            .await
            .context("Failed to persist provider config to session")?;

        *current_provider = Some(provider);
        *self.current_gosling_mode.lock().await = mode;
        Ok(())
    }

    pub async fn update_gosling_mode(&self, mode: GoslingMode, session_id: &str) -> Result<()> {
        // Clone the Arc out and drop the guard before awaiting: holding the
        // lock across update_mode's round-trip to the provider (which can
        // be an external subprocess for ACP-backed providers, with no
        // timeout) would stall every other task that needs self.provider,
        // including the main reply loop, for as long as that hangs.
        let _transition = self.state_transition.lock().await;
        let mut current_mode = self.current_gosling_mode.lock().await;
        let previous_mode = *current_mode;
        self.config
            .session_manager
            .clone()
            .update(session_id)
            .gosling_mode(mode)
            .apply()
            .await
            .context("Failed to persist gosling_mode to session")?;

        let provider = self.provider.lock().await.clone();
        if let Some(provider) = provider {
            if let Err(error) = provider.update_mode(session_id, mode).await {
                let provider_rollback = provider.update_mode(session_id, previous_mode).await;
                let rollback = self
                    .config
                    .session_manager
                    .clone()
                    .update(session_id)
                    .gosling_mode(previous_mode)
                    .apply()
                    .await;
                let mut rollback_errors = Vec::new();
                if let Err(provider_rollback) = provider_rollback {
                    rollback_errors.push(format!("provider: {provider_rollback}"));
                }
                if let Err(rollback_error) = rollback {
                    rollback_errors.push(format!("session: {rollback_error}"));
                }
                let rollback_detail = if rollback_errors.is_empty() {
                    String::new()
                } else {
                    format!("; rollback errors: {}", rollback_errors.join("; "))
                };
                return Err(anyhow::anyhow!(
                    "Provider rejected mode update: {error}{rollback_detail}"
                ));
            }
        }

        *current_mode = mode;
        let _ = self.gosling_mode_changes.send(mode);
        Ok(())
    }

    pub async fn gosling_mode(&self) -> GoslingMode {
        *self.current_gosling_mode.lock().await
    }

    pub async fn recreate_provider_for_session(
        &self,
        session_id: &str,
        provider_name: &str,
        model_config: gosling_providers::model::ModelConfig,
    ) -> Result<()> {
        self.transition_provider(
            session_id,
            provider_name,
            model_config,
            gosling_sdk_types::session_handoff::SessionHandoffTriggerDto::ModelChangeRequiresRecreation,
            None,
            None,
            false,
        )
        .await
        .map(|_| ())
    }

    pub async fn update_thinking_effort(
        &self,
        session_id: &str,
        effort: ThinkingEffort,
    ) -> Result<()> {
        let current_provider = self.provider().await?;
        let provider_name = current_provider.get_name().to_string();
        let model_config = self
            .model_config_for_session(session_id)
            .await?
            .with_thinking_effort(effort);

        self.transition_provider(
            session_id,
            &provider_name,
            model_config,
            gosling_sdk_types::session_handoff::SessionHandoffTriggerDto::ThinkingEffortChangeRequiresRecreation,
            None,
            None,
            false,
        )
        .await
        .map(|_| ())
    }

    /// Restore the provider from session data or fall back to global config
    /// This is used when resuming a session to restore the provider state
    /// Returns true if the session's provider was replaced with a fallback.
    pub async fn restore_provider_from_session(&self, session: &Session) -> Result<bool> {
        use crate::providers::base::ContextOwnership;
        use gosling_sdk_types::session_handoff::{
            SessionContinuityClassDto, SessionHandoffStatusDto, SessionHandoffTriggerDto,
        };

        let _transition = self.state_transition.lock().await;
        let config = Config::global();

        let provider_name = session
            .provider_name
            .clone()
            .or_else(|| config.get_gosling_provider().ok())
            .ok_or_else(|| anyhow!("Could not configure agent: missing provider"))?;

        let model_config = match session.model_config.clone() {
            Some(saved_config) => saved_config,
            None => {
                let model_name = config
                    .get_gosling_model()
                    .ok()
                    .ok_or_else(|| anyhow!("Could not configure agent: missing model"))?;
                crate::model_config::model_config_from_user_config(&provider_name, &model_name)
                    .map_err(|e| anyhow!("Could not configure agent: invalid model {}", e))?
            }
        };

        let extensions =
            EnabledExtensionsState::extensions_or_default(Some(&session.extension_data), config);

        // Try the session's saved provider first whenever its type is
        // registered at all — not just when it's registered AND already
        // configured. The fallback below exists specifically to survive a
        // known provider type whose credentials were revoked/removed; gating
        // it on registry presence alone meant that case always hit a hard
        // create_with_working_dir error instead of ever reaching it.
        let primary_result = if crate::providers::get_from_registry(&provider_name)
            .await
            .is_ok()
        {
            Some(
                self.create_provider_with_session_scope(
                    session,
                    &provider_name,
                    extensions.clone(),
                )
                .await,
            )
        } else {
            None
        };

        let (provider, active_provider_name, active_model_config, provider_changed) =
            match primary_result {
                Some(Ok(p)) => (p, provider_name.clone(), model_config, false),
                Some(Err(error)) if session.credential_profile_id.is_some() => {
                    return Err(anyhow!(
                        "Pinned credential profile is unavailable for provider '{}': {}",
                        provider_name,
                        error
                    ));
                }
                None if session.credential_profile_id.is_some() => {
                    return Err(anyhow!(
                        "Pinned provider '{}' is no longer available",
                        provider_name
                    ));
                }
                primary_result => {
                    let primary_error = primary_result.and_then(Result::err);

                    let fallback_provider_name = config
                        .get_gosling_provider()
                        .ok()
                        .filter(|name| name != &provider_name)
                        .ok_or_else(|| match &primary_error {
                            Some(e) => {
                                anyhow!("Could not create provider '{}': {}", provider_name, e)
                            }
                            None => anyhow!(
                                "Could not create provider: provider '{}' not found",
                                provider_name
                            ),
                        })?;

                    tracing::warn!(
                        "Session provider '{}' unavailable ({}), falling back to '{}'",
                        provider_name,
                        primary_error
                            .as_ref()
                            .map(|e| e.to_string())
                            .unwrap_or_else(|| "not found in registry".to_string()),
                        fallback_provider_name
                    );

                    let fallback_model_name = config.get_gosling_model().ok().ok_or_else(|| {
                        anyhow!("Could not configure fallback provider: missing model")
                    })?;
                    let fallback_model_config = crate::model_config::model_config_from_user_config(
                        &fallback_provider_name,
                        &fallback_model_name,
                    )
                    .map_err(|e| {
                        anyhow!("Could not configure fallback provider: invalid model {}", e)
                    })?;

                    let fallback_provider = self
                        .create_provider_with_session_scope(
                            session,
                            &fallback_provider_name,
                            extensions,
                        )
                        .await
                        .map_err(|e| {
                            anyhow!(
                                "Could not create provider '{}' or fallback '{}': {}",
                                provider_name,
                                fallback_provider_name,
                                e
                            )
                        })?;

                    (
                        fallback_provider,
                        fallback_provider_name,
                        fallback_model_config,
                        true,
                    )
                }
            };

        let target_entry = crate::providers::get_from_registry(&active_provider_name).await?;
        let active_model_config = target_entry.normalize_model_config(active_model_config)?;
        let capabilities = provider.capabilities();
        let requires_handoff = session.message_count > 0
            && (provider_changed || capabilities.context_ownership != ContextOwnership::Gosling);

        if requires_handoff {
            let snapshot =
                crate::session::handoff::SessionHandoffBuilder::new(&self.config.session_manager)
                    .build(
                        &session.id,
                        &active_provider_name,
                        &active_model_config.model_name,
                        active_model_config.context_limit(),
                        capabilities,
                        SessionHandoffTriggerDto::SessionResume,
                    )
                    .await?;
            anyhow::ensure!(
                snapshot.continuity_class != SessionContinuityClassDto::NewContextOnly,
                "Provider '{}' cannot safely resume this session; choose a provider with handoff support",
                active_provider_name
            );
            let snapshot = self
                .config
                .session_manager
                .prepare_handoff_snapshot(snapshot, None)
                .await?;
            let snapshot = self
                .config
                .session_manager
                .update_handoff_status(
                    &snapshot.snapshot_id,
                    SessionHandoffStatusDto::Activating,
                    None,
                )
                .await?;

            if let Err(error) = provider
                .update_mode(&session.id, session.gosling_mode)
                .await
            {
                let message = format!("Provider rejected mode update: {error}");
                let _ = self
                    .config
                    .session_manager
                    .update_handoff_status(
                        &snapshot.snapshot_id,
                        SessionHandoffStatusDto::Failed,
                        Some(&message),
                    )
                    .await;
                anyhow::bail!(message);
            }

            if let Err(error) = self
                .config
                .session_manager
                .commit_provider_transition(
                    &snapshot.snapshot_id,
                    &active_provider_name,
                    active_model_config,
                    session.gosling_mode,
                )
                .await
            {
                let _ = self
                    .config
                    .session_manager
                    .update_handoff_status(
                        &snapshot.snapshot_id,
                        SessionHandoffStatusDto::Failed,
                        Some(&error.to_string()),
                    )
                    .await;
                return Err(error);
            }

            *self.provider.lock().await = Some(provider);
            *self.current_gosling_mode.lock().await = session.gosling_mode;
            let _ = self.gosling_mode_changes.send(session.gosling_mode);
        } else {
            self.apply_provider_transition(
                provider,
                active_model_config,
                &session.id,
                session.gosling_mode,
            )
            .await?;
        }
        Ok(provider_changed)
    }

    pub(super) fn provider_failover_target(&self) -> Option<ProviderFailoverTarget> {
        if let Some(failover) = self.config.provider_failover.clone() {
            return Some(ProviderFailoverTarget::Ready(failover));
        }

        let config = Config::global();
        let provider_name = config
            .get_param::<String>("GOSLING_FAILOVER_PROVIDER")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let model_name = config
            .get_param::<String>("GOSLING_FAILOVER_MODEL")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());

        match (provider_name, model_name) {
            (None, None) => None,
            (Some(provider_name), Some(model_name)) => Some(ProviderFailoverTarget::Configured {
                provider_name,
                model_name,
            }),
            _ => Some(ProviderFailoverTarget::Invalid(
                "GOSLING_FAILOVER_PROVIDER and GOSLING_FAILOVER_MODEL must be configured together"
                    .to_string(),
            )),
        }
    }

    pub(super) async fn resolve_provider_failover(
        &self,
        target: ProviderFailoverTarget,
        session: &Session,
        primary_provider: &dyn Provider,
        primary_model_config: &ModelConfig,
    ) -> Result<ProviderFailoverConfig> {
        if session.credential_profile_id.is_some() {
            bail!("automatic failover is disabled for credential-pinned sessions");
        }
        if primary_provider.capabilities().context_ownership
            != crate::providers::base::ContextOwnership::Gosling
            || primary_provider.executes_tools_outside_gosling()
            || primary_provider.permission_routing() != PermissionRouting::Noop
        {
            bail!(
                "provider '{}' does not use Gosling's host-managed API turn loop",
                primary_provider.get_name()
            );
        }

        let mut failover = match target {
            ProviderFailoverTarget::Ready(failover) => failover,
            ProviderFailoverTarget::Configured {
                provider_name,
                model_name,
            } => {
                let entry = crate::providers::get_from_registry(&provider_name).await?;
                if entry.capabilities().context_ownership
                    != crate::providers::base::ContextOwnership::Gosling
                    || entry.executes_tools_outside_gosling()
                {
                    bail!(
                        "fallback provider '{provider_name}' does not use Gosling's host-managed API turn loop"
                    );
                }

                let model_config =
                    crate::model_config::model_config_from_user_config(&provider_name, &model_name)
                        .and_then(|model_config| entry.normalize_model_config(model_config))?;
                let extensions = EnabledExtensionsState::extensions_or_default(
                    Some(&session.extension_data),
                    Config::global(),
                );
                let provider = crate::providers::create_with_working_dir(
                    &provider_name,
                    extensions,
                    session.working_dir.clone(),
                )
                .await?;

                ProviderFailoverConfig::new(provider, model_config)
            }
            ProviderFailoverTarget::Invalid(message) => bail!(message),
        };

        if failover.provider.capabilities().context_ownership
            != crate::providers::base::ContextOwnership::Gosling
            || failover.provider.executes_tools_outside_gosling()
            || failover.provider.permission_routing() != PermissionRouting::Noop
        {
            bail!(
                "fallback provider '{}' does not use Gosling's host-managed API turn loop",
                failover.provider.get_name()
            );
        }

        if let Ok(entry) = crate::providers::get_from_registry(failover.provider.get_name()).await {
            failover.model_config = entry.normalize_model_config(failover.model_config)?;
        }

        if failover.provider.get_name() == primary_provider.get_name()
            && failover.model_config.model_name == primary_model_config.model_name
        {
            bail!("fallback provider and model are identical to the primary route");
        }
        if failover.model_config.toolshim != primary_model_config.toolshim {
            bail!("fallback and primary models must use the same toolshim setting");
        }

        failover
            .provider
            .update_mode(&session.id, session.gosling_mode)
            .await
            .map_err(|error| anyhow!("Fallback provider rejected mode update: {error}"))?;

        Ok(failover)
    }

    async fn create_provider_with_session_scope(
        &self,
        session: &Session,
        provider_name: &str,
        extensions: Vec<ExtensionConfig>,
    ) -> Result<Arc<dyn Provider>> {
        self.validate_session_provider_scope(session, provider_name)?;
        let entry = crate::providers::get_from_registry(provider_name).await?;
        let extensions = if entry.executes_tools_outside_gosling() {
            let executable = std::env::current_exe()
                .context("Could not locate Gosling for the session history bridge")?;
            let data_dir = self.config.session_manager.data_dir();
            extensions
                .into_iter()
                .map(|extension| {
                    crate::agents::platform_extensions::session_history::bridge_for_provider_owned_tools(
                        extension,
                        &executable,
                        &data_dir,
                        &session.id,
                    )
                    .map_err(anyhow::Error::msg)
                })
                .collect::<Result<Vec<_>>>()?
        } else {
            extensions
        };
        let Some(profile_id) = session.credential_profile_id.as_deref() else {
            return entry
                .create_with_working_dir(extensions, session.working_dir.clone())
                .await;
        };
        let service = self
            .config
            .workspace_service
            .as_ref()
            .ok_or_else(|| anyhow!("Workspace credential service is unavailable"))?;
        let scope = service.config_scope(profile_id).await?;
        Config::with_resolution_scope(scope, async {
            entry
                .create_with_working_dir(extensions, session.working_dir.clone())
                .await
        })
        .await
    }

    fn validate_session_provider_scope(
        &self,
        session: &Session,
        provider_name: &str,
    ) -> Result<()> {
        let Some(profile_id) = session.credential_profile_id.as_deref() else {
            return Ok(());
        };
        let service = self
            .config
            .workspace_service
            .as_ref()
            .ok_or_else(|| anyhow!("Workspace credential service is unavailable"))?;
        let resolution = service.profile_resolution(profile_id)?;
        if resolution.provider != provider_name {
            // The pinned profile's scope covers only its own provider. Allowing
            // a mismatch would silently use global credentials while the UI
            // still claims that the session is isolated by the pinned profile.
            bail!(
                "credential profile is pinned to provider '{}', not '{provider_name}'",
                resolution.provider
            );
        }
        Ok(())
    }
}
