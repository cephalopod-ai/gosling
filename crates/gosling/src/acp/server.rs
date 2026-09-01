//! ACP server compatibility facade over protocol, session, tool, shell, and transport modules.
//!
//! Maintainers: keep public paths here while delegating cohesive behavior to sibling modules.
//! Clients: request schemas, notification ordering, errors, and response bounds remain stable.

use crate::acp::custom_notifications::*;
use crate::acp::custom_requests::*;
use crate::acp::fs::AcpTools;
pub(super) use crate::acp::response_builder::{
    build_config_options, build_mode_state, build_model_state, build_provider_options,
    build_session_info, build_session_setup_config, compatible_mode,
    send_session_setup_notifications, session_meta, session_provider_selection,
    session_response_meta, should_refresh_inventory_for_session_init,
};
use crate::acp::shell::ShellRuntime;
use crate::acp::tools::AcpAwareToolMeta;
use crate::acp::{PermissionDecision, ACP_CURRENT_MODEL};
use crate::agents::extension::{Envs, PLATFORM_EXTENSIONS};
use crate::agents::extension_manager::TRUSTED_TOOL_UPDATE_META_KEY;
use crate::agents::mcp_client::{GoslingMcpHostInfo, McpClientTrait};
use crate::agents::platform_extensions::developer::DeveloperClient;
use crate::agents::{
    Agent, AgentConfig, ExtensionConfig, ExtensionLoadResult, GoslingPlatform, SessionConfig,
};
use crate::config::base::CONFIG_YAML_NAME;
use crate::config::extensions::{
    get_enabled_extensions_with_config_for_cwd, is_builtin_disabled_by_user,
};
use crate::config::paths::Paths;
use crate::config::paths::RuntimePaths;
use crate::config::permission::PermissionManager;
use crate::config::{Config, GoslingMode};
use crate::conversation::message::{
    ActionRequiredData, Message, MessageContent, SystemNotificationContent, SystemNotificationType,
    ToolRequest,
};
use crate::execution::manager::{AgentManager, AgentManagerGetResult, RuntimeContext};
use crate::mcp_utils::ToolResult;
use crate::permission::permission_confirmation::PrincipalType;
use crate::permission::{Permission, PermissionConfirmation};
use crate::providers::base::Provider;
use crate::providers::inventory::{
    ProviderInventoryEntry, ProviderInventoryService, RefreshJobPlan, RefreshPlan,
    RefreshSkipReason,
};
use crate::session::{
    AcpPromptRunState, EnabledExtensionsState, ExtensionData, ExtensionState,
    NewSessionLibraryContent, Session, SessionArtifact, SessionArtifactProvenance,
    SessionArtifactRelation, SessionLibraryItem, SessionLibraryItemKind, SessionLibraryScope,
    SessionManager, SessionType, DEFAULT_SESSION_TAIL_LIMIT, MAX_SESSION_MESSAGE_PAGE_LIMIT,
};
use crate::source_roots::SourceRoot;
use crate::utils::sanitize_unicode_tags;
use crate::workspace::WorkspaceService;
use agent_client_protocol::schema::v1::{
    AgentCapabilities, Annotations, AuthMethod, AuthMethodAgent, AuthenticateRequest,
    AuthenticateResponse, BlobResourceContents, CancelNotification, CloseSessionRequest,
    CloseSessionResponse, ConfigOptionUpdate, Content, ContentBlock, ContentChunk, Cost,
    CurrentModeUpdate, EmbeddedResource, EmbeddedResourceResource, FileSystemCapabilities,
    ForkSessionRequest, ForkSessionResponse, ImageContent, Implementation, InitializeRequest,
    InitializeResponse, ListSessionsRequest, ListSessionsResponse, LoadSessionRequest,
    LoadSessionResponse, McpCapabilities, McpServer, Meta, NewSessionRequest, NewSessionResponse,
    PermissionOption, PermissionOptionKind, PromptCapabilities, PromptRequest, PromptResponse,
    RequestPermissionOutcome, RequestPermissionRequest, ResourceLink, SessionCapabilities,
    SessionCloseCapabilities, SessionConfigOption, SessionId, SessionInfoUpdate,
    SessionListCapabilities, SessionNotification, SessionUpdate, SetSessionConfigOptionRequest,
    SetSessionConfigOptionResponse, SetSessionModeRequest, SetSessionModeResponse, StopReason,
    TextContent, TextResourceContents, ToolCall, ToolCallContent, ToolCallId, ToolCallLocation,
    ToolCallStatus, ToolCallUpdate, ToolCallUpdateFields, ToolKind, Usage, UsageUpdate,
};
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::util::MatchDispatchFrom;
use agent_client_protocol::{
    Agent as SacpAgent, ByteStreams, Client, ConnectionTo, Dispatch, HandleDispatchFrom, Handled,
    Responder,
};
use anyhow::Result;
use fs_err as fs;
use futures::channel::oneshot;
use futures::future::{select, BoxFuture, Either, FutureExt};
use futures::stream::{self, StreamExt};
use futures::AsyncRead;
use rmcp::model::{
    AnnotateAble, CallToolResult, RawContent, RawTextContent, ResourceContents, Role,
};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use tokio::sync::{Mutex, OnceCell};
use tokio_util::compat::{TokioAsyncReadCompatExt as _, TokioAsyncWriteCompatExt as _};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, warn};
use url::Url;
use uuid::Uuid;

mod agent_requests;
pub use agent_requests::agent_request_schemas;
mod active_runs;
mod agent_mentions;
mod config;
mod custom_dispatch;
mod diagnostics;
mod dictation;
mod dispatch;
mod elicitation;
mod extension_selection;
mod extensions;
mod fork_session;
mod initialization;
mod list_sessions;
mod load_session;
mod manage_sessions;
mod message_projection;
mod new_session;
mod onboarding;
mod presentation;
mod prompt_execution;
mod prompts;
mod providers;
mod research_completion;
mod resources;
mod session_activation;
mod session_configuration;
mod shell_handlers;
mod shell_library_formats;
mod slash_commands;

use prompt_execution::build_prompt_usage;
pub(super) use prompt_execution::build_usage_updates;
use session_configuration::{
    resolve_default_provider_model_config, resolve_provider_default_model_config,
};
mod sources;
mod tool_metadata;
mod tool_notifications;
mod tools;
mod workspace_handlers;

