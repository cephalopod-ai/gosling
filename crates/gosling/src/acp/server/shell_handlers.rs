use super::*;
use base64::Engine as _;
use std::io::Read as _;

const SHELL_CREDENTIAL_LOOKUP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);
const SHELL_ARTIFACT_LIMIT: usize = 100;
const SHELL_ARTIFACT_NAME_LIMIT: usize = 256;
const SHELL_LIBRARY_NAME_LIMIT: usize = 128;
const SHELL_LIBRARY_TEXT_LIMIT: usize = 256 * 1024;
const SHELL_LIBRARY_FILE_LIMIT: u64 = 20 * 1024 * 1024;
const SHELL_LIBRARY_IMAGE_LIMIT: usize = 5 * 1024 * 1024;
const SHELL_LIBRARY_PROMPT_ITEM_LIMIT: usize = 16;
const SHELL_LIBRARY_PROMPT_TEXT_LIMIT: usize = 512 * 1024;
const SHELL_LIBRARY_PROMPT_IMAGE_LIMIT: usize = 10 * 1024 * 1024;
const SHELL_LIBRARY_PDF_PAGE_LIMIT: usize = 256;
const SHELL_LIBRARY_PDF_OBJECT_LIMIT: usize = 50_000;
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

    pub(super) async fn on_list_shell_library(
        &self,
        request: ShellLibraryListRequest,
    ) -> Result<ShellLibraryListResponse, agent_client_protocol::Error> {
        self.require_library_session(&request.session_id).await?;
        let items = self
            .session_manager
            .list_session_library_items(&request.session_id)
            .await
            .map_err(shell_library_internal_error)?;
        Ok(ShellLibraryListResponse {
            items: items.into_iter().map(shell_library_summary).collect(),
        })
    }

    pub(super) async fn on_add_shell_library_text(
        &self,
        request: ShellLibraryAddTextRequest,
    ) -> Result<ShellLibraryAddResponse, agent_client_protocol::Error> {
        self.require_library_session(&request.session_id).await?;
        let name = validated_library_name(&request.name)?;
        let text = sanitize_unicode_tags(&request.text);
        if text.trim().is_empty() || text.len() > SHELL_LIBRARY_TEXT_LIMIT {
            return Err(shell_library_invalid("SHELL_LIBRARY_TEXT_INVALID"));
        }
        let item = self
            .session_manager
            .add_session_library_item(
                &request.session_id,
                session_library_scope(request.scope),
                name,
                NewSessionLibraryContent::Text(text),
            )
            .await
            .map_err(shell_library_internal_error)?;
        Ok(ShellLibraryAddResponse {
            item: shell_library_summary(item),
        })
    }

    pub(super) async fn on_add_shell_library_image(
        &self,
        request: ShellLibraryAddImageRequest,
    ) -> Result<ShellLibraryAddResponse, agent_client_protocol::Error> {
        self.require_library_session(&request.session_id).await?;
        let name = validated_library_name(&request.name)?;
        if !matches!(
            request.mime_type.as_str(),
            "image/png" | "image/jpeg" | "image/webp" | "image/gif"
        ) {
            return Err(shell_library_invalid("SHELL_LIBRARY_IMAGE_TYPE_INVALID"));
        }
        let decoded = base64::engine::general_purpose::STANDARD
            .decode(&request.data)
            .map_err(|_| shell_library_invalid("SHELL_LIBRARY_IMAGE_INVALID"))?;
        if decoded.is_empty() || decoded.len() > SHELL_LIBRARY_IMAGE_LIMIT {
            return Err(shell_library_invalid("SHELL_LIBRARY_IMAGE_INVALID"));
        }
        if !bytes_match_mime(&decoded, &request.mime_type) {
            return Err(shell_library_invalid("SHELL_LIBRARY_IMAGE_TYPE_INVALID"));
        }
        let item = self
            .session_manager
            .add_session_library_item(
                &request.session_id,
                session_library_scope(request.scope),
                name,
                NewSessionLibraryContent::Image {
                    data: request.data,
                    mime_type: request.mime_type,
                },
            )
            .await
            .map_err(shell_library_internal_error)?;
        Ok(ShellLibraryAddResponse {
            item: shell_library_summary(item),
        })
    }

    pub(super) async fn on_link_shell_library_file(
        &self,
        request: ShellLibraryLinkFileRequest,
    ) -> Result<ShellLibraryAddResponse, agent_client_protocol::Error> {
        self.require_library_session(&request.session_id).await?;
        let requested = std::path::Path::new(&request.path);
        if !requested.is_absolute() {
            return Err(shell_library_invalid("SHELL_LIBRARY_FILE_INVALID"));
        }
        let path = fs::canonicalize(requested)
            .map_err(|_| shell_library_invalid("SHELL_LIBRARY_FILE_UNAVAILABLE"))?;
        let metadata = fs::metadata(&path)
            .map_err(|_| shell_library_invalid("SHELL_LIBRARY_FILE_UNAVAILABLE"))?;
        if !metadata.is_file() || metadata.len() == 0 || metadata.len() > SHELL_LIBRARY_FILE_LIMIT {
            return Err(shell_library_invalid("SHELL_LIBRARY_FILE_INVALID"));
        }
        let mime_type = shell_library_file_mime_type(&path)
            .ok_or_else(|| shell_library_invalid("SHELL_LIBRARY_FILE_TYPE_INVALID"))?;
        if mime_type.starts_with("image/") && metadata.len() > SHELL_LIBRARY_IMAGE_LIMIT as u64 {
            return Err(shell_library_invalid("SHELL_LIBRARY_FILE_INVALID"));
        }
        let name = path
            .file_name()
            .and_then(|value| value.to_str())
            .ok_or_else(|| shell_library_invalid("SHELL_LIBRARY_FILE_INVALID"))?;
        let name = validated_library_name(name)?;
        let item = self
            .session_manager
            .add_session_library_item(
                &request.session_id,
                session_library_scope(request.scope),
                name,
                NewSessionLibraryContent::File {
                    path: path.to_string_lossy().into_owned(),
                    mime_type: mime_type.to_string(),
                },
            )
            .await
            .map_err(shell_library_internal_error)?;
        Ok(ShellLibraryAddResponse {
            item: shell_library_summary(item),
        })
    }

    pub(super) async fn on_remove_shell_library_item(
        &self,
        request: ShellLibraryRemoveRequest,
    ) -> Result<ShellLibraryRemoveResponse, agent_client_protocol::Error> {
        self.require_library_session(&request.session_id).await?;
        if request.item_id.is_empty() || request.item_id.len() > 128 {
            return Err(shell_library_invalid("SHELL_LIBRARY_ITEM_INVALID"));
        }
        let removed = self
            .session_manager
            .remove_session_library_item(&request.session_id, &request.item_id)
            .await
            .map_err(shell_library_internal_error)?;
        Ok(ShellLibraryRemoveResponse { removed })
    }

    pub(super) async fn on_resolve_shell_library(
        &self,
        request: ShellLibraryResolveRequest,
    ) -> Result<ShellLibraryResolveResponse, agent_client_protocol::Error> {
        self.require_library_session(&request.session_id).await?;
        if request.item_ids.is_empty()
            || request.item_ids.len() > SHELL_LIBRARY_PROMPT_ITEM_LIMIT
            || request
                .item_ids
                .iter()
                .any(|item_id| item_id.is_empty() || item_id.len() > 128)
            || request.item_ids.iter().collect::<HashSet<_>>().len() != request.item_ids.len()
        {
            return Err(shell_library_invalid("SHELL_LIBRARY_SELECTION_INVALID"));
        }
        let stored = self
            .session_manager
            .get_session_library_items(&request.session_id, &request.item_ids)
            .await
            .map_err(|_| shell_library_invalid("SHELL_LIBRARY_ITEM_UNAVAILABLE"))?;
        let mut text_bytes = 0usize;
        let mut image_bytes = 0usize;
        let mut items = Vec::with_capacity(stored.len());
        for item in stored {
            let resolved = resolve_shell_library_item(&item)
                .map_err(|_| shell_library_invalid("SHELL_LIBRARY_ITEM_UNAVAILABLE"))?;
            text_bytes = text_bytes.saturating_add(resolved.text_bytes);
            image_bytes = image_bytes.saturating_add(resolved.image_bytes);
            if text_bytes > SHELL_LIBRARY_PROMPT_TEXT_LIMIT
                || image_bytes > SHELL_LIBRARY_PROMPT_IMAGE_LIMIT
            {
                return Err(shell_library_invalid("SHELL_LIBRARY_SELECTION_TOO_LARGE"));
            }
            items.push(ShellLibraryResolvedItem {
                id: item.id,
                name: item.name,
                content: resolved.content,
            });
        }
        Ok(ShellLibraryResolveResponse { items })
    }

    async fn require_library_session(
        &self,
        session_id: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        self.require_active_shell_session(
            session_id,
            "SHELL_LIBRARY_SESSION_INVALID",
            "SHELL_LIBRARY_SESSION_UNAVAILABLE",
        )
        .await
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

fn shell_library_invalid(code: &str) -> agent_client_protocol::Error {
    agent_client_protocol::Error::invalid_params().data(serde_json::json!({ "code": code }))
}

fn shell_library_internal_error(error: anyhow::Error) -> agent_client_protocol::Error {
    agent_client_protocol::Error::internal_error().data(error.to_string())
}

fn validated_library_name(name: &str) -> Result<String, agent_client_protocol::Error> {
    let name = sanitize_unicode_tags(name).trim().to_string();
    if name.is_empty() || name.chars().count() > SHELL_LIBRARY_NAME_LIMIT {
        return Err(shell_library_invalid("SHELL_LIBRARY_NAME_INVALID"));
    }
    Ok(name)
}

fn session_library_scope(scope: ShellLibraryScope) -> SessionLibraryScope {
    match scope {
        ShellLibraryScope::Project => SessionLibraryScope::Project,
        ShellLibraryScope::Session => SessionLibraryScope::Session,
    }
}

fn shell_library_summary(item: SessionLibraryItem) -> ShellLibraryItemSummary {
    let status = if item
        .file_path
        .as_deref()
        .is_some_and(|path| !std::path::Path::new(path).is_file())
    {
        ShellLibraryItemStatus::Missing
    } else {
        ShellLibraryItemStatus::Available
    };
    ShellLibraryItemSummary {
        id: item.id,
        name: item.name,
        kind: match item.kind {
            SessionLibraryItemKind::Text => ShellLibraryItemKind::Text,
            SessionLibraryItemKind::Image => ShellLibraryItemKind::Image,
            SessionLibraryItemKind::File => ShellLibraryItemKind::File,
        },
        scope: match item.scope {
            SessionLibraryScope::Project => ShellLibraryScope::Project,
            SessionLibraryScope::Session => ShellLibraryScope::Session,
        },
        status,
        mime_type: item.mime_type,
        size_bytes: item.size_bytes,
    }
}

fn shell_library_file_mime_type(path: &std::path::Path) -> Option<&'static str> {
    let extension_mime = match path
        .extension()
        .and_then(|value| value.to_str())?
        .to_ascii_lowercase()
        .as_str()
    {
        "pdf" => Some("application/pdf"),
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "json" => Some("application/json"),
        "csv" => Some("text/csv"),
        "tsv" => Some("text/tab-separated-values"),
        "md" => Some("text/markdown"),
        "txt" | "rs" | "js" | "jsx" | "ts" | "tsx" | "py" | "go" | "java" | "c" | "h" | "cpp"
        | "hpp" | "swift" | "kt" | "rb" | "sh" | "css" | "html" | "sql" | "toml" | "yaml"
        | "yml" | "xml" => Some("text/plain"),
        _ => None,
    }?;
    let bytes = fs::read(path).ok()?;
    bytes_match_mime(&bytes, extension_mime).then_some(extension_mime)
}

