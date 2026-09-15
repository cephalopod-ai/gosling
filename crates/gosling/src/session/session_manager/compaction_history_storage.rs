use super::{Session, SessionManager, SessionStorage};
use crate::context_mgmt::CompactionResult;
use crate::conversation::Conversation;
use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use gosling_providers::conversation::token_usage::Usage;
use gosling_sdk_types::custom_requests::{
    ApplyCompactionHistoryPolicyResponse, CompactionEffectDto, CompactionHistoryImpactDto,
    CompactionHistoryPolicyDto, CompactionHistoryPurgeMode, CompactionHistoryStatsDto,
    CompactionRevisionDto, CompactionRevisionListItemDto, CompactionTriggerDto,
    DeleteCompactionRevisionRequest, DeleteCompactionRevisionResponse,
    GetCompactionRevisionRequest, GetCompactionRevisionResponse, ListCompactionRevisionsRequest,
    ListCompactionRevisionsResponse, PreviewCompactionHistoryPolicyResponse,
    PurgeCompactionHistoryRequest, PurgeCompactionHistoryResponse,
    SetCompactionRevisionPinnedRequest,
};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Sqlite, Transaction};
use std::collections::{HashMap, HashSet};

const COMPACTION_REVISION_SCHEMA_VERSION: u32 = 1;
const MAX_PAGE_SIZE: usize = 200;
const MAX_RETENTION_DAYS: u32 = 3_650;
const MAX_PURGE_GRACE_DAYS: u32 = 365;
const MAX_REVISIONS_PER_SESSION: u32 = 10_000;
const MAX_TOTAL_BYTES: u64 = 16 * 1024 * 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum CompactionHistoryError {
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Validation(String),
    #[error("{0}")]
    Partial(String),
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CompactionHistoryPolicyV1 {
    pub version: u32,
    pub capture_enabled: bool,
    pub retention_days: Option<u32>,
    pub purge_grace_days: u32,
    pub max_revisions_per_session: u32,
    pub max_total_bytes: u64,
}

impl Default for CompactionHistoryPolicyV1 {
    fn default() -> Self {
        Self {
            version: 1,
            capture_enabled: true,
            retention_days: Some(90),
            purge_grace_days: 7,
            max_revisions_per_session: 100,
            max_total_bytes: 256 * 1024 * 1024,
        }
    }
}

impl CompactionHistoryPolicyV1 {
    pub fn validate(&self) -> Result<()> {
        anyhow::ensure!(
            self.version == 1,
            "unsupported compaction history policy version"
        );
        anyhow::ensure!(
            self.max_revisions_per_session > 0,
            "max_revisions_per_session must be greater than zero"
        );
        anyhow::ensure!(
            self.max_revisions_per_session <= MAX_REVISIONS_PER_SESSION,
            "max_revisions_per_session must be at most {MAX_REVISIONS_PER_SESSION}"
        );
        anyhow::ensure!(
            self.max_total_bytes > 0,
            "max_total_bytes must be greater than zero"
        );
        anyhow::ensure!(
            self.max_total_bytes <= MAX_TOTAL_BYTES,
            "max_total_bytes must be at most {MAX_TOTAL_BYTES}"
        );
        if let Some(days) = self.retention_days {
            anyhow::ensure!(days > 0, "retention_days must be null or greater than zero");
            anyhow::ensure!(
                days <= MAX_RETENTION_DAYS,
                "retention_days must be at most {MAX_RETENTION_DAYS}"
            );
        }
        anyhow::ensure!(
            self.purge_grace_days <= MAX_PURGE_GRACE_DAYS,
            "purge_grace_days must be at most {MAX_PURGE_GRACE_DAYS}"
        );
        Ok(())
    }

    pub fn configured_result() -> Result<Self> {
        let policy: Self =
            match crate::config::Config::global().get_param("GOSLING_COMPACTION_HISTORY_POLICY") {
                Ok(policy) => policy,
                Err(crate::config::ConfigError::NotFound(_)) => Self::default(),
                Err(error) => return Err(error.into()),
            };
        policy.validate()?;
        Ok(policy)
    }
}

impl From<CompactionHistoryPolicyV1> for CompactionHistoryPolicyDto {
    fn from(value: CompactionHistoryPolicyV1) -> Self {
        Self {
            version: value.version,
            capture_enabled: value.capture_enabled,
            retention_days: value.retention_days,
            purge_grace_days: value.purge_grace_days,
            max_revisions_per_session: value.max_revisions_per_session,
            max_total_bytes: value.max_total_bytes,
        }
    }
}

impl TryFrom<CompactionHistoryPolicyDto> for CompactionHistoryPolicyV1 {
    type Error = anyhow::Error;

    fn try_from(value: CompactionHistoryPolicyDto) -> Result<Self> {
        let policy = Self {
            version: value.version,
            capture_enabled: value.capture_enabled,
            retention_days: value.retention_days,
            purge_grace_days: value.purge_grace_days,
            max_revisions_per_session: value.max_revisions_per_session,
            max_total_bytes: value.max_total_bytes,
        };
        policy.validate()?;
        Ok(policy)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CompactionRevisionPayloadV1 {
    schema_version: u32,
    pub summary: String,
    pub source_message_ids: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct CompactionRevisionDraft {
    trigger: String,
    effect: String,
    first_source_message_id: Option<String>,
    last_source_message_id: Option<String>,
    source_message_count: i64,
    source_hash: String,
    summary_hash: String,
    prompt_hash: String,
    provider: Option<String>,
    selected_model: Option<String>,
    resolved_model: String,
    usage: Usage,
    estimated_tokens_before: i64,
    estimated_tokens_after: i64,
    payload_json: String,
    payload_bytes: i64,
}

impl CompactionRevisionDraft {
    pub fn from_result(
        session: &Session,
        result: &CompactionResult,
        temporary: bool,
        estimated_tokens_after: Option<i32>,
    ) -> Self {
        let payload = CompactionRevisionPayloadV1 {
            schema_version: COMPACTION_REVISION_SCHEMA_VERSION,
            summary: result.summary.clone(),
            source_message_ids: result.source_message_ids.clone(),
        };
        let payload_json = serde_json::to_string(&payload).expect("compaction payload serializes");
        Self {
            trigger: match result.trigger {
                crate::context_mgmt::CompactionTrigger::Manual => "manual",
                crate::context_mgmt::CompactionTrigger::AutomaticThreshold => "automatic_threshold",
                crate::context_mgmt::CompactionTrigger::OverflowRecovery => "overflow_recovery",
            }
            .to_string(),
            effect: if temporary { "temporary" } else { "durable" }.to_string(),
            first_source_message_id: result.source_message_ids.first().cloned(),
            last_source_message_id: result.source_message_ids.last().cloned(),
            source_message_count: result.source_message_count as i64,
            source_hash: result.source_hash.clone(),
            summary_hash: result.summary_hash.clone(),
            prompt_hash: result.prompt_hash.clone(),
            provider: session.provider_name.clone(),
            selected_model: session
                .model_config
                .as_ref()
                .map(|model| model.model_name.clone()),
            resolved_model: result.usage.model.clone(),
            usage: result.usage.usage,
            estimated_tokens_before: result.estimated_tokens_before as i64,
            estimated_tokens_after: estimated_tokens_after
                .map(i64::from)
                .unwrap_or(result.estimated_tokens_after as i64),
            payload_bytes: payload_json.len() as i64,
            payload_json,
        }
    }
}

#[derive(Clone, Debug, FromRow, PartialEq, Eq)]
pub struct CompactionRevision {
    pub revision_id: String,
    pub session_id: String,
    pub generation: i64,
    pub trigger: String,
    pub effect: String,
    pub parent_revision_id: Option<String>,
    pub first_source_message_id: Option<String>,
    pub last_source_message_id: Option<String>,
    pub source_message_count: i64,
    pub source_hash: String,
    pub summary_hash: String,
    pub prompt_hash: String,
    pub provider: Option<String>,
    pub selected_model: Option<String>,
    pub resolved_model: String,
    pub usage_json: String,
    pub estimated_tokens_before: i64,
    pub estimated_tokens_after: i64,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub purge_after: Option<String>,
    pub pinned_at: Option<String>,
    pub payload_json: String,
    pub payload_bytes: i64,
}

impl CompactionRevision {
    pub fn to_dto(&self) -> Result<CompactionRevisionDto> {
        let payload: CompactionRevisionPayloadV1 = serde_json::from_str(&self.payload_json)?;
        Ok(CompactionRevisionDto {
            revision_id: self.revision_id.clone(),
            session_id: self.session_id.clone(),
            generation: u64::try_from(self.generation)?,
            trigger: self.trigger_dto()?,
            effect: self.effect_dto()?,
            parent_revision_id: self.parent_revision_id.clone(),
            first_source_message_id: self.first_source_message_id.clone(),
            last_source_message_id: self.last_source_message_id.clone(),
            source_message_count: u64::try_from(self.source_message_count)?,
            source_hash: self.source_hash.clone(),
            summary_hash: self.summary_hash.clone(),
            prompt_hash: self.prompt_hash.clone(),
            provider: self.provider.clone(),
            selected_model: self.selected_model.clone(),
            resolved_model: self.resolved_model.clone(),
            usage: serde_json::from_str(&self.usage_json)?,
            estimated_tokens_before: u64::try_from(self.estimated_tokens_before)?,
            estimated_tokens_after: u64::try_from(self.estimated_tokens_after)?,
            created_at: self.created_at.clone(),
            expires_at: self.expires_at.clone(),
            purge_after: self.purge_after.clone(),
            pinned_at: self.pinned_at.clone(),
            expired: self.expired()?,
            summary: payload.summary,
            source_message_ids: payload.source_message_ids,
            payload_bytes: u64::try_from(self.payload_bytes)?,
        })
    }

    fn trigger_dto(&self) -> Result<CompactionTriggerDto> {
        compaction_trigger_dto(&self.trigger)
    }

    fn effect_dto(&self) -> Result<CompactionEffectDto> {
        compaction_effect_dto(&self.effect)
    }

    fn expired(&self) -> Result<bool> {
        compaction_expired(self.pinned_at.as_deref(), self.expires_at.as_deref())
    }
}

#[derive(Clone, Debug, FromRow)]
struct CompactionRevisionListItemRow {
    revision_id: String,
    generation: i64,
    trigger: String,
    effect: String,
    source_message_count: i64,
    summary_hash: String,
    provider: Option<String>,
    selected_model: Option<String>,
    resolved_model: String,
    estimated_tokens_before: i64,
    estimated_tokens_after: i64,
    created_at: String,
    expires_at: Option<String>,
    purge_after: Option<String>,
    pinned_at: Option<String>,
    payload_bytes: i64,
}

impl CompactionRevisionListItemRow {
    fn to_dto(&self) -> Result<CompactionRevisionListItemDto> {
        Ok(CompactionRevisionListItemDto {
            revision_id: self.revision_id.clone(),
            generation: u64::try_from(self.generation)?,
            trigger: compaction_trigger_dto(&self.trigger)?,
            effect: compaction_effect_dto(&self.effect)?,
            source_message_count: u64::try_from(self.source_message_count)?,
            summary_hash: self.summary_hash.clone(),
            provider: self.provider.clone(),
            selected_model: self.selected_model.clone(),
            resolved_model: self.resolved_model.clone(),
            estimated_tokens_before: u64::try_from(self.estimated_tokens_before)?,
            estimated_tokens_after: u64::try_from(self.estimated_tokens_after)?,
            created_at: self.created_at.clone(),
            expires_at: self.expires_at.clone(),
            purge_after: self.purge_after.clone(),
            pinned_at: self.pinned_at.clone(),
            expired: compaction_expired(self.pinned_at.as_deref(), self.expires_at.as_deref())?,
            payload_bytes: u64::try_from(self.payload_bytes)?,
        })
    }
}

#[derive(Clone, Debug, FromRow, Serialize)]
struct CompactionImpactRow {
    revision_id: String,
    session_id: String,
    generation: i64,
    created_at: String,
    pinned_at: Option<String>,
    payload_bytes: i64,
}

fn parse_timestamp(value: &str) -> Result<DateTime<Utc>> {
    Ok(DateTime::parse_from_rfc3339(value)?.with_timezone(&Utc))
}

fn compaction_trigger_dto(value: &str) -> Result<CompactionTriggerDto> {
    match value {
        "manual" => Ok(CompactionTriggerDto::Manual),
        "automatic_threshold" => Ok(CompactionTriggerDto::AutomaticThreshold),
        "overflow_recovery" => Ok(CompactionTriggerDto::OverflowRecovery),
        value => anyhow::bail!("unknown compaction trigger: {value}"),
    }
}

fn compaction_effect_dto(value: &str) -> Result<CompactionEffectDto> {
    match value {
        "durable" => Ok(CompactionEffectDto::Durable),
        "temporary" => Ok(CompactionEffectDto::Temporary),
        value => anyhow::bail!("unknown compaction effect: {value}"),
    }
}

fn compaction_expired(pinned_at: Option<&str>, expires_at: Option<&str>) -> Result<bool> {
    Ok(pinned_at.is_none()
        && expires_at
            .map(parse_timestamp)
            .transpose()?
            .is_some_and(|expires_at| expires_at <= Utc::now()))
}

impl SessionStorage {
    pub(super) async fn create_compaction_history_schema(
        tx: &mut Transaction<'_, Sqlite>,
    ) -> Result<()> {
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS session_compaction_state (
                session_id TEXT PRIMARY KEY REFERENCES sessions(id) ON DELETE CASCADE,
                next_generation INTEGER NOT NULL DEFAULT 1 CHECK(next_generation >= 1),
                purged_count INTEGER NOT NULL DEFAULT 0 CHECK(purged_count >= 0)
            )"#,
        )
        .execute(&mut **tx)
        .await?;
        sqlx::query(
            r#"CREATE TABLE IF NOT EXISTS session_compaction_revisions (
                revision_id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
                generation INTEGER NOT NULL CHECK(generation >= 1),
                schema_version INTEGER NOT NULL CHECK(schema_version = 1),
                trigger TEXT NOT NULL CHECK(trigger IN ('manual', 'automatic_threshold', 'overflow_recovery')),
                effect TEXT NOT NULL CHECK(effect IN ('durable', 'temporary')),
                parent_revision_id TEXT,
                first_source_message_id TEXT,
                last_source_message_id TEXT,
                source_message_count INTEGER NOT NULL CHECK(source_message_count >= 0),
                source_hash TEXT NOT NULL,
                summary_hash TEXT NOT NULL,
                prompt_hash TEXT NOT NULL,
                provider TEXT,
                selected_model TEXT,
                resolved_model TEXT NOT NULL,
                usage_json TEXT NOT NULL,
                estimated_tokens_before INTEGER NOT NULL CHECK(estimated_tokens_before >= 0),
                estimated_tokens_after INTEGER NOT NULL CHECK(estimated_tokens_after >= 0),
                created_at TEXT NOT NULL,
                expires_at TEXT,
                purge_after TEXT,
                pinned_at TEXT,
                payload_json TEXT NOT NULL,
                payload_bytes INTEGER NOT NULL CHECK(payload_bytes >= 0),
                UNIQUE(session_id, generation)
            )"#,
        ).execute(&mut **tx).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_session_compaction_generation ON session_compaction_revisions(session_id, generation DESC)").execute(&mut **tx).await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS idx_session_compaction_purge ON session_compaction_revisions(purge_after, pinned_at, created_at)").execute(&mut **tx).await?;
        Ok(())
    }

    async fn insert_compaction_revision_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: &str,
        draft: CompactionRevisionDraft,
        policy: &CompactionHistoryPolicyV1,
    ) -> Result<()> {
        sqlx::query("INSERT OR IGNORE INTO session_compaction_state(session_id) VALUES (?)")
            .bind(session_id)
            .execute(&mut **tx)
            .await?;
        let generation: i64 = sqlx::query_scalar(
            "SELECT next_generation FROM session_compaction_state WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&mut **tx)
        .await?;
        let parent: Option<String> = sqlx::query_scalar("SELECT revision_id FROM session_compaction_revisions WHERE session_id = ? ORDER BY generation DESC LIMIT 1")
            .bind(session_id).fetch_optional(&mut **tx).await?;
        let created_at = Utc::now();
        let expires_at = policy
            .retention_days
            .map(|days| created_at + Duration::days(i64::from(days)));
        let purge_after =
            expires_at.map(|expiry| expiry + Duration::days(i64::from(policy.purge_grace_days)));
        sqlx::query(
            r#"INSERT INTO session_compaction_revisions (
            revision_id, session_id, generation, schema_version, trigger, effect,
            parent_revision_id, first_source_message_id, last_source_message_id,
            source_message_count, source_hash, summary_hash, prompt_hash, provider,
            selected_model, resolved_model, usage_json, estimated_tokens_before,
            estimated_tokens_after, created_at, expires_at, purge_after, payload_json,
            payload_bytes
        ) VALUES (?, ?, ?, 1, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"#,
        )
        .bind(uuid::Uuid::now_v7().to_string())
        .bind(session_id)
        .bind(generation)
        .bind(draft.trigger)
        .bind(draft.effect)
        .bind(parent)
        .bind(draft.first_source_message_id)
        .bind(draft.last_source_message_id)
        .bind(draft.source_message_count)
        .bind(draft.source_hash)
        .bind(draft.summary_hash)
        .bind(draft.prompt_hash)
        .bind(draft.provider)
        .bind(draft.selected_model)
        .bind(draft.resolved_model)
        .bind(serde_json::to_string(&draft.usage)?)
        .bind(draft.estimated_tokens_before)
        .bind(draft.estimated_tokens_after)
        .bind(created_at.to_rfc3339())
        .bind(expires_at.map(|value| value.to_rfc3339()))
        .bind(purge_after.map(|value| value.to_rfc3339()))
        .bind(draft.payload_json)
        .bind(draft.payload_bytes)
        .execute(&mut **tx)
        .await?;
        sqlx::query("UPDATE session_compaction_state SET next_generation = next_generation + 1 WHERE session_id = ?")
            .bind(session_id).execute(&mut **tx).await?;
        Ok(())
    }

    pub(super) async fn cleanup_compaction_history_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        policy: &CompactionHistoryPolicyV1,
    ) -> Result<()> {
        let now = Utc::now().to_rfc3339();
        sqlx::query("DELETE FROM session_compaction_revisions WHERE pinned_at IS NULL AND purge_after IS NOT NULL AND purge_after <= ?")
            .bind(now).execute(&mut **tx).await?;
        sqlx::query(
            r#"DELETE FROM session_compaction_revisions WHERE revision_id IN (
            SELECT revision_id FROM (
                SELECT revision_id, pinned_at,
                    ROW_NUMBER() OVER (
                        PARTITION BY session_id
                        ORDER BY (pinned_at IS NOT NULL) DESC, generation DESC
                    ) AS position
                FROM session_compaction_revisions
            ) WHERE position > ? AND pinned_at IS NULL
        )"#,
        )
        .bind(i64::from(policy.max_revisions_per_session))
        .execute(&mut **tx)
        .await?;
        let total: i64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(payload_bytes), 0) FROM session_compaction_revisions",
        )
        .fetch_one(&mut **tx)
        .await?;
        let excess =
            total.saturating_sub(i64::try_from(policy.max_total_bytes).unwrap_or(i64::MAX));
        if excess > 0 {
            let candidates: Vec<(String, i64)> = sqlx::query_as("SELECT revision_id, payload_bytes FROM session_compaction_revisions WHERE pinned_at IS NULL ORDER BY created_at, generation")
                .fetch_all(&mut **tx).await?;
            let mut reclaimed = 0;
            for (revision_id, bytes) in candidates {
                if reclaimed >= excess {
                    break;
                }
                sqlx::query("DELETE FROM session_compaction_revisions WHERE revision_id = ?")
                    .bind(revision_id)
                    .execute(&mut **tx)
                    .await?;
                reclaimed += bytes;
            }
        }
        Self::refresh_compaction_purge_counts_in_tx(tx).await?;
        Ok(())
    }

    async fn refresh_compaction_purge_counts_in_tx(tx: &mut Transaction<'_, Sqlite>) -> Result<()> {
        sqlx::query(r#"UPDATE session_compaction_state SET purged_count = next_generation - 1 - (
            SELECT COUNT(*) FROM session_compaction_revisions revisions WHERE revisions.session_id = session_compaction_state.session_id
        )"#).execute(&mut **tx).await?;
        Ok(())
    }

    pub(super) async fn reconcile_compaction_expiration_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        policy: &CompactionHistoryPolicyV1,
    ) -> Result<()> {
        let rows: Vec<(String, String)> = sqlx::query_as(
            "SELECT revision_id, created_at FROM session_compaction_revisions WHERE pinned_at IS NULL",
        )
        .fetch_all(&mut **tx)
        .await?;
        for (revision_id, created_at) in rows {
            let created_at = parse_timestamp(&created_at)?;
            let expires_at = policy
                .retention_days
                .map(|days| created_at + Duration::days(i64::from(days)));
            let purge_after = expires_at
                .map(|expiry| expiry + Duration::days(i64::from(policy.purge_grace_days)));
            sqlx::query(
                "UPDATE session_compaction_revisions SET expires_at = ?, purge_after = ? WHERE revision_id = ?",
            )
            .bind(expires_at.map(|value| value.to_rfc3339()))
            .bind(purge_after.map(|value| value.to_rfc3339()))
            .bind(revision_id)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }

    async fn compaction_history_stats(
        &self,
        session_id: Option<&str>,
    ) -> Result<CompactionHistoryStatsDto> {
        let pool = self.pool().await?;
        let mut tx = pool.begin().await?;
        let stats = Self::compaction_history_stats_in_tx(&mut tx, session_id).await?;
        tx.commit().await?;
        Ok(stats)
    }

    async fn compaction_history_stats_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        session_id: Option<&str>,
    ) -> Result<CompactionHistoryStatsDto> {
        let (revision_count, pinned_count, payload_bytes, pinned_bytes): (i64, i64, i64, i64) =
            if let Some(session_id) = session_id {
                sqlx::query_as(
                    r#"SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN pinned_at IS NOT NULL THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(payload_bytes), 0),
                    COALESCE(SUM(CASE WHEN pinned_at IS NOT NULL THEN payload_bytes ELSE 0 END), 0)
                    FROM session_compaction_revisions WHERE session_id = ?"#,
                )
                .bind(session_id)
                .fetch_one(&mut **tx)
                .await?
            } else {
                sqlx::query_as(
                    r#"SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN pinned_at IS NOT NULL THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(payload_bytes), 0),
                    COALESCE(SUM(CASE WHEN pinned_at IS NOT NULL THEN payload_bytes ELSE 0 END), 0)
                    FROM session_compaction_revisions"#,
                )
                .fetch_one(&mut **tx)
                .await?
            };
        let purged_count: i64 = if let Some(session_id) = session_id {
            sqlx::query_scalar(
                "SELECT COALESCE(purged_count, 0) FROM session_compaction_state WHERE session_id = ?",
            )
            .bind(session_id)
            .fetch_optional(&mut **tx)
            .await?
            .unwrap_or(0)
        } else {
            sqlx::query_scalar(
                "SELECT COALESCE(SUM(purged_count), 0) FROM session_compaction_state",
            )
            .fetch_one(&mut **tx)
            .await?
        };
        Ok(CompactionHistoryStatsDto {
            revision_count: u64::try_from(revision_count)?,
            pinned_count: u64::try_from(pinned_count)?,
            payload_bytes: u64::try_from(payload_bytes)?,
            pinned_bytes: u64::try_from(pinned_bytes)?,
            purged_count: u64::try_from(purged_count)?,
        })
    }

    async fn preview_compaction_history_policy(
        &self,
        policy: &CompactionHistoryPolicyV1,
    ) -> Result<PreviewCompactionHistoryPolicyResponse> {
        policy.validate()?;
        let pool = self.pool().await?;
        let mut tx = pool.begin().await?;
        let preview = Self::preview_compaction_history_policy_in_tx(&mut tx, policy).await?;
        tx.commit().await?;
        Ok(preview)
    }

    async fn preview_compaction_history_policy_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        policy: &CompactionHistoryPolicyV1,
    ) -> Result<PreviewCompactionHistoryPolicyResponse> {
        let rows: Vec<CompactionImpactRow> = sqlx::query_as(
            r#"SELECT revision_id, session_id, generation, created_at, pinned_at, payload_bytes
            FROM session_compaction_revisions ORDER BY session_id, generation"#,
        )
        .fetch_all(&mut **tx)
        .await?;
        let current = Self::compaction_history_stats_in_tx(tx, None).await?;
        let now = Utc::now();
        let mut time_removed = HashSet::new();
        let mut would_expire_count = 0_u64;
        if let Some(retention_days) = policy.retention_days {
            let retention = Duration::days(i64::from(retention_days));
            let grace = Duration::days(i64::from(policy.purge_grace_days));
            for row in &rows {
                if row.pinned_at.is_some() {
                    continue;
                }
                let expires_at = parse_timestamp(&row.created_at)? + retention;
                if expires_at <= now {
                    would_expire_count += 1;
                }
                if expires_at + grace <= now {
                    time_removed.insert(row.revision_id.clone());
                }
            }
        }

        let mut limit_removed = HashSet::new();
        let mut by_session: HashMap<&str, Vec<&CompactionImpactRow>> = HashMap::new();
        for row in &rows {
            if !time_removed.contains(&row.revision_id) {
                by_session.entry(&row.session_id).or_default().push(row);
            }
        }
        for session_rows in by_session.values_mut() {
            session_rows.sort_by(|left, right| {
                right
                    .pinned_at
                    .is_some()
                    .cmp(&left.pinned_at.is_some())
                    .then_with(|| right.generation.cmp(&left.generation))
            });
            for row in session_rows
                .iter()
                .skip(policy.max_revisions_per_session as usize)
            {
                if row.pinned_at.is_none() {
                    limit_removed.insert(row.revision_id.clone());
                }
            }
        }
        let pinned_count_exceeds_limit = by_session.values().any(|session_rows| {
            session_rows
                .iter()
                .filter(|row| row.pinned_at.is_some())
                .count()
                > policy.max_revisions_per_session as usize
        });

        let mut projected_bytes = rows
            .iter()
            .filter(|row| {
                !time_removed.contains(&row.revision_id)
                    && !limit_removed.contains(&row.revision_id)
            })
            .try_fold(0_u64, |total, row| {
                Ok::<_, anyhow::Error>(total + u64::try_from(row.payload_bytes)?)
            })?;
        let mut capacity_candidates: Vec<&CompactionImpactRow> = rows
            .iter()
            .filter(|row| {
                row.pinned_at.is_none()
                    && !time_removed.contains(&row.revision_id)
                    && !limit_removed.contains(&row.revision_id)
            })
            .collect();
        capacity_candidates.sort_by(|left, right| {
            left.created_at
                .cmp(&right.created_at)
                .then_with(|| left.generation.cmp(&right.generation))
        });
        for row in capacity_candidates {
            if projected_bytes <= policy.max_total_bytes {
                break;
            }
            projected_bytes = projected_bytes.saturating_sub(u64::try_from(row.payload_bytes)?);
            limit_removed.insert(row.revision_id.clone());
        }

        let projected_revision_count = rows
            .iter()
            .filter(|row| {
                !time_removed.contains(&row.revision_id)
                    && !limit_removed.contains(&row.revision_id)
            })
            .count() as u64;
        let projected_over_budget_bytes = projected_bytes.saturating_sub(policy.max_total_bytes);
        let mut warnings = Vec::new();
        if !policy.capture_enabled {
            warnings.push("New compaction snapshots will not be recorded".to_string());
        }
        if pinned_count_exceeds_limit {
            warnings.push(
                "Pinned snapshots exceed a per-session count limit and will be retained until unpinned"
                    .to_string(),
            );
        }
        if projected_over_budget_bytes > 0 {
            warnings.push(
                "Pinned snapshots exceed the byte limit and will be retained until unpinned"
                    .to_string(),
            );
        }
        let impact = CompactionHistoryImpactDto {
            current,
            would_expire_count,
            would_purge_now_count: time_removed.len() as u64,
            would_remove_for_limits_count: limit_removed.len() as u64,
            projected_revision_count,
            projected_payload_bytes: projected_bytes,
            projected_over_budget_bytes,
            warnings,
        };
        let preview_material = serde_json::to_vec(&(policy, &rows, &impact))?;
        let preview_hash = blake3::hash(&preview_material).to_hex().to_string();
        Ok(PreviewCompactionHistoryPolicyResponse {
            policy: policy.clone().into(),
            impact,
            preview_hash,
        })
    }

    pub(super) async fn commit_compaction(
        &self,
        session_id: &str,
        conversation: Option<&Conversation>,
        current_usage: Usage,
        accumulated_delta: Usage,
        cost_delta: Option<f64>,
        revision: CompactionRevisionDraft,
    ) -> Result<()> {
        let policy = CompactionHistoryPolicyV1::configured_result()?;
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        Self::ensure_compaction_allowed_in_tx(&mut tx, session_id).await?;
        let source_binding = if conversation.is_some() {
            Self::open_plan_source_binding_in_tx(&mut tx, session_id).await?
        } else {
            None
        };
        if let Some(conversation) = conversation {
            Self::replace_conversation_in_tx(&mut tx, session_id, conversation).await?;
        }
        let plan_staled = if let Some((_, expected_hash)) = source_binding {
            let (_, current_hash) = self.current_source_hash_in_tx(&mut tx, session_id).await?;
            if current_hash != expected_hash {
                Self::stale_open_plan_in_tx(
                    &mut tx,
                    session_id,
                    "conversation compaction changed the plan source ledger",
                )
                .await?
            } else {
                false
            }
        } else {
            false
        };
        Self::record_usage_in_tx(
            &mut tx,
            session_id,
            current_usage,
            accumulated_delta,
            cost_delta,
        )
        .await?;
        if policy.capture_enabled {
            Self::insert_compaction_revision_in_tx(&mut tx, session_id, revision, &policy).await?;
        }
        Self::cleanup_compaction_history_in_tx(&mut tx, &policy).await?;
        tx.commit().await?;
        self.publish_stale_plan_update(session_id, plan_staled)
            .await;
        Ok(())
    }

    pub(super) async fn list_compaction_revisions(
        &self,
        session_id: &str,
        limit: usize,
        before_generation: Option<u64>,
    ) -> Result<Vec<CompactionRevision>> {
        let pool = self.pool().await?;
        sqlx::query_as(
            r#"SELECT revision_id, session_id, generation, trigger, effect,
            parent_revision_id, first_source_message_id, last_source_message_id,
            source_message_count, source_hash, summary_hash, prompt_hash,
            provider, selected_model, resolved_model, usage_json, estimated_tokens_before,
            estimated_tokens_after, created_at, expires_at, purge_after, pinned_at,
            payload_json, payload_bytes FROM session_compaction_revisions
            WHERE session_id = ? AND (? IS NULL OR generation < ?)
            ORDER BY generation DESC LIMIT ?"#,
        )
        .bind(session_id)
        .bind(before_generation.map(|value| value as i64))
        .bind(before_generation.map(|value| value as i64))
        .bind(limit.clamp(1, MAX_PAGE_SIZE) as i64)
        .fetch_all(pool)
        .await
        .map_err(Into::into)
    }

    async fn list_compaction_revisions_page(
        &self,
        session_id: &str,
        limit: usize,
        before_generation: Option<u64>,
        include_expired: bool,
    ) -> Result<(Vec<CompactionRevisionListItemRow>, bool, u64)> {
        let pool = self.pool().await?;
        let limit = limit.clamp(1, MAX_PAGE_SIZE);
        let before_generation = before_generation.map(|value| value as i64);
        let now = Utc::now().to_rfc3339();
        let mut revisions: Vec<CompactionRevisionListItemRow> = if include_expired {
            sqlx::query_as(
                r#"SELECT revision_id, generation, trigger, effect, source_message_count,
                summary_hash, provider, selected_model, resolved_model, estimated_tokens_before,
                estimated_tokens_after, created_at, expires_at, purge_after, pinned_at, payload_bytes
                FROM session_compaction_revisions
                WHERE session_id = ? AND (? IS NULL OR generation < ?)
                ORDER BY generation DESC LIMIT ?"#,
            )
            .bind(session_id)
            .bind(before_generation)
            .bind(before_generation)
            .bind((limit + 1) as i64)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as(
                r#"SELECT revision_id, generation, trigger, effect, source_message_count,
                summary_hash, provider, selected_model, resolved_model, estimated_tokens_before,
                estimated_tokens_after, created_at, expires_at, purge_after, pinned_at, payload_bytes
                FROM session_compaction_revisions
                WHERE session_id = ? AND (? IS NULL OR generation < ?)
                AND (pinned_at IS NOT NULL OR expires_at IS NULL OR expires_at > ?)
                ORDER BY generation DESC LIMIT ?"#,
            )
            .bind(session_id)
            .bind(before_generation)
            .bind(before_generation)
            .bind(&now)
            .bind((limit + 1) as i64)
            .fetch_all(pool)
            .await?
        };
        let has_more = revisions.len() > limit;
        revisions.truncate(limit);
        let total_count: i64 = if include_expired {
            sqlx::query_scalar(
                "SELECT COUNT(*) FROM session_compaction_revisions WHERE session_id = ?",
            )
            .bind(session_id)
            .fetch_one(pool)
            .await?
        } else {
            sqlx::query_scalar(
                r#"SELECT COUNT(*) FROM session_compaction_revisions WHERE session_id = ?
                AND (pinned_at IS NOT NULL OR expires_at IS NULL OR expires_at > ?)"#,
            )
            .bind(session_id)
            .bind(now)
            .fetch_one(pool)
            .await?
        };
        Ok((revisions, has_more, u64::try_from(total_count)?))
    }

    async fn get_compaction_revision(
        &self,
        session_id: &str,
        generation: u64,
    ) -> Result<CompactionRevision> {
        sqlx::query_as(
            r#"SELECT revision_id, session_id, generation, trigger, effect,
            parent_revision_id, first_source_message_id, last_source_message_id,
            source_message_count, source_hash, summary_hash, prompt_hash,
            provider, selected_model, resolved_model, usage_json, estimated_tokens_before,
            estimated_tokens_after, created_at, expires_at, purge_after, pinned_at,
            payload_json, payload_bytes FROM session_compaction_revisions
            WHERE session_id = ? AND generation = ?"#,
        )
        .bind(session_id)
        .bind(i64::try_from(generation)?)
        .fetch_optional(self.pool().await?)
        .await?
        .ok_or_else(|| {
            CompactionHistoryError::NotFound(format!(
                "Compaction generation {generation} was not found for session {session_id}"
            ))
            .into()
        })
    }
}

