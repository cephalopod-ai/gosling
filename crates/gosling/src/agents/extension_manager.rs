use anyhow::Result;
use axum::http::{HeaderMap, HeaderName, HeaderValue};
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
use std::path::PathBuf;
use std::pin::Pin;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tempfile::{tempdir, TempDir};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;
use tracing::{error, warn};

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
use crate::config::{get_all_extensions, CodeExecutionRuntime, Config};
use crate::oauth::{oauth_flow, GoslingCredentialStore};
use crate::prompt_template;
use crate::subprocess::configure_subprocess;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ErrorCode, ErrorData, GetPromptResult, Meta,
    Prompt, Resource, ResourceContents, ServerInfo, Tool,
};
use rmcp::transport::auth::{AuthClient, CredentialStore};
use schemars::_private::NoSerialize;
use serde_json::Value;

type McpClientBox = Arc<dyn McpClientTrait>;

const MAX_MCP_LIST_PAGES: usize = 100;
const MAX_MCP_LIST_ITEMS: usize = 10_000;

#[derive(Default)]
struct PaginationGuard {
    pages: usize,
    items: usize,
    cursors: HashSet<String>,
}

impl PaginationGuard {
    fn record_page(
        &mut self,
        item_kind: &str,
        page_items: usize,
        next_cursor: Option<&str>,
    ) -> std::result::Result<(), String> {
        self.pages += 1;
        if self.pages > MAX_MCP_LIST_PAGES {
            return Err(format!(
                "MCP {item_kind} pagination exceeded {MAX_MCP_LIST_PAGES} pages"
            ));
        }

        self.items = self
            .items
            .checked_add(page_items)
            .ok_or_else(|| format!("MCP {item_kind} item count overflowed"))?;
        if self.items > MAX_MCP_LIST_ITEMS {
            return Err(format!(
                "MCP {item_kind} pagination exceeded {MAX_MCP_LIST_ITEMS} items"
            ));
        }

        if let Some(cursor) = next_cursor {
            if !self.cursors.insert(cursor.to_string()) {
                return Err(format!(
                    "MCP {item_kind} pagination repeated cursor {cursor:?}"
                ));
            }
        }
        Ok(())
    }
}

async fn collect_paginated_tools(
    client: &McpClientBox,
    session_id: &str,
    cancellation_token: CancellationToken,
) -> std::result::Result<Vec<Tool>, String> {
    let mut cursor = None;
    let mut guard = PaginationGuard::default();
    let mut tools = Vec::new();
    loop {
        let page = client
            .list_tools(session_id, cursor, cancellation_token.clone())
            .await
            .map_err(|error| format!("MCP list tools request failed: {error}"))?;
        guard.record_page("tool", page.tools.len(), page.next_cursor.as_deref())?;
        tools.extend(page.tools);
        match page.next_cursor {
            Some(next_cursor) => cursor = Some(next_cursor),
            None => return Ok(tools),
        }
    }
}

async fn collect_paginated_resources(
    client: &McpClientBox,
    session_id: &str,
    cancellation_token: CancellationToken,
) -> std::result::Result<Vec<Resource>, String> {
    let mut cursor = None;
    let mut guard = PaginationGuard::default();
    let mut resources = Vec::new();
    loop {
        let page = client
            .list_resources(session_id, cursor, cancellation_token.clone())
            .await
            .map_err(|error| format!("MCP list resources request failed: {error}"))?;
        guard.record_page(
            "resource",
            page.resources.len(),
            page.next_cursor.as_deref(),
        )?;
        resources.extend(page.resources);
        match page.next_cursor {
            Some(next_cursor) => cursor = Some(next_cursor),
            None => return Ok(resources),
        }
    }
}

async fn collect_paginated_prompts(
    client: &McpClientBox,
    session_id: &str,
    cancellation_token: CancellationToken,
) -> std::result::Result<Vec<Prompt>, String> {
    let mut cursor = None;
    let mut guard = PaginationGuard::default();
    let mut prompts = Vec::new();
    loop {
        let page = client
            .list_prompts(session_id, cursor, cancellation_token.clone())
            .await
            .map_err(|error| format!("MCP list prompts request failed: {error}"))?;
        guard.record_page("prompt", page.prompts.len(), page.next_cursor.as_deref())?;
        prompts.extend(page.prompts);
        match page.next_cursor {
            Some(next_cursor) => cursor = Some(next_cursor),
            None => return Ok(prompts),
        }
    }
}

struct ActionRequiredStream {
    inner: ReceiverStream<crate::conversation::message::Message>,
    session_id: String,
    tool_call_request_id: String,
}

impl ActionRequiredStream {
    fn new(
        receiver: tokio::sync::mpsc::Receiver<crate::conversation::message::Message>,
        session_id: String,
        tool_call_request_id: String,
    ) -> Self {
        Self {
            inner: ReceiverStream::new(receiver),
            session_id,
            tool_call_request_id,
        }
    }
}

impl Stream for ActionRequiredStream {
    type Item = crate::conversation::message::Message;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.inner).poll_next(cx)
    }
}

impl Drop for ActionRequiredStream {
    fn drop(&mut self) {
        let session_id = self.session_id.clone();
        let tool_call_request_id = self.tool_call_request_id.clone();
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        handle.spawn(async move {
            ActionRequiredManager::global()
                .unregister_action_required_stream(&session_id, &tool_call_request_id)
                .await;
        });
    }
}

static RE_ENV_BRACES: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\$\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}").expect("valid regex"));

static RE_ENV_SIMPLE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").expect("valid regex"));

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
}

impl Drop for Extension {
    fn drop(&mut self) {
        let Some(docker_process) = self.docker_process.take() else {
            return;
        };
        // Covers every teardown path uniformly: explicit removal
        // (`remove_extension`), silent replacement when an extension is
        // reconfigured (`extensions.insert` drops the old value), and the
        // whole manager going away (e.g. agent eviction).
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        handle.spawn(async move {
            docker_process.kill().await;
        });
    }
}

