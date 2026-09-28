use super::base::{Config, ConfigError};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use serde_yaml::Mapping;
use std::env;
use tracing::warn;

const PROVIDERS_CONFIG_KEY: &str = "providers";
const ACTIVE_PROVIDER_KEY: &str = "active_provider";

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ProviderEntry {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub configured: bool,
}

fn parse_providers_map(raw: Mapping) -> IndexMap<String, ProviderEntry> {
    let mut map = IndexMap::with_capacity(raw.len());
    for (k, v) in raw {
        match (k, serde_yaml::from_value::<ProviderEntry>(v)) {
            (serde_yaml::Value::String(key), Ok(entry)) => {
                map.insert(key, entry);
            }
            (k, v) => {
                warn!(
                    key = ?k,
                    value = ?v,
                    "Skipping malformed provider config entry"
                );
            }
        }
    }
    map
}

fn get_providers_map(config: &Config) -> IndexMap<String, ProviderEntry> {
    let raw: Mapping = config
        .get_param(PROVIDERS_CONFIG_KEY)
        .unwrap_or_else(|_| Default::default());
    parse_providers_map(raw)
}

pub fn get_provider_entry(config: &Config, name: &str) -> Option<ProviderEntry> {
    get_providers_map(config).get(name).cloned()
}

pub fn set_provider_entry(
    config: &Config,
    name: &str,
    entry: &ProviderEntry,
) -> Result<(), ConfigError> {
    let name = name.to_string();
    let entry = entry.clone();
    config.update_param::<Mapping, _, _>(PROVIDERS_CONFIG_KEY, |raw| {
        let mut map = parse_providers_map(raw);
        map.insert(name, entry);
        map
    })
}

pub fn get_active_provider(config: &Config) -> Option<String> {
    if let Ok(val) = env::var("GOSLING_PROVIDER") {
        return Some(val);
    }
    if let Ok(val) = config.get_param::<String>(ACTIVE_PROVIDER_KEY) {
        return Some(val);
    }
    config.get_param::<String>("GOSLING_PROVIDER").ok()
}

pub fn get_active_model(config: &Config) -> Option<String> {
    if let Ok(val) = env::var("GOSLING_MODEL") {
        return Some(val);
    }
    if let Some(provider_name) = get_active_provider(config) {
        if let Some(entry) = get_provider_entry(config, &provider_name) {
            if !entry.model.is_empty() {
                return Some(entry.model);
            }
        }
    }
    config.get_param::<String>("GOSLING_MODEL").ok()
}

/// A root-level `GOSLING_PROVIDER` config key shadowed by the `active_provider`
/// key `gosling configure` writes, described for the user. Hand edits of the
/// documented root keys used to be silently ignored. (GSL-PT-20260927-A03)
pub fn ignored_legacy_provider(config: &Config) -> Option<String> {
    if env::var("GOSLING_PROVIDER").is_ok() {
        return None;
    }
    let active = config.get_param::<String>(ACTIVE_PROVIDER_KEY).ok()?;
    let legacy = config.get_param::<String>("GOSLING_PROVIDER").ok()?;
    (active != legacy).then(|| {
        format!(
            "GOSLING_PROVIDER: {legacy} in the config is ignored because {ACTIVE_PROVIDER_KEY}: {active} takes precedence. Edit {ACTIVE_PROVIDER_KEY} or run 'gosling configure'."
        )
    })
}

/// The `GOSLING_MODEL` counterpart of [`ignored_legacy_provider`], shadowed by
/// `providers.<active provider>.model`.
pub fn ignored_legacy_model(config: &Config) -> Option<String> {
    if env::var("GOSLING_MODEL").is_ok() {
        return None;
    }
    let provider = get_active_provider(config)?;
    let legacy = config.get_param::<String>("GOSLING_MODEL").ok()?;
    let model = get_provider_entry(config, &provider)?.model;
    (!model.is_empty() && model != legacy).then(|| {
        format!(
            "GOSLING_MODEL: {legacy} in the config is ignored because {PROVIDERS_CONFIG_KEY}.{provider}.model: {model} takes precedence. Edit {PROVIDERS_CONFIG_KEY}.{provider}.model or run 'gosling configure'."
        )
    })
}

pub fn ignored_legacy_provider_settings(config: &Config) -> Vec<String> {
    ignored_legacy_provider(config)
        .into_iter()
        .chain(ignored_legacy_model(config))
        .collect()
}

pub fn set_active_provider(config: &Config, name: &str, model: &str) -> Result<(), ConfigError> {
    config.set_param(ACTIVE_PROVIDER_KEY, name)?;
    let entry = ProviderEntry {
        enabled: true,
        model: model.to_string(),
        configured: true,
    };
    set_provider_entry(config, name, &entry)
}

