use crate::model::ModelConfig;
use crate::thinking::ThinkingEffort;
use serde_json::Value;

pub fn reasoning_effort(model: &str, effort: ThinkingEffort) -> &'static str {
    match effort {
        // Meta's hosted models reject "none"; use their shortest reasoning pass.
        ThinkingEffort::Off => "minimal",
        ThinkingEffort::Low => "low",
        ThinkingEffort::Medium => "medium",
        ThinkingEffort::High => "high",
        ThinkingEffort::Max | ThinkingEffort::Ultra => {
            if matches!(model, "muse-wonder-1.0" | "muse-spark-1.3") {
                "max"
            } else {
                "xhigh"
            }
        }
    }
}

pub fn prepare_request(mut payload: Value, model: &ModelConfig) -> Value {
    if payload.get("reasoning_effort").is_none() {
        if let Some(effort) = model.thinking_effort() {
            payload["reasoning_effort"] = reasoning_effort(&model.model_name, effort).into();
        }
    }
    payload
}
