use anyhow::{anyhow, Result};
use async_trait::async_trait;
use gosling::config::GoslingMode;
use gosling::conversation::message::{Message, ToolRequest};
use gosling::tool_inspection::{
    security_prompt_for_request, single_flagged_domain_for_request, InspectionAction,
    InspectionResult, ToolInspectionManager, ToolInspector,
};

struct MockInspectorOk {
    name: &'static str,
    results: Vec<InspectionResult>,
}

struct MockInspectorErr {
    name: &'static str,
}

#[async_trait]
impl ToolInspector for MockInspectorOk {
    fn name(&self) -> &'static str {
        self.name
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    async fn inspect(
        &self,
        _session_id: &str,
        _tool_requests: &[ToolRequest],
        _messages: &[Message],
        _gosling_mode: GoslingMode,
    ) -> Result<Vec<InspectionResult>> {
        Ok(self.results.clone())
    }
}

#[async_trait]
impl ToolInspector for MockInspectorErr {
    fn name(&self) -> &'static str {
        self.name
    }
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    async fn inspect(
        &self,
        _session_id: &str,
        _tool_requests: &[ToolRequest],
        _messages: &[Message],
        _gosling_mode: GoslingMode,
    ) -> Result<Vec<InspectionResult>> {
        Err(anyhow!("simulated failure"))
    }
}

#[tokio::test]
async fn test_inspect_tools_aggregates_and_handles_errors() {
    // Arrange: create a manager with one successful and one failing inspector
    let ok_results = vec![
        InspectionResult {
            tool_request_id: "req_1".to_string(),
            action: InspectionAction::Allow,
            reason: "looks safe".to_string(),
            confidence: 0.95,
            inspector_name: "ok".to_string(),
            finding_id: None,
            metadata: None,
        },
        InspectionResult {
            tool_request_id: "req_2".to_string(),
            action: InspectionAction::RequireApproval(Some("double check".to_string())),
            reason: "needs user confirmation".to_string(),
            confidence: 0.7,
            inspector_name: "ok".to_string(),
            finding_id: Some("FND-123".to_string()),
            metadata: None,
        },
    ];

    let mut manager = ToolInspectionManager::new();
    manager.add_inspector(Box::new(MockInspectorOk {
        name: "ok",
        results: ok_results.clone(),
    }));
    manager.add_inspector(Box::new(MockInspectorErr { name: "err" }));

    // No specific input is required for this aggregation behavior
    let tool_requests: Vec<ToolRequest> = vec![];
    let messages: Vec<Message> = vec![];

    // Act
    let results = manager
        .inspect_tools(
            gosling_test_support::TEST_SESSION_ID,
            &tool_requests,
            &messages,
            GoslingMode::Approve,
        )
        .await
        .expect("inspect_tools should not fail when one inspector errors");

    // Assert: results from the successful inspector are returned; failing inspector is ignored
    assert_eq!(
        results.len(),
        2,
        "Should aggregate results from successful inspectors only"
    );
    // Also verify inspector_names() order/presence
    let names = manager.inspector_names();
    assert_eq!(
        names,
        vec!["ok", "err"],
        "Inspector names should reflect registration order"
    );

    // Verify that specific actions are preserved
    assert!(results
        .iter()
        .any(|r| matches!(r.action, InspectionAction::Allow)));
    assert!(results
        .iter()
        .any(|r| matches!(r.action, InspectionAction::RequireApproval(_))));
}

fn result(
    tool_request_id: &str,
    inspector_name: &str,
    action: InspectionAction,
    metadata: Option<serde_json::Value>,
) -> InspectionResult {
    InspectionResult {
        tool_request_id: tool_request_id.to_string(),
        action,
        reason: String::new(),
        confidence: 1.0,
        inspector_name: inspector_name.to_string(),
        finding_id: None,
        metadata,
    }
}

/// The permission baseline reports before the scope and egress inspectors.
/// A stored always-allow for the tool produced an `Allow` first, and the
/// approval prompt used to read only that first result, so a later
/// `RequireApproval` from `working_dir_scope` was silently auto-approved in
/// Auto mode. The prompt must come from whichever inspector required it.
#[test]
fn security_prompt_is_not_shadowed_by_an_earlier_allow_for_the_same_request() {
    let results = vec![
        result("req_1", "permission", InspectionAction::Allow, None),
        result(
            "req_1",
            "working_dir_scope",
            InspectionAction::RequireApproval(Some("outside the working directories".into())),
            None,
        ),
        result(
            "req_2",
            "permission",
            InspectionAction::RequireApproval(None),
            None,
        ),
    ];

    assert_eq!(
        security_prompt_for_request("req_1", &results),
        Some("outside the working directories")
    );
    assert_eq!(security_prompt_for_request("req_2", &results), None);
    assert_eq!(security_prompt_for_request("req_3", &results), None);
}

#[test]
fn flagged_domain_comes_from_any_result_for_the_request_and_only_when_unambiguous() {
    let results = vec![
        result("req_1", "permission", InspectionAction::Allow, None),
        result(
            "req_1",
            "egress",
            InspectionAction::RequireApproval(Some("Egress destinations detected".into())),
            Some(serde_json::json!({ "domains": ["exfil.example"] })),
        ),
        result(
            "req_2",
            "egress",
            InspectionAction::RequireApproval(Some("Egress destinations detected".into())),
            Some(serde_json::json!({ "domains": ["a.example", "b.example"] })),
        ),
    ];

    assert_eq!(
        single_flagged_domain_for_request("req_1", &results).as_deref(),
        Some("exfil.example")
    );
    assert_eq!(single_flagged_domain_for_request("req_2", &results), None);
}
