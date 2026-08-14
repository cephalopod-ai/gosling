use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SHELL_PROVISIONING_SCHEMA_VERSION: u32 = 1;
pub const SHELL_HANDOFF_SCHEMA_VERSION: u32 = 1;
pub const SHELL_MODULE_CONTRACT_VERSION: u32 = 1;
pub const SHELL_SETTINGS_SCHEMA_VERSION: u32 = 1;
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

/// Governs whether a shell may read Gosling's credential catalog and select a profile.
///
/// `Fixed` keeps the pre-DS-4 behavior: the provisioned `credential_profile_id` is the only
/// permitted profile and no catalog method is available. Absence of the field defaults to `Fixed`
/// so an unmodified provisioning document never silently widens credential access.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellCredentialPolicy {
    #[default]
    Fixed,
    SelectableCatalog,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellSessionProvisioning {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    #[serde(default)]
    pub credential_policy: ShellCredentialPolicy,
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

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellInstructionProfile {
    pub system_prompt: String,
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
    /// Version of the product-local settings document this shell expects.
    ///
    /// Absent means the contract default; a version this build does not know fails closed rather
    /// than silently migrating an operator's document.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub settings_schema_version: Option<u32>,
    #[serde(default)]
    pub protocol_policy: ShellProtocolPolicy,
    #[serde(default)]
    pub session: ShellSessionProvisioning,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instructions: Option<ShellInstructionProfile>,
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
    InvalidInstructions,
    InvalidDomainAdapter,
    InvalidCredentialPolicy,
    UnsupportedSettingsSchemaVersion,
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
    #[serde(default)]
    pub credential_policy: ShellCredentialPolicy,
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
#[serde(rename_all = "camelCase")]
pub struct ShellProvisioningReadRequest {
    /// The accepted working directory this provisioning should be validated against.
    ///
    /// Extensions and skills can be project-local, so a shell whose selected directory differs from
    /// the backend's startup directory must be judged against the directory its sessions will use.
    /// Absent falls back to the startup directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
}

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
    /// See `ShellProvisioningReadRequest::working_dir`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct ShellProvisioningValidateResponse {
    pub provisioning: ShellProvisioning,
    pub validation: ShellProvisioningValidationReport,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellDirectoryStatus {
    #[default]
    Valid,
    Invalid,
    Unavailable,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellDirectoryReason {
    #[default]
    NotAbsolute,
    NotFound,
    NotADirectory,
    Inaccessible,
    PathTooLong,
    InvalidPath,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(
    method = "_gosling/unstable/shell/directory/validate",
    response = ShellDirectoryValidateResponse
)]
#[serde(rename_all = "camelCase")]
pub struct ShellDirectoryValidateRequest {
    pub path: String,
}

#[derive(
    Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, JsonRpcResponse,
)]
#[serde(rename_all = "camelCase")]
pub struct ShellDirectoryValidateResponse {
    pub status: ShellDirectoryStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canonical_path: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<ShellDirectoryReason>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellCredentialStatus {
    Configured,
    #[default]
    RelinkRequired,
}

/// The only credential facts a shell may observe. Auth kind, source, configured secret-field names,
/// non-secret provider parameters, timestamps, and usage stay inside main Gosling.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellCredentialSummary {
    pub id: String,
    pub name: String,
    pub provider_or_service_id: String,
    pub status: ShellCredentialStatus,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellCredentialCatalogStatus {
    #[default]
    Available,
    Denied,
    Unavailable,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(
    method = "_gosling/unstable/shell/credentials/list",
    response = ShellCredentialListResponse
)]
pub struct ShellCredentialListRequest {}

#[derive(
    Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, JsonRpcResponse,
)]
#[serde(rename_all = "camelCase")]
pub struct ShellCredentialListResponse {
    pub status: ShellCredentialCatalogStatus,
    #[serde(default)]
    pub profiles: Vec<ShellCredentialSummary>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellModuleKind {
    #[default]
    Core,
    Extension,
    Skill,
    Adapter,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ShellModuleStatus {
    #[default]
    Ready,
    Unavailable,
    Incompatible,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellModuleSummary {
    pub id: String,
    pub kind: ShellModuleKind,
    pub status: ShellModuleStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(
    method = "_gosling/unstable/shell/modules/list",
    response = ShellModuleListResponse
)]
#[serde(rename_all = "camelCase")]
pub struct ShellModuleListRequest {
    /// The accepted working directory this inventory should be resolved against.
    ///
    /// Project-local extensions and skills exist per directory, so resolving against the backend's
    /// startup directory would report modules a session in the selected directory does not get.
    /// Absent falls back to the startup directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,
}

#[derive(
    Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, JsonRpcResponse,
)]
#[serde(rename_all = "camelCase")]
pub struct ShellModuleListResponse {
    pub contract_version: u32,
    #[serde(default)]
    pub modules: Vec<ShellModuleSummary>,
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
    fn instruction_profile_is_shell_owned() {
        let provisioning = ShellProvisioning {
            schema_version: SHELL_PROVISIONING_SCHEMA_VERSION,
            identity: ShellIdentity {
                id: "default_shell".into(),
                display_name: "Default Shell".into(),
                version: "1".into(),
                runtime_namespace: "default_shell".into(),
            },
            instructions: Some(ShellInstructionProfile {
                system_prompt: "You are the Default Shell assistant.".into(),
            }),
            ..ShellProvisioning::default()
        };

        let json = serde_json::to_value(provisioning).unwrap();
        assert_eq!(
            json["instructions"]["systemPrompt"],
            "You are the Default Shell assistant."
        );
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
    fn credential_policy_defaults_to_fixed_when_absent() {
        let session: ShellSessionProvisioning =
            serde_json::from_value(serde_json::json!({})).unwrap();
        assert_eq!(session.credential_policy, ShellCredentialPolicy::Fixed);
        assert!(session.credential_profile_id.is_none());
    }

    #[test]
    fn credential_summary_excludes_every_non_shell_credential_field() {
        let summary = ShellCredentialSummary {
            id: "profile-id".into(),
            name: "Work account".into(),
            provider_or_service_id: "anthropic".into(),
            status: ShellCredentialStatus::Configured,
        };

        let json = serde_json::to_value(summary).unwrap();
        let object = json.as_object().unwrap();
        assert_eq!(
            object.keys().map(String::as_str).collect::<Vec<_>>(),
            ["id", "name", "providerOrServiceId", "status"]
        );
        for excluded in [
            "authKind",
            "source",
            "configuredSecretFields",
            "nonSecretFields",
            "createdAt",
            "updatedAt",
        ] {
            assert!(object.get(excluded).is_none(), "{excluded} leaked");
        }
    }

    #[test]
    fn directory_validation_never_echoes_a_path_on_failure() {
        let response = ShellDirectoryValidateResponse {
            status: ShellDirectoryStatus::Invalid,
            canonical_path: None,
            reason: Some(ShellDirectoryReason::NotFound),
        };

        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["status"], "invalid");
        assert_eq!(json["reason"], "not_found");
        assert!(json.get("canonicalPath").is_none());
    }

    #[test]
    fn module_summary_carries_identity_and_status_without_transport_details() {
        let response = ShellModuleListResponse {
            contract_version: SHELL_MODULE_CONTRACT_VERSION,
            modules: vec![ShellModuleSummary {
                id: "adapter:neutral-fixture".into(),
                kind: ShellModuleKind::Adapter,
                status: ShellModuleStatus::Ready,
                version: Some("0.1.0".into()),
                capabilities: vec!["inspect".into(), "toggle".into()],
            }],
        };

        let json = serde_json::to_value(response).unwrap();
        assert_eq!(json["contractVersion"], SHELL_MODULE_CONTRACT_VERSION);
        assert_eq!(json["modules"][0]["kind"], "adapter");
        let module = json["modules"][0].as_object().unwrap();
        for excluded in ["command", "args", "env", "uri", "transport", "pid"] {
            assert!(module.get(excluded).is_none(), "{excluded} leaked");
        }
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