use active_runs::ActivePromptRun;
#[cfg(test)]
use active_runs::{register_active_prompt_run, unregister_active_prompt_run};
pub(crate) use extension_selection::{
    apply_shell_extension_selection, push_or_replace_extension, selected_builtin_extensions,
};
use extension_selection::{
    builtin_to_extension_config, mcp_server_to_extension_config, rehydrate_configured_envs,
};
use initialization::{
    custom_method_names, extract_client_capabilities_meta,
    extract_client_supports_gosling_custom_notifications,
};
use message_projection::{
    build_tool_call_content, extract_tool_call_update_meta, extract_tool_raw_output,
    merge_replay_message_meta, message_update_meta, outcome_to_confirmation,
    prompt_error_from_message_content, replay_message_meta, send_status_message_update,
    session_artifact_dto,
};
use tool_metadata::{
    extend_chain_membership, extract_locations_from_meta, extract_tool_locations, format_tool_name,
    pending_tool_call_from_request, read_resource_link, tool_call_identity_meta,
    with_tool_chain_summary_meta,
};
#[cfg(test)]
use tool_metadata::{get_requested_line, is_developer_file_tool, summarize_tool_call};

pub type AcpProviderFactory = Arc<
    dyn Fn(
            String,
            Vec<ExtensionConfig>,
            Option<PathBuf>,
        ) -> BoxFuture<'static, Result<Arc<dyn Provider>>>
        + Send
        + Sync,
>;

/// Convenience conversions from any `Display` error into an `agent_client_protocol::Error`.
///
/// Replaces the repetitive `.internal_err()`
/// pattern. Use `.internal_err()?` for server-side failures and `.invalid_params_err()?`
/// for bad client input. For custom messages use `.internal_err_ctx("context")?`.
#[allow(dead_code)]
trait ResultExt<T> {
    fn internal_err(self) -> Result<T, agent_client_protocol::Error>;
    fn invalid_params_err(self) -> Result<T, agent_client_protocol::Error>;
    fn internal_err_ctx(self, context: &str) -> Result<T, agent_client_protocol::Error>;
    fn invalid_params_err_ctx(self, context: &str) -> Result<T, agent_client_protocol::Error>;
}

impl<T, E: std::fmt::Display> ResultExt<T> for Result<T, E> {
    fn internal_err(self) -> Result<T, agent_client_protocol::Error> {
        self.map_err(|e| agent_client_protocol::Error::internal_error().data(e.to_string()))
    }
    fn invalid_params_err(self) -> Result<T, agent_client_protocol::Error> {
        self.map_err(|e| agent_client_protocol::Error::invalid_params().data(e.to_string()))
    }
    fn internal_err_ctx(self, context: &str) -> Result<T, agent_client_protocol::Error> {
        self.map_err(|e| {
            agent_client_protocol::Error::internal_error().data(format!("{context}: {e}"))
        })
    }
    fn invalid_params_err_ctx(self, context: &str) -> Result<T, agent_client_protocol::Error> {
        self.map_err(|e| {
            agent_client_protocol::Error::invalid_params().data(format!("{context}: {e}"))
        })
    }
}

pub(super) const DEFAULT_PROVIDER_ID: &str = "gosling";
pub(super) const DEFAULT_PROVIDER_LABEL: &str = "Gosling (Default)";
const PROVIDER_CONFIG_STATUS_CHECK_CONCURRENCY: usize = 16;

/// In-memory state for an active ACP session.
///
/// ## Terminology (temporary, until all clients migrate to ACP)
///
/// The ACP protocol uses "session" to mean the conversation as the human sees it —
/// a durable, append-only exchange of messages. Internally, gosling also has a concept
/// called "Session" (the `sessions` DB table) which represents the agent's working
/// state: the message list the LLM sees, compaction state, provider binding, etc.
///
/// The ACP session ID maps directly to a `sessions` row. The `sessions` HashMap
/// below is keyed by session ID.
struct GoslingAcpSession {
    agent: Arc<Agent>,
    tool_requests: HashMap<String, crate::conversation::message::ToolRequest>,
    compacted_context: bool,
    tail_limit: usize,
    /// For each tool_call_id that belongs to a multi-tool chain (run of
    /// consecutive ToolRequest blocks within one assistant message), the chain
    /// it belongs to. Populated when the assistant message is processed.
    /// Used by `handle_tool_response` to detect when a chain has fully
    /// completed and fire a single LLM summary covering the run.
    chain_membership: HashMap<String, Arc<ToolChain>>,
    /// Set of tool_call_ids whose ToolResponse has already been processed.
    /// Drives the "all responses present" check for chain completion.
    responded_tool_ids: HashSet<String>,
    /// Tool_call_ids of chains that have already had a summary task fired.
    /// Idempotence guard so we summarize each chain at most once.
    summarized_chains: HashSet<String>,
}

/// A run of consecutive ToolRequest blocks within one assistant message,
/// tracked by [`GoslingAcpSession::chain_membership`]. Used to drive a single
/// LLM summary for the whole run once every step has a recorded ToolResponse.
#[derive(Debug, Clone)]
struct ToolChain {
    /// Tool call ids in document order. Always `len() >= 2`.
    ids: Vec<String>,
    /// The message_id of the assistant message containing these tool calls.
    /// Used to persist chain summaries back to the messages table.
    message_id: String,
}

pub struct GoslingAcpAgentOptions {
    pub state_dir: PathBuf,
    pub provider_factory: AcpProviderFactory,
    pub builtins: Vec<String>,
    pub data_dir: std::path::PathBuf,
    pub platform_data_dir: std::path::PathBuf,
    pub config_dir: std::path::PathBuf,
    pub disable_session_naming: bool,
    pub gosling_platform: GoslingPlatform,
    pub additional_source_roots: Vec<SourceRoot>,
    pub shell_runtime: ShellRuntime,
}

