use super::*;

impl GoslingAcpAgent {
    pub(super) async fn shell_provisioning_validation(
        &self,
        provisioning: &ShellProvisioning,
    ) -> ShellProvisioningValidationReport {
        crate::acp::shell_validation::validate_shell_provisioning(
            provisioning,
            Config::global(),
            &self.workspace_service,
            &self.builtins,
            &self.default_working_folder,
        )
        .await
    }

    pub(super) async fn shell_provisioning_validation_for_working_dir(
        &self,
        provisioning: &ShellProvisioning,
        working_dir: &std::path::Path,
    ) -> ShellProvisioningValidationReport {
        crate::acp::shell_validation::validate_shell_provisioning_for_working_dir(
            provisioning,
            Config::global(),
            &self.workspace_service,
            &self.builtins,
            &self.default_working_folder,
            Some(working_dir),
        )
        .await
    }

    pub(super) async fn on_read_shell_provisioning(&self) -> ShellProvisioningReadResponse {
        let provisioning = self.shell_runtime.provisioning().clone();
        let validation = self.shell_provisioning_validation(&provisioning).await;
        ShellProvisioningReadResponse {
            provisioning,
            validation,
        }
    }

    pub(super) async fn on_validate_shell_provisioning(
        &self,
        request: ShellProvisioningValidateRequest,
    ) -> ShellProvisioningValidateResponse {
        let provisioning = request
            .provisioning
            .unwrap_or_else(|| self.shell_runtime.provisioning().clone());
        let validation = self.shell_provisioning_validation(&provisioning).await;
        ShellProvisioningValidateResponse {
            provisioning,
            validation,
        }
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
        let Ok(profiles) = self.workspace_service.credential_profiles() else {
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
        self.require_active_domain_session(&request.session_id)
            .await?;
        self.shell_runtime.perform_domain_action(request).await
    }

    pub(super) async fn on_domain_action_confirm(
        &self,
        request: DomainActionConfirmRequest,
    ) -> Result<DomainActionConfirmResponse, agent_client_protocol::Error> {
        self.require_active_domain_session(&request.session_id)
            .await?;
        self.shell_runtime.confirm_domain_action(request).await
    }

    async fn require_active_domain_session(
        &self,
        session_id: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        if session_id.is_empty() || session_id.len() > 512 {
            return Err(agent_client_protocol::Error::invalid_params()
                .data(serde_json::json!({ "code": "DOMAIN_SESSION_INVALID" })));
        }
        if self.sessions.lock().await.contains_key(session_id) {
            return Ok(());
        }
        Err(agent_client_protocol::Error::invalid_params()
            .data(serde_json::json!({ "code": "DOMAIN_SESSION_UNAVAILABLE" })))
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
