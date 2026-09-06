//! Session artifact discovery and the artifact inventory store: scanning
//! successful tool results and assistant markdown for referenced files, and
//! the deduplicating upsert/list/get surface over `session_artifacts`.
//!
//! Extracted from `crate::session::session_manager` in a behavior-preserving
//! modularization (see `docs/logs/session/2026-08-22-modularize-session-manager.md`).
//! `pub(super)` matches these methods' pre-extraction (private, same-module)
//! visibility — the facade's `impl SessionManager` delegates to them, and
//! `upsert_artifacts_in_tx` is also called from the schema submodule's
//! `backfill_session_artifacts` (both are `session_manager` descendants, so
//! `pub(super)` covers that call too).

use super::{SessionArtifactPage, SessionStorage};
use crate::conversation::message::{Message, MessageContent};
use crate::session::artifacts::{
    assistant_reference_bases, discover_from_assistant_markdown, discover_from_successful_tool,
    DiscoveredArtifact, SessionArtifact, SessionArtifactProvenance,
};
use anyhow::Result;
use chrono::{DateTime, Utc};
use rmcp::model::Role;
use sqlx::Sqlite;
use std::path::Path;

impl SessionStorage {
    pub(super) async fn discover_message_artifacts_in_tx(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        session_id: &str,
        message: &Message,
    ) -> Result<()> {
        let (working_dir, workspace_id) = sqlx::query_as::<_, (String, Option<String>)>(
            "SELECT working_dir, workspace_id FROM sessions WHERE id = ?",
        )
        .bind(session_id)
        .fetch_one(&mut **tx)
        .await?;

        let source_id = message.id.as_deref();
        let mut artifacts = Vec::new();
        for content in &message.content {
            if let MessageContent::ToolResponse(response) = content {
                let request_json = sqlx::query_scalar::<_, String>(
                    r#"
                        SELECT request.value
                        FROM messages, json_each(messages.content_json) AS request
                        WHERE messages.session_id = ?
                          AND json_extract(request.value, '$.type') = 'toolRequest'
                          AND json_extract(request.value, '$.id') = ?
                        ORDER BY messages.id DESC
                        LIMIT 1
                    "#,
                )
                .bind(session_id)
                .bind(&response.id)
                .fetch_optional(&mut **tx)
                .await?;
                if let (Some(request_json), Ok(result)) =
                    (request_json, response.tool_result.as_ref())
                {
                    let MessageContent::ToolRequest(request) = serde_json::from_str(&request_json)?
                    else {
                        continue;
                    };
                    if let Ok(tool_call) = request.tool_call.as_ref() {
                        artifacts.extend(discover_from_successful_tool(
                            tool_call,
                            result,
                            Path::new(&working_dir),
                            workspace_id.as_deref(),
                            source_id.or(Some(response.id.as_str())),
                        ));
                    }
                }
            }
        }
        Self::upsert_artifacts_in_tx(tx, session_id, &artifacts).await?;
        Ok(())
    }

