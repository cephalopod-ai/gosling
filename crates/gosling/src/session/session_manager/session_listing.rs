//! Session listing and pagination: the dynamic filtered/keyword-searched
//! SQL builder and its paged/unpaged/type-filtered call shapes.
//!
//! Extracted from `crate::session::session_manager` in a behavior-preserving
//! modularization (see `docs/logs/session/2026-08-22-modularize-session-manager.md`).
//! `SessionListQuery`, `keyword_terms`, and `message_keyword_clause` move
//! here from module level (their only use is `list_sessions_matching`, in
//! this same module). `pub(super)` matches the four methods' pre-extraction
//! (private, same-module) visibility — the facade's `impl SessionManager`
//! delegates to `list_sessions_by_types`/`list_sessions_paged`/
//! `list_sessions`.

use super::{
    session_sort_at, Session, SessionArchiveState, SessionListCursor, SessionListFilters,
    SessionListPage, SessionListPageQuery, SessionStorage, SessionType,
    MILLISECOND_TIMESTAMP_THRESHOLD,
};
use anyhow::Result;

/// Newest message time in seconds. Millisecond and second timestamps are
/// maxed separately so each side is an index-range `MAX` rather than a scan
/// over a normalized expression; SQLite's multi-argument `MAX` returns NULL
/// when either side is NULL, hence the explicit cases.
const LAST_MESSAGE_TIMESTAMP_SQL: &str = "CASE \
    WHEN s.millisecond_last_message IS NULL THEN s.second_last_message \
    WHEN s.second_last_message IS NULL THEN s.millisecond_last_message \
    ELSE MAX(s.millisecond_last_message, s.second_last_message) END";

#[derive(Debug, Default)]
struct SessionListQuery<'a> {
    filters: SessionListFilters<'a>,
    cursor: Option<&'a SessionListCursor>,
    limit: Option<usize>,
}

fn keyword_terms(query: Option<&str>) -> Vec<String> {
    query
        .unwrap_or_default()
        .split_whitespace()
        .map(|word| word.to_lowercase())
        .collect()
}

fn message_keyword_clause(keyword_count: usize) -> String {
    let keyword_clauses = (0..keyword_count)
        .map(|_| "instr(LOWER(json_extract(value, '$.text')), ?) > 0")
        .collect::<Vec<_>>()
        .join(" OR ");

    format!(
        r#"
        EXISTS (
            SELECT 1
            FROM messages mq
            WHERE mq.session_id = s.id
              AND EXISTS (
                  SELECT 1
                  FROM json_each(mq.content_json)
                  WHERE json_extract(value, '$.type') = 'text'
                    AND ({keyword_clauses})
              )
        )
        "#
    )
}

impl SessionStorage {
    async fn list_sessions_matching(&self, query: SessionListQuery<'_>) -> Result<Vec<Session>> {
        let filters = &query.filters;
        if matches!(filters.types, Some(types) if types.is_empty()) {
            return Ok(Vec::new());
        }

        let keywords = keyword_terms(filters.keyword);
        let mut where_clauses = Vec::new();
        if let Some(types) = filters.types {
            let placeholders = types.iter().map(|_| "?").collect::<Vec<_>>().join(", ");
            where_clauses.push(format!("s.session_type IN ({})", placeholders));
        }
        match filters.archive_state {
            SessionArchiveState::Active => where_clauses.push("s.archived_at IS NULL".to_string()),
            SessionArchiveState::Archived => {
                where_clauses.push("s.archived_at IS NOT NULL".to_string())
            }
            SessionArchiveState::All => {}
        }
        if filters.working_dir.is_some() {
            where_clauses.push("s.working_dir = ?".to_string());
        }
        if filters.workspace_id.is_some() {
            where_clauses.push(if filters.include_unassigned {
                "(s.workspace_id = ? OR s.workspace_id IS NULL)".to_string()
            } else {
                "s.workspace_id = ?".to_string()
            });
        }
        if filters.only_sessions_with_messages {
            where_clauses
                .push("EXISTS (SELECT 1 FROM messages m WHERE m.session_id = s.id)".to_string());
        }
        if !keywords.is_empty() {
            where_clauses.push(message_keyword_clause(keywords.len()));
        }

        let where_clause = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };
        let cursor_clause = if query.cursor.is_some() {
            "WHERE (s.sort_timestamp < ? OR (s.sort_timestamp = ? AND s.id < ?))"
        } else {
            ""
        };
        let order_by = "ORDER BY s.sort_timestamp DESC, s.id DESC";
        let limit_clause = if query.limit.is_some() { "LIMIT ?" } else { "" };

