use crate::acp::custom_requests::{GoslingExtension, ShellCredentialPolicy};
use crate::acp::server::{meta_string, validate_absolute_cwd, ResultExt};
use crate::agents::ExtensionLoadResult;
use crate::config::{Config, GoslingMode};
use crate::session::{DeepResearchState, ExtensionData, ExtensionState, Session, SessionType};
use crate::workspace::{PreparedWorkspaceSession, WorkspaceSessionLaunchOverrides};

use super::GoslingAcpAgent;
use agent_client_protocol::schema::v1::{Meta, NewSessionRequest, NewSessionResponse, SessionId};
use agent_client_protocol::{Client, ConnectionTo};
use gosling_providers::model::ModelConfig;
use gosling_providers::thinking::ThinkingEffort;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::warn;

struct InitialSessionConfig {
    provider: String,
    model_config: ModelConfig,
    gosling_mode: GoslingMode,
    extension_data: ExtensionData,
    project_id: Option<String>,
    workspace: Option<PreparedWorkspaceSession>,
    shell_credential_profile: Option<crate::workspace::CredentialProfile>,
}

struct NewSessionSetup {
    args: NewSessionRequest,
    project_id: Option<String>,
    workspace: Option<PreparedWorkspaceSession>,
    shell_credential_profile: Option<crate::workspace::CredentialProfile>,
}

impl GoslingAcpAgent {
    /// Resolves which credential profile this session launch may use, enforcing the shell policy.
    ///
    /// `fixed` provisioning ignores any caller selection and refuses one outright, so a product
    /// that never opted into catalog selection cannot be switched by an ACP caller.
    fn shell_credential_profile_id(
        &self,
        meta: Option<&Meta>,
    ) -> Result<Option<String>, agent_client_protocol::Error> {
        let selected = meta_string(meta, "shellCredentialProfileId")?;
        if selected.is_some()
            && self.shell_runtime.credential_policy() != ShellCredentialPolicy::SelectableCatalog
        {
            return Err(agent_client_protocol::Error::invalid_params()
                .data(serde_json::json!({ "code": "SHELL_CREDENTIAL_SELECTION_DENIED" })));
        }
        Ok(self
            .shell_runtime
            .effective_credential_profile_id(selected.as_deref()))
    }

    async fn resolve_shell_credential_profile(
        &self,
        meta: Option<&Meta>,
    ) -> Result<Option<crate::workspace::CredentialProfile>, agent_client_protocol::Error> {
        let Some(profile_id) = self.shell_credential_profile_id(meta)? else {
            return Ok(None);
        };
        let profiles = self.shell_credential_profiles().await.ok_or_else(|| {
            agent_client_protocol::Error::invalid_params().data(serde_json::json!({
                "code": "SHELL_CREDENTIAL_CATALOG_UNAVAILABLE"
            }))
        })?;
        let profile = profiles
            .into_iter()
            .find(|profile| profile.id == profile_id)
            .ok_or_else(|| {
                agent_client_protocol::Error::invalid_params().data(serde_json::json!({
                    "code": "SHELL_CREDENTIAL_PROFILE_UNAVAILABLE"
                }))
            })?;
        if profile.status != crate::workspace::CredentialProfileStatus::Configured {
            return Err(
                agent_client_protocol::Error::invalid_params().data(serde_json::json!({
                    "code": "SHELL_CREDENTIAL_PROFILE_UNAVAILABLE"
                })),
            );
        }
        if self
            .shell_runtime
            .provisioning()
            .session
            .provider
            .as_deref()
            .is_some_and(|provider| provider != profile.provider_or_service_id)
        {
            return Err(
                agent_client_protocol::Error::invalid_params().data(serde_json::json!({
                    "code": "SHELL_CREDENTIAL_PROVIDER_MISMATCH"
                })),
            );
        }
        Ok(Some(profile))
    }

