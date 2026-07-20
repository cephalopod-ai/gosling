use async_stream::try_stream;
use futures::stream::{self, BoxStream};
use futures::{Stream, StreamExt};
use rmcp::model::CallToolResult;
use std::collections::HashMap;
use std::future::Future;
use tokio_util::sync::CancellationToken;

use std::path::PathBuf;

use crate::config::permission::PermissionLevel;
use crate::conversation::message::Message;
use crate::mcp_utils::ToolResult;
use crate::permission::Permission;
use crate::permission::permission_confirmation::PrincipalType;
use rmcp::model::{Content, ServerNotification};

/// Context passed through the tool call dispatch chain.
#[derive(Clone)]
pub struct ToolCallContext {
    pub session_id: String,
    pub working_dir: Option<PathBuf>,
    pub tool_call_request_id: Option<String>,
    pub tool_operation_id: Option<String>,
}

impl ToolCallContext {
    pub fn new(
        session_id: String,
        working_dir: Option<PathBuf>,
        tool_call_request_id: Option<String>,
    ) -> Self {
        Self {
            session_id,
            working_dir,
            tool_call_request_id,
            tool_operation_id: None,
        }
    }

    pub fn with_tool_operation_id(mut self, tool_operation_id: String) -> Self {
        self.tool_operation_id = Some(tool_operation_id);
        self
    }

    pub fn working_dir_str(&self) -> Option<&str> {
        self.working_dir.as_ref().and_then(|p| p.to_str())
    }
}

// ToolCallResult combines the result of a tool call with an optional notification stream that
// can be used to receive notifications from the tool.
pub struct ToolCallResult {
    pub result: Box<dyn Future<Output = ToolResult<rmcp::model::CallToolResult>> + Send + Unpin>,
    pub notification_stream: Option<Box<dyn Stream<Item = ServerNotification> + Send + Unpin>>,
    pub action_required_stream: Option<Box<dyn Stream<Item = Message> + Send + Unpin>>,
}

impl From<ToolResult<rmcp::model::CallToolResult>> for ToolCallResult {
    fn from(result: ToolResult<rmcp::model::CallToolResult>) -> Self {
        Self {
            result: Box::new(futures::future::ready(result)),
            notification_stream: None,
            action_required_stream: None,
        }
    }
}

use super::agent::{tool_stream, ToolOperationGuard, ToolStream};
use crate::agents::Agent;
use crate::conversation::message::ToolRequest;
use crate::session::Session;
use crate::tool_inspection::get_security_finding_id_from_results;

pub const DECLINED_RESPONSE: &str = "The user has declined to run this tool. \
    DO NOT attempt to call this tool again. \
    If there are no alternative methods to proceed, clearly explain the situation and STOP.";

pub const SUBAGENT_APPROVAL_UNAVAILABLE_RESPONSE: &str =
    "This tool call was blocked by a safety inspector and cannot be escalated for approval \
    inside a delegated subagent. DO NOT attempt to call this tool again with the same or \
    equivalent arguments. If there are no alternative methods to proceed, clearly explain the \
    situation to the parent agent and STOP.";

pub const CHAT_MODE_TOOL_SKIPPED_RESPONSE: &str = "Let the user know the tool call was skipped in gosling chat mode. \
                                        DO NOT apologize for skipping the tool call. DO NOT say sorry. \
                                        Provide an explanation of what the tool call would do, structured as a \
                                        plan for the user. Again, DO NOT apologize. \
                                        **Example Plan:**\n \
                                        1. **Identify Task Scope** - Determine the purpose and expected outcome.\n \
                                        2. **Outline Steps** - Break down the steps.\n \
                                        If needed, adjust the explanation based on user preferences or questions.";

