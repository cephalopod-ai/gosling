use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    AlwaysAllow,
    /// Like `AlwaysAllow`, but scoped to the single domain an egress prompt
    /// flagged rather than the tool as a whole.
    AlwaysAllowDomain,
    /// Adds the folder a working-directory scope prompt flagged to this
    /// session's allowed folders.
    AllowFolderForSession,
    /// Adds the folder a working-directory scope prompt flagged to the
    /// operator's trusted folders, which every session allows.
    AlwaysAllowFolder,
    AllowOnce,
    Cancel,
    DenyOnce,
    AlwaysDeny,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, ToSchema)]
pub enum PrincipalType {
    Extension,
    Tool,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct PermissionConfirmation {
    pub principal_type: PrincipalType,
    pub permission: Permission,
}
