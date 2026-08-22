//! One-time import of pre-database, on-disk legacy `.jsonl` sessions into
//! `sessions.db`, plus the completion marker that makes the import retry
//! safely if interrupted and never replay against an already-imported store.
//!
//! Extracted from `crate::session::session_manager` in a behavior-preserving
//! modularization (see `docs/logs/session/2026-08-22-modularize-session-manager.md`).
//! `legacy_import_completed` and `import_legacy` are `pub(super)`: the
//! facade's pool-init path (`SessionStorage::pool`) calls them directly.
//! `mark_legacy_import_complete` and `import_legacy_session` are only
//! called from within this module.

use super::{Session, SessionStorage};
use anyhow::Result;
use sqlx::{Pool, Sqlite};
use std::path::PathBuf;
use tracing::{info, warn};

impl SessionStorage {
    pub(super) async fn legacy_import_completed(pool: &Pool<Sqlite>) -> Result<bool> {
        let completed = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS (SELECT 1 FROM legacy_import_status WHERE id = 1)",
        )
        .fetch_one(pool)
        .await?;
        Ok(completed)
    }

    pub(super) async fn mark_legacy_import_complete(pool: &Pool<Sqlite>) -> Result<()> {
        sqlx::query(
            "INSERT OR IGNORE INTO legacy_import_status (id, completed_at) VALUES (1, CURRENT_TIMESTAMP)",
        )
        .execute(pool)
        .await?;
        Ok(())
    }

    pub(super) async fn import_legacy(pool: &Pool<Sqlite>, session_dir: &PathBuf) -> Result<()> {
        use crate::session::legacy;

        let sessions = match legacy::list_sessions(session_dir) {
            Ok(sessions) => sessions,
            Err(_) => {
                warn!("No legacy sessions found to import");
                return Ok(());
            }
        };

        if sessions.is_empty() {
            return Ok(());
        }

        let mut imported_count = 0;
        let mut failed_count = 0;

        for (session_name, session_path) in sessions {
            match legacy::load_session(&session_name, &session_path) {
                Ok(session) => match Self::import_legacy_session(pool, &session).await {
                    Ok(_) => {
                        imported_count += 1;
                        info!("  ✓ Imported: {}", session_name);
                    }
                    Err(e) => {
                        failed_count += 1;
                        info!("  ✗ Failed to import {}: {}", session_name, e);
                    }
                },
                Err(e) => {
                    failed_count += 1;
                    info!("  ✗ Failed to load {}: {}", session_name, e);
                }
            }
        }

        info!(
            "Import complete: {} successful, {} failed",
            imported_count, failed_count
        );
        Ok(())
    }

    async fn import_legacy_session(pool: &Pool<Sqlite>, session: &Session) -> Result<()> {
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;

        let model_config_json = match &session.model_config {
            Some(model_config) => Some(serde_json::to_string(model_config)?),
            None => None,
        };

        sqlx::query(
            r#"
        INSERT INTO sessions (
            id, name, user_set_name, session_type, working_dir, created_at, updated_at, extension_data,
            total_tokens, input_tokens, output_tokens,
            cache_read_tokens, cache_write_tokens,
            accumulated_total_tokens, accumulated_input_tokens, accumulated_output_tokens,
            accumulated_cache_read_tokens, accumulated_cache_write_tokens,
            accumulated_cost,
            provider_name, model_config_json, gosling_mode
        ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        "#,
        )
        .bind(&session.id)
        .bind(&session.name)
        .bind(session.user_set_name)
        .bind(session.session_type.to_string())
        .bind(&*session.working_dir.to_string_lossy())
        .bind(session.created_at)
        .bind(session.updated_at)
        .bind(serde_json::to_string(&session.extension_data)?)
        .bind(session.usage.total_tokens)
        .bind(session.usage.input_tokens)
        .bind(session.usage.output_tokens)
        .bind(session.usage.cache_read_input_tokens)
        .bind(session.usage.cache_write_input_tokens)
        .bind(session.accumulated_usage.total_tokens)
        .bind(session.accumulated_usage.input_tokens)
        .bind(session.accumulated_usage.output_tokens)
        .bind(session.accumulated_usage.cache_read_input_tokens)
        .bind(session.accumulated_usage.cache_write_input_tokens)
        .bind(session.accumulated_cost)
        .bind(&session.provider_name)
        .bind(model_config_json)
        .bind(session.gosling_mode.to_string())
        .execute(&mut *tx)
        .await?;

        // The session row and its messages commit together so a crash
        // mid-import can never leave a session that looks imported (row
        // present) but is missing its conversation. On retry, the `INSERT
        // INTO sessions` above fails fast on the id's PRIMARY KEY for any
        // session that already fully committed, which safely skips
        // re-importing it instead of clobbering conversation history it may
        // have accumulated since.
        if let Some(conversation) = &session.conversation {
            Self::replace_conversation_in_tx(&mut tx, &session.id, conversation).await?;
        }

        tx.commit().await?;
        Ok(())
    }
}
