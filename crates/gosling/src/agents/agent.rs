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
mod reply_entry;
mod reply_stream;
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
