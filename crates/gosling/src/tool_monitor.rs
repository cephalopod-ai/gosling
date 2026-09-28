use crate::agents::DECLINED_RESPONSE;
use crate::config::GoslingMode;
use crate::conversation::message::{Message, MessageContent, ToolRequest, ToolResponse};
use crate::tool_inspection::{InspectionAction, InspectionResult, ToolInspector};
use anyhow::Result;
use async_trait::async_trait;
use rmcp::model::CallToolRequestParams;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

const REPETITION_INSPECTOR_NAME: &str = "repetition";

// Helper struct for internal tracking
#[derive(Debug, Clone)]
struct InternalToolCall {
    name: String,
    parameters: Value,
}

impl InternalToolCall {
    fn matches(&self, other: &InternalToolCall) -> bool {
        self.name == other.name && self.parameters == other.parameters
    }

    fn from_tool_call(tool_call: &CallToolRequestParams) -> Self {
        let name = tool_call.name.to_string();
        let parameters = tool_call
            .arguments
            .as_ref()
            .map(|obj| Value::Object(obj.clone()))
            .unwrap_or(Value::Null);
        Self { name, parameters }
    }
}

#[derive(Debug)]
pub struct RepetitionInspector {
    max_repetitions: Option<u32>,
    states: Mutex<HashMap<String, RepetitionState>>,
}

#[derive(Debug, Default)]
struct RepetitionState {
    last_call: Option<InternalToolCall>,
    repeat_count: u32,
    /// Calls inspected this turn whose result has not been seen yet, by request id.
    awaiting_result: HashMap<String, InternalToolCall>,
    /// Calls that failed earlier in this turn.
    failed_calls: Vec<InternalToolCall>,
}

impl RepetitionState {
    /// A call's result is appended before the next inspection, so the newest
    /// response to its request id is that call's result even when a provider
    /// reuses tool-call ids.
    fn record_results(&mut self, messages: &[Message]) {
        let responses = messages
            .iter()
            .rev()
            .flat_map(|message| message.content.iter().rev())
            .filter_map(MessageContent::as_tool_response);
        for response in responses {
            if self.awaiting_result.is_empty() {
                break;
            }
            if let Some(call) = self.awaiting_result.remove(&response.id) {
                if tool_call_failed(response) {
                    self.failed_calls.push(call);
                }
            }
        }
    }
}

impl RepetitionInspector {
    pub fn new(max_repetitions: Option<u32>) -> Self {
        Self {
            max_repetitions,
            states: Mutex::new(HashMap::new()),
        }
    }

    pub fn check_tool_call(&self, tool_call: CallToolRequestParams) -> bool {
        let mut states = self.states.lock().unwrap();
        let state = states.entry("direct".into()).or_default();
        self.record_tool_call(state, &tool_call)
    }

    fn record_tool_call(
        &self,
        state: &mut RepetitionState,
        tool_call: &CallToolRequestParams,
    ) -> bool {
        let internal_call = InternalToolCall::from_tool_call(tool_call);

        if self.max_repetitions.is_none() {
            state.last_call = Some(internal_call);
            state.repeat_count = 1;
            return true;
        }

        if let Some(last) = &state.last_call {
            if last.matches(&internal_call) {
                state.repeat_count += 1;
                if state.repeat_count > self.max_repetitions.unwrap() {
                    return false;
                }
            } else {
                state.repeat_count = 1;
            }
        } else {
            state.repeat_count = 1;
        }

        state.last_call = Some(internal_call);
        true
    }

    pub fn reset(&mut self) {
        self.states.get_mut().unwrap().clear();
    }

    /// Repetition protection guards one autonomous run. A new user turn starts
    /// clean, so the user can have a call retried that failed or repeated in an
    /// earlier turn, e.g. after fixing what made it fail.
    pub fn start_turn(&self, session_id: &str) {
        self.states.lock().unwrap().remove(session_id);
    }
}

fn tool_call_failed(response: &ToolResponse) -> bool {
    match &response.tool_result {
        Err(_) => true,
        // A declined call never ran, so asking again must reach the user.
        Ok(result) => {
            result.is_error == Some(true)
                && !result.content.iter().any(|content| {
                    content
                        .as_text()
                        .is_some_and(|text| text.text == DECLINED_RESPONSE)
                })
        }
    }
}

