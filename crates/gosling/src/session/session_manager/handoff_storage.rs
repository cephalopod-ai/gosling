use super::{HandoffToolOperation, SessionManager, SessionStorage};
use anyhow::Result;
use chrono::Utc;
use gosling_providers::conversation::token_usage::Usage;
use gosling_providers::model::ModelConfig;
use gosling_sdk_types::session_handoff::{
    SessionContinuityClassDto, SessionHandoffSnapshotV1Dto, SessionHandoffStatusDto,
};
use sqlx::{Sqlite, Transaction};

const DEFAULT_HANDOFF_RETENTION_GENERATIONS: i64 = 5;

fn handoff_retention_generations() -> i64 {
    std::env::var("GOSLING_HANDOFF_RETENTION_GENERATIONS")
        .ok()
        .and_then(|value| value.parse::<i64>().ok())
        .unwrap_or(DEFAULT_HANDOFF_RETENTION_GENERATIONS)
        .clamp(1, 100)
}

fn status_name(status: SessionHandoffStatusDto) -> &'static str {
    match status {
        SessionHandoffStatusDto::Prepared => "prepared",
        SessionHandoffStatusDto::Activating => "activating",
        SessionHandoffStatusDto::Active => "active",
        SessionHandoffStatusDto::Failed => "failed",
        SessionHandoffStatusDto::RolledBack => "rolled_back",
        SessionHandoffStatusDto::Superseded => "superseded",
    }
}

impl SessionStorage {
    pub(super) async fn insert_handoff_snapshot_in_tx(
        tx: &mut Transaction<'_, Sqlite>,
        snapshot: &SessionHandoffSnapshotV1Dto,
    ) -> Result<()> {
        let snapshot_json = serde_json::to_string(snapshot)?;
        sqlx::query(
            r#"
            INSERT INTO session_handoff_snapshots (
                snapshot_id, session_id, generation, schema_version, trigger, status,
                from_provider, from_model, to_provider, to_model, covered_through_row_id,
                source_hash, estimated_tokens, snapshot_json, failure, created_at,
                activated_at, acknowledged_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&snapshot.snapshot_id)
        .bind(&snapshot.session_id)
        .bind(snapshot.generation as i64)
        .bind(snapshot.schema_version as i64)
        .bind(
            serde_json::to_value(snapshot.trigger)?
                .as_str()
                .unwrap_or_default(),
        )
        .bind(status_name(snapshot.status))
        .bind(&snapshot.source.provider_id)
        .bind(&snapshot.source.requested_model)
        .bind(snapshot.target.provider_id.as_deref().unwrap_or_default())
        .bind(
            snapshot
                .target
                .requested_model
                .as_deref()
                .unwrap_or_default(),
        )
        .bind(snapshot.coverage.covered_through_row_id)
        .bind(&snapshot.coverage.source_hash)
        .bind(snapshot.coverage.estimated_tokens as i64)
        .bind(snapshot_json)
        .bind(&snapshot.failure)
        .bind(&snapshot.created_at)
        .bind(&snapshot.activated_at)
        .bind(&snapshot.acknowledged_at)
        .execute(&mut **tx)
        .await?;
        Ok(())
    }

    pub(super) async fn prepare_handoff_snapshot(
        &self,
        mut snapshot: SessionHandoffSnapshotV1Dto,
        expected_current_generation: Option<u64>,
    ) -> Result<SessionHandoffSnapshotV1Dto> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let current_generation = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(generation) FROM session_handoff_snapshots WHERE session_id = ?",
        )
        .bind(&snapshot.session_id)
        .fetch_one(&mut *tx)
        .await?
        .unwrap_or(0) as u64;
        if let Some(expected) = expected_current_generation {
            anyhow::ensure!(
                current_generation == expected,
                "stale handoff generation: expected {expected}, current {current_generation}"
            );
        }

        snapshot.generation = current_generation + 1;
        snapshot.status = SessionHandoffStatusDto::Prepared;
        Self::insert_handoff_snapshot_in_tx(&mut tx, &snapshot).await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    pub(super) async fn update_handoff_status(
        &self,
        snapshot_id: &str,
        status: SessionHandoffStatusDto,
        failure: Option<&str>,
    ) -> Result<SessionHandoffSnapshotV1Dto> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let snapshot_json = sqlx::query_scalar::<_, String>(
            "SELECT snapshot_json FROM session_handoff_snapshots WHERE snapshot_id = ?",
        )
        .bind(snapshot_id)
        .fetch_one(&mut *tx)
        .await?;
        let mut snapshot: SessionHandoffSnapshotV1Dto = serde_json::from_str(&snapshot_json)?;
        snapshot.status = status;
        snapshot.failure = None;
        if let Some(failure) = failure {
            crate::session::handoff::set_redacted_failure(&mut snapshot, failure);
        }
        if status == SessionHandoffStatusDto::Active {
            snapshot.activated_at = Some(Utc::now().to_rfc3339());
        }
        let snapshot_json = serde_json::to_string(&snapshot)?;
        sqlx::query(
            "UPDATE session_handoff_snapshots SET status = ?, failure = ?, activated_at = ?, snapshot_json = ? WHERE snapshot_id = ?",
        )
        .bind(status_name(status))
        .bind(&snapshot.failure)
        .bind(&snapshot.activated_at)
        .bind(snapshot_json)
        .bind(snapshot_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(snapshot)
    }

