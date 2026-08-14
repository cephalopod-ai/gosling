use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SHELL_PROVISIONING_SCHEMA_VERSION: u32 = 1;
pub const SHELL_HANDOFF_SCHEMA_VERSION: u32 = 1;
pub const DOMAIN_ADAPTER_PROTOCOL_VERSION: &str = "1.0.0";

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellIdentity {
    pub id: String,
    pub display_name: String,
    pub version: String,
    #[serde(default)]
    pub runtime_namespace: String,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellAuthorityMode {
    #[default]
    Inherit,
    Restricted,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellProtocolPolicy {
    #[serde(default)]
    pub mode: ShellAuthorityMode,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied_methods: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellExtensionSelection {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub available_tools: Option<Vec<String>>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellSessionProvisioning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Vec<ShellExtensionSelection>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill_ids: Option<Vec<String>>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainAdapterActionKind {
    #[default]
    Read,
    Mutate,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainAdapterStatus {
    #[default]
    Ready,
    Crashed,
    Hung,
    Incompatible,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DomainAdapterAction {
    pub name: String,
    pub kind: DomainAdapterActionKind,
    pub schema_ref: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DomainAdapterDescriptor {
    pub domain_id: String,
    pub display_name: String,
    pub version: String,
    pub protocol_version: String,
    #[serde(default)]
    pub actions: Vec<DomainAdapterAction>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellSettingsAuthority {
    #[default]
    MainGosling,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellProvisioning {
    pub schema_version: u32,
    pub identity: ShellIdentity,
    #[serde(default)]
    pub settings_authority: ShellSettingsAuthority,
    #[serde(default)]
    pub protocol_policy: ShellProtocolPolicy,
    #[serde(default)]
    pub session: ShellSessionProvisioning,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain_adapter: Option<DomainAdapterDescriptor>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellProvisioningIssueSeverity {
    #[default]
    Error,
    Warning,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellProvisioningIssueCode {
    UnsupportedSchemaVersion,
    InvalidIdentity,
    #[default]
    MissingWorkspace,
    InvalidWorkspace,
    MissingCredentialProfile,
    CredentialProfileUnavailable,
    CredentialProviderMismatch,
    MissingProvider,
    InvalidModel,
    MissingExtension,
    DuplicateExtension,
    InvalidToolSelection,
    MissingSkill,
    DuplicateSkill,
    InvalidDeniedMethod,
    InvalidDomainAdapter,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellProvisioningIssue {
    pub code: ShellProvisioningIssueCode,
    pub severity: ShellProvisioningIssueSeverity,
    pub path: String,
    pub message: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellProvisioningResolution {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default)]
    pub extensions: Vec<ShellExtensionSelection>,
    #[serde(default)]
    pub skill_ids: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellProvisioningValidationReport {
    pub valid: bool,
    #[serde(default)]
    pub issues: Vec<ShellProvisioningIssue>,
    #[serde(default)]
    pub resolution: ShellProvisioningResolution,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(
    method = "_gosling/unstable/shell/provisioning/read",
    response = ShellProvisioningReadResponse
)]
pub struct ShellProvisioningReadRequest {}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct ShellProvisioningReadResponse {
    pub provisioning: ShellProvisioning,
    pub validation: ShellProvisioningValidationReport,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(
    method = "_gosling/unstable/shell/provisioning/validate",
    response = ShellProvisioningValidateResponse
)]
#[serde(rename_all = "camelCase")]
pub struct ShellProvisioningValidateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provisioning: Option<ShellProvisioning>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct ShellProvisioningValidateResponse {
    pub provisioning: ShellProvisioning,
    pub validation: ShellProvisioningValidationReport,
}

#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DomainResourceReference {
    pub kind: String,
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(
    method = "_gosling/unstable/shell/domain/snapshot",
    response = DomainSnapshotResponse
)]
#[serde(rename_all = "camelCase")]
pub struct DomainSnapshotRequest {
    #[serde(default)]
    pub input: serde_json::Value,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct DomainSnapshotResponse {
    pub domain_id: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub resources: Vec<DomainResourceReference>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(
    method = "_gosling/unstable/shell/domain/action",
    response = DomainActionResponse
)]
#[serde(rename_all = "camelCase")]
pub struct DomainActionRequest {
    pub session_id: String,
    pub generation: u64,
    pub action: String,
    #[serde(default)]
    pub input: serde_json::Value,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct DomainActionResponse {
    pub domain_id: String,
    pub action: String,
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub resources: Vec<DomainResourceReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation_action_id: Option<String>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DomainActionConfirmationStatus {
    #[default]
    Approved,
    Denied,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(
    method = "_gosling/unstable/shell/domain/action/confirm",
    response = DomainActionConfirmResponse
)]
#[serde(rename_all = "camelCase")]
pub struct DomainActionConfirmRequest {
    pub session_id: String,
    pub generation: u64,
    pub action_id: String,
    pub approve: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct DomainActionConfirmResponse {
    pub status: DomainActionConfirmationStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result: Option<DomainActionResponse>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellHandoffReference {
    pub kind: String,
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub uri: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(
    method = "_gosling/unstable/shell/handoff/prepare",
    response = ShellHandoffPrepareResponse
)]
#[serde(rename_all = "camelCase")]
pub struct ShellHandoffPrepareRequest {
    pub session_id: String,
    pub question: String,
    pub requested_capability: String,
    #[serde(default)]
    pub references: Vec<ShellHandoffReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_destination: Option<String>,
    #[serde(default)]
    pub allow_mutation: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellHandoffEnvelope {
    pub schema_version: u32,
    pub handoff_id: String,
    pub origin: ShellIdentity,
    pub source_session_id: String,
    pub question: String,
    pub requested_capability: String,
    #[serde(default)]
    pub references: Vec<ShellHandoffReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub return_destination: Option<String>,
    #[serde(default)]
    pub allow_mutation: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct ShellHandoffPrepareResponse {
    pub handoff: ShellHandoffEnvelope,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authority_defaults_to_inherited() {
        let policy: ShellProtocolPolicy = serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(policy.mode, ShellAuthorityMode::Inherit);
        assert!(policy.denied_methods.is_empty());
    }

    #[test]
    fn provisioning_contract_contains_references_not_secret_values() {
        let provisioning = ShellProvisioning {
            schema_version: SHELL_PROVISIONING_SCHEMA_VERSION,
            identity: ShellIdentity {
                id: "physics".into(),
                display_name: "Physics".into(),
                version: "1".into(),
                runtime_namespace: "physics".into(),
            },
            session: ShellSessionProvisioning {
                credential_profile_id: Some("profile-id".into()),
                ..ShellSessionProvisioning::default()
            },
            ..ShellProvisioning::default()
        };

        let json = serde_json::to_value(provisioning).unwrap();
        assert_eq!(json["session"]["credentialProfileId"], "profile-id");
        assert!(json.get("secrets").is_none());
        assert!(json.get("credentials").is_none());
    }

    #[test]
    fn validation_report_contains_structured_paths_without_secret_fields() {
        let report = ShellProvisioningValidationReport {
            valid: false,
            issues: vec![ShellProvisioningIssue {
                code: ShellProvisioningIssueCode::MissingCredentialProfile,
                severity: ShellProvisioningIssueSeverity::Error,
                path: "session.credentialProfileId".into(),
                message: "credential profile does not exist".into(),
            }],
            ..ShellProvisioningValidationReport::default()
        };
        let json = serde_json::to_value(report).unwrap();
        assert_eq!(json["issues"][0]["path"], "session.credentialProfileId");
        assert!(json.get("secrets").is_none());
    }

    #[test]
    fn domain_adapter_descriptor_carries_protocol_and_action_authority() {
        let descriptor = DomainAdapterDescriptor {
            domain_id: "neutral-fixture".into(),
            display_name: "Neutral Fixture".into(),
            version: "0.1.0".into(),
            protocol_version: DOMAIN_ADAPTER_PROTOCOL_VERSION.into(),
            actions: vec![
                DomainAdapterAction {
                    name: "inspect".into(),
                    kind: DomainAdapterActionKind::Read,
                    schema_ref: "neutral-fixture/inspect@1".into(),
                },
                DomainAdapterAction {
                    name: "toggle".into(),
                    kind: DomainAdapterActionKind::Mutate,
                    schema_ref: "neutral-fixture/toggle@1".into(),
                },
            ],
        };

        let json = serde_json::to_value(descriptor).unwrap();
        assert_eq!(json["protocolVersion"], DOMAIN_ADAPTER_PROTOCOL_VERSION);
        assert_eq!(json["actions"][0]["kind"], "read");
        assert_eq!(json["actions"][1]["kind"], "mutate");
        assert!(json.get("confirmationToken").is_none());
    }

    #[test]
    fn domain_confirmation_uses_only_a_fenced_opaque_action_id() {
        let action = DomainActionRequest {
            session_id: "session-a".into(),
            generation: 4,
            action: "toggle".into(),
            input: serde_json::json!({ "enabled": true }),
        };
        let confirmation = DomainActionConfirmRequest {
            session_id: "session-a".into(),
            generation: 4,
            action_id: "018f0000-0000-7000-8000-000000000000".into(),
            approve: true,
        };

        let action_json = serde_json::to_value(action).unwrap();
        let confirmation_json = serde_json::to_value(confirmation).unwrap();

        assert_eq!(action_json["sessionId"], "session-a");
        assert_eq!(action_json["generation"], 4);
        assert!(action_json.get("confirmationToken").is_none());
        assert_eq!(
            confirmation_json["actionId"],
            "018f0000-0000-7000-8000-000000000000"
        );
        assert!(confirmation_json.get("token").is_none());
    }
}
