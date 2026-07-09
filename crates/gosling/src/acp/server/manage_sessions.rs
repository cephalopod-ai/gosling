use super::*;

impl GoslingAcpAgent {
    pub(super) async fn on_update_working_dir(
        &self,
        req: UpdateWorkingDirRequest,
    ) -> Result<EmptyResponse, agent_client_protocol::Error> {
        let working_dir = req.working_dir.trim().to_string();
        if working_dir.is_empty() {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("working directory cannot be empty"));
        }
        let path = std::path::PathBuf::from(&working_dir);
        validate_absolute_cwd(&path)?;
        let session_id = &req.session_id;

        let session = self
            .session_manager
            .get_session(session_id, false)
            .await
            .map_err(|_| {
                agent_client_protocol::Error::resource_not_found(Some(session_id.to_string()))
                    .data(format!("Session not found: {}", session_id))
            })?;

        if path == session.working_dir {
            return Ok(EmptyResponse {});
        }

        self.session_manager
            .update(session_id)
            .working_dir(path)
            .apply()
            .await
            .internal_err_ctx("Failed to update session working directory")?;

        let session = self
            .session_manager
            .get_session(session_id, false)
            .await
            .internal_err_ctx("Failed to reload session")?;

        let agent = self.get_session_agent(session_id).await?;
        agent
            .restore_provider_from_session(&session)
            .await
            .internal_err_ctx("Failed to refresh provider from session")?;

        agent
            .extension_manager
            .update_working_dirs(&session.working_dir, &session.additional_working_dirs)
            .await;