pub struct GoslingAcpAgent {
    runtime_paths: RuntimePaths,
    sessions: Arc<Mutex<HashMap<String, GoslingAcpSession>>>,
    active_prompt_runs: Arc<Mutex<HashMap<String, ActivePromptRun>>>,
    closed_session_ids: Arc<Mutex<HashSet<String>>>,
    agent_manager: Arc<AgentManager>,
    provider_factory: AcpProviderFactory,
    builtins: Vec<String>,
    client_fs_capabilities: OnceCell<FileSystemCapabilities>,
    client_terminal: OnceCell<bool>,
    client_mcp_host_info: OnceCell<GoslingMcpHostInfo>,
    client_supports_acp_elicitation: OnceCell<bool>,
    client_supports_gosling_custom_notifications: OnceCell<bool>,
    use_login_shell_path: OnceCell<bool>,
    client_cx: OnceCell<ConnectionTo<Client>>,
    config_dir: std::path::PathBuf,
    session_manager: Arc<SessionManager>,
    permission_manager: Arc<PermissionManager>,
    disable_session_naming: bool,
    provider_inventory: ProviderInventoryService,
    additional_source_roots: Vec<SourceRoot>,
    workspace_service: Arc<WorkspaceService>,
    default_working_folder: PathBuf,
    shell_runtime: ShellRuntime,
    shell_credential_lookup_cooldown_until: std::sync::Mutex<Option<std::time::Instant>>,
}

/// Shorten a session/thread id for perf log correlation.
/// All `perf:` logs use `sid=<8-char-prefix>` so a single session's activity
/// can be extracted with `grep 'perf:' <log> | grep 'sid=abc12345'`.
pub(super) fn sid_short(id: &str) -> String {
    id.chars().take(8).collect()
}

fn meta_string(
    meta: Option<&Meta>,
    key: &str,
) -> Result<Option<String>, agent_client_protocol::Error> {
    let Some(value) = meta.and_then(|m| m.get(key)) else {
        return Ok(None);
    };
    if value.is_null() {
        return Ok(None);
    }
    let Some(value) = value.as_str() else {
        return Err(
            agent_client_protocol::Error::invalid_params().data(format!("{key} must be a string"))
        );
    };
    Ok(Some(value.to_string()))
}

#[derive(Debug, Clone, Copy)]
struct SessionLoadOptions {
    compacted: bool,
    tail_limit: usize,
}

fn compacted_load_options_from_meta(
    meta: Option<&Meta>,
) -> Result<SessionLoadOptions, agent_client_protocol::Error> {
    let Some(gosling) = meta
        .and_then(|m| m.get("gosling"))
        .and_then(|value| value.as_object())
    else {
        return Ok(SessionLoadOptions {
            compacted: false,
            tail_limit: DEFAULT_SESSION_TAIL_LIMIT,
        });
    };

    let load_mode = gosling
        .get("loadMode")
        .and_then(|value| value.as_str())
        .unwrap_or("full");
    let compacted = match load_mode {
        "compacted" => true,
        "full" => false,
        other => {
            return Err(agent_client_protocol::Error::invalid_params().data(format!(
                "gosling.loadMode must be 'full' or 'compacted', got {other}"
            )));
        }
    };

    let tail_limit = match gosling.get("tailLimit") {
        Some(value) if value.is_null() => DEFAULT_SESSION_TAIL_LIMIT,
        Some(value) => {
            let Some(raw_limit) = value.as_u64() else {
                return Err(agent_client_protocol::Error::invalid_params()
                    .data("gosling.tailLimit must be a number"));
            };
            raw_limit
                .clamp(1, MAX_SESSION_MESSAGE_PAGE_LIMIT as u64)
                .try_into()
                .unwrap_or(DEFAULT_SESSION_TAIL_LIMIT)
        }
        None => DEFAULT_SESSION_TAIL_LIMIT,
    };

    Ok(SessionLoadOptions {
        compacted,
        tail_limit,
    })
}

fn spawn_session_name_update_notifier(
    cx: ConnectionTo<Client>,
) -> tokio::sync::mpsc::UnboundedSender<crate::session::SessionNameUpdate> {
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<crate::session::SessionNameUpdate>();
    tokio::spawn(async move {
        while let Some(update) = rx.recv().await {
            let mut meta = serde_json::Map::new();
            meta.insert(
                "messageCount".to_string(),
                serde_json::Value::Number(update.message_count.into()),
            );
            meta.insert(
                "userSetName".to_string(),
                serde_json::Value::Bool(update.user_set_name),
            );
            let notification = SessionNotification::new(
                SessionId::new(update.session_id.clone()),
                SessionUpdate::SessionInfoUpdate(
                    SessionInfoUpdate::new()
                        .title(update.name)
                        .updated_at(update.updated_at.to_rfc3339())
                        .meta(meta),
                ),
            );
            if let Err(error) = cx.send_notification(notification) {
                warn!(
                    session_id = %update.session_id,
                    error = %error,
                    "Failed to send generated session name update"
                );
            }
        }
    });
    tx
}

pub(super) fn validate_absolute_cwd(cwd: &Path) -> Result<(), agent_client_protocol::Error> {
    if !cwd.is_absolute() {
        return Err(
            agent_client_protocol::Error::invalid_params().data("cwd must be an absolute path")
        );
    }

    if !cwd.exists() || !cwd.is_dir() {
        return Err(agent_client_protocol::Error::invalid_params().data("invalid directory path"));
    }

    Ok(())
}

impl GoslingAcpAgent {
    pub fn permission_manager(&self) -> Arc<PermissionManager> {
        Arc::clone(&self.permission_manager)
    }

    pub(super) fn supports_gosling_custom_notifications(&self) -> bool {
        self.client_supports_gosling_custom_notifications
            .get()
            .copied()
            .unwrap_or(false)
    }

    fn supports_acp_elicitation(&self) -> bool {
        self.client_supports_acp_elicitation
            .get()
            .copied()
            .unwrap_or(false)
    }