pub struct ExtensionManagerCapabilities {
    pub mcpui: bool,
    pub host_info: Option<GoslingMcpHostInfo>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GoslingMcpAppToolAttachment {
    pub tool_name: String,
    pub extension_name: String,
    pub resource_uri: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_meta: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_error: Option<String>,
}

pub(crate) const TRUSTED_TOOL_UPDATE_META_KEY: &str = "__gosling_tool_update_meta";

/// Manages gosling extensions / MCP clients and their interactions
pub struct ExtensionManager {
    extensions: Mutex<HashMap<String, Extension>>,
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

fn resolve_command(cmd: &str) -> PathBuf {
    SearchPaths::builder()
        .with_npm()
        .resolve(cmd)
        .unwrap_or_else(|_| {
            // let the OS raise the error
            PathBuf::from(cmd)
        })
}

fn minimal_child_environment() -> HashMap<String, String> {
    let mut env = HashMap::new();
    for key in ["PATH", "HOME", "USER", "TMPDIR", "TEMP", "TMP"] {
        if let Ok(value) = std::env::var(key) {
            env.insert(key.to_string(), value);
        }
    }

    #[cfg(windows)]
    for key in ["SystemRoot", "COMSPEC", "PATHEXT", "USERPROFILE"] {
        if let Ok(value) = std::env::var(key) {
            env.insert(key.to_string(), value);
        }
    }

    env
}

fn apply_minimal_child_environment(command: &mut Command) {
    command.env_clear().envs(minimal_child_environment());
}

fn require_str_parameter<'a>(v: &'a serde_json::Value, name: &str) -> Result<&'a str, ErrorData> {
    let v = v.get(name).ok_or_else(|| {
        ErrorData::new(
            ErrorCode::INVALID_PARAMS,
            format!("The parameter {name} is required"),
            None,
        )
    })?;
    match v.as_str() {
        Some(r) => Ok(r),
        None => Err(ErrorData::new(
            ErrorCode::INVALID_PARAMS,
            format!("The parameter {name} must be a string"),
            None,
        )),
    }
}

pub fn get_parameter_names(tool: &Tool) -> Vec<String> {
    let mut names: Vec<String> = tool
        .input_schema
        .get("properties")
        .and_then(|props| props.as_object())
        .map(|props| props.keys().cloned().collect())
        .unwrap_or_default();
    names.sort();
    names
}

const TOOL_EXTENSION_META_KEY: &str = "gosling_extension";

