use agent_client_protocol::JsonRpcMessage;
use gosling_sdk_types::custom_notifications::{
    GoslingSessionNotification, GoslingSessionUpdate, PlanUpdate,
};
use gosling_sdk_types::custom_requests::{
    AbandonSessionPlanRequest, AddSessionPlanFeedbackRequest, ApproveSessionPlanRequest,
    ExportSessionPlanRequest, GetSessionPlanRequest, PlanRevisionIdentityDto, PlanStatusDto,
    PlanningCapabilityDto, SessionPlanResponse, StartSessionPlanRequest,
};
use serde_json::json;

#[test]
fn plan_request_methods_are_pinned() {
    assert_eq!(
        GetSessionPlanRequest::default().method(),
        "_gosling/unstable/session/plan/get"
    );
    assert_eq!(
        StartSessionPlanRequest::default().method(),
        "_gosling/unstable/session/plan/start"
    );
    assert_eq!(
        AddSessionPlanFeedbackRequest::default().method(),
        "_gosling/unstable/session/plan/feedback"
    );
    assert_eq!(
        ApproveSessionPlanRequest::default().method(),
        "_gosling/unstable/session/plan/approve"
    );
    assert_eq!(
        AbandonSessionPlanRequest::default().method(),
        "_gosling/unstable/session/plan/abandon"
    );
    assert_eq!(
        ExportSessionPlanRequest::default().method(),
        "_gosling/unstable/session/plan/export"
    );
}

#[test]
fn plan_status_and_capability_wire_values_are_snake_case() {
    let statuses = [
        PlanStatusDto::Drafting,
        PlanStatusDto::AwaitingReview,
        PlanStatusDto::Approved,
        PlanStatusDto::Abandoned,
        PlanStatusDto::Stale,
    ];
    assert_eq!(
        serde_json::to_value(statuses).unwrap(),
        json!([
            "drafting",
            "awaiting_review",
            "approved",
            "abandoned",
            "stale"
        ])
    );

    let capabilities = [
        PlanningCapabilityDto::WorkspaceTree,
        PlanningCapabilityDto::WorkspaceReadText,
        PlanningCapabilityDto::WorkspaceSearchText,
        PlanningCapabilityDto::SessionHistorySearch,
        PlanningCapabilityDto::SessionHistoryRead,
        PlanningCapabilityDto::PlanUpdate,
        PlanningCapabilityDto::PlanRequestReview,
    ];
    assert_eq!(
        serde_json::to_value(capabilities).unwrap(),
        json!([
            "workspace_tree",
            "workspace_read_text",
            "workspace_search_text",
            "session_history_search",
            "session_history_read",
            "plan_update",
            "plan_request_review"
        ])
    );
}

#[test]
fn plan_mutation_requests_use_camel_case_expectations() {
    let request = AddSessionPlanFeedbackRequest {
        session_id: "s1".to_string(),
        body: "Please clarify this step.".to_string(),
        start_line: Some(4),
        end_line: Some(5),
        selected_text: Some("Step text".to_string()),
        expected_generation: 2,
        expected_revision_id: "revision-3".to_string(),
        expected_revision_sha256: "revision-sha".to_string(),
        expected_source_hash: "source-hash".to_string(),
        expected_scope_hash: "scope-hash".to_string(),
    };

    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({
            "sessionId": "s1",
            "body": "Please clarify this step.",
            "startLine": 4,
            "endLine": 5,
            "selectedText": "Step text",
            "expectedGeneration": 2,
            "expectedRevisionId": "revision-3",
            "expectedRevisionSha256": "revision-sha",
            "expectedSourceHash": "source-hash",
            "expectedScopeHash": "scope-hash"
        })
    );
}

#[test]
fn plan_export_pins_the_reviewed_status() {
    let request = ExportSessionPlanRequest {
        session_id: "session-1".to_string(),
        expected_generation: 4,
        expected_revision_id: "revision-7".to_string(),
        expected_revision_sha256: "sha-7".to_string(),
        expected_status: PlanStatusDto::Approved,
    };

    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({
            "sessionId": "session-1",
            "expectedGeneration": 4,
            "expectedRevisionId": "revision-7",
            "expectedRevisionSha256": "sha-7",
            "expectedStatus": "approved",
        })
    );
}

#[test]
fn implementation_reference_is_optional_server_output() {
    let without_reference = SessionPlanResponse {
        snapshot: None,
        provider_supports_host_enforced_planning: true,
        permitted_capabilities: Vec::new(),
        implementation_reference: None,
    };
    assert!(serde_json::to_value(without_reference)
        .unwrap()
        .get("implementationReference")
        .is_none());

    let with_reference = SessionPlanResponse {
        implementation_reference: Some("server-authored reference".to_string()),
        ..Default::default()
    };
    assert_eq!(
        serde_json::to_value(with_reference).unwrap()["implementationReference"],
        "server-authored reference"
    );
}

#[test]
fn compact_plan_notification_omits_plan_content() {
    let notification = GoslingSessionNotification {
        session_id: "s1".to_string(),
        update: GoslingSessionUpdate::PlanUpdate(PlanUpdate {
            plan_id: "plan-1".to_string(),
            generation: 2,
            status: PlanStatusDto::Approved,
            active_revision: Some(PlanRevisionIdentityDto {
                id: "revision-3".to_string(),
                revision: 3,
                content_sha256: "revision-sha".to_string(),
            }),
            updated_at: "2026-09-13T12:00:00Z".to_string(),
        }),
    };

    let value = serde_json::to_value(notification).unwrap();
    assert_eq!(value["update"]["sessionUpdate"], "plan_update");
    assert_eq!(value["update"]["status"], "approved");
    assert!(value["update"].get("contentMarkdown").is_none());
    assert!(value["update"].get("feedback").is_none());
    assert!(value["update"].get("recentEvents").is_none());
}
