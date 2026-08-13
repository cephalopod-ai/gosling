use super::base::{Config, ConfigError};
use crate::agents::extension::PLATFORM_EXTENSIONS;
use crate::agents::ExtensionConfig;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_yaml::Mapping;
use std::path::Path;
use std::sync::{LazyLock, Mutex, MutexGuard, PoisonError};
use tracing::{info, warn};
use utoipa::ToSchema;

pub const DEFAULT_EXTENSION: &str = "developer";
pub const DEFAULT_EXTENSION_TIMEOUT: u64 = 300;
pub const DEFAULT_EXTENSION_DESCRIPTION: &str = "";
pub const DEFAULT_DISPLAY_NAME: &str = "Developer";
const EXTENSIONS_CONFIG_KEY: &str = "extensions";
static EXTENSION_MUTATION_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

fn lock_extension_mutations() -> MutexGuard<'static, ()> {
    EXTENSION_MUTATION_LOCK
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

#[derive(Debug, Deserialize, Serialize, Clone, ToSchema)]
pub struct ExtensionEntry {
    pub enabled: bool,
    /// Absolute directory paths this extension is scoped to.
    ///
    /// `None` or an empty list (the default, and what every config predating
    /// this field deserializes to) means the extension is available in every
    /// session regardless of working directory - unchanged from historical
    /// behavior. When non-empty, the extension is only made available to a
    /// session whose working directory is equal to, or a descendant of, one
    /// of these paths. See [`ExtensionEntry::is_active_for_cwd`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activation_paths: Option<Vec<String>>,
    #[serde(flatten)]
    pub config: ExtensionConfig,
}

impl ExtensionEntry {
    /// Returns true when this extension entry should be considered for a
    /// session whose working directory is `cwd`. See the `activation_paths`
    /// field docs for the exact matching semantics.
    pub fn is_active_for_cwd(&self, cwd: &Path) -> bool {
        extension_active_for_cwd(self.activation_paths.as_deref(), cwd)
    }
}

/// Core matcher behind [`ExtensionEntry::is_active_for_cwd`], split out as a
/// free function so it can be unit tested against raw path lists without
/// constructing a full `ExtensionEntry`.
///
/// `None`/empty `activation_paths` always match (backward compatible default).
/// Otherwise `cwd` must equal, or descend from, one of the listed paths. Both
/// sides are canonicalized before comparing so symlinks (e.g. macOS `/tmp` vs
/// `/private/tmp`) and trailing slashes don't cause false negatives; if a path
/// can't be canonicalized (e.g. it doesn't exist on this machine) the raw path
/// is used as a best-effort fallback instead of unconditionally failing the match.
pub(crate) fn extension_active_for_cwd(activation_paths: Option<&[String]>, cwd: &Path) -> bool {
    let Some(paths) = activation_paths else {
        return true;
    };
    if paths.is_empty() {
        return true;
    }

    let canonical_cwd = cwd.canonicalize().unwrap_or_else(|_| cwd.to_path_buf());

    paths.iter().any(|raw_path| {
        let candidate = Path::new(raw_path.as_str());
        let canonical_candidate = candidate
            .canonicalize()
            .unwrap_or_else(|_| candidate.to_path_buf());
        canonical_cwd.starts_with(&canonical_candidate)
    })
}

pub fn name_to_key(name: &str) -> String {
    let mut result = String::with_capacity(name.len());
    for c in name.chars() {
        result.push(match c {
            c if c.is_ascii_alphanumeric() || c == '_' || c == '-' => c,
            c if c.is_whitespace() => continue,
            _ => '_',
        });
    }
    result.to_lowercase()
}

pub(crate) fn is_extension_available(config: &ExtensionConfig) -> bool {
    match config {
        ExtensionConfig::Platform { name, .. } => {
            crate::agents::extension::PLATFORM_EXTENSIONS.contains_key(name_to_key(name).as_str())
        }
        _ => true,
    }
}

fn parse_extensions_map(raw: &Mapping) -> IndexMap<String, ExtensionEntry> {
    let mut extensions_map = IndexMap::with_capacity(raw.len());
    for (k, v) in raw {
        let Some(key) = k.as_str() else {
            warn!(key = ?k, "Skipping malformed extension config entry");
            continue;
        };

        match serde_yaml::from_value::<ExtensionEntry>(v.clone()) {
            Ok(entry) => {
                if !is_extension_available(&entry.config) {
                    continue;
                }
                extensions_map.insert(key.to_string(), entry);
            }
            Err(err) => {
                info!(
                    key = %key,
                    error = %err,
                    "Skipping malformed extension config entry"
                );
            }
        }
    }

    extensions_map
}