    // TODO[POLISH-20260827-006]: gosling reads Paths::in_state_dir globally (e.g. RequestLog), ignoring this data_dir.
    pub async fn new(options: GoslingAcpAgentOptions) -> Result<Self> {
        let runtime_paths = RuntimePaths::new(
            options.config_dir.clone(),
            options.data_dir.clone(),
            options.state_dir.clone(),
        );
        let agent_runtime_paths = runtime_paths.clone();

        Paths::scope(runtime_paths, async move {
            let default_working_folder = std::env::var_os("GOSLING_WORKING_DIR")
                .map(PathBuf::from)
                .filter(|path| path.is_absolute())
                .or_else(|| std::env::current_dir().ok())
                .unwrap_or_else(|| PathBuf::from("/"));
            let workspace_service = Arc::new(
                WorkspaceService::initialize(&options.platform_data_dir, &default_working_folder)
                    .await?,
            );
            let session_manager = Arc::new(SessionManager::new(options.data_dir));

            session_manager.storage().pool().await?;

            let permission_manager = PermissionManager::for_config_dir(options.config_dir.clone());
            let provider_inventory =
                ProviderInventoryService::new(session_manager.storage().clone());
            let config = Config::global();
            let agent_config = AgentConfig::new(
                Arc::clone(&session_manager),
                Arc::clone(&permission_manager),
                config.get_gosling_mode().unwrap_or_default(),
                options.disable_session_naming,
                options.gosling_platform.clone(),
            )
            .with_code_execution_runtime(config.resolve_gosling_code_execution_runtime())
            .with_workspace_service(Arc::clone(&workspace_service));
            let agent_manager = Arc::new(AgentManager::new(agent_config, None).await?);

            Ok(Self {
                runtime_paths: agent_runtime_paths,
                sessions: Arc::new(Mutex::new(HashMap::new())),
                active_prompt_runs: Arc::new(Mutex::new(HashMap::new())),
                closed_session_ids: Arc::new(Mutex::new(HashSet::new())),
                agent_manager,
                provider_factory: options.provider_factory,
                builtins: options.builtins,
                client_fs_capabilities: OnceCell::new(),
                client_terminal: OnceCell::new(),
                client_mcp_host_info: OnceCell::new(),
                client_supports_acp_elicitation: OnceCell::new(),
                client_supports_gosling_custom_notifications: OnceCell::new(),
                use_login_shell_path: OnceCell::new(),
                client_cx: OnceCell::new(),
                config_dir: options.config_dir,
                session_manager,
                permission_manager,
                disable_session_naming: options.disable_session_naming,
                provider_inventory,
                additional_source_roots: options.additional_source_roots,
                workspace_service,
                default_working_folder,
                shell_runtime: options.shell_runtime,
                shell_credential_lookup_cooldown_until: std::sync::Mutex::new(None),
            })
        })
        .await
    }

