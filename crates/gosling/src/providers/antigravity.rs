use anyhow::Result;
use async_stream::try_stream;
use async_trait::async_trait;
use futures::future::BoxFuture;
use gosling_providers::conversation::token_usage::{ProviderUsage, Usage};
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use rmcp::model::{Role, Tool};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::{Arc, RwLock};
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::process::Command;

use super::base::{
    stream_from_single_message, ConfigKey, MessageStream, ModelInfo, Provider, ProviderDef,
    ProviderMetadata,
};
use super::cli_common::{
    generate_simple_session_description, is_session_description_request, reject_hosted_tools,
};
use super::utils::filter_extensions_from_system_prompt;
use crate::config::search_path::SearchPaths;
use crate::config::{Config, ExtensionConfig, GoslingMode};
use crate::conversation::message::{Message, MessageContent};
use crate::subprocess::configure_subprocess;

const ANTIGRAVITY_PROVIDER_NAME: &str = "antigravity";
pub const ANTIGRAVITY_DEFAULT_MODEL: &str = "gemini-3.1-pro-high";
pub const ANTIGRAVITY_FAST_MODEL: &str = "gemini-3.8-flash-low";
pub const ANTIGRAVITY_DOC_URL: &str = "https://antigravity.google/docs/cli/reference";

/// Fallback list used when `agy models` cannot be reached; the CLI serves the
/// authoritative list per account, so `fetch_supported_models` prefers it.
///
/// Antigravity fronts models from three vendors under its own ids, which the
/// canonical registry does not carry, so each limit is stated here from the
/// serving vendor's published window rather than left to the 128k default.
pub const ANTIGRAVITY_KNOWN_MODELS: &[(&str, usize)] = &[
    ("gemini-3.1-pro-high", 1_048_576),
    ("gemini-3.1-pro-low", 1_048_576),
    ("gemini-3.8-flash-high", 1_048_576),
    ("gemini-3.8-flash-medium", 1_048_576),
    ("gemini-3.8-flash-low", 1_048_576),
    ("claude-opus-4-6-thinking", 200_000),
    ("claude-sonnet-4-6", 200_000),
    ("gpt-oss-120b-medium", 131_072),
];

/// Gosling owns turn cancellation, so the CLI's own print-mode deadline only
/// needs to be long enough never to truncate a legitimate agent turn.
const PRINT_TIMEOUT: &str = "24h";

const MAX_STDERR_CAPTURE: usize = 64 * 1024;

/// How many prior messages to replay when a turn starts against a freshly
/// spawned child that cannot know about history Gosling already has. Matches
/// `DEFAULT_SESSION_TAIL_LIMIT`, the window shown on a compacted session
/// reload, so the backfill matches what is visibly on screen.
const RESTART_BACKFILL_MESSAGES: usize = 50;

/// The NDJSON turn protocol behind `agy --print= --input-format stream-json
/// --output-format stream-json`: one `{"event":"user",...}` line per turn on
/// stdin, and an `init`/`step_update`*/`result` sequence per turn on stdout.
struct AntigravityProcess {
    child: tokio::process::Child,
    stdin: Box<dyn AsyncWrite + Unpin + Send>,
    reader: BufReader<Box<dyn AsyncRead + Unpin + Send>>,
    stderr_handle: tokio::task::JoinHandle<String>,
    model: String,
}

impl std::fmt::Debug for AntigravityProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AntigravityProcess")
            .field("model", &self.model)
            .finish_non_exhaustive()
    }
}

impl Drop for AntigravityProcess {
    fn drop(&mut self) {
        self.stderr_handle.abort();
        let _ = self.child.start_kill();
    }
}

/// Everything needed to spawn a child, captured so the stream body can respawn
/// without borrowing the provider.
#[derive(Clone)]
struct SpawnSettings {
    command: PathBuf,
    working_dir: PathBuf,
    mode: GoslingMode,
}