impl SessionManager {
    pub async fn compaction_history_stats(
        &self,
        session_id: Option<&str>,
    ) -> Result<CompactionHistoryStatsDto> {
        if let Some(session_id) = session_id {
            self.get_session(session_id, false).await?;
        }
        self.storage.compaction_history_stats(session_id).await
    }

    pub async fn preview_compaction_history_policy(
        &self,
        policy: CompactionHistoryPolicyV1,
    ) -> Result<PreviewCompactionHistoryPolicyResponse> {
        self.storage
            .preview_compaction_history_policy(&policy)
            .await
    }

    pub async fn apply_compaction_history_policy(
        &self,
        policy: CompactionHistoryPolicyV1,
    ) -> Result<ApplyCompactionHistoryPolicyResponse> {
        self.apply_compaction_history_policy_inner(policy, None, false, || Ok(()))
            .await
    }

    pub(crate) async fn apply_compaction_history_policy_if_unchanged<F>(
        &self,
        policy: CompactionHistoryPolicyV1,
        expected_preview_hash: &str,
        persist_policy: F,
    ) -> Result<ApplyCompactionHistoryPolicyResponse>
    where
        F: FnOnce() -> Result<()> + Send,
    {
        self.apply_compaction_history_policy_inner(
            policy,
            Some(expected_preview_hash),
            true,
            persist_policy,
        )
        .await
    }

