//! ACP extension selection and client-supplied MCP server normalization.
//!
//! Maintainers: endpoint identity gates restoration of server-owned environment values.
//! Clients: this module preserves the compatibility facade's extension-selection behavior.

use super::*;

fn extract_timeout_from_meta(meta: &Option<Meta>) -> Option<u64> {
    meta.as_ref()
        .and_then(|m| m.get("timeout"))
        .and_then(|v| v.as_u64())
}

pub(super) fn mcp_server_to_extension_config(
    mcp_server: McpServer,
) -> Result<ExtensionConfig, String> {
    match mcp_server {
        McpServer::Stdio(stdio) => {
            let timeout = extract_timeout_from_meta(&stdio.meta);
            Ok(ExtensionConfig::Stdio {
                name: stdio.name,
                description: String::new(),
                cmd: stdio.command.to_string_lossy().to_string(),
                args: stdio.args,
                envs: Envs::new(stdio.env.into_iter().map(|e| (e.name, e.value)).collect()),
                env_keys: vec![],
                timeout,
                cwd: None,
                bundled: Some(false),
                available_tools: vec![],
            })
        }
        McpServer::Http(http) => {
            let timeout = extract_timeout_from_meta(&http.meta);
            Ok(ExtensionConfig::StreamableHttp {
                name: http.name,
                description: String::new(),
                uri: http.url,
                envs: Envs::default(),
                env_keys: vec![],
                headers: http
                    .headers
                    .into_iter()
                    .map(|h| (h.name, h.value))
                    .collect(),
                timeout,
                socket: None,
                client_id: None,
                client_secret_key: None,
                scopes: vec![],
                bundled: Some(false),
                available_tools: vec![],
            })
        }
        McpServer::Sse(_) => Err("SSE is unsupported, migrate to streamable_http".to_string()),
        _ => Err("Unknown MCP server type".to_string()),
    }
}

/// Restore config-stored plain `envs` (and `env_keys`) on a client-supplied
/// extension that matches a configured endpoint exactly.
///
/// The client-facing extension DTO intentionally strips plain `envs` so env
/// values never leave the server (see `config_to_gosling_extension`). But
/// clients echo those stripped extensions back at session creation, so
/// without this merge a session silently loses every configured environment
/// variable (e.g. a stdio server's `envs:` block in config.yaml). Binding the
/// merge to command/arguments or URI/headers/socket prevents a client from
/// redirecting stored secrets by reusing only the configured name. Values the
/// client did supply win on key collisions; stored `env_keys` are only adopted
/// when the client sent none.
pub(super) fn rehydrate_configured_envs(
    extension: &mut ExtensionConfig,
    configured: &[ExtensionConfig],
) {
    let Some(stored) = configured
        .iter()
        .find(|stored| same_extension_secret_destination(extension, stored))
    else {
        return;
    };
    match (extension, stored) {
        (
            ExtensionConfig::Stdio { envs, env_keys, .. },
            ExtensionConfig::Stdio {
                envs: stored_envs,
                env_keys: stored_keys,
                ..
            },
        )
        | (
            ExtensionConfig::StreamableHttp { envs, env_keys, .. },
            ExtensionConfig::StreamableHttp {
                envs: stored_envs,
                env_keys: stored_keys,
                ..
            },
        ) => {
            let mut merged = stored_envs.get_env();
            merged.extend(envs.get_env());
            *envs = Envs::new(merged);
            if env_keys.is_empty() {
                *env_keys = stored_keys.clone();
            }
        }
        _ => {}
    }
}

fn same_extension_secret_destination(
    candidate: &ExtensionConfig,
    stored: &ExtensionConfig,
) -> bool {
    match (candidate, stored) {
        (
            ExtensionConfig::Stdio {
                name, cmd, args, ..
            },
            ExtensionConfig::Stdio {
                name: stored_name,
                cmd: stored_cmd,
                args: stored_args,
                ..
            },
        ) => name == stored_name && cmd == stored_cmd && args == stored_args,
        (
            ExtensionConfig::StreamableHttp {
                name,
                uri,
                headers,
                socket,
                ..
            },
            ExtensionConfig::StreamableHttp {
                name: stored_name,
                uri: stored_uri,
                headers: stored_headers,
                socket: stored_socket,
                ..
            },
        ) => {
            name == stored_name
                && uri == stored_uri
                && headers == stored_headers
                && socket == stored_socket
        }
        _ => false,
    }
}

pub(crate) fn selected_builtin_extensions(
    config: &Config,
    builtins: &[String],
) -> Vec<ExtensionConfig> {
    let mut extensions = Vec::new();
    for builtin in builtins {
        let builtin_config = builtin_to_extension_config(builtin);
        if is_builtin_disabled_by_user(config, &builtin_config.name()) {
            continue;
        }
        push_or_replace_extension(&mut extensions, builtin_config);
    }
    extensions
}

pub(crate) fn apply_shell_extension_selection(
    extensions: &mut Vec<ExtensionConfig>,
    selections: Option<&[ShellExtensionSelection]>,
) {
    let Some(selections) = selections else {
        return;
    };
    extensions.retain(|extension| {
        selections
            .iter()
            .any(|selection| selection.name == extension.name())
    });
    for extension in extensions {
        let Some(selection) = selections
            .iter()
            .find(|selection| selection.name == extension.name())
        else {
            continue;
        };
        if let Some(tools) = &selection.available_tools {
            extension.set_available_tools(tools.clone());
        }
    }
}

pub(crate) fn push_or_replace_extension(
    extensions: &mut Vec<ExtensionConfig>,
    extension: ExtensionConfig,
) {
    let name = extension.name().to_string();
    if let Some(index) = extensions
        .iter()
        .position(|existing| existing.name() == name)
    {
        extensions.remove(index);
    }
    extensions.push(extension);
}

pub(super) fn builtin_to_extension_config(name: &str) -> ExtensionConfig {
    if let Some(def) = PLATFORM_EXTENSIONS.get(name) {
        ExtensionConfig::Platform {
            name: def.name.into(),
            description: def.description.into(),
            display_name: Some(def.display_name.into()),
            bundled: Some(true),
            available_tools: vec![],
        }
    } else {
        ExtensionConfig::Builtin {
            name: name.into(),
            display_name: None,
            timeout: None,
            bundled: Some(true),
            description: name.into(),
            available_tools: vec![],
        }
    }
}
