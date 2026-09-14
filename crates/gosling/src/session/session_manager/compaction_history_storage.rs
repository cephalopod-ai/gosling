use super::{Session, SessionStorage};
use crate::context_mgmt::CompactionResult;
use crate::conversation::Conversation;
use anyhow::Result;
use chrono::{Duration, Utc};
use gosling_providers::conversation::token_usage::Usage;
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, Sqlite, Transaction};

const COMPACTION_REVISION_SCHEMA_VERSION: u32 = 1;
const MAX_PAGE_SIZE: usize = 200;

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
            self.max_total_bytes > 0,
            "max_total_bytes must be greater than zero"
        );
        Ok(())
    }

    pub(super) fn configured() -> Self {
        let policy: Self = crate::config::Config::global()
            .get_param("GOSLING_COMPACTION_HISTORY_POLICY")
            .unwrap_or_default();
        policy.validate().map(|()| policy).unwrap_or_default()
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct CompactionRevisionPayloadV1 {
    schema_version: u32,
    summary: String,
    source_message_ids: Vec<String>,
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
        sqlx::query(r#"UPDATE session_compaction_state SET purged_count = next_generation - 1 - (
            SELECT COUNT(*) FROM session_compaction_revisions revisions WHERE revisions.session_id = session_compaction_state.session_id
        )"#).execute(&mut **tx).await?;
        Ok(())
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
        let policy = CompactionHistoryPolicyV1::configured();
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
}
