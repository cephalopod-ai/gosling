mod builder;
mod completion;
pub mod editor;
mod elicitation;
mod export;
mod input;
mod output;
pub mod streaming_buffer;
mod thinking;

use gosling::conversation::Conversation;
use gosling::session::{
    InteractionPolicy, NewPlanFeedback, PlanExpectation, PlanSnapshot, PlanStatus,
};
use std::env;
use std::str::FromStr;
use tokio::signal::ctrl_c;
use tokio_util::task::AbortOnDropHandle;

pub use self::export::message_to_markdown;
pub use builder::{build_session, SessionBuilderConfig};
use console::Color;
use gosling::agents::AgentEvent;
use gosling::agents::SUBAGENT_TOOL_REQUEST_TYPE;
use gosling::permission::permission_confirmation::PrincipalType;
use gosling::permission::Permission;
use gosling::permission::PermissionConfirmation;
use gosling::providers::base::ProviderUsage;
use gosling::utils::safe_truncate;
use gosling_providers::thinking::ThinkingEffort;

use anyhow::Result;
use completion::GoslingCompleter;
use gosling::agents::extension::{Envs, ExtensionConfig, PLATFORM_EXTENSIONS};
use gosling::agents::{Agent, SessionConfig, COMPACT_TRIGGERS};
use gosling::config::extensions::name_to_key;
use gosling::config::{Config, GoslingMode};
use input::InputResult;
use rmcp::model::ServerNotification;
use rmcp::model::{ElicitationAction, PromptMessage};
use rmcp::model::{ErrorCode, ErrorData};
use strum::VariantNames;

use gosling::config::paths::Paths;
use gosling::conversation::message::{ActionRequiredData, Message, MessageContent};
use rustyline::EditMode;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;
use tokio;
use tokio_util::sync::CancellationToken;
use tracing::warn;

const CANCELLED_TURN_NOTICE: &str = "Run cancelled by user before completion.";
const GOSLING_PLANNER_CONTEXT_LIMIT: &str = "GOSLING_PLANNER_CONTEXT_LIMIT";
const GOSLING_PLANNER_MODEL: &str = "GOSLING_PLANNER_MODEL";
const GOSLING_PLANNER_PROVIDER: &str = "GOSLING_PLANNER_PROVIDER";

#[derive(Debug, PartialEq, Eq)]
enum PlanImplementationSubmission {
    Started,
    Failed(String),
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonOutput {
    messages: Vec<Message>,
    metadata: JsonMetadata,
}

#[derive(Serialize, Deserialize, Debug)]
struct JsonMetadata {
    total_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    input_tokens: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    output_tokens: Option<i32>,
    status: String,
}

#[derive(Serialize, Debug)]
#[serde(tag = "type", rename_all = "snake_case")]
enum StreamEvent {
    Message {
        message: Message,
    },
    Notification {
        extension_id: String,
        #[serde(flatten)]
        data: NotificationData,
    },
    Error {
        error: String,
    },
    Complete {
        total_tokens: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        input_tokens: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        output_tokens: Option<i32>,
    },
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "snake_case")]
enum NotificationData {
    Log {
        message: String,
    },
    Progress {
        progress: f64,
        total: Option<f64>,
        message: Option<String>,
    },
}

fn model_switch_label(provider: &str, model: &str, effort: Option<ThinkingEffort>) -> String {
    match effort {
        Some(ThinkingEffort::Off) | None => format!("{provider}/{model}"),
        Some(effort) => format!("{provider}/{model} {effort}"),
    }
}

struct HistoryManager {
    history_file: PathBuf,
    old_history_file: PathBuf,
    enabled: bool,
}

impl HistoryManager {
    fn new(enabled: bool) -> Self {
        Self {
            history_file: Paths::state_dir().join("history.txt"),
            old_history_file: Paths::config_dir().join("history.txt"),
            enabled,
        }
    }

    fn load(
        &self,
        editor: &mut rustyline::Editor<GoslingCompleter, rustyline::history::DefaultHistory>,
    ) {
        if !self.enabled {
            return;
        }
        if let Some(parent) = self.history_file.parent() {
            if !parent.exists() {
                if let Err(e) = std::fs::create_dir_all(parent) {
                    eprintln!("Warning: Failed to create history directory: {}", e);
                }
            }
        }

        let history_files = [&self.history_file, &self.old_history_file];
        if let Some(file) = history_files.iter().find(|f| f.exists()) {
            if let Err(err) = editor.load_history(file) {
                eprintln!("Warning: Failed to load command history: {}", err);
            }
        }
    }

    fn save(
        &self,
        editor: &mut rustyline::Editor<GoslingCompleter, rustyline::history::DefaultHistory>,
    ) {
        if !self.enabled {
            return;
        }
        if let Err(err) = editor.save_history(&self.history_file) {
            eprintln!("Warning: Failed to save command history: {}", err);
        } else if self.old_history_file.exists() {
            if let Err(err) = std::fs::remove_file(&self.old_history_file) {
                eprintln!("Warning: Failed to remove old history file: {}", err);
            }
        }
    }
}

pub struct CliSession {
    agent: Agent,
    messages: Conversation,
    session_id: String,
    completion_cache: Arc<std::sync::RwLock<CompletionCache>>,
    debug: bool,
    max_turns: Option<u32>,
    edit_mode: Option<EditMode>,
    output_format: String,
    stats: bool,
    persist_local_state: bool,
    _ephemeral_state: Option<EphemeralSessionState>,
}

struct EphemeralSessionState {
    _session_dir: tempfile::TempDir,
    _transcript_suppression: gosling::providers::utils::LocalTranscriptSuppressionGuard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HintStatus {
    Default,
    Interrupted,
    MaybeExit,
}

// Cache structure for completion data
pub struct CompletionCache {
    pub prompts: HashMap<String, Vec<String>>,
    pub prompt_info: HashMap<String, output::PromptInfo>,
    pub last_updated: Instant,
    pub hint_status: HintStatus,
}

impl CompletionCache {
    fn new() -> Self {
        Self {
            prompts: HashMap::new(),
            prompt_info: HashMap::new(),
            last_updated: Instant::now(),
            hint_status: HintStatus::Default,
        }
    }
}

impl CliSession {
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        agent: Agent,
        session_id: String,
        debug: bool,
        max_turns: Option<u32>,
        edit_mode: Option<EditMode>,
        output_format: String,
        stats: bool,
    ) -> Self {
        let messages = agent
            .config
            .session_manager
            .get_session(&session_id, true)
            .await
            .map(|session| session.conversation.unwrap_or_default())
            .unwrap();

        CliSession {
            agent,
            messages,
            session_id,
            completion_cache: Arc::new(std::sync::RwLock::new(CompletionCache::new())),
            debug,
            max_turns,
            edit_mode,
            output_format,
            stats,
            persist_local_state: true,
            _ephemeral_state: None,
        }
    }

    pub(super) fn use_ephemeral_state(
        &mut self,
        session_dir: tempfile::TempDir,
        transcript_suppression: gosling::providers::utils::LocalTranscriptSuppressionGuard,
    ) {
        self.persist_local_state = false;
        self._ephemeral_state = Some(EphemeralSessionState {
            _session_dir: session_dir,
            _transcript_suppression: transcript_suppression,
        });
    }

    pub fn session_id(&self) -> &String {
        &self.session_id
    }

    /// Parse a stdio extension command string into an ExtensionConfig
    /// Format: "ENV1=val1 ENV2=val2 command args..."
    pub fn parse_stdio_extension(extension_command: &str) -> Result<ExtensionConfig> {
        let mut parts = gosling::utils::split_command_args(extension_command)?;
        let mut envs = HashMap::new();

        while let Some(part) = parts.first() {
            if !part.contains('=') {
                break;
            }
            let env_part = parts.remove(0);
            let (key, value) = env_part.split_once('=').unwrap();
            envs.insert(key.to_string(), value.to_string());
        }

        if parts.is_empty() {
            return Err(anyhow::anyhow!("No command provided in extension string"));
        }

        let cmd = parts.remove(0);
        let name = std::path::Path::new(&cmd)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("unnamed")
            .to_string();

        Ok(ExtensionConfig::Stdio {
            name,
            cmd,
            args: parts,
            envs: Envs::new(envs),
            env_keys: Vec::new(),
            description: gosling::config::DEFAULT_EXTENSION_DESCRIPTION.to_string(),
            timeout: Some(gosling::config::DEFAULT_EXTENSION_TIMEOUT),
            cwd: None,
            bundled: None,
            available_tools: Vec::new(),
        })
    }