        // Message activity is derived per session from index seeks and the
        // message count only for the returned page; aggregating the join over
        // every message row before LIMIT scaled with the whole messages table.
        let sql = format!(
            r#"
            SELECT s.id, s.working_dir, s.additional_working_dirs_json, s.restrict_tools_to_working_dirs, s.name, s.description, s.user_set_name, s.session_type, s.created_at, s.updated_at, s.extension_data,
                   s.total_tokens, s.input_tokens, s.output_tokens,
                   s.cache_read_tokens, s.cache_write_tokens,
                   s.accumulated_total_tokens, s.accumulated_input_tokens, s.accumulated_output_tokens,
                   s.accumulated_cache_read_tokens, s.accumulated_cache_write_tokens,
                   s.accumulated_cost,
                   s.provider_name, s.model_config_json, s.gosling_mode,
                   s.archived_at, s.project_id, s.workspace_id, s.workspace_name,
                   s.credential_profile_id, s.credential_profile_name,
                   s.credential_binding_id, s.workspace_context_json,
                   (SELECT COUNT(*) FROM messages m WHERE m.session_id = s.id) AS message_count,
                   s.last_message_timestamp
            FROM (
                SELECT s.*
                FROM (
                    SELECT s.*,
                           {last_message_timestamp} AS last_message_timestamp,
                           COALESCE({last_message_timestamp}, unixepoch(s.updated_at)) AS sort_timestamp
                    FROM (
                        SELECT s.*,
                               (SELECT MAX(m.created_timestamp) / 1000 FROM messages m
                                WHERE m.session_id = s.id AND m.created_timestamp > {threshold}) AS millisecond_last_message,
                               (SELECT MAX(m.created_timestamp) FROM messages m
                                WHERE m.session_id = s.id AND m.created_timestamp <= {threshold}) AS second_last_message
                        FROM sessions s
                        {where_clause}
                    ) s
                ) s
                {cursor_clause}
                {order_by}
                {limit_clause}
            ) s
            {order_by}
            "#,
            last_message_timestamp = LAST_MESSAGE_TIMESTAMP_SQL,
            threshold = MILLISECOND_TIMESTAMP_THRESHOLD,
        );

        let mut q = sqlx::query_as::<_, Session>(&sql);
        if let Some(types) = filters.types {
            for session_type in types {
                q = q.bind(session_type.to_string());
            }
        }
        if let Some(working_dir) = filters.working_dir {
            q = q.bind(working_dir.to_string_lossy().to_string());
        }
        if let Some(workspace_id) = filters.workspace_id {
            q = q.bind(workspace_id);
        }
        for term in keywords {
            q = q.bind(term);
        }
        if let Some(cursor) = query.cursor {
            let sort_at = cursor.sort_at.timestamp();
            q = q.bind(sort_at);
            q = q.bind(sort_at);
            q = q.bind(&cursor.session_id);
        }
        if let Some(limit) = query.limit {
            q = q.bind(limit as i64);
        }

