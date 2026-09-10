//! Session handoff: a fresh session seeded with a continuation briefing
//! instead of the source session's full conversation.
//!
//! Maintainers: unlike `fork_session`, the new session is left dormant —
//! resuming it (session/load, which every navigation into an existing
//! session already goes through) is what activates it, so this handler
//! doesn't duplicate that activation logic.

use super::*;
impl GoslingAcpAgent {
    pub(super) async fn on_handoff_session(
        &self,
        req: HandoffSessionRequest,
    ) -> Result<HandoffSessionResponse, agent_client_protocol::Error> {
        let source_session_id = req.session_id.trim();
        if source_session_id.is_empty() {
            return Err(
                agent_client_protocol::Error::invalid_params().data("sessionId cannot be empty")
            );
        }

        if self
            .active_prompt_runs
            .lock()
            .await
            .contains_key(source_session_id)
        {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("Cannot hand off while the session has an active turn or approval"));
        }

        let source = self
            .session_manager
            .get_session(source_session_id, false)
            .await
            .internal_err()?;
        let agent = self.get_session_agent(source_session_id).await?;
        let current_provider = agent.provider().await.internal_err()?;
        let current_model_config = agent
            .model_config_for_session(source_session_id)
            .await
            .internal_err()?;
        let target_provider = req
            .target_provider
            .as_deref()
            .unwrap_or(current_provider.get_name());
        let target_model = req
            .target_model
            .as_deref()
            .unwrap_or(&current_model_config.model_name);
        self.validate_model_for_provider(target_provider, target_model)
            .await?;
        let target_model_config =
            crate::model_config::model_config_from_user_config_with_session_settings(
                target_provider,
                target_model,
                Some(&current_model_config),
                None,
                None,
            )
            .invalid_params_err_ctx("Invalid handoff target model")?;
        let target_entry = crate::providers::get_from_registry(target_provider)
            .await
            .internal_err_ctx("Failed to read handoff target capabilities")?;
        let snapshot = crate::session::handoff::SessionHandoffBuilder::new(&self.session_manager)
            .build(
                source_session_id,
                target_provider,
                target_model,
                target_model_config.context_limit(),
                target_entry.capabilities(),
                SessionHandoffTriggerDto::SessionFork,
            )
            .await
            .internal_err_ctx("Failed to prepare handoff checkpoint")?;
        if snapshot.continuity_class == SessionContinuityClassDto::NewContextOnly
            && !req.confirm_new_context
        {
            return Err(agent_client_protocol::Error::invalid_params().data(
                "The selected provider supports new context only; set confirmNewContext to continue",
            ));
        }

        let handoff_name = if source.name.trim().is_empty() {
            "(handoff)".to_string()
        } else {
            format!("{} (handoff)", source.name)
        };
        let (new_session, snapshot) = self
            .session_manager
            .create_handoff_session(
                source_session_id,
                handoff_name,
                target_provider.to_string(),
                target_model_config,
                snapshot,
            )
            .await
            .internal_err()?;
        let continuation_prompt = "Continue from the saved session checkpoint. First restate the objective, current state, and next safe action; do not repeat prior side effects."
            .to_string();

        Ok(HandoffSessionResponse {
            session_id: new_session.id,
            snapshot,
            continuation_prompt: continuation_prompt.clone(),
            handoff_summary: Some(continuation_prompt),
        })
    }
}
