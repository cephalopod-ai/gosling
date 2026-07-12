use super::contracts::{TagteamLaunchSpecV1, MAX_IDENTIFIER_BYTES, TAGTEAM_CONTRACT_VERSION};
use super::reducer::{
    validate_snapshot, ReduceOutcome, ReducerError, RunClass, TagteamEventReducer,
    TagteamObservation, TagteamRunSnapshot,
};
use crate::session::{SessionManager, SessionWorkflow};
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagteamRunBinding {
    pub session_id: String,
    pub launch_generation: u64,
    pub schema_version: u32,
    pub launch_spec: TagteamLaunchSpecV1,
    pub action_digest: String,
    pub launch_nonce: String,
    pub producer_run_id: Option<String>,
    pub run_dir: Option<PathBuf>,
    pub state_root: Option<PathBuf>,
    pub last_sequence: u64,
    pub snapshot: TagteamRunSnapshot,
    pub terminal_class: Option<RunClass>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl TagteamRunBinding {
    pub fn idempotency_key(&self) -> String {
        let mut digest = Sha256::new();
        digest.update(b"gosling-tagteam-start-v1\0");
        digest.update(self.session_id.as_bytes());
        digest.update([0]);
        digest.update(self.launch_generation.to_be_bytes());
        digest.update([0]);
        digest.update(self.launch_nonce.as_bytes());
        digest.update([0]);
        digest.update(self.action_digest.as_bytes());
        let digest = digest.finalize();
        format!("gosling-tagteam-v1-{digest:x}")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingUpdate {
    Applied,
    Duplicate { current_sequence: u64 },
    Stale { current_sequence: u64 },
    AlreadyAttached,
    AttachmentConflict { attached_run_id: String },
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn create_binding(
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
            r#"
            INSERT INTO tagteam_launch_counters (session_id, last_generation)
            VALUES (?, 1)
            ON CONFLICT(session_id) DO UPDATE SET
                last_generation = tagteam_launch_counters.last_generation + 1
            WHERE tagteam_launch_counters.last_generation < ?
            RETURNING last_generation
            "#,
        )
        .bind(session_id)
        .bind(i64::MAX)
        .fetch_optional(&mut *tx)
        .await?
        .context("Tagteam launch generation is exhausted")?;
        let launch_generation = u64::try_from(launch_generation_sql)
            .context("generated Tagteam launch generation is negative")?;
        let launch_nonce = Uuid::new_v4().simple().to_string();
        sqlx::query(
            r#"
            INSERT INTO tagteam_launch_identities(
                launch_nonce, session_id, launch_generation
            ) VALUES (?, ?, ?)
            "#,
        )
        .bind(&launch_nonce)
        .bind(session_id)
        .bind(launch_generation_sql)
        .execute(&mut *tx)
        .await?;
        let pending_run_id = format!("pending:{session_id}:{launch_generation}");
        let snapshot = TagteamRunSnapshot::configured(&pending_run_id, Utc::now());
        let snapshot_json = serde_json::to_string(&snapshot)?;

        let result = sqlx::query(
            r#"
            INSERT INTO tagteam_run_bindings (
                session_id, launch_generation, schema_version, launch_spec_json,
                action_digest, launch_nonce, last_sequence, snapshot_json
            ) VALUES (?, ?, ?, ?, ?, ?, 0, ?)
            "#,
        )
        .bind(session_id)
        .bind(launch_generation_sql)
        .bind(i64::from(TAGTEAM_CONTRACT_VERSION))
        .bind(launch_spec_json)
        .bind(&action_digest)
        .bind(&launch_nonce)
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn attach_run(
        &self,
        session_id: &str,
        launch_generation: u64,
        producer_run_id: &str,
        run_dir: Option<PathBuf>,
        state_root: Option<PathBuf>,
    ) -> Result<BindingUpdate> {
        validate_producer_run_id(producer_run_id)?;
        let run_dir = canonical_attachment_path(run_dir, "run directory")?;
        let state_root = canonical_attachment_path(state_root, "state root")?;
        let launch_generation_sql = sqlite_integer(launch_generation, "launch generation")?;
        let pool = self.storage.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = get_in_transaction(&mut tx, session_id, launch_generation_sql)
            .await?
            .with_context(|| {
                format!(
                    "Tagteam run binding not found for session {session_id} generation {launch_generation}"
                )
            })?;
        if let Some(attached_run_id) = current.producer_run_id {
            tx.commit().await?;
            return if attached_run_id == producer_run_id
                && current.run_dir == run_dir
                && current.state_root == state_root
            {
                Ok(BindingUpdate::AlreadyAttached)
            } else {
                Ok(BindingUpdate::AttachmentConflict { attached_run_id })
            };
        }

        sqlx::query(
            r#"
            INSERT INTO tagteam_producer_run_ids(
                producer_run_id, session_id, launch_nonce
            ) VALUES (?, ?, ?)
            "#,
        )
        .bind(producer_run_id)
        .bind(session_id)
        .bind(&current.launch_nonce)
        .execute(&mut *tx)
        .await
        .context("producer run identity is already owned by another launch")?;

        let snapshot = TagteamRunSnapshot::configured(producer_run_id, Utc::now());
        let snapshot_json = serde_json::to_string(&snapshot)?;
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
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() != 1 {
            anyhow::bail!("Tagteam run binding changed while attaching producer run");
        }
        tx.commit().await?;
        Ok(BindingUpdate::Applied)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn apply_observation(
        &self,
        session_id: &str,
        launch_generation: u64,
        observation: TagteamObservation,
    ) -> Result<BindingUpdate> {
        let launch_generation_sql = sqlite_integer(launch_generation, "launch generation")?;
        let pool = self.storage.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let current = get_in_transaction(&mut tx, session_id, launch_generation_sql)
            .await?
            .with_context(|| {
                format!(
                    "Tagteam run binding not found for session {session_id} generation {launch_generation}"
                )
            })?;
        let producer_run_id = current
            .producer_run_id
            .as_deref()
            .context("cannot apply a Tagteam observation before attaching a producer run")?;
        validate_observation_scope(&observation, &current.launch_spec)?;
        let previous_sequence = current.last_sequence;
        let mut snapshot = current.snapshot.clone();
        match TagteamEventReducer::apply(&mut snapshot, observation) {
            Ok(ReduceOutcome::Applied) => {}
            Ok(ReduceOutcome::Duplicate) => {
                tx.commit().await?;
                return Ok(BindingUpdate::Duplicate {
                    current_sequence: previous_sequence,
                });
            }
            Err(ReducerError::StaleSequence { current, .. }) => {
                tx.commit().await?;
                return Ok(BindingUpdate::Stale {
                    current_sequence: current,
                });
            }
            Err(error) => return Err(error.into()),
        }

        let previous_sequence_sql = sqlite_integer(previous_sequence, "event sequence")?;
        let last_sequence = sqlite_integer(snapshot.last_sequence, "event sequence")?;
        let snapshot_json = serde_json::to_string(&snapshot)?;
        let terminal_class = snapshot
            .class
            .is_terminal()
            .then(|| serde_json::to_string(&snapshot.class))
            .transpose()?;
        let result = sqlx::query(
            r#"
            UPDATE tagteam_run_bindings
            SET last_sequence = ?, snapshot_json = ?, terminal_class = ?,
                updated_at = datetime('now')
            WHERE session_id = ? AND launch_generation = ? AND producer_run_id = ?
              AND last_sequence = ?
              AND (terminal_class IS NULL OR terminal_class = ?)
            "#,
        )
        .bind(last_sequence)
        .bind(snapshot_json)
        .bind(terminal_class.as_deref())
        .bind(session_id)
        .bind(launch_generation_sql)
        .bind(producer_run_id)
        .bind(previous_sequence_sql)
        .bind(terminal_class.as_deref())
        .execute(&mut *tx)
        .await?;
        if result.rows_affected() != 1 {
            anyhow::bail!("Tagteam snapshot compare-and-set invariant failed");
        }
        tx.commit().await?;
        Ok(BindingUpdate::Applied)
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
                   action_digest, launch_nonce, producer_run_id, run_dir, state_root, last_sequence,
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
                   action_digest, launch_nonce, producer_run_id, run_dir, state_root, last_sequence,
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

    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) async fn prune_completed(&self, session_id: &str, keep: usize) -> Result<u64> {
        let pool = self.storage.pool().await?;
        let result = if keep == 0 {
            sqlx::query(
                "DELETE FROM tagteam_run_bindings WHERE session_id = ? AND terminal_class IS NOT NULL",
            )
            .bind(session_id)
            .execute(pool)
            .await?
        } else {
            let keep =
                i64::try_from(keep).context("completed binding retention exceeds SQLite range")?;
            sqlx::query(
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
            .await?
        };
        Ok(result.rows_affected())
    }
}

async fn get_in_transaction(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    session_id: &str,
    launch_generation: i64,
) -> Result<Option<TagteamRunBinding>> {
    let row = sqlx::query(
        r#"
        SELECT session_id, launch_generation, schema_version, launch_spec_json,
               action_digest, launch_nonce, producer_run_id, run_dir, state_root, last_sequence,
               snapshot_json, terminal_class, created_at, updated_at
        FROM tagteam_run_bindings
        WHERE session_id = ? AND launch_generation = ?
        "#,
    )
    .bind(session_id)
    .bind(launch_generation)
    .fetch_optional(&mut **tx)
    .await?;
    row.map(row_to_binding).transpose()
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
    let binding = TagteamRunBinding {
        session_id: row.try_get("session_id")?,
        launch_generation,
        schema_version,
        launch_spec: serde_json::from_str(&row.try_get::<String, _>("launch_spec_json")?)?,
        action_digest: row.try_get("action_digest")?,
        launch_nonce: row.try_get("launch_nonce")?,
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
    };
    validate_binding(&binding)?;
    Ok(binding)
}

fn validate_binding(binding: &TagteamRunBinding) -> Result<()> {
    if binding.launch_generation == 0 {
        anyhow::bail!("stored Tagteam launch generation must be greater than zero");
    }
    if binding.launch_nonce.len() != 32
        || !binding
            .launch_nonce
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        anyhow::bail!("stored Tagteam launch nonce is malformed");
    }
    if binding.schema_version != TAGTEAM_CONTRACT_VERSION
        || binding.launch_spec.schema_version != TAGTEAM_CONTRACT_VERSION
        || binding.snapshot.schema_version != TAGTEAM_CONTRACT_VERSION
    {
        anyhow::bail!("stored Tagteam binding contains an unsupported schema version");
    }
    binding.launch_spec.validate()?;
    if binding.launch_spec.action_digest()? != binding.action_digest {
        anyhow::bail!("stored Tagteam action digest does not match its launch specification");
    }
    if binding.last_sequence != binding.snapshot.last_sequence {
        anyhow::bail!("stored Tagteam sequence does not match its snapshot");
    }
    validate_snapshot(&binding.snapshot)?;
    if let Some(producer_run_id) = &binding.producer_run_id {
        validate_producer_run_id(producer_run_id)?;
        if binding.snapshot.run_id != *producer_run_id {
            anyhow::bail!("stored Tagteam producer run identity does not match its snapshot");
        }
    } else {
        let expected_pending_run_id = format!(
            "pending:{}:{}",
            binding.session_id, binding.launch_generation
        );
        if binding.snapshot.run_id != expected_pending_run_id
            || binding.last_sequence != 0
            || binding.snapshot.class != RunClass::Configured
        {
            anyhow::bail!("stored pending Tagteam binding is inconsistent");
        }
    }
    validate_stored_attachment_path(binding.run_dir.as_ref(), "run directory")?;
    validate_stored_attachment_path(binding.state_root.as_ref(), "state root")?;
    let snapshot_terminal = binding
        .snapshot
        .class
        .is_terminal()
        .then_some(binding.snapshot.class);
    if binding.terminal_class != snapshot_terminal {
        anyhow::bail!("stored Tagteam terminal class does not match its snapshot");
    }
    Ok(())
}

fn validate_producer_run_id(producer_run_id: &str) -> Result<()> {
    if producer_run_id.is_empty()
        || producer_run_id.trim() != producer_run_id
        || producer_run_id.len() > MAX_IDENTIFIER_BYTES
        || producer_run_id.chars().any(char::is_control)
    {
        anyhow::bail!("producer run identity is malformed");
    }
    Ok(())
}

fn canonical_attachment_path(path: Option<PathBuf>, label: &str) -> Result<Option<PathBuf>> {
    path.map(|path| {
        validate_path_text(&path, label)?;
        let canonical = std::fs::canonicalize(&path)
            .with_context(|| format!("{label} does not resolve to an existing directory"))?;
        if !canonical.is_dir() {
            anyhow::bail!("{label} is not a directory");
        }
        Ok(canonical)
    })
    .transpose()
}

fn validate_stored_attachment_path(path: Option<&PathBuf>, label: &str) -> Result<()> {
    let Some(path) = path else {
        return Ok(());
    };
    validate_path_text(path, label)?;
    if !path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
    {
        anyhow::bail!("stored {label} is not an absolute normalized path");
    }
    Ok(())
}

fn validate_path_text(path: &std::path::Path, label: &str) -> Result<()> {
    let value = path
        .to_str()
        .with_context(|| format!("{label} must be valid UTF-8"))?;
    if value.is_empty() || value.len() > 4096 || value.chars().any(char::is_control) {
        anyhow::bail!("{label} is malformed");
    }
    Ok(())
}

fn validate_observation_scope(
    observation: &TagteamObservation,
    launch_spec: &TagteamLaunchSpecV1,
) -> Result<()> {
    let Some(diff) = &observation.diff else {
        return Ok(());
    };
    for changed_path in &diff.changed_paths {
        let in_scope = launch_spec.allowed_paths.iter().any(|allowed_path| {
            changed_path == allowed_path.as_str()
                || changed_path
                    .strip_prefix(allowed_path.as_str())
                    .is_some_and(|suffix| suffix.starts_with('/'))
        });
        if !in_scope {
            anyhow::bail!("observed changed path is outside the approved launch scope");
        }
    }
    Ok(())
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
    use crate::tagteam::Completeness;
    use tempfile::TempDir;

    fn spec(root: &std::path::Path) -> TagteamLaunchSpecV1 {
        assert!(std::process::Command::new("git")
            .args(["init", "-q"])
            .arg(root)
            .status()
            .unwrap()
            .success());
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

    fn observation(run_id: &str, sequence: u64, class: RunClass) -> TagteamObservation {
        TagteamObservation {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            run_id: run_id.to_string(),
            sequence,
            observed_at: Utc::now(),
            class,
            producer_status: format!("{class:?}").to_lowercase(),
            phase: None,
            role: None,
            reason_code: None,
            summary: None,
            diff: None,
            open_findings: Some(0),
            completeness: Completeness::Complete,
            authoritative_terminal: class.is_terminal(),
            scope_verified: class.is_terminal(),
        }
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
        let reloaded_first = store
            .get(&session_id, first.launch_generation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.idempotency_key(), reloaded_first.idempotency_key());
        assert_ne!(first.idempotency_key(), second.idempotency_key());
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
    async fn observation_updates_are_reduced_before_persistence() {
        let root = TempDir::new().unwrap();
        let manager = SessionManager::new(root.path().to_path_buf());
        let session_id = session(&manager, root.path()).await;
        let store = TagteamRunStore::new(&manager);
        let binding = store
            .create_binding(&session_id, spec(root.path()))
            .await
            .unwrap();
        let run_id = "producer-run-1";
        assert_eq!(
            store
                .attach_run(&session_id, binding.launch_generation, run_id, None, None)
                .await
                .unwrap(),
            BindingUpdate::Applied
        );
        let mut running = observation(run_id, 1, RunClass::Running);
        running.phase = Some("implementing".to_string());
        running.role = Some("worker".to_string());
        assert_eq!(
            store
                .apply_observation(&session_id, binding.launch_generation, running.clone())
                .await
                .unwrap(),
            BindingUpdate::Applied
        );
        assert_eq!(
            store
                .apply_observation(&session_id, binding.launch_generation, running)
                .await
                .unwrap(),
            BindingUpdate::Duplicate {
                current_sequence: 1
            }
        );
        assert_eq!(
            store
                .apply_observation(
                    &session_id,
                    binding.launch_generation,
                    observation(run_id, 0, RunClass::Configured),
                )
                .await
                .unwrap(),
            BindingUpdate::Stale {
                current_sequence: 1
            }
        );

        assert!(store
            .apply_observation(
                &session_id,
                binding.launch_generation,
                observation("different-run", 2, RunClass::Running),
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("does not match snapshot run"));
        assert!(store
            .apply_observation(
                &session_id,
                binding.launch_generation,
                observation(run_id, 3, RunClass::Testing),
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("skips expected sequence 2"));
        let reloaded = store
            .get(&session_id, binding.launch_generation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reloaded.last_sequence, 1);
        assert_eq!(reloaded.snapshot.class, RunClass::Running);
        assert!(store
            .get(&session_id, u64::MAX)
            .await
            .unwrap_err()
            .to_string()
            .contains("exceeds SQLite integer range"));
    }

    #[tokio::test]
    async fn attachment_is_idempotent_and_reports_conflicting_run() {
        let root = TempDir::new().unwrap();
        let manager = SessionManager::new(root.path().to_path_buf());
        let session_id = session(&manager, root.path()).await;
        let store = TagteamRunStore::new(&manager);
        let binding = store
            .create_binding(&session_id, spec(root.path()))
            .await
            .unwrap();

        assert_eq!(
            store
                .attach_run(
                    &session_id,
                    binding.launch_generation,
                    "producer-run-1",
                    None,
                    None,
                )
                .await
                .unwrap(),
            BindingUpdate::Applied
        );
        assert_eq!(
            store
                .attach_run(
                    &session_id,
                    binding.launch_generation,
                    "producer-run-1",
                    None,
                    None,
                )
                .await
                .unwrap(),
            BindingUpdate::AlreadyAttached
        );
        assert_eq!(
            store
                .attach_run(
                    &session_id,
                    binding.launch_generation,
                    "producer-run-1",
                    Some(root.path().to_path_buf()),
                    None,
                )
                .await
                .unwrap(),
            BindingUpdate::AttachmentConflict {
                attached_run_id: "producer-run-1".to_string(),
            }
        );
        assert_eq!(
            store
                .attach_run(
                    &session_id,
                    binding.launch_generation,
                    "producer-run-2",
                    None,
                    None,
                )
                .await
                .unwrap(),
            BindingUpdate::AttachmentConflict {
                attached_run_id: "producer-run-1".to_string(),
            }
        );
        assert!(store
            .attach_run(
                &session_id,
                binding.launch_generation,
                "producer\nforged",
                None,
                None,
            )
            .await
            .is_err());
    }

    #[tokio::test]
    async fn persisted_terminal_state_cannot_regress() {
        let root = TempDir::new().unwrap();
        let manager = SessionManager::new(root.path().to_path_buf());
        let session_id = session(&manager, root.path()).await;
        let store = TagteamRunStore::new(&manager);
        let binding = store
            .create_binding(&session_id, spec(root.path()))
            .await
            .unwrap();
        let run_id = "producer-terminal";
        store
            .attach_run(&session_id, binding.launch_generation, run_id, None, None)
            .await
            .unwrap();
        store
            .apply_observation(
                &session_id,
                binding.launch_generation,
                observation(run_id, 1, RunClass::Running),
            )
            .await
            .unwrap();
        store
            .apply_observation(
                &session_id,
                binding.launch_generation,
                observation(run_id, 2, RunClass::Passed),
            )
            .await
            .unwrap();

        assert!(store
            .apply_observation(
                &session_id,
                binding.launch_generation,
                observation(run_id, 3, RunClass::Running),
            )
            .await
            .unwrap_err()
            .to_string()
            .contains("cannot transition"));
        let reloaded = store
            .get(&session_id, binding.launch_generation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(reloaded.last_sequence, 2);
        assert_eq!(reloaded.snapshot.class, RunClass::Passed);
        assert_eq!(reloaded.terminal_class, Some(RunClass::Passed));
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
    async fn recreated_session_uses_a_new_durable_launch_identity() {
        let root = TempDir::new().unwrap();
        let manager = SessionManager::new(root.path().to_path_buf());
        let session_id = session(&manager, root.path()).await;
        let store = TagteamRunStore::new(&manager);
        let first = store
            .create_binding(&session_id, spec(root.path()))
            .await
            .unwrap();
        let first_key = first.idempotency_key();

        manager.delete_session(&session_id).await.unwrap();
        let recreated_session_id = session(&manager, root.path()).await;
        assert_eq!(recreated_session_id, session_id);
        let recreated = store
            .create_binding(&recreated_session_id, spec(root.path()))
            .await
            .unwrap();

        assert_eq!(recreated.launch_generation, 1);
        assert_ne!(recreated.launch_nonce, first.launch_nonce);
        assert_ne!(recreated.idempotency_key(), first_key);
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
            store
                .attach_run(&session_id, binding.launch_generation, &run_id, None, None)
                .await
                .unwrap();
            store
                .apply_observation(
                    &session_id,
                    binding.launch_generation,
                    observation(&run_id, 1, RunClass::Running),
                )
                .await
                .unwrap();
            store
                .apply_observation(
                    &session_id,
                    binding.launch_generation,
                    observation(&run_id, 2, RunClass::Passed),
                )
                .await
                .unwrap();
        }

        assert_eq!(store.prune_completed(&session_id, 1).await.unwrap(), 2);
        let latest = store.latest(&session_id).await.unwrap().unwrap();
        assert_eq!(latest.launch_generation, 3);
        assert_eq!(latest.terminal_class, Some(RunClass::Passed));
    }

    #[tokio::test]
    async fn pruning_all_completed_bindings_does_not_reuse_generation() {
        let root = TempDir::new().unwrap();
        let manager = SessionManager::new(root.path().to_path_buf());
        let session_id = session(&manager, root.path()).await;
        let store = TagteamRunStore::new(&manager);
        let first = store
            .create_binding(&session_id, spec(root.path()))
            .await
            .unwrap();
        let first_key = first.idempotency_key();
        let run_id = "producer-pruned";
        store
            .attach_run(&session_id, first.launch_generation, run_id, None, None)
            .await
            .unwrap();
        store
            .apply_observation(
                &session_id,
                first.launch_generation,
                observation(run_id, 1, RunClass::Running),
            )
            .await
            .unwrap();
        store
            .apply_observation(
                &session_id,
                first.launch_generation,
                observation(run_id, 2, RunClass::Passed),
            )
            .await
            .unwrap();

        assert_eq!(store.prune_completed(&session_id, 0).await.unwrap(), 1);
        assert!(store.latest(&session_id).await.unwrap().is_none());

        let second = store
            .create_binding(&session_id, spec(root.path()))
            .await
            .unwrap();
        assert_eq!(second.launch_generation, 2);
        assert_ne!(second.idempotency_key(), first_key);
        assert!(store
            .attach_run(&session_id, second.launch_generation, run_id, None, None,)
            .await
            .unwrap_err()
            .to_string()
            .contains("already owned"));
    }
}
