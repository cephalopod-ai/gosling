//! ACP provider, model, mode, and thinking-effort configuration.
//!
//! Maintainers: keep provider validation and config update projection together.
//! Clients: configuration choices, validation errors, and notifications remain stable.

use super::*;

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
            .recreate_provider_for_session(session_id, &provider_name, model_config)
            .await
            .internal_err_ctx("Failed to recreate provider")?;
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
        agent
            .update_thinking_effort(session_id, effort)
            .await
            .internal_err_ctx("Failed to update thinking effort")?;

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

        let executes_tools_outside_gosling =
            crate::providers::get_from_registry(&resolved_provider_name)
                .await
                .internal_err_ctx("Failed to read provider capabilities")?
                .executes_tools_outside_gosling();
        let compatible_mode =
            compatible_mode(agent.gosling_mode().await, executes_tools_outside_gosling);
        if compatible_mode != agent.gosling_mode().await {
            agent
                .update_gosling_mode(compatible_mode, session_id)
                .await
                .internal_err_ctx("Failed to select a provider-compatible mode")?;
        }

        agent
            .recreate_provider_for_session(session_id, &resolved_provider_name, model_config)
            .await
            .internal_err_ctx("Failed to recreate provider")?;

        // provider_name is already updated on the session by the agent's update_provider call.
        Ok(())
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