    fn config(&self) -> Result<&'static Config, agent_client_protocol::Error> {
        Ok(Config::global())
    }

    async fn create_provider(
        &self,
        provider_name: &str,
        extensions: Vec<ExtensionConfig>,
        working_dir: Option<PathBuf>,
    ) -> Result<Arc<dyn Provider>> {
        (self.provider_factory)(provider_name.to_string(), extensions, working_dir).await
    }

    async fn maybe_refresh_provider_inventory_with_agent(
        &self,
        gosling_session: &Session,
        agent: &Arc<Agent>,
    ) {
        let Some(provider_name) = gosling_session.provider_name.as_deref() else {
            return;
        };
        let Some(mut inventory) = self
            .provider_inventory
            .find_entry_for_provider(provider_name)
            .await
        else {
            return;
        };
        if !should_refresh_inventory_for_session_init(&inventory) {
            return;
        }
        let provider = match agent.provider().await {
            Ok(provider) => provider,
            Err(error) => {
                warn!(
                    provider = %provider_name,
                    session = %gosling_session.id,
                    error = %error,
                    "agent has no provider available for inventory refresh"
                );
                return;
            }
        };
        self.provider_inventory
            .refresh_with_provider(provider_name, &provider, &mut inventory, "session init")
            .await;
    }

    async fn get_or_create_session_agent_with_results(
        &self,
        cx: &ConnectionTo<Client>,
        session_id: String,
    ) -> Result<AgentManagerGetResult, agent_client_protocol::Error> {
        self.agent_manager
            .get_or_create_agent_with_runtime_context(
                session_id,
                RuntimeContext {
                    mcp_host_info: self.client_mcp_host_info.get().cloned(),
                    use_login_shell_path: self.use_login_shell_path.get().copied(),
                    session_name_update_tx: (!self.disable_session_naming)
                        .then(|| spawn_session_name_update_notifier(cx.clone())),
                },
            )
            .await
            .internal_err_ctx("Failed to create agent")
    }

    #[allow(clippy::too_many_arguments)]
    async fn handle_message_content(
        &self,
        content_item: &MessageContent,
        session_id: &SessionId,
        session_id_str: &str,
        message_id: Option<&str>,
        message_created: i64,
        role: &Role,
        steer: bool,
        agent: &Arc<Agent>,
        session: &mut GoslingAcpSession,
        cx: &ConnectionTo<Client>,
    ) -> Result<(), agent_client_protocol::Error> {
        match content_item {
            MessageContent::Text(text) => {
                let chunk = ContentChunk::new(ContentBlock::Text(TextContent::new(
                    presentation::project_live_text(&text.text, "Message text"),
                )))
                .meta(message_update_meta(message_id, message_created, steer));
                let update = match role {
                    Role::User => SessionUpdate::UserMessageChunk(chunk),
                    Role::Assistant => SessionUpdate::AgentMessageChunk(chunk),
                };
                cx.send_notification(SessionNotification::new(session_id.clone(), update))?;
            }
            MessageContent::ToolRequest(tool_request) => {
                self.handle_tool_request(
                    tool_request,
                    session_id,
                    session_id_str,
                    message_id,
                    session,
                    cx,
                )
                .await?;
            }
            MessageContent::ToolResponse(tool_response) => {
                self.handle_tool_response(
                    tool_response,
                    session_id,
                    session_id_str,
                    message_id,
                    session,
                    cx,
                )
                .await?;
            }
            MessageContent::Thinking(thinking) => {
                cx.send_notification(SessionNotification::new(
                    session_id.clone(),
                    SessionUpdate::AgentThoughtChunk(
                        ContentChunk::new(ContentBlock::Text(TextContent::new(
                            presentation::project_live_text(&thinking.thinking, "Thinking content"),
                        )))
                        .meta(message_update_meta(
                            message_id,
                            message_created,
                            steer,
                        )),
                    ),
                ))?;
            }
            MessageContent::ActionRequired(action_required) => match &action_required.data {
                ActionRequiredData::ToolConfirmation {
                    id,
                    tool_name,
                    arguments,
                    prompt,
                    domain,
                } => {
                    self.handle_tool_permission_request(
                        cx,
                        agent,
                        session_id,
                        id.clone(),
                        tool_name.clone(),
                        arguments.clone(),
                        prompt.clone(),
                        domain.clone(),
                    )?;
                }
                ActionRequiredData::Elicitation {
                    id,
                    message,
                    requested_schema,
                } => {
                    self.handle_form_elicitation(
                        cx,
                        session_id,
                        id,
                        message,
                        requested_schema,
                        message_update_meta(message_id, message_created, false),
                    )
                    .await?;
                }
                ActionRequiredData::ElicitationResponse { .. } => {}
            },
            MessageContent::SystemNotification(notification) => {
                send_status_message_update(
                    cx,
                    self.supports_gosling_custom_notifications(),
                    session_id.0.as_ref(),
                    notification,
                )?;
            }
            _ => {}
        }
        Ok(())
    }

    async fn handle_tool_request(
        &self,
        tool_request: &crate::conversation::message::ToolRequest,
        session_id: &SessionId,
        session_id_for_persist: &str,
        message_id: Option<&str>,
        session: &mut GoslingAcpSession,
        cx: &ConnectionTo<Client>,
    ) -> Result<(), agent_client_protocol::Error> {
        session
            .tool_requests
            .insert(tool_request.id.clone(), tool_request.clone());

        let pending_tool_call = pending_tool_call_from_request(tool_request);
        let initial_tool_call = pending_tool_call
            .tool_call
            .meta(pending_tool_call.identity_meta.clone());
        cx.send_notification(SessionNotification::new(
            session_id.clone(),
            SessionUpdate::ToolCall(initial_tool_call),
        ))?;

        if Config::global()
            .get_gosling_disable_tool_call_summary()
            .unwrap_or(false)
        {
            return Ok(());
        }

        if let Ok(tool_call) = &tool_request.tool_call {
            let agent = session.agent.clone();
            let sid = session_id.clone();
            let request_id = tool_request.id.clone();
            let cx = cx.clone();
            let name = tool_call.name.to_string();
            let identity_meta = pending_tool_call.identity_meta.clone();
            let fallback_title = pending_tool_call.fallback_title.clone();
            let session_id_for_persist = session_id_for_persist.to_string();
            let message_id_for_persist = message_id.map(|s| s.to_string());
            let session_manager = self.session_manager.clone();
            let args_json = tool_call
                .arguments
                .as_ref()
                .map(|a| {
                    let s = serde_json::to_string(a).unwrap_or_default();
                    if s.len() > 300 {
                        format!("{}…", crate::utils::safe_truncate(&s, 300))
                    } else {
                        s
                    }
                })
                .unwrap_or_default();

            tokio::spawn(async move {
                let (title, from_llm) = match agent.provider().await {
                    Ok(provider) => {
                        if provider.manages_own_context() {
                            return;
                        }

                        let system =
                            "Summarize this tool call in a short lowercase phrase (3-8 words). \
                             No punctuation. No quotes. Examples: reading project configuration, \
                             checking network connectivity, listing files in src directory";
                        let user_text = format!("Tool: {name}\nArguments: {args_json}");
                        let message = Message::user().with_text(&user_text);
                        let model_config = match agent.model_config_for_session(&sid.0).await {
                            Ok(config) => config,
                            Err(_) => return,
                        };
                        let fast_model_config = match crate::model_config::get_fast_model(
                            provider.get_name(),
                            &model_config,
                        )
                        .await
                        {
                            Ok(config) => config,
                            Err(_) => return,
                        };
                        // The fast model occasionally returns an empty response
                        // under load (rate limiting, transient network). One
                        // retry with a short backoff is enough to recover the
                        // common cases without paying for the regular model.
                        let mut llm_outcome: Option<String> = None;
                        for attempt in 0..2 {
                            match crate::session_context::with_session_id(
                                Some(sid.0.to_string()),
                                provider.complete(
                                    &fast_model_config,
                                    system,
                                    std::slice::from_ref(&message),
                                    &[],
                                ),
                            )
                            .await
                            {
                                Ok((response, _)) => {
                                    let summary: String = response
                                        .content
                                        .iter()
                                        .filter_map(|c: &MessageContent| c.as_text())
                                        .collect::<String>()
                                        .trim()
                                        .to_string();
                                    if !summary.is_empty() {
                                        llm_outcome = Some(summary);
                                        break;
                                    }
                                    if attempt == 0 {
                                        warn!(
                                            "tool call summary: fast_complete returned empty for {request_id} ({name}), retrying once",
                                        );
                                        tokio::time::sleep(std::time::Duration::from_millis(150))
                                            .await;
                                    }
                                }
                                Err(e) => {
                                    if attempt == 0 {
                                        warn!(
                                            "tool call summary: fast_complete errored for {request_id} ({name}): {e}, retrying once",
                                        );
                                        tokio::time::sleep(std::time::Duration::from_millis(150))
                                            .await;
                                    } else {
                                        warn!(
                                            "tool call summary: fast_complete errored for {request_id} ({name}) after retry: {e}",
                                        );
                                    }
                                }
                            }
                        }
                        match llm_outcome {
                            Some(summary) => (summary, true),
                            None => {
                                warn!(
                                    "tool call summary: falling back to deterministic title for {request_id} ({name}) — replay will not show an LLM summary for this call",
                                );
                                (fallback_title.clone(), false)
                            }
                        }
                    }
                    Err(e) => {
                        warn!("tool call summary: failed to get provider: {e}");
                        (fallback_title.clone(), false)
                    }
                };

                let title = presentation::project_tool_title(&title);
                let fields = ToolCallUpdateFields::new().title(title.clone());
                let _ = cx.send_notification(SessionNotification::new(
                    sid,
                    SessionUpdate::ToolCallUpdate(
                        ToolCallUpdate::new(
                            ToolCallId::new(presentation::project_identifier(&request_id)),
                            fields,
                        )
                        .meta(identity_meta),
                    ),
                ));

                // Best-effort persistence: only persist the LLM-generated title
                // (not the deterministic fallback) so reload uses fallback_title
                // for older or failed cases just like today.
                if from_llm {
                    if let Some(msg_id) = message_id_for_persist {
                        let patch = serde_json::json!({
                            crate::conversation::message::TOOL_META_TITLE_KEY: title,
                        });
                        if let Err(e) = session_manager
                            .update_tool_request_meta(
                                &session_id_for_persist,
                                &msg_id,
                                &request_id,
                                patch,
                            )
                            .await
                        {
                            warn!(
                                "tool call summary: persist failed for {request_id} in {msg_id}: {e}",
                            );
                        }
                    } else {
                        warn!(
                            "tool call summary: missing message_id for {request_id} — title will not survive reload",
                        );
                    }
                }
            });
        }

        Ok(())
    }

    async fn handle_tool_response(
        &self,
        tool_response: &crate::conversation::message::ToolResponse,
        session_id: &SessionId,
        session_id_str: &str,
        message_id: Option<&str>,
        session: &mut GoslingAcpSession,
        cx: &ConnectionTo<Client>,
    ) -> Result<(), agent_client_protocol::Error> {
        let status = match &tool_response.tool_result {
            Ok(result) if result.is_error == Some(true) => ToolCallStatus::Failed,
            Ok(_) => ToolCallStatus::Completed,
            Err(_) => ToolCallStatus::Failed,
        };

        let mut presented_response = tool_response.clone();
        presented_response.tool_result =
            presentation::project_tool_result_for_update(&tool_response.tool_result);

        let mut fields = ToolCallUpdateFields::new().status(status);
        if let Some(raw_output) = extract_tool_raw_output(&presented_response.tool_result) {
            fields = fields.raw_output(raw_output);
        }
        if !presented_response
            .tool_result
            .as_ref()
            .is_ok_and(|r| r.is_acp_aware())
        {
            let content = build_tool_call_content(&presented_response.tool_result);
            fields = fields.content(content);

            let locations = extract_locations_from_meta(tool_response).unwrap_or_else(|| {
                if let Some(tool_request) = session.tool_requests.get(&tool_response.id) {
                    extract_tool_locations(tool_request, tool_response)
                } else {
                    Vec::new()
                }
            });
            if !locations.is_empty() {
                fields = fields.locations(locations);
            }
        }

        let update = ToolCallUpdate::new(
            ToolCallId::new(presentation::project_identifier(&tool_response.id)),
            fields,
        )
        .meta(extract_tool_call_update_meta(&presented_response));
        cx.send_notification(SessionNotification::new(
            session_id.clone(),
            SessionUpdate::ToolCallUpdate(update),
        ))?;

        // Chain summarization: when this response completes a multi-tool
        // chain, fire one LLM summary covering the run.
        session.responded_tool_ids.insert(tool_response.id.clone());
        self.maybe_summarize_chain(&tool_response.id, session_id, session_id_str, session, cx);
        let _ = message_id;

        Ok(())
    }

    /// If `tool_call_id` belongs to a multi-tool chain and every step in that
    /// chain has now had its response processed, spawn a single LLM
    /// summarization task that persists the chain summary on the first tool
    /// request and notifies the client. Idempotent — fires at most once per
    /// chain.
    fn maybe_summarize_chain(
        &self,
        tool_call_id: &str,
        session_id: &SessionId,
        _session_id_str: &str,
        session: &mut GoslingAcpSession,
        cx: &ConnectionTo<Client>,
    ) {
        let Some(chain) = session.chain_membership.get(tool_call_id).cloned() else {
            warn!(
                "tool chain summary: skipped — no chain registered for tool_call_id {tool_call_id}",
            );
            return;
        };
        if !chain
            .ids
            .iter()
            .all(|id| session.responded_tool_ids.contains(id))
        {
            let total = chain.ids.len();
            let responded = chain
                .ids
                .iter()
                .filter(|id| session.responded_tool_ids.contains(*id))
                .count();
            let missing: Vec<&String> = chain
                .ids
                .iter()
                .filter(|id| !session.responded_tool_ids.contains(*id))
                .collect();
            warn!(
                "tool chain summary: waiting on {pending}/{total} responses for chain anchored at {anchor:?} (missing: {missing:?})",
                pending = total - responded,
                anchor = chain.ids.first(),
            );
            return;
        }
        let Some(first_id) = chain.ids.first() else {
            warn!("tool chain summary: skipped — empty chain.ids for tool_call_id {tool_call_id}");
            return;
        };
        if !session.summarized_chains.insert(first_id.clone()) {
            debug!("tool chain summary: chain anchored at {first_id} already summarized; skipping");
            return;
        }

        let agent = session.agent.clone();

        // Snapshot (name, args_json) for each step in document order.
        let steps: Vec<(String, String)> = chain
            .ids
            .iter()
            .filter_map(|id| {
                let req = session.tool_requests.get(id)?;
                let tool_call = req.tool_call.as_ref().ok()?;
                let name = tool_call.name.to_string();
                let args = tool_call
                    .arguments
                    .as_ref()
                    .map(|a| serde_json::to_string(a).unwrap_or_default())
                    .unwrap_or_default();
                let args = if args.len() > 200 {
                    format!("{}…", crate::utils::safe_truncate(&args, 200))
                } else {
                    args
                };
                Some((name, args))
            })
            .collect();
        if steps.len() < 2 {
            return;
        }

        let identity_meta = session
            .tool_requests
            .get(first_id)
            .and_then(tool_call_identity_meta);

        let sid = session_id.clone();
        let chain_for_task = chain.clone();
        let cx = cx.clone();
        let session_manager = self.session_manager.clone();

        let first_id = first_id.clone();
        tokio::spawn(async move {
            let provider = match agent.provider().await {
                Ok(p) => p,
                Err(e) => {
                    warn!(
                        "tool chain summary: failed to get provider for chain anchored at {first_id}: {e}",
                    );
                    return;
                }
            };
            if provider.manages_own_context() {
                warn!(
                    "tool chain summary: provider manages own context; skipping chain anchored at {first_id}",
                );
                return;
            }

            let system = "Summarize this sequence of tool calls in a short lowercase phrase \
                 (3-8 words). No punctuation. No quotes. \
                 Examples: applied dark mode polish, scanned for security issues, \
                 refactored config loading";

            let mut user_text = String::from("Tool call sequence:\n");
            for (i, (name, args)) in steps.iter().enumerate() {
                user_text.push_str(&format!("Step {}: {} {}\n", i + 1, name, args));
            }
            let message = Message::user().with_text(&user_text);
            let model_config = match agent.model_config_for_session(&sid.0).await {
                Ok(config) => config,
                Err(_) => return,
            };
            let fast_model_config =
                match crate::model_config::get_fast_model(provider.get_name(), &model_config).await
                {
                    Ok(config) => config,
                    Err(_) => return,
                };

            // Match the per-tool retry policy: one retry on empty/error keeps
            // the chain header reliable when the fast model is rate-limited or
            // momentarily flaky, without escalating to the regular model.
            let mut summary: Option<String> = None;
            for attempt in 0..2 {
                match crate::session_context::with_session_id(
                    Some(sid.0.to_string()),
                    provider.complete(
                        &fast_model_config,
                        system,
                        std::slice::from_ref(&message),
                        &[],
                    ),
                )
                .await
                {
                    Ok((response, _)) => {
                        let s = response
                            .content
                            .iter()
                            .filter_map(|c: &MessageContent| c.as_text())
                            .collect::<String>()
                            .trim()
                            .to_string();
                        if !s.is_empty() {
                            summary = Some(s);
                            break;
                        }
                        if attempt == 0 {
                            warn!(
                                "tool chain summary: fast_complete returned empty for chain anchored at {first_id} ({} steps), retrying once",
                                steps.len(),
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                        }
                    }
                    Err(e) => {
                        if attempt == 0 {
                            warn!(
                                "tool chain summary: fast_complete errored for chain anchored at {first_id}: {e}, retrying once",
                            );
                            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
                        } else {
                            warn!(
                                "tool chain summary: fast_complete errored for chain anchored at {first_id} after retry: {e}",
                            );
                        }
                    }
                }
            }
            let Some(summary) = summary else {
                warn!(
                    "tool chain summary: no LLM summary produced for chain anchored at {first_id} — replay will fall back to the deterministic phrase",
                );
                return;
            };
            let summary = presentation::project_tool_chain_summary(&summary);

            let count = chain_for_task.ids.len();
            let patch = serde_json::json!({
                crate::conversation::message::TOOL_META_CHAIN_SUMMARY_KEY: {
                    "summary": &summary,
                    "count": count,
                },
            });
            if let Err(e) = session_manager
                .update_tool_request_meta(&sid.0, &chain_for_task.message_id, &first_id, patch)
                .await
            {
                warn!(
                    "tool chain summary: persist failed for chain anchored at {first_id} in {}: {e}",
                    chain_for_task.message_id,
                );
            }

            let meta = with_tool_chain_summary_meta(identity_meta, &summary, count);
            let fields = ToolCallUpdateFields::new();
            let _ = cx.send_notification(SessionNotification::new(
                sid,
                SessionUpdate::ToolCallUpdate(
                    ToolCallUpdate::new(
                        ToolCallId::new(presentation::project_identifier(&first_id)),
                        fields,
                    )
                    .meta(meta),
                ),
            ));
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn handle_tool_permission_request(
        &self,
        cx: &ConnectionTo<Client>,
        agent: &Arc<Agent>,
        session_id: &SessionId,
        request_id: String,
        tool_name: String,
        arguments: serde_json::Map<String, serde_json::Value>,
        prompt: Option<String>,
        domain: Option<String>,
    ) -> Result<(), agent_client_protocol::Error> {
        let cx = cx.clone();
        let agent = agent.clone();
        let session_id = session_id.clone();

        let formatted_name = presentation::project_tool_title(&format_tool_name(&tool_name));

        // A security prompt means an inspector flagged this specific call. A
        // persistent "always allow" would carry that one-off decision forward
        // to every future call of the tool, so the option is withheld -- the
        // CLI already does this via `security_prompt.is_none()`, but ACP
        // clients (Desktop, TUI) were offered it. (WFG-GOS-006)
        let is_security_prompt = prompt.is_some();

        let mut fields = ToolCallUpdateFields::new()
            .title(formatted_name)
            .kind(ToolKind::default())
            .status(ToolCallStatus::Pending)
            .raw_input(presentation::project_tool_input(
                &serde_json::Value::Object(arguments),
            ));
        if let Some(p) = prompt {
            fields = fields.content(vec![ToolCallContent::Content(Content::new(
                ContentBlock::Text(TextContent::new(presentation::project_live_text(
                    &p,
                    "Permission prompt",
                ))),
            ))]);
        }
        let tool_call_update = ToolCallUpdate::new(
            ToolCallId::new(presentation::project_identifier(&request_id)),
            fields,
        );

        fn option(kind: PermissionOptionKind) -> PermissionOption {
            let id = serde_json::to_value(kind)
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
            PermissionOption::new(id.clone(), id, kind)
        }
        let mut options = Vec::new();
        if !is_security_prompt {
            options.push(option(PermissionOptionKind::AllowAlways));
        } else if let Some(domain) = &domain {
            // A domain-scoped grant is a narrower, more auditable claim than
            // the tool-wide always-allow withheld above by WFG-GOS-006: it
            // covers only the flagged destination, not every future call of
            // the tool. The ACP `PermissionOptionKind` enum has no
            // domain-scoped variant, so this reuses `AllowAlways` for the
            // kind hint but carries a distinct `option_id` so the response
            // can be told apart from a tool-wide grant.
            options.push(PermissionOption::new(
                PermissionDecision::AllowAlwaysDomain.to_string(),
                format!("Always allow {domain}"),
                PermissionOptionKind::AllowAlways,
            ));
        }
        options.extend([
            option(PermissionOptionKind::AllowOnce),
            option(PermissionOptionKind::RejectOnce),
            option(PermissionOptionKind::RejectAlways),
        ]);

        let permission_request =
            RequestPermissionRequest::new(session_id, tool_call_update, options);

        cx.send_request(permission_request)
            .on_receiving_result(move |result| async move {
                match result {
                    Ok(response) => {
                        agent
                            .handle_confirmation(
                                request_id,
                                outcome_to_confirmation(&response.outcome),
                            )
                            .await;
                        Ok(())
                    }
                    Err(e) => {
                        error!(error = ?e, "permission request failed");
                        agent
                            .handle_confirmation(
                                request_id,
                                PermissionConfirmation {
                                    principal_type: PrincipalType::Tool,
                                    permission: Permission::Cancel,
                                },
                            )
                            .await;
                        Ok(())
                    }
                }
            })?;

        Ok(())
    }
}

impl GoslingAcpAgent {
    async fn on_new_session(
        &self,
        cx: &ConnectionTo<Client>,
        args: NewSessionRequest,
    ) -> Result<NewSessionResponse, agent_client_protocol::Error> {
        self.handle_new_session(cx, args).await
    }

    /// Look up the session's agent.
    async fn get_session_agent(
        &self,
        session_id: &str,
    ) -> Result<Arc<Agent>, agent_client_protocol::Error> {
        if self.closed_session_ids.lock().await.contains(session_id) {
            return Err(agent_client_protocol::Error::resource_not_found(Some(
                session_id.to_string(),
            ))
            .data(format!("Session not found: {}", session_id)));
        }

        {
            let sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get(session_id) {
                return Ok(session.agent.clone());
            }
        }

        let cx = self.client_cx.get().ok_or_else(|| {
            agent_client_protocol::Error::resource_not_found(Some(session_id.to_string()))
                .data(format!("Session not found: {}", session_id))
        })?;
        let session = self
            .session_manager
            .get_session(session_id, false)
            .await
            .map_err(|_| {
                agent_client_protocol::Error::resource_not_found(Some(session_id.to_string()))
                    .data(format!("Session not found: {}", session_id))
            })?;
        let (agent, _) = self
            .activate_acp_session(cx, &session, HashMap::new())
            .await?;
        Ok(agent)
    }

    async fn on_load_session(
        &self,
        cx: &ConnectionTo<Client>,
        args: LoadSessionRequest,
    ) -> Result<LoadSessionResponse, agent_client_protocol::Error> {
        self.handle_load_session(cx, args).await
    }

    async fn on_fork_session(
        &self,
        cx: &ConnectionTo<Client>,
        args: ForkSessionRequest,
    ) -> Result<ForkSessionResponse, agent_client_protocol::Error> {
        self.handle_fork_session(cx, args).await
    }
}

pub struct GoslingAcpHandler {
    pub agent: Arc<GoslingAcpAgent>,
}

fn negotiate_protocol_version(
    requested: ProtocolVersion,
) -> Result<ProtocolVersion, agent_client_protocol::Error> {
    if requested != ProtocolVersion::LATEST {
        return Err(agent_client_protocol::Error::invalid_params().data(format!(
            "Unsupported ACP protocol version {requested}; expected {}",
            ProtocolVersion::LATEST
        )));
    }
    Ok(ProtocolVersion::LATEST)
}

struct EofAwareReader<R> {
    inner: R,
    eof_sender: Option<oneshot::Sender<()>>,
}

impl<R> EofAwareReader<R> {
    fn new(inner: R, eof_sender: oneshot::Sender<()>) -> Self {
        Self {
            inner,
            eof_sender: Some(eof_sender),
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for EofAwareReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buffer: &mut [u8],
    ) -> Poll<std::io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_read(cx, buffer);
        if matches!(result, Poll::Ready(Ok(0))) {
            if let Some(sender) = self.eof_sender.take() {
                let _ = sender.send(());
            }
        }
        result
    }
}

async fn finish_connection_on_eof<F>(
    connection: F,
    eof_receiver: oneshot::Receiver<()>,
) -> Result<()>
where
    F: std::future::Future<Output = Result<(), agent_client_protocol::Error>>,
{
    match select(Box::pin(connection), Box::pin(eof_receiver)).await {
        Either::Left((result, _)) => result?,
        Either::Right((Ok(()), connection)) => {
            if let Ok(result) =
                tokio::time::timeout(std::time::Duration::from_secs(1), connection).await
            {
                result?;
            }
        }
        Either::Right((Err(_), connection)) => connection.await?,
    }
    Ok(())
}

pub fn serve<R, W>(
    agent: Arc<GoslingAcpAgent>,
    read: R,
    write: W,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<()>> + Send>>
where
    R: futures::AsyncRead + Unpin + Send + 'static,
    W: futures::AsyncWrite + Unpin + Send + 'static,
{
    let runtime_paths = agent.runtime_paths.clone();
    Box::pin(Paths::scope(runtime_paths, async move {
        let handler = GoslingAcpHandler { agent };
        let (eof_sender, eof_receiver) = oneshot::channel();
        let read = EofAwareReader::new(read, eof_sender);

        let connection = SacpAgent
            .builder()
            .name("gosling-acp")
            .with_handler(handler)
            .connect_to(ByteStreams::new(write, read));

        finish_connection_on_eof(connection, eof_receiver).await
    }))
}

/// A lazily-initialized agent connection used by the HTTP/WebSocket transport.
///
/// The `agent-client-protocol-http` server takes a synchronous factory that
/// yields a [`ConnectTo<Client>`] per connection, but creating a gosling agent is
/// async. Agent creation is therefore deferred into [`ConnectTo::connect_to`],
/// which runs as the connection's serving future.
pub struct GoslingAgentConnection {
    server: Arc<crate::acp::server_factory::AcpServer>,
}

impl GoslingAgentConnection {
    pub fn new(server: Arc<crate::acp::server_factory::AcpServer>) -> Self {
        Self { server }
    }
}

impl agent_client_protocol::ConnectTo<Client> for GoslingAgentConnection {
    async fn connect_to(
        self,
        client: impl agent_client_protocol::ConnectTo<SacpAgent>,
    ) -> std::result::Result<(), agent_client_protocol::Error> {
        let agent = self.server.create_agent().await.internal_err()?;
        let handler = GoslingAcpHandler { agent };
        SacpAgent
            .builder()
            .name("gosling-acp")
            .with_handler(handler)
            .connect_to(client)
            .await
    }
}

pub async fn run(builtins: Vec<String>) -> Result<()> {
    info!("listening on stdio");

    let outgoing = tokio::io::stdout().compat_write();
    let incoming = tokio::io::stdin().compat();

    let server = crate::acp::server_factory::AcpServer::new(
        crate::acp::server_factory::AcpServerFactoryConfig {
            builtins,
            state_dir: Paths::state_dir(),
            data_dir: Paths::data_dir(),
            platform_data_dir: Paths::data_dir(),
            config_dir: Paths::config_dir(),
            gosling_platform: GoslingPlatform::GoslingCli,
            additional_source_roots: Vec::new(),
            shell_runtime: Default::default(),
        },
    );
    let agent = server.create_agent().await?;
    serve(agent, incoming, outgoing).await
}

#[cfg(test)]
mod tests;
