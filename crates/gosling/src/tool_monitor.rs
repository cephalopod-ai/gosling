use crate::config::GoslingMode;
use crate::conversation::message::{Message, ToolRequest};
use crate::tool_inspection::{InspectionAction, InspectionResult, ToolInspector};
use anyhow::Result;
use async_trait::async_trait;
use rmcp::model::CallToolRequestParams;
use serde_json::Value;
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
    state: Mutex<RepetitionState>,
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
            state: Mutex::new(RepetitionState::default()),
        }
    }

    pub fn check_tool_call(&self, tool_call: CallToolRequestParams) -> bool {
        let mut state = self.state.lock().unwrap();
        self.record_tool_call(&mut state, &tool_call)
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
        *self.state.get_mut().unwrap() = RepetitionState::default();
    }
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
        _session_id: &str,
        tool_requests: &[ToolRequest],
        _messages: &[Message],
        _gosling_mode: GoslingMode,
    ) -> Result<Vec<InspectionResult>> {
        let mut results = Vec::new();
        let mut state = self.state.lock().unwrap();

        for tool_request in tool_requests {
            if let Ok(tool_call) = &tool_request.tool_call {
                if !self.record_tool_call(&mut state, tool_call) {
                    results.push(InspectionResult {
                        tool_request_id: tool_request.id.clone(),
                        action: InspectionAction::Deny,
                        reason: format!(
                            "Tool '{}' has exceeded maximum repetitions",
                            tool_call.name
                        ),
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
}