impl SpawnSettings {
    fn build_command(&self, model: &str) -> Result<Command, ProviderError> {
        let mut cmd = Command::new(&self.command);
        configure_subprocess(&mut cmd);
        cmd.current_dir(&self.working_dir);

        if let Ok(path) = SearchPaths::builder().path() {
            cmd.env("PATH", path);
        }

        cmd.arg("--print=")
            .arg("--input-format")
            .arg("stream-json")
            .arg("--output-format")
            .arg("stream-json")
            .arg("--print-timeout")
            .arg(PRINT_TIMEOUT)
            .arg("--disable-slash-commands")
            .arg("--model")
            .arg(model);

        apply_permission_flags(&mut cmd, self.mode)?;

        cmd.stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Ok(cmd)
    }

    async fn spawn(&self, model: &str) -> Result<AntigravityProcess, ProviderError> {
        let mut cmd = self.build_command(model)?;
        let mut child = cmd.spawn().map_err(|e| {
            ProviderError::RequestFailed(format!(
                "Failed to spawn Antigravity CLI '{}': {e}. \
                 Install the Antigravity CLI and make sure `agy` is on the configured search paths.",
                self.command.display()
            ))
        })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| ProviderError::RequestFailed("Failed to capture stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ProviderError::RequestFailed("Failed to capture stdout".to_string()))?;
        let stderr = child.stderr.take();

        // The child persists across turns, so keep only a bounded tail while
        // draining so it never blocks on a full stderr pipe.
        let stderr_handle = tokio::spawn(async move {
            let mut captured: Vec<u8> = Vec::new();
            if let Some(mut stderr) = stderr {
                let mut chunk = [0u8; 8192];
                loop {
                    match stderr.read(&mut chunk).await {
                        Ok(0) | Err(_) => break,
                        Ok(n) => {
                            captured.extend_from_slice(&chunk[..n]);
                            if captured.len() > MAX_STDERR_CAPTURE {
                                let excess = captured.len() - MAX_STDERR_CAPTURE;
                                captured.drain(..excess);
                            }
                        }
                    }
                }
            }
            String::from_utf8_lossy(&captured).into_owned()
        });

        Ok(AntigravityProcess {
            child,
            stdin: Box::new(stdin),
            reader: BufReader::new(Box::new(stdout)),
            stderr_handle,
            model: model.to_string(),
        })
    }
}

/// Antigravity's headless print mode exposes no channel for routing an approval
/// back to the caller: a tool its own `toolPermission` setting will not clear is
/// soft-denied rather than surfaced. Only Auto has an honest mapping.
fn apply_permission_flags(cmd: &mut Command, mode: GoslingMode) -> Result<(), ProviderError> {
    match mode {
        GoslingMode::Auto => {
            cmd.arg("--dangerously-skip-permissions");
            Ok(())
        }
        GoslingMode::SmartApprove | GoslingMode::Approve | GoslingMode::Chat => {
            Err(ProviderError::ExecutionError(format!(
                "antigravity cannot route Gosling mode '{mode}' approvals in headless mode"
            )))
        }
    }
}

fn user_event_line(prompt: &str) -> String {
    let event = json!({
        "event": "user",
        "message": { "role": "user", "content": prompt },
    });
    format!("{event}\n")
}

fn usage_from_event(usage: &Value) -> Usage {
    let get = |key: &str| {
        usage
            .get(key)
            .and_then(Value::as_i64)
            .and_then(|v| i32::try_from(v).ok())
    };
    // Antigravity reports `input_tokens` and `total_tokens` exclusive of the
    // tokens it served from cache, which it counts separately.
    Usage::from_cache_exclusive_input(
        get("input_tokens"),
        get("output_tokens"),
        get("total_tokens"),
        get("cache_read_tokens"),
        None,
    )
}

fn error_from_result(result: &Value) -> ProviderError {
    let message = result
        .get("error")
        .and_then(Value::as_str)
        .filter(|e| !e.is_empty())
        .unwrap_or("Unknown error");
    if message.contains("context window exceeded") || message.contains("context length") {
        ProviderError::ContextLengthExceeded(message.to_string())
    } else {
        ProviderError::RequestFailed(format!("Antigravity CLI error: {message}"))
    }
}

fn parse_model_listing(stdout: &str) -> Vec<String> {
    stdout
        .lines()
        .filter_map(|line| line.split_once('\t'))
        .map(|(id, _display)| id.trim().to_string())
        .filter(|id| !id.is_empty())
        .collect()
}

#[derive(Debug, serde::Serialize)]
pub struct AntigravityProvider {
    command: PathBuf,
    #[serde(skip)]
    name: String,
    working_dir: PathBuf,
    #[serde(skip)]
    process: Arc<tokio::sync::Mutex<Option<AntigravityProcess>>>,
    #[serde(skip)]
    gosling_mode: RwLock<GoslingMode>,
}

impl AntigravityProvider {
    fn spawn_settings(&self) -> Result<SpawnSettings, ProviderError> {
        let mode = *self
            .gosling_mode
            .read()
            .map_err(|_| ProviderError::RequestFailed("Antigravity mode lock poisoned".into()))?;
        Ok(SpawnSettings {
            command: self.command.clone(),
            working_dir: self.working_dir.clone(),
            mode,
        })
    }