    pub(super) async fn register_completed_assistant_artifacts(
        &self,
        session_id: &str,
        message: &Message,
    ) -> Result<()> {
        if message.role != Role::Assistant || message.metadata.imported_untrusted {
            return Ok(());
        }
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        let (working_dir, additional_dirs_json, extension_data_json, workspace_id) =
            sqlx::query_as::<_, (String, String, String, Option<String>)>(
                "SELECT working_dir, additional_working_dirs_json, extension_data, workspace_id FROM sessions WHERE id = ?",
            )
            .bind(session_id)
            .fetch_one(&mut *tx)
            .await?;
        let additional_dirs = assistant_reference_bases(
            serde_json::from_str(&additional_dirs_json).unwrap_or_default(),
            &serde_json::from_str(&extension_data_json).unwrap_or_default(),
        );
        let artifacts = message
            .content
            .iter()
            .filter_map(|content| match content {
                MessageContent::Text(text) => Some(discover_from_assistant_markdown(
                    &text.text,
                    Path::new(&working_dir),
                    &additional_dirs,
                    workspace_id.as_deref(),
                    message.id.as_deref(),
                    SessionArtifactProvenance::AssistantMessage,
                )),
                _ => None,
            })
            .flatten()
            .collect::<Vec<_>>();
        Self::upsert_artifacts_in_tx(&mut tx, session_id, &artifacts).await?;
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn upsert_artifacts_in_tx(
        tx: &mut sqlx::Transaction<'_, Sqlite>,
        session_id: &str,
        artifacts: &[DiscoveredArtifact],
    ) -> Result<()> {
        for artifact in artifacts {
            sqlx::query(
                r#"
                INSERT INTO session_artifacts (
                    session_id, display_path, resolved_path, base_working_dir, workspace_id,
                    mime_type, relation, provenance, source_id
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(session_id, resolved_path) DO UPDATE SET
                    display_path = CASE
                        WHEN instr('built_in_tool,mcp_resource_link,tool_metadata,tool_argument,assistant_message,compatibility_inference', excluded.provenance)
                           < instr('built_in_tool,mcp_resource_link,tool_metadata,tool_argument,assistant_message,compatibility_inference', session_artifacts.provenance)
                        THEN excluded.display_path ELSE session_artifacts.display_path END,
                    base_working_dir = CASE
                        WHEN instr('built_in_tool,mcp_resource_link,tool_metadata,tool_argument,assistant_message,compatibility_inference', excluded.provenance)
                           < instr('built_in_tool,mcp_resource_link,tool_metadata,tool_argument,assistant_message,compatibility_inference', session_artifacts.provenance)
                        THEN excluded.base_working_dir ELSE session_artifacts.base_working_dir END,
                    workspace_id = COALESCE(excluded.workspace_id, session_artifacts.workspace_id),
                    mime_type = COALESCE(excluded.mime_type, session_artifacts.mime_type),
                    relation = CASE
                        WHEN instr('created,modified,referenced', excluded.relation)
                           < instr('created,modified,referenced', session_artifacts.relation)
                        THEN excluded.relation ELSE session_artifacts.relation END,
                    provenance = CASE
                        WHEN instr('built_in_tool,mcp_resource_link,tool_metadata,tool_argument,assistant_message,compatibility_inference', excluded.provenance)
                           < instr('built_in_tool,mcp_resource_link,tool_metadata,tool_argument,assistant_message,compatibility_inference', session_artifacts.provenance)
                        THEN excluded.provenance ELSE session_artifacts.provenance END,
                    source_id = CASE
                        WHEN instr('built_in_tool,mcp_resource_link,tool_metadata,tool_argument,assistant_message,compatibility_inference', excluded.provenance)
                           < instr('built_in_tool,mcp_resource_link,tool_metadata,tool_argument,assistant_message,compatibility_inference', session_artifacts.provenance)
                        THEN COALESCE(excluded.source_id, session_artifacts.source_id)
                        ELSE COALESCE(session_artifacts.source_id, excluded.source_id) END,
                    last_seen_at = CURRENT_TIMESTAMP
                "#,
            )
            .bind(session_id)
            .bind(&artifact.display_path)
            .bind(&artifact.resolved_path)
            .bind(&artifact.base_working_dir)
            .bind(&artifact.workspace_id)
            .bind(&artifact.mime_type)
            .bind(artifact.relation.to_string())
            .bind(artifact.provenance.to_string())
            .bind(&artifact.source_id)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }

    pub(super) async fn upsert_session_artifacts(
        &self,
        session_id: &str,
        artifacts: &[DiscoveredArtifact],
    ) -> Result<Vec<SessionArtifact>> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        Self::upsert_artifacts_in_tx(&mut tx, session_id, artifacts).await?;
        tx.commit().await?;

        let mut stored = Vec::with_capacity(artifacts.len());
        for artifact in artifacts {
            if let Some(value) = self
                .get_session_artifact(session_id, &artifact.resolved_path)
                .await?
            {
                stored.push(value);
            }
        }
        Ok(stored)
    }

    async fn get_session_artifact(
        &self,
        session_id: &str,
        resolved_path: &str,
    ) -> Result<Option<SessionArtifact>> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                Option<String>,
                Option<String>,
                String,
                String,
                Option<String>,
                DateTime<Utc>,
                DateTime<Utc>,
            ),
        >(
            r#"
            SELECT session_id, display_path, resolved_path, base_working_dir, workspace_id,
                   mime_type, relation, provenance, source_id, first_seen_at, last_seen_at
            FROM session_artifacts WHERE session_id = ? AND resolved_path = ?
            "#,
        )
        .bind(session_id)
        .bind(resolved_path)
        .fetch_optional(self.pool().await?)
        .await?;
        row.map(session_artifact_from_row).transpose()
    }