    pub(super) async fn handle_new_session(
        &self,
        cx: &ConnectionTo<Client>,
        args: NewSessionRequest,
    ) -> Result<NewSessionResponse, agent_client_protocol::Error> {
        let config = Config::global();
        let project_id = meta_string(args.meta.as_ref(), "projectId")?;
        let workspace_id = meta_string(args.meta.as_ref(), "workspaceId")?.or_else(|| {
            self.shell_runtime
                .provisioning()
                .session
                .workspace_id
                .clone()
        });
        let mut workspace_overrides = workspace_launch_overrides(args.meta.as_ref())?;
        if self.shell_runtime.is_shell_product()
            && workspace_overrides.credential_profile_id.is_some()
        {
            // A shell product's credential authority is its provisioned policy. Honoring the full
            // Gosling per-chat credential override here would let any ACP caller replace a fixed
            // profile, so shells select through `shellCredentialProfileId` or not at all.
            return Err(agent_client_protocol::Error::invalid_params()
                .data(serde_json::json!({ "code": "SHELL_CREDENTIAL_SELECTION_DENIED" })));
        }
        let shell_credential_profile = self
            .resolve_shell_credential_profile(args.meta.as_ref())
            .await?;
        // Without a workspace the shell profile is pinned directly by the shell-specific paths
        // below; putting it here would make a workspace-free launch fail the overrides check.
        if workspace_overrides.credential_profile_id.is_none() && workspace_id.is_some() {
            workspace_overrides.credential_profile_id = shell_credential_profile
                .as_ref()
                .map(|profile| profile.id.clone());
        }
        let workspace = match workspace_id.as_deref() {
            Some(workspace_id) => Some(
                self.workspace_service
                    .prepare_session_with_overrides(workspace_id, &workspace_overrides)
                    .invalid_params_err_ctx("Workspace is unavailable")?,
            ),
            None if !workspace_overrides.is_empty() => {
                return Err(agent_client_protocol::Error::invalid_params()
                    .data("workspace launch overrides require workspaceId"));
            }
            None => None,
        };
        let mut effective_cwd = workspace
            .as_ref()
            .map(|workspace| workspace.working_folder.clone())
            .unwrap_or_else(|| args.cwd.clone());
        validate_absolute_cwd(&effective_cwd)?;
        if self.shell_runtime.is_shell_product() {
            effective_cwd = crate::acp::shell_directory::accepted_shell_directory(&effective_cwd)
                .map_err(|reason| {
                agent_client_protocol::Error::invalid_params().data(serde_json::json!({
                    "code": "SHELL_DIRECTORY_UNAVAILABLE",
                    "reason": reason,
                }))
            })?;
        }
        let shell_validation = self
            .shell_provisioning_validation_for_working_dir(
                self.shell_runtime.provisioning(),
                &effective_cwd,
            )
            .await;
        if !shell_validation.valid {
            return Err(
                agent_client_protocol::Error::invalid_params().data(serde_json::json!({
                    "message": "Shell provisioning is invalid",
                    "validation": shell_validation,
                })),
            );
        }
        let session_type = match meta_string(args.meta.as_ref(), "client")? {
            Some(_) => SessionType::User,
            None => SessionType::Acp,
        };
        let current_mode: GoslingMode = config.get_gosling_mode().unwrap_or_default();

        let session = self
            .session_manager
            .create_session(
                effective_cwd,
                "New Chat".to_string(),
                session_type,
                current_mode,
            )
            .await
            .internal_err_ctx("Failed to create session")?;
        match self
            .finish_new_session_setup(
                cx,
                config,
                &session,
                NewSessionSetup {
                    args,
                    project_id,
                    workspace,
                    shell_credential_profile,
                },
            )
            .await
        {
            Ok(response) => Ok(response),
            Err(error) => {
                self.cleanup_failed_new_session(&session.id).await;
                Err(error)
            }
        }
    }

    async fn finish_new_session_setup(
        &self,
        cx: &ConnectionTo<Client>,
        config: &Config,
        session: &Session,
        setup: NewSessionSetup,
    ) -> Result<NewSessionResponse, agent_client_protocol::Error> {
        self.configure_new_session(config, session, setup).await?;

        let reloaded_session = self.reload_session(&session.id).await?;
        let (_agent, extension_results) = self
            .activate_acp_session(cx, &reloaded_session, HashMap::new())
            .await?;

        let reloaded_session = self.reload_session(&session.id).await?;
        let response = self
            .build_new_session_response(&reloaded_session, &extension_results)
            .await?;
        super::send_session_setup_notifications(
            cx,
            &reloaded_session,
            self.supports_gosling_custom_notifications(),
        )?;
        Ok(response)
    }

    pub(super) async fn cleanup_failed_new_session(&self, session_id: &str) {
        if let Err(error) = self.session_manager.delete_session(session_id).await {
            warn!(
                session_id,
                %error,
                "Failed to delete session during new-session cleanup"
            );
        }
        self.sessions.lock().await.remove(session_id);
        if let Err(error) = self
            .agent_manager
            .remove_session_if_loaded(session_id)
            .await
        {
            warn!(
                session_id,
                %error,
                "Failed to remove in-memory agent during new-session cleanup"
            );
        }
    }

