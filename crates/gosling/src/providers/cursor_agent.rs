use anyhow::Result;
use async_trait::async_trait;
use rmcp::model::Role;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::RwLock;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, BufReader};
use tokio::process::Command;

use super::base::{
    stream_from_single_message, ConfigKey, MessageStream, Provider, ProviderDef, ProviderMetadata,
};
use super::utils::filter_extensions_from_system_prompt;
use crate::config::search_path::SearchPaths;
use crate::config::GoslingMode;
use crate::conversation::message::{Message, MessageContent};
use crate::subprocess::configure_subprocess;
use futures::future::BoxFuture;
use gosling_providers::conversation::token_usage::{ProviderUsage, Usage};
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use gosling_providers::request_log::{start_log, LoggerHandleExt};
use rmcp::model::Tool;

const CURSOR_AGENT_PROVIDER_NAME: &str = "cursor-agent";
const MAX_CURSOR_STDERR_CAPTURE: usize = 64 * 1024;
pub const CURSOR_AGENT_DEFAULT_MODEL: &str = "auto";
pub const CURSOR_AGENT_KNOWN_MODELS: &[&str] = &["auto", "composer-2", "composer-2-fast"];

pub const CURSOR_AGENT_DOC_URL: &str = "https://docs.cursor.com/en/cli/overview";

async fn read_nonempty_lines(reader: impl AsyncRead + Unpin) -> std::io::Result<Vec<String>> {
    let mut reader = BufReader::new(reader);
    let mut lines = Vec::new();
    let mut line = String::new();
    loop {
        line.clear();
        if reader.read_line(&mut line).await? == 0 {
            return Ok(lines);
        }
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            lines.push(trimmed.to_string());
        }
    }
}