fn get_extensions_map_with_config(config: &Config) -> IndexMap<String, ExtensionEntry> {
    let raw: Mapping = config
        .get_param(EXTENSIONS_CONFIG_KEY)
        .unwrap_or_else(|err| {
            warn!(
                "Failed to load {}: {err}. Falling back to empty object.",
                EXTENSIONS_CONFIG_KEY
            );
            Default::default()
        });

    parse_extensions_map(&raw)
}

fn get_extensions_map() -> IndexMap<String, ExtensionEntry> {
    get_extensions_map_with_config(Config::global())
}

enum ExtensionMutation {
    Upsert(String, Box<ExtensionEntry>),
    Remove(String),
    Noop,
}

fn with_raw_extensions_mapping<F>(config: &Config, mutate: F) -> Result<(), ConfigError>
where
    F: FnOnce(&mut IndexMap<String, ExtensionEntry>) -> ExtensionMutation,
{
    let mut serialize_error = None;
    let result = config.update_param::<Mapping, Mapping, _>(EXTENSIONS_CONFIG_KEY, |mut raw| {
        let mut extensions = parse_extensions_map(&raw);

        match mutate(&mut extensions) {
            ExtensionMutation::Upsert(key, entry) => match serde_yaml::to_value(entry) {
                Ok(value) => {
                    raw.insert(serde_yaml::Value::String(key), value);
                }
                Err(err) => {
                    serialize_error = Some(err);
                }
            },
            ExtensionMutation::Remove(key) => {
                raw.shift_remove(key.as_str());
            }
            ExtensionMutation::Noop => {}
        }

        raw
    });

    if let Some(error) = serialize_error {
        Err(error.into())
    } else {
        result
    }
}

pub fn get_extension_by_name(name: &str) -> Option<ExtensionConfig> {
    get_extension_by_name_with_config(Config::global(), name)
}

fn get_extension_by_name_with_config(config: &Config, name: &str) -> Option<ExtensionConfig> {
    let extensions = get_extensions_map_with_config(config);
    let key = name_to_key(name);

    if let Some(entry) = extensions
        .values()
        .find(|entry| entry.config.name() == name)
        .or_else(|| extensions.get(&key))
    {
        return Some(entry.config.clone());
    }

    get_available_extensions()
        .into_iter()
        .find(|config| config.name() == name || config.key() == key)
}

pub fn set_extension(entry: ExtensionEntry) -> Result<(), ConfigError> {
    let _guard = lock_extension_mutations();
    let _file_guard = Config::global().lock_extension_transaction()?;
    set_extension_with_config(Config::global(), entry)
}

pub fn set_extension_with_secrets(
    entry: ExtensionEntry,
    secret_updates: &[(String, Value)],
) -> anyhow::Result<()> {
    let _guard = lock_extension_mutations();
    let _file_guard = Config::global().lock_extension_transaction()?;
    set_extension_with_secrets_and_config(Config::global(), entry, secret_updates)
}

fn set_extension_with_secrets_and_config(
    config: &Config,
    entry: ExtensionEntry,
    secret_updates: &[(String, Value)],
) -> anyhow::Result<()> {
    let key = entry.config.key();
    let previous = get_extensions_map_with_config(config).shift_remove(&key);
    let secret_snapshot = if secret_updates.is_empty() {
        IndexMap::new()
    } else {
        let stored_secrets = config.all_secrets()?;
        let snapshot = secret_updates
            .iter()
            .map(|(key, _)| (key.clone(), stored_secrets.get(key).cloned()))
            .collect::<IndexMap<_, _>>();
        config.set_secret_values(secret_updates)?;
        snapshot
    };
    if let Err(config_error) = set_extension_with_config(config, entry.clone()) {
        return match restore_secret_snapshot(config, &secret_snapshot) {
            Ok(()) => Err(config_error.into()),
            Err(rollback_error) => anyhow::bail!(
                "failed to persist extension config: {config_error}; failed to restore secrets: {rollback_error}"
            ),
        };
    }

    let persisted = get_extensions_map_with_config(config)
        .get(&key)
        .is_some_and(|saved| saved.enabled == entry.enabled && saved.config == entry.config);
    if persisted {
        return Ok(());
    }

    let config_rollback = match previous {
        Some(previous) => set_extension_with_config(config, previous),
        None => remove_extension_with_config(config, &key).map(|_| ()),
    };
    let secret_rollback = restore_secret_snapshot(config, &secret_snapshot);
    match (config_rollback, secret_rollback) {
        (Ok(()), Ok(())) => anyhow::bail!("extension '{key}' could not be read back after persistence"),
        (config_result, secret_result) => anyhow::bail!(
            "extension '{key}' could not be read back after persistence; config rollback: {}; secret rollback: {}",
            rollback_status(config_result),
            rollback_status(secret_result)
        ),
    }
}

