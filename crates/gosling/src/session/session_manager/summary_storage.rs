//! Durable session summaries and extracted facts: the compaction store that
//! lets a resumed session present a compacted history instead of the full
//! transcript, plus the freshness check that decides whether a stored
//! summary may be presented as covering history before the loaded tail.
//!
//! Extracted from `crate::session::session_manager` in a behavior-preserving
//! modularization (see `docs/logs/session/2026-08-22-modularize-session-manager.md`).
//! `summary_covers_history_before` moves here from module level (its only
//! caller is `get_session_for_compacted_resume`, in this same module).
//! `pub(super)` on the five moved methods matches their pre-extraction
//! (private, same-module) visibility — the facade's `impl SessionManager`
//! delegates to all of them.

use super::{
    Session, SessionMessagePage, SessionStorage, SessionSummary, SessionSummaryFact,
    SessionSummaryStatus,
};
use crate::conversation::message::Message;
use crate::conversation::Conversation;
use anyhow::Result;
use chrono::{DateTime, Utc};

/// Whether a stored summary may be presented as covering the history that
/// precedes `page`.
///
/// Resume previously injected any non-empty summary, ignoring both its
/// `status` and how far it actually reached (DAT-GSL-001). `Stale` is the
/// `Default` for the status column, so a summary written before a schema
/// or worker change reads as stale and must not be presented as fact. The
/// row-id check closes the more damaging case: if the session grew after
/// the summary was written, the messages between `covered_through_row_id`
/// and the tail's oldest row are in neither the summary nor the tail, and
/// injecting the summary would silently claim continuous coverage over
/// that gap.
pub(super) fn summary_covers_history_before(
    summary: &SessionSummary,
    page: &SessionMessagePage,
) -> bool {
    if summary.status != SessionSummaryStatus::Current {
        return false;
    }
    match page.oldest_row_id {
        // The summary must reach the row immediately before the tail.
        Some(oldest) => summary.covered_through_row_id >= oldest - 1,
        // Empty tail: nothing can be uncovered between the two.
        None => true,
    }
}

impl SessionStorage {
    pub(super) async fn get_session_summary(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionSummary>> {
        let row = sqlx::query_as::<_, (String, String, i64, i64, i64, String, Option<String>, String, Option<String>, DateTime<Utc>)>(
            "SELECT session_id, summary, covered_through_row_id, covered_through_timestamp, covered_message_count, source_hash, summarizer_model, status, error, updated_at FROM session_summaries WHERE session_id = ?",
        )
        .bind(session_id)
        .fetch_optional(self.pool().await?)
        .await?;

        Ok(row.map(
            |(
                session_id,
                summary,
                covered_through_row_id,
                covered_through_timestamp,
                covered_message_count,
                source_hash,
                summarizer_model,
                status,
                error,
                updated_at,
            )| SessionSummary {
                session_id,
                summary,
                covered_through_row_id,
                covered_through_timestamp,
                covered_message_count: covered_message_count as usize,
                source_hash,
                summarizer_model,
                status: status.parse().unwrap_or(SessionSummaryStatus::Stale),
                error,
                updated_at,
            },
        ))
    }

    pub(super) async fn get_session_summary_facts(
        &self,
        session_id: &str,
    ) -> Result<Vec<SessionSummaryFact>> {
        let rows = sqlx::query_as::<_, (i64, String, Option<String>, String, String, String, String, f32, Option<i64>, Option<i64>, DateTime<Utc>)>(
            "SELECT id, session_id, project_id, working_dir, scope, fact_type, content, confidence, source_start_row_id, source_end_row_id, created_at FROM session_summary_facts WHERE session_id = ? ORDER BY id",
        )
        .bind(session_id)
        .fetch_all(self.pool().await?)
        .await?;

        Ok(rows
            .into_iter()
            .map(
                |(
                    id,
                    session_id,
                    project_id,
                    working_dir,
                    scope,
                    fact_type,
                    content,
                    confidence,
                    source_start_row_id,
                    source_end_row_id,
                    created_at,
                )| SessionSummaryFact {
                    id,
                    session_id,
                    project_id,
                    working_dir,
                    scope,
                    fact_type,
                    content,
                    confidence,
                    source_start_row_id,
                    source_end_row_id,
                    created_at,
                },
            )
            .collect())
    }

    pub(super) async fn upsert_session_summary(&self, summary: &SessionSummary) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO session_summaries (
                session_id, summary, covered_through_row_id, covered_through_timestamp,
                covered_message_count, source_hash, summarizer_model, status, error, updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(session_id) DO UPDATE SET
                summary = excluded.summary,
                covered_through_row_id = excluded.covered_through_row_id,
                covered_through_timestamp = excluded.covered_through_timestamp,
                covered_message_count = excluded.covered_message_count,
                source_hash = excluded.source_hash,
                summarizer_model = excluded.summarizer_model,
                status = excluded.status,
                error = excluded.error,
                updated_at = excluded.updated_at
            "#,
        )
        .bind(&summary.session_id)
        .bind(&summary.summary)
        .bind(summary.covered_through_row_id)
        .bind(summary.covered_through_timestamp)
        .bind(summary.covered_message_count as i64)
        .bind(&summary.source_hash)
        .bind(&summary.summarizer_model)
        .bind(summary.status.to_string())
        .bind(&summary.error)
        .bind(summary.updated_at)
        .execute(self.pool().await?)
        .await?;
        Ok(())
    }

