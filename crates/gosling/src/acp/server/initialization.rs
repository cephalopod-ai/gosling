//! ACP client capability negotiation and initialize response construction.
//!
//! Maintainers: derive advertised custom methods from the canonical schema registry.
//! Clients: initialization preserves capability, metadata, and notification negotiation.

use super::*;

#[derive(Debug, Default, Deserialize)]
pub(super) struct ClientCapabilitiesMeta {
    #[serde(default)]
    pub(super) gosling: Option<GoslingClientCapabilities>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct GoslingClientCapabilities {
    #[serde(rename = "mcpHostCapabilities", default)]
    pub(super) mcp_host_capabilities: Option<GoslingMcpHostCapabilities>,
    #[serde(rename = "customNotifications", default)]
    pub(super) custom_notifications: Option<bool>,
}

#[derive(Debug, Default, Deserialize)]
pub(super) struct GoslingMcpHostCapabilities {
    #[serde(default)]
    pub(super) extensions: Option<rmcp::model::ExtensionCapabilities>,
}

pub(super) fn extract_client_capabilities_meta(
    args: &InitializeRequest,
) -> Option<ClientCapabilitiesMeta> {
    args.client_capabilities
        .meta
        .as_ref()
        .and_then(|meta| serde_json::from_value(serde_json::Value::Object(meta.clone())).ok())
}

fn extract_client_mcp_host_info(
    args: &InitializeRequest,
    gosling_client_capabilities: Option<&GoslingClientCapabilities>,
) -> GoslingMcpHostInfo {
    let host_capabilities =
        gosling_client_capabilities.and_then(|gosling| gosling.mcp_host_capabilities.as_ref());
    let explicit_extensions = host_capabilities
        .as_ref()
        .and_then(|capabilities| capabilities.extensions.as_ref())
        .is_some();
    let extensions = host_capabilities
        .and_then(|capabilities| capabilities.extensions.clone())
        .unwrap_or_default();

    GoslingMcpHostInfo {
        explicit_extensions,
        extensions,
        client_name: args.client_info.as_ref().map(|info| info.name.clone()),
        client_version: args.client_info.as_ref().map(|info| info.version.clone()),
    }
}

fn extract_use_login_shell_path(args: &InitializeRequest) -> bool {
    args.meta
        .as_ref()
        .and_then(|meta| meta.get("gosling/useLoginShellPath"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub(super) fn extract_client_supports_gosling_custom_notifications(
    gosling_client_capabilities: Option<&GoslingClientCapabilities>,
) -> bool {
    gosling_client_capabilities
        .and_then(|gosling| gosling.custom_notifications)
        .unwrap_or(false)
}

pub(super) fn custom_method_names() -> Vec<String> {
    let mut methods =
        GoslingAcpAgent::custom_method_schemas(&mut schemars::SchemaGenerator::default())
            .into_iter()
            .map(|schema| schema.method)
            .collect::<Vec<_>>();
    methods.sort();
    methods
}

fn shell_capabilities_meta(shell_runtime: &ShellRuntime) -> Meta {
    let provisioning = shell_runtime.provisioning();
    serde_json::Map::from_iter([(
        "goslingShell".to_string(),
        serde_json::json!({
            "schemaVersion": provisioning.schema_version,
            "identity": &provisioning.identity,
            "authorityMode": provisioning.protocol_policy.mode,
            "settingsAuthority": &provisioning.settings_authority,
            "provisioningMethod": "_gosling/unstable/shell/provisioning/read",
            "availableMethods": custom_method_names(),
            "domainAdapter": &provisioning.domain_adapter,
        }),
    )])
}

impl GoslingAcpAgent {
    fn spawn_domain_adapter_status_notifier(&self) {
        if !self.supports_gosling_custom_notifications() {
            return;
        }
        let Some(mut status) = self.shell_runtime.subscribe_domain_adapter_status() else {
            return;
        };
        let Some(cx) = self.client_cx.get().cloned() else {
            return;
        };
        tokio::spawn(async move {
            while status.changed().await.is_ok() {
                if cx
                    .send_notification(DomainStatusNotification {
                        status: *status.borrow(),
                    })
                    .is_err()
                {
                    return;
                }
            }
        });
    }

    pub(super) async fn on_initialize(
        &self,
        args: InitializeRequest,
    ) -> Result<InitializeResponse, agent_client_protocol::Error> {
        debug!(?args, "initialize request");

        let protocol_version = negotiate_protocol_version(args.protocol_version)?;

        let _ = self
            .client_fs_capabilities
            .set(args.client_capabilities.fs.clone());
        let _ = self.client_terminal.set(args.client_capabilities.terminal);
        let gosling_client_capabilities =
            extract_client_capabilities_meta(&args).and_then(|meta| meta.gosling);
        let _ = self.client_mcp_host_info.set(extract_client_mcp_host_info(
            &args,
            gosling_client_capabilities.as_ref(),
        ));
        let _ = self.client_supports_gosling_custom_notifications.set(
            extract_client_supports_gosling_custom_notifications(
                gosling_client_capabilities.as_ref(),
            ),
        );
        let _ = self
            .client_supports_acp_elicitation
            .set(elicitation::client_supports_form_elicitation(&args));
        let _ = self
            .use_login_shell_path
            .set(extract_use_login_shell_path(&args));

        let capabilities = AgentCapabilities::new()
            .load_session(true)
            .session_capabilities(
                SessionCapabilities::new()
                    .list(SessionListCapabilities::new())
                    .close(SessionCloseCapabilities::new()),
            )
            .prompt_capabilities(
                PromptCapabilities::new()
                    .image(true)
                    .audio(false)
                    .embedded_context(true),
            )
            .mcp_capabilities(McpCapabilities::new().http(true))
            .meta(Some(shell_capabilities_meta(&self.shell_runtime)));
        self.spawn_domain_adapter_status_notifier();
        Ok(InitializeResponse::new(protocol_version)
            .agent_info(Implementation::new("gosling", env!("CARGO_PKG_VERSION")))
            .agent_capabilities(capabilities)
            .auth_methods(vec![AuthMethod::Agent(
                AuthMethodAgent::new("gosling-provider", "Configure Provider")
                    .description("Run `gosling configure` to set up your AI provider and API key"),
            )]))
    }
}
