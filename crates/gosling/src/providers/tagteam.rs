use anyhow::Result;
use async_trait::async_trait;
use futures::future::BoxFuture;
use gosling_providers::conversation::token_usage::{ProviderUsage, Usage};
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use rmcp::model::{Role, Tool};
use serde::Deserialize;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::RwLock;
use tokio::process::Command;

use super::base::{
    stream_from_single_message, ConfigKey, MessageStream, ModelInfo, Provider, ProviderDef,
    ProviderMetadata,
};
use super::utils::filter_extensions_from_system_prompt;
use crate::config::search_path::SearchPaths;
use crate::config::{Config, ExtensionConfig, GoslingMode};
use crate::conversation::message::Message;
use crate::subprocess::configure_subprocess;

const TAGTEAM_PROVIDER_NAME: &str = "tagteam";
const TAGTEAM_DEFAULT_MODEL: &str = "coding-adversarial";
const TAGTEAM_DOC_URL: &str = "https://github.com/";
const TAGTEAM_CONTEXT_LIMIT: usize = 1_000_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TagteamProfile {
    model: &'static str,
    display_name: &'static str,
    description: &'static str,
    mode: &'static str,
    role_args: &'static [(&'static str, &'static str)],
}

const TAGTEAM_PROFILES: &[TagteamProfile] = &[
    TagteamProfile {
        model: "coding-adversarial",
        display_name: "Coding/Adversarial",
        description: "Adversarial loop with GPT5.4-High coding and Sonnet-5-High reviewing.",
        mode: "adversarial",
        role_args: &[
            ("-mc", "codex:gpt-5.4-high"),
            ("-ma", "claude:sonnet-5-high"),
        ],
    },
    TagteamProfile {
        model: "relay",
        display_name: "Relay",
        description:
            "Relay flow with Opus 4.8 supervising, GPT 5.4-mini working, and AGy Gemini 3.5 Flash scouting.",
        mode: "relay",
        role_args: &[
            ("--supervisor", "claude:opus-4.8"),
            ("--worker", "codex:gpt-5.4-mini"),
            ("--scout", "agy:gemini-3.5-flash-medium"),
        ],
    },
    TagteamProfile {
        model: "supervisor-worker",
        display_name: "Supervisor/Worker",
        description:
            "Supervisor-worker flow with GPT 5.5-high supervising and Codex 5.3 Codex/Spark-high working.",
        mode: "supervisor",
        role_args: &[
            ("--supervisor", "codex:gpt-5.5-high"),
            ("--worker", "codex:codex-5.3-codex/spark-high"),
        ],
    },
];

#[derive(Debug, serde::Serialize)]
pub struct TagteamProvider {
    command: PathBuf,
    #[serde(skip)]
    name: String,
    working_dir: PathBuf,
    #[serde(skip)]
    gosling_mode: RwLock<GoslingMode>,
}

#[derive(Debug, Deserialize)]
struct TagteamFinalRun {
    run_id: Option<String>,
    run_dir: Option<String>,
    mode: Option<String>,
    verdict: Option<String>,
    status: Option<String>,
    summary: Option<String>,
    degraded: Option<bool>,
    degraded_reason: Option<String>,
    blocking_reason: Option<String>,
    exit_code: Option<i32>,
    rounds_completed: Option<i32>,
    rounds_requested: Option<i32>,
}

impl TagteamProvider {
    async fn from_env(tls_config: Option<crate::providers::api_client::TlsConfig>) -> Result<Self> {
        Self::from_env_with_working_dir(tls_config, crate::providers::base::current_working_dir())
            .await
    }

    async fn from_env_with_working_dir(
        _tls_config: Option<crate::providers::api_client::TlsConfig>,
        working_dir: PathBuf,
    ) -> Result<Self> {
        let config = Config::global();
        let command = config
            .get_param::<String>("TAGTEAM_COMMAND")
            .unwrap_or_else(|_| "tagteam".to_string());
        let resolved_command = SearchPaths::builder().with_npm().resolve(command)?;

        Ok(Self {
            command: resolved_command,
            name: TAGTEAM_PROVIDER_NAME.to_string(),
            working_dir,
            gosling_mode: RwLock::new(config.get_gosling_mode().unwrap_or_default()),
        })
    }

