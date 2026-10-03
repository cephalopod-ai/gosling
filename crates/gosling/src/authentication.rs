//! Host-owned authentication references. Session state is a launch snapshot;
//! workspace edits do not change credentials beneath an existing chat.

use crate::session::ExtensionData;
use anyhow::{bail, Result};
pub use gosling_sdk_types::authentication::*;

pub const SESSION_AUTHENTICATION_KEY: &str = "authentication.v1";
const SECRET_PREFIX: &str = "scoped-authentication::";

pub fn session_authentication(data: &ExtensionData) -> Result<AuthenticationSettings> {
    let settings = data
        .extension_states
        .get(SESSION_AUTHENTICATION_KEY)
        .map(|value| serde_json::from_value(value.clone()))
        .transpose()
        .map(|value| value.unwrap_or_default())
        .map_err(anyhow::Error::from)?;
    validate_settings(&settings)?;
    Ok(settings)
}

pub(crate) fn validate_settings(settings: &AuthenticationSettings) -> Result<()> {
    if settings.extensions.len() > 64 {
        bail!("too many extension authentication bindings");
    }
    for (name, binding) in &settings.extensions {
        if name.is_empty()
            || name.len() > 256
            || name != &crate::config::extensions::name_to_key(name)
        {
            bail!("invalid extension authentication name");
        }
        if let Some(namespace) = &binding.credential_namespace {
            uuid::Uuid::parse_str(namespace)?;
            if binding.destination.as_ref().is_none_or(|value| {
                value.len() != 64 || !value.chars().all(|ch| ch.is_ascii_hexdigit())
            }) {
                bail!("scoped authentication requires a valid extension destination");
            }
        } else if !binding.secret_fields.is_empty() {
            bail!("scoped secret fields require an account reference");
        }
        if binding.secret_fields.len() > 64
            || binding.secret_fields.iter().any(|key| {
                key.is_empty()
                    || key.len() > 256
                    || !key
                        .chars()
                        .all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
            })
        {
            bail!("invalid scoped credential fields");
        }
    }
    Ok(())
}

pub(crate) fn secret_key(namespace: &str, field: &str) -> String {
    format!("{SECRET_PREFIX}{namespace}::{field}")
}

pub(crate) fn is_secret_key(key: &str) -> bool {
    key.starts_with(SECRET_PREFIX)
}

pub(crate) fn extension_destination(
    config: &crate::config::ExtensionConfig,
    working_dir: &std::path::Path,
) -> Result<String> {
    let destination = match config {
        crate::config::ExtensionConfig::Stdio { cmd, args, cwd, .. } => {
            let root =
                std::fs::canonicalize(working_dir).unwrap_or_else(|_| working_dir.to_path_buf());
            serde_json::json!(["stdio", cmd, args, cwd, root])
        }
        crate::config::ExtensionConfig::StreamableHttp {
            uri,
            headers,
            socket,
            client_id,
            ..
        } => {
            let headers = headers.iter().collect::<std::collections::BTreeMap<_, _>>();
            serde_json::json!(["http", uri, headers, socket, client_id])
        }
        _ => bail!("authentication requires an MCP extension"),
    };
    Ok(blake3::hash(&serde_json::to_vec(&destination)?)
        .to_hex()
        .to_string())
}
