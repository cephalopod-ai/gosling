use crate::agents::extension::PlatformExtensionContext;
use crate::agents::mcp_client::{Error, McpClientTrait};
use crate::agents::tool_execution::ToolCallContext;
use crate::agents::{AgentEvent, SessionConfig};
use crate::config::{Config, ExtensionConfig};
use crate::context_mgmt::format_message_for_compacting;
use crate::conversation::message::{ActionRequiredData, Message};
use crate::execution::manager::AgentManager;
use crate::providers;
use crate::providers::base::Provider;
use crate::session::extension_data::{EnabledExtensionsState, ExtensionData, ExtensionState};
use crate::session::session_manager::SessionType;
use anyhow::Result;
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::StreamExt;
use rmcp::model::{
    CallToolResult, Content, Implementation, InitializeResult, JsonObject, ListToolsResult,
    ServerCapabilities, Tool,
};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

pub static EXTENSION_NAME: &str = "orchestrator";

struct CancelTokenGuard {
    manager: Arc<AgentManager>,
    session_id: String,
    disarmed: bool,
}

impl CancelTokenGuard {
    fn new(manager: Arc<AgentManager>, session_id: String) -> Self {
        Self {
            manager,
            session_id,
            disarmed: false,
        }
    }

    fn disarm(&mut self) {
        self.disarmed = true;
    }
}

impl Drop for CancelTokenGuard {
    fn drop(&mut self) {
        if !self.disarmed {
            let manager = self.manager.clone();
            let session_id = self.session_id.clone();
            tokio::spawn(async move {
                manager.unregister_cancel_token(&session_id).await;
            });
        }
    }
}

