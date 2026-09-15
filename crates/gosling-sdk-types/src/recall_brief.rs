//! Typed request and report contract for an explicit, read-only Recall Brief.

use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(
    method = "_gosling/unstable/session/recall/brief",
    response = RecallBriefResponse
)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecallBriefRequest {
    pub session_id: String,
    pub extension_name: String,
    pub query: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facets: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selectors: Option<RecallSelectors>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecallSelectors {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_kind: Option<RecallArtifactKind>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub conversation_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_path_prefix: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub repository_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retrieval_context: Option<RecallRetrievalContext>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub until: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecallArtifactKind {
    Chat,
    Code,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecallRetrievalContext {
    pub active_repository: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_path_prefix: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecallBriefStatus {
    Synthesized,
    EvidenceOnly,
    Empty,
    Partial,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecallClaimKind {
    RecordExists,
    ReportedBelief,
    ReportedChange,
    ReportedWorldAssertion,
    HistoricalReferentReport,
    Inference,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecallEvidenceQuote {
    pub source_key: String,
    pub quote: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecallFinding {
    pub claim_kind: RecallClaimKind,
    pub statement: String,
    pub subject: String,
    pub referent_sense: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_label: Option<String>,
    pub source_evidence: Vec<RecallEvidenceQuote>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecallUnresolved {
    pub question: String,
    pub reason: String,
    pub examined_sources: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecallSourceEvidence {
    pub source_key: String,
    pub resource_uri: String,
    pub store_id: String,
    pub memory_id: String,
    pub revision: u64,
    pub title: String,
    pub quote: String,
    pub content_truncated: bool,
    pub source_ref: String,
    pub source_ref_truncated: bool,
    pub source_kind: String,
    pub trust_tier: String,
    pub status: String,
    pub recorded_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_time: Option<serde_json::Value>,
    pub content_safety: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecallFacetState {
    pub lane: String,
    pub state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub candidates_fetched: Option<u64>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RecallReceipt {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub partial: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_unique: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_unique: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item_budget: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_count: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unique_findings: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub generation_changed: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facets_covered: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facets_empty: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facets_failed: Option<Vec<String>>,
    #[serde(default)]
    pub lanes: Vec<RecallFacetState>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct RecallBriefResponse {
    pub status: RecallBriefStatus,
    pub provider_name: String,
    pub model_name: String,
    pub findings: Vec<RecallFinding>,
    pub source_evidence: Vec<RecallSourceEvidence>,
    pub unresolved: Vec<RecallUnresolved>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub receipt: Option<RecallReceipt>,
    pub rendered: String,
    pub notice: String,
}