    pub fn parse_streamable_http_extension(extension_url: &str, timeout: u64) -> ExtensionConfig {
        let name = url::Url::parse(extension_url)
            .ok()
            .map(|u| {
                let mut s = String::new();
                if let Some(host) = u.host_str() {
                    s.push_str(host);
                }
                if let Some(port) = u.port() {
                    s.push('_');
                    s.push_str(&port.to_string());
                }
                let path = u.path().trim_matches('/');
                if !path.is_empty() {
                    s.push('_');
                    s.push_str(path);
                }
                name_to_key(&s)
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "unnamed".to_string());

        ExtensionConfig::StreamableHttp {
            name,
            uri: extension_url.to_string(),
            envs: Envs::new(HashMap::new()),
            env_keys: Vec::new(),
            headers: HashMap::new(),
            description: gosling::config::DEFAULT_EXTENSION_DESCRIPTION.to_string(),
            timeout: Some(timeout),
            socket: None,
            client_id: None,
            client_secret_key: None,
            scopes: Vec::new(),
            bundled: None,
            available_tools: Vec::new(),
        }
    }

    /// Parse builtin extension names (comma-separated) into ExtensionConfigs
    pub fn parse_builtin_extensions(builtin_name: &str) -> Vec<ExtensionConfig> {
        builtin_name
            .split(',')
            .map(|name| {
                let extension_name = name.trim();
                if PLATFORM_EXTENSIONS.contains_key(extension_name) {
                    ExtensionConfig::Platform {
                        name: extension_name.to_string(),
                        description: extension_name.to_string(),
                        display_name: None,
                        bundled: None,
                        available_tools: Vec::new(),
                    }
                } else {
                    ExtensionConfig::Builtin {
                        name: extension_name.to_string(),
                        display_name: None,
                        timeout: None,
                        bundled: None,
                        description: extension_name.to_string(),
                        available_tools: Vec::new(),
                    }
                }
            })
            .collect()
    }

    async fn add_and_persist_extensions(&mut self, configs: Vec<ExtensionConfig>) -> Result<()> {
        for config in configs {
            self.agent
                .add_extension(config, &self.session_id)
                .await
                .map_err(|e| anyhow::anyhow!("Failed to start extension: {}", e))?;
        }

        self.invalidate_completion_cache().await;

        Ok(())
    }

    pub async fn add_extension(&mut self, extension_command: String) -> Result<()> {
        let config = Self::parse_stdio_extension(&extension_command)?;
        self.add_and_persist_extensions(vec![config]).await
    }

    pub async fn add_streamable_http_extension(&mut self, extension_url: String) -> Result<()> {
        let config = Self::parse_streamable_http_extension(
            &extension_url,
            gosling::config::DEFAULT_EXTENSION_TIMEOUT,
        );
        self.add_and_persist_extensions(vec![config]).await
    }

    pub async fn add_builtin(&mut self, builtin_name: String) -> Result<()> {
        let configs = Self::parse_builtin_extensions(&builtin_name);
        self.add_and_persist_extensions(configs).await
    }

    pub async fn list_prompts(
        &mut self,
        extension: Option<String>,
    ) -> Result<HashMap<String, Vec<String>>> {
        let prompts = self.agent.list_extension_prompts(&self.session_id).await;

        // Early validation if filtering by extension
        if let Some(filter) = &extension {
            if !prompts.contains_key(filter) {
                return Err(anyhow::anyhow!("Extension '{}' not found", filter));
            }
        }

        // Convert prompts into filtered map of extension names to prompt names
        Ok(prompts
            .into_iter()
            .filter(|(ext, _)| extension.as_ref().is_none_or(|f| f == ext))
            .map(|(extension, prompt_list)| {
                let names = prompt_list.into_iter().map(|p| p.name).collect();
                (extension, names)
            })
            .collect())
    }

    pub async fn get_prompt_info(&mut self, name: &str) -> Result<Option<output::PromptInfo>> {
        let prompts = self.agent.list_extension_prompts(&self.session_id).await;

        // Find which extension has this prompt
        for (extension, prompt_list) in prompts {
            if let Some(prompt) = prompt_list.iter().find(|p| p.name == name) {
                return Ok(Some(output::PromptInfo {
                    name: prompt.name.clone(),
                    description: prompt.description.clone(),
                    arguments: prompt.arguments.clone(),
                    extension: Some(extension),
                }));
            }
        }

        Ok(None)
    }

    pub async fn get_prompt(&mut self, name: &str, arguments: Value) -> Result<Vec<PromptMessage>> {
        Ok(self
            .agent
            .get_prompt(&self.session_id, name, arguments)
            .await?
            .messages)
    }

    /// Process a single message and get the response
    pub(crate) async fn process_message(
        &mut self,
        message: Message,
        cancel_token: CancellationToken,
        interactive: bool,
    ) -> Result<()> {
        let cancel_token = cancel_token.clone();
        self.push_message(message);
        self.process_agent_response(interactive, cancel_token)
            .await?;
        Ok(())
    }

    /// Start an interactive session, optionally with an initial message
    pub async fn interactive(&mut self, prompt: Option<String>) -> Result<()> {
        let result = self.run_interactive(prompt).await;

        self.agent
            .emit_hook(gosling::hooks::HookEvent::SessionEnd, &self.session_id)
            .await;

        if result.is_ok() {
            println!(
                "\n  {} {}",
                console::style("●").red(),
                console::style(format!("session closed · {}", &self.session_id)).dim()
            );
        }

        result
    }

    async fn run_interactive(&mut self, prompt: Option<String>) -> Result<()> {
        if let Some(prompt) = prompt {
            let msg = Message::user().with_text(&prompt);
            self.process_message(msg, CancellationToken::default(), true)
                .await?;
        }

        self.update_completion_cache().await?;

        let mut editor = self.create_editor()?;
        let history_manager = HistoryManager::new(self.persist_local_state);
        history_manager.load(&mut editor);

        loop {
            self.display_context_usage().await?;

            let conversation_strings: Vec<String> = self
                .messages
                .iter()
                .map(|msg| {
                    let role = match msg.role {
                        rmcp::model::Role::User => "User",
                        rmcp::model::Role::Assistant => "Assistant",
                    };
                    format!("## {}: {}", role, msg.as_concat_text())
                })
                .collect();

            self.run_status_hook("waiting").await;
            let input = input::get_input(&mut editor, Some(&conversation_strings))?;
            if matches!(input, InputResult::Exit) {
                break;
            }
            self.handle_input(input, &history_manager, &mut editor, &conversation_strings)
                .await?;
        }

        Ok(())
    }

    fn create_editor(
        &self,
    ) -> Result<rustyline::Editor<GoslingCompleter, rustyline::history::DefaultHistory>> {
        let builder =
            rustyline::Config::builder().completion_type(rustyline::CompletionType::Circular);
        let builder = match self.edit_mode {
            Some(mode) => builder.edit_mode(mode),
            None => builder.edit_mode(EditMode::Emacs),
        };
        let config = builder.build();
        let mut editor =
            rustyline::Editor::<GoslingCompleter, rustyline::history::DefaultHistory>::with_config(
                config,
            )?;
        let completer = GoslingCompleter::new(self.completion_cache.clone());
        editor.set_helper(Some(completer));
        Ok(editor)
    }

    async fn handle_input(
        &mut self,
        input: InputResult,
        history: &HistoryManager,
        editor: &mut rustyline::Editor<GoslingCompleter, rustyline::history::DefaultHistory>,
        conversation_messages: &[String],
    ) -> Result<()> {
        match input {
            InputResult::Message(content) => {
                self.handle_message_input(&content, history, editor).await?;
            }
            InputResult::Exit => unreachable!("Exit is handled in the main loop"),
            InputResult::AddExtension(cmd) => {
                history.save(editor);
                match self.add_extension(cmd.clone()).await {
                    Ok(_) => output::render_extension_success(&cmd),
                    Err(e) => output::render_extension_error(&cmd, &e.to_string()),
                }
            }
            InputResult::AddBuiltin(names) => {
                history.save(editor);
                match self.add_builtin(names.clone()).await {
                    Ok(_) => output::render_builtin_success(&names),
                    Err(e) => output::render_builtin_error(&names, &e.to_string()),
                }
            }
            InputResult::ToggleTheme => {
                history.save(editor);
                self.handle_toggle_theme();
            }
            InputResult::ToggleFullToolOutput => {
                history.save(editor);
                self.handle_toggle_full_tool_output();
            }
            InputResult::SelectTheme(theme_name) => {
                history.save(editor);
                self.handle_select_theme(&theme_name);
            }
            InputResult::Retry => {}
            InputResult::ListPrompts(extension) => {
                history.save(editor);
                match self.list_prompts(extension).await {
                    Ok(prompts) => output::render_prompts(&prompts),
                    Err(e) => output::render_error(&e.to_string()),
                }
            }
            InputResult::GoslingMode(mode) => {
                history.save(editor);
                self.handle_gosling_mode(&mode).await?;
            }
            InputResult::Model(model) => {
                history.save(editor);
                self.handle_model(model.as_deref()).await?;
            }
            InputResult::Plan(options) => {
                history.save(editor);
                if let Err(error) = self.handle_plan_mode(options).await {
                    self.render_plan_command_error(&error);
                }
            }
            InputResult::PlanStatus => {
                history.save(editor);
                if let Err(error) = self.handle_plan_status().await {
                    self.render_plan_command_error(&error);
                }
            }
            InputResult::PlanFeedback(feedback) => {
                history.save(editor);
                if let Err(error) = self.handle_plan_feedback(&feedback).await {
                    self.render_plan_command_error(&error);
                }
            }
            InputResult::PlanComment(comment) => {
                history.save(editor);
                if let Err(error) = self.handle_plan_comment(&comment).await {
                    self.render_plan_command_error(&error);
                }
            }
            InputResult::PlanApprove => {
                history.save(editor);
                if let Err(error) = self.handle_plan_approve().await {
                    self.render_plan_command_error(&error);
                }
            }
            InputResult::PlanApproveAndRun => {
                history.save(editor);
                if let Err(error) = self.handle_plan_approve_and_run().await {
                    self.render_plan_command_error(&error);
                }
            }
            InputResult::PlanAbandon => {
                history.save(editor);
                if let Err(error) = self.handle_plan_abandon().await {
                    self.render_plan_command_error(&error);
                }
            }
            InputResult::PlanEnd => {
                history.save(editor);
                if let Err(error) = self.handle_plan_end().await {
                    self.render_plan_command_error(&error);
                }
            }
            InputResult::PlanExport => {
                history.save(editor);
                if let Err(error) = self.handle_plan_export().await {
                    self.render_plan_command_error(&error);
                }
            }
            InputResult::Status => {
                self.display_session_status().await?;
            }
            InputResult::Clear => {
                history.save(editor);
                self.handle_clear().await?;
            }
            InputResult::PromptCommand(opts) => {
                history.save(editor);
                self.handle_prompt_command(opts).await?;
            }
            InputResult::Compact => {
                history.save(editor);
                self.handle_compact().await?;
            }
            InputResult::Edit(prefill) => {
                history.save(editor);
                match crate::session::editor::resolve_editor_command() {
                    Some(editor_cmd) => {
                        let messages: Vec<&str> =
                            conversation_messages.iter().map(|s| s.as_str()).collect();
                        match crate::session::editor::get_editor_input(
                            &editor_cmd,
                            &messages,
                            prefill.as_deref(),
                        ) {
                            Ok((message, true)) => {
                                editor.add_history_entry(message.as_str())?;
                                history.save(editor);
                                self.handle_message_input(&message, history, editor).await?;
                            }
                            Ok((_, false)) => {}
                            Err(e) => {
                                output::render_error(&format!("Failed to open editor: {}", e));
                            }
                        }
                    }
                    None => {
                        output::render_error(
                            "No editor found. Set one with:\n  \
                                 GOSLING_PROMPT_EDITOR=vim in your environment or config.yaml\n  \
                                 or set $VISUAL or $EDITOR in your shell.",
                        );
                    }
                }
            }
            InputResult::LoadSkills(names) => {
                history.save(editor);
                self.handle_load_skills(&names).await?;
            }
            InputResult::ListSkills => {
                history.save(editor);
                self.handle_list_skills().await?;
            }
        }
        Ok(())
    }

    async fn handle_message_input(
        &mut self,
        content: &str,
        history: &HistoryManager,
        editor: &mut rustyline::Editor<GoslingCompleter, rustyline::history::DefaultHistory>,
    ) -> Result<()> {
        history.save(editor);
        self.submit_visible_turn(content, "thinking").await
    }

    fn handle_toggle_theme(&self) {
        let current = output::get_theme();
        let new_theme = match current {
            output::Theme::Ansi => {
                println!("Switching to Light theme");
                output::Theme::Light
            }
            output::Theme::Light => {
                println!("Switching to Dark theme");
                output::Theme::Dark
            }
            output::Theme::Dark => {
                println!("Switching to Ansi theme");
                output::Theme::Ansi
            }
        };
        output::set_theme(new_theme);
    }

    fn handle_select_theme(&self, theme_name: &str) {
        let new_theme = match theme_name {
            "light" => {
                println!("Switching to Light theme");
                output::Theme::Light
            }
            "dark" => {
                println!("Switching to Dark theme");
                output::Theme::Dark
            }
            "ansi" => {
                println!("Switching to Ansi theme");
                output::Theme::Ansi
            }
            _ => output::Theme::Dark,
        };
        output::set_theme(new_theme);
    }

    fn handle_toggle_full_tool_output(&self) {
        let enabled = output::toggle_full_tool_output();
        if enabled {
            println!(
                "{}",
                console::style(
                    "✓ Full tool output enabled - tool parameters will no longer be truncated"
                )
                .green()
            );
        } else {
            println!(
                "{}",
                console::style(
                    "✓ Full tool output disabled - tool parameters will be truncated to fit terminal width"
                )
                .dim()
            );
        }
    }

    async fn handle_gosling_mode(&self, mode: &str) -> Result<()> {
        let config = Config::global();
        if mode.trim().is_empty() {
            let session = self.get_session().await?;
            output::gosling_mode_message(&format!(
                "Current mode: '{}'. Usage: /mode <name> (one of: {})",
                session.gosling_mode,
                GoslingMode::VARIANTS.join(", ")
            ));
            return Ok(());
        }
        let mode = match GoslingMode::from_str(&mode.to_lowercase()) {
            Ok(mode) => mode,
            Err(_) => {
                output::render_error(&format!(
                    "Invalid mode '{mode}'. Mode must be one of: {}",
                    GoslingMode::VARIANTS.join(", ")
                ));
                return Ok(());
            }
        };
        self.agent
            .update_gosling_mode(mode, &self.session_id)
            .await?;
        config.set_gosling_mode(mode)?;
        output::gosling_mode_message(&format!("Gosling mode set to '{mode}'"));
        Ok(())
    }

    async fn handle_model(&self, model: Option<&str>) -> Result<()> {
        let provider = self.agent.provider().await?;
        let current_provider_name = provider.get_name().to_string();
        let current_model_config = self
            .agent
            .model_config_for_session(&self.session_id)
            .await?;
        let current_model_name = current_model_config.model_name.clone();

        if model.is_none() {
            output::gosling_mode_message(&format!(
                "Current session model: '{}' (provider '{}')",
                current_model_name, current_provider_name
            ));
            return Ok(());
        }

        let model_name = model.unwrap_or_default().trim();
        if model_name.is_empty() {
            output::render_error("Model name cannot be empty");
            return Ok(());
        }

        if provider.capabilities().context_ownership
            != gosling_providers::base::ContextOwnership::Gosling
        {
            output::render_error(&format!(
                "Session model switching is not supported for provider '{}' because it manages its own conversation context.",
                current_provider_name
            ));
            return Ok(());
        }

        let new_model_config =
            build_switched_model_config(&current_provider_name, model_name, &current_model_config)?;
        let new_model_name = new_model_config.model_name.clone();

        let configured_effort = Config::global().get_gosling_thinking_effort();
        let new_effort = new_model_config.thinking_effort().or(configured_effort);
        let current_effort = current_model_config.thinking_effort().or(configured_effort);
        if new_model_config.model_name == current_model_config.model_name
            && new_effort == current_effort
        {
            output::gosling_mode_message(&format!(
                "Session already using model '{}' for provider '{}'",
                current_model_name, current_provider_name
            ));
            return Ok(());
        }

        self.agent
            .transition_provider(
                &self.session_id,
                &current_provider_name,
                new_model_config,
                gosling::session_handoff::SessionHandoffTriggerDto::ModelChangeRequiresRecreation,
                None,
                None,
                false,
            )
            .await?;
        let previous_label =
            model_switch_label(&current_provider_name, &current_model_name, current_effort);
        let new_label = model_switch_label(&current_provider_name, &new_model_name, new_effort);
        gosling::session::SessionManager::instance()
            .add_model_switch_record(
                &self.session_id,
                format!("Model changed: {previous_label} -> {new_label}"),
            )
            .await?;
        output::gosling_mode_message(&format!(
            "Session model switched from '{}' to '{}' for provider '{}'",
            current_model_name, new_model_name, current_provider_name
        ));
        Ok(())
    }

    async fn handle_plan_mode(&mut self, options: input::PlanCommandOptions) -> Result<()> {
        let current = self
            .agent
            .config
            .session_manager
            .plans()
            .snapshot(&self.session_id)
            .await?;
        let provider = self.agent.provider().await?;
        let model_config = self
            .agent
            .model_config_for_session(&self.session_id)
            .await?;
        let planner_model =
            resolve_cli_planner_model(provider.get_name(), &model_config, Config::global())?;
        let snapshot = self
            .agent
            .config
            .session_manager
            .plans()
            .start_or_resume(
                &self.session_id,
                provider.as_ref(),
                Some(planner_model),
                current.as_ref().map(|snapshot| snapshot.plan.generation),
            )
            .await?;

        if options.message_text.is_empty() {
            output::render_plan_snapshot(&snapshot, false);
            return Ok(());
        }
        if snapshot.plan.status != PlanStatus::Drafting {
            anyhow::bail!(
                "Plan generation {} is {}; use /plan-feedback to request a revision",
                snapshot.plan.generation,
                snapshot.plan.status
            );
        }

        self.submit_visible_turn(&options.message_text, "planning")
            .await
    }

    async fn handle_plan_status(&self) -> Result<()> {
        match self
            .agent
            .config
            .session_manager
            .plans()
            .snapshot(&self.session_id)
            .await?
        {
            Some(snapshot) => output::render_plan_snapshot(&snapshot, true),
            None => output::render_no_plan(),
        }
        Ok(())
    }

    async fn handle_plan_feedback(&mut self, feedback: &str) -> Result<()> {
        let feedback = feedback.trim();
        if feedback.is_empty() {
            anyhow::bail!("Usage: /plan-feedback <text>");
        }
        self.record_plan_feedback(NewPlanFeedback {
            body: feedback.to_string(),
            start_line: None,
            end_line: None,
            selected_text: None,
        })
        .await
    }

    async fn handle_plan_comment(&mut self, comment: &str) -> Result<()> {
        let (start_line, end_line, body) = parse_plan_comment(comment)?;
        self.record_plan_feedback(NewPlanFeedback {
            body,
            start_line: Some(start_line),
            end_line: Some(end_line),
            selected_text: None,
        })
        .await
    }

    async fn record_plan_feedback(&mut self, feedback: NewPlanFeedback) -> Result<()> {
        let current = self.require_current_plan().await?;
        if current.plan.status != PlanStatus::AwaitingReview {
            anyhow::bail!(
                "Plan generation {} is {}; feedback requires a plan awaiting review",
                current.plan.generation,
                current.plan.status
            );
        }
        let previous_feedback_ids = current
            .feedback
            .iter()
            .map(|item| item.id.as_str())
            .collect::<HashSet<_>>();
        let updated = self
            .agent
            .config
            .session_manager
            .plans()
            .add_feedback(
                &self.session_id,
                &PlanExpectation::for_snapshot(&current),
                feedback,
            )
            .await?;
        let new_feedback_ids = updated
            .feedback
            .iter()
            .filter(|item| !previous_feedback_ids.contains(item.id.as_str()))
            .map(|item| item.id.clone())
            .collect::<Vec<_>>();
        if new_feedback_ids.is_empty() {
            anyhow::bail!("Plan feedback was recorded, but its durable identifier was unavailable");
        }
        output::render_plan_feedback_recorded(&updated, &new_feedback_ids);
        let revision_prompt = format!(
            "Revise the current host-enforced plan using durable feedback {}. Update the plan revision and request review again when it is ready. Do not implement the plan.",
            new_feedback_ids.join(", ")
        );
        self.submit_visible_turn(&revision_prompt, "planning").await
    }

    async fn handle_plan_approve(&self) -> Result<()> {
        let approved = self
            .approve_current_plan(Some("approved from the CLI".to_string()))
            .await?;
        output::render_plan_approved(&approved, false);
        Ok(())
    }

    async fn handle_plan_approve_and_run(&mut self) -> Result<()> {
        let approved = self
            .approve_current_plan(Some(
                "approved and submitted for implementation from the CLI".to_string(),
            ))
            .await?;
        output::render_plan_approved(&approved, true);
        let implementation_prompt =
            gosling::session::approved_plan_implementation_reference(&approved)?;
        report_plan_implementation_submission(
            &approved,
            self.submit_visible_turn(&implementation_prompt, "thinking")
                .await,
        );
        Ok(())
    }

    async fn approve_current_plan(&self, decision_note: Option<String>) -> Result<PlanSnapshot> {
        let current = self.require_current_plan().await?;
        self.agent
            .config
            .session_manager
            .plans()
            .approve(
                &self.session_id,
                &PlanExpectation::for_snapshot(&current),
                decision_note,
            )
            .await
            .map_err(Into::into)
    }

    async fn handle_plan_abandon(&self) -> Result<()> {
        let current = self.require_current_plan().await?;
        if !current.plan.status.is_open() {
            anyhow::bail!(
                "Plan generation {} is already {}; only an open plan can be abandoned",
                current.plan.generation,
                current.plan.status
            );
        }
        let abandoned = self
            .agent
            .config
            .session_manager
            .plans()
            .abandon(
                &self.session_id,
                current.plan.generation,
                Some("abandoned from the CLI".to_string()),
            )
            .await?;
        output::render_plan_abandoned(&abandoned);
        Ok(())
    }

    async fn handle_plan_end(&self) -> Result<()> {
        let current = self.require_current_plan().await?;
        if current.plan.status == PlanStatus::AwaitingReview {
            if !std::io::stdin().is_terminal() {
                anyhow::bail!(
                    "A reviewable plan requires explicit abandonment; run /plan-abandon from a non-interactive client"
                );
            }
            let confirmed = cliclack::confirm(format!(
                "Abandon reviewable plan generation {}?",
                current.plan.generation
            ))
            .initial_value(false)
            .interact()?;
            if !confirmed {
                output::gosling_mode_message("Plan remains awaiting review.");
                return Ok(());
            }
        }
        self.handle_plan_abandon().await
    }

    async fn handle_plan_export(&self) -> Result<()> {
        let current = self.require_current_plan().await?;
        let markdown = self.export_selected_plan(&current).await?;
        output::render_plan_export(&markdown);
        Ok(())
    }

    async fn export_selected_plan(&self, selected: &PlanSnapshot) -> Result<String> {
        let revision = selected.active_revision.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Plan generation {} has no revision to export",
                selected.plan.generation
            )
        })?;
        self.agent
            .config
            .session_manager
            .plans()
            .export_markdown(
                &self.session_id,
                selected.plan.generation,
                selected.plan.status,
                &revision.id,
                &revision.content_sha256,
            )
            .await
            .map_err(Into::into)
    }

    async fn require_current_plan(&self) -> Result<PlanSnapshot> {
        self.agent
            .config
            .session_manager
            .plans()
            .snapshot(&self.session_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("No plan exists for this session; start one with /plan"))
    }

    async fn submit_visible_turn(&mut self, content: &str, status: &str) -> Result<()> {
        // Resolve the provider before changing local presentation state. Core performs
        // the authoritative policy/readiness check again before persisting the prompt.
        let _provider = self.agent.provider().await?;
        self.push_message(Message::user().with_text(content));

        if self.persist_local_state {
            if let Err(error) = crate::project_tracker::update_project_tracker(
                Some(content),
                Some(&self.session_id),
            ) {
                eprintln!(
                    "Warning: Failed to update project tracker with instruction: {}",
                    error
                );
            }
        }

        println!();
        self.run_status_hook(status).await;
        output::show_thinking();
        let start_time = Instant::now();
        let result = self
            .process_agent_response(true, CancellationToken::default())
            .await;
        output::hide_thinking();
        result?;

        let elapsed_str = format_elapsed_time(start_time.elapsed());
        println!("{}", console::style(format!("  ⏱ {}", elapsed_str)).dim());
        if let Some(snapshot) = self
            .agent
            .config
            .session_manager
            .plans()
            .snapshot(&self.session_id)
            .await?
            .filter(|snapshot| snapshot.plan.status == PlanStatus::AwaitingReview)
        {
            output::render_plan_snapshot(&snapshot, true);
            output::render_plan_review_commands();
        }
        Ok(())
    }

    fn render_plan_command_error(&self, error: &anyhow::Error) {
        output::render_error(&error.to_string());
    }

    async fn handle_clear(&mut self) -> Result<()> {
        if let Err(e) = self
            .agent
            .config
            .session_manager
            .replace_conversation(&self.session_id, &Conversation::default())
            .await
        {
            output::render_error(&format!("Failed to clear session: {}", e));
            return Ok(());
        }

        if let Err(e) = self
            .agent
            .config
            .session_manager
            .update(&self.session_id)
            .usage(gosling_providers::conversation::token_usage::Usage::new(
                Some(0),
                Some(0),
                Some(0),
            ))
            .apply()
            .await
        {
            output::render_error(&format!("Failed to reset token counts: {}", e));
            return Ok(());
        }

        self.messages.clear();
        tracing::info!("Chat context cleared by user.");
        output::render_message(
            &Message::assistant().with_text("Chat context cleared.\n"),
            self.debug,
        );
        Ok(())
    }

    async fn handle_load_skills(&mut self, names: &[String]) -> Result<()> {
        // NOTE: We don't validate the skill names here because the load_skill tool will
        // handle that and provide feedback to the user if any skill names are invalid.
        let message = format!(
            "Use the load_skill tool to load the following skills: {}.",
            names
                .iter()
                .map(|n| format!("\"{}\"", n))
                .collect::<Vec<_>>()
                .join(", ")
        );
        self.push_message(Message::user().with_text(&message));
        output::show_thinking();
        let result = self
            .process_agent_response(true, CancellationToken::default())
            .await;
        output::hide_thinking();
        result?;

        Ok(())
    }

    async fn handle_list_skills(&mut self) -> Result<()> {
        use comfy_table::{presets, Cell, ContentArrangement, Table};
        use gosling::custom_requests::SourceType;
        use gosling::skills::list_installed_skills;
        let cwd = std::env::current_dir().unwrap_or_default();
        let skills = list_installed_skills(Some(&cwd));

        if skills.is_empty() {
            println!("{}", console::style("No skills available.").yellow());
            return Ok(());
        }

        let mut table = Table::new();
        table.set_content_arrangement(ContentArrangement::Dynamic);
        table.load_preset(presets::ASCII_FULL);
        table.set_header(vec!["Skill", "Location", "Description"]);

        let mut sorted_skills = skills;
        sorted_skills.sort_by(|a, b| a.name.cmp(&b.name));

        for skill in &sorted_skills {
            let location = if skill.source_type == SourceType::BuiltinSkill {
                "built-in"
            } else if skill.global {
                "global"
            } else {
                "project"
            };
            table.add_row(vec![
                Cell::new(&skill.name),
                Cell::new(location),
                Cell::new(&skill.description),
            ]);
        }

        println!("{table}");
        Ok(())
    }

    async fn handle_compact(&mut self) -> Result<()> {
        let prompt = "Are you sure you want to compact this conversation? This will condense the message history.";
        let should_summarize = match cliclack::confirm(prompt).initial_value(true).interact() {
            Ok(choice) => choice,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::Interrupted {
                    false
                } else {
                    return Err(e.into());
                }
            }
        };

        if should_summarize {
            self.push_message(Message::user().with_text(COMPACT_TRIGGERS[0]));
            output::show_thinking();
            self.process_agent_response(true, CancellationToken::default())
                .await?;
            output::hide_thinking();
        } else {
            println!("{}", console::style("Compaction cancelled.").yellow());
        }
        Ok(())
    }

    /// Process a single message and exit
    pub async fn headless(&mut self, prompt: String) -> Result<()> {
        let message = Message::user().with_text(&prompt);
        let result = self
            .process_message(message, CancellationToken::default(), false)
            .await;
        self.agent
            .emit_hook(gosling::hooks::HookEvent::SessionEnd, &self.session_id)
            .await;
        result?;
        Ok(())
    }

    async fn run_status_hook(&self, status: &str) {
        self.run_status_hook_with(status, output::run_status_hook)
            .await;
    }

    async fn run_status_hook_with(&self, status: &str, run_hook: impl FnOnce(&str)) {
        if self.status_hook_allowed().await {
            run_hook(status);
        }
    }

    async fn status_hook_allowed(&self) -> bool {
        match self
            .agent
            .config
            .session_manager
            .plans()
            .interaction_policy(&self.session_id)
            .await
        {
            Ok(InteractionPolicy::Normal) => true,
            Ok(InteractionPolicy::Planning { .. }) => {
                warn!(
                    security.event_type = "planning_side_channel_denied",
                    security.reason = "planning_status_hook_denied",
                    session.id = self.session_id.as_str(),
                    "host planning boundary suppressed the configured CLI status hook"
                );
                false
            }
            Err(error) => {
                warn!(
                    security.event_type = "planning_side_channel_denied",
                    security.reason = "planning_state_unavailable",
                    session.id = self.session_id.as_str(),
                    error = %error,
                    "host planning boundary could not verify durable state for the configured CLI status hook"
                );
                false
            }
        }
    }

    async fn process_agent_response(
        &mut self,
        interactive: bool,
        cancel_token: CancellationToken,
    ) -> Result<()> {
        let is_json_mode = self.output_format == "json";
        let is_stream_json_mode = self.output_format == "stream-json";

        let session_config = SessionConfig {
            id: self.session_id.clone(),
            max_turns: self.max_turns,
            compacted_context: false,
            tail_limit: None,
        };
        let user_message = {
            let mut message = self
                .messages
                .pop()
                .ok_or_else(|| anyhow::anyhow!("No user message"))?;
            if message.id.is_none() {
                message = message.with_generated_id();
            }
            self.messages.push(message.clone());
            message
        };
        let turn_message_id = user_message.id.clone();

        let cancel_token_interrupt = cancel_token.clone();
        let handle = tokio::spawn(async move {
            if ctrl_c().await.is_ok() {
                cancel_token_interrupt.cancel();
            }
        });
        let _drop_handle = AbortOnDropHandle::new(handle);

        let mut stream = self
            .agent
            .reply(
                user_message,
                session_config.clone(),
                Some(cancel_token.clone()),
            )
            .await?;

        let mut progress_bars = output::McpSpinners::new();
        let cancel_token_clone = cancel_token.clone();
        let mut markdown_buffer = streaming_buffer::MarkdownBuffer::new();
        let mut prompted_credits_urls: HashSet<String> = HashSet::new();
        let mut thinking_header_shown = false;
        let run_started = Instant::now();
        let mut first_token_at: Option<Instant> = None;
        let mut last_usage: Option<ProviderUsage> = None;
        let mut terminal_error: Option<String> = None;
        let mut execution_limit_reached = false;
        let mut interrupted = false;

        use futures::StreamExt;
        loop {
            tokio::select! {
                result = stream.next() => {
                    match result {
                        Some(Ok(AgentEvent::Message(message))) => {
                            if !interactive && terminal_error.is_none() {
                                terminal_error = terminal_error_reason(&message);
                            }
                            execution_limit_reached |= execution_limit_reason(&message);
                            if first_token_at.is_none() && message_has_text(&message) {
                                first_token_at = Some(Instant::now());
                            }
                            if let Some((id, security_prompt)) = find_tool_confirmation(&message) {
                                if !interactive {
                                    let config = Config::global();
                                    let gosling_mode = config.get_gosling_mode().unwrap_or_default();
                                    self.agent.handle_confirmation(id.clone(), PermissionConfirmation {
                                        principal_type: PrincipalType::Tool,
                                        permission: non_interactive_confirmation_permission(),
                                    }).await;
                                    self.agent.config.session_manager
                                        .cancel_undispatched_tool_requests(&self.session_id, &id)
                                        .await?;
                                    cancel_token_clone.cancel();
                                    drop(stream);
                                    return Err(anyhow::anyhow!(
                                        "Tool approval required in non-interactive mode with GoslingMode::{gosling_mode}; the tool was denied because no operator is available."
                                    ));
                                }
                                let permission = prompt_tool_confirmation(&security_prompt)?;

                                if permission == Permission::Cancel {
                                    output::render_text("Tool call cancelled. Returning to chat...", Some(Color::Yellow), true);
                                    self.agent.handle_confirmation(id.clone(), PermissionConfirmation {
                                        principal_type: PrincipalType::Tool,
                                        permission: Permission::DenyOnce,
                                    }).await;
                                    let response_message = persist_cancelled_tool_response(
                                        &self.agent.config.session_manager,
                                        &self.session_id,
                                        id.clone(),
                                    )
                                    .await?;
                                    self.agent.config.session_manager
                                        .cancel_undispatched_tool_requests(&self.session_id, &id)
                                        .await?;
                                    self.messages.push(response_message);
                                    cancel_token_clone.cancel();
                                    drop(stream);
                                    break;
                                }
                                self.agent.handle_confirmation(id, PermissionConfirmation {
                                    principal_type: PrincipalType::Tool,
                                    permission,
                                }).await;
                            } else if let Some((elicitation_id, elicitation_message, schema)) = find_elicitation_request(&message) {
                                if !interactive {
                                    // Non-interactive/headless mode: cannot collect user input
                                    tracing::warn!(
                                        "Elicitation requested in non-interactive mode, cancelling"
                                    );
                                    cancel_token_clone.cancel();
                                    drop(stream);
                                    return Err(anyhow::anyhow!(
                                        "Elicitation requested but no interactive terminal is available to collect user input"
                                    ));
                                }

                                output::hide_thinking();
                                let _ = progress_bars.hide();

                                match elicitation::collect_elicitation_input(&elicitation_message, &schema) {
                                    Ok(input) => {
                                        match &input.action {
                                            ElicitationAction::Decline => {
                                                output::render_text("Information request declined.", Some(Color::Yellow), true);
                                            }
                                            ElicitationAction::Cancel => {
                                                output::render_text("Information request cancelled.", Some(Color::Yellow), true);
                                            }
                                            ElicitationAction::Accept => {}
                                        }

                                        let should_cancel = input.action == ElicitationAction::Cancel;
                                        let action = input.action;
                                        let user_data_value = serde_json::to_value(input.user_data)
                                            .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                                        let response_message = Message::user()
                                            .with_content(MessageContent::action_required_elicitation_response(
                                                elicitation_id,
                                                user_data_value,
                                                action,
                                            ))
                                            .with_visibility(false, true);
                                        self.messages.push(response_message.clone());
                                        // Elicitation responses return an empty stream - the response
                                        // unblocks the waiting tool call via ActionRequiredManager
                                        let _ = self.agent.reply(response_message, session_config.clone(), Some(cancel_token.clone())).await?;
                                        if should_cancel {
                                            cancel_token_clone.cancel();
                                            drop(stream);
                                            break;
                                        }
                                    }
                                    Err(e) => {
                                        output::render_error(&format!("Failed to collect input: {}", e));
                                        cancel_token_clone.cancel();
                                        drop(stream);
                                        break;
                                    }
                                }
                            } else {
                                log_tool_metrics(&message, &self.messages);
                                self.messages.push(message.clone());

                                if interactive { output::hide_thinking() };
                                let _ = progress_bars.hide();

                                if is_stream_json_mode {
                                    emit_stream_event(&StreamEvent::Message { message: message.clone() });
                                } else if !is_json_mode {
                                    output::render_message_streaming(&message, &mut markdown_buffer, &mut thinking_header_shown, self.debug);
                                    maybe_open_credits_top_up_url(
                                        &message,
                                        interactive,
                                        &mut prompted_credits_urls,
                                    );
                                }
                            }
                        }
                        Some(Ok(AgentEvent::Usage(usage))) => {
                            last_usage = Some(usage);
                        }
                        Some(Ok(AgentEvent::ContextUsage(_))) => {}
                        Some(Ok(AgentEvent::McpNotification((extension_id, notification)))) => {
                            handle_mcp_notification(
                                &extension_id,
                                &notification,
                                &mut progress_bars,
                                is_stream_json_mode,
                                interactive,
                                is_json_mode,
                                self.debug,
                            );
                        }
                        Some(Ok(AgentEvent::HistoryReplaced(updated_conversation))) => {
                            self.messages = updated_conversation;
                        }
                        Some(Err(e)) => {
                            handle_agent_error(&e, is_json_mode, is_stream_json_mode);
                            terminal_error = Some(e.to_string());
                            cancel_token_clone.cancel();
                            drop(stream);
                            // Another owner holds the session now: its history is not ours to
                            // truncate, and the error already tells the user to reload.
                            if is_turn_lease_lost(&e) {
                                break;
                            }
                            if let Err(e) = self
                                .handle_interrupted_messages(
                                    false,
                                    interactive,
                                    turn_message_id.as_deref(),
                                )
                                .await
                            {
                                eprintln!("Error handling interruption: {}", e);
                            } else if !is_stream_json_mode {
                                output::render_error(
                                    "The error above was an exception we were not able to handle.\n\
                                    These errors are often related to connection or authentication\n\
                                    We've removed the conversation up to the most recent user message\n\
                                    - depending on the error you may be able to continue",
                                );
                            }
                            break;
                        }
                        None => {
                            // A cancelled command can finish before this select polls cancellation.
                            interrupted = cancel_token_clone.is_cancelled();
                            drop(stream);
                            break;
                        }
                    }
                }
                _ = cancel_token_clone.cancelled() => {
                    interrupted = true;
                    drop(stream);
                    break;
                }
            }
        }

        if interrupted {
            if let Err(e) = self
                .handle_interrupted_messages(true, interactive, turn_message_id.as_deref())
                .await
            {
                eprintln!("Error handling interruption: {}", e);
            }
            if !interactive {
                terminal_error = Some("Run cancelled by user".to_string());
            }
        }

        if terminal_error.is_none() && execution_limit_reached {
            let notice_text = "Execution stopped before all requested work completed because an action or repetition limit was reached. Some requested operations did not run; do not treat earlier completion claims as authoritative.";
            let notice = Message::assistant()
                .with_text(notice_text)
                .with_generated_id();
            self.messages.push(notice.clone());
            let _ = self
                .agent
                .config
                .session_manager
                .add_message(&self.session_id, &notice)
                .await;
            if is_stream_json_mode {
                emit_stream_event(&StreamEvent::Message { message: notice });
                handle_agent_error(&anyhow::anyhow!(notice_text), false, true);
            } else if !is_json_mode {
                output::render_message(&notice, self.debug);
            }
            terminal_error = Some(notice_text.to_string());
        }

        if !is_json_mode && !is_stream_json_mode {
            output::flush_markdown_buffer_current_theme(&mut markdown_buffer);
        }

        if is_json_mode {
            let metadata = match self
                .agent
                .config
                .session_manager
                .get_session(&self.session_id, false)
                .await
            {
                Ok(session) => JsonMetadata {
                    total_tokens: session
                        .accumulated_usage
                        .total_tokens
                        .or(session.usage.total_tokens),
                    input_tokens: session
                        .accumulated_usage
                        .input_tokens
                        .or(session.usage.input_tokens),
                    output_tokens: session
                        .accumulated_usage
                        .output_tokens
                        .or(session.usage.output_tokens),
                    status: if terminal_error.is_some() {
                        "error".to_string()
                    } else {
                        "completed".to_string()
                    },
                },
                Err(_) => JsonMetadata {
                    total_tokens: None,
                    input_tokens: None,
                    output_tokens: None,
                    status: if terminal_error.is_some() {
                        "error".to_string()
                    } else {
                        "completed".to_string()
                    },
                },
            };
            let json_output = JsonOutput {
                messages: self.messages.messages().to_vec(),
                metadata,
            };
            println!("{}", serde_json::to_string_pretty(&json_output)?);
        } else if is_stream_json_mode {
            let session = self
                .agent
                .config
                .session_manager
                .get_session(&self.session_id, false)
                .await
                .ok();
            let (total_tokens, input_tokens, output_tokens) = match session {
                Some(s) => (
                    s.accumulated_usage.total_tokens.or(s.usage.total_tokens),
                    s.accumulated_usage.input_tokens.or(s.usage.input_tokens),
                    s.accumulated_usage.output_tokens.or(s.usage.output_tokens),
                ),
                None => (None, None, None),
            };
            if terminal_error.is_none() {
                emit_stream_event(&StreamEvent::Complete {
                    total_tokens,
                    input_tokens,
                    output_tokens,
                });
            }
        } else {
            println!();
            if self.stats {
                print_run_stats(run_started, first_token_at, last_usage.as_ref());
            }
        }

        match terminal_error {
            Some(error) => Err(anyhow::anyhow!(error)),
            None => Ok(()),
        }
    }

    async fn handle_interrupted_messages(
        &mut self,
        interrupt: bool,
        interactive: bool,
        turn_message_id: Option<&str>,
    ) -> Result<()> {
        if interrupt {
            let mut cache = self.completion_cache.write().unwrap();
            cache.hint_status = HintStatus::Interrupted;
        }

        if let Some(message_id) = turn_message_id {
            if !interactive {
                if interrupt {
                    let notice = Message::assistant()
                        .with_text(CANCELLED_TURN_NOTICE)
                        .with_generated_id();
                    self.messages.push(notice.clone());
                    self.agent
                        .config
                        .session_manager
                        .add_message(&self.session_id, &notice)
                        .await?;
                }
                return Ok(());
            }

            if interrupt {
                let session_manager = &self.agent.config.session_manager;
                let turn_tool_request_ids = turn_tool_request_ids(&self.messages, message_id);
                if turn_tool_request_ids.is_empty() {
                    session_manager
                        .truncate_conversation_after_message(&self.session_id, message_id)
                        .await?;
                } else {
                    // A dispatched tool may already have had side effects, and its ledger row
                    // outlives a truncation: recovery would re-insert its request and in-doubt
                    // response with no preceding user prompt. Keep the turn and answer every
                    // request so the stored history stays well-formed.
                    for request_id in &turn_tool_request_ids {
                        session_manager
                            .cancel_undispatched_tool_requests(&self.session_id, request_id)
                            .await?;
                    }
                    session_manager
                        .recover_tool_operations(&self.session_id)
                        .await?;
                }
                self.messages = session_manager
                    .get_session(&self.session_id, true)
                    .await?
                    .conversation
                    .unwrap_or_default();
                let notice = Message::assistant()
                    .with_text(CANCELLED_TURN_NOTICE)
                    .with_generated_id();
                session_manager
                    .add_message(&self.session_id, &notice)
                    .await?;
                self.messages.push(notice.clone());
                if self.output_format == "text" {
                    output::render_message(&notice, self.debug);
                }
            } else {
                self.agent
                    .config
                    .session_manager
                    .truncate_conversation_from_message(&self.session_id, message_id)
                    .await?;
                remove_local_turn(&mut self.messages, message_id);
                let assistant_msg =
                    Message::assistant().with_text("Yes — what would you like me to do?");
                self.push_message(assistant_msg.clone());
                if self.output_format == "text" {
                    output::render_message(&assistant_msg, self.debug);
                }
            }
            return Ok(());
        }

        let tool_requests = self
            .messages
            .last()
            .filter(|msg| msg.role == rmcp::model::Role::Assistant)
            .map_or(Vec::new(), |msg| {
                msg.content
                    .iter()
                    .filter_map(|content| {
                        if let MessageContent::ToolRequest(req) = content {
                            Some((req.id.clone(), req.tool_call.clone()))
                        } else {
                            None
                        }
                    })
                    .collect()
            });

        let interrupt_prompt = "Yes — what would you like me to do?";

        if !tool_requests.is_empty() {
            let mut response_message = Message::user();

            let notification = if interrupt {
                "Interrupted by the user to make a correction".to_string()
            } else {
                "An uncaught error happened during tool use".to_string()
            };
            for (req_id, _) in &tool_requests {
                response_message.content.push(MessageContent::tool_response(
                    req_id.clone(),
                    Err(ErrorData {
                        code: ErrorCode::INTERNAL_ERROR,
                        message: std::borrow::Cow::from(notification.clone()),
                        data: None,
                    }),
                ));
            }
            self.push_message(response_message);
            self.push_message(Message::assistant().with_text(interrupt_prompt));
            if self.output_format == "text" {
                output::render_message(
                    &Message::assistant().with_text(interrupt_prompt),
                    self.debug,
                );
            }
        } else if let Some(last_msg) = self.messages.last() {
            if last_msg.role == rmcp::model::Role::User {
                match last_msg.content.first() {
                    Some(MessageContent::ToolResponse(_)) => {
                        self.push_message(Message::assistant().with_text(interrupt_prompt));
                        if self.output_format == "text" {
                            output::render_message(
                                &Message::assistant().with_text(interrupt_prompt),
                                self.debug,
                            );
                        }
                    }
                    Some(_) => {
                        self.messages.pop();
                        let assistant_msg = Message::assistant().with_text(interrupt_prompt);
                        self.push_message(assistant_msg.clone());
                        if self.output_format == "text" {
                            output::render_message(&assistant_msg, self.debug);
                        }
                    }
                    None => {
                        // Empty message content — nothing to do, just continue gracefully
                    }
                }
            }
        }
        Ok(())
    }

    /// Update the completion cache with fresh data
    /// This should be called before the interactive session starts
    pub async fn update_completion_cache(&mut self) -> Result<()> {
        // Get fresh data
        let prompts = self.agent.list_extension_prompts(&self.session_id).await;

        // Update the cache with write lock
        let mut cache = self.completion_cache.write().unwrap();
        cache.prompts.clear();
        cache.prompt_info.clear();

        for (extension, prompt_list) in prompts {
            let names: Vec<String> = prompt_list.iter().map(|p| p.name.clone()).collect();
            cache.prompts.insert(extension.clone(), names);

            for prompt in prompt_list {
                cache.prompt_info.insert(
                    prompt.name.clone(),
                    output::PromptInfo {
                        name: prompt.name.clone(),
                        description: prompt.description.clone(),
                        arguments: prompt.arguments.clone(),
                        extension: Some(extension.clone()),
                    },
                );
            }
        }

        cache.last_updated = Instant::now();
        Ok(())
    }

    /// Invalidate the completion cache
    /// This should be called when extensions are added or removed
    async fn invalidate_completion_cache(&self) {
        let mut cache = self.completion_cache.write().unwrap();
        cache.prompts.clear();
        cache.prompt_info.clear();
        cache.last_updated = Instant::now();
    }

    pub fn message_history(&self) -> Conversation {
        self.messages.clone()
    }

    /// Render all past messages from the session history
    pub fn render_message_history(&self) {
        if self.messages.is_empty() {
            return;
        }

        println!(
            "\n  {} {}",
            console::style("↻").cyan(),
            console::style(format!("{} messages restored", self.messages.len())).dim()
        );

        // Render each message
        for message in self.messages.iter() {
            output::render_message(message, self.debug);
            println!();
        }

        println!();
    }

    pub async fn get_session(&self) -> Result<gosling::session::Session> {
        self.agent
            .config
            .session_manager
            .get_session(&self.session_id, false)
            .await
    }

    pub async fn get_total_token_usage(&self) -> Result<Option<i32>> {
        let metadata = self.get_session().await?;
        Ok(metadata.accumulated_usage.total_tokens)
    }

    /// Display enhanced context usage with session totals
    pub async fn display_context_usage(&self) -> Result<()> {
        let provider = self.agent.provider().await?;
        let model_config = self
            .agent
            .model_config_for_session(&self.session_id)
            .await?;
        let context_limit = provider
            .get_context_limit(&model_config)
            .await
            .unwrap_or_else(|_| model_config.context_limit());

        let config = Config::global();
        let show_cost = config
            .get_param::<bool>("GOSLING_CLI_SHOW_COST")
            .unwrap_or(false);

        let provider_name = config
            .get_gosling_provider()
            .unwrap_or_else(|_| "unknown".to_string());

        match self.get_session().await {
            Ok(metadata) => {
                let total_tokens = metadata.usage.total_tokens.unwrap_or(0) as usize;

                output::display_context_usage(total_tokens, context_limit);

                if show_cost {
                    output::display_cost_usage(
                        &provider_name,
                        &model_config.model_name,
                        &metadata.usage,
                    );
                }
            }
            Err(_) => {
                output::display_context_usage(0, context_limit);
            }
        }

        Ok(())
    }

    async fn display_session_status(&self) -> Result<()> {
        let provider = self.agent.provider().await?;
        let model_config = self
            .agent
            .model_config_for_session(&self.session_id)
            .await?;
        let session = self.get_session().await?;

        output::display_session_status(
            provider.get_name(),
            &model_config.model_name,
            &session.gosling_mode.to_string(),
            &session.usage,
            &session.accumulated_usage,
        );
        if let Some(snapshot) = self
            .agent
            .config
            .session_manager
            .plans()
            .snapshot(&self.session_id)
            .await?
        {
            output::render_plan_snapshot(&snapshot, false);
        }
        self.display_context_usage().await
    }

    /// Handle prompt command execution
    async fn handle_prompt_command(&mut self, opts: input::PromptCommandOptions) -> Result<()> {
        // name is required
        if opts.name.is_empty() {
            output::render_error("Prompt name argument is required");
            return Ok(());
        }

        if opts.info {
            match self.get_prompt_info(&opts.name).await? {
                Some(info) => output::render_prompt_info(&info),
                None => output::render_error(&format!("Prompt '{}' not found", opts.name)),
            }
        } else {
            // Convert the arguments HashMap to a Value
            let arguments = serde_json::to_value(opts.arguments)
                .map_err(|e| anyhow::anyhow!("Failed to serialize arguments: {}", e))?;

            match self.get_prompt(&opts.name, arguments).await {
                Ok(messages) => {
                    let start_len = self.messages.len();
                    let mut valid = true;
                    let num_messages = messages.len();
                    for (i, prompt_message) in messages.into_iter().enumerate() {
                        let msg = Message::from(prompt_message);
                        // ensure we get a User - Assistant - User type pattern
                        let expected_role = if i % 2 == 0 {
                            rmcp::model::Role::User
                        } else {
                            rmcp::model::Role::Assistant
                        };

                        if msg.role != expected_role {
                            output::render_error(&format!(
                                "Expected {:?} message at position {}, but found {:?}",
                                expected_role, i, msg.role
                            ));
                            valid = false;
                            // get rid of everything we added to messages
                            self.messages.truncate(start_len);
                            break;
                        }

                        if msg.role == rmcp::model::Role::User {
                            output::render_message(&msg, self.debug);
                        }
                        self.push_message(msg);
                    }

                    if valid {
                        if num_messages > 1 {
                            for i in 0..(num_messages - 1) {
                                let msg = &self.messages.messages()[start_len + i];
                                self.agent
                                    .config
                                    .session_manager
                                    .add_message(&self.session_id, msg)
                                    .await?;
                            }
                        }

                        output::show_thinking();
                        self.process_agent_response(true, CancellationToken::default())
                            .await?;
                        output::hide_thinking();
                    }
                }
                Err(e) => output::render_error(&e.to_string()),
            }
        }

        Ok(())
    }

    fn push_message(&mut self, message: Message) {
        self.messages.push(message);
    }
}

