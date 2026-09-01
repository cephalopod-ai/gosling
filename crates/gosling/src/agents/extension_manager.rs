use anyhow::Result;
#[cfg(unix)]
use axum::http::HeaderValue;
use axum::http::{HeaderMap, HeaderName};
use chrono::{DateTime, Utc};
use futures::stream::{FuturesUnordered, StreamExt};
use futures::Stream;
use futures::{future, FutureExt};
use once_cell::sync::Lazy;
use rmcp::service::{ClientInitializeError, ServiceError};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransportConfig, StreamableHttpError,
};
use rmcp::transport::{
    ConfigureCommandExt, DynamicTransportError, StreamableHttpClientTransport, TokioChildProcess,
};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tempfile::{tempdir, TempDir};
use tokio::io::{AsyncRead, AsyncReadExt, ReadBuf};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

#[cfg(test)]
use super::container::DockerTestContainerGuard;
use super::container::{Container, DockerExecProcess};
use super::extension::{
    ExtensionConfig, ExtensionError, ExtensionInfo, ExtensionResult, PlatformExtensionContext,
    ToolInfo, PLATFORM_EXTENSIONS,
};
use super::tool_execution::{ToolCallContext, ToolCallResult};
use super::types::SharedProvider;
use crate::action_required_manager::ActionRequiredManager;
use crate::agents::extension::{Envs, ProcessExit};
use crate::agents::extension_malware_check;
use crate::agents::mcp_client::{
    GoslingMcpClientCapabilities, GoslingMcpHostInfo, McpClient, McpClientTrait,
};
use crate::builtin_extension::get_builtin_extension;
use crate::config::extensions::name_to_key;
use crate::config::search_path::SearchPaths;
use crate::config::{get_all_extensions, AdapterRegistration, CodeExecutionRuntime, Config};
use crate::oauth::{oauth_flow, GoslingCredentialStore, StaticOAuthClientConfig};
use crate::prompt_template;
use crate::subprocess::{configure_shell_owned_subprocess, configure_subprocess};
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ErrorCode, ErrorData, GetPromptResult, Meta,
    Prompt, Resource, ResourceContents, ServerInfo, Tool,
};
use rmcp::transport::async_rw::AsyncRwTransport;
use rmcp::transport::auth::{AuthClient, CredentialStore};
use schemars::_private::NoSerialize;
use serde_json::Value;

type McpClientBox = Arc<dyn McpClientTrait>;

mod action_required_stream;
mod child_process;
mod discovery;
mod environment;
mod lifecycle;

use environment::resolve_static_oauth_client;
pub(crate) use environment::{merge_environments, substitute_env_vars};

use child_process::{
    apply_minimal_child_environment, apply_minimal_docker_client_environment, child_process_client,
    minimal_child_environment, resolve_command, write_docker_env_file,
};
mod oauth;
mod operator_stdio;

use oauth::{
    build_streamable_http_client, clear_credentials_on_post_refresh_auth_failure,
    connect_with_auth, create_streamable_http_client, should_attempt_oauth_fallback,
};
mod pagination;
mod prompts;
mod resources;

pub use operator_stdio::{connect_operator_stdio_client, OperatorProcessExit, OperatorStdioClient};
mod tool_catalog;
mod tool_dispatch;
mod tool_metadata;

pub(crate) use tool_metadata::TRUSTED_TOOL_UPDATE_META_KEY;
pub use tool_metadata::{
    get_parameter_names, get_tool_owner, is_first_class_extension, is_hidden_extension,
    GoslingMcpAppToolAttachment,
};
use tool_metadata::{
    get_tool_meta_value, get_tool_resource_uri, insert_trusted_tool_update_meta,
    is_unprefixed_extension, remove_untrusted_mcp_app_meta, require_str_parameter, ResolvedTool,
    TOOL_EXTENSION_META_KEY,
};

use action_required_stream::ActionRequiredStream;

use pagination::{collect_paginated_prompts, collect_paginated_resources, collect_paginated_tools};
#[cfg(test)]
use pagination::{PaginationGuard, MAX_MCP_LIST_ITEMS, MAX_MCP_LIST_PAGES};

fn resolve_timeout(timeout: Option<u64>) -> u64 {
    timeout.unwrap_or_else(|| {
        Config::global()
            .get_gosling_default_extension_timeout()
            .unwrap_or(crate::config::DEFAULT_EXTENSION_TIMEOUT)
    })
}

struct Extension {
    pub config: ExtensionConfig,
    /// Resolved config snapshot (with secrets from keyring substituted)
    /// captured at client-creation time. Used to detect secret rotation
    /// without re-reading the keyring on every comparison. Only held in
    /// memory — never serialized to disk.
    resolved_config: ExtensionConfig,

    client: McpClientBox,
    server_info: Option<ServerInfo>,
    _temp_dir: Option<tempfile::TempDir>,
    /// Set when this extension's server runs inside a shared Docker
    /// container via `docker exec`. Killing the local `docker exec` client
    /// (which is all `client`'s own drop/cleanup does) does not terminate
    /// the process inside the container, so `Drop` below explicitly does.
    docker_process: Option<DockerExecProcess>,
}

impl Extension {
    fn new(
        config: ExtensionConfig,
        resolved_config: ExtensionConfig,
        client: McpClientBox,
        server_info: Option<ServerInfo>,
        temp_dir: Option<tempfile::TempDir>,
        docker_process: Option<DockerExecProcess>,
    ) -> Self {
        Self {
            client,
            config,
            resolved_config,
            server_info,
            _temp_dir: temp_dir,
            docker_process,
        }
    }

    fn supports_resources(&self) -> bool {
        self.server_info
            .as_ref()
            .and_then(|info| info.capabilities.resources.as_ref())
            .is_some()
    }

    fn get_instructions(&self) -> Option<String> {
        self.client.get_instructions()
    }

    fn get_client(&self) -> McpClientBox {
        self.client.clone()
    }

    async fn shutdown(mut self) {
        // Await the MCP transport's own teardown (stdio child, SSE/HTTP task)
        // before the docker branch below, so callers of `remove_extension`
        // get a real guarantee the extension's resources are gone rather than
        // relying solely on eventual cleanup via `Drop`.
        self.client.close().await;
        if let Some(docker_process) = self.docker_process.take() {
            docker_process.kill().await;
        }
    }
}

impl Drop for Extension {
    fn drop(&mut self) {
        let Some(docker_process) = self.docker_process.take() else {
            return;
        };
        // Drop can run while Tokio is shutting down, so spawning here can
        // silently abandon cleanup. Explicit lifecycle paths await shutdown;
        // this bounded fallback covers eviction, panic, and runtime teardown.
        docker_process.kill_blocking();
    }
}

pub struct ExtensionManagerCapabilities {
    pub mcpui: bool,
    pub host_info: Option<GoslingMcpHostInfo>,
}

/// Manages gosling extensions / MCP clients and their interactions
pub struct ExtensionManager {
    extensions: Mutex<HashMap<String, Extension>>,
    lifecycle_lock: Mutex<()>,
    runtime_blocked_extensions: Mutex<HashMap<String, ExtensionConfig>>,
    context: PlatformExtensionContext,
    provider: SharedProvider,
    tools_cache: Mutex<Option<Arc<Vec<Tool>>>>,
    tools_cache_version: AtomicU64,
    client_name: String,
    capabilities: ExtensionManagerCapabilities,
    code_execution_runtime: CodeExecutionRuntime,
}

/// A flattened representation of a resource used by the agent to prepare inference
#[derive(Debug, Clone)]
pub struct ResourceItem {
    pub extension_name: String, // The name of the extension that owns the resource
    pub uri: String,            // The URI of the resource
    pub name: String,           // The name of the resource
    pub content: String,        // The content of the resource
    pub timestamp: DateTime<Utc>, // The timestamp of the resource
    pub priority: f32,          // The priority of the resource
    pub token_count: Option<u32>, // The token count of the resource (filled in by the agent)
}

impl ResourceItem {
    pub fn new(
        extension_name: String,
        uri: String,
        name: String,
        content: String,
        timestamp: DateTime<Utc>,
        priority: f32,
    ) -> Self {
        Self {
            extension_name,
            uri,
            name,
            content,
            timestamp,
            priority,
            token_count: None,
        }
    }
}

/// Retry with OAuth for typed auth challenges and wrapped bare HTTP 401 responses.

impl ExtensionManager {
    fn mcp_client_capabilities(&self) -> GoslingMcpClientCapabilities {
        GoslingMcpClientCapabilities {
            mcpui: self.capabilities.mcpui,
            host_info: self.capabilities.host_info.clone(),
        }
    }

