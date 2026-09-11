//! ACP provider, model, mode, and thinking-effort configuration.
//!
//! Maintainers: keep provider validation and config update projection together.
//! Clients: configuration choices, validation errors, and notifications remain stable.

use super::*;

fn tool_state_hash(
    session: &Session,
    tools: &[rmcp::model::Tool],
    permission_manager: &crate::config::permission::PermissionManager,
) -> Result<String, agent_client_protocol::Error> {
    let mut tool_permissions = tools
        .iter()
        .map(|tool| {
            let tool_name = tool.name.to_string();
            let permission = permission_manager.get_user_permission(&tool_name);
            (tool_name, permission)
        })
        .collect::<Vec<_>>();
    tool_permissions.sort_by(|left, right| left.0.cmp(&right.0));
    let state = serde_json::to_vec(&(
        session.gosling_mode,
        session
            .extension_data
            .get_extension_state("enabled_extensions", "v0"),
        session
            .extension_data
            .get_extension_state("shell_skill_selection", "v0"),
        &session.working_dir,
        &session.additional_working_dirs,
        session.restrict_tools_to_working_dirs,
        tool_permissions,
    ))
    .internal_err_ctx("Failed to serialize session tool state")?;
    Ok(blake3::hash(&state).to_hex().to_string())
}

pub(super) fn resolve_default_provider_model_config(
    config: &Config,
) -> Result<(String, gosling_providers::model::ModelConfig), agent_client_protocol::Error> {
    let resolved_provider = config.get_gosling_provider().map_err(|error| {
        agent_client_protocol::Error::internal_error()
            .data(format!("Failed to resolve provider: {}", error))
    })?;
    let resolved_model = config.get_gosling_model().map_err(|error| {
        agent_client_protocol::Error::internal_error()
            .data(format!("Failed to resolve model: {}", error))
    })?;
    let resolved_model_config =
        crate::model_config::model_config_from_user_config(&resolved_provider, &resolved_model)
            .map_err(|error| {
                agent_client_protocol::Error::internal_error()
                    .data(format!("Failed to resolve model: {}", error))
            })?;
    Ok((resolved_provider, resolved_model_config))
}

pub(super) async fn resolve_provider_default_model_config(
    provider_name: &str,
) -> Result<gosling_providers::model::ModelConfig, agent_client_protocol::Error> {
    let entry = crate::providers::get_from_registry(provider_name)
        .await
        .map_err(|error| {
            agent_client_protocol::Error::invalid_params()
                .data(format!("Unknown provider '{}': {}", provider_name, error))
        })?;
    crate::model_config::model_config_from_user_config(
        provider_name,
        &entry.metadata().default_model,
    )
    .map_err(|error| {
        agent_client_protocol::Error::internal_error()
            .data(format!("Failed to resolve model: {}", error))
    })
}

impl GoslingAcpAgent {
    async fn tool_continuity_preview(
        &self,
        session: &Session,
        agent: &Arc<Agent>,
        target_provider: &str,
        target_executes_tools_outside_gosling: bool,
    ) -> Result<ToolContinuityPreviewDto, agent_client_protocol::Error> {
        let source_provider = agent
            .provider()
            .await
            .internal_err_ctx("Failed to get provider")?;
        let source_provider_name = source_provider.get_name();
        let source_executes_tools_outside_gosling =
            source_provider.executes_tools_outside_gosling();
        let mut enabled_extension_names = agent.list_extensions().await;
        enabled_extension_names.sort();
        enabled_extension_names.dedup();
        let tools = agent
            .list_tools(&session.id, None)
            .await
            .internal_err_ctx("Failed to inspect session tools")?;
        let ungranted_side_effecting_tool_count = if session.gosling_mode == GoslingMode::Auto {
            tools
                .iter()
                .filter(|tool| {
                    let tool_name = tool.name.as_ref();
                    crate::permission::tool_class::requires_explicit_grant_in_auto(tool_name)
                        && self.permission_manager.get_user_permission(tool_name)
                            != Some(crate::config::permission::PermissionLevel::AlwaysAllow)
                })
                .count() as u64
        } else {
            0
        };

        Ok(ToolContinuityPreviewDto {
            enabled_extension_names,
            gosling_tool_count: tools.len() as u64,
            authorization_mode: session.gosling_mode.to_string(),
            provider_native_tooling_may_change: source_provider_name != target_provider
                && (source_executes_tools_outside_gosling || target_executes_tools_outside_gosling),
            ungranted_side_effecting_tool_count,
            state_hash: tool_state_hash(session, &tools, &self.permission_manager)?,
        })
    }

