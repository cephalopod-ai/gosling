//! Calls the local-LLM summarizer endpoint and parses its response.
//!
//! Routed through Gosling's existing OpenAI-compatible provider abstraction
//! ([`OpenAiCompatibleProvider`]) so this needs no new HTTP plumbing — just a
//! client pointed at a different (local) endpoint than the main foundation
//! model. Every failure mode (endpoint down, slow, malformed JSON) resolves
//! to `None`; the caller's deterministic fallback is always the safety net.

use std::time::Duration;

use gosling_providers::api_client::{ApiClient, AuthMethod};
use gosling_providers::base::Provider;
use gosling_providers::model::ModelConfig;
use gosling_providers::openai_compatible::OpenAiCompatibleProvider;
use indoc::indoc;

use crate::conversation::message::Message;

use super::schema::WorkerResponse;

const SUMMARIZER_PROVIDER_NAME: &str = "gosling-summarizer";

/// Bounds the summary itself; the worker isn't meant to write a novel.
const MAX_RESPONSE_TOKENS: i32 = 700;

const SYSTEM_PROMPT: &str = indoc! {r#"
    You compress older parts of a coding agent's conversation history into a
    faithful digest for its long-term memory, and extract any durable facts
    worth recalling later.

    Reply with ONLY JSON matching this shape — no prose, no markdown fences:
    {"summary": "...", "facts": [{"content": "...", "type": "fact|decision|preference|entity", "confidence": 0.0}]}

    Rules:
    - "summary" must preserve named entities, file paths, config values, and
      stated constraints verbatim. Do not paraphrase them away.
    - "facts" may be empty. Only extract durable facts, decisions, or
      preferences worth recalling in a future conversation; skip ephemeral or
      one-off content.
"#};

/// Config for the summarizer's local endpoint, separate from the main
/// foundation-model provider (different client, different rate limit).
#[derive(Debug, Clone)]
pub struct SummarizerConfig {
    pub endpoint: String,
    pub model: String,
    pub timeout_ms: u64,
}

/// Runs the worker against `text`, returning `None` on any failure (endpoint
/// unreachable, timeout, or a response that isn't valid JSON matching
/// [`WorkerResponse`]). Never returns an `Err` — there is nothing for a
/// caller to do with a worker failure except fall back to the deterministic
/// path, so failures are collapsed to `None` here.
pub async fn summarize(text: &str, config: &SummarizerConfig) -> Option<WorkerResponse> {
    let provider = build_provider(&config.endpoint).ok()?;
    let model_config = ModelConfig::new(&config.model).with_max_tokens(Some(MAX_RESPONSE_TOKENS));
    let messages = [Message::user().with_text(text)];

    let call = provider.complete(&model_config, SYSTEM_PROMPT, &messages, &[]);
    let (response, _usage) = tokio::time::timeout(Duration::from_millis(config.timeout_ms), call)
        .await
        .ok()?
        .ok()?;

    parse_response(&response.as_concat_text())
}

fn build_provider(endpoint: &str) -> anyhow::Result<OpenAiCompatibleProvider> {
    // The outer `tokio::time::timeout` in `summarize` is the real deadline;
    // this is just a generous upper bound so a hung connection doesn't leak.
    let api_client = ApiClient::with_timeout_and_tls(
        endpoint.to_string(),
        AuthMethod::NoAuth,
        Duration::from_secs(60),
        None,
    )?;
    Ok(OpenAiCompatibleProvider::new(
        SUMMARIZER_PROVIDER_NAME.to_string(),
        api_client,
        String::new(),
    )
    // Non-streaming: this is a one-shot background call, not something a
    // user watches token-by-token, and plain JSON is simpler to parse and
    // to mock in tests than SSE framing.
    .with_supports_streaming(false))
}

fn parse_response(raw: &str) -> Option<WorkerResponse> {
    let cleaned = strip_code_fence(raw.trim());
    serde_json::from_str(cleaned).ok()
}

/// The worker is instructed to never wrap its JSON in a code fence, but
/// small local models don't always follow instructions faithfully — strip
/// one defensively rather than treating it as a hard parse failure.
fn strip_code_fence(s: &str) -> &str {
    let Some(rest) = s.strip_prefix("```") else {
        return s;
    };
    let rest = rest.strip_prefix("json").unwrap_or(rest);
    rest.trim().trim_end_matches("```").trim()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_response_accepts_raw_json() {
        let raw = r#"{"summary": "digest", "facts": []}"#;
        let parsed = parse_response(raw).unwrap();
        assert_eq!(parsed.summary, "digest");
    }

    #[test]
    fn parse_response_strips_json_code_fence() {
        let raw = "```json\n{\"summary\": \"digest\", \"facts\": []}\n```";
        let parsed = parse_response(raw).unwrap();
        assert_eq!(parsed.summary, "digest");
    }

    #[test]
    fn parse_response_strips_bare_code_fence() {
        let raw = "```\n{\"summary\": \"digest\", \"facts\": []}\n```";
        let parsed = parse_response(raw).unwrap();
        assert_eq!(parsed.summary, "digest");
    }

    #[test]
    fn parse_response_rejects_prose() {
        assert!(parse_response("Sure, here's a summary: the user did stuff.").is_none());
    }

    #[tokio::test]
    async fn summarize_returns_none_when_endpoint_unreachable() {
        let config = SummarizerConfig {
            endpoint: "http://127.0.0.1:1".to_string(),
            model: "test-model".to_string(),
            timeout_ms: 500,
        };
        assert!(summarize("some older conversation text", &config)
            .await
            .is_none());
    }

    fn chat_completion_response(content: &str) -> serde_json::Value {
        serde_json::json!({
            "choices": [{
                "message": { "role": "assistant", "content": content },
            }],
            "usage": { "prompt_tokens": 10, "completion_tokens": 10, "total_tokens": 20 },
        })
    }

    #[tokio::test]
    async fn summarize_returns_digest_on_well_formed_response() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let body = chat_completion_response(
            r#"{"summary": "Discussed the gosling rebrand.", "facts": [{"content": "Project renamed to gosling", "type": "fact", "confidence": 0.9}]}"#,
        );
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let config = SummarizerConfig {
            endpoint: server.uri(),
            model: "test-model".to_string(),
            timeout_ms: 2_000,
        };
        let response = summarize("some older conversation text", &config)
            .await
            .expect("well-formed endpoint response should parse");
        assert_eq!(response.summary, "Discussed the gosling rebrand.");
        assert_eq!(response.facts.len(), 1);
    }

    #[tokio::test]
    async fn summarize_returns_none_on_malformed_json_body() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        // The endpoint responds successfully, but the model ignored
        // instructions and replied with prose instead of JSON.
        let body = chat_completion_response("Sure! Here's a summary of what happened.");
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let config = SummarizerConfig {
            endpoint: server.uri(),
            model: "test-model".to_string(),
            timeout_ms: 2_000,
        };
        assert!(summarize("some older conversation text", &config)
            .await
            .is_none());
    }

    #[tokio::test]
    async fn summarize_returns_none_on_timeout() {
        use std::time::Duration as StdDuration;
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let server = MockServer::start().await;
        let body = chat_completion_response(r#"{"summary": "too slow", "facts": []}"#);
        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(body)
                    .set_delay(StdDuration::from_millis(500)),
            )
            .mount(&server)
            .await;

        let config = SummarizerConfig {
            endpoint: server.uri(),
            model: "test-model".to_string(),
            timeout_ms: 50,
        };
        assert!(summarize("some older conversation text", &config)
            .await
            .is_none());
    }
}
