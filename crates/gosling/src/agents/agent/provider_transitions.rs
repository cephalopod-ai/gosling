//! Provider, model, mode, thinking-effort, and credential-scoped transitions.
//!
//! Maintainers: preserve persist-before-live-swap and rollback ordering here.
//! Clients: provider restoration, fallback, and mode behavior remain stable.

use super::*;

impl Agent {
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

    async fn update_provider_with_mode(
        &self,
        provider: Arc<dyn Provider>,
        model_config: gosling_providers::model::ModelConfig,
        session_id: &str,
        mode: GoslingMode,
    ) -> Result<()> {
        let _transition = self.state_transition.lock().await;
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
        let session = self
            .config
            .session_manager
            .get_session(session_id, false)
            .await
            .context("Failed to get session")?;

        let extensions = EnabledExtensionsState::extensions_or_default(
            Some(&session.extension_data),
            Config::global(),
        );

        let provider = self
            .create_provider_with_session_scope(&session, provider_name, extensions)
            .await
            .map_err(|e| anyhow!("Could not create provider: {}", e))?;

        self.update_provider(provider, model_config, session_id)
            .await?;

        let mode = self.gosling_mode().await;
        self.update_gosling_mode(mode, session_id).await
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

        self.recreate_provider_for_session(session_id, &provider_name, model_config)
            .await
    }

    /// Restore the provider from session data or fall back to global config
    /// This is used when resuming a session to restore the provider state
    /// Returns true if the session's provider was replaced with a fallback.
    pub async fn restore_provider_from_session(&self, session: &Session) -> Result<bool> {
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

        let (provider, active_model_config, provider_changed) = match primary_result {
            Some(Ok(p)) => (p, model_config, false),
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
                        Some(e) => anyhow!("Could not create provider '{}': {}", provider_name, e),
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

                let fallback_provider = crate::providers::create_with_working_dir(
                    &fallback_provider_name,
                    extensions,
                    session.working_dir.clone(),
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

                if let Err(e) = self
                    .config
                    .session_manager
                    .update(&session.id)
                    .provider_name(&fallback_provider_name)
                    .model_config(fallback_model_config.clone())
                    .apply()
                    .await
                {
                    tracing::warn!("Failed to update session provider: {}", e);
                }

                (fallback_provider, fallback_model_config, true)
            }
        };

        self.update_provider_with_mode(
            provider,
            active_model_config,
            &session.id,
            session.gosling_mode,
        )
        .await?;
        Ok(provider_changed)
    }

    async fn create_provider_with_session_scope(
        &self,
        session: &Session,
        provider_name: &str,
        extensions: Vec<ExtensionConfig>,
    ) -> Result<Arc<dyn Provider>> {
        let Some(profile_id) = session.credential_profile_id.as_deref() else {
            return crate::providers::create_with_working_dir(
                provider_name,
                extensions,
                session.working_dir.clone(),
            )
            .await;
        };
        let service = self
            .config
            .workspace_service
            .as_ref()
            .ok_or_else(|| anyhow!("Workspace credential service is unavailable"))?;
        let resolution = service.profile_resolution(profile_id)?;
        if resolution.provider != provider_name {
            // The pinned credential profile's scope only covers its own
            // provider's config keys. Falling through to an unscoped
            // provider here would silently run this session on global
            // config instead of the isolated profile the session (and its
            // "Pinned" UI indicator) claims to be using — defeating
            // workspace credential isolation without telling the user.
            // Fail closed instead: a mismatch means the workspace's
            // default provider and its default credential binding disagree,
            // or the caller is trying to switch a pinned session to a
            // provider outside its pinned profile. Both need the workspace
            // (or the session's provider selection) fixed, not a silent
            // downgrade.
            bail!(
                "credential profile is pinned to provider '{}', not '{provider_name}'",
                resolution.provider
            );
        }
        let scope = service.config_scope(profile_id).await?;
        Config::with_resolution_scope(scope, async {
            crate::providers::create_with_working_dir(
                provider_name,
                extensions,
                session.working_dir.clone(),
            )
            .await
        })
        .await
    }
}
