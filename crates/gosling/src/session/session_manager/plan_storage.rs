use super::SessionStorage;
use crate::conversation::message::{MessageContent, MessageMetadata};
use crate::session::plans::{
    approved_plan_implementation_reference, plan_content_sha256, selected_text_metadata,
    sha256_hex, versioned_sha256, NewPlanFeedback, NewPlanRevision, OpenPlanDisposition, PlanError,
    PlanExpectation, PlanResult, PlanSnapshot, PlanStatus, SessionPlan, SessionPlanEvent,
    SessionPlanFeedback, SessionPlanRevision, PLAN_CAPABILITY_POLICY_VERSION,
    PLAN_CONTENT_MAX_BYTES, PLAN_FEEDBACK_MAX_BYTES, PLAN_HISTORY_MAX_AGGREGATE_BYTES,
    PLAN_HISTORY_MAX_EVENTS, PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES,
    PLAN_HISTORY_MAX_EVENT_METADATA_BYTES, PLAN_HISTORY_MAX_FEEDBACK, PLAN_HISTORY_MAX_PLANS,
    PLAN_HISTORY_MAX_REVISIONS, PLAN_SCOPE_HASH_VERSION, PLAN_SELECTED_TEXT_PREVIEW_MAX_CHARS,
    PLAN_SNAPSHOT_EVENT_LIMIT, PLAN_SNAPSHOT_FEEDBACK_LIMIT, PLAN_SOURCE_HASH_VERSION,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, Sqlite, Transaction};
use std::collections::{HashMap, HashSet};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{LazyLock, Mutex};

const PLAN_UPDATE_TOOL_NAMES: &[&str] = &["planning__plan_update", "plan_update"];
const PLAN_REQUEST_REVIEW_TOOL_NAMES: &[&str] =
    &["planning__plan_request_review", "plan_request_review"];
const SOURCE_ROW_CACHE_LIMIT: usize = 100_000;
const SOURCE_HASH_CACHE_LIMIT: usize = 4_096;

#[derive(Clone)]
struct CachedSourceRow {
    fingerprint: [u8; 32],
    agent_visible: bool,
    visibility_flags: [u8; 3],
    lifecycle_request_ids: Vec<String>,
    requires_lifecycle_filter: bool,
    ordinary_content_hash: Option<String>,
}

static SOURCE_ROW_CACHE: LazyLock<Mutex<HashMap<(String, i64), CachedSourceRow>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub(crate) const NATIVE_PLAN_HISTORY_KEY: &str = "plan_history_v1";
pub(super) const NATIVE_PLAN_HISTORY_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativePlanHistoryV1 {
    pub schema_version: u32,
    #[serde(deserialize_with = "deserialize_plan_records")]
    pub plans: Vec<NativePlanRecordV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct NativePlanRecordV1 {
    pub plan: SessionPlan,
    #[serde(deserialize_with = "deserialize_plan_revisions")]
    pub revisions: Vec<SessionPlanRevision>,
    #[serde(deserialize_with = "deserialize_plan_feedback")]
    pub feedback: Vec<SessionPlanFeedback>,
    #[serde(deserialize_with = "deserialize_plan_events")]
    pub events: Vec<SessionPlanEvent>,
}

fn deserialize_plan_records<'de, D>(deserializer: D) -> Result<Vec<NativePlanRecordV1>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_vec(deserializer, PLAN_HISTORY_MAX_PLANS, "plans")
}

fn deserialize_plan_revisions<'de, D>(deserializer: D) -> Result<Vec<SessionPlanRevision>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_vec(deserializer, PLAN_HISTORY_MAX_REVISIONS, "revisions")
}

fn deserialize_plan_feedback<'de, D>(deserializer: D) -> Result<Vec<SessionPlanFeedback>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_vec(deserializer, PLAN_HISTORY_MAX_FEEDBACK, "feedback")
}

fn deserialize_plan_events<'de, D>(deserializer: D) -> Result<Vec<SessionPlanEvent>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_vec(deserializer, PLAN_HISTORY_MAX_EVENTS, "events")
}

fn deserialize_bounded_vec<'de, D, T>(
    deserializer: D,
    maximum: usize,
    label: &'static str,
) -> Result<Vec<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    struct BoundedVecVisitor<T> {
        maximum: usize,
        label: &'static str,
        marker: std::marker::PhantomData<T>,
    }

    impl<'de, T> serde::de::Visitor<'de> for BoundedVecVisitor<T>
    where
        T: Deserialize<'de>,
    {
        type Value = Vec<T>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(
                formatter,
                "at most {} native plan history {}",
                self.maximum, self.label
            )
        }

        fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
        where
            A: serde::de::SeqAccess<'de>,
        {
            let capacity = sequence.size_hint().unwrap_or_default().min(self.maximum);
            let mut values = Vec::with_capacity(capacity);
            while values.len() < self.maximum {
                let Some(value) = sequence.next_element()? else {
                    return Ok(values);
                };
                values.push(value);
            }
            if sequence.next_element::<serde::de::IgnoredAny>()?.is_some() {
                return Err(serde::de::Error::custom(format!(
                    "native plan history contains more than {} {}",
                    self.maximum, self.label
                )));
            }
            Ok(values)
        }
    }

    deserializer.deserialize_seq(BoundedVecVisitor {
        maximum,
        label,
        marker: std::marker::PhantomData,
    })
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
struct NativePlanHistoryMetrics {
    plans: usize,
    revisions: usize,
    feedback: usize,
    events: usize,
    aggregate_bytes: usize,
    max_event_metadata_bytes: usize,
    max_event_detail_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PlanHistorySelection {
    All,
    Latest,
}

impl SessionStorage {
    pub(crate) fn subscribe_plan_updates(
        &self,
    ) -> tokio::sync::broadcast::Receiver<crate::session::plans::PlanUpdate> {
        self.plan_updates.subscribe()
    }

    pub(crate) fn publish_plan_update(&self, snapshot: &PlanSnapshot) {
        let _ = self.plan_updates.send(snapshot.into());
    }

    pub(crate) async fn resolve_approved_implementation_reference(
        &self,
        session_id: &str,
        reference: &str,
    ) -> PlanResult<Option<PlanSnapshot>> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await.map_err(PlanError::from)?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let Some(plan) = latest_plan_optional(&mut tx, session_id).await? else {
            tx.commit().await?;
            return Ok(None);
        };
        let snapshot = snapshot_for_plan_tx(&mut tx, plan).await?;
        if snapshot.plan.status != PlanStatus::Approved
            || reference != approved_plan_implementation_reference(&snapshot)?
        {
            tx.commit().await?;
            return Ok(None);
        }
        let revision = snapshot.active_revision.as_ref().ok_or_else(|| {
            PlanError::CorruptState(format!(
                "approved plan {} has no active revision",
                snapshot.plan.id
            ))
        })?;
        let (source_through_row_id, source_hash) = self
            .source_hash_through(&mut tx, session_id, revision.source_through_row_id)
            .await?;
        let scope_hash = Self::current_scope_hash_in_tx(&mut tx, session_id).await?;
        if source_through_row_id != revision.source_through_row_id
            || source_hash != revision.source_hash
            || scope_hash != revision.scope_hash
        {
            return Err(PlanError::Conflict(
                "the approved plan source transcript or workspace scope changed before implementation"
                    .to_string(),
            ));
        }
        tx.commit().await?;
        Ok(Some(snapshot))
    }