    async fn send_session_usage_update(
        &self,
        session_id: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        let Some(cx) = self.client_cx.get() else {
            return Ok(());
        };
        let session = self
            .session_manager
            .get_session(session_id, false)
            .await
            .internal_err_ctx("Failed to load transitioned session usage")?;
        let agent = self.get_session_agent(session_id).await?;
        let context_limit = resolve_active_context_limit(&agent, &session).await;
        let Some(updates) = build_usage_updates_with_limit(&session, context_limit) else {
            return Ok(());
        };
        if self.supports_gosling_custom_notifications() {
            cx.send_notification(updates.custom)?;
        }
        cx.send_notification(SessionNotification::new(
            SessionId::new(session_id.to_string()),
            SessionUpdate::UsageUpdate(updates.standard),
        ))?;
        Ok(())
    }

    async fn send_session_usage_update_after_transition(&self, session_id: &str) {
        if let Err(error) = self.send_session_usage_update(session_id).await {
            warn!(
                session_id,
                %error,
                "Provider transition committed but the usage update could not be sent"
            );
        }
    }

    pub(super) async fn on_set_model(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        let agent = self.get_session_agent(session_id).await?;
        let current_provider = agent
            .provider()
            .await
            .internal_err_ctx("Failed to get provider")?;
        let provider_name = current_provider.get_name().to_string();
        let current_model_config = agent
            .model_config_for_session(session_id)
            .await
            .internal_err_ctx("Failed to resolve model config")?;
        self.validate_model_for_provider(&provider_name, model_id)
            .await?;
        let model_config =
            crate::model_config::model_config_from_user_config_with_session_settings(
                &provider_name,
                model_id,
                Some(&current_model_config),
                None,
                None,
            )
            .invalid_params_err_ctx("Invalid model config")?;
        agent
            .transition_provider(
                session_id,
                &provider_name,
                model_config,
                SessionHandoffTriggerDto::ModelChangeRequiresRecreation,
                None,
                None,
                false,
            )
            .await
            .internal_err_ctx("Failed to transition model")?;
        self.send_session_usage_update_after_transition(session_id)
            .await;
        // model_config is already updated on the session by the agent's update_provider call.
        Ok(())
    }

    pub(super) async fn build_config_update(
        &self,
        session_id: &SessionId,
    ) -> Result<(SessionNotification, Vec<SessionConfigOption>), agent_client_protocol::Error> {
        let session = self
            .session_manager
            .get_session(&session_id.0, false)
            .await
            .internal_err()?;
        let agent = self.get_session_agent(&session_id.0).await?;
        let provider = agent
            .provider()
            .await
            .internal_err_ctx("Failed to get provider")?;
        let provider_name = provider.get_name().to_string();
        let current_model_config = agent
            .model_config_for_session(&session_id.0)
            .await
            .internal_err_ctx("Failed to resolve model config")?;
        let current_model = current_model_config.model_name.clone();
        let gosling_mode = agent.gosling_mode().await;
        let inventory = self
            .provider_inventory
            .entry_for_provider(&provider_name)
            .await
            .internal_err()?;
        let Some(inventory) = inventory else {
            return Err(agent_client_protocol::Error::internal_error()
                .data(format!("Unknown provider inventory: {}", provider_name)));
        };
        let model_state = build_model_state(current_model.as_str(), &inventory);
        let executes_tools_outside_gosling = crate::providers::get_from_registry(&provider_name)
            .await
            .internal_err_ctx("Failed to read provider capabilities")?
            .executes_tools_outside_gosling();
        let mode_state = build_mode_state(gosling_mode, executes_tools_outside_gosling)?;
        let provider_options = build_provider_options(Some(&provider_name)).await;
        let config_options = build_config_options(
            &mode_state,
            &model_state,
            &current_model_config,
            session_provider_selection(&session),
            provider_options,
        );
        let notification = SessionNotification::new(
            session_id.clone(),
            SessionUpdate::ConfigOptionUpdate(ConfigOptionUpdate::new(config_options.clone())),
        );
        presentation::ensure_response_fits(&notification, "Session configuration update")?;
        Ok((notification, config_options))
    }