fn restore_secret_snapshot(
    config: &Config,
    snapshot: &IndexMap<String, Option<Value>>,
) -> Result<(), ConfigError> {
    let mut updates = Vec::new();
    let mut deletions = Vec::new();
    for (key, value) in snapshot {
        match value {
            Some(value) => updates.push((key.clone(), value.clone())),
            None => deletions.push(key.clone()),
        }
    }
    config.update_secret_values(&updates, &deletions)
}

fn rollback_status<E: std::fmt::Display>(result: Result<(), E>) -> String {
    match result {
        Ok(()) => "ok".to_string(),
        Err(error) => error.to_string(),
    }
}

fn set_extension_with_config(config: &Config, entry: ExtensionEntry) -> Result<(), ConfigError> {
    let key = entry.config.key();
    set_extension_at_key_with_config(config, key, entry)
}

fn set_extension_at_key_with_config(
    config: &Config,
    key: String,
    entry: ExtensionEntry,
) -> Result<(), ConfigError> {
    with_raw_extensions_mapping(config, |_| ExtensionMutation::Upsert(key, Box::new(entry)))
}

pub fn remove_extension(key: &str) -> Result<bool, ConfigError> {
    let _guard = lock_extension_mutations();
    let _file_guard = Config::global().lock_extension_transaction()?;
    remove_extension_with_config(Config::global(), key)
}

fn remove_extension_with_config(config: &Config, key: &str) -> Result<bool, ConfigError> {
    let mut removed = false;
    with_raw_extensions_mapping(config, |extensions| {
        removed = extensions.contains_key(key);
        if removed {
            ExtensionMutation::Remove(key.to_string())
        } else {
            ExtensionMutation::Noop
        }
    })?;
    Ok(removed)
}

pub fn remove_extension_and_permissions(key: &str) -> anyhow::Result<bool> {
    let _guard = lock_extension_mutations();
    let _file_guard = Config::global().lock_extension_transaction()?;
    let Some(previous) = get_extensions_map().shift_remove(key) else {
        return Ok(false);
    };

    if !remove_extension_with_config(Config::global(), key)? {
        return Ok(false);
    }
    if let Err(permission_error) =
        crate::config::PermissionManager::instance().remove_extension(key)
    {
        return match set_extension_at_key_with_config(Config::global(), key.to_string(), previous) {
            Ok(()) => Err(permission_error),
            Err(config_error) => anyhow::bail!(
                "failed to remove extension permissions: {permission_error}; failed to restore extension config: {config_error}"
            ),
        };
    }
    Ok(true)
}

/// Returns true when an existing extension was updated, false when the key was missing.
pub fn set_extension_enabled(key: &str, enabled: bool) -> Result<bool, ConfigError> {
    let _guard = lock_extension_mutations();
    let _file_guard = Config::global().lock_extension_transaction()?;
    set_extension_enabled_with_config(Config::global(), key, enabled)
}

fn set_extension_enabled_with_config(
    config: &Config,
    key: &str,
    enabled: bool,
) -> Result<bool, ConfigError> {
    let mut updated = false;
    with_raw_extensions_mapping(config, |extensions| {
        let Some(entry) = extensions.get_mut(key) else {
            return ExtensionMutation::Noop;
        };

        entry.enabled = enabled;
        updated = true;
        ExtensionMutation::Upsert(key.to_string(), Box::new(entry.clone()))
    })?;

    Ok(updated)
}

pub fn get_all_extensions() -> Vec<ExtensionEntry> {
    let extensions = get_extensions_map();
    extensions.into_values().collect()
}

pub fn get_all_extension_names() -> Vec<String> {
    let extensions = get_extensions_map();
    extensions.keys().cloned().collect()
}

pub fn is_extension_enabled(key: &str) -> bool {
    let extensions = get_extensions_map();
    extensions.get(key).map(|e| e.enabled).unwrap_or(false)
}

/// Returns the configured enabled state for an extension, or `None` when the
/// extension has no entry in the config at all.
///
/// This lets callers distinguish "not configured" (e.g. a fresh install where a
/// bundled builtin should still load by default) from "explicitly turned off".
pub fn configured_enabled_state(config: &Config, name: &str) -> Option<bool> {
    let extensions = get_extensions_map_with_config(config);
    let key = name_to_key(name);
    extensions
        .values()
        .find(|entry| entry.config.name() == name)
        .or_else(|| extensions.get(&key))
        .map(|entry| entry.enabled)
}

/// Returns true when a builtin the platform wants to load has been turned off
/// by the user.
///
/// A builtin is considered user-disabled only when its config entry is
/// `enabled: false` *and* it would otherwise be on by default. This matters
/// because `run_read_migrations` synthesizes a config entry for every platform
/// extension using its `default_enabled` value, so a default-off extension
/// (e.g. code_execution) looks identical to one a user turned off. Gating on
/// the default lets an explicit builtins request still load default-off
/// extensions while honoring a user disabling a default-on one (e.g. developer).
pub fn is_builtin_disabled_by_user(config: &Config, name: &str) -> bool {
    if configured_enabled_state(config, name) != Some(false) {
        return false;
    }

    match PLATFORM_EXTENSIONS.get(name_to_key(name).as_str()) {
        Some(def) => def.default_enabled,
        None => true,
    }
}

