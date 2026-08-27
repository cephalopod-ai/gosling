use crate::agents::ExtensionConfig;
use anyhow::{anyhow, bail, Context, Result};
use futures::StreamExt;
use serde::Deserialize;
use std::collections::HashSet;
use std::time::Duration;
use tokio::sync::OnceCell;

const ALLOWLIST_FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const ALLOWLIST_MAX_BYTES: usize = 1024 * 1024;

static EXTENSION_ALLOWLIST: OnceCell<Result<ExtensionAllowlist, String>> = OnceCell::const_new();

#[derive(Debug)]
enum ExtensionAllowlist {
    Disabled,
    Enabled(HashSet<Vec<String>>),
}

#[derive(Deserialize)]
struct AllowlistDocument {
    #[serde(default)]
    extensions: Vec<AllowlistEntry>,
}

#[derive(Deserialize)]
struct AllowlistEntry {
    command: String,
}

pub async fn enforce_extension(config: &ExtensionConfig) -> Result<()> {
    let ExtensionConfig::Stdio { cmd, args, .. } = config else {
        return Ok(());
    };

    let allowlist = EXTENSION_ALLOWLIST
        .get_or_init(|| async { load_allowlist().await.map_err(|error| error.to_string()) })
        .await
        .as_ref()
        .map_err(|error| anyhow!(error.clone()))?;

    let ExtensionAllowlist::Enabled(allowed_commands) = allowlist else {
        return Ok(());
    };

    let mut requested = Vec::with_capacity(args.len() + 1);
    requested.push(cmd.clone());
    requested.extend(args.iter().cloned());
    if !allowed_commands.contains(&requested) {
        bail!(
            "extension command is not present in GOSLING_ALLOWLIST: {}",
            requested.join(" ")
        );
    }
    Ok(())
}

async fn load_allowlist() -> Result<ExtensionAllowlist> {
    let Some(location) = std::env::var("GOSLING_ALLOWLIST")
        .ok()
        .filter(|value| !value.trim().is_empty())
    else {
        return Ok(ExtensionAllowlist::Disabled);
    };

    if env_truthy("GOSLING_ALLOWLIST_BYPASS") {
        tracing::warn!(
            security.event_type = "extension_allowlist_bypassed",
            "GOSLING_ALLOWLIST enforcement is disabled by GOSLING_ALLOWLIST_BYPASS"
        );
        return Ok(ExtensionAllowlist::Disabled);
    }

    let url = reqwest::Url::parse(&location).context("GOSLING_ALLOWLIST is not a valid URL")?;
    if url.scheme() != "https" {
        bail!("GOSLING_ALLOWLIST must use https");
    }

    let client = reqwest::Client::builder()
        .timeout(ALLOWLIST_FETCH_TIMEOUT)
        .build()?;
    let response = client
        .get(url)
        .send()
        .await
        .context("failed to fetch GOSLING_ALLOWLIST")?
        .error_for_status()
        .context("GOSLING_ALLOWLIST request failed")?;

    if response
        .content_length()
        .is_some_and(|length| length > ALLOWLIST_MAX_BYTES as u64)
    {
        bail!("GOSLING_ALLOWLIST exceeds {ALLOWLIST_MAX_BYTES} bytes");
    }

    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.context("failed while reading GOSLING_ALLOWLIST")?;
        if body.len().saturating_add(chunk.len()) > ALLOWLIST_MAX_BYTES {
            bail!("GOSLING_ALLOWLIST exceeds {ALLOWLIST_MAX_BYTES} bytes");
        }
        body.extend_from_slice(&chunk);
    }

    parse_allowlist(&body)
}

fn parse_allowlist(body: &[u8]) -> Result<ExtensionAllowlist> {
    let document: AllowlistDocument =
        serde_yaml::from_slice(body).context("GOSLING_ALLOWLIST is not valid YAML")?;
    let mut commands = HashSet::new();
    for entry in document.extensions {
        let command = crate::utils::split_command_args(&entry.command)
            .context("GOSLING_ALLOWLIST contains an invalid command")?;
        if command.is_empty() {
            bail!("GOSLING_ALLOWLIST contains an empty command");
        }
        commands.insert(command);
    }
    Ok(ExtensionAllowlist::Enabled(commands))
}

fn env_truthy(name: &str) -> bool {
    std::env::var(name)
        .is_ok_and(|value| matches!(value.trim().to_ascii_lowercase().as_str(), "1" | "true"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_and_arguments_must_match_exactly() {
        let ExtensionAllowlist::Enabled(commands) = parse_allowlist(
            br#"
extensions:
  - id: example
    command: npx -y '@scope/example'
"#,
        )
        .unwrap() else {
            panic!("parsed allowlist must be enabled");
        };

        assert!(commands.contains(&vec![
            "npx".to_string(),
            "-y".to_string(),
            "@scope/example".to_string()
        ]));
        assert!(!commands.contains(&vec!["npx".to_string()]));
        assert!(!commands.contains(&vec![
            "npx".to_string(),
            "-y".to_string(),
            "@scope/example".to_string(),
            "--unsafe-extra".to_string()
        ]));
    }

    #[test]
    fn invalid_or_empty_commands_fail_the_whole_allowlist_closed() {
        assert!(parse_allowlist(b"extensions:\n  - command: ''\n").is_err());
        assert!(parse_allowlist(b"extensions:\n  - command: '\\\"unterminated'\n").is_err());
    }
}