    pub(super) async fn on_set_mode(
        &self,
        session_id: &str,
        mode_id: &str,
    ) -> Result<SetSessionModeResponse, agent_client_protocol::Error> {
        let mode = mode_id.parse::<GoslingMode>().map_err(|_| {
            agent_client_protocol::Error::invalid_params()
                .data(format!("Invalid mode: {}", mode_id))
        })?;

        let agent = self.get_session_agent(session_id).await?;
        agent
            .update_gosling_mode(mode, session_id)
            .await
            .internal_err_ctx("Failed to update mode")?;

        // gosling_mode is already updated on the session above.

        Ok(SetSessionModeResponse::new())
    }

    pub(super) async fn on_set_thinking_effort(
        &self,
        session_id: &str,
        effort_id: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        let effort = effort_id
            .parse::<gosling_providers::thinking::ThinkingEffort>()
            .map_err(|_| {
                agent_client_protocol::Error::invalid_params()
                    .data(format!("Invalid thinking effort: {}", effort_id))
            })?;
        let agent = self.get_session_agent(session_id).await?;
        let provider = agent
            .provider()
            .await
            .internal_err_ctx("Failed to get provider")?;
        let model_config = agent
            .model_config_for_session(session_id)
            .await
            .internal_err_ctx("Failed to resolve model config")?
            .with_thinking_effort(effort);
        agent
            .transition_provider(
                session_id,
                provider.get_name(),
                model_config,
                SessionHandoffTriggerDto::ThinkingEffortChangeRequiresRecreation,
                None,
                None,
                false,
            )
            .await
            .internal_err_ctx("Failed to transition thinking effort")?;
        self.send_session_usage_update_after_transition(session_id)
            .await;

        Ok(())
    }

    pub(super) async fn update_provider(
        &self,
        session_id: &str,
        provider_name: &str,
        model_name: Option<&str>,
        context_limit: Option<usize>,
        request_params: Option<std::collections::HashMap<String, serde_json::Value>>,
    ) -> Result<(), agent_client_protocol::Error> {
        let config = self.config()?;
        let agent = self.get_session_agent(session_id).await?;
        let current_provider = agent
            .provider()
            .await
            .internal_err_ctx("Failed to get provider")?;
        let current_provider_name = current_provider.get_name();
        let current_model_config = agent
            .model_config_for_session(session_id)
            .await
            .internal_err_ctx("Failed to resolve model config")?;
        let current_model = current_model_config.model_name.clone();
        let use_default_provider = provider_name == DEFAULT_PROVIDER_ID;
        // A workspace's own default provider/model take precedence over the
        // app-wide default so picking "Default" inside a workspace session
        // doesn't silently jump to an unrelated provider.
        let workspace_default = if use_default_provider {
            match self.session_manager.get_session(session_id, false).await {
                Ok(session) => session
                    .workspace_id
                    .as_deref()
                    .and_then(|id| self.workspace_service.get(id).ok())
                    .and_then(|workspace| {
                        workspace
                            .default_provider
                            .map(|p| (p, workspace.default_model))
                    }),
                Err(_) => None,
            }
        } else {
            None
        };
        let resolved_provider_name = if let Some((provider, _)) = &workspace_default {
            provider.clone()
        } else if use_default_provider {
            config
                .get_gosling_provider()
                .internal_err_ctx("Failed to resolve default provider from config")?
        } else {
            provider_name.to_string()
        };
        let is_changing_provider = resolved_provider_name != current_provider_name;
        let default_model = if let Some(model_name) = model_name {
            model_name.to_string()
        } else if let Some(model) = workspace_default
            .as_ref()
            .and_then(|(_, model)| model.clone())
        {
            model
        } else if workspace_default.is_some() {
            // The workspace only supplied a provider, no default model; use
            // that provider's own registry default instead of the unrelated
            // app-wide GOSLING_MODEL.
            crate::providers::get_from_registry(&resolved_provider_name)
                .await
                .ok()
                .map(|entry| entry.metadata().default_model.clone())
                .unwrap_or(ACP_CURRENT_MODEL.to_string())
        } else if use_default_provider {
            // Returning to "Gosling Default" (no workspace override) should
            // restore the user's saved app-wide default model, not the
            // resolved provider's registry default.
            config
                .get_gosling_model()
                .internal_err_ctx("Failed to resolve default model from config")?
        } else if is_changing_provider {
            crate::providers::get_from_registry(&resolved_provider_name)
                .await
                .ok()
                .map(|entry| entry.metadata().default_model.clone())
                .unwrap_or(ACP_CURRENT_MODEL.to_string())
        } else {
            current_model
        };
        let model = model_name.unwrap_or(&default_model);
        self.validate_model_for_provider(&resolved_provider_name, model)
            .await?;
        let model_config =
            crate::model_config::model_config_from_user_config_with_session_settings(
                &resolved_provider_name,
                model,
                Some(&current_model_config),
                request_params,
                context_limit,
            )
            .invalid_params_err_ctx("Invalid model config")?;

        agent
            .transition_provider(
                session_id,
                &resolved_provider_name,
                model_config,
                SessionHandoffTriggerDto::UserRequestedSwitch,
                None,
                None,
                false,
            )
            .await
            .internal_err_ctx("Failed to transition provider")?;
        self.send_session_usage_update_after_transition(session_id)
            .await;

        // provider_name is already updated on the session by the agent's update_provider call.
        Ok(())
    }

