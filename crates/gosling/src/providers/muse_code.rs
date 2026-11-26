use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::Result;
use async_stream::try_stream;
use async_trait::async_trait;
use futures::{future::BoxFuture, StreamExt};
use gosling_providers::conversation::token_usage::ProviderUsage;
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use rmcp::model::Tool;
use serde_json::{json, Value};
use tokio::time::{timeout, timeout_at, Instant};
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::task::AbortOnDropHandle;

use super::base::{
    stream_from_single_message, ConfigKey, MessageStream, ModelInfo, Provider, ProviderDef,
    ProviderDescriptor, ProviderMetadata, DEFAULT_PROVIDER_TIMEOUT_SECS,
};
use super::cli_common;
use super::utils::filter_extensions_from_system_prompt;
use crate::agents::mcp_client::ExtensionProcessGroup;
use crate::config::search_path::SearchPaths;
use crate::config::{Config, GoslingMode};
use crate::conversation::message::{Message, MessageContent};

mod cli;
mod output;

use cli::{diagnostic_stderr, MuseCli};
use output::{protocol_error, MuseOutput};

pub(crate) const MUSE_CODE_PROVIDER_NAME: &str = "muse-code";
const DEFAULT_MODEL: &str = "muse-spark-1.3";
const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;
const MAX_RUN_DURATION: Duration = Duration::from_secs(30 * 60);
const EXIT_TIMEOUT: Duration = Duration::from_secs(10);
const KNOWN_MODELS: &[&str] = &["muse-spark-1.3", "muse-wonder-1.0", "muse-spark-1.2"];

pub struct MuseCodeProvider {
    cli: Arc<MuseCli>,
    mode: RwLock<GoslingMode>,
}

pub(crate) fn resolved_muse_command() -> Result<PathBuf> {
    let command = Config::global()
        .get_param::<String>("MUSE_CODE_COMMAND")
        .unwrap_or_else(|_| "muse".to_string());
    SearchPaths::builder().resolve(command)
}

fn require_auto(mode: GoslingMode) -> Result<(), ProviderError> {
    if mode != GoslingMode::Auto {
        return Err(ProviderError::ExecutionError(format!(
            "Muse Code requires Auto mode: its headless CLI cannot relay Gosling '{mode}' approvals. Use the Meta Muse API provider for Gosling-managed permissions and tools."
        )));
    }
    Ok(())
}

fn build_prompt(system: &str, messages: &[Message]) -> Result<String, ProviderError> {
    let mut history = Vec::new();
    for message in messages {
        let mut content = Vec::new();
        for item in &message.content {
            match item {
                MessageContent::Text(_)
                | MessageContent::ToolRequest(_)
                | MessageContent::ToolResponse(_) => content.push(item),
                MessageContent::Image(_) => {
                    return Err(ProviderError::ExecutionError(
                        "Muse Code's Gosling wrapper accepts text only; use the Meta Muse API provider for image attachments".to_string(),
                    ));
                }
                _ => {}
            }
        }
        if !content.is_empty() {
            history.push(json!({"role": message.role, "content": content}));
        }
    }
    Ok(format!(
        "{}\n\nConversation history (JSON):\n{}\n\nContinue the conversation by answering the latest user request.",
        filter_extensions_from_system_prompt(system),
        Value::Array(history)
    ))
}

fn write_prompt(prompt: &str) -> Result<tempfile::NamedTempFile> {
    let mut file = tempfile::NamedTempFile::new()?;
    file.write_all(prompt.as_bytes())?;
    file.flush()?;
    Ok(file)
}

fn failure_hint(diagnostic: &str) -> Option<&'static str> {
    // CLI diagnostics can echo arbitrary tool output or prompt text.
    let diagnostic = diagnostic.to_ascii_lowercase();
    let contains = |patterns: &[&str]| patterns.iter().any(|pattern| diagnostic.contains(pattern));
    if contains(&[
        "missing meta credentials",
        "not logged in",
        "authentication expired",
        "unauthorized",
        "unauthenticated",
    ]) {
        Some("could not authenticate. Check the subscribed browser sign-in with `muse login`")
    } else if contains(&[
        "subscription limit",
        "subscription allowance",
        "quota exceeded",
    ]) {
        Some("reported a quota or subscription limit. Check `muse usage`")
    } else if contains(&[
        "unknown model",
        "model not found",
        "model is not available",
        "model unavailable",
    ]) {
        Some("could not use the selected model. Check `muse exec --provider meta --list-models`")
    } else if contains(&[
        "permission denied",
        "sandbox denied",
        "sandbox unavailable",
        "approval denied",
    ]) {
        Some("reported a permission or sandbox denial. Review the workspace and Muse permission settings")
    } else if contains(&[
        "connection refused",
        "failed to connect",
        "network error",
        "dns error",
    ]) {
        Some("reported a network failure. Check the connection before trying again")
    } else if contains(&["tool execution failed", "tool execution error"]) {
        Some("reported a tool execution failure. Check the workspace before trying again")
    } else {
        None
    }
}

