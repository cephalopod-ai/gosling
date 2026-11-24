use gosling_providers::conversation::token_usage::Usage;
use gosling_providers::errors::ProviderError;
use serde_json::Value;

use super::MAX_EVENT_BYTES;

#[derive(Default)]
pub(super) struct MuseOutput {
    linked_run_id: Option<String>,
    run_id: Option<String>,
    streamed_text: String,
    terminal_text: Option<String>,
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

        let payload = &event["payload"];
        let event_run = payload["run_stream"]["id"]
            .as_str()
            .filter(|id| !id.is_empty() && payload["run_stream"]["kind"] == "run");
        if event_type == "session.run.linked"
            && self.run_id.is_none()
            && self.linked_run_id.is_none()
        {
            self.linked_run_id = event_run.map(str::to_string);
        }
        if event_type == "run.lifecycle.started" && self.run_id.is_none() {
            let id =
                event_run.ok_or_else(|| protocol_error("started a run without an identity"))?;
            if self
                .linked_run_id
                .as_deref()
                .is_none_or(|linked| linked == id)
            {
                self.run_id = Some(id.to_string());
            }
            return Ok(None);
        }

        if !matches!(event_type, "run.output.delta" | "run.usage")
            && !event_type.starts_with("run.terminal.")
        {
            return Ok(None);
        }
        if self
            .linked_run_id
            .as_deref()
            .is_some_and(|linked| event_run != Some(linked))
        {
            return Ok(None);
        }
        let run_id = self
            .run_id
            .as_deref()
            .ok_or_else(|| protocol_error("reported run output before starting a run"))?;
        if event_run != Some(run_id) {
            return Ok(None);
        }
        if self.terminal_text.is_some() {
            return Err(protocol_error(if event_type.starts_with("run.terminal.") {
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
                self.terminal_text = Some(text.to_string());
                Ok(None)
            }
            "run.terminal.cancelled" => Err(protocol_error("cancelled the run")),
            _ => Err(super::run_failure(
                payload["reason"].as_str().unwrap_or_default(),
            )),
        }
    }

    pub(super) fn finish(self) -> Result<(String, Usage), ProviderError> {
        let text = self
            .terminal_text
            .ok_or_else(|| protocol_error("exited without a completed terminal result"))?;
        // Provider message streams append text. A divergent final answer
        // cannot replace an already visible prefix through this interface.
        let remainder = text.strip_prefix(&self.streamed_text).ok_or_else(|| {
            protocol_error("returned a terminal result inconsistent with its streamed text")
        })?;
        Ok((remainder.to_string(), self.usage))
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
        cache_read_input_tokens: sum("cached_tokens")
            .zip(input)
            .and_then(|(cached, input)| (cached <= input).then_some(cached)),
        cache_write_input_tokens: None,
    }
}

pub(super) fn protocol_error(message: &str) -> ProviderError {
    ProviderError::ExecutionError(format!("Muse Code {message}."))
}