const DEFAULT_LIST_LIMIT: usize = 10;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ListSessionsParams {
    /// Filter by session type: "user", "sub_agent", "scheduled", "hidden", "terminal", "acp".
    /// If omitted, returns all session types.
    session_type: Option<String>,
    /// Maximum number of sessions to return (most recent first). Defaults to 10.
    last_n: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ViewSessionParams {
    /// The session ID to inspect
    session_id: String,
    /// How to view the conversation: "first_last" returns the first and last message,
    /// "summarize" calls the LLM to produce a summary. If omitted, returns first and last.
    mode: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct StartAgentParams {
    /// Working directory for the new agent session
    working_dir: String,
    /// Human-readable name for the session
    name: Option<String>,
    // TODO[POLISH-20260827-007]: Add a "model_tier" parameter (e.g. "fast" vs "normal") to let the orchestrator
    // choose between a fast/cheap model and the default one. For now we inherit the
    // orchestrator's own provider and model.
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct SendMessageParams {
    /// The session ID of the agent to send a message to
    session_id: String,
    /// The message text to send
    message: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct InterruptAgentParams {
    /// The session ID of the agent to interrupt
    session_id: String,
}

pub struct OrchestratorClient {
    info: InitializeResult,
    context: PlatformExtensionContext,
}

impl OrchestratorClient {
    pub fn new(context: PlatformExtensionContext) -> Result<Self> {
        let info = InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(EXTENSION_NAME, "1.0.0").with_title("Orchestrator"),
            )
            .with_instructions(
                "Manage agent sessions: list, view, start, send messages, and interrupt agents.",
            );

        Ok(Self { info, context })
    }

    async fn get_agent_manager(&self) -> Result<Arc<AgentManager>, String> {
        AgentManager::instance()
            .await
            .map_err(|e| format!("Failed to get agent manager: {}", e))
    }

    async fn get_provider(&self) -> Result<Arc<dyn Provider>, String> {
        let extension_manager = self
            .context
            .extension_manager
            .as_ref()
            .and_then(|weak| weak.upgrade())
            .ok_or("Extension manager not available")?;

        let provider_guard = extension_manager.get_provider().lock().await;
        provider_guard
            .as_ref()
            .cloned()
            .ok_or_else(|| "Provider not available".to_string())
    }

    async fn parent_model_config(
        &self,
        provider_name: &str,
    ) -> Result<gosling_providers::model::ModelConfig, String> {
        if let Some(session) = self.context.session.as_ref() {
            return self.context.model_config_for_session(&session.id).await;
        }

        let model_name = Config::global()
            .get_gosling_model()
            .map_err(|_| "Could not resolve model config: missing model".to_string())?;
        crate::model_config::model_config_from_user_config(provider_name, &model_name)
            .map_err(|e| format!("Could not resolve model config: {e}"))
    }

    fn parent_extensions(&self) -> Vec<ExtensionConfig> {
        let extension_data = self.context.session.as_ref().map(|s| &s.extension_data);
        EnabledExtensionsState::extensions_or_default(extension_data, Config::global())
    }

    async fn handle_list_sessions(
        &self,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, String> {
        let type_filter = arguments
            .as_ref()
            .and_then(|args| args.get("session_type"))
            .and_then(|v| v.as_str());

        let limit = arguments
            .as_ref()
            .and_then(|args| args.get("last_n"))
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_LIST_LIMIT);

        let manager = self.get_agent_manager().await?;

        let mut sessions = if let Some(type_str) = type_filter {
            let session_type: SessionType = type_str
                .parse()
                .map_err(|e| format!("Invalid session type '{}': {}", type_str, e))?;
            self.context
                .session_manager
                .list_sessions_by_types(&[session_type])
                .await
                .map_err(|e| format!("Failed to list sessions: {}", e))?
        } else {
            self.context
                .session_manager
                .list_sessions()
                .await
                .map_err(|e| format!("Failed to list sessions: {}", e))?
        };

        // Most recent first
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        let total = sessions.len();
        sessions.truncate(limit);

        if sessions.is_empty() {
            return Ok(CallToolResult::success(vec![Content::text(
                "No sessions found.",
            )]));
        }

        let active_ids = manager.list_active_session_ids().await;

        let mut lines = vec![format!(
            "Showing {} of {} session(s):\n",
            sessions.len(),
            total
        )];
        for session in &sessions {
            let is_loaded = active_ids.contains(&session.id);
            let is_busy = if is_loaded {
                manager.is_session_busy(&session.id).await
            } else {
                false
            };

            let status = if is_busy {
                "🔄 busy"
            } else if is_loaded {
                "✓ loaded"
            } else {
                "○ idle"
            };

            lines.push(format!(
                "- **{}** ({})\n  Type: {} | Status: {} | Messages: {} | Updated: {}",
                session.name,
                session.id,
                session.session_type,
                status,
                session.message_count,
                session.updated_at.format("%Y-%m-%d %H:%M"),
            ));
        }

        Ok(CallToolResult::success(vec![Content::text(
            lines.join("\n"),
        )]))
    }

    async fn handle_view_session(
        &self,
        session_id_for_llm: &str,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, String> {
        let args = arguments.ok_or("Missing arguments")?;
        let session_id = extract_string(&args, "session_id")?;
        let mode = args
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("first_last");

        let session = self
            .context
            .session_manager
            .get_session(&session_id, true)
            .await
            .map_err(|e| format!("Session '{}' not found: {}", session_id, e))?;

        let manager = self.get_agent_manager().await?;
        let is_busy = manager.is_session_busy(&session_id).await;

        let mut output = vec![format!(
            "# Session: {} ({})\n\nType: {} | Status: {} | Working dir: {}\nMessages: {} | Updated: {}\n",
            session.name,
            session.id,
            session.session_type,
            if is_busy { "🔄 busy" } else { "idle" },
            session.working_dir.display(),
            session.message_count,
            session.updated_at.format("%Y-%m-%d %H:%M"),
        )];

        match mode {
            "first_last" => {
                if let Some(conversation) = &session.conversation {
                    let messages = conversation.messages();
                    if messages.is_empty() {
                        output.push("No messages in this session.".to_string());
                    } else {
                        output.push("## First message\n".to_string());
                        output.push(format_message_for_compacting(&messages[0]));

                        if messages.len() > 1 {
                            output.push(format!("\n*({} messages omitted)*\n", messages.len() - 2));
                            output.push("## Last message\n".to_string());
                            output
                                .push(format_message_for_compacting(&messages[messages.len() - 1]));
                        }
                    }
                } else {
                    output.push("No messages in this session.".to_string());
                }
            }
            "summarize" => {
                if let Some(conversation) = &session.conversation {
                    let messages = conversation.messages();
                    if messages.is_empty() {
                        output.push("No messages to summarize.".to_string());
                    } else {
                        let summary = self
                            .summarize_conversation(session_id_for_llm, messages)
                            .await?;
                        output.push(format!("## Summary\n\n{}", summary));
                    }
                } else {
                    output.push("No messages to summarize.".to_string());
                }
            }
            other => {
                return Err(format!(
                    "Unknown mode '{}'. Use 'first_last' or 'summarize'.",
                    other
                ));
            }
        }

        Ok(CallToolResult::success(vec![Content::text(
            output.join("\n"),
        )]))
    }

    async fn summarize_conversation(
        &self,
        session_id: &str,
        messages: &[Message],
    ) -> Result<String, String> {
        let provider = self.get_provider().await?;

        let conversation_text = messages
            .iter()
            .filter(|m| m.is_agent_visible())
            .map(format_message_for_compacting)
            .collect::<Vec<_>>()
            .join("\n");

        let system =
            "You are a helpful assistant. Summarize the following conversation concisely, \
                       capturing the key topics, decisions, and current state. Be brief.";

        let user_message = Message::user().with_text(format!(
            "Summarize this conversation ({} messages):\n\n{}",
            messages.len(),
            conversation_text
        ));

        let model_config = self.parent_model_config(provider.get_name()).await?;
        let (response, _usage) = crate::model_config::complete_fast(
            provider.as_ref(),
            &model_config,
            session_id,
            system,
            &[user_message],
            &[],
        )
        .await
        .map_err(|e| format!("LLM summarization failed: {}", e))?;

        Ok(response
            .content
            .iter()
            .filter_map(|c| {
                if let crate::conversation::message::MessageContent::Text(t) = c {
                    Some(t.text.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n"))
    }

    async fn handle_start_agent(
        &self,
        parent_session_id: &str,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, String> {
        let args = arguments.ok_or("Missing arguments")?;
        let working_dir = extract_string(&args, "working_dir")?;
        let name = args
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("Orchestrated Agent")
            .to_string();

        let parent = self
            .context
            .session_manager
            .get_session(parent_session_id, false)
            .await
            .map_err(|e| e.to_string())?;
        if parent.session_type == SessionType::SubAgent {
            return Err("Subagents cannot start further agents".to_string());
        }
        super::summon::ensure_parent_allows_delegation(parent.gosling_mode)?;
        let path = super::summon::resolve_working_dir(&parent.working_dir, &working_dir)
            .map_err(|e| e.to_string())?;
        let authentication = crate::authentication::session_authentication(&parent.extension_data)
            .map_err(|e| e.to_string())?;
        if authentication.provider_disconnected {
            return Err("Parent provider authentication is disconnected".to_string());
        }
        let parent_provider = self.get_provider().await?;
        let mode = super::summon::delegate_mode(parent_provider.executes_tools_outside_gosling());
        let model_config = self
            .context
            .model_config_for_session(parent_session_id)
            .await?;
        let extensions = EnabledExtensionsState::extensions_or_default(
            Some(&parent.extension_data),
            Config::global(),
        );
        for extension in &extensions {
            if let Some(binding) = authentication.extensions.get(&extension.key()) {
                if binding.credential_namespace.is_some() && !binding.disconnected {
                    let destination =
                        crate::authentication::extension_destination(extension, &path)
                            .map_err(|e| e.to_string())?;
                    if binding.destination.as_deref() != Some(destination.as_str()) {
                        return Err(format!("Scoped authentication for {} does not authorize the child directory; use the parent's directory or configure a separate agent explicitly", extension.key()));
                    }
                }
            }
        }
        let mut extension_data = ExtensionData::new();
        extension_data.extension_states.insert(
            crate::authentication::SESSION_AUTHENTICATION_KEY.to_string(),
            serde_json::to_value(authentication).map_err(|e| e.to_string())?,
        );
        EnabledExtensionsState::new(extensions)
            .to_extension_data(&mut extension_data)
            .map_err(|e| e.to_string())?;
        extension_data.extension_states.insert(
            "output_agent.v1".to_string(),
            serde_json::json!({"parentSessionId": parent_session_id, "name": name}),
        );

        let session = self
            .context
            .session_manager
            .create_session(path, name.clone(), SessionType::SubAgent, mode)
            .await
            .map_err(|e| format!("Failed to create session: {}", e))?;
        let mut snapshot = self
            .context
            .session_manager
            .update(&session.id)
            .additional_working_dirs(parent.additional_working_dirs.clone())
            .restrict_tools_to_working_dirs(parent.restrict_tools_to_working_dirs)
            .workspace_context(parent.workspace_context.clone())
            .extension_data(extension_data)
            .provider_name(parent_provider.get_name())
            .model_config(model_config.clone());
        if let (Some(id), Some(name), Some(context)) = (
            &parent.workspace_id,
            &parent.workspace_name,
            &parent.workspace_context,
        ) {
            snapshot = snapshot.workspace_snapshot(
                id.clone(),
                name.clone(),
                parent.credential_profile_id.clone(),
                parent.credential_profile_name.clone(),
                parent.credential_binding_id.clone(),
                context.clone(),
            );
        } else if let (Some(id), Some(name)) = (
            &parent.credential_profile_id,
            &parent.credential_profile_name,
        ) {
            snapshot = snapshot.credential_profile_snapshot(id.clone(), name.clone());
        }
        snapshot.apply().await.map_err(|e| e.to_string())?;
        self.context
            .session_manager
            .inherit_skill_admissions(parent_session_id, &session.id)
            .await
            .map_err(|e| {
                format!("Failed to carry admitted skill restrictions to the new agent: {e}")
            })?;

        let manager = self.get_agent_manager().await?;
        let agent = manager
            .get_or_create_agent(session.id.clone())
            .await
            .map_err(|e| format!("Failed to create agent: {}", e))?;

        agent
            .provider()
            .await
            .map_err(|e| format!("Failed to restore provider for new agent: {e}"))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Started agent session '{}' with ID: {}\n\nUse send_message with this session_id to interact with it.",
            name, session.id
        ))]))
    }

    async fn handle_send_message(
        &self,
        parent_session_id: &str,
        parent_cancel: &CancellationToken,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, String> {
        let args = arguments.ok_or("Missing arguments")?;
        let session_id = extract_string(&args, "session_id")?;
        let message_text = extract_string(&args, "message")?;

        if session_id == parent_session_id {
            return Err("Cannot send a message to the orchestrator's own session".into());
        }

        let manager = self.get_agent_manager().await?;

        let agent = manager
            .get_or_create_agent(session_id.clone())
            .await
            .map_err(|e| format!("Failed to get agent for session '{}': {}", session_id, e))?;

        if agent.provider().await.is_err() {
            if let Ok(parent_provider) = self.get_provider().await {
                let extensions = self.parent_extensions();
                let model_config = self.parent_model_config(parent_provider.get_name()).await?;
                if let Ok(provider) =
                    providers::create(parent_provider.get_name(), extensions).await
                {
                    agent
                        .update_provider(provider, model_config, &session_id)
                        .await
                        .map_err(|e| format!("Failed to set provider: {}", e))?;
                }
            }
        }

        let cancel_token = CancellationToken::new();
        manager
            .try_register_cancel_token(&session_id, cancel_token.clone())
            .await
            .map_err(|_| {
                format!(
                    "Session '{}' is currently busy. Use interrupt_agent first, or wait.",
                    session_id
                )
            })?;

        let mut guard = CancelTokenGuard::new(manager.clone(), session_id.clone());

        let user_message = Message::user().with_text(&message_text);
        let session_config = SessionConfig {
            id: session_id.clone(),
            max_turns: None,
            compacted_context: false,
            tail_limit: None,
        };

        let mut stream = agent
            .reply(user_message, session_config, Some(cancel_token.clone()))
            .await
            .map_err(|e| format!("Failed to start reply: {}", e))?;

        let result =
            drain_managed_agent_reply(&session_id, &mut stream, &cancel_token, parent_cancel).await;

        drop(stream);
        guard.disarm();
        manager.unregister_cancel_token(&session_id).await;

        let response_parts = result?;

        if response_parts.is_empty() {
            Ok(CallToolResult::success(vec![Content::text(
                "Agent completed without producing text output.",
            )]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(format!(
                "## Response from session {}\n\n{}",
                session_id,
                response_parts.join("\n\n")
            ))]))
        }
    }

    async fn handle_interrupt_agent(
        &self,
        arguments: Option<JsonObject>,
    ) -> Result<CallToolResult, String> {
        let args = arguments.ok_or("Missing arguments")?;
        let session_id = extract_string(&args, "session_id")?;

        let manager = self.get_agent_manager().await?;

        manager
            .cancel_session(&session_id)
            .await
            .map_err(|e| format!("Failed to interrupt session '{}': {}", session_id, e))?;

        Ok(CallToolResult::success(vec![Content::text(format!(
            "Interrupted agent session '{}'.",
            session_id
        ))]))
    }
}

#[async_trait]
impl McpClientTrait for OrchestratorClient {
    async fn list_tools(
        &self,
        _session_id: &str,
        _next_cursor: Option<String>,
        _cancel_token: CancellationToken,
    ) -> Result<ListToolsResult, Error> {
        let tools = vec![
            Tool::new(
                "list_sessions".to_string(),
                "List agent sessions with their status (loaded, busy, idle). Returns the most recent 10 by default. Optionally filter by session type."
                    .to_string(),
                schema::<ListSessionsParams>(),
            ),
            Tool::new(
                "view_session".to_string(),
                "View a session's details and conversation. Mode 'first_last' (default) returns the first and last message. Mode 'summarize' calls the LLM to produce a conversation summary."
                    .to_string(),
                schema::<ViewSessionParams>(),
            ),
            Tool::new(
                "start_agent".to_string(),
                "Start a new agent session with its own working directory. Inherits the current provider and model. Returns a session_id for future interaction."
                    .to_string(),
                schema::<StartAgentParams>(),
            ),
            Tool::new(
                "send_message".to_string(),
                "Send a message to an existing agent session and get the response. Returns an error if the agent is currently busy."
                    .to_string(),
                schema::<SendMessageParams>(),
            ),
            Tool::new(
                "interrupt_agent".to_string(),
                "Interrupt a busy agent by cancelling its current operation."
                    .to_string(),
                schema::<InterruptAgentParams>(),
            ),
        ];

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        ctx: &ToolCallContext,
        name: &str,
        arguments: Option<JsonObject>,
        cancel_token: CancellationToken,
    ) -> Result<CallToolResult, Error> {
        let result = match name {
            "list_sessions" => self.handle_list_sessions(arguments).await,
            "view_session" => self.handle_view_session(&ctx.session_id, arguments).await,
            "start_agent" => self.handle_start_agent(&ctx.session_id, arguments).await,
            "send_message" => {
                self.handle_send_message(&ctx.session_id, &cancel_token, arguments)
                    .await
            }
            "interrupt_agent" => self.handle_interrupt_agent(arguments).await,
            _ => Err(format!("Unknown tool: {}", name)),
        };

        match result {
            Ok(result) => Ok(result),
            Err(error) => Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: {}",
                error
            ))])),
        }
    }

    fn get_info(&self) -> Option<&InitializeResult> {
        Some(&self.info)
    }
}