    async fn apply_compaction_history_policy_inner<F>(
        &self,
        policy: CompactionHistoryPolicyV1,
        expected_preview_hash: Option<&str>,
        persisted_to_config: bool,
        persist_policy: F,
    ) -> Result<ApplyCompactionHistoryPolicyResponse>
    where
        F: FnOnce() -> Result<()> + Send,
    {
        policy.validate()?;
        let write_guard = self.storage.acquire_write_guard().await;
        let pool = self.storage.pool().await?.clone();
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let current_preview =
            SessionStorage::preview_compaction_history_policy_in_tx(&mut tx, &policy).await?;
        if expected_preview_hash.is_some_and(|expected| expected != current_preview.preview_hash) {
            return Err(CompactionHistoryError::Conflict(
                "Context History changed after the preview; review the updated impact before saving"
                    .to_string(),
            )
            .into());
        }
        let before = current_preview.impact.current;
        SessionStorage::reconcile_compaction_expiration_in_tx(&mut tx, &policy).await?;
        SessionStorage::cleanup_compaction_history_in_tx(&mut tx, &policy).await?;
        let remaining = SessionStorage::compaction_history_stats_in_tx(&mut tx, None).await?;
        persist_policy()?;
        let commit_policy = policy.clone();
        // The config save is durable; finish SQLite work even if the ACP request is cancelled.
        let remaining = tokio::spawn(async move {
            let _write_guard = write_guard;
            match tx.commit().await {
                Ok(()) => Ok(remaining),
                Err(commit_error) => {
                    let recovery = async {
                        let mut recovery_tx = pool.begin_with("BEGIN IMMEDIATE").await?;
                        SessionStorage::reconcile_compaction_expiration_in_tx(
                            &mut recovery_tx,
                            &commit_policy,
                        )
                        .await?;
                        SessionStorage::cleanup_compaction_history_in_tx(
                            &mut recovery_tx,
                            &commit_policy,
                        )
                        .await?;
                        let recovered =
                            SessionStorage::compaction_history_stats_in_tx(&mut recovery_tx, None)
                                .await?;
                        recovery_tx.commit().await?;
                        Ok::<_, anyhow::Error>(recovered)
                    }
                    .await;
                    match recovery {
                        Ok(recovered) => Ok(recovered),
                        Err(recovery_error) if persisted_to_config => {
                            Err(CompactionHistoryError::Partial(format!(
                                "Context History policy was saved, but snapshot cleanup could not be confirmed ({commit_error}); immediate recovery failed ({recovery_error}). The saved policy will be reconciled when session storage opens again"
                            ))
                            .into())
                        }
                        Err(recovery_error) => Err(recovery_error),
                    }
                }
            }
        })
        .await
        .map_err(|join_error| -> anyhow::Error {
            if persisted_to_config {
                CompactionHistoryError::Partial(format!(
                    "Context History policy was saved, but snapshot cleanup could not be confirmed because the commit task stopped ({join_error}). The saved policy will be reconciled when session storage opens again"
                ))
                .into()
            } else {
                join_error.into()
            }
        })??;
        Ok(ApplyCompactionHistoryPolicyResponse {
            policy: policy.into(),
            cleanup: PurgeCompactionHistoryResponse {
                deleted_count: before
                    .revision_count
                    .saturating_sub(remaining.revision_count),
                deleted_bytes: before.payload_bytes.saturating_sub(remaining.payload_bytes),
                remaining,
            },
        })
    }

