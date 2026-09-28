use crate::config::Config;
use gosling_providers::secret_redaction::{SecretRedactor, REDACTED};
use serde_json::Value;

/// Redactor for data that leaves this installation (diagnostics bundles,
/// session exports and share links): every value in the secret store and every
/// provider credential set in the environment is redacted verbatim, on top of
/// the shape-based patterns. (GSL-PT-20260927-D05)
pub async fn installation_secret_redactor() -> SecretRedactor {
    let mut values = Vec::new();
    match Config::global().all_secrets() {
        Ok(secrets) => {
            for value in secrets.values() {
                collect_secret_strings(value, &mut values);
            }
        }
        Err(error) => {
            tracing::warn!(%error, "Could not read stored secrets; redacting by pattern only");
        }
    }
    for name in crate::providers::provider_secret_key_names().await {
        if let Ok(value) = std::env::var(&name) {
            values.push(value);
        }
    }
    SecretRedactor::new(values)
}

/// Custom-header secrets are `Name=value` lists; a tool may echo just a value.
fn collect_secret_strings(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::String(text) => {
            out.extend(
                text.split(',')
                    .filter_map(|pair| pair.split_once('='))
                    .map(|(_, value)| value.trim().to_string()),
            );
            out.push(text.clone());
        }
        Value::Array(items) => items
            .iter()
            .for_each(|item| collect_secret_strings(item, out)),
        Value::Object(fields) => fields
            .values()
            .for_each(|field| collect_secret_strings(field, out)),
        _ => {}
    }
}

/// `SessionManager::export_session` output prepared to leave this installation.
pub async fn redact_exported_session_json(exported: &str) -> anyhow::Result<String> {
    let redactor = installation_secret_redactor().await;
    let session = redact_session_export(serde_json::from_str(exported)?, &redactor);
    Ok(serde_json::to_string_pretty(&session)?)
}

/// A session export as shared outside this installation: every string
/// redacted like a diagnostics bundle, and extension `envs` values replaced,
/// because they hold whatever was passed on the command line, e.g.
/// `--with-extension "GITHUB_TOKEN=... cmd"`. (GSL-PT-20260927-D06, C15)
pub fn redact_session_export(mut session: Value, redactor: &SecretRedactor) -> Value {
    if let Some(extension_data) = session.get_mut("extension_data") {
        redact_extension_env_values(extension_data);
    }
    redactor.redact_json(session)
}

fn redact_extension_env_values(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (key, field) in fields.iter_mut() {
                match field {
                    Value::Object(envs) if key == "envs" => {
                        envs.values_mut()
                            .for_each(|env| *env = Value::String(REDACTED.to_string()));
                    }
                    _ => redact_extension_env_values(field),
                }
            }
        }
        Value::Array(items) => items.iter_mut().for_each(redact_extension_env_values),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn stored_secret_strings_include_custom_header_values() {
        let mut values = Vec::new();
        collect_secret_strings(
            &json!({
                "OPENAI_CUSTOM_HEADERS": "X-Playtest-Token=FAKESECRET-HDR-t9y0,X-Other=second-header-value",
                "oauth": {"access_token": "oauth-access-token-value", "expires_in": 3600}
            }),
            &mut values,
        );
        for expected in [
            "FAKESECRET-HDR-t9y0",
            "second-header-value",
            "oauth-access-token-value",
        ] {
            assert!(values.iter().any(|value| value == expected), "{values:?}");
        }
    }

    #[test]
    fn exports_redact_extension_env_values_and_secret_strings_only() {
        let session = json!({
            "id": "20260928_1",
            "name": "keep the name",
            "extension_data": {
                "enabled_extensions.v0": {"extensions": [
                    {"type": "stdio", "name": "sec", "cmd": "python3",
                     "envs": {"PLAYTEST_TOKEN": "ENVVAL-PT-C-4411"},
                     "env_keys": ["PLAYTEST_SECRET"]}
                ]}
            },
            "conversation": [{"content": [{"text": "pasted FAKESECRET-SE02-TOOL-7f3a and sk-proj-FAKESE02abcdef1234567890XYZ"}]}]
        });
        let redactor = SecretRedactor::new(["FAKESECRET-SE02-TOOL-7f3a".to_string()]);

        let exported = redact_session_export(session, &redactor);

        let extension = &exported["extension_data"]["enabled_extensions.v0"]["extensions"][0];
        assert_eq!(extension["envs"]["PLAYTEST_TOKEN"], REDACTED);
        assert_eq!(extension["env_keys"][0], "PLAYTEST_SECRET");
        assert_eq!(extension["cmd"], "python3");
        assert_eq!(
            exported["conversation"][0]["content"][0]["text"],
            "pasted [REDACTED] and [REDACTED]"
        );
        assert_eq!(exported["name"], "keep the name");
        assert_eq!(exported["id"], "20260928_1");
    }
}