    async fn configure_new_session(
        &self,
        config: &Config,
        session: &Session,
        setup: NewSessionSetup,
    ) -> Result<(), agent_client_protocol::Error> {
        let (provider, model_config) = self
            .resolve_provider_and_model(
                config,
                setup.args.meta.as_ref(),
                setup.workspace.as_ref(),
                setup.shell_credential_profile.as_ref(),
            )
            .await?;
        let executes_tools_outside_gosling = crate::providers::get_from_registry(&provider)
            .await
            .internal_err_ctx("Failed to read provider capabilities")?
            .executes_tools_outside_gosling();
        let gosling_mode =
            super::compatible_mode(session.gosling_mode, executes_tools_outside_gosling);
        let NewSessionSetup {
            args,
            project_id,
            workspace,
            shell_credential_profile,
        } = setup;

        let gosling_extensions = if self
            .shell_runtime
            .provisioning()
            .session
            .extensions
            .is_some()
        {
            None
        } else {
            meta_gosling_extensions(args.meta.as_ref())?
        };
        let deep_research_state = deep_research_state(args.meta.as_ref(), workspace.as_ref())?;
        let mut extension_data = self.build_enabled_extensions_data(
            config,
            session,
            args.mcp_servers,
            gosling_extensions,
        )?;
        if let Some(state) = deep_research_state {
            state
                .to_extension_data(&mut extension_data)
                .internal_err_ctx("Failed to initialize Deep Research state")?;
        }

        self.apply_initial_session_config(
            &session.id,
            InitialSessionConfig {
                provider,
                model_config,
                gosling_mode,
                extension_data,
                project_id,
                workspace,
                shell_credential_profile,
            },
        )
        .await?;

        Ok(())
    }

    async fn reload_session(
        &self,
        session_id: &str,
    ) -> Result<Session, agent_client_protocol::Error> {
        self.session_manager
            .get_session(session_id, false)
            .await
            .internal_err_ctx("Failed to reload session")
    }

    async fn resolve_provider_and_model(
        &self,
        config: &Config,
        meta: Option<&Meta>,
        workspace: Option<&PreparedWorkspaceSession>,
        shell_credential_profile: Option<&crate::workspace::CredentialProfile>,
    ) -> Result<(String, ModelConfig), agent_client_protocol::Error> {
        let shell_session = &self.shell_runtime.provisioning().session;
        if let Some(provider) =
            meta_string(meta, "provider")?.or_else(|| shell_session.provider.clone())
        {
            let mut model_config = if let Some(model) =
                meta_string(meta, "model")?.or_else(|| shell_session.model.clone())
            {
                crate::model_config::model_config_from_user_config(&provider, &model)
                    .invalid_params_err_ctx("Selected model is invalid")?
            } else {
                super::resolve_provider_default_model_config(&provider).await?
            };
            self.validate_model_for_provider(&provider, &model_config.model_name)
                .await?;
            if let Some(workspace) = workspace {
                if let Some(effort) = workspace.thinking_effort {
                    model_config =
                        model_config.with_thinking_effort(provider_thinking_effort(effort));
                }
            }
            return Ok((provider, model_config));
        }
        if let Some(workspace) = workspace {
            if let Some(provider) = workspace.provider.clone() {
                let mut model_config = if let Some(model) = workspace.model.as_deref() {
                    crate::model_config::model_config_from_user_config(&provider, model)
                        .invalid_params_err_ctx("Workspace model is invalid")?
                } else {
                    super::resolve_provider_default_model_config(&provider).await?
                };
                self.validate_model_for_provider(&provider, &model_config.model_name)
                    .await?;
                if let Some(effort) = workspace.thinking_effort {
                    model_config =
                        model_config.with_thinking_effort(provider_thinking_effort(effort));
                }
                return Ok((provider, model_config));
            }
        }
        if let Some(profile) = shell_credential_profile {
            let provider = profile.provider_or_service_id.clone();
            let model_config = if let Some(model) = shell_session.model.as_deref() {
                crate::model_config::model_config_from_user_config(&provider, model)
                    .invalid_params_err_ctx("Shell model is invalid")?
            } else {
                super::resolve_provider_default_model_config(&provider).await?
            };
            self.validate_model_for_provider(&provider, &model_config.model_name)
                .await?;
            return Ok((provider, model_config));
        }
        let (provider, model_config) = match meta_string(meta, "provider")? {
            Some(provider) => {
                let model_config = super::resolve_provider_default_model_config(&provider).await?;
                (provider, model_config)
            }
            None => super::resolve_default_provider_model_config(config)?,
        };
        self.validate_model_for_provider(&provider, &model_config.model_name)
            .await?;

        Ok((provider, model_config))
    }

