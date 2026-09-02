//! Goal, prompt customization, extension prompt, and frontend-result APIs.
//!
//! Maintainers: keep public prompt controls and bounded frontend waits together.
//! Clients: goal state, prompt lookup, and frontend result behavior remain stable.

use super::*;

impl Agent {
    pub async fn extend_system_prompt(&self, key: String, instruction: String) {
        let mut prompt_manager = self.prompt_manager.lock().await;
        prompt_manager.add_system_prompt_extra(key, instruction);
    }

    pub async fn remove_system_prompt_extra(&self, key: &str) {
        let mut prompt_manager = self.prompt_manager.lock().await;
        prompt_manager.remove_system_prompt_extra(key);
    }

    pub async fn set_goal(&self, goal: Option<String>) {
        *self.goal.lock().await = goal;
    }

    pub async fn get_goal(&self) -> Option<String> {
        self.goal.lock().await.clone()
    }

    pub async fn set_grind(&self, goal: Option<String>) {
        *self.grind.lock().await = goal;
    }

    pub async fn get_grind(&self) -> Option<String> {
        self.grind.lock().await.clone()
    }

    /// Override the system prompt with a custom template
    pub async fn override_system_prompt(&self, template: String) {
        let mut prompt_manager = self.prompt_manager.lock().await;
        prompt_manager.set_system_prompt_override(template);
    }

    pub async fn configure_shell_instructions(&self, template: String) {
        let mut prompt_manager = self.prompt_manager.lock().await;
        prompt_manager.configure_shell_instructions(template);
    }

    pub async fn clear_system_prompt_override(&self) {
        let mut prompt_manager = self.prompt_manager.lock().await;
        prompt_manager.clear_system_prompt_override();
    }

    pub async fn list_extension_prompts(&self, session_id: &str) -> HashMap<String, Vec<Prompt>> {
        // `list_prompts` never returns `Err` (per-extension failures are
        // collected and logged internally, not propagated) - `unwrap_or_default`
        // documents that contract instead of asserting a failure mode the
        // callee cannot produce (REL-GOS-003).
        self.extension_manager
            .list_prompts(session_id, CancellationToken::default())
            .await
            .unwrap_or_default()
    }

    pub async fn get_prompt(
        &self,
        session_id: &str,
        name: &str,
        arguments: Value,
    ) -> Result<GetPromptResult> {
        // First find which extension has this prompt
        let prompts = self
            .extension_manager
            .list_prompts(session_id, CancellationToken::default())
            .await
            .map_err(|e| anyhow!("Failed to list prompts: {}", e))?;

        if let Some(extension) = prompts
            .iter()
            .find(|(_, prompt_list)| prompt_list.iter().any(|p| p.name == name))
            .map(|(extension, _)| extension)
        {
            return self
                .extension_manager
                .get_prompt(
                    session_id,
                    extension,
                    name,
                    arguments,
                    CancellationToken::default(),
                )
                .await
                .map_err(|e| anyhow!("Failed to get prompt: {}", e));
        }

        Err(anyhow!("Prompt '{}' not found", name))
    }

    pub async fn get_plan_prompt(&self, session_id: &str) -> Result<String> {
        let tools = self
            .extension_manager
            .get_prefixed_tools(session_id, None)
            .await?;
        let tools_info = tools
            .into_iter()
            .map(|tool| {
                ToolInfo::new(
                    &tool.name,
                    tool.description
                        .as_ref()
                        .map(|d| d.as_ref())
                        .unwrap_or_default(),
                    get_parameter_names(&tool),
                    None,
                )
            })
            .collect();

        let plan_prompt = self.extension_manager.get_planning_prompt(tools_info).await;

        Ok(plan_prompt)
    }

    pub async fn handle_tool_result(&self, id: String, result: ToolResult<CallToolResult>) {
        self.frontend_tool_result_router.deliver(id, result).await;
    }

    /// Frontend tool calls can legitimately involve a human in the loop, so
    /// this mirrors the elicitation wait's 300s bound rather than a short
    /// per-request timeout.
    const FRONTEND_TOOL_RESULT_TIMEOUT: Duration = Duration::from_secs(300);

    pub(in crate::agents) async fn wait_for_frontend_tool_result(
        &self,
        request_id: String,
    ) -> Option<ToolResult<CallToolResult>> {
        match self
            .frontend_tool_result_router
            .register(request_id.clone())
            .await
        {
            FrontendToolResultRegistration::Ready(result) => Some(result),
            // A crashed/disconnected frontend never calls `deliver`, and
            // nothing else resolves this channel - without a bound, the whole
            // reply stream hangs forever (REL-GOS-002).
            FrontendToolResultRegistration::Pending(rx) => {
                match tokio::time::timeout(Self::FRONTEND_TOOL_RESULT_TIMEOUT, rx).await {
                    Ok(received) => received.ok(),
                    Err(_) => {
                        tracing::warn!(
                            request_id = %request_id,
                            timeout = ?Self::FRONTEND_TOOL_RESULT_TIMEOUT,
                            "Frontend tool result wait timed out",
                        );
                        None
                    }
                }
            }
        }
    }
}
