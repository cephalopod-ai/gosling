use super::contracts::{TagteamLaunchSpecV1, TAGTEAM_CONTRACT_VERSION};
use super::reducer::{RunClass, TagteamRunSnapshot};
use crate::session::{SessionManager, SessionWorkflow};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::Row;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagteamRunBinding {
    pub session_id: String,
    pub launch_generation: u64,
    pub schema_version: u32,
    pub launch_spec: TagteamLaunchSpecV1,
    pub action_digest: String,
    pub producer_run_id: Option<String>,
    pub run_dir: Option<PathBuf>,
    pub state_root: Option<PathBuf>,
    pub last_sequence: u64,
    pub snapshot: TagteamRunSnapshot,
    pub terminal_class: Option<RunClass>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BindingUpdate {
    Applied,
    Stale { current_sequence: u64 },
}

pub struct TagteamRunStore {
    storage: Arc<crate::session::session_manager::SessionStorage>,
}

impl TagteamRunStore {
    pub fn new(session_manager: &SessionManager) -> Self {
        Self {
            storage: Arc::clone(session_manager.storage()),
        }
    }

    pub async fn create_binding(
        &self,
        session_id: &str,
        launch_spec: TagteamLaunchSpecV1,
    ) -> Result<TagteamRunBinding> {
        launch_spec.validate()?;
        let action_digest = launch_spec.action_digest()?;
        let launch_spec_json = serde_json::to_string(&launch_spec)?;
        let pool = self.storage.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let launch_generation_sql: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(launch_generation), 0) + 1 FROM tagteam_run_bindings WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(&mut *tx)
        .await?;
        let launch_generation = u64::try_from(launch_generation_sql)
            .context("generated Tagteam launch generation is negative")?;
        let pending_run_id = format!("pending:{session_id}:{launch_generation}");
        let snapshot = TagteamRunSnapshot::configured(&pending_run_id, Utc::now());
        let snapshot_json = serde_json::to_string(&snapshot)?;

        let result = sqlx::query(
            r#"
            INSERT INTO tagteam_run_bindings (
                session_id, launch_generation, schema_version, launch_spec_json,
                action_digest, last_sequence, snapshot_json
            ) VALUES (?, ?, ?, ?, ?, 0, ?)
            "#,
        )
        .bind(session_id)
        .bind(launch_generation_sql)
        .bind(i64::from(TAGTEAM_CONTRACT_VERSION))
        .bind(launch_spec_json)
        .bind(&action_digest)
        .bind(snapshot_json)
        .execute(&mut *tx)
        .await;

        if let Err(error) = result {
            return Err(error).context("create Tagteam run binding");
        }
        sqlx::query(
            "UPDATE sessions SET workflow_kind = ?, updated_at = datetime('now') WHERE id = ?",
        )
        .bind(SessionWorkflow::Tagteam.to_string())
        .bind(session_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        self.get(session_id, launch_generation)
            .await?
            .context("created Tagteam run binding was not readable")
    }

    pub async fn attach_run(
        &self,
        session_id: &str,
        launch_generation: u64,
        producer_run_id: &str,
        run_dir: Option<PathBuf>,
        state_root: Option<PathBuf>,
        snapshot: TagteamRunSnapshot,
    ) -> Result<BindingUpdate> {
        if producer_run_id.trim().is_empty() || snapshot.run_id != producer_run_id {
            anyhow::bail!("producer run identity and snapshot run identity must match");
        }
        if snapshot.last_sequence != 0 {
            anyhow::bail!("initial attached snapshot must start at sequence zero");
        }
        let launch_generation_sql = sqlite_integer(launch_generation, "launch generation")?;
        let snapshot_json = serde_json::to_string(&snapshot)?;
        let pool = self.storage.pool().await?;
        let result = sqlx::query(
            r#"
            UPDATE tagteam_run_bindings
            SET producer_run_id = ?, run_dir = ?, state_root = ?, snapshot_json = ?,
                updated_at = datetime('now')
            WHERE session_id = ? AND launch_generation = ? AND last_sequence = 0
              AND producer_run_id IS NULL
            "#,
        )
        .bind(producer_run_id)
        .bind(
            run_dir
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
        )
        .bind(
            state_root
                .as_ref()
                .map(|path| path.to_string_lossy().to_string()),
        )
        .bind(snapshot_json)
        .bind(session_id)
        .bind(launch_generation_sql)
        .execute(pool)
        .await?;
        self.classify_update(session_id, launch_generation, result.rows_affected())
            .await
    }

    pub async fn update_snapshot(
        &self,
        session_id: &str,
        launch_generation: u64,
        snapshot: &TagteamRunSnapshot,
    ) -> Result<BindingUpdate> {
        let current = self
            .get(session_id, launch_generation)
            .await?
            .with_context(|| {
                format!(
                    "Tagteam run binding not found for session {session_id} generation {launch_generation}"
                )
            })?;
        if current.producer_run_id.as_deref() != Some(snapshot.run_id.as_str()) {
            anyhow::bail!(
                "snapshot run identity {:?} does not match attached run identity {:?}",
                snapshot.run_id,
                current.producer_run_id
            );
        }
        let launch_generation_sql = sqlite_integer(launch_generation, "launch generation")?;
        let last_sequence = sqlite_integer(snapshot.last_sequence, "event sequence")?;
        let snapshot_json = serde_json::to_string(snapshot)?;
        let terminal_class = snapshot
            .class
            .is_terminal()
            .then(|| serde_json::to_string(&snapshot.class))
            .transpose()?;
        let pool = self.storage.pool().await?;
        let result = sqlx::query(
            r#"
            UPDATE tagteam_run_bindings
            SET last_sequence = ?, snapshot_json = ?, terminal_class = ?,
                updated_at = datetime('now')
            WHERE session_id = ? AND launch_generation = ? AND producer_run_id = ?
              AND last_sequence < ?
            "#,
        )
        .bind(last_sequence)
        .bind(snapshot_json)
        .bind(terminal_class)
        .bind(session_id)
        .bind(launch_generation_sql)
        .bind(&snapshot.run_id)
        .bind(last_sequence)
        .execute(pool)
        .await?;
        self.classify_update(session_id, launch_generation, result.rows_affected())
            .await
    }

    async fn classify_update(
        &self,
        session_id: &str,
        launch_generation: u64,
        rows_affected: u64,
    ) -> Result<BindingUpdate> {
        if rows_affected == 1 {
            return Ok(BindingUpdate::Applied);
        }
        let current = self
            .get(session_id, launch_generation)
            .await?
            .with_context(|| {
                format!(
                    "Tagteam run binding not found for session {session_id} generation {launch_generation}"
                )
            })?;
        Ok(BindingUpdate::Stale {
            current_sequence: current.last_sequence,
        })
    }

    pub async fn get(
        &self,
        session_id: &str,
        launch_generation: u64,
    ) -> Result<Option<TagteamRunBinding>> {
        let launch_generation = sqlite_integer(launch_generation, "launch generation")?;
        let pool = self.storage.pool().await?;
        let row = sqlx::query(
            r#"
            SELECT session_id, launch_generation, schema_version, launch_spec_json,
                   action_digest, producer_run_id, run_dir, state_root, last_sequence,
                   snapshot_json, terminal_class, created_at, updated_at
            FROM tagteam_run_bindings
            WHERE session_id = ? AND launch_generation = ?
            "#,
        )
        .bind(session_id)
        .bind(launch_generation)
        .fetch_optional(pool)
        .await?;
        row.map(row_to_binding).transpose()
    }

    pub async fn latest(&self, session_id: &str) -> Result<Option<TagteamRunBinding>> {
        let pool = self.storage.pool().await?;
        let row = sqlx::query(
            r#"
            SELECT session_id, launch_generation, schema_version, launch_spec_json,
                   action_digest, producer_run_id, run_dir, state_root, last_sequence,
                   snapshot_json, terminal_class, created_at, updated_at
            FROM tagteam_run_bindings
            WHERE session_id = ?
            ORDER BY launch_generation DESC
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .fetch_optional(pool)
        .await?;
        row.map(row_to_binding).transpose()
    }

    pub async fn prune_completed(&self, session_id: &str, keep: usize) -> Result<u64> {
        let keep =
            i64::try_from(keep).context("completed binding retention exceeds SQLite range")?;
        let pool = self.storage.pool().await?;
        let result = sqlx::query(
            r#"
            DELETE FROM tagteam_run_bindings
            WHERE session_id = ?
              AND terminal_class IS NOT NULL
              AND launch_generation NOT IN (
                  SELECT launch_generation
                  FROM tagteam_run_bindings
                  WHERE session_id = ? AND terminal_class IS NOT NULL
                  ORDER BY launch_generation DESC
                  LIMIT ?
              )
            "#,
        )
        .bind(session_id)
        .bind(session_id)
        .bind(keep)
        .execute(pool)
        .await?;
        Ok(result.rows_affected())
    }
}

fn row_to_binding(row: sqlx::sqlite::SqliteRow) -> Result<TagteamRunBinding> {
    let launch_generation = u64::try_from(row.try_get::<i64, _>("launch_generation")?)
        .context("stored Tagteam launch generation is negative")?;
    let schema_version = u32::try_from(row.try_get::<i64, _>("schema_version")?)
        .context("stored Tagteam schema version is outside the supported integer range")?;
    let last_sequence = u64::try_from(row.try_get::<i64, _>("last_sequence")?)
        .context("stored Tagteam event sequence is negative")?;
    let terminal_class = row
        .try_get::<Option<String>, _>("terminal_class")?
        .map(|value| serde_json::from_str(&value))
        .transpose()?;
    Ok(TagteamRunBinding {
        session_id: row.try_get("session_id")?,
        launch_generation,
        schema_version,
        launch_spec: serde_json::from_str(&row.try_get::<String, _>("launch_spec_json")?)?,
        action_digest: row.try_get("action_digest")?,
        producer_run_id: row.try_get("producer_run_id")?,
        run_dir: row
            .try_get::<Option<String>, _>("run_dir")?
            .map(PathBuf::from),
        state_root: row
            .try_get::<Option<String>, _>("state_root")?
            .map(PathBuf::from),
        last_sequence,
        snapshot: serde_json::from_str(&row.try_get::<String, _>("snapshot_json")?)?,
        terminal_class,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}

fn sqlite_integer(value: u64, field: &str) -> Result<i64> {
    i64::try_from(value).with_context(|| format!("{field} exceeds SQLite integer range"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GoslingMode;
    use crate::session::SessionType;
    use crate::tagteam::contracts::{
        AllowedPath, RecoveryPolicy, RepositoryIdentity, RoleTarget, TeamSpec, TimeBudget,
    };
    use crate::tagteam::reducer::{RunClass, TagteamObservation};
    use crate::tagteam::{Completeness, TagteamEventReducer};
    use tempfile::TempDir;

    fn spec(root: &std::path::Path) -> TagteamLaunchSpecV1 {
        std::fs::create_dir_all(root.join(".git")).unwrap();
        TagteamLaunchSpecV1 {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            repository: RepositoryIdentity::from_path(root).unwrap(),
            prompt: "fixture".to_string(),
            team: TeamSpec::Solo {
                worker: RoleTarget::new("ollama:fixture").unwrap(),
            },
            allowed_paths: vec![AllowedPath::new("src/").unwrap()],
            rounds: 1,
            time_budget: TimeBudget {
                invocation_timeout_seconds: 60,
                watchdog_timeout_seconds: 30,
                wall_timeout_seconds: 120,
            },
            test_preset: None,
            recovery_policy: RecoveryPolicy::Assist,
        }
    }

    async fn session(manager: &SessionManager, root: &std::path::Path) -> String {
        manager
            .create_session(
                root.to_path_buf(),
                "tagteam fixture".to_string(),
                SessionType::User,
                GoslingMode::default(),
            )
            .await
            .unwrap()
            .id
    }

    #[tokio::test]
    async fn binding_creation_switches_workflow_and_increments_generation() {
        let root = TempDir::new().unwrap();
        let manager = SessionManager::new(root.path().to_path_buf());
        let session_id = session(&manager, root.path()).await;
        let store = TagteamRunStore::new(&manager);

        let first = store
            .create_binding(&session_id, spec(root.path()))
            .await
            .unwrap();
        let second = store
            .create_binding(&session_id, spec(root.path()))
            .await
            .unwrap();
        assert_eq!(first.launch_generation, 1);
        assert_eq!(second.launch_generation, 2);
        assert_eq!(
            manager
                .get_session(&session_id, false)
                .await
                .unwrap()
                .workflow_kind,
            SessionWorkflow::Tagteam
        );
    }

    #[tokio::test]
    async fn snapshot_updates_reject_stale_sequences() {
        let root = TempDir::new().unwrap();
        let manager = SessionManager::new(root.path().to_path_buf());
        let session_id = session(&manager, root.path()).await;
        let store = TagteamRunStore::new(&manager);
        let binding = store
            .create_binding(&session_id, spec(root.path()))
            .await
            .unwrap();
        let run_id = "producer-run-1";
        let mut snapshot = TagteamRunSnapshot::configured(run_id, Utc::now());
        assert_eq!(
            store
                .attach_run(
                    &session_id,
                    binding.launch_generation,
                    run_id,
                    None,
                    None,
                    snapshot.clone(),
                )
                .await
                .unwrap(),
            BindingUpdate::Applied
        );
        TagteamEventReducer::apply(
            &mut snapshot,
            TagteamObservation {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                run_id: run_id.to_string(),
                sequence: 1,
                observed_at: Utc::now(),
                class: RunClass::Running,
                producer_status: "running".to_string(),
                phase: Some("implementing".to_string()),
                role: Some("worker".to_string()),
                reason_code: None,
                summary: None,
                diff: None,
                open_findings: Some(0),
                completeness: Completeness::Complete,
                authoritative_terminal: false,
            },
        )
        .unwrap();
        assert_eq!(
            store
                .update_snapshot(&session_id, binding.launch_generation, &snapshot)
                .await
                .unwrap(),
            BindingUpdate::Applied
        );
        assert_eq!(
            store
                .update_snapshot(&session_id, binding.launch_generation, &snapshot)
                .await
                .unwrap(),
            BindingUpdate::Stale {
                current_sequence: 1
            }
        );

        let wrong_run = TagteamRunSnapshot::configured("different-run", Utc::now());
        assert!(store
            .update_snapshot(&session_id, binding.launch_generation, &wrong_run)
            .await
            .unwrap_err()
            .to_string()
            .contains("does not match attached run identity"));
        assert!(store
            .get(&session_id, u64::MAX)
            .await
            .unwrap_err()
            .to_string()
            .contains("exceeds SQLite integer range"));
    }

    #[tokio::test]
    async fn deleting_session_cascades_bindings() {
        let root = TempDir::new().unwrap();
        let manager = SessionManager::new(root.path().to_path_buf());
        let session_id = session(&manager, root.path()).await;
        let store = TagteamRunStore::new(&manager);
        store
            .create_binding(&session_id, spec(root.path()))
            .await
            .unwrap();

        manager.delete_session(&session_id).await.unwrap();
        assert!(store.latest(&session_id).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn completed_binding_retention_keeps_latest_generation() {
        let root = TempDir::new().unwrap();
        let manager = SessionManager::new(root.path().to_path_buf());
        let session_id = session(&manager, root.path()).await;
        let store = TagteamRunStore::new(&manager);

        for index in 1..=3 {
            let binding = store
                .create_binding(&session_id, spec(root.path()))
                .await
                .unwrap();
            let run_id = format!("producer-run-{index}");
            let mut snapshot = TagteamRunSnapshot::configured(&run_id, Utc::now());
            store
                .attach_run(
                    &session_id,
                    binding.launch_generation,
                    &run_id,
                    None,
                    None,
                    snapshot.clone(),
                )
                .await
                .unwrap();
            TagteamEventReducer::apply(
                &mut snapshot,
                TagteamObservation {
                    schema_version: TAGTEAM_CONTRACT_VERSION,
                    run_id,
                    sequence: 1,
                    observed_at: Utc::now(),
                    class: RunClass::Passed,
                    producer_status: "passed".to_string(),
                    phase: None,
                    role: None,
                    reason_code: None,
                    summary: None,
                    diff: None,
                    open_findings: Some(0),
                    completeness: Completeness::Complete,
                    authoritative_terminal: true,
                },
            )
            .unwrap();
            store
                .update_snapshot(&session_id, binding.launch_generation, &snapshot)
                .await
                .unwrap();
        }

        assert_eq!(store.prune_completed(&session_id, 1).await.unwrap(), 2);
        let latest = store.latest(&session_id).await.unwrap().unwrap();
        assert_eq!(latest.launch_generation, 3);
        assert_eq!(latest.terminal_class, Some(RunClass::Passed));
    }
}
