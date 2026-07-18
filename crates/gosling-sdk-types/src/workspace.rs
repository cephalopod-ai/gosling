use agent_client_protocol::{JsonRpcRequest, JsonRpcResponse};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const WORKSPACE_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceFolderKind {
    #[default]
    Source,
    Reference,
    Working,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceFolderAccess {
    #[default]
    Read,
    ReadWrite,
}

#[derive(
    Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProductType {
    #[default]
    Document,
    Spreadsheet,
    Presentation,
    Image,
    Video,
    Code,
    Data,
    Export,
    Other,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CredentialTargetKind {
    #[default]
    Provider,
    Extension,
    Service,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CredentialAuthKind {
    #[default]
    ApiKey,
    ConfigFields,
    Oauth,
    Cli,
    Local,
    Other,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CredentialProfileStatus {
    Configured,
    #[default]
    Missing,
    NeedsAuthentication,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CredentialProfileSource {
    #[default]
    WorkspaceSecureStorage,
    GlobalConfigurationAlias,
    DistributionTemplate,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceIssueSeverity {
    #[default]
    Warning,
    Error,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceIssueCode {
    #[default]
    MissingFolder,
    NotDirectory,
    InaccessibleFolder,
    RelativePath,
    PathTraversal,
    DuplicatePath,
    MissingPrimaryFolder,
    MissingCredentialProfile,
    CredentialNeedsAuthentication,
    InvalidCredentialBinding,
    MissingOutputFolder,
    InvalidOutputConfiguration,
    UnsupportedSchemaVersion,
    SecretFieldRejected,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFolder {
    pub id: String,
    pub label: String,
    pub path: String,
    pub kind: WorkspaceFolderKind,
    pub access: WorkspaceFolderAccess,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProductOutputFolder {
    pub id: String,
    pub label: String,
    pub path: String,
    pub product_types: Vec<ProductType>,
    pub is_default: bool,
    pub create_if_missing: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CredentialBinding {
    pub id: String,
    pub label: String,
    pub credential_profile_id: String,
    pub target_kind: CredentialTargetKind,
    pub target_id: String,
    pub is_default: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: String,
    pub schema_version: u32,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub working_folder: String,
    #[serde(default)]
    pub folders: Vec<WorkspaceFolder>,
    pub product_output_folders: Vec<ProductOutputFolder>,
    #[serde(default)]
    pub credential_bindings: Vec<CredentialBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_credential_binding_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub last_opened_at: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceMutation {
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub working_folder: String,
    #[serde(default)]
    pub folders: Vec<WorkspaceFolder>,
    pub product_output_folders: Vec<ProductOutputFolder>,
    #[serde(default)]
    pub credential_bindings: Vec<CredentialBinding>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_credential_binding_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_model: Option<String>,
}

impl From<&Workspace> for WorkspaceMutation {
    fn from(workspace: &Workspace) -> Self {
        Self {
            name: workspace.name.clone(),
            description: workspace.description.clone(),
            icon: workspace.icon.clone(),
            working_folder: workspace.working_folder.clone(),
            folders: workspace.folders.clone(),
            product_output_folders: workspace.product_output_folders.clone(),
            credential_bindings: workspace.credential_bindings.clone(),
            default_credential_binding_id: workspace.default_credential_binding_id.clone(),
            default_provider: workspace.default_provider.clone(),
            default_model: workspace.default_model.clone(),
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfile {
    pub id: String,
    pub name: String,
    pub provider_or_service_id: String,
    pub auth_kind: CredentialAuthKind,
    #[serde(default)]
    pub configured_secret_fields: Vec<String>,
    #[serde(default)]
    pub non_secret_fields: BTreeMap<String, String>,
    pub status: CredentialProfileStatus,
    pub source: CredentialProfileSource,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceIssue {
    pub code: WorkspaceIssueCode,
    pub severity: WorkspaceIssueSeverity,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceValidationReport {
    pub valid_for_session: bool,
    #[serde(default)]
    pub issues: Vec<WorkspaceIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub normalized_working_folder: Option<String>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSessionContext {
    pub workspace_id: String,
    pub workspace_name: String,
    pub primary_working_folder: String,
    #[serde(default)]
    pub folders: Vec<WorkspaceFolder>,
    pub product_output_folders: Vec<ProductOutputFolder>,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceWithValidation {
    pub workspace: Workspace,
    pub validation: WorkspaceValidationReport,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CredentialFieldUpdate {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/workspaces/list", response = WorkspaceListResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListRequest {}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceListResponse {
    pub workspaces: Vec<WorkspaceWithValidation>,
    pub active_workspace_id: String,
    pub default_workspace_id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/workspaces/create", response = WorkspaceResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreateRequest {
    pub workspace: WorkspaceMutation,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/workspaces/update", response = WorkspaceResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceUpdateRequest {
    pub workspace_id: String,
    pub workspace: WorkspaceMutation,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/workspaces/duplicate", response = WorkspaceResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDuplicateRequest {
    pub workspace_id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceResponse {
    pub workspace: Workspace,
    pub validation: WorkspaceValidationReport,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/workspaces/delete", response = WorkspaceDeleteResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeleteRequest {
    pub workspace_id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceDeleteResponse {
    pub active_workspace_id: String,
    pub default_workspace_id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/workspaces/active/set", response = WorkspaceResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSetActiveRequest {
    pub workspace_id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/workspaces/validate", response = WorkspaceValidationResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceValidateRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workspace_id: Option<String>,
    pub workspace: WorkspaceMutation,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceValidationResponse {
    pub validation: WorkspaceValidationReport,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/workspaces/export", response = WorkspaceExportResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceExportRequest {
    pub workspace_id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceExportResponse {
    pub document: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/workspaces/import", response = WorkspaceResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceImportRequest {
    pub document: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/workspaces/output/create", response = WorkspaceValidationResponse)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceCreateOutputFolderRequest {
    pub workspace_id: String,
    pub output_folder_id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/credential-profiles/list", response = CredentialProfileListResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileListRequest {}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileListResponse {
    pub profiles: Vec<CredentialProfile>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/credential-profiles/create", response = CredentialProfileResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileCreateRequest {
    pub name: String,
    pub provider_or_service_id: String,
    pub auth_kind: CredentialAuthKind,
    #[serde(default)]
    pub non_secret_fields: BTreeMap<String, String>,
    #[serde(default)]
    pub secret_fields: Vec<CredentialFieldUpdate>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/credential-profiles/update", response = CredentialProfileResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileUpdateRequest {
    pub profile_id: String,
    pub name: String,
    pub auth_kind: CredentialAuthKind,
    #[serde(default)]
    pub non_secret_fields: BTreeMap<String, String>,
    #[serde(default)]
    pub secret_fields: Vec<CredentialFieldUpdate>,
    #[serde(default)]
    pub clear_secret_fields: Vec<String>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileResponse {
    pub profile: CredentialProfile,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/credential-profiles/delete", response = CredentialProfileDeleteResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileDeleteRequest {
    pub profile_id: String,
    #[serde(default)]
    pub confirm_referenced: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileDeleteResponse {
    pub deleted: bool,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/credential-profiles/usage", response = CredentialProfileUsageResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileUsageRequest {
    pub profile_id: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileWorkspaceReference {
    pub workspace_id: String,
    pub workspace_name: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileUsageResponse {
    pub workspaces: Vec<CredentialProfileWorkspaceReference>,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcRequest)]
#[request(method = "_gosling/unstable/credential-profiles/test", response = CredentialProfileTestResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileTestRequest {
    pub profile_id: String,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize, JsonSchema, JsonRpcResponse)]
#[serde(rename_all = "camelCase")]
pub struct CredentialProfileTestResponse {
    pub status: CredentialProfileStatus,
    pub supported: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace_fixture() -> Workspace {
        Workspace {
            id: "workspace-id".to_string(),
            schema_version: WORKSPACE_SCHEMA_VERSION,
            name: "Annual Meeting".to_string(),
            working_folder: "/projects/annual-meeting".to_string(),
            product_output_folders: vec![ProductOutputFolder {
                id: "output-id".to_string(),
                label: "Documents".to_string(),
                path: "/projects/annual-meeting/outputs".to_string(),
                product_types: vec![ProductType::Document],
                is_default: true,
                create_if_missing: true,
            }],
            created_at: "2026-07-18T00:00:00Z".to_string(),
            updated_at: "2026-07-18T00:00:00Z".to_string(),
            last_opened_at: "2026-07-18T00:00:00Z".to_string(),
            ..Workspace::default()
        }
    }

    #[test]
    fn workspace_round_trips_through_json() {
        let workspace = workspace_fixture();
        let json = serde_json::to_string(&workspace).unwrap();

        assert_eq!(serde_json::from_str::<Workspace>(&json).unwrap(), workspace);
    }

    #[test]
    fn credential_profile_serialization_has_no_secret_value_field() {
        let profile = CredentialProfile {
            id: "profile-id".to_string(),
            name: "Provider profile".to_string(),
            provider_or_service_id: "openai".to_string(),
            configured_secret_fields: vec!["OPENAI_API_KEY".to_string()],
            ..CredentialProfile::default()
        };

        let value = serde_json::to_value(profile).unwrap();
        let object = value.as_object().unwrap();
        assert!(!object.contains_key("secretFields"));
        assert!(!object.contains_key("value"));
        assert_eq!(object["configuredSecretFields"][0], "OPENAI_API_KEY");
    }

    #[test]
    fn workspace_mutation_is_a_full_editable_projection() {
        let workspace = workspace_fixture();

        assert_eq!(WorkspaceMutation::from(&workspace).name, workspace.name);
        assert_eq!(
            WorkspaceMutation::from(&workspace).product_output_folders,
            workspace.product_output_folders
        );
    }
}
