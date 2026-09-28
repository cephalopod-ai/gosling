//! Repetition protection through the real reply, inspection and dispatch path
//! (GSL-PT-20260927-E02): failures are remembered for the user turn they
//! happened in, and a decline is not a failure.

use async_trait::async_trait;
use futures::StreamExt;
use gosling::agents::mcp_client::{Error as McpError, McpClientTrait};
use gosling::agents::{
    Agent, AgentConfig, AgentEvent, ExtensionConfig, GoslingPlatform, SessionConfig,
};
use gosling::config::permission::PermissionManager;
use gosling::config::GoslingMode;
use gosling::conversation::message::{ActionRequiredData, Message, MessageContent, ToolResponse};
use gosling::permission::permission_confirmation::PrincipalType;
use gosling::permission::{Permission, PermissionConfirmation};
use gosling::providers::base::{stream_from_single_message, MessageStream, Provider};
use gosling::session::session_manager::SessionType;
use gosling::session::SessionManager;
use gosling_providers::conversation::token_usage::{ProviderUsage, Usage};
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, InitializeResult, JsonObject, ListToolsResult,
    Tool,
};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

const CHECK_TOOL: &str = "fixture__run_check";
const TURN_BUDGET: u32 = 20;

/// Fails its first `failures` calls, then passes, like a test suite that is
/// fixed in between runs. Counts every call that reached the extension.
struct CheckFixture {
    failures: usize,
    calls: Arc<AtomicUsize>,
}

#[async_trait]
impl McpClientTrait for CheckFixture {
    async fn list_tools(
        &self,
        _session_id: &str,
        _next_cursor: Option<String>,
        _cancellation_token: CancellationToken,
    ) -> Result<ListToolsResult, McpError> {
        let schema =
            serde_json::json!({"type": "object", "properties": {"command": {"type": "string"}}});
        Ok(ListToolsResult {
            tools: vec![Tool::new(
                "run_check",
                "Run the project check",
                Arc::new(schema.as_object().unwrap().clone()),
            )],
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        _ctx: &gosling::agents::ToolCallContext,
        _name: &str,
        _arguments: Option<JsonObject>,
        _cancellation_token: CancellationToken,
    ) -> Result<CallToolResult, McpError> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        Ok(if call <= self.failures {
            CallToolResult::error(vec![Content::text("CHECK-FAILED")])
        } else {
            CallToolResult::success(vec![Content::text("CHECK-PASSED")])
        })
    }

    fn get_info(&self) -> Option<&InitializeResult> {
        None
    }
}

/// One step per provider request: `true` calls the check with identical
/// arguments, `false` answers. Past the script it keeps calling the check.
struct ScriptedModel {
    script: Vec<bool>,
    requests: Arc<AtomicUsize>,
}

#[async_trait]
impl Provider for ScriptedModel {
    async fn stream(
        &self,
        _model_config: &ModelConfig,
        _system_prompt: &str,
        _messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        let request = self.requests.fetch_add(1, Ordering::SeqCst);
        let message = if self.script.get(request).copied().unwrap_or(true) {
            Message::assistant().with_tool_request(
                format!("check-{request}"),
                Ok(CallToolRequestParams::new(CHECK_TOOL)
                    .with_arguments(rmcp::object!({ "command": "cargo test" }))),
            )
        } else {
            Message::assistant().with_text("done")
        };
        Ok(stream_from_single_message(
            message,
            ProviderUsage::new("mock-model".to_string(), Usage::default()),
        ))
    }

