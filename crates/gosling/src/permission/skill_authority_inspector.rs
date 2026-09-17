use crate::agents::interaction_policy::public_name_may_be_verified_non_mutating;
use crate::config::GoslingMode;
use crate::conversation::message::{Message, ToolRequest};
use crate::session::SessionManager;
use crate::tool_inspection::{InspectionAction, InspectionResult, ToolInspector};
use anyhow::Result;
use async_trait::async_trait;
use std::sync::Arc;

/// Routes calls that exceed an admitted skill's authority ceiling to explicit
/// approval. The prompt is mandatory: Auto mode cannot downgrade it, a saved
/// tool-wide grant cannot satisfy it, and a delegated subagent turns it into a
/// denial. The tool-operation begin transaction re-checks the durable ceiling,
/// so this inspector decides only how a call is presented, not whether a call
/// without approval may start.
pub struct SkillAuthorityInspector {
    session_manager: Arc<SessionManager>,
}

impl SkillAuthorityInspector {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }
}

#[async_trait]
impl ToolInspector for SkillAuthorityInspector {
    fn name(&self) -> &'static str {
        "skill_authority"
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn auto_downgrades_require_approval(&self) -> bool {
        false
    }

    async fn inspect(
        &self,
        session_id: &str,
        tool_requests: &[ToolRequest],
        _messages: &[Message],
        gosling_mode: GoslingMode,
    ) -> Result<Vec<InspectionResult>> {
        if gosling_mode == GoslingMode::Chat {
            return Ok(Vec::new());
        }
        let active = self
            .session_manager
            .active_skill_ceiling(session_id)
            .await?;
        if !active.ceiling.is_restrictive() {
            return Ok(Vec::new());
        }
        let skills = active.describe_skills();
        Ok(tool_requests
            .iter()
            .filter_map(|request| {
                let tool_call = request.tool_call.as_ref().ok()?;
                if public_name_may_be_verified_non_mutating(&tool_call.name) {
                    return None;
                }
                Some(InspectionResult {
                    tool_request_id: request.id.clone(),
                    action: InspectionAction::RequireApproval(Some(format!(
                        "Admitted skill(s) {skills} limit this turn to read-only work. Gosling cannot verify that `{}` stays within that limit, so it needs your approval for this call.",
                        tool_call.name
                    ))),
                    reason: format!(
                        "outside the {} ceiling of admitted skill(s) {skills}",
                        active.ceiling.as_str()
                    ),
                    confidence: 1.0,
                    inspector_name: self.name().to_string(),
                    finding_id: None,
                    metadata: None,
                })
            })
            .collect())
    }
}