async fn read_bounded_tail(mut reader: impl AsyncRead + Unpin) -> std::io::Result<String> {
    let mut captured = Vec::new();
    let mut chunk = [0u8; 8192];
    loop {
        let read = reader.read(&mut chunk).await?;
        if read == 0 {
            return Ok(String::from_utf8_lossy(&captured).into_owned());
        }
        captured.extend_from_slice(&chunk[..read]);
        if captured.len() > MAX_CURSOR_STDERR_CAPTURE {
            let excess = captured.len() - MAX_CURSOR_STDERR_CAPTURE;
            captured.drain(..excess);
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct CursorAgentProvider {
    command: PathBuf,
    #[serde(skip)]
    name: String,
    working_dir: PathBuf,
    #[serde(skip)]
    gosling_mode: RwLock<GoslingMode>,
}

impl CursorAgentProvider {
    pub async fn from_env(
        tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> Result<Self> {
        Self::from_env_with_working_dir(tls_config, crate::providers::base::current_working_dir())
            .await
    }

    async fn from_env_with_working_dir(
        _tls_config: Option<crate::providers::api_client::TlsConfig>,
        working_dir: PathBuf,
    ) -> Result<Self> {
        let config = crate::config::Config::global();
        let command: String = config.get_cursor_agent_command().unwrap_or_default().into();
        let resolved_command = SearchPaths::builder().with_npm().resolve(&command)?;

        Ok(Self {
            command: resolved_command,
            name: CURSOR_AGENT_PROVIDER_NAME.to_string(),
            working_dir,
            gosling_mode: RwLock::new(config.get_gosling_mode().unwrap_or_default()),
        })
    }

    /// Get authentication status from cursor-agent
    async fn get_authentication_status(&self) -> bool {
        Command::new(&self.command)
            .arg("status")
            .current_dir(&self.working_dir)
            .output()
            .await
            .ok()
            .map(|output| String::from_utf8_lossy(&output.stdout).contains("✓ Logged in as"))
            .unwrap_or(false)
    }

    /// Convert gosling messages to a simple prompt format for cursor-agent CLI
    fn messages_to_cursor_agent_format(&self, system: &str, messages: &[Message]) -> String {
        let mut full_prompt = String::new();

        let filtered_system = filter_extensions_from_system_prompt(system);
        full_prompt.push_str(&filtered_system);
        full_prompt.push_str("\n\n");

        // Add conversation history
        for message in messages {
            let role_prefix = match message.role {
                Role::User => "Human: ",
                Role::Assistant => "Assistant: ",
            };
            full_prompt.push_str(role_prefix);

            for content in &message.content {
                match content {
                    MessageContent::Text(text_content) => {
                        full_prompt.push_str(&text_content.text);
                        full_prompt.push('\n');
                    }
                    MessageContent::ToolRequest(tool_request) => {
                        if let Ok(tool_call) = &tool_request.tool_call {
                            full_prompt.push_str(&format!(
                                "Tool Use: {} with args: {:?}\n",
                                tool_call.name, tool_call.arguments
                            ));
                        }
                    }
                    MessageContent::ToolResponse(tool_response) => {
                        if let Ok(result) = &tool_response.tool_result {
                            let content_text = result
                                .content
                                .iter()
                                .filter_map(|content| match &content.raw {
                                    rmcp::model::RawContent::Text(text_content) => {
                                        Some(text_content.text.as_str())
                                    }
                                    _ => None,
                                })
                                .collect::<Vec<&str>>()
                                .join("\n");

                            full_prompt.push_str(&format!("Tool Result: {}\n", content_text));
                        }
                    }
                    _ => {
                        // Skip other content types for now
                    }
                }
            }
            full_prompt.push('\n');
        }

        full_prompt.push_str("Assistant: ");
        full_prompt
    }

    /// Parse the JSON response from cursor-agent CLI
    fn parse_cursor_agent_response(
        &self,
        lines: &[String],
    ) -> Result<(Message, Usage), ProviderError> {
        let mut result_text = None;
        for line in lines {
            let json_value = serde_json::from_str::<Value>(line).map_err(|error| {
                ProviderError::RequestFailed(format!(
                    "Malformed JSON from cursor-agent CLI: {error}"
                ))
            })?;
            match json_value.get("type").and_then(Value::as_str) {
                Some("error") => {
                    let message = json_value
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("cursor-agent reported an error");
                    return Err(ProviderError::RequestFailed(format!(
                        "cursor-agent CLI error: {message}"
                    )));
                }
                Some("result") => {
                    let result = json_value
                        .get("result")
                        .and_then(Value::as_str)
                        .filter(|result| !result.trim().is_empty());
                    if json_value
                        .get("is_error")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                    {
                        return Err(ProviderError::RequestFailed(format!(
                            "cursor-agent CLI error: {}",
                            result.unwrap_or("cursor-agent returned an error response")
                        )));
                    }
                    let result = result.ok_or_else(|| {
                        ProviderError::RequestFailed(
                            "cursor-agent returned an empty result".to_string(),
                        )
                    })?;
                    if result_text.replace(result.to_string()).is_some() {
                        return Err(ProviderError::RequestFailed(
                            "cursor-agent returned multiple terminal results".to_string(),
                        ));
                    }
                }
                _ => {}
            }
        }

        let response_text = result_text.ok_or_else(|| {
            ProviderError::RequestFailed("cursor-agent returned no terminal result".to_string())
        })?;

        let message_content = vec![MessageContent::text(response_text)];
        let response_message = Message::new(
            Role::Assistant,
            chrono::Utc::now().timestamp(),
            message_content,
        );
        Ok((response_message, Usage::default()))
    }

    fn apply_permission_flags(
        command: &mut Command,
        mode: GoslingMode,
    ) -> Result<(), ProviderError> {
        match mode {
            GoslingMode::Auto => {
                command.arg("--force");
                Ok(())
            }
            GoslingMode::SmartApprove | GoslingMode::Approve | GoslingMode::Chat => {
                Err(ProviderError::ExecutionError(format!(
                    "cursor-agent cannot route Gosling mode '{mode}' approvals in headless mode"
                )))
            }
        }
    }

    fn build_command(&self, model: &ModelConfig, prompt: &str) -> Result<Command, ProviderError> {
        let mut command = Command::new(&self.command);
        configure_subprocess(&mut command);
        command.current_dir(&self.working_dir);

        if let Ok(path) = SearchPaths::builder().with_npm().path() {
            command.env("PATH", path);
        }

        command
            .arg("--model")
            .arg(&model.model_name)
            .arg("-p")
            .arg(prompt)
            .arg("--output-format")
            .arg("json");

        let gosling_mode = *self
            .gosling_mode
            .read()
            .map_err(|_| ProviderError::RequestFailed("Cursor mode lock poisoned".to_string()))?;
        Self::apply_permission_flags(&mut command, gosling_mode)?;
        command.stdout(Stdio::piped()).stderr(Stdio::piped());
        Ok(command)
    }

    async fn execute_command(
        &self,
        model: &ModelConfig,
        system: &str,
        messages: &[Message],
        _tools: &[Tool],
    ) -> Result<Vec<String>, ProviderError> {
        let prompt = self.messages_to_cursor_agent_format(system, messages);

        if std::env::var("GOSLING_CURSOR_AGENT_DEBUG").is_ok() {
            println!("=== CURSOR AGENT PROVIDER DEBUG ===");
            println!("Command: {:?}", self.command);
            println!("Original system prompt length: {} chars", system.len());
            println!(
                "Filtered system prompt length: {} chars",
                filter_extensions_from_system_prompt(system).len()
            );
            println!("Full prompt: {}", prompt);
            println!("Model: {}", model.model_name);
            println!("================================");
        }

        let mut cmd = self.build_command(model, &prompt)?;

        let mut child = cmd
                .spawn()
                .map_err(|e| ProviderError::RequestFailed(format!(
                    "Failed to spawn cursor-agent CLI command '{:?}': {}. \
                    Make sure the cursor-agent CLI is installed and available in the configured search paths, or set CURSOR_AGENT_COMMAND in your config to the correct path.",
                    self.command, e
                )))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| ProviderError::RequestFailed("Failed to capture stdout".to_string()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| ProviderError::RequestFailed("Failed to capture stderr".to_string()))?;
        let stdout_task = tokio::spawn(read_nonempty_lines(stdout));
        let stderr_task = tokio::spawn(read_bounded_tail(stderr));

        let exit_status = child.wait().await.map_err(|e| {
            ProviderError::RequestFailed(format!("Failed to wait for command: {}", e))
        })?;
        let lines = stdout_task
            .await
            .map_err(|e| ProviderError::RequestFailed(format!("stdout task failed: {e}")))?
            .map_err(|e| ProviderError::RequestFailed(format!("Failed to read stdout: {e}")))?;
        let stderr = stderr_task
            .await
            .map_err(|e| ProviderError::RequestFailed(format!("stderr task failed: {e}")))?
            .map_err(|e| ProviderError::RequestFailed(format!("Failed to read stderr: {e}")))?;

        if !exit_status.success() {
            if !self.get_authentication_status().await {
                return Err(ProviderError::Authentication(
                    "You are not logged in to cursor-agent. Please run 'cursor-agent login' to authenticate first."
                        .to_string()));
            }
            let stderr = stderr.trim();
            let detail = if stderr.is_empty() {
                String::new()
            } else {
                format!(": {stderr}")
            };
            return Err(ProviderError::RequestFailed(format!(
                "Command failed with exit code: {:?}{detail}",
                exit_status.code(),
            )));
        }

        tracing::debug!("Command executed successfully, got {} lines", lines.len());
        for (i, line) in lines.iter().enumerate() {
            tracing::debug!("Line {}: {}", i, line);
        }

        Ok(lines)
    }
}

impl gosling_providers::base::ProviderDescriptor for CursorAgentProvider {
    fn metadata() -> ProviderMetadata {
        ProviderMetadata::new(
            CURSOR_AGENT_PROVIDER_NAME,
            "Cursor Agent",
            "Execute AI models via cursor-agent CLI tool",
            CURSOR_AGENT_DEFAULT_MODEL,
            CURSOR_AGENT_KNOWN_MODELS.to_vec(),
            CURSOR_AGENT_DOC_URL,
            vec![ConfigKey::new(
                "CURSOR_AGENT_COMMAND",
                true,
                false,
                Some("cursor-agent"),
                true,
            )],
        )
    }
}

impl ProviderDef for CursorAgentProvider {
    type Provider = Self;

    fn from_env(
        _extensions: Vec<crate::config::ExtensionConfig>,
        tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<Self::Provider>> {
        Box::pin(Self::from_env(tls_config))
    }

    fn from_env_with_working_dir(
        _extensions: Vec<crate::config::ExtensionConfig>,
        working_dir: PathBuf,
        tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<Self::Provider>> {
        Box::pin(Self::from_env_with_working_dir(tls_config, working_dir))
    }
}

#[async_trait]
impl Provider for CursorAgentProvider {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn executes_tools_outside_gosling(&self) -> bool {
        true
    }

    async fn update_mode(&self, _session_id: &str, mode: GoslingMode) -> Result<(), ProviderError> {
        if mode != GoslingMode::Auto {
            return Err(ProviderError::ExecutionError(format!(
                "cursor-agent cannot route Gosling mode '{mode}' approvals in headless mode"
            )));
        }
        *self
            .gosling_mode
            .write()
            .map_err(|_| ProviderError::RequestFailed("Cursor mode lock poisoned".to_string()))? =
            mode;
        Ok(())
    }

    async fn fetch_supported_models(&self) -> Result<Vec<String>, ProviderError> {
        Ok(CURSOR_AGENT_KNOWN_MODELS
            .iter()
            .map(|s| s.to_string())
            .collect())
    }

    async fn stream(
        &self,
        model_config: &ModelConfig,
        system: &str,
        messages: &[Message],
        tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        if super::cli_common::is_session_description_request(system) {
            let (message, provider_usage) = super::cli_common::generate_simple_session_description(
                &model_config.model_name,
                messages,
            )?;
            return Ok(stream_from_single_message(message, provider_usage));
        }

        let lines = self
            .execute_command(model_config, system, messages, tools)
            .await?;

        let (message, usage) = self.parse_cursor_agent_response(&lines)?;

        // Create a dummy payload for debug tracing
        let payload = json!({
            "command": self.command,
            "model": model_config.model_name,
            "system": system,
            "messages": messages.len()
        });

        let response = json!({
            "lines": lines.len(),
            "usage": usage
        });

        let mut log = start_log(model_config, &payload)?;
        log.write(&response, Some(&usage))?;

        let provider_usage = ProviderUsage::new(model_config.model_name.clone(), usage);
        Ok(stream_from_single_message(message, provider_usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider(command: PathBuf, working_dir: PathBuf) -> CursorAgentProvider {
        CursorAgentProvider {
            command,
            name: CURSOR_AGENT_PROVIDER_NAME.to_string(),
            working_dir,
            gosling_mode: RwLock::new(GoslingMode::Auto),
        }
    }

    #[test]
    fn parser_requires_successful_nonempty_terminal_result() {
        let provider = provider(PathBuf::from("cursor-agent"), PathBuf::from("/tmp"));

        for lines in [
            vec![r#"{"type":"result","is_error":true,"result":"boom"}"#.to_string()],
            vec![r#"{"type":"result","result":""}"#.to_string()],
            vec![r#"{"type":"system","message":"started"}"#.to_string()],
            vec!["not json".to_string()],
            Vec::new(),
        ] {
            assert!(provider.parse_cursor_agent_response(&lines).is_err());
        }
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stderr_flood_does_not_block_successful_command() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let command = directory.path().join("cursor-agent");
        std::fs::write(
            &command,
            concat!(
                "#!/bin/sh\n",
                "i=0\n",
                "while [ \"$i\" -lt 3000 ]; do\n",
                "  printf 'stderr-flood-012345678901234567890123456789\\n' >&2\n",
                "  i=$((i + 1))\n",
                "done\n",
                "printf '%s\\n' '{\"type\":\"result\",\"result\":\"done\"}'\n",
            ),
        )
        .unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
        let provider = provider(command, directory.path().to_path_buf());

        let lines = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            provider.execute_command(&ModelConfig::new("auto"), "system", &[], &[]),
        )
        .await
        .unwrap()
        .unwrap();

        assert!(provider.parse_cursor_agent_response(&lines).is_ok());
    }

    #[test]
    fn permission_flags_are_mode_sensitive_and_fail_closed_for_non_auto_modes() {
        for mode in [
            GoslingMode::SmartApprove,
            GoslingMode::Approve,
            GoslingMode::Chat,
        ] {
            let mut command = Command::new("cursor-agent");
            assert!(CursorAgentProvider::apply_permission_flags(&mut command, mode).is_err());
            assert!(!command.as_std().get_args().any(|arg| arg == "--force"));
        }

        let mut automatic = Command::new("cursor-agent");
        CursorAgentProvider::apply_permission_flags(&mut automatic, GoslingMode::Auto).unwrap();
        assert!(automatic.as_std().get_args().any(|arg| arg == "--force"));
    }

    #[test]
    fn command_uses_session_working_directory() {
        let provider = provider(
            PathBuf::from("cursor-agent"),
            PathBuf::from("/tmp/cursor-project"),
        );
        let command = provider
            .build_command(&ModelConfig::new("auto"), "prompt")
            .unwrap();
        assert_eq!(
            command.as_std().get_current_dir(),
            Some(PathBuf::from("/tmp/cursor-project").as_path())
        );
    }
}
