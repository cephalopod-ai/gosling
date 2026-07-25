use crate::agents::extension::PlatformExtensionContext;
use crate::agents::mcp_client::{Error, McpClientTrait};
use crate::agents::tool_execution::ToolCallContext;
use crate::session::session_manager::SessionType;
use anyhow::Result;
use async_trait::async_trait;
use indoc::indoc;
use rmcp::model::{
    CallToolResult, Content, Implementation, InitializeResult, JsonObject, ListToolsResult,
    ServerCapabilities, Tool, ToolAnnotations,
};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

pub static EXTENSION_NAME: &str = "chatrecall";

fn truncate_recall_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{truncated}\n[…truncated; search again for another hit if more detail is needed]")
    } else {
        truncated
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ChatRecallParams {
    /// Search keywords. Use distinctive terms from the remembered discussion. Mutually exclusive with session_id.
    #[serde(skip_serializing_if = "Option::is_none")]
    query: Option<String>,
    /// Session ID to load. Combine with message_id to hydrate context around an exact search hit.
    #[serde(skip_serializing_if = "Option::is_none")]
    session_id: Option<String>,
    /// Exact message ID returned by search. Requires session_id; returns a bounded context window.
    #[serde(skip_serializing_if = "Option::is_none")]
    message_id: Option<String>,
    /// Max results (default: 8, max: 20). Search mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    limit: Option<i64>,
    /// ISO 8601 date (e.g., '2025-10-01T00:00:00Z'). Search mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    after_date: Option<String>,
    /// ISO 8601 date (e.g., '2025-10-15T23:59:59Z'). Search mode only.
    #[serde(skip_serializing_if = "Option::is_none")]
    before_date: Option<String>,
}

pub struct ChatRecallClient {
    info: InitializeResult,
    context: PlatformExtensionContext,
}

impl ChatRecallClient {
    pub fn new(context: PlatformExtensionContext) -> Result<Self> {
        let info = InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new(EXTENSION_NAME.to_string(), "1.0.0".to_string())
                    .with_title("Chat Recall"),
            )
            .with_instructions(indoc! {r#"
                Chat Recall

                Search past conversations and hydrate only the relevant context when the user expects some memory or context.

                Two modes:
                - Search mode: Use query to find relevance-ranked, bounded text snippets
                - Context mode: Use the returned session_id and message_id to get a small window around that exact hit
            "#}.to_string());

        Ok(Self { info, context })
    }

    fn search_session_types(&self) -> Vec<SessionType> {
        match self.context.session.as_ref().map(|s| s.session_type) {
            Some(SessionType::Acp) => vec![SessionType::Acp],
            _ => vec![SessionType::User, SessionType::Scheduled],
        }
    }

    #[allow(clippy::too_many_lines)]
    async fn handle_chatrecall(
        &self,
        current_session_id: &str,
        arguments: Option<JsonObject>,
    ) -> Result<Vec<Content>, String> {
        let arguments = arguments.ok_or("Missing arguments")?;

        let target_session_id = arguments
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let target_message_id = arguments
            .get("message_id")
            .and_then(|v| v.as_str())
            .map(str::to_owned);

        if let Some(sid) = target_session_id {
            // CONTEXT MODE: Load a bounded window rather than an entire
            // transcript. The old first/last sampling missed the useful middle
            // of long chats and could hand the model megabytes of text.
            match self.context.session_manager.get_session(&sid, false).await {
                Ok(loaded_session) => {
                    let (msgs, context_label) = if let Some(message_id) = &target_message_id {
                        (
                            self.context
                                .session_manager
                                .get_session_message_window(&sid, message_id, 3, 3)
                                .await
                                .map_err(|e| format!("Failed to load message context: {e}"))?,
                            format!("Context around message {message_id}"),
                        )
                    } else {
                        let page = self
                            .context
                            .session_manager
                            .get_session_tail_page(&sid, 6)
                            .await
                            .map_err(|e| format!("Failed to load session tail: {e}"))?;
                        (page.messages, "Most recent message context".to_string())
                    };

                    if msgs.is_empty() {
                        return Ok(vec![Content::text(format!(
                            "Session {} has no matching messages.",
                            sid
                        ))]);
                    }

                    let mut output = format!(
                        "Session: {} (ID: {})\nWorking Dir: {}\n{} ({} message(s))\n\n",
                        loaded_session.name,
                        sid,
                        loaded_session.working_dir.display(),
                        context_label,
                        msgs.len(),
                    );
                    for (idx, msg) in msgs.iter().enumerate() {
                        output.push_str(&format!("{}. [{:?}]\n", idx + 1, msg.role));
                        let text = msg
                            .content
                            .iter()
                            .filter_map(|content| content.as_text())
                            .collect::<Vec<_>>()
                            .join("\n");
                        output.push_str(&truncate_recall_text(&text, 2_000));
                        output.push_str("\n\n");
                    }

                    Ok(vec![Content::text(output)])
                }
                Err(e) => Err(format!("Failed to load session: {}", e)),
            }
        } else {
            // SEARCH MODE: Search across all sessions
            let query = arguments
                .get("query")
                .and_then(|v| v.as_str())
                .ok_or("Missing required parameter: query or session_id")?
                .to_string();

            let limit = arguments
                .get("limit")
                .and_then(|v| v.as_i64())
                .map(|l| l as usize)
                .unwrap_or(8)
                .clamp(1, 20);

            let after_date = arguments
                .get("after_date")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            let before_date = arguments
                .get("before_date")
                .and_then(|v| v.as_str())
                .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                .map(|dt| dt.with_timezone(&chrono::Utc));

            let exclude_session_id = Some(current_session_id.to_string());

            match self
                .context
                .session_manager
                .search_chat_history(
                    &query,
                    Some(limit),
                    after_date,
                    before_date,
                    exclude_session_id,
                    self.search_session_types(),
                )
                .await
            {
                Ok(results) => {
                    let formatted_results = if results.total_matches == 0 {
                        format!("No results found for query: '{}'", query)
                    } else {
                        let mut output = format!(
                            "Found {} matching message(s) across {} session(s) for query: '{}'\n\n",
                            results.total_matches,
                            results.results.len(),
                            query
                        );
                        for (idx, result) in results.results.iter().enumerate() {
                            output.push_str(&format!(
                                "{}. Session: {} (ID: {})\n   Working Dir: {}\n   Last Activity: {}\n   {} matching hit(s); {} total message(s). Use this session ID plus a hit's message ID to load bounded context.\n\n",
                                idx + 1,
                                result.session_description,
                                result.session_id,
                                result.session_working_dir,
                                result.last_activity.format("%Y-%m-%d"),
                                result.messages.len(),
                                result.total_messages_in_session
                            ));

                            for (msg_idx, message) in result.messages.iter().enumerate() {
                                output.push_str(&format!(
                                    "   {}.{} [{}]\n   {}\n\n",
                                    idx + 1,
                                    msg_idx + 1,
                                    message.role,
                                    format!(
                                        "Message ID: {}\n{}",
                                        message.message_id.as_deref().unwrap_or("unavailable"),
                                        message.content
                                    )
                                    .lines()
                                    .map(|line| format!("   {}", line))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                                ));
                            }
                        }
                        output
                    };
                    Ok(vec![Content::text(formatted_results)])
                }
                Err(e) => Err(format!("Chat recall failed: {}", e)),
            }
        }
    }

    fn get_tools() -> Vec<Tool> {
        let schema = schema_for!(ChatRecallParams);
        let schema_value =
            serde_json::to_value(schema).expect("Failed to serialize ChatRecallParams schema");

        let input_schema = schema_value
            .as_object()
            .expect("Schema should be an object")
            .clone();

        vec![Tool::new(
            "chatrecall".to_string(),
            indoc! {r#"
                Search past chat, then load bounded context around an exact hit. Use when it is clear the user expects memory or context.

                search mode (query): Returns relevance-ranked text snippets grouped by session. Supports date filters.
                context mode (session_id + message_id): Returns up to three text messages before and after that hit. Session-only mode returns a six-message tail.
            "#}
            .to_string(),
            input_schema,
        )
        .annotate(ToolAnnotations::from_raw(
            Some("Recall past conversations".to_string()),
            Some(true),
            Some(false),
            Some(true),
            Some(false),
        ))]
    }
}

#[async_trait]
impl McpClientTrait for ChatRecallClient {
    async fn list_tools(
        &self,
        _session_id: &str,
        _next_cursor: Option<String>,
        _cancellation_token: CancellationToken,
    ) -> Result<ListToolsResult, Error> {
        Ok(ListToolsResult {
            tools: Self::get_tools(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        ctx: &ToolCallContext,
        name: &str,
        arguments: Option<JsonObject>,
        _cancellation_token: CancellationToken,
    ) -> Result<CallToolResult, Error> {
        let session_id = &ctx.session_id;
        let content = match name {
            "chatrecall" => self.handle_chatrecall(session_id, arguments).await,
            _ => Err(format!("Unknown tool: {}", name)),
        };

        match content {
            Ok(content) => Ok(CallToolResult::success(content)),
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
