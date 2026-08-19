use super::*;

const SHELL_CREDENTIAL_LOOKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
const SHELL_ARTIFACT_LIMIT: usize = 100;
const SHELL_ARTIFACT_NAME_LIMIT: usize = 256;
// A failed lookup (timeout, panic, or read error) is usually transient — a locked keychain or a
// momentarily unreadable workspace document — so callers back off for this long instead of
// retrying on every request, but still retry on the next call after it elapses rather than
// disabling credential selection for the rest of the process's life.
const SHELL_CREDENTIAL_LOOKUP_COOLDOWN: std::time::Duration = std::time::Duration::from_secs(15);

impl GoslingAcpAgent {
    pub(super) async fn shell_credential_profiles(
        &self,
    ) -> Option<Vec<crate::workspace::CredentialProfile>> {
        let cooldown_until = *self.shell_credential_lookup_cooldown_until.lock().unwrap();
        if cooldown_until.is_some_and(|until| std::time::Instant::now() < until) {
            return None;
        }
        let workspace_service = std::sync::Arc::clone(&self.workspace_service);
        let lookup = tokio::task::spawn_blocking(move || workspace_service.credential_profiles());
        match tokio::time::timeout(SHELL_CREDENTIAL_LOOKUP_TIMEOUT, lookup).await {
            Ok(Ok(Ok(profiles))) => {
                *self.shell_credential_lookup_cooldown_until.lock().unwrap() = None;
                Some(profiles)
            }
            Ok(Ok(Err(_))) | Ok(Err(_)) | Err(_) => {
                *self.shell_credential_lookup_cooldown_until.lock().unwrap() =
                    Some(std::time::Instant::now() + SHELL_CREDENTIAL_LOOKUP_COOLDOWN);
                None
            }
        }
    }

    async fn provisioning_credential_profiles(
        &self,
        provisioning: &ShellProvisioning,
    ) -> Option<Result<Vec<crate::workspace::CredentialProfile>, String>> {
        provisioning.session.credential_profile_id.as_ref()?;
        Some(
            self.shell_credential_profiles()
                .await
                .ok_or_else(|| "credential profile lookup timed out or failed".to_string()),
        )
    }

    pub(super) async fn shell_provisioning_validation(
        &self,
        provisioning: &ShellProvisioning,
    ) -> ShellProvisioningValidationReport {
        let profiles = self.provisioning_credential_profiles(provisioning).await;
        crate::acp::shell_validation::validate_shell_provisioning_for_working_dir_with_profiles(
            provisioning,
            Config::global(),
            &self.workspace_service,
            &self.builtins,
            &self.default_working_folder,
            None,
            profiles,
        )
        .await
    }

    pub(super) async fn shell_provisioning_validation_for_working_dir(
        &self,
        provisioning: &ShellProvisioning,
        working_dir: &std::path::Path,
    ) -> ShellProvisioningValidationReport {
        let profiles = self.provisioning_credential_profiles(provisioning).await;
        crate::acp::shell_validation::validate_shell_provisioning_for_working_dir_with_profiles(
            provisioning,
            Config::global(),
            &self.workspace_service,
            &self.builtins,
            &self.default_working_folder,
            Some(working_dir),
            profiles,
        )
        .await
    }

    /// Resolves the directory a provisioning request should be judged against.
    ///
    /// A caller-supplied directory is canonicalized through the same accepted-directory helper the
    /// session path uses, so validation and session creation cannot disagree about which directory
    /// they mean.
    fn requested_working_dir(
        &self,
        requested: Option<&str>,
    ) -> Result<Option<std::path::PathBuf>, agent_client_protocol::Error> {
        requested
            .map(|requested| {
                crate::acp::shell_directory::accepted_shell_directory(std::path::Path::new(
                    requested,
                ))
                .map_err(|reason| {
                    agent_client_protocol::Error::invalid_params().data(serde_json::json!({
                        "code": "SHELL_DIRECTORY_UNAVAILABLE",
                        "reason": reason,
                    }))
                })
            })
            .transpose()
    }

    pub(super) async fn on_read_shell_provisioning(
        &self,
        request: ShellProvisioningReadRequest,
    ) -> Result<ShellProvisioningReadResponse, agent_client_protocol::Error> {
        let working_dir = self.requested_working_dir(request.working_dir.as_deref())?;
        let provisioning = self.shell_runtime.provisioning().clone();
        let validation = match working_dir.as_deref() {
            Some(working_dir) => {
                self.shell_provisioning_validation_for_working_dir(&provisioning, working_dir)
                    .await
            }
            None => self.shell_provisioning_validation(&provisioning).await,
        };
        Ok(ShellProvisioningReadResponse {
            provisioning,
            validation,
        })
    }

    pub(super) async fn on_validate_shell_provisioning(
        &self,
        request: ShellProvisioningValidateRequest,
    ) -> Result<ShellProvisioningValidateResponse, agent_client_protocol::Error> {
        let working_dir = self.requested_working_dir(request.working_dir.as_deref())?;
        let provisioning = request
            .provisioning
            .unwrap_or_else(|| self.shell_runtime.provisioning().clone());
        let validation = match working_dir.as_deref() {
            Some(working_dir) => {
                self.shell_provisioning_validation_for_working_dir(&provisioning, working_dir)
                    .await
            }
            None => self.shell_provisioning_validation(&provisioning).await,
        };
        Ok(ShellProvisioningValidateResponse {
            provisioning,
            validation,
        })
    }