pub fn clear_active_provider(config: &Config) -> Result<(), ConfigError> {
    for key in [ACTIVE_PROVIDER_KEY, "GOSLING_PROVIDER", "GOSLING_MODEL"] {
        match config.delete(key) {
            Ok(()) | Err(ConfigError::NotFound(_)) => {}
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    fn new_test_config() -> Config {
        let config_file = NamedTempFile::new().unwrap();
        let secrets_file = NamedTempFile::new().unwrap();
        Config::new_with_file_secrets(config_file.path(), secrets_file.path()).unwrap()
    }

    #[test]
    fn test_set_and_get_provider_entry() {
        let config = new_test_config();
        let entry = ProviderEntry {
            enabled: true,
            model: "gpt-4o".to_string(),
            configured: true,
        };
        set_provider_entry(&config, "openai", &entry).unwrap();

        let loaded = get_provider_entry(&config, "openai").unwrap();
        assert!(loaded.enabled);
        assert_eq!(loaded.model, "gpt-4o");
        assert!(loaded.configured);
    }

    #[test]
    fn test_set_active_provider_writes_structured_keys() {
        let config = new_test_config();
        set_active_provider(&config, "claude-acp", "current").unwrap();

        let active: String = config.get_param(ACTIVE_PROVIDER_KEY).unwrap();
        assert_eq!(active, "claude-acp");

        let entry = get_provider_entry(&config, "claude-acp").unwrap();
        assert!(entry.enabled);
        assert!(entry.configured);
        assert_eq!(entry.model, "current");
    }

    #[test]
    fn test_clear_active_provider_preserves_provider_entries() {
        let config = new_test_config();
        set_active_provider(&config, "openai", "gpt-4o").unwrap();

        clear_active_provider(&config).unwrap();

        assert!(get_active_provider(&config).is_none());
        let entry = get_provider_entry(&config, "openai").unwrap();
        assert_eq!(entry.model, "gpt-4o");
        assert!(entry.configured);
    }

    #[test]
    fn test_clear_active_provider_removes_legacy_keys() {
        let config = new_test_config();
        config.set_param("GOSLING_PROVIDER", "anthropic").unwrap();
        config.set_param("GOSLING_MODEL", "claude").unwrap();

        clear_active_provider(&config).unwrap();

        assert!(get_active_provider(&config).is_none());
        assert!(get_active_model(&config).is_none());
    }

    fn hand_edit(config: &Config, lines: &str) {
        let existing = std::fs::read_to_string(config.path()).unwrap_or_default();
        std::fs::write(config.path(), format!("{existing}{lines}")).unwrap();
    }

    /// GSL-PT-20260927-A03
    #[test]
    fn hand_edited_root_keys_shadowed_by_structured_keys_are_reported() {
        let _guard = env_lock::lock_env([
            ("GOSLING_PROVIDER", None::<&str>),
            ("GOSLING_MODEL", None::<&str>),
        ]);
        let config = new_test_config();
        set_active_provider(&config, "openai", "gpt-4o").unwrap();
        assert!(ignored_legacy_provider_settings(&config).is_empty());

        hand_edit(
            &config,
            "GOSLING_MODEL: playtest-model-b\nGOSLING_PROVIDER: nonsense-prov\n",
        );
        let ignored = ignored_legacy_provider_settings(&config);
        assert_eq!(ignored.len(), 2, "{ignored:?}");
        assert!(ignored[0].starts_with("GOSLING_PROVIDER: nonsense-prov"));
        assert!(ignored[0].contains("active_provider: openai"));
        assert!(ignored[1].starts_with("GOSLING_MODEL: playtest-model-b"));
        assert!(ignored[1].contains("providers.openai.model: gpt-4o"));
        assert_eq!(get_active_provider(&config), Some("openai".to_string()));
        assert_eq!(get_active_model(&config), Some("gpt-4o".to_string()));
    }

    #[test]
    fn root_keys_that_are_in_effect_or_agree_are_not_reported() {
        let _guard = env_lock::lock_env([
            ("GOSLING_PROVIDER", None::<&str>),
            ("GOSLING_MODEL", None::<&str>),
        ]);
        let legacy_only = new_test_config();
        hand_edit(
            &legacy_only,
            "GOSLING_PROVIDER: anthropic\nGOSLING_MODEL: claude\n",
        );
        assert!(ignored_legacy_provider_settings(&legacy_only).is_empty());
        assert_eq!(get_active_model(&legacy_only), Some("claude".to_string()));

        let agreeing = new_test_config();
        set_active_provider(&agreeing, "openai", "gpt-4o").unwrap();
        hand_edit(
            &agreeing,
            "GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\n",
        );
        assert!(ignored_legacy_provider_settings(&agreeing).is_empty());
    }

    #[test]
    fn test_get_active_model_from_provider_entry() {
        let config = new_test_config();
        set_active_provider(&config, "openai", "gpt-4o").unwrap();

        let result = get_active_model(&config);
        assert_eq!(result, Some("gpt-4o".to_string()));
    }

    #[test]
    fn test_multiple_providers_preserved() {
        let config = new_test_config();
        set_active_provider(&config, "openai", "gpt-4o").unwrap();
        set_active_provider(&config, "anthropic", "claude-3-opus").unwrap();

        let openai = get_provider_entry(&config, "openai").unwrap();
        assert_eq!(openai.model, "gpt-4o");
        assert!(openai.configured);

        let anthropic = get_provider_entry(&config, "anthropic").unwrap();
        assert_eq!(anthropic.model, "claude-3-opus");
        assert!(anthropic.configured);

        assert_eq!(get_active_provider(&config), Some("anthropic".to_string()));
    }
}