fn bytes_match_mime(bytes: &[u8], mime_type: &str) -> bool {
    match mime_type {
        "application/pdf" => bytes.starts_with(b"%PDF-"),
        "image/png" => bytes.starts_with(b"\x89PNG\r\n\x1a\n"),
        "image/jpeg" => bytes.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a"),
        "image/webp" => bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP",
        "application/json" => serde_json::from_slice::<serde_json::Value>(bytes).is_ok(),
        mime if mime.starts_with("text/") => {
            !bytes.contains(&0) && std::str::from_utf8(bytes).is_ok()
        }
        _ => false,
    }
}

struct ResolvedShellLibraryContent {
    content: ShellLibraryResolvedContent,
    text_bytes: usize,
    image_bytes: usize,
}

fn resolve_shell_library_item(item: &SessionLibraryItem) -> Result<ResolvedShellLibraryContent> {
    match item.kind {
        SessionLibraryItemKind::Text => {
            let text = item
                .text_content
                .as_deref()
                .ok_or_else(|| anyhow::anyhow!("stored text is missing"))?;
            Ok(ResolvedShellLibraryContent {
                content: ShellLibraryResolvedContent::Text {
                    text: wrap_library_text(&item.name, text),
                },
                text_bytes: text.len(),
                image_bytes: 0,
            })
        }
        SessionLibraryItemKind::Image => {
            let data = item
                .image_data
                .clone()
                .ok_or_else(|| anyhow::anyhow!("stored image is missing"))?;
            let bytes = base64::engine::general_purpose::STANDARD.decode(&data)?;
            anyhow::ensure!(
                bytes_match_mime(&bytes, &item.mime_type),
                "image type changed"
            );
            Ok(ResolvedShellLibraryContent {
                content: ShellLibraryResolvedContent::Image {
                    data,
                    mime_type: item.mime_type.clone(),
                },
                text_bytes: 0,
                image_bytes: bytes.len(),
            })
        }
        SessionLibraryItemKind::File => resolve_shell_library_file(item),
    }
}

