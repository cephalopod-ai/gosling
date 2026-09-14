use agent_client_protocol::JsonRpcMessage;
use gosling_sdk_types::custom_requests::{
    ApplyCompactionHistoryPolicyRequest, CompactionHistoryPolicyDto,
    DeleteCompactionRevisionRequest, GetCompactionRevisionRequest, ListCompactionRevisionsRequest,
    PreviewCompactionHistoryPolicyRequest, PurgeCompactionHistoryRequest,
    ReadCompactionHistoryPolicyRequest, SetCompactionRevisionPinnedRequest,
};
use serde_json::json;

#[test]
fn compaction_history_request_methods_are_pinned() {
    assert_eq!(
        ListCompactionRevisionsRequest::default().method(),
        "_gosling/unstable/session/compactions/history"
    );
    assert_eq!(
        GetCompactionRevisionRequest::default().method(),
        "_gosling/unstable/session/compactions/revision"
    );
    assert_eq!(
        SetCompactionRevisionPinnedRequest::default().method(),
        "_gosling/unstable/session/compactions/pin"
    );
    assert_eq!(
        DeleteCompactionRevisionRequest::default().method(),
        "_gosling/unstable/session/compactions/delete"
    );
    assert_eq!(
        PurgeCompactionHistoryRequest::default().method(),
        "_gosling/unstable/session/compactions/purge"
    );
    assert_eq!(
        ReadCompactionHistoryPolicyRequest::default().method(),
        "_gosling/unstable/context-history/policy"
    );
    assert_eq!(
        PreviewCompactionHistoryPolicyRequest::default().method(),
        "_gosling/unstable/context-history/policy/preview"
    );
    assert_eq!(
        ApplyCompactionHistoryPolicyRequest::default().method(),
        "_gosling/unstable/context-history/policy/apply"
    );
}

#[test]
fn compaction_history_policy_uses_camel_case_and_optional_retention() {
    let policy = CompactionHistoryPolicyDto {
        version: 1,
        capture_enabled: true,
        retention_days: None,
        purge_grace_days: 7,
        max_revisions_per_session: 100,
        max_total_bytes: 268_435_456,
    };

    assert_eq!(
        serde_json::to_value(policy).unwrap(),
        json!({
            "version": 1,
            "captureEnabled": true,
            "purgeGraceDays": 7,
            "maxRevisionsPerSession": 100,
            "maxTotalBytes": 268435456
        })
    );
}
