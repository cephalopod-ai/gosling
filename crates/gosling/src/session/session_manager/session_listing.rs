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
    normalized_message_timestamp_sql, session_sort_at, Session, SessionArchiveState,
    SessionListCursor, SessionListFilters, SessionListPage, SessionListPageQuery, SessionStorage,
    SessionType,
};
use anyhow::Result;

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
        let mut having_clauses = Vec::new();
        let normalized_message_timestamp = normalized_message_timestamp_sql("m.created_timestamp");
        let sort_timestamp_sql =
            format!("COALESCE(MAX({normalized_message_timestamp}), unixepoch(s.updated_at))");
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
        if !keywords.is_empty() {
            where_clauses.push(message_keyword_clause(keywords.len()));
        }
        if query.cursor.is_some() {
            having_clauses.push(format!(
                "({sort_timestamp_sql} < ? OR ({sort_timestamp_sql} = ? AND s.id < ?))"
            ));
        }

        let where_clause = if where_clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", where_clauses.join(" AND "))
        };
        let having_clause = if having_clauses.is_empty() {
            String::new()
        } else {
            format!("HAVING {}", having_clauses.join(" AND "))
        };
        let message_join = if filters.only_sessions_with_messages {
            "JOIN messages m ON s.id = m.session_id"
        } else {
            "LEFT JOIN messages m ON s.id = m.session_id"
        };
        let order_by = "ORDER BY sort_timestamp DESC, s.id DESC";
        let limit_clause = if query.limit.is_some() { "LIMIT ?" } else { "" };

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
                   COUNT(m.id) as message_count,
                   MAX({}) as last_message_timestamp,
                   {} as sort_timestamp
            FROM sessions s
            {}
            {}
            GROUP BY s.id
            {}
            {}
            {}
            "#,
            normalized_message_timestamp,
            sort_timestamp_sql,
            message_join,
            where_clause,
            having_clause,
            order_by,
            limit_clause
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
