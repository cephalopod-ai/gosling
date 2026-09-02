//! Agent lifecycle hooks, tool hooks, and queued steering.
//!
//! Maintainers: preserve hook ordering, denial behavior, and steer fencing here.
//! Clients: lifecycle notifications and steer semantics remain stable.

use super::*;

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
                    if hook_manager.has_hooks(ext_event) {
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
