use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{anyhow, bail, Context, Result};
use futures::stream::BoxStream;
use futures::{stream, FutureExt, Stream, StreamExt, TryStreamExt};
use tracing_futures::Instrument;
use uuid::Uuid;

use super::container::Container;
use super::frontend_tool_result_router::{
    FrontendToolResultRegistration, FrontendToolResultRouter,
};
use super::mcp_client::GoslingMcpHostInfo;
use super::tool_confirmation_router::ToolConfirmationRouter;
use super::tool_execution::{
    ToolCallResult, CHAT_MODE_TOOL_SKIPPED_RESPONSE, DECLINED_RESPONSE,
    SUBAGENT_APPROVAL_UNAVAILABLE_RESPONSE,
};
use crate::action_required_manager::ElicitationOutcome;
use crate::agents::extension::{ExtensionConfig, ExtensionResult, ToolInfo};
use crate::agents::extension_manager::{
    get_parameter_names, ExtensionManager, ExtensionManagerCapabilities,
};
use crate::agents::platform_extensions::MANAGE_EXTENSIONS_TOOL_NAME_COMPLETE;
use crate::agents::prompt_manager::PromptManager;
use crate::agents::types::{FrontendTool, SessionConfig, SharedProvider};
use crate::config::extensions::name_to_key;
use crate::config::permission::PermissionManager;
use crate::config::{CodeExecutionRuntime, Config, GoslingMode};
use crate::context_mgmt::{
    check_if_compaction_needed, compact_messages, context_manager_mode, resolve_provider_input,
    summarizer, ContextBuildRequest, ContextManager, ContextManagerMode, FileMemorySource,
    MemoryQuery, MemorySource, SummarizerMode, DEFAULT_COMPACTION_THRESHOLD,
};
use crate::conversation::message::{
    ActionRequiredData, InferenceMetadata, Message, MessageContent, ProviderMetadata,
    SystemNotificationType, ToolRequest,
};
use crate::conversation::{debug_conversation_fix, fix_conversation, Conversation};
use crate::hints::SubdirectoryHintTracker;
use crate::mcp_utils::ToolResult;
use crate::permission::permission_confirmation::PrincipalType;
use crate::permission::permission_inspector::PermissionInspector;
use crate::permission::permission_judge::PermissionCheckResult;
use crate::permission::working_dir_scope_inspector::WorkingDirScopeInspector;
use crate::permission::{Permission, PermissionConfirmation};
use crate::providers::base::{PermissionRouting, Provider};
use crate::security::adversary_inspector::AdversaryInspector;
use crate::security::egress_inspector::EgressInspector;
use crate::security::security_inspector::SecurityInspector;
use crate::session::extension_data::{EnabledExtensionsState, ExtensionState};
use crate::session::{
    Session, SessionManager, SessionNameUpdate, SessionType, ToolOperationStart,
    DEFAULT_SESSION_TAIL_LIMIT,
};
use crate::tool_inspection::ToolInspectionManager;
use crate::tool_monitor::RepetitionInspector;
use crate::utils::is_token_cancelled;
use crate::workspace::WorkspaceService;
use gosling_providers::errors::ProviderError;
use gosling_providers::retry::{should_retry, RetryConfig};
use gosling_providers::thinking::ThinkingEffort;
use rmcp::model::{
    CallToolRequestParams, CallToolResult, Content, ElicitationAction, ErrorCode, ErrorData,
    GetPromptResult, Prompt, ServerNotification, Tool,
};
use serde_json::Value;
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, instrument, warn};

const DEFAULT_MAX_TURNS: u32 = 1000;
const DEFAULT_STOP_HOOK_BLOCK_CAP: u32 = 8;
// Bounds the "grind" nudge independently of `max_turns`: without its own cap, a
// grind goal that never completes re-injects "keep working" on every no-tool
// turn, run after run, relying solely on the shared 1000-turn ceiling to end it.
const DEFAULT_MAX_GRIND_NUDGES: u32 = 50;
const COMPACTION_THINKING_TEXT: &str = "gosling is compacting the conversation...";
const MAX_TURNS_MESSAGE: &str = "I've reached the maximum number of actions I can do without user input. Would you like me to continue?";
const MAX_GRIND_NUDGES_MESSAGE: &str = "I've kept working on the grind goal without completing it after many attempts. Stopping to avoid an unbounded loop — let me know if you'd like me to continue.";
const DEFAULT_FRONTEND_INSTRUCTIONS: &str = "The following tools are provided directly by the frontend and will be executed by the frontend when called.";
const STREAM_CHECKPOINT_INTERVAL: Duration = Duration::from_millis(250);
// A provider stream that dies partway through is only retryable while no tool
// from it has run: re-issuing the request replays the whole assistant message,
// which is fine for text nobody has acted on and wrong once a tool has taken
// effect in the world. `ProviderRetry` only covers establishing the stream, so
// without this a connection dropped mid-response ended the turn.
const MAX_MID_STREAM_RETRIES: usize = 3;

mod extensions;
mod frontend_extensions;
mod hooks;
mod reply_context;
mod tool_dispatch;

pub(super) struct ToolOperationGuard {
    session_manager: Arc<SessionManager>,
    operation_id: Option<String>,
}

impl ToolOperationGuard {
    pub(super) fn new(session_manager: Arc<SessionManager>, operation_id: String) -> Self {
        Self {
            session_manager,
            operation_id: Some(operation_id),
        }
    }

    pub(super) fn disarm(&mut self) {
        self.operation_id = None;
    }
}

impl Drop for ToolOperationGuard {
    fn drop(&mut self) {
        let Some(operation_id) = self.operation_id.take() else {
            return;
        };
        self.session_manager.release_tool_operation(&operation_id);
        if let Ok(runtime) = tokio::runtime::Handle::try_current() {
            let session_manager = self.session_manager.clone();
            runtime.spawn(async move {
                if let Err(error) = session_manager
                    .mark_tool_operation_in_doubt(&operation_id)
                    .await
                {
                    warn!(
                        "Failed to mark abandoned tool operation {} in doubt: {}",
                        operation_id, error
                    );
                }
            });
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ToolCategory {
    Shell,
    Read,
    Write,
    Other,
}

fn categorize_tool(tool_name: &str) -> ToolCategory {
    let local = tool_name.rsplit("__").next().unwrap_or(tool_name);
    match local {
        "shell" | "bash" | "exec" | "run" => ToolCategory::Shell,
        "read" | "view" | "cat" | "read_file" => ToolCategory::Read,
        "write" | "edit" | "patch" | "write_file" | "edit_file" => ToolCategory::Write,
        _ => ToolCategory::Other,
    }
}

fn take_tool_confirmation_requests(message: &mut Message) -> Vec<String> {
    let mut request_ids = Vec::new();
    message.content.retain(|content| {
        let MessageContent::ActionRequired(action_required) = content else {
            return true;
        };
        let ActionRequiredData::ToolConfirmation { id, .. } = &action_required.data else {
            return true;
        };

        request_ids.push(id.clone());
        false
    });
    request_ids
}

fn extract_string_arg(input: &Value, keys: &[&str]) -> Option<String> {
    let obj = input.as_object()?;
    for k in keys {
        if let Some(s) = obj.get(*k).and_then(|v| v.as_str()) {
            if !s.is_empty() {
                return Some(s.to_string());
            }
        }
    }
    None
}

fn stop_hook_denial_context_message(plugin: &str, reason: &str) -> Message {
    let nudge = format!(
        "Stop hook `{plugin}` blocked ending this turn:

{reason}

Address this policy hook denial before trying to stop again."
    );
    Message::user()
        .with_text(nudge)
        .with_visibility(false, true)
}

fn stop_hook_denial_notification(plugin: &str) -> Message {
    Message::assistant().with_system_notification(
        SystemNotificationType::InlineMessage,
        format!("Stop hook `{plugin}` blocked ending this turn."),
    )
}

fn stop_hook_block_cap_warning(plugin: &str, cap: u32) -> Message {
    Message::assistant().with_system_notification(
        SystemNotificationType::InlineMessage,
        format!(
            "Stop hook `{plugin}` blocked the turn from ending more than {cap} consecutive times — overriding and ending turn to avoid an infinite loop. Set GOSLING_STOP_HOOK_BLOCK_CAP to raise this limit."
        ),
    )
}

/// Builds the message for a provider failure the mid-stream retry could not
/// absorb.
///
/// Whether the turn is actually over decides both halves. Once a tool has run
/// there is a result to carry forward, so the agent goes on by itself: telling
/// the user to resend describes something that isn't happening, and marking the
/// message `terminal_error` fails a non-interactive run that then went on to
/// finish its work.
fn provider_failure_message(
    provider_err: &ProviderError,
    ending_text: &str,
    turn_ends: bool,
) -> Message {
    if turn_ends {
        Message::assistant()
            .with_text(ending_text)
            .with_terminal_error(provider_err.to_string())
    } else {
        Message::assistant().with_text(format!(
            "{provider_err}\n\nContinuing with the tool results already collected."
        ))
    }
}

/// Context needed for the reply function
pub struct ReplyContext {
    pub conversation: Conversation,
    pub tools: Vec<Tool>,
    pub toolshim_tools: Vec<Tool>,
    pub system_prompt: String,
    pub gosling_mode: GoslingMode,
    pub tool_call_cut_off: usize,
    pub model_config: gosling_providers::model::ModelConfig,
}

pub struct ToolCategorizeResult {
    pub frontend_requests: Vec<ToolRequest>,
    pub remaining_requests: Vec<ToolRequest>,
    pub filtered_response: Message,
}

#[derive(Debug, Clone, serde::Serialize, utoipa::ToSchema)]
pub struct ExtensionLoadResult {
    pub name: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug)]
pub enum GoslingPlatform {
    GoslingDesktop,
    GoslingCli,
}

impl fmt::Display for GoslingPlatform {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GoslingPlatform::GoslingCli => write!(f, "gosling-cli"),
            GoslingPlatform::GoslingDesktop => write!(f, "gosling-desktop"),
        }
    }
}

#[derive(Clone)]
pub struct AgentConfig {
    pub session_manager: Arc<SessionManager>,
    pub permission_manager: Arc<PermissionManager>,
    pub gosling_mode: GoslingMode,
    pub code_execution_runtime: CodeExecutionRuntime,
    pub disable_session_naming: bool,
    pub gosling_platform: GoslingPlatform,
    pub mcp_host_info: Option<GoslingMcpHostInfo>,
    pub session_name_update_tx: Option<mpsc::UnboundedSender<SessionNameUpdate>>,
    pub use_login_shell_path: Option<bool>,
    pub workspace_service: Option<Arc<WorkspaceService>>,
}

impl AgentConfig {
    pub fn new(
        session_manager: Arc<SessionManager>,
        permission_manager: Arc<PermissionManager>,
        gosling_mode: GoslingMode,
        disable_session_naming: bool,
        gosling_platform: GoslingPlatform,
    ) -> Self {
        Self {
            session_manager,
            permission_manager,
            gosling_mode,
            code_execution_runtime: CodeExecutionRuntime::Disabled,
            disable_session_naming,
            gosling_platform,
            mcp_host_info: None,
            session_name_update_tx: None,
            use_login_shell_path: None,
            workspace_service: None,
        }
    }

