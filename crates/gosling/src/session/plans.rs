use super::SessionManager;
use crate::providers::base::Provider;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;
use utoipa::ToSchema;

pub const PLAN_CAPABILITY_POLICY_VERSION: u32 = 1;
pub const PLAN_CONTENT_MAX_BYTES: usize = 256 * 1024;
pub const PLAN_FEEDBACK_MAX_BYTES: usize = 32 * 1024;
pub const PLAN_DECISION_NOTE_MAX_BYTES: usize = 32 * 1024;
pub const PLAN_SELECTED_TEXT_PREVIEW_MAX_CHARS: usize = 512;
pub const PLAN_SNAPSHOT_FEEDBACK_LIMIT: usize = 100;
pub const PLAN_SNAPSHOT_EVENT_LIMIT: usize = 100;
/// Transfer limits for the optional `plan_history_v1` native session section.
///
/// These totals apply across every generation in one transferred session. They
/// bound both untrusted import admission and same-store export/copy work.
pub const PLAN_HISTORY_MAX_PLANS: usize = 256;
pub const PLAN_HISTORY_MAX_REVISIONS: usize = 1_024;
pub const PLAN_HISTORY_MAX_FEEDBACK: usize = 1_024;
pub const PLAN_HISTORY_MAX_EVENTS: usize = 4_096;
pub const PLAN_HISTORY_MAX_EVENT_METADATA_BYTES: usize = 4 * 1024;
// Large enough for a maximum-size 32 KiB decision note plus its JSON envelope.
pub const PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES: usize = 64 * 1024;
/// Sum of UTF-8 bytes in transferred payload fields. Locally re-keyed identity
/// and provenance fields are excluded; collection counts separately bound
/// their fixed JSON/row overhead.
pub const PLAN_HISTORY_MAX_AGGREGATE_BYTES: usize = 8 * 1024 * 1024;
pub const PLAN_CONTENT_HASH_VERSION: &str = "sha256";
// Schema 35 and the v1 source/scope hashes are still unshipped. The accepted
// canonical contract therefore corrects their missing collection counts in
// place instead of preserving an unpublished encoding under a misleading v1
// tag or introducing a migration for development-only database state.
pub const PLAN_SOURCE_HASH_VERSION: &str = "source_hash_v1";
pub const PLAN_SCOPE_HASH_VERSION: &str = "scope_hash_v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, strum::Display)]
#[serde(rename_all = "snake_case")]
#[strum(serialize_all = "snake_case")]
pub enum PlanStatus {
    Drafting,
    AwaitingReview,
    Approved,
    Abandoned,
    Stale,
}

impl PlanStatus {
    pub fn is_open(self) -> bool {
        matches!(self, Self::Drafting | Self::AwaitingReview)
    }
}

impl std::str::FromStr for PlanStatus {
    type Err = PlanError;

