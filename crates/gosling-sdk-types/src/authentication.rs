use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Saved references and disconnection policy; never contains credential values.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationSettings {
    #[serde(default)]
    pub provider_disconnected: bool,
    #[serde(default)]
    pub extensions: BTreeMap<String, ExtensionAuthentication>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionAuthentication {
    #[serde(default)]
    pub disconnected: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_namespace: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub destination: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub secret_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AuthenticationTarget {
    Session { id: String },
    Workspace { id: String },
}

impl Default for AuthenticationTarget {
    fn default() -> Self {
        Self::Session { id: String::new() }
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/authentication/read", response = AuthenticationResponse)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationReadRequest {
    pub target: AuthenticationTarget,
}

/// A null profile disconnects the target; it does not select global credentials.
#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/authentication/provider/set", response = AuthenticationResponse)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationProviderSetRequest {
    pub target: AuthenticationTarget,
    pub profile_id: Option<String>,
}

/// Secret fields are stored securely under a fresh scoped account reference.
/// Disconnecting retains the reference and its credentials for reconnection.
#[derive(Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/authentication/extension/set", response = AuthenticationResponse)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationExtensionSetRequest {
    pub target: AuthenticationTarget,
    pub name: String,
    pub connected: bool,
    #[serde(default)]
    pub sign_in: bool,
    #[serde(default)]
    pub secret_fields: Vec<crate::workspace::CredentialFieldUpdate>,
}

impl std::fmt::Debug for AuthenticationExtensionSetRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthenticationExtensionSetRequest")
            .field("target", &self.target)
            .field("name", &self.name)
            .field("connected", &self.connected)
            .field("sign_in", &self.sign_in)
            .field("secret_fields", &"[redacted]")
            .finish()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationExtensionSummary {
    pub key: String,
    pub name: String,
    pub supports_oauth: bool,
    pub secret_fields: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct AuthenticationResponse {
    pub settings: AuthenticationSettings,
    pub provider_id: Option<String>,
    pub credential_profile_id: Option<String>,
    pub credential_profile_name: Option<String>,
    pub extensions: Vec<AuthenticationExtensionSummary>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_request_debug_redacts_values() {
        let request = AuthenticationExtensionSetRequest {
            secret_fields: vec![crate::workspace::CredentialFieldUpdate {
                key: "TOKEN".into(),
                value: "fixture-private-value".into(),
            }],
            ..Default::default()
        };
        assert!(!format!("{request:?}").contains("fixture-private-value"));
    }

    #[test]
    fn null_profile_is_an_explicit_targeted_disconnect() {
        let request: AuthenticationProviderSetRequest = serde_json::from_value(serde_json::json!({ "target": { "type": "workspace", "id": "workspace-id" }, "profileId": null })).unwrap();
        assert!(request.profile_id.is_none());
        assert_eq!(
            request.target,
            AuthenticationTarget::Workspace {
                id: "workspace-id".into()
            }
        );
        assert!(
            serde_json::from_value::<AuthenticationReadRequest>(serde_json::json!({})).is_err()
        );
    }
}
