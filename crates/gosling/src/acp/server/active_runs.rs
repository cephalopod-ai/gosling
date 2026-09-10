//! Active ACP prompt-run registration, cancellation, steering, and close cleanup.
//!
//! Maintainers: AgentManager busy pins and the ACP run map must change atomically.
//! Clients: run identifiers, steer fences, cancellation, and close behavior remain stable.

use super::*;

#[derive(Default)]
struct SessionOperationState {
    active_run_id: Option<String>,
    last_completed_run_id: Option<String>,
    provider_transition_pending: bool,
}

#[derive(Default)]
pub(super) struct SessionOperationGate {
    state: std::sync::Mutex<SessionOperationState>,
    changed: Notify,
}

pub(super) struct PromptOperationGuard {
    gate: Arc<SessionOperationGate>,
    run_id: String,
}

impl Drop for PromptOperationGuard {
    fn drop(&mut self) {
        let mut state = self
            .gate
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.active_run_id.as_deref() == Some(self.run_id.as_str()) {
            state.active_run_id = None;
            state.last_completed_run_id = Some(self.run_id.clone());
        }
        drop(state);
        self.gate.changed.notify_waiters();
    }
}

pub(super) struct ProviderTransitionGuard {
    gate: Arc<SessionOperationGate>,
}

impl ProviderTransitionGuard {
    pub(super) async fn wait_until_idle(&self) {
        loop {
            let changed = self.gate.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self
                .gate
                .state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .active_run_id
                .is_none()
            {
                return;
            }
            changed.await;
        }
    }
}

impl Drop for ProviderTransitionGuard {
    fn drop(&mut self) {
        self.gate
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .provider_transition_pending = false;
        self.gate.changed.notify_waiters();
    }
}

impl SessionOperationGate {
    pub(super) async fn begin_prompt(
        self: &Arc<Self>,
        run_id: &str,
    ) -> Result<PromptOperationGuard, agent_client_protocol::Error> {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let mut state = self
                    .state
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                if !state.provider_transition_pending {
                    if let Some(active_run_id) = state.active_run_id.as_deref() {
                        return Err(agent_client_protocol::Error::invalid_params().data(format!(
                            "session already has active run `{active_run_id}`; use _gosling/unstable/session/steer"
                        )));
                    }
                    state.active_run_id = Some(run_id.to_string());
                    return Ok(PromptOperationGuard {
                        gate: Arc::clone(self),
                        run_id: run_id.to_string(),
                    });
                }
            }
            changed.await;
        }
    }

    pub(super) fn queue_provider_transition(
        self: &Arc<Self>,
        expected_active_run_id: Option<&str>,
        require_active_run_fence: bool,
    ) -> Result<ProviderTransitionGuard, agent_client_protocol::Error> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.provider_transition_pending {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("A provider or model transition is already queued for this session"));
        }
        if require_active_run_fence {
            let actual_run_id = state
                .active_run_id
                .as_deref()
                .or(state.last_completed_run_id.as_deref());
            if expected_active_run_id.is_some() && expected_active_run_id != actual_run_id {
                return Err(agent_client_protocol::Error::invalid_params().data(
                    serde_json::json!({
                        "message": "The active turn changed after the provider switch was previewed; review the checkpoint again",
                        "expectedActiveRunId": expected_active_run_id,
                        "actualActiveRunId": state.active_run_id.as_deref(),
                    }),
                ));
            }
            if state.active_run_id.is_some() && expected_active_run_id.is_none() {
                return Err(agent_client_protocol::Error::invalid_params().data(
                    serde_json::json!({
                        "message": "The active turn changed after the provider switch was previewed; review the checkpoint again",
                        "expectedActiveRunId": expected_active_run_id,
                        "actualActiveRunId": state.active_run_id.as_deref(),
                    }),
                ));
            }
        }
        state.provider_transition_pending = true;
        Ok(ProviderTransitionGuard {
            gate: Arc::clone(self),
        })
    }

    pub(super) fn active_run_id(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .active_run_id
            .clone()
    }
}

pub(super) struct ActivePromptRun {
    run_id: String,
    cancel_token: CancellationToken,
    operation_guard: PromptOperationGuard,
}

impl ActivePromptRun {
    pub(super) fn was_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}

pub(super) async fn register_active_prompt_run(
    active_prompt_runs: &Mutex<HashMap<String, ActivePromptRun>>,
    agent_manager: &AgentManager,
    session_id: &str,
    run_id: String,
    cancel_token: CancellationToken,
    operation_guard: PromptOperationGuard,
) -> Result<(), agent_client_protocol::Error> {
    {
        let active_prompt_runs = active_prompt_runs.lock().await;
        if let Some(active_run) = active_prompt_runs.get(session_id) {
            return Err(agent_client_protocol::Error::invalid_params().data(format!(
                "session already has active run `{}`; use _gosling/unstable/session/steer",
                active_run.run_id.as_str()
            )));
        }
    }

    agent_manager
        .try_register_cancel_token(session_id, cancel_token.clone())
        .await
        .map_err(|error| agent_client_protocol::Error::invalid_params().data(error.to_string()))?;

    active_prompt_runs.lock().await.insert(
        session_id.to_string(),
        ActivePromptRun {
            run_id,
            cancel_token,
            operation_guard,
        },
    );
    Ok(())
}