    pub(super) fn on_validate_shell_directory(
        &self,
        request: ShellDirectoryValidateRequest,
    ) -> ShellDirectoryValidateResponse {
        crate::acp::shell_directory::canonicalize_shell_directory(&request.path)
    }

    pub(super) async fn on_list_shell_credentials(&self) -> ShellCredentialListResponse {
        if self.shell_runtime.credential_policy() != ShellCredentialPolicy::SelectableCatalog {
            return ShellCredentialListResponse {
                status: ShellCredentialCatalogStatus::Denied,
                profiles: Vec::new(),
            };
        }
        let Some(profiles) = self.shell_credential_profiles().await else {
            return ShellCredentialListResponse {
                status: ShellCredentialCatalogStatus::Unavailable,
                profiles: Vec::new(),
            };
        };
        let provider = self.shell_runtime.provisioning().session.provider.clone();
        ShellCredentialListResponse {
            status: ShellCredentialCatalogStatus::Available,
            profiles: crate::acp::shell_validation::shell_credential_summaries(
                &profiles,
                provider.as_deref(),
            ),
        }
    }

    pub(super) async fn on_list_shell_modules(
        &self,
        request: ShellModuleListRequest,
    ) -> Result<ShellModuleListResponse, agent_client_protocol::Error> {
        let provisioning = self.shell_runtime.provisioning().clone();
        let working_dir = match request.working_dir.as_deref() {
            Some(requested) => crate::acp::shell_directory::accepted_shell_directory(
                std::path::Path::new(requested),
            )
            .map_err(|reason| {
                agent_client_protocol::Error::invalid_params().data(serde_json::json!({
                    "code": "SHELL_DIRECTORY_UNAVAILABLE",
                    "reason": reason,
                }))
            })?,
            None => self.default_working_folder.clone(),
        };
        // The provisioned selection is the "selected" side of the intersection, not the validation
        // resolution: the resolution has already dropped anything the backend could not resolve,
        // which is exactly the case that must surface as `unavailable`.
        let selected_extensions = provisioning.session.extensions.clone().unwrap_or_default();
        let selected_skills = provisioning.session.skill_ids.clone().unwrap_or_default();

        let mut available_extensions = std::collections::HashSet::new();
        for extension in crate::config::extensions::get_enabled_extensions_with_config_for_cwd(
            Config::global(),
            &working_dir,
        ) {
            available_extensions.insert(extension.name().to_string());
        }
        for extension in super::selected_builtin_extensions(Config::global(), &self.builtins) {
            available_extensions.insert(extension.name().to_string());
        }
        // Provisioning validation and session construction both resolve plugin-supplied MCP
        // servers, so omitting them here would report a plugin extension as unavailable while the
        // session actually runs it.
        for extension in crate::plugins::mcp_servers::enabled_plugin_mcp_servers(Some(&working_dir))
        {
            available_extensions.insert(extension.name().to_string());
        }
        let available_skills = crate::skills::discover_skills(Some(&working_dir))
            .into_iter()
            .map(|skill| skill.name)
            .collect::<std::collections::HashSet<_>>();
        let skills_extension_available = available_extensions.contains("skills");
        let adapter = self.shell_runtime.domain_adapter_descriptor();

        Ok(ShellModuleListResponse {
            contract_version: SHELL_MODULE_CONTRACT_VERSION,
            modules: crate::acp::shell_modules::resolve_shell_modules(
                crate::acp::shell_modules::ShellModuleInputs {
                    session_capabilities: &["prompt".to_string(), "resume".to_string()],
                    selected_extensions: &selected_extensions,
                    available_extensions: &available_extensions,
                    selected_skills: &selected_skills,
                    available_skills: &available_skills,
                    skills_extension_available,
                    adapter: adapter.as_ref(),
                    adapter_status: self.shell_runtime.domain_adapter_status(),
                },
            ),
        })
    }

    pub(super) async fn on_list_shell_artifacts(
        &self,
        request: ShellArtifactListRequest,
    ) -> Result<ShellArtifactListResponse, agent_client_protocol::Error> {
        self.require_active_shell_session(
            &request.session_id,
            "SHELL_SESSION_INVALID",
            "SHELL_SESSION_UNAVAILABLE",
        )
        .await?;
        let page = self
            .session_manager
            .list_session_artifacts(&request.session_id, None, SHELL_ARTIFACT_LIMIT)
            .await
            .map_err(|error| {
                agent_client_protocol::Error::internal_error().data(error.to_string())
            })?;
        Ok(ShellArtifactListResponse {
            truncated: page.next_cursor.is_some(),
            total_count: page.total_count,
            artifacts: page
                .artifacts
                .into_iter()
                .map(shell_artifact_summary)
                .collect(),
        })
    }