    pub(super) async fn on_transition_session_provider(
        &self,
        req: TransitionSessionProviderRequest,
    ) -> Result<TransitionSessionProviderResponse, agent_client_protocol::Error> {
        let session_id = req.session_id.trim();
        let target_provider = req.target_provider.trim();
        let target_model = req.target_model.trim();
        if session_id.is_empty() || target_provider.is_empty() || target_model.is_empty() {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("sessionId, targetProvider, and targetModel are required"));
        }
        let transition_guard = self
            .queue_provider_transition(session_id, req.expected_active_run_id.as_deref(), true)
            .await?;
        self.validate_model_for_provider(target_provider, target_model)
            .await?;
        transition_guard.wait_until_idle().await;
        let agent = self.get_session_agent(session_id).await?;
        if let Some(expected_tool_state_hash) = req.expected_tool_state_hash.as_deref() {
            let session = self
                .session_manager
                .get_session(session_id, false)
                .await
                .internal_err_ctx("Failed to read session tool state")?;
            let tools = agent
                .list_tools(&session.id, None)
                .await
                .internal_err_ctx("Failed to inspect session tools")?;
            if tool_state_hash(&session, &tools, &self.permission_manager)?
                != expected_tool_state_hash
            {
                return Err(agent_client_protocol::Error::invalid_params().data(
                    "Session tools or authorization mode changed after the provider switch was previewed; review the checkpoint again",
                ));
            }
        }
        let previous_provider = agent
            .provider()
            .await
            .internal_err_ctx("Failed to get provider")?
            .get_name()
            .to_string();
        let current_model_config = agent
            .model_config_for_session(session_id)
            .await
            .internal_err_ctx("Failed to resolve model config")?;
        let previous_model = current_model_config.model_name.clone();
        let context_limit = req.target_context_limit.map(|value| value as usize);
        let mut model_config =
            crate::model_config::model_config_from_user_config_with_session_settings(
                target_provider,
                target_model,
                Some(&current_model_config),
                req.request_params,
                context_limit,
            )
            .invalid_params_err_ctx("Invalid model config")?;
        let trigger = if previous_provider != target_provider {
            SessionHandoffTriggerDto::UserRequestedSwitch
        } else if previous_model != target_model {
            SessionHandoffTriggerDto::ModelChangeRequiresRecreation
        } else {
            SessionHandoffTriggerDto::ThinkingEffortChangeRequiresRecreation
        };
        if let Some(effort) = req.target_thinking_effort {
            let effort = effort
                .parse::<gosling_providers::thinking::ThinkingEffort>()
                .map_err(|_| {
                    agent_client_protocol::Error::invalid_params()
                        .data("Invalid targetThinkingEffort")
                })?;
            model_config = model_config.with_thinking_effort(effort);
        }

