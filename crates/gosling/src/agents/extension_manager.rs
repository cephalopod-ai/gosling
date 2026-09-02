// Extension-management compatibility facade for lifecycle and MCP orchestration.
// Production callers keep stable manager, helper, and operator-transport paths here.
// Responsibility modules live under extension_manager/ behind this public surface.

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

use oauth::create_streamable_http_client;
#[cfg(test)]
use oauth::{
    build_streamable_http_client, clear_credentials_on_post_refresh_auth_failure,
    connect_with_auth, should_attempt_oauth_fallback,
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
mod tests;