    pub async fn list_compaction_history(
        &self,
        request: ListCompactionRevisionsRequest,
    ) -> Result<ListCompactionRevisionsResponse> {
        self.get_session(&request.session_id, false).await?;
        let limit = request.limit.unwrap_or(50).clamp(1, MAX_PAGE_SIZE);
        let (rows, has_more, total_count) = self
            .storage
            .list_compaction_revisions_page(
                &request.session_id,
                limit,
                request.before_generation,
                request.include_expired,
            )
            .await?;
        let revisions = rows
            .iter()
            .map(CompactionRevisionListItemRow::to_dto)
            .collect::<Result<Vec<_>>>()?;
        let next_before_generation = has_more
            .then(|| revisions.last().map(|revision| revision.generation))
            .flatten();
        let purged_count = self
            .storage
            .compaction_history_stats(Some(&request.session_id))
            .await?
            .purged_count;
        Ok(ListCompactionRevisionsResponse {
            revisions,
            next_before_generation,
            total_count,
            purged_count,
        })
    }

    pub async fn get_compaction_history_revision(
        &self,
        request: GetCompactionRevisionRequest,
    ) -> Result<GetCompactionRevisionResponse> {
        self.get_session(&request.session_id, false).await?;
        Ok(GetCompactionRevisionResponse {
            revision: self
                .storage
                .get_compaction_revision(&request.session_id, request.generation)
                .await?
                .to_dto()?,
        })
    }

