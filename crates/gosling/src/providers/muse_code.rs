use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::RwLock;
use std::time::Duration;

use anyhow::Result;
use async_stream::try_stream;
use async_trait::async_trait;
use futures::{future::BoxFuture, StreamExt};
use gosling_providers::conversation::token_usage::{ProviderUsage, Usage};
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use gosling_providers::secret_redaction::redact_secrets;
use rmcp::model::Tool;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio_util::codec::{FramedRead, LinesCodec};
use tokio_util::task::AbortOnDropHandle;

use super::base::{
    stream_from_single_message, MessageStream, ModelInfo, Provider, ProviderDef,
    ProviderDescriptor, ProviderMetadata, DEFAULT_PROVIDER_TIMEOUT_SECS,
};
use super::cli_common;
use super::utils::filter_extensions_from_system_prompt;
use crate::agents::mcp_client::ExtensionProcessGroup;
use crate::config::search_path::SearchPaths;
use crate::config::{Config, GoslingMode};
use crate::conversation::message::{Message, MessageContent};
use crate::subprocess::configure_subprocess;

pub(crate) const MUSE_CODE_PROVIDER_NAME: &str = "muse-code";
const DEFAULT_MODEL: &str = "muse-spark-1.3";
const MAX_EVENT_BYTES: usize = 16 * 1024 * 1024;
const KNOWN_MODELS: &[&str] = &["muse-spark-1.3", "muse-wonder-1.0", "muse-spark-1.2"];

pub struct MuseCodeProvider {
    command: PathBuf,
    working_dir: PathBuf,
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

#[derive(Default)]
struct MuseOutput {
    run_id: Option<String>,
    terminal_text: Option<String>,
}

impl MuseOutput {
    fn push(&mut self, event: Value) -> Result<(), ProviderError> {
        let event_type = event["payload_type"].as_str().ok_or_else(|| {
            ProviderError::RequestFailed("Muse Code emitted an invalid event envelope".to_string())
        })?;
        if event["schema_version"].as_u64() != Some(1)
            || event["payload_schema_version"].as_u64() != Some(1)
        {
            return Err(ProviderError::RequestFailed(
                "Unsupported Muse Code event schema; update Gosling or use a compatible Muse CLI"
                    .to_string(),
            ));
        }
        let payload = &event["payload"];
        if event_type == "run.lifecycle.started" && self.run_id.is_none() {
            self.run_id = payload["run_stream"]["id"].as_str().map(str::to_string);
            if self.run_id.is_none() {
                return Err(ProviderError::RequestFailed(
                    "Muse Code started a run without an identity".to_string(),
                ));
            }
        }
        if !event_type.starts_with("run.terminal.") {
            return Ok(());
        }
        let run_id = self.run_id.as_deref().ok_or_else(|| {
            ProviderError::RequestFailed(
                "Muse Code returned a terminal before starting a run".to_string(),
            )
        })?;
        if payload["run_stream"]["id"].as_str() != Some(run_id) {
            return Ok(());
        }
        if event_type != "run.terminal.completed" || payload["terminal"] != "completed" {
            let reason = payload["reason"].as_str().unwrap_or("run did not complete");
            return Err(ProviderError::RequestFailed(format!(
                "Muse Code {event_type}: {}",
                redact_secrets(reason)
            )));
        }
        let text = payload["text"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .ok_or_else(|| {
                ProviderError::RequestFailed(
                    "Muse Code returned an empty terminal result".to_string(),
                )
            })?;
        if self.terminal_text.replace(text.to_string()).is_some() {
            return Err(ProviderError::RequestFailed(
                "Muse Code returned multiple terminal results".to_string(),
            ));
        }
        Ok(())
    }

    fn finish(self) -> Result<String, ProviderError> {
        self.terminal_text.ok_or_else(|| {
            ProviderError::RequestFailed(
                "Muse Code exited without a completed terminal result; run `muse login` and verify the CLI works in this folder".to_string(),
            )
        })
    }
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
            vec![],
        )
        .with_setup_steps(vec![
            "Install Muse Code using https://dev.meta.ai/docs/muse-code and confirm `muse --version` works",
            "Run `muse login` in a terminal and sign in to the account with your Muse Code subscription",
            "Verify Muse is using your subscription: a stored CLI API key takes priority over browser sign-in",
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
                command: resolved_muse_command()?,
                working_dir,
                mode: RwLock::new(mode),
            })
        })
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
        Ok(KNOWN_MODELS
            .iter()
            .map(|name| (*name).to_string())
            .collect())
    }

    async fn fetch_supported_model_info(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(Self::metadata().known_models)
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

        let prompt = build_prompt(system, messages)?;
        let prompt_file = write_prompt(&prompt)?;

        let mut command = Command::new(&self.command);
        configure_subprocess(&mut command);
        command
            .current_dir(&self.working_dir)
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
            .args(["--disable-approval", "--no-session-log"])
            .env_remove("META_API_KEY")
            .env_remove("MODEL_API_KEY")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(effort) = model.thinking_effort() {
            command.arg("--reasoning-effort").arg(
                gosling_providers::formats::meta::reasoning_effort(&model.model_name, effort),
            );
        }
        if let Ok(path) = SearchPaths::builder().path() {
            command.env("PATH", path);
        }
        let model_name = model.model_name.clone();

        Ok(Box::pin(try_stream! {
            let _prompt_file = prompt_file;
            let mut child = command.spawn().map_err(|error| ProviderError::RequestFailed(format!(
                "Cannot start Muse Code: {error}. Install `muse` or configure MUSE_CODE_COMMAND."
            )))?;
            let _process_group = ExtensionProcessGroup::new(child.id());
            let stdout = child.stdout.take().ok_or_else(|| ProviderError::RequestFailed("Muse stdout is unavailable".to_string()))?;
            let mut stderr = child.stderr.take().ok_or_else(|| ProviderError::RequestFailed("Muse stderr is unavailable".to_string()))?;
            let stderr_task = AbortOnDropHandle::new(tokio::spawn(async move {
                tokio::io::copy(&mut stderr, &mut tokio::io::sink()).await
            }));
            let mut lines = FramedRead::new(stdout, LinesCodec::new_with_max_length(MAX_EVENT_BYTES));
            let mut output = MuseOutput::default();
            while let Some(line) = tokio::time::timeout(Duration::from_secs(DEFAULT_PROVIDER_TIMEOUT_SECS), lines.next())
                .await.map_err(|_| ProviderError::RequestFailed("Muse Code stopped producing events".to_string()))?
            {
                let line = line.map_err(|error| ProviderError::RequestFailed(format!("Cannot read Muse Code event: {error}")))?;
                if line.trim().is_empty() {
                    continue;
                }
                let event = serde_json::from_str(&line).map_err(|error| ProviderError::RequestFailed(format!("Malformed Muse Code JSON event: {error}")))?;
                output.push(event)?;
            }
            let status = tokio::time::timeout(Duration::from_secs(10), child.wait())
                .await.map_err(|_| ProviderError::RequestFailed("Muse Code did not exit after closing stdout".to_string()))?
                .map_err(|error| ProviderError::RequestFailed(error.to_string()))?;
            if !status.success() {
                Err(ProviderError::RequestFailed(format!(
                    "Muse Code exited with status {status}. Run `muse login` and verify the CLI works in this folder."
                )))?;
            }
            drop(stderr_task);
            let text = output.finish()?;
            yield (Some(Message::assistant().with_text(text)), Some(ProviderUsage::new(model_name, Usage::default())));
        }))
    }
}
