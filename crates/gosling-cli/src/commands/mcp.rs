use anyhow::{bail, Context, Result};
use etcetera::{choose_app_strategy, AppStrategy, AppStrategyArgs};
use gosling::agents::{extension::Envs, ExtensionConfig};
use gosling::config::extensions::{
    get_all_extension_names, get_all_extensions, name_to_key, remove_extension_and_permissions,
    set_extension_with_secrets,
};
use gosling::config::{Config, ExtensionEntry, DEFAULT_EXTENSION_TIMEOUT};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

pub struct InstallArgs {
    pub name: String,
    pub cmd: Option<String>,
    pub envs: Vec<String>,
    pub secrets: Vec<String>,
    pub timeout: Option<u64>,
    pub description: Option<String>,
    pub cwd: Option<String>,
    pub from_goose: bool,
    pub goose_config: Option<PathBuf>,
}

pub fn handle_install(args: InstallArgs) -> Result<()> {
    let env_pairs = parse_key_values(&args.envs, "--env")?;
    let secret_pairs = parse_secret_values(&args.secrets)?;

    let mut entry = if args.from_goose {
        import_goose_entry(&args.name, args.goose_config.clone())?
    } else {
        let command = args
            .cmd
            .as_deref()
            .expect("clap requires --cmd unless --from-goose is set");
        let mut parts = gosling::utils::split_command_args(command)?;
        if parts.is_empty() {
            bail!("--cmd must not be empty");
        }
        ExtensionEntry {
            enabled: true,
            config: ExtensionConfig::Stdio {
                name: args.name.clone(),
                description: String::new(),
                cmd: parts.remove(0),
                args: parts,
                envs: Envs::default(),
                env_keys: Vec::new(),
                timeout: Some(DEFAULT_EXTENSION_TIMEOUT),
                cwd: None,
                bundled: None,
                available_tools: Vec::new(),
            },
        }
    };

    entry.enabled = true;
    let secret_keys: Vec<String> = secret_pairs.iter().map(|(key, _)| key.clone()).collect();
    let supplied_secret_keys = secret_keys.iter().cloned().collect::<HashSet<_>>();
    apply_overrides(&mut entry.config, &args, env_pairs, secret_keys)?;
    if args.from_goose {
        let unresolved = unresolved_env_keys(&entry.config, &supplied_secret_keys);
        if !unresolved.is_empty() {
            bail!(
                "Goose extension '{}' references secrets unavailable to gosling: {}. Goose keyring values are not copied; export each key and pass --secret KEY, or use --secret KEY=VALUE",
                args.name,
                unresolved.join(", ")
            );
        }
    }

    let key = entry.config.key();
    let previous = get_all_extensions()
        .into_iter()
        .find(|existing| existing.config.key() == key);
    let existed = previous.is_some();
    let secret_updates = secret_pairs
        .iter()
        .map(|(key, value)| (key.clone(), serde_json::Value::String(value.clone())))
        .collect::<Vec<_>>();
    set_extension_with_secrets(entry, &secret_updates)
        .context("failed to persist extension config and secrets")?;
    println!(
        "{} extension '{}' in {}",
        if existed { "Updated" } else { "Installed" },
        key,
        Config::global().path()
    );
    Ok(())
}

pub fn handle_remove(name: &str) -> Result<()> {
    let key = name_to_key(name);
    if !get_all_extension_names().contains(&key) {
        bail!("no extension '{}' in {}", key, Config::global().path());
    }
    if !remove_extension_and_permissions(&key)
        .context("failed to persist extension and permission removal")?
    {
        bail!("no extension '{}' in {}", key, Config::global().path());
    }
    println!("Removed extension '{}'", key);
    Ok(())
}

pub fn handle_list() -> Result<()> {
    let extensions = get_all_extensions();
    if extensions.is_empty() {
        println!("No extensions configured in {}", Config::global().path());
        return Ok(());
    }

    for entry in extensions {
        let status = if entry.enabled { "enabled" } else { "disabled" };
        let (kind, target) = describe(&entry.config);
        println!(
            "{:<24} {:<9} {:<16} {}",
            entry.config.key(),
            status,
            kind,
            target
        );
    }
    Ok(())
}

fn describe(config: &ExtensionConfig) -> (&'static str, String) {
    match config {
        ExtensionConfig::Stdio { cmd, args, .. } => {
            let mut target = cmd.clone();
            if !args.is_empty() {
                target.push(' ');
                target.push_str(&args.join(" "));
            }
            ("stdio", target)
        }
        ExtensionConfig::StreamableHttp { uri, .. } => ("streamable_http", uri.clone()),
        ExtensionConfig::Sse { .. } => ("sse", "(unsupported)".to_string()),
        ExtensionConfig::Builtin { .. } => ("builtin", String::new()),
        ExtensionConfig::Platform { .. } => ("platform", String::new()),
        ExtensionConfig::Frontend { .. } => ("frontend", String::new()),
        ExtensionConfig::InlinePython { .. } => ("inline_python", String::new()),
    }
}