    fn from_str(value: &str) -> PlanResult<Self> {
        match value {
            "drafting" => Ok(Self::Drafting),
            "awaiting_review" => Ok(Self::AwaitingReview),
            "approved" => Ok(Self::Approved),
            "abandoned" => Ok(Self::Abandoned),
            "stale" => Ok(Self::Stale),
            _ => Err(PlanError::CorruptState(format!(
                "unknown stored plan status `{value}`"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum OpenPlanDisposition {
    Stale,
    Abandon,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlan {
    pub id: String,
    pub session_id: String,
    pub generation: u64,
    pub status: PlanStatus,
    pub active_revision_id: Option<String>,
    pub source_through_row_id: Option<i64>,
    pub source_hash: String,
    pub scope_hash: String,
    pub capability_policy_version: u32,
    pub planner_provider: Option<String>,
    pub planner_model: Option<String>,
    pub stale_reason: Option<String>,
    pub derived_from_session_id: Option<String>,
    pub derived_from_plan_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlanRevision {
    pub id: String,
    pub plan_id: String,
    pub revision: u64,
    pub parent_revision_id: Option<String>,
    pub content_markdown: String,
    pub content_sha256: String,
    pub planner_provider: Option<String>,
    pub planner_model: Option<String>,
    pub source_through_row_id: Option<i64>,
    pub source_hash: String,
    pub scope_hash: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanRevisionIdentity {
    pub id: String,
    pub revision: u64,
    pub content_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanUpdate {
    pub plan_id: String,
    pub session_id: String,
    pub generation: u64,
    pub status: PlanStatus,
    pub active_revision: Option<PlanRevisionIdentity>,
    pub updated_at: DateTime<Utc>,
}

impl From<&PlanSnapshot> for PlanUpdate {
    fn from(snapshot: &PlanSnapshot) -> Self {
        Self {
            plan_id: snapshot.plan.id.clone(),
            session_id: snapshot.plan.session_id.clone(),
            generation: snapshot.plan.generation,
            status: snapshot.plan.status,
            active_revision: snapshot.active_revision.as_ref().map(|revision| {
                PlanRevisionIdentity {
                    id: revision.id.clone(),
                    revision: revision.revision,
                    content_sha256: revision.content_sha256.clone(),
                }
            }),
            updated_at: snapshot.plan.updated_at,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlanFeedback {
    pub id: String,
    pub plan_id: String,
    pub revision_id: String,
    pub body: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub selected_text_sha256: Option<String>,
    pub selected_text_preview: Option<String>,
    pub consumed_by_revision_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionPlanEvent {
    pub id: i64,
    pub plan_id: String,
    pub event_type: String,
    pub from_status: Option<PlanStatus>,
    pub to_status: Option<PlanStatus>,
    pub revision_id: Option<String>,
    pub revision_sha256: Option<String>,
    pub actor: String,
    pub detail: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanSnapshot {
    pub plan: SessionPlan,
    pub active_revision: Option<SessionPlanRevision>,
    pub feedback: Vec<SessionPlanFeedback>,
    pub recent_events: Vec<SessionPlanEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanExpectation {
    pub generation: u64,
    pub revision_id: Option<String>,
    pub revision_sha256: Option<String>,
    pub source_hash: String,
    pub scope_hash: String,
}

impl PlanExpectation {
    pub fn for_plan(plan: &SessionPlan) -> Self {
        Self {
            generation: plan.generation,
            revision_id: None,
            revision_sha256: None,
            source_hash: plan.source_hash.clone(),
            scope_hash: plan.scope_hash.clone(),
        }
    }

    pub fn for_snapshot(snapshot: &PlanSnapshot) -> Self {
        let revision = snapshot.active_revision.as_ref();
        Self {
            generation: snapshot.plan.generation,
            revision_id: revision.map(|value| value.id.clone()),
            revision_sha256: revision.map(|value| value.content_sha256.clone()),
            source_hash: revision
                .map(|value| value.source_hash.clone())
                .unwrap_or_else(|| snapshot.plan.source_hash.clone()),
            scope_hash: revision
                .map(|value| value.scope_hash.clone())
                .unwrap_or_else(|| snapshot.plan.scope_hash.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPlanRevision {
    pub content_markdown: String,
    pub expected_generation: u64,
    pub expected_parent_revision_id: Option<String>,
    pub planner_provider: Option<String>,
    pub planner_model: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewPlanFeedback {
    pub body: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub selected_text: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case", tag = "kind", content = "plan")]
pub enum InteractionPolicy {
    Normal,
    Planning {
        plan_id: String,
        generation: u64,
        capability_policy_version: u32,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("Session not found: {0}")]
    SessionNotFound(String),
    #[error("No open plan exists for session {0}")]
    NoOpenPlan(String),
    #[error("Plan not found: {0}")]
    PlanNotFound(String),
    #[error("Plan state conflict: {0}")]
    Conflict(String),
    #[error("Invalid plan transition: {0}")]
    InvalidTransition(String),
    #[error("Invalid plan input: {0}")]
    InvalidInput(String),
    #[error("Plan limit exceeded: {0}")]
    LimitExceeded(String),
    #[error("Plan operation is busy: {0}")]
    Busy(String),
    #[error("Planning is unsupported by this provider: {0}")]
    ProviderUnsupported(String),
    #[error("Stored plan state is invalid: {0}")]
    CorruptState(String),
    #[error("Plan storage failed: {0}")]
    Storage(String),
}

pub type PlanResult<T> = std::result::Result<T, PlanError>;

impl From<sqlx::Error> for PlanError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

impl From<anyhow::Error> for PlanError {
    fn from(error: anyhow::Error) -> Self {
        Self::Storage(error.to_string())
    }
}

#[derive(Clone)]
pub struct PlanService {
    session_manager: SessionManager,
}

impl fmt::Debug for PlanService {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PlanService")
            .finish_non_exhaustive()
    }
}

impl PlanService {
    pub fn new(session_manager: SessionManager) -> Self {
        Self { session_manager }
    }

    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<PlanUpdate> {
        self.session_manager.storage().subscribe_plan_updates()
    }

    pub async fn snapshot(&self, session_id: &str) -> PlanResult<Option<PlanSnapshot>> {
        self.session_manager
            .storage()
            .plan_snapshot(session_id)
            .await
    }

    pub async fn snapshot_generation(
        &self,
        session_id: &str,
        generation: u64,
    ) -> PlanResult<Option<PlanSnapshot>> {
        self.session_manager
            .storage()
            .plan_snapshot_generation(session_id, generation)
            .await
    }

    pub async fn export_markdown(
        &self,
        session_id: &str,
        generation: u64,
        expected_status: PlanStatus,
        revision_id: &str,
        revision_sha256: &str,
    ) -> PlanResult<String> {
        let snapshot = self
            .snapshot_generation(session_id, generation)
            .await?
            .ok_or_else(|| {
                PlanError::PlanNotFound(format!("session {session_id} generation {generation}"))
            })?;
        if snapshot.plan.status != expected_status {
            return Err(PlanError::Conflict(format!(
                "plan status changed from expected {expected_status} to {}",
                snapshot.plan.status
            )));
        }
        let revision = snapshot.active_revision.as_ref().ok_or_else(|| {
            PlanError::InvalidTransition("the selected plan has no active revision".to_string())
        })?;
        if revision.id != revision_id || revision.content_sha256 != revision_sha256 {
            return Err(PlanError::Conflict(
                "the selected plan revision or content hash changed".to_string(),
            ));
        }
        let provider = serde_json::to_string(&revision.planner_provider)
            .map_err(|error| PlanError::Storage(error.to_string()))?;
        let model = serde_json::to_string(&revision.planner_model)
            .map_err(|error| PlanError::Storage(error.to_string()))?;
        Ok(format!(
            "# Gosling Plan\n\n- Plan ID: `{}`\n- Generation: {}\n- Status: `{}`\n- Revision ID: `{}`\n- Revision SHA-256: `{}`\n- Source hash: `{}`\n- Scope hash: `{}`\n- Planner provider: {}\n- Planner model: {}\n\n---\n\n{}",
            snapshot.plan.id,
            snapshot.plan.generation,
            snapshot.plan.status,
            revision.id,
            revision.content_sha256,
            revision.source_hash,
            revision.scope_hash,
            provider,
            model,
            revision.content_markdown
        ))
    }

    pub async fn interaction_policy(&self, session_id: &str) -> PlanResult<InteractionPolicy> {
        let Some(plan) = self
            .session_manager
            .storage()
            .latest_plan_status(session_id)
            .await?
        else {
            return Ok(InteractionPolicy::Normal);
        };
        if plan.status.is_open() {
            Ok(InteractionPolicy::Planning {
                plan_id: plan.id,
                generation: plan.generation,
                capability_policy_version: plan.capability_policy_version,
            })
        } else {
            Ok(InteractionPolicy::Normal)
        }
    }

    pub async fn approved_implementation_context(
        &self,
        session_id: &str,
        reference: &str,
    ) -> PlanResult<Option<String>> {
        let Some(snapshot) = self
            .session_manager
            .storage()
            .resolve_approved_implementation_reference(session_id, reference)
            .await?
        else {
            return Ok(None);
        };
        let revision = snapshot.active_revision.as_ref().ok_or_else(|| {
            PlanError::CorruptState(format!(
                "approved plan {} has no active revision",
                snapshot.plan.id
            ))
        })?;
        serde_json::to_string(&serde_json::json!({
            "planId": snapshot.plan.id,
            "generation": snapshot.plan.generation,
            "revisionId": revision.id,
            "revisionSha256": revision.content_sha256,
            "sourceHash": revision.source_hash,
            "scopeHash": revision.scope_hash,
            "contentMarkdown": revision.content_markdown,
        }))
        .map(Some)
        .map_err(|error| PlanError::Storage(error.to_string()))
    }

    pub async fn start_or_resume(
        &self,
        session_id: &str,
        provider: &dyn Provider,
        planner_model: Option<String>,
        expected_generation: Option<u64>,
    ) -> PlanResult<PlanSnapshot> {
        if provider.executes_tools_outside_gosling() {
            return Err(PlanError::ProviderUnsupported(
                "the selected provider executes tools outside gosling and cannot provide host-enforced planning"
                    .to_string(),
            ));
        }
        let snapshot = self
            .session_manager
            .storage()
            .start_or_resume_plan(
                session_id,
                Some(provider.get_name().to_string()),
                planner_model,
                expected_generation,
                None,
            )
            .await?;
        self.session_manager
            .storage()
            .publish_plan_update(&snapshot);
        Ok(snapshot)
    }

    pub async fn start_new(
        &self,
        session_id: &str,
        provider: &dyn Provider,
        planner_model: Option<String>,
        expected_generation: Option<u64>,
        replace_open: OpenPlanDisposition,
    ) -> PlanResult<PlanSnapshot> {
        if provider.executes_tools_outside_gosling() {
            return Err(PlanError::ProviderUnsupported(
                "the selected provider executes tools outside gosling and cannot provide host-enforced planning"
                    .to_string(),
            ));
        }
        let snapshot = self
            .session_manager
            .storage()
            .start_or_resume_plan(
                session_id,
                Some(provider.get_name().to_string()),
                planner_model,
                expected_generation,
                Some(replace_open),
            )
            .await?;
        self.session_manager
            .storage()
            .publish_plan_update(&snapshot);
        Ok(snapshot)
    }

    pub async fn update_revision(
        &self,
        session_id: &str,
        revision: NewPlanRevision,
    ) -> PlanResult<PlanSnapshot> {
        let content = normalize_plan_markdown(&revision.content_markdown);
        if content.trim().is_empty() {
            return Err(PlanError::InvalidInput(
                "plan Markdown must not be empty".to_string(),
            ));
        }
        if content.len() > PLAN_CONTENT_MAX_BYTES {
            return Err(PlanError::LimitExceeded(format!(
                "plan Markdown is {} bytes; maximum is {PLAN_CONTENT_MAX_BYTES}",
                content.len()
            )));
        }
        let snapshot = self
            .session_manager
            .storage()
            .insert_plan_revision(session_id, revision, content)
            .await?;
        self.session_manager
            .storage()
            .publish_plan_update(&snapshot);
        Ok(snapshot)
    }

    pub async fn request_review(
        &self,
        session_id: &str,
        expectation: &PlanExpectation,
    ) -> PlanResult<PlanSnapshot> {
        let snapshot = self
            .session_manager
            .storage()
            .request_plan_review(session_id, expectation)
            .await?;
        self.session_manager
            .storage()
            .publish_plan_update(&snapshot);
        Ok(snapshot)
    }

    pub async fn add_feedback(
        &self,
        session_id: &str,
        expectation: &PlanExpectation,
        feedback: NewPlanFeedback,
    ) -> PlanResult<PlanSnapshot> {
        validate_feedback(&feedback)?;
        let snapshot = self
            .session_manager
            .storage()
            .add_plan_feedback(session_id, expectation, feedback)
            .await?;
        self.session_manager
            .storage()
            .publish_plan_update(&snapshot);
        Ok(snapshot)
    }

    pub async fn approve(
        &self,
        session_id: &str,
        expectation: &PlanExpectation,
        decision_note: Option<String>,
    ) -> PlanResult<PlanSnapshot> {
        validate_optional_note(decision_note.as_deref())?;
        let snapshot = self
            .session_manager
            .storage()
            .approve_plan(session_id, expectation, decision_note)
            .await?;
        self.session_manager
            .storage()
            .publish_plan_update(&snapshot);
        Ok(snapshot)
    }

    pub async fn abandon(
        &self,
        session_id: &str,
        expected_generation: u64,
        note: Option<String>,
    ) -> PlanResult<PlanSnapshot> {
        validate_optional_note(note.as_deref())?;
        let snapshot = self
            .session_manager
            .storage()
            .abandon_plan(session_id, expected_generation, note)
            .await?;
        self.session_manager
            .storage()
            .publish_plan_update(&snapshot);
        Ok(snapshot)
    }

    pub async fn mark_stale(&self, session_id: &str, reason: &str) -> PlanResult<bool> {
        if reason.trim().is_empty() {
            return Err(PlanError::InvalidInput(
                "stale reason must not be empty".to_string(),
            ));
        }
        let changed = self
            .session_manager
            .storage()
            .stale_open_plan(session_id, reason)
            .await?;
        if changed {
            if let Some(snapshot) = self.snapshot(session_id).await? {
                self.session_manager
                    .storage()
                    .publish_plan_update(&snapshot);
            }
        }
        Ok(changed)
    }
}

pub fn approved_plan_implementation_reference(snapshot: &PlanSnapshot) -> PlanResult<String> {
    if snapshot.plan.status != PlanStatus::Approved {
        return Err(PlanError::InvalidTransition(format!(
            "plan {} is not approved",
            snapshot.plan.id
        )));
    }
    let revision = snapshot.active_revision.as_ref().ok_or_else(|| {
        PlanError::CorruptState(format!(
            "approved plan {} has no active revision",
            snapshot.plan.id
        ))
    })?;
    Ok(format!(
        "Implement approved plan {} generation {} revision {} ({}; source {}; scope {}). Follow the stored plan exactly; report deviations.",
        snapshot.plan.id,
        snapshot.plan.generation,
        revision.id,
        revision.content_sha256,
        revision.source_hash,
        revision.scope_hash
    ))
}

pub fn normalize_plan_markdown(content: &str) -> String {
    content.replace("\r\n", "\n").replace('\r', "\n")
}

pub fn plan_content_sha256(content: &str) -> String {
    sha256_hex(content.as_bytes())
}

pub(crate) fn versioned_sha256(version: &str, parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(version.as_bytes());
    hasher.update([0]);
    for part in parts {
        hasher.update((part.len() as u64).to_le_bytes());
        hasher.update(part);
    }
    format!("{version}:{}", hex_digest(hasher.finalize()))
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex_digest(Sha256::digest(bytes))
}

fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    bytes
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn validate_feedback(feedback: &NewPlanFeedback) -> PlanResult<()> {
    if feedback.body.trim().is_empty() {
        return Err(PlanError::InvalidInput(
            "feedback body must not be empty".to_string(),
        ));
    }
    if feedback.body.len() > PLAN_FEEDBACK_MAX_BYTES {
        return Err(PlanError::LimitExceeded(format!(
            "feedback is {} bytes; maximum is {PLAN_FEEDBACK_MAX_BYTES}",
            feedback.body.len()
        )));
    }
    match (feedback.start_line, feedback.end_line) {
        (None, None) => {
            if feedback.selected_text.is_some() {
                return Err(PlanError::InvalidInput(
                    "selected text requires a line range".to_string(),
                ));
            }
        }
        (Some(start), Some(end)) if start > 0 && start <= end => {}
        (Some(_), Some(_)) => {
            return Err(PlanError::InvalidInput(
                "feedback lines must be positive and start must not exceed end".to_string(),
            ));
        }
        _ => {
            return Err(PlanError::InvalidInput(
                "feedback line range requires both start and end".to_string(),
            ));
        }
    }
    Ok(())
}

fn validate_optional_note(note: Option<&str>) -> PlanResult<()> {
    if let Some(note) = note {
        if note.len() > PLAN_DECISION_NOTE_MAX_BYTES {
            return Err(PlanError::LimitExceeded(format!(
                "plan note is {} bytes; maximum is {PLAN_DECISION_NOTE_MAX_BYTES}",
                note.len()
            )));
        }
    }
    Ok(())
}

pub(crate) fn selected_text_metadata(
    selected_text: Option<&str>,
) -> (Option<String>, Option<String>) {
    match selected_text {
        Some(text) => (
            Some(sha256_hex(text.as_bytes())),
            Some(
                text.chars()
                    .take(PLAN_SELECTED_TEXT_PREVIEW_MAX_CHARS)
                    .collect(),
            ),
        ),
        None => (None, None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GoslingMode;
    use crate::conversation::message::Message;
    use crate::session::SessionType;
    use tempfile::TempDir;

    async fn session_with_open_plan() -> (TempDir, SessionManager, PlanSnapshot) {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().join("data"));
        let session = manager
            .create_session(
                temp_dir.path().to_path_buf(),
                "Plan test".to_string(),
                SessionType::User,
                GoslingMode::Chat,
            )
            .await
            .unwrap();
        manager
            .add_message(&session.id, &Message::user().with_text("Design the change"))
            .await
            .unwrap();
        let snapshot = manager
            .storage()
            .start_or_resume_plan(
                &session.id,
                Some("test".to_string()),
                Some("test-model".to_string()),
                None,
                None,
            )
            .await
            .unwrap();
        (temp_dir, manager, snapshot)
    }

    async fn revision_awaiting_review(
        manager: &SessionManager,
        snapshot: PlanSnapshot,
    ) -> PlanSnapshot {
        let snapshot = manager
            .plans()
            .update_revision(
                &snapshot.plan.session_id,
                NewPlanRevision {
                    content_markdown: "# Plan\n\n1. Change it.\n".to_string(),
                    expected_generation: snapshot.plan.generation,
                    expected_parent_revision_id: snapshot.plan.active_revision_id.clone(),
                    planner_provider: Some("test".to_string()),
                    planner_model: Some("test-model".to_string()),
                },
            )
            .await
            .unwrap();
        manager
            .plans()
            .request_review(
                &snapshot.plan.session_id,
                &PlanExpectation::for_snapshot(&snapshot),
            )
            .await
            .unwrap()
    }

    #[test]
    fn normalizes_newlines_before_hashing() {
        let normalized = normalize_plan_markdown("one\r\ntwo\rthree\n");
        assert_eq!(normalized, "one\ntwo\nthree\n");
        assert_eq!(
            plan_content_sha256(&normalized),
            "b6285c57e8797db5d4c51c80d6f11938afda9b11c6a003549709189e9b4b92a2"
        );
    }

    #[tokio::test]
    async fn source_hash_has_a_byte_exact_fixture() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().join("data"));
        let session = manager
            .create_session(
                temp_dir.path().to_path_buf(),
                "Source hash fixture".to_string(),
                SessionType::User,
                GoslingMode::Chat,
            )
            .await
            .unwrap();
        manager
            .add_message(
                &session.id,
                &Message::user()
                    .with_id("source-fixture-message")
                    .with_text("Design the change"),
            )
            .await
            .unwrap();
        let snapshot = manager
            .storage()
            .start_or_resume_plan(&session.id, None, None, None, None)
            .await
            .unwrap();

        assert_eq!(snapshot.plan.source_through_row_id, Some(1));
        assert_eq!(
            snapshot.plan.source_hash,
            "source_hash_v1:15025f2e43c3ce350d43b279c6f13a51b2954c0106a7030b8fe098a9462f7848"
        );
    }

    #[tokio::test]
    async fn markdown_export_status_cas_rejects_lifecycle_races() {
        let (_temp_dir, manager, started) = session_with_open_plan().await;
        let drafting = manager
            .plans()
            .update_revision(
                &started.plan.session_id,
                NewPlanRevision {
                    content_markdown: "# Export race\n".to_string(),
                    expected_generation: started.plan.generation,
                    expected_parent_revision_id: None,
                    planner_provider: None,
                    planner_model: None,
                },
            )
            .await
            .unwrap();
        let revision = drafting.active_revision.as_ref().unwrap();
        assert!(manager
            .plans()
            .export_markdown(
                &drafting.plan.session_id,
                drafting.plan.generation,
                PlanStatus::Drafting,
                &revision.id,
                &revision.content_sha256,
            )
            .await
            .unwrap()
            .contains("- Status: `drafting`"));

        let awaiting = manager
            .plans()
            .request_review(
                &drafting.plan.session_id,
                &PlanExpectation::for_snapshot(&drafting),
            )
            .await
            .unwrap();
        let error = manager
            .plans()
            .export_markdown(
                &awaiting.plan.session_id,
                awaiting.plan.generation,
                PlanStatus::Drafting,
                &revision.id,
                &revision.content_sha256,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, PlanError::Conflict(_)));
        assert!(manager
            .plans()
            .export_markdown(
                &awaiting.plan.session_id,
                awaiting.plan.generation,
                PlanStatus::AwaitingReview,
                &revision.id,
                &revision.content_sha256,
            )
            .await
            .is_ok());

        let approved = manager
            .plans()
            .approve(
                &awaiting.plan.session_id,
                &PlanExpectation::for_snapshot(&awaiting),
                None,
            )
            .await
            .unwrap();
        let error = manager
            .plans()
            .export_markdown(
                &approved.plan.session_id,
                approved.plan.generation,
                PlanStatus::AwaitingReview,
                &revision.id,
                &revision.content_sha256,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, PlanError::Conflict(_)));
        assert!(manager
            .plans()
            .export_markdown(
                &approved.plan.session_id,
                approved.plan.generation,
                PlanStatus::Approved,
                &revision.id,
                &revision.content_sha256,
            )
            .await
            .is_ok());
    }

    #[test]
    fn feedback_validation_requires_complete_positive_ranges() {
        let valid = NewPlanFeedback {
            body: "Change this".to_string(),
            start_line: Some(1),
            end_line: Some(2),
            selected_text: Some("line one\nline two".to_string()),
        };
        assert!(validate_feedback(&valid).is_ok());

        for feedback in [
            NewPlanFeedback {
                start_line: Some(0),
                ..valid.clone()
            },
            NewPlanFeedback {
                start_line: Some(3),
                end_line: Some(2),
                ..valid.clone()
            },
            NewPlanFeedback {
                end_line: None,
                ..valid.clone()
            },
        ] {
            assert!(validate_feedback(&feedback).is_err());
        }
    }

    #[tokio::test]
    async fn approved_reference_resolves_the_frozen_revision_server_side() {
        let (_temp_dir, manager, started) = session_with_open_plan().await;
        let awaiting = revision_awaiting_review(&manager, started).await;
        let approved = manager
            .plans()
            .approve(
                &awaiting.plan.session_id,
                &PlanExpectation::for_snapshot(&awaiting),
                None,
            )
            .await
            .unwrap();
        let reference = approved_plan_implementation_reference(&approved).unwrap();
        let revision = approved.active_revision.as_ref().unwrap();

        assert!(!reference.contains("# Plan"));
        assert!(reference.contains(&format!("generation {}", approved.plan.generation)));
        assert!(reference.contains(&format!("source {}", revision.source_hash)));
        assert!(reference.contains(&format!("scope {}", revision.scope_hash)));
        let context = manager
            .plans()
            .approved_implementation_context(&approved.plan.session_id, &reference)
            .await
            .unwrap()
            .unwrap();
        assert!(context.contains("# Plan\\n\\n1. Change it."));
        assert!(context.contains(revision.content_sha256.as_str()));
        assert!(context.contains(revision.source_hash.as_str()));
        assert!(context.contains(revision.scope_hash.as_str()));
        assert!(manager
            .plans()
            .approved_implementation_context(&approved.plan.session_id, "Implement some other plan")
            .await
            .unwrap()
            .is_none());
        assert!(manager
            .plans()
            .approved_implementation_context(&approved.plan.session_id, &format!("{reference}\n"))
            .await
            .unwrap()
            .is_none());

        manager
            .update(&approved.plan.session_id)
            .working_dir(std::env::temp_dir())
            .apply()
            .await
            .unwrap();
        let error = manager
            .plans()
            .approved_implementation_context(&approved.plan.session_id, &reference)
            .await
            .unwrap_err();
        assert!(matches!(error, PlanError::Conflict(_)));
    }

    #[tokio::test]
    async fn approved_reference_rejects_edits_to_its_bound_source_prefix() {
        let (_temp_dir, manager, started) = session_with_open_plan().await;
        let awaiting = revision_awaiting_review(&manager, started).await;
        let approved = manager
            .plans()
            .approve(
                &awaiting.plan.session_id,
                &PlanExpectation::for_snapshot(&awaiting),
                None,
            )
            .await
            .unwrap();
        let reference = approved_plan_implementation_reference(&approved).unwrap();
        let source_message_id = manager
            .get_session(&approved.plan.session_id, true)
            .await
            .unwrap()
            .conversation
            .unwrap()
            .messages()[0]
            .id
            .clone()
            .unwrap();

        manager
            .upsert_message(
                &approved.plan.session_id,
                &crate::conversation::message::Message::user()
                    .with_id(source_message_id)
                    .with_text("edited source evidence"),
            )
            .await
            .unwrap();

        let error = manager
            .plans()
            .approved_implementation_context(&approved.plan.session_id, &reference)
            .await
            .unwrap_err();
        assert!(matches!(error, PlanError::Conflict(_)));
    }

    #[tokio::test]
    async fn lifecycle_persists_without_mutating_session_mode_or_history() {
        let (temp_dir, manager, started) = session_with_open_plan().await;
        assert_eq!(started.plan.status, PlanStatus::Drafting);
        assert!(matches!(
            manager
                .plans()
                .interaction_policy(&started.plan.session_id)
                .await
                .unwrap(),
            InteractionPolicy::Planning { generation: 1, .. }
        ));

        let review = revision_awaiting_review(&manager, started).await;
        let approved = manager
            .plans()
            .approve(
                &review.plan.session_id,
                &PlanExpectation::for_snapshot(&review),
                Some("Reviewed".to_string()),
            )
            .await
            .unwrap();
        assert_eq!(approved.plan.status, PlanStatus::Approved);

        let restarted = SessionManager::new(temp_dir.path().join("data"));
        let persisted = restarted
            .plans()
            .snapshot(&approved.plan.session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(persisted.plan.status, PlanStatus::Approved);
        assert_eq!(persisted.active_revision, approved.active_revision);
        let session = restarted
            .get_session(&approved.plan.session_id, true)
            .await
            .unwrap();
        assert_eq!(session.gosling_mode, GoslingMode::Chat);
        assert_eq!(session.conversation.unwrap().len(), 1);
        assert_eq!(
            restarted
                .plans()
                .interaction_policy(&approved.plan.session_id)
                .await
                .unwrap(),
            InteractionPolicy::Normal
        );
    }

    #[tokio::test]
    async fn approval_rejects_transcript_changes_after_review() {
        let (_temp_dir, manager, started) = session_with_open_plan().await;
        let review = revision_awaiting_review(&manager, started).await;
        manager
            .add_message(
                &review.plan.session_id,
                &Message::user().with_text("New evidence"),
            )
            .await
            .unwrap();
        let error = manager
            .plans()
            .approve(
                &review.plan.session_id,
                &PlanExpectation::for_snapshot(&review),
                None,
            )
            .await
            .unwrap_err();
        assert!(matches!(error, PlanError::Conflict(_)));
        assert_eq!(
            manager
                .plans()
                .snapshot(&review.plan.session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::AwaitingReview
        );
    }

    #[tokio::test]
    async fn concurrent_feedback_and_approval_have_one_winner() {
        let (_temp_dir, manager, started) = session_with_open_plan().await;
        let review = revision_awaiting_review(&manager, started).await;
        let expectation = PlanExpectation::for_snapshot(&review);
        let session_id = review.plan.session_id.clone();
        let approve_manager = manager.clone();
        let approve_expectation = expectation.clone();
        let approve_session_id = session_id.clone();
        let approve = tokio::spawn(async move {
            approve_manager
                .plans()
                .approve(&approve_session_id, &approve_expectation, None)
                .await
        });
        let feedback_manager = manager.clone();
        let feedback = tokio::spawn(async move {
            feedback_manager
                .plans()
                .add_feedback(
                    &session_id,
                    &expectation,
                    NewPlanFeedback {
                        body: "Please revise".to_string(),
                        start_line: None,
                        end_line: None,
                        selected_text: None,
                    },
                )
                .await
        });
        let approve = approve.await.unwrap();
        let feedback = feedback.await.unwrap();
        assert_ne!(approve.is_ok(), feedback.is_ok());
        let status = manager
            .plans()
            .snapshot(&review.plan.session_id)
            .await
            .unwrap()
            .unwrap()
            .plan
            .status;
        assert!(matches!(
            status,
            PlanStatus::Approved | PlanStatus::Drafting
        ));
    }

    #[tokio::test]
    async fn replacing_an_open_plan_preserves_one_open_generation() {
        let (_temp_dir, manager, started) = session_with_open_plan().await;
        let replacement = manager
            .storage()
            .start_or_resume_plan(
                &started.plan.session_id,
                Some("test".to_string()),
                Some("next".to_string()),
                Some(started.plan.generation),
                Some(OpenPlanDisposition::Abandon),
            )
            .await
            .unwrap();
        assert_eq!(replacement.plan.generation, 2);
        assert_eq!(replacement.plan.status, PlanStatus::Drafting);
        let pool = manager.storage().pool().await.unwrap();
        let open_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM session_plans WHERE session_id = ? AND status IN ('drafting', 'awaiting_review')",
        )
        .bind(&started.plan.session_id)
        .fetch_one(pool)
        .await
        .unwrap();
        assert_eq!(open_count, 1);
    }

    #[tokio::test]
    async fn deleting_a_session_deletes_plan_history() {
        let (_temp_dir, manager, started) = session_with_open_plan().await;
        manager
            .delete_session(&started.plan.session_id)
            .await
            .unwrap();
        let pool = manager.storage().pool().await.unwrap();
        for table in [
            "session_plans",
            "session_plan_revisions",
            "session_plan_feedback",
            "session_plan_events",
        ] {
            let count: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM {table}"))
                .fetch_one(pool)
                .await
                .unwrap();
            assert_eq!(count, 0, "{table} should be empty");
        }
    }

    #[tokio::test]
    async fn migration_35_matches_the_fresh_plan_schema() {
        let temp_dir = TempDir::new().unwrap();
        let data_dir = temp_dir.path().join("data");
        let manager = SessionManager::new(data_dir.clone());
        manager.healthy().await.unwrap();
        let fresh = {
            let pool = manager.storage().pool().await.unwrap();
            sqlx::query_as::<_, (String, String, String)>(
                r#"
                SELECT type, name, sql FROM sqlite_master
                WHERE (name LIKE 'session_plan%' OR name LIKE 'idx_session_plan%')
                  AND sql IS NOT NULL
                ORDER BY type, name
                "#,
            )
            .fetch_all(pool)
            .await
            .unwrap()
        };
        {
            let pool = manager.storage().pool().await.unwrap();
            for trigger in [
                "session_plan_source_session_after_insert",
                "session_plan_source_after_insert",
                "session_plan_source_after_delete",
                "session_plan_source_after_update",
            ] {
                sqlx::query(&format!("DROP TRIGGER {trigger}"))
                    .execute(pool)
                    .await
                    .unwrap();
            }
            for table in [
                "session_plan_feedback",
                "session_plan_events",
                "session_plan_revisions",
                "session_plans",
                "session_plan_source_versions",
            ] {
                sqlx::query(&format!("DROP TABLE {table}"))
                    .execute(pool)
                    .await
                    .unwrap();
            }
            sqlx::query("UPDATE schema_version SET version = 34")
                .execute(pool)
                .await
                .unwrap();
        }
        drop(manager);

        let upgraded = SessionManager::new(data_dir);
        upgraded.healthy().await.unwrap();
        let migrated = {
            let pool = upgraded.storage().pool().await.unwrap();
            sqlx::query_as::<_, (String, String, String)>(
                r#"
                SELECT type, name, sql FROM sqlite_master
                WHERE (name LIKE 'session_plan%' OR name LIKE 'idx_session_plan%')
                  AND sql IS NOT NULL
                ORDER BY type, name
                "#,
            )
            .fetch_all(pool)
            .await
            .unwrap()
        };
        assert_eq!(migrated, fresh);
    }
}
