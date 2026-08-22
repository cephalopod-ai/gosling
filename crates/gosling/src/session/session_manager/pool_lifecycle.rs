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

impl SessionStorage {
    fn create_pool(path: &Path) -> Pool<Sqlite> {
        if let Some(parent) = path.parent() {
            // Don't panic here: this runs inside a process-global LazyLock, so
            // a panic would poison it and crash every later session access.
            // If the directory really is unusable the first query returns a
            // recoverable error instead.
            if let Err(e) = fs::create_dir_all(parent) {
                tracing::error!("Failed to create session database directory {parent:?}: {e}");
            }
            // sessions.db holds full conversation history (including
            // whatever secrets a tool call happened to echo back) and
            // SQLite creates it, its -wal, and its -shm sidecars with the
            // platform-default permissions (typically world-readable).
            // Restricting the directory itself to owner-only is sufficient
            // to keep every file SQLite creates in it unreachable by other
            // local users, without having to chase each one individually.
            #[cfg(unix)]
            if let Err(e) = fs::set_permissions(
                parent,
                <std::fs::Permissions as std::os::unix::fs::PermissionsExt>::from_mode(0o700),
            ) {
                tracing::error!(
                    "Failed to restrict session database directory {parent:?} to owner-only: {e}"
                );
            }
        }

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
        Self::create_schema(&storage.pool).await?;
        Ok(storage)
    }
}
