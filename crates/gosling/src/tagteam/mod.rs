//! Phase 1 contracts for the future Tagteam workflow.
//!
//! This module is feature-gated and has no production transport or UI wiring.

mod client;
mod contracts;
mod mcp;
mod policy;
mod reducer;
mod store;

pub use client::{
    validate_run_snapshot, TagteamClient, TagteamClientError, MAX_SNAPSHOT_CHANGED_PATHS,
    MAX_SNAPSHOT_TEXT_BYTES,
};
pub use contracts::*;
pub use mcp::{
    ControlApproval, ControlDiagnostics, ControlFinding, ControlLaunchValidation, ControlPage,
    ControlPlanItem, ControlRecoveryAssessment, ControlRunStatus, ControlStartPreparation,
    McpTagteamClient, TagteamControlClient, TagteamControlError,
};
pub use policy::{PolicyDecision, StewardAction, StewardCapabilityPolicy};
pub use reducer::{
    render_deterministic_status, DiffSummary, ReduceOutcome, ReducerError, RunClass,
    TagteamEventReducer, TagteamObservation, TagteamRunSnapshot,
};
pub use store::{BindingUpdate, TagteamRunBinding, TagteamRunStore};

pub const TAGTEAM_WORKFLOW_FEATURE_ENABLED: bool = cfg!(feature = "tagteam-workflow");