fn message_has_text(message: &Message) -> bool {
    message.content.iter().any(
        |content| matches!(content, MessageContent::Text(text) if !text.text.trim().is_empty()),
    )
}

fn parse_plan_comment(value: &str) -> Result<(u32, u32, String)> {
    let value = value.trim();
    let split_at = value
        .find(char::is_whitespace)
        .ok_or_else(|| anyhow::anyhow!("Usage: /plan-comment <start>-<end> <text>"))?;
    let (range, body) = value.split_at(split_at);
    let body = body.trim();
    let (start, end) = range
        .split_once('-')
        .ok_or_else(|| anyhow::anyhow!("Line range must use <start>-<end>"))?;
    let start = start
        .parse::<u32>()
        .map_err(|_| anyhow::anyhow!("Plan comment start line must be a positive integer"))?;
    let end = end
        .parse::<u32>()
        .map_err(|_| anyhow::anyhow!("Plan comment end line must be a positive integer"))?;
    if start == 0 || end == 0 || start > end {
        anyhow::bail!("Plan comment range must be positive and ordered");
    }
    if body.is_empty() {
        anyhow::bail!("Plan comment text must not be empty");
    }
    Ok((start, end, body.to_string()))
}

fn terminal_error_reason(message: &Message) -> Option<String> {
    message.metadata.terminal_error.clone()
}