    async fn apply_initial_session_config(
        &self,
        session_id: &str,
        config: InitialSessionConfig,
    ) -> Result<(), agent_client_protocol::Error> {
        let mut builder = self
            .session_manager
            .update(session_id)
            .provider_name(config.provider)
            .model_config(config.model_config)
            .gosling_mode(config.gosling_mode)
            .extension_data(config.extension_data);
        if let Some(project_id) = config.project_id {
            builder = builder.project_id(Some(project_id));
        }
        if let Some(workspace) = config.workspace {
            builder = builder.workspace_snapshot(
                workspace.workspace_id,
                workspace.workspace_name,
                workspace.credential_profile_id,
                workspace.credential_profile_name,
                workspace.credential_binding_id,
                workspace.context,
            );
        } else if let Some(profile) = config.shell_credential_profile {
            builder = builder.credential_profile_snapshot(profile.id, profile.name);
        }
        builder
            .apply()
            .await
            .internal_err_ctx("Failed to update session")?;
        Ok(())
    }

    async fn build_new_session_response(
        &self,
        session: &Session,
        extension_results: &[ExtensionLoadResult],
    ) -> Result<NewSessionResponse, agent_client_protocol::Error> {
        let (mode_state, config_options) =
            super::build_session_setup_config(&self.provider_inventory, session).await?;

        let mut response =
            NewSessionResponse::new(SessionId::new(session.id.clone())).modes(mode_state);
        if let Some(co) = config_options {
            response = response.config_options(co);
        }
        response = response.meta(super::session_response_meta(session, extension_results));
        super::presentation::ensure_response_fits(response, "New session")
    }
}

fn provider_thinking_effort(effort: crate::workspace::WorkspaceThinkingEffort) -> ThinkingEffort {
    match effort {
        crate::workspace::WorkspaceThinkingEffort::Off => ThinkingEffort::Off,
        crate::workspace::WorkspaceThinkingEffort::Low => ThinkingEffort::Low,
        crate::workspace::WorkspaceThinkingEffort::Medium => ThinkingEffort::Medium,
        crate::workspace::WorkspaceThinkingEffort::High => ThinkingEffort::High,
        crate::workspace::WorkspaceThinkingEffort::Max => ThinkingEffort::Max,
        crate::workspace::WorkspaceThinkingEffort::Ultra => ThinkingEffort::Ultra,
    }
}

fn meta_gosling_extensions(
    meta: Option<&Meta>,
) -> Result<Option<Vec<GoslingExtension>>, agent_client_protocol::Error> {
    let Some(value) = meta.and_then(|m| m.get("enabledExtensions")) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    serde_json::from_value(value.clone())
        .map(Some)
        .map_err(|e| {
            agent_client_protocol::Error::invalid_params().data(format!("enabledExtensions: {e}"))
        })
}

fn workspace_launch_overrides(
    meta: Option<&Meta>,
) -> Result<WorkspaceSessionLaunchOverrides, agent_client_protocol::Error> {
    let additional_folders = match meta.and_then(|value| value.get("workspaceAdditionalFolders")) {
        None | Some(serde_json::Value::Null) => Vec::new(),
        Some(serde_json::Value::Array(values)) => values
            .iter()
            .map(|value| {
                value.as_str().map(PathBuf::from).ok_or_else(|| {
                    agent_client_protocol::Error::invalid_params()
                        .data("workspaceAdditionalFolders must contain only strings")
                })
            })
            .collect::<Result<Vec<_>, _>>()?,
        Some(_) => {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("workspaceAdditionalFolders must be an array"));
        }
    };
    Ok(WorkspaceSessionLaunchOverrides {
        working_folder: meta_string(meta, "workspaceWorkingDir")?.map(PathBuf::from),
        additional_folders,
        credential_profile_id: meta_string(meta, "workspaceCredentialProfileId")?,
    })
}

