use crate::model::ModelConfig;
use crate::thinking::ThinkingEffort;
use serde_json::{json, Value};

pub fn prepare_request(mut payload: Value, model: &ModelConfig) -> Value {
    if model.is_reasoning_model() && payload.get("reasoning_effort").is_none() {
        if let Some(effort) = model.thinking_effort() {
            let glm = model.model_name.starts_with("zai-glm-");
            let effort = match (glm, effort) {
                (true, ThinkingEffort::Off | ThinkingEffort::Low) => "low",
                (true, ThinkingEffort::Medium | ThinkingEffort::High) => "high",
                (true, ThinkingEffort::Max | ThinkingEffort::Ultra) => "max",
                (false, ThinkingEffort::Off | ThinkingEffort::Low | ThinkingEffort::Medium) => {
                    "none"
                }
                (false, ThinkingEffort::High | ThinkingEffort::Max | ThinkingEffort::Ultra) => {
                    "high"
                }
            };
            payload["reasoning_effort"] = json!(effort);
        }
    }

    if let Some(messages) = payload.get_mut("messages").and_then(Value::as_array_mut) {
        for message in messages {
            if message.get("role").and_then(Value::as_str) != Some("assistant") {
                continue;
            }
            let Some(obj) = message.as_object_mut() else {
                continue;
            };
            let Some(reasoning) = obj.remove("reasoning_content") else {
                continue;
            };
            let mut content = vec![json!({
                "type": "thinking",
                "thinking": [{"type": "text", "text": reasoning}],
            })];
            match obj.remove("content") {
                Some(Value::String(text)) => content.push(json!({"type": "text", "text": text})),
                Some(Value::Array(parts)) => content.extend(parts),
                _ => {}
            }
            obj.insert("content".to_owned(), Value::Array(content));
        }
    }
    payload
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::conversation::message::Message;
    use crate::formats::openai::create_request;
    use crate::images::ImageFormat;

    #[test]
    fn replays_thinking_with_text_and_tools_in_mistrals_content_shape() {
        let model = ModelConfig::new("zai-glm-5-3").with_thinking_effort(ThinkingEffort::Max);
        let payload = json!({"messages": [{
            "role": "assistant", "content": "Answer", "reasoning_content": "Reasoning",
            "tool_calls": [{"id": "tool12345", "type": "function", "function": {"name": "read", "arguments": "{}"}}]
        }, {"role": "tool", "tool_call_id": "tool12345", "content": "Result"}]});
        let payload = prepare_request(payload, &model);
        assert_eq!(
            payload["messages"][0]["content"],
            json!([
                {"type": "thinking", "thinking": [{"type": "text", "text": "Reasoning"}]},
                {"type": "text", "text": "Answer"}
            ])
        );
        assert!(payload["messages"][0].get("reasoning_content").is_none());
        assert_eq!(payload["messages"][0]["tool_calls"][0]["id"], "tool12345");
        assert_eq!(payload["messages"][1]["tool_call_id"], "tool12345");
    }

    #[test]
    fn maps_ui_effort_to_supported_mistral_and_glm_levels() {
        for (name, effort, expected) in [
            ("mistral-large-4", ThinkingEffort::Off, "none"),
            ("mistral-large-4", ThinkingEffort::High, "high"),
            ("zai-glm-5-3", ThinkingEffort::Off, "low"),
            ("zai-glm-5-3", ThinkingEffort::High, "high"),
            ("zai-glm-5-3", ThinkingEffort::Ultra, "max"),
        ] {
            let mut model = ModelConfig::new(name).with_thinking_effort(effort);
            model.reasoning = Some(true);
            let payload = create_request(
                &model,
                "System",
                &[Message::user().with_text("Hi")],
                &[],
                &ImageFormat::OpenAi,
                true,
            )
            .unwrap();
            assert_eq!(
                prepare_request(payload, &model)["reasoning_effort"],
                expected
            );
        }
    }

    #[test]
    fn preserves_explicit_parameters_and_omits_effort_for_non_reasoning_models() {
        let mut model =
            ModelConfig::new("mistral-small-2506").with_thinking_effort(ThinkingEffort::High);
        model.reasoning = Some(false);
        assert!(prepare_request(json!({}), &model)
            .get("reasoning_effort")
            .is_none());
        model.reasoning = Some(true);
        assert_eq!(
            prepare_request(json!({"reasoning_effort": "none"}), &model)["reasoning_effort"],
            "none"
        );
    }
}
