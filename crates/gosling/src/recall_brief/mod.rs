//! Read-only interpretation of authorized, exact-revision Muninn recall results.

mod evidence;
mod proposal;
mod render;

pub use evidence::{normalize_recall_result, EvidenceBundle};
pub use proposal::synthesize_brief;
pub use render::{evidence_only_brief, unavailable_brief};