    fn last_user_text(messages: &[Message]) -> String {
        messages
            .iter()
            .rev()
            .find(|m| m.role == Role::User)
            .map(|m| m.as_concat_text())
            .unwrap_or_default()
    }

    /// The CLI has no system-prompt flag, so a freshly spawned child gets the
    /// system prompt folded into its first user turn. On a mid-session respawn
    /// it also gets a bounded replay of Gosling's own record, which the child
    /// has no other way to know about.
    fn build_prompt(system: &str, messages: &[Message], is_fresh: bool) -> String {
        let user_text = Self::last_user_text(messages);
        if !is_fresh {
            return user_text;
        }

        let mut parts = Vec::new();
        let filtered_system = filter_extensions_from_system_prompt(system);
        if !filtered_system.is_empty() {
            parts.push(filtered_system);
        }
        if let Some(backfill) = Self::restart_backfill_text(messages) {
            parts.push(backfill);
        }
        parts.push(user_text);
        parts.join("\n\n")
    }

    fn restart_backfill_text(messages: &[Message]) -> Option<String> {
        let latest_user_idx = messages.iter().rposition(|m| m.role == Role::User)?;
        let backfill_start =
            latest_user_idx.saturating_sub(RESTART_BACKFILL_MESSAGES.min(latest_user_idx));
        let backfill = &messages[backfill_start..latest_user_idx];
        if backfill.is_empty() {
            return None;
        }

        let replay = backfill
            .iter()
            .map(crate::context_mgmt::format_message_for_compacting)
            .collect::<Vec<_>>()
            .join("\n\n");

        Some(format!(
            "[Gosling reconnected to this session after a restart. The Antigravity CLI process \
             handling it did not survive, so it has no memory of the conversation below. \
             Replaying the last {} message(s) from Gosling's own record for context. This is not \
             a summary; some detail may still be missing if the conversation is longer than the \
             replay window.\n\n{}\n\n--- end of replay, continuing normally below ---]",
            backfill.len(),
            replay
        ))
    }

    async fn models_from_cli(&self) -> Result<Vec<String>, ProviderError> {
        let mut cmd = Command::new(&self.command);
        configure_subprocess(&mut cmd);
        cmd.current_dir(&self.working_dir);
        if let Ok(path) = SearchPaths::builder().path() {
            cmd.env("PATH", path);
        }
        cmd.arg("models")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        let output = cmd.output().await.map_err(|e| {
            ProviderError::RequestFailed(format!(
                "Failed to run `{} models`: {e}",
                self.command.display()
            ))
        })?;

        let models = parse_model_listing(&String::from_utf8_lossy(&output.stdout));
        if models.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ProviderError::Authentication(format!(
                "Antigravity CLI returned no models. Sign in once by running `agy` in a terminal \
                 and completing the Google sign-in, then retry. Details: {}",
                stderr.trim()
            )));
        }
        Ok(models)
    }
}