    pub fn with_mcp_host_info(mut self, mcp_host_info: Option<GoslingMcpHostInfo>) -> Self {
        self.mcp_host_info = mcp_host_info;
        self
    }

    pub fn with_code_execution_runtime(mut self, runtime: CodeExecutionRuntime) -> Self {
        self.code_execution_runtime = runtime;
        self
    }

    pub fn with_session_name_update_tx(
        mut self,
        tx: Option<mpsc::UnboundedSender<SessionNameUpdate>>,
    ) -> Self {
        self.session_name_update_tx = tx;
        self
    }

    pub fn with_use_login_shell_path(mut self, use_login_shell_path: bool) -> Self {
        self.use_login_shell_path = Some(use_login_shell_path);
        self
    }

    pub fn with_workspace_service(mut self, service: Arc<WorkspaceService>) -> Self {
        self.workspace_service = Some(service);
        self
    }

    fn resolve_use_login_shell_path(&self) -> bool {
        resolve_use_login_shell_path(self.use_login_shell_path, &self.gosling_platform)
    }
}

fn resolve_use_login_shell_path(explicit: Option<bool>, platform: &GoslingPlatform) -> bool {
    explicit.unwrap_or(matches!(platform, GoslingPlatform::GoslingDesktop))
}

/// The main gosling Agent
pub struct Agent {
    pub(super) provider: SharedProvider,
    pub config: AgentConfig,
    pub(super) current_gosling_mode: Mutex<GoslingMode>,
    pub(super) gosling_mode_changes: tokio::sync::watch::Sender<GoslingMode>,
    state_transition: Mutex<()>,

    pub extension_manager: Arc<ExtensionManager>,
    pub(super) frontend_extensions: Mutex<HashMap<String, ExtensionConfig>>,
    pub(super) frontend_tools: Mutex<HashMap<String, FrontendTool>>,
    pub(super) frontend_instructions: Mutex<Option<String>>,
    pub(super) prompt_manager: Mutex<PromptManager>,
    pub(super) subdirectory_hint_tracker: Mutex<SubdirectoryHintTracker>,
    pub tool_confirmation_router: ToolConfirmationRouter,
    pub(super) frontend_tool_result_router: FrontendToolResultRouter,

    pub(super) tool_inspection_manager: ToolInspectionManager,
    pub(super) hook_manager: crate::hooks::HookManager,
    #[cfg(test)]
    stop_hook_block_cap_override: Option<u32>,
    container: Mutex<Option<Container>>,
    goal: Mutex<Option<String>>,
    grind: Mutex<Option<String>>,
    pending_steers: Mutex<HashMap<String, VecDeque<Message>>>,
}

#[derive(Clone, Debug)]
pub enum AgentEvent {
    Message(Message),
    Usage(crate::providers::base::ProviderUsage),
    McpNotification((String, ServerNotification)),
    HistoryReplaced(Conversation),
}

impl Default for Agent {
    fn default() -> Self {
        Self::new()
    }
}

pub enum ToolStreamItem<T> {
    ActionRequired(Message),
    Message(ServerNotification),
    Result(T),
}

pub type ToolStream =
    Pin<Box<dyn Stream<Item = ToolStreamItem<ToolResult<CallToolResult>>> + Send>>;

// tool_stream combines a stream of ServerNotifications with a future representing the
// final result of the tool call. MCP notifications are not request-scoped, but
// this lets us capture all notifications emitted during the tool call for
// simpler consumption
pub fn tool_stream<S, A, F>(rx: S, action_required_rx: A, done: F) -> ToolStream
where
    S: Stream<Item = ServerNotification> + Send + Unpin + 'static,
    A: Stream<Item = Message> + Send + Unpin + 'static,
    F: Future<Output = ToolResult<CallToolResult>> + Send + 'static,
{
    Box::pin(async_stream::stream! {
        tokio::pin!(done);
        let mut rx = rx;
        let mut action_required_rx = action_required_rx;

        loop {
            tokio::select! {
                Some(msg) = action_required_rx.next() => {
                    yield ToolStreamItem::ActionRequired(msg);
                }
                Some(msg) = rx.next() => {
                    yield ToolStreamItem::Message(msg);
                }
                r = &mut done => {
                    yield ToolStreamItem::Result(r);
                    break;
                }
            }
        }
    })
}

impl Agent {
    pub fn new() -> Self {
        let config = Config::global();
        let agent_config = AgentConfig::new(
            Arc::new(SessionManager::instance()),
            PermissionManager::instance(),
            config.get_gosling_mode().unwrap_or_default(),
            config.get_gosling_disable_session_naming().unwrap_or(false),
            GoslingPlatform::GoslingCli,
        )
        .with_code_execution_runtime(config.resolve_gosling_code_execution_runtime());
        Self::with_config(agent_config)
    }

    pub fn with_config(config: AgentConfig) -> Self {
        let provider = Arc::new(Mutex::new(None));

        let gosling_platform = config.gosling_platform.clone();
        let initial_mode = config.gosling_mode;
        let (gosling_mode_changes, _) = tokio::sync::watch::channel(initial_mode);
        let explicit_mcp_host_info = config.mcp_host_info.clone();
        let mcpui = explicit_mcp_host_info
            .as_ref()
            .filter(|host_info| host_info.explicit_extensions)
            .map(GoslingMcpHostInfo::mcpui_enabled)
            .unwrap_or_else(|| match config.gosling_platform {
                GoslingPlatform::GoslingDesktop => true,
                GoslingPlatform::GoslingCli => false,
            });
        let capabilities = ExtensionManagerCapabilities {
            mcpui,
            host_info: explicit_mcp_host_info.clone(),
        };
        let client_name = explicit_mcp_host_info
            .as_ref()
            .and_then(|host_info| host_info.client_name.clone())
            .unwrap_or_else(|| gosling_platform.to_string());
        let session_manager = Arc::clone(&config.session_manager);
        let inspection_session_manager = Arc::clone(&config.session_manager);
        let permission_manager = Arc::clone(&config.permission_manager);
        let use_login_shell_path = config.resolve_use_login_shell_path();
        let code_execution_runtime = config.code_execution_runtime;
        Self {
            provider: provider.clone(),
            config,
            current_gosling_mode: Mutex::new(initial_mode),
            gosling_mode_changes,
            state_transition: Mutex::new(()),
            extension_manager: Arc::new(ExtensionManager::new(
                provider.clone(),
                session_manager,
                client_name,
                capabilities,
                use_login_shell_path,
                code_execution_runtime,
            )),
            frontend_extensions: Mutex::new(HashMap::new()),
            frontend_tools: Mutex::new(HashMap::new()),
            frontend_instructions: Mutex::new(None),
            prompt_manager: Mutex::new(PromptManager::new()),
            subdirectory_hint_tracker: Mutex::new(SubdirectoryHintTracker::new()),
            tool_confirmation_router: ToolConfirmationRouter::new(),
            frontend_tool_result_router: FrontendToolResultRouter::new(),
            tool_inspection_manager: Self::create_tool_inspection_manager(
                permission_manager,
                provider.clone(),
                inspection_session_manager,
            ),
            hook_manager: crate::hooks::HookManager::load(
                std::env::current_dir().ok().as_deref(),
                use_login_shell_path,
            ),
            #[cfg(test)]
            stop_hook_block_cap_override: None,
            container: Mutex::new(None),
            goal: Mutex::new(None),
            grind: Mutex::new(None),
            pending_steers: Mutex::new(HashMap::new()),
        }
    }

    pub async fn shutdown(&self) {
        self.extension_manager.shutdown().await;
    }

    /// Get a reference count clone to the provider
    pub async fn provider(&self) -> Result<Arc<dyn Provider>, anyhow::Error> {
        match &*self.provider.lock().await {
            Some(provider) => Ok(Arc::clone(provider)),
            None => Err(anyhow!("Provider not set")),
        }
    }

    /// Resolve the active model config for a session.
    ///
    /// The session is the source of truth for the selected model and its
    /// settings. When the session has no stored config (e.g. before the
    /// provider has been persisted), fall back to the configured provider
    /// defaults.
    pub async fn model_config_for_session(
        &self,
        session_id: &str,
    ) -> Result<gosling_providers::model::ModelConfig> {
        if let Ok(session) = self
            .config
            .session_manager
            .get_session(session_id, false)
            .await
        {
            if let Some(model_config) = session.model_config {
                return Ok(model_config);
            }
        }

        let config = Config::global();
        let provider_name = config
            .get_gosling_provider()
            .map_err(|_| anyhow!("Could not resolve model config: missing provider"))?;
        let model_name = config
            .get_gosling_model()
            .map_err(|_| anyhow!("Could not resolve model config: missing model"))?;
        crate::model_config::model_config_from_user_config(&provider_name, &model_name)
            .map_err(|e| anyhow!("Could not resolve model config: {e}"))
    }

    /// Handle a confirmation response for a tool request
    pub async fn handle_confirmation(
        &self,
        request_id: String,
        confirmation: PermissionConfirmation,
    ) {
        let provider = self.provider.lock().await.clone();
        if let Some(provider) = provider.as_ref() {
            if provider.permission_routing() == PermissionRouting::ActionRequired
                && provider
                    .handle_permission_confirmation(&request_id, &confirmation)
                    .await
            {
                return;
            }
        }
        if !self
            .tool_confirmation_router
            .deliver(request_id, confirmation)
            .await
        {
            error!("Failed to deliver confirmation");
        }
    }

    pub async fn supports_action_required_permissions(&self) -> bool {
        if let Some(provider) = self.provider.lock().await.as_ref() {
            return provider.permission_routing() == PermissionRouting::ActionRequired;
        }
        false
    }

    /// Pre-flight for paths that reach the provider. Must run before the user
    /// message is persisted: bailing after `add_message` leaves a stray copy in
    /// the conversation that gets replayed to the provider once a later submit
    /// succeeds.
    async fn ensure_provider_ready(&self, restrict_to_working_dirs: bool) -> Result<()> {
        let provider = self.provider().await?;
        if restrict_to_working_dirs && provider.executes_tools_outside_gosling() {
            anyhow::bail!(
                "Provider '{}' runs tools outside Gosling's inspection pipeline, so it can't be used while this session restricts tools to working directories. Turn off \"Restrict tools to working directories\" for this session to allow it — the toggle is in the working-directories menu (folder icon in the chat's top-right corner).",
                provider.get_name()
            );
        }
        Ok(())
    }