fn execution_limit_reason(message: &Message) -> bool {
    let text = message.as_concat_text();
    (message.role == rmcp::model::Role::User && text.contains("has exceeded maximum repetitions"))
        || (message.role == rmcp::model::Role::Assistant
            && text.contains("reached the maximum number of actions"))
}

fn remove_local_turn(conversation: &mut Conversation, message_id: &str) -> bool {
    let Some(index) = conversation
        .messages()
        .iter()
        .position(|message| message.id.as_deref() == Some(message_id))
    else {
        return false;
    };

    while conversation.messages().len() > index {
        conversation.pop();
    }
    true
}

fn is_turn_lease_lost(error: &anyhow::Error) -> bool {
    error.to_string().starts_with("Session turn lease was lost")
}

fn turn_tool_request_ids(conversation: &Conversation, message_id: &str) -> Vec<String> {
    conversation
        .messages()
        .iter()
        .skip_while(|message| message.id.as_deref() != Some(message_id))
        .flat_map(|message| message.content.iter())
        .filter_map(|content| match content {
            MessageContent::ToolRequest(request) => Some(request.id.clone()),
            _ => None,
        })
        .collect()
}

async fn persist_cancelled_tool_response(
    session_manager: &gosling::session::SessionManager,
    session_id: &str,
    request_id: String,
) -> Result<Message> {
    let mut response_message = Message::user().with_generated_id();
    response_message.content.push(MessageContent::tool_response(
        request_id,
        Err(ErrorData {
            code: ErrorCode::INVALID_REQUEST,
            message: std::borrow::Cow::from("Tool call cancelled by user"),
            data: None,
        }),
    ));
    session_manager
        .add_message(session_id, &response_message)
        .await?;
    Ok(response_message)
}

