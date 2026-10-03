//! Explicit session bindings and workspace defaults; disconnect never deletes an account.

use super::*;
use crate::authentication::{session_authentication, SESSION_AUTHENTICATION_KEY};
use crate::config::extensions::name_to_key;

impl GoslingAcpAgent {
    async fn authentication_extensions(
        &self,
        target: &AuthenticationTarget,
    ) -> Result<Vec<ExtensionConfig>, agent_client_protocol::Error> {
        let root = match target {
            AuthenticationTarget::Session { id } => {
                self.session_manager
                    .get_session(id, false)
                    .await
                    .internal_err()?
                    .working_dir
            }
            AuthenticationTarget::Workspace { id } => PathBuf::from(
                self.workspace_service
                    .get(id)
                    .internal_err()?
                    .working_folder,
            ),
        };
        let mut extensions = crate::config::extensions::get_all_extensions()
            .into_iter()
            .map(|entry| entry.config)
            .collect::<Vec<_>>();
        for extension in crate::plugins::mcp_servers::enabled_plugin_mcp_servers(Some(&root)) {
            push_or_replace_extension(&mut extensions, extension);
        }
        if let AuthenticationTarget::Session { id } = target {
            let session = self
                .session_manager
                .get_session(id, false)
                .await
                .internal_err()?;
            for extension in EnabledExtensionsState::extensions_or_default(
                Some(&session.extension_data),
                Config::global(),
            ) {
                push_or_replace_extension(&mut extensions, extension);
            }
        }
        extensions.retain(|extension| {
            matches!(
                extension,
                ExtensionConfig::Stdio { .. } | ExtensionConfig::StreamableHttp { .. }
            )
        });
        extensions.sort_by_key(ExtensionConfig::key);
        extensions.dedup_by_key(|extension| extension.key());
        Ok(extensions)
    }

    pub(super) async fn on_authentication_read(
        &self,
        target: AuthenticationTarget,
    ) -> Result<AuthenticationResponse, agent_client_protocol::Error> {
        let (settings, provider_id, profile_id, profile_name) = match &target {
            AuthenticationTarget::Session { id } => {
                let session = self
                    .session_manager
                    .get_session(id, false)
                    .await
                    .internal_err()?;
                (
                    session_authentication(&session.extension_data).internal_err()?,
                    session.provider_name,
                    session.credential_profile_id,
                    session.credential_profile_name,
                )
            }
            AuthenticationTarget::Workspace { id } => {
                let workspace = self.workspace_service.get(id).internal_err()?;
                let profile_id = workspace
                    .default_credential_binding_id
                    .as_ref()
                    .and_then(|id| {
                        workspace
                            .credential_bindings
                            .iter()
                            .find(|binding| &binding.id == id)
                    })
                    .map(|binding| binding.credential_profile_id.clone());
                let profile = self
                    .workspace_service
                    .credential_profiles()
                    .internal_err()?
                    .into_iter()
                    .find(|profile| Some(&profile.id) == profile_id.as_ref());
                (
                    workspace.authentication,
                    workspace
                        .default_provider
                        .or_else(|| {
                            profile
                                .as_ref()
                                .map(|profile| profile.provider_or_service_id.clone())
                        })
                        .or_else(|| Config::global().get_gosling_provider().ok()),
                    profile_id,
                    profile.map(|profile| profile.name),
                )
            }
        };
        let extensions = self
            .authentication_extensions(&target)
            .await?
            .into_iter()
            .map(|extension| {
                let key = extension.key();
                let name = extension.name();
                let (supports_oauth, secret_fields) = match extension {
                    ExtensionConfig::Stdio { env_keys, .. } => (false, env_keys),
                    ExtensionConfig::StreamableHttp {
                        mut env_keys,
                        headers,
                        socket,
                        client_secret_key,
                        ..
                    } => {
                        if let Some(key) = client_secret_key {
                            if !env_keys.contains(&key) {
                                env_keys.push(key);
                            }
                        }
                        (
                            socket.is_none()
                                && !headers
                                    .keys()
                                    .any(|key| key.eq_ignore_ascii_case("authorization")),
                            env_keys,
                        )
                    }
                    _ => unreachable!(),
                };
                AuthenticationExtensionSummary {
                    key,
                    name,
                    supports_oauth,
                    secret_fields,
                }
            })
            .collect();
        Ok(AuthenticationResponse {
            settings,
            provider_id,
            credential_profile_id: profile_id,
            credential_profile_name: profile_name,
            extensions,
        })
    }

