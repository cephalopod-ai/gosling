use crate::session::session_manager::SessionType;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Pool, Sqlite};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize)]
pub struct ChatRecallResult {
    pub session_id: String,
    pub session_description: String,
    pub session_working_dir: String,
    pub last_activity: DateTime<Utc>,
    pub total_messages_in_session: usize,
    pub messages: Vec<ChatRecallMessage>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatRecallMessage {
    /// Stable row identity lets callers hydrate context around this exact hit
    /// instead of loading an entire transcript.
    pub row_id: i64,
    pub message_id: Option<String>,
    pub role: String,
    /// A bounded FTS snippet. Full message content is deliberately loaded only
    /// by the targeted context API.
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ChatRecallResults {
    pub results: Vec<ChatRecallResult>,
    pub total_matches: usize,
}

type SqlQueryRow = (
    String,
    String,
    String,
    i64,
    i64,
    Option<String>,
    String,
    String,
    DateTime<Utc>,
);

type SessionMessageGroup = (String, String, usize, Vec<ChatRecallMessage>);

/// Searches the durable FTS projection maintained by the session store.
///
/// The projection contains only text blocks; tool calls, tool responses, and
/// thinking are intentionally omitted. That keeps recall results useful and
/// makes the hot query proportional to matching terms rather than to every
/// JSON message in the database.
pub struct ChatHistorySearch<'a> {
    pool: &'a Pool<Sqlite>,
    query: &'a str,
    limit: usize,
    after_date: Option<DateTime<Utc>>,
    before_date: Option<DateTime<Utc>>,
    exclude_session_id: Option<String>,
    session_types: Vec<SessionType>,
}

impl<'a> ChatHistorySearch<'a> {
    pub fn new(
        pool: &'a Pool<Sqlite>,
        query: &'a str,
        limit: Option<usize>,
        after_date: Option<DateTime<Utc>>,
        before_date: Option<DateTime<Utc>>,
        exclude_session_id: Option<String>,
        session_types: Vec<SessionType>,
    ) -> Self {
        Self {
            pool,
            query,
            limit: limit.unwrap_or(10).clamp(1, 50),
            after_date,
            before_date,
            exclude_session_id,
            session_types,
        }
    }

    pub async fn execute(self) -> Result<ChatRecallResults> {
        let Some(fts_query) = self.fts_query() else {
            return Ok(ChatRecallResults {
                results: vec![],
                total_matches: 0,
            });
        };

        let rows = self.fetch_rows(&fts_query).await?;
        let session_messages = self.process_rows(rows);
        let results = self.convert_to_results(session_messages);
        Ok(results)
    }

    async fn fetch_rows(&self, fts_query: &str) -> Result<Vec<SqlQueryRow>> {
        let mut sql = String::from(
            r#"
            WITH matched_messages AS MATERIALIZED (
              SELECT
                s.id AS session_id,
                COALESCE(NULLIF(s.description, ''), s.name) AS session_description,
                s.working_dir AS session_working_dir,
                m.id AS row_id,
                m.message_id,
                m.role,
                snippet(message_search, 0, '[', ']', '…', 32) AS snippet,
                m.timestamp,
                bm25(message_search) AS relevance
              FROM message_search
              INNER JOIN messages m ON m.id = message_search.rowid
              INNER JOIN sessions s ON m.session_id = s.id
              WHERE message_search MATCH ?
        "#,
        );

        if self.exclude_session_id.is_some() {
            sql.push_str(" AND s.id != ?");
        }
        if !self.session_types.is_empty() {
            let placeholders = std::iter::repeat_n("?", self.session_types.len())
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND s.session_type IN ({placeholders})"));
        }
        if self.after_date.is_some() {
            sql.push_str(" AND m.timestamp >= ?");
        }
        if self.before_date.is_some() {
            sql.push_str(" AND m.timestamp <= ?");
        }
        sql.push_str(
            r#"
              ORDER BY relevance, m.timestamp DESC
              LIMIT ?
            ),
            session_counts AS (
              SELECT session_id, COUNT(*) AS total_messages_in_session
              FROM messages
              WHERE session_id IN (SELECT DISTINCT session_id FROM matched_messages)
              GROUP BY session_id
            )
            SELECT
              matched_messages.session_id,
              matched_messages.session_description,
              matched_messages.session_working_dir,
              session_counts.total_messages_in_session,
              matched_messages.row_id,
              matched_messages.message_id,
              matched_messages.role,
              matched_messages.snippet,
              matched_messages.timestamp
            FROM matched_messages
            INNER JOIN session_counts
              ON session_counts.session_id = matched_messages.session_id
            ORDER BY matched_messages.relevance, matched_messages.timestamp DESC
            "#,
        );

        let mut query = sqlx::query_as::<_, SqlQueryRow>(&sql).bind(fts_query);
        if let Some(exclude_id) = &self.exclude_session_id {
            query = query.bind(exclude_id);
        }
        for session_type in &self.session_types {
            query = query.bind(session_type.to_string());
        }
        if let Some(after) = self.after_date {
            query = query.bind(after);
        }
        if let Some(before) = self.before_date {
            query = query.bind(before);
        }
        Ok(query.bind(self.limit as i64).fetch_all(self.pool).await?)
    }

    fn fts_query(&self) -> Option<String> {
        let terms: Vec<String> = self
            .query
            .split_whitespace()
            .filter_map(|term| {
                let term: String = term
                    .chars()
                    .filter(|ch| ch.is_alphanumeric() || *ch == '_' || *ch == '-')
                    .collect();
                (!term.is_empty()).then(|| format!("\"{term}\""))
            })
            .collect();
        (!terms.is_empty()).then(|| terms.join(" AND "))
    }

    fn process_rows(&self, rows: Vec<SqlQueryRow>) -> HashMap<String, SessionMessageGroup> {
        let mut session_messages = HashMap::with_capacity(rows.len());
        for (
            session_id,
            session_description,
            session_working_dir,
            total_messages_in_session,
            row_id,
            message_id,
            role,
            content,
            timestamp,
        ) in rows
        {
            let entry = session_messages.entry(session_id).or_insert_with(|| {
                (
                    session_description,
                    session_working_dir,
                    total_messages_in_session as usize,
                    Vec::new(),
                )
            });
            entry.3.push(ChatRecallMessage {
                row_id,
                message_id,
                role,
                content,
                timestamp,
            });
        }
        session_messages
    }

    fn convert_to_results(
        &self,
        session_messages: HashMap<String, SessionMessageGroup>,
    ) -> ChatRecallResults {
        let mut results: Vec<ChatRecallResult> = session_messages
            .into_iter()
            .map(
                |(session_id, (description, working_dir, total_messages_in_session, messages))| {
                    let last_activity = messages
                        .iter()
                        .map(|message| message.timestamp)
                        .max()
                        .unwrap_or_else(Utc::now);
                    ChatRecallResult {
                        session_id,
                        session_description: description,
                        session_working_dir: working_dir,
                        last_activity,
                        total_messages_in_session,
                        messages,
                    }
                },
            )
            .collect();
        results.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));
        let total_matches = results.iter().map(|result| result.messages.len()).sum();
        ChatRecallResults {
            results,
            total_matches,
        }
    }
}
