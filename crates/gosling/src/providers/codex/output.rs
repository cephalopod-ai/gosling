use gosling_providers::errors::ProviderError;
use std::process::ExitStatus;

fn extract_error(parsed: &serde_json::Value) -> Option<String> {
    parsed
        .get("message")
        .and_then(|message| message.as_str())
        .map(str::to_string)
        .or_else(|| {
            parsed
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(|message| message.as_str())
                .map(str::to_string)
        })
}

fn classify_error(message: String) -> ProviderError {
    if message.contains("context window") || message.contains("context_length_exceeded") {
        ProviderError::ContextLengthExceeded(message)
    } else if message.to_lowercase().contains("rate limit") {
        ProviderError::RateLimitExceeded {
            details: message,
            retry_delay: None,
        }
    } else {
        ProviderError::RequestFailed(format!("Codex CLI error: {message}"))
    }
}

pub(super) fn terminal_event_error(lines: &[String]) -> Option<ProviderError> {
    lines.iter().find_map(|line| {
        let parsed = serde_json::from_str::<serde_json::Value>(line).ok()?;
        let event_type = parsed.get("type").and_then(|value| value.as_str())?;
        if event_type != "error" && !event_type.ends_with(".failed") {
            return None;
        }
        let message = extract_error(&parsed)
            .unwrap_or_else(|| format!("Codex CLI emitted terminal event {event_type}"));
        Some(classify_error(message))
    })
}

pub(super) fn command_failure(exit_status: ExitStatus, stderr: &str) -> ProviderError {
    let stderr = stderr.trim();
    let detail = if stderr.is_empty() {
        String::new()
    } else {
        format!(": {stderr}")
    };
    ProviderError::RequestFailed(format!(
        "Codex command failed with exit code: {:?}{detail}",
        exit_status.code()
    ))
}