impl gosling_providers::base::ProviderDescriptor for AntigravityProvider {
    fn metadata() -> ProviderMetadata {
        ProviderMetadata::with_models(
            ANTIGRAVITY_PROVIDER_NAME,
            "Antigravity",
            "Google's agentic coding CLI (`agy`), driven headless over its stream-json protocol. \
             Reuses the sign-in already held by the Antigravity CLI or IDE.",
            ANTIGRAVITY_DEFAULT_MODEL,
            ANTIGRAVITY_KNOWN_MODELS
                .iter()
                .map(|(name, context_limit)| ModelInfo::new(*name, *context_limit))
                .collect(),
            ANTIGRAVITY_DOC_URL,
            vec![ConfigKey::new(
                "ANTIGRAVITY_COMMAND",
                true,
                false,
                Some("agy"),
                true,
            )],
        )
        .with_fast_model(ANTIGRAVITY_FAST_MODEL)
        .with_setup_steps(vec![
            "Install the Antigravity CLI (it ships with the Antigravity IDE and the Antigravity VS Code extension), then run `agy install` to put `agy` on your PATH",
            "Sign in once in a terminal: run `agy`, complete the Google sign-in, then exit. Gosling reuses that session from `~/.gemini/`",
            "Confirm headless access works: `agy models` should list models without prompting",
            "Add to your gosling config file (`~/.config/gosling/config.yaml` on macOS/Linux):\n  GOSLING_PROVIDER: antigravity\n  GOSLING_MODEL: gemini-3.1-pro-high",
            "Restart gosling for changes to take effect",
        ])
    }
}

impl ProviderDef for AntigravityProvider {
    type Provider = Self;
    const MANAGES_OWN_CONTEXT: bool = true;
    const EXECUTES_TOOLS_OUTSIDE_GOSLING: bool = true;