        Ok(EmptyResponse {})
    }

    pub(super) async fn on_add_session_working_dir(
        &self,
        req: AddSessionWorkingDirRequest,
    ) -> Result<SessionWorkingDirsResponse, agent_client_protocol::Error> {
        let working_dir = req.working_dir.trim().to_string();
        if working_dir.is_empty() {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("working directory cannot be empty"));
        }
        let path = std::path::PathBuf::from(&working_dir);
        validate_absolute_cwd(&path)?;
        let session_id = &req.session_id;

        let session = self
            .session_manager
            .get_session(session_id, false)
            .await
            .map_err(|_| {
                agent_client_protocol::Error::resource_not_found(Some(session_id.to_string()))
                    .data(format!("Session not found: {}", session_id))
            })?;

        let mut additional_working_dirs = session.additional_working_dirs.clone();
        if path != session.working_dir && !additional_working_dirs.contains(&path) {
            additional_working_dirs.push(path);
        }

        self.session_manager
            .update(session_id)
            .additional_working_dirs(additional_working_dirs)
            .apply()
            .await
            .internal_err_ctx("Failed to add session working directory")?;

        self.refresh_session_working_dirs(session_id).await
    }

    pub(super) async fn on_remove_session_working_dir(
        &self,
        req: RemoveSessionWorkingDirRequest,
    ) -> Result<SessionWorkingDirsResponse, agent_client_protocol::Error> {
        let working_dir = req.working_dir.trim().to_string();
        if working_dir.is_empty() {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("working directory cannot be empty"));
        }
        let path = std::path::PathBuf::from(&working_dir);
        let session_id = &req.session_id;

        let session = self
            .session_manager
            .get_session(session_id, false)
            .await
            .map_err(|_| {
                agent_client_protocol::Error::resource_not_found(Some(session_id.to_string()))
                    .data(format!("Session not found: {}", session_id))
            })?;

        let additional_working_dirs: Vec<_> = session
            .additional_working_dirs
            .into_iter()
            .filter(|dir| dir != &path)
            .collect();

        self.session_manager
            .update(session_id)
            .additional_working_dirs(additional_working_dirs)
            .apply()
            .await
            .internal_err_ctx("Failed to remove session working directory")?;

        self.refresh_session_working_dirs(session_id).await
    }

    pub(super) async fn on_set_session_working_dir_restriction(
        &self,
        req: SetSessionWorkingDirRestrictionRequest,
    ) -> Result<EmptyResponse, agent_client_protocol::Error> {
        let session_id = &req.session_id;
        self.session_manager
            .get_session(session_id, false)
            .await
            .map_err(|_| {
                agent_client_protocol::Error::resource_not_found(Some(session_id.to_string()))
                    .data(format!("Session not found: {}", session_id))
            })?;

        self.session_manager
            .update(session_id)
            .restrict_tools_to_working_dirs(req.restrict)
            .apply()
            .await
            .internal_err_ctx("Failed to update working directory restriction")?;

        Ok(EmptyResponse {})
    }

    async fn refresh_session_working_dirs(
        &self,
        session_id: &str,
    ) -> Result<SessionWorkingDirsResponse, agent_client_protocol::Error> {
        let session = self
            .session_manager
            .get_session(session_id, false)
            .await
            .internal_err_ctx("Failed to reload session")?;

        let agent = self.get_session_agent(session_id).await?;
        agent
            .extension_manager
            .update_working_dirs(&session.working_dir, &session.additional_working_dirs)
            .await;

        Ok(SessionWorkingDirsResponse {
            working_dir: session.working_dir.to_string_lossy().to_string(),
            additional_working_dirs: session
                .additional_working_dirs
                .iter()
                .map(|dir| dir.to_string_lossy().to_string())
                .collect(),
        })
    }

    pub(super) async fn on_set_session_system_prompt(
        &self,
        req: SetSessionSystemPromptRequest,
    ) -> Result<EmptyResponse, agent_client_protocol::Error> {
        let session_id = req.session_id.trim();
        if session_id.is_empty() {
            return Err(
                agent_client_protocol::Error::invalid_params().data("sessionId cannot be empty")
            );
        }

        let agent = self.get_session_agent(session_id).await?;
        match req.mode {
            SessionSystemPromptMode::Set => {
                if req.text.trim().is_empty() {
                    agent.clear_system_prompt_override().await;
                } else {
                    agent.override_system_prompt(req.text).await;
                }
            }
            SessionSystemPromptMode::Append => {
                let key = req
                    .key
                    .as_deref()
                    .map(str::trim)
                    .filter(|key| !key.is_empty())
                    .ok_or_else(|| {
                        agent_client_protocol::Error::invalid_params()
                            .data("key cannot be empty for append mode")
                    })?;
                if req.text.trim().is_empty() {
                    agent.remove_system_prompt_extra(key).await;
                } else {
                    agent.extend_system_prompt(key.to_string(), req.text).await;
                }
            }
        }

        Ok(EmptyResponse {})
    }

    pub(super) async fn on_delete_session(
        &self,
        req: DeleteSessionRequest,
    ) -> Result<EmptyResponse, agent_client_protocol::Error> {
        self.session_manager
            .delete_session(&req.session_id)
            .await
            .internal_err()?;
        self.sessions.lock().await.remove(&req.session_id);
        self.agent_manager
            .remove_session_if_loaded(&req.session_id)
            .await
            .internal_err_ctx("Failed to remove in-memory agent")?;
        Ok(EmptyResponse {})
    }

    pub(super) async fn on_export_session(
        &self,
        req: ExportSessionRequest,
    ) -> Result<ExportSessionResponse, agent_client_protocol::Error> {
        let data = self
            .session_manager
            .export_session(&req.session_id)
            .await
            .internal_err()?;
        Ok(ExportSessionResponse { data })
    }

    pub(super) async fn on_import_session(
        &self,
        req: ImportSessionRequest,
    ) -> Result<ImportSessionResponse, agent_client_protocol::Error> {
        let is_nostr = match req.source {
            SessionImportSource::Auto => is_nostr_session_link(&req.input),
            SessionImportSource::Json => false,
            SessionImportSource::Nostr => true,
        };
        let (data, session_type) = if is_nostr {
            (
                import_nostr_session_json(&req.input).await?,
                Some(SessionType::User),
            )
        } else {
            (req.input, None)
        };

        let session = self
            .session_manager
            .import_session(&data, session_type)
            .await
            .internal_err()?;

        let msg_count = session.message_count as u64;

        Ok(ImportSessionResponse {
            session_id: session.id,
            title: Some(session.name),
            updated_at: Some(session.updated_at.to_rfc3339()),
            message_count: msg_count,
        })
    }

    pub(super) async fn on_share_session_nostr(
        &self,
        req: ShareSessionNostrRequest,
    ) -> Result<ShareSessionNostrResponse, agent_client_protocol::Error> {
        let data = self
            .session_manager
            .export_session(&req.session_id)
            .await
            .internal_err()?;

        let share = publish_session_to_nostr(&data, req.relays).await?;

        Ok(ShareSessionNostrResponse {
            deeplink: share.deeplink,
            nevent: share.nevent,
            event_id: share.event_id,
            relays: share.relays,
        })
    }

    pub(super) async fn on_get_session_info(
        &self,
        req: GetSessionInfoRequest,
    ) -> Result<GetSessionInfoResponse, agent_client_protocol::Error> {
        let session_id = req.session_id.trim();
        if session_id.is_empty() {
            return Err(
                agent_client_protocol::Error::invalid_params().data("sessionId cannot be empty")
            );
        }

        let session = self
            .session_manager
            .get_session(session_id, false)
            .await
            .map_err(|_| {
                agent_client_protocol::Error::resource_not_found(Some(session_id.to_string()))
                    .data(format!("Session not found: {}", session_id))
            })?;

        Ok(GetSessionInfoResponse {
            session: build_session_info(session),
        })
    }

    pub(super) async fn on_list_session_messages(
        &self,
        req: ListSessionMessagesRequest,
    ) -> Result<ListSessionMessagesResponse, agent_client_protocol::Error> {
        let session_id = req.session_id.trim();
        if session_id.is_empty() {
            return Err(
                agent_client_protocol::Error::invalid_params().data("sessionId cannot be empty")
            );
        }
        let limit = req.limit.unwrap_or(DEFAULT_SESSION_TAIL_LIMIT);
        let page = self
            .session_manager
            .get_session_message_page(session_id, req.before_cursor.as_deref(), limit)
            .await
            .map_err(|error| {
                if error.to_string().contains("Invalid before cursor") {
                    agent_client_protocol::Error::invalid_params().data(error.to_string())
                } else {
                    agent_client_protocol::Error::internal_error().data(error.to_string())
                }
            })?;
        let messages = page
            .messages
            .into_iter()
            .map(|message| serde_json::to_value(message).unwrap_or(serde_json::Value::Null))
            .collect();
        Ok(ListSessionMessagesResponse {
            messages,
            next_before_cursor: page.next_before_cursor,
            total_count: page.total_count,
            oldest_row_id: page.oldest_row_id,
            newest_row_id: page.newest_row_id,
        })
    }

    pub(super) async fn on_search_session_messages(
        &self,
        req: SearchSessionMessagesRequest,
    ) -> Result<SearchSessionMessagesResponse, agent_client_protocol::Error> {
        let session_id = req.session_id.trim();
        if session_id.is_empty() {
            return Err(
                agent_client_protocol::Error::invalid_params().data("sessionId cannot be empty")
            );
        }
        let results = self
            .session_manager
            .search_session_messages(session_id, req.query.trim(), req.limit.unwrap_or(20))
            .await
            .internal_err()?;
        Ok(SearchSessionMessagesResponse {
            matches: results
                .matches
                .into_iter()
                .map(|m| SessionMessageSearchMatch {
                    row_id: m.row_id,
                    message_id: m.message_id,
                    role: m.role,
                    snippet: m.snippet,
                    created: m.created,
                    before_cursor: m.before_cursor,
                })
                .collect(),
            total_matches: results.total_matches,
        })
    }

    pub(super) async fn on_get_session_summary(
        &self,
        req: GetSessionSummaryRequest,
    ) -> Result<GetSessionSummaryResponse, agent_client_protocol::Error> {
        let session_id = req.session_id.trim();
        if session_id.is_empty() {
            return Err(
                agent_client_protocol::Error::invalid_params().data("sessionId cannot be empty")
            );
        }
        let summary = self
            .session_manager
            .get_session_summary(session_id)
            .await
            .internal_err()?
            .map(|summary| SessionSummaryDto {
                summary: summary.summary,
                covered_through_row_id: summary.covered_through_row_id,
                covered_through_timestamp: summary.covered_through_timestamp,
                covered_message_count: summary.covered_message_count,
                status: summary.status.to_string(),
                error: summary.error,
                updated_at: summary.updated_at.to_rfc3339(),
            });
        let facts = self
            .session_manager
            .get_session_summary_facts(session_id)
            .await
            .internal_err()?
            .into_iter()
            .map(|fact| SessionSummaryFactDto {
                id: fact.id,
                scope: fact.scope,
                fact_type: fact.fact_type,
                content: fact.content,
                confidence: fact.confidence,
            })
            .collect();
        Ok(GetSessionSummaryResponse { summary, facts })
    }

    pub(super) async fn on_truncate_session_conversation(
        &self,
        req: TruncateSessionConversationRequest,
    ) -> Result<EmptyResponse, agent_client_protocol::Error> {
        let session_id = req.session_id.trim();
        if session_id.is_empty() {
            return Err(
                agent_client_protocol::Error::invalid_params().data("sessionId cannot be empty")
            );
        }

        self.session_manager
            .truncate_conversation(session_id, req.truncate_from)
            .await
            .internal_err()?;
        Ok(EmptyResponse {})
    }

    pub(super) async fn on_update_session_project(
        &self,
        req: UpdateSessionProjectRequest,
    ) -> Result<EmptyResponse, agent_client_protocol::Error> {
        self.session_manager
            .update(&req.session_id)
            .project_id(req.project_id)
            .apply()
            .await
            .internal_err()?;
        Ok(EmptyResponse {})
    }

    pub(super) async fn on_rename_session(
        &self,
        req: RenameSessionRequest,
    ) -> Result<EmptyResponse, agent_client_protocol::Error> {
        self.session_manager
            .update(&req.session_id)
            .user_provided_name(req.title)
            .apply()
            .await
            .map_err(|e| agent_client_protocol::Error::internal_error().data(e.to_string()))?;
        Ok(EmptyResponse {})
    }

    pub(super) async fn on_archive_session(
        &self,
        req: ArchiveSessionRequest,
    ) -> Result<EmptyResponse, agent_client_protocol::Error> {
        self.session_manager
            .update(&req.session_id)
            .archived_at(Some(chrono::Utc::now()))
            .apply()
            .await
            .internal_err()?;
        self.sessions.lock().await.remove(&req.session_id);
        self.agent_manager
            .remove_session_if_loaded(&req.session_id)
            .await
            .internal_err_ctx("Failed to remove in-memory agent")?;
        Ok(EmptyResponse {})
    }

    pub(super) async fn on_unarchive_session(
        &self,
        req: UnarchiveSessionRequest,
    ) -> Result<EmptyResponse, agent_client_protocol::Error> {
        self.session_manager
            .update(&req.session_id)
            .archived_at(None)
            .apply()
            .await
            .internal_err()?;
        Ok(EmptyResponse {})
    }
}

