use crate::conversation::message::MessageContent;
use crate::session::session_manager::SessionType;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Pool, QueryBuilder, Sqlite};
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
    pub role: String,
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
    DateTime<Utc>,
    String,
    String,
    DateTime<Utc>,
);

type SessionMessageGroup = (
    String,
    String,
    DateTime<Utc>,
    Vec<(String, String, DateTime<Utc>)>,
);

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
            limit: limit.unwrap_or(10),
            after_date,
            before_date,
            exclude_session_id,
            session_types,
        }
    }

    pub async fn execute(self) -> Result<ChatRecallResults> {
        let keywords = self.parse_keywords();
        if keywords.is_empty() {
            return Ok(ChatRecallResults {
                results: vec![],
                total_matches: 0,
            });
        }

        let rows = self.fetch_rows(&keywords).await?;
        let session_messages = self.process_rows(rows);
        let session_totals = self.get_session_totals(&session_messages).await?;
        let results = self.convert_to_results(session_messages, session_totals);

        Ok(results)
    }

    async fn fetch_rows(&self, keywords: &[String]) -> Result<Vec<SqlQueryRow>> {
        let sql = self.build_sql(keywords);
        let mut query_builder = sqlx::query_as::<_, SqlQueryRow>(&sql);

        for keyword in keywords {
            query_builder = query_builder.bind(keyword);
        }

        if let Some(exclude_id) = &self.exclude_session_id {
            query_builder = query_builder.bind(exclude_id);
        }

        for t in &self.session_types {
            query_builder = query_builder.bind(t.to_string());
        }

        if let Some(after) = self.after_date {
            query_builder = query_builder.bind(after);
        }
        if let Some(before) = self.before_date {
            query_builder = query_builder.bind(before);
        }

        query_builder = query_builder.bind(self.limit as i64);

        Ok(query_builder.fetch_all(self.pool).await?)
    }

    fn parse_keywords(&self) -> Vec<String> {
        self.query
            .split_whitespace()
            .map(|word| format!("%{}%", word.to_lowercase()))
            .collect()
    }

    fn build_sql(&self, keywords: &[String]) -> String {
        let mut sql = String::from(
            r#"
            SELECT 
                s.id as session_id,
                s.description as session_description,
                s.working_dir as session_working_dir,
                s.created_at as session_created_at,
                m.role,
                m.content_json,
                m.timestamp
            FROM messages m
            INNER JOIN sessions s ON m.session_id = s.id
            WHERE EXISTS (
                SELECT 1 FROM json_each(m.content_json) 
                WHERE json_extract(value, '$.type') = 'text' 
                AND (
        "#,
        );

        for (i, _) in keywords.iter().enumerate() {
            if i > 0 {
                sql.push_str(" OR ");
            }
            sql.push_str("LOWER(json_extract(value, '$.text')) LIKE ?");
        }

        sql.push_str(
            r#"
                )
            )
        "#,
        );

        if self.exclude_session_id.is_some() {
            sql.push_str(" AND s.id != ?");
        }

        if !self.session_types.is_empty() {
            let placeholders: String = self
                .session_types
                .iter()
                .map(|_| "?")
                .collect::<Vec<_>>()
                .join(", ");
            sql.push_str(&format!(" AND s.session_type IN ({})", placeholders));
        }

        if self.after_date.is_some() {
            sql.push_str(" AND m.timestamp >= ?");
        }
        if self.before_date.is_some() {
            sql.push_str(" AND m.timestamp <= ?");
        }

        sql.push_str(" ORDER BY m.timestamp DESC LIMIT ?");

        sql
    }

    fn process_rows(&self, rows: Vec<SqlQueryRow>) -> HashMap<String, SessionMessageGroup> {
        let mut session_messages: HashMap<String, SessionMessageGroup> =
            HashMap::with_capacity(rows.len());

        for (
            session_id,
            session_description,
            session_working_dir,
            session_created_at,
            role,
            content_json,
            timestamp,
        ) in rows
        {
            if let Ok(content_vec) = serde_json::from_str::<Vec<MessageContent>>(&content_json) {
                let text_parts = Self::extract_text_content(content_vec);

                if !text_parts.is_empty() {
                    let entry = session_messages.entry(session_id).or_insert_with(|| {
                        (
                            session_description,
                            session_working_dir,
                            session_created_at,
                            Vec::new(),
                        )
                    });
                    entry.3.push((role, text_parts.join("\n"), timestamp));
                }
            }
        }

        session_messages
    }

    fn extract_text_content(content_vec: Vec<MessageContent>) -> Vec<String> {
        content_vec
            .into_iter()
            .filter_map(|content| match content {
                MessageContent::Text(tc) => Some(tc.raw.text),
                MessageContent::ToolRequest(tr) => {
                    Some(format!("[Tool: {}]", tr.to_readable_string()))
                }
                MessageContent::ToolResponse(_) => Some("[Tool Response]".to_string()),
                MessageContent::Thinking(t) => Some(format!("[Thinking: {}]", t.thinking)),
                _ => None,
            })
            .collect()
    }

    async fn get_session_totals(
        &self,
        session_messages: &HashMap<String, SessionMessageGroup>,
    ) -> Result<HashMap<String, usize>> {
        if session_messages.is_empty() {
            return Ok(HashMap::new());
        }

        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT session_id, COUNT(*) FROM messages WHERE session_id IN (",
        );
        {
            let mut session_ids = query.separated(", ");
            for session_id in session_messages.keys() {
                session_ids.push_bind(session_id);
            }
        }
        query.push(") GROUP BY session_id");

        let totals = query
            .build_query_as::<(String, i64)>()
            .fetch_all(self.pool)
            .await
            .unwrap_or_default();

        Ok(totals
            .into_iter()
            .map(|(session_id, count)| (session_id, count as usize))
            .collect())
    }

    fn convert_to_results(
        &self,
        session_messages: HashMap<String, SessionMessageGroup>,
        session_totals: HashMap<String, usize>,
    ) -> ChatRecallResults {
        let mut results: Vec<ChatRecallResult> = session_messages
            .into_iter()
            .map(
                |(session_id, (description, working_dir, _created_at, messages))| {
                    let message_vec: Vec<ChatRecallMessage> = messages
                        .into_iter()
                        .map(|(role, content, timestamp)| ChatRecallMessage {
                            role,
                            content,
                            timestamp,
                        })
                        .collect();

                    let last_activity = message_vec
                        .iter()
                        .map(|m| m.timestamp)
                        .max()
                        .unwrap_or_else(chrono::Utc::now);

                    let total_messages_in_session =
                        session_totals.get(&session_id).copied().unwrap_or(0);

                    ChatRecallResult {
                        session_id,
                        session_description: description,
                        session_working_dir: working_dir,
                        last_activity,
                        total_messages_in_session,
                        messages: message_vec,
                    }
                },
            )
            .collect();

        results.sort_by(|a, b| b.last_activity.cmp(&a.last_activity));

        let total_matches = results.iter().map(|r| r.messages.len()).sum();
        ChatRecallResults {
            results,
            total_matches,
        }
    }
}