fn print_run_stats(
    run_started: Instant,
    first_token_at: Option<Instant>,
    usage: Option<&ProviderUsage>,
) {
    let elapsed = run_started.elapsed();
    let output_tokens = usage
        .and_then(|usage| usage.usage.output_tokens)
        .and_then(|tokens| usize::try_from(tokens).ok())
        .or_else(|| usage.and_then(|usage| usage.stats.as_ref()?.output_tokens));
    let tokens_per_second = output_tokens.map(|tokens| {
        if elapsed.as_secs_f64() > 0.0 {
            tokens as f64 / elapsed.as_secs_f64()
        } else {
            0.0
        }
    });

    eprintln!("\nStats:");
    match first_token_at {
        Some(first) => eprintln!(
            "  Time to first token: {:.2}s",
            first.duration_since(run_started).as_secs_f64()
        ),
        None => eprintln!("  Time to first token: unavailable"),
    }
    match tokens_per_second {
        Some(rate) => eprintln!("  Tokens/sec: {:.2}", rate),
        None => eprintln!("  Tokens/sec: unavailable"),
    }
    if let Some(tokens) = output_tokens {
        eprintln!("  Output tokens: {tokens}");
    }

    if let Some(draft) = usage
        .and_then(|usage| usage.stats.as_ref())
        .and_then(|stats| stats.draft.as_ref())
    {
        eprintln!("  Draft accept rate: {:.1}%", draft.accept_rate * 100.0);
        eprintln!(
            "  Draft tokens: {} accepted: {} target verified: {} rounds: {}",
            draft.draft_tokens, draft.accepted_tokens, draft.target_tokens, draft.rounds
        );
        if let Some(model) = &draft.model {
            eprintln!("  Draft model: {model}");
        }
    }
}

fn maybe_open_credits_top_up_url(
    message: &Message,
    interactive: bool,
    prompted_credits_urls: &mut HashSet<String>,
) {
    if !interactive || !std::io::stdout().is_terminal() {
        return;
    }

    let Some(url) = output::get_credits_top_up_url(message) else {
        return;
    };

    if !prompted_credits_urls.insert(url.clone()) {
        return;
    }

    let should_open = cliclack::confirm("Open the top-up URL in your browser?")
        .initial_value(false)
        .interact()
        .unwrap_or(false);

    if should_open && webbrowser::open(&url).is_err() {
        output::render_text(
            "Could not open browser automatically. Visit the URL above.",
            Some(Color::Yellow),
            true,
        );
    }
}

fn emit_stream_event(event: &StreamEvent) {
    if let Ok(json) = serde_json::to_string(event) {
        println!("{}", json);
    }
}

// Enter on an untouched menu, or stray typing meant for the chat prompt, must never approve a
// tool. Cancel is the last item, so arrow-style keys (h/j/k/l) typed by accident reach Deny
// before any Allow option.
const TOOL_CONFIRMATION_DEFAULT: Permission = Permission::Cancel;

/// Prompt user for tool call confirmation, returns the Permission selected
fn prompt_tool_confirmation(security_prompt: &Option<String>) -> Result<Permission> {
    output::hide_thinking();

    let prompt = if let Some(security_message) = security_prompt {
        println!("\n{}", security_message);
        "Do you allow this tool call?".to_string()
    } else {
        "Gosling would like to call the above tool, do you allow?".to_string()
    };

    let permission_result = if security_prompt.is_none() {
        cliclack::select(prompt)
            .item(Permission::AllowOnce, "Allow", "Allow the tool call once")
            .item(
                Permission::AlwaysAllow,
                "Always Allow",
                "Always allow the tool call",
            )
            .item(Permission::DenyOnce, "Deny", "Deny the tool call")
            .item(
                Permission::Cancel,
                "Cancel",
                "Cancel the AI response and tool call",
            )
            .initial_value(TOOL_CONFIRMATION_DEFAULT)
            .interact()
    } else {
        cliclack::select(prompt)
            .item(Permission::AllowOnce, "Allow", "Allow the tool call once")
            .item(Permission::DenyOnce, "Deny", "Deny the tool call")
            .item(
                Permission::Cancel,
                "Cancel",
                "Cancel the AI response and tool call",
            )
            .initial_value(TOOL_CONFIRMATION_DEFAULT)
            .interact()
    };

    match permission_result {
        Ok(p) => Ok(p),
        Err(e) => {
            if e.kind() == std::io::ErrorKind::Interrupted {
                Ok(Permission::Cancel)
            } else {
                Err(e.into())
            }
        }
    }
}

fn non_interactive_confirmation_permission() -> Permission {
    Permission::DenyOnce
}

/// Extract tool confirmation request from a message
fn find_tool_confirmation(message: &Message) -> Option<(String, Option<String>)> {
    message.content.iter().find_map(|content| {
        if let MessageContent::ActionRequired(action) = content {
            if let ActionRequiredData::ToolConfirmation { id, prompt, .. } = &action.data {
                return Some((id.clone(), prompt.clone()));
            }
        }
        None
    })
}

/// Extract elicitation request from a message
fn find_elicitation_request(message: &Message) -> Option<(String, String, Value)> {
    message.content.iter().find_map(|content| {
        if let MessageContent::ActionRequired(action) = content {
            if let ActionRequiredData::Elicitation {
                id,
                message,
                requested_schema,
            } = &action.data
            {
                return Some((id.clone(), message.clone(), requested_schema.clone()));
            }
        }
        None
    })
}

/// Handle MCP notification event (logging or progress)
fn handle_mcp_notification(
    extension_id: &str,
    notification: &ServerNotification,
    progress_bars: &mut output::McpSpinners,
    is_stream_json_mode: bool,
    interactive: bool,
    is_json_mode: bool,
    debug: bool,
) {
    match notification {
        ServerNotification::LoggingMessageNotification(log_notif) => {
            if let Some(obj) = log_notif.params.data.as_object() {
                if obj.get("type").and_then(|v| v.as_str()) == Some(SUBAGENT_TOOL_REQUEST_TYPE) {
                    if let (Some(subagent_id), Some(tool_call)) = (
                        obj.get("subagent_id").and_then(|v| v.as_str()),
                        obj.get("tool_call").and_then(|v| v.as_object()),
                    ) {
                        let tool_name = tool_call
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        let arguments = tool_call
                            .get("arguments")
                            .and_then(|v| v.as_object())
                            .cloned();

                        if interactive {
                            let _ = progress_bars.hide();
                        }
                        if is_stream_json_mode {
                            emit_stream_event(&StreamEvent::Notification {
                                extension_id: extension_id.to_string(),
                                data: NotificationData::Log {
                                    message: output::format_subagent_tool_call_message(
                                        subagent_id,
                                        tool_name,
                                    ),
                                },
                            });
                            return;
                        }
                        if !is_json_mode {
                            output::render_subagent_tool_call(
                                subagent_id,
                                tool_name,
                                arguments.as_ref(),
                                debug,
                            );
                            return;
                        }
                    }
                }
            }

            let (formatted, subagent_id, notif_type) =
                format_logging_notification(&log_notif.params.data, debug);

            if is_stream_json_mode {
                emit_stream_event(&StreamEvent::Notification {
                    extension_id: extension_id.to_string(),
                    data: NotificationData::Log {
                        message: formatted.clone(),
                    },
                });
            } else {
                display_log_notification(
                    &formatted,
                    subagent_id.as_deref(),
                    notif_type.as_deref(),
                    progress_bars,
                    interactive,
                    is_json_mode,
                );
            }
        }
        ServerNotification::ProgressNotification(prog_notif) => {
            if is_stream_json_mode {
                emit_stream_event(&StreamEvent::Notification {
                    extension_id: extension_id.to_string(),
                    data: NotificationData::Progress {
                        progress: prog_notif.params.progress,
                        total: prog_notif.params.total,
                        message: prog_notif.params.message.clone(),
                    },
                });
            } else {
                progress_bars.update(
                    &prog_notif.params.progress_token.0.to_string(),
                    prog_notif.params.progress,
                    prog_notif.params.total,
                    prog_notif.params.message.as_deref(),
                );
            }
        }
        _ => (),
    }
}

/// Format a logging notification from MCP, returns (formatted_message, subagent_id, notification_type)
fn format_logging_notification(
    data: &Value,
    debug: bool,
) -> (String, Option<String>, Option<String>) {
    match data {
        Value::String(s) => (s.clone(), None, None),
        Value::Object(o) => {
            if let Some(Value::String(msg)) = o.get("message") {
                let subagent_id = o.get("subagent_id").and_then(|v| v.as_str());
                let notification_type = o.get("type").and_then(|v| v.as_str());

                let formatted = match notification_type {
                    Some("subagent_created") | Some("completed") | Some("terminated") => {
                        format!("🤖 {}", msg)
                    }
                    Some("tool_usage") | Some("tool_completed") | Some("tool_error") => {
                        format!("🔧 {}", msg)
                    }
                    Some("message_processing") | Some("turn_progress") => {
                        format!("💭 {}", msg)
                    }
                    Some("response_generated") => {
                        let config = Config::global();
                        let min_priority = config
                            .get_param::<f32>("GOSLING_CLI_MIN_PRIORITY")
                            .ok()
                            .unwrap_or(output::DEFAULT_MIN_PRIORITY);

                        if min_priority > 0.1 && !debug {
                            if let Some(response_content) = msg.strip_prefix("Responded: ") {
                                format!("🤖 Responded: {}", safe_truncate(response_content, 100))
                            } else {
                                format!("🤖 {}", msg)
                            }
                        } else {
                            format!("🤖 {}", msg)
                        }
                    }
                    _ => msg.to_string(),
                };
                (
                    formatted,
                    subagent_id.map(str::to_string),
                    notification_type.map(str::to_string),
                )
            } else if let Some(Value::String(output)) = o.get("output") {
                let notification_type = o.get("type").and_then(|v| v.as_str()).map(str::to_string);
                (output.to_owned(), None, notification_type)
            } else {
                (data.to_string(), None, None)
            }
        }
        v => (v.to_string(), None, None),
    }
}

/// Display a logging notification based on its type and context
fn display_log_notification(
    formatted_message: &str,
    subagent_id: Option<&str>,
    notification_type: Option<&str>,
    progress_bars: &mut output::McpSpinners,
    interactive: bool,
    is_json_mode: bool,
) {
    if subagent_id.is_some() {
        if interactive {
            let _ = progress_bars.hide();
            if !is_json_mode {
                println!("{}", console::style(formatted_message).green().dim());
            }
        } else if !is_json_mode {
            progress_bars.log(formatted_message);
        }
    } else if let Some(ntype) = notification_type {
        if ntype == "shell_output" {
            let config = Config::global();
            let min_priority = config
                .get_param::<f32>("GOSLING_CLI_MIN_PRIORITY")
                .ok()
                .unwrap_or(output::DEFAULT_MIN_PRIORITY);

            if min_priority < 0.1 {
                if interactive {
                    let _ = progress_bars.hide();
                }
                if !is_json_mode {
                    println!("    {}", console::style(formatted_message).dim());
                }
            }
        }
    } else if output::is_showing_thinking() {
        output::set_thinking_message(&formatted_message.to_string());
    } else {
        progress_bars.log(formatted_message);
    }
}