fn deep_research_state(
    meta: Option<&Meta>,
    workspace: Option<&PreparedWorkspaceSession>,
) -> Result<Option<DeepResearchState>, agent_client_protocol::Error> {
    let Some(requested) = meta_string(meta, "researchLibraryPath")? else {
        return Ok(None);
    };
    let requested = PathBuf::from(requested);
    if !requested.is_absolute() {
        return Err(agent_client_protocol::Error::invalid_params()
            .data("researchLibraryPath must be an absolute path"));
    }
    let canonical = std::fs::canonicalize(&requested).map_err(|_| {
        agent_client_protocol::Error::invalid_params()
            .data("researchLibraryPath must identify an available directory")
    })?;
    if !canonical.is_dir() {
        return Err(agent_client_protocol::Error::invalid_params()
            .data("researchLibraryPath must identify a directory"));
    }
    let Some(workspace) = workspace else {
        return Err(agent_client_protocol::Error::invalid_params()
            .data("researchLibraryPath requires a workspace session"));
    };
    let listed_as_additional_folder = meta
        .and_then(|value| value.get("workspaceAdditionalFolders"))
        .and_then(|value| value.as_array())
        .is_some_and(|folders| {
            folders.iter().any(|folder| {
                folder
                    .as_str()
                    .and_then(|folder| std::fs::canonicalize(folder).ok())
                    .is_some_and(|folder| folder == canonical)
            })
        });
    let granted_by_workspace = workspace
        .context
        .folder_policy
        .roots
        .iter()
        .any(|root| PathBuf::from(&root.path) == canonical);
    if !listed_as_additional_folder
        || !granted_by_workspace
        || canonical == workspace.working_folder
    {
        return Err(agent_client_protocol::Error::invalid_params()
            .data("researchLibraryPath must be an explicitly granted additional session folder"));
    }
    let output_paths = workspace
        .context
        .product_output_folders
        .iter()
        .map(|output| {
            std::fs::canonicalize(&output.path).map_err(|_| {
                agent_client_protocol::Error::invalid_params()
                    .data("workspace output folders must be available directories")
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if output_paths.is_empty() || output_paths.iter().any(|path| !path.is_dir()) {
        return Err(agent_client_protocol::Error::invalid_params()
            .data("workspace output folders must be available directories"));
    }
    Ok(Some(DeepResearchState {
        library_path: canonical.to_string_lossy().into_owned(),
        output_paths: output_paths
            .into_iter()
            .map(|path| path.to_string_lossy().into_owned())
            .collect(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::WorkspaceThinkingEffort;

    #[test]
    fn workspace_thinking_effort_maps_without_global_state() {
        assert_eq!(
            provider_thinking_effort(WorkspaceThinkingEffort::Medium),
            ThinkingEffort::Medium
        );
        assert_eq!(
            provider_thinking_effort(WorkspaceThinkingEffort::Ultra),
            ThinkingEffort::Ultra
        );
    }

    #[test]
    fn workspace_launch_overrides_accept_explicit_session_scope() {
        let meta = serde_json::Map::from_iter([
            (
                "workspaceWorkingDir".to_string(),
                serde_json::Value::String("/workspace/feature".into()),
            ),
            (
                "workspaceCredentialProfileId".to_string(),
                serde_json::Value::String("profile-1".into()),
            ),
            (
                "workspaceAdditionalFolders".to_string(),
                serde_json::json!(["/workspace/reference", "/workspace/design"]),
            ),
        ]);

        let overrides = workspace_launch_overrides(Some(&meta)).unwrap();

        assert_eq!(
            overrides.working_folder,
            Some(PathBuf::from("/workspace/feature"))
        );
        assert_eq!(
            overrides.credential_profile_id.as_deref(),
            Some("profile-1")
        );
        assert_eq!(
            overrides.additional_folders,
            vec![
                PathBuf::from("/workspace/reference"),
                PathBuf::from("/workspace/design"),
            ]
        );
    }

    #[test]
    fn workspace_launch_overrides_reject_non_string_folder_values() {
        let meta = serde_json::Map::from_iter([(
            "workspaceAdditionalFolders".to_string(),
            serde_json::json!(["/workspace/reference", 42]),
        )]);

        assert!(workspace_launch_overrides(Some(&meta)).is_err());
    }

    #[test]
    fn deep_research_state_rejects_ungranted_library_path() {
        let root = tempfile::tempdir().unwrap();
        let library = root.path().join("library");
        std::fs::create_dir(&library).unwrap();
        let meta = serde_json::Map::from_iter([(
            "researchLibraryPath".to_string(),
            serde_json::Value::String(library.to_string_lossy().into_owned()),
        )]);

        let error = deep_research_state(Some(&meta), None).unwrap_err();

        assert!(format!("{error:?}").contains("requires a workspace session"));
    }
}