#[async_trait]
impl ToolInspector for RepetitionInspector {
    fn name(&self) -> &'static str {
        REPETITION_INSPECTOR_NAME
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    async fn inspect(
        &self,
        session_id: &str,
        tool_requests: &[ToolRequest],
        messages: &[Message],
        _gosling_mode: GoslingMode,
    ) -> Result<Vec<InspectionResult>> {
        let mut results = Vec::new();
        let mut states = self.states.lock().unwrap();
        let state = states.entry(session_id.to_string()).or_default();
        state.record_results(messages);

        for tool_request in tool_requests {
            if let Ok(tool_call) = &tool_request.tool_call {
                let current = InternalToolCall::from_tool_call(tool_call);
                let repeated_failure = state
                    .failed_calls
                    .iter()
                    .any(|failed| failed.matches(&current));
                state
                    .awaiting_result
                    .insert(tool_request.id.clone(), current);
                if repeated_failure || !self.record_tool_call(state, tool_call) {
                    results.push(InspectionResult {
                        tool_request_id: tool_request.id.clone(),
                        action: InspectionAction::Deny,
                        reason: if repeated_failure {
                            format!(
                                "Tool '{}' already failed with identical arguments",
                                tool_call.name
                            )
                        } else {
                            format!("Tool '{}' has exceeded maximum repetitions", tool_call.name)
                        },
                        confidence: 1.0,
                        inspector_name: REPETITION_INSPECTOR_NAME.to_string(),
                        finding_id: Some("REP-001".to_string()),
                        metadata: None,
                    });
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::message::ToolRequest;
    use crate::mcp_utils::ToolResult;
    use crate::tool_inspection::ToolInspector;
    use rmcp::model::{CallToolResult, Content, ErrorCode, ErrorData};
    use rmcp::object;

    fn request(id: &str, value: u32) -> ToolRequest {
        ToolRequest {
            id: id.into(),
            tool_call: Ok(CallToolRequestParams::new("Markdown")
                .with_arguments(object!({ "selector": value }))),
            metadata: None,
            tool_meta: None,
        }
    }

    #[tokio::test]
    async fn live_inspector_denies_fourth_identical_call_and_resets_after_change() {
        let inspector = RepetitionInspector::new(Some(3));
        for id in ["one", "two", "three"] {
            assert!(inspector
                .inspect("session", &[request(id, 1)], &[], GoslingMode::Auto)
                .await
                .unwrap()
                .is_empty());
        }

        let denied = inspector
            .inspect("session", &[request("four", 1)], &[], GoslingMode::Auto)
            .await
            .unwrap();
        assert_eq!(denied.len(), 1);
        assert_eq!(denied[0].action, InspectionAction::Deny);
        assert_eq!(denied[0].finding_id.as_deref(), Some("REP-001"));

        assert!(inspector
            .inspect("session", &[request("changed", 2)], &[], GoslingMode::Auto)
            .await
            .unwrap()
            .is_empty());
    }

    fn failing_request(id: &str) -> ToolRequest {
        ToolRequest {
            id: id.into(),
            tool_call: Ok(CallToolRequestParams::new("Markdown")
                .with_arguments(object!({ "selector": "main", "source": "invalid" }))),
            metadata: None,
            tool_meta: None,
        }
    }

    fn answered(request: &ToolRequest, result: ToolResult<CallToolResult>) -> Vec<Message> {
        vec![
            Message::assistant().with_tool_request(request.id.clone(), request.tool_call.clone()),
            Message::user().with_tool_response(request.id.clone(), result),
        ]
    }

    fn invalid_params() -> ToolResult<CallToolResult> {
        Err(ErrorData::new(
            ErrorCode::INVALID_PARAMS,
            "unexpected field `source`",
            None,
        ))
    }

    async fn denials(
        inspector: &RepetitionInspector,
        request: ToolRequest,
        messages: &[Message],
    ) -> Vec<InspectionResult> {
        inspector
            .inspect("session", &[request], messages, GoslingMode::Auto)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn live_inspector_denies_an_identical_failed_call_but_allows_a_correction() {
        let inspector = RepetitionInspector::new(Some(3));
        let failed = failing_request("failed");
        assert!(denials(&inspector, failed.clone(), &[]).await.is_empty());
        let messages = answered(&failed, invalid_params());

        let denied = denials(&inspector, failing_request("repeat"), &messages).await;
        assert_eq!(denied.len(), 1);
        assert!(denied[0].reason.contains("already failed"));

        assert!(denials(&inspector, request("corrected", 2), &messages)
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn a_failure_is_remembered_only_for_the_turn_it_happened_in() {
        let inspector = RepetitionInspector::new(Some(3));
        let failed = failing_request("failed");
        assert!(denials(&inspector, failed.clone(), &[]).await.is_empty());
        let messages = answered(&failed, invalid_params());

        inspector.start_turn("session");

        assert!(denials(&inspector, failing_request("next-turn"), &messages)
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn a_declined_call_is_asked_again_rather_than_denied_as_failed() {
        let inspector = RepetitionInspector::new(Some(3));
        let declined = failing_request("declined");
        assert!(denials(&inspector, declined.clone(), &[]).await.is_empty());
        let messages = answered(
            &declined,
            Ok(CallToolResult::error(vec![Content::text(
                DECLINED_RESPONSE,
            )])),
        );

        assert!(
            denials(&inspector, failing_request("asked-again"), &messages)
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_new_turn_restarts_the_consecutive_repetition_count() {
        let inspector = RepetitionInspector::new(Some(3));
        for id in ["one", "two", "three"] {
            assert!(denials(&inspector, request(id, 1), &[]).await.is_empty());
        }

        inspector.start_turn("session");

        assert!(denials(&inspector, request("next-turn", 1), &[])
            .await
            .is_empty());
    }

    #[tokio::test]
    async fn a_reused_tool_call_id_keeps_each_call_paired_with_its_own_result() {
        let inspector = RepetitionInspector::new(Some(3));
        let failed = failing_request("reused");
        assert!(denials(&inspector, failed.clone(), &[]).await.is_empty());
        let mut messages = answered(&failed, invalid_params());
        let succeeded = request("reused", 2);
        assert!(denials(&inspector, succeeded.clone(), &messages)
            .await
            .is_empty());
        messages.extend(answered(
            &succeeded,
            Ok(CallToolResult::success(vec![Content::text("ok")])),
        ));

        let denied = denials(&inspector, failing_request("again"), &messages).await;
        assert_eq!(denied.len(), 1);
        assert!(denied[0].reason.contains("already failed"));
        assert!(denials(&inspector, request("reused-ok", 2), &messages)
            .await
            .is_empty());
    }
}