    pub(super) async fn on_domain_snapshot(
        &self,
        request: DomainSnapshotRequest,
    ) -> Result<DomainSnapshotResponse, agent_client_protocol::Error> {
        self.shell_runtime.domain_snapshot(request).await
    }

    pub(super) async fn on_domain_action(
        &self,
        request: DomainActionRequest,
    ) -> Result<DomainActionResponse, agent_client_protocol::Error> {
        self.require_active_shell_session(
            &request.session_id,
            "DOMAIN_SESSION_INVALID",
            "DOMAIN_SESSION_UNAVAILABLE",
        )
        .await?;
        self.shell_runtime.perform_domain_action(request).await
    }

    pub(super) async fn on_domain_action_confirm(
        &self,
        request: DomainActionConfirmRequest,
    ) -> Result<DomainActionConfirmResponse, agent_client_protocol::Error> {
        self.require_active_shell_session(
            &request.session_id,
            "DOMAIN_SESSION_INVALID",
            "DOMAIN_SESSION_UNAVAILABLE",
        )
        .await?;
        self.shell_runtime.confirm_domain_action(request).await
    }

    async fn require_active_shell_session(
        &self,
        session_id: &str,
        invalid_code: &str,
        unavailable_code: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        if session_id.is_empty() || session_id.len() > 512 {
            return Err(agent_client_protocol::Error::invalid_params()
                .data(serde_json::json!({ "code": invalid_code })));
        }
        if self.sessions.lock().await.contains_key(session_id) {
            return Ok(());
        }
        Err(agent_client_protocol::Error::invalid_params()
            .data(serde_json::json!({ "code": unavailable_code })))
    }

    pub(super) fn on_prepare_shell_handoff(
        &self,
        request: ShellHandoffPrepareRequest,
    ) -> ShellHandoffPrepareResponse {
        ShellHandoffPrepareResponse {
            handoff: self.shell_runtime.prepare_handoff(request),
        }
    }
}

fn shell_artifact_summary(artifact: SessionArtifact) -> ShellArtifactSummary {
    let file_name = std::path::Path::new(&artifact.resolved_path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Output");
    let name = sanitize_unicode_tags(file_name)
        .chars()
        .take(SHELL_ARTIFACT_NAME_LIMIT)
        .collect::<String>();
    let extension = std::path::Path::new(&name)
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    let mime_type = artifact.mime_type.as_deref().unwrap_or_default();
    let kind = if mime_type.starts_with("image/") {
        ShellArtifactKind::Image
    } else if mime_type == "application/pdf"
        || matches!(
            extension.as_deref(),
            Some("pdf" | "doc" | "docx" | "ppt" | "pptx")
        )
    {
        ShellArtifactKind::Document
    } else if matches!(
        extension.as_deref(),
        Some(
            "rs" | "js"
                | "jsx"
                | "ts"
                | "tsx"
                | "py"
                | "go"
                | "java"
                | "c"
                | "h"
                | "cpp"
                | "hpp"
                | "swift"
                | "kt"
                | "rb"
                | "sh"
                | "css"
                | "html"
                | "sql"
        )
    ) {
        ShellArtifactKind::Code
    } else if mime_type == "application/json"
        || matches!(
            extension.as_deref(),
            Some("csv" | "tsv" | "json" | "xlsx" | "xls")
        )
    {
        ShellArtifactKind::Data
    } else if mime_type.starts_with("text/")
        || matches!(extension.as_deref(), Some("txt" | "md" | "rtf"))
    {
        ShellArtifactKind::Text
    } else {
        ShellArtifactKind::Other
    };
    ShellArtifactSummary {
        name: if name.is_empty() {
            "Output".to_string()
        } else {
            name
        },
        kind,
        relation: match artifact.relation {
            SessionArtifactRelation::Created => ShellArtifactRelation::Created,
            SessionArtifactRelation::Modified => ShellArtifactRelation::Modified,
            SessionArtifactRelation::Referenced => ShellArtifactRelation::Referenced,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_artifact_projection_exposes_only_safe_metadata() {
        let artifact = SessionArtifact {
            session_id: "secret-session".into(),
            display_path: "/private/project/reports/result.pdf".into(),
            resolved_path: "/private/project/reports/result.pdf".into(),
            base_working_dir: "/private/project".into(),
            workspace_id: Some("secret-workspace".into()),
            mime_type: Some("application/pdf".into()),
            relation: SessionArtifactRelation::Created,
            provenance: SessionArtifactProvenance::BuiltInTool,
            source_id: Some("secret-source".into()),
            first_seen_at: chrono::Utc::now(),
            last_seen_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(shell_artifact_summary(artifact)).unwrap();
        assert_eq!(json["name"], "result.pdf");
        assert_eq!(json["kind"], "document");
        assert_eq!(json["relation"], "created");
        assert_eq!(json.as_object().unwrap().len(), 3);
        for forbidden in [
            "sessionId",
            "displayPath",
            "resolvedPath",
            "baseWorkingDir",
            "workspaceId",
            "mimeType",
            "provenance",
            "sourceId",
        ] {
            assert!(json.get(forbidden).is_none());
        }
    }
}