impl Agent {
    pub(crate) fn handle_approval_tool_requests<'a>(
        &'a self,
        tool_requests: &'a [ToolRequest],
        tool_futures: &'a mut Vec<(String, ToolStream)>,
        request_to_response_map: &'a mut HashMap<String, Message>,
        cancellation_token: Option<CancellationToken>,
        session: &'a Session,
        inspection_results: &'a [crate::tool_inspection::InspectionResult],
    ) -> BoxStream<'a, anyhow::Result<Message>> {
        try_stream! {
        for request in tool_requests.iter() {
            if let Ok(tool_call) = request.tool_call.clone() {
                let security_message = inspection_results.iter()
                    .find(|result| result.tool_request_id == request.id)
                    .and_then(|result| {
                        if let crate::tool_inspection::InspectionAction::RequireApproval(Some(message)) = &result.action {
                            Some(message.clone())
                        } else {
                            None
                        }
                    });

                let mut mode_changes = self.gosling_mode_changes.subscribe();
                let confirmation_rx = self.tool_confirmation_router.register(request.id.clone()).await;
                let auto_approve = self.gosling_mode().await == crate::config::GoslingMode::Auto
                    && security_message.is_none();

                let action_required_msg = Message::assistant()
                    .with_action_required(
                        request.id.clone(),
                        tool_call.name.to_string().clone(),
                        tool_call.arguments.clone().unwrap_or_default(),
                        security_message,
                    )
                    .user_only();
                if !auto_approve {
                    yield action_required_msg;
                }

                let confirmation = if auto_approve {
                    PermissionConfirmation {
                        principal_type: PrincipalType::Tool,
                        permission: Permission::AllowOnce,
                    }
                } else {
                    let mut confirmation_rx = confirmation_rx;
                    loop {
                        tokio::select! {
                            confirmation = &mut confirmation_rx => break confirmation
                                .map_err(|_| anyhow::anyhow!("Confirmation channel closed for request {}", request.id))?,
                            changed = mode_changes.changed(), if security_message.is_none() => {
                                if changed.is_ok() && *mode_changes.borrow() == crate::config::GoslingMode::Auto {
                                    break PermissionConfirmation {
                                        principal_type: PrincipalType::Tool,
                                        permission: Permission::AllowOnce,
                                    };
                                }
                            }
                        }
                    }
                };

                if let Some(finding_id) = get_security_finding_id_from_results(&request.id, inspection_results) {
                    let action = match confirmation.permission {
                        Permission::AllowOnce | Permission::AlwaysAllow => "ALLOW",
                        _ => "BLOCK",
                    };
                    tracing::info!(
                        monotonic_counter.gosling.prompt_injection_user_decisions = 1,
                        security.event_type = "user_decision",
                        security.action = action,
                        security.finding_id = %finding_id,
                        tool.request_id = %request.id,
                        user.decision = ?confirmation.permission,
                        "security finding: user decision"
                    );
                }

                if confirmation.permission == Permission::AllowOnce || confirmation.permission == Permission::AlwaysAllow {
                    let (req_id, tool_result) = self.dispatch_conversation_tool_call(tool_call.clone(), request.id.clone(), cancellation_token.clone(), session).await;

                    tool_futures.push((req_id, match tool_result {
                        Ok(result) => tool_stream(
                            result.notification_stream.unwrap_or_else(|| Box::new(stream::empty())),
                            result.action_required_stream.unwrap_or_else(|| Box::new(stream::empty())),
                            result.result,
                        ),
                        Err(e) => tool_stream(
                            Box::new(stream::empty()),
                            Box::new(stream::empty()),
                            futures::future::ready(Err(e)),
                        ),
                    }));

                    if confirmation.permission == Permission::AlwaysAllow {
                        self.tool_inspection_manager
                            .update_permission_manager(&tool_call.name, PermissionLevel::AlwaysAllow)
                            .await;
                    }
                } else {
                    if let Some(response) = request_to_response_map.get_mut(&request.id) {
                        response.add_tool_response_with_metadata(
                            request.id.clone(),
                            Ok(CallToolResult::error(vec![Content::text(DECLINED_RESPONSE)])),
                            request.metadata.as_ref(),
                        );
                    }

                    if confirmation.permission == Permission::AlwaysDeny {
                        self.tool_inspection_manager
                            .update_permission_manager(&tool_call.name, PermissionLevel::NeverAllow)
                            .await;
                    }
                }
            }
        }
    }.boxed()
    }

    pub(crate) fn handle_frontend_tool_request<'a>(
        &'a self,
        tool_request: &'a ToolRequest,
        message_tool_response: &'a mut Message,
        session: &'a Session,
    ) -> BoxStream<'a, anyhow::Result<Message>> {
        try_stream! {
                if let Ok(tool_call) = tool_request.tool_call.clone() {
                    if self.is_frontend_tool(&tool_call.name).await {
                        let expected_request_id = tool_request.id.clone();
                        let operation_id = match self
                            .config
                            .session_manager
                            .begin_tool_operation(
                                &session.id,
                                &expected_request_id,
                                &tool_call,
                                true,
                            )
                            .await?
                        {
                            crate::session::ToolOperationStart::Execute { operation_id } => operation_id,
                            crate::session::ToolOperationStart::Replay { result, .. } => {
                                message_tool_response.add_tool_response_with_metadata(
                                    expected_request_id.clone(),
                                    result,
                                    tool_request.metadata.as_ref(),
                                );
                                self.config.session_manager
                                    .persist_tool_operation_response(
                                        &session.id,
                                        &expected_request_id,
                                        message_tool_response,
                                    )
                                    .await?;
                                return;
                            }
                            crate::session::ToolOperationStart::InDoubt { operation_id } => {
                                Err::<String, anyhow::Error>(anyhow::anyhow!(
                                    "Frontend tool operation {operation_id} is in doubt and will not be dispatched again automatically"
                                ))?
                            }
                        };
                        let mut operation_guard = ToolOperationGuard::new(
                            self.config.session_manager.clone(),
                            operation_id.clone(),
                        );
                        yield Message::assistant().with_frontend_tool_request(
                            expected_request_id.clone(),
                            Ok(tool_call.clone())
                        );

                        if let Some(result) = self
                            .wait_for_frontend_tool_result(expected_request_id.clone())
                            .await
                        {
                            self.config.session_manager
                                .complete_tool_operation(&operation_id, &result)
                                .await?;
                            message_tool_response.add_tool_response_with_metadata(
                                expected_request_id.clone(),
                                result,
                                tool_request.metadata.as_ref(),
                            );
                            self.config.session_manager
                                .persist_tool_operation_response(
                                    &session.id,
                                    &expected_request_id,
                                    message_tool_response,
                                )
                                .await?;
                            operation_guard.disarm();
                        } else {
                            Err(anyhow::anyhow!(
                                "Frontend tool result channel closed after durable dispatch began; execution status is in doubt"
                            ))?;
                        }
                    }
            }
        }
        .boxed()
    }
}