fn schema<T: JsonSchema>() -> JsonObject {
    let mut obj = serde_json::to_value(schema_for!(T))
        .map(|v| v.as_object().unwrap().clone())
        .expect("valid schema");
    obj.entry("properties")
        .or_insert_with(|| serde_json::json!({}));
    obj
}

fn extract_string(args: &JsonObject, key: &str) -> Result<String, String> {
    args.get(key)
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| format!("Missing or invalid '{}'", key))
}

fn approval_unavailable_error(session_id: &str, tool_name: &str) -> String {
    format!(
        "Agent session '{session_id}' needs human approval to run tool '{tool_name}', but \
        orchestrated agent sessions have no UI attached to answer approval requests. The call \
        was aborted instead of hanging. Grant the managed agent a permission mode that does not \
        require approval for this tool (e.g. auto) before delegating tasks that need it, or \
        perform the tool call yourself."
    )
}

// Drives a managed agent's reply stream to completion, collecting response text.
//
// Any `ActionRequired::ToolConfirmation` message means the managed agent's tool call is
// blocked waiting on `Agent::handle_confirmation`, which nothing will ever call for an
// orchestrator-managed session — there is no UI attached to it. Rather than let the stream
// hang forever on that unanswerable wait, cancel the managed agent's turn and fail closed
// with an actionable error.
async fn drain_managed_agent_reply<'a>(
    session_id: &str,
    stream: &mut BoxStream<'a, Result<AgentEvent>>,
    cancel_token: &CancellationToken,
    parent_cancel: &CancellationToken,
) -> Result<Vec<String>, String> {
    let mut response_parts: Vec<String> = Vec::new();

    loop {
        tokio::select! {
            _ = parent_cancel.cancelled() => {
                cancel_token.cancel();
                return Err("Cancelled by parent session".into());
            }
            event = stream.next() => {
                match event {
                    Some(Ok(AgentEvent::Message(msg))) => {
                        if let Some(action_required) =
                            msg.content.iter().find_map(|c| c.as_action_required())
                        {
                            if let ActionRequiredData::ToolConfirmation { tool_name, .. } =
                                &action_required.data
                            {
                                cancel_token.cancel();
                                return Err(approval_unavailable_error(session_id, tool_name));
                            }
                        }

                        let text = msg.as_concat_text();
                        if !text.is_empty() {
                            response_parts.push(text);
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => {
                        response_parts.push(format!("Error during agent processing: {}", e));
                        break;
                    }
                    None => break,
                }
            }
        }
    }

    Ok(response_parts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::stream;
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn start_agent_rejects_unapproved_parents_and_outside_directories() {
        use crate::config::GoslingMode;
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let data = tempfile::tempdir().unwrap();
        let manager = Arc::new(crate::session::SessionManager::new(
            data.path().to_path_buf(),
        ));
        let client = OrchestratorClient::new(PlatformExtensionContext {
            extension_manager: None,
            session_manager: Arc::clone(&manager),
            session: None,
            use_login_shell_path: false,
            code_execution_runtime: crate::config::CodeExecutionRuntime::Enabled,
        })
        .unwrap();
        let user = manager
            .create_session(
                root.path().to_path_buf(),
                "Parent".into(),
                SessionType::User,
                GoslingMode::SmartApprove,
            )
            .await
            .unwrap();
        let arguments = || {
            Some(
                serde_json::json!({"working_dir": "."})
                    .as_object()
                    .unwrap()
                    .clone(),
            )
        };
        let error = client
            .handle_start_agent(&user.id, arguments())
            .await
            .unwrap_err();
        assert!(error.contains("Autonomous"));
        let child = manager
            .create_session(
                root.path().to_path_buf(),
                "Child".into(),
                SessionType::SubAgent,
                GoslingMode::Auto,
            )
            .await
            .unwrap();
        assert!(client
            .handle_start_agent(&child.id, arguments())
            .await
            .unwrap_err()
            .contains("Subagents cannot"));
        manager
            .update(&user.id)
            .gosling_mode(GoslingMode::Auto)
            .apply()
            .await
            .unwrap();
        let arguments = Some(
            serde_json::json!({"working_dir": outside.path()})
                .as_object()
                .unwrap()
                .clone(),
        );
        assert!(client
            .handle_start_agent(&user.id, arguments)
            .await
            .unwrap_err()
            .contains("outside the parent"));
    }

    fn pending_forever_after<'a>(
        first: anyhow::Result<AgentEvent>,
    ) -> BoxStream<'a, Result<AgentEvent>> {
        stream::once(async move { first })
            .chain(stream::pending())
            .boxed()
    }

    // Reproduces ORCH-003: a managed agent's tool call needing approval used to hang
    // `drain_managed_agent_reply` forever, since nothing ever answers the confirmation
    // channel for an orchestrator-managed session. This stream mimics that exactly: it
    // yields the `ActionRequired` message the real approval flow would emit, then never
    // yields again (as `handle_approval_tool_requests` would, parked on `confirmation_rx`).
    #[tokio::test]
    async fn drain_managed_agent_reply_fails_closed_on_approval_request_instead_of_hanging() {
        let action_required_msg = Message::assistant().with_action_required(
            "req-1".to_string(),
            "developer__shell".to_string(),
            serde_json::Map::new(),
            None,
            None,
            None,
        );
        let mut fake_stream = pending_forever_after(Ok(AgentEvent::Message(action_required_msg)));

        let cancel_token = CancellationToken::new();
        let parent_cancel = CancellationToken::new();

        let result = timeout(
            Duration::from_secs(5),
            drain_managed_agent_reply("session-1", &mut fake_stream, &cancel_token, &parent_cancel),
        )
        .await
        .expect("drain_managed_agent_reply hung instead of failing closed");

        let err = result.expect_err("expected an Err when approval is required and unanswerable");
        assert!(
            err.contains("session-1"),
            "error should name the session: {err}"
        );
        assert!(
            err.contains("developer__shell"),
            "error should name the blocked tool: {err}"
        );
        assert!(
            cancel_token.is_cancelled(),
            "the managed agent's turn should be cancelled so it doesn't keep running"
        );
    }

    #[tokio::test]
    async fn drain_managed_agent_reply_returns_text_when_no_approval_needed() {
        let mut fake_stream = stream::iter(vec![
            Ok(AgentEvent::Message(Message::assistant().with_text("hello"))),
            Ok(AgentEvent::Message(Message::assistant().with_text("world"))),
        ])
        .boxed();

        let cancel_token = CancellationToken::new();
        let parent_cancel = CancellationToken::new();

        let result = timeout(
            Duration::from_secs(5),
            drain_managed_agent_reply("session-1", &mut fake_stream, &cancel_token, &parent_cancel),
        )
        .await
        .expect("drain_managed_agent_reply should not hang on a normal stream");

        assert_eq!(result, Ok(vec!["hello".to_string(), "world".to_string()]));
        assert!(
            !cancel_token.is_cancelled(),
            "the normal path must not cancel the managed agent's turn"
        );
    }

    #[tokio::test]
    async fn drain_managed_agent_reply_honors_parent_cancellation() {
        let mut fake_stream: BoxStream<'_, Result<AgentEvent>> = stream::pending().boxed();

        let cancel_token = CancellationToken::new();
        let parent_cancel = CancellationToken::new();
        parent_cancel.cancel();

        let result = timeout(
            Duration::from_secs(5),
            drain_managed_agent_reply("session-1", &mut fake_stream, &cancel_token, &parent_cancel),
        )
        .await
        .expect("drain_managed_agent_reply should not hang when the parent cancels");

        assert_eq!(result, Err("Cancelled by parent session".to_string()));
        assert!(cancel_token.is_cancelled());
    }

    #[test]
    fn approval_unavailable_error_names_session_and_tool() {
        let err = approval_unavailable_error("session-42", "developer__shell");
        assert!(err.contains("session-42"));
        assert!(err.contains("developer__shell"));
        assert!(err.contains("approval"));
    }
}