    #[instrument(
        skip(self, user_message, session_config, cancel_token),
        fields(user_message, trace_input, session.id = %session_config.id)
    )]
    pub async fn reply(
        &self,
        user_message: Message,
        session_config: SessionConfig,
        cancel_token: Option<CancellationToken>,
    ) -> Result<BoxStream<'_, Result<AgentEvent>>> {
        if is_token_cancelled(&cancel_token) {
            return Ok(Box::pin(futures::stream::empty()));
        }

        let session_manager = self.config.session_manager.clone();
        session_manager
            .recover_tool_operations(&session_config.id)
            .await?;

        let message_text_for_trace = user_message.as_concat_text();
        tracing::Span::current().record("user_message", message_text_for_trace.as_str());
        tracing::Span::current().record("trace_input", message_text_for_trace.as_str());

        for content in &user_message.content {
            if let MessageContent::ActionRequired(action_required) = content {
                if let ActionRequiredData::ElicitationResponse {
                    id,
                    user_data,
                    action,
                } = &action_required.data
                {
                    // Surface stale/cancelled/timed-out elicitations as a hard
                    // error so callers (e.g. the HTTP handler) can propagate
                    // failure to the client instead of silently reporting
                    // success while the blocked tool call stays unblocked.
                    // The success path returns an empty stream after the MCP
                    // server receives the user's accept/decline/cancel action.
                    let response = match action {
                        ElicitationAction::Accept => ElicitationOutcome::Accept(user_data.clone()),
                        ElicitationAction::Decline => ElicitationOutcome::Decline,
                        ElicitationAction::Cancel => ElicitationOutcome::Cancel,
                    };
                    crate::elicitation::complete_elicitation_with_message(
                        &session_manager,
                        &session_config.id,
                        id,
                        response,
                        &user_message,
                    )
                    .await
                    .map_err(|e| {
                        error!("Failed to submit elicitation response: {}", e);
                        anyhow!("Failed to submit elicitation response: {}", e)
                    })?;
                    return Ok(Box::pin(futures::stream::empty()));
                }
            }
        }

        let turn_lease = session_manager
            .acquire_session_turn_lease(&session_config.id)
            .await?;

        let message_text = user_message.as_concat_text();

        let session = session_manager
            .get_session(&session_config.id, false)
            .await?;
        let is_first_turn = session.message_count == 0;
        if is_first_turn {
            self.emit_hook(crate::hooks::HookEvent::SessionStart, &session_config.id)
                .await;
        }

        if self
            .hook_manager
            .has_hooks(crate::hooks::HookEvent::UserPromptSubmit)
        {
            let ctx = crate::hooks::HookContext::new(
                crate::hooks::HookEvent::UserPromptSubmit,
                &session_config.id,
            )
            .with_message(message_text.clone());
            self.hook_manager
                .emit(crate::hooks::HookEvent::UserPromptSubmit, ctx)
                .await;
        }

        let command_result = self
            .execute_command(&message_text, &session_config.id)
            .await;

        let mut command_preamble: Vec<AgentEvent> = Vec::new();

        match command_result {
            Err(e) => {
                let error_message = Message::assistant()
                    .with_text(e.to_string())
                    .with_visibility(true, false);
                return Ok(Box::pin(stream::once(async move {
                    Ok(AgentEvent::Message(error_message))
                })));
            }
            Ok(Some(response))
                if response.role == rmcp::model::Role::Assistant
                    && crate::agents::execute_commands::command_starts_turn(&message_text) =>
            {
                // Setting a goal/grind should immediately start a turn so the
                // agent begins pursuing it, rather than waiting for the next
                // user prompt. Record the command and its confirmation as
                // user-visible only, then inject an agent-visible kickoff and
                // fall through into the reply loop.
                self.ensure_provider_ready(session.restrict_tools_to_working_dirs)
                    .await?;
                session_manager
                    .add_message(
                        &session_config.id,
                        &user_message.clone().with_visibility(true, false),
                    )
                    .await?;
                session_manager
                    .add_message(
                        &session_config.id,
                        &response.clone().with_visibility(true, false),
                    )
                    .await?;
                let goal_text = crate::agents::execute_commands::parse_slash_command(&message_text)
                    .map(|parsed| parsed.params_str.to_string())
                    .unwrap_or_default();
                let kickoff = Message::user()
                    .with_text(format!(
                        "Start working toward this goal now:\n\n**Goal:** {goal_text}"
                    ))
                    .with_visibility(false, true);
                session_manager
                    .add_message(&session_config.id, &kickoff)
                    .await?;

                command_preamble = vec![
                    AgentEvent::Message(user_message.clone()),
                    AgentEvent::Message(response.clone()),
                ];
            }
            Ok(Some(response)) if response.role == rmcp::model::Role::Assistant => {
                session_manager
                    .add_message(
                        &session_config.id,
                        &user_message.clone().with_visibility(true, false),
                    )
                    .await?;
                session_manager
                    .add_message(
                        &session_config.id,
                        &response.clone().with_visibility(true, false),
                    )
                    .await?;

                // Check if this was a command that modifies conversation history
                let modifies_history = crate::agents::execute_commands::COMPACT_TRIGGERS
                    .contains(&message_text.trim())
                    || message_text.trim() == "/clear";

                return Ok(Box::pin(async_stream::try_stream! {
                    let _turn_lease = turn_lease;
                    yield AgentEvent::Message(user_message);
                    yield AgentEvent::Message(response);

                    // After commands that modify history, notify UI that history was replaced
                    if modifies_history {
                        let updated_session = session_manager.get_session(&session_config.id, true)
                            .await
                            .map_err(|e| anyhow!("Failed to fetch updated session: {}", e))?;
                        let updated_conversation = updated_session
                            .conversation
                            .ok_or_else(|| anyhow!("Session has no conversation after history modification"))?;
                        yield AgentEvent::HistoryReplaced(updated_conversation);
                    }
                }));
            }
            Ok(Some(resolved_message)) => {
                self.ensure_provider_ready(session.restrict_tools_to_working_dirs)
                    .await?;
                session_manager
                    .add_message(
                        &session_config.id,
                        &user_message.clone().with_visibility(true, false),
                    )
                    .await?;
                session_manager
                    .add_message(
                        &session_config.id,
                        &resolved_message.clone().with_visibility(false, true),
                    )
                    .await?;
            }
            Ok(None) => {
                self.ensure_provider_ready(session.restrict_tools_to_working_dirs)
                    .await?;
                session_manager
                    .add_message(&session_config.id, &user_message)
                    .await?;
            }
        }
        let session = if session_config.compacted_context {
            session_manager
                .get_session_for_compacted_resume(
                    &session_config.id,
                    session_config
                        .tail_limit
                        .unwrap_or(DEFAULT_SESSION_TAIL_LIMIT),
                )
                .await?
        } else {
            session_manager
                .get_session(&session_config.id, true)
                .await?
        };
        let provider = self.provider().await?;
        let conversation = session
            .conversation
            .clone()
            .ok_or_else(|| anyhow::anyhow!("Session {} has no conversation", session_config.id))?;

        let needs_auto_compact =
            check_if_compaction_needed(provider.as_ref(), &conversation, None, &session).await?;

        let conversation_to_compact = conversation.clone();

        Ok(Box::pin(async_stream::try_stream! {
            let _turn_lease = turn_lease;
            for event in command_preamble {
                yield event;
            }

            let final_conversation = if !needs_auto_compact {
                conversation
            } else {
                let config = Config::global();
                let threshold = config
                    .get_param::<f64>("GOSLING_AUTO_COMPACT_THRESHOLD")
                    .unwrap_or(DEFAULT_COMPACTION_THRESHOLD);
                let threshold_percentage = (threshold * 100.0) as u32;

                let inline_msg = format!(
                    "Exceeded auto-compact threshold of {}%. Performing auto-compaction...",
                    threshold_percentage
                );

                yield AgentEvent::Message(
                    Message::assistant().with_system_notification(
                        SystemNotificationType::InlineMessage,
                        inline_msg,
                    )
                );

                yield AgentEvent::Message(
                    Message::assistant().with_system_notification(
                        SystemNotificationType::ThinkingMessage,
                        COMPACTION_THINKING_TEXT,
                    )
                );

                let compact_model_config = self.model_config_for_session(&session_config.id).await?;
                match self
                    .perform_compact(&compact_model_config, &session_config, &conversation_to_compact)
                    .await
                {
                    Ok(compacted_conversation) => {
                        yield AgentEvent::HistoryReplaced(compacted_conversation.clone());
                        yield AgentEvent::Message(
                            Message::assistant().with_system_notification(
                                SystemNotificationType::InlineMessage,
                                "Compaction complete",
                            )
                        );
                        compacted_conversation
                    }
                    Err(e) => {
                        yield AgentEvent::Message(
                            Message::assistant()
                                .with_text(crate::context_mgmt::compaction_failure_message(&e))
                        );
                        return;
                    }
                }
            };

            let mut reply_stream = self.reply_internal(final_conversation, session_config, session, cancel_token).await?;
            while let Some(event) = reply_stream.next().await {
                yield event?;
            }
        }))
    }

    async fn perform_compact(
        &self,
        model_config: &gosling_providers::model::ModelConfig,
        session_config: &SessionConfig,
        conversation: &Conversation,
    ) -> Result<Conversation> {
        let (compacted_conversation, usage) = compact_messages(
            self.provider().await?.as_ref(),
            model_config,
            &session_config.id,
            conversation,
            false,
        )
        .await?;
        let session_manager = self.config.session_manager.clone();
        session_manager
            .replace_conversation(&session_config.id, &compacted_conversation)
            .await?;
        self.update_session_metrics(&session_config.id, &usage, true)
            .await?;
        Ok(compacted_conversation)
    }

    /// Runs the Context Manager (`GOSLING_CONTEXT_MANAGER`) ahead of a provider
    /// call and decides what to actually send. `off` skips packet assembly
    /// entirely so behavior and cost are unchanged; `shadow` builds and logs
    /// the packet but still returns the pre-existing prompt/messages; `on`
    /// returns the packet's own prompt/messages. Falls back to the
    /// pre-existing prompt/messages on any build error so this can never make
    /// a turn fail that would otherwise have succeeded.
    #[allow(clippy::too_many_arguments)]
    async fn apply_context_manager(
        &self,
        session_id: &str,
        base_system_prompt: &str,
        project_addendum: Option<&str>,
        merged_system_prompt: &str,
        conversation: &Conversation,
        model_config: &gosling_providers::model::ModelConfig,
        working_dir: &std::path::Path,
    ) -> (String, Vec<Message>) {
        let mode = context_manager_mode();
        let fallback = || {
            (
                merged_system_prompt.to_string(),
                conversation.messages().clone(),
            )
        };

        if mode == ContextManagerMode::Off {
            return fallback();
        }

        // A self-managing backend (Claude Code, Codex/ACP, Gemini CLI) runs
        // its own agent loop and compaction, so a Gosling-curated packet
        // driving its input is wasted or counterproductive. Cap `on` to
        // shadow — still build and log the packet, but hand the backend its
        // own prompt/messages — and route the summarizer's extracted facts to
        // the backend's durable file instead of the (unused) packet.
        let (self_managing, summarizer_target) = match self.provider().await {
            Ok(provider) => (
                provider.manages_own_context(),
                summarizer::target_for_provider(provider.as_ref(), working_dir),
            ),
            Err(_) => (false, summarizer::SummarizerTarget::ContextPacket),
        };
        let effective_mode = if self_managing && mode == ContextManagerMode::On {
            debug!(
                "Context Manager capped to shadow: provider manages its own context; skipping packet takeover"
            );
            ContextManagerMode::Shadow
        } else {
            mode
        };

        let context_limit = match self.provider().await {
            Ok(provider) => provider
                .get_context_limit(model_config)
                .await
                .unwrap_or_else(|_| model_config.context_limit()),
            Err(_) => model_config.context_limit(),
        };
        let reserved_response_tokens = model_config
            .max_tokens
            .filter(|tokens| *tokens > 0)
            .map(|tokens| tokens as usize)
            .unwrap_or(crate::context_mgmt::budget::DEFAULT_RESERVED_RESPONSE_TOKENS);

        // This is the memory retrieval point: FileMemorySource recalls from
        // the local memories.jsonl (GOSLING_MEMORY_FILE to override); with no
        // file present it recalls nothing. Swap the source here to back the
        // RetrievedMemory slot with something richer.
        let memory_query = MemoryQuery {
            session_id,
            messages: conversation.messages(),
            reserved_tokens: crate::context_mgmt::ContextBudgetPolicy::new(
                context_limit,
                reserved_response_tokens,
            )
            .retrieved_memory_reserved_tokens(),
        };
        let retrieved_memory = FileMemorySource::from_config().retrieve(&memory_query);

        let request = ContextBuildRequest {
            system_prompt: base_system_prompt.to_string(),
            project_instructions: project_addendum.map(|s| s.to_string()),
            conversation_messages: conversation.messages().clone(),
            context_limit,
            reserved_response_tokens,
            retrieved_memory,
        };

        match ContextManager::build(request).await {
            Ok(packet) => {
                crate::context_mgmt::telemetry::log_context_packet(effective_mode, &packet);
                self.maybe_dispatch_summarizer(session_id, &packet, summarizer_target);
                resolve_provider_input(
                    effective_mode,
                    &packet,
                    merged_system_prompt,
                    conversation.messages(),
                )
            }
            Err(e) => {
                warn!("Context Manager failed to build context packet, falling back to existing behavior: {e}");
                fallback()
            }
        }
    }

    /// Fires the local-LLM summarizer worker (`GOSLING_SUMMARIZER`) over any
    /// blocks the packet just rendered with the naive truncation stub.
    /// Spawned rather than awaited so it never sits on the critical path to
    /// the provider call. `target` (chosen from the current provider) decides
    /// where the output lands: a raw API provider caches a better digest for
    /// the *next* turn's packet (see `summarize_group` in
    /// `context_mgmt::packet`) and appends facts to `memories.jsonl`; a
    /// self-managing backend takes no digest handoff and routes facts to its
    /// durable file (`CLAUDE.md` / `AGENTS.md`). In `shadow` mode it only
    /// logs; a no-op in `off` mode and whenever nothing needed summarizing.
    fn maybe_dispatch_summarizer(
        &self,
        session_id: &str,
        packet: &crate::context_mgmt::ContextPacket,
        target: summarizer::SummarizerTarget,
    ) {
        let mode = summarizer::summarizer_mode();
        if mode == SummarizerMode::Off || packet.metadata.pending_summaries.is_empty() {
            return;
        }

        let session_id = session_id.to_string();
        let pending = packet.metadata.pending_summaries.clone();
        tokio::spawn(async move {
            summarizer::run_pending(mode, &session_id, pending, target).await;
        });
    }

    async fn reply_internal(
        &self,
        conversation: Conversation,
        session_config: SessionConfig,
        session: Session,
        cancel_token: Option<CancellationToken>,
    ) -> Result<BoxStream<'_, Result<AgentEvent>>> {
        let context = self
            .prepare_reply_context(
                &session.id,
                conversation,
                session.working_dir.as_path(),
                &session.additional_working_dirs,
            )
            .await?;
        let ReplyContext {
            mut conversation,
            mut tools,
            mut toolshim_tools,
            mut system_prompt,
            tool_call_cut_off,
            gosling_mode,
            model_config,
        } = context;

        // Kept separately (rather than only the merged `system_prompt`) so the
        // Context Manager can account for system vs. project-instructions
        // tokens as distinct slots instead of double-counting the addendum.
        let base_system_prompt = system_prompt.clone();
        let project_addendum = self.load_project_instructions(&session).await;
        if let Some(ref addendum) = project_addendum {
            system_prompt = format!("{system_prompt}\n\n{addendum}");
        }

        let provider = self.provider().await?;
        let provider_name = provider.get_name().to_string();
        let requested_model = model_config.model_name.clone();
        let inference = provider
            .fetch_model_info(&requested_model)
            .await
            .ok()
            .and_then(|model_info| model_info.resolved_model)
            .map(|resolved_model| InferenceMetadata {
                provider: provider_name,
                requested_model,
                resolved_model: Some(resolved_model),
            });
        let session_manager = self.config.session_manager.clone();
        let session_id = session_config.id.clone();
        if !self.config.disable_session_naming {
            let provider = provider.clone();
            let manager_for_spawn = session_manager.clone();
            let session_name_update_tx = self.config.session_name_update_tx.clone();
            tokio::spawn(async move {
                match manager_for_spawn
                    .maybe_update_name(&session_id, provider)
                    .await
                {
                    Ok(Some(update)) => {
                        if let Some(tx) = session_name_update_tx {
                            if tx.send(update).is_err() {
                                warn!("Failed to publish generated session name");
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(e) => warn!("Failed to generate session description: {}", e),
                }
            });
        }

        // Count tool calls present before this reply — everything added during
        // the reply loop is part of the current turn and should not be summarized.
        let pre_turn_tool_count = conversation
            .messages()
            .iter()
            .flat_map(|m| m.content.iter())
            .filter(|c| matches!(c, MessageContent::ToolRequest(_)))
            .count();

        let working_dir = session.working_dir.clone();
        let reply_stream_span = tracing::info_span!(
            target: "gosling::agents::agent",
            "reply_stream",
            trace_output = tracing::field::Empty,
            session.id = %session_config.id,
            session.user = %crate::session_context::session_user(),
            session.host = %crate::session_context::session_host(),
            session.agent_type = "gosling",
        );
        let inner = Box::pin(async_stream::try_stream! {
            let mut turns_taken = 0u32;
            let max_turns = session_config.max_turns.unwrap_or_else(|| {
                Config::global()
                    .get_param::<u32>("GOSLING_MAX_TURNS")
                    .unwrap_or(DEFAULT_MAX_TURNS)
            });
            let mut compaction_attempts = 0;
            let mut last_assistant_text = String::new();
            let mut goal_check_pending = false;
            let mut grind_nudges_sent = 0u32;
            let mut tool_pair_summarization_done = false;
            let mut stop_hook_handled_for_exit = false;
            let mut retrying_after_stop_hook_denial = false;
            let mut mid_stream_retries = 0usize;
            let mut retrying_stream = false;
            let mut consecutive_stop_hook_blocks = 0u32;
            let stop_hook_block_cap = self.stop_hook_block_cap();
            let mut can_drain_pending_steers = false;

            loop {
                if is_token_cancelled(&cancel_token) {
                    break;
                }

                if can_drain_pending_steers {
                    for message in self.drain_pending_steers(&session_config.id).await {
                        let message_text = message.as_concat_text();
                        if self
                            .hook_manager
                            .has_hooks(crate::hooks::HookEvent::UserPromptSubmit)
                        {
                            let ctx = crate::hooks::HookContext::new(
                                crate::hooks::HookEvent::UserPromptSubmit,
                                &session_config.id,
                            )
                            .with_message(message_text);
                            self.hook_manager
                                .emit(crate::hooks::HookEvent::UserPromptSubmit, ctx)
                                .await;
                        }
                        session_manager.add_message(&session_config.id, &message).await?;
                        conversation.push(message.clone());
                        yield AgentEvent::Message(message);
                    }
                }

                // Neither a stop-hook retry nor a re-issued stream is a new turn:
                // counting them would spend the user's `max_turns` budget on
                // recovery rather than on work.
                if retrying_after_stop_hook_denial {
                    retrying_after_stop_hook_denial = false;
                } else if retrying_stream {
                    retrying_stream = false;
                } else {
                    turns_taken += 1;
                }
                if turns_taken > max_turns {
                    last_assistant_text = MAX_TURNS_MESSAGE.to_string();
                    yield AgentEvent::Message(Message::assistant().with_text(last_assistant_text.clone()));
                    break;
                }

                // Proactively compact if the conversation has grown past the threshold since
                // the check in reply(). This catches growth during tool loops, including
                // long approval-pending waits.
                // Reload the session to get current token counts — the stale snapshot
                // passed into reply_internal won't reflect updates from update_session_metrics.
                let current_session_for_compact = session_manager.get_session(&session_config.id, false).await?;
                if check_if_compaction_needed(
                    self.provider().await?.as_ref(),
                    &conversation,
                    None,
                    &current_session_for_compact,
                )
                .await?
                {
                    let config = Config::global();
                    let threshold = config
                        .get_param::<f64>("GOSLING_AUTO_COMPACT_THRESHOLD")
                        .unwrap_or(DEFAULT_COMPACTION_THRESHOLD);
                    let threshold_percentage = (threshold * 100.0) as u32;

                    yield AgentEvent::Message(
                        Message::assistant().with_system_notification(
                            SystemNotificationType::InlineMessage,
                            format!(
                                "Exceeded auto-compact threshold of {}%. Performing auto-compaction...",
                                threshold_percentage
                            ),
                        )
                    );
                    yield AgentEvent::Message(
                        Message::assistant().with_system_notification(
                            SystemNotificationType::ThinkingMessage,
                            COMPACTION_THINKING_TEXT,
                        )
                    );

                    match self.perform_compact(&model_config, &session_config, &conversation).await {
                        Ok(compacted_conversation) => {
                            conversation = compacted_conversation;
                            yield AgentEvent::HistoryReplaced(conversation.clone());
                            yield AgentEvent::Message(
                                Message::assistant().with_system_notification(
                                    SystemNotificationType::InlineMessage,
                                    "Compaction complete",
                                )
                            );
                        }
                        Err(e) => {
                            yield AgentEvent::Message(
                                Message::assistant()
                                    .with_text(crate::context_mgmt::compaction_failure_message(&e))
                            );
                            break;
                        }
                    }
                }

                let conversation_with_moim = super::moim::inject_moim(
                    &session_config.id,
                    &conversation,
                    &self.extension_manager,
                    turns_taken,
                    max_turns,
                )
                .await;
                let conversation_for_context = conversation_with_moim.as_ref().unwrap_or(&conversation);

                let (provider_system_prompt, provider_messages) = self
                    .apply_context_manager(
                        &session_config.id,
                        &base_system_prompt,
                        project_addendum.as_deref(),
                        &system_prompt,
                        conversation_for_context,
                        &model_config,
                        &working_dir,
                    )
                    .await;

                let mut stream = Self::stream_response_from_provider(
                    self.provider().await?,
                    model_config.clone(),
                    &session_config.id,
                    &provider_system_prompt,
                    &provider_messages,
                    &tools,
                    &toolshim_tools,
                ).await?;
                last_assistant_text.clear();

                let current_turn_tool_count = conversation.messages().iter()
                    .flat_map(|m| m.content.iter())
                    .filter(|c| matches!(c, MessageContent::ToolRequest(_)))
                    .count()
                    .saturating_sub(pre_turn_tool_count);

                let tool_pair_summarization_task = if tool_pair_summarization_done {
                    None
                } else {
                    crate::context_mgmt::maybe_summarize_tool_pairs(
                        self.provider().await?,
                        model_config.clone(),
                        session_config.id.clone(),
                        &conversation,
                        tool_call_cut_off,
                        current_turn_tool_count,
                    )
                };

                let mut no_tools_called = true;
                let mut messages_to_add = Conversation::default();
                let mut tools_updated = false;
                let mut did_recovery_compact_this_iteration = false;
                let mut exit_chat = false;
                let stream_message_id = format!("msg_{}", Uuid::new_v4());
                let mut last_stream_checkpoint_at: Option<Instant> = None;
                let mut last_stream_checkpoint_id: Option<String> = None;
                // First message this stream persisted, so a mid-stream failure can
                // truncate the session back to where the stream began.
                let mut stream_rollback_anchor: Option<String> = None;

                // Track whether this provider turn has already emitted visible
                // thinking so a later tool-call chunk can suppress replayed
                // reasoning without hiding final-only non-streaming thoughts.
                let mut surfaced_thinking_in_turn = false;

                while let Some(next) = stream.next().await {
                    if is_token_cancelled(&cancel_token) || exit_chat {
                        break;
                    }

                    match next {
                        Ok((response, usage)) => {
                            compaction_attempts = 0;

                            if let Some(ref usage) = usage {
                                self.update_session_metrics(&session_config.id, usage, false).await?;
                                yield AgentEvent::Usage(usage.clone());
                            }

                            if let Some(response) = response {
                                let response = if response.id.is_some() {
                                    response
                                } else {
                                    response.with_id(stream_message_id.clone())
                                };
                                let ToolCategorizeResult {
                                    frontend_requests,
                                    remaining_requests,
                                    filtered_response,
                                } = self
                                    .categorize_tools(
                                        &response,
                                        &tools,
                                        surfaced_thinking_in_turn,
                                    )
                                    .await;

                                let mut filtered_response = if let Some(inference) = inference.as_ref() {
                                    filtered_response.with_inference(inference.clone())
                                } else {
                                    filtered_response
                                };
                                let mut response = if let Some(inference) = inference.as_ref() {
                                    response.with_inference(inference.clone())
                                } else {
                                    response
                                };

                                if gosling_mode == GoslingMode::Auto {
                                    let mut permission_request_ids =
                                        take_tool_confirmation_requests(&mut response);
                                    for request_id in
                                        take_tool_confirmation_requests(&mut filtered_response)
                                    {
                                        if !permission_request_ids.contains(&request_id) {
                                            permission_request_ids.push(request_id);
                                        }
                                    }

                                    for request_id in permission_request_ids {
                                        self.handle_confirmation(
                                            request_id,
                                            PermissionConfirmation {
                                                principal_type: PrincipalType::Tool,
                                                permission: Permission::DenyOnce,
                                            },
                                        )
                                        .await;
                                    }

                                    if filtered_response.content.is_empty() {
                                        continue;
                                    }
                                }

                                surfaced_thinking_in_turn |= filtered_response.content.iter().any(
                                    |content| {
                                        matches!(
                                            content,
                                            MessageContent::Thinking(_)
                                                | MessageContent::RedactedThinking(_)
                                        )
                                    },
                                );

                                let num_tool_requests = frontend_requests.len() + remaining_requests.len();
                                if num_tool_requests == 0 {
                                    let text = filtered_response.as_concat_text();
                                    if !text.is_empty() {
                                        last_assistant_text.push_str(&text);
                                    }
                                    messages_to_add.push(response);

                                    if let Some(message) = messages_to_add.last() {
                                        let is_new_message = message.id.as_deref()
                                            != last_stream_checkpoint_id.as_deref();
                                        let checkpoint_due = last_stream_checkpoint_at
                                            .map(|checkpoint| checkpoint.elapsed() >= STREAM_CHECKPOINT_INTERVAL)
                                            .unwrap_or(true);
                                        if is_new_message || checkpoint_due {
                                            session_manager
                                                .upsert_message(&session_config.id, message)
                                                .await?;
                                            last_stream_checkpoint_at = Some(Instant::now());
                                            last_stream_checkpoint_id = message.id.clone();
                                            if stream_rollback_anchor.is_none() {
                                                stream_rollback_anchor = message.id.clone();
                                            }
                                        }
                                    }

                                    yield AgentEvent::Message(filtered_response.clone());
                                    tokio::task::yield_now().await;
                                    continue;
                                }

                                yield AgentEvent::Message(filtered_response.clone());
                                tokio::task::yield_now().await;

                                let mut request_to_response_map = HashMap::new();
                                let mut request_metadata: HashMap<String, Option<ProviderMetadata>> = HashMap::new();
                                for request in frontend_requests.iter().chain(remaining_requests.iter()) {
                                    request_to_response_map.insert(request.id.clone(), Message::user().with_generated_id());
                                    request_metadata.insert(request.id.clone(), request.metadata.clone());
                                }

                                let direct_thinking: Vec<MessageContent> = response
                                    .content
                                    .iter()
                                    .filter(|content| {
                                        matches!(
                                            content,
                                            MessageContent::Thinking(_)
                                                | MessageContent::RedactedThinking(_)
                                        )
                                    })
                                    .cloned()
                                    .collect();
                                if !direct_thinking.is_empty() {
                                    let thinking_msg = Message::new(
                                        response.role.clone(),
                                        response.created,
                                        direct_thinking.clone(),
                                    )
                                    .with_id(format!("msg_{}", Uuid::new_v4()));
                                    session_manager
                                        .upsert_message(&session_config.id, &thinking_msg)
                                        .await?;
                                    messages_to_add.push(thinking_msg);
                                }
                                let response_thinking = if direct_thinking.is_empty() {
                                    messages_to_add
                                        .messages()
                                        .iter()
                                        .rev()
                                        .find(|message| {
                                            message.role == response.role
                                                && !message.content.is_empty()
                                                && message.content.iter().all(|content| {
                                                    matches!(
                                                        content,
                                                        MessageContent::Thinking(_)
                                                            | MessageContent::RedactedThinking(_)
                                                    )
                                                })
                                        })
                                        .map(|message| message.content.clone())
                                        .unwrap_or_default()
                                } else {
                                    direct_thinking
                                };

                                let mut request_msg = Message::assistant()
                                    .with_id(format!("msg_{}", Uuid::new_v4()));
                                for thinking in &response_thinking {
                                    request_msg = request_msg.with_content(thinking.clone());
                                }
                                for content in response.content.iter().filter(|content| {
                                    matches!(content, MessageContent::Text(_) | MessageContent::Image(_))
                                }) {
                                    request_msg = request_msg.with_content(content.clone());
                                }
                                for request in frontend_requests.iter().chain(remaining_requests.iter()) {
                                    let history_tool_call = match &request.tool_call {
                                        Ok(_) => request.tool_call.clone(),
                                        Err(_) => Ok(CallToolRequestParams::new(
                                            "unparseable_tool_call",
                                        )
                                        .with_arguments(serde_json::Map::new())),
                                    };
                                    request_msg = request_msg.with_tool_request_with_metadata(
                                        request.id.clone(),
                                        history_tool_call,
                                        request.metadata.as_ref(),
                                        request.tool_meta.clone(),
                                    );
                                    if let Some(response_placeholder) =
                                        request_to_response_map.get(&request.id)
                                    {
                                        if request_msg.created > response_placeholder.created {
                                            request_msg.created = response_placeholder.created;
                                        }
                                    }
                                }
                                session_manager
                                    .upsert_message(&session_config.id, &request_msg)
                                    .await?;
                                messages_to_add.push(request_msg);

                                // Chat mode must run no tools at all. This loop
                                // used to sit above the `GoslingMode::Chat`
                                // branch below, which only skipped
                                // `remaining_requests` — so frontend tool
                                // requests still executed in the one mode whose
                                // entire contract is "answer, don't act".
                                // (STT-GOS-001)
                                if gosling_mode == GoslingMode::Chat {
                                    for request in frontend_requests.iter() {
                                        Self::record_chat_mode_tool_skip(
                                            request,
                                            &mut request_to_response_map,
                                        );
                                    }
                                } else {
                                    for request in frontend_requests.iter() {
                                        let response_msg = request_to_response_map.get_mut(&request.id)
                                            .ok_or_else(|| anyhow::anyhow!("missing response entry for request {}", request.id))?;
                                        let mut frontend_tool_stream = self.handle_frontend_tool_request(
                                            request,
                                            response_msg,
                                            &session,
                                        );

                                        while let Some(msg) = frontend_tool_stream.try_next().await? {
                                            yield AgentEvent::Message(msg);
                                        }
                                    }
                                }
                                if gosling_mode == GoslingMode::Chat {
                                    for request in remaining_requests.iter() {
                                        Self::record_chat_mode_tool_skip(
                                            request,
                                            &mut request_to_response_map,
                                        );
                                    }
                                } else {
                                    let inspection_results = self
                                        .tool_inspection_manager
                                        .inspect_tools(
                                            &session_config.id,
                                            &remaining_requests,
                                            conversation.messages(),
                                            gosling_mode,
                                        )
                                        .await?;

                                    let mut permission_check_result = self
                                        .tool_inspection_manager
                                        .process_inspection_results_with_permission_inspector(
                                            &remaining_requests,
                                            &inspection_results,
                                        )
                                        .unwrap_or_else(|| {
                                            let mut result = PermissionCheckResult {
                                                approved: vec![],
                                                needs_approval: vec![],
                                                denied: vec![],
                                            };
                                            result
                                                .needs_approval
                                                .extend(remaining_requests.iter().cloned());
                                            result
                                        });

                                    Self::redirect_unapprovable_subagent_requests(
                                        gosling_mode,
                                        session.session_type,
                                        &mut permission_check_result,
                                        &mut request_to_response_map,
                                    );

                                    // Track extension requests
                                    let mut enable_extension_request_ids = vec![];
                                    for request in &remaining_requests {
                                        if let Ok(tool_call) = &request.tool_call {
                                            if tool_call.name == MANAGE_EXTENSIONS_TOOL_NAME_COMPLETE {
                                                enable_extension_request_ids.push(request.id.clone());
                                            }
                                        }
                                    }

                                    let mut tool_futures = self.handle_approved_and_denied_tools(
                                        &permission_check_result,
                                        &mut request_to_response_map,
                                        cancel_token.clone(),
                                        &session,
                                    ).await?;

                                    {
                                        let mut tool_approval_stream = self.handle_approval_tool_requests(
                                            &permission_check_result.needs_approval,
                                            &mut tool_futures,
                                            &mut request_to_response_map,
                                            cancel_token.clone(),
                                            &session,
                                            &inspection_results,
                                        );

                                        while let Some(msg) = tool_approval_stream.try_next().await? {
                                            yield AgentEvent::Message(msg);
                                        }
                                    }

                                    let with_id = tool_futures
                                        .into_iter()
                                        .map(|(request_id, stream)| {
                                            stream.map(move |item| (request_id.clone(), item))
                                        })
                                        .collect::<Vec<_>>();

                                    let mut combined = stream::select_all(with_id);
                                    let mut all_install_successful = true;
                                    let mut tool_persistence_error = None;

                                    loop {
                                        if is_token_cancelled(&cancel_token) {
                                            break;
                                        }

                                        tokio::select! {
                                            biased;

                                            tool_item = combined.next() => {
                                                match tool_item {
                                                    Some((request_id, item)) => {
                                                        match item {
                                                            ToolStreamItem::ActionRequired(mut msg) => {
                                                                if msg.id.is_none() {
                                                                    msg = msg.with_generated_id();
                                                                }
                                                                if let Err(e) = session_manager.add_message(&session_config.id, &msg).await {
                                                                    warn!("Failed to save elicitation message to session: {}", e);
                                                                }
                                                                yield AgentEvent::Message(msg);
                                                            }
                                                            ToolStreamItem::Result(output) => {
                                                                if let Ok(ref call_result) = output {
                                                                    if let Some(ref meta) = call_result.meta {
                                                                        if let Some(notification_data) = meta.0.get("platform_notification") {
                                                                            if let Some(method) = notification_data.get("method").and_then(|v| v.as_str()) {
                                                                                let params = notification_data.get("params").cloned();
                                                                                let custom_notification = rmcp::model::CustomNotification::new(
                                                                                    method.to_string(),
                                                                                    params,
                                                                                );

                                                                                let server_notification = rmcp::model::ServerNotification::CustomNotification(custom_notification);
                                                                                yield AgentEvent::McpNotification((request_id.clone(), server_notification));
                                                                            }
                                                                        }
                                                                    }
                                                                }

                                                                if enable_extension_request_ids.contains(&request_id)
                                                                    && output.is_err()
                                                                {
                                                                    all_install_successful = false;
                                                                }
                                                                if let Some(response) = request_to_response_map.get_mut(&request_id) {
                                                                    let metadata = request_metadata.get(&request_id).and_then(|m| m.as_ref());
                                                                    response.add_tool_response_with_metadata(request_id.clone(), output, metadata);
                                                                    if let Err(error) = session_manager
                                                                        .persist_tool_operation_response(
                                                                            &session_config.id,
                                                                            &request_id,
                                                                            response,
                                                                        )
                                                                        .await
                                                                    {
                                                                        tool_persistence_error = Some(error);
                                                                        break;
                                                                    }
                                                                }
                                                            }
                                                            ToolStreamItem::Message(msg) => {
                                                                yield AgentEvent::McpNotification((request_id, msg));
                                                            }
                                                        }
                                                    }
                                                    None => break,
                                                }
                                            }

                                            _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
                                        }
                                    }

                                    if let Some(error) = tool_persistence_error {
                                        Err(error)?;
                                    }

                                    if all_install_successful && !enable_extension_request_ids.is_empty() {
                                        if let Err(e) = self.save_extension_state(&session_config).await {
                                            warn!("Failed to save extension state after runtime changes: {}", e);
                                        }
                                        tools_updated = true;
                                    }
                                }

                                for request in frontend_requests.iter().chain(remaining_requests.iter()) {
                                    let final_response = match &request.tool_call {
                                        Ok(_) => {
                                            let Some(response) =
                                                request_to_response_map.remove(&request.id)
                                            else {
                                                continue;
                                            };
                                            let has_tool_response =
                                                response.content.iter().any(|c| {
                                                    matches!(c, MessageContent::ToolResponse(r) if r.id == request.id)
                                                });
                                            if !has_tool_response {
                                                // Cancelled before this tool call's result
                                                // arrived: the placeholder is still empty.
                                                // Leave it out of this turn's persisted
                                                // history rather than pairing an
                                                // already-durable ToolRequest with a
                                                // misleading empty response
                                                // (AOC-ORCH-002); `recover_tool_operations`
                                                // synthesizes the correct in-doubt response
                                                // for it on the next `reply()` call.
                                                continue;
                                            }
                                            response
                                        }
                                        Err(error) => {
                                            error!("Tool call could not be parsed: {error}");
                                            let mut response = request_to_response_map
                                                .remove(&request.id)
                                                .unwrap_or_else(|| Message::user().with_generated_id());
                                            // Only feed the parse error back if this id isn't
                                            // already answered. In Chat mode the skip branch above
                                            // already added a tool response for it; adding another
                                            // here would duplicate the tool_call_id (which strict
                                            // providers reject).
                                            let already_answered = response.content.iter().any(|c| {
                                                matches!(c, MessageContent::ToolResponse(r) if r.id == request.id)
                                            });
                                            if !already_answered {
                                                response.add_tool_response_with_metadata(
                                                    request.id.clone(),
                                                    Err(error.clone()),
                                                    request.metadata.as_ref(),
                                                );
                                            }
                                            response
                                        }
                                    };

                                    yield AgentEvent::Message(final_response.clone());
                                    messages_to_add.push(final_response);
                                }

                                no_tools_called = false;
                                // Agent is actively working — re-check goal when it next finishes
                                goal_check_pending = false;
                            }
                        }
                        #[allow(unused_variables)]
                        Err(ref provider_err @ ProviderError::ContextLengthExceeded(_)) => {
                            #[cfg(feature = "telemetry")]
                            crate::posthog::emit_error(provider_err.telemetry_type(), &provider_err.to_string());
                            compaction_attempts += 1;

                            if compaction_attempts >= 2 {
                                error!("Context limit exceeded after compaction - prompt too large");
                            yield AgentEvent::Message(
                                Message::assistant().with_system_notification(
                                    SystemNotificationType::InlineMessage,
                                    "Unable to continue: Context limit still exceeded after compaction. Try using a shorter message, a model with a larger context window, or start a new session."
                                ).with_terminal_error("Context limit still exceeded after compaction")
                            );
                                break;
                            }

                            yield AgentEvent::Message(
                                Message::assistant().with_system_notification(
                                    SystemNotificationType::InlineMessage,
                                    "Context limit reached. Compacting to continue conversation...",
                                )
                            );
                            yield AgentEvent::Message(
                                Message::assistant().with_system_notification(
                                    SystemNotificationType::ThinkingMessage,
                                    COMPACTION_THINKING_TEXT,
                                )
                            );

                            match self
                                .perform_compact(&model_config, &session_config, &conversation)
                                .await
                            {
                                Ok(compacted_conversation) => {
                                    conversation = compacted_conversation;
                                    did_recovery_compact_this_iteration = true;
                                    yield AgentEvent::HistoryReplaced(conversation.clone());
                                    break;
                                }
                                Err(e) => {
                                    #[cfg(feature = "telemetry")]
                                    crate::posthog::emit_error("compaction_failed", &e.to_string());
                                    error!("Compaction failed: {}", e);
                                    yield AgentEvent::Message(
                                        Message::assistant()
                                            .with_text(crate::context_mgmt::compaction_failure_message(&e))
                                            .with_terminal_error(e.to_string())
                                    );
                                    break;
                                }
                            }
                        }
                        Err(ref provider_err @ ProviderError::CreditsExhausted { details: _, ref top_up_url }) => {
                            #[cfg(feature = "telemetry")]
                            crate::posthog::emit_error(provider_err.telemetry_type(), &provider_err.to_string());
                            error!("Error: {}", provider_err);

                            let user_msg = if top_up_url.is_some() {
                                "Please add credits to your account, then resend your message to continue.".to_string()
                            } else {
                                "Please check your account with your provider to add more credits, then resend your message to continue.".to_string()
                            };

                            let notification_data = serde_json::json!({
                                "top_up_url": top_up_url,
                            });

                            yield AgentEvent::Message(
                                Message::assistant().with_system_notification_with_data(
                                    SystemNotificationType::CreditsExhausted,
                                    user_msg,
                                    notification_data,
                                ).with_terminal_error(provider_err.to_string())
                            );
                            break;
                        }
                        Err(ref provider_err @ ProviderError::Refusal { ref details, ref category }) => {
                            #[cfg(feature = "telemetry")]
                            crate::posthog::emit_error(provider_err.telemetry_type(), &provider_err.to_string());
                            error!("Error: {}", provider_err);

                            let category = category.as_deref().map(|c| format!("\n\nCategory: {c}")).unwrap_or_default();
                            yield AgentEvent::Message(
                                Message::assistant().with_text(format!(
                                    "The provider refused this request.\n\n{details}{category}\n\nPlease start a new session to continue — resending this conversation is likely to be refused again."
                                )).with_terminal_error(provider_err.to_string())
                            );
                            // A refusal is terminal: skip goal/grind nudges,
                            // which would resend the same refused conversation.
                            exit_chat = true;
                            break;
                        }
                        // A stream that dies before any of its tools have run can be
                        // re-issued: the partial assistant message is rolled back out
                        // of the session and the UI, and the outer loop asks the
                        // provider again from the same conversation. Once a tool has
                        // run this arm is skipped — replaying it could repeat a side
                        // effect — and the error falls through to the arms below.
                        Err(ref provider_err) if no_tools_called
                            && mid_stream_retries < MAX_MID_STREAM_RETRIES
                            && should_retry(provider_err, &RetryConfig::default()) => {
                            #[cfg(feature = "telemetry")]
                            crate::posthog::emit_error(provider_err.telemetry_type(), &provider_err.to_string());

                            mid_stream_retries += 1;
                            warn!(
                                "Provider stream failed mid-response, retrying ({}/{}): {}",
                                mid_stream_retries, MAX_MID_STREAM_RETRIES, provider_err
                            );

                            if let Some(anchor) = stream_rollback_anchor.take() {
                                session_manager
                                    .truncate_conversation_from_message(&session_config.id, &anchor)
                                    .await?;
                            }
                            // Dropping this keeps the partial answer out of the
                            // conversation the retry is built from — the tail of
                            // this iteration would otherwise extend it in.
                            messages_to_add = Conversation::default();
                            yield AgentEvent::HistoryReplaced(conversation.clone());

                            yield AgentEvent::Message(
                                Message::assistant().with_system_notification(
                                    SystemNotificationType::InlineMessage,
                                    format!(
                                        "The model's response was interrupted. Retrying ({mid_stream_retries}/{MAX_MID_STREAM_RETRIES})..."
                                    ),
                                )
                            );

                            let backoff = RetryConfig::default().delay_for_attempt(mid_stream_retries);
                            match cancel_token.as_ref() {
                                Some(token) => {
                                    tokio::select! {
                                        _ = tokio::time::sleep(backoff) => {}
                                        _ = token.cancelled() => {}
                                    }
                                }
                                None => tokio::time::sleep(backoff).await,
                            }

                            retrying_stream = true;
                            break;
                        }
                        Err(ref provider_err @ ProviderError::NetworkError(_)) => {
                            #[cfg(feature = "telemetry")]
                            crate::posthog::emit_error(provider_err.telemetry_type(), &provider_err.to_string());
                            error!("Error: {}", provider_err);
                            yield AgentEvent::Message(provider_failure_message(
                                provider_err,
                                &format!("{provider_err}\n\nPlease resend your message to try again."),
                                no_tools_called,
                            ));
                            break;
                        }
                        Err(ref provider_err) => {
                            #[cfg(feature = "telemetry")]
                            crate::posthog::emit_error(provider_err.telemetry_type(), &provider_err.to_string());
                            error!("Error: {}", provider_err);
                            yield AgentEvent::Message(provider_failure_message(
                                provider_err,
                                &format!("Ran into this error: {provider_err}.\n\nPlease retry if you think this is a transient or recoverable error."),
                                no_tools_called,
                            ));
                            break;
                        }
                    }
                }
                can_drain_pending_steers = true;

                // The budget is for consecutive failures. A stream that ran to
                // completion means the connection recovered, so the next blip
                // starts from a full allowance rather than an exhausted one.
                if !retrying_stream {
                    mid_stream_retries = 0;
                }

                if tools_updated {
                    (tools, toolshim_tools, system_prompt, _) = self
                        .prepare_tools_and_prompt_with_additional_dirs(
                            &session_config.id,
                            &session.working_dir,
                            &session.additional_working_dirs,
                        )
                        .await?;
                }

                {
                    let hint_text = self
                        .subdirectory_hint_tracker
                        .lock()
                        .await
                        .collect_new_hints(&working_dir);
                    if let Some(hints) = hint_text {
                        messages_to_add
                            .push(Message::user().with_text(hints).with_visibility(false, true));
                    }
                }

                if no_tools_called && !exit_chat {
                    if did_recovery_compact_this_iteration || retrying_stream {
                        // continue from last user message after recovery compact,
                        // or re-issue the request the failed stream was serving —
                        // in neither case has the assistant actually answered yet
                    } else if self.has_pending_steers(&session_config.id).await {
                    } else {
                        // Clone out of the mutexes before branching: an `if let`
                        // scrutinee that locks keeps its guard alive for the whole
                        // if/else chain, which would deadlock against
                        // set_goal/set_grind in the final arm.
                        let goal_nudge = if goal_check_pending {
                            None
                        } else {
                            self.goal.lock().await.clone()
                        };
                        let grind_nudge = self.grind.lock().await.clone();
                        if let Some(goal) = goal_nudge {
                            goal_check_pending = true;
                            let nudge = format!(
                                "Before finishing, check whether the following goal has been fully met:\n\n\
                                 **Goal:** {goal}\n\n\
                                 If not, continue working toward it."
                            );
                            let message = Message::user().with_text(&nudge)
                                .with_visibility(false, true);
                            messages_to_add.push(message);
                            yield AgentEvent::Message(
                                Message::assistant().with_system_notification(
                                    SystemNotificationType::InlineMessage,
                                    format!("Goal: {goal}"),
                                )
                            );
                        } else if let Some(grind) = grind_nudge {
                            if grind_nudges_sent < DEFAULT_MAX_GRIND_NUDGES {
                                grind_nudges_sent += 1;
                                let nudge = format!(
                                    "Keep working. The grind goal is not yet complete:\n\n\
                                     **Goal:** {grind}\n\n\
                                     Continue until it is fully done."
                                );
                                let message = Message::user().with_text(&nudge)
                                    .with_visibility(false, true);
                                messages_to_add.push(message);
                                yield AgentEvent::Message(
                                    Message::assistant().with_system_notification(
                                        SystemNotificationType::InlineMessage,
                                        format!("Grind: {grind}"),
                                    )
                                );
                            } else {
                                self.set_goal(None).await;
                                self.set_grind(None).await;
                                yield AgentEvent::Message(
                                    Message::assistant().with_text(MAX_GRIND_NUDGES_MESSAGE)
                                );
                                exit_chat = true;
                            }
                        } else {
                            self.set_goal(None).await;
                            self.set_grind(None).await;
                            exit_chat = true;
                        }
                    }
                }

                if is_token_cancelled(&cancel_token) {
                    if let Some(ref task) = tool_pair_summarization_task {
                        task.abort();
                    }
                }

                if let Some(task) = tool_pair_summarization_task {
                    tool_pair_summarization_done = true;
                    if let Ok(summaries) = task.await {
                        for (summary_msg, tool_id) in summaries {
                            let matching_ids: Vec<String> = conversation.messages()
                                .iter()
                                .filter(|msg| {
                                    msg.id.is_some() && msg.content.iter().any(|c| match c {
                                        MessageContent::ToolRequest(req) => req.id == tool_id,
                                        MessageContent::ToolResponse(resp) => resp.id == tool_id,
                                        _ => false,
                                    })
                                })
                                .filter_map(|msg| msg.id.clone())
                                .collect();

                            if matching_ids.len() == 2 {
                                for id in &matching_ids {
                                    SessionManager::update_message_metadata(&session_config.id, id, |metadata| {
                                        metadata.with_agent_invisible()
                                    }).await?;
                                }
                                session_manager.add_message(&session_config.id, &summary_msg).await?;
                            } else {
                                warn!("Expected a tool request/reply pair, but found {} matching messages",
                                    matching_ids.len());
                            }
                        }
                    }
                }

                let messages_to_add = if let Some(ref inference) = inference {
                    Conversation::new_unvalidated(
                        messages_to_add
                            .into_iter()
                            .map(|message| message.with_inference_if_assistant(inference.clone())),
                    )
                } else {
                    messages_to_add
                };

                for msg in &messages_to_add {
                    session_manager.upsert_message(&session_config.id, msg).await?;
                    session_manager
                        .register_completed_assistant_artifacts(&session_config.id, msg)
                        .await?;
                }
                conversation.extend(messages_to_add);

                if exit_chat && self.has_pending_steers(&session_config.id).await {
                    exit_chat = false;
                }

                if exit_chat {
                    match self
                        .emit_stop_hook_blocking(&session_config.id, &last_assistant_text)
                        .await
                    {
                        crate::hooks::HookDecision::Allow => {
                            stop_hook_handled_for_exit = true;
                            break;
                        }
                        crate::hooks::HookDecision::Deny { reason, plugin } => {
                            consecutive_stop_hook_blocks += 1;
                            if consecutive_stop_hook_blocks > stop_hook_block_cap {
                                let message = stop_hook_block_cap_warning(&plugin, stop_hook_block_cap);
                                session_manager.add_message(&session_config.id, &message).await?;
                                yield AgentEvent::Message(message);
                                stop_hook_handled_for_exit = true;
                                break;
                            }
                            let message = stop_hook_denial_context_message(&plugin, &reason);
                            session_manager.add_message(&session_config.id, &message).await?;
                            conversation.push(message);
                            yield AgentEvent::Message(stop_hook_denial_notification(&plugin));
                            retrying_after_stop_hook_denial = true;
                        }
                    }
                }

                tokio::task::yield_now().await;
            }

            if !last_assistant_text.is_empty() {
                tracing::Span::current().record("trace_output", last_assistant_text.as_str());
            }

            if !stop_hook_handled_for_exit {
                self.emit_stop_hook(&session_config.id, &last_assistant_text).await;
            }

            summarizer::spawn_session_rollup(
                summarizer::summarizer_mode(),
                session_manager.clone(),
                session_config.id.clone(),
                session_config.tail_limit.unwrap_or(DEFAULT_SESSION_TAIL_LIMIT),
            );
        }.instrument(reply_stream_span));
        Ok(inner)
    }

    pub async fn extend_system_prompt(&self, key: String, instruction: String) {
        let mut prompt_manager = self.prompt_manager.lock().await;
        prompt_manager.add_system_prompt_extra(key, instruction);
    }

    pub async fn remove_system_prompt_extra(&self, key: &str) {
        let mut prompt_manager = self.prompt_manager.lock().await;
        prompt_manager.remove_system_prompt_extra(key);
    }

    pub async fn set_goal(&self, goal: Option<String>) {
        *self.goal.lock().await = goal;
    }

    pub async fn get_goal(&self) -> Option<String> {
        self.goal.lock().await.clone()
    }

    pub async fn set_grind(&self, goal: Option<String>) {
        *self.grind.lock().await = goal;
    }

    pub async fn get_grind(&self) -> Option<String> {
        self.grind.lock().await.clone()
    }

    pub async fn update_provider(
        &self,
        provider: Arc<dyn Provider>,
        model_config: gosling_providers::model::ModelConfig,
        session_id: &str,
    ) -> Result<()> {
        let _transition = self.state_transition.lock().await;
        let mode = self.gosling_mode().await;
        self.apply_provider_transition(provider, model_config, session_id, mode)
            .await
    }

    async fn update_provider_with_mode(
        &self,
        provider: Arc<dyn Provider>,
        model_config: gosling_providers::model::ModelConfig,
        session_id: &str,
        mode: GoslingMode,
    ) -> Result<()> {
        let _transition = self.state_transition.lock().await;
        self.apply_provider_transition(provider, model_config, session_id, mode)
            .await
    }

    async fn apply_provider_transition(
        &self,
        provider: Arc<dyn Provider>,
        model_config: gosling_providers::model::ModelConfig,
        session_id: &str,
        mode: GoslingMode,
    ) -> Result<()> {
        let provider_name = provider.get_name().to_string();

        // Normalize against the provider entry so custom/declarative providers
        // backfill `context_limit` from their known models before the config is
        // persisted as the session source of truth; otherwise auto-compaction
        // would fall back to DEFAULT_CONTEXT_LIMIT.
        let model_config = match crate::providers::get_from_registry(&provider_name).await {
            Ok(entry) => entry
                .normalize_model_config(model_config.clone())
                .unwrap_or(model_config),
            Err(_) => model_config,
        };

        provider
            .update_mode(session_id, mode)
            .await
            .map_err(|e| anyhow::anyhow!("Provider rejected mode update: {e}"))?;

        let mut current_provider = self.provider.lock().await;
        self.config
            .session_manager
            .clone()
            .update(session_id)
            .provider_name(&provider_name)
            .model_config(model_config)
            .apply()
            .await
            .context("Failed to persist provider config to session")?;

        *current_provider = Some(provider);
        *self.current_gosling_mode.lock().await = mode;
        Ok(())
    }

    pub async fn update_gosling_mode(&self, mode: GoslingMode, session_id: &str) -> Result<()> {
        // Clone the Arc out and drop the guard before awaiting: holding the
        // lock across update_mode's round-trip to the provider (which can
        // be an external subprocess for ACP-backed providers, with no
        // timeout) would stall every other task that needs self.provider,
        // including the main reply loop, for as long as that hangs.
        let _transition = self.state_transition.lock().await;
        let mut current_mode = self.current_gosling_mode.lock().await;
        let previous_mode = *current_mode;
        self.config
            .session_manager
            .clone()
            .update(session_id)
            .gosling_mode(mode)
            .apply()
            .await
            .context("Failed to persist gosling_mode to session")?;

        let provider = self.provider.lock().await.clone();
        if let Some(provider) = provider {
            if let Err(error) = provider.update_mode(session_id, mode).await {
                let provider_rollback = provider.update_mode(session_id, previous_mode).await;
                let rollback = self
                    .config
                    .session_manager
                    .clone()
                    .update(session_id)
                    .gosling_mode(previous_mode)
                    .apply()
                    .await;
                let mut rollback_errors = Vec::new();
                if let Err(provider_rollback) = provider_rollback {
                    rollback_errors.push(format!("provider: {provider_rollback}"));
                }
                if let Err(rollback_error) = rollback {
                    rollback_errors.push(format!("session: {rollback_error}"));
                }
                let rollback_detail = if rollback_errors.is_empty() {
                    String::new()
                } else {
                    format!("; rollback errors: {}", rollback_errors.join("; "))
                };
                return Err(anyhow::anyhow!(
                    "Provider rejected mode update: {error}{rollback_detail}"
                ));
            }
        }

        *current_mode = mode;
        let _ = self.gosling_mode_changes.send(mode);
        Ok(())
    }

    pub async fn gosling_mode(&self) -> GoslingMode {
        *self.current_gosling_mode.lock().await
    }

    pub async fn recreate_provider_for_session(
        &self,
        session_id: &str,
        provider_name: &str,
        model_config: gosling_providers::model::ModelConfig,
    ) -> Result<()> {
        let session = self
            .config
            .session_manager
            .get_session(session_id, false)
            .await
            .context("Failed to get session")?;

        let extensions = EnabledExtensionsState::extensions_or_default(
            Some(&session.extension_data),
            Config::global(),
        );

        let provider = self
            .create_provider_with_session_scope(&session, provider_name, extensions)
            .await
            .map_err(|e| anyhow!("Could not create provider: {}", e))?;

        self.update_provider(provider, model_config, session_id)
            .await?;

        let mode = self.gosling_mode().await;
        self.update_gosling_mode(mode, session_id).await
    }

    pub async fn update_thinking_effort(
        &self,
        session_id: &str,
        effort: ThinkingEffort,
    ) -> Result<()> {
        let current_provider = self.provider().await?;
        let provider_name = current_provider.get_name().to_string();
        let model_config = self
            .model_config_for_session(session_id)
            .await?
            .with_thinking_effort(effort);

        self.recreate_provider_for_session(session_id, &provider_name, model_config)
            .await
    }

    /// Restore the provider from session data or fall back to global config
    /// This is used when resuming a session to restore the provider state
    /// Returns true if the session's provider was replaced with a fallback.
    pub async fn restore_provider_from_session(&self, session: &Session) -> Result<bool> {
        let config = Config::global();

        let provider_name = session
            .provider_name
            .clone()
            .or_else(|| config.get_gosling_provider().ok())
            .ok_or_else(|| anyhow!("Could not configure agent: missing provider"))?;

        let model_config = match session.model_config.clone() {
            Some(saved_config) => saved_config,
            None => {
                let model_name = config
                    .get_gosling_model()
                    .ok()
                    .ok_or_else(|| anyhow!("Could not configure agent: missing model"))?;
                crate::model_config::model_config_from_user_config(&provider_name, &model_name)
                    .map_err(|e| anyhow!("Could not configure agent: invalid model {}", e))?
            }
        };

        let extensions =
            EnabledExtensionsState::extensions_or_default(Some(&session.extension_data), config);

        // Try the session's saved provider first whenever its type is
        // registered at all — not just when it's registered AND already
        // configured. The fallback below exists specifically to survive a
        // known provider type whose credentials were revoked/removed; gating
        // it on registry presence alone meant that case always hit a hard
        // create_with_working_dir error instead of ever reaching it.
        let primary_result = if crate::providers::get_from_registry(&provider_name)
            .await
            .is_ok()
        {
            Some(
                self.create_provider_with_session_scope(
                    session,
                    &provider_name,
                    extensions.clone(),
                )
                .await,
            )
        } else {
            None
        };

        let (provider, active_model_config, provider_changed) = match primary_result {
            Some(Ok(p)) => (p, model_config, false),
            Some(Err(error)) if session.credential_profile_id.is_some() => {
                return Err(anyhow!(
                    "Pinned credential profile is unavailable for provider '{}': {}",
                    provider_name,
                    error
                ));
            }
            None if session.credential_profile_id.is_some() => {
                return Err(anyhow!(
                    "Pinned provider '{}' is no longer available",
                    provider_name
                ));
            }
            primary_result => {
                let primary_error = primary_result.and_then(Result::err);

                let fallback_provider_name = config
                    .get_gosling_provider()
                    .ok()
                    .filter(|name| name != &provider_name)
                    .ok_or_else(|| match &primary_error {
                        Some(e) => anyhow!("Could not create provider '{}': {}", provider_name, e),
                        None => anyhow!(
                            "Could not create provider: provider '{}' not found",
                            provider_name
                        ),
                    })?;

                tracing::warn!(
                    "Session provider '{}' unavailable ({}), falling back to '{}'",
                    provider_name,
                    primary_error
                        .as_ref()
                        .map(|e| e.to_string())
                        .unwrap_or_else(|| "not found in registry".to_string()),
                    fallback_provider_name
                );

                let fallback_model_name = config.get_gosling_model().ok().ok_or_else(|| {
                    anyhow!("Could not configure fallback provider: missing model")
                })?;
                let fallback_model_config = crate::model_config::model_config_from_user_config(
                    &fallback_provider_name,
                    &fallback_model_name,
                )
                .map_err(|e| {
                    anyhow!("Could not configure fallback provider: invalid model {}", e)
                })?;

                let fallback_provider = crate::providers::create_with_working_dir(
                    &fallback_provider_name,
                    extensions,
                    session.working_dir.clone(),
                )
                .await
                .map_err(|e| {
                    anyhow!(
                        "Could not create provider '{}' or fallback '{}': {}",
                        provider_name,
                        fallback_provider_name,
                        e
                    )
                })?;

                if let Err(e) = self
                    .config
                    .session_manager
                    .update(&session.id)
                    .provider_name(&fallback_provider_name)
                    .model_config(fallback_model_config.clone())
                    .apply()
                    .await
                {
                    tracing::warn!("Failed to update session provider: {}", e);
                }

                (fallback_provider, fallback_model_config, true)
            }
        };

        self.update_provider_with_mode(
            provider,
            active_model_config,
            &session.id,
            session.gosling_mode,
        )
        .await?;
        Ok(provider_changed)
    }

    async fn create_provider_with_session_scope(
        &self,
        session: &Session,
        provider_name: &str,
        extensions: Vec<ExtensionConfig>,
    ) -> Result<Arc<dyn Provider>> {
        let Some(profile_id) = session.credential_profile_id.as_deref() else {
            return crate::providers::create_with_working_dir(
                provider_name,
                extensions,
                session.working_dir.clone(),
            )
            .await;
        };
        let service = self
            .config
            .workspace_service
            .as_ref()
            .ok_or_else(|| anyhow!("Workspace credential service is unavailable"))?;
        let resolution = service.profile_resolution(profile_id)?;
        if resolution.provider != provider_name {
            // The pinned credential profile's scope only covers its own
            // provider's config keys. Falling through to an unscoped
            // provider here would silently run this session on global
            // config instead of the isolated profile the session (and its
            // "Pinned" UI indicator) claims to be using — defeating
            // workspace credential isolation without telling the user.
            // Fail closed instead: a mismatch means the workspace's
            // default provider and its default credential binding disagree,
            // or the caller is trying to switch a pinned session to a
            // provider outside its pinned profile. Both need the workspace
            // (or the session's provider selection) fixed, not a silent
            // downgrade.
            bail!(
                "credential profile is pinned to provider '{}', not '{provider_name}'",
                resolution.provider
            );
        }
        let scope = service.config_scope(profile_id).await?;
        Config::with_resolution_scope(scope, async {
            crate::providers::create_with_working_dir(
                provider_name,
                extensions,
                session.working_dir.clone(),
            )
            .await
        })
        .await
    }

    /// Override the system prompt with a custom template
    pub async fn override_system_prompt(&self, template: String) {
        let mut prompt_manager = self.prompt_manager.lock().await;
        prompt_manager.set_system_prompt_override(template);
    }

    pub async fn configure_shell_instructions(&self, template: String) {
        let mut prompt_manager = self.prompt_manager.lock().await;
        prompt_manager.configure_shell_instructions(template);
    }

    pub async fn clear_system_prompt_override(&self) {
        let mut prompt_manager = self.prompt_manager.lock().await;
        prompt_manager.clear_system_prompt_override();
    }

    pub async fn list_extension_prompts(&self, session_id: &str) -> HashMap<String, Vec<Prompt>> {
        // `list_prompts` never returns `Err` (per-extension failures are
        // collected and logged internally, not propagated) - `unwrap_or_default`
        // documents that contract instead of asserting a failure mode the
        // callee cannot produce (REL-GOS-003).
        self.extension_manager
            .list_prompts(session_id, CancellationToken::default())
            .await
            .unwrap_or_default()
    }

    pub async fn get_prompt(
        &self,
        session_id: &str,
        name: &str,
        arguments: Value,
    ) -> Result<GetPromptResult> {
        // First find which extension has this prompt
        let prompts = self
            .extension_manager
            .list_prompts(session_id, CancellationToken::default())
            .await
            .map_err(|e| anyhow!("Failed to list prompts: {}", e))?;

        if let Some(extension) = prompts
            .iter()
            .find(|(_, prompt_list)| prompt_list.iter().any(|p| p.name == name))
            .map(|(extension, _)| extension)
        {
            return self
                .extension_manager
                .get_prompt(
                    session_id,
                    extension,
                    name,
                    arguments,
                    CancellationToken::default(),
                )
                .await
                .map_err(|e| anyhow!("Failed to get prompt: {}", e));
        }

        Err(anyhow!("Prompt '{}' not found", name))
    }

    pub async fn get_plan_prompt(&self, session_id: &str) -> Result<String> {
        let tools = self
            .extension_manager
            .get_prefixed_tools(session_id, None)
            .await?;
        let tools_info = tools
            .into_iter()
            .map(|tool| {
                ToolInfo::new(
                    &tool.name,
                    tool.description
                        .as_ref()
                        .map(|d| d.as_ref())
                        .unwrap_or_default(),
                    get_parameter_names(&tool),
                    None,
                )
            })
            .collect();

        let plan_prompt = self.extension_manager.get_planning_prompt(tools_info).await;

        Ok(plan_prompt)
    }

    pub async fn handle_tool_result(&self, id: String, result: ToolResult<CallToolResult>) {
        self.frontend_tool_result_router.deliver(id, result).await;
    }

    /// Frontend tool calls can legitimately involve a human in the loop, so
    /// this mirrors the elicitation wait's 300s bound rather than a short
    /// per-request timeout.
    const FRONTEND_TOOL_RESULT_TIMEOUT: Duration = Duration::from_secs(300);

    pub(super) async fn wait_for_frontend_tool_result(
        &self,
        request_id: String,
    ) -> Option<ToolResult<CallToolResult>> {
        match self
            .frontend_tool_result_router
            .register(request_id.clone())
            .await
        {
            FrontendToolResultRegistration::Ready(result) => Some(result),
            // A crashed/disconnected frontend never calls `deliver`, and
            // nothing else resolves this channel - without a bound, the whole
            // reply stream hangs forever (REL-GOS-002).
            FrontendToolResultRegistration::Pending(rx) => {
                match tokio::time::timeout(Self::FRONTEND_TOOL_RESULT_TIMEOUT, rx).await {
                    Ok(received) => received.ok(),
                    Err(_) => {
                        tracing::warn!(
                            request_id = %request_id,
                            timeout = ?Self::FRONTEND_TOOL_RESULT_TIMEOUT,
                            "Frontend tool result wait timed out",
                        );
                        None
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests;
