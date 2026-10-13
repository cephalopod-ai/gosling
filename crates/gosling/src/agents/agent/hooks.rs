//! Agent lifecycle hooks, tool hooks, and queued steering.
//!
//! Maintainers: preserve hook ordering, denial behavior, and steer fencing here.
//! Clients: lifecycle notifications and steer semantics remain stable.

use super::*;

pub(super) struct SteerWindowGuard {
    sessions: Arc<std::sync::Mutex<HashSet<String>>>,
    session_id: String,
}

impl Drop for SteerWindowGuard {
    fn drop(&mut self) {
        self.sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(&self.session_id);
    }
}

async fn hooks_allowed(
    session_manager: &crate::session::SessionManager,
    session_id: &str,
    event: crate::hooks::HookEvent,
) -> bool {
    match session_manager.plans().interaction_policy(session_id).await {
        Ok(crate::session::InteractionPolicy::Normal) => true,
        Ok(crate::session::InteractionPolicy::Planning { .. }) => {
            tracing::warn!(
                security.event_type = "planning_side_channel_denied",
                security.reason = "planning_lifecycle_hook_denied",
                hook.event = %event,
                session.id = session_id,
                "host planning boundary suppressed a command-backed lifecycle hook"
            );
            false
        }
        Err(error) => {
            tracing::warn!(
                security.event_type = "planning_side_channel_denied",
                security.reason = "planning_state_unavailable",
                hook.event = %event,
                session.id = session_id,
                error = %error,
                "host planning boundary could not verify durable state for a lifecycle hook"
            );
            false
        }
    }
}

impl Agent {
    /// Emit a lifecycle hook event with no extra context. Useful for events
    /// that have no matcher (e.g. `SessionStart`, `SessionEnd`).
    #[cfg(test)]
    pub(crate) fn set_hook_manager_for_test(&mut self, hook_manager: crate::hooks::HookManager) {
        self.hook_manager = hook_manager;
    }

    #[cfg(test)]
    pub(crate) fn set_stop_hook_block_cap_for_test(&mut self, cap: u32) {
        self.stop_hook_block_cap_override = Some(cap);
    }

    pub(super) fn stop_hook_block_cap(&self) -> u32 {
        #[cfg(test)]
        if let Some(cap) = self.stop_hook_block_cap_override {
            return cap;
        }

        Config::global()
            .get_param::<u32>("GOSLING_STOP_HOOK_BLOCK_CAP")
            .unwrap_or(DEFAULT_STOP_HOOK_BLOCK_CAP)
    }

    pub async fn emit_hook(&self, event: crate::hooks::HookEvent, session_id: &str) {
        if !self.hook_manager.has_hooks(event) {
            return;
        }
        if !hooks_allowed(&self.config.session_manager, session_id, event).await {
            return;
        }
        self.hook_manager
            .emit(event, crate::hooks::HookContext::new(event, session_id))
            .await;
    }

    fn stop_hook_context(
        session_id: &str,
        last_assistant_message: &str,
    ) -> crate::hooks::HookContext {
        crate::hooks::HookContext::new(crate::hooks::HookEvent::Stop, session_id)
            .with_last_assistant_message(last_assistant_message.to_string())
    }

    pub(super) async fn emit_stop_hook(&self, session_id: &str, last_assistant_message: &str) {
        if !self.hook_manager.has_hooks(crate::hooks::HookEvent::Stop) {
            return;
        }
        if !hooks_allowed(
            &self.config.session_manager,
            session_id,
            crate::hooks::HookEvent::Stop,
        )
        .await
        {
            return;
        }
        self.hook_manager
            .emit(
                crate::hooks::HookEvent::Stop,
                Self::stop_hook_context(session_id, last_assistant_message),
            )
            .await;
    }

    pub(super) async fn emit_stop_hook_blocking(
        &self,
        session_id: &str,
        last_assistant_message: &str,
    ) -> crate::hooks::HookDecision {
        if !self.hook_manager.has_hooks(crate::hooks::HookEvent::Stop) {
            return crate::hooks::HookDecision::Allow;
        }
        if !hooks_allowed(
            &self.config.session_manager,
            session_id,
            crate::hooks::HookEvent::Stop,
        )
        .await
        {
            return crate::hooks::HookDecision::Allow;
        }
        self.hook_manager
            .emit_blocking(
                crate::hooks::HookEvent::Stop,
                Self::stop_hook_context(session_id, last_assistant_message),
            )
            .await
    }