fn resolve_shell_library_file(item: &SessionLibraryItem) -> Result<ResolvedShellLibraryContent> {
    let path = item
        .file_path
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("linked file path is missing"))?;
    let metadata = fs::metadata(path)?;
    anyhow::ensure!(
        metadata.is_file() && metadata.len() > 0 && metadata.len() <= SHELL_LIBRARY_FILE_LIMIT,
        "linked file is unavailable"
    );
    let detected_mime = shell_library_file_mime_type(std::path::Path::new(path))
        .ok_or_else(|| anyhow::anyhow!("linked file type is invalid"))?;
    anyhow::ensure!(detected_mime == item.mime_type, "linked file type changed");
    if item.mime_type.starts_with("image/") {
        let bytes = fs::read(path)?;
        anyhow::ensure!(
            bytes.len() <= SHELL_LIBRARY_IMAGE_LIMIT,
            "image is too large"
        );
        let image_bytes = bytes.len();
        return Ok(ResolvedShellLibraryContent {
            content: ShellLibraryResolvedContent::Image {
                data: base64::engine::general_purpose::STANDARD.encode(bytes),
                mime_type: item.mime_type.clone(),
            },
            text_bytes: 0,
            image_bytes,
        });
    }
    let text = if item.mime_type == "application/pdf" {
        extract_bounded_pdf_text(std::path::Path::new(path))?
    } else {
        let mut file = fs::File::open(path)?;
        let mut bytes = Vec::new();
        file.by_ref()
            .take((SHELL_LIBRARY_PROMPT_TEXT_LIMIT + 1) as u64)
            .read_to_end(&mut bytes)?;
        String::from_utf8(bytes)?
    };
    let text = truncate_library_text(text);
    let text_bytes = text.len().min(SHELL_LIBRARY_PROMPT_TEXT_LIMIT);
    Ok(ResolvedShellLibraryContent {
        content: ShellLibraryResolvedContent::Text {
            text: wrap_library_text(&item.name, &text),
        },
        text_bytes,
        image_bytes: 0,
    })
}

