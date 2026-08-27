use anyhow::Result;
use futures::future::BoxFuture;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::acp::{
    resolved_extension_configs_to_mcp_servers, AcpProvider, AcpProviderConfig, ACP_CURRENT_MODEL,
};
use crate::config::search_path::SearchPaths;
use crate::config::{Config, GoslingMode};
use crate::providers::base::{
    current_working_dir, ProviderDef, ProviderDescriptor, ProviderMetadata,
};

pub(crate) const VIBE_ACP_PROVIDER_NAME: &str = "vibe-acp";
const VIBE_ACP_DOC_URL: &str = "https://docs.mistral.ai/vibe";
/// Console script shipped by the `mistral-vibe` package (`vibe-acp =
/// vibe.acp.entrypoint:main`). It speaks ACP over stdio; the plain `vibe`
/// binary is the interactive CLI and is not an ACP agent.
pub(crate) const VIBE_ACP_BINARY: &str = "vibe-acp";

pub struct VibeAcpProvider;

impl gosling_providers::base::ProviderDescriptor for VibeAcpProvider {
    fn metadata() -> ProviderMetadata {
        ProviderMetadata::new(
            VIBE_ACP_PROVIDER_NAME,
            "Mistral Vibe",
            "Use gosling with your Mistral Vibe subscription via the vibe-acp adapter.",
            ACP_CURRENT_MODEL,
            vec![],
            VIBE_ACP_DOC_URL,
            vec![],
        )
        .with_setup_steps(vec![
            "Install the Mistral Vibe CLI: `uv tool install mistral-vibe`",
            "Authenticate it once: `vibe --setup`",
            "Confirm the ACP adapter is on PATH: `vibe-acp --version`",
            "Add to your gosling config file (`~/.config/gosling/config.yaml` on macOS/Linux):\n  GOSLING_PROVIDER: vibe-acp\n  GOSLING_MODEL: current\n  vibe-acp_configured: true",
            "Restart gosling for changes to take effect",
        ])
        .with_model_selection_hint("Use the Mistral Vibe CLI to configure models")
    }
}

impl ProviderDef for VibeAcpProvider {
    type Provider = AcpProvider;
    const MANAGES_OWN_CONTEXT: bool = true;
    const EXECUTES_TOOLS_OUTSIDE_GOSLING: bool = true;

    fn from_env(
        extensions: Vec<crate::config::ExtensionConfig>,
        tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<AcpProvider>> {
        Self::from_env_with_working_dir(extensions, current_working_dir(), tls_config)
    }

    fn from_env_with_working_dir(
        extensions: Vec<crate::config::ExtensionConfig>,
        working_dir: PathBuf,
        _tls_config: Option<crate::providers::api_client::TlsConfig>,
    ) -> BoxFuture<'static, Result<AcpProvider>> {
        Box::pin(async move {
            let config = Config::global();
            let resolved_command = SearchPaths::builder().resolve(VIBE_ACP_BINARY)?;
            let gosling_mode = config.get_gosling_mode().unwrap_or_default();

            let provider_config = AcpProviderConfig {
                command: resolved_command,
                args: vec![],
                env: vec![],
                env_remove: vec![],
                work_dir: working_dir,
                mcp_servers: resolved_extension_configs_to_mcp_servers(&extensions).await,
                session_mode_id: Some(vibe_mode_mapping()[&gosling_mode].clone()),
                session_config_options: vec![],
                model_config_option_id: None,
                mode_mapping: vibe_mode_mapping(),
                notification_callback: None,
            };

            let metadata = Self::metadata();
            AcpProvider::connect(metadata.name, gosling_mode, provider_config).await
        })
    }
}

/// Maps Gosling's approval modes onto Vibe's session modes.
///
/// The ids and their meanings are Vibe's own, read from a live `vibe-acp`
/// `session/new` response rather than inferred:
///
/// | id | Vibe's description |
/// |---|---|
/// | `ask` | Requires approval for tool executions |
/// | `plan` | Read-only agent for exploration and planning |
/// | `accept-edits` | Auto-approves file edits only |
/// | `auto-approve` | Auto-approves all tool executions |
///
/// `SmartApprove` maps to `accept-edits` because that is Vibe's middle tier —
/// it clears low-risk edits and still prompts for execution — which is the
/// same shape as SmartApprove, and it is also Vibe's own default.
///
/// `Chat` maps to `plan`, the most restrictive mode Vibe offers, matching what
/// `copilot_acp` does for Chat. This is an approximation, not an equivalence,
/// and the gap is wider than "read-only" suggests: observed end to end, `plan`
/// declines to execute the request and instead writes a plan file under
/// `~/.vibe/plans/`, then asks the operator to leave plan mode. So a Gosling
/// session in Chat mode still causes Vibe to touch the filesystem, where Chat
/// would otherwise run no tools at all. Chat is usable for planning against
/// Vibe, but it is not a no-side-effects mode.
fn vibe_mode_mapping() -> HashMap<GoslingMode, String> {
    HashMap::from([
        (GoslingMode::Auto, "auto-approve".to_string()),
        (GoslingMode::Approve, "ask".to_string()),
        (GoslingMode::SmartApprove, "accept-edits".to_string()),
        (GoslingMode::Chat, "plan".to_string()),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_gosling_mode_maps_to_a_mode_vibe_advertises() {
        // Verified against a live `vibe-acp` session/new response.
        let advertised = ["ask", "plan", "accept-edits", "auto-approve"];
        let mapping = vibe_mode_mapping();
        for mode in [
            GoslingMode::Auto,
            GoslingMode::Approve,
            GoslingMode::SmartApprove,
            GoslingMode::Chat,
        ] {
            let mapped = mapping
                .get(&mode)
                .unwrap_or_else(|| panic!("{mode:?} has no Vibe mode"));
            assert!(
                advertised.contains(&mapped.as_str()),
                "{mode:?} maps to {mapped}, which vibe-acp does not advertise"
            );
        }
    }

    #[test]
    fn autonomous_and_restrictive_modes_are_not_confused() {
        let mapping = vibe_mode_mapping();
        assert_eq!(mapping[&GoslingMode::Auto], "auto-approve");
        assert_eq!(mapping[&GoslingMode::Chat], "plan");
        assert_ne!(mapping[&GoslingMode::Approve], "auto-approve");
        assert_ne!(mapping[&GoslingMode::SmartApprove], "auto-approve");
    }

    #[test]
    fn metadata_names_the_acp_adapter_not_the_interactive_cli() {
        let metadata = VibeAcpProvider::metadata();
        assert_eq!(metadata.name, VIBE_ACP_PROVIDER_NAME);
        assert_eq!(VIBE_ACP_BINARY, "vibe-acp");
    }
}