    pub(super) async fn update_handoff_snapshot(
        &self,
        snapshot: &SessionHandoffSnapshotV1Dto,
    ) -> Result<()> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let updated = sqlx::query(
            "UPDATE session_handoff_snapshots SET snapshot_json = ?, acknowledged_at = ? WHERE snapshot_id = ? AND status = 'activating'",
        )
        .bind(serde_json::to_string(snapshot)?)
        .bind(&snapshot.acknowledged_at)
        .bind(&snapshot.snapshot_id)
        .execute(pool)
        .await?;
        anyhow::ensure!(
            updated.rows_affected() == 1,
            "handoff snapshot is no longer activating"
        );
        Ok(())
    }

    pub(super) async fn commit_provider_transition(
        &self,
        session_manager: &SessionManager,
        snapshot_id: &str,
        provider_name: &str,
        model_config: ModelConfig,
        mode: crate::config::GoslingMode,
    ) -> Result<SessionHandoffSnapshotV1Dto> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let snapshot_json = sqlx::query_scalar::<_, String>(
            "SELECT snapshot_json FROM session_handoff_snapshots WHERE snapshot_id = ? AND status = 'activating'",
        )
        .bind(snapshot_id)
        .fetch_one(&mut *tx)
        .await?;
        let mut snapshot: SessionHandoffSnapshotV1Dto = serde_json::from_str(&snapshot_json)?;
        let latest_generation = sqlx::query_scalar::<_, i64>(
            "SELECT MAX(generation) FROM session_handoff_snapshots WHERE session_id = ?",
        )
        .bind(&snapshot.session_id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(
            latest_generation == snapshot.generation as i64,
            "handoff snapshot was superseded before activation"
        );
        let latest_message_row = sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(id) FROM messages WHERE session_id = ?",
        )
        .bind(&snapshot.session_id)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(
            latest_message_row == snapshot.coverage.covered_through_row_id,
            "session changed after the handoff checkpoint was prepared"
        );

        let current_context_tokens =
            if snapshot.continuity_class == SessionContinuityClassDto::NewContextOnly {
                0
            } else {
                i32::try_from(snapshot.coverage.estimated_tokens).unwrap_or(i32::MAX)
            };
        let builder = session_manager
            .update(&snapshot.session_id)
            .provider_name(provider_name)
            .model_config(model_config)
            .usage(Usage::new(
                Some(current_context_tokens),
                Some(0),
                Some(current_context_tokens),
            ))
            .gosling_mode(mode);
        Self::apply_update_in_tx(&mut tx, builder).await?;

        let superseded_at = Utc::now().to_rfc3339();
        let prior_active = sqlx::query_as::<_, (String, String)>(
            "SELECT snapshot_id, snapshot_json FROM session_handoff_snapshots WHERE session_id = ? AND status IN ('prepared', 'activating', 'active') AND snapshot_id <> ?",
        )
        .bind(&snapshot.session_id)
        .bind(snapshot_id)
        .fetch_all(&mut *tx)
        .await?;
        for (prior_snapshot_id, prior_snapshot_json) in prior_active {
            let mut prior_snapshot: SessionHandoffSnapshotV1Dto =
                serde_json::from_str(&prior_snapshot_json)?;
            prior_snapshot.status = SessionHandoffStatusDto::Superseded;
            sqlx::query(
                "UPDATE session_handoff_snapshots SET status = 'superseded', superseded_at = ?, snapshot_json = ? WHERE snapshot_id = ?",
            )
            .bind(&superseded_at)
            .bind(serde_json::to_string(&prior_snapshot)?)
            .bind(prior_snapshot_id)
            .execute(&mut *tx)
            .await?;
        }
        snapshot.status = SessionHandoffStatusDto::Active;
        snapshot.activated_at = Some(Utc::now().to_rfc3339());

        if let Some(covered_through_row_id) = snapshot.coverage.covered_through_row_id {
            let metadata_rows = sqlx::query_as::<_, (i64, String)>(
                "SELECT id, metadata_json FROM messages WHERE session_id = ? AND id <= ?",
            )
            .bind(&snapshot.session_id)
            .bind(covered_through_row_id)
            .fetch_all(&mut *tx)
            .await?;
            for (row_id, metadata_json) in metadata_rows {
                let metadata: crate::conversation::message::MessageMetadata =
                    serde_json::from_str(&metadata_json)?;
                sqlx::query("UPDATE messages SET metadata_json = ? WHERE id = ?")
                    .bind(serde_json::to_string(&metadata.with_agent_invisible())?)
                    .bind(row_id)
                    .execute(&mut *tx)
                    .await?;
            }
        }
        if snapshot.delivery_strategy
            != gosling_sdk_types::session_handoff::HandoffDeliveryStrategyDto::NewContext
        {
            let checkpoint = crate::session::handoff::handoff_bootstrap_message(&snapshot)?;
            sqlx::query(
                "INSERT INTO messages (message_id, session_id, role, content_json, created_timestamp, metadata_json) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(checkpoint.id.as_deref())
            .bind(&snapshot.session_id)
            .bind(super::role_to_string(&checkpoint.role))
            .bind(serde_json::to_string(&checkpoint.content)?)
            .bind(checkpoint.created)
            .bind(serde_json::to_string(&checkpoint.metadata)?)
            .execute(&mut *tx)
            .await?;
        }
        let snapshot_json = serde_json::to_string(&snapshot)?;
        sqlx::query(
            "UPDATE session_handoff_snapshots SET status = 'active', activated_at = ?, snapshot_json = ? WHERE snapshot_id = ?",
        )
        .bind(&snapshot.activated_at)
        .bind(snapshot_json)
        .bind(snapshot_id)
        .execute(&mut *tx)
        .await?;

        let retention_floor = snapshot.generation as i64 - handoff_retention_generations() + 1;
        if retention_floor > 1 {
            sqlx::query(
                "DELETE FROM session_handoff_snapshots WHERE session_id = ? AND generation < ? AND status IN ('failed', 'rolled_back', 'superseded')",
            )
            .bind(&snapshot.session_id)
            .bind(retention_floor)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(snapshot)
    }

    pub(super) async fn latest_handoff_snapshot(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionHandoffSnapshotV1Dto>> {
        let snapshot_json = sqlx::query_scalar::<_, String>(
            "SELECT snapshot_json FROM session_handoff_snapshots WHERE session_id = ? ORDER BY generation DESC LIMIT 1",
        )
        .bind(session_id)
        .fetch_optional(self.pool().await?)
        .await?;
        snapshot_json
            .map(|json| serde_json::from_str(&json).map_err(Into::into))
            .transpose()
    }

    pub(super) async fn latest_handoff_generation(&self, session_id: &str) -> Result<u64> {
        Ok(sqlx::query_scalar::<_, Option<i64>>(
            "SELECT MAX(generation) FROM session_handoff_snapshots WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(self.pool().await?)
        .await?
        .unwrap_or(0) as u64)
    }

    pub(super) async fn acknowledge_handoff_snapshot(&self, snapshot_id: &str) -> Result<()> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let snapshot_json = sqlx::query_scalar::<_, String>(
            "SELECT snapshot_json FROM session_handoff_snapshots WHERE snapshot_id = ? AND status IN ('activating', 'active')",
        )
        .bind(snapshot_id)
        .fetch_one(&mut *tx)
        .await?;
        let mut snapshot: SessionHandoffSnapshotV1Dto = serde_json::from_str(&snapshot_json)?;
        if snapshot.acknowledged_at.is_none() {
            snapshot.acknowledged_at = Some(Utc::now().to_rfc3339());
        }
        sqlx::query(
            "UPDATE session_handoff_snapshots SET acknowledged_at = ?, snapshot_json = ? WHERE snapshot_id = ?",
        )
        .bind(&snapshot.acknowledged_at)
        .bind(serde_json::to_string(&snapshot)?)
        .bind(snapshot_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn handoff_tool_operations(
        &self,
        session_id: &str,
        limit: usize,
    ) -> Result<Vec<HandoffToolOperation>> {
        let rows = sqlx::query_as::<_, (String, String, String, String)>(
            "SELECT operation_id, tool_request_id, tool_name, state FROM tool_operations WHERE session_id = ? ORDER BY updated_at DESC, operation_id DESC LIMIT ?",
        )
        .bind(session_id)
        .bind(limit.clamp(1, 100) as i64)
        .fetch_all(self.pool().await?)
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(operation_id, tool_request_id, tool_name, state)| HandoffToolOperation {
                    operation_id,
                    tool_request_id,
                    tool_name,
                    state,
                },
            )
            .collect())
    }
}