    fn from_env(
        extensions: Vec<ExtensionConfig>,
        tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<Self::Provider>> {
        Self::from_env_with_working_dir(
            extensions,
            crate::providers::base::current_working_dir(),
            tls_config,
        )
    }

    fn from_env_with_working_dir(
        _extensions: Vec<ExtensionConfig>,
        working_dir: PathBuf,
        _tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<Self::Provider>> {
        Box::pin(async move {
            let config = Config::global();
            let command: String = config.get_antigravity_command().unwrap_or_default().into();
            let resolved_command = SearchPaths::builder().resolve(&command)?;

            Ok(Self {
                command: resolved_command,
                name: ANTIGRAVITY_PROVIDER_NAME.to_string(),
                working_dir,
                process: Arc::new(tokio::sync::Mutex::new(None)),
                gosling_mode: RwLock::new(config.get_gosling_mode().unwrap_or_default()),
            })
        })
    }
}

#[async_trait]
impl Provider for AntigravityProvider {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn manages_own_context(&self) -> bool {
        true
    }

    fn executes_tools_outside_gosling(&self) -> bool {
        true
    }

    async fn fetch_supported_models(&self) -> Result<Vec<String>, ProviderError> {
        self.models_from_cli().await
    }

    async fn update_mode(&self, _session_id: &str, mode: GoslingMode) -> Result<(), ProviderError> {
        if mode != GoslingMode::Auto {
            return Err(ProviderError::ExecutionError(format!(
                "antigravity cannot route Gosling mode '{mode}' approvals in headless mode"
            )));
        }
        *self
            .gosling_mode
            .write()
            .map_err(|_| ProviderError::RequestFailed("Antigravity mode lock poisoned".into()))? =
            mode;
        Ok(())
    }

    async fn stream(
        &self,
        model_config: &ModelConfig,
        system: &str,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        reject_hosted_tools("Antigravity", tools)?;

        if is_session_description_request(system) {
            let (message, usage) =
                generate_simple_session_description(&model_config.model_name, messages)?;
            return Ok(stream_from_single_message(message, usage));
        }

        let settings = self.spawn_settings()?;
        let model_name = model_config.model_name.clone();
        let slot = Arc::clone(&self.process);
        let system = system.to_string();
        let messages = messages.to_vec();
        let message_id = uuid::Uuid::new_v4().to_string();

        Ok(Box::pin(try_stream! {
            // Held across write-and-read so two turns can never interleave on
            // the one child's stdio.
            let mut guard = slot.lock_owned().await;

            // A model change cannot be pushed into a running child, so the old
            // one is retired and the replacement gets a history replay.
            if guard.as_ref().is_some_and(|p| p.model != model_name) {
                *guard = None;
            }
            let is_fresh = guard.is_none();
            if is_fresh {
                *guard = Some(settings.spawn(&model_name).await?);
            }
            let process = guard.as_mut().expect("process spawned above");

            let prompt = AntigravityProvider::build_prompt(&system, &messages, is_fresh);
            if let Err(e) = process.stdin.write_all(user_event_line(&prompt).as_bytes()).await {
                *guard = None;
                Err(ProviderError::RequestFailed(format!("Failed to write to Antigravity CLI: {e}")))?;
                return;
            }
            let process = guard.as_mut().expect("process still present");
            if let Err(e) = process.stdin.flush().await {
                *guard = None;
                Err(ProviderError::RequestFailed(format!("Failed to flush to Antigravity CLI: {e}")))?;
                return;
            }

            let process = guard.as_mut().expect("process still present");
            let stream_timestamp = chrono::Utc::now().timestamp();
            let mut usage = Usage::default();
            let mut line = String::new();
            let mut stream_error: Option<ProviderError> = None;

            loop {
                line.clear();
                match process.reader.read_line(&mut line).await {
                    Ok(0) => {
                        stream_error = Some(ProviderError::RequestFailed(
                            "Antigravity CLI process terminated unexpectedly".to_string(),
                        ));
                        break;
                    }
                    Err(e) => {
                        stream_error = Some(ProviderError::RequestFailed(format!(
                            "Failed to read from Antigravity CLI: {e}"
                        )));
                        break;
                    }
                    Ok(_) => {}
                }

                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                let Ok(parsed) = serde_json::from_str::<Value>(trimmed) else {
                    tracing::trace!(line = trimmed, "Non-JSON line from Antigravity CLI");
                    continue;
                };

                match parsed.get("event").and_then(Value::as_str) {
                    Some("step_update") => {
                        let Some(step) = parsed.get("step_update") else { continue };
                        if step.get("step_type").and_then(Value::as_str) != Some("agent_response") {
                            continue;
                        }
                        if let Some(text) = step
                            .get("text_delta")
                            .and_then(Value::as_str)
                            .filter(|t| !t.is_empty())
                        {
                            let mut partial = Message::new(
                                Role::Assistant,
                                stream_timestamp,
                                vec![MessageContent::text(text)],
                            );
                            partial.id = Some(message_id.clone());
                            yield (Some(partial), None);
                        }
                    }
                    Some("result") => {
                        let Some(result) = parsed.get("result") else { break };
                        if result.get("status").and_then(Value::as_str) != Some("SUCCESS") {
                            stream_error = Some(error_from_result(result));
                            break;
                        }
                        if let Some(reported) = result.get("usage") {
                            usage = usage_from_event(reported);
                        }
                        break;
                    }
                    _ => {}
                }
            }

            if let Some(err) = stream_error {
                // The child's stdio is out of sync with the turn protocol once a
                // turn ends abnormally; the next turn starts a clean one.
                *guard = None;
                Err(err)?;
            }

            yield (None, Some(ProviderUsage::new(model_name, usage)));
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use gosling_providers::base::ProviderDescriptor as _;
    use test_case::test_case;

    fn make_provider() -> AntigravityProvider {
        AntigravityProvider {
            command: PathBuf::from("agy"),
            name: ANTIGRAVITY_PROVIDER_NAME.to_string(),
            working_dir: PathBuf::from("/tmp"),
            process: Arc::new(tokio::sync::Mutex::new(None)),
            gosling_mode: RwLock::new(GoslingMode::Auto),
        }
    }

    #[test]
    fn metadata_names_the_agy_binary_and_a_known_default_model() {
        let metadata = AntigravityProvider::metadata();
        assert_eq!(metadata.name, ANTIGRAVITY_PROVIDER_NAME);
        assert_eq!(metadata.config_keys[0].default.as_deref(), Some("agy"));
        let default_model = metadata
            .known_models
            .iter()
            .find(|m| m.name == ANTIGRAVITY_DEFAULT_MODEL)
            .expect("default model is a known model");
        assert_eq!(default_model.context_limit, 1_048_576);
        assert_eq!(metadata.fast_model.as_deref(), Some(ANTIGRAVITY_FAST_MODEL));
    }

    #[test_case(GoslingMode::Auto, true; "auto_skips_permission_prompts")]
    #[test_case(GoslingMode::SmartApprove, false; "smart_approve_is_unroutable")]
    #[test_case(GoslingMode::Approve, false; "approve_is_unroutable")]
    #[test_case(GoslingMode::Chat, false; "chat_is_unroutable")]
    fn only_auto_mode_has_an_honest_headless_mapping(mode: GoslingMode, supported: bool) {
        let mut cmd = Command::new("agy");
        assert_eq!(apply_permission_flags(&mut cmd, mode).is_ok(), supported);
    }

    #[test]
    fn auto_mode_command_drives_the_stream_json_turn_protocol() {
        let settings = SpawnSettings {
            command: PathBuf::from("agy"),
            working_dir: PathBuf::from("/tmp"),
            mode: GoslingMode::Auto,
        };
        let cmd = settings.build_command("gemini-3.1-pro-high").unwrap();
        let args: Vec<String> = cmd
            .as_std()
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();

        assert!(args.contains(&"--print=".to_string()));
        assert!(args
            .windows(2)
            .any(|w| w == ["--input-format", "stream-json"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--output-format", "stream-json"]));
        assert!(args
            .windows(2)
            .any(|w| w == ["--model", "gemini-3.1-pro-high"]));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"--disable-slash-commands".to_string()));
    }

    #[test]
    fn user_event_line_matches_the_cli_stream_input_schema() {
        let line = user_event_line("hello");
        assert!(line.ends_with('\n'));
        let parsed: Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(parsed["event"], "user");
        assert_eq!(parsed["message"]["role"], "user");
        assert_eq!(parsed["message"]["content"], "hello");
    }

    #[test]
    fn usage_folds_cache_reads_back_into_input_and_total() {
        // Values captured from a live `agy` turn.
        let usage = usage_from_event(&json!({
            "input_tokens": 30861,
            "output_tokens": 850,
            "thinking_tokens": 591,
            "cache_read_tokens": 48898,
            "total_tokens": 31711,
        }));
        assert_eq!(usage.input_tokens, Some(30861 + 48898));
        assert_eq!(usage.output_tokens, Some(850));
        assert_eq!(usage.total_tokens, Some(31711 + 48898));
        assert_eq!(usage.cache_read_input_tokens, Some(48898));
    }

    #[test]
    fn model_listing_parses_the_tab_separated_cli_output() {
        let stdout = "gemini-3.8-flash-high\tGemini 3.8 Flash (High)\n\
                      gemini-3.1-pro-high\tGemini 3.1 Pro (High)\n\
                      not a model row\n";
        assert_eq!(
            parse_model_listing(stdout),
            vec!["gemini-3.8-flash-high", "gemini-3.1-pro-high"]
        );
    }

    #[test_case(json!({"error": "the model request failed"}), false; "generic_failure")]
    #[test_case(json!({"error": "context window exceeded"}), true; "context_overflow")]
    fn result_errors_classify_context_overflow(result: Value, is_context_exceeded: bool) {
        assert_eq!(
            matches!(
                error_from_result(&result),
                ProviderError::ContextLengthExceeded(_)
            ),
            is_context_exceeded
        );
    }

    #[test]
    fn first_turn_folds_in_the_system_prompt_but_later_turns_do_not() {
        let messages = vec![Message::user().with_text("do the thing")];
        let fresh = AntigravityProvider::build_prompt("SYSTEM RULES", &messages, true);
        assert!(fresh.starts_with("SYSTEM RULES"));
        assert!(fresh.ends_with("do the thing"));

        let resumed = AntigravityProvider::build_prompt("SYSTEM RULES", &messages, false);
        assert_eq!(resumed, "do the thing");
    }

    #[test]
    fn a_fresh_child_mid_session_replays_prior_history() {
        let messages = vec![
            Message::user().with_text("first question"),
            Message::assistant().with_text("first answer"),
            Message::user().with_text("follow up"),
        ];
        let prompt = AntigravityProvider::build_prompt("", &messages, true);
        assert!(prompt.contains("Gosling reconnected to this session"));
        assert!(prompt.contains("first question"));
        assert!(prompt.ends_with("follow up"));
    }

    struct CannedTurn {
        provider: AntigravityProvider,
        /// Keeps the far end of the stdin pipe open so writes succeed.
        _stdin_sink: tokio::io::DuplexStream,
        text: String,
        usage: Option<ProviderUsage>,
        error: Option<ProviderError>,
    }

    async fn stream_canned_turn(stdout: &str) -> CannedTurn {
        let provider = make_provider();
        let (client, server) = tokio::io::duplex(64 * 1024);

        // A real child is required only so the process slot holds the same
        // shape it does in production; its stdio is replaced below.
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child.stdin.take();
        child.stdout.take();
        let stderr_handle = tokio::spawn(async { String::new() });

        *provider.process.lock().await = Some(AntigravityProcess {
            child,
            stdin: Box::new(client),
            reader: BufReader::new(Box::new(std::io::Cursor::new(stdout.as_bytes().to_vec()))),
            stderr_handle,
            model: "gemini-3.1-pro-high".to_string(),
        });

        let model = ModelConfig::new("gemini-3.1-pro-high");
        let mut stream = provider.stream(&model, "", &[], &[]).await.unwrap();

        let mut turn = CannedTurn {
            text: String::new(),
            usage: None,
            error: None,
            provider,
            _stdin_sink: server,
        };
        while let Some(item) = stream.next().await {
            match item {
                Ok((message, provider_usage)) => {
                    if let Some(message) = message {
                        turn.text.push_str(&message.as_concat_text());
                    }
                    if provider_usage.is_some() {
                        turn.usage = provider_usage;
                    }
                }
                Err(e) => turn.error = Some(e),
            }
        }
        drop(stream);
        turn
    }

    #[tokio::test]
    async fn agent_response_deltas_concatenate_into_the_final_answer() {
        // Shape captured from a live `agy` stream-json turn.
        let stdout = concat!(
            r#"{"event":"init","conversation_id":"c1","init":{"cwd":"/tmp","tools":[]}}"#,
            "\n",
            r#"{"event":"step_update","step_update":{"step_index":0,"state":"DONE","step_type":"user_input"}}"#,
            "\n",
            r#"{"event":"step_update","step_update":{"step_index":1,"state":"ACTIVE","step_type":"tool","tool_name":"view_file"}}"#,
            "\n",
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"ACTIVE","step_type":"agent_response","text_delta":"Files in "}}"#,
            "\n",
            r#"{"event":"step_update","step_update":{"step_index":2,"state":"DONE","step_type":"agent_response","text_delta":"the directory."}}"#,
            "\n",
            r#"{"event":"result","result":{"status":"SUCCESS","response":"Files in the directory.","usage":{"input_tokens":100,"output_tokens":20,"cache_read_tokens":5,"total_tokens":120}}}"#,
            "\n",
        );

        let turn = stream_canned_turn(stdout).await;
        assert!(turn.error.is_none());
        assert_eq!(turn.text, "Files in the directory.");
        let usage = turn.usage.unwrap();
        assert_eq!(usage.usage.input_tokens, Some(105));
        assert_eq!(usage.usage.output_tokens, Some(20));
        assert_eq!(usage.usage.cache_read_input_tokens, Some(5));
        // The child survives a clean turn and is reused by the next one.
        assert!(turn.provider.process.lock().await.is_some());
    }

    #[tokio::test]
    async fn an_error_result_retires_the_child_so_the_next_turn_starts_clean() {
        let stdout = concat!(
            r#"{"event":"result","result":{"status":"ERROR","response":"","error":"the model request failed"}}"#,
            "\n",
        );
        let turn = stream_canned_turn(stdout).await;
        assert!(matches!(turn.error, Some(ProviderError::RequestFailed(_))));
        assert!(turn.usage.is_none());
        assert!(turn.provider.process.lock().await.is_none());
    }
}
