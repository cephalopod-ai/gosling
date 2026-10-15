use crate::config::GoslingMode;
use crate::conversation::message::{Message, ToolRequest};
use crate::session::{client_tool_policy, SessionManager};
use crate::tool_inspection::{InspectionAction, InspectionResult, ToolInspector};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

pub struct ClientToolInspector(pub Arc<SessionManager>);

#[async_trait]
impl ToolInspector for ClientToolInspector {
    fn name(&self) -> &'static str {
        "client_tool_policy"
    }
    fn auto_downgrades_require_approval(&self) -> bool {
        false
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    async fn inspect(
        &self,
        session_id: &str,
        requests: &[ToolRequest],
        _messages: &[Message],
        _mode: GoslingMode,
    ) -> Result<Vec<InspectionResult>> {
        let session = self.0.get_session(session_id, false).await?;
        if !client_tool_policy::is_bounded(&session) {
            return Ok(Vec::new());
        }
        Ok(requests
            .iter()
            .map(|request| InspectionResult {
                tool_request_id: request.id.clone(),
                action: InspectionAction::RequireApproval(None),
                reason: "The client requires approval for this tool invocation".into(),
                confidence: 1.0,
                inspector_name: self.name().into(),
                finding_id: None,
                metadata: None,
            })
            .collect())
    }
}
