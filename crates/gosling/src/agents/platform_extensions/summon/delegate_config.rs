// Owns delegate task, provider, model, turn-limit, and working-directory configuration.
// Extracted from `summon.rs` in a behavior-preserving modularization.
// The `summon` compatibility facade keeps these details behind `SummonClient`.

use super::*;

impl SummonClient {
    pub(super) async fn build_task_config(
        &self,
        params: &DelegateParams,
        spec: &DelegateSpec,
        session: &crate::session::Session,
    ) -> Result<TaskConfig, anyhow::Error> {
        let (provider, model_config) = self.resolve_provider(params, spec, session).await?;

        let parent_extensions = EnabledExtensionsState::extensions_or_default(
            Some(&session.extension_data),
            Config::global(),
        );
        let extensions =
            resolve_delegate_extensions(parent_extensions, spec, params.extensions.as_deref())
                .map_err(anyhow::Error::msg)?;

        let max_turns = params.max_turns.unwrap_or_else(|| self.resolve_max_turns());

        if max_turns == 0 || max_turns > u32::MAX as usize {
            anyhow::bail!(
                "max_turns must be between 1 and {} (got {})",
                u32::MAX,
                max_turns
            );
        }

        let effective_working_dir = match &params.working_dir {
            Some(dir) => resolve_working_dir(&session.working_dir, dir)?,
            None => session.working_dir.clone(),
        };

        let task_config = TaskConfig::new(
            provider,
            model_config,
            &session.id,
            &effective_working_dir,
            extensions,
        )
        .with_max_turns(Some(max_turns));

        Ok(task_config)
    }

    pub(super) fn resolve_model_config(
        &self,
        params: &DelegateParams,
        spec: &DelegateSpec,
        session: &crate::session::Session,
        provider_name: &str,
    ) -> Result<gosling_providers::model::ModelConfig, anyhow::Error> {
        let mut model_config = session.model_config.clone().map(Ok).unwrap_or_else(|| {
            crate::model_config::model_config_from_user_config(provider_name, "default")
        })?;

        let override_model = params
            .model
            .clone()
            .or_else(|| spec.model.clone())
            .or_else(|| {
                Config::global()
                    .get_param::<String>("GOSLING_SUBAGENT_MODEL")
                    .ok()
            });

        if let Some(model) = override_model {
            if model != model_config.model_name {
                // Build the new config from scratch so canonical fields
                // (context_limit, max_tokens, reasoning) and env-derived
                // overrides (GOSLING_CONTEXT_LIMIT, GOSLING_MAX_TOKENS) match the
                // overridden model, then preserve session-level state that is
                // not model-specific from the parent.
                let parent = model_config;
                let mut cfg =
                    crate::model_config::model_config_from_user_config(provider_name, &model)?;
                cfg.toolshim = parent.toolshim;
                cfg.toolshim_model = parent.toolshim_model;
                cfg.temperature = cfg.temperature.or(parent.temperature);
                if let Some(parent_params) = parent.request_params {
                    let merged = cfg.request_params.get_or_insert_with(Default::default);
                    for (k, v) in parent_params {
                        merged.insert(k, v);
                    }
                }
                model_config = cfg;
            }
        }

        if let Some(temp) = params.temperature {
            model_config = model_config.with_temperature(Some(temp));
        }

        Ok(model_config)
    }

    async fn resolve_provider(
        &self,
        params: &DelegateParams,
        spec: &DelegateSpec,
        session: &crate::session::Session,
    ) -> Result<
        (
            Arc<dyn crate::providers::base::Provider>,
            gosling_providers::model::ModelConfig,
        ),
        anyhow::Error,
    > {
        let provider_name = params
            .provider
            .clone()
            .or_else(|| {
                Config::global()
                    .get_param::<String>("GOSLING_SUBAGENT_PROVIDER")
                    .ok()
            })
            .or_else(|| session.provider_name.clone())
            .ok_or_else(|| anyhow::anyhow!("No provider configured"))?;

        let model_config = self.resolve_model_config(params, spec, session, &provider_name)?;
        let provider = providers::create(&provider_name, Vec::new()).await?;
        Ok((provider, model_config))
    }

    pub(super) fn resolve_max_turns(&self) -> usize {
        std::env::var("GOSLING_SUBAGENT_MAX_TURNS")
            .ok()
            .and_then(|v| v.parse().ok())
            .or_else(|| {
                Config::global()
                    .get_param::<usize>("GOSLING_SUBAGENT_MAX_TURNS")
                    .ok()
            })
            .unwrap_or(DEFAULT_SUBAGENT_MAX_TURNS)
    }
}

/// Resolve a requested `working_dir` override against the parent session
/// directory. Relative paths are joined to the parent dir; the result must
/// canonicalize to an existing directory contained within the parent dir.
pub(super) fn resolve_working_dir(
    parent_dir: &Path,
    requested: &str,
) -> Result<PathBuf, anyhow::Error> {
    let requested_path = PathBuf::from(requested);
    let resolved = if requested_path.is_absolute() {
        requested_path
    } else {
        parent_dir.join(&requested_path)
    };
    let canonical = resolved
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("working_dir '{}' could not be resolved: {}", requested, e))?;
    let parent_canonical = parent_dir
        .canonicalize()
        .unwrap_or_else(|_| parent_dir.to_path_buf());
    if !canonical.starts_with(&parent_canonical) {
        anyhow::bail!(
            "working_dir '{}' is outside the parent session directory",
            requested
        );
    }
    if !canonical.is_dir() {
        anyhow::bail!("working_dir '{}' is not a directory", requested);
    }
    Ok(canonical)
}
