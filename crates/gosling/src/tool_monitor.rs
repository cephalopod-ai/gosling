use crate::config::GoslingMode;
use crate::conversation::message::{Message, MessageContent, ToolRequest};
use crate::tool_inspection::{InspectionAction, InspectionResult, ToolInspector};
use anyhow::Result;
use async_trait::async_trait;
use rmcp::model::CallToolRequestParams;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Mutex;

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
}

fn failed_tool_calls(messages: &[Message]) -> Vec<InternalToolCall> {
    let mut requests = HashMap::new();
    let mut failed = Vec::new();
    for message in messages {
        for content in &message.content {
            match content {
                MessageContent::ToolRequest(request) => {
                    if let Ok(tool_call) = &request.tool_call {
                        requests.insert(
                            request.id.as_str(),
                            InternalToolCall::from_tool_call(tool_call),
                        );
                    }
                }
                MessageContent::ToolResponse(response) => {
                    let response_failed = match &response.tool_result {
                        Err(_) => true,
                        Ok(result) => result.is_error == Some(true),
                    };
                    if response_failed {
                        if let Some(request) = requests.get(response.id.as_str()) {
                            failed.push(request.clone());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    failed
}

#[async_trait]
impl ToolInspector for RepetitionInspector {
    fn name(&self) -> &'static str {
        "repetition"
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
        let failed_calls = failed_tool_calls(messages);
        let mut states = self.states.lock().unwrap();
        let state = states.entry(session_id.to_string()).or_default();

        for tool_request in tool_requests {
            if let Ok(tool_call) = &tool_request.tool_call {
                let current = InternalToolCall::from_tool_call(tool_call);
                let repeated_failure = failed_calls.iter().any(|failed| failed.matches(&current));
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
                        inspector_name: "repetition".to_string(),
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
    use crate::tool_inspection::ToolInspector;
    use rmcp::model::{ErrorCode, ErrorData};
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

    #[tokio::test]
    async fn live_inspector_denies_an_identical_failed_call_but_allows_a_correction() {
        let inspector = RepetitionInspector::new(Some(3));
        let failed_call = CallToolRequestParams::new("Markdown")
            .with_arguments(object!({ "selector": "main", "source": "invalid" }));
        let failure = ErrorData::new(ErrorCode::INVALID_PARAMS, "unexpected field `source`", None);
        let messages = vec![
            Message::assistant().with_tool_request("failed", Ok(failed_call.clone())),
            Message::user().with_tool_response("failed", Err(failure)),
        ];

        let repeated = ToolRequest {
            id: "repeat".into(),
            tool_call: Ok(failed_call),
            metadata: None,
            tool_meta: None,
        };
        let denied = inspector
            .inspect("session", &[repeated], &messages, GoslingMode::Auto)
            .await
            .unwrap();
        assert_eq!(denied.len(), 1);
        assert!(denied[0].reason.contains("already failed"));

        assert!(inspector
            .inspect(
                "session",
                &[request("corrected", 2)],
                &messages,
                GoslingMode::Auto,
            )
            .await
            .unwrap()
            .is_empty());
    }
}