/// Removes and returns the matching active run, preserving its operation guard
/// until terminal prompt bookkeeping has completed.
pub(super) async fn unregister_active_prompt_run(
    active_prompt_runs: &Mutex<HashMap<String, ActivePromptRun>>,
    agent_manager: &AgentManager,
    session_id: &str,
    run_id: &str,
) -> Option<ActivePromptRun> {
    let active_run = {
        let mut active_prompt_runs = active_prompt_runs.lock().await;
        let active_run = active_prompt_runs.get(session_id)?;
        if active_run.run_id != run_id {
            return None;
        }
        active_prompt_runs.remove(session_id)
    };
    agent_manager.unregister_cancel_token(session_id).await;
    active_run
}

impl GoslingAcpAgent {
    pub(super) async fn start_active_run(
        &self,
        session_id: &str,
        run_id: String,
        cancel_token: CancellationToken,
    ) -> Result<(), agent_client_protocol::Error> {
        if self.closed_session_ids.lock().await.contains(session_id) {
            return Err(agent_client_protocol::Error::resource_not_found(Some(
                session_id.to_string(),
            ))
            .data(format!("Session not found: {}", session_id)));
        }

        let operation_gate = self.session_operation_gate(session_id).await?;
        let operation_guard = operation_gate.begin_prompt(&run_id).await?;
        if self.closed_session_ids.lock().await.contains(session_id) {
            return Err(agent_client_protocol::Error::resource_not_found(Some(
                session_id.to_string(),
            ))
            .data(format!("Session not found: {}", session_id)));
        }

        register_active_prompt_run(
            &self.active_prompt_runs,
            &self.agent_manager,
            session_id,
            run_id,
            cancel_token,
            operation_guard,
        )
        .await
    }

    pub(super) async fn clear_active_run(
        &self,
        session_id: &str,
        run_id: &str,
    ) -> Option<PromptOperationGuard> {
        let active_run = unregister_active_prompt_run(
            &self.active_prompt_runs,
            &self.agent_manager,
            session_id,
            run_id,
        )
        .await?;
        let was_cancelled = active_run.was_cancelled();

        // A steer queued for this run only belongs to a *future* run when the
        // run it targeted was explicitly cancelled — an uncancelled run drains
        // its own pending steers before it lets itself finish (see
        // `reply_stream.rs`'s `has_pending_steers` checks), so anything left
        // here on a normal completion is, at worst, a narrow race that should
        // still reach the user on the next turn rather than vanish silently.
        if was_cancelled {
            let agent = {
                let sessions = self.sessions.lock().await;
                sessions
                    .get(session_id)
                    .map(|session| session.agent.clone())
            };
            if let Some(agent) = agent {
                agent.discard_pending_steers(session_id).await;
            }
        }

        if self.closed_session_ids.lock().await.contains(session_id) {
            self.sessions.lock().await.remove(session_id);
            if let Err(error) = self
                .agent_manager
                .remove_session_if_loaded(session_id)
                .await
            {
                warn!(
                    session_id,
                    %error,
                    "Failed to remove in-memory agent for closed session"
                );
            }
        }
        Some(active_run.operation_guard)
    }

    async fn session_operation_gate(
        &self,
        session_id: &str,
    ) -> Result<Arc<SessionOperationGate>, agent_client_protocol::Error> {
        self.sessions
            .lock()
            .await
            .get(session_id)
            .map(|session| Arc::clone(&session.operation_gate))
            .ok_or_else(|| {
                agent_client_protocol::Error::resource_not_found(Some(session_id.to_string()))
                    .data(format!("Session not found: {session_id}"))
            })
    }

    pub(super) async fn queue_provider_transition(
        &self,
        session_id: &str,
        expected_active_run_id: Option<&str>,
        require_active_run_fence: bool,
    ) -> Result<ProviderTransitionGuard, agent_client_protocol::Error> {
        self.session_operation_gate(session_id)
            .await?
            .queue_provider_transition(expected_active_run_id, require_active_run_fence)
    }

    pub(super) async fn active_run_id(
        &self,
        session_id: &str,
    ) -> Result<Option<String>, agent_client_protocol::Error> {
        Ok(self
            .session_operation_gate(session_id)
            .await?
            .active_run_id())
    }

    pub(super) async fn require_active_run(
        &self,
        session_id: &str,
        expected_run_id: &str,
    ) -> Result<String, agent_client_protocol::Error> {
        if expected_run_id.is_empty() {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("expectedRunId must not be empty"));
        }

