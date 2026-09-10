use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SESSION_HANDOFF_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ContextOwnershipDto {
    #[default]
    Gosling,
    Provider,
    Hybrid,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitySupportDto {
    #[default]
    Unsupported,
    Supported,
    Required,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProviderCapabilitiesDto {
    pub context_ownership: ContextOwnershipDto,
    pub native_resume: CapabilitySupportDto,
    pub history_import: CapabilitySupportDto,
    pub in_place_model_change: CapabilitySupportDto,
    pub session_fork: CapabilitySupportDto,
    pub bootstrap_handoff: CapabilitySupportDto,
    pub bootstrap_acknowledgement: CapabilitySupportDto,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionContinuityClassDto {
    SeamlessResume,
    #[default]
    SummarizedHandoff,
    NewContextOnly,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandoffDeliveryStrategyDto {
    NativeResume,
    HistoryImport,
    Bootstrap,
    #[default]
    ContextInjection,
    NewContext,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionHandoffTriggerDto {
    #[default]
    UserRequestedSwitch,
    ProviderFailure,
    ModelChangeRequiresRecreation,
    ThinkingEffortChangeRequiresRecreation,
    SessionResume,
    SessionFork,
    ManualCheckpoint,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionHandoffStatusDto {
    #[default]
    Prepared,
    Activating,
    Active,
    Failed,
    RolledBack,
    Superseded,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HandoffEvidenceClassDto {
    #[default]
    Observed,
    Summarized,
    Unknown,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffEvidenceItemDto {
    pub content: String,
    pub evidence: HandoffEvidenceClassDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_row_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<i64>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffEndpointDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requested_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffCoverageDto {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub first_row_id: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub covered_through_row_id: Option<i64>,
    pub covered_message_count: u64,
    pub total_message_count: u64,
    pub source_hash: String,
    pub summary_status: String,
    pub recent_tail_message_count: u64,
    pub estimated_tokens: u64,
    #[serde(default)]
    pub truncations: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffFileDto {
    pub path: String,
    pub operation: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_id: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffOperationDto {
    pub operation_id: String,
    pub tool_request_id: String,
    pub tool_name: String,
    pub state: String,
    pub retryable: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffConversationItemDto {
    pub role: String,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_message_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_row_id: Option<i64>,
    pub timestamp: i64,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffRedactionReportDto {
    pub redaction_count: u64,
    pub truncated_item_count: u64,
    pub excluded_binary_count: u64,
    #[serde(default)]
    pub categories: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HandoffEnrichmentDto {
    pub source: String,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionHandoffSnapshotV1Dto {
    pub snapshot_id: String,
    pub schema_version: u32,
    pub session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_session_id: Option<String>,
    pub generation: u64,
    pub trigger: SessionHandoffTriggerDto,
    pub status: SessionHandoffStatusDto,
    pub created_at: String,
    pub source: HandoffEndpointDto,
    pub target: HandoffEndpointDto,
    pub continuity_class: SessionContinuityClassDto,
    pub delivery_strategy: HandoffDeliveryStrategyDto,
    pub coverage: HandoffCoverageDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_objective: Option<HandoffEvidenceItemDto>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_user_intent: Option<HandoffEvidenceItemDto>,
    #[serde(default)]
    pub completed_work: Vec<HandoffEvidenceItemDto>,
    #[serde(default)]
    pub decisions: Vec<HandoffEvidenceItemDto>,
    #[serde(default)]
    pub files_touched: Vec<HandoffFileDto>,
    #[serde(default)]
    pub workspace_state: Vec<HandoffEvidenceItemDto>,
    #[serde(default)]
    pub commands_and_checks: Vec<HandoffEvidenceItemDto>,
    #[serde(default)]
    pub active_or_interrupted_operations: Vec<HandoffOperationDto>,
    #[serde(default)]
    pub current_errors: Vec<HandoffEvidenceItemDto>,
    #[serde(default)]
    pub attempted_mitigations: Vec<HandoffEvidenceItemDto>,
    #[serde(default)]
    pub pending_approvals: Vec<HandoffEvidenceItemDto>,
    #[serde(default)]
    pub unresolved_questions: Vec<HandoffEvidenceItemDto>,
    #[serde(default)]
    pub next_actions: Vec<HandoffEvidenceItemDto>,
    #[serde(default)]
    pub recent_conversation_tail: Vec<HandoffConversationItemDto>,
    pub redaction_report: HandoffRedactionReportDto,
    pub enrichment: HandoffEnrichmentDto,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activated_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub acknowledged_at: Option<String>,
}
