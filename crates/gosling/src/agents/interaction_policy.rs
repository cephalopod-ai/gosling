//! Host-enforced tool capabilities for durable planning turns.
//!
//! Capability is derived from the concrete host extension configuration, never
//! from model-visible names, descriptions, annotations, or MCP-owned metadata.

use crate::agents::extension::ExtensionConfig;
use crate::config::permission::{PermissionLevel, PermissionManager};
use crate::session::{InteractionPolicy, PlanStatus, SessionManager};
use rmcp::model::{ErrorCode, ErrorData};

pub const PLANNING_EXTENSION_NAME: &str = "planning";
pub const SESSION_HISTORY_EXTENSION_NAME: &str = "session_history";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HostExtensionKind {
    Platform,
    Builtin,
    Frontend,
    Stdio,
    StreamableHttp,
    Sse,
    InlinePython,
}

impl HostExtensionKind {
    fn from_config(config: &ExtensionConfig) -> Self {
        match config {
            ExtensionConfig::Platform { .. } => Self::Platform,
            ExtensionConfig::Builtin { .. } => Self::Builtin,
            ExtensionConfig::Frontend { .. } => Self::Frontend,
            ExtensionConfig::Stdio { .. } => Self::Stdio,
            ExtensionConfig::StreamableHttp { .. } => Self::StreamableHttp,
            ExtensionConfig::Sse { .. } => Self::Sse,
            ExtensionConfig::InlinePython { .. } => Self::InlinePython,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostToolIdentity {
    extension_kind: HostExtensionKind,
    extension_name: String,
    tool_name: String,
}

impl HostToolIdentity {
    pub(crate) fn from_extension_config(config: &ExtensionConfig, tool_name: &str) -> Self {
        Self {
            extension_kind: HostExtensionKind::from_config(config),
            extension_name: config.key(),
            tool_name: tool_name.to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PlanningCapability {
    WorkspaceTree,
    WorkspaceReadText,
    WorkspaceSearchText,
    PlanUpdate,
    PlanRequestReview,
    SessionSearch,
    SessionRead,
}

impl PlanningCapability {
    pub(crate) const FIXED_V1: [(Self, &'static str, &'static str); 7] = [
        (
            Self::WorkspaceTree,
            PLANNING_EXTENSION_NAME,
            "workspace_tree",
        ),
        (
            Self::WorkspaceReadText,
            PLANNING_EXTENSION_NAME,
            "workspace_read_text",
        ),
        (
            Self::WorkspaceSearchText,
            PLANNING_EXTENSION_NAME,
            "workspace_search_text",
        ),
        (Self::PlanUpdate, PLANNING_EXTENSION_NAME, "plan_update"),
        (
            Self::PlanRequestReview,
            PLANNING_EXTENSION_NAME,
            "plan_request_review",
        ),
        (
            Self::SessionSearch,
            SESSION_HISTORY_EXTENSION_NAME,
            "session_search",
        ),
        (
            Self::SessionRead,
            SESSION_HISTORY_EXTENSION_NAME,
            "session_read",
        ),
    ];

    pub(crate) fn fixed_v1_capabilities(
    ) -> impl ExactSizeIterator<Item = (Self, &'static str, &'static str)> {
        Self::FIXED_V1.into_iter()
    }

    pub(crate) fn is_capability_owner_extension(extension_name: &str) -> bool {
        Self::fixed_v1_capabilities()
            .any(|(_, registered_extension, _)| registered_extension == extension_name)
    }

    pub fn from_host_identity(identity: &HostToolIdentity) -> Option<Self> {
        if identity.extension_kind != HostExtensionKind::Platform {
            return None;
        }
        match (
            identity.extension_name.as_str(),
            identity.tool_name.as_str(),
        ) {
            (PLANNING_EXTENSION_NAME, "workspace_tree") => Some(Self::WorkspaceTree),
            (PLANNING_EXTENSION_NAME, "workspace_read_text") => Some(Self::WorkspaceReadText),
            (PLANNING_EXTENSION_NAME, "workspace_search_text") => Some(Self::WorkspaceSearchText),
            (PLANNING_EXTENSION_NAME, "plan_update") => Some(Self::PlanUpdate),
            (PLANNING_EXTENSION_NAME, "plan_request_review") => Some(Self::PlanRequestReview),
            (SESSION_HISTORY_EXTENSION_NAME, "session_search") => Some(Self::SessionSearch),
            (SESSION_HISTORY_EXTENSION_NAME, "session_read") => Some(Self::SessionRead),
            _ => None,
        }
    }

    fn reuses_normal_permission(self) -> bool {
        matches!(self, Self::SessionSearch | Self::SessionRead)
    }

    pub(crate) fn is_permitted_for_catalog(
        self,
        stored_permission: Option<PermissionLevel>,
    ) -> bool {
        !self.reuses_normal_permission() || stored_permission != Some(PermissionLevel::NeverAllow)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DispatchOrigin {
    ModelNative,
    #[allow(dead_code)]
    Toolshim,
    Frontend,
    AppDirect,
    AgentDirect,
    #[allow(dead_code)]
    CodeModeNested,
    #[allow(dead_code)]
    ProviderOwned,
}

impl DispatchOrigin {
    fn allowed_during_planning(self) -> bool {
        matches!(self, Self::ModelNative)
    }
}

pub fn planning_capability_for_dispatch(
    origin: DispatchOrigin,
    identity: &HostToolIdentity,
) -> Option<PlanningCapability> {
    origin
        .allowed_during_planning()
        .then(|| PlanningCapability::from_host_identity(identity))
        .flatten()
}

pub fn evaluate_planning_candidate(
    status: PlanStatus,
    origin: DispatchOrigin,
    identity: Option<&HostToolIdentity>,
    stored_permission: Option<PermissionLevel>,
) -> Result<PlanningCapability, PlanningDenialReason> {
    if status != PlanStatus::Drafting {
        return Err(PlanningDenialReason::PlanNotDrafting);
    }
    if !origin.allowed_during_planning() {
        return Err(PlanningDenialReason::OriginNotHostMediated);
    }
    let capability = identity
        .and_then(|identity| planning_capability_for_dispatch(origin, identity))
        .ok_or(PlanningDenialReason::CapabilityNotGranted)?;
    if !capability.is_permitted_for_catalog(stored_permission) {
        return Err(PlanningDenialReason::PersistedNeverAllow);
    }
    Ok(capability)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanningDenialReason {
    PolicySnapshotMismatch,
    PlanNotDrafting,
    OriginNotHostMediated,
    CapabilityNotGranted,
    PersistedNeverAllow,
}

impl PlanningDenialReason {
    fn code(self) -> &'static str {
        match self {
            Self::PolicySnapshotMismatch => "planning_policy_snapshot_mismatch",
            Self::PlanNotDrafting => "planning_plan_not_drafting",
            Self::OriginNotHostMediated => "planning_dispatch_origin_denied",
            Self::CapabilityNotGranted => "planning_capability_denied",
            Self::PersistedNeverAllow => "planning_saved_denial",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionAuthorization {
    Normal,
    Planning(PlanningCapability),
}

fn denial(tool_name: &str, reason: PlanningDenialReason) -> ErrorData {
    tracing::warn!(
        security.event_type = "planning_dispatch_denied",
        security.reason = reason.code(),
        tool.name = tool_name,
        "host planning boundary denied a tool call before dispatch"
    );
    ErrorData::new(
        ErrorCode::INVALID_REQUEST,
        format!(
            "Tool `{tool_name}` is denied by the host-enforced planning boundary ({})",
            reason.code()
        ),
        Some(serde_json::json!({
            "code": reason.code(),
            "retryable": false,
            "approvalAvailable": false
        })),
    )
}

pub fn policy_denial(tool_name: &str) -> ErrorData {
    denial(tool_name, PlanningDenialReason::CapabilityNotGranted)
}

pub fn atomic_policy_denial(tool_name: &str, detail: impl std::fmt::Display) -> ErrorData {
    tracing::warn!(
        security.event_type = "planning_atomic_dispatch_denied",
        tool.name = tool_name,
        reason = %detail,
        "durable planning state changed before tool-operation begin"
    );
    ErrorData::new(
        ErrorCode::INVALID_REQUEST,
        format!(
            "Tool `{tool_name}` was denied because the durable planning state changed before dispatch"
        ),
        Some(serde_json::json!({
            "code": "planning_atomic_authorization_denied",
            "retryable": false,
            "approvalAvailable": false
        })),
    )
}

pub fn catalog_tool_is_visible(
    policy: &InteractionPolicy,
    identity: Option<&HostToolIdentity>,
) -> bool {
    match policy {
        InteractionPolicy::Normal => identity.is_none_or(|identity| {
            !(identity.extension_kind == HostExtensionKind::Platform
                && identity.extension_name == PLANNING_EXTENSION_NAME)
        }),
        InteractionPolicy::Planning { .. } => identity
            .and_then(PlanningCapability::from_host_identity)
            .is_some(),
    }
}

pub async fn authorize_tool_execution(
    session_manager: &SessionManager,
    permission_manager: Option<&PermissionManager>,
    session_id: &str,
    turn_policy: &InteractionPolicy,
    origin: DispatchOrigin,
    identity: Option<&HostToolIdentity>,
    public_tool_name: &str,
) -> Result<ExecutionAuthorization, ErrorData> {
    let durable = session_manager
        .plans()
        .snapshot(session_id)
        .await
        .map_err(|error| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Could not verify durable planning state: {error}"),
                Some(serde_json::json!({
                    "code": "planning_state_unavailable",
                    "retryable": false,
                    "approvalAvailable": false
                })),
            )
        })?;

    match turn_policy {
        InteractionPolicy::Normal => {
            if durable
                .as_ref()
                .is_some_and(|snapshot| snapshot.plan.status.is_open())
            {
                return Err(denial(
                    public_tool_name,
                    PlanningDenialReason::PolicySnapshotMismatch,
                ));
            }
            if identity.is_some_and(|identity| {
                identity.extension_kind == HostExtensionKind::Platform
                    && identity.extension_name == PLANNING_EXTENSION_NAME
            }) {
                return Err(denial(
                    public_tool_name,
                    PlanningDenialReason::CapabilityNotGranted,
                ));
            }
            Ok(ExecutionAuthorization::Normal)
        }
        InteractionPolicy::Planning {
            plan_id,
            generation,
            capability_policy_version,
        } => {
            let Some(snapshot) = durable else {
                return Err(denial(
                    public_tool_name,
                    PlanningDenialReason::PolicySnapshotMismatch,
                ));
            };
            if snapshot.plan.id != *plan_id
                || snapshot.plan.generation != *generation
                || snapshot.plan.capability_policy_version != *capability_policy_version
            {
                return Err(denial(
                    public_tool_name,
                    PlanningDenialReason::PolicySnapshotMismatch,
                ));
            }
            let stored_permission = permission_manager
                .and_then(|manager| manager.get_user_permission(public_tool_name));
            let capability = evaluate_planning_candidate(
                snapshot.plan.status,
                origin,
                identity,
                stored_permission,
            )
            .map_err(|reason| denial(public_tool_name, reason))?;
            Ok(ExecutionAuthorization::Planning(capability))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(kind: HostExtensionKind, extension: &str, tool: &str) -> HostToolIdentity {
        HostToolIdentity {
            extension_kind: kind,
            extension_name: extension.to_string(),
            tool_name: tool.to_string(),
        }
    }

    #[test]
    fn capability_registry_has_exactly_seven_typed_host_identities() {
        let allowed = PlanningCapability::fixed_v1_capabilities().collect::<Vec<_>>();
        assert_eq!(allowed.len(), 7);
        for (_, extension, tool) in allowed {
            assert!(PlanningCapability::from_host_identity(&identity(
                HostExtensionKind::Platform,
                extension,
                tool
            ))
            .is_some());
        }
    }

    #[test]
    fn names_and_annotations_cannot_mint_host_capability() {
        assert_eq!(
            PlanningCapability::from_host_identity(&identity(
                HostExtensionKind::StreamableHttp,
                PLANNING_EXTENSION_NAME,
                "workspace_read_text"
            )),
            None
        );
        assert_eq!(
            PlanningCapability::from_host_identity(&identity(
                HostExtensionKind::Platform,
                PLANNING_EXTENSION_NAME,
                "workspace_read"
            )),
            None
        );
        assert_eq!(
            PlanningCapability::from_host_identity(&identity(
                HostExtensionKind::Platform,
                PLANNING_EXTENSION_NAME,
                "shell"
            )),
            None
        );
    }

    #[test]
    fn planning_catalog_is_fail_closed_for_unknown_owners() {
        let policy = InteractionPolicy::Planning {
            plan_id: "plan".into(),
            generation: 1,
            capability_policy_version: 1,
        };
        assert!(!catalog_tool_is_visible(&policy, None));
        assert!(!catalog_tool_is_visible(
            &policy,
            Some(&identity(
                HostExtensionKind::Frontend,
                PLANNING_EXTENSION_NAME,
                "workspace_tree"
            ))
        ));
    }

    #[test]
    fn planning_saved_denials_only_narrow_reused_session_history_capabilities() {
        assert!(!PlanningCapability::SessionRead
            .is_permitted_for_catalog(Some(PermissionLevel::NeverAllow)));
        assert!(PlanningCapability::SessionSearch
            .is_permitted_for_catalog(Some(PermissionLevel::AskBefore)));
        assert!(PlanningCapability::WorkspaceReadText
            .is_permitted_for_catalog(Some(PermissionLevel::NeverAllow)));
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ConformanceCase {
        name: String,
        host_kind: String,
        extension: String,
        tool: String,
        origin: String,
        allowed: bool,
    }

    #[test]
    fn fixed_v1_conformance_matrix() {
        let cases: Vec<ConformanceCase> = serde_json::from_str(include_str!(
            "../../tests/fixtures/planning_dispatch_conformance.json"
        ))
        .expect("planning conformance fixture must parse");
        assert!(!cases.is_empty());
        for case in cases {
            let kind = match case.host_kind.as_str() {
                "platform" => HostExtensionKind::Platform,
                "frontend" => HostExtensionKind::Frontend,
                "stdio" => HostExtensionKind::Stdio,
                "streamable_http" => HostExtensionKind::StreamableHttp,
                other => panic!("unknown host kind {other} in {}", case.name),
            };
            let origin = match case.origin.as_str() {
                "model_native" => DispatchOrigin::ModelNative,
                "toolshim" => DispatchOrigin::Toolshim,
                "frontend" => DispatchOrigin::Frontend,
                "app_direct" => DispatchOrigin::AppDirect,
                "agent_direct" => DispatchOrigin::AgentDirect,
                "code_mode_nested" => DispatchOrigin::CodeModeNested,
                "provider_owned" => DispatchOrigin::ProviderOwned,
                other => panic!("unknown origin {other} in {}", case.name),
            };
            let identity = identity(kind, &case.extension, &case.tool);
            assert_eq!(
                planning_capability_for_dispatch(origin, &identity).is_some(),
                case.allowed,
                "{}",
                case.name
            );
        }
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct ComposedDecisionCase {
        case_id: String,
        rationale: String,
        tool_origin: String,
        canonical_host_identity: Option<FixtureIdentity>,
        public_tool_name: String,
        annotations: serde_json::Value,
        arguments: serde_json::Value,
        gosling_mode: String,
        stored_permission: Option<String>,
        working_directory_restriction: serde_json::Value,
        interaction_policy: FixturePolicy,
        expected_final_result: String,
        expected_mandatory_inspector: Option<String>,
        expected_finding: Option<String>,
    }

    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct FixtureIdentity {
        host_kind: String,
        extension: String,
        tool: String,
    }

    #[derive(serde::Deserialize)]
    struct FixturePolicy {
        kind: String,
        status: String,
        generation: u64,
    }

    #[test]
    fn planning_composed_decision_corpus_is_normative_and_never_approves() {
        let cases: Vec<ComposedDecisionCase> = serde_json::from_str(include_str!(
            "../../tests/fixtures/permission_cases/planning_composed_decisions.json"
        ))
        .expect("composed planning decisions fixture must parse");
        assert!(cases.len() >= 20);

        for case in cases {
            assert_eq!(case.interaction_policy.kind, "planning", "{}", case.case_id);
            assert!(case.interaction_policy.generation > 0, "{}", case.case_id);
            assert!(!case.rationale.trim().is_empty(), "{}", case.case_id);
            assert!(!case.public_tool_name.is_empty(), "{}", case.case_id);
            assert!(case.arguments.is_object(), "{}", case.case_id);
            assert!(case.annotations.is_object(), "{}", case.case_id);
            assert!(
                case.working_directory_restriction.is_object(),
                "{}",
                case.case_id
            );
            assert!(
                matches!(
                    case.gosling_mode.as_str(),
                    "auto" | "approve" | "smart_approve" | "chat"
                ),
                "{}",
                case.case_id
            );

            let identity = case.canonical_host_identity.map(|fixture_identity| {
                let kind = match fixture_identity.host_kind.as_str() {
                    "platform" => HostExtensionKind::Platform,
                    "stdio" => HostExtensionKind::Stdio,
                    "streamable_http" => HostExtensionKind::StreamableHttp,
                    "frontend" => HostExtensionKind::Frontend,
                    other => panic!("unknown host kind {other} in {}", case.case_id),
                };
                identity(kind, &fixture_identity.extension, &fixture_identity.tool)
            });
            let origin = match case.tool_origin.as_str() {
                "builtin" | "mcp" => DispatchOrigin::ModelNative,
                "frontend" => DispatchOrigin::Frontend,
                "direct_app" => DispatchOrigin::AppDirect,
                "code_mode" => DispatchOrigin::CodeModeNested,
                "provider_owned" => DispatchOrigin::ProviderOwned,
                other => panic!("unknown tool origin {other} in {}", case.case_id),
            };
            let status = match case.interaction_policy.status.as_str() {
                "drafting" => PlanStatus::Drafting,
                "awaiting_review" => PlanStatus::AwaitingReview,
                "approved" => PlanStatus::Approved,
                "abandoned" => PlanStatus::Abandoned,
                "stale" => PlanStatus::Stale,
                other => panic!("unknown plan status {other} in {}", case.case_id),
            };
            let stored_permission = match case.stored_permission.as_deref() {
                None => None,
                Some("always_allow") => Some(PermissionLevel::AlwaysAllow),
                Some("ask_before") => Some(PermissionLevel::AskBefore),
                Some("never_allow") => Some(PermissionLevel::NeverAllow),
                Some(other) => panic!("unknown stored permission {other} in {}", case.case_id),
            };

            let decision =
                evaluate_planning_candidate(status, origin, identity.as_ref(), stored_permission);
            let actual_result = if decision.is_ok() { "allow" } else { "deny" };
            assert_ne!(case.expected_final_result, "approval", "{}", case.case_id);
            assert_eq!(
                actual_result, case.expected_final_result,
                "{}",
                case.case_id
            );
            match decision {
                Ok(_) => {
                    assert!(
                        case.expected_mandatory_inspector.is_none(),
                        "{}",
                        case.case_id
                    );
                    assert!(case.expected_finding.is_none(), "{}", case.case_id);
                }
                Err(reason) => {
                    assert_eq!(
                        case.expected_mandatory_inspector.as_deref(),
                        Some("host_planning_boundary"),
                        "{}",
                        case.case_id
                    );
                    assert_eq!(
                        case.expected_finding.as_deref(),
                        Some(reason.code()),
                        "{}",
                        case.case_id
                    );
                }
            }
        }
    }
}