    pub async fn steer(&self, session_id: &str, message: Message) {
        self.pending_steers
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .push_back(message);
        self.steer_arrived.notify_one();
    }

    pub(super) fn begin_steer_window(&self, session_id: &str) -> SteerWindowGuard {
        self.accepting_steer_sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(session_id.to_owned());
        SteerWindowGuard {
            sessions: self.accepting_steer_sessions.clone(),
            session_id: session_id.to_owned(),
        }
    }

    pub(crate) async fn try_steer_active_reply(&self, session_id: &str, message: Message) -> bool {
        let mut pending = self.pending_steers.lock().await;
        let accepting = self
            .accepting_steer_sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if !accepting.contains(session_id) {
            return false;
        }
        pending
            .entry(session_id.to_owned())
            .or_default()
            .push_back(message);
        drop(accepting);
        self.steer_arrived.notify_one();
        true
    }

    pub(super) async fn close_steer_window_if_empty(&self, session_id: &str) -> bool {
        let pending = self.pending_steers.lock().await;
        if pending
            .get(session_id)
            .is_some_and(|messages| !messages.is_empty())
        {
            return false;
        }
        self.accepting_steer_sessions
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(session_id);
        true
    }

    /// Hands queued steers to a provider stream that takes input mid-turn.
    /// The first one it refuses goes back to the front of the queue with
    /// everything behind it, so the between-calls drain keeps their order.
    pub(super) async fn forward_pending_steers(
        &self,
        session_id: &str,
        provider: &dyn Provider,
    ) -> Vec<Message> {
        let mut forwarded = Vec::new();
        let mut steers = self.drain_pending_steers(session_id).await.into_iter();
        while let Some(message) = steers.next() {
            if !provider.deliver_mid_turn_message(&message).await {
                let mut pending = self.pending_steers.lock().await;
                let queue = pending.entry(session_id.to_string()).or_default();
                for refused in std::iter::once(message).chain(steers).rev() {
                    queue.push_front(refused);
                }
                break;
            }
            forwarded.push(message);
        }
        forwarded
    }

    pub(super) async fn emit_steer_prompt_hook(&self, session_id: &str, message: &Message) {
        if !self
            .hook_manager
            .has_hooks(crate::hooks::HookEvent::UserPromptSubmit)
        {
            return;
        }
        if !hooks_allowed(
            &self.config.session_manager,
            session_id,
            crate::hooks::HookEvent::UserPromptSubmit,
        )
        .await
        {
            return;
        }
        let ctx =
            crate::hooks::HookContext::new(crate::hooks::HookEvent::UserPromptSubmit, session_id)
                .with_message(message.as_concat_text());
        self.hook_manager
            .emit(crate::hooks::HookEvent::UserPromptSubmit, ctx)
            .await;
    }

    pub async fn discard_pending_steers(&self, session_id: &str) {
        self.pending_steers.lock().await.remove(session_id);
    }

    pub(super) async fn has_pending_steers(&self, session_id: &str) -> bool {
        self.pending_steers
            .lock()
            .await
            .get(session_id)
            .is_some_and(|messages| !messages.is_empty())
    }

    pub(super) async fn drain_pending_steers(&self, session_id: &str) -> Vec<Message> {
        self.pending_steers
            .lock()
            .await
            .remove(session_id)
            .map(|messages| messages.into_iter().map(Message::with_steer).collect())
            .unwrap_or_default()
    }

    pub(super) async fn emit_pre_tool_extended_hooks(
        &self,
        tool_name: &str,
        tool_input: Option<&Value>,
        session: &Session,
    ) {
        let working_dir = session.working_dir.to_string_lossy().to_string();
        match categorize_tool(tool_name) {
            ToolCategory::Shell => {
                if let Some(cmd) = tool_input.and_then(|v| extract_string_arg(v, &["command"])) {
                    self.emit_with_matcher(
                        crate::hooks::HookEvent::BeforeShellExecution,
                        &session.id,
                        &cmd,
                        tool_name,
                        tool_input.cloned(),
                        &working_dir,
                    )
                    .await;
                }
            }
            ToolCategory::Read => {
                if let Some(path) =
                    tool_input.and_then(|v| extract_string_arg(v, &["path", "file", "file_path"]))
                {
                    self.emit_with_matcher(
                        crate::hooks::HookEvent::BeforeReadFile,
                        &session.id,
                        &path,
                        tool_name,
                        tool_input.cloned(),
                        &working_dir,
                    )
                    .await;
                }
            }
            ToolCategory::Write | ToolCategory::Other => {}
        }
    }