fn is_nostr_session_link(input: &str) -> bool {
    #[cfg(feature = "nostr")]
    {
        crate::session::nostr_share::is_session_share_deeplink(input)
    }
    #[cfg(not(feature = "nostr"))]
    {
        let _ = input;
        false
    }
}

#[cfg(feature = "nostr")]
async fn import_nostr_session_json(deeplink: &str) -> Result<String, agent_client_protocol::Error> {
    crate::session::nostr_share::import_session_json_from_deeplink(deeplink)
        .await
        .invalid_params_err()
}

#[cfg(not(feature = "nostr"))]
async fn import_nostr_session_json(
    _deeplink: &str,
) -> Result<String, agent_client_protocol::Error> {
    Err(agent_client_protocol::Error::invalid_params()
        .data("Nostr session import is not available in this build"))
}

#[cfg(feature = "nostr")]
async fn publish_session_to_nostr(
    data: &str,
    relays: Vec<String>,
) -> Result<NostrSessionShare, agent_client_protocol::Error> {
    let relays = crate::session::nostr_share::resolve_relays(relays, Config::global());
    let share = crate::session::nostr_share::publish_session_json(data, relays)
        .await
        .internal_err()?;
    Ok(NostrSessionShare {
        deeplink: share.deeplink,
        nevent: share.nevent,
        event_id: share.event_id,
        relays: share.relays,
    })
}

#[cfg(not(feature = "nostr"))]
async fn publish_session_to_nostr(
    _data: &str,
    _relays: Vec<String>,
) -> Result<NostrSessionShare, agent_client_protocol::Error> {
    Err(agent_client_protocol::Error::invalid_params()
        .data("Nostr session sharing is not available in this build"))
}

struct NostrSessionShare {
    deeplink: String,
    nevent: String,
    event_id: String,
    relays: Vec<String>,
}
