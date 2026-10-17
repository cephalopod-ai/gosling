use gosling::config::{Config, ConfigResolutionScope};
use gosling::model_config::{
    model_config_from_user_config, model_config_from_user_config_with_session_settings,
};
use serde_json::{json, Value};
use std::collections::HashMap;

fn scope(global_limit: Option<usize>) -> ConfigResolutionScope {
    let mut values = HashMap::from([(
        "providers".to_string(),
        json!({"together": {"model": "fixture-spark", "context_limit": 1048576}}),
    )]);
    if let Some(limit) = global_limit {
        values.insert("GOSLING_CONTEXT_LIMIT".to_string(), Value::from(limit));
    }
    ConfigResolutionScope::new(
        [
            "providers",
            "GOSLING_CONTEXT_LIMIT",
            "GOSLING_MAX_TOKENS",
            "GOSLING_TEMPERATURE",
            "GOSLING_THINKING_EFFORT",
            "GOSLING_TOOLSHIM",
            "GOSLING_TOOLSHIM_OLLAMA_MODEL",
        ]
        .map(str::to_string),
        HashMap::new(),
        values,
    )
}

#[tokio::test]
async fn saved_default_context_is_scoped_and_session_overrides_take_priority() -> anyhow::Result<()>
{
    Config::with_resolution_scope(scope(None), async {
        assert_eq!(
            model_config_from_user_config("together", "fixture-spark")?.context_limit,
            Some(1_048_576)
        );
        assert_eq!(
            model_config_from_user_config("other-provider", "fixture-spark")?.context_limit,
            None
        );
        assert_eq!(
            model_config_from_user_config("together", "other-model")?.context_limit,
            None
        );
        assert_eq!(
            model_config_from_user_config_with_session_settings(
                "together",
                "fixture-spark",
                None,
                None,
                Some(64_000)
            )?
            .context_limit,
            Some(64_000)
        );
        let error = model_config_from_user_config_with_session_settings(
            "together",
            "fixture-spark",
            None,
            None,
            Some(0),
        )
        .unwrap_err();
        assert!(error.to_string().contains("greater than 0"));
        Ok(())
    })
    .await
}

#[tokio::test]
async fn global_context_configuration_precedes_saved_default() -> anyhow::Result<()> {
    Config::with_resolution_scope(scope(Some(32_000)), async {
        assert_eq!(
            model_config_from_user_config("together", "fixture-spark")?.context_limit,
            Some(32_000)
        );
        assert_eq!(
            model_config_from_user_config_with_session_settings(
                "together",
                "fixture-spark",
                None,
                None,
                Some(64_000)
            )?
            .context_limit,
            Some(64_000)
        );
        Ok(())
    })
    .await
}
