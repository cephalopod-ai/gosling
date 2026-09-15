use agent_client_protocol::JsonRpcMessage;
use gosling_sdk_types::recall_brief::{
    RecallArtifactKind, RecallBriefRequest, RecallBriefStatus, RecallClaimKind, RecallSelectors,
};
use serde_json::json;

#[test]
fn recall_brief_method_and_explicit_selector_wire_shape_are_pinned() {
    assert_eq!(
        RecallBriefRequest::default().method(),
        "_gosling/unstable/session/recall/brief"
    );
    let request = RecallBriefRequest {
        session_id: "session-1".to_string(),
        extension_name: "muninn".to_string(),
        query: "Santa beliefs".to_string(),
        facets: None,
        cursor: None,
        selectors: Some(RecallSelectors {
            artifact_kind: Some(RecallArtifactKind::Chat),
            store_id: Some("personal".to_string()),
            ..Default::default()
        }),
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        json!({
            "sessionId": "session-1",
            "extensionName": "muninn",
            "query": "Santa beliefs",
            "selectors": {"artifactKind": "chat", "storeId": "personal"}
        })
    );
}

#[test]
fn recall_brief_rejects_unrecognized_scope_fields() {
    assert!(serde_json::from_value::<RecallBriefRequest>(json!({
        "sessionId": "session-1",
        "extensionName": "muninn",
        "query": "Santa beliefs",
        "selectors": {"storeId": "personal", "rawUrl": "https://untrusted.example"}
    }))
    .is_err());
}

#[test]
fn recall_brief_status_and_claim_kinds_have_no_verified_world_variant() {
    assert_eq!(
        serde_json::to_value([
            RecallBriefStatus::Synthesized,
            RecallBriefStatus::EvidenceOnly,
            RecallBriefStatus::Empty,
            RecallBriefStatus::Partial,
            RecallBriefStatus::Unavailable
        ])
        .unwrap(),
        json!([
            "synthesized",
            "evidence_only",
            "empty",
            "partial",
            "unavailable"
        ])
    );
    assert_eq!(
        serde_json::to_value(RecallClaimKind::ReportedWorldAssertion).unwrap(),
        "reported_world_assertion"
    );
    assert!(serde_json::from_value::<RecallClaimKind>(json!("verified_world_fact")).is_err());
}