fn extract_bounded_pdf_text(path: &std::path::Path) -> Result<String> {
    let document = lopdf::Document::load(path)?;
    let pages = document.get_pages();
    ensure_pdf_complexity(document.objects.len(), pages.len())?;

    let mut text = String::new();
    for page in pages.keys().copied() {
        let page_text = document.extract_text(&[page])?;
        if text.len().saturating_add(page_text.len()) > SHELL_LIBRARY_PROMPT_TEXT_LIMIT {
            let remaining = SHELL_LIBRARY_PROMPT_TEXT_LIMIT.saturating_sub(text.len());
            let mut boundary = remaining.min(page_text.len());
            while !page_text.is_char_boundary(boundary) {
                boundary -= 1;
            }
            text.push_str(&page_text[..boundary]);
            text.push_str("\n[Content truncated]");
            break;
        }
        text.push_str(&page_text);
    }
    Ok(text)
}

fn ensure_pdf_complexity(object_count: usize, page_count: usize) -> Result<()> {
    anyhow::ensure!(
        object_count <= SHELL_LIBRARY_PDF_OBJECT_LIMIT,
        "PDF has too many objects"
    );
    anyhow::ensure!(
        page_count <= SHELL_LIBRARY_PDF_PAGE_LIMIT,
        "PDF has too many pages"
    );
    Ok(())
}

