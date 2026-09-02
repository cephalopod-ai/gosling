// Owns extension environment merging, substitution, and static OAuth registration.
// Transport startup receives resolved values while callers keep existing helper paths.
// The extension_manager compatibility facade re-exports crate-visible helpers.

use super::*;

static RE_ENV_BRACES: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\$\{\s*([A-Za-z_][A-Za-z0-9_]*)\s*\}").expect("valid regex"));

static RE_ENV_SIMPLE: Lazy<regex::Regex> =
    Lazy::new(|| regex::Regex::new(r"\$([A-Za-z_][A-Za-z0-9_]*)").expect("valid regex"));

pub(crate) async fn merge_environments(
    envs: &Envs,
    env_keys: &[String],
    ext_name: &str,
    config: &Config,
) -> Result<HashMap<String, String>, ExtensionError> {
    let mut all_envs = envs.get_env();

    for key in env_keys {
        if all_envs.contains_key(key) {
            continue;
        }

        match config.get(key, true) {
            Ok(value) => {
                if value.is_null() {
                    warn!(
                        key = %key,
                        ext_name = %ext_name,
                        "Secret key not found in config (returned null)."
                    );
                    continue;
                }

                if let Some(str_val) = value.as_str() {
                    all_envs.insert(key.clone(), str_val.to_string());
                } else {
                    warn!(
                        key = %key,
                        ext_name = %ext_name,
                        value_type = %value.get("type").and_then(|t| t.as_str()).unwrap_or("unknown"),
                        "Secret value is not a string; skipping."
                    );
                }
            }
            Err(e) => {
                error!(
                    key = %key,
                    ext_name = %ext_name,
                    error = %e,
                    "Failed to fetch secret from config."
                );
                return Err(ExtensionError::ConfigError(format!(
                    "Failed to fetch secret '{}' from config: {}",
                    key, e
                )));
            }
        }
    }

    Ok(Envs::new(all_envs).get_env())
}

/// Substitute environment variables in a string. Supports both ${VAR} and $VAR syntax.
pub(crate) fn substitute_env_vars(value: &str, env_map: &HashMap<String, String>) -> String {
    let mut result = value.to_string();

    for cap in RE_ENV_BRACES.captures_iter(value) {
        if let Some(var_name) = cap.get(1) {
            if let Some(env_value) = env_map.get(var_name.as_str()) {
                result = result.replace(&cap[0], env_value);
            }
        }
    }

    // Scan the original input for $VAR patterns (not the post-substitution result)
    // to avoid recursive expansion when a substituted value contains $OTHER_VAR.
    for cap in RE_ENV_SIMPLE.captures_iter(value) {
        if let Some(var_name) = cap.get(1) {
            if !value.contains(&format!("${{{}}}", var_name.as_str())) {
                if let Some(env_value) = env_map.get(var_name.as_str()) {
                    result = result.replace(&cap[0], env_value);
                }
            }
        }
    }

    result
}

#[allow(clippy::result_large_err)]
pub(super) fn resolve_static_oauth_client(
    client_id: Option<&str>,
    client_secret_key: Option<&str>,
    scopes: &[String],
    envs: &HashMap<String, String>,
    config: &Config,
) -> ExtensionResult<Option<StaticOAuthClientConfig>> {
    let Some(client_id) = client_id else {
        if client_secret_key.is_some() || !scopes.is_empty() {
            return Err(ExtensionError::ConfigError(
                "client_secret_key and scopes require client_id for streamable_http OAuth"
                    .to_string(),
            ));
        }
        return Ok(None);
    };

    let client_id = substitute_env_vars(client_id, envs);
    if client_id.trim().is_empty() {
        return Err(ExtensionError::ConfigError(
            "client_id for streamable_http OAuth cannot be empty".to_string(),
        ));
    }

    let client_secret = match client_secret_key {
        Some(key) => match envs.get(key) {
            Some(value) => Some(value.clone()),
            None => config
                .get_secret::<String>(key)
                .ok()
                .filter(|value| !value.is_empty()),
        },
        None => None,
    };
    if client_secret_key.is_some() && client_secret.is_none() {
        return Err(ExtensionError::ConfigError(format!(
            "OAuth client secret '{}' was not found",
            client_secret_key.unwrap_or_default()
        )));
    }

    Ok(Some(StaticOAuthClientConfig {
        client_id,
        client_secret,
        scopes: scopes.to_vec(),
    }))
}