    fn get_name(&self) -> &str {
        "repetition-scripted"
    }
}

struct Harness {
    agent: Agent,
    session_id: String,
    tool_calls: Arc<AtomicUsize>,
    provider_requests: Arc<AtomicUsize>,
    _root: tempfile::TempDir,
}

struct TurnOutcome {
    messages: Vec<Message>,
    approval_prompts: usize,
}

impl TurnOutcome {
    fn tool_results(&self) -> Vec<&ToolResponse> {
        self.messages
            .iter()
            .flat_map(|message| &message.content)
            .filter_map(MessageContent::as_tool_response)
            .collect()
    }
}

fn result_text(response: &ToolResponse) -> String {
    match &response.tool_result {
        Ok(result) => result
            .content
            .iter()
            .filter_map(|content| content.as_text().map(|text| text.text.clone()))
            .collect(),
        Err(error) => error.message.to_string(),
    }
}

impl Harness {
    async fn new(mode: GoslingMode, failures: usize, script: Vec<bool>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let sessions = Arc::new(SessionManager::new(root.path().join("data")));
        let permissions = Arc::new(PermissionManager::new(root.path().join("config")));
        let session = sessions
            .create_session(
                root.path().to_path_buf(),
                "repetition".to_string(),
                SessionType::User,
                mode,
            )
            .await
            .unwrap();
        let agent = Agent::with_config(AgentConfig::new(
            sessions,
            permissions,
            mode,
            true,
            GoslingPlatform::GoslingCli,
        ));
        let tool_calls = Arc::new(AtomicUsize::new(0));
        agent
            .extension_manager
            .add_client(
                "fixture".to_string(),
                ExtensionConfig::Builtin {
                    name: "fixture".to_string(),
                    display_name: None,
                    description: "check fixture".to_string(),
                    timeout: None,
                    bundled: None,
                    available_tools: Vec::new(),
                },
                Arc::new(CheckFixture {
                    failures,
                    calls: tool_calls.clone(),
                }),
                None,
                None,
            )
            .await;
        let provider_requests = Arc::new(AtomicUsize::new(0));
        agent
            .update_provider(
                Arc::new(ScriptedModel {
                    script,
                    requests: provider_requests.clone(),
                }),
                ModelConfig::new("mock-model"),
                &session.id,
            )
            .await
            .unwrap();
        Self {
            agent,
            session_id: session.id,
            tool_calls,
            provider_requests,
            _root: root,
        }
    }

    /// Runs one user turn, answering approval prompts with `decisions` in order.
    async fn turn(&self, mut decisions: VecDeque<Permission>) -> TurnOutcome {
        let reply = self
            .agent
            .reply(
                Message::user().with_text("run the check"),
                SessionConfig {
                    id: self.session_id.clone(),
                    max_turns: Some(TURN_BUDGET),
                    compacted_context: false,
                    tail_limit: None,
                },
                None,
            )
            .await
            .unwrap();
        tokio::pin!(reply);
        let mut outcome = TurnOutcome {
            messages: Vec::new(),
            approval_prompts: 0,
        };
        while let Some(event) = reply.next().await {
            let AgentEvent::Message(message) = event.unwrap() else {
                continue;
            };
            for content in &message.content {
                if let MessageContent::ActionRequired(action) = content {
                    if let ActionRequiredData::ToolConfirmation { id, .. } = &action.data {
                        outcome.approval_prompts += 1;
                        let permission = decisions.pop_front().unwrap_or(Permission::DenyOnce);
                        self.agent
                            .handle_confirmation(
                                id.clone(),
                                PermissionConfirmation {
                                    principal_type: PrincipalType::Tool,
                                    permission,
                                },
                            )
                            .await;
                    }
                }
            }
            outcome.messages.push(message);
        }
        outcome
    }

    fn tool_calls(&self) -> usize {
        self.tool_calls.load(Ordering::SeqCst)
    }

    fn provider_requests(&self) -> usize {
        self.provider_requests.load(Ordering::SeqCst)
    }
}

#[tokio::test]
async fn a_call_that_failed_in_an_earlier_turn_runs_again_in_the_next_turn() {
    let harness = Harness::new(GoslingMode::Auto, 1, vec![true, false, true, false]).await;

    let first = harness.turn(VecDeque::new()).await;
    assert!(result_text(first.tool_results()[0]).contains("CHECK-FAILED"));

    let second = harness.turn(VecDeque::new()).await;

    assert_eq!(harness.tool_calls(), 2);
    assert_eq!(harness.provider_requests(), 4);
    assert!(
        result_text(second.tool_results()[0]).contains("CHECK-PASSED"),
        "{}",
        result_text(second.tool_results()[0])
    );
}

#[tokio::test]
async fn a_declined_call_is_asked_again_instead_of_being_denied_as_failed() {
    let harness = Harness::new(GoslingMode::Approve, 0, vec![true, true, false]).await;

    let outcome = harness
        .turn(VecDeque::from([
            Permission::DenyOnce,
            Permission::AllowOnce,
        ]))
        .await;

    assert_eq!(outcome.approval_prompts, 2);
    assert_eq!(harness.tool_calls(), 1);
    assert!(result_text(outcome.tool_results()[1]).contains("CHECK-PASSED"));
}