    pub(super) async fn list_session_artifacts(
        &self,
        session_id: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<SessionArtifactPage> {
        let pool = self.pool().await?;
        let limit = limit.clamp(1, 200);
        let before_id = cursor.map(str::parse::<i64>).transpose()?;
        let total_count = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM session_artifacts WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_one(pool)
        .await? as usize;
        let rows = if let Some(before_id) = before_id {
            sqlx::query_as::<
                _,
                (
                    i64,
                    String,
                    String,
                    String,
                    String,
                    Option<String>,
                    Option<String>,
                    String,
                    String,
                    Option<String>,
                    DateTime<Utc>,
                    DateTime<Utc>,
                ),
            >(
                r#"
                SELECT id, session_id, display_path, resolved_path, base_working_dir, workspace_id,
                       mime_type, relation, provenance, source_id, first_seen_at, last_seen_at
                FROM session_artifacts
                WHERE session_id = ? AND id < ? ORDER BY id DESC LIMIT ?
                "#,
            )
            .bind(session_id)
            .bind(before_id)
            .bind((limit + 1) as i64)
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<
                _,
                (
                    i64,
                    String,
                    String,
                    String,
                    String,
                    Option<String>,
                    Option<String>,
                    String,
                    String,
                    Option<String>,
                    DateTime<Utc>,
                    DateTime<Utc>,
                ),
            >(
                r#"
                SELECT id, session_id, display_path, resolved_path, base_working_dir, workspace_id,
                       mime_type, relation, provenance, source_id, first_seen_at, last_seen_at
                FROM session_artifacts
                WHERE session_id = ? ORDER BY id DESC LIMIT ?
                "#,
            )
            .bind(session_id)
            .bind((limit + 1) as i64)
            .fetch_all(pool)
            .await?
        };
        let has_more = rows.len() > limit;
        let page_rows = rows.into_iter().take(limit).collect::<Vec<_>>();
        let next_cursor = has_more
            .then(|| page_rows.last().map(|row| row.0.to_string()))
            .flatten();
        let artifacts = page_rows
            .into_iter()
            .map(|row| {
                session_artifact_from_row((
                    row.1, row.2, row.3, row.4, row.5, row.6, row.7, row.8, row.9, row.10, row.11,
                ))
            })
            .collect::<Result<Vec<_>>>()?;
        Ok(SessionArtifactPage {
            artifacts,
            next_cursor,
            total_count,
        })
    }
}

type SessionArtifactRow = (
    String,
    String,
    String,
    String,
    Option<String>,
    Option<String>,
    String,
    String,
    Option<String>,
    DateTime<Utc>,
    DateTime<Utc>,
);

fn session_artifact_from_row(row: SessionArtifactRow) -> Result<SessionArtifact> {
    Ok(SessionArtifact {
        session_id: row.0,
        display_path: row.1,
        resolved_path: row.2,
        base_working_dir: row.3,
        workspace_id: row.4,
        mime_type: row.5,
        relation: row.6.parse()?,
        provenance: row.7.parse()?,
        source_id: row.8,
        first_seen_at: row.9,
        last_seen_at: row.10,
    })
}