fn run_failure(diagnostic: &str) -> ProviderError {
    protocol_error(
        failure_hint(diagnostic)
            .unwrap_or("reported a failure. Check Muse in the working directory"),
    )
}

impl ProviderDescriptor for MuseCodeProvider {
    fn metadata() -> ProviderMetadata {
        let models = KNOWN_MODELS
            .iter()
            .map(|name| {
                let mut info = ModelInfo::new(*name, 1_048_576);
                info.reasoning = true;
                info
            })
            .collect();
        ProviderMetadata::with_models(
            MUSE_CODE_PROVIDER_NAME,
            "Meta Muse Code (Subscription/CLI)",
            "Use your signed-in Muse Code CLI in Auto mode with Muse's tools and sandbox.",
            DEFAULT_MODEL,
            models,
            "https://dev.meta.ai/docs/muse-code",
            vec![ConfigKey::new("MUSE_CODE_COMMAND", false, false, None, true)],
        )
        .with_setup_steps(vec![
            "Install Muse Code using https://dev.meta.ai/docs/muse-code and confirm `muse --version` works",
            "Run `muse login` in a terminal and sign in to the account with your Muse Code subscription",
            "Check subscription billing with `muse usage --json`; a stored CLI API key takes priority over browser sign-in",
            "If needed, set MUSE_CODE_COMMAND to the full path of your Muse executable",
            "Select Auto mode in Gosling; this wrapper uses Muse's tools and sandbox, and cannot relay Gosling approval prompts or extensions",
        ])
    }
}

impl ProviderDef for MuseCodeProvider {
    type Provider = Self;
    const EXECUTES_TOOLS_OUTSIDE_GOSLING: bool = true;