        let active_prompt_runs = self.active_prompt_runs.lock().await;
        let active_run = active_prompt_runs.get(session_id).ok_or_else(|| {
            agent_client_protocol::Error::invalid_params().data("no active run to steer")
        })?;
        if active_run.run_id != expected_run_id {
            return Err(
                agent_client_protocol::Error::invalid_params().data(serde_json::json!({
                    "message": format!(
                        "expected active run id `{expected_run_id}` but found `{}`",
                        active_run.run_id.as_str()
                    ),
                    "expectedRunId": expected_run_id,
                    "actualRunId": active_run.run_id.as_str(),
                })),
            );
        }
        Ok(active_run.run_id.clone())
    }

    fn active_run_meta(active_run_id: Option<&str>) -> Meta {
        let mut gosling = serde_json::Map::new();
        gosling.insert(
            "activeRunId".to_string(),
            active_run_id
                .map(|run_id| serde_json::Value::String(run_id.to_string()))
                .unwrap_or(serde_json::Value::Null),
        );

        let mut meta = serde_json::Map::new();
        meta.insert("gosling".to_string(), serde_json::Value::Object(gosling));
        meta
    }

    pub(super) fn send_active_run_update(
        cx: &ConnectionTo<Client>,
        session_id: &SessionId,
        active_run_id: Option<&str>,
    ) -> Result<(), agent_client_protocol::Error> {
        cx.send_notification(SessionNotification::new(
            session_id.clone(),
            SessionUpdate::SessionInfoUpdate(
                SessionInfoUpdate::new().meta(Self::active_run_meta(active_run_id)),
            ),
        ))
    }

    fn send_queued_steer_update(
        cx: &ConnectionTo<Client>,
        session_id: &SessionId,
        message_id: &str,
        run_id: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        let mut gosling = serde_json::Map::new();
        gosling.insert(
            "queuedSteer".to_string(),
            serde_json::json!({
                "messageId": message_id,
                "runId": run_id,
            }),
        );
        let mut meta = serde_json::Map::new();
        meta.insert("gosling".to_string(), serde_json::Value::Object(gosling));

        cx.send_notification(SessionNotification::new(
            session_id.clone(),
            SessionUpdate::SessionInfoUpdate(SessionInfoUpdate::new().meta(meta)),
        ))
    }

    pub(super) async fn on_steer_session(
        &self,
        req: SteerSessionRequest,
    ) -> Result<SteerSessionResponse, agent_client_protocol::Error> {
        if req.prompt.is_empty() {
            return Err(
                agent_client_protocol::Error::invalid_params().data("prompt must not be empty")
            );
        }

        self.require_active_run(&req.session_id, &req.expected_run_id)
            .await?;
        let agent = self.get_session_agent(&req.session_id).await?;
        let active_run_id = self
            .require_active_run(&req.session_id, &req.expected_run_id)
            .await?;

        let message = Self::convert_acp_prompt_to_message(&req.prompt);
        if message.content.is_empty() {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("prompt must contain steerable content"));
        }

        let message_id = format!("steer_{}", Uuid::new_v4());
        let message = message.with_id(message_id.clone());
        agent.steer(&req.session_id, message).await;

        if let Some(cx) = self.client_cx.get() {
            let _ = Self::send_queued_steer_update(
                cx,
                &SessionId::new(req.session_id.clone()),
                &message_id,
                &active_run_id,
            );
        }

        Ok(SteerSessionResponse {
            run_id: active_run_id,
            message_id,
        })
    }

    pub(super) async fn on_cancel(
        &self,
        args: CancelNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        debug!(?args, "cancel request");

        let session_id = args.session_id.0.to_string();
        let token = {
            let active_prompt_runs = self.active_prompt_runs.lock().await;
            active_prompt_runs
                .get(&session_id)
                .map(|active_run| active_run.cancel_token.clone())
        };

        if let Some(token) = token {
            info!(session_id = %session_id, "prompt cancelled");
            token.cancel();
        } else if !self.sessions.lock().await.contains_key(&session_id) {
            warn!(session_id = %session_id, "cancel request for unknown session");
        }

        Ok(())
    }

    pub(super) async fn on_close_session(
        &self,
        session_id: &str,
    ) -> Result<CloseSessionResponse, agent_client_protocol::Error> {
        self.closed_session_ids
            .lock()
            .await
            .insert(session_id.to_string());

        let active_run_token = {
            let active_prompt_runs = self.active_prompt_runs.lock().await;
            active_prompt_runs
                .get(session_id)
                .map(|active_run| active_run.cancel_token.clone())
        };

        if let Some(token) = active_run_token {
            token.cancel();
        }

        let mut sessions = self.sessions.lock().await;
        sessions.remove(session_id);
        drop(sessions);

        self.agent_manager
            .remove_session_if_loaded(session_id)
            .await
            .internal_err_ctx("Failed to remove in-memory agent")?;

        info!(session_id = %session_id, "ACP session closed");
        Ok(CloseSessionResponse::new())
    }
}
