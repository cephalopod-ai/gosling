use super::CanonicalModel;
use anyhow::{Context, Result};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::Path;

/// Cached bundled canonical model registry
static BUNDLED_REGISTRY: Lazy<Result<CanonicalModelRegistry>> = Lazy::new(|| {
    const CANONICAL_MODELS_JSON: &str = include_str!("data/canonical_models.json");

    let models: Vec<CanonicalModel> = serde_json::from_str(CANONICAL_MODELS_JSON)
        .context("Failed to parse bundled canonical models JSON")?;

    let mut registry = CanonicalModelRegistry::new();
    for model in models {
        // Extract provider and model from id (format: "provider/model")
        if let Some((provider, model_name)) = model.id.split_once('/') {
            let provider = provider.to_string();
            let model_name = model_name.to_string();
            registry.register(&provider, &model_name, model);
        }
    }

    super::apply_curated_model_contracts(&mut registry);

    Ok(registry)
});

#[derive(Debug, Clone)]
pub struct CanonicalModelRegistry {
    // Key: (provider, model) tuple
    models: HashMap<(String, String), CanonicalModel>,
    // Retired models remain resolvable without returning to model pickers.
    compatibility_models: HashMap<(String, String), CanonicalModel>,
}

impl CanonicalModelRegistry {
    pub fn new() -> Self {
        Self {
            models: HashMap::new(),
            compatibility_models: HashMap::new(),
        }
    }

    pub fn bundled() -> Result<&'static Self> {
        BUNDLED_REGISTRY
            .as_ref()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self> {
        let content = std::fs::read_to_string(path.as_ref())
            .context("Failed to read canonical models file")?;

        let models: Vec<CanonicalModel> =
            serde_json::from_str(&content).context("Failed to parse canonical models JSON")?;

        let mut registry = Self::new();
        for model in models {
            if let Some((provider, model_name)) = model.id.split_once('/') {
                let provider = provider.to_string();
                let model_name = model_name.to_string();
                registry.register(&provider, &model_name, model);
            }
        }

        Ok(registry)
    }

    pub fn to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let mut models: Vec<&CanonicalModel> = self.models.values().collect();
        models.sort_by(|a, b| a.id.cmp(&b.id));

        let mut json = serde_json::to_string_pretty(&models)
            .context("Failed to serialize canonical models")?;
        json.push('\n');

        std::fs::write(path.as_ref(), json).context("Failed to write canonical models file")?;

        Ok(())
    }

    pub fn register(&mut self, provider: &str, model: &str, canonical_model: CanonicalModel) {
        self.models
            .insert((provider.to_string(), model.to_string()), canonical_model);
    }

    pub fn get(&self, provider: &str, model: &str) -> Option<&CanonicalModel> {
        let key = (provider.to_string(), model.to_string());
        self.models
            .get(&key)
            .or_else(|| self.compatibility_models.get(&key))
    }

    pub fn get_active(&self, provider: &str, model: &str) -> Option<&CanonicalModel> {
        self.models.get(&(provider.to_string(), model.to_string()))
    }

    pub(super) fn get_mut(&mut self, provider: &str, model: &str) -> Option<&mut CanonicalModel> {
        self.models
            .get_mut(&(provider.to_string(), model.to_string()))
    }

    pub(super) fn register_compatibility(
        &mut self,
        provider: &str,
        model: &str,
        canonical_model: CanonicalModel,
    ) {
        let key = (provider.to_string(), model.to_string());
        self.models.remove(&key);
        self.compatibility_models.insert(key, canonical_model);
    }

    pub fn get_all_models_for_provider(&self, provider: &str) -> Vec<CanonicalModel> {
        self.models
            .iter()
            .filter(|((p, _), _)| p == provider)
            .map(|(_, model)| model.clone())
            .collect()
    }

    pub fn all_models(&self) -> Vec<&CanonicalModel> {
        self.models.values().collect()
    }

    pub fn count(&self) -> usize {
        self.models.len()
    }
}

impl Default for CanonicalModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialized_registry_ends_with_newline() {
        let file = tempfile::NamedTempFile::new().unwrap();
        CanonicalModelRegistry::new().to_file(file.path()).unwrap();

        assert_eq!(std::fs::read_to_string(file.path()).unwrap(), "[]\n");
    }
}
