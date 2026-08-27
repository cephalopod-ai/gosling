//! Session-library items: least-privilege pasted text/image/file inputs
//! scoped to a session or shared across a project, keyed by workspace,
//! project id, or a hash of the working directory when neither is set.
//!
//! Extracted from `crate::session::session_manager` in a behavior-preserving
//! modularization (see `docs/logs/session/2026-08-22-modularize-session-manager.md`).
//! `pub(super)` matches these methods' pre-extraction (private, same-module)
//! visibility — the facade's `impl SessionManager` delegates to all five.

use super::SessionStorage;
use crate::session::library::{
    NewSessionLibraryContent, SessionLibraryItem, SessionLibraryItemKind, SessionLibraryScope,
};
use anyhow::Result;
use base64::Engine as _;
use chrono::{DateTime, Utc};
use sha2::{Digest, Sha256};
use std::fs;

impl SessionStorage {
    pub(super) async fn session_library_scope_keys(
        &self,
        session_id: &str,
    ) -> Result<(String, String)> {
        let (project_id, workspace_id, working_dir) =
            sqlx::query_as::<_, (Option<String>, Option<String>, String)>(
                "SELECT project_id, workspace_id, working_dir FROM sessions WHERE id = ?",
            )
            .bind(session_id)
            .fetch_one(self.pool().await?)
            .await?;
        let project_key = if let Some(project_id) = project_id.filter(|value| !value.is_empty()) {
            format!("project:{project_id}")
        } else if let Some(workspace_id) = workspace_id.filter(|value| !value.is_empty()) {
            format!("workspace:{workspace_id}")
        } else {
            let digest = Sha256::digest(working_dir.as_bytes());
            format!("directory:{}", crate::utils::bytes_to_hex(digest))
        };
        Ok((format!("session:{session_id}"), project_key))
    }

    pub(super) async fn list_session_library_items(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionLibraryItem>> {
        let (session_key, project_key) = self.session_library_scope_keys(session_id).await?;
        let rows = sqlx::query_as::<_, SessionLibraryItemRow>(
            r#"
            SELECT id, scope, name, kind, mime_type, size_bytes, text_content, image_data,
                   file_path, created_at
            FROM session_library_items
            WHERE (scope = 'session' AND scope_key = ?)
               OR (scope = 'project' AND scope_key = ?)
            ORDER BY created_at DESC, id DESC
            LIMIT 128
            "#,
        )
        .bind(session_key)
        .bind(project_key)
        .fetch_all(self.pool().await?)
        .await?;
        rows.into_iter()
            .map(session_library_item_from_row)
            .collect()
    }

    pub(super) async fn add_session_library_item(
        &self,
        session_id: &str,
        scope: SessionLibraryScope,
        name: String,
        content: NewSessionLibraryContent,
    ) -> Result<SessionLibraryItem> {
        let (session_key, project_key) = self.session_library_scope_keys(session_id).await?;
        let scope_key = match scope {
            SessionLibraryScope::Project => project_key,
            SessionLibraryScope::Session => session_key,
        };
        let (kind, mime_type, size_bytes, text_content, image_data, file_path) = match content {
            NewSessionLibraryContent::Text(text) => (
                SessionLibraryItemKind::Text,
                "text/plain".to_string(),
                text.len(),
                Some(text),
                None,
                None,
            ),
            NewSessionLibraryContent::Image { data, mime_type } => {
                let size_bytes = base64::engine::general_purpose::STANDARD
                    .decode(&data)?
                    .len();
                (
                    SessionLibraryItemKind::Image,
                    mime_type,
                    size_bytes,
                    None,
                    Some(data),
                    None,
                )
            }
            NewSessionLibraryContent::File { path, mime_type } => {
                let size_bytes = fs::metadata(&path)?.len() as usize;
                (
                    SessionLibraryItemKind::File,
                    mime_type,
                    size_bytes,
                    None,
                    None,
                    Some(path),
                )
            }
        };
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM session_library_items WHERE scope = ? AND scope_key = ?",
        )
        .bind(scope.to_string())
        .bind(&scope_key)
        .fetch_one(&mut *tx)
        .await?;
        anyhow::ensure!(count < 64, "library scope is full");
        let id = format!("lib_{}", uuid::Uuid::new_v4());
        sqlx::query(
            r#"
            INSERT INTO session_library_items (
                id, scope, scope_key, name, kind, mime_type, size_bytes,
                text_content, image_data, file_path
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(scope.to_string())
        .bind(scope_key)
        .bind(&name)
        .bind(kind.to_string())
        .bind(&mime_type)
        .bind(size_bytes as i64)
        .bind(&text_content)
        .bind(&image_data)
        .bind(&file_path)
        .execute(&mut *tx)
        .await?;
        let row = sqlx::query_as::<_, SessionLibraryItemRow>(
            r#"
            SELECT id, scope, name, kind, mime_type, size_bytes, text_content, image_data,
                   file_path, created_at
            FROM session_library_items WHERE id = ?
            "#,
        )
        .bind(&id)
        .fetch_one(&mut *tx)
        .await?;
        tx.commit().await?;
        session_library_item_from_row(row)
    }

    pub(super) async fn remove_session_library_item(
        &self,
        session_id: &str,
        item_id: &str,
    ) -> Result<bool> {
        let (session_key, project_key) = self.session_library_scope_keys(session_id).await?;
        let result = sqlx::query(
            r#"
            DELETE FROM session_library_items
            WHERE id = ? AND (
                (scope = 'session' AND scope_key = ?)
                OR (scope = 'project' AND scope_key = ?)
            )
            "#,
        )
        .bind(item_id)
        .bind(session_key)
        .bind(project_key)
        .execute(self.pool().await?)
        .await?;
        Ok(result.rows_affected() == 1)
    }

    pub(super) async fn get_session_library_items(
        &self,
        session_id: &str,
        item_ids: &[String],
    ) -> Result<Vec<SessionLibraryItem>> {
        let (session_key, project_key) = self.session_library_scope_keys(session_id).await?;
        let pool = self.pool().await?;
        let mut items = Vec::with_capacity(item_ids.len());
        for item_id in item_ids {
            let row = sqlx::query_as::<_, SessionLibraryItemRow>(
                r#"
                SELECT id, scope, name, kind, mime_type, size_bytes, text_content, image_data,
                       file_path, created_at
                FROM session_library_items
                WHERE id = ? AND (
                    (scope = 'session' AND scope_key = ?)
                    OR (scope = 'project' AND scope_key = ?)
                )
                "#,
            )
            .bind(item_id)
            .bind(&session_key)
            .bind(&project_key)
            .fetch_optional(pool)
            .await?;
            let row = row.ok_or_else(|| anyhow::anyhow!("library item is unavailable"))?;
            items.push(session_library_item_from_row(row)?);
        }
        Ok(items)
    }
}

type SessionLibraryItemRow = (
    String,
    String,
    String,
    String,
    String,
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    DateTime<Utc>,
);

fn session_library_item_from_row(row: SessionLibraryItemRow) -> Result<SessionLibraryItem> {
    Ok(SessionLibraryItem {
        id: row.0,
        scope: row.1.parse()?,
        name: row.2,
        kind: row.3.parse()?,
        mime_type: row.4,
        size_bytes: usize::try_from(row.5)?,
        text_content: row.6,
        image_data: row.7,
        file_path: row.8,
        created_at: row.9,
    })
}