    async fn emit_with_matcher(
        &self,
        event: crate::hooks::HookEvent,
        session_id: &str,
        matcher_context: &str,
        tool_name: &str,
        tool_input: Option<Value>,
        working_dir: &str,
    ) {
        if !self.hook_manager.has_hooks(event) {
            return;
        }
        if !hooks_allowed(&self.config.session_manager, session_id, event).await {
            return;
        }
        let mut ctx = crate::hooks::HookContext::new(event, session_id)
            .with_tool(tool_name.to_string(), tool_input)
            .with_working_dir(working_dir.to_string());
        ctx.matcher_context = Some(matcher_context.to_string());
        self.hook_manager.emit(event, ctx).await;
    }

    pub(super) fn with_post_tool_hook(
        &self,
        result: ToolCallResult,
        tool_call: &CallToolRequestParams,
        session: &Session,
    ) -> ToolCallResult {
        let hook_manager = self.hook_manager.clone();
        let session_manager = self.config.session_manager.clone();
        let session_id = session.id.clone();
        let working_dir = session.working_dir.to_string_lossy().to_string();
        let tool_name = tool_call.name.to_string();
        let tool_input = tool_call
            .arguments
            .as_ref()
            .map(|a| serde_json::Value::Object(a.clone()));
        let category = categorize_tool(&tool_name);

        let fut = async move {
            let processed_result =
                crate::agents::large_response_handler::process_tool_response(result.result.await);
            let event = match &processed_result {
                Ok(call_result) if call_result.is_error != Some(true) => {
                    crate::hooks::HookEvent::PostToolUse
                }
                _ => crate::hooks::HookEvent::PostToolUseFailure,
            };

            let has_extended_hook = match category {
                ToolCategory::Shell => hook_manager.has_hooks(crate::hooks::HookEvent::AfterShellExecution),
                ToolCategory::Write => hook_manager.has_hooks(crate::hooks::HookEvent::AfterFileEdit),
                _ => false,
            };
            if !hook_manager.has_hooks(event) && !has_extended_hook {
                return processed_result;
            }
            if !hooks_allowed(&session_manager, &session_id, event).await {
                return processed_result;
            }
            if hook_manager.has_hooks(event) {
                let ctx = crate::hooks::HookContext::new(event, &session_id)
                    .with_tool(tool_name.clone(), tool_input.clone())
                    .with_working_dir(working_dir.clone());
                hook_manager.emit(event, ctx).await;
            }

            if event == crate::hooks::HookEvent::PostToolUse {
                let extended = match category {
                    ToolCategory::Shell => Some((
                        crate::hooks::HookEvent::AfterShellExecution,
                        tool_input
                            .as_ref()
                            .and_then(|v| extract_string_arg(v, &["command"])),
                    )),
                    ToolCategory::Write => Some((
                        crate::hooks::HookEvent::AfterFileEdit,
                        tool_input
                            .as_ref()
                            .and_then(|v| extract_string_arg(v, &["path", "file", "file_path"])),
                    )),
                    _ => None,
                };
                if let Some((ext_event, Some(matcher))) = extended {
                    if hook_manager.has_hooks(ext_event)
                        && hooks_allowed(&session_manager, &session_id, ext_event).await
                    {
                        let mut ctx = crate::hooks::HookContext::new(ext_event, &session_id)
                            .with_tool(tool_name, tool_input)
                            .with_working_dir(working_dir);
                        ctx.matcher_context = Some(matcher);
                        hook_manager.emit(ext_event, ctx).await;
                    }
                }
            }

            processed_result
        };

        ToolCallResult {
            notification_stream: result.notification_stream,
            action_required_stream: result.action_required_stream,
            result: Box::new(fut.boxed()),
        }
    }
}