fn parse_key_values(pairs: &[String], flag: &str) -> Result<Vec<(String, String)>> {
    pairs
        .iter()
        .map(|pair| {
            pair.split_once('=')
                .filter(|(key, _)| !key.is_empty())
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .with_context(|| format!("{flag} requires KEY=VALUE"))
        })
        .collect()
}

fn parse_secret_values(values: &[String]) -> Result<Vec<(String, String)>> {
    values
        .iter()
        .map(|value| {
            if let Some((key, secret)) = value.split_once('=').filter(|(key, _)| !key.is_empty()) {
                return Ok((key.to_string(), secret.to_string()));
            }
            if value.is_empty() {
                bail!("--secret requires KEY or KEY=VALUE");
            }
            let secret = std::env::var(value).with_context(|| {
                format!("--secret {value} requires the {value} environment variable or KEY=VALUE")
            })?;
            Ok((value.clone(), secret))
        })
        .collect()
}

fn apply_overrides(
    config: &mut ExtensionConfig,
    args: &InstallArgs,
    env_pairs: Vec<(String, String)>,
    secret_keys: Vec<String>,
) -> Result<()> {
    match config {
        ExtensionConfig::Stdio {
            description,
            envs,
            env_keys,
            timeout,
            cwd,
            ..
        } => {
            if let Some(value) = &args.description {
                *description = value.clone();
            }
            if let Some(value) = args.timeout {
                *timeout = Some(value);
            }
            if let Some(value) = &args.cwd {
                *cwd = Some(value.clone());
            }
            merge_envs(envs, env_pairs);
            merge_env_keys(env_keys, secret_keys);
        }
        ExtensionConfig::StreamableHttp {
            description,
            envs,
            env_keys,
            timeout,
            ..
        } => {
            if args.cwd.is_some() {
                bail!("--cwd does not apply to a streamable_http extension");
            }
            if let Some(value) = &args.description {
                *description = value.clone();
            }
            if let Some(value) = args.timeout {
                *timeout = Some(value);
            }
            merge_envs(envs, env_pairs);
            merge_env_keys(env_keys, secret_keys);
        }
        other => bail!(
            "extension '{}' is not an MCP server (stdio or streamable_http)",
            other.name()
        ),
    }
    Ok(())
}

fn merge_envs(envs: &mut Envs, additions: Vec<(String, String)>) {
    if additions.is_empty() {
        return;
    }
    let mut map = envs.get_env();
    map.extend(additions);
    *envs = Envs::new(map);
}

fn merge_env_keys(env_keys: &mut Vec<String>, additions: Vec<String>) {
    for key in additions {
        if !env_keys.contains(&key) {
            env_keys.push(key);
        }
    }
}

fn unresolved_env_keys(
    config: &ExtensionConfig,
    supplied_secret_keys: &HashSet<String>,
) -> Vec<String> {
    let (envs, env_keys) = match config {
        ExtensionConfig::Stdio { envs, env_keys, .. }
        | ExtensionConfig::StreamableHttp { envs, env_keys, .. } => (envs, env_keys),
        _ => return Vec::new(),
    };
    let direct_envs = envs.get_env();
    let mut unresolved = env_keys
        .iter()
        .filter(|key| {
            let key = key.as_str();
            !direct_envs.contains_key(key)
                && !supplied_secret_keys.contains(key)
                && !Config::global()
                    .get(key, true)
                    .ok()
                    .is_some_and(|value| value.as_str().is_some())
        })
        .cloned()
        .collect::<Vec<_>>();
    unresolved.sort();
    unresolved.dedup();
    unresolved
}

fn goose_config_path() -> Result<PathBuf> {
    let strategy = choose_app_strategy(AppStrategyArgs {
        top_level_domain: "Block".to_string(),
        author: "Block".to_string(),
        app_name: "goose".to_string(),
    })
    .context("could not determine the home directory")?;
    Ok(strategy.config_dir().join("config.yaml"))
}

#[derive(serde::Deserialize)]
struct GooseConfigFile {
    #[serde(default)]
    extensions: HashMap<String, serde_yaml::Value>,
}

fn import_goose_entry(name: &str, config_path: Option<PathBuf>) -> Result<ExtensionEntry> {
    let path = match config_path {
        Some(path) => path,
        None => goose_config_path()?,
    };
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read Goose config at {}", path.display()))?;
    let goose: GooseConfigFile = serde_yaml::from_str(&content)
        .with_context(|| format!("could not parse Goose config at {}", path.display()))?;

    let key = name_to_key(name);
    let mut extensions = goose.extensions;
    let value = extensions.remove(&key).or_else(|| {
        extensions
            .into_values()
            .find(|value| value.get("name").and_then(|n| n.as_str()) == Some(name))
    });
    let Some(value) = value else {
        bail!("no extension '{}' in {}", name, path.display());
    };

    let entry: ExtensionEntry = serde_yaml::from_value(value).with_context(|| {
        format!(
            "extension '{}' in {} is not a valid extension entry",
            name,
            path.display()
        )
    })?;

    match entry.config {
        ExtensionConfig::Stdio { .. } | ExtensionConfig::StreamableHttp { .. } => Ok(entry),
        _ => bail!(
            "extension '{}' in {} is not an MCP server (stdio or streamable_http)",
            name,
            path.display()
        ),
    }
}