    pub(super) async fn replace_session_summary_facts(
        &self,
        session_id: &str,
        facts: &[SessionSummaryFact],
    ) -> Result<()> {
        let _write_guard = self.acquire_write_guard().await;
        let pool = self.pool().await?;
        let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
        sqlx::query("DELETE FROM session_summary_facts WHERE session_id = ?")
            .bind(session_id)
            .execute(&mut *tx)
            .await?;
        for fact in facts {
            sqlx::query(
                r#"
                INSERT INTO session_summary_facts (
                    session_id, project_id, working_dir, scope, fact_type, content, confidence,
                    source_start_row_id, source_end_row_id, created_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                "#,
            )
            .bind(session_id)
            .bind(&fact.project_id)
            .bind(&fact.working_dir)
            .bind(&fact.scope)
            .bind(&fact.fact_type)
            .bind(&fact.content)
            .bind(fact.confidence)
            .bind(fact.source_start_row_id)
            .bind(fact.source_end_row_id)
            .bind(fact.created_at)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    pub(super) async fn get_session_for_compacted_resume(
        &self,
        session_id: &str,
        tail_limit: usize,
    ) -> Result<Session> {
        let mut session = self.get_session(session_id, false).await?;
        let page = self.get_session_tail_page(session_id, tail_limit).await?;
        let mut messages = Vec::new();
        let summary = self
            .get_session_summary(session_id)
            .await?
            .filter(|summary| summary_covers_history_before(summary, &page));
        if let Some(summary) = summary {
            if !summary.summary.trim().is_empty() {
                messages.push(
                    Message::user()
                        .with_text(format!(
                            "[Compacted summary of {} earlier message(s)]: {}",
                            summary.covered_message_count, summary.summary
                        ))
                        .with_visibility(false, true),
                );
            }
        } else if page.total_count > page.messages.len() {
            messages.push(
                Message::user()
                    .with_text(format!(
                        "[Older session history exists: {} message(s) before the loaded tail. No durable summary is available yet.]",
                        page.total_count.saturating_sub(page.messages.len())
                    ))
                    .with_visibility(false, true),
            );
        }
        messages.extend(page.messages);
        session.conversation = Some(Conversation::new_unvalidated(messages));
        session.message_count = page.total_count;
        Ok(session)
    }
}