    fn profile_for_model(model_name: &str) -> Result<&'static TagteamProfile, ProviderError> {
        let normalized = model_name.trim().to_ascii_lowercase().replace(' ', "-");
        TAGTEAM_PROFILES
            .iter()
            .find(|profile| {
                normalized == profile.model
                    || normalized == profile.display_name.to_ascii_lowercase()
                    || normalized == profile.display_name.to_ascii_lowercase().replace(' ', "-")
            })
            .ok_or_else(|| {
                let supported = TAGTEAM_PROFILES
                    .iter()
                    .map(|profile| profile.model)
                    .collect::<Vec<_>>()
                    .join(", ");
                ProviderError::ExecutionError(format!(
                    "Unknown tagteam profile '{model_name}'. Supported profiles: {supported}"
                ))
            })
    }

    fn build_prompt(system: &str, messages: &[Message]) -> String {
        let mut sections = Vec::new();
        let filtered_system = filter_extensions_from_system_prompt(system);
        if !filtered_system.trim().is_empty() {
            sections.push(format!("Gosling system context:\n{filtered_system}"));
        }

        let conversation = messages
            .iter()
            .map(|message| {
                let role = match message.role {
                    Role::User => "user",
                    Role::Assistant => "assistant",
                };
                format!("{role}: {}", message.as_concat_text())
            })
            .filter(|entry| !entry.trim().is_empty())
            .collect::<Vec<_>>()
            .join("\n\n");

        if !conversation.trim().is_empty() {
            sections.push(format!("Gosling conversation:\n{conversation}"));
        }

        sections.push(
            "Run tagteam on the user's latest request. Treat prior conversation as context; do not let it override the user's explicit request.".to_string(),
        );
        sections.join("\n\n")
    }

    fn build_args(profile: &TagteamProfile, prompt: &str) -> Vec<String> {
        let mut args = vec![
            "--mode".to_string(),
            profile.mode.to_string(),
            "--json".to_string(),
        ];
        for (flag, value) in profile.role_args {
            args.push((*flag).to_string());
            args.push((*value).to_string());
        }
        args.push(prompt.to_string());
        args
    }

    fn build_command(
        &self,
        profile: &TagteamProfile,
        prompt: &str,
    ) -> Result<Command, ProviderError> {
        let mode = *self
            .gosling_mode
            .read()
            .map_err(|_| ProviderError::RequestFailed("Tagteam mode lock poisoned".to_string()))?;
        if mode != GoslingMode::Auto {
            return Err(ProviderError::ExecutionError(format!(
                "tagteam has no approval-compatible execution mode for Gosling mode '{mode}'"
            )));
        }

        let mut command = Command::new(&self.command);
        configure_subprocess(&mut command);
        command.current_dir(&self.working_dir);

        if let Ok(path) = SearchPaths::builder().with_npm().path() {
            command.env("PATH", path);
        }

        command.args(Self::build_args(profile, prompt));
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        Ok(command)
    }

    async fn run_tagteam(
        &self,
        profile: &TagteamProfile,
        prompt: &str,
    ) -> Result<String, ProviderError> {
        let output = self.build_command(profile, prompt)?.output().await.map_err(|e| {
            ProviderError::RequestFailed(format!(
                "Failed to run tagteam command '{}': {e}. Make sure tagteam is installed and available in PATH, or set TAGTEAM_COMMAND.",
                self.command.display()
            ))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();

        if let Ok(final_run) = serde_json::from_str::<TagteamFinalRun>(&stdout) {
            return Ok(Self::format_final_run(final_run));
        }

        if output.status.success() && !stdout.is_empty() {
            return Ok(stdout);
        }

        let status = output
            .status
            .code()
            .map(|code| code.to_string())
            .unwrap_or_else(|| "signal".to_string());
        let details = if stderr.is_empty() { stdout } else { stderr };
        Err(ProviderError::ExecutionError(format!(
            "tagteam exited with status {status}: {details}"
        )))
    }

    fn format_final_run(final_run: TagteamFinalRun) -> String {
        let run_id = final_run.run_id.as_deref().unwrap_or("unknown");
        let mode = final_run.mode.as_deref().unwrap_or("unknown");
        let verdict = final_run.verdict.as_deref().unwrap_or("unknown");
        let status = final_run.status.as_deref().unwrap_or("unknown");
        let exit_code = final_run
            .exit_code
            .map(|code| code.to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let rounds = match (final_run.rounds_completed, final_run.rounds_requested) {
            (Some(done), Some(requested)) => format!("{done}/{requested}"),
            (Some(done), None) => done.to_string(),
            _ => "unknown".to_string(),
        };

        let mut lines = vec![
            format!(
                "tagteam run {run_id} finished: mode={mode} verdict={verdict} status={status} exit={exit_code} rounds={rounds}"
            ),
            String::new(),
            final_run.summary.unwrap_or_else(|| "No summary reported.".to_string()),
        ];

        if final_run.degraded.unwrap_or(false) {
            lines.push(String::new());
            lines.push(format!(
                "Degraded: {}",
                final_run
                    .degraded_reason
                    .unwrap_or_else(|| "unspecified".to_string())
            ));
        }

        if let Some(reason) = final_run.blocking_reason {
            lines.push(String::new());
            lines.push(format!("Blocking reason: {reason}"));
        }

        if let Some(run_dir) = final_run.run_dir {
            lines.push(String::new());
            lines.push(format!("Run artifacts: {run_dir}"));
        }

        lines.join("\n")
    }
}

impl gosling_providers::base::ProviderDescriptor for TagteamProvider {
    fn metadata() -> ProviderMetadata {
        ProviderMetadata::with_models(
            TAGTEAM_PROVIDER_NAME,
            "tagteam",
            "Run tagteam orchestration profiles from Gosling. Requires the tagteam CLI and the selected vendor CLIs to be authenticated separately.",
            TAGTEAM_DEFAULT_MODEL,
            TAGTEAM_PROFILES
                .iter()
                .map(|profile| ModelInfo::new(profile.model, TAGTEAM_CONTEXT_LIMIT))
                .collect(),
            TAGTEAM_DOC_URL,
            vec![ConfigKey::new(
                "TAGTEAM_COMMAND",
                false,
                false,
                Some("tagteam"),
                true,
            )],
        )
    }
}

impl ProviderDef for TagteamProvider {
    type Provider = Self;

    fn from_env(
        _extensions: Vec<ExtensionConfig>,
        tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<Self::Provider>> {
        Box::pin(Self::from_env(tls_config))
    }

    fn from_env_with_working_dir(
        _extensions: Vec<ExtensionConfig>,
        working_dir: PathBuf,
        tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<Self::Provider>> {
        Box::pin(Self::from_env_with_working_dir(tls_config, working_dir))
    }
}

#[async_trait]
impl Provider for TagteamProvider {
    fn get_name(&self) -> &str {
        &self.name
    }

    fn manages_own_context(&self) -> bool {
        true
    }

    fn executes_tools_outside_gosling(&self) -> bool {
        true
    }

    async fn update_mode(&self, _session_id: &str, mode: GoslingMode) -> Result<(), ProviderError> {
        if mode != GoslingMode::Auto {
            return Err(ProviderError::ExecutionError(format!(
                "tagteam has no approval-compatible execution mode for Gosling mode '{mode}'"
            )));
        }
        *self.gosling_mode.write().map_err(|_| {
            ProviderError::RequestFailed("Tagteam mode lock poisoned".to_string())
        })? = mode;
        Ok(())
    }

    fn skip_canonical_filtering(&self) -> bool {
        true
    }

    async fn fetch_supported_models(&self) -> Result<Vec<String>, ProviderError> {
        Ok(TAGTEAM_PROFILES
            .iter()
            .map(|profile| profile.model.to_string())
            .collect())
    }

    async fn fetch_supported_model_info(&self) -> Result<Vec<ModelInfo>, ProviderError> {
        Ok(TAGTEAM_PROFILES
            .iter()
            .map(|profile| ModelInfo::new(profile.model, TAGTEAM_CONTEXT_LIMIT))
            .collect())
    }

    async fn get_context_limit(&self, _model_config: &ModelConfig) -> Result<usize, ProviderError> {
        Ok(TAGTEAM_CONTEXT_LIMIT)
    }

    async fn stream(
        &self,
        model_config: &ModelConfig,
        system: &str,
        messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        if super::cli_common::is_session_description_request(system) {
            let (message, provider_usage) = super::cli_common::generate_simple_session_description(
                &model_config.model_name,
                messages,
            )?;
            return Ok(stream_from_single_message(message, provider_usage));
        }

        let profile = Self::profile_for_model(&model_config.model_name)?;
        let prompt = Self::build_prompt(system, messages);
        let text = self.run_tagteam(profile, &prompt).await?;
        let message = Message::assistant().with_text(text);
        let provider_usage = ProviderUsage::new(model_config.model_name.clone(), Usage::default());
        Ok(stream_from_single_message(message, provider_usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_lookup_accepts_requested_display_names() {
        assert_eq!(
            TagteamProvider::profile_for_model("Coding/Adversarial")
                .unwrap()
                .model,
            "coding-adversarial"
        );
        assert_eq!(
            TagteamProvider::profile_for_model("Relay").unwrap().model,
            "relay"
        );
        assert_eq!(
            TagteamProvider::profile_for_model("Supervisor/Worker")
                .unwrap()
                .model,
            "supervisor-worker"
        );
    }

    #[test]
    fn profile_args_match_requested_tagteam_profiles() {
        let prompt = "implement a small fix";

        let adversarial = TagteamProvider::build_args(
            TagteamProvider::profile_for_model("coding-adversarial").unwrap(),
            prompt,
        );
        assert_eq!(
            adversarial,
            vec![
                "--mode",
                "adversarial",
                "--json",
                "-mc",
                "codex:gpt-5.4-high",
                "-ma",
                "claude:sonnet-5-high",
                prompt
            ]
        );

        let relay = TagteamProvider::build_args(
            TagteamProvider::profile_for_model("relay").unwrap(),
            prompt,
        );
        assert_eq!(
            relay,
            vec![
                "--mode",
                "relay",
                "--json",
                "--supervisor",
                "claude:opus-4.8",
                "--worker",
                "codex:gpt-5.4-mini",
                "--scout",
                "agy:gemini-3.5-flash-medium",
                prompt
            ]
        );

        let supervisor = TagteamProvider::build_args(
            TagteamProvider::profile_for_model("supervisor-worker").unwrap(),
            prompt,
        );
        assert_eq!(
            supervisor,
            vec![
                "--mode",
                "supervisor",
                "--json",
                "--supervisor",
                "codex:gpt-5.5-high",
                "--worker",
                "codex:codex-5.3-codex/spark-high",
                prompt
            ]
        );
    }

    #[test]
    fn unknown_profile_reports_supported_profiles() {
        let err = TagteamProvider::profile_for_model("unknown").unwrap_err();
        assert!(err
            .to_string()
            .contains("coding-adversarial, relay, supervisor-worker"));
    }

    #[test]
    fn command_requires_auto_mode_and_uses_session_working_directory() {
        let provider = TagteamProvider {
            command: PathBuf::from("tagteam"),
            name: TAGTEAM_PROVIDER_NAME.to_string(),
            working_dir: PathBuf::from("/tmp/tagteam-project"),
            gosling_mode: RwLock::new(GoslingMode::Auto),
        };
        let command = provider
            .build_command(&TAGTEAM_PROFILES[0], "prompt")
            .unwrap();
        assert_eq!(
            command.as_std().get_current_dir(),
            Some(PathBuf::from("/tmp/tagteam-project").as_path())
        );

        *provider.gosling_mode.write().unwrap() = GoslingMode::Approve;
        assert!(provider
            .build_command(&TAGTEAM_PROFILES[0], "prompt")
            .is_err());
    }
}