        let snapshot = agent
            .transition_provider(
                session_id,
                target_provider,
                model_config,
                trigger,
                req.expected_current_generation,
                if req.expected_active_run_id.is_some() {
                    None
                } else {
                    req.expected_source_hash.as_deref()
                },
                req.confirm_new_context,
            )
            .await
            .internal_err_ctx("Provider transition failed")?;
        self.send_session_usage_update_after_transition(session_id)
            .await;
        Ok(TransitionSessionProviderResponse {
            snapshot,
            previous_provider,
            previous_model,
            active_provider: target_provider.to_string(),
            active_model: target_model.to_string(),
        })
    }

    pub(super) async fn on_preview_session_handoff(
        &self,
        req: PreviewSessionHandoffRequest,
    ) -> Result<PreviewSessionHandoffResponse, agent_client_protocol::Error> {
        let session_id = req.session_id.trim();
        let target_provider = req.target_provider.trim();
        let target_model = req.target_model.trim();
        if session_id.is_empty() || target_provider.is_empty() || target_model.is_empty() {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("sessionId, targetProvider, and targetModel are required"));
        }
        self.validate_model_for_provider(target_provider, target_model)
            .await?;
        let queued_after_run_id = self.active_run_id(session_id).await?;
        let agent = self.get_session_agent(session_id).await?;
        let session = self
            .session_manager
            .get_session(session_id, false)
            .await
            .internal_err_ctx("Failed to read session tool state")?;
        let current_model_config = agent
            .model_config_for_session(session_id)
            .await
            .internal_err_ctx("Failed to resolve model config")?;
        let target_entry = crate::providers::get_from_registry(target_provider)
            .await
            .internal_err_ctx("Failed to read target provider capabilities")?;
        let target_model_config =
            crate::model_config::model_config_from_user_config_with_session_settings(
                target_provider,
                target_model,
                Some(&current_model_config),
                None,
                req.target_context_limit.map(|value| value as usize),
            )
            .invalid_params_err_ctx("Invalid model config")?;
        let target_model_config = target_entry
            .normalize_model_config(target_model_config)
            .invalid_params_err_ctx("Invalid model config")?;
        let tool_continuity = self
            .tool_continuity_preview(
                &session,
                &agent,
                target_provider,
                target_entry.executes_tools_outside_gosling(),
            )
            .await?;
        let expected_current_generation = self
            .session_manager
            .latest_handoff_generation(session_id)
            .await
            .internal_err_ctx("Failed to read handoff generation")?;
        let mut snapshot =
            crate::session::handoff::SessionHandoffBuilder::new(&self.session_manager)
                .build(
                    session_id,
                    target_provider,
                    target_model,
                    target_model_config.context_limit(),
                    target_entry.capabilities(),
                    SessionHandoffTriggerDto::UserRequestedSwitch,
                )
                .await
                .internal_err_ctx("Failed to preview session checkpoint")?;
        snapshot.generation = expected_current_generation + 1;
        Ok(PreviewSessionHandoffResponse {
            snapshot,
            expected_current_generation,
            queued_after_run_id,
            tool_continuity,
        })
    }

    pub(super) async fn on_read_session_handoff_checkpoint(
        &self,
        req: ReadSessionHandoffCheckpointRequest,
    ) -> Result<ReadSessionHandoffCheckpointResponse, agent_client_protocol::Error> {
        let session_id = req.session_id.trim();
        if session_id.is_empty() {
            return Err(
                agent_client_protocol::Error::invalid_params().data("sessionId cannot be empty")
            );
        }
        self.session_manager
            .get_session(session_id, false)
            .await
            .map_err(|_| {
                agent_client_protocol::Error::resource_not_found(Some(session_id.to_string()))
                    .data(format!("Session not found: {session_id}"))
            })?;
        Ok(ReadSessionHandoffCheckpointResponse {
            snapshot: self
                .session_manager
                .latest_handoff_snapshot(session_id)
                .await
                .internal_err_ctx("Failed to read session checkpoint")?,
        })
    }

    pub(super) async fn validate_model_for_provider(
        &self,
        provider_id: &str,
        model_id: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        let entry = self
            .provider_inventory
            .entry_for_provider(provider_id)
            .await
            .internal_err_ctx("Failed to read provider inventory")?
            .ok_or_else(|| {
                agent_client_protocol::Error::invalid_params()
                    .data(format!("Unknown provider: {provider_id}"))
            })?;
        let model_exists = entry.default_model == model_id
            || entry.models.iter().any(|model| model.id == model_id);
        if model_exists {
            return Ok(());
        }

        let provider = self
            .create_provider(provider_id, Vec::new(), None)
            .await
            .internal_err_ctx("Failed to initialize provider for model validation")?;
        let supported_models = provider
            .fetch_supported_models()
            .await
            .internal_err_ctx("Failed to fetch provider models for validation")?;
        if !supported_models.iter().any(|model| model == model_id) {
            return Err(agent_client_protocol::Error::invalid_params().data(format!(
                "Model '{model_id}' is not available for provider '{provider_id}'"
            )));
        }
        Ok(())
    }
}
