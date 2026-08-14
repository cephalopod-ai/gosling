use super::{Config, ConfigError};
use crate::agents::extension::Envs;
use serde::{Deserialize, Serialize};

pub const DEFAULT_ADAPTER_MAX_MESSAGE_BYTES: usize = 1024 * 1024;
pub const MAX_ADAPTER_MAX_MESSAGE_BYTES: usize = 4 * 1024 * 1024;
const DOMAIN_ADAPTERS_CONFIG_KEY: &str = "domain_adapters";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterRegistration {
    pub domain_id: String,
    pub cmd: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub envs: Envs,
    #[serde(default)]
    pub env_keys: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default = "default_adapter_max_message_bytes")]
    pub max_message_bytes: usize,
}

fn default_adapter_max_message_bytes() -> usize {
    DEFAULT_ADAPTER_MAX_MESSAGE_BYTES
}

impl AdapterRegistration {
    pub fn validate(&self) -> Result<(), String> {
        if self.domain_id.trim().is_empty() {
            return Err("domainId must not be empty".to_string());
        }
        if self.cmd.trim().is_empty() {
            return Err("cmd must not be empty".to_string());
        }
        if self.max_message_bytes == 0 || self.max_message_bytes > MAX_ADAPTER_MAX_MESSAGE_BYTES {
            return Err(format!(
                "maxMessageBytes must be between 1 and {MAX_ADAPTER_MAX_MESSAGE_BYTES}"
            ));
        }
        if self.env_keys.iter().any(|key| key.trim().is_empty()) {
            return Err("envKeys must not contain empty values".to_string());
        }
        self.envs.validate().map_err(|error| error.to_string())
    }
}

pub fn get_domain_adapter_registration(
    config: &Config,
    domain_id: &str,
) -> Result<Option<AdapterRegistration>, ConfigError> {
    let registrations =
        match config.get_param::<Vec<AdapterRegistration>>(DOMAIN_ADAPTERS_CONFIG_KEY) {
            Ok(registrations) => registrations,
            Err(ConfigError::NotFound(_)) => return Ok(None),
            Err(error) => return Err(error),
        };

    let mut matching = registrations
        .into_iter()
        .filter(|registration| registration.domain_id == domain_id);
    let registration = matching.next();
    if matching.next().is_some() {
        return Err(ConfigError::DeserializeError(format!(
            "domain adapter registration {domain_id:?} is duplicated"
        )));
    }
    if let Some(registration) = &registration {
        registration
            .validate()
            .map_err(ConfigError::DeserializeError)?;
    }
    Ok(registration)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn registration() -> AdapterRegistration {
        AdapterRegistration {
            domain_id: "neutral-fixture".to_string(),
            cmd: "adapter".to_string(),
            args: Vec::new(),
            envs: Envs::default(),
            env_keys: Vec::new(),
            timeout: Some(30),
            cwd: None,
            max_message_bytes: DEFAULT_ADAPTER_MAX_MESSAGE_BYTES,
        }
    }

    fn config() -> (TempDir, Config) {
        let directory = TempDir::new().unwrap();
        let config =
            Config::new(directory.path().join("config.yaml"), "domain-adapter-test").unwrap();
        (directory, config)
    }

    #[test]
    fn registration_defaults_max_message_bytes() {
        let registration: AdapterRegistration = serde_json::from_value(serde_json::json!({
            "domainId": "neutral-fixture",
            "cmd": "adapter"
        }))
        .unwrap();

        assert_eq!(
            registration.max_message_bytes,
            DEFAULT_ADAPTER_MAX_MESSAGE_BYTES
        );
    }

    #[test]
    fn lookup_rejects_oversized_registration_before_spawn() {
        let (_directory, config) = config();
        let mut registration = registration();
        registration.max_message_bytes = MAX_ADAPTER_MAX_MESSAGE_BYTES + 1;
        config
            .set_param(DOMAIN_ADAPTERS_CONFIG_KEY, vec![registration])
            .unwrap();

        let error = get_domain_adapter_registration(&config, "neutral-fixture").unwrap_err();

        assert!(error.to_string().contains("maxMessageBytes"));
    }

    #[test]
    fn lookup_fails_closed_for_duplicate_domain_id() {
        let (_directory, config) = config();
        config
            .set_param(
                DOMAIN_ADAPTERS_CONFIG_KEY,
                vec![registration(), registration()],
            )
            .unwrap();

        let error = get_domain_adapter_registration(&config, "neutral-fixture").unwrap_err();

        assert!(error.to_string().contains("duplicated"));
    }

    #[test]
    fn lookup_returns_none_when_no_registry_exists() {
        let (_directory, config) = config();

        assert!(get_domain_adapter_registration(&config, "neutral-fixture")
            .unwrap()
            .is_none());
    }
}