    pub(super) async fn validate_tool_operation_policy_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
        turn_policy: &crate::session::plans::InteractionPolicy,
        planning_capability_allowed: bool,
    ) -> PlanResult<()> {
        match turn_policy {
            crate::session::plans::InteractionPolicy::Normal => {
                let has_open = sqlx::query_scalar::<_, bool>(
                    "SELECT EXISTS(SELECT 1 FROM session_plans WHERE session_id = ? AND status IN ('drafting', 'awaiting_review'))",
                )
                .bind(session_id)
                .fetch_one(&mut **tx)
                .await?;
                if has_open {
                    return Err(PlanError::Conflict(
                        "the turn's normal policy is stale because a plan is open".to_string(),
                    ));
                }
            }
            crate::session::plans::InteractionPolicy::Planning {
                plan_id,
                generation,
                capability_policy_version,
            } => {
                if !planning_capability_allowed {
                    return Err(PlanError::InvalidTransition(
                        "the resolved host tool has no planning capability".to_string(),
                    ));
                }
                let matches = sqlx::query_scalar::<_, bool>(
                    r#"
                    SELECT EXISTS(
                        SELECT 1 FROM session_plans
                        WHERE id = ? AND session_id = ? AND generation = ?
                          AND capability_policy_version = ? AND status = 'drafting'
                    )
                    "#,
                )
                .bind(plan_id)
                .bind(session_id)
                .bind(i64::try_from(*generation).map_err(|_| {
                    PlanError::InvalidInput("plan generation exceeds SQLite range".to_string())
                })?)
                .bind(i64::from(*capability_policy_version))
                .fetch_one(&mut **tx)
                .await?;
                if !matches {
                    return Err(PlanError::Conflict(
                        "the planning turn no longer names the current drafting generation"
                            .to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    pub(super) async fn create_session_plan_schema(
        tx: &mut Transaction<'_, Sqlite>,
    ) -> anyhow::Result<()> {
        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS session_plans (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id),
                generation INTEGER NOT NULL CHECK(generation >= 1),
                status TEXT NOT NULL CHECK(status IN (
                    'drafting', 'awaiting_review', 'approved', 'abandoned', 'stale'
                )),
                active_revision_id TEXT,
                source_through_row_id INTEGER,
                source_hash TEXT NOT NULL,
                scope_hash TEXT NOT NULL,
                capability_policy_version INTEGER NOT NULL CHECK(capability_policy_version >= 1),
                planner_provider TEXT,
                planner_model TEXT,
                stale_reason TEXT,
                derived_from_session_id TEXT,
                derived_from_plan_id TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                updated_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(session_id, generation)
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            CREATE UNIQUE INDEX IF NOT EXISTS idx_session_plans_one_open
            ON session_plans(session_id)
            WHERE status IN ('drafting', 'awaiting_review')
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_session_plans_session_generation
            ON session_plans(session_id, generation DESC)
            "#,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS session_plan_revisions (
                id TEXT PRIMARY KEY,
                plan_id TEXT NOT NULL REFERENCES session_plans(id),
                revision INTEGER NOT NULL CHECK(revision >= 1),
                parent_revision_id TEXT REFERENCES session_plan_revisions(id),
                content_markdown TEXT NOT NULL,
                content_sha256 TEXT NOT NULL,
                planner_provider TEXT,
                planner_model TEXT,
                source_through_row_id INTEGER,
                source_hash TEXT NOT NULL,
                scope_hash TEXT NOT NULL,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(plan_id, revision)
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_session_plan_revisions_plan_revision
            ON session_plan_revisions(plan_id, revision DESC)
            "#,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS session_plan_feedback (
                id TEXT PRIMARY KEY,
                plan_id TEXT NOT NULL REFERENCES session_plans(id),
                revision_id TEXT NOT NULL REFERENCES session_plan_revisions(id),
                body TEXT NOT NULL,
                start_line INTEGER,
                end_line INTEGER,
                selected_text_sha256 TEXT,
                selected_text_preview TEXT,
                consumed_by_revision_id TEXT REFERENCES session_plan_revisions(id),
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_session_plan_feedback_plan_created
            ON session_plan_feedback(plan_id, created_at DESC, id DESC)
            "#,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS session_plan_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                plan_id TEXT NOT NULL REFERENCES session_plans(id),
                event_type TEXT NOT NULL,
                from_status TEXT,
                to_status TEXT,
                revision_id TEXT,
                revision_sha256 TEXT,
                actor TEXT NOT NULL,
                detail_json TEXT,
                created_at TIMESTAMP NOT NULL DEFAULT CURRENT_TIMESTAMP
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            CREATE INDEX IF NOT EXISTS idx_session_plan_events_plan_id
            ON session_plan_events(plan_id, id DESC)
            "#,
        )
        .execute(&mut **tx)
        .await?;

        sqlx::query(
            r#"
            CREATE TABLE IF NOT EXISTS session_plan_source_versions (
                session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
                revision_token TEXT NOT NULL CHECK(length(revision_token) = 32)
            )
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            INSERT INTO session_plan_source_versions (session_id, revision_token)
            SELECT id, lower(hex(randomblob(16))) FROM sessions
            WHERE TRUE
            ON CONFLICT(session_id) DO NOTHING
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            CREATE TRIGGER IF NOT EXISTS session_plan_source_session_after_insert
            AFTER INSERT ON sessions
            BEGIN
                INSERT INTO session_plan_source_versions (session_id, revision_token)
                VALUES (NEW.id, lower(hex(randomblob(16))))
                ON CONFLICT(session_id) DO NOTHING;
            END
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            CREATE TRIGGER IF NOT EXISTS session_plan_source_after_insert
            AFTER INSERT ON messages
            BEGIN
                INSERT INTO session_plan_source_versions (session_id, revision_token)
                VALUES (NEW.session_id, lower(hex(randomblob(16))))
                ON CONFLICT(session_id) DO UPDATE
                SET revision_token = lower(hex(randomblob(16)));
            END
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            CREATE TRIGGER IF NOT EXISTS session_plan_source_after_delete
            AFTER DELETE ON messages
            BEGIN
                INSERT INTO session_plan_source_versions (session_id, revision_token)
                VALUES (OLD.session_id, lower(hex(randomblob(16))))
                ON CONFLICT(session_id) DO UPDATE
                SET revision_token = lower(hex(randomblob(16)));
            END
            "#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"
            CREATE TRIGGER IF NOT EXISTS session_plan_source_after_update
            AFTER UPDATE OF session_id, message_id, role, content_json, metadata_json ON messages
            BEGIN
                INSERT INTO session_plan_source_versions (session_id, revision_token)
                VALUES (OLD.session_id, lower(hex(randomblob(16))))
                ON CONFLICT(session_id) DO UPDATE
                SET revision_token = lower(hex(randomblob(16)));
                INSERT INTO session_plan_source_versions (session_id, revision_token)
                VALUES (NEW.session_id, lower(hex(randomblob(16))))
                ON CONFLICT(session_id) DO UPDATE
                SET revision_token = lower(hex(randomblob(16)));
            END
            "#,
        )
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub(crate) async fn plan_snapshot(&self, session_id: &str) -> PlanResult<Option<PlanSnapshot>> {
        let pool = self.pool().await.map_err(PlanError::from)?;
        let row = sqlx::query(
            r#"
            SELECT * FROM session_plans
            WHERE session_id = ?
            ORDER BY CASE WHEN status IN ('drafting', 'awaiting_review') THEN 0 ELSE 1 END,
                     generation DESC
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        let plan = plan_from_row(&row)?;
        snapshot_for_plan(pool, plan).await.map(Some)
    }

    pub(crate) async fn plan_snapshot_generation(
        &self,
        session_id: &str,
        generation: u64,
    ) -> PlanResult<Option<PlanSnapshot>> {
        let generation = i64::try_from(generation)
            .map_err(|_| PlanError::InvalidInput("plan generation is too large".to_string()))?;
        let pool = self.pool().await.map_err(PlanError::from)?;
        let row = sqlx::query(
            "SELECT * FROM session_plans WHERE session_id = ? AND generation = ? LIMIT 1",
        )
        .bind(session_id)
        .bind(generation)
        .fetch_optional(pool)
        .await?;
        let Some(row) = row else {
            return Ok(None);
        };
        snapshot_for_plan(pool, plan_from_row(&row)?)
            .await
            .map(Some)
    }

    pub(crate) async fn start_or_resume_plan(
        &self,
        session_id: &str,
        planner_provider: Option<String>,
        planner_model: Option<String>,
        expected_generation: Option<u64>,
        replace_open: Option<OpenPlanDisposition>,
    ) -> PlanResult<PlanSnapshot> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await.map_err(PlanError::from)?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

        ensure_session_not_archived(&mut tx, session_id).await?;
        ensure_session_idle(self, &mut tx, session_id).await?;
        let current_generation = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(generation) FROM session_plans WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&mut *tx)
        .await?
        .map(|value| {
            u64::try_from(value)
                .map_err(|_| PlanError::CorruptState("negative plan generation".to_string()))
        })
        .transpose()?;
        if current_generation != expected_generation {
            return Err(PlanError::Conflict(format!(
                "expected latest generation {:?}, current latest generation is {:?}",
                expected_generation, current_generation
            )));
        }
        if let Some(row) = open_plan_row(&mut tx, session_id).await? {
            let open = plan_from_row(&row)?;
            if let Some(disposition) = replace_open {
                let (status, event, reason) = match disposition {
                    OpenPlanDisposition::Stale => (
                        PlanStatus::Stale,
                        "plan_staled",
                        Some("replaced by a new plan generation"),
                    ),
                    OpenPlanDisposition::Abandon => (PlanStatus::Abandoned, "plan_abandoned", None),
                };
                let changed = sqlx::query(
                    "UPDATE session_plans SET status = ?, stale_reason = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND status IN ('drafting', 'awaiting_review')",
                )
                .bind(status.to_string())
                .bind(reason)
                .bind(&open.id)
                .execute(&mut *tx)
                .await?;
                if changed.rows_affected() != 1 {
                    return Err(PlanError::Conflict(
                        "the open plan changed while a new generation was starting".to_string(),
                    ));
                }
                append_event(
                    &mut tx,
                    &open.id,
                    event,
                    Some(open.status),
                    Some(status),
                    open.active_revision_id.as_deref(),
                    None,
                    "user",
                    reason.map(|value| serde_json::json!({ "reason": value })),
                )
                .await?;
            } else {
                let snapshot = snapshot_for_plan_tx(&mut tx, open).await?;
                tx.commit().await?;
                return Ok(snapshot);
            }
        }

        let generation = current_generation.unwrap_or(0) + 1;
        let (source_through_row_id, source_hash) =
            self.current_source_hash(&mut tx, session_id).await?;
        let scope_hash = Self::current_scope_hash_in_tx(&mut tx, session_id).await?;
        let plan_id = format!("plan_{}", uuid::Uuid::now_v7());
        sqlx::query(
            r#"
            INSERT INTO session_plans (
                id, session_id, generation, status, source_through_row_id,
                source_hash, scope_hash, capability_policy_version,
                planner_provider, planner_model
            ) VALUES (?, ?, ?, 'drafting', ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&plan_id)
        .bind(session_id)
        .bind(i64::try_from(generation).map_err(|_| {
            PlanError::LimitExceeded("plan generation exceeds SQLite range".to_string())
        })?)
        .bind(source_through_row_id)
        .bind(&source_hash)
        .bind(&scope_hash)
        .bind(i64::from(PLAN_CAPABILITY_POLICY_VERSION))
        .bind(&planner_provider)
        .bind(&planner_model)
        .execute(&mut *tx)
        .await?;
        append_event(
            &mut tx,
            &plan_id,
            "plan_started",
            None,
            Some(PlanStatus::Drafting),
            None,
            None,
            "user",
            None,
        )
        .await?;
        let plan = plan_by_id(&mut tx, &plan_id).await?;
        let snapshot = snapshot_for_plan_tx(&mut tx, plan).await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    pub(crate) async fn insert_plan_revision(
        &self,
        session_id: &str,
        revision: NewPlanRevision,
        normalized_content: String,
    ) -> PlanResult<PlanSnapshot> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await.map_err(PlanError::from)?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_session_not_archived(&mut tx, session_id).await?;
        let plan = require_open_plan(&mut tx, session_id).await?;
        if plan.status != PlanStatus::Drafting {
            return Err(PlanError::InvalidTransition(format!(
                "cannot update a revision while plan {} is {}",
                plan.id, plan.status
            )));
        }
        if plan.generation != revision.expected_generation {
            return Err(PlanError::Conflict(format!(
                "expected generation {}, current generation is {}",
                revision.expected_generation, plan.generation
            )));
        }
        if plan.active_revision_id != revision.expected_parent_revision_id {
            return Err(PlanError::Conflict(
                "the active plan revision changed".to_string(),
            ));
        }

        let (source_through_row_id, source_hash) =
            self.current_source_hash(&mut tx, session_id).await?;
        let scope_hash = Self::current_scope_hash_in_tx(&mut tx, session_id).await?;
        let next_revision = sqlx::query_scalar::<_, i64>(
            "SELECT COALESCE(MAX(revision), 0) + 1 FROM session_plan_revisions WHERE plan_id = ?",
        )
        .bind(&plan.id)
        .fetch_one(&mut *tx)
        .await?;
        let revision_id = format!("planrev_{}", uuid::Uuid::now_v7());
        let content_sha256 = plan_content_sha256(&normalized_content);
        sqlx::query(
            r#"
            INSERT INTO session_plan_revisions (
                id, plan_id, revision, parent_revision_id, content_markdown,
                content_sha256, planner_provider, planner_model,
                source_through_row_id, source_hash, scope_hash
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&revision_id)
        .bind(&plan.id)
        .bind(next_revision)
        .bind(&revision.expected_parent_revision_id)
        .bind(&normalized_content)
        .bind(&content_sha256)
        .bind(
            revision
                .planner_provider
                .as_ref()
                .or(plan.planner_provider.as_ref()),
        )
        .bind(
            revision
                .planner_model
                .as_ref()
                .or(plan.planner_model.as_ref()),
        )
        .bind(source_through_row_id)
        .bind(&source_hash)
        .bind(&scope_hash)
        .execute(&mut *tx)
        .await?;
        let changed = sqlx::query(
            r#"
            UPDATE session_plans
            SET active_revision_id = ?, source_through_row_id = ?, source_hash = ?,
                scope_hash = ?, planner_provider = COALESCE(?, planner_provider),
                planner_model = COALESCE(?, planner_model), updated_at = CURRENT_TIMESTAMP
            WHERE id = ? AND generation = ? AND status = 'drafting'
              AND (active_revision_id IS ? OR active_revision_id = ?)
            "#,
        )
        .bind(&revision_id)
        .bind(source_through_row_id)
        .bind(&source_hash)
        .bind(&scope_hash)
        .bind(&revision.planner_provider)
        .bind(&revision.planner_model)
        .bind(&plan.id)
        .bind(plan.generation as i64)
        .bind(&revision.expected_parent_revision_id)
        .bind(&revision.expected_parent_revision_id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PlanError::Conflict(
                "the plan changed while the revision was being stored".to_string(),
            ));
        }
        sqlx::query(
            "UPDATE session_plan_feedback SET consumed_by_revision_id = ? WHERE plan_id = ? AND consumed_by_revision_id IS NULL",
        )
        .bind(&revision_id)
        .bind(&plan.id)
        .execute(&mut *tx)
        .await?;
        append_event(
            &mut tx,
            &plan.id,
            "revision_created",
            Some(PlanStatus::Drafting),
            Some(PlanStatus::Drafting),
            Some(&revision_id),
            Some(&content_sha256),
            "model",
            Some(serde_json::json!({ "revision": next_revision })),
        )
        .await?;
        let plan = plan_by_id(&mut tx, &plan.id).await?;
        let snapshot = snapshot_for_plan_tx(&mut tx, plan).await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    pub(crate) async fn request_plan_review(
        &self,
        session_id: &str,
        expectation: &PlanExpectation,
    ) -> PlanResult<PlanSnapshot> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await.map_err(PlanError::from)?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_session_not_archived(&mut tx, session_id).await?;
        let plan = require_open_plan(&mut tx, session_id).await?;
        if plan.status != PlanStatus::Drafting {
            return Err(PlanError::InvalidTransition(format!(
                "cannot request review while plan {} is {}",
                plan.id, plan.status
            )));
        }
        let revision = require_expected_revision(&mut tx, &plan, expectation).await?;
        ensure_revision_evidence_current(self, &mut tx, session_id, &revision, expectation).await?;
        let changed = sqlx::query(
            "UPDATE session_plans SET status = 'awaiting_review', updated_at = CURRENT_TIMESTAMP WHERE id = ? AND generation = ? AND status = 'drafting' AND active_revision_id = ?",
        )
        .bind(&plan.id)
        .bind(plan.generation as i64)
        .bind(&revision.id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PlanError::Conflict(
                "the plan changed before review could be requested".to_string(),
            ));
        }
        append_event(
            &mut tx,
            &plan.id,
            "review_requested",
            Some(PlanStatus::Drafting),
            Some(PlanStatus::AwaitingReview),
            Some(&revision.id),
            Some(&revision.content_sha256),
            "model",
            None,
        )
        .await?;
        let plan = plan_by_id(&mut tx, &plan.id).await?;
        let snapshot = snapshot_for_plan_tx(&mut tx, plan).await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    pub(crate) async fn add_plan_feedback(
        &self,
        session_id: &str,
        expectation: &PlanExpectation,
        feedback: NewPlanFeedback,
    ) -> PlanResult<PlanSnapshot> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await.map_err(PlanError::from)?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_session_not_archived(&mut tx, session_id).await?;
        let plan = require_open_plan(&mut tx, session_id).await?;
        ensure_session_idle(self, &mut tx, session_id).await?;
        if plan.status != PlanStatus::AwaitingReview {
            return Err(PlanError::InvalidTransition(format!(
                "cannot add review feedback while plan {} is {}",
                plan.id, plan.status
            )));
        }
        let revision = require_expected_revision(&mut tx, &plan, expectation).await?;
        ensure_revision_evidence_current(self, &mut tx, session_id, &revision, expectation).await?;
        validate_feedback_range(&revision, &feedback)?;
        let (selected_text_sha256, selected_text_preview) =
            selected_text_metadata(feedback.selected_text.as_deref());
        let feedback_id = format!("planfb_{}", uuid::Uuid::now_v7());
        sqlx::query(
            r#"
            INSERT INTO session_plan_feedback (
                id, plan_id, revision_id, body, start_line, end_line,
                selected_text_sha256, selected_text_preview
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&feedback_id)
        .bind(&plan.id)
        .bind(&revision.id)
        .bind(&feedback.body)
        .bind(feedback.start_line.map(i64::from))
        .bind(feedback.end_line.map(i64::from))
        .bind(&selected_text_sha256)
        .bind(&selected_text_preview)
        .execute(&mut *tx)
        .await?;
        let changed = sqlx::query(
            "UPDATE session_plans SET status = 'drafting', updated_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'awaiting_review' AND active_revision_id = ?",
        )
        .bind(&plan.id)
        .bind(&revision.id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PlanError::Conflict(
                "the plan changed before feedback could be stored".to_string(),
            ));
        }
        append_event(
            &mut tx,
            &plan.id,
            "feedback_added",
            Some(PlanStatus::AwaitingReview),
            Some(PlanStatus::Drafting),
            Some(&revision.id),
            Some(&revision.content_sha256),
            "user",
            Some(serde_json::json!({ "feedbackId": feedback_id })),
        )
        .await?;
        let plan = plan_by_id(&mut tx, &plan.id).await?;
        let snapshot = snapshot_for_plan_tx(&mut tx, plan).await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    pub(crate) async fn approve_plan(
        &self,
        session_id: &str,
        expectation: &PlanExpectation,
        decision_note: Option<String>,
    ) -> PlanResult<PlanSnapshot> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await.map_err(PlanError::from)?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_session_not_archived(&mut tx, session_id).await?;
        let latest = latest_plan(&mut tx, session_id).await?;
        ensure_session_idle(self, &mut tx, session_id).await?;
        if latest.status == PlanStatus::Approved {
            let revision = require_expected_revision(&mut tx, &latest, expectation).await?;
            let snapshot = snapshot_for_plan_tx(&mut tx, latest).await?;
            tx.commit().await?;
            debug_assert_eq!(snapshot.active_revision.as_ref(), Some(&revision));
            return Ok(snapshot);
        }
        if latest.status != PlanStatus::AwaitingReview {
            return Err(PlanError::InvalidTransition(format!(
                "cannot approve plan {} while it is {}",
                latest.id, latest.status
            )));
        }
        let revision = require_expected_revision(&mut tx, &latest, expectation).await?;
        ensure_revision_evidence_current(self, &mut tx, session_id, &revision, expectation).await?;
        let changed = sqlx::query(
            "UPDATE session_plans SET status = 'approved', updated_at = CURRENT_TIMESTAMP WHERE id = ? AND status = 'awaiting_review' AND active_revision_id = ?",
        )
        .bind(&latest.id)
        .bind(&revision.id)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PlanError::Conflict(
                "the plan changed before approval committed".to_string(),
            ));
        }
        append_event(
            &mut tx,
            &latest.id,
            "plan_approved",
            Some(PlanStatus::AwaitingReview),
            Some(PlanStatus::Approved),
            Some(&revision.id),
            Some(&revision.content_sha256),
            "user",
            decision_note.map(|note| serde_json::json!({ "note": note })),
        )
        .await?;
        let plan = plan_by_id(&mut tx, &latest.id).await?;
        let snapshot = snapshot_for_plan_tx(&mut tx, plan).await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    pub(crate) async fn abandon_plan(
        &self,
        session_id: &str,
        expected_generation: u64,
        note: Option<String>,
    ) -> PlanResult<PlanSnapshot> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await.map_err(PlanError::from)?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        ensure_session_not_archived(&mut tx, session_id).await?;
        let latest = latest_plan(&mut tx, session_id).await?;
        ensure_session_idle(self, &mut tx, session_id).await?;
        if latest.generation != expected_generation {
            return Err(PlanError::Conflict(format!(
                "expected generation {expected_generation}, current generation is {}",
                latest.generation
            )));
        }
        if latest.status == PlanStatus::Abandoned {
            let snapshot = snapshot_for_plan_tx(&mut tx, latest).await?;
            tx.commit().await?;
            return Ok(snapshot);
        }
        if !latest.status.is_open() {
            return Err(PlanError::InvalidTransition(format!(
                "cannot abandon plan {} while it is {}",
                latest.id, latest.status
            )));
        }
        let changed = sqlx::query(
            "UPDATE session_plans SET status = 'abandoned', updated_at = CURRENT_TIMESTAMP WHERE id = ? AND generation = ? AND status IN ('drafting', 'awaiting_review')",
        )
        .bind(&latest.id)
        .bind(expected_generation as i64)
        .execute(&mut *tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PlanError::Conflict(
                "the plan changed before abandon committed".to_string(),
            ));
        }
        append_event(
            &mut tx,
            &latest.id,
            "plan_abandoned",
            Some(latest.status),
            Some(PlanStatus::Abandoned),
            latest.active_revision_id.as_deref(),
            None,
            "user",
            note.map(|note| serde_json::json!({ "note": note })),
        )
        .await?;
        let plan = plan_by_id(&mut tx, &latest.id).await?;
        let snapshot = snapshot_for_plan_tx(&mut tx, plan).await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    pub(crate) async fn stale_open_plan(&self, session_id: &str, reason: &str) -> PlanResult<bool> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await.map_err(PlanError::from)?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let changed = Self::stale_open_plan_in_tx(&mut tx, session_id, reason).await?;
        tx.commit().await?;
        Ok(changed)
    }

    pub(super) async fn stale_open_plan_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
        reason: &str,
    ) -> PlanResult<bool> {
        ensure_session_not_archived(tx, session_id).await?;
        let Some(row) = open_plan_row(tx, session_id).await? else {
            return Ok(false);
        };
        let plan = plan_from_row(&row)?;
        let changed = sqlx::query(
            "UPDATE session_plans SET status = 'stale', stale_reason = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ? AND status IN ('drafting', 'awaiting_review')",
        )
        .bind(reason)
        .bind(&plan.id)
        .execute(&mut **tx)
        .await?;
        if changed.rows_affected() != 1 {
            return Err(PlanError::Conflict(
                "the plan changed before it could be marked stale".to_string(),
            ));
        }
        append_event(
            tx,
            &plan.id,
            "plan_staled",
            Some(plan.status),
            Some(PlanStatus::Stale),
            plan.active_revision_id.as_deref(),
            None,
            "host",
            Some(serde_json::json!({ "reason": reason })),
        )
        .await?;
        Ok(true)
    }

    pub(super) async fn open_plan_source_boundary_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> PlanResult<Option<i64>> {
        sqlx::query_scalar(
            "SELECT source_through_row_id FROM session_plans WHERE session_id = ? AND status IN ('drafting', 'awaiting_review') ORDER BY generation DESC LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(PlanError::from)
        .map(Option::flatten)
    }

    pub(super) async fn open_plan_source_binding_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> PlanResult<Option<(Option<i64>, String)>> {
        sqlx::query_as(
            "SELECT source_through_row_id, source_hash FROM session_plans WHERE session_id = ? AND status IN ('drafting', 'awaiting_review') ORDER BY generation DESC LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(&mut **tx)
        .await
        .map_err(PlanError::from)
    }

    pub(super) async fn has_open_plan_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> PlanResult<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM session_plans WHERE session_id = ? AND status IN ('drafting', 'awaiting_review'))",
        )
        .bind(session_id)
        .fetch_one(&mut **tx)
        .await
        .map_err(PlanError::from)
    }

    pub(super) async fn ensure_compaction_allowed_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> PlanResult<()> {
        let status = sqlx::query_scalar::<_, String>(
            "SELECT status FROM session_plans WHERE session_id = ? AND status IN ('drafting', 'awaiting_review') ORDER BY generation DESC LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(&mut **tx)
        .await?;
        if status.as_deref() == Some("awaiting_review") {
            return Err(PlanError::InvalidTransition(
                "conversation compaction is blocked while a plan awaits review".to_string(),
            ));
        }
        Ok(())
    }

    pub(super) async fn native_plan_history_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
        selection: PlanHistorySelection,
    ) -> PlanResult<NativePlanHistoryV1> {
        // Count and byte admission happens before any plan-history `fetch_all`
        // allocation. Copy and export both enter through this method, so an
        // oversized same-store history fails before it can be materialized.
        let metrics = native_plan_history_metrics_in_tx(tx, session_id, selection).await?;
        validate_native_plan_history_metrics(metrics)?;
        let plan_rows = match selection {
            PlanHistorySelection::All => {
                sqlx::query("SELECT * FROM session_plans WHERE session_id = ? ORDER BY generation")
                    .bind(session_id)
                    .fetch_all(&mut **tx)
                    .await?
            }
            PlanHistorySelection::Latest => sqlx::query(
                "SELECT * FROM session_plans WHERE session_id = ? ORDER BY generation DESC LIMIT 1",
            )
            .bind(session_id)
            .fetch_all(&mut **tx)
            .await?,
        };

        let mut plans = Vec::with_capacity(plan_rows.len());
        for row in plan_rows {
            let plan = plan_from_row(&row)?;
            let revisions = sqlx::query(
                "SELECT * FROM session_plan_revisions WHERE plan_id = ? ORDER BY revision",
            )
            .bind(&plan.id)
            .fetch_all(&mut **tx)
            .await?
            .iter()
            .map(revision_from_row)
            .collect::<PlanResult<Vec<_>>>()?;
            let feedback = sqlx::query(
                "SELECT * FROM session_plan_feedback WHERE plan_id = ? ORDER BY created_at, id",
            )
            .bind(&plan.id)
            .fetch_all(&mut **tx)
            .await?
            .iter()
            .map(feedback_from_row)
            .collect::<PlanResult<Vec<_>>>()?;
            let events =
                sqlx::query("SELECT * FROM session_plan_events WHERE plan_id = ? ORDER BY id")
                    .bind(&plan.id)
                    .fetch_all(&mut **tx)
                    .await?
                    .iter()
                    .map(event_from_row)
                    .collect::<PlanResult<Vec<_>>>()?;
            plans.push(NativePlanRecordV1 {
                plan,
                revisions,
                feedback,
                events,
            });
        }
        Ok(NativePlanHistoryV1 {
            schema_version: NATIVE_PLAN_HISTORY_SCHEMA_VERSION,
            plans,
        })
    }

    pub(super) async fn clone_plan_history_as_stale_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        source_session_id: &str,
        target_session_id: &str,
        selection: PlanHistorySelection,
        reason: &str,
    ) -> PlanResult<usize> {
        let history = Self::native_plan_history_in_tx(tx, source_session_id, selection).await?;
        insert_stale_plan_history_in_tx(
            tx,
            target_session_id,
            &history,
            Some(source_session_id),
            reason,
        )
        .await
    }

    pub(super) async fn import_plan_history_as_stale_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        target_session_id: &str,
        history: &NativePlanHistoryV1,
    ) -> PlanResult<usize> {
        insert_stale_plan_history_in_tx(
            tx,
            target_session_id,
            history,
            None,
            "imported plan history is untrusted",
        )
        .await
    }

    pub(super) async fn delete_plan_history_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> PlanResult<()> {
        for statement in [
            "DELETE FROM session_plan_feedback WHERE plan_id IN (SELECT id FROM session_plans WHERE session_id = ?)",
            "DELETE FROM session_plan_events WHERE plan_id IN (SELECT id FROM session_plans WHERE session_id = ?)",
            "DELETE FROM session_plan_revisions WHERE plan_id IN (SELECT id FROM session_plans WHERE session_id = ?)",
            "DELETE FROM session_plans WHERE session_id = ?",
        ] {
            sqlx::query(statement)
                .bind(session_id)
                .execute(&mut **tx)
                .await?;
        }
        Ok(())
    }
}