pub fn get_enabled_extensions() -> Vec<ExtensionConfig> {
    get_all_extensions()
        .into_iter()
        .filter(|ext| ext.enabled)
        .map(|ext| ext.config)
        .collect()
}

pub fn get_enabled_extensions_with_config(config: &Config) -> Vec<ExtensionConfig> {
    get_extensions_map_with_config(config)
        .into_values()
        .filter(|ext| ext.enabled)
        .map(|ext| ext.config)
        .collect()
}

/// Like [`get_enabled_extensions_with_config`], but additionally filters out
/// extensions whose `activation_paths` don't cover `cwd`.
///
/// Use this at session-start call sites where the session's working directory
/// is known, so extensions locked to an unrelated project (e.g. a codebase
/// memory server hard-coded to a different repo) don't get loaded into every
/// session. Callers with no session/cwd concept (config editing commands,
/// `gosling mcp list`, analytics) should keep using the cwd-agnostic version.
pub fn get_enabled_extensions_with_config_for_cwd(
    config: &Config,
    cwd: &Path,
) -> Vec<ExtensionConfig> {
    get_extensions_map_with_config(config)
        .into_values()
        .filter(|ext| ext.enabled)
        .filter(|ext| ext.is_active_for_cwd(cwd))
        .map(|ext| ext.config)
        .collect()
}

/// [`get_enabled_extensions_with_config_for_cwd`] against the global config.
pub fn get_enabled_extensions_for_cwd(cwd: &Path) -> Vec<ExtensionConfig> {
    get_enabled_extensions_with_config_for_cwd(Config::global(), cwd)
}

pub fn get_available_extensions() -> Vec<ExtensionConfig> {
    let mut builtin_names = crate::builtin_extension::get_builtin_extension_names();
    builtin_names.sort_unstable();

    let mut platform_definitions = PLATFORM_EXTENSIONS
        .values()
        .filter(|definition| !definition.hidden)
        .collect::<Vec<_>>();
    platform_definitions.sort_unstable_by_key(|definition| definition.name);

    builtin_names
        .into_iter()
        .map(|name| ExtensionConfig::Builtin {
            name: name.to_string(),
            description: String::new(),
            display_name: Some(name.to_string()),
            timeout: None,
            bundled: Some(true),
            available_tools: Vec::new(),
        })
        .chain(
            platform_definitions
                .into_iter()
                .map(|definition| ExtensionConfig::Platform {
                    name: definition.name.to_string(),
                    description: definition.description.to_string(),
                    display_name: Some(definition.display_name.to_string()),
                    bundled: Some(true),
                    available_tools: Vec::new(),
                }),
        )
        .collect()
}

pub fn get_warnings() -> Vec<String> {
    let raw: Mapping = Config::global()
        .get_param(EXTENSIONS_CONFIG_KEY)
        .unwrap_or_default();

    let mut warnings = Vec::new();
    for (k, v) in raw {
        if let (serde_yaml::Value::String(key), Ok(entry)) =
            (k, serde_yaml::from_value::<ExtensionEntry>(v))
        {
            if matches!(entry.config, ExtensionConfig::Sse { .. }) {
                warnings.push(format!(
                    "'{}': SSE is unsupported, migrate to streamable_http",
                    key
                ));
            }
        }
    }
    warnings
}

pub fn resolve_extensions_for_new_session(
    override_extensions: Option<Vec<ExtensionConfig>>,
) -> Vec<ExtensionConfig> {
    resolve_extensions_for_new_session_inner(override_extensions, None)
}

/// Like [`resolve_extensions_for_new_session`], but scopes the extension set
/// to the new session's working directory via `activation_paths`. Prefer this
/// at any session-creation call site that knows the session's cwd up front.
pub fn resolve_extensions_for_new_session_for_cwd(
    override_extensions: Option<Vec<ExtensionConfig>>,
    cwd: &Path,
) -> Vec<ExtensionConfig> {
    resolve_extensions_for_new_session_inner(override_extensions, Some(cwd))
}

