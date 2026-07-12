//! Phase 1 contracts for the future Tagteam workflow.
//!
//! This module is feature-gated and has no production transport or UI wiring.

mod client;
mod contracts;
mod policy;
mod reducer;
mod store;

pub use client::{TagteamClient, TagteamClientError};
pub use contracts::*;
pub use policy::{PolicyDecision, StewardAction, StewardCapabilityPolicy};
pub use reducer::{render_deterministic_status, ReduceOutcome, ReducerError, TagteamEventReducer};
pub use store::{BindingUpdate, TagteamRunBinding, TagteamRunStore};

pub const TAGTEAM_WORKFLOW_FEATURE_ENABLED: bool = cfg!(feature = "tagteam-workflow");
