use super::{
    CanonicalModel, CanonicalModelRegistry, Limit, Modalities, Modality, Pricing, ThinkingMode,
};

struct AnthropicContract {
    model: &'static str,
    context: usize,
    output: usize,
    thinking_mode: Option<ThinkingMode>,
}

const ANTHROPIC_CONTRACTS: &[AnthropicContract] = &[
    AnthropicContract {
        model: "claude-fable-5",
        context: 1_000_000,
        output: 128_000,
        thinking_mode: Some(ThinkingMode::AlwaysOnAdaptive),
    },
    AnthropicContract {
        model: "claude-haiku-4.5",
        context: 200_000,
        output: 64_000,
        thinking_mode: None,
    },
    AnthropicContract {
        model: "claude-opus-4.1",
        context: 200_000,
        output: 32_000,
        thinking_mode: None,
    },
    AnthropicContract {
        model: "claude-opus-4.5",
        context: 200_000,
        output: 64_000,
        thinking_mode: None,
    },
    AnthropicContract {
        model: "claude-opus-4.6",
        context: 1_000_000,
        output: 128_000,
        thinking_mode: Some(ThinkingMode::Adaptive),
    },
    AnthropicContract {
        model: "claude-opus-4.7",
        context: 1_000_000,
        output: 128_000,
        thinking_mode: Some(ThinkingMode::Adaptive),
    },
    AnthropicContract {
        model: "claude-opus-4.8",
        context: 1_000_000,
        output: 128_000,
        thinking_mode: Some(ThinkingMode::Adaptive),
    },
    AnthropicContract {
        model: "claude-sonnet-4.5",
        context: 200_000,
        output: 64_000,
        thinking_mode: None,
    },
    AnthropicContract {
        model: "claude-sonnet-4.6",
        context: 1_000_000,
        output: 128_000,
        thinking_mode: Some(ThinkingMode::Adaptive),
    },
    AnthropicContract {
        model: "claude-sonnet-5",
        context: 1_000_000,
        output: 128_000,
        thinking_mode: Some(ThinkingMode::Adaptive),
    },
];

pub fn apply_curated_model_contracts(registry: &mut CanonicalModelRegistry) {
    for contract in ANTHROPIC_CONTRACTS {
        if let Some(model) = registry.get_mut("anthropic", contract.model) {
            model.reasoning = Some(true);
            model.thinking_mode = contract.thinking_mode;
            model.limit = Limit {
                context: contract.context,
                output: Some(contract.output),
            };
        }
    }

    for model in retired_anthropic_models() {
        let model_name = model
            .id
            .strip_prefix("anthropic/")
            .expect("retired Anthropic model id must use the Anthropic prefix")
            .to_string();
        registry.register_compatibility("anthropic", &model_name, model);
    }
}

fn retired_anthropic_models() -> Vec<CanonicalModel> {
    vec![
        retired_sonnet(
            "claude-3.5-sonnet",
            "Claude Sonnet 3.5 v2",
            false,
            "2024-04-30",
            "2024-10-22",
            8_192,
        ),
        retired_sonnet(
            "claude-3.7-sonnet",
            "Claude Sonnet 3.7",
            true,
            "2024-10-31",
            "2025-02-19",
            64_000,
        ),
        retired_sonnet(
            "claude-sonnet-4",
            "Claude Sonnet 4",
            true,
            "2025-03-31",
            "2025-05-22",
            64_000,
        ),
    ]
}

fn retired_sonnet(
    id: &str,
    name: &str,
    reasoning: bool,
    knowledge: &str,
    release_date: &str,
    output: usize,
) -> CanonicalModel {
    CanonicalModel {
        id: format!("anthropic/{id}"),
        name: name.to_string(),
        family: Some("claude-sonnet".to_string()),
        attachment: Some(true),
        reasoning: Some(reasoning),
        thinking_mode: None,
        tool_call: true,
        temperature: Some(true),
        knowledge: Some(knowledge.to_string()),
        release_date: Some(release_date.to_string()),
        last_updated: Some(release_date.to_string()),
        modalities: Modalities {
            input: vec![Modality::Text, Modality::Image, Modality::Pdf],
            output: vec![Modality::Text],
        },
        open_weights: Some(false),
        cost: Pricing {
            input: Some(3.0),
            output: Some(15.0),
            cache_read: Some(0.3),
            cache_write: Some(3.75),
        },
        limit: Limit {
            context: 200_000,
            output: Some(output),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn current_model(id: &str) -> CanonicalModel {
        CanonicalModel {
            id: format!("anthropic/{id}"),
            name: id.to_string(),
            family: None,
            attachment: None,
            reasoning: None,
            thinking_mode: None,
            tool_call: false,
            temperature: None,
            knowledge: None,
            release_date: None,
            last_updated: None,
            modalities: Modalities::default(),
            open_weights: None,
            cost: Pricing::default(),
            limit: Limit {
                context: 123,
                output: Some(456),
            },
        }
    }

    #[test]
    fn corrects_current_anthropic_contracts() {
        let mut registry = CanonicalModelRegistry::new();
        registry.register(
            "anthropic",
            "claude-sonnet-4.5",
            current_model("claude-sonnet-4.5"),
        );
        registry.register(
            "anthropic",
            "claude-sonnet-5",
            current_model("claude-sonnet-5"),
        );

        apply_curated_model_contracts(&mut registry);

        let sonnet_45 = registry.get("anthropic", "claude-sonnet-4.5").unwrap();
        assert_eq!(sonnet_45.limit.context, 200_000);
        assert_eq!(sonnet_45.limit.output, Some(64_000));
        assert_eq!(sonnet_45.thinking_mode, None);

        let sonnet_5 = registry.get("anthropic", "claude-sonnet-5").unwrap();
        assert_eq!(sonnet_5.limit.context, 1_000_000);
        assert_eq!(sonnet_5.limit.output, Some(128_000));
        assert_eq!(sonnet_5.thinking_mode, Some(ThinkingMode::Adaptive));
    }

    #[test]
    fn retired_models_resolve_without_becoming_recommendations() {
        let mut registry = CanonicalModelRegistry::new();
        registry.register(
            "anthropic",
            "claude-3.5-sonnet",
            retired_sonnet(
                "claude-3.5-sonnet",
                "stale active entry",
                false,
                "2024-04-30",
                "2024-10-22",
                8_192,
            ),
        );
        apply_curated_model_contracts(&mut registry);

        assert!(registry.get("anthropic", "claude-3.5-sonnet").is_some());
        assert!(registry.get("anthropic", "claude-3.7-sonnet").is_some());
        assert!(registry.get("anthropic", "claude-sonnet-4").is_some());
        assert!(registry
            .get_active("anthropic", "claude-3.5-sonnet")
            .is_none());
        assert!(registry.get_all_models_for_provider("anthropic").is_empty());
        assert_eq!(registry.count(), 0);

        let file = tempfile::NamedTempFile::new().unwrap();
        registry.to_file(file.path()).unwrap();
        let serialized = std::fs::read_to_string(file.path()).unwrap();
        assert!(!serialized.contains("claude-3.5-sonnet"));
    }
}