    pub async fn set_compaction_history_pinned(
        &self,
        request: SetCompactionRevisionPinnedRequest,
    ) -> Result<GetCompactionRevisionResponse> {
        self.get_session(&request.session_id, false).await?;
        let policy = (!request.pinned)
            .then(CompactionHistoryPolicyV1::configured_result)
            .transpose()?;
        let _write_guard = self.storage.acquire_write_guard().await;
        let pool = self.storage.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let created_at: Option<String> = sqlx::query_scalar(
            "SELECT created_at FROM session_compaction_revisions WHERE session_id = ? AND generation = ?",
        )
        .bind(&request.session_id)
        .bind(i64::try_from(request.generation)?)
        .fetch_optional(&mut *tx)
        .await?;
        let created_at = created_at.ok_or_else(|| {
            CompactionHistoryError::NotFound(format!(
                "Compaction generation {} was not found for session {}",
                request.generation, request.session_id
            ))
        })?;
        if request.pinned {
            sqlx::query(
                r#"UPDATE session_compaction_revisions
                SET pinned_at = ?, expires_at = NULL, purge_after = NULL
                WHERE session_id = ? AND generation = ?"#,
            )
            .bind(Utc::now().to_rfc3339())
            .bind(&request.session_id)
            .bind(i64::try_from(request.generation)?)
            .execute(&mut *tx)
            .await?;
        } else {
            let created_at = parse_timestamp(&created_at)?;
            let policy = policy.expect("unpinned revisions require the configured policy");
            let expires_at = policy
                .retention_days
                .map(|days| created_at + Duration::days(i64::from(days)));
            let purge_after = expires_at
                .map(|expiry| expiry + Duration::days(i64::from(policy.purge_grace_days)));
            sqlx::query(
                r#"UPDATE session_compaction_revisions
                SET pinned_at = NULL, expires_at = ?, purge_after = ?
                WHERE session_id = ? AND generation = ?"#,
            )
            .bind(expires_at.map(|value| value.to_rfc3339()))
            .bind(purge_after.map(|value| value.to_rfc3339()))
            .bind(&request.session_id)
            .bind(i64::try_from(request.generation)?)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        self.get_compaction_history_revision(GetCompactionRevisionRequest {
            session_id: request.session_id,
            generation: request.generation,
        })
        .await
    }

    pub async fn delete_compaction_history_revision(
        &self,
        request: DeleteCompactionRevisionRequest,
    ) -> Result<DeleteCompactionRevisionResponse> {
        self.get_session(&request.session_id, false).await?;
        let _write_guard = self.storage.acquire_write_guard().await;
        let pool = self.storage.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let result = sqlx::query(
            "DELETE FROM session_compaction_revisions WHERE session_id = ? AND generation = ?",
        )
        .bind(&request.session_id)
        .bind(i64::try_from(request.generation)?)
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() == 0 {
            return Err(CompactionHistoryError::NotFound(format!(
                "Compaction generation {} was not found for session {}",
                request.generation, request.session_id
            ))
            .into());
        }
        SessionStorage::refresh_compaction_purge_counts_in_tx(&mut tx).await?;
        let purged_count: i64 = sqlx::query_scalar(
            "SELECT purged_count FROM session_compaction_state WHERE session_id = ?",
        )
        .bind(&request.session_id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(DeleteCompactionRevisionResponse {
            deleted: true,
            purged_count: u64::try_from(purged_count)?,
        })
    }

    pub async fn purge_compaction_history(
        &self,
        request: PurgeCompactionHistoryRequest,
    ) -> Result<PurgeCompactionHistoryResponse> {
        self.get_session(&request.session_id, false).await?;
        let before = self
            .storage
            .compaction_history_stats(Some(&request.session_id))
            .await?;
        let _write_guard = self.storage.acquire_write_guard().await;
        let pool = self.storage.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let result = match request.mode {
            CompactionHistoryPurgeMode::Expired => {
                sqlx::query(
                    r#"DELETE FROM session_compaction_revisions
                    WHERE session_id = ? AND pinned_at IS NULL
                    AND expires_at IS NOT NULL AND expires_at <= ?"#,
                )
                .bind(&request.session_id)
                .bind(Utc::now().to_rfc3339())
                .execute(&mut *tx)
                .await?
            }
            CompactionHistoryPurgeMode::AllUnpinned => {
                sqlx::query(
                    "DELETE FROM session_compaction_revisions WHERE session_id = ? AND pinned_at IS NULL",
                )
                .bind(&request.session_id)
                .execute(&mut *tx)
                .await?
            }
        };
        SessionStorage::refresh_compaction_purge_counts_in_tx(&mut tx).await?;
        tx.commit().await?;
        let remaining = self
            .storage
            .compaction_history_stats(Some(&request.session_id))
            .await?;
        Ok(PurgeCompactionHistoryResponse {
            deleted_count: result.rows_affected(),
            deleted_bytes: before.payload_bytes.saturating_sub(remaining.payload_bytes),
            remaining,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_defaults_match_the_version_one_contract() {
        let policy = CompactionHistoryPolicyV1::default();
        policy.validate().unwrap();
        assert!(policy.capture_enabled);
        assert_eq!(policy.retention_days, Some(90));
        assert_eq!(policy.purge_grace_days, 7);
        assert_eq!(policy.max_revisions_per_session, 100);
        assert_eq!(policy.max_total_bytes, 268_435_456);
    }

    #[test]
    fn policy_rejects_non_positive_capacity_limits() {
        let policy = CompactionHistoryPolicyV1 {
            max_revisions_per_session: 0,
            ..Default::default()
        };
        assert!(policy.validate().is_err());
        let policy = CompactionHistoryPolicyV1 {
            max_total_bytes: 0,
            ..Default::default()
        };
        assert!(policy.validate().is_err());
    }

    #[test]
    fn policy_rejects_values_above_supported_bounds() {
        for policy in [
            CompactionHistoryPolicyV1 {
                retention_days: Some(MAX_RETENTION_DAYS + 1),
                ..Default::default()
            },
            CompactionHistoryPolicyV1 {
                purge_grace_days: MAX_PURGE_GRACE_DAYS + 1,
                ..Default::default()
            },
            CompactionHistoryPolicyV1 {
                max_revisions_per_session: MAX_REVISIONS_PER_SESSION + 1,
                ..Default::default()
            },
            CompactionHistoryPolicyV1 {
                max_total_bytes: MAX_TOTAL_BYTES + 1,
                ..Default::default()
            },
        ] {
            assert!(policy.validate().is_err());
        }
    }
}