    fn require_authentication_controls(&self) -> Result<(), agent_client_protocol::Error> {
        if self.shell_runtime.is_shell_product() {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("This shell's authentication is managed by its provisioning policy"));
        }
        Ok(())
    }

    pub(super) async fn on_authentication_provider_set(
        &self,
        req: AuthenticationProviderSetRequest,
    ) -> Result<AuthenticationResponse, agent_client_protocol::Error> {
        self.require_authentication_controls()?;
        let _guard = match &req.target {
            AuthenticationTarget::Session { id } => {
                Some(self.queue_provider_transition(id, None, true).await?)
            }
            AuthenticationTarget::Workspace { .. } => None,
        };
        let current = self.on_authentication_read(req.target.clone()).await?;
        let profile = match req.profile_id {
            Some(id) => Some(
                self.workspace_service
                    .listed_credential_profiles()
                    .await
                    .internal_err()?
                    .into_iter()
                    .find(|profile| {
                        profile.id == id
                            && profile.status
                                == crate::workspace::CredentialProfileStatus::Configured
                    })
                    .ok_or_else(|| {
                        agent_client_protocol::Error::invalid_params()
                            .data("Credential profile is missing or requires setup")
                    })?,
            ),
            None => None,
        };
        if let Some(profile) = &profile {
            if current
                .provider_id
                .as_deref()
                .is_some_and(|provider| provider != profile.provider_or_service_id)
            {
                return Err(agent_client_protocol::Error::invalid_params().data("Choose a credential profile for the selected provider; change providers separately"));
            }
        }
        match &req.target {
            AuthenticationTarget::Session { id } => self
                .get_session_agent(id)
                .await?
                .set_session_credential_profile(id, profile)
                .await
                .internal_err()?,
            AuthenticationTarget::Workspace { id } => self
                .workspace_service
                .set_authentication_provider(id, profile.as_ref())
                .await
                .internal_err()?,
        }
        self.on_authentication_read(req.target).await
    }

    pub(super) async fn on_authentication_extension_set(
        &self,
        req: AuthenticationExtensionSetRequest,
    ) -> Result<AuthenticationResponse, agent_client_protocol::Error> {
        self.require_authentication_controls()?;
        let _guard = match &req.target {
            AuthenticationTarget::Session { id } => {
                Some(self.queue_provider_transition(id, None, true).await?)
            }
            AuthenticationTarget::Workspace { .. } => None,
        };
        let current = self.on_authentication_read(req.target.clone()).await?;
        let name = name_to_key(&req.name);
        let extension = self
            .authentication_extensions(&req.target)
            .await?
            .into_iter()
            .find(|extension| extension.key() == name)
            .ok_or_else(|| {
                agent_client_protocol::Error::invalid_params().data("MCP extension not found")
            })?;
        let summary = current
            .extensions
            .iter()
            .find(|entry| name_to_key(&entry.name) == name)
            .ok_or_else(|| {
                agent_client_protocol::Error::invalid_params().data("MCP extension not found")
            })?;
        if !req.connected && (req.sign_in || !req.secret_fields.is_empty()) {
            return Err(agent_client_protocol::Error::invalid_params()
                .data("Disconnect cannot also supply credentials"));
        }
        if req.sign_in && !summary.supports_oauth {
            return Err(agent_client_protocol::Error::invalid_params().data(
                "This extension uses static credentials; configure its declared secret fields",
            ));
        }
        if req.sign_in
            || !req.secret_fields.is_empty()
            || matches!(&req.target, AuthenticationTarget::Session { .. })
        {
            if let Some(provider_id) = &current.provider_id {
                if crate::providers::get_from_registry(provider_id)
                    .await
                    .internal_err()?
                    .executes_tools_outside_gosling()
                {
                    return Err(agent_client_protocol::Error::invalid_params()
                        .data("This provider manages its own MCP connections. Use workspace defaults for future chats, or choose a provider with gosling-managed tools for session authentication."));
                }
            }
        }
        let mut seen = HashSet::new();
        for field in &req.secret_fields {
            if !summary.secret_fields.contains(&field.key)
                || field.value.trim().is_empty()
                || !seen.insert(field.key.clone())
            {
                return Err(agent_client_protocol::Error::invalid_params().data(
                    "Supply non-empty credentials only for this extension's declared secret fields",
                ));
            }
        }
        let mut binding = current
            .settings
            .extensions
            .get(&name)
            .cloned()
            .unwrap_or_default();
        binding.disconnected = !req.connected;
        let working_dir = match &req.target {
            AuthenticationTarget::Session { id } => {
                self.session_manager
                    .get_session(id, false)
                    .await
                    .internal_err()?
                    .working_dir
            }
            AuthenticationTarget::Workspace { id } => PathBuf::from(
                self.workspace_service
                    .get(id)
                    .internal_err()?
                    .working_folder,
            ),
        };
        if req.sign_in || !req.secret_fields.is_empty() {
            let destination =
                crate::authentication::extension_destination(&extension, &working_dir)
                    .internal_err()?;
            if binding.credential_namespace.is_some()
                && binding.destination.as_deref() != Some(destination.as_str())
            {
                binding
                    .secret_fields
                    .retain(|field| summary.secret_fields.contains(field));
                if binding
                    .secret_fields
                    .iter()
                    .any(|field| !req.secret_fields.iter().any(|update| &update.key == field))
                {
                    return Err(agent_client_protocol::Error::invalid_params().data(
                        "The extension destination changed. Supply fresh values for its saved credential fields before reconnecting.",
                    ));
                }
            }
            binding.destination = Some(destination);
        }
        if req.sign_in || !req.secret_fields.is_empty() {
            // Use a fresh account reference so editing this target cannot overwrite a shared account.
            let old_namespace = binding.credential_namespace.clone();
            let namespace = Uuid::now_v7().to_string();
            let mut values = Vec::new();
            for field in &binding.secret_fields {
                if !req.secret_fields.iter().any(|update| &update.key == field) {
                    let old = old_namespace.as_deref().ok_or_else(|| {
                        agent_client_protocol::Error::invalid_params()
                            .data("Scoped account reference is missing")
                    })?;
                    let value = Config::global()
                        .get_secret::<String>(&crate::authentication::secret_key(old, field))
                        .internal_err()?;
                    values.push((
                        crate::authentication::secret_key(&namespace, field),
                        serde_json::Value::String(value),
                    ));
                }
            }
            for field in &req.secret_fields {
                if !binding.secret_fields.contains(&field.key) {
                    binding.secret_fields.push(field.key.clone());
                }
                values.push((
                    crate::authentication::secret_key(&namespace, &field.key),
                    serde_json::Value::String(field.value.clone()),
                ));
            }
            binding.credential_namespace = Some(namespace);
            let mut proposed = current.settings.clone();
            proposed.extensions.insert(name.clone(), binding.clone());
            crate::authentication::validate_settings(&proposed).internal_err()?;
            Config::global().set_secret_values(&values).internal_err()?;
        }
        if req.sign_in {
            let ExtensionConfig::StreamableHttp {
                uri,
                envs,
                env_keys,
                client_id,
                client_secret_key,
                scopes,
                ..
            } = &extension
            else {
                unreachable!()
            };
            let merged = crate::agents::extension_manager::merge_authenticated_environments(
                envs,
                env_keys,
                &name,
                Config::global(),
                &binding,
            )
            .await
            .internal_err()?;
            let uri = crate::agents::extension_manager::substitute_env_vars(uri, &merged);
            let static_client = crate::agents::extension_manager::authentication_static_client(
                client_id.as_deref(),
                client_secret_key.as_deref(),
                scopes,
                &merged,
                &name,
                Config::global(),
            )
            .await
            .internal_err()?;
            crate::oauth::oauth_flow_with_store(
                &uri,
                &extension.name(),
                static_client.as_ref(),
                crate::oauth::SharedCredentialStore::new(Box::new(
                    crate::oauth::GoslingCredentialStore::scoped(
                        binding.credential_namespace.as_deref().unwrap(),
                        &uri,
                    ),
                )),
            )
            .await
            .internal_err()?;
        }
        let mut settings = current.settings;
        settings.extensions.insert(name.clone(), binding.clone());
        crate::authentication::validate_settings(&settings).internal_err()?;
        match &req.target {
            AuthenticationTarget::Workspace { id } => self
                .workspace_service
                .set_extension_authentication(id, extension.name(), binding)
                .await
                .internal_err()?,
            AuthenticationTarget::Session { id } => {
                let agent = self.get_session_agent(id).await?;
                self.session_manager
                    .merge_extension_state(
                        id,
                        SESSION_AUTHENTICATION_KEY,
                        serde_json::to_value(&settings).internal_err()?,
                    )
                    .await
                    .internal_err()?;
                if req.connected {
                    if let Err(error) = agent.add_extension(extension.clone(), id).await {
                        agent
                            .extension_manager
                            .suspend_authenticated_extension(extension)
                            .await
                            .internal_err()?;
                        return Err(agent_client_protocol::Error::internal_error().data(format!(
                            "Authentication was saved, but the extension could not connect: {error}"
                        )));
                    }
                } else {
                    agent
                        .extension_manager
                        .suspend_authenticated_extension(extension)
                        .await
                        .internal_err()?;
                }
            }
        }
        self.on_authentication_read(req.target).await
    }
}