fn truncate_library_text(mut text: String) -> String {
    if text.len() <= SHELL_LIBRARY_PROMPT_TEXT_LIMIT {
        return text;
    }
    let mut boundary = SHELL_LIBRARY_PROMPT_TEXT_LIMIT;
    while !text.is_char_boundary(boundary) {
        boundary -= 1;
    }
    text.truncate(boundary);
    text.push_str("\n[Content truncated]");
    text
}

fn wrap_library_text(name: &str, text: &str) -> String {
    format!("[Library item: {}]\n{}", sanitize_unicode_tags(name), text)
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

    #[test]
    fn shell_library_projection_omits_payloads_and_paths() {
        let item = SessionLibraryItem {
            id: "lib-one".into(),
            scope: SessionLibraryScope::Project,
            name: "reference.pdf".into(),
            kind: SessionLibraryItemKind::File,
            mime_type: "application/pdf".into(),
            size_bytes: 2048,
            text_content: None,
            image_data: None,
            file_path: Some("/private/project/reference.pdf".into()),
            created_at: chrono::Utc::now(),
        };

        let json = serde_json::to_value(shell_library_summary(item)).unwrap();
        assert_eq!(json["id"], "lib-one");
        assert_eq!(json["name"], "reference.pdf");
        assert_eq!(json["kind"], "file");
        assert_eq!(json["scope"], "project");
        assert_eq!(json["status"], "missing");
        let serialized = json.to_string();
        assert!(!serialized.contains("/private"));
        assert!(!serialized.contains("filePath"));
        assert!(!serialized.contains("textContent"));
        assert!(!serialized.contains("imageData"));
    }

    #[test]
    fn pasted_library_text_resolves_as_labeled_content() {
        let item = SessionLibraryItem {
            id: "lib-text".into(),
            scope: SessionLibraryScope::Session,
            name: "Meeting notes".into(),
            kind: SessionLibraryItemKind::Text,
            mime_type: "text/plain".into(),
            size_bytes: 12,
            text_content: Some("Decide today".into()),
            image_data: None,
            file_path: None,
            created_at: chrono::Utc::now(),
        };

        assert_eq!(
            resolve_shell_library_item(&item).unwrap().content,
            ShellLibraryResolvedContent::Text {
                text: "[Library item: Meeting notes]\nDecide today".into()
            }
        );
    }

    #[test]
    fn linked_file_type_requires_matching_content() {
        let root = tempfile::tempdir().unwrap();
        let disguised_pdf = root.path().join("report.pdf");
        fs::write(&disguised_pdf, "not a PDF").unwrap();
        assert_eq!(shell_library_file_mime_type(&disguised_pdf), None);

        let json = root.path().join("evidence.json");
        fs::write(&json, br#"{"verified":true}"#).unwrap();
        assert_eq!(
            shell_library_file_mime_type(&json),
            Some("application/json")
        );
        fs::write(&json, "{not valid json}").unwrap();
        assert_eq!(shell_library_file_mime_type(&json), None);
    }

    #[test]
    fn rejects_pdf_complexity_before_extraction() {
        ensure_pdf_complexity(SHELL_LIBRARY_PDF_OBJECT_LIMIT, SHELL_LIBRARY_PDF_PAGE_LIMIT)
            .unwrap();
        assert!(ensure_pdf_complexity(SHELL_LIBRARY_PDF_OBJECT_LIMIT + 1, 1)
            .unwrap_err()
            .to_string()
            .contains("too many objects"));
        assert!(ensure_pdf_complexity(1, SHELL_LIBRARY_PDF_PAGE_LIMIT + 1)
            .unwrap_err()
            .to_string()
            .contains("too many pages"));
    }
}