    fn from_env(
        extensions: Vec<crate::config::ExtensionConfig>,
        tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<Self>> {
        Self::from_env_with_working_dir(extensions, super::base::current_working_dir(), tls_config)
    }

    fn from_env_with_working_dir(
        _extensions: Vec<crate::config::ExtensionConfig>,
        working_dir: PathBuf,
        _tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<Self>> {
        Box::pin(async move {
            let config = Config::global();
            let mode = config.resolve_gosling_mode()?;
            Ok(Self {
                cli: Arc::new(MuseCli::new(resolved_muse_command()?, working_dir)),
                mode: RwLock::new(mode),
            })
        })
    }
}

impl MuseCodeProvider {
    pub(crate) fn accepts_model_id(model: &str) -> bool {
        cli::valid_model_id(model)
    }
}

#[async_trait]
impl Provider for MuseCodeProvider {
    fn get_name(&self) -> &str {
        MUSE_CODE_PROVIDER_NAME
    }

    fn executes_tools_outside_gosling(&self) -> bool {
        true
    }

    async fn update_mode(&self, _session_id: &str, mode: GoslingMode) -> Result<(), ProviderError> {
        require_auto(mode)?;
        *self
            .mode
            .write()
            .map_err(|_| ProviderError::ExecutionError("Muse mode lock poisoned".to_string()))? =
            mode;
        Ok(())
    }

    async fn fetch_supported_models(&self) -> Result<Vec<String>, ProviderError> {
        if let Ok(capabilities) = self.cli.capabilities().await {
            if let Some(models) = self.cli.discover_models(&capabilities).await {
                return Ok(models);
            }
        }
        Ok(KNOWN_MODELS
            .iter()
            .map(|name| (*name).to_string())
            .collect())
    }

    async fn fetch_supported_model_info(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        let known = Self::metadata().known_models;
        Ok(self
            .fetch_supported_models()
            .await?
            .into_iter()
            .map(|name| {
                known
                    .iter()
                    .find(|model| model.name == name)
                    .cloned()
                    .unwrap_or_else(|| {
                        super::base::model_info_for_provider_model(MUSE_CODE_PROVIDER_NAME, &name)
                    })
            })
            .collect())
    }

    async fn stream(
        &self,
        model: &ModelConfig,
        system: &str,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        cli_common::reject_hosted_tools("Muse Code", tools)?;
        require_auto(
            *self.mode.read().map_err(|_| {
                ProviderError::ExecutionError("Muse mode lock poisoned".to_string())
            })?,
        )?;
        if cli_common::is_session_description_request(system) {
            let (message, usage) =
                cli_common::generate_simple_session_description(&model.model_name, messages)?;
            return Ok(stream_from_single_message(message, usage));
        }
        if !Self::accepts_model_id(&model.model_name) {
            return Err(protocol_error("requires a valid model ID"));
        }

        let prompt = build_prompt(system, messages)?;
        let counter = crate::token_counter::shared_token_counter()
            .await
            .map_err(|_| protocol_error("could not estimate the prompt size"))?;
        let estimated_tokens = counter.count_tokens(&prompt);
        let context_limit = self.get_context_limit(model).await?;
        if context_limit > 0 && estimated_tokens >= context_limit {
            return Err(ProviderError::ContextLengthExceeded(format!(
                "Muse Code was not started. The formatted prompt is estimated at {estimated_tokens} tokens for a {context_limit}-token window."
            )));
        }
        let capabilities = self.cli.capabilities().await?;
        if model.thinking_effort().is_some() {
            capabilities.require_reasoning_effort()?;
        }
        let prompt_file = write_prompt(&prompt)
            .map_err(|_| protocol_error("could not write the private prompt file"))?;

        let mut command = self.cli.command(&capabilities.executable);
        command
            .args([
                "exec",
                "--provider",
                "meta",
                "--json",
                "--model",
                &model.model_name,
            ])
            .arg("--prompt-file")
            .arg(prompt_file.path())
            .args(["--disable-approval", "--no-session-log"]);
        if let Some(effort) = model.thinking_effort() {
            command.arg("--reasoning-effort").arg(
                gosling_providers::formats::meta::reasoning_effort(&model.model_name, effort),
            );
        }
        let model_name = model.model_name.clone();
        let cli = Arc::clone(&self.cli);

        Ok(Box::pin(try_stream! {
            let _prompt_file = prompt_file;
            cli.require_subscription(&capabilities).await?;
            let mut child = cli::spawn(command).map_err(|_| {
                protocol_error("could not start exec; check installation, MUSE_CODE_COMMAND and the working directory")
            })?;
            let process_group = ExtensionProcessGroup::new(child.id());
            let stdout = child.stdout().take().ok_or_else(|| protocol_error("stdout is unavailable"))?;
            let stderr = child.stderr().take().ok_or_else(|| protocol_error("stderr is unavailable"))?;
            let mut stderr_task = AbortOnDropHandle::new(tokio::spawn(diagnostic_stderr(stderr)));
            let mut lines = FramedRead::new(stdout, LinesCodec::new_with_max_length(MAX_EVENT_BYTES));
            let mut output = MuseOutput::default();
            let deadline = Instant::now() + MAX_RUN_DURATION;
            let inactivity = Duration::from_secs(DEFAULT_PROVIDER_TIMEOUT_SECS);
            let mut last_event = Instant::now();
            loop {
                let read_deadline = deadline.min(last_event + inactivity);
                let timed_out = || {
                    protocol_error(if Instant::now() >= deadline {
                        "exceeded the overall run time limit"
                    } else {
                        "stopped producing events"
                    })
                };
                if Instant::now() >= read_deadline {
                    Err(timed_out())?;
                }
                let line = timeout_at(read_deadline, lines.next()).await.map_err(|_| timed_out())?;
                let Some(line) = line else { break };
                let line = line.map_err(|error| protocol_error(&format!("could not read a JSONL event: {error}")))?;
                if line.trim().is_empty() {
                    continue;
                }
                let event = serde_json::from_str(&line).map_err(|error| {
                    protocol_error(&format!("emitted malformed JSON at column {}", error.column()))
                })?;
                last_event = Instant::now();
                if let Some(delta) = output.push(event, capabilities.accurate_run_usage)? {
                    yield (Some(Message::assistant().with_text(delta)), None);
                }
            }
            let status = timeout_at(deadline.min(Instant::now() + EXIT_TIMEOUT), child.wait())
                .await.map_err(|_| protocol_error("did not exit after closing stdout"))?
                .map_err(|_| protocol_error("could not get the process exit status"))?;
            drop(process_group);
            drop(child);
            if !status.success() {
                let diagnostic = timeout(Duration::from_millis(250), &mut stderr_task)
                    .await.ok().and_then(Result::ok).and_then(Result::ok);
                let hint = diagnostic.as_deref().and_then(failure_hint)
                    .unwrap_or("exited unsuccessfully. Check Muse in the working directory");
                Err(ProviderError::ExecutionError(format!("Muse Code {hint} ({status}).")))?;
            }
            drop(stderr_task);
            let (text, usage) = output.finish()?;
            let message = (!text.is_empty()).then(|| Message::assistant().with_text(text));
            yield (message, Some(ProviderUsage::new(model_name, usage)));
        }))
    }
}