pub fn get_tool_owner(tool: &Tool) -> Option<String> {
    tool.meta
        .as_ref()
        .and_then(|m| m.0.get(TOOL_EXTENSION_META_KEY))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn get_tool_meta_value(tool: &Tool) -> Option<Value> {
    tool.meta.as_ref().map(|meta| Value::Object(meta.0.clone()))
}

fn get_tool_resource_uri(tool: &Tool) -> Option<String> {
    tool.meta
        .as_ref()
        .and_then(|meta| meta.0.get("ui"))
        .and_then(Value::as_object)
        .and_then(|ui| ui.get("resourceUri"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn remove_untrusted_mcp_app_meta(result: &mut CallToolResult) {
    let Some(meta) = result.meta.as_mut() else {
        return;
    };

    meta.0.remove(TRUSTED_TOOL_UPDATE_META_KEY);

    let remove_gosling = meta
        .0
        .get_mut("gosling")
        .and_then(Value::as_object_mut)
        .map(|gosling_meta| {
            gosling_meta.remove("mcpApp");
            gosling_meta.is_empty()
        })
        .unwrap_or(false);

    if remove_gosling {
        meta.0.remove("gosling");
    }

    if meta.0.is_empty() {
        result.meta = None;
    }
}

fn insert_trusted_tool_update_meta(
    result: &mut CallToolResult,
    attachment: &GoslingMcpAppToolAttachment,
) {
    let Ok(attachment_value) = serde_json::to_value(attachment) else {
        return;
    };

    let mut meta_map = result
        .meta
        .as_ref()
        .map(|meta| meta.0.clone())
        .unwrap_or_default();
    let mut trusted_meta = serde_json::Map::new();
    trusted_meta.insert("mcpApp".to_string(), attachment_value);
    meta_map.insert(
        TRUSTED_TOOL_UPDATE_META_KEY.to_string(),
        Value::Object(trusted_meta),
    );
    result.meta = Some(Meta(meta_map));
}

fn is_unprefixed_extension(config: &ExtensionConfig) -> bool {
    match config {
        ExtensionConfig::Platform { name, .. } | ExtensionConfig::Builtin { name, .. } => {
            PLATFORM_EXTENSIONS
                .get(name_to_key(name).as_str())
                .is_some_and(|def| def.unprefixed_tools)
        }
        _ => false,
    }
}

/// Returns true if the named extension is a first-class platform extension
/// whose tools are exposed unprefixed and remain visible during code execution mode.
pub fn is_first_class_extension(name: &str) -> bool {
    PLATFORM_EXTENSIONS
        .get(name_to_key(name).as_str())
        .is_some_and(|def| def.unprefixed_tools)
}

pub fn is_hidden_extension(name: &str) -> bool {
    PLATFORM_EXTENSIONS
        .get(name_to_key(name).as_str())
        .is_some_and(|def| def.hidden)
}

/// Result of resolving a tool call to its owning extension
struct ResolvedTool {
    tool_name: String,
    extension_name: String,
    actual_tool_name: String,
    client: McpClientBox,
    tool_meta: Option<Value>,
    resource_uri: Option<String>,
}

async fn child_process_client(
    mut command: Command,
    timeout: &Option<u64>,
    provider: SharedProvider,
    working_dir: &PathBuf,
    docker_container: Option<String>,
    client_name: String,
    capabilities: GoslingMcpClientCapabilities,
) -> ExtensionResult<McpClient> {
    configure_subprocess(&mut command);

    if let Ok(path) = SearchPaths::builder().path() {
        command.env("PATH", path);
    }

    if working_dir.exists() && working_dir.is_dir() {
        tracing::info!("Setting MCP process working directory: {:?}", working_dir);
        command.current_dir(working_dir);
    } else {
        tracing::warn!(
            "Working directory doesn't exist or isn't a directory: {:?}",
            working_dir
        );
    }

    let (transport, mut stderr) = TokioChildProcess::builder(command)
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stderr = stderr.take().ok_or_else(|| {
        ExtensionError::SetupError("failed to attach child process stderr".to_owned())
    })?;

    let stderr_task = tokio::spawn(async move {
        // This buffer is only consumed to build an error message if the connect
        // below fails. On the success path the task is detached and lives as long
        // as the MCP server, so an unbounded `read_to_end` would accumulate every
        // stderr byte a long-lived, chatty server emits for the whole session. Cap
        // what we retain (enough to diagnose a startup failure) but keep reading so
        // the child never blocks writing to a full stderr pipe.
        const MAX_STDERR_CAPTURE: usize = 64 * 1024;
        let mut captured: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let n = stderr.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            if captured.len() < MAX_STDERR_CAPTURE {
                let take = n.min(MAX_STDERR_CAPTURE - captured.len());
                captured.extend_from_slice(&chunk[..take]);
            }
        }
        Ok::<String, std::io::Error>(String::from_utf8_lossy(&captured).into())
    });

    let client_result = McpClient::connect_with_container(
        transport,
        Duration::from_secs(resolve_timeout(*timeout)),
        provider,
        docker_container,
        client_name,
        capabilities,
        working_dir.clone(),
    )
    .await;

    match client_result {
        Ok(client) => Ok(client),
        Err(error) => {
            let error_task_out = stderr_task.await?;
            Err::<McpClient, ExtensionError>(match error_task_out {
                Ok(stderr_content) => ProcessExit::new(stderr_content, error).into(),
                Err(e) => e.into(),
            })
        }
    }
}

/// Retry with OAuth for typed auth challenges and wrapped bare HTTP 401 responses.
fn is_oauth_auth_failure(err: &ClientInitializeError) -> bool {
    let ClientInitializeError::TransportError {
        error: DynamicTransportError { error, .. },
        ..
    } = err
    else {
        return false;
    };

    if let Some(http_err) = error.downcast_ref::<StreamableHttpError<reqwest::Error>>() {
        return match http_err {
            StreamableHttpError::AuthRequired(_) => true,
            StreamableHttpError::UnexpectedServerResponse(body) => body.starts_with("HTTP 401"),
            _ => false,
        };
    }

    #[cfg(unix)]
    if let Some(http_err) = error
        .downcast_ref::<StreamableHttpError<rmcp::transport::common::unix_socket::UnixSocketError>>(
        )
    {
        return match http_err {
            StreamableHttpError::AuthRequired(_) => true,
            StreamableHttpError::UnexpectedServerResponse(body) => body.starts_with("HTTP 401"),
            _ => false,
        };
    }

    error
        .to_string()
        .contains("unexpected server response: HTTP 401")
}

fn should_attempt_oauth_fallback(res: &Result<McpClient, ClientInitializeError>) -> bool {
    res.as_ref().err().is_some_and(is_oauth_auth_failure)
}

async fn clear_credentials_on_post_refresh_auth_failure(
    credential_store: &dyn CredentialStore,
    name: &str,
    error: &ExtensionError,
) -> bool {
    let ExtensionError::InitializeError(err) = error else {
        return false;
    };

    if !is_oauth_auth_failure(err) {
        return false;
    }

    if let Err(e) = credential_store.clear().await {
        warn!(
            "[OAuth:{}] error clearing rejected credentials: {}",
            name, e
        );
    }
    true
}

/// Merge environment variables from direct envs and keychain-stored env_keys
pub(crate) async fn merge_environments(
    envs: &Envs,
    env_keys: &[String],
    ext_name: &str,
    config: &Config,
) -> Result<HashMap<String, String>, ExtensionError> {
    let mut all_envs = envs.get_env();

    for key in env_keys {
        if all_envs.contains_key(key) {
            continue;
        }

        match config.get(key, true) {
            Ok(value) => {
                if value.is_null() {
                    warn!(
                        key = %key,
                        ext_name = %ext_name,
                        "Secret key not found in config (returned null)."
                    );
                    continue;
                }

                if let Some(str_val) = value.as_str() {
                    all_envs.insert(key.clone(), str_val.to_string());
                } else {
                    warn!(
                        key = %key,
                        ext_name = %ext_name,
                        value_type = %value.get("type").and_then(|t| t.as_str()).unwrap_or("unknown"),
                        "Secret value is not a string; skipping."
                    );
                }
            }
            Err(e) => {
                error!(
                    key = %key,
                    ext_name = %ext_name,
                    error = %e,
                    "Failed to fetch secret from config."
                );
                return Err(ExtensionError::ConfigError(format!(
                    "Failed to fetch secret '{}' from config: {}",
                    key, e
                )));
            }
        }
    }

    Ok(Envs::new(all_envs).get_env())
}

/// Substitute environment variables in a string. Supports both ${VAR} and $VAR syntax.
pub(crate) fn substitute_env_vars(value: &str, env_map: &HashMap<String, String>) -> String {
    let mut result = value.to_string();

    for cap in RE_ENV_BRACES.captures_iter(value) {
        if let Some(var_name) = cap.get(1) {
            if let Some(env_value) = env_map.get(var_name.as_str()) {
                result = result.replace(&cap[0], env_value);
            }
        }
    }

    // Scan the original input for $VAR patterns (not the post-substitution result)
    // to avoid recursive expansion when a substituted value contains $OTHER_VAR.
    for cap in RE_ENV_SIMPLE.captures_iter(value) {
        if let Some(var_name) = cap.get(1) {
            if !value.contains(&format!("${{{}}}", var_name.as_str())) {
                if let Some(env_value) = env_map.get(var_name.as_str()) {
                    result = result.replace(&cap[0], env_value);
                }
            }
        }
    }

    result
}

const GOSLING_USER_AGENT: reqwest::header::HeaderValue =
    reqwest::header::HeaderValue::from_static(concat!("gosling/", env!("CARGO_PKG_VERSION")));

#[allow(clippy::too_many_arguments)]
async fn connect_with_auth(
    auth_manager: rmcp::transport::AuthorizationManager,
    uri: &str,
    timeout: Duration,
    headers: &HashMap<String, String>,
    provider: SharedProvider,
    client_name: String,
    capabilities: GoslingMcpClientCapabilities,
    roots_dir: &std::path::Path,
) -> ExtensionResult<Box<dyn McpClientTrait>> {
    let mut auth_headers = HeaderMap::new();
    auth_headers.insert(reqwest::header::USER_AGENT, GOSLING_USER_AGENT);
    for (key, value) in headers {
        auth_headers.insert(
            HeaderName::try_from(key)
                .map_err(|_| ExtensionError::ConfigError(format!("invalid header: {}", key)))?,
            value.parse().map_err(|_| {
                ExtensionError::ConfigError(format!("invalid header value: {}", key))
            })?,
        );
    }
    #[allow(unused_mut)]
    let mut auth_client_builder = reqwest::Client::builder().default_headers(auth_headers);
    #[cfg(target_os = "linux")]
    {
        auth_client_builder = auth_client_builder.tcp_user_timeout(Some(timeout));
    }
    let auth_http_client = auth_client_builder
        .build()
        .map_err(|_| ExtensionError::ConfigError("could not construct http client".to_string()))?;
    let auth_client = AuthClient::new(auth_http_client, auth_manager);
    let transport = StreamableHttpClientTransport::with_client(
        auth_client,
        StreamableHttpClientTransportConfig::with_uri(uri),
    );
    Ok(Box::new(
        McpClient::connect(
            transport,
            timeout,
            provider,
            client_name,
            capabilities,
            roots_dir.to_path_buf(),
        )
        .await?,
    ))
}

#[allow(clippy::too_many_arguments)]
async fn create_streamable_http_client(
    uri: &str,
    timeout: Option<u64>,
    headers: &HashMap<String, String>,
    name: &str,
    socket: Option<&str>,
    credential_store: Box<dyn CredentialStore>,
    provider: SharedProvider,
    client_name: String,
    capabilities: GoslingMcpClientCapabilities,
    roots_dir: &std::path::Path,
) -> ExtensionResult<Box<dyn McpClientTrait>> {
    #[cfg(unix)]
    if let Some(socket_path) = socket {
        return create_unix_socket_http_client(
            uri,
            timeout,
            headers,
            name,
            socket_path,
            provider,
            client_name,
            capabilities,
            roots_dir,
        )
        .await;
    }
    #[cfg(not(unix))]
    if socket.is_some() {
        return Err(ExtensionError::ConfigError(
            "Unix domain socket transport is not supported on this platform".to_string(),
        ));
    }

    let mut default_headers = HeaderMap::new();

    default_headers.insert(reqwest::header::USER_AGENT, GOSLING_USER_AGENT);

    for (key, value) in headers {
        default_headers.insert(
            HeaderName::try_from(key)
                .map_err(|_| ExtensionError::ConfigError(format!("invalid header: {}", key)))?,
            value.parse().map_err(|_| {
                ExtensionError::ConfigError(format!("invalid header value: {}", key))
            })?,
        );
    }

    let timeout_duration = Duration::from_secs(resolve_timeout(timeout));

    #[allow(unused_mut)]
    let mut http_client_builder = reqwest::Client::builder().default_headers(default_headers);
    #[cfg(target_os = "linux")]
    {
        http_client_builder = http_client_builder.tcp_user_timeout(Some(timeout_duration));
    }
    let http_client = http_client_builder
        .build()
        .map_err(|_| ExtensionError::ConfigError("could not construct http client".to_string()))?;

    let transport = StreamableHttpClientTransport::with_client(
        http_client,
        StreamableHttpClientTransportConfig::with_uri(uri),
    );

    // If we have stored OAuth credentials, try refreshing and connecting directly.
    // This avoids the unnecessary 401 → browser re-auth cycle on every new session.
    if credential_store.load().await.is_ok_and(|c| c.is_some()) {
        match oauth_flow(&uri.to_string(), &name.to_string()).await {
            Ok(auth_manager) => {
                let auth_result = connect_with_auth(
                    auth_manager,
                    uri,
                    timeout_duration,
                    headers,
                    provider.clone(),
                    client_name.clone(),
                    capabilities.clone(),
                    roots_dir,
                )
                .await;

                if let Err(error) = &auth_result {
                    if clear_credentials_on_post_refresh_auth_failure(
                        credential_store.as_ref(),
                        name,
                        error,
                    )
                    .await
                    {
                        warn!(
                            "[OAuth:{}] Refreshed token was rejected, falling back to browser auth",
                            name
                        );
                    } else {
                        return auth_result;
                    }
                } else {
                    return auth_result;
                }
            }
            Err(e) => {
                warn!(
                    "[OAuth:{}] Proactive refresh failed: {}, falling back to unauthenticated attempt",
                    name, e
                );
            }
        }
    }

    let client_res = McpClient::connect(
        transport,
        timeout_duration,
        provider.clone(),
        client_name.clone(),
        capabilities.clone(),
        roots_dir.to_path_buf(),
    )
    .await;

    if should_attempt_oauth_fallback(&client_res) {
        match oauth_flow(&uri.to_string(), &name.to_string()).await {
            Ok(auth_manager) => {
                connect_with_auth(
                    auth_manager,
                    uri,
                    timeout_duration,
                    headers,
                    provider,
                    client_name,
                    capabilities,
                    roots_dir,
                )
                .await
            }
            Err(_) => Ok(Box::new(client_res?)),
        }
    } else {
        Ok(Box::new(client_res?))
    }
}

#[cfg(unix)]
#[allow(clippy::too_many_arguments)]
async fn create_unix_socket_http_client(
    uri: &str,
    timeout: Option<u64>,
    headers: &HashMap<String, String>,
    name: &str,
    socket_path: &str,
    provider: SharedProvider,
    client_name: String,
    capabilities: GoslingMcpClientCapabilities,
    roots_dir: &std::path::Path,
) -> ExtensionResult<Box<dyn McpClientTrait>> {
    use rmcp::transport::UnixSocketHttpClient;

    let unix_client = UnixSocketHttpClient::new(socket_path, uri);

    let mut custom_headers = std::collections::HashMap::<HeaderName, HeaderValue>::new();

    custom_headers.insert(
        HeaderName::from_static("user-agent"),
        GOSLING_USER_AGENT
            .to_str()
            .unwrap_or("gosling")
            .parse()
            .unwrap_or_else(|_| HeaderValue::from_static("gosling")),
    );

    for (key, value) in headers {
        let header_name = HeaderName::try_from(key)
            .map_err(|_| ExtensionError::ConfigError(format!("invalid header: {}", key)))?;
        let header_value = value
            .parse::<HeaderValue>()
            .map_err(|_| ExtensionError::ConfigError(format!("invalid header value: {}", key)))?;
        custom_headers.insert(header_name, header_value);
    }

    let config = StreamableHttpClientTransportConfig::with_uri(uri).custom_headers(custom_headers);
    let transport = StreamableHttpClientTransport::with_client(unix_client, config);

    let timeout_duration = Duration::from_secs(resolve_timeout(timeout));

    let client_res = McpClient::connect(
        transport,
        timeout_duration,
        provider.clone(),
        client_name.clone(),
        capabilities.clone(),
        roots_dir.to_path_buf(),
    )
    .await;

    if should_attempt_oauth_fallback(&client_res) {
        tracing::warn!(
            "Extension '{}' returned 401 over Unix domain socket transport; \
             OAuth is not supported for UDS connections",
            name,
        );
    }
    Ok(Box::new(client_res?))
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
        let sanitized_name = config.key();

        // Compare both the unresolved config (to detect structural changes like
        // migrating from plaintext envs to env_keys) and the resolved config (to
        // detect secret rotation where only keyring values changed). Only skip
        // restart if both match.
        let resolved_config = config.clone().resolve(Config::global()).await?;

        if let Some(existing) = self.extensions.lock().await.get(&sanitized_name) {
            if existing.config == config && existing.resolved_config == resolved_config {
                return Ok(());
            }
            tracing::debug!(
                name = sanitized_name,
                "extension config changed, restarting with updated config"
            );
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
                create_streamable_http_client(
                    &resolved_uri,
                    *timeout,
                    &resolved_headers,
                    name,
                    resolved_socket.as_deref(),
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
                    .map(PathBuf::from)
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
                    Command::new("docker").configure(|command| {
                        command.arg("exec").arg("-i");
                        for (key, value) in &all_envs {
                            command.arg("-e").arg(format!("{}={}", key, value));
                        }
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

        let mut extensions = self.extensions.lock().await;
        extensions.insert(
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
        drop(extensions);
        self.invalidate_tools_cache_and_bump_version().await;

        Ok(())
    }

    pub async fn add_client(
        &self,
        name: String,
        config: ExtensionConfig,
        client: McpClientBox,
        info: Option<ServerInfo>,
        temp_dir: Option<TempDir>,
    ) {
        let normalized = name_to_key(&name);
        self.extensions.lock().await.insert(
            normalized,
            Extension::new(config.clone(), config.clone(), client, info, temp_dir, None),
        );
        self.invalidate_tools_cache_and_bump_version().await;
    }

    /// Get extensions info for building the system prompt
    pub async fn get_extensions_info(&self, working_dir: &std::path::Path) -> Vec<ExtensionInfo> {
        let working_dir_str = working_dir.to_string_lossy();
        self.extensions
            .lock()
            .await
            .iter()
            .map(|(name, ext)| {
                let instructions = ext.get_instructions().unwrap_or_default();
                let instructions = instructions.replace("{{WORKING_DIR}}", &working_dir_str);
                ExtensionInfo::new(name, &instructions, ext.supports_resources())
            })
            .collect()
    }

    /// Get aggregated usage statistics
    pub async fn remove_extension(&self, name: &str) -> ExtensionResult<()> {
        let sanitized_name = name_to_key(name);
        self.extensions.lock().await.remove(&sanitized_name);
        self.runtime_blocked_extensions
            .lock()
            .await
            .remove(&sanitized_name);
        self.invalidate_tools_cache_and_bump_version().await;
        Ok(())
    }

    pub async fn update_working_dirs(
        &self,
        primary: &std::path::Path,
        additional: &[std::path::PathBuf],
    ) -> ExtensionResult<()> {
        let extensions = self.extensions.lock().await;
        let mut failures = Vec::new();
        for (name, ext) in extensions.iter() {
            if let Err(e) = ext
                .client
                .update_working_dirs(primary.to_path_buf(), additional.to_vec())
                .await
            {
                failures.push(format!("{name}: {e}"));
            }
        }
        if failures.is_empty() {
            Ok(())
        } else {
            Err(ExtensionError::SetupError(format!(
                "failed to update extension roots: {}",
                failures.join("; ")
            )))
        }
    }

    pub async fn get_extension_and_tool_counts(&self, session_id: &str) -> (usize, usize) {
        let enabled_extensions_count = self.extensions.lock().await.len();

        let total_tools = self
            .get_prefixed_tools(session_id, None)
            .await
            .map(|tools| tools.len())
            .unwrap_or(0);

        (enabled_extensions_count, total_tools)
    }

    pub async fn list_extensions(&self) -> ExtensionResult<Vec<String>> {
        Ok(self.extensions.lock().await.keys().cloned().collect())
    }

    pub async fn is_extension_enabled(&self, name: &str) -> bool {
        let normalized = name_to_key(name);
        self.extensions.lock().await.contains_key(&normalized)
    }

    pub async fn get_extension_configs(&self) -> Vec<ExtensionConfig> {
        self.extensions
            .lock()
            .await
            .values()
            .map(|ext| ext.config.clone())
            .collect()
    }

    pub async fn get_extension_configs_for_persistence(&self) -> Vec<ExtensionConfig> {
        let mut configs = self.get_extension_configs().await;
        configs.extend(
            self.runtime_blocked_extensions
                .lock()
                .await
                .values()
                .cloned(),
        );
        configs
    }

    pub fn is_code_execution_runtime_enabled(&self) -> bool {
        self.code_execution_runtime.is_enabled()
    }

    /// Get all tools from all clients with proper prefixing
    pub async fn get_prefixed_tools(
        &self,
        session_id: &str,
        extension_name: Option<String>,
    ) -> ExtensionResult<Vec<Tool>> {
        let all_tools = self.get_all_tools_cached(session_id).await?;
        Ok(self.filter_tools(&all_tools, extension_name.as_deref(), None))
    }

    pub async fn get_prefixed_tools_excluding(
        &self,
        session_id: &str,
        exclude: &str,
    ) -> ExtensionResult<Vec<Tool>> {
        let all_tools = self.get_all_tools_cached(session_id).await?;
        Ok(self.filter_tools(&all_tools, None, Some(exclude)))
    }

    fn filter_tools(
        &self,
        tools: &[Tool],
        extension_name: Option<&str>,
        exclude: Option<&str>,
    ) -> Vec<Tool> {
        let extension_name_normalized = extension_name.map(name_to_key);
        let exclude_normalized = exclude.map(name_to_key);

        tools
            .iter()
            .filter(|tool| {
                let tool_owner = get_tool_owner(tool)
                    .map(|s| name_to_key(&s))
                    .unwrap_or_else(|| tool.name.split("__").next().unwrap_or("").to_string());

                if let Some(ref excluded) = exclude_normalized {
                    if tool_owner == *excluded {
                        return false;
                    }
                }

                if let Some(ref name_filter) = extension_name_normalized {
                    tool_owner == *name_filter
                } else {
                    true
                }
            })
            .cloned()
            .collect()
    }

    async fn get_all_tools_cached(&self, session_id: &str) -> ExtensionResult<Arc<Vec<Tool>>> {
        {
            let cache = self.tools_cache.lock().await;
            if let Some(ref tools) = *cache {
                return Ok(Arc::clone(tools));
            }
        }

        let version_before = self.tools_cache_version.load(Ordering::SeqCst);
        let tools = Arc::new(self.fetch_all_tools(session_id).await?);

        {
            let mut cache = self.tools_cache.lock().await;
            let version_after = self.tools_cache_version.load(Ordering::SeqCst);
            if version_after == version_before && cache.is_none() {
                *cache = Some(Arc::clone(&tools));
            }
        }

        Ok(tools)
    }

    fn host_supports_mcp_apps(&self) -> bool {
        if let Some(host_info) = &self.capabilities.host_info {
            if host_info.explicit_extensions {
                return host_info.mcpui_enabled();
            }
        }

        self.capabilities.mcpui
    }

    async fn hydrate_mcp_app_attachment(
        client: &McpClientBox,
        session_id: &str,
        resolved_tool: &ResolvedTool,
        cancellation_token: CancellationToken,
    ) -> Option<GoslingMcpAppToolAttachment> {
        let resource_uri = resolved_tool.resource_uri.clone()?;

        let mut attachment = GoslingMcpAppToolAttachment {
            tool_name: resolved_tool.tool_name.clone(),
            extension_name: resolved_tool.extension_name.clone(),
            resource_uri: resource_uri.clone(),
            tool_meta: resolved_tool.tool_meta.clone(),
            resource_result: None,
            read_error: None,
        };

        match client
            .read_resource(session_id, &resource_uri, cancellation_token)
            .await
        {
            Ok(resource_result) => {
                attachment.resource_result = serde_json::to_value(&resource_result).ok();
            }
            Err(error) => {
                attachment.read_error = Some(error.to_string());
            }
        }

        Some(attachment)
    }

    async fn invalidate_tools_cache_and_bump_version(&self) {
        self.tools_cache_version.fetch_add(1, Ordering::SeqCst);
        *self.tools_cache.lock().await = None;
    }

    async fn fetch_all_tools(&self, session_id: &str) -> ExtensionResult<Vec<Tool>> {
        let clients: Vec<_> = self
            .extensions
            .lock()
            .await
            .iter()
            .map(|(name, ext)| (name.clone(), ext.config.clone(), ext.get_client()))
            .collect();

        let cancel_token = CancellationToken::default();
        let client_futures = clients.into_iter().map(|(name, config, client)| {
            let cancel_token = cancel_token.clone();
            let ext_name = name.clone();
            async move {
                let mut tools = Vec::new();
                let client_tools =
                    match collect_paginated_tools(&client, session_id, cancel_token).await {
                        Ok(tools) => tools,
                        Err(e) => {
                            warn!(extension = %ext_name, error = %e, "Failed to list tools");
                            return Err((name, e));
                        }
                    };

                let expose_unprefixed = is_unprefixed_extension(&config);

                for mut tool in client_tools {
                    if config.is_tool_available(&tool.name) {
                        let public_name = if expose_unprefixed {
                            tool.name.to_string()
                        } else {
                            format!("{}__{}", name, tool.name)
                        };

                        let mut meta_map =
                            tool.meta.as_ref().map(|m| m.0.clone()).unwrap_or_default();
                        meta_map.insert(
                            TOOL_EXTENSION_META_KEY.to_string(),
                            serde_json::Value::String(name.clone()),
                        );

                        tool.name = public_name.into();
                        tool.meta = Some(rmcp::model::Meta(meta_map));

                        tools.push(tool);
                    }
                }

                Ok((name, tools))
            }
        });

        let results = future::join_all(client_futures).await;
        let errors = results
            .iter()
            .filter_map(|result| result.as_ref().err().cloned())
            .collect::<Vec<_>>();
        if !errors.is_empty() {
            let detail = errors
                .into_iter()
                .map(|(name, error)| format!("{name}: {error}"))
                .collect::<Vec<_>>()
                .join("; ");
            return Err(ExtensionError::SetupError(format!(
                "Failed to enumerate extension tools: {detail}"
            )));
        }

        let mut seen_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut tools = Vec::new();
        for (ext_name, client_tools) in results.into_iter().flatten() {
            for tool in client_tools {
                let tool_name = tool.name.to_string();
                if seen_names.contains(&tool_name) {
                    warn!(
                        tool = %tool_name,
                        extension = %ext_name,
                        "Duplicate tool name - skipping"
                    );
                    continue;
                }
                seen_names.insert(tool_name);
                tools.push(tool);
            }
        }

        Ok(tools)
    }

    /// Get the extension prompt including client instructions
    pub async fn get_planning_prompt(&self, tools_info: Vec<ToolInfo>) -> String {
        let mut context: HashMap<&str, Value> = HashMap::new();
        context.insert(
            "tools",
            serde_json::to_value(tools_info).unwrap_or_default(),
        );

        prompt_template::render_template("plan.md", &context).unwrap_or_else(|e| {
            tracing::error!("Failed to render planning prompt: {e}");
            String::new()
        })
    }

    // Function that gets executed for read_resource tool
    pub async fn read_resource_tool(
        &self,
        session_id: &str,
        params: Value,
        cancellation_token: CancellationToken,
    ) -> Result<Vec<Content>, ErrorData> {
        let uri = require_str_parameter(&params, "uri")?;
        let extension_name = require_str_parameter(&params, "extension_name")?;

        let read_result = self
            .read_resource(session_id, uri, extension_name, cancellation_token)
            .await?;

        let mut result = Vec::new();
        for content in read_result.contents {
            if let ResourceContents::TextResourceContents { text, .. } = content {
                result.push(Content::text(format!("{}\n\n{}", uri, text)));
            }
        }
        Ok(result)
    }

    pub async fn read_resource(
        &self,
        session_id: &str,
        uri: &str,
        extension_name: &str,
        cancellation_token: CancellationToken,
    ) -> Result<rmcp::model::ReadResourceResult, ErrorData> {
        let available_extensions = self
            .extensions
            .lock()
            .await
            .keys()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>()
            .join(", ");
        let error_msg = format!(
            "Extension '{}' not found. Here are the available extensions: {}",
            extension_name, available_extensions
        );

        let client = self
            .get_server_client(extension_name)
            .await
            .ok_or(ErrorData::new(ErrorCode::INVALID_PARAMS, error_msg, None))?;

        client
            .read_resource(session_id, uri, cancellation_token)
            .await
            .map_err(|_| {
                ErrorData::new(
                    ErrorCode::INTERNAL_ERROR,
                    format!("Could not read resource with uri: {}", uri),
                    None,
                )
            })
    }

    pub async fn get_ui_resources(
        &self,
        session_id: &str,
    ) -> Result<Vec<(String, Resource)>, ErrorData> {
        let mut ui_resources = Vec::new();

        let extensions_to_check: Vec<(String, McpClientBox)> = {
            let extensions = self.extensions.lock().await;
            extensions
                .iter()
                .map(|(name, ext)| (name.clone(), ext.get_client()))
                .collect()
        };

        for (extension_name, client) in extensions_to_check {
            match collect_paginated_resources(&client, session_id, CancellationToken::default())
                .await
            {
                Ok(resources) => {
                    for resource in resources {
                        if resource.uri.starts_with("ui://") {
                            ui_resources.push((extension_name.clone(), resource));
                        }
                    }
                }
                Err(e) => {
                    warn!("Failed to list resources for {}: {:?}", extension_name, e);
                }
            }
        }

        Ok(ui_resources)
    }

    async fn list_resources_from_extension(
        &self,
        session_id: &str,
        extension_name: &str,
        cancellation_token: CancellationToken,
    ) -> Result<Vec<Content>, ErrorData> {
        let client = self
            .get_server_client(extension_name)
            .await
            .ok_or_else(|| {
                ErrorData::new(
                    ErrorCode::INVALID_PARAMS,
                    format!("Extension {} is not valid", extension_name),
                    None,
                )
            })?;

        collect_paginated_resources(&client, session_id, cancellation_token)
            .await
            .map_err(|e| {
                ErrorData::new(
                    ErrorCode::INTERNAL_ERROR,
                    format!("Unable to list resources for {}, {:?}", extension_name, e),
                    None,
                )
            })
            .map(|resources| {
                let resource_list = resources
                    .into_iter()
                    .map(|r| format!("{} - {}, uri: ({})", extension_name, r.name, r.uri))
                    .collect::<Vec<String>>()
                    .join("\n");

                vec![Content::text(resource_list)]
            })
    }

    pub async fn list_resources(
        &self,
        session_id: &str,
        params: Value,
        cancellation_token: CancellationToken,
    ) -> Result<Vec<Content>, ErrorData> {
        let extension = params.get("extension_name").and_then(|v| v.as_str());

        match extension {
            Some(extension_name) => {
                // Handle single extension case
                self.list_resources_from_extension(session_id, extension_name, cancellation_token)
                    .await
            }
            None => {
                // Handle all extensions case using FuturesUnordered
                let mut futures = FuturesUnordered::new();

                // Create futures for each resource_capable_extension
                self.extensions
                    .lock()
                    .await
                    .iter()
                    .filter(|(_name, ext)| ext.supports_resources())
                    .map(|(name, _ext)| name.clone())
                    .for_each(|name| {
                        let token = cancellation_token.clone();
                        futures.push(async move {
                            self.list_resources_from_extension(session_id, name.as_str(), token)
                                .await
                        });
                    });

                let mut all_resources = Vec::new();
                let mut errors = Vec::new();

                // Process results as they complete
                while let Some(result) = futures.next().await {
                    match result {
                        Ok(content) => {
                            all_resources.extend(content);
                        }
                        Err(tool_error) => {
                            errors.push(tool_error);
                        }
                    }
                }

                if !errors.is_empty() {
                    tracing::error!(
                        errors = ?errors
                            .into_iter()
                            .map(|e| format!("{:?}", e))
                            .collect::<Vec<_>>(),
                        "errors from listing resources"
                    );
                }

                Ok(all_resources)
            }
        }
    }

    async fn resolve_tool(
        &self,
        session_id: &str,
        tool_name: &str,
    ) -> Result<ResolvedTool, ErrorData> {
        let tools = self.get_all_tools_cached(session_id).await.map_err(|e| {
            ErrorData::new(
                ErrorCode::INTERNAL_ERROR,
                format!("Failed to get tools: {}", e),
                None,
            )
        })?;

        if let Some(tool) = tools.iter().find(|t| *t.name == *tool_name) {
            let owner = get_tool_owner(tool)
                .or_else(|| {
                    tool_name
                        .split_once("__")
                        .map(|(prefix, _)| name_to_key(prefix))
                })
                .ok_or_else(|| {
                    ErrorData::new(
                        ErrorCode::RESOURCE_NOT_FOUND,
                        format!("Tool '{}' has no owner", tool_name),
                        None,
                    )
                })?;

            let actual_tool_name = tool_name
                .strip_prefix(&format!("{owner}__"))
                .unwrap_or(tool_name)
                .to_string();

            let client = self.get_server_client(&owner).await.ok_or_else(|| {
                ErrorData::new(
                    ErrorCode::RESOURCE_NOT_FOUND,
                    format!("Extension '{}' not found for tool '{}'", owner, tool_name),
                    None,
                )
            })?;

            return Ok(ResolvedTool {
                tool_name: tool.name.to_string(),
                extension_name: owner,
                actual_tool_name,
                client,
                tool_meta: get_tool_meta_value(tool),
                resource_uri: get_tool_resource_uri(tool),
            });
        }

        if let Some((prefix, actual)) = tool_name.split_once("__") {
            let owner = name_to_key(prefix);
            if let Some(client) = self.get_server_client(&owner).await {
                return Ok(ResolvedTool {
                    tool_name: tool_name.to_string(),
                    extension_name: owner,
                    actual_tool_name: actual.to_string(),
                    client,
                    tool_meta: None,
                    resource_uri: None,
                });
            }
        }

        let available = tools
            .iter()
            .map(|t| t.name.as_ref())
            .collect::<Vec<&str>>()
            .join(", ");

        Err(ErrorData::new(
            ErrorCode::RESOURCE_NOT_FOUND,
            format!(
                "Tool '{}' not found. Available tools: [{}]",
                tool_name, available
            ),
            None,
        ))
    }

    pub async fn dispatch_tool_call(
        &self,
        ctx: &super::tool_execution::ToolCallContext,
        tool_call: CallToolRequestParams,
        cancellation_token: CancellationToken,
    ) -> Result<ToolCallResult> {
        let tool_name_str = tool_call.name.to_string();
        let resolved = self.resolve_tool(&ctx.session_id, &tool_name_str).await?;

        if let Some(extension) = self.extensions.lock().await.get(&resolved.extension_name) {
            if !extension
                .config
                .is_tool_available(&resolved.actual_tool_name)
            {
                return Err(ErrorData::new(
                    ErrorCode::RESOURCE_NOT_FOUND,
                    format!(
                        "Tool '{}' is not available for extension '{}'",
                        resolved.actual_tool_name, resolved.extension_name
                    ),
                    None,
                )
                .into());
            }
        }

        let arguments = tool_call.arguments.clone();
        let client = resolved.client.clone();
        let hydration_client = client.clone();
        let notifications_receiver = client.subscribe().await;
        let session_id = ctx.session_id.clone();
        let action_required_tool_call_request_id = ctx.tool_call_request_id.clone();
        let action_required_receiver =
            if let Some(tool_call_request_id) = action_required_tool_call_request_id.clone() {
                if ActionRequiredManager::global()
                    .has_action_required_stream(&session_id, &tool_call_request_id)
                    .await
                {
                    None
                } else {
                    let registered_tool_call_request_id = tool_call_request_id.clone();
                    let receiver = ActionRequiredManager::global()
                        .register_action_required_stream(session_id.clone(), tool_call_request_id)
                        .await;
                    Some((
                        receiver,
                        session_id.clone(),
                        registered_tool_call_request_id,
                    ))
                }
            } else {
                None
            };
        let actual_tool_name = resolved.actual_tool_name.clone();
        let resolved_tool = resolved;
        let should_hydrate_mcp_app = self.host_supports_mcp_apps();
        let read_cancellation_token = cancellation_token.clone();
        let owned_ctx = ToolCallContext::new(
            ctx.session_id.clone(),
            ctx.working_dir.clone(),
            ctx.tool_call_request_id.clone(),
        );

        let fut = async move {
            tracing::debug!(
                "dispatch_tool_call: calling client.call_tool tool={} session_id={} working_dir={:?}",
                actual_tool_name,
                owned_ctx.session_id,
                owned_ctx.working_dir,
            );
            let call_result = client
                .call_tool(&owned_ctx, &actual_tool_name, arguments, cancellation_token)
                .await
                .map_err(|e| match e {
                    ServiceError::McpError(error_data) => error_data,
                    _ => {
                        ErrorData::new(ErrorCode::INTERNAL_ERROR, e.to_string(), e.maybe_to_value())
                    }
                });

            let mut result = call_result?;

            remove_untrusted_mcp_app_meta(&mut result);

            if should_hydrate_mcp_app && result.is_error != Some(true) {
                if let Some(attachment) = Self::hydrate_mcp_app_attachment(
                    &hydration_client,
                    &session_id,
                    &resolved_tool,
                    read_cancellation_token,
                )
                .await
                {
                    insert_trusted_tool_update_meta(&mut result, &attachment);
                }
            }

            Ok(result)
        };

        Ok(ToolCallResult {
            result: Box::new(fut.boxed()),
            notification_stream: Some(Box::new(ReceiverStream::new(notifications_receiver))),
            action_required_stream: action_required_receiver.map(
                |(rx, session_id, tool_call_request_id)| {
                    Box::new(ActionRequiredStream::new(
                        rx,
                        session_id,
                        tool_call_request_id,
                    )) as _
                },
            ),
        })
    }

    pub async fn list_prompts_from_extension(
        &self,
        session_id: &str,
        extension_name: &str,
        cancellation_token: CancellationToken,
    ) -> Result<Vec<Prompt>, ErrorData> {
        let client = self
            .get_server_client(extension_name)
            .await
            .ok_or_else(|| {
                ErrorData::new(
                    ErrorCode::INVALID_PARAMS,
                    format!("Extension {} is not valid", extension_name),
                    None,
                )
            })?;

        collect_paginated_prompts(&client, session_id, cancellation_token)
            .await
            .map_err(|e| {
                ErrorData::new(
                    ErrorCode::INTERNAL_ERROR,
                    format!("Unable to list prompts for {}, {:?}", extension_name, e),
                    None,
                )
            })
    }

    pub async fn list_prompts(
        &self,
        session_id: &str,
        cancellation_token: CancellationToken,
    ) -> Result<HashMap<String, Vec<Prompt>>, ErrorData> {
        let mut futures = FuturesUnordered::new();

        let names: Vec<_> = self.extensions.lock().await.keys().cloned().collect();
        for extension_name in names {
            let token = cancellation_token.clone();
            futures.push(async move {
                (
                    extension_name.clone(),
                    self.list_prompts_from_extension(session_id, extension_name.as_str(), token)
                        .await,
                )
            });
        }

        let mut all_prompts = HashMap::new();
        let mut errors = Vec::new();

        // Process results as they complete
        while let Some(result) = futures.next().await {
            let (name, prompts) = result;
            match prompts {
                Ok(content) => {
                    all_prompts.insert(name.to_string(), content);
                }
                Err(tool_error) => {
                    errors.push(tool_error);
                }
            }
        }

        if !errors.is_empty() {
            tracing::debug!(
                errors = ?errors
                    .into_iter()
                    .map(|e| format!("{:?}", e))
                    .collect::<Vec<_>>(),
                "errors from listing prompts"
            );
        }

        Ok(all_prompts)
    }

    pub async fn get_prompt(
        &self,
        session_id: &str,
        extension_name: &str,
        name: &str,
        arguments: Value,
        cancellation_token: CancellationToken,
    ) -> Result<GetPromptResult> {
        let client = self
            .get_server_client(extension_name)
            .await
            .ok_or_else(|| anyhow::anyhow!("Extension {} not found", extension_name))?;

        client
            .get_prompt(session_id, name, arguments, cancellation_token)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to get prompt: {}", e))
    }

    pub async fn search_available_extensions(&self) -> Result<Vec<Content>, ErrorData> {
        let mut output_parts = vec![];

        // First get disabled extensions from current config (skip hidden ones)
        let mut disabled_extensions: Vec<String> = vec![];
        for extension in get_all_extensions() {
            if !extension.enabled && !is_hidden_extension(&extension.config.name()) {
                let config = extension.config.clone();
                let description = match &config {
                    ExtensionConfig::Builtin {
                        description,
                        display_name,
                        ..
                    } => {
                        if description.is_empty() {
                            display_name.as_deref().unwrap_or("Built-in extension")
                        } else {
                            description
                        }
                    }
                    ExtensionConfig::Sse { .. } => "SSE extension (unsupported)",
                    ExtensionConfig::Platform { description, .. }
                    | ExtensionConfig::StreamableHttp { description, .. }
                    | ExtensionConfig::Stdio { description, .. }
                    | ExtensionConfig::Frontend { description, .. }
                    | ExtensionConfig::InlinePython { description, .. } => description,
                };
                disabled_extensions.push(format!("- {} - {}", config.name(), description));
            }
        }

        // Get currently enabled extensions that can be disabled (skip hidden ones)
        let enabled_extensions: Vec<String> = self
            .extensions
            .lock()
            .await
            .keys()
            .filter(|name| !is_hidden_extension(name))
            .cloned()
            .collect();

        // Build output string
        if !disabled_extensions.is_empty() {
            output_parts.push(format!(
                "Extensions available to enable:\n{}\n",
                disabled_extensions.join("\n")
            ));
        } else {
            output_parts.push("No extensions available to enable.\n".to_string());
        }

        if !enabled_extensions.is_empty() {
            output_parts.push(format!(
                "\n\nExtensions available to disable:\n{}\n",
                enabled_extensions
                    .iter()
                    .map(|name| format!("- {}", name))
                    .collect::<Vec<_>>()
                    .join("\n")
            ));
        } else {
            output_parts.push("No extensions that can be disabled.\n".to_string());
        }

        Ok(vec![Content::text(output_parts.join("\n"))])
    }

    async fn get_server_client(&self, name: impl Into<String>) -> Option<McpClientBox> {
        let normalized = name_to_key(&name.into());
        self.extensions
            .lock()
            .await
            .get(&normalized)
            .map(|ext| ext.get_client())
    }

    pub async fn collect_moim_parts(&self, session_id: &str) -> Vec<String> {
        let platform_clients: Vec<(String, McpClientBox)> = {
            let extensions = self.extensions.lock().await;
            extensions
                .iter()
                .filter_map(|(name, extension)| {
                    let is_platform = match &extension.config {
                        ExtensionConfig::Platform { .. } => true,
                        ExtensionConfig::Builtin { name: ext_name, .. } => {
                            PLATFORM_EXTENSIONS.contains_key(name_to_key(ext_name).as_str())
                        }
                        _ => false,
                    };
                    if is_platform {
                        Some((name.clone(), extension.get_client()))
                    } else {
                        None
                    }
                })
                .collect()
        };

        let mut parts = Vec::new();
        for (name, client) in platform_clients {
            if let Some(moim_content) = client.get_moim(session_id).await {
                tracing::debug!("MOIM content from {}: {} chars", name, moim_content.len());
                parts.push(moim_content);
            }
        }
        parts
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
    /// process it started inside the container (Extension's Drop impl
    /// invoking `DockerExecProcess::kill`), not just the local `docker exec`
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
        struct ContainerGuard(String);
        impl Drop for ContainerGuard {
            fn drop(&mut self) {
                let name = self.0.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let _ = tokio::process::Command::new("docker")
                            .args(["rm", "-f", &name])
                            .kill_on_drop(true)
                            .output()
                            .await;
                    });
                }
            }
        }
        let _guard = ContainerGuard(container_name.clone());

        let argv = vec!["sleep".to_string(), "300".to_string()];
        let exec = tokio::process::Command::new("docker")
            .arg("exec")
            .arg("-d")
            .arg(&container_name)
            .args(&argv)
            .kill_on_drop(true)
            .output()
            .await
            .expect("failed to invoke docker exec");
        assert!(
            exec.status.success(),
            "failed to start process in container"
        );

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

        // Extension::drop spawns the cleanup as a detached task; give it a
        // moment to run.
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;

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