async fn insert_stale_plan_history_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    target_session_id: &str,
    history: &NativePlanHistoryV1,
    trusted_source_session_id: Option<&str>,
    reason: &str,
) -> PlanResult<usize> {
    validate_native_plan_history(history)?;
    let mut inserted = 0;
    for record in &history.plans {
        let plan_id = format!("plan_{}", uuid::Uuid::now_v7());
        let mut revision_ids = HashMap::new();
        for revision in &record.revisions {
            revision_ids.insert(
                revision.id.clone(),
                format!("planrev_{}", uuid::Uuid::now_v7()),
            );
        }
        let derived_session_id = trusted_source_session_id.unwrap_or(&record.plan.session_id);
        sqlx::query(
            r#"
            INSERT INTO session_plans (
                id, session_id, generation, status, active_revision_id,
                source_through_row_id, source_hash, scope_hash,
                capability_policy_version, planner_provider, planner_model,
                stale_reason, derived_from_session_id, derived_from_plan_id,
                created_at, updated_at
            ) VALUES (?, ?, ?, 'stale', NULL, NULL, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&plan_id)
        .bind(target_session_id)
        .bind(i64::try_from(record.plan.generation).map_err(|_| {
            PlanError::InvalidInput("plan generation exceeds SQLite range".to_string())
        })?)
        .bind(&record.plan.source_hash)
        .bind(&record.plan.scope_hash)
        .bind(i64::from(record.plan.capability_policy_version))
        .bind(&record.plan.planner_provider)
        .bind(&record.plan.planner_model)
        .bind(reason)
        .bind(derived_session_id)
        .bind(&record.plan.id)
        .bind(record.plan.created_at)
        .bind(record.plan.updated_at)
        .execute(&mut **tx)
        .await?;

        for revision in &record.revisions {
            sqlx::query(
                r#"
                INSERT INTO session_plan_revisions (
                    id, plan_id, revision, parent_revision_id, content_markdown,
                    content_sha256, planner_provider, planner_model,
                    source_through_row_id, source_hash, scope_hash, created_at
                ) VALUES (?, ?, ?, NULL, ?, ?, ?, ?, NULL, ?, ?, ?)
                "#,
            )
            .bind(&revision_ids[&revision.id])
            .bind(&plan_id)
            .bind(i64::try_from(revision.revision).map_err(|_| {
                PlanError::InvalidInput("plan revision exceeds SQLite range".to_string())
            })?)
            .bind(&revision.content_markdown)
            .bind(&revision.content_sha256)
            .bind(&revision.planner_provider)
            .bind(&revision.planner_model)
            .bind(&revision.source_hash)
            .bind(&revision.scope_hash)
            .bind(revision.created_at)
            .execute(&mut **tx)
            .await?;
        }
        for revision in &record.revisions {
            if let Some(parent) = revision.parent_revision_id.as_deref() {
                sqlx::query(
                    "UPDATE session_plan_revisions SET parent_revision_id = ? WHERE id = ?",
                )
                .bind(&revision_ids[parent])
                .bind(&revision_ids[&revision.id])
                .execute(&mut **tx)
                .await?;
            }
        }

        for feedback in &record.feedback {
            let consumed_by = feedback
                .consumed_by_revision_id
                .as_ref()
                .map(|id| revision_ids.get(id).expect("validated revision reference"));
            sqlx::query(
                r#"
                INSERT INTO session_plan_feedback (
                    id, plan_id, revision_id, body, start_line, end_line,
                    selected_text_sha256, selected_text_preview,
                    consumed_by_revision_id, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(format!("planfb_{}", uuid::Uuid::now_v7()))
            .bind(&plan_id)
            .bind(&revision_ids[&feedback.revision_id])
            .bind(&feedback.body)
            .bind(feedback.start_line.map(i64::from))
            .bind(feedback.end_line.map(i64::from))
            .bind(&feedback.selected_text_sha256)
            .bind(&feedback.selected_text_preview)
            .bind(consumed_by)
            .bind(feedback.created_at)
            .execute(&mut **tx)
            .await?;
        }

        for event in &record.events {
            let revision_id = event
                .revision_id
                .as_ref()
                .and_then(|id| revision_ids.get(id));
            let detail_json = serialize_event_detail(event.detail.as_ref())?;
            sqlx::query(
                r#"
                INSERT INTO session_plan_events (
                    plan_id, event_type, from_status, to_status, revision_id,
                    revision_sha256, actor, detail_json, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(&plan_id)
            .bind(&event.event_type)
            .bind(event.from_status.map(|value| value.to_string()))
            .bind(event.to_status.map(|value| value.to_string()))
            .bind(revision_id)
            .bind(&event.revision_sha256)
            .bind(&event.actor)
            .bind(detail_json)
            .bind(event.created_at)
            .execute(&mut **tx)
            .await?;
        }

        let active_revision_id = record
            .plan
            .active_revision_id
            .as_ref()
            .map(|id| revision_ids.get(id).expect("validated active revision"));
        sqlx::query("UPDATE session_plans SET active_revision_id = ? WHERE id = ?")
            .bind(active_revision_id)
            .bind(&plan_id)
            .execute(&mut **tx)
            .await?;
        // Transfer creates a new local plan directly in `stale` state. The
        // stale reason and derived-from columns record provenance; appending a
        // synthetic transition would make an exactly-at-limit v1 event
        // collection exceed its documented cardinality after import/copy.
        inserted += 1;
    }
    Ok(inserted)
}

async fn native_plan_history_metrics_in_tx(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
    selection: PlanHistorySelection,
) -> PlanResult<NativePlanHistoryMetrics> {
    // The selection fragment is fixed by the enum, never caller-controlled.
    let selected_plans = match selection {
        PlanHistorySelection::All => "SELECT * FROM session_plans WHERE session_id = ?",
        PlanHistorySelection::Latest => {
            "SELECT * FROM session_plans WHERE session_id = ? ORDER BY generation DESC LIMIT 1"
        }
    };
    let query = format!(
        r#"
        WITH selected_plans AS ({selected_plans})
        SELECT
            (SELECT COUNT(*) FROM selected_plans) AS plan_count,
            (SELECT COUNT(*) FROM session_plan_revisions r
                JOIN selected_plans p ON p.id = r.plan_id) AS revision_count,
            (SELECT COUNT(*) FROM session_plan_feedback f
                JOIN selected_plans p ON p.id = f.plan_id) AS feedback_count,
            (SELECT COUNT(*) FROM session_plan_events e
                JOIN selected_plans p ON p.id = e.plan_id) AS event_count,
            COALESCE((SELECT SUM(
                length(CAST(source_hash AS BLOB)) +
                length(CAST(scope_hash AS BLOB)) +
                length(CAST(COALESCE(planner_provider, '') AS BLOB)) +
                length(CAST(COALESCE(planner_model, '') AS BLOB))
            ) FROM selected_plans), 0) +
            COALESCE((SELECT SUM(
                length(CAST(r.content_markdown AS BLOB)) +
                length(CAST(r.content_sha256 AS BLOB)) +
                length(CAST(COALESCE(r.planner_provider, '') AS BLOB)) +
                length(CAST(COALESCE(r.planner_model, '') AS BLOB)) +
                length(CAST(r.source_hash AS BLOB)) +
                length(CAST(r.scope_hash AS BLOB))
            ) FROM session_plan_revisions r
                JOIN selected_plans p ON p.id = r.plan_id), 0) +
            COALESCE((SELECT SUM(
                length(CAST(f.body AS BLOB)) +
                length(CAST(COALESCE(f.selected_text_sha256, '') AS BLOB)) +
                length(CAST(COALESCE(f.selected_text_preview, '') AS BLOB))
            ) FROM session_plan_feedback f
                JOIN selected_plans p ON p.id = f.plan_id), 0) +
            COALESCE((SELECT SUM(
                length(CAST(e.event_type AS BLOB)) +
                length(CAST(COALESCE(e.from_status, '') AS BLOB)) +
                length(CAST(COALESCE(e.to_status, '') AS BLOB)) +
                length(CAST(COALESCE(e.revision_sha256, '') AS BLOB)) +
                length(CAST(e.actor AS BLOB)) +
                length(CAST(COALESCE(e.detail_json, '') AS BLOB))
            ) FROM session_plan_events e
                JOIN selected_plans p ON p.id = e.plan_id), 0) AS aggregate_bytes,
            COALESCE((SELECT MAX(length(CAST(COALESCE(e.detail_json, '') AS BLOB)))
                FROM session_plan_events e
                JOIN selected_plans p ON p.id = e.plan_id), 0) AS max_event_detail_bytes,
            COALESCE((SELECT MAX(
                length(CAST(e.event_type AS BLOB)) +
                length(CAST(COALESCE(e.from_status, '') AS BLOB)) +
                length(CAST(COALESCE(e.to_status, '') AS BLOB)) +
                length(CAST(COALESCE(e.revision_sha256, '') AS BLOB)) +
                length(CAST(e.actor AS BLOB))
            ) FROM session_plan_events e
                JOIN selected_plans p ON p.id = e.plan_id), 0) AS max_event_metadata_bytes
        "#,
    );
    let row = sqlx::query(&query)
        .bind(session_id)
        .fetch_one(&mut **tx)
        .await?;
    Ok(NativePlanHistoryMetrics {
        plans: metric_usize(&row, "plan_count")?,
        revisions: metric_usize(&row, "revision_count")?,
        feedback: metric_usize(&row, "feedback_count")?,
        events: metric_usize(&row, "event_count")?,
        aggregate_bytes: metric_usize(&row, "aggregate_bytes")?,
        max_event_metadata_bytes: metric_usize(&row, "max_event_metadata_bytes")?,
        max_event_detail_bytes: metric_usize(&row, "max_event_detail_bytes")?,
    })
}

fn metric_usize(row: &SqliteRow, column: &str) -> PlanResult<usize> {
    let value = row.try_get::<i64, _>(column)?;
    usize::try_from(value).map_err(|_| {
        PlanError::CorruptState(format!(
            "native plan history metric {column} is outside the supported range"
        ))
    })
}

fn native_plan_history_metrics(
    history: &NativePlanHistoryV1,
) -> PlanResult<NativePlanHistoryMetrics> {
    let mut metrics = NativePlanHistoryMetrics {
        plans: history.plans.len(),
        ..NativePlanHistoryMetrics::default()
    };
    for record in &history.plans {
        metrics.revisions = metrics
            .revisions
            .checked_add(record.revisions.len())
            .ok_or_else(|| {
                PlanError::LimitExceeded("plan revision count overflowed".to_string())
            })?;
        metrics.feedback = metrics
            .feedback
            .checked_add(record.feedback.len())
            .ok_or_else(|| {
                PlanError::LimitExceeded("plan feedback count overflowed".to_string())
            })?;
        metrics.events = metrics
            .events
            .checked_add(record.events.len())
            .ok_or_else(|| PlanError::LimitExceeded("plan event count overflowed".to_string()))?;

        for value in [
            record.plan.source_hash.as_str(),
            record.plan.scope_hash.as_str(),
            record.plan.planner_provider.as_deref().unwrap_or_default(),
            record.plan.planner_model.as_deref().unwrap_or_default(),
        ] {
            add_history_bytes(&mut metrics, value.len())?;
        }
        for revision in &record.revisions {
            for value in [
                revision.content_markdown.as_str(),
                revision.content_sha256.as_str(),
                revision.planner_provider.as_deref().unwrap_or_default(),
                revision.planner_model.as_deref().unwrap_or_default(),
                revision.source_hash.as_str(),
                revision.scope_hash.as_str(),
            ] {
                add_history_bytes(&mut metrics, value.len())?;
            }
        }
        for feedback in &record.feedback {
            for value in [
                feedback.body.as_str(),
                feedback.selected_text_sha256.as_deref().unwrap_or_default(),
                feedback
                    .selected_text_preview
                    .as_deref()
                    .unwrap_or_default(),
            ] {
                add_history_bytes(&mut metrics, value.len())?;
            }
        }
        for event in &record.events {
            let metadata_bytes = event_metadata_bytes(
                &event.event_type,
                event.from_status,
                event.to_status,
                event.revision_sha256.as_deref(),
                &event.actor,
            )?;
            metrics.max_event_metadata_bytes = metrics.max_event_metadata_bytes.max(metadata_bytes);
            add_history_bytes(&mut metrics, metadata_bytes)?;
            let detail_bytes = event
                .detail
                .as_ref()
                .map(serialized_json_len)
                .transpose()?
                .unwrap_or_default();
            metrics.max_event_detail_bytes = metrics.max_event_detail_bytes.max(detail_bytes);
            add_history_bytes(&mut metrics, detail_bytes)?;
        }
    }
    Ok(metrics)
}

fn event_metadata_bytes(
    event_type: &str,
    from_status: Option<PlanStatus>,
    to_status: Option<PlanStatus>,
    revision_sha256: Option<&str>,
    actor: &str,
) -> PlanResult<usize> {
    [
        event_type.len(),
        from_status
            .map(|status| status.to_string().len())
            .unwrap_or_default(),
        to_status
            .map(|status| status.to_string().len())
            .unwrap_or_default(),
        revision_sha256.unwrap_or_default().len(),
        actor.len(),
    ]
    .into_iter()
    .try_fold(0_usize, |total, bytes| {
        total.checked_add(bytes).ok_or_else(|| {
            PlanError::LimitExceeded("plan event metadata byte count overflowed".to_string())
        })
    })
}

fn add_history_bytes(metrics: &mut NativePlanHistoryMetrics, bytes: usize) -> PlanResult<()> {
    metrics.aggregate_bytes = metrics.aggregate_bytes.checked_add(bytes).ok_or_else(|| {
        PlanError::LimitExceeded("native plan history aggregate byte count overflowed".to_string())
    })?;
    Ok(())
}

fn validate_native_plan_history_metrics(metrics: NativePlanHistoryMetrics) -> PlanResult<()> {
    for (label, actual, maximum) in [
        ("plans", metrics.plans, PLAN_HISTORY_MAX_PLANS),
        ("revisions", metrics.revisions, PLAN_HISTORY_MAX_REVISIONS),
        ("feedback", metrics.feedback, PLAN_HISTORY_MAX_FEEDBACK),
        ("events", metrics.events, PLAN_HISTORY_MAX_EVENTS),
    ] {
        if actual > maximum {
            return Err(PlanError::LimitExceeded(format!(
                "native plan history contains {actual} {label}; maximum is {maximum}"
            )));
        }
    }
    if metrics.max_event_detail_bytes > PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES {
        return Err(PlanError::LimitExceeded(format!(
            "native plan history event detail is {} bytes; maximum is {PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES}",
            metrics.max_event_detail_bytes
        )));
    }
    if metrics.max_event_metadata_bytes > PLAN_HISTORY_MAX_EVENT_METADATA_BYTES {
        return Err(PlanError::LimitExceeded(format!(
            "native plan history event metadata is {} bytes; maximum is {PLAN_HISTORY_MAX_EVENT_METADATA_BYTES}",
            metrics.max_event_metadata_bytes
        )));
    }
    if metrics.aggregate_bytes > PLAN_HISTORY_MAX_AGGREGATE_BYTES {
        return Err(PlanError::LimitExceeded(format!(
            "native plan history is {} aggregate bytes; maximum is {PLAN_HISTORY_MAX_AGGREGATE_BYTES}",
            metrics.aggregate_bytes
        )));
    }
    Ok(())
}

#[derive(Default)]
struct CountingWriter {
    bytes: usize,
}

impl std::io::Write for CountingWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.bytes = self
            .bytes
            .checked_add(buffer.len())
            .ok_or_else(|| std::io::Error::other("serialized JSON byte count overflowed"))?;
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn serialized_json_len(value: &serde_json::Value) -> PlanResult<usize> {
    let mut writer = CountingWriter::default();
    serde_json::to_writer(&mut writer, value)
        .map_err(|error| PlanError::InvalidInput(format!("invalid plan event detail: {error}")))?;
    Ok(writer.bytes)
}

fn serialize_event_detail(detail: Option<&serde_json::Value>) -> PlanResult<Option<String>> {
    let Some(detail) = detail else {
        return Ok(None);
    };
    let bytes = serialized_json_len(detail)?;
    if bytes > PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES {
        return Err(PlanError::LimitExceeded(format!(
            "plan event detail is {bytes} bytes; maximum is {PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES}"
        )));
    }
    serde_json::to_string(detail)
        .map(Some)
        .map_err(|error| PlanError::InvalidInput(format!("invalid plan event detail: {error}")))
}

fn validate_native_plan_history(history: &NativePlanHistoryV1) -> PlanResult<()> {
    if history.schema_version != NATIVE_PLAN_HISTORY_SCHEMA_VERSION {
        return Err(PlanError::InvalidInput(format!(
            "unsupported native plan history schema version {}",
            history.schema_version
        )));
    }
    validate_native_plan_history_metrics(native_plan_history_metrics(history)?)?;
    let mut plan_ids = HashSet::new();
    let mut generations = HashSet::new();
    for record in &history.plans {
        if record.plan.id.trim().is_empty()
            || record.plan.session_id.trim().is_empty()
            || record.plan.generation == 0
            || record.plan.capability_policy_version == 0
            || record.plan.source_hash.trim().is_empty()
            || record.plan.scope_hash.trim().is_empty()
            || !plan_ids.insert(record.plan.id.clone())
            || !generations.insert(record.plan.generation)
        {
            return Err(PlanError::InvalidInput(
                "plan history contains an empty or duplicate plan identity".to_string(),
            ));
        }
        if matches!(
            record.plan.status,
            PlanStatus::AwaitingReview | PlanStatus::Approved
        ) && record.plan.active_revision_id.is_none()
        {
            return Err(PlanError::InvalidInput(
                "plan history status requires an active revision".to_string(),
            ));
        }
        let mut revision_ids = HashSet::new();
        let mut revision_numbers = HashSet::new();
        for revision in &record.revisions {
            if revision.plan_id != record.plan.id
                || revision.id.trim().is_empty()
                || revision.revision == 0
                || revision.source_hash.trim().is_empty()
                || revision.scope_hash.trim().is_empty()
                || !revision_ids.insert(revision.id.clone())
                || !revision_numbers.insert(revision.revision)
                || revision.content_markdown.len() > PLAN_CONTENT_MAX_BYTES
                || plan_content_sha256(&revision.content_markdown) != revision.content_sha256
            {
                return Err(PlanError::InvalidInput(
                    "plan history contains an invalid revision".to_string(),
                ));
            }
        }
        if record
            .plan
            .active_revision_id
            .as_ref()
            .is_some_and(|id| !revision_ids.contains(id))
        {
            return Err(PlanError::InvalidInput(
                "plan history active revision is missing".to_string(),
            ));
        }
        for revision in &record.revisions {
            if revision
                .parent_revision_id
                .as_ref()
                .is_some_and(|id| !revision_ids.contains(id))
            {
                return Err(PlanError::InvalidInput(
                    "plan history revision parent is missing".to_string(),
                ));
            }
        }
        let mut feedback_ids = HashSet::new();
        for feedback in &record.feedback {
            if feedback.plan_id != record.plan.id
                || feedback.id.trim().is_empty()
                || !feedback_ids.insert(feedback.id.clone())
                || !revision_ids.contains(&feedback.revision_id)
                || feedback.body.trim().is_empty()
                || feedback.body.len() > PLAN_FEEDBACK_MAX_BYTES
                || feedback
                    .selected_text_preview
                    .as_ref()
                    .is_some_and(|text| text.chars().count() > PLAN_SELECTED_TEXT_PREVIEW_MAX_CHARS)
                || feedback
                    .consumed_by_revision_id
                    .as_ref()
                    .is_some_and(|id| !revision_ids.contains(id))
            {
                return Err(PlanError::InvalidInput(
                    "plan history contains invalid feedback".to_string(),
                ));
            }
            match (feedback.start_line, feedback.end_line) {
                (None, None) | (Some(1..), Some(1..)) => {}
                _ => {
                    return Err(PlanError::InvalidInput(
                        "plan history contains an invalid feedback range".to_string(),
                    ));
                }
            }
            if matches!((feedback.start_line, feedback.end_line), (Some(start), Some(end)) if start > end)
            {
                return Err(PlanError::InvalidInput(
                    "plan history contains an invalid feedback range".to_string(),
                ));
            }
        }
        for event in &record.events {
            if event.plan_id != record.plan.id
                || event.event_type.trim().is_empty()
                || event.actor.trim().is_empty()
                || event
                    .revision_id
                    .as_ref()
                    .is_some_and(|id| !revision_ids.contains(id))
            {
                return Err(PlanError::InvalidInput(
                    "plan history contains an invalid event".to_string(),
                ));
            }
        }
    }
    Ok(())
}

async fn ensure_session_not_archived(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
) -> PlanResult<()> {
    let archived =
        sqlx::query_scalar::<_, bool>("SELECT archived_at IS NOT NULL FROM sessions WHERE id = ?")
            .bind(session_id)
            .fetch_optional(&mut **tx)
            .await?;
    match archived {
        None => Err(PlanError::SessionNotFound(session_id.to_string())),
        Some(true) => Err(PlanError::InvalidTransition(
            "archived sessions have read-only plan state".to_string(),
        )),
        Some(false) => Ok(()),
    }
}

async fn ensure_session_idle(
    storage: &SessionStorage,
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
) -> PlanResult<()> {
    if storage
        .live_turn_owner(&mut *tx, session_id)
        .await?
        .is_some()
    {
        return Err(PlanError::Busy(format!(
            "session {session_id} has an active turn"
        )));
    }
    Ok(())
}

async fn open_plan_row(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
) -> PlanResult<Option<SqliteRow>> {
    sqlx::query(
        "SELECT * FROM session_plans WHERE session_id = ? AND status IN ('drafting', 'awaiting_review') ORDER BY generation DESC LIMIT 1",
    )
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(PlanError::from)
}

async fn require_open_plan(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
) -> PlanResult<SessionPlan> {
    let row = open_plan_row(tx, session_id)
        .await?
        .ok_or_else(|| PlanError::NoOpenPlan(session_id.to_string()))?;
    plan_from_row(&row)
}

async fn latest_plan(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
) -> PlanResult<SessionPlan> {
    latest_plan_optional(tx, session_id)
        .await?
        .ok_or_else(|| PlanError::NoOpenPlan(session_id.to_string()))
}

async fn latest_plan_optional(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
) -> PlanResult<Option<SessionPlan>> {
    sqlx::query("SELECT * FROM session_plans WHERE session_id = ? ORDER BY generation DESC LIMIT 1")
        .bind(session_id)
        .fetch_optional(&mut **tx)
        .await?
        .as_ref()
        .map(plan_from_row)
        .transpose()
}

async fn plan_by_id(tx: &mut Transaction<'_, Sqlite>, plan_id: &str) -> PlanResult<SessionPlan> {
    let row = sqlx::query("SELECT * FROM session_plans WHERE id = ?")
        .bind(plan_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| PlanError::PlanNotFound(plan_id.to_string()))?;
    plan_from_row(&row)
}

fn plan_from_row(row: &SqliteRow) -> PlanResult<SessionPlan> {
    let generation = row.try_get::<i64, _>("generation")?;
    let capability_policy_version = row.try_get::<i64, _>("capability_policy_version")?;
    Ok(SessionPlan {
        id: row.try_get("id")?,
        session_id: row.try_get("session_id")?,
        generation: u64::try_from(generation)
            .map_err(|_| PlanError::CorruptState("negative plan generation".to_string()))?,
        status: PlanStatus::from_str(&row.try_get::<String, _>("status")?)?,
        active_revision_id: row.try_get("active_revision_id")?,
        source_through_row_id: row.try_get("source_through_row_id")?,
        source_hash: row.try_get("source_hash")?,
        scope_hash: row.try_get("scope_hash")?,
        capability_policy_version: u32::try_from(capability_policy_version).map_err(|_| {
            PlanError::CorruptState("invalid planning capability policy version".to_string())
        })?,
        planner_provider: row.try_get("planner_provider")?,
        planner_model: row.try_get("planner_model")?,
        stale_reason: row.try_get("stale_reason")?,
        derived_from_session_id: row.try_get("derived_from_session_id")?,
        derived_from_plan_id: row.try_get("derived_from_plan_id")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn revision_from_row(row: &SqliteRow) -> PlanResult<SessionPlanRevision> {
    let revision = row.try_get::<i64, _>("revision")?;
    Ok(SessionPlanRevision {
        id: row.try_get("id")?,
        plan_id: row.try_get("plan_id")?,
        revision: u64::try_from(revision)
            .map_err(|_| PlanError::CorruptState("negative plan revision".to_string()))?,
        parent_revision_id: row.try_get("parent_revision_id")?,
        content_markdown: row.try_get("content_markdown")?,
        content_sha256: row.try_get("content_sha256")?,
        planner_provider: row.try_get("planner_provider")?,
        planner_model: row.try_get("planner_model")?,
        source_through_row_id: row.try_get("source_through_row_id")?,
        source_hash: row.try_get("source_hash")?,
        scope_hash: row.try_get("scope_hash")?,
        created_at: row.try_get("created_at")?,
    })
}

fn feedback_from_row(row: &SqliteRow) -> PlanResult<SessionPlanFeedback> {
    Ok(SessionPlanFeedback {
        id: row.try_get("id")?,
        plan_id: row.try_get("plan_id")?,
        revision_id: row.try_get("revision_id")?,
        body: row.try_get("body")?,
        start_line: optional_positive_u32(row.try_get("start_line")?, "feedback start line")?,
        end_line: optional_positive_u32(row.try_get("end_line")?, "feedback end line")?,
        selected_text_sha256: row.try_get("selected_text_sha256")?,
        selected_text_preview: row.try_get("selected_text_preview")?,
        consumed_by_revision_id: row.try_get("consumed_by_revision_id")?,
        created_at: row.try_get("created_at")?,
    })
}

fn event_from_row(row: &SqliteRow) -> PlanResult<SessionPlanEvent> {
    let from_status = row
        .try_get::<Option<String>, _>("from_status")?
        .map(|value| PlanStatus::from_str(&value))
        .transpose()?;
    let to_status = row
        .try_get::<Option<String>, _>("to_status")?
        .map(|value| PlanStatus::from_str(&value))
        .transpose()?;
    let detail = row
        .try_get::<Option<String>, _>("detail_json")?
        .map(|value| {
            serde_json::from_str(&value).map_err(|error| {
                PlanError::CorruptState(format!("invalid plan event detail JSON: {error}"))
            })
        })
        .transpose()?;
    Ok(SessionPlanEvent {
        id: row.try_get("id")?,
        plan_id: row.try_get("plan_id")?,
        event_type: row.try_get("event_type")?,
        from_status,
        to_status,
        revision_id: row.try_get("revision_id")?,
        revision_sha256: row.try_get("revision_sha256")?,
        actor: row.try_get("actor")?,
        detail,
        created_at: row.try_get("created_at")?,
    })
}

fn optional_positive_u32(value: Option<i64>, label: &str) -> PlanResult<Option<u32>> {
    value
        .map(|value| {
            u32::try_from(value)
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| PlanError::CorruptState(format!("invalid {label}")))
        })
        .transpose()
}

async fn snapshot_for_plan<'a, E>(executor: E, plan: SessionPlan) -> PlanResult<PlanSnapshot>
where
    E: sqlx::Executor<'a, Database = Sqlite> + Copy,
{
    let active_revision = match plan.active_revision_id.as_deref() {
        Some(revision_id) => {
            let row =
                sqlx::query("SELECT * FROM session_plan_revisions WHERE id = ? AND plan_id = ?")
                    .bind(revision_id)
                    .bind(&plan.id)
                    .fetch_optional(executor)
                    .await?
                    .ok_or_else(|| {
                        PlanError::CorruptState(format!(
                            "plan {} points to missing or foreign active revision {revision_id}",
                            plan.id
                        ))
                    })?;
            Some(revision_from_row(&row)?)
        }
        None => None,
    };
    if matches!(
        plan.status,
        PlanStatus::AwaitingReview | PlanStatus::Approved
    ) && active_revision.is_none()
    {
        return Err(PlanError::CorruptState(format!(
            "plan {} is {} without an active revision",
            plan.id, plan.status
        )));
    }

    let mut feedback = sqlx::query(
        "SELECT * FROM session_plan_feedback WHERE plan_id = ? ORDER BY created_at DESC, id DESC LIMIT ?",
    )
    .bind(&plan.id)
    .bind(PLAN_SNAPSHOT_FEEDBACK_LIMIT as i64)
    .fetch_all(executor)
    .await?
    .iter()
    .map(feedback_from_row)
    .collect::<PlanResult<Vec<_>>>()?;
    feedback.reverse();
    let mut recent_events =
        sqlx::query("SELECT * FROM session_plan_events WHERE plan_id = ? ORDER BY id DESC LIMIT ?")
            .bind(&plan.id)
            .bind(PLAN_SNAPSHOT_EVENT_LIMIT as i64)
            .fetch_all(executor)
            .await?
            .iter()
            .map(event_from_row)
            .collect::<PlanResult<Vec<_>>>()?;
    recent_events.reverse();
    validate_plan_references(executor, &plan.id).await?;
    Ok(PlanSnapshot {
        plan,
        active_revision,
        feedback,
        recent_events,
    })
}

async fn snapshot_for_plan_tx(
    tx: &mut Transaction<'_, Sqlite>,
    plan: SessionPlan,
) -> PlanResult<PlanSnapshot> {
    let active_revision = match plan.active_revision_id.as_deref() {
        Some(revision_id) => {
            let row =
                sqlx::query("SELECT * FROM session_plan_revisions WHERE id = ? AND plan_id = ?")
                    .bind(revision_id)
                    .bind(&plan.id)
                    .fetch_optional(&mut **tx)
                    .await?
                    .ok_or_else(|| {
                        PlanError::CorruptState(format!(
                            "plan {} points to missing or foreign active revision {revision_id}",
                            plan.id
                        ))
                    })?;
            Some(revision_from_row(&row)?)
        }
        None => None,
    };
    if matches!(
        plan.status,
        PlanStatus::AwaitingReview | PlanStatus::Approved
    ) && active_revision.is_none()
    {
        return Err(PlanError::CorruptState(format!(
            "plan {} is {} without an active revision",
            plan.id, plan.status
        )));
    }
    let feedback_rows = sqlx::query(
        "SELECT * FROM session_plan_feedback WHERE plan_id = ? ORDER BY created_at DESC, id DESC LIMIT ?",
    )
    .bind(&plan.id)
    .bind(PLAN_SNAPSHOT_FEEDBACK_LIMIT as i64)
    .fetch_all(&mut **tx)
    .await?;
    let mut feedback = feedback_rows
        .iter()
        .map(feedback_from_row)
        .collect::<PlanResult<Vec<_>>>()?;
    feedback.reverse();
    let event_rows =
        sqlx::query("SELECT * FROM session_plan_events WHERE plan_id = ? ORDER BY id DESC LIMIT ?")
            .bind(&plan.id)
            .bind(PLAN_SNAPSHOT_EVENT_LIMIT as i64)
            .fetch_all(&mut **tx)
            .await?;
    let mut recent_events = event_rows
        .iter()
        .map(event_from_row)
        .collect::<PlanResult<Vec<_>>>()?;
    recent_events.reverse();
    validate_plan_references(&mut **tx, &plan.id).await?;
    Ok(PlanSnapshot {
        plan,
        active_revision,
        feedback,
        recent_events,
    })
}

async fn validate_plan_references<'a, E>(executor: E, plan_id: &str) -> PlanResult<()>
where
    E: sqlx::Executor<'a, Database = Sqlite>,
{
    let invalid = sqlx::query_scalar::<_, bool>(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM session_plan_revisions AS revision
            LEFT JOIN session_plan_revisions AS parent
              ON parent.id = revision.parent_revision_id
            WHERE revision.plan_id = ?
              AND revision.parent_revision_id IS NOT NULL
              AND (parent.id IS NULL OR parent.plan_id != ?)
            UNION ALL
            SELECT 1
            FROM session_plan_feedback AS feedback
            LEFT JOIN session_plan_revisions AS feedback_revision
              ON feedback_revision.id = feedback.revision_id
            LEFT JOIN session_plan_revisions AS consumed_revision
              ON consumed_revision.id = feedback.consumed_by_revision_id
            WHERE feedback.plan_id = ?
              AND (
                feedback_revision.id IS NULL OR feedback_revision.plan_id != ?
                OR (
                  feedback.consumed_by_revision_id IS NOT NULL
                  AND (consumed_revision.id IS NULL OR consumed_revision.plan_id != ?)
                )
              )
            UNION ALL
            SELECT 1
            FROM session_plan_events AS event
            LEFT JOIN session_plan_revisions AS event_revision
              ON event_revision.id = event.revision_id
            WHERE event.plan_id = ?
              AND event.revision_id IS NOT NULL
              AND (
                event_revision.id IS NULL OR event_revision.plan_id != ?
                OR (
                  event.revision_sha256 IS NOT NULL
                  AND event.revision_sha256 != event_revision.content_sha256
                )
              )
        )
        "#,
    )
    .bind(plan_id)
    .bind(plan_id)
    .bind(plan_id)
    .bind(plan_id)
    .bind(plan_id)
    .bind(plan_id)
    .bind(plan_id)
    .fetch_one(executor)
    .await?;
    if invalid {
        return Err(PlanError::CorruptState(format!(
            "plan {plan_id} has a foreign or mismatched revision reference"
        )));
    }
    Ok(())
}

async fn require_expected_revision(
    tx: &mut Transaction<'_, Sqlite>,
    plan: &SessionPlan,
    expectation: &PlanExpectation,
) -> PlanResult<SessionPlanRevision> {
    if plan.generation != expectation.generation {
        return Err(PlanError::Conflict(format!(
            "expected generation {}, current generation is {}",
            expectation.generation, plan.generation
        )));
    }
    let expected_id = expectation
        .revision_id
        .as_deref()
        .ok_or_else(|| PlanError::InvalidInput("expected revision id is required".to_string()))?;
    let expected_hash = expectation
        .revision_sha256
        .as_deref()
        .ok_or_else(|| PlanError::InvalidInput("expected revision hash is required".to_string()))?;
    if plan.active_revision_id.as_deref() != Some(expected_id) {
        return Err(PlanError::Conflict(
            "the active plan revision changed".to_string(),
        ));
    }
    let row = sqlx::query("SELECT * FROM session_plan_revisions WHERE id = ? AND plan_id = ?")
        .bind(expected_id)
        .bind(&plan.id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| {
            PlanError::CorruptState(format!(
                "active revision {expected_id} is missing from plan {}",
                plan.id
            ))
        })?;
    let revision = revision_from_row(&row)?;
    if revision.content_sha256 != expected_hash {
        return Err(PlanError::Conflict(
            "the active plan revision hash changed".to_string(),
        ));
    }
    Ok(revision)
}

async fn ensure_revision_evidence_current(
    storage: &SessionStorage,
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
    revision: &SessionPlanRevision,
    expectation: &PlanExpectation,
) -> PlanResult<()> {
    if expectation.source_hash != revision.source_hash
        || expectation.scope_hash != revision.scope_hash
    {
        return Err(PlanError::Conflict(
            "the supplied source or scope expectation is stale".to_string(),
        ));
    }
    let (source_through_row_id, source_hash) = storage.current_source_hash(tx, session_id).await?;
    let scope_hash = SessionStorage::current_scope_hash_in_tx(tx, session_id).await?;
    if source_through_row_id != revision.source_through_row_id
        || source_hash != revision.source_hash
        || scope_hash != revision.scope_hash
    {
        return Err(PlanError::Conflict(
            "the plan source transcript or workspace scope changed".to_string(),
        ));
    }
    Ok(())
}

fn validate_feedback_range(
    revision: &SessionPlanRevision,
    feedback: &NewPlanFeedback,
) -> PlanResult<()> {
    let (Some(start), Some(end)) = (feedback.start_line, feedback.end_line) else {
        return Ok(());
    };
    let lines = revision.content_markdown.lines().collect::<Vec<_>>();
    if end as usize > lines.len() {
        return Err(PlanError::InvalidInput(format!(
            "feedback line {end} exceeds the active revision's {} lines",
            lines.len()
        )));
    }
    if let Some(selected) = feedback.selected_text.as_deref() {
        let current = lines[(start - 1) as usize..end as usize].join("\n");
        if selected != current {
            return Err(PlanError::Conflict(
                "the selected plan text no longer matches the requested line range".to_string(),
            ));
        }
    }
    Ok(())
}

async fn source_revision_token(
    tx: &mut Transaction<'_, Sqlite>,
    session_id: &str,
) -> PlanResult<String> {
    sqlx::query_scalar(
        "SELECT revision_token FROM session_plan_source_versions WHERE session_id = ?",
    )
    .bind(session_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| PlanError::SessionNotFound(session_id.to_string()))
}

fn source_hash_from_rows(
    session_id: &str,
    rows: &[SqliteRow],
) -> PlanResult<(Option<i64>, String)> {
    let mut cache = SOURCE_ROW_CACHE
        .lock()
        .map_err(|_| PlanError::CorruptState("plan source hash cache is poisoned".to_string()))?;
    let mut cached_rows = Vec::with_capacity(rows.len());
    let mut lifecycle_request_ids = HashSet::new();
    for row in rows {
        let cached = cached_source_row(session_id, row, &mut cache)?;
        lifecycle_request_ids.extend(cached.lifecycle_request_ids.iter().cloned());
        cached_rows.push(cached);
    }

    let mut evidence_rows = Vec::with_capacity(rows.len());
    for (row, cached) in rows.iter().zip(cached_rows) {
        let row_id: i64 = row.try_get("id")?;
        if !cached.agent_visible {
            continue;
        }

        let content_hash = if cached.requires_lifecycle_filter {
            let content_json: String = row.try_get("content_json")?;
            let content: Vec<MessageContent> =
                serde_json::from_str(&content_json).map_err(|error| {
                    PlanError::CorruptState(format!(
                        "invalid message content in plan source: {error}"
                    ))
                })?;
            let content = content
                .into_iter()
                .filter(|item| match item {
                    MessageContent::ToolRequest(request) => !request
                        .tool_call
                        .as_ref()
                        .ok()
                        .is_some_and(|tool| is_plan_lifecycle_tool(&tool.name)),
                    MessageContent::ToolResponse(response) => {
                        !lifecycle_request_ids.contains(&response.id)
                    }
                    _ => true,
                })
                .collect::<Vec<_>>();
            if content.is_empty() {
                None
            } else {
                let canonical_content = serde_json::to_vec(&content).map_err(|error| {
                    PlanError::CorruptState(format!(
                        "cannot canonicalize plan source message: {error}"
                    ))
                })?;
                Some(sha256_hex(&canonical_content))
            }
        } else {
            cached.ordinary_content_hash
        };
        let Some(content_hash) = content_hash else {
            continue;
        };
        let message_id: Option<String> = row.try_get("message_id")?;
        let role: String = row.try_get("role")?;
        evidence_rows.push((
            row_id,
            message_id.unwrap_or_default(),
            role,
            cached.visibility_flags,
            content_hash,
        ));
    }

    let mut hasher = Sha256::new();
    hasher.update(PLAN_SOURCE_HASH_VERSION.as_bytes());
    hasher.update([0]);
    let evidence_count = u64::try_from(evidence_rows.len()).map_err(|_| {
        PlanError::LimitExceeded("plan source evidence row count exceeds u64".to_string())
    })?;
    update_source_hash_part(&mut hasher, &evidence_count.to_le_bytes());
    let mut through = None;
    for (row_id, message_id, role, visibility_flags, content_hash) in evidence_rows {
        update_source_hash_part(&mut hasher, &row_id.to_le_bytes());
        update_source_hash_part(&mut hasher, message_id.as_bytes());
        update_source_hash_part(&mut hasher, role.as_bytes());
        update_source_hash_part(&mut hasher, &visibility_flags);
        update_source_hash_part(&mut hasher, content_hash.as_bytes());
        through = Some(row_id);
    }
    let digest = hasher.finalize();
    let mut digest_hex = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut digest_hex, "{byte:02x}").expect("writing to a String cannot fail");
    }
    Ok((through, format!("{PLAN_SOURCE_HASH_VERSION}:{digest_hex}")))
}

fn cached_source_row(
    session_id: &str,
    row: &SqliteRow,
    cache: &mut HashMap<(String, i64), CachedSourceRow>,
) -> PlanResult<CachedSourceRow> {
    let row_id: i64 = row.try_get("id")?;
    let content_json: String = row.try_get("content_json")?;
    let metadata_json: Option<String> = row.try_get("metadata_json")?;
    let fingerprint = source_row_fingerprint(&content_json, metadata_json.as_deref());
    let key = (session_id.to_string(), row_id);
    if let Some(cached) = cache.get(&key) {
        if cached.fingerprint == fingerprint {
            return Ok(cached.clone());
        }
    }

    let metadata: MessageMetadata = metadata_json
        .as_deref()
        .map(serde_json::from_str)
        .transpose()
        .map_err(|error| {
            PlanError::CorruptState(format!("invalid message metadata in plan source: {error}"))
        })?
        .unwrap_or_default();
    let content: Vec<MessageContent> = serde_json::from_str(&content_json).map_err(|error| {
        PlanError::CorruptState(format!("invalid message content in plan source: {error}"))
    })?;
    let lifecycle_request_ids = content
        .iter()
        .filter_map(|item| match item {
            MessageContent::ToolRequest(request)
                if request
                    .tool_call
                    .as_ref()
                    .ok()
                    .is_some_and(|tool| is_plan_lifecycle_tool(&tool.name)) =>
            {
                Some(request.id.clone())
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    let requires_lifecycle_filter = !lifecycle_request_ids.is_empty()
        || content
            .iter()
            .any(|item| matches!(item, MessageContent::ToolResponse(_)));
    let ordinary_content_hash =
        if metadata.agent_visible && !requires_lifecycle_filter && !content.is_empty() {
            let canonical_content = serde_json::to_vec(&content).map_err(|error| {
                PlanError::CorruptState(format!("cannot canonicalize plan source message: {error}"))
            })?;
            Some(sha256_hex(&canonical_content))
        } else {
            None
        };
    let cached = CachedSourceRow {
        fingerprint,
        agent_visible: metadata.agent_visible,
        visibility_flags: [
            u8::from(metadata.user_visible),
            u8::from(metadata.agent_visible),
            u8::from(metadata.imported_untrusted),
        ],
        lifecycle_request_ids,
        requires_lifecycle_filter,
        ordinary_content_hash,
    };
    if cache.len() >= SOURCE_ROW_CACHE_LIMIT && !cache.contains_key(&key) {
        cache.clear();
    }
    cache.insert(key, cached.clone());
    Ok(cached)
}

fn source_row_fingerprint(content_json: &str, metadata_json: Option<&str>) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    update_blake3_part(&mut hasher, content_json.as_bytes());
    update_blake3_part(&mut hasher, metadata_json.unwrap_or_default().as_bytes());
    *hasher.finalize().as_bytes()
}

fn update_blake3_part(hasher: &mut blake3::Hasher, part: &[u8]) {
    hasher.update(&(part.len() as u64).to_le_bytes());
    hasher.update(part);
}

fn update_source_hash_part(hasher: &mut Sha256, part: &[u8]) {
    hasher.update((part.len() as u64).to_le_bytes());
    hasher.update(part);
}

fn is_plan_lifecycle_tool(name: &str) -> bool {
    PLAN_UPDATE_TOOL_NAMES.contains(&name) || PLAN_REQUEST_REVIEW_TOOL_NAMES.contains(&name)
}

impl SessionStorage {
    async fn current_source_hash(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> PlanResult<(Option<i64>, String)> {
        let revision_token = source_revision_token(tx, session_id).await?;
        let cache_key = (session_id.to_string(), -1);
        if let Some((_, through, hash)) = self
            .plan_source_hash_cache
            .lock()
            .map_err(|_| PlanError::CorruptState("plan source hash cache is poisoned".to_string()))?
            .get(&cache_key)
            .filter(|(cached_token, _, _)| cached_token == &revision_token)
            .cloned()
        {
            return Ok((through, hash));
        }
        let rows = sqlx::query(
            "SELECT id, message_id, role, content_json, metadata_json FROM messages WHERE session_id = ? ORDER BY id",
        )
        .bind(session_id)
        .fetch_all(&mut **tx)
        .await?;
        let result = source_hash_from_rows(session_id, &rows)?;
        let mut cache = self.plan_source_hash_cache.lock().map_err(|_| {
            PlanError::CorruptState("plan source hash cache is poisoned".to_string())
        })?;
        if cache.len() >= SOURCE_HASH_CACHE_LIMIT && !cache.contains_key(&cache_key) {
            cache.clear();
        }
        cache.insert(cache_key, (revision_token, result.0, result.1.clone()));
        Ok(result)
    }

    async fn source_hash_through(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
        through_row_id: Option<i64>,
    ) -> PlanResult<(Option<i64>, String)> {
        let revision_token = source_revision_token(tx, session_id).await?;
        let cache_key = (session_id.to_string(), through_row_id.unwrap_or(0));
        if let Some((_, through, hash)) = self
            .plan_source_hash_cache
            .lock()
            .map_err(|_| PlanError::CorruptState("plan source hash cache is poisoned".to_string()))?
            .get(&cache_key)
            .filter(|(cached_token, _, _)| cached_token == &revision_token)
            .cloned()
        {
            return Ok((through, hash));
        }
        let rows = match through_row_id {
            Some(through_row_id) => sqlx::query(
                "SELECT id, message_id, role, content_json, metadata_json FROM messages WHERE session_id = ? AND id <= ? ORDER BY id",
            )
            .bind(session_id)
            .bind(through_row_id)
            .fetch_all(&mut **tx)
            .await?,
            None => Vec::new(),
        };
        let result = source_hash_from_rows(session_id, &rows)?;
        let mut cache = self.plan_source_hash_cache.lock().map_err(|_| {
            PlanError::CorruptState("plan source hash cache is poisoned".to_string())
        })?;
        if cache.len() >= SOURCE_HASH_CACHE_LIMIT && !cache.contains_key(&cache_key) {
            cache.clear();
        }
        cache.insert(cache_key, (revision_token, result.0, result.1.clone()));
        Ok(result)
    }

    pub(super) async fn current_source_hash_in_tx(
        &self,
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> PlanResult<(Option<i64>, String)> {
        self.current_source_hash(tx, session_id).await
    }

    pub(super) async fn current_scope_hash_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
    ) -> PlanResult<String> {
        let row = sqlx::query(
            r#"
        SELECT working_dir, additional_working_dirs_json,
               restrict_tools_to_working_dirs, workspace_id, project_id
        FROM sessions WHERE id = ?
        "#,
        )
        .bind(session_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| PlanError::SessionNotFound(session_id.to_string()))?;
        let working_dir: String = row.try_get("working_dir")?;
        let additional_json: String = row.try_get("additional_working_dirs_json")?;
        let additional: Vec<PathBuf> = serde_json::from_str(&additional_json).map_err(|error| {
            PlanError::CorruptState(format!("invalid additional working directories: {error}"))
        })?;
        let primary = canonical_scope_path(Path::new(&working_dir))?;
        let additional = additional
            .iter()
            .map(|path| canonical_scope_path(path))
            .collect::<PlanResult<Vec<_>>>()?;
        let workspace_id: Option<String> = row.try_get("workspace_id")?;
        let project_id: Option<String> = row.try_get("project_id")?;
        scope_hash_from_canonical_parts(
            &primary,
            &additional,
            workspace_id.as_deref().unwrap_or_default(),
            project_id.as_deref().unwrap_or_default(),
            row.try_get::<bool, _>("restrict_tools_to_working_dirs")?,
            PLAN_CAPABILITY_POLICY_VERSION,
        )
    }
}

fn scope_hash_from_canonical_parts(
    primary: &str,
    additional: &[String],
    workspace_id: &str,
    project_id: &str,
    restrict_tools_to_working_dirs: bool,
    capability_policy_version: u32,
) -> PlanResult<String> {
    let additional_count = u64::try_from(additional.len()).map_err(|_| {
        PlanError::LimitExceeded("additional workspace root count exceeds u64".to_string())
    })?;
    let mut parts = Vec::with_capacity(additional.len() + 6);
    parts.push(primary.as_bytes());
    let count_bytes = additional_count.to_le_bytes();
    parts.push(count_bytes.as_slice());
    for path in additional {
        parts.push(path.as_bytes());
    }
    parts.push(workspace_id.as_bytes());
    parts.push(project_id.as_bytes());
    let restrict = [u8::from(restrict_tools_to_working_dirs)];
    parts.push(restrict.as_slice());
    let capability = capability_policy_version.to_le_bytes();
    parts.push(capability.as_slice());
    Ok(versioned_sha256(PLAN_SCOPE_HASH_VERSION, &parts))
}

fn canonical_scope_path(path: &Path) -> PlanResult<String> {
    std::fs::canonicalize(path)
        .map(|path| path.to_string_lossy().to_string())
        .map_err(|error| {
            PlanError::InvalidInput(format!(
                "cannot enter planning because workspace root {} cannot be resolved: {error}",
                path.display()
            ))
        })
}

#[allow(clippy::too_many_arguments)]
async fn append_event(
    tx: &mut Transaction<'_, Sqlite>,
    plan_id: &str,
    event_type: &str,
    from_status: Option<PlanStatus>,
    to_status: Option<PlanStatus>,
    revision_id: Option<&str>,
    revision_sha256: Option<&str>,
    actor: &str,
    detail: Option<serde_json::Value>,
) -> PlanResult<()> {
    let metadata_bytes =
        event_metadata_bytes(event_type, from_status, to_status, revision_sha256, actor)?;
    if metadata_bytes > PLAN_HISTORY_MAX_EVENT_METADATA_BYTES {
        return Err(PlanError::LimitExceeded(format!(
            "plan event metadata is {metadata_bytes} bytes; maximum is {PLAN_HISTORY_MAX_EVENT_METADATA_BYTES}"
        )));
    }
    let detail_json = serialize_event_detail(detail.as_ref())?;
    sqlx::query(
        r#"
        INSERT INTO session_plan_events (
            plan_id, event_type, from_status, to_status, revision_id,
            revision_sha256, actor, detail_json
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?)
        "#,
    )
    .bind(plan_id)
    .bind(event_type)
    .bind(from_status.map(|value| value.to_string()))
    .bind(to_status.map(|value| value.to_string()))
    .bind(revision_id)
    .bind(revision_sha256)
    .bind(actor)
    .bind(detail_json)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

#[cfg(test)]
mod continuity_tests {
    use super::*;
    use crate::config::GoslingMode;
    use crate::conversation::{message::Message, Conversation};
    use crate::session::{
        NewPlanFeedback, NewPlanRevision, SessionImportOutcome, SessionManager, SessionType,
    };
    use gosling_providers::conversation::token_usage::Usage;
    use rmcp::model::CallToolRequestParams;
    use std::time::{Duration, Instant};
    use tempfile::TempDir;

    fn p95(mut samples: Vec<Duration>) -> Duration {
        samples.sort_unstable();
        samples[(samples.len() * 95).div_ceil(100).saturating_sub(1)]
    }

    fn fixture_canonical_bytes(version: &str, parts: &[&[u8]]) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(version.as_bytes());
        bytes.push(0);
        for part in parts {
            bytes.extend_from_slice(&(part.len() as u64).to_le_bytes());
            bytes.extend_from_slice(part);
        }
        bytes
    }

    fn fixture_hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn empty_history_record(index: usize) -> NativePlanRecordV1 {
        let now = chrono::Utc::now();
        NativePlanRecordV1 {
            plan: SessionPlan {
                id: format!("fixture-plan-{index}"),
                session_id: "fixture-session".to_string(),
                generation: (index + 1) as u64,
                status: PlanStatus::Stale,
                active_revision_id: None,
                source_through_row_id: None,
                source_hash: "source_hash_v1:fixture".to_string(),
                scope_hash: "scope_hash_v1:fixture".to_string(),
                capability_policy_version: 1,
                planner_provider: None,
                planner_model: None,
                stale_reason: Some("fixture".to_string()),
                derived_from_session_id: None,
                derived_from_plan_id: None,
                created_at: now,
                updated_at: now,
            },
            revisions: Vec::new(),
            feedback: Vec::new(),
            events: Vec::new(),
        }
    }

    fn fixture_revision(plan_id: &str, revision: usize) -> SessionPlanRevision {
        SessionPlanRevision {
            id: format!("fixture-revision-{revision}"),
            plan_id: plan_id.to_string(),
            revision: (revision + 1) as u64,
            parent_revision_id: None,
            content_markdown: "#".to_string(),
            content_sha256: plan_content_sha256("#"),
            planner_provider: None,
            planner_model: None,
            source_through_row_id: None,
            source_hash: "source_hash_v1:fixture".to_string(),
            scope_hash: "scope_hash_v1:fixture".to_string(),
            created_at: chrono::Utc::now(),
        }
    }

    fn history_with_record(record: NativePlanRecordV1) -> NativePlanHistoryV1 {
        NativePlanHistoryV1 {
            schema_version: NATIVE_PLAN_HISTORY_SCHEMA_VERSION,
            plans: vec![record],
        }
    }

    async fn session_with_revision() -> (TempDir, SessionManager, PlanSnapshot) {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().join("data"));
        let session = manager
            .create_session(
                temp_dir.path().to_path_buf(),
                "Continuity".to_string(),
                SessionType::User,
                GoslingMode::Approve,
            )
            .await
            .unwrap();
        manager
            .add_message(
                &session.id,
                &Message::user()
                    .with_id("source-message")
                    .with_text("source evidence"),
            )
            .await
            .unwrap();
        let started = manager
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
        let revision = manager
            .plans()
            .update_revision(
                &session.id,
                NewPlanRevision {
                    content_markdown: "# Plan\n\n1. Preserve authority.\n".to_string(),
                    expected_generation: started.plan.generation,
                    expected_parent_revision_id: None,
                    planner_provider: Some("test".to_string()),
                    planner_model: Some("test-model".to_string()),
                },
            )
            .await
            .unwrap();
        (temp_dir, manager, revision)
    }

    #[tokio::test]
    async fn source_hash_v1_has_byte_exact_mixed_filtered_unicode_and_order_fixtures() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().join("data"));
        let session = manager
            .create_session(
                temp_dir.path().to_path_buf(),
                "Canonical source fixture".to_string(),
                SessionType::User,
                GoslingMode::Chat,
            )
            .await
            .unwrap();
        manager
            .add_message(
                &session.id,
                &Message::assistant()
                    .with_id("mixed-α")
                    .with_text("Résumé 🪿")
                    .with_tool_request(
                        "plan-call",
                        Ok(CallToolRequestParams::new("planning__plan_update")),
                    ),
            )
            .await
            .unwrap();
        manager
            .add_message(
                &session.id,
                &Message::assistant()
                    .with_id("filtered-empty")
                    .with_tool_request(
                        "review-call",
                        Ok(CallToolRequestParams::new("planning__plan_request_review")),
                    ),
            )
            .await
            .unwrap();
        manager
            .add_message(
                &session.id,
                &Message::user().with_id("終-id").with_text("終"),
            )
            .await
            .unwrap();

        let snapshot = manager
            .storage()
            .start_or_resume_plan(&session.id, None, None, None, None)
            .await
            .unwrap();
        assert_eq!(snapshot.plan.source_through_row_id, Some(3));

        let count = 2_u64.to_le_bytes();
        let first_row = 1_i64.to_le_bytes();
        let last_row = 3_i64.to_le_bytes();
        let visible_flags = [1_u8, 1, 0];
        let parts = [
            count.as_slice(),
            first_row.as_slice(),
            "mixed-α".as_bytes(),
            b"assistant".as_slice(),
            visible_flags.as_slice(),
            b"acf2979b3ae1db2b3df2397ad516c630c654d70773d06d49a04eac8d89a48567".as_slice(),
            last_row.as_slice(),
            "終-id".as_bytes(),
            b"user".as_slice(),
            visible_flags.as_slice(),
            b"3901543b543e86114dab7c25d002e423aafd3df798a842019a962916f53a9fe7".as_slice(),
        ];
        let canonical = fixture_canonical_bytes(PLAN_SOURCE_HASH_VERSION, &parts);
        assert_eq!(
            fixture_hex(&canonical),
            "736f757263655f686173685f763100080000000000000002000000000000000800000000000000010000000000000008000000000000006d697865642dceb10900000000000000617373697374616e740300000000000000010100400000000000000061636632393739623361653164623262336466323339376164353136633633306336353464373037373364303664343961303465616338643839613438353637080000000000000003000000000000000600000000000000e7b5822d69640400000000000000757365720300000000000000010100400000000000000033393031353433623534336538363131346461623763323564303032653432336161666433646637393861383432303139613936323931366635336139666537"
        );
        assert_eq!(
            snapshot.plan.source_hash,
            format!("{PLAN_SOURCE_HASH_VERSION}:{}", sha256_hex(&canonical))
        );
        assert_eq!(
            snapshot.plan.source_hash,
            "source_hash_v1:bf434b96bd863b1360c7be18d5aa23fad8f91643956a03d5037c6fb1262a8a21"
        );

        let version_changed = fixture_canonical_bytes("source_hash_v2", &parts);
        assert_eq!(
            sha256_hex(&version_changed),
            "bfbba9d6ee8cbeac29a2135f15d057927137efee8379d3e7c6f5fe150d6a2d20"
        );
        let swapped = fixture_canonical_bytes(
            PLAN_SOURCE_HASH_VERSION,
            &[
                count.as_slice(),
                first_row.as_slice(),
                b"assistant".as_slice(),
                "mixed-α".as_bytes(),
                [1_u8, 1, 0].as_slice(),
                b"acf2979b3ae1db2b3df2397ad516c630c654d70773d06d49a04eac8d89a48567".as_slice(),
                last_row.as_slice(),
                "終-id".as_bytes(),
                b"user".as_slice(),
                [1_u8, 1, 0].as_slice(),
                b"3901543b543e86114dab7c25d002e423aafd3df798a842019a962916f53a9fe7".as_slice(),
            ],
        );
        assert_eq!(
            sha256_hex(&swapped),
            "84b43e19134b5c0da4fe1c87eb6079427aa7f5d50b1d457ad5e0243c05de32c7"
        );
        assert_ne!(sha256_hex(&swapped), sha256_hex(&canonical));
    }

    #[test]
    fn scope_hash_v1_has_byte_exact_field_and_additional_root_count_fixtures() {
        let additional = vec!["/extra/一".to_string(), "/extra/two".to_string()];
        let count = 2_u64.to_le_bytes();
        let capability = 1_u32.to_le_bytes();
        let canonical = fixture_canonical_bytes(
            PLAN_SCOPE_HASH_VERSION,
            &[
                "/scope/α".as_bytes(),
                count.as_slice(),
                additional[0].as_bytes(),
                additional[1].as_bytes(),
                "workspace-雪".as_bytes(),
                "project-π".as_bytes(),
                [1_u8].as_slice(),
                capability.as_slice(),
            ],
        );
        assert_eq!(
            fixture_hex(&canonical),
            "73636f70655f686173685f76310009000000000000002f73636f70652fceb1080000000000000002000000000000000a000000000000002f65787472612fe4b8800a000000000000002f65787472612f74776f0d00000000000000776f726b73706163652de99baa0a0000000000000070726f6a6563742dcf80010000000000000001040000000000000001000000"
        );
        assert_eq!(
            scope_hash_from_canonical_parts(
                "/scope/α",
                &additional,
                "workspace-雪",
                "project-π",
                true,
                1,
            )
            .unwrap(),
            "scope_hash_v1:29a4bfec8ea8d4c61dcdaa0b99fb1c975394d5ce7a82928c039fea7124a454b8"
        );
        assert_eq!(
            scope_hash_from_canonical_parts("/scope/α", &[], "workspace-雪", "project-π", true, 1,)
                .unwrap(),
            "scope_hash_v1:1cb26e8f8a64e2cecadcb3e82cd5587330a57bfaf8916199bbede47132f50ae4"
        );
        assert_eq!(
            scope_hash_from_canonical_parts(
                "/scope/α",
                &["/extra/一".to_string()],
                "workspace-雪",
                "project-π",
                true,
                1,
            )
            .unwrap(),
            "scope_hash_v1:5bd81852d825ae8f9eed432e1ed78d274d149131c1a02a506b3e9486577a9ce9"
        );
    }

    #[tokio::test]
    async fn schema_35_plan_hashes_survive_migration_to_current_schema() {
        let (temp_dir, manager, before) = session_with_revision().await;
        assert!(before.plan.source_hash.starts_with("source_hash_v1:"));
        assert!(before.plan.scope_hash.starts_with("scope_hash_v1:"));
        let pool = manager.storage().pool().await.unwrap();
        sqlx::query("DROP TABLE session_compaction_revisions")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("DROP TABLE session_compaction_state")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("DELETE FROM schema_version")
            .execute(pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO schema_version(version) VALUES (35)")
            .execute(pool)
            .await
            .unwrap();
        drop(manager);

        let migrated = SessionManager::new(temp_dir.path().join("data"));
        let pool = migrated.storage().pool().await.unwrap();
        let version: i64 = sqlx::query_scalar("SELECT MAX(version) FROM schema_version")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(
            version,
            i64::from(crate::session::session_manager::CURRENT_SCHEMA_VERSION)
        );
        let after = migrated
            .plans()
            .snapshot(&before.plan.session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.plan.source_hash, before.plan.source_hash);
        assert_eq!(after.plan.scope_hash, before.plan.scope_hash);
        assert_eq!(after.active_revision, before.active_revision);
    }

    #[test]
    fn native_plan_history_metrics_accept_maxima_and_reject_max_plus_one() {
        let maximum = NativePlanHistoryMetrics {
            plans: PLAN_HISTORY_MAX_PLANS,
            revisions: PLAN_HISTORY_MAX_REVISIONS,
            feedback: PLAN_HISTORY_MAX_FEEDBACK,
            events: PLAN_HISTORY_MAX_EVENTS,
            aggregate_bytes: PLAN_HISTORY_MAX_AGGREGATE_BYTES,
            max_event_metadata_bytes: PLAN_HISTORY_MAX_EVENT_METADATA_BYTES,
            max_event_detail_bytes: PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES,
        };
        validate_native_plan_history_metrics(maximum).unwrap();
        for over_limit in [
            NativePlanHistoryMetrics {
                plans: PLAN_HISTORY_MAX_PLANS + 1,
                ..maximum
            },
            NativePlanHistoryMetrics {
                revisions: PLAN_HISTORY_MAX_REVISIONS + 1,
                ..maximum
            },
            NativePlanHistoryMetrics {
                feedback: PLAN_HISTORY_MAX_FEEDBACK + 1,
                ..maximum
            },
            NativePlanHistoryMetrics {
                events: PLAN_HISTORY_MAX_EVENTS + 1,
                ..maximum
            },
            NativePlanHistoryMetrics {
                aggregate_bytes: PLAN_HISTORY_MAX_AGGREGATE_BYTES + 1,
                ..maximum
            },
            NativePlanHistoryMetrics {
                max_event_detail_bytes: PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES + 1,
                ..maximum
            },
            NativePlanHistoryMetrics {
                max_event_metadata_bytes: PLAN_HISTORY_MAX_EVENT_METADATA_BYTES + 1,
                ..maximum
            },
        ] {
            assert!(matches!(
                validate_native_plan_history_metrics(over_limit),
                Err(PlanError::LimitExceeded(_))
            ));
        }
    }

    #[test]
    fn native_plan_history_validation_exercises_each_exact_collection_and_byte_limit() {
        let mut plans = NativePlanHistoryV1 {
            schema_version: NATIVE_PLAN_HISTORY_SCHEMA_VERSION,
            plans: (0..PLAN_HISTORY_MAX_PLANS)
                .map(empty_history_record)
                .collect(),
        };
        validate_native_plan_history(&plans).unwrap();
        plans
            .plans
            .push(empty_history_record(PLAN_HISTORY_MAX_PLANS));
        assert!(matches!(
            validate_native_plan_history(&plans),
            Err(PlanError::LimitExceeded(_))
        ));

        let mut revisions_record = empty_history_record(0);
        revisions_record.revisions = (0..PLAN_HISTORY_MAX_REVISIONS)
            .map(|index| fixture_revision(&revisions_record.plan.id, index))
            .collect();
        let mut revisions = history_with_record(revisions_record);
        validate_native_plan_history(&revisions).unwrap();
        let revisions_plan_id = revisions.plans[0].plan.id.clone();
        revisions.plans[0].revisions.push(fixture_revision(
            &revisions_plan_id,
            PLAN_HISTORY_MAX_REVISIONS,
        ));
        assert!(matches!(
            validate_native_plan_history(&revisions),
            Err(PlanError::LimitExceeded(_))
        ));

        let mut feedback_record = empty_history_record(0);
        let revision = fixture_revision(&feedback_record.plan.id, 0);
        feedback_record.revisions.push(revision.clone());
        feedback_record.feedback = (0..PLAN_HISTORY_MAX_FEEDBACK)
            .map(|index| SessionPlanFeedback {
                id: format!("fixture-feedback-{index}"),
                plan_id: feedback_record.plan.id.clone(),
                revision_id: revision.id.clone(),
                body: "x".to_string(),
                start_line: None,
                end_line: None,
                selected_text_sha256: None,
                selected_text_preview: None,
                consumed_by_revision_id: None,
                created_at: chrono::Utc::now(),
            })
            .collect();
        let mut feedback = history_with_record(feedback_record);
        validate_native_plan_history(&feedback).unwrap();
        let feedback_plan_id = feedback.plans[0].plan.id.clone();
        feedback.plans[0].feedback.push(SessionPlanFeedback {
            id: "fixture-feedback-over-limit".to_string(),
            plan_id: feedback_plan_id,
            revision_id: revision.id,
            body: "x".to_string(),
            start_line: None,
            end_line: None,
            selected_text_sha256: None,
            selected_text_preview: None,
            consumed_by_revision_id: None,
            created_at: chrono::Utc::now(),
        });
        assert!(matches!(
            validate_native_plan_history(&feedback),
            Err(PlanError::LimitExceeded(_))
        ));

        let mut events_record = empty_history_record(0);
        events_record.events = (0..PLAN_HISTORY_MAX_EVENTS)
            .map(|index| SessionPlanEvent {
                id: index as i64,
                plan_id: events_record.plan.id.clone(),
                event_type: "e".to_string(),
                from_status: None,
                to_status: None,
                revision_id: None,
                revision_sha256: None,
                actor: "h".to_string(),
                detail: None,
                created_at: chrono::Utc::now(),
            })
            .collect();
        let mut events = history_with_record(events_record);
        validate_native_plan_history(&events).unwrap();
        let events_plan_id = events.plans[0].plan.id.clone();
        events.plans[0].events.push(SessionPlanEvent {
            id: PLAN_HISTORY_MAX_EVENTS as i64,
            plan_id: events_plan_id,
            event_type: "e".to_string(),
            from_status: None,
            to_status: None,
            revision_id: None,
            revision_sha256: None,
            actor: "h".to_string(),
            detail: None,
            created_at: chrono::Utc::now(),
        });
        assert!(matches!(
            validate_native_plan_history(&events),
            Err(PlanError::LimitExceeded(_))
        ));

        let mut aggregate_record = empty_history_record(0);
        aggregate_record.plan.source_hash = "s".to_string();
        aggregate_record.plan.scope_hash = "q".to_string();
        aggregate_record.plan.planner_model =
            Some("x".repeat(PLAN_HISTORY_MAX_AGGREGATE_BYTES - 2));
        let mut aggregate = history_with_record(aggregate_record);
        validate_native_plan_history(&aggregate).unwrap();
        aggregate.plans[0].plan.planner_model =
            Some("x".repeat(PLAN_HISTORY_MAX_AGGREGATE_BYTES - 1));
        assert!(matches!(
            validate_native_plan_history(&aggregate),
            Err(PlanError::LimitExceeded(_))
        ));

        let mut detail_record = empty_history_record(0);
        detail_record.events.push(SessionPlanEvent {
            id: 1,
            plan_id: detail_record.plan.id.clone(),
            event_type: "event".to_string(),
            from_status: None,
            to_status: None,
            revision_id: None,
            revision_sha256: None,
            actor: "host".to_string(),
            detail: Some(serde_json::Value::String(
                "x".repeat(PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES - 2),
            )),
            created_at: chrono::Utc::now(),
        });
        let mut detail = history_with_record(detail_record);
        validate_native_plan_history(&detail).unwrap();
        detail.plans[0].events[0].detail = Some(serde_json::Value::String(
            "x".repeat(PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES - 1),
        ));
        assert!(matches!(
            validate_native_plan_history(&detail),
            Err(PlanError::LimitExceeded(_))
        ));

        let mut metadata_record = empty_history_record(0);
        metadata_record.events.push(SessionPlanEvent {
            id: 1,
            plan_id: metadata_record.plan.id.clone(),
            event_type: "x".repeat(PLAN_HISTORY_MAX_EVENT_METADATA_BYTES - 1),
            from_status: None,
            to_status: None,
            revision_id: None,
            revision_sha256: None,
            actor: "h".to_string(),
            detail: None,
            created_at: chrono::Utc::now(),
        });
        let mut metadata = history_with_record(metadata_record);
        validate_native_plan_history(&metadata).unwrap();
        metadata.plans[0].events[0].event_type = "x".repeat(PLAN_HISTORY_MAX_EVENT_METADATA_BYTES);
        assert!(matches!(
            validate_native_plan_history(&metadata),
            Err(PlanError::LimitExceeded(_))
        ));
    }

    #[tokio::test]
    async fn every_max_plus_one_history_fails_before_the_first_insert() {
        async fn assert_no_insert(
            manager: &SessionManager,
            target_session_id: &str,
            history: &NativePlanHistoryV1,
        ) {
            let pool = manager.storage().pool().await.unwrap();
            let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
            assert!(matches!(
                insert_stale_plan_history_in_tx(
                    &mut tx,
                    target_session_id,
                    history,
                    None,
                    "limit fixture",
                )
                .await,
                Err(PlanError::LimitExceeded(_))
            ));
            let inserted: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM session_plans WHERE session_id = ?")
                    .bind(target_session_id)
                    .fetch_one(&mut *tx)
                    .await
                    .unwrap();
            assert_eq!(inserted, 0);
            tx.rollback().await.unwrap();
        }

        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().join("data"));
        let target = manager
            .create_session(
                temp_dir.path().to_path_buf(),
                "Limit target".to_string(),
                SessionType::User,
                GoslingMode::Chat,
            )
            .await
            .unwrap();

        let plans = NativePlanHistoryV1 {
            schema_version: NATIVE_PLAN_HISTORY_SCHEMA_VERSION,
            plans: (0..=PLAN_HISTORY_MAX_PLANS)
                .map(empty_history_record)
                .collect(),
        };
        assert_no_insert(&manager, &target.id, &plans).await;
        drop(plans);

        let mut revisions_record = empty_history_record(0);
        revisions_record.revisions = (0..=PLAN_HISTORY_MAX_REVISIONS)
            .map(|index| fixture_revision(&revisions_record.plan.id, index))
            .collect();
        assert_no_insert(&manager, &target.id, &history_with_record(revisions_record)).await;

        let mut feedback_record = empty_history_record(0);
        let revision = fixture_revision(&feedback_record.plan.id, 0);
        feedback_record.revisions.push(revision.clone());
        feedback_record.feedback = (0..=PLAN_HISTORY_MAX_FEEDBACK)
            .map(|index| SessionPlanFeedback {
                id: format!("fixture-feedback-{index}"),
                plan_id: feedback_record.plan.id.clone(),
                revision_id: revision.id.clone(),
                body: "x".to_string(),
                start_line: None,
                end_line: None,
                selected_text_sha256: None,
                selected_text_preview: None,
                consumed_by_revision_id: None,
                created_at: chrono::Utc::now(),
            })
            .collect();
        assert_no_insert(&manager, &target.id, &history_with_record(feedback_record)).await;

        let mut events_record = empty_history_record(0);
        events_record.events = (0..=PLAN_HISTORY_MAX_EVENTS)
            .map(|index| SessionPlanEvent {
                id: index as i64,
                plan_id: events_record.plan.id.clone(),
                event_type: "e".to_string(),
                from_status: None,
                to_status: None,
                revision_id: None,
                revision_sha256: None,
                actor: "h".to_string(),
                detail: None,
                created_at: chrono::Utc::now(),
            })
            .collect();
        assert_no_insert(&manager, &target.id, &history_with_record(events_record)).await;

        let mut aggregate_record = empty_history_record(0);
        aggregate_record.plan.source_hash = "s".to_string();
        aggregate_record.plan.scope_hash = "q".to_string();
        aggregate_record.plan.planner_model =
            Some("x".repeat(PLAN_HISTORY_MAX_AGGREGATE_BYTES - 1));
        assert_no_insert(&manager, &target.id, &history_with_record(aggregate_record)).await;

        let mut detail_record = empty_history_record(0);
        detail_record.events.push(SessionPlanEvent {
            id: 1,
            plan_id: detail_record.plan.id.clone(),
            event_type: "event".to_string(),
            from_status: None,
            to_status: None,
            revision_id: None,
            revision_sha256: None,
            actor: "host".to_string(),
            detail: Some(serde_json::Value::String(
                "x".repeat(PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES - 1),
            )),
            created_at: chrono::Utc::now(),
        });
        assert_no_insert(&manager, &target.id, &history_with_record(detail_record)).await;

        let mut metadata_record = empty_history_record(0);
        metadata_record.events.push(SessionPlanEvent {
            id: 1,
            plan_id: metadata_record.plan.id.clone(),
            event_type: "x".repeat(PLAN_HISTORY_MAX_EVENT_METADATA_BYTES),
            from_status: None,
            to_status: None,
            revision_id: None,
            revision_sha256: None,
            actor: "h".to_string(),
            detail: None,
            created_at: chrono::Utc::now(),
        });
        assert_no_insert(&manager, &target.id, &history_with_record(metadata_record)).await;
    }

    #[tokio::test]
    async fn database_and_wire_history_metrics_match_exactly() {
        let (_temp_dir, manager, revision) = session_with_revision().await;
        let review = manager
            .plans()
            .request_review(
                &revision.plan.session_id,
                &PlanExpectation::for_snapshot(&revision),
            )
            .await
            .unwrap();
        manager
            .plans()
            .add_feedback(
                &review.plan.session_id,
                &PlanExpectation::for_snapshot(&review),
                NewPlanFeedback {
                    body: "Unicode feedback 雪".to_string(),
                    start_line: None,
                    end_line: None,
                    selected_text: None,
                },
            )
            .await
            .unwrap();

        let pool = manager.storage().pool().await.unwrap();
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let database = native_plan_history_metrics_in_tx(
            &mut tx,
            &revision.plan.session_id,
            PlanHistorySelection::All,
        )
        .await
        .unwrap();
        let history = SessionStorage::native_plan_history_in_tx(
            &mut tx,
            &revision.plan.session_id,
            PlanHistorySelection::All,
        )
        .await
        .unwrap();
        let wire = native_plan_history_metrics(&history).unwrap();
        tx.rollback().await.unwrap();
        assert_eq!(database, wire);
    }

    #[test]
    fn bounded_history_deserializers_accept_max_and_reject_max_plus_one_without_capacity_growth() {
        fn assert_bound(maximum: usize, label: &'static str) {
            let at_limit = serde_json::to_string(&(0..maximum).collect::<Vec<_>>()).unwrap();
            let mut deserializer = serde_json::Deserializer::from_str(&at_limit);
            let decoded: Vec<usize> =
                deserialize_bounded_vec(&mut deserializer, maximum, label).unwrap();
            assert_eq!(decoded.len(), maximum);

            let over_limit = serde_json::to_string(&(0..=maximum).collect::<Vec<_>>()).unwrap();
            let mut deserializer = serde_json::Deserializer::from_str(&over_limit);
            let error =
                deserialize_bounded_vec::<_, usize>(&mut deserializer, maximum, label).unwrap_err();
            assert!(error.to_string().contains("more than"));
        }

        for (maximum, label) in [
            (PLAN_HISTORY_MAX_PLANS, "plans"),
            (PLAN_HISTORY_MAX_REVISIONS, "revisions"),
            (PLAN_HISTORY_MAX_FEEDBACK, "feedback"),
            (PLAN_HISTORY_MAX_EVENTS, "events"),
        ] {
            assert_bound(maximum, label);
        }
    }

    #[test]
    fn event_detail_serialization_enforces_the_exact_byte_boundary() {
        let maximum =
            serde_json::Value::String("x".repeat(PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES - 2));
        assert_eq!(
            serialized_json_len(&maximum).unwrap(),
            PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES
        );
        assert_eq!(
            serialize_event_detail(Some(&maximum))
                .unwrap()
                .unwrap()
                .len(),
            PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES
        );
        let over_limit =
            serde_json::Value::String("x".repeat(PLAN_HISTORY_MAX_EVENT_DETAIL_BYTES - 1));
        assert!(matches!(
            serialize_event_detail(Some(&over_limit)),
            Err(PlanError::LimitExceeded(_))
        ));
    }

    #[tokio::test]
    async fn same_store_copy_accepts_max_plans_and_rolls_back_max_plus_one() {
        async fn seed_plans(manager: &SessionManager, session_id: &str, count: usize) {
            let pool = manager.storage().pool().await.unwrap();
            sqlx::query(
                r#"
                WITH RECURSIVE sequence(generation) AS (
                    VALUES(1)
                    UNION ALL
                    SELECT generation + 1 FROM sequence WHERE generation < ?
                )
                INSERT INTO session_plans (
                    id, session_id, generation, status, source_hash, scope_hash,
                    capability_policy_version
                )
                SELECT 'seed-plan-' || ? || '-' || generation, ?, generation, 'stale',
                       'source_hash_v1:seed', 'scope_hash_v1:seed', 1
                FROM sequence
                "#,
            )
            .bind(i64::try_from(count).unwrap())
            .bind(session_id)
            .bind(session_id)
            .execute(pool)
            .await
            .unwrap();
        }

        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().join("data"));
        let at_limit = manager
            .create_session(
                temp_dir.path().to_path_buf(),
                "At limit".to_string(),
                SessionType::User,
                GoslingMode::Chat,
            )
            .await
            .unwrap();
        seed_plans(&manager, &at_limit.id, PLAN_HISTORY_MAX_PLANS).await;
        let copied = manager
            .copy_session(&at_limit.id, "At limit copy".to_string())
            .await
            .unwrap();
        let copied_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM session_plans WHERE session_id = ?")
                .bind(&copied.id)
                .fetch_one(manager.storage().pool().await.unwrap())
                .await
                .unwrap();
        assert_eq!(copied_count, PLAN_HISTORY_MAX_PLANS as i64);

        let over_limit = manager
            .create_session(
                temp_dir.path().to_path_buf(),
                "Over limit".to_string(),
                SessionType::User,
                GoslingMode::Chat,
            )
            .await
            .unwrap();
        seed_plans(&manager, &over_limit.id, PLAN_HISTORY_MAX_PLANS + 1).await;
        let session_count_before: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(manager.storage().pool().await.unwrap())
            .await
            .unwrap();
        assert!(manager
            .copy_session(&over_limit.id, "Rejected copy".to_string())
            .await
            .is_err());
        let session_count_after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(manager.storage().pool().await.unwrap())
            .await
            .unwrap();
        assert_eq!(session_count_after, session_count_before);
        assert!(manager.export_session(&over_limit.id).await.is_err());

        let mut over_limit_import: serde_json::Value =
            serde_json::from_str(&manager.export_session(&at_limit.id).await.unwrap()).unwrap();
        let extra_plan = over_limit_import[NATIVE_PLAN_HISTORY_KEY]["plans"][0].clone();
        over_limit_import[NATIVE_PLAN_HISTORY_KEY]["plans"]
            .as_array_mut()
            .unwrap()
            .push(extra_plan);
        let session_count_before_import: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(manager.storage().pool().await.unwrap())
            .await
            .unwrap();
        assert!(manager
            .import_session(
                &serde_json::to_string(&over_limit_import).unwrap(),
                Some(SessionType::User),
                temp_dir.path().to_path_buf(),
                crate::session::import_formats::SessionImportTransport::Json,
            )
            .await
            .is_err());
        let session_count_after_import: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(manager.storage().pool().await.unwrap())
            .await
            .unwrap();
        assert_eq!(session_count_after_import, session_count_before_import);
    }

    #[tokio::test]
    async fn snapshot_rejects_cross_plan_revision_references() {
        let (temp_dir, manager, first) = session_with_revision().await;
        let second_session = manager
            .create_session(
                temp_dir.path().to_path_buf(),
                "Other plan".to_string(),
                SessionType::User,
                GoslingMode::Approve,
            )
            .await
            .unwrap();
        let second_started = manager
            .storage()
            .start_or_resume_plan(&second_session.id, None, None, None, None)
            .await
            .unwrap();
        let second = manager
            .plans()
            .update_revision(
                &second_session.id,
                NewPlanRevision {
                    content_markdown: "# Other plan".to_string(),
                    expected_generation: second_started.plan.generation,
                    expected_parent_revision_id: None,
                    planner_provider: None,
                    planner_model: None,
                },
            )
            .await
            .unwrap();
        let first_revision = first.active_revision.as_ref().unwrap();
        let second_revision = second.active_revision.as_ref().unwrap();
        let pool = manager.storage().pool().await.unwrap();

        sqlx::query("UPDATE session_plan_revisions SET parent_revision_id = ? WHERE id = ?")
            .bind(&second_revision.id)
            .bind(&first_revision.id)
            .execute(pool)
            .await
            .unwrap();
        assert!(matches!(
            manager.plans().snapshot(&first.plan.session_id).await,
            Err(PlanError::CorruptState(_))
        ));
        sqlx::query("UPDATE session_plan_revisions SET parent_revision_id = NULL WHERE id = ?")
            .bind(&first_revision.id)
            .execute(pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO session_plan_feedback (id, plan_id, revision_id, body) VALUES (?, ?, ?, ?)",
        )
        .bind("foreign-feedback")
        .bind(&first.plan.id)
        .bind(&second_revision.id)
        .bind("cross-plan")
        .execute(pool)
        .await
        .unwrap();
        assert!(matches!(
            manager.plans().snapshot(&first.plan.session_id).await,
            Err(PlanError::CorruptState(_))
        ));
        sqlx::query("DELETE FROM session_plan_feedback WHERE id = ?")
            .bind("foreign-feedback")
            .execute(pool)
            .await
            .unwrap();

        sqlx::query(
            "INSERT INTO session_plan_events (plan_id, event_type, revision_id, revision_sha256, actor) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&first.plan.id)
        .bind("corrupt_event")
        .bind(&second_revision.id)
        .bind(&second_revision.content_sha256)
        .bind("test")
        .execute(pool)
        .await
        .unwrap();
        assert!(matches!(
            manager.plans().snapshot(&first.plan.session_id).await,
            Err(PlanError::CorruptState(_))
        ));
    }

    #[tokio::test]
    async fn truncation_stales_only_when_it_removes_source_rows() {
        let (_temp_dir, manager, revision) = session_with_revision().await;
        let session_id = &revision.plan.session_id;
        manager
            .add_message(
                session_id,
                &Message::user()
                    .with_id("later-message")
                    .with_text("later evidence"),
            )
            .await
            .unwrap();
        manager
            .truncate_conversation_from_message(session_id, "later-message")
            .await
            .unwrap();
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Drafting
        );

        manager
            .truncate_conversation_from_message(session_id, "source-message")
            .await
            .unwrap();
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Stale
        );
    }

    #[tokio::test]
    async fn history_staling_and_truncation_roll_back_together() {
        let (_temp_dir, manager, revision) = session_with_revision().await;
        let session_id = &revision.plan.session_id;
        let pool = manager.storage().pool().await.unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_plan_stale_event BEFORE INSERT ON session_plan_events WHEN NEW.event_type = 'plan_staled' BEGIN SELECT RAISE(ABORT, 'injected stale failure'); END",
        )
        .execute(pool)
        .await
        .unwrap();

        assert!(manager
            .truncate_conversation_from_message(session_id, "source-message")
            .await
            .is_err());
        assert_eq!(
            manager
                .get_session(session_id, true)
                .await
                .unwrap()
                .conversation
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Drafting
        );
    }

    #[tokio::test]
    async fn upsert_only_stales_when_it_edits_bound_source_evidence() {
        let (_temp_dir, manager, revision) = session_with_revision().await;
        let session_id = &revision.plan.session_id;
        manager
            .upsert_message(
                session_id,
                &Message::assistant()
                    .with_id("later-upsert")
                    .with_text("ordinary streaming output"),
            )
            .await
            .unwrap();
        manager
            .upsert_message(
                session_id,
                &Message::assistant()
                    .with_id("later-upsert")
                    .with_text("ordinary streaming output continued"),
            )
            .await
            .unwrap();
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Drafting
        );

        manager
            .upsert_message(
                session_id,
                &Message::user()
                    .with_id("source-message")
                    .with_text("edited source evidence"),
            )
            .await
            .unwrap();
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Stale
        );
    }

    #[tokio::test]
    async fn compaction_stales_drafting_and_blocks_review() {
        let (_temp_dir, manager, revision) = session_with_revision().await;
        let session_id = &revision.plan.session_id;
        let compacted =
            Conversation::new_unvalidated(vec![Message::assistant().with_text("summary")]);
        manager
            .replace_conversation_and_record_usage(
                session_id,
                &compacted,
                Usage::default(),
                Usage::default(),
                None,
            )
            .await
            .unwrap();
        let snapshot = manager.plans().snapshot(session_id).await.unwrap().unwrap();
        assert_eq!(snapshot.plan.status, PlanStatus::Stale);
        assert_eq!(
            snapshot.active_revision.unwrap().content_markdown,
            "# Plan\n\n1. Preserve authority.\n"
        );

        let (_review_temp_dir, review_manager, review_revision) = session_with_revision().await;
        let review_session_id = &review_revision.plan.session_id;
        review_manager
            .plans()
            .request_review(
                review_session_id,
                &PlanExpectation::for_snapshot(&review_revision),
            )
            .await
            .unwrap();
        assert!(review_manager
            .replace_conversation_and_record_usage(
                review_session_id,
                &compacted,
                Usage::default(),
                Usage::default(),
                None,
            )
            .await
            .is_err());
        let preserved = review_manager
            .get_session(review_session_id, true)
            .await
            .unwrap()
            .conversation
            .unwrap();
        assert_eq!(preserved.messages()[0].as_concat_text(), "source evidence");
        assert_eq!(
            review_manager
                .plans()
                .snapshot(review_session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::AwaitingReview
        );
    }

    #[tokio::test]
    async fn scope_changes_stale_but_provider_transitions_are_blocked() {
        let (temp_dir, manager, revision) = session_with_revision().await;
        let session_id = &revision.plan.session_id;
        manager
            .update(session_id)
            .working_dir(temp_dir.path().to_path_buf())
            .apply()
            .await
            .unwrap();
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Drafting
        );
        assert!(manager
            .update(session_id)
            .provider_name("other")
            .apply()
            .await
            .is_err());
        let handoff = crate::session::handoff::SessionHandoffBuilder::new(&manager)
            .build(
                session_id,
                "other",
                "other-model",
                128_000,
                crate::providers::base::ProviderCapabilities::gosling_managed(),
                gosling_sdk_types::session_handoff::SessionHandoffTriggerDto::ManualCheckpoint,
            )
            .await
            .unwrap();
        assert!(manager
            .prepare_handoff_snapshot(handoff, Some(0))
            .await
            .is_err());
        assert_eq!(
            manager.latest_handoff_generation(session_id).await.unwrap(),
            0
        );

        let next_root = temp_dir.path().join("next-root");
        std::fs::create_dir(&next_root).unwrap();
        manager
            .update(session_id)
            .working_dir(next_root)
            .apply()
            .await
            .unwrap();
        let session = manager.get_session(session_id, false).await.unwrap();
        assert!(session.provider_name.is_none());
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Stale
        );

        let second = manager
            .storage()
            .start_or_resume_plan(session_id, None, None, Some(1), None)
            .await
            .unwrap();
        let additional_root = temp_dir.path().join("additional-root");
        std::fs::create_dir(&additional_root).unwrap();
        manager
            .update(session_id)
            .additional_working_dirs(vec![additional_root])
            .apply()
            .await
            .unwrap();
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Stale
        );

        manager
            .storage()
            .start_or_resume_plan(session_id, None, None, Some(second.plan.generation), None)
            .await
            .unwrap();
        manager
            .update(session_id)
            .restrict_tools_to_working_dirs(true)
            .apply()
            .await
            .unwrap();
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Stale
        );

        manager
            .storage()
            .start_or_resume_plan(session_id, None, None, Some(3), None)
            .await
            .unwrap();
        manager
            .update(session_id)
            .project_id(Some("project-two".to_string()))
            .apply()
            .await
            .unwrap();
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Stale
        );
    }

    #[tokio::test]
    async fn scope_change_and_plan_staling_roll_back_together() {
        let (temp_dir, manager, revision) = session_with_revision().await;
        let session_id = &revision.plan.session_id;
        let original_root = manager
            .get_session(session_id, false)
            .await
            .unwrap()
            .working_dir;
        let next_root = temp_dir.path().join("rollback-root");
        std::fs::create_dir(&next_root).unwrap();
        let pool = manager.storage().pool().await.unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_scope_stale_event BEFORE INSERT ON session_plan_events WHEN NEW.event_type = 'plan_staled' BEGIN SELECT RAISE(ABORT, 'injected scope stale failure'); END",
        )
        .execute(pool)
        .await
        .unwrap();

        assert!(manager
            .update(session_id)
            .working_dir(next_root)
            .apply()
            .await
            .is_err());
        assert_eq!(
            manager
                .get_session(session_id, false)
                .await
                .unwrap()
                .working_dir,
            original_root
        );
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Drafting
        );
    }

    #[tokio::test]
    async fn copy_and_import_rekey_full_history_as_stale() {
        let (temp_dir, manager, revision) = session_with_revision().await;
        let review = manager
            .plans()
            .request_review(
                &revision.plan.session_id,
                &PlanExpectation::for_snapshot(&revision),
            )
            .await
            .unwrap();
        let feedback = manager
            .plans()
            .add_feedback(
                &review.plan.session_id,
                &PlanExpectation::for_snapshot(&review),
                NewPlanFeedback {
                    body: "Keep the transaction atomic".to_string(),
                    start_line: Some(3),
                    end_line: Some(3),
                    selected_text: Some("1. Preserve authority.".to_string()),
                },
            )
            .await
            .unwrap();
        let revised = manager
            .plans()
            .update_revision(
                &feedback.plan.session_id,
                NewPlanRevision {
                    content_markdown: "# Plan\n\n1. Preserve authority atomically.\n".to_string(),
                    expected_generation: feedback.plan.generation,
                    expected_parent_revision_id: feedback.plan.active_revision_id.clone(),
                    planner_provider: None,
                    planner_model: None,
                },
            )
            .await
            .unwrap();

        let copied = manager
            .copy_session(&revised.plan.session_id, "copy".to_string())
            .await
            .unwrap();
        let copied_plan = manager.plans().snapshot(&copied.id).await.unwrap().unwrap();
        assert_eq!(copied_plan.plan.status, PlanStatus::Stale);
        assert_ne!(copied_plan.plan.id, revised.plan.id);
        assert_eq!(
            copied_plan.plan.derived_from_session_id.as_deref(),
            Some(revised.plan.session_id.as_str())
        );
        assert_eq!(copied_plan.feedback.len(), 1);
        assert_ne!(
            copied_plan.active_revision.as_ref().unwrap().id,
            revised.active_revision.as_ref().unwrap().id
        );
        assert_ne!(copied_plan.feedback[0].id, revised.feedback[0].id);
        assert_ne!(
            copied_plan.feedback[0].revision_id,
            revised.feedback[0].revision_id
        );
        let copied_revision_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM session_plan_revisions WHERE plan_id = ?")
                .bind(&copied_plan.plan.id)
                .fetch_one(manager.storage().pool().await.unwrap())
                .await
                .unwrap();
        assert_eq!(copied_revision_count, 2);

        let exported = manager
            .export_session(&revised.plan.session_id)
            .await
            .unwrap();
        assert!(exported.len() <= crate::session::import_formats::MAX_SESSION_IMPORT_BYTES);
        let exported_value = serde_json::from_str::<serde_json::Value>(&exported).unwrap();
        assert!(exported_value.get(NATIVE_PLAN_HISTORY_KEY).is_some());
        assert!(exported_value.get("_gosling_plan_history").is_none());
        let session_count_before_invalid: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(manager.storage().pool().await.unwrap())
            .await
            .unwrap();
        let mut invalid_value = exported_value.clone();
        invalid_value[NATIVE_PLAN_HISTORY_KEY]["plans"][0]["revisions"][0]["contentSha256"] =
            serde_json::Value::String("invalid".to_string());
        assert!(manager
            .import_session(
                &serde_json::to_string(&invalid_value).unwrap(),
                Some(SessionType::User),
                temp_dir.path().to_path_buf(),
                crate::session::import_formats::SessionImportTransport::Json,
            )
            .await
            .is_err());
        let session_count_after_invalid: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(manager.storage().pool().await.unwrap())
            .await
            .unwrap();
        assert_eq!(session_count_after_invalid, session_count_before_invalid);
        let outcome = manager
            .import_session(
                &exported,
                Some(SessionType::User),
                temp_dir.path().to_path_buf(),
                crate::session::import_formats::SessionImportTransport::Json,
            )
            .await
            .unwrap();
        let SessionImportOutcome::Imported(imported) = outcome else {
            panic!("first import of new content must create a session");
        };
        let imported_plan = manager
            .plans()
            .snapshot(&imported.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(imported_plan.plan.status, PlanStatus::Stale);
        assert_ne!(imported_plan.plan.id, revised.plan.id);
        assert_eq!(imported_plan.feedback.len(), 1);
        assert_ne!(
            imported_plan.active_revision.as_ref().unwrap().id,
            revised.active_revision.as_ref().unwrap().id
        );
        assert_ne!(imported_plan.feedback[0].id, revised.feedback[0].id);
        assert_ne!(
            imported_plan.feedback[0].revision_id,
            revised.feedback[0].revision_id
        );

        let mut legacy_value: serde_json::Value = serde_json::from_str(&exported).unwrap();
        legacy_value
            .as_object_mut()
            .unwrap()
            .remove(NATIVE_PLAN_HISTORY_KEY);
        let legacy_outcome = manager
            .import_session(
                &serde_json::to_string(&legacy_value).unwrap(),
                Some(SessionType::User),
                temp_dir.path().to_path_buf(),
                crate::session::import_formats::SessionImportTransport::Json,
            )
            .await
            .unwrap();
        let SessionImportOutcome::Imported(legacy_import) = legacy_outcome else {
            panic!("importing a different session id must create a session");
        };
        assert!(manager
            .plans()
            .snapshot(&legacy_import.id)
            .await
            .unwrap()
            .is_none());
    }

    #[tokio::test]
    async fn plan_copy_failure_rolls_back_the_target_session() {
        let (_temp_dir, manager, revision) = session_with_revision().await;
        let source_session_id = &revision.plan.session_id;
        let pool = manager.storage().pool().await.unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_copied_plan_revision BEFORE INSERT ON session_plan_revisions WHEN (SELECT COUNT(*) FROM sessions) > 1 BEGIN SELECT RAISE(ABORT, 'injected plan-copy failure'); END",
        )
        .execute(pool)
        .await
        .unwrap();

        assert!(manager
            .copy_session(source_session_id, "failed copy".to_string())
            .await
            .is_err());
        let session_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM sessions")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(session_count, 1);
        assert_eq!(
            manager
                .plans()
                .snapshot(source_session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .id,
            revision.plan.id
        );
    }

    #[tokio::test]
    async fn copy_and_import_preserve_every_generation_as_stale_history() {
        let (temp_dir, manager, first) = session_with_revision().await;
        let session_id = &first.plan.session_id;
        manager
            .plans()
            .abandon(
                session_id,
                first.plan.generation,
                Some("superseded".to_string()),
            )
            .await
            .unwrap();
        let second = manager
            .storage()
            .start_or_resume_plan(session_id, None, None, Some(first.plan.generation), None)
            .await
            .unwrap();
        manager
            .plans()
            .update_revision(
                session_id,
                NewPlanRevision {
                    content_markdown: "# Replacement plan\n".to_string(),
                    expected_generation: second.plan.generation,
                    expected_parent_revision_id: None,
                    planner_provider: None,
                    planner_model: None,
                },
            )
            .await
            .unwrap();

        let copied = manager
            .copy_session(session_id, "all generations".to_string())
            .await
            .unwrap();
        let copied_statuses = sqlx::query_scalar::<_, String>(
            "SELECT status FROM session_plans WHERE session_id = ? ORDER BY generation",
        )
        .bind(&copied.id)
        .fetch_all(manager.storage().pool().await.unwrap())
        .await
        .unwrap();
        assert_eq!(
            copied_statuses,
            vec!["stale".to_string(), "stale".to_string()]
        );

        let exported = manager.export_session(session_id).await.unwrap();
        let outcome = manager
            .import_session(
                &exported,
                Some(SessionType::User),
                temp_dir.path().to_path_buf(),
                crate::session::import_formats::SessionImportTransport::Json,
            )
            .await
            .unwrap();
        let SessionImportOutcome::Imported(imported) = outcome else {
            panic!("first import of new content must create a session");
        };
        let imported_statuses = sqlx::query_scalar::<_, String>(
            "SELECT status FROM session_plans WHERE session_id = ? ORDER BY generation",
        )
        .bind(&imported.id)
        .fetch_all(manager.storage().pool().await.unwrap())
        .await
        .unwrap();
        assert_eq!(
            imported_statuses,
            vec!["stale".to_string(), "stale".to_string()]
        );
    }

    #[tokio::test]
    async fn archive_preserves_plan_and_delete_failure_is_atomic() {
        let (_temp_dir, manager, revision) = session_with_revision().await;
        let session_id = &revision.plan.session_id;
        manager
            .update(session_id)
            .archived_at(Some(chrono::Utc::now()))
            .apply()
            .await
            .unwrap();
        assert_eq!(
            manager
                .plans()
                .snapshot(session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Drafting
        );
        assert!(manager
            .plans()
            .abandon(session_id, revision.plan.generation, None)
            .await
            .is_err());
        assert!(manager
            .truncate_conversation_from_message(session_id, "source-message")
            .await
            .is_err());
        assert_eq!(
            manager
                .get_session(session_id, true)
                .await
                .unwrap()
                .conversation
                .unwrap()
                .len(),
            1
        );
        manager
            .update(session_id)
            .archived_at(None)
            .apply()
            .await
            .unwrap();

        let pool = manager.storage().pool().await.unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_plan_event_delete BEFORE DELETE ON session_plan_events BEGIN SELECT RAISE(ABORT, 'injected delete failure'); END",
        )
        .execute(pool)
        .await
        .unwrap();
        assert!(manager.delete_session(session_id).await.is_err());
        assert!(manager.get_session(session_id, false).await.is_ok());
        assert!(manager
            .plans()
            .snapshot(session_id)
            .await
            .unwrap()
            .is_some());
        sqlx::query("DROP TRIGGER fail_plan_event_delete")
            .execute(pool)
            .await
            .unwrap();
        manager.delete_session(session_id).await.unwrap();
        let remaining_plans: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM session_plans WHERE session_id = ?")
                .bind(session_id)
                .fetch_one(pool)
                .await
                .unwrap();
        assert_eq!(remaining_plans, 0);
        assert!(manager.get_session(session_id, false).await.is_err());
    }

    #[tokio::test]
    async fn source_hash_cache_tracks_direct_edits_and_transaction_rollbacks() {
        let (_temp_dir, manager, revision) = session_with_revision().await;
        let session_id = &revision.plan.session_id;
        let storage = manager.storage();
        let pool = storage.pool().await.unwrap();

        let mut baseline_tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let baseline = storage
            .current_source_hash_in_tx(&mut baseline_tx, session_id)
            .await
            .unwrap();
        baseline_tx.commit().await.unwrap();

        let rolled_back_content =
            serde_json::to_string(&Message::user().with_text("rolled-back evidence").content)
                .unwrap();
        let mut rolled_back_tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query("UPDATE messages SET content_json = ? WHERE session_id = ?")
            .bind(rolled_back_content)
            .bind(session_id)
            .execute(&mut *rolled_back_tx)
            .await
            .unwrap();
        let rolled_back = storage
            .current_source_hash_in_tx(&mut rolled_back_tx, session_id)
            .await
            .unwrap();
        assert_ne!(rolled_back, baseline);
        rolled_back_tx.rollback().await.unwrap();

        let mut restored_tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        let restored = storage
            .current_source_hash_in_tx(&mut restored_tx, session_id)
            .await
            .unwrap();
        restored_tx.commit().await.unwrap();
        assert_eq!(restored, baseline);

        let committed_content =
            serde_json::to_string(&Message::user().with_text("committed evidence").content)
                .unwrap();
        let mut committed_tx = pool.begin_with("BEGIN IMMEDIATE").await.unwrap();
        sqlx::query("UPDATE messages SET content_json = ? WHERE session_id = ?")
            .bind(committed_content)
            .bind(session_id)
            .execute(&mut *committed_tx)
            .await
            .unwrap();
        let committed = storage
            .current_source_hash_in_tx(&mut committed_tx, session_id)
            .await
            .unwrap();
        assert_ne!(committed, baseline);
        assert_ne!(committed, rolled_back);
        committed_tx.commit().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "local performance measurement; run explicitly with --ignored --nocapture"]
    async fn plan_performance_10k_messages_100_revisions() {
        let temp_dir = TempDir::new().unwrap();
        let manager = SessionManager::new(temp_dir.path().join("data"));
        let session = manager
            .create_session(
                temp_dir.path().to_path_buf(),
                "Plan performance".to_string(),
                SessionType::User,
                GoslingMode::Approve,
            )
            .await
            .unwrap();

        let message = Message::user().with_text("bounded source evidence");
        let content_json = serde_json::to_string(&message.content).unwrap();
        let metadata_json = serde_json::to_string(&message.metadata).unwrap();
        let pool = manager.storage().pool().await.unwrap();
        let mut tx = pool.begin().await.unwrap();
        for index in 0..10_000_i64 {
            sqlx::query(
                "INSERT INTO messages (message_id, session_id, role, content_json, created_timestamp, metadata_json) VALUES (?, ?, 'user', ?, ?, ?)",
            )
            .bind(format!("performance-message-{index}"))
            .bind(&session.id)
            .bind(&content_json)
            .bind(index)
            .bind(&metadata_json)
            .execute(&mut *tx)
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();

        let mut snapshot = manager
            .storage()
            .start_or_resume_plan(
                &session.id,
                Some("performance-provider".to_string()),
                Some("performance-model".to_string()),
                None,
                None,
            )
            .await
            .unwrap();
        let mut mutation_samples = Vec::with_capacity(100);
        for revision in 1..=100 {
            let started = Instant::now();
            snapshot = manager
                .plans()
                .update_revision(
                    &session.id,
                    NewPlanRevision {
                        content_markdown: format!("# Plan revision {revision}\n"),
                        expected_generation: snapshot.plan.generation,
                        expected_parent_revision_id: snapshot.plan.active_revision_id.clone(),
                        planner_provider: Some("performance-provider".to_string()),
                        planner_model: Some("performance-model".to_string()),
                    },
                )
                .await
                .unwrap();
            mutation_samples.push(started.elapsed());
        }

        let mut lookup_samples = Vec::with_capacity(200);
        for _ in 0..200 {
            let started = Instant::now();
            manager
                .plans()
                .snapshot(&session.id)
                .await
                .unwrap()
                .unwrap();
            lookup_samples.push(started.elapsed());
        }
        let lookup_p95 = p95(lookup_samples);
        let mutation_p95 = p95(mutation_samples);
        eprintln!(
            "plan-performance messages=10000 revisions=100 lookup_p95_us={} mutation_p95_us={}",
            lookup_p95.as_micros(),
            mutation_p95.as_micros()
        );
        assert!(
            lookup_p95 < Duration::from_millis(10),
            "plan snapshot p95 {lookup_p95:?} exceeded the 10 ms local target"
        );
        assert!(
            mutation_p95 < Duration::from_millis(25),
            "plan mutation p95 {mutation_p95:?} exceeded the 25 ms local target"
        );
    }
}
