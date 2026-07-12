use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StewardAction {
    Status,
    Plan,
    Findings,
    Diagnostics,
    PrepareResume,
    Start,
    Resume,
    Cancel,
    Shell,
    FileMutation,
    ExtensionManagement,
    DelegateSubagent,
    WidenScope,
    DeferFinding,
    Transfer,
    InvokeTagteam,
    UnrelatedMcp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    AllowRead,
    RequireApproval,
    Deny,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct StewardCapabilityPolicy;

impl StewardCapabilityPolicy {
    pub fn phase_one() -> Self {
        Self
    }

    pub fn decide(self, action: StewardAction) -> PolicyDecision {
        match action {
            StewardAction::Status
            | StewardAction::Plan
            | StewardAction::Findings
            | StewardAction::Diagnostics
            | StewardAction::PrepareResume => PolicyDecision::AllowRead,
            StewardAction::Start | StewardAction::Resume | StewardAction::Cancel => {
                PolicyDecision::RequireApproval
            }
            StewardAction::Shell
            | StewardAction::FileMutation
            | StewardAction::ExtensionManagement
            | StewardAction::DelegateSubagent
            | StewardAction::WidenScope
            | StewardAction::DeferFinding
            | StewardAction::Transfer
            | StewardAction::InvokeTagteam
            | StewardAction::UnrelatedMcp => PolicyDecision::Deny,
        }
    }

    pub fn exposed_tool_names(self) -> &'static [&'static str] {
        &[
            "tagteam_status",
            "tagteam_plan",
            "tagteam_findings",
            "tagteam_diagnostics",
            "tagteam_prepare_resume",
        ]
    }

    pub fn approval_bound_action_names(self) -> &'static [&'static str] {
        &["tagteam_start", "tagteam_resume", "tagteam_cancel"]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn phase_one_inventory_is_exact_and_read_only() {
        let policy = StewardCapabilityPolicy::phase_one();
        assert_eq!(
            policy.exposed_tool_names(),
            [
                "tagteam_status",
                "tagteam_plan",
                "tagteam_findings",
                "tagteam_diagnostics",
                "tagteam_prepare_resume",
            ]
        );
        for action in [
            StewardAction::Status,
            StewardAction::Plan,
            StewardAction::Findings,
            StewardAction::Diagnostics,
            StewardAction::PrepareResume,
        ] {
            assert_eq!(policy.decide(action), PolicyDecision::AllowRead);
        }
    }

    #[test]
    fn mutation_and_inherited_authority_are_denied() {
        let policy = StewardCapabilityPolicy::phase_one();
        for action in [
            StewardAction::Shell,
            StewardAction::FileMutation,
            StewardAction::ExtensionManagement,
            StewardAction::DelegateSubagent,
            StewardAction::WidenScope,
            StewardAction::DeferFinding,
            StewardAction::Transfer,
            StewardAction::InvokeTagteam,
            StewardAction::UnrelatedMcp,
        ] {
            assert_eq!(policy.decide(action), PolicyDecision::Deny);
        }
    }

    #[test]
    fn mutating_tagteam_actions_are_not_exposed_in_phase_one() {
        let policy = StewardCapabilityPolicy::phase_one();
        for action in [
            StewardAction::Start,
            StewardAction::Resume,
            StewardAction::Cancel,
        ] {
            assert_eq!(policy.decide(action), PolicyDecision::RequireApproval);
        }
        assert_eq!(
            policy.approval_bound_action_names(),
            ["tagteam_start", "tagteam_resume", "tagteam_cancel"]
        );
        assert!(policy
            .exposed_tool_names()
            .iter()
            .all(|name| !policy.approval_bound_action_names().contains(name)));
    }
}