        let pool = self.pool().await?;
        q.fetch_all(pool).await.map_err(Into::into)
    }

    pub(super) async fn list_sessions_by_types(
        &self,
        types: Option<&[SessionType]>,
        archive_state: SessionArchiveState,
    ) -> Result<Vec<Session>> {
        self.list_sessions_matching(SessionListQuery {
            filters: SessionListFilters {
                types,
                archive_state,
                ..Default::default()
            },
            ..Default::default()
        })
        .await
    }

    pub(super) async fn list_sessions_paged(
        &self,
        query: SessionListPageQuery<'_>,
    ) -> Result<SessionListPage> {
        if matches!(query.filters.types, Some(types) if types.is_empty()) || query.page_size == 0 {
            return Ok(SessionListPage {
                sessions: Vec::new(),
                next_cursor: None,
            });
        }

        let page_size = query.page_size;
        let include_last_message_snippet = query.include_last_message_snippet;
        let mut sessions = self
            .list_sessions_matching(SessionListQuery {
                filters: query.filters,
                cursor: query.cursor,
                limit: Some(page_size + 1),
            })
            .await?;
        let has_next_page = sessions.len() > page_size;
        let next_cursor = if has_next_page {
            let anchor = &sessions[page_size - 1];
            Some(SessionListCursor {
                sort_at: session_sort_at(anchor),
                session_id: anchor.id.clone(),
            })
        } else {
            None
        };
        if has_next_page {
            sessions.truncate(page_size);
        }
        if include_last_message_snippet {
            let pool = self.pool().await?;
            crate::session::last_message_snippet::hydrate_last_message_snippets(
                pool,
                &mut sessions,
            )
            .await?;
        }

        Ok(SessionListPage {
            sessions,
            next_cursor,
        })
    }

    pub(super) async fn list_sessions(&self) -> Result<Vec<Session>> {
        self.list_sessions_by_types(
            Some(&[SessionType::User, SessionType::Scheduled]),
            SessionArchiveState::Active,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use crate::config::GoslingMode;
    use crate::session::session_manager::{
        session_sort_at, Session, SessionListCursor, SessionListFilters, SessionListPageQuery,
        SessionManager, SessionType,
    };
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    const WORKING_DIR: &str = "/tmp/session-listing";

    async fn listing_session(sm: &SessionManager, name: &str) -> String {
        sm.create_session(
            PathBuf::from(WORKING_DIR),
            name.to_string(),
            SessionType::User,
            GoslingMode::default(),
        )
        .await
        .unwrap()
        .id
    }

    async fn insert_messages(sm: &SessionManager, session_id: &str, created_timestamps: &[i64]) {
        let pool = sm.storage().pool().await.unwrap();
        for (index, created) in created_timestamps.iter().enumerate() {
            sqlx::query(
                "INSERT INTO messages (message_id, session_id, role, content_json, created_timestamp) VALUES (?, ?, 'user', ?, ?)",
            )
            .bind(format!("{session_id}-{index}"))
            .bind(session_id)
            .bind(r#"[{"type":"text","text":"listing fixture"}]"#)
            .bind(created)
            .execute(pool)
            .await
            .unwrap();
        }
    }

    async fn set_updated_at(sm: &SessionManager, session_id: &str, unix_seconds: i64) {
        let pool = sm.storage().pool().await.unwrap();
        sqlx::query("UPDATE sessions SET updated_at = datetime(?, 'unixepoch') WHERE id = ?")
            .bind(unix_seconds)
            .bind(session_id)
            .execute(pool)
            .await
            .unwrap();
    }

    async fn list_all_pages(
        sm: &SessionManager,
        page_size: usize,
        only_sessions_with_messages: bool,
    ) -> Vec<Session> {
        let types = [SessionType::User];
        let mut cursor: Option<SessionListCursor> = None;
        let mut sessions = Vec::new();
        loop {
            let page = sm
                .list_sessions_paged(SessionListPageQuery {
                    filters: SessionListFilters {
                        types: Some(&types),
                        working_dir: Some(Path::new(WORKING_DIR)),
                        only_sessions_with_messages,
                        ..Default::default()
                    },
                    cursor: cursor.as_ref(),
                    page_size,
                    include_last_message_snippet: false,
                })
                .await
                .unwrap();
            sessions.extend(page.sessions);
            match page.next_cursor {
                Some(next) => cursor = Some(next),
                None => return sessions,
            }
        }
    }

    fn assert_matches_activity(listed: &[Session], expected: &[Session]) {
        let listed_ids = listed.iter().map(|s| s.id.as_str()).collect::<Vec<_>>();
        let expected_ids = expected.iter().map(|s| s.id.as_str()).collect::<Vec<_>>();
        assert_eq!(listed_ids, expected_ids);
        for (listed, expected) in listed.iter().zip(expected) {
            assert_eq!(
                listed.message_count, expected.message_count,
                "{}",
                listed.id
            );
            assert_eq!(
                listed.last_message_at, expected.last_message_at,
                "{}",
                listed.id
            );
        }
    }

    #[tokio::test]
    async fn test_session_list_activity_matches_per_session_aggregates() {
        let temp_dir = TempDir::new().unwrap();
        let sm = SessionManager::new(temp_dir.path().to_path_buf());
        let second = 1_700_000_000_i64;
        let millisecond = |seconds: i64| seconds * 1000 + 999;

        let fixtures: [(&str, Vec<i64>, i64); 7] = [
            ("empty, recently updated", vec![], second + 5_000),
            (
                "seconds only",
                vec![second + 10, second + 400, second + 20],
                second,
            ),
            ("milliseconds only", vec![millisecond(second + 300)], second),
            (
                "mixed, newest in milliseconds",
                vec![second + 100, millisecond(second + 500)],
                second,
            ),
            (
                "mixed, newest in seconds",
                vec![millisecond(second + 200), second + 600],
                second,
            ),
            (
                "stale messages, newer updated_at",
                vec![second + 1],
                second + 9_000,
            ),
            ("tie with seconds only", vec![second + 400], second),
        ];
        let mut ids = Vec::new();
        for (name, created_timestamps, updated_at) in &fixtures {
            let id = listing_session(&sm, name).await;
            insert_messages(&sm, &id, created_timestamps).await;
            set_updated_at(&sm, &id, *updated_at).await;
            ids.push(id);
        }

        let mut expected = Vec::new();
        for id in &ids {
            expected.push(sm.get_session(id, false).await.unwrap());
        }
        expected.sort_by(|a, b| {
            session_sort_at(b)
                .cmp(&session_sort_at(a))
                .then_with(|| b.id.cmp(&a.id))
        });
        assert_eq!(
            expected[0].message_count, 0,
            "empty session sorts by updated_at"
        );

        assert_matches_activity(&list_all_pages(&sm, 1, false).await, &expected);
        assert_matches_activity(&list_all_pages(&sm, 3, false).await, &expected);
        assert_matches_activity(&list_all_pages(&sm, 100, false).await, &expected);

        let with_messages = expected
            .iter()
            .filter(|session| session.message_count > 0)
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(with_messages.len(), 6);
        assert_matches_activity(&list_all_pages(&sm, 2, true).await, &with_messages);

        let unpaged = sm.list_sessions().await.unwrap();
        assert_matches_activity(&unpaged, &expected);
    }

    #[tokio::test]
    #[ignore = "manual same-workload session listing benchmark; run with --ignored --nocapture"]
    async fn benchmark_session_listing() {
        use std::time::Instant;

        fn report(label: &str, mut samples: Vec<f64>) {
            samples.sort_by(f64::total_cmp);
            println!(
                "{label}: median={:.2}us p95={:.2}us min={:.2}us max={:.2}us n={}",
                samples[samples.len() / 2],
                samples[(samples.len() * 95).div_ceil(100) - 1],
                samples[0],
                samples[samples.len() - 1],
                samples.len(),
            );
        }

        const SESSIONS: usize = 300;
        const MESSAGES_PER_SESSION: i64 = 100;
        let temp_dir = TempDir::new().unwrap();
        let sm = SessionManager::new(temp_dir.path().to_path_buf());
        let pool = sm.storage().pool().await.unwrap();
        for index in 0..SESSIONS {
            let id = listing_session(&sm, &format!("session {index}")).await;
            let base = 1_700_000_000 + index as i64;
            let mut insert = String::from(
                "INSERT INTO messages (message_id, session_id, role, content_json, created_timestamp) VALUES ",
            );
            let rows = (0..MESSAGES_PER_SESSION)
                .map(|m| {
                    format!(
                        "('{id}-{m}', '{id}', 'user', '[{{\"type\":\"text\",\"text\":\"benchmark fixture\"}}]', {})",
                        base + m * SESSIONS as i64
                    )
                })
                .collect::<Vec<_>>();
            insert.push_str(&rows.join(", "));
            sqlx::query(&insert).execute(pool).await.unwrap();
        }

        async fn first_page(sm: &SessionManager) -> super::SessionListPage {
            let types = [SessionType::User];
            sm.list_sessions_paged(SessionListPageQuery {
                filters: SessionListFilters {
                    types: Some(&types),
                    working_dir: Some(Path::new(WORKING_DIR)),
                    only_sessions_with_messages: true,
                    ..Default::default()
                },
                cursor: None,
                page_size: 50,
                include_last_message_snippet: false,
            })
            .await
            .unwrap()
        }

        let expected = first_page(&sm).await;
        assert_eq!(expected.sessions.len(), 50);
        assert!(expected.next_cursor.is_some());
        for _ in 0..10 {
            first_page(&sm).await;
        }
        let mut samples = Vec::new();
        for _ in 0..30 {
            let start = Instant::now();
            let page = first_page(&sm).await;
            samples.push(start.elapsed().as_secs_f64() * 1_000_000.0);
            let ids = |page: &super::SessionListPage| {
                page.sessions
                    .iter()
                    .map(|s| (s.id.clone(), s.message_count, s.last_message_at))
                    .collect::<Vec<_>>()
            };
            assert_eq!(ids(&page), ids(&expected));
        }
        report(
            &format!("first page of {SESSIONS} sessions x {MESSAGES_PER_SESSION} messages"),
            samples,
        );

        for _ in 0..5 {
            sm.list_sessions().await.unwrap();
        }
        let mut samples = Vec::new();
        for _ in 0..30 {
            let start = Instant::now();
            let sessions = sm.list_sessions().await.unwrap();
            samples.push(start.elapsed().as_secs_f64() * 1_000_000.0);
            assert_eq!(sessions.len(), SESSIONS);
        }
        report("unpaged list of all sessions", samples);
    }
}
