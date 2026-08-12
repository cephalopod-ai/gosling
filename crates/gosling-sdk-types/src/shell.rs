use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const SHELL_PROVISIONING_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ShellIdentity {
    pub id: String,
    pub display_name: String,
    pub version: String,
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

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DomainAdapterDescriptor {
    pub domain_id: String,
    pub display_name: String,
    pub version: String,
    #[serde(default)]
    pub actions: Vec<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    pub action: String,
    #[serde(default)]
    pub input: serde_json::Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirmation_token: Option<String>,
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
}