    pub fn new(
        provider: SharedProvider,
        session_manager: Arc<crate::session::SessionManager>,
        client_name: String,
        capabilities: ExtensionManagerCapabilities,
        use_login_shell_path: bool,
        code_execution_runtime: CodeExecutionRuntime,
    ) -> Self {
        Self {
            extensions: Mutex::new(HashMap::new()),
            lifecycle_lock: Mutex::new(()),
            runtime_blocked_extensions: Mutex::new(HashMap::new()),
            context: PlatformExtensionContext {
                extension_manager: None,
                session_manager,
                session: None,
                use_login_shell_path,
                code_execution_runtime,
            },
            provider,
            tools_cache: Mutex::new(None),
            tools_cache_version: AtomicU64::new(0),
            client_name,
            capabilities,
            code_execution_runtime,
        }
    }

    pub fn new_without_provider(data_dir: std::path::PathBuf) -> Self {
        let session_manager = Arc::new(crate::session::SessionManager::new(data_dir));
        Self::new(
            Arc::new(Mutex::new(None)),
            session_manager,
            "gosling-cli".to_string(),
            ExtensionManagerCapabilities {
                mcpui: false,
                host_info: None,
            },
            false,
            CodeExecutionRuntime::Enabled,
        )
    }

    pub fn get_context(&self) -> &PlatformExtensionContext {
        &self.context
    }

    pub fn get_provider(&self) -> &SharedProvider {
        &self.provider
    }

    pub async fn supports_resources(&self) -> bool {
        self.extensions
            .lock()
            .await
            .values()
            .any(|ext| ext.supports_resources())
    }