/// Log tool request/response metrics
fn log_tool_metrics(message: &Message, messages: &Conversation) {
    for content in &message.content {
        if let MessageContent::ToolRequest(tool_request) = content {
            if let Ok(tool_call) = &tool_request.tool_call {
                tracing::info!(
                    monotonic_counter.gosling.tool_calls = 1,
                    tool_name = %tool_call.name,
                    "Tool call started"
                );
            }
        }
        if let MessageContent::ToolResponse(tool_response) = content {
            let tool_name = messages
                .iter()
                .rev()
                .find_map(|msg| {
                    msg.content.iter().find_map(|c| {
                        if let MessageContent::ToolRequest(req) = c {
                            if req.id == tool_response.id {
                                req.tool_call.as_ref().ok().map(|tc| tc.name.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| "unknown".to_string().into());

            let result_status = if tool_response.tool_result.is_ok() {
                "success"
            } else {
                "error"
            };
            tracing::info!(
                monotonic_counter.gosling.tool_completions = 1,
                tool_name = %tool_name,
                result = %result_status,
                "Tool call completed"
            );
        }
    }
}

/// Handle and display an agent error
fn handle_agent_error(e: &anyhow::Error, is_json_mode: bool, is_stream_json_mode: bool) {
    let error_msg = e.to_string();

    if is_stream_json_mode {
        emit_stream_event(&StreamEvent::Error {
            error: error_msg.clone(),
        });
    }

    if e.downcast_ref::<gosling_providers::errors::ProviderError>()
        .map(|provider_error| {
            matches!(
                provider_error,
                gosling_providers::errors::ProviderError::ContextLengthExceeded(_)
            )
        })
        .unwrap_or(false)
    {
        if !is_json_mode && !is_stream_json_mode {
            output::render_text(
                "Compaction requested. Should have happened in the agent!",
                Some(Color::Yellow),
                true,
            );
        }
        warn!("Compaction requested. Should have happened in the agent!");
    }

    if !is_stream_json_mode {
        eprintln!("Error: {}", error_msg);
    }
}

fn report_plan_implementation_submission(
    approved: &PlanSnapshot,
    submission: Result<()>,
) -> PlanImplementationSubmission {
    match submission {
        Ok(()) => PlanImplementationSubmission::Started,
        Err(error) => {
            let error = error.to_string();
            output::render_plan_implementation_partial_success(approved, &error);
            PlanImplementationSubmission::Failed(error)
        }
    }
}

/// Resolve legacy planner-selection settings without reintroducing the old
/// second provider loop. The host-enforced path currently supports only the
/// session's active provider/model/context; distinct planner settings fail
/// explicitly before PlanService creates or resumes a generation.
fn resolve_cli_planner_model(
    current_provider: &str,
    current_model: &gosling_providers::model::ModelConfig,
    config: &Config,
) -> Result<String> {
    let configured_provider = config.get_param::<String>(GOSLING_PLANNER_PROVIDER).ok();
    if let Some(provider) = configured_provider.as_deref() {
        if provider.trim().is_empty() {
            anyhow::bail!("{GOSLING_PLANNER_PROVIDER} must not be empty");
        }
        if provider != current_provider {
            anyhow::bail!(
                "Configured separate planner provider '{provider}' is not yet supported by host-enforced planning; use the session provider '{current_provider}'"
            );
        }
    }

    let configured_model = config.get_param::<String>(GOSLING_PLANNER_MODEL).ok();
    if let Some(model) = configured_model.as_deref() {
        if model.trim().is_empty() {
            anyhow::bail!("{GOSLING_PLANNER_MODEL} must not be empty");
        }
        let selected =
            gosling::model_config::model_config_from_user_config(current_provider, model)?;
        if selected.model_name != current_model.model_name
            || selected.thinking_effort() != current_model.thinking_effort()
        {
            anyhow::bail!(
                "Configured separate planner model '{model}' is not yet supported by host-enforced planning; use the session model '{}'",
                current_model.model_name
            );
        }
    }

    let configured_context_limit = match env::var(GOSLING_PLANNER_CONTEXT_LIMIT) {
        Ok(value) => {
            let limit = value
                .parse::<usize>()
                .map_err(|error| anyhow::anyhow!("{GOSLING_PLANNER_CONTEXT_LIMIT}: {error}"))?;
            if limit < 4096 {
                anyhow::bail!("{GOSLING_PLANNER_CONTEXT_LIMIT} must be at least 4096");
            }
            Some(limit)
        }
        Err(env::VarError::NotPresent) => None,
        Err(error) => anyhow::bail!("{GOSLING_PLANNER_CONTEXT_LIMIT}: {error}"),
    };
    if let Some(limit) = configured_context_limit {
        let session_limit = current_model.context_limit();
        if limit != session_limit {
            anyhow::bail!(
                "Configured separate planner context limit {limit} is not yet supported by host-enforced planning; use the session context limit {session_limit}"
            );
        }
    }

    Ok(current_model.model_name.clone())
}

/// Format elapsed time duration
/// Shows seconds if less than 60, otherwise shows minutes:seconds
fn format_elapsed_time(duration: std::time::Duration) -> String {
    let total_secs = duration.as_secs();
    if total_secs < 60 {
        format!("{:.2}s", duration.as_secs_f64())
    } else {
        let minutes = total_secs / 60;
        let seconds = total_secs % 60;
        format!("{}m {:02}s", minutes, seconds)
    }
}

fn build_switched_model_config(
    provider_name: &str,
    model_name: &str,
    current_model_config: &gosling_providers::model::ModelConfig,
) -> Result<gosling_providers::model::ModelConfig> {
    gosling::model_config::model_config_from_user_config(provider_name, model_name)
        .map(|config| {
            config
                .with_temperature(current_model_config.temperature)
                .with_toolshim(current_model_config.toolshim)
                .with_toolshim_model(current_model_config.toolshim_model.clone())
        })
        .map_err(|e| anyhow::anyhow!("Failed to create model configuration: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use gosling::agents::extension::Envs;
    use gosling::config::ExtensionConfig;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use std::time::Duration;
    use test_case::test_case;

    #[test]
    fn test_format_elapsed_time_under_60_seconds() {
        // Test sub-second duration
        let duration = Duration::from_millis(500);
        assert_eq!(format_elapsed_time(duration), "0.50s");

        // Test exactly 1 second
        let duration = Duration::from_secs(1);
        assert_eq!(format_elapsed_time(duration), "1.00s");

        // Test 45.75 seconds
        let duration = Duration::from_millis(45750);
        assert_eq!(format_elapsed_time(duration), "45.75s");

        // Test 59.99 seconds
        let duration = Duration::from_millis(59990);
        assert_eq!(format_elapsed_time(duration), "59.99s");
    }

    #[test]
    fn test_format_elapsed_time_minutes() {
        // Test exactly 60 seconds (1 minute)
        let duration = Duration::from_secs(60);
        assert_eq!(format_elapsed_time(duration), "1m 00s");

        // Test 61 seconds (1 minute 1 second)
        let duration = Duration::from_secs(61);
        assert_eq!(format_elapsed_time(duration), "1m 01s");

        // Test 90 seconds (1 minute 30 seconds)
        let duration = Duration::from_secs(90);
        assert_eq!(format_elapsed_time(duration), "1m 30s");

        // Test 119 seconds (1 minute 59 seconds)
        let duration = Duration::from_secs(119);
        assert_eq!(format_elapsed_time(duration), "1m 59s");

        // Test 120 seconds (2 minutes)
        let duration = Duration::from_secs(120);
        assert_eq!(format_elapsed_time(duration), "2m 00s");

        // Test 605 seconds (10 minutes 5 seconds)
        let duration = Duration::from_secs(605);
        assert_eq!(format_elapsed_time(duration), "10m 05s");

        // Test 3661 seconds (61 minutes 1 second)
        let duration = Duration::from_secs(3661);
        assert_eq!(format_elapsed_time(duration), "61m 01s");
    }

    #[test]
    fn test_format_elapsed_time_edge_cases() {
        // Test zero duration
        let duration = Duration::from_secs(0);
        assert_eq!(format_elapsed_time(duration), "0.00s");

        // Test very small duration (1 millisecond)
        let duration = Duration::from_millis(1);
        assert_eq!(format_elapsed_time(duration), "0.00s");

        // Test fractional seconds are truncated for minute display
        // 60.5 seconds should still show as 1m 00s (not 1m 00.5s)
        let duration = Duration::from_millis(60500);
        assert_eq!(format_elapsed_time(duration), "1m 00s");
    }

    #[test_case(
        "/usr/bin/my-server",
        ExtensionConfig::Stdio {
            name: "my-server".into(),
            cmd: "/usr/bin/my-server".into(),
            args: vec![],
            envs: Envs::default(),
            env_keys: vec![],
            description: gosling::config::DEFAULT_EXTENSION_DESCRIPTION.to_string(),
            timeout: Some(gosling::config::DEFAULT_EXTENSION_TIMEOUT),
            cwd: None,
            bundled: None,
            available_tools: vec![],
        }
        ; "name_from_cmd_basename"
    )]
    #[test_case(
        "MY_SECRET=s3cret npx -y @modelcontextprotocol/server-everything",
        ExtensionConfig::Stdio {
            name: "npx".into(),
            cmd: "npx".into(),
            args: vec!["-y".into(), "@modelcontextprotocol/server-everything".into()],
            envs: Envs::new([("MY_SECRET".into(), "s3cret".into())].into()),
            env_keys: vec![],
            description: gosling::config::DEFAULT_EXTENSION_DESCRIPTION.to_string(),
            timeout: Some(gosling::config::DEFAULT_EXTENSION_TIMEOUT),
            cwd: None,
            bundled: None,
            available_tools: vec![],
        }
        ; "env_prefix_name_from_cmd"
    )]
    #[test_case(
        r#""/Applications/IntelliJ IDEA.app/Contents/jbr/Contents/Home/bin/java" -classpath "/path/with spaces/lib.jar" Main"#,
        ExtensionConfig::Stdio {
            name: "java".into(),
            cmd: "/Applications/IntelliJ IDEA.app/Contents/jbr/Contents/Home/bin/java".into(),
            args: vec!["-classpath".into(), "/path/with spaces/lib.jar".into(), "Main".into()],
            envs: Envs::default(),
            env_keys: vec![],
            description: gosling::config::DEFAULT_EXTENSION_DESCRIPTION.to_string(),
            timeout: Some(gosling::config::DEFAULT_EXTENSION_TIMEOUT),
            cwd: None,
            bundled: None,
            available_tools: vec![],
        }
        ; "quoted_path_with_spaces"
    )]
    fn test_parse_stdio_extension(input: &str, expected: ExtensionConfig) {
        assert_eq!(CliSession::parse_stdio_extension(input).unwrap(), expected);
    }

    #[test]
    fn test_parse_stdio_extension_no_command() {
        assert!(CliSession::parse_stdio_extension("").is_err());
    }

    #[test]
    fn test_build_switched_model_config_rebuilds_target_model_settings() {
        let _guard = env_lock::lock_env([
            ("GOSLING_MAX_TOKENS", None::<&str>),
            ("GOSLING_TEMPERATURE", None::<&str>),
            ("GOSLING_CONTEXT_LIMIT", None::<&str>),
            ("GOSLING_TOOLSHIM", None::<&str>),
            ("GOSLING_TOOLSHIM_OLLAMA_MODEL", None::<&str>),
        ]);

        let current_model_config = gosling_providers::model::ModelConfig {
            model_name: "gpt-4o".to_string(),
            context_limit: Some(128_000),
            temperature: Some(0.25),
            max_tokens: Some(16_384),
            toolshim: true,
            toolshim_model: Some("qwen2.5-coder".to_string()),
            request_params: Some(HashMap::from([(
                "anthropic_beta".to_string(),
                serde_json::json!(["output-128k-2025-02-19"]),
            )])),
            reasoning: Some(false),
        };

        let switched =
            build_switched_model_config("openai", "gpt-5.4", &current_model_config).unwrap();
        let expected = gosling_providers::model::ModelConfig::new("gpt-5.4")
            .with_canonical_limits("openai")
            .with_temperature(Some(0.25))
            .with_toolshim(true)
            .with_toolshim_model(Some("qwen2.5-coder".to_string()));

        assert_eq!(switched.model_name, expected.model_name);
        assert_eq!(switched.context_limit, expected.context_limit);
        assert_eq!(switched.max_tokens, expected.max_tokens);
        assert_eq!(switched.request_params, expected.request_params);
        assert_eq!(switched.reasoning, expected.reasoning);
        assert_eq!(switched.temperature, Some(0.25));
        assert!(switched.toolshim);
        assert_eq!(switched.toolshim_model.as_deref(), Some("qwen2.5-coder"));
    }

    #[test]
    fn test_build_switched_model_config_detects_effort_suffix_change() {
        let _guard = env_lock::lock_env([
            ("GOSLING_MAX_TOKENS", None::<&str>),
            ("GOSLING_TEMPERATURE", None::<&str>),
            ("GOSLING_CONTEXT_LIMIT", None::<&str>),
            ("GOSLING_TOOLSHIM", None::<&str>),
            ("GOSLING_TOOLSHIM_OLLAMA_MODEL", None::<&str>),
            ("GOSLING_THINKING_EFFORT", None::<&str>),
        ]);

        let current = gosling_providers::model::ModelConfig::new("gpt-5.4-high")
            .with_canonical_limits("openai");
        assert_eq!(current.model_name, "gpt-5.4");
        assert_eq!(
            current.thinking_effort(),
            Some(gosling_providers::thinking::ThinkingEffort::High)
        );

        let switched = build_switched_model_config("openai", "gpt-5.4", &current).unwrap();

        assert_eq!(switched.model_name, current.model_name);
        assert_ne!(switched.thinking_effort(), current.thinking_effort());
    }

    #[test]
    fn planner_compatibility_accepts_only_the_active_session_selection() {
        let _guard = env_lock::lock_env([
            (GOSLING_PLANNER_PROVIDER, Some("planning-test")),
            (GOSLING_PLANNER_MODEL, Some("planner-model")),
            (GOSLING_PLANNER_CONTEXT_LIMIT, Some("8192")),
        ]);
        let current =
            gosling::model_config::model_config_from_user_config("planning-test", "planner-model")
                .unwrap()
                .with_context_limit(Some(8192));

        assert_eq!(
            resolve_cli_planner_model("planning-test", &current, Config::global()).unwrap(),
            "planner-model"
        );
    }

    #[test]
    fn planner_compatibility_rejects_a_distinct_provider_before_plan_start() {
        let _guard = env_lock::lock_env([
            (GOSLING_PLANNER_PROVIDER, Some("other-provider")),
            (GOSLING_PLANNER_MODEL, Some("planner-model")),
            (GOSLING_PLANNER_CONTEXT_LIMIT, None::<&str>),
        ]);
        let current =
            gosling::model_config::model_config_from_user_config("planning-test", "planner-model")
                .unwrap();

        let error =
            resolve_cli_planner_model("planning-test", &current, Config::global()).unwrap_err();

        assert!(error
            .to_string()
            .contains("Configured separate planner provider"));
    }

    #[test]
    fn planner_compatibility_rejects_a_distinct_model_before_plan_start() {
        let _guard = env_lock::lock_env([
            (GOSLING_PLANNER_PROVIDER, Some("planning-test")),
            (GOSLING_PLANNER_MODEL, Some("other-model")),
            (GOSLING_PLANNER_CONTEXT_LIMIT, None::<&str>),
        ]);
        let current = gosling_providers::model::ModelConfig::new("planner-model");

        let error =
            resolve_cli_planner_model("planning-test", &current, Config::global()).unwrap_err();

        assert!(error
            .to_string()
            .contains("Configured separate planner model"));
    }

    #[test]
    fn planner_compatibility_rejects_a_distinct_context_limit_before_plan_start() {
        let _guard = env_lock::lock_env([
            (GOSLING_PLANNER_PROVIDER, Some("planning-test")),
            (GOSLING_PLANNER_MODEL, Some("planner-model")),
            (GOSLING_PLANNER_CONTEXT_LIMIT, Some("4096")),
        ]);
        let current =
            gosling::model_config::model_config_from_user_config("planning-test", "planner-model")
                .unwrap()
                .with_context_limit(Some(8192));

        let error =
            resolve_cli_planner_model("planning-test", &current, Config::global()).unwrap_err();

        assert!(error
            .to_string()
            .contains("Configured separate planner context limit"));
    }

    #[test]
    fn plan_comment_parser_binds_a_positive_ordered_line_range() {
        assert_eq!(
            parse_plan_comment("2-4 preserve rollback steps").unwrap(),
            (2, 4, "preserve rollback steps".to_string())
        );
        for invalid in [
            "",
            "2-4",
            "0-2 text",
            "4-2 text",
            "two-4 text",
            "2-four text",
            "2:4 text",
        ] {
            assert!(parse_plan_comment(invalid).is_err(), "accepted {invalid:?}");
        }
    }

    #[test]
    fn test_split_command_args_windows_paths() {
        assert_eq!(
            gosling::utils::split_command_args(r"C:\tools\mcp.exe --arg value").unwrap(),
            vec![r"C:\tools\mcp.exe", "--arg", "value"]
        );
        assert_eq!(
            gosling::utils::split_command_args(r#""C:\Program Files\server\mcp.exe" --arg"#)
                .unwrap(),
            vec![r"C:\Program Files\server\mcp.exe", "--arg"]
        );
    }

    #[test]
    fn test_split_command_args_unmatched_quote() {
        assert!(gosling::utils::split_command_args(r#""unmatched"#).is_err());
    }

    #[test]
    fn execution_limit_reason_detects_repetition_and_turn_budgets() {
        assert!(execution_limit_reason(
            &Message::user().with_text("Tool 'shell' has exceeded maximum repetitions")
        ));
        assert!(execution_limit_reason(&Message::assistant().with_text(
            "I've reached the maximum number of actions I can do without user input."
        )));
        assert!(!execution_limit_reason(&Message::assistant().with_text(
            "The phrase 'has exceeded maximum repetitions' may appear in documentation."
        )));
        assert!(!execution_limit_reason(
            &Message::assistant().with_text("All requested work completed")
        ));
    }

    #[test]
    fn terminal_error_reason_requires_explicit_message_metadata() {
        let marked = Message::assistant()
            .with_text("Request failed")
            .with_terminal_error("provider unavailable");
        let unmarked = Message::assistant().with_text("provider unavailable");

        assert_eq!(
            terminal_error_reason(&marked).as_deref(),
            Some("provider unavailable")
        );
        assert_eq!(terminal_error_reason(&unmarked), None);
    }

    #[tokio::test]
    async fn cancelled_empty_reply_is_not_successful_in_machine_output() {
        for output_format in ["json", "stream-json"] {
            for _ in 0..16 {
                let temp = tempfile::tempdir().unwrap();
                let manager = Arc::new(gosling::session::SessionManager::new(temp.path().into()));
                let agent = Agent::with_config(gosling::agents::AgentConfig::new(
                    manager.clone(),
                    Arc::new(gosling::config::PermissionManager::new(temp.path().into())),
                    GoslingMode::Auto,
                    true,
                    gosling::agents::GoslingPlatform::GoslingCli,
                ));
                let session = manager
                    .create_session(
                        temp.path().into(),
                        "Cancelled reply".into(),
                        gosling::session::SessionType::Hidden,
                        GoslingMode::Auto,
                    )
                    .await
                    .unwrap();
                let mut cli = CliSession::new(
                    agent,
                    session.id.clone(),
                    false,
                    None,
                    None,
                    output_format.into(),
                    false,
                )
                .await;
                cli.messages
                    .push(Message::user().with_text("/compact").with_generated_id());
                let cancel = CancellationToken::new();
                cancel.cancel();

                let error = cli
                    .process_agent_response(false, cancel)
                    .await
                    .expect_err("cancelled EOF must not report completion");
                assert_eq!(error.to_string(), "Run cancelled by user");
                let stored = manager.get_session(&session.id, true).await.unwrap();
                let notices = stored
                    .conversation
                    .unwrap()
                    .messages()
                    .iter()
                    .filter(|message| {
                        message.as_concat_text() == "Run cancelled by user before completion."
                    })
                    .count();
                assert_eq!(notices, 1, "cancellation cleanup must run exactly once");
            }
        }
    }

    #[tokio::test]
    async fn uncancelled_command_reply_preserves_machine_output_success() {
        let temp = tempfile::tempdir().unwrap();
        let manager = Arc::new(gosling::session::SessionManager::new(temp.path().into()));
        let agent = Agent::with_config(gosling::agents::AgentConfig::new(
            manager.clone(),
            Arc::new(gosling::config::PermissionManager::new(temp.path().into())),
            GoslingMode::Auto,
            true,
            gosling::agents::GoslingPlatform::GoslingCli,
        ));
        let session = manager
            .create_session(
                temp.path().into(),
                "Completed reply".into(),
                gosling::session::SessionType::Hidden,
                GoslingMode::Auto,
            )
            .await
            .unwrap();
        let mut cli = CliSession::new(
            agent,
            session.id.clone(),
            false,
            None,
            None,
            "json".into(),
            false,
        )
        .await;
        cli.messages
            .push(Message::user().with_text("/clear").with_generated_id());
        cli.process_agent_response(false, CancellationToken::new())
            .await
            .unwrap();
        let stored = manager.get_session(&session.id, true).await.unwrap();
        assert!(stored
            .conversation
            .unwrap()
            .messages()
            .iter()
            .any(|message| message.as_concat_text() == "Conversation cleared"));
    }

    #[test]
    fn turn_lease_loss_is_distinguished_from_other_turn_errors() {
        assert!(is_turn_lease_lost(&anyhow::anyhow!(
            "Session turn lease was lost; this turn stopped before completion. Reload the session before retrying."
        )));
        assert!(!is_turn_lease_lost(&anyhow::anyhow!(
            "Request failed: connection refused"
        )));
    }

    #[test]
    fn remove_local_turn_removes_only_the_interrupted_suffix() {
        let before = Message::assistant().with_text("before").with_id("before");
        let interrupted = Message::user()
            .with_text("long request")
            .with_id("interrupted");
        let partial = Message::assistant().with_text("partial").with_id("partial");
        let mut conversation =
            Conversation::new_unvalidated(vec![before.clone(), interrupted, partial]);

        assert!(remove_local_turn(&mut conversation, "interrupted"));
        assert_eq!(conversation.messages(), std::slice::from_ref(&before));
        assert!(!remove_local_turn(&mut conversation, "missing"));
        assert_eq!(conversation.messages(), &[before]);
    }

    #[tokio::test]
    async fn cancelled_tool_response_is_persisted_before_the_next_turn() {
        let temp_dir = tempfile::tempdir().unwrap();
        let session_manager = gosling::session::SessionManager::new(temp_dir.path().to_path_buf());
        let session = session_manager
            .create_session(
                temp_dir.path().to_path_buf(),
                "Cancelled tool".to_string(),
                gosling::session::SessionType::User,
                GoslingMode::Approve,
            )
            .await
            .unwrap();
        let request_id = "cancelled-request";
        session_manager
            .add_message(
                &session.id,
                &Message::assistant().with_generated_id().with_tool_request(
                    request_id,
                    Ok(rmcp::model::CallToolRequestParams::new("write")),
                ),
            )
            .await
            .unwrap();

        let response =
            persist_cancelled_tool_response(&session_manager, &session.id, request_id.to_string())
                .await
                .unwrap();

        assert!(response.id.is_some());
        let reloaded = session_manager
            .get_session(&session.id, true)
            .await
            .unwrap();
        let conversation = reloaded.conversation.unwrap();
        let responses = conversation
            .messages()
            .iter()
            .flat_map(|message| message.content.iter())
            .filter_map(MessageContent::as_tool_response)
            .filter(|tool_response| tool_response.id == request_id)
            .collect::<Vec<_>>();
        assert_eq!(responses.len(), 1);
        let error = responses[0]
            .tool_result
            .as_ref()
            .expect_err("cancelled tool response should be an error");
        assert!(error.message.contains("Tool call cancelled by user"));
    }

    async fn cli_session_with_messages(
        temp: &tempfile::TempDir,
        turn: &[Message],
    ) -> (CliSession, Arc<gosling::session::SessionManager>, String) {
        let manager = Arc::new(gosling::session::SessionManager::new(temp.path().into()));
        let agent = Agent::with_config(gosling::agents::AgentConfig::new(
            manager.clone(),
            Arc::new(gosling::config::PermissionManager::new(temp.path().into())),
            GoslingMode::Auto,
            true,
            gosling::agents::GoslingPlatform::GoslingCli,
        ));
        let session = manager
            .create_session(
                temp.path().into(),
                "Interrupted turn".into(),
                gosling::session::SessionType::User,
                GoslingMode::Auto,
            )
            .await
            .unwrap();
        let mut cli = CliSession::new(
            agent,
            session.id.clone(),
            false,
            None,
            None,
            "text".into(),
            false,
        )
        .await;
        for message in turn {
            manager.add_message(&session.id, message).await.unwrap();
            cli.messages.push(message.clone());
        }
        (cli, manager, session.id)
    }

    struct PlanningTestProvider {
        stream_calls: Arc<AtomicUsize>,
        fail_stream: bool,
        external_tools: AtomicBool,
    }

    impl PlanningTestProvider {
        fn new(fail_stream: bool, external_tools: bool) -> Self {
            Self {
                stream_calls: Arc::new(AtomicUsize::new(0)),
                fail_stream,
                external_tools: AtomicBool::new(external_tools),
            }
        }

        fn set_external_tools(&self, external_tools: bool) {
            self.external_tools.store(external_tools, Ordering::SeqCst);
        }
    }

    #[async_trait::async_trait]
    impl gosling::providers::base::Provider for PlanningTestProvider {
        fn get_name(&self) -> &str {
            "planning-test"
        }

        async fn stream(
            &self,
            _model_config: &gosling_providers::model::ModelConfig,
            _system: &str,
            _messages: &[Message],
            _tools: &[rmcp::model::Tool],
        ) -> Result<gosling_providers::base::MessageStream, gosling_providers::errors::ProviderError>
        {
            self.stream_calls.fetch_add(1, Ordering::SeqCst);
            if self.fail_stream {
                return Err(gosling_providers::errors::ProviderError::RequestFailed(
                    "planner test failure".to_string(),
                ));
            }
            let message = Message::assistant().with_text("implementation response");
            let usage = gosling_providers::conversation::token_usage::ProviderUsage::new(
                "planning-test".to_string(),
                gosling_providers::conversation::token_usage::Usage::default(),
            );
            Ok(gosling::providers::base::stream_from_single_message(
                message, usage,
            ))
        }

        fn executes_tools_outside_gosling(&self) -> bool {
            self.external_tools.load(Ordering::SeqCst)
        }
    }

    async fn create_reviewable_plan(
        cli: &CliSession,
        manager: &gosling::session::SessionManager,
        session_id: &str,
        provider: Arc<PlanningTestProvider>,
    ) -> PlanSnapshot {
        let model = gosling_providers::model::ModelConfig::new("planner-model");
        cli.agent
            .update_provider(provider.clone(), model.clone(), session_id)
            .await
            .unwrap();
        let started = manager
            .plans()
            .start_or_resume(session_id, provider.as_ref(), Some(model.model_name), None)
            .await
            .unwrap();
        let revised = manager
            .plans()
            .update_revision(
                session_id,
                gosling::session::NewPlanRevision {
                    content_markdown: "# Exact plan\n\n1. Keep history.".to_string(),
                    expected_generation: started.plan.generation,
                    expected_parent_revision_id: None,
                    planner_provider: Some("planning-test".to_string()),
                    planner_model: Some("planner-model".to_string()),
                },
            )
            .await
            .unwrap();
        manager
            .plans()
            .request_review(session_id, &PlanExpectation::for_snapshot(&revised))
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn cli_restart_observes_the_durable_plan_generation() {
        let temp = tempfile::tempdir().unwrap();
        let (cli, manager, session_id) = cli_session_with_messages(&temp, &[]).await;
        let provider = PlanningTestProvider::new(false, false);
        manager
            .plans()
            .start_or_resume(&session_id, &provider, Some("planner".to_string()), None)
            .await
            .unwrap();
        let restarted = CliSession::new(
            cli.agent,
            session_id,
            false,
            None,
            None,
            "text".into(),
            false,
        )
        .await;

        let snapshot = restarted.require_current_plan().await.unwrap();
        assert_eq!(snapshot.plan.status, PlanStatus::Drafting);
        assert_eq!(snapshot.plan.generation, 1);
    }

    #[tokio::test]
    async fn configured_status_hooks_are_suppressed_while_a_plan_is_open() {
        let temp = tempfile::tempdir().unwrap();
        let (cli, manager, session_id) = cli_session_with_messages(&temp, &[]).await;
        let provider = PlanningTestProvider::new(false, false);

        assert!(matches!(
            manager
                .plans()
                .interaction_policy(&session_id)
                .await
                .unwrap(),
            InteractionPolicy::Normal
        ));
        let hook_calls = AtomicUsize::new(0);
        cli.run_status_hook_with("normal-test", |_| {
            hook_calls.fetch_add(1, Ordering::SeqCst);
        })
        .await;
        assert_eq!(hook_calls.load(Ordering::SeqCst), 1);

        manager
            .plans()
            .start_or_resume(&session_id, &provider, Some("planner".to_string()), None)
            .await
            .unwrap();

        assert!(matches!(
            manager
                .plans()
                .interaction_policy(&session_id)
                .await
                .unwrap(),
            InteractionPolicy::Planning { .. }
        ));
        cli.run_status_hook_with("planning-test", |_| {
            hook_calls.fetch_add(1, Ordering::SeqCst);
        })
        .await;
        assert_eq!(hook_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn plan_export_rejects_stale_selected_status_after_concurrent_approval() {
        let temp = tempfile::tempdir().unwrap();
        let (cli, manager, session_id) = cli_session_with_messages(&temp, &[]).await;
        let provider = Arc::new(PlanningTestProvider::new(false, false));
        let selected = create_reviewable_plan(&cli, manager.as_ref(), &session_id, provider).await;

        let approved = manager
            .plans()
            .approve(&session_id, &PlanExpectation::for_snapshot(&selected), None)
            .await
            .unwrap();
        assert_eq!(approved.plan.status, PlanStatus::Approved);

        let error = cli.export_selected_plan(&selected).await.unwrap_err();
        assert!(matches!(
            error.downcast_ref::<gosling::session::PlanError>(),
            Some(gosling::session::PlanError::Conflict(message))
                if message == "plan status changed from expected awaiting_review to approved"
        ));
    }

    #[tokio::test]
    async fn abandon_command_preserves_conversation_history() {
        let temp = tempfile::tempdir().unwrap();
        let earlier = Message::user()
            .with_text("PM04-T1 hello")
            .with_id("earlier");
        let (cli, manager, session_id) = cli_session_with_messages(&temp, &[earlier]).await;
        let provider = PlanningTestProvider::new(false, false);
        manager
            .plans()
            .start_or_resume(&session_id, &provider, Some("planner".to_string()), None)
            .await
            .unwrap();
        cli.handle_plan_abandon().await.unwrap();

        let stored = manager.get_session(&session_id, true).await.unwrap();
        assert_eq!(stored.conversation.unwrap_or_default().len(), 1);
        assert_eq!(cli.messages.len(), 1);
        assert_eq!(
            manager
                .plans()
                .snapshot(&session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            gosling::session::PlanStatus::Abandoned
        );
    }

    #[tokio::test]
    async fn incompatible_provider_fails_before_plan_or_prompt_persistence() {
        let _guard = env_lock::lock_env([
            (GOSLING_PLANNER_PROVIDER, Some("planning-test")),
            (GOSLING_PLANNER_MODEL, Some("planner-model")),
            (GOSLING_PLANNER_CONTEXT_LIMIT, None::<&str>),
        ]);
        let temp = tempfile::tempdir().unwrap();
        let earlier = Message::user().with_text("keep me").with_id("earlier");
        let (mut cli, manager, session_id) = cli_session_with_messages(&temp, &[earlier]).await;
        let provider = Arc::new(PlanningTestProvider::new(false, true));
        cli.agent
            .update_provider(
                provider.clone(),
                gosling::model_config::model_config_from_user_config(
                    "planning-test",
                    "planner-model",
                )
                .unwrap(),
                &session_id,
            )
            .await
            .unwrap();

        let error = cli
            .handle_plan_mode(input::PlanCommandOptions {
                message_text: "write a plan".to_string(),
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("unsupported by this provider"));
        assert!(manager
            .plans()
            .snapshot(&session_id)
            .await
            .unwrap()
            .is_none());
        assert_eq!(
            manager
                .get_session(&session_id, true)
                .await
                .unwrap()
                .conversation
                .unwrap()
                .len(),
            1
        );
        assert_eq!(provider.stream_calls.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn plain_plan_approval_does_not_submit_a_prompt_or_change_mode() {
        let temp = tempfile::tempdir().unwrap();
        let earlier = Message::user().with_text("keep me").with_id("earlier");
        let (cli, manager, session_id) = cli_session_with_messages(&temp, &[earlier]).await;
        let provider = Arc::new(PlanningTestProvider::new(false, false));
        create_reviewable_plan(&cli, manager.as_ref(), &session_id, provider.clone()).await;
        let mode_before = cli.agent.gosling_mode().await;

        cli.handle_plan_approve().await.unwrap();

        assert_eq!(provider.stream_calls.load(Ordering::SeqCst), 0);
        assert_eq!(cli.agent.gosling_mode().await, mode_before);
        assert_eq!(
            manager
                .plans()
                .snapshot(&session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Approved
        );
        assert_eq!(
            manager
                .get_session(&session_id, true)
                .await
                .unwrap()
                .conversation
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn approve_and_run_submits_one_visible_prompt_and_preserves_history_and_mode() {
        let temp = tempfile::tempdir().unwrap();
        let earlier = Message::user().with_text("keep me").with_id("earlier");
        let (mut cli, manager, session_id) = cli_session_with_messages(&temp, &[earlier]).await;
        let provider = Arc::new(PlanningTestProvider::new(false, false));
        create_reviewable_plan(&cli, manager.as_ref(), &session_id, provider.clone()).await;
        let mode_before = cli.agent.gosling_mode().await;

        cli.handle_plan_approve_and_run().await.unwrap();

        assert_eq!(provider.stream_calls.load(Ordering::SeqCst), 1);
        assert_eq!(cli.agent.gosling_mode().await, mode_before);
        let approved = manager
            .plans()
            .snapshot(&session_id)
            .await
            .unwrap()
            .unwrap();
        let revision = approved.active_revision.as_ref().unwrap();
        let expected_prompt = format!(
            "Implement approved plan {} generation {} revision {} ({}; source {}; scope {}). Follow the stored plan exactly; report deviations.",
            approved.plan.id,
            approved.plan.generation,
            revision.id,
            revision.content_sha256,
            revision.source_hash,
            revision.scope_hash
        );
        let stored = manager.get_session(&session_id, true).await.unwrap();
        let conversation = stored.conversation.unwrap();
        assert_eq!(conversation.messages()[0].as_concat_text(), "keep me");
        let implementation_prompts = conversation
            .messages()
            .iter()
            .filter(|message| {
                message.role == rmcp::model::Role::User
                    && message
                        .as_concat_text()
                        .starts_with("Implement approved plan")
            })
            .count();
        assert_eq!(implementation_prompts, 1);
        let implementation_prompt = conversation
            .messages()
            .iter()
            .find(|message| {
                message.role == rmcp::model::Role::User
                    && message
                        .as_concat_text()
                        .starts_with("Implement approved plan")
            })
            .map(Message::as_concat_text)
            .unwrap();
        assert_eq!(implementation_prompt, expected_prompt);
        assert!(!implementation_prompt.contains("# Exact plan"));
        assert_eq!(approved.plan.status, PlanStatus::Approved);
    }

    #[tokio::test]
    async fn approve_and_run_reports_prompt_failure_without_rolling_back_approval() {
        let temp = tempfile::tempdir().unwrap();
        let earlier = Message::user().with_text("keep me").with_id("earlier");
        let (mut cli, manager, session_id) = cli_session_with_messages(&temp, &[earlier]).await;
        let provider = Arc::new(PlanningTestProvider::new(true, false));
        create_reviewable_plan(&cli, manager.as_ref(), &session_id, provider.clone()).await;
        let mode_before = cli.agent.gosling_mode().await;

        cli.handle_plan_approve_and_run().await.unwrap();

        assert_eq!(provider.stream_calls.load(Ordering::SeqCst), 1);
        assert_eq!(cli.agent.gosling_mode().await, mode_before);
        assert_eq!(
            manager
                .plans()
                .snapshot(&session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Approved
        );
        let stored = manager.get_session(&session_id, true).await.unwrap();
        assert_eq!(
            stored
                .conversation
                .unwrap_or_default()
                .messages()
                .first()
                .map(Message::as_concat_text)
                .as_deref(),
            Some("keep me")
        );
    }

    #[tokio::test]
    async fn implementation_start_failure_is_a_partial_success_after_approval() {
        let temp = tempfile::tempdir().unwrap();
        let (cli, manager, session_id) = cli_session_with_messages(&temp, &[]).await;
        let provider = Arc::new(PlanningTestProvider::new(false, false));
        let current = create_reviewable_plan(&cli, manager.as_ref(), &session_id, provider).await;
        let approved = manager
            .plans()
            .approve(&session_id, &PlanExpectation::for_snapshot(&current), None)
            .await
            .unwrap();

        let outcome = report_plan_implementation_submission(
            &approved,
            Err(anyhow::anyhow!("implementation prompt did not start")),
        );

        assert_eq!(
            outcome,
            PlanImplementationSubmission::Failed("implementation prompt did not start".to_string())
        );
        assert_eq!(
            manager
                .plans()
                .snapshot(&session_id)
                .await
                .unwrap()
                .unwrap()
                .plan
                .status,
            PlanStatus::Approved
        );
    }

    #[tokio::test]
    async fn plan_feedback_remains_durable_when_the_revision_prompt_fails() {
        let temp = tempfile::tempdir().unwrap();
        let (mut cli, manager, session_id) = cli_session_with_messages(&temp, &[]).await;
        let provider = Arc::new(PlanningTestProvider::new(false, false));
        create_reviewable_plan(&cli, manager.as_ref(), &session_id, provider.clone()).await;
        provider.set_external_tools(true);

        let error = cli
            .handle_plan_feedback("Keep the migration reversible")
            .await
            .unwrap_err();

        assert!(error
            .to_string()
            .contains("cannot be used for host-enforced planning"));
        assert_eq!(provider.stream_calls.load(Ordering::SeqCst), 0);
        let snapshot = manager
            .plans()
            .snapshot(&session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(snapshot.plan.status, PlanStatus::Drafting);
        assert!(snapshot
            .feedback
            .iter()
            .any(|item| item.body == "Keep the migration reversible"));
    }

    #[tokio::test]
    async fn interrupting_a_tool_turn_keeps_the_prompt_and_answers_the_request() {
        let temp = tempfile::tempdir().unwrap();
        let prompt = Message::user()
            .with_text("run the slow tool")
            .with_id("turn");
        let request = Message::assistant().with_generated_id().with_tool_request(
            "slow-request",
            Ok(rmcp::model::CallToolRequestParams::new("slow_wait")),
        );
        let (mut cli, manager, session_id) =
            cli_session_with_messages(&temp, &[prompt, request]).await;

        cli.handle_interrupted_messages(true, true, Some("turn"))
            .await
            .unwrap();

        let stored = manager
            .get_session(&session_id, true)
            .await
            .unwrap()
            .conversation
            .unwrap();
        let messages = stored.messages();
        assert_eq!(messages[0].as_concat_text(), "run the slow tool");
        assert!(messages.iter().any(|message| {
            message
                .content
                .iter()
                .filter_map(MessageContent::as_tool_response)
                .any(|response| response.id == "slow-request")
        }));
        assert_eq!(
            messages.last().map(Message::as_concat_text).as_deref(),
            Some(CANCELLED_TURN_NOTICE)
        );
        assert_eq!(cli.messages.messages()[0].id.as_deref(), Some("turn"));
        assert_eq!(
            cli.messages
                .messages()
                .last()
                .map(Message::as_concat_text)
                .as_deref(),
            Some(CANCELLED_TURN_NOTICE)
        );
    }

    #[tokio::test]
    async fn interrupting_a_text_turn_preserves_the_prompt_and_records_cancellation() {
        let temp = tempfile::tempdir().unwrap();
        let before = Message::user().with_text("earlier").with_id("earlier");
        let prompt = Message::user()
            .with_text("long answer please")
            .with_id("turn");
        let partial = Message::assistant().with_text("partial").with_id("partial");
        let (mut cli, manager, session_id) =
            cli_session_with_messages(&temp, &[before, prompt, partial]).await;

        cli.handle_interrupted_messages(true, true, Some("turn"))
            .await
            .unwrap();

        let stored = manager
            .get_session(&session_id, true)
            .await
            .unwrap()
            .conversation
            .unwrap();
        let texts: Vec<String> = stored
            .messages()
            .iter()
            .map(Message::as_concat_text)
            .collect();
        assert_eq!(
            texts,
            vec![
                "earlier".to_string(),
                "long answer please".to_string(),
                "Run cancelled by user before completion.".to_string(),
            ]
        );
        assert_eq!(
            cli.messages
                .messages()
                .iter()
                .map(Message::as_concat_text)
                .collect::<Vec<_>>(),
            texts
        );
    }

    #[test_case(
        "https://mcp.kiwi.com", 300,
        ExtensionConfig::StreamableHttp {
            name: "mcp_kiwi_com".into(),
            uri: "https://mcp.kiwi.com".into(),
            envs: Envs::default(),
            env_keys: vec![],
            headers: HashMap::new(),
            description: gosling::config::DEFAULT_EXTENSION_DESCRIPTION.to_string(),
            timeout: Some(300),
            socket: None,
            client_id: None,
            client_secret_key: None,
            scopes: vec![],
            bundled: None,
            available_tools: vec![],
        }
        ; "name_from_host"
    )]
    #[test_case(
        "http://localhost:8080/api", 300,
        ExtensionConfig::StreamableHttp {
            name: "localhost_8080_api".into(),
            uri: "http://localhost:8080/api".into(),
            envs: Envs::default(),
            env_keys: vec![],
            headers: HashMap::new(),
            description: gosling::config::DEFAULT_EXTENSION_DESCRIPTION.to_string(),
            timeout: Some(300),
            socket: None,
            client_id: None,
            client_secret_key: None,
            scopes: vec![],
            bundled: None,
            available_tools: vec![],
        }
        ; "port_and_path"
    )]
    #[test_case(
        "http://localhost:9090/other", 300,
        ExtensionConfig::StreamableHttp {
            name: "localhost_9090_other".into(),
            uri: "http://localhost:9090/other".into(),
            envs: Envs::default(),
            env_keys: vec![],
            headers: HashMap::new(),
            description: gosling::config::DEFAULT_EXTENSION_DESCRIPTION.to_string(),
            timeout: Some(300),
            socket: None,
            client_id: None,
            client_secret_key: None,
            scopes: vec![],
            bundled: None,
            available_tools: vec![],
        }
        ; "different_port_and_path"
    )]
    fn test_parse_streamable_http_extension(url: &str, timeout: u64, expected: ExtensionConfig) {
        assert_eq!(
            CliSession::parse_streamable_http_extension(url, timeout),
            expected
        );
    }
}
#[test]
fn tool_confirmation_menu_defaults_to_a_non_approving_choice() {
    assert!(!matches!(
        TOOL_CONFIRMATION_DEFAULT,
        Permission::AllowOnce | Permission::AlwaysAllow | Permission::AlwaysAllowDomain
    ));
}

#[test]
fn non_interactive_confirmations_are_denied() {
    assert_eq!(
        non_interactive_confirmation_permission(),
        Permission::DenyOnce
    );
}