fn resolve_extensions_for_new_session_inner(
    override_extensions: Option<Vec<ExtensionConfig>>,
    cwd: Option<&Path>,
) -> Vec<ExtensionConfig> {
    let extensions = match (override_extensions, cwd) {
        (Some(exts), _) => exts,
        (None, Some(cwd)) => get_enabled_extensions_for_cwd(cwd),
        (None, None) => get_enabled_extensions(),
    };

    extensions
        .into_iter()
        .filter(is_extension_available)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fmt;
    use std::sync::{Arc, Mutex};
    use tempfile::{NamedTempFile, TempDir};
    use tracing::{Event, Level, Subscriber};
    use tracing_subscriber::layer::SubscriberExt;

    fn test_config(content: &str) -> (Config, NamedTempFile, NamedTempFile) {
        let config_file = NamedTempFile::new().unwrap();
        let secrets_file = NamedTempFile::new().unwrap();
        std::fs::write(config_file.path(), content).unwrap();
        let config =
            Config::new_with_file_secrets(config_file.path(), secrets_file.path()).unwrap();
        (config, config_file, secrets_file)
    }

    fn read_extensions(config: &Config) -> Mapping {
        let content = std::fs::read_to_string(config.path()).unwrap();
        let values: Mapping = serde_yaml::from_str(&content).unwrap();
        values
            .get(EXTENSIONS_CONFIG_KEY)
            .unwrap()
            .as_mapping()
            .unwrap()
            .clone()
    }

    fn builtin_entry(name: &str, enabled: bool) -> ExtensionEntry {
        builtin_entry_with_activation_paths(name, enabled, None)
    }

    fn builtin_entry_with_activation_paths(
        name: &str,
        enabled: bool,
        activation_paths: Option<Vec<String>>,
    ) -> ExtensionEntry {
        ExtensionEntry {
            enabled,
            activation_paths,
            config: ExtensionConfig::Builtin {
                name: name.to_string(),
                description: format!("{name} description"),
                display_name: Some(name.to_string()),
                timeout: None,
                bundled: None,
                available_tools: Vec::new(),
            },
        }
    }

    #[test]
    fn test_is_extension_available_filters_unknown_platform() {
        let unknown_platform = ExtensionConfig::Platform {
            name: "definitely_not_real_platform_extension".to_string(),
            description: "unknown".to_string(),
            display_name: None,
            bundled: None,
            available_tools: Vec::new(),
        };

        let builtin = ExtensionConfig::Builtin {
            name: "developer".to_string(),
            description: "".to_string(),
            display_name: Some("Developer".to_string()),
            timeout: None,
            bundled: None,
            available_tools: Vec::new(),
        };

        assert!(!is_extension_available(&unknown_platform));
        assert!(is_extension_available(&builtin));
    }

    #[test]
    fn test_set_extension_enabled_preserves_clean_siblings() {
        let (config, _config_file, _secrets_file) = test_config(
            r#"
extensions:
  first:
    enabled: true
    type: builtin
    name: first
    description: first description
    display_name: First
  second:
    enabled: true
    type: builtin
    name: second
    description: second description
    display_name: Second
    extra_field: preserved
"#,
        );
        let before = read_extensions(&config);
        let second_before = before.get("second").unwrap().clone();

        set_extension_enabled_with_config(&config, "first", false).unwrap();

        let extensions = read_extensions(&config);
        assert_eq!(
            extensions
                .get("first")
                .unwrap()
                .as_mapping()
                .unwrap()
                .get("enabled")
                .unwrap()
                .as_bool(),
            Some(false)
        );
        assert_eq!(extensions.get("second").unwrap(), &second_before);
    }

    #[test]
    fn test_set_extension_enabled_preserves_unparseable_sibling() {
        let (config, _config_file, _secrets_file) = test_config(
            r#"
extensions:
  valid:
    enabled: true
    type: builtin
    name: valid
    description: valid description
    display_name: Valid
  broken:
    enabled: true
    type: stdio
    name: Broken
    description: missing cmd
    args: []
"#,
        );
        let before = read_extensions(&config);
        let broken_before = before.get("broken").unwrap().clone();

        set_extension_enabled_with_config(&config, "valid", false).unwrap();

        let extensions = read_extensions(&config);
        assert!(extensions.contains_key("valid"));
        assert_eq!(extensions.get("broken").unwrap(), &broken_before);
        assert_eq!(
            extensions
                .get("valid")
                .unwrap()
                .as_mapping()
                .unwrap()
                .get("enabled")
                .unwrap()
                .as_bool(),
            Some(false)
        );
    }

    #[test]
    fn test_set_extension_adds_entry_without_dropping_unparseable_entries() {
        let (config, _config_file, _secrets_file) = test_config(
            r#"
extensions:
  broken:
    enabled: true
    type: stdio
    name: Broken
    description: missing cmd
    args: []
"#,
        );
        let before = read_extensions(&config);
        let broken_before = before.get("broken").unwrap().clone();

        set_extension_with_config(&config, builtin_entry("new extension", true)).unwrap();

        let extensions = read_extensions(&config);
        assert_eq!(extensions.get("broken").unwrap(), &broken_before);
        assert!(extensions.contains_key("newextension"));
    }

    #[test]
    fn test_set_extension_propagates_write_failure() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        let secrets_path = temp_dir.path().join("secrets.yaml");
        let config = Config::new_with_file_secrets(&config_path, &secrets_path).unwrap();
        std::fs::create_dir(config_path.with_extension("save.lock")).unwrap();

        let error = set_extension_with_config(&config, builtin_entry("new extension", true))
            .expect_err("config write must fail when the save lock path is a directory");

        assert!(matches!(error, ConfigError::FileError(_)));
        assert!(!config_path.exists());
    }

    #[test]
    fn test_set_extension_with_secrets_restores_values_after_config_failure() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        let secrets_path = temp_dir.path().join("secrets.yaml");
        let config = Config::new_with_file_secrets(&config_path, &secrets_path).unwrap();
        config.set_secret("TOKEN", &"old-value").unwrap();
        std::fs::create_dir(config_path.with_extension("save.lock")).unwrap();

        let error = set_extension_with_secrets_and_config(
            &config,
            builtin_entry("new extension", true),
            &[("TOKEN".to_string(), Value::String("new-value".to_string()))],
        )
        .expect_err("config failure must roll back the secret update");

        assert!(error.to_string().contains("Failed to read config file"));
        assert_eq!(config.get_secret::<String>("TOKEN").unwrap(), "old-value");
        assert!(!config_path.exists());
    }

    #[test]
    fn test_config_only_extension_does_not_read_secret_storage() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("config.yaml");
        let secrets_path = temp_dir.path().join("secrets.yaml");
        std::fs::create_dir(&secrets_path).unwrap();
        let config = Config::new_with_file_secrets(&config_path, &secrets_path).unwrap();

        set_extension_with_secrets_and_config(&config, builtin_entry("new extension", true), &[])
            .unwrap();

        assert!(read_extensions(&config).contains_key("newextension"));
    }

    #[test]
    fn test_get_extension_by_name_falls_back_to_available_builtin() {
        fn spawn_builtin(_: tokio::io::DuplexStream, _: tokio::io::DuplexStream) {}
        crate::builtin_extension::register_builtin_extension("autovisualiser", spawn_builtin);

        let extension = get_extension_by_name("autovisualiser").unwrap();

        assert!(matches!(
            extension,
            ExtensionConfig::Builtin { ref name, .. } if name == "autovisualiser"
        ));
    }

    #[test]
    fn test_get_extension_by_name_resolves_saved_entry_by_key() {
        let saved = ExtensionEntry {
            enabled: true,
            activation_paths: None,
            config: ExtensionConfig::Stdio {
                name: "My Tool".to_string(),
                description: "saved description".to_string(),
                cmd: "my-tool".to_string(),
                args: Vec::new(),
                envs: Default::default(),
                env_keys: Vec::new(),
                timeout: Some(120),
                cwd: None,
                bundled: None,
                available_tools: vec!["run".to_string()],
            },
        };
        let key = saved.config.key();
        assert_ne!(key, saved.config.name());

        let (config, _config_file, _secrets_file) = test_config("");
        set_extension_with_config(&config, saved).unwrap();

        let resolved = get_extension_by_name_with_config(&config, &key).unwrap();

        match resolved {
            ExtensionConfig::Stdio {
                timeout,
                available_tools,
                ..
            } => {
                assert_eq!(timeout, Some(120));
                assert_eq!(available_tools, vec!["run".to_string()]);
            }
            other => panic!("expected stdio, got {other:?}"),
        }
    }

    #[test]
    fn test_remove_extension_preserves_unparseable_sibling() {
        let (config, _config_file, _secrets_file) = test_config(
            r#"
extensions:
  valid:
    enabled: true
    type: builtin
    name: valid
    description: valid description
    display_name: Valid
  broken:
    enabled: true
    type: stdio
    name: Broken
    description: missing cmd
    args: []
"#,
        );
        let before = read_extensions(&config);
        let broken_before = before.get("broken").unwrap().clone();

        assert!(remove_extension_with_config(&config, "valid").unwrap());

        let extensions = read_extensions(&config);
        assert!(!extensions.contains_key("valid"));
        assert_eq!(extensions.get("broken").unwrap(), &broken_before);
    }

    #[test]
    fn test_remove_missing_extension_reports_no_change() {
        let (config, _config_file, _secrets_file) = test_config("");

        assert!(!remove_extension_with_config(&config, "missing").unwrap());
    }

    #[derive(Clone, Default)]
    struct CapturedLogs {
        events: Arc<Mutex<Vec<CapturedEvent>>>,
    }

    #[derive(Debug)]
    struct CapturedEvent {
        level: Level,
        message: String,
        key: Option<String>,
    }

    impl<S> tracing_subscriber::Layer<S> for CapturedLogs
    where
        S: Subscriber,
    {
        fn on_event(&self, event: &Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
            let mut visitor = EventVisitor::default();
            event.record(&mut visitor);
            self.events.lock().unwrap().push(CapturedEvent {
                level: *event.metadata().level(),
                message: visitor.message,
                key: visitor.key,
            });
        }
    }

    #[derive(Default)]
    struct EventVisitor {
        message: String,
        key: Option<String>,
    }

    impl tracing::field::Visit for EventVisitor {
        fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
            match field.name() {
                "message" => self.message = value.to_string(),
                "key" => self.key = Some(value.to_string()),
                _ => {}
            }
        }

        fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn fmt::Debug) {
            match field.name() {
                "message" => self.message = format!("{value:?}").trim_matches('"').to_string(),
                "key" => {
                    self.key = Some(format!("{value:?}").trim_matches('"').to_string());
                }
                _ => {}
            }
        }
    }

    #[test]
    fn test_deserialization_failure_logs_offending_key() {
        let (config, _config_file, _secrets_file) = test_config(
            r#"
extensions:
  valid:
    enabled: true
    type: builtin
    name: valid
    description: valid description
    display_name: Valid
  broken:
    enabled: true
    type: stdio
    name: Broken
    description: missing cmd
    args: []
"#,
        );
        let logs = CapturedLogs::default();
        let subscriber = tracing_subscriber::registry().with(logs.clone());

        tracing::subscriber::with_default(subscriber, || {
            let extensions = get_enabled_extensions_with_config(&config);
            // Bundled platform extensions are auto-injected; filter to user-declared entries
            // (Builtin or anything with the test YAML's names) for the invariant check.
            let user_names: Vec<&str> = extensions
                .iter()
                .filter_map(|ext| match ext {
                    ExtensionConfig::Builtin { name, .. } => Some(name.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(
                user_names,
                vec!["valid"],
                "expected only the parseable user extension to be enabled, got {:?}",
                user_names
            );
        });

        let matching_events: Vec<_> = logs
            .events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| {
                event.level == Level::INFO
                    && event
                        .message
                        .contains("Skipping malformed extension config entry")
            })
            .map(|event| event.key.clone())
            .collect();

        let broken_logs: Vec<_> = matching_events
            .iter()
            .filter(|k| k.as_deref() == Some("broken"))
            .collect();
        assert!(
            !broken_logs.is_empty(),
            "expected at least one log naming the broken extension key, got {:?}",
            matching_events
        );
        let other_keys: Vec<_> = matching_events
            .iter()
            .filter(|k| k.as_deref() != Some("broken"))
            .collect();
        assert!(
            other_keys.is_empty(),
            "expected no logs for other extension keys, got {:?}",
            other_keys
        );
    }

    #[test]
    fn test_configured_enabled_state_unknown_extension_is_none() {
        let (config, _config_file, _secrets_file) = test_config("");

        assert_eq!(
            configured_enabled_state(&config, "not_a_real_extension"),
            None
        );
    }

    #[test]
    fn test_default_on_extension_not_disabled_when_config_empty() {
        let (config, _config_file, _secrets_file) = test_config("");

        assert_eq!(configured_enabled_state(&config, "developer"), Some(true));
        assert!(!is_builtin_disabled_by_user(&config, "developer"));
    }

    #[test]
    fn test_default_on_extension_disabled_by_user() {
        let (config, _config_file, _secrets_file) = test_config("");
        set_extension_with_config(&config, builtin_entry("developer", false)).unwrap();

        assert_eq!(configured_enabled_state(&config, "developer"), Some(false));
        assert!(is_builtin_disabled_by_user(&config, "developer"));

        set_extension_enabled_with_config(&config, "developer", true).unwrap();
        assert!(!is_builtin_disabled_by_user(&config, "developer"));
    }

    #[test]
    fn test_default_off_extension_not_treated_as_user_disabled() {
        // summarize is default_enabled: false, so read-migration synthesizes
        // `enabled: false`. That must NOT count as the user disabling it, otherwise
        // an explicit builtins request (e.g. code mode's code_execution) would be
        // skipped even though it is default-off.
        let (config, _config_file, _secrets_file) = test_config("");

        assert_eq!(configured_enabled_state(&config, "summarize"), Some(false));
        assert!(!is_builtin_disabled_by_user(&config, "summarize"));
    }

    #[test]
    fn test_unknown_builtin_disabled_when_explicitly_off() {
        let (config, _config_file, _secrets_file) = test_config("");
        set_extension_with_config(&config, builtin_entry("some_custom_builtin", false)).unwrap();

        assert!(is_builtin_disabled_by_user(&config, "some_custom_builtin"));
    }

    mod activation_paths {
        use super::*;

        #[test]
        fn test_no_activation_paths_always_active() {
            let cwd = TempDir::new().unwrap();

            assert!(extension_active_for_cwd(None, cwd.path()));
        }

        #[test]
        fn test_empty_activation_paths_always_active() {
            let cwd = TempDir::new().unwrap();

            assert!(extension_active_for_cwd(Some(&[]), cwd.path()));
        }

        #[test]
        fn test_cwd_equal_to_activation_path_is_active() {
            let root = TempDir::new().unwrap();
            let paths = vec![root.path().to_string_lossy().to_string()];

            assert!(extension_active_for_cwd(Some(&paths), root.path()));
        }

        #[test]
        fn test_cwd_outside_activation_path_is_inactive() {
            let root = TempDir::new().unwrap();
            let unrelated = TempDir::new().unwrap();
            let paths = vec![root.path().to_string_lossy().to_string()];

            assert!(!extension_active_for_cwd(Some(&paths), unrelated.path()));
        }

        #[test]
        fn test_cwd_descendant_of_activation_path_is_active() {
            let root = TempDir::new().unwrap();
            let nested = root.path().join("nested").join("deeper");
            std::fs::create_dir_all(&nested).unwrap();
            let paths = vec![root.path().to_string_lossy().to_string()];

            assert!(extension_active_for_cwd(Some(&paths), &nested));
        }

        #[test]
        fn test_sibling_directory_with_shared_prefix_is_inactive() {
            // Regression guard for naive string-prefix matching: "/tmp/foo-bar"
            // must not match an activation path of "/tmp/foo" even though the
            // string "/tmp/foo" is a textual prefix of "/tmp/foo-bar".
            let root = TempDir::new().unwrap();
            let scoped = root.path().join("foo");
            let sibling = root.path().join("foo-bar");
            std::fs::create_dir_all(&scoped).unwrap();
            std::fs::create_dir_all(&sibling).unwrap();
            let paths = vec![scoped.to_string_lossy().to_string()];

            assert!(!extension_active_for_cwd(Some(&paths), &sibling));
        }

        #[test]
        fn test_trailing_slash_on_activation_path_is_normalized() {
            let root = TempDir::new().unwrap();
            let mut with_slash = root.path().to_string_lossy().to_string();
            with_slash.push(std::path::MAIN_SEPARATOR);
            let paths = vec![with_slash];

            assert!(extension_active_for_cwd(Some(&paths), root.path()));
        }

        #[cfg(unix)]
        #[test]
        fn test_symlinked_cwd_resolves_to_activation_path() {
            let real_root = TempDir::new().unwrap();
            let outer = TempDir::new().unwrap();
            let symlink_path = outer.path().join("link-to-real-root");
            std::os::unix::fs::symlink(real_root.path(), &symlink_path).unwrap();
            let paths = vec![real_root.path().to_string_lossy().to_string()];

            // cwd is reached through a symlink; only canonicalizing both sides
            // reveals that it actually resolves inside the activation path.
            assert!(extension_active_for_cwd(Some(&paths), &symlink_path));
        }

        #[test]
        fn test_extension_entry_is_active_for_cwd_matches_free_function() {
            let root = TempDir::new().unwrap();
            let entry = builtin_entry_with_activation_paths(
                "scoped",
                true,
                Some(vec![root.path().to_string_lossy().to_string()]),
            );

            assert!(entry.is_active_for_cwd(root.path()));

            let unrelated = TempDir::new().unwrap();
            assert!(!entry.is_active_for_cwd(unrelated.path()));
        }

        #[test]
        fn test_get_enabled_extensions_with_config_for_cwd_filters_scoped_extension() {
            let (config, _config_file, _secrets_file) = test_config("");
            let scoped_root = TempDir::new().unwrap();
            let unrelated = TempDir::new().unwrap();

            set_extension_with_config(&config, builtin_entry("global", true)).unwrap();
            set_extension_with_config(
                &config,
                builtin_entry_with_activation_paths(
                    "scoped",
                    true,
                    Some(vec![scoped_root.path().to_string_lossy().to_string()]),
                ),
            )
            .unwrap();

            let names_for = |cwd: &std::path::Path| -> Vec<String> {
                get_enabled_extensions_with_config_for_cwd(&config, cwd)
                    .into_iter()
                    .filter_map(|ext| match ext {
                        ExtensionConfig::Builtin { name, .. } => Some(name),
                        _ => None,
                    })
                    .collect()
            };

            let in_scope = names_for(scoped_root.path());
            assert!(in_scope.contains(&"global".to_string()));
            assert!(in_scope.contains(&"scoped".to_string()));

            let out_of_scope = names_for(unrelated.path());
            assert!(out_of_scope.contains(&"global".to_string()));
            assert!(!out_of_scope.contains(&"scoped".to_string()));
        }
    }
}
