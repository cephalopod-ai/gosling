//! Session store construction and lazy first-use initialization: pool
//! creation with owner-only directory permissions, and the one-time
//! schema-create-or-migrate-then-legacy-import sequence gated behind a
//! `tokio::sync::OnceCell`.
//!
//! Extracted from `crate::session::session_manager` in a behavior-preserving
//! modularization (see `docs/logs/session/2026-08-22-modularize-session-manager.md`).
//! Orchestrates the schema (`schema.rs`), migrations (`migrations.rs`), and
//! legacy-import (`legacy_import.rs`) submodules extracted earlier in this
//! run — all reachable via `Self::` regardless of file, since those
//! functions are `pub(super)` (visible throughout `session_manager` and its
//! descendants, which includes this module). `new` and `create` stay `pub`
//! and `pool` stays `pub(crate)`, matching their pre-extraction visibility.

use super::{SessionStorage, DB_NAME, SESSIONS_FOLDER};
use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tracing::warn;

#[cfg(unix)]
fn prepare_session_directory_with<F>(path: &Path, set_permissions: F) -> std::io::Result<()>
where
    F: FnOnce(&Path, fs::Permissions) -> std::io::Result<()>,
{
    use std::os::unix::fs::PermissionsExt;

    fs::create_dir_all(path)?;
    set_permissions(path, fs::Permissions::from_mode(0o700))
}

fn prepare_session_directory(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        prepare_session_directory_with(path, |directory, permissions| {
            fs::set_permissions(directory, permissions)
        })
    }
    #[cfg(not(unix))]
    {
        fs::create_dir_all(path)
    }
}

impl SessionStorage {
    fn create_pool(path: &Path) -> Pool<Sqlite> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(30))
            .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
            // WAL + NORMAL is the standard pairing: commits skip the
            // per-commit WAL fsync (a large cost on macOS where fsync is
            // F_FULLFSYNC) while remaining corruption-safe; at most the last
            // commit is lost on power failure.
            .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

        SqlitePoolOptions::new().connect_lazy_with(options)
    }

    pub fn new(data_dir: PathBuf) -> Self {
        let session_dir = data_dir.join(SESSIONS_FOLDER);
        let db_path = session_dir.join(DB_NAME);
        Self {
            pool: Self::create_pool(&db_path),
            initialized: tokio::sync::OnceCell::new(),
            session_dir,
            owner_id: uuid::Uuid::new_v4().to_string(),
            active_tool_operations: std::sync::Mutex::new(HashSet::new()),
        }
    }

    pub(crate) async fn pool(&self) -> Result<&Pool<Sqlite>> {
        self.initialized
            .get_or_try_init(|| async {
                prepare_session_directory(&self.session_dir).map_err(|error| {
                    anyhow::anyhow!(
                        "cannot secure session database directory {:?}: {error}",
                        self.session_dir
                    )
                })?;
                // Propagate probe failures (e.g. SQLITE_BUSY past the timeout
                // while another process holds the write lock). Treating an
                // error as "no schema" would stamp an existing older DB with
                // the current version and permanently skip its migrations.
                let schema_exists = sqlx::query_scalar::<_, bool>(
                    r#"SELECT EXISTS (SELECT name FROM sqlite_master WHERE type='table' AND name='schema_version')"#,
                )
                .fetch_one(&self.pool)
                .await?;

                if schema_exists {
                    Self::run_migrations(&self.pool).await?;
                } else {
                    Self::create_schema(&self.pool).await?;
                }

                // Gated independently of schema creation/migration above: on a
                // brand-new database `legacy_import_status` starts unmarked, so
                // if the process is killed mid-import the next startup retries
                // it rather than treating the (already-committed) schema as
                // proof the import also finished. Databases that predate this
                // marker get it backfilled as already-complete by migration 21
                // (see `apply_migration`), so upgrading installs never get
                // silently re-imported.
                if !Self::legacy_import_completed(&self.pool).await? {
                    // The completion marker used to be written even when the
                    // import failed, so anything that did not make it across
                    // was never retried and the failure survived only as a log
                    // line (DAT-GSL-005). A failed import now leaves the
                    // marker unset so the next start tries again.
                    match Self::import_legacy(&self.pool, &self.session_dir).await {
                        Ok(()) => Self::mark_legacy_import_complete(&self.pool).await?,
                        Err(e) => warn!(
                            "Failed to import some legacy sessions; will retry on next start: {}",
                            e
                        ),
                    }
                }
                Ok::<(), anyhow::Error>(())
            })
            .await?;
        Ok(&self.pool)
    }

    pub async fn create(session_dir: &Path) -> Result<Self> {
        let storage = Self::new(session_dir.to_path_buf());
        storage.pool().await?;
        Ok(storage)
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn session_directory_permission_failure_is_returned() {
        let temp = tempfile::tempdir().unwrap();
        let error = prepare_session_directory_with(temp.path(), |_, _| {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "simulated chmod failure",
            ))
        })
        .unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::PermissionDenied);
    }
}