    /// Add an extension with an optional working directory.
    /// If working_dir is None, falls back to current_dir.
    #[allow(clippy::too_many_lines)]
    pub async fn add_extension(
        self: &Arc<Self>,
        config: ExtensionConfig,
        working_dir: Option<PathBuf>,
        container: Option<&Container>,
        session_id: Option<&str>,
    ) -> ExtensionResult<()> {
        crate::config::extension_allowlist::enforce_extension(&config)
            .await
            .map_err(|error| ExtensionError::ConfigError(error.to_string()))?;
        let sanitized_name = config.key();

        // Compare both the unresolved config (to detect structural changes like
        // migrating from plaintext envs to env_keys) and the resolved config (to
        // detect secret rotation where only keyring values changed). Only skip
        // restart if both match.
        let resolved_config = config.clone().resolve(Config::global()).await?;

        let _lifecycle_guard = self.lifecycle_lock.lock().await;

        let stopped_docker_extension = {
            let mut extensions = self.extensions.lock().await;
            if let Some(existing) = extensions.get(&sanitized_name) {
                if existing.config == config && existing.resolved_config == resolved_config {
                    return Ok(());
                }
                tracing::debug!(
                    name = sanitized_name,
                    "extension config changed, restarting with updated config"
                );
            }
            if extensions
                .get(&sanitized_name)
                .is_some_and(|extension| extension.docker_process.is_some())
            {
                extensions.remove(&sanitized_name)
            } else {
                None
            }
        };
        if let Some(stopped_docker_extension) = stopped_docker_extension {
            // DockerExecProcess identifies the in-container process by argv,
            // so two identical generations cannot overlap without cleanup
            // risking both. Other transports stay live until replacement
            // creation succeeds.
            stopped_docker_extension.shutdown().await;
            self.invalidate_tools_cache_and_bump_version().await;
        }

        let mut temp_dir = None;
        let mut docker_process = None;

        let effective_working_dir = working_dir
            .clone()
            .or_else(|| std::env::var("GOSLING_WORKING_DIR").ok().map(PathBuf::from))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let client: Box<dyn McpClientTrait> = match &config {
            ExtensionConfig::Sse { .. } => {
                return Err(ExtensionError::ConfigError(
                    "SSE is unsupported, migrate to streamable_http".to_string(),
                ));
            }
            ExtensionConfig::StreamableHttp {
                uri,
                timeout,
                headers,
                name,
                envs,
                env_keys,
                socket,
                client_id,
                client_secret_key,
                scopes,
                ..
            } => {
                let config = Config::global();
                let all_envs = merge_environments(envs, env_keys, &sanitized_name, config).await?;
                let resolved_uri = substitute_env_vars(uri, &all_envs);
                let resolved_headers = headers
                    .iter()
                    .map(|(k, v)| (k.clone(), substitute_env_vars(v, &all_envs)))
                    .collect();
                let resolved_socket = socket.as_ref().map(|s| substitute_env_vars(s, &all_envs));
                let static_oauth_client = resolve_static_oauth_client(
                    client_id.as_deref(),
                    client_secret_key.as_deref(),
                    scopes,
                    &all_envs,
                    config,
                )?;
                create_streamable_http_client(
                    &resolved_uri,
                    *timeout,
                    &resolved_headers,
                    name,
                    resolved_socket.as_deref(),
                    static_oauth_client,
                    Box::new(GoslingCredentialStore::new(name.to_string())),
                    self.provider.clone(),
                    self.client_name.clone(),
                    self.mcp_client_capabilities(),
                    &effective_working_dir,
                )
                .await?
            }
            ExtensionConfig::Builtin { ref name, .. }
            | ExtensionConfig::Platform { ref name, .. } => {
                let timeout = if let ExtensionConfig::Builtin { timeout, .. } = &config {
                    *timeout
                } else {
                    None
                };
                let normalized_name = name_to_key(name);

                if normalized_name == "code_execution" {
                    if !self.code_execution_runtime.is_enabled() {
                        self.runtime_blocked_extensions
                            .lock()
                            .await
                            .insert(sanitized_name.clone(), config.clone());
                        return Err(ExtensionError::ConfigError(
                            "Code execution runtime is disabled by \
                             GOSLING_CODE_EXECUTION_RUNTIME=disabled. Set it to enabled and \
                             restart Gosling to use Code Mode."
                                .to_string(),
                        ));
                    }

                    if !PLATFORM_EXTENSIONS.contains_key(normalized_name.as_str()) {
                        return Err(ExtensionError::ConfigError(
                            "Code Mode (code_execution) is not available in this build. \
                             Rebuild Gosling with the 'code-mode' feature enabled to use it."
                                .to_string(),
                        ));
                    }
                }

                if let Some(def) = PLATFORM_EXTENSIONS.get(normalized_name.as_str()) {
                    // Platform extension: create via in-process client factory
                    let mut context = self.context.clone();
                    context.extension_manager = Some(Arc::downgrade(self));
                    if let Some(id) = session_id {
                        if let Ok(session) =
                            self.context.session_manager.get_session(id, false).await
                        {
                            context.session = Some(Arc::new(session));
                        }
                    }
                    (def.client_factory)(context)
                } else {
                    // Builtin MCP server extension
                    let timeout_secs = resolve_timeout(timeout);
                    let extension_fn =
                        get_builtin_extension(normalized_name.as_str()).ok_or_else(|| {
                            ExtensionError::ConfigError(format!("Unknown extension: {}", name))
                        })?;

                    if let Some(container) = container {
                        let container_id = container.id();
                        tracing::info!(
                            container = %container_id,
                            builtin = %name,
                            "Starting builtin extension inside Docker container"
                        );
                        let in_container_argv = vec![
                            "gosling".to_string(),
                            "mcp".to_string(),
                            normalized_name.clone(),
                        ];
                        docker_process =
                            Some(DockerExecProcess::new(container, in_container_argv.clone()));
                        let command = Command::new("docker").configure(|command| {
                            apply_minimal_docker_client_environment(command);
                            command.arg("exec").arg("-i").arg(container_id);
                            command.args(&in_container_argv);
                        });

                        let client = child_process_client(
                            command,
                            &Some(timeout_secs),
                            self.provider.clone(),
                            &effective_working_dir,
                            Some(container_id.to_string()),
                            self.client_name.clone(),
                            self.mcp_client_capabilities(),
                        )
                        .await?;
                        Box::new(client)
                    } else {
                        let (server_read, client_write) = tokio::io::duplex(65536);
                        let (client_read, server_write) = tokio::io::duplex(65536);
                        extension_fn(server_read, server_write);

                        Box::new(
                            McpClient::connect(
                                (client_read, client_write),
                                Duration::from_secs(timeout_secs),
                                self.provider.clone(),
                                self.client_name.clone(),
                                self.mcp_client_capabilities(),
                                effective_working_dir.clone(),
                            )
                            .await?,
                        )
                    }
                }
            }
            ExtensionConfig::Stdio {
                cmd,
                args,
                envs,
                env_keys,
                timeout,
                cwd,
                ..
            } => {
                let config = Config::global();
                let mut all_envs =
                    merge_environments(envs, env_keys, &sanitized_name, config).await?;
                let process_working_dir = cwd
                    .as_deref()
                    .map(|raw| {
                        // Match the StreamableHttp arm: a configured cwd may
                        // reference ${VAR} placeholders that only resolve
                        // against the extension's merged environment.
                        let substituted = PathBuf::from(substitute_env_vars(raw, &all_envs));
                        if substituted.is_relative() {
                            effective_working_dir.join(substituted)
                        } else {
                            substituted
                        }
                    })
                    .unwrap_or_else(|| effective_working_dir.clone());

                if let Some(sid) = session_id {
                    all_envs.insert("AGENT_SESSION_ID".to_string(), sid.to_string());
                }

                // Check for malicious packages before launching the process
                extension_malware_check::deny_if_malicious_cmd_args(cmd, args).await?;

                let command = if let Some(container) = container {
                    let container_id = container.id();
                    tracing::info!(
                        container = %container_id,
                        cmd = %cmd,
                        "Starting stdio extension inside Docker container"
                    );
                    let mut in_container_argv = vec![cmd.clone()];
                    in_container_argv.extend(args.iter().cloned());
                    docker_process =
                        Some(DockerExecProcess::new(container, in_container_argv.clone()));
                    // Resolved env can contain keyring secrets; pass them via
                    // an --env-file instead of `-e KEY=VALUE`, which would put
                    // them in argv and be readable via `ps`/`/proc/<pid>/cmdline`.
                    let env_file_dir = tempdir()?;
                    let env_file_path = env_file_dir.path().join("docker-exec.env");
                    write_docker_env_file(&env_file_path, &all_envs)?;
                    temp_dir = Some(env_file_dir);
                    Command::new("docker").configure(|command| {
                        apply_minimal_docker_client_environment(command);
                        command.arg("exec").arg("-i");
                        command.arg("--env-file").arg(&env_file_path);
                        command.arg(container_id);
                        command.args(&in_container_argv);
                    })
                } else {
                    let cmd = resolve_command(cmd);
                    for (key, value) in minimal_child_environment() {
                        all_envs.entry(key).or_insert(value);
                    }
                    Command::new(cmd).configure(|command| {
                        command.env_clear().args(args).envs(all_envs);
                    })
                };

                let client = child_process_client(
                    command,
                    timeout,
                    self.provider.clone(),
                    &process_working_dir,
                    container.map(|c| c.id().to_string()),
                    self.client_name.clone(),
                    self.mcp_client_capabilities(),
                )
                .await?;
                Box::new(client)
            }
            ExtensionConfig::InlinePython {
                name,
                code,
                timeout,
                dependencies,
                ..
            } => {
                // Check for malicious packages before launching the process
                if let Some(deps) = dependencies.as_deref() {
                    extension_malware_check::deny_if_malicious_pypi_dependencies(deps).await?;
                }

                let dir = tempdir()?;
                let file_path = dir.path().join(format!("{}.py", name));
                temp_dir = Some(dir);
                std::fs::write(&file_path, code)?;

                let command = Command::new("uvx").configure(|command| {
                    apply_minimal_child_environment(command);
                    command.arg("--with").arg("mcp");
                    dependencies.iter().flatten().for_each(|dep| {
                        command.arg("--with").arg(dep);
                    });
                    command.arg("python").arg(&file_path);
                });

                let client = child_process_client(
                    command,
                    timeout,
                    self.provider.clone(),
                    &effective_working_dir,
                    container.map(|c| c.id().to_string()),
                    self.client_name.clone(),
                    self.mcp_client_capabilities(),
                )
                .await?;

                Box::new(client)
            }
            ExtensionConfig::Frontend { .. } => {
                return Err(ExtensionError::ConfigError(
                    "Invalid extension type: Frontend extensions cannot be added as server extensions".to_string()
                ));
            }
        };

        let server_info = client.get_info().cloned();

        self.runtime_blocked_extensions
            .lock()
            .await
            .remove(&sanitized_name);

        let replaced = self.extensions.lock().await.insert(
            sanitized_name,
            Extension::new(
                config,
                resolved_config,
                Arc::from(client),
                server_info,
                temp_dir,
                docker_process,
            ),
        );
        if let Some(replaced) = replaced {
            replaced.shutdown().await;
        }
        self.invalidate_tools_cache_and_bump_version().await;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::CallToolResult;
    use rmcp::model::{AnnotateAble, RawResource};
    use rmcp::model::{InitializeResult, JsonObject};
    use rmcp::{object, ServiceError as Error};

    use rmcp::model::ListPromptsResult;
    use rmcp::model::ListResourcesResult;
    use rmcp::model::ListToolsResult;
    use rmcp::model::ReadResourceResult;
    use rmcp::model::ServerNotification;

    use tokio::sync::mpsc;

    #[tokio::test]
    async fn streamable_http_client_enforces_the_extension_timeout() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (_socket, _) = listener.accept().await.unwrap();
            tokio::time::sleep(Duration::from_secs(1)).await;
        });
        let client =
            build_streamable_http_client(HeaderMap::new(), Duration::from_millis(25)).unwrap();

        let error = client
            .get(format!("http://{address}"))
            .send()
            .await
            .unwrap_err();

        assert!(error.is_timeout());
        server.abort();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn minimal_child_environment_drops_inherited_secrets() {
        let mut command = Command::new("sh");
        command.env("GOSLING_INHERITED_SECRET", "secret");
        apply_minimal_child_environment(&mut command);
        let output = command
            .arg("-c")
            .arg("printf %s \"${GOSLING_INHERITED_SECRET-unset}\"")
            .output()
            .await
            .unwrap();

        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), "unset");
    }

    #[test]
    fn docker_env_file_is_deterministic_and_rejects_line_injection() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("extension.env");
        let envs = HashMap::from([
            ("TOKEN".to_string(), "secret".to_string()),
            ("ALPHA".to_string(), "first".to_string()),
        ]);

        write_docker_env_file(&path, &envs).unwrap();
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "ALPHA=first\nTOKEN=secret\n"
        );

        for invalid in [
            HashMap::from([("INJECTED\nOTHER".to_string(), "value".to_string())]),
            HashMap::from([("TOKEN".to_string(), "secret\nOTHER=value".to_string())]),
            HashMap::from([("BAD=NAME".to_string(), "value".to_string())]),
        ] {
            assert_eq!(
                write_docker_env_file(&path, &invalid).unwrap_err().kind(),
                std::io::ErrorKind::InvalidInput
            );
        }
    }

    impl ExtensionManager {
        async fn add_mock_extension(&self, name: String, client: McpClientBox) {
            self.add_mock_extension_with_tools(name, client, vec![])
                .await;
        }

        async fn add_mock_extension_with_tools(
            &self,
            name: String,
            client: McpClientBox,
            available_tools: Vec<String>,
        ) {
            self.add_mock_extension_with_docker_process(name, client, available_tools, None)
                .await;
        }

        async fn add_mock_extension_with_docker_process(
            &self,
            name: String,
            client: McpClientBox,
            available_tools: Vec<String>,
            docker_process: Option<DockerExecProcess>,
        ) {
            let _lifecycle_guard = self.lifecycle_lock.lock().await;
            let sanitized_name = name_to_key(&name);
            let config = ExtensionConfig::Builtin {
                name: name.clone(),
                display_name: Some(name.clone()),
                description: "built-in".to_string(),
                timeout: None,
                bundled: None,
                available_tools,
            };
            let extension = Extension::new(
                config.clone(),
                config.clone(),
                client,
                None,
                None,
                docker_process,
            );
            let replaced = self.extensions.lock().await.remove(&sanitized_name);
            if let Some(replaced) = replaced {
                replaced.shutdown().await;
            }
            self.extensions
                .lock()
                .await
                .insert(sanitized_name, extension);
            self.invalidate_tools_cache_and_bump_version().await;
        }
    }

    async fn docker_available() -> bool {
        tokio::process::Command::new("docker")
            .arg("info")
            .kill_on_drop(true)
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    async fn start_detached_container_process(container_name: &str, argv: &[String], label: &str) {
        let mut last_error = String::new();
        for attempt in 1..=3 {
            let output = tokio::process::Command::new("docker")
                .arg("exec")
                .arg("-d")
                .arg(container_name)
                .args(argv)
                .kill_on_drop(true)
                .output()
                .await
                .expect("failed to invoke docker exec");
            if output.status.success() {
                return;
            }
            last_error = String::from_utf8_lossy(&output.stderr).into_owned();
            if attempt < 3 {
                tokio::time::sleep(Duration::from_millis(100)).await;
            }
        }
        panic!("failed to start {label} in container after retries: {last_error}");
    }

    struct MockClient {}

    struct FailingListToolsClient;

    struct PaginatedDiscoveryClient {
        repeat_tool_cursor: bool,
    }

    #[async_trait::async_trait]
    impl McpClientTrait for PaginatedDiscoveryClient {
        fn get_info(&self) -> Option<&InitializeResult> {
            None
        }

        async fn list_resources(
            &self,
            _session_id: &str,
            next_cursor: Option<String>,
            _cancellation_token: CancellationToken,
        ) -> Result<ListResourcesResult, Error> {
            let (name, next_cursor) = match next_cursor.as_deref() {
                None => ("resource-one", Some("resources-page-two".to_string())),
                Some("resources-page-two") => ("resource-two", None),
                _ => return Err(Error::UnexpectedResponse),
            };
            Ok(ListResourcesResult {
                resources: vec![RawResource::new(format!("ui://{name}"), name).no_annotation()],
                next_cursor,
                meta: None,
            })
        }

        async fn list_tools(
            &self,
            _session_id: &str,
            next_cursor: Option<String>,
            _cancellation_token: CancellationToken,
        ) -> Result<ListToolsResult, Error> {
            let (name, next_cursor) = match next_cursor.as_deref() {
                None => ("tool-one", Some("tools-page-two".to_string())),
                Some("tools-page-two") => (
                    "tool-two",
                    self.repeat_tool_cursor
                        .then(|| "tools-page-two".to_string()),
                ),
                _ => return Err(Error::UnexpectedResponse),
            };
            Ok(ListToolsResult {
                tools: vec![Tool::new(
                    name,
                    format!("{name} description"),
                    Arc::new(JsonObject::new()),
                )],
                next_cursor,
                meta: None,
            })
        }

        async fn call_tool(
            &self,
            _ctx: &ToolCallContext,
            _name: &str,
            _arguments: Option<JsonObject>,
            _cancellation_token: CancellationToken,
        ) -> Result<CallToolResult, Error> {
            Err(Error::UnexpectedResponse)
        }

        async fn list_prompts(
            &self,
            _session_id: &str,
            next_cursor: Option<String>,
            _cancellation_token: CancellationToken,
        ) -> Result<ListPromptsResult, Error> {
            let (name, next_cursor) = match next_cursor.as_deref() {
                None => ("prompt-one", Some("prompts-page-two".to_string())),
                Some("prompts-page-two") => ("prompt-two", None),
                _ => return Err(Error::UnexpectedResponse),
            };
            Ok(ListPromptsResult {
                prompts: vec![Prompt::new(name, Some(format!("{name} description")), None)],
                next_cursor,
                meta: None,
            })
        }
    }

    #[tokio::test]
    async fn paginated_discovery_collects_all_pages() {
        let client: McpClientBox = Arc::new(PaginatedDiscoveryClient {
            repeat_tool_cursor: false,
        });

        let tools = collect_paginated_tools(&client, "session", CancellationToken::new())
            .await
            .unwrap();
        let resources = collect_paginated_resources(&client, "session", CancellationToken::new())
            .await
            .unwrap();
        let prompts = collect_paginated_prompts(&client, "session", CancellationToken::new())
            .await
            .unwrap();

        assert_eq!(
            tools
                .into_iter()
                .map(|tool| tool.name.to_string())
                .collect::<Vec<_>>(),
            vec!["tool-one".to_string(), "tool-two".to_string()]
        );
        assert_eq!(
            resources
                .into_iter()
                .map(|resource| resource.name.clone())
                .collect::<Vec<_>>(),
            vec!["resource-one".to_string(), "resource-two".to_string()]
        );
        assert_eq!(
            prompts
                .into_iter()
                .map(|prompt| prompt.name)
                .collect::<Vec<_>>(),
            vec!["prompt-one".to_string(), "prompt-two".to_string()]
        );
    }

    #[tokio::test]
    async fn paginated_discovery_rejects_repeated_cursor() {
        let client: McpClientBox = Arc::new(PaginatedDiscoveryClient {
            repeat_tool_cursor: true,
        });

        let error = collect_paginated_tools(&client, "session", CancellationToken::new())
            .await
            .unwrap_err();

        assert!(error.contains("repeated cursor"));
    }

    #[test]
    fn pagination_guard_enforces_page_and_item_limits() {
        let mut page_guard = PaginationGuard::default();
        for _ in 0..MAX_MCP_LIST_PAGES {
            page_guard.record_page("tool", 0, None).unwrap();
        }
        assert!(page_guard.record_page("tool", 0, None).is_err());

        let mut item_guard = PaginationGuard::default();
        assert!(item_guard
            .record_page("tool", MAX_MCP_LIST_ITEMS + 1, None)
            .is_err());
    }

    #[async_trait::async_trait]
    impl McpClientTrait for MockClient {
        fn get_info(&self) -> Option<&InitializeResult> {
            None
        }

        async fn list_resources(
            &self,
            _session_id: &str,
            _next_cursor: Option<String>,
            _cancellation_token: CancellationToken,
        ) -> Result<ListResourcesResult, Error> {
            Err(Error::TransportClosed)
        }

        async fn read_resource(
            &self,
            _session_id: &str,
            _uri: &str,
            _cancellation_token: CancellationToken,
        ) -> Result<ReadResourceResult, Error> {
            Err(Error::TransportClosed)
        }

        async fn list_tools(
            &self,
            _session_id: &str,
            _next_cursor: Option<String>,
            _cancellation_token: CancellationToken,
        ) -> Result<ListToolsResult, Error> {
            use serde_json::json;
            use std::sync::Arc;
            Ok(ListToolsResult {
                tools: vec![
                    Tool::new(
                        "tool".to_string(),
                        "A basic tool".to_string(),
                        Arc::new(json!({}).as_object().unwrap().clone()),
                    ),
                    Tool::new(
                        "available_tool".to_string(),
                        "An available tool".to_string(),
                        Arc::new(json!({}).as_object().unwrap().clone()),
                    ),
                    Tool::new(
                        "hidden_tool".to_string(),
                        "hidden tool".to_string(),
                        Arc::new(json!({}).as_object().unwrap().clone()),
                    ),
                ],
                next_cursor: None,
                meta: None,
            })
        }

        async fn call_tool(
            &self,
            _ctx: &ToolCallContext,
            name: &str,
            _arguments: Option<JsonObject>,
            _cancellation_token: CancellationToken,
        ) -> Result<CallToolResult, Error> {
            match name {
                "tool" | "test__tool" | "available_tool" | "hidden_tool" => {
                    Ok(CallToolResult::success(vec![]))
                }
                _ => Err(Error::TransportClosed),
            }
        }

        async fn list_prompts(
            &self,
            _session_id: &str,
            _next_cursor: Option<String>,
            _cancellation_token: CancellationToken,
        ) -> Result<ListPromptsResult, Error> {
            Err(Error::TransportClosed)
        }

        async fn get_prompt(
            &self,
            _session_id: &str,
            _name: &str,
            _arguments: Value,
            _cancellation_token: CancellationToken,
        ) -> Result<GetPromptResult, Error> {
            Err(Error::TransportClosed)
        }

        async fn subscribe(&self) -> mpsc::Receiver<ServerNotification> {
            mpsc::channel(1).1
        }
    }

    #[async_trait::async_trait]
    impl McpClientTrait for FailingListToolsClient {
        fn get_info(&self) -> Option<&InitializeResult> {
            None
        }

        async fn list_resources(
            &self,
            _session_id: &str,
            _next_cursor: Option<String>,
            _cancellation_token: CancellationToken,
        ) -> Result<ListResourcesResult, Error> {
            Err(Error::TransportClosed)
        }

        async fn read_resource(
            &self,
            _session_id: &str,
            _uri: &str,
            _cancellation_token: CancellationToken,
        ) -> Result<ReadResourceResult, Error> {
            Err(Error::TransportClosed)
        }

        async fn list_tools(
            &self,
            _session_id: &str,
            _next_cursor: Option<String>,
            _cancellation_token: CancellationToken,
        ) -> Result<ListToolsResult, Error> {
            Err(Error::TransportClosed)
        }

        async fn call_tool(
            &self,
            _ctx: &ToolCallContext,
            _name: &str,
            _arguments: Option<JsonObject>,
            _cancellation_token: CancellationToken,
        ) -> Result<CallToolResult, Error> {
            Err(Error::TransportClosed)
        }

        async fn list_prompts(
            &self,
            _session_id: &str,
            _next_cursor: Option<String>,
            _cancellation_token: CancellationToken,
        ) -> Result<ListPromptsResult, Error> {
            Err(Error::TransportClosed)
        }

        async fn get_prompt(
            &self,
            _session_id: &str,
            _name: &str,
            _arguments: Value,
            _cancellation_token: CancellationToken,
        ) -> Result<GetPromptResult, Error> {
            Err(Error::TransportClosed)
        }

        async fn subscribe(&self) -> mpsc::Receiver<ServerNotification> {
            mpsc::channel(1).1
        }
    }

    fn extension_manager_with_runtime(
        data_dir: std::path::PathBuf,
        runtime: CodeExecutionRuntime,
    ) -> ExtensionManager {
        let session_manager = Arc::new(crate::session::SessionManager::new(data_dir));
        ExtensionManager::new(
            Arc::new(tokio::sync::Mutex::new(None)),
            session_manager,
            "gosling-cli".to_string(),
            ExtensionManagerCapabilities {
                mcpui: false,
                host_info: None,
            },
            false,
            runtime,
        )
    }

    fn platform_extension_config(name: &str) -> ExtensionConfig {
        ExtensionConfig::Platform {
            name: name.to_string(),
            description: name.to_string(),
            display_name: Some(name.to_string()),
            bundled: Some(true),
            available_tools: vec![],
        }
    }

    #[tokio::test]
    async fn test_dispatch_tool_call() {
        use super::super::tool_execution::ToolCallContext;

        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());

        // Add some mock clients using the helper method
        extension_manager
            .add_mock_extension("test_client".to_string(), Arc::new(MockClient {}))
            .await;

        extension_manager
            .add_mock_extension("__cli__ent__".to_string(), Arc::new(MockClient {}))
            .await;

        extension_manager
            .add_mock_extension("client 🚀".to_string(), Arc::new(MockClient {}))
            .await;

        let ctx = ToolCallContext::new(
            "test-session-id".to_string(),
            None,
            Some("test-req-id".to_string()),
        );

        let tool_call =
            CallToolRequestParams::new("test_client__tool".to_string()).with_arguments(object!({}));

        let result = extension_manager
            .dispatch_tool_call(&ctx, tool_call, CancellationToken::default())
            .await;
        assert!(result.is_ok());

        let tool_call = CallToolRequestParams::new("test_client__available_tool".to_string())
            .with_arguments(object!({}));

        let result = extension_manager
            .dispatch_tool_call(&ctx, tool_call, CancellationToken::default())
            .await;
        assert!(result.is_ok());

        let tool_call = CallToolRequestParams::new("__cli__ent____tool".to_string())
            .with_arguments(object!({}));

        let result = extension_manager
            .dispatch_tool_call(&ctx, tool_call, CancellationToken::default())
            .await;
        assert!(result.is_ok());

        let tool_call =
            CallToolRequestParams::new("client___tool".to_string()).with_arguments(object!({}));

        let result = extension_manager
            .dispatch_tool_call(&ctx, tool_call, CancellationToken::default())
            .await;
        assert!(result.is_ok());

        let invalid_tool_call =
            CallToolRequestParams::new("client___tools".to_string()).with_arguments(object!({}));

        let result = extension_manager
            .dispatch_tool_call(&ctx, invalid_tool_call, CancellationToken::default())
            .await;
        if let Err(err) = result {
            let tool_err = err.downcast_ref::<ErrorData>().expect("Expected ErrorData");
            assert_eq!(tool_err.code, ErrorCode::RESOURCE_NOT_FOUND);
        } else {
            panic!("Expected ErrorData with ErrorCode::RESOURCE_NOT_FOUND");
        }

        let invalid_tool_call =
            CallToolRequestParams::new("_client__tools".to_string()).with_arguments(object!({}));

        let result = extension_manager
            .dispatch_tool_call(&ctx, invalid_tool_call, CancellationToken::default())
            .await;
        if let Err(err) = result {
            let tool_err = err.downcast_ref::<ErrorData>().expect("Expected ErrorData");
            assert_eq!(tool_err.code, ErrorCode::RESOURCE_NOT_FOUND);
        } else {
            panic!("Expected ErrorData with ErrorCode::RESOURCE_NOT_FOUND");
        }
    }

    #[tokio::test]
    async fn test_tool_availability_filtering() {
        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());

        // Only "available_tool" should be available to the LLM
        let available_tools = vec!["available_tool".to_string()];

        extension_manager
            .add_mock_extension_with_tools(
                "test_extension".to_string(),
                Arc::new(MockClient {}),
                available_tools,
            )
            .await;

        let tools = extension_manager
            .get_prefixed_tools("test-session-id", None)
            .await
            .unwrap();

        let tool_names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        assert!(!tool_names.iter().any(|name| name == "test_extension__tool")); // Default unavailable
        assert!(tool_names
            .iter()
            .any(|name| name == "test_extension__available_tool"));
        assert!(!tool_names
            .iter()
            .any(|name| name == "test_extension__hidden_tool"));
        assert!(tool_names.len() == 1);
    }

    #[tokio::test]
    async fn test_tool_availability_defaults_to_available() {
        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());

        extension_manager
            .add_mock_extension_with_tools(
                "test_extension".to_string(),
                Arc::new(MockClient {}),
                vec![], // Empty available_tools means all tools are available by default
            )
            .await;

        let tools = extension_manager
            .get_prefixed_tools("test-session-id", None)
            .await
            .unwrap();

        let tool_names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();
        assert!(tool_names.iter().any(|name| name == "test_extension__tool"));
        assert!(tool_names
            .iter()
            .any(|name| name == "test_extension__available_tool"));
        assert!(tool_names
            .iter()
            .any(|name| name == "test_extension__hidden_tool"));
        assert!(tool_names.len() == 3);
    }

    #[tokio::test]
    async fn test_get_prefixed_tools_fails_visible_when_extension_tool_listing_fails() {
        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());

        extension_manager
            .add_mock_extension("healthy".to_string(), Arc::new(MockClient {}))
            .await;
        extension_manager
            .add_mock_extension("broken".to_string(), Arc::new(FailingListToolsClient))
            .await;

        let error = extension_manager
            .get_prefixed_tools("test-session-id", None)
            .await
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("Failed to enumerate extension tools"));
        assert!(error.to_string().contains("broken"));
    }

    #[tokio::test]
    async fn test_dispatch_unavailable_tool_returns_error() {
        use super::super::tool_execution::ToolCallContext;

        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());

        let available_tools = vec!["available_tool".to_string()];

        extension_manager
            .add_mock_extension_with_tools(
                "test_extension".to_string(),
                Arc::new(MockClient {}),
                available_tools,
            )
            .await;

        let ctx = ToolCallContext::new(
            "test-session-id".to_string(),
            None,
            Some("test-req-id".to_string()),
        );

        let unavailable_tool_call = CallToolRequestParams::new("test_extension__tool".to_string())
            .with_arguments(object!({}));

        let result = extension_manager
            .dispatch_tool_call(&ctx, unavailable_tool_call, CancellationToken::default())
            .await;

        if let Err(err) = result {
            let tool_err = err.downcast_ref::<ErrorData>().expect("Expected ErrorData");
            assert_eq!(tool_err.code, ErrorCode::RESOURCE_NOT_FOUND);
        } else {
            panic!("Expected ErrorData with ErrorCode::RESOURCE_NOT_FOUND");
        }

        // Try to call an available tool - should succeed
        let available_tool_call =
            CallToolRequestParams::new("test_extension__available_tool".to_string())
                .with_arguments(object!({}));

        let result = extension_manager
            .dispatch_tool_call(&ctx, available_tool_call, CancellationToken::default())
            .await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_streamable_http_header_env_substitution() {
        let mut env_map = HashMap::new();
        env_map.insert("AUTH_TOKEN".to_string(), "secret123".to_string());
        env_map.insert("API_KEY".to_string(), "key456".to_string());

        // Test ${VAR} syntax
        let result = substitute_env_vars("Bearer ${ AUTH_TOKEN }", &env_map);
        assert_eq!(result, "Bearer secret123");

        // Test ${VAR} syntax without spaces
        let result = substitute_env_vars("Bearer ${AUTH_TOKEN}", &env_map);
        assert_eq!(result, "Bearer secret123");

        // Test $VAR syntax
        let result = substitute_env_vars("Bearer $AUTH_TOKEN", &env_map);
        assert_eq!(result, "Bearer secret123");

        // Test multiple substitutions
        let result = substitute_env_vars("Key: $API_KEY, Token: ${AUTH_TOKEN}", &env_map);
        assert_eq!(result, "Key: key456, Token: secret123");

        // Test no substitution when variable doesn't exist
        let result = substitute_env_vars("Bearer ${UNKNOWN_VAR}", &env_map);
        assert_eq!(result, "Bearer ${UNKNOWN_VAR}");

        // Test mixed content
        let result = substitute_env_vars(
            "Authorization: Bearer ${AUTH_TOKEN} and API ${API_KEY}",
            &env_map,
        );
        assert_eq!(result, "Authorization: Bearer secret123 and API key456");
    }

    #[tokio::test]
    async fn test_substitute_env_vars_no_recursive_expansion() {
        let mut env_map = HashMap::new();
        env_map.insert("TOKEN".to_string(), "abc$KEY".to_string());
        env_map.insert("KEY".to_string(), "xyz".to_string());

        // A substituted value containing $KEY should NOT be re-expanded
        let result = substitute_env_vars("${TOKEN}", &env_map);
        assert_eq!(result, "abc$KEY");

        let result = substitute_env_vars("$TOKEN", &env_map);
        assert_eq!(result, "abc$KEY");
    }

    #[tokio::test]
    async fn test_tools_cache_invalidated_on_add_extension() {
        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());

        extension_manager
            .add_mock_extension("ext_a".to_string(), Arc::new(MockClient {}))
            .await;

        let tools_after_first = extension_manager
            .get_prefixed_tools("test-session-id", None)
            .await
            .unwrap();
        let tool_names: Vec<String> = tools_after_first
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        assert!(tool_names.iter().any(|n| n.starts_with("ext_a__")));
        assert!(!tool_names.iter().any(|n| n.starts_with("ext_b__")));

        extension_manager
            .add_mock_extension("ext_b".to_string(), Arc::new(MockClient {}))
            .await;

        let tools_after_second = extension_manager
            .get_prefixed_tools("test-session-id", None)
            .await
            .unwrap();
        let tool_names: Vec<String> = tools_after_second
            .iter()
            .map(|t| t.name.to_string())
            .collect();
        assert!(tool_names.iter().any(|n| n.starts_with("ext_a__")));
        assert!(tool_names.iter().any(|n| n.starts_with("ext_b__")));
    }

    #[tokio::test]
    async fn test_tools_cache_invalidated_on_remove_extension() {
        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());

        extension_manager
            .add_mock_extension("ext_a".to_string(), Arc::new(MockClient {}))
            .await;
        extension_manager
            .add_mock_extension("ext_b".to_string(), Arc::new(MockClient {}))
            .await;

        let tools_before = extension_manager
            .get_prefixed_tools("test-session-id", None)
            .await
            .unwrap();
        let tool_names: Vec<String> = tools_before.iter().map(|t| t.name.to_string()).collect();
        assert!(tool_names.iter().any(|n| n.starts_with("ext_a__")));
        assert!(tool_names.iter().any(|n| n.starts_with("ext_b__")));

        extension_manager.remove_extension("ext_b").await.unwrap();

        let tools_after = extension_manager
            .get_prefixed_tools("test-session-id", None)
            .await
            .unwrap();
        let tool_names: Vec<String> = tools_after.iter().map(|t| t.name.to_string()).collect();
        assert!(tool_names.iter().any(|n| n.starts_with("ext_a__")));
        assert!(!tool_names.iter().any(|n| n.starts_with("ext_b__")));
    }

    /// RES-003: removing a Docker-backed extension must terminate the
    /// process it started inside the container (the awaited extension
    /// shutdown invoking `DockerExecProcess::kill`), not just the local `docker exec`
    /// client the extension's `client` field owns — and it must not stop
    /// the container itself, since other extensions/sessions may share it.
    /// Requires a real Docker daemon; skips (not fails) if unavailable so
    /// CI environments without Docker aren't broken by this test.
    #[tokio::test]
    async fn test_remove_extension_kills_docker_exec_process_but_not_container() {
        if !docker_available().await {
            eprintln!("skipping: docker is not available in this environment");
            return;
        }

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or_default();
        let container_name = format!("gosling-res003-em-test-{}-{}", std::process::id(), nanos);

        let run = tokio::process::Command::new("docker")
            .args([
                "run",
                "-d",
                "--rm",
                "--name",
                &container_name,
                "busybox",
                "tail",
                "-f",
                "/dev/null",
            ])
            .kill_on_drop(true)
            .output()
            .await
            .expect("failed to invoke docker run");
        if !run.status.success() {
            eprintln!(
                "skipping: could not start busybox test container: {}",
                String::from_utf8_lossy(&run.stderr)
            );
            return;
        }
        let guard = DockerTestContainerGuard::new(container_name.clone());

        let argv = vec!["sleep".to_string(), "300".to_string()];
        start_detached_container_process(&container_name, &argv, "extension process").await;

        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());
        let container = Container::new(container_name.clone());
        extension_manager
            .add_mock_extension_with_docker_process(
                "docker_ext".to_string(),
                Arc::new(MockClient {}),
                vec![],
                Some(DockerExecProcess::new(&container, argv.clone())),
            )
            .await;

        extension_manager
            .remove_extension("docker_ext")
            .await
            .unwrap();

        let pgrep = tokio::process::Command::new("docker")
            .arg("exec")
            .arg(&container_name)
            .arg("pgrep")
            .arg("-f")
            .arg(r"sleep\s+300")
            .kill_on_drop(true)
            .output()
            .await
            .expect("failed to invoke docker exec pgrep");
        assert!(
            !pgrep.status.success(),
            "removing the extension should have killed the process it started in the container"
        );

        let inspect = tokio::process::Command::new("docker")
            .args(["inspect", "-f", "{{.State.Running}}", &container_name])
            .kill_on_drop(true)
            .output()
            .await
            .expect("failed to inspect container");
        assert_eq!(
            String::from_utf8_lossy(&inspect.stdout).trim(),
            "true",
            "removing one extension's process must not stop the shared container"
        );

        let shutdown_argv = vec!["sleep".to_string(), "301".to_string()];
        start_detached_container_process(&container_name, &shutdown_argv, "shutdown process").await;
        extension_manager
            .add_mock_extension_with_docker_process(
                "shutdown_ext".to_string(),
                Arc::new(MockClient {}),
                vec![],
                Some(DockerExecProcess::new(&container, shutdown_argv)),
            )
            .await;

        extension_manager.shutdown().await;

        let pgrep = tokio::process::Command::new("docker")
            .arg("exec")
            .arg(&container_name)
            .arg("pgrep")
            .arg("-f")
            .arg(r"sleep\s+301")
            .kill_on_drop(true)
            .output()
            .await
            .expect("failed to verify manager shutdown cleanup");
        assert!(
            !pgrep.status.success(),
            "manager shutdown should drain Docker-backed extensions"
        );

        let drop_argv = vec!["sleep".to_string(), "302".to_string()];
        start_detached_container_process(&container_name, &drop_argv, "drop fallback process")
            .await;
        extension_manager
            .add_mock_extension_with_docker_process(
                "drop_ext".to_string(),
                Arc::new(MockClient {}),
                vec![],
                Some(DockerExecProcess::new(&container, drop_argv)),
            )
            .await;

        drop(extension_manager);

        let pgrep = tokio::process::Command::new("docker")
            .arg("exec")
            .arg(&container_name)
            .arg("pgrep")
            .arg("-f")
            .arg(r"sleep\s+302")
            .kill_on_drop(true)
            .output()
            .await
            .expect("failed to verify drop fallback cleanup");
        assert!(
            !pgrep.status.success(),
            "dropping a manager must clean up without a detached runtime task"
        );

        guard.cleanup().await;
    }

    #[tokio::test]
    async fn test_get_prefixed_tools_excluding() {
        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());

        extension_manager
            .add_mock_extension("ext_a".to_string(), Arc::new(MockClient {}))
            .await;
        extension_manager
            .add_mock_extension("ext_b".to_string(), Arc::new(MockClient {}))
            .await;

        let tools = extension_manager
            .get_prefixed_tools_excluding("test-session-id", "ext_a")
            .await
            .unwrap();
        let tool_names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();

        assert!(!tool_names.iter().any(|n| n.starts_with("ext_a__")));
        assert!(tool_names.iter().any(|n| n.starts_with("ext_b__")));
    }

    #[tokio::test]
    async fn test_get_prefixed_tools_by_extension_name() {
        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());

        extension_manager
            .add_mock_extension("ext_a".to_string(), Arc::new(MockClient {}))
            .await;
        extension_manager
            .add_mock_extension("ext_b".to_string(), Arc::new(MockClient {}))
            .await;

        let tools = extension_manager
            .get_prefixed_tools("test-session-id", Some("ext_a".to_string()))
            .await
            .unwrap();
        let tool_names: Vec<String> = tools.iter().map(|t| t.name.to_string()).collect();

        assert!(tool_names.iter().any(|n| n.starts_with("ext_a__")));
        assert!(!tool_names.iter().any(|n| n.starts_with("ext_b__")));
    }

    #[tokio::test]
    async fn test_resolve_tool_error_includes_available_tools() {
        let temp_dir = tempfile::tempdir().unwrap();
        let extension_manager =
            ExtensionManager::new_without_provider(temp_dir.path().to_path_buf());

        extension_manager
            .add_mock_extension("ext_a".to_string(), Arc::new(MockClient {}))
            .await;

        let result = extension_manager
            .resolve_tool("test-session-id", "definitely_not_a_real_tool")
            .await;
        let err = match result {
            Ok(_) => panic!("resolve_tool should fail for an unknown name"),
            Err(e) => e,
        };

        let msg = err.message.to_string();
        assert!(
            msg.contains("definitely_not_a_real_tool"),
            "error should echo the bad name; got: {msg}"
        );
        assert!(
            msg.contains("ext_a__"),
            "error should list at least one real tool name; got: {msg}"
        );
    }

    #[test]
    fn test_remove_untrusted_mcp_app_meta_strips_spoofed_payload() {
        let mut result = CallToolResult::success(vec![]);
        result.meta = Some(Meta(
            serde_json::from_value(serde_json::json!({
                "gosling": {
                    "mcpApp": {
                        "resourceUri": "ui://spoofed/app",
                    },
                    "other": true,
                },
                TRUSTED_TOOL_UPDATE_META_KEY: {
                    "mcpApp": {
                        "resourceUri": "ui://spoofed/internal",
                    },
                },
            }))
            .unwrap(),
        ));

        remove_untrusted_mcp_app_meta(&mut result);

        let meta = result.meta.expect("expected remaining meta");
        assert_eq!(meta.0.get(TRUSTED_TOOL_UPDATE_META_KEY), None);
        assert_eq!(
            meta.0.get("gosling"),
            Some(&serde_json::json!({ "other": true }))
        );
    }

    #[test]
    fn test_insert_trusted_tool_update_meta_stores_backend_payload() {
        let mut result = CallToolResult::success(vec![]);
        let attachment = GoslingMcpAppToolAttachment {
            tool_name: "weather__render".to_string(),
            extension_name: "weather".to_string(),
            resource_uri: "ui://weather/app".to_string(),
            tool_meta: None,
            resource_result: Some(serde_json::json!({
                "contents": [
                    {
                        "uri": "ui://weather/app",
                        "mimeType": "text/html;profile=mcp-app",
                        "text": "<div>Hello</div>",
                    },
                ],
            })),
            read_error: None,
        };

        insert_trusted_tool_update_meta(&mut result, &attachment);

        let meta = result.meta.expect("expected trusted meta");
        assert_eq!(
            meta.0.get(TRUSTED_TOOL_UPDATE_META_KEY),
            Some(&serde_json::json!({
                "mcpApp": {
                    "toolName": "weather__render",
                    "extensionName": "weather",
                    "resourceUri": "ui://weather/app",
                    "resourceResult": {
                        "contents": [
                            {
                                "uri": "ui://weather/app",
                                "mimeType": "text/html;profile=mcp-app",
                                "text": "<div>Hello</div>",
                            },
                        ],
                    },
                },
            })),
        );
    }

    #[tokio::test]
    async fn test_add_extension_noop_on_identical_config() {
        // When add_extension is called with a config that is byte-for-byte identical to
        // the already-loaded one, it must return Ok(()) without removing the extension.
        let temp_dir = tempfile::tempdir().unwrap();
        let em = Arc::new(ExtensionManager::new_without_provider(
            temp_dir.path().to_path_buf(),
        ));

        let config = ExtensionConfig::Frontend {
            name: "test-ext".to_string(),
            description: "original".to_string(),
            tools: vec![],
            instructions: None,
            bundled: None,
            available_tools: vec![],
        };

        em.add_client(
            "test-ext".to_string(),
            config.clone(),
            Arc::new(MockClient {}),
            None,
            None,
        )
        .await;
        assert_eq!(em.extensions.lock().await.len(), 1);

        // Calling add_extension with the same config must be a no-op (Ok, count unchanged).
        let result = em.add_extension(config, None, None, None).await;
        assert!(result.is_ok(), "identical config should be a no-op");
        assert_eq!(
            em.extensions.lock().await.len(),
            1,
            "extension must not be removed on no-op"
        );
    }

    #[tokio::test]
    async fn test_code_execution_runtime_disabled_blocks_active_extension_and_preserves_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let em = Arc::new(extension_manager_with_runtime(
            temp_dir.path().to_path_buf(),
            CodeExecutionRuntime::Disabled,
        ));
        let config = platform_extension_config("code_execution");

        let err = em
            .add_extension(config.clone(), None, None, None)
            .await
            .unwrap_err();

        assert!(matches!(err, ExtensionError::ConfigError(_)));
        assert!(!em.is_extension_enabled("code_execution").await);
        assert!(em.get_extension_configs().await.is_empty());
        assert!(em
            .get_prefixed_tools("test-session", None)
            .await
            .unwrap()
            .is_empty());
        assert_eq!(
            em.get_extension_configs_for_persistence().await,
            vec![config.clone()]
        );

        em.remove_extension("code_execution").await.unwrap();
        assert!(em.get_extension_configs_for_persistence().await.is_empty());
    }

    #[tokio::test]
    async fn test_code_execution_runtime_disabled_allows_unrelated_platform_extensions() {
        let temp_dir = tempfile::tempdir().unwrap();
        let em = Arc::new(extension_manager_with_runtime(
            temp_dir.path().to_path_buf(),
            CodeExecutionRuntime::Disabled,
        ));
        let config = platform_extension_config("todo");

        em.add_extension(config.clone(), None, None, None)
            .await
            .unwrap();

        assert!(em.is_extension_enabled("todo").await);
        assert_eq!(em.get_extension_configs().await, vec![config.clone()]);
        assert_eq!(
            em.get_extension_configs_for_persistence().await,
            vec![config]
        );
    }

    #[tokio::test]
    async fn test_add_extension_replaces_extension_on_config_change() {
        // When add_extension is called with an updated config (same name, different fields),
        // the existing extension must be removed so the caller can re-add with new config.
        let temp_dir = tempfile::tempdir().unwrap();
        let em = Arc::new(ExtensionManager::new_without_provider(
            temp_dir.path().to_path_buf(),
        ));

        let config_a = ExtensionConfig::Frontend {
            name: "test-ext".to_string(),
            description: "version-a".to_string(),
            tools: vec![],
            instructions: None,
            bundled: None,
            available_tools: vec![],
        };
        let config_b = ExtensionConfig::Frontend {
            name: "test-ext".to_string(),
            description: "version-b".to_string(), // changed
            tools: vec![],
            instructions: None,
            bundled: None,
            available_tools: vec![],
        };

        em.add_client(
            "test-ext".to_string(),
            config_a,
            Arc::new(MockClient {}),
            None,
            None,
        )
        .await;
        assert_eq!(em.extensions.lock().await.len(), 1);

        // add_extension with changed config attempts to create a new client (fails here
        // because Frontend configs cannot be added as server extensions), but must preserve
        // the old extension so the session isn't left without it.
        let result = em.add_extension(config_b, None, None, None).await;
        assert!(result.is_err(), "Frontend add_extension must return Err");
        assert_eq!(
            em.extensions.lock().await.len(),
            1,
            "old extension must be preserved when replacement client creation fails"
        );
    }

    fn transport_err(error: Box<dyn std::error::Error + Send + Sync>) -> ClientInitializeError {
        ClientInitializeError::TransportError {
            error: rmcp::transport::DynamicTransportError::from_parts(
                "test",
                std::any::TypeId::of::<()>(),
                error,
            ),
            context: "test context".into(),
        }
    }

    fn streamable_err(
        e: rmcp::transport::streamable_http_client::StreamableHttpError<reqwest::Error>,
    ) -> ClientInitializeError {
        transport_err(Box::new(e))
    }

    #[test]
    fn test_oauth_fallback_on_typed_auth_required() {
        let err = streamable_err(
            rmcp::transport::streamable_http_client::StreamableHttpError::AuthRequired(
                rmcp::transport::streamable_http_client::AuthRequiredError::new(
                    "Bearer realm=\"test\"".to_string(),
                ),
            ),
        );
        assert!(should_attempt_oauth_fallback(&Err(err)));
    }

    #[test]
    fn test_oauth_fallback_on_unexpected_response_http_401_prefix() {
        let err = streamable_err(
            rmcp::transport::streamable_http_client::StreamableHttpError::UnexpectedServerResponse(
                std::borrow::Cow::Borrowed("HTTP 401 Unauthorized"),
            ),
        );
        assert!(should_attempt_oauth_fallback(&Err(err)));
    }

    #[test]
    fn resolve_static_oauth_client_uses_registered_client_values() {
        let config_dir = tempdir().unwrap();
        let config = Config::new_with_file_secrets(
            config_dir.path().join("config.yaml"),
            config_dir.path().join("secrets.yaml"),
        )
        .unwrap();
        let envs = HashMap::from([
            ("MCP_CLIENT_ID".to_string(), "registered-client".to_string()),
            (
                "MCP_CLIENT_SECRET".to_string(),
                "registered-secret".to_string(),
            ),
        ]);

        let resolved = resolve_static_oauth_client(
            Some("${MCP_CLIENT_ID}"),
            Some("MCP_CLIENT_SECRET"),
            &["tools.read".to_string()],
            &envs,
            &config,
        )
        .unwrap()
        .unwrap();

        assert_eq!(resolved.client_id, "registered-client");
        assert_eq!(resolved.client_secret.as_deref(), Some("registered-secret"));
        assert_eq!(resolved.scopes, vec!["tools.read"]);
    }

    #[test]
    fn resolve_static_oauth_client_rejects_orphaned_oauth_fields() {
        let config_dir = tempdir().unwrap();
        let config = Config::new_with_file_secrets(
            config_dir.path().join("config.yaml"),
            config_dir.path().join("secrets.yaml"),
        )
        .unwrap();

        let error = resolve_static_oauth_client(
            None,
            Some("MCP_CLIENT_SECRET"),
            &[],
            &HashMap::new(),
            &config,
        )
        .unwrap_err();

        assert!(error.to_string().contains("require client_id"));
    }

    #[test]
    fn resolve_static_oauth_client_reads_client_secret_from_config() {
        let config_dir = tempdir().unwrap();
        let config = Config::new_with_file_secrets(
            config_dir.path().join("config.yaml"),
            config_dir.path().join("secrets.yaml"),
        )
        .unwrap();
        config
            .set("MCP_CLIENT_SECRET", &"registered-secret", true)
            .unwrap();

        let resolved = resolve_static_oauth_client(
            Some("registered-client"),
            Some("MCP_CLIENT_SECRET"),
            &[],
            &HashMap::new(),
            &config,
        )
        .unwrap()
        .unwrap();

        assert_eq!(resolved.client_secret.as_deref(), Some("registered-secret"));
    }

    #[tokio::test]
    async fn test_post_refresh_auth_failure_clears_credentials() {
        use rmcp::transport::auth::{
            InMemoryCredentialStore, OAuthTokenResponse, StoredCredentials,
        };

        let token_response: OAuthTokenResponse = serde_json::from_value(serde_json::json!({
            "access_token": "rejected-token",
            "token_type": "bearer",
        }))
        .expect("valid fake token JSON");
        let store = InMemoryCredentialStore::new();
        store
            .save(StoredCredentials::new(
                "test-client".to_string(),
                Some(token_response),
                vec![],
                None,
            ))
            .await
            .unwrap();

        let err = streamable_err(
            rmcp::transport::streamable_http_client::StreamableHttpError::AuthRequired(
                rmcp::transport::streamable_http_client::AuthRequiredError::new(
                    "Bearer error=\"invalid_token\"".to_string(),
                ),
            ),
        );
        let error = ExtensionError::InitializeError(err);

        assert!(clear_credentials_on_post_refresh_auth_failure(&store, "test-ext", &error).await);
        assert!(store.load().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_invalid_header_name_returns_config_error() {
        let mut headers = HashMap::new();
        headers.insert("bad header name".to_string(), "value".to_string());

        let temp_dir = tempdir().unwrap();
        let provider: SharedProvider = Arc::new(Mutex::new(None));
        let capabilities = GoslingMcpClientCapabilities {
            mcpui: false,
            host_info: None,
        };

        let result = create_streamable_http_client(
            "http://localhost:1",
            None,
            &headers,
            "test-ext",
            None,
            None,
            Box::new(rmcp::transport::auth::InMemoryCredentialStore::new()),
            provider,
            "gosling-test".to_string(),
            capabilities,
            temp_dir.path(),
        )
        .await;

        let Err(ExtensionError::ConfigError(msg)) = result else {
            panic!("expected ConfigError, got a different result");
        };
        assert!(
            msg.contains("invalid header"),
            "unexpected error message: {msg}"
        );
    }

    #[tokio::test]
    async fn test_invalid_header_value_returns_config_error() {
        let mut headers = HashMap::new();
        headers.insert("x-valid-name".to_string(), "bad\r\nvalue".to_string());

        let temp_dir = tempdir().unwrap();
        let provider: SharedProvider = Arc::new(Mutex::new(None));
        let capabilities = GoslingMcpClientCapabilities {
            mcpui: false,
            host_info: None,
        };

        let result = create_streamable_http_client(
            "http://localhost:1",
            None,
            &headers,
            "test-ext",
            None,
            None,
            Box::new(rmcp::transport::auth::InMemoryCredentialStore::new()),
            provider,
            "gosling-test".to_string(),
            capabilities,
            temp_dir.path(),
        )
        .await;

        let Err(ExtensionError::ConfigError(msg)) = result else {
            panic!("expected ConfigError, got a different result");
        };
        assert!(
            msg.contains("invalid header value"),
            "unexpected error message: {msg}"
        );
    }

    #[tokio::test]
    async fn test_custom_headers_forwarded_to_http_extension() {
        use wiremock::matchers::any;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let mut headers = HashMap::new();
        headers.insert("x-api-key".to_string(), "test-secret-123".to_string());

        let temp_dir = tempdir().unwrap();
        let provider: SharedProvider = Arc::new(Mutex::new(None));
        let capabilities = GoslingMcpClientCapabilities {
            mcpui: false,
            host_info: None,
        };

        // The MCP handshake will fail against the stub server. We only care that
        // the outgoing HTTP request carried the custom header.
        let _ = create_streamable_http_client(
            &mock_server.uri(),
            None,
            &headers,
            "test-ext",
            None,
            None,
            Box::new(rmcp::transport::auth::InMemoryCredentialStore::new()),
            provider,
            "gosling-test".to_string(),
            capabilities,
            temp_dir.path(),
        )
        .await;

        let received = mock_server.received_requests().await.unwrap();
        assert!(
            !received.is_empty(),
            "expected at least one HTTP request to reach the mock server"
        );
        let header_found = received.iter().any(|req| {
            req.headers
                .get("x-api-key")
                .map(|v| v == "test-secret-123")
                .unwrap_or(false)
        });
        assert!(
            header_found,
            "custom header x-api-key was not forwarded to the extension server"
        );
    }

    /// Directly exercises `connect_with_auth`, which is the code path fixed by
    /// the PR (custom headers were dropped when the OAuth connection path was
    /// taken).  Uses a pre-seeded `InMemoryCredentialStore` with a fake,
    /// non-expiring token so `get_access_token()` returns immediately without
    /// touching any OAuth endpoints or the system keychain.
    #[tokio::test]
    async fn test_custom_headers_forwarded_oauth_path() {
        use rmcp::transport::auth::{
            InMemoryCredentialStore, OAuthTokenResponse, StoredCredentials,
        };
        use wiremock::matchers::any;
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(200))
            .mount(&mock_server)
            .await;

        let mut headers = HashMap::new();
        headers.insert("x-api-key".to_string(), "test-secret-oauth".to_string());

        // Build a fake, non-expiring token. token_received_at=None skips the
        // expiry check, so get_access_token() returns without any network call.
        let token_response: OAuthTokenResponse = serde_json::from_value(serde_json::json!({
            "access_token": "fake-test-token",
            "token_type": "bearer",
        }))
        .expect("valid fake token JSON");
        let creds = StoredCredentials::new(
            "test-client".to_string(),
            Some(token_response),
            vec![],
            None,
        );
        let store = InMemoryCredentialStore::new();
        store.save(creds).await.unwrap();

        let mut auth_manager = rmcp::transport::AuthorizationManager::new(mock_server.uri())
            .await
            .expect("AuthorizationManager::new should not make network calls");
        auth_manager.set_credential_store(store);

        let temp_dir = tempdir().unwrap();
        let provider: SharedProvider = Arc::new(Mutex::new(None));
        let capabilities = GoslingMcpClientCapabilities {
            mcpui: false,
            host_info: None,
        };

        // connect_with_auth will fail (mock server isn't an MCP server) but we
        // only care that the outgoing request carried the custom header.
        let _ = connect_with_auth(
            auth_manager,
            &mock_server.uri(),
            Duration::from_secs(5),
            &headers,
            provider,
            "gosling-test".to_string(),
            capabilities,
            temp_dir.path(),
        )
        .await;

        let received = mock_server.received_requests().await.unwrap();
        assert!(
            !received.is_empty(),
            "expected at least one HTTP request to reach the mock server"
        );
        let header_found = received.iter().any(|req| {
            req.headers
                .get("x-api-key")
                .map(|v| v == "test-secret-oauth")
                .unwrap_or(false)
        });
        assert!(
            header_found,
            "custom header x-api-key was not forwarded through the OAuth connection path"
        );
    }
}
