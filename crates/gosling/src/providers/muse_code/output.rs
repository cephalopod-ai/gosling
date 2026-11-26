use gosling_providers::conversation::token_usage::Usage;
use gosling_providers::errors::ProviderError;
use serde_json::Value;

use super::MAX_EVENT_BYTES;

#[derive(Default)]
pub(super) struct MuseOutput {
    linked_run_id: Option<String>,
    run_id: Option<String>,
    streamed_text: String,
    terminal_remainder: Option<String>,
    usage: Usage,
}

impl MuseOutput {
    pub(super) fn push(
        &mut self,
        event: Value,
        accurate_usage: bool,
    ) -> Result<Option<String>, ProviderError> {
        let event_type = event["payload_type"]
            .as_str()
            .ok_or_else(|| protocol_error("emitted an invalid event envelope"))?;
        if event["schema_version"].as_u64() != Some(1)
            || event["payload_schema_version"].as_u64() != Some(1)
        {
            return Err(ProviderError::ExecutionError(
                "Unsupported Muse Code event schema. Update gosling or use a compatible Muse CLI."
                    .to_string(),
            ));
        }

        let terminal = event_type.starts_with("run.terminal.");
        if !terminal
            && !matches!(
                event_type,
                "session.run.linked" | "run.lifecycle.started" | "run.output.delta" | "run.usage"
            )
        {
            return Ok(None);
        }
        let payload = &event["payload"];
        let event_run = payload["run_stream"]["id"]
            .as_str()
            .filter(|id| !id.trim().is_empty() && payload["run_stream"]["kind"] == "run")
            .ok_or_else(|| protocol_error("reported a run event without a valid identity"))?;
        if event_type == "session.run.linked" {
            if self.linked_run_id.is_none() {
                if self.run_id.as_deref().is_some_and(|run| run != event_run) {
                    return Err(protocol_error(
                        "linked a different run after starting the root run",
                    ));
                }
                self.linked_run_id = Some(event_run.to_string());
            }
            return Ok(None);
        }
        if self
            .linked_run_id
            .as_deref()
            .is_some_and(|linked| event_run != linked)
        {
            return Ok(None);
        }
        if event_type == "run.lifecycle.started" {
            self.run_id.get_or_insert_with(|| event_run.to_string());
            return Ok(None);
        }
        let run_id = self
            .run_id
            .as_deref()
            .ok_or_else(|| protocol_error("reported run output before starting a run"))?;
        if event_run != run_id {
            return Ok(None);
        }
        if self.terminal_remainder.is_some() {
            return Err(protocol_error(if terminal {
                "returned multiple terminal results"
            } else {
                "emitted run output after its terminal result"
            }));
        }

        match event_type {
            "run.output.delta" => {
                let text = payload["text"]
                    .as_str()
                    .ok_or_else(|| protocol_error("emitted a text delta with no text"))?;
                if self.streamed_text.len() + text.len() > MAX_EVENT_BYTES {
                    return Err(protocol_error(
                        "exceeded the limit for accumulated response text",
                    ));
                }
                self.streamed_text.push_str(text);
                Ok((!text.is_empty()).then(|| text.to_string()))
            }
            "run.usage" => {
                // A run.usage event is a snapshot of the entire run, including
                // child calls if Muse counted them within the root snapshot.
                if accurate_usage {
                    self.usage = run_usage(payload);
                }
                Ok(None)
            }
            "run.terminal.completed" if payload["terminal"] == "completed" => {
                let text = payload["text"]
                    .as_str()
                    .filter(|text| !text.trim().is_empty())
                    .ok_or_else(|| protocol_error("returned an empty terminal result"))?;
                // The stream can append to the visible prefix but cannot replace it.
                let remainder = text.strip_prefix(&self.streamed_text).ok_or_else(|| {
                    protocol_error("returned a terminal result inconsistent with its streamed text")
                })?;
                self.terminal_remainder = Some(remainder.to_string());
                Ok(None)
            }
            "run.terminal.cancelled" => Err(protocol_error("cancelled the run")),
            _ => Err(super::run_failure(
                payload["reason"].as_str().unwrap_or_default(),
            )),
        }
    }

    pub(super) fn finish(self) -> Result<(String, Usage), ProviderError> {
        let remainder = self
            .terminal_remainder
            .ok_or_else(|| protocol_error("exited without a completed terminal result"))?;
        Ok((remainder, self.usage))
    }
}

fn run_usage(payload: &Value) -> Usage {
    let Some(models) = payload["models"]
        .as_array()
        .filter(|models| !models.is_empty())
    else {
        return Usage::default();
    };
    let sum = |key| {
        models.iter().try_fold(0i32, |sum, model| {
            let tokens = i32::try_from(model[key].as_u64()?).ok()?;
            sum.checked_add(tokens)
        })
    };
    let input = sum("input_tokens");
    let output = sum("output_tokens");
    let total = input.zip(output).and_then(|(a, b)| a.checked_add(b));
    Usage {
        input_tokens: input,
        output_tokens: output,
        total_tokens: total,
        cache_read_input_tokens: input.and_then(|_| {
            models.iter().try_fold(0i32, |sum, model| {
                let cached = model["cached_tokens"].as_u64()?;
                let input = model["input_tokens"].as_u64()?;
                (cached <= input).then_some(())?;
                sum.checked_add(i32::try_from(cached).ok()?)
            })
        }),
        cache_write_input_tokens: None,
    }
}

pub(super) fn protocol_error(message: &str) -> ProviderError {
    ProviderError::ExecutionError(format!("Muse Code {message}."))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn malformed_cache_breakdown_does_not_invalidate_known_run_totals() {
        let usage = run_usage(&json!({"models": [
            {"input_tokens": 4, "output_tokens": 1, "cached_tokens": 5},
            {"input_tokens": 10, "output_tokens": 2, "cached_tokens": 0}
        ]}));
        assert_eq!(usage.input_tokens, Some(14));
        assert_eq!(usage.output_tokens, Some(3));
        assert_eq!(usage.total_tokens, Some(17));
        assert_eq!(usage.cache_read_input_tokens, None);
    }
}
