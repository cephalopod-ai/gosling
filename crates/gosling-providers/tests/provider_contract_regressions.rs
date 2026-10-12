use futures::StreamExt;
use gosling_providers::anthropic::{AnthropicProvider, AnthropicProviderBuilder};
use gosling_providers::api_client::{ApiClient, AuthMethod};
use gosling_providers::base::Provider;
use gosling_providers::conversation::message::{Message, MessageContent};
use gosling_providers::errors::ProviderError;
use gosling_providers::formats::openai::{response_to_message, response_to_streaming_message};
use gosling_providers::formats::openai_responses::{
    create_responses_request, responses_api_to_message, responses_api_to_streaming_message,
    ResponsesApiResponse,
};
use gosling_providers::model::ModelConfig;
use serde_json::json;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};

fn anthropic(server: &MockServer) -> AnthropicProvider {
    AnthropicProviderBuilder::new(
        ApiClient::new_with_tls(server.uri(), AuthMethod::NoAuth, None).unwrap(),
    )
    .custom_models(Some(vec!["static-fallback".to_owned()]))
    .build()
}

#[tokio::test]
async fn anthropic_lists_all_pages_and_deduplicates_models() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(|request: &Request| {
            let cursor = request.url.query_pairs().find(|(key, _)| key == "after_id");
            let body = match cursor {
                None => json!({"data": [{"id": "new-model"}, {"id": "shared"}], "has_more": true, "last_id": "cursor/with&symbols"}),
                Some((_, cursor)) => {
                    assert_eq!(cursor, "cursor/with&symbols");
                    json!({"data": [{"id": "shared"}, {"id": "older-model"}], "has_more": false})
                }
            };
            ResponseTemplate::new(200).set_body_json(body)
        })
        .expect(2)
        .mount(&server).await;
    assert_eq!(
        anthropic(&server).fetch_supported_models().await.unwrap(),
        vec!["new-model", "older-model", "shared"]
    );
}

#[tokio::test]
async fn anthropic_rejects_repeated_pagination_without_returning_a_partial_catalog() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [{"id": "first-page"}], "has_more": true, "last_id": "repeated-cursor"
        })))
        .expect(2)
        .mount(&server)
        .await;
    assert!(matches!(
        anthropic(&server).fetch_supported_models().await,
        Err(ProviderError::RequestFailed(_))
    ));
}

#[tokio::test]
async fn anthropic_preserves_errors_on_a_later_models_page() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/models"))
        .respond_with(|request: &Request| {
            if request.url.query_pairs().any(|(key, _)| key == "after_id") {
                ResponseTemplate::new(401).set_body_json(json!({"error": {"type": "authentication_error", "message": "Invalid API key"}}))
            } else {
                ResponseTemplate::new(200).set_body_json(json!({"data": [{"id": "first-page"}], "has_more": true, "last_id": "cursor"}))
            }
        })
        .expect(2).mount(&server).await;
    assert!(matches!(
        anthropic(&server).fetch_supported_models().await,
        Err(ProviderError::Authentication(_))
    ));
}

const OVERLOADED_EVENT: &str = "data: {\"type\":\"error\",\"error\":{\"type\":\"overloaded_error\",\"message\":\"Overloaded\"}}\n\n";
const TEXT_EVENT: &str = "data: {\"type\":\"content_block_delta\",\"index\":0,\"delta\":{\"type\":\"text_delta\",\"text\":\"Answer\"}}\n\n";

#[tokio::test]
async fn anthropic_does_not_report_a_truncated_text_stream_as_complete() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(ResponseTemplate::new(200).set_body_string(TEXT_EVENT))
        .expect(1)
        .mount(&server)
        .await;
    let mut stream = anthropic(&server)
        .stream(
            &ModelConfig::new("claude-sonnet-4-5"),
            "System",
            &[Message::user().with_text("Hi")],
            &[],
        )
        .await
        .unwrap();
    assert!(stream.next().await.unwrap().unwrap().0.is_some());
    assert!(matches!(
        stream.next().await.unwrap(),
        Err(ProviderError::NetworkError(_))
    ));
}

#[tokio::test]
async fn anthropic_retries_an_overload_before_any_content_is_delivered() {
    let server = MockServer::start().await;
    let attempts = Arc::new(AtomicUsize::new(0));
    let requests = attempts.clone();
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(move |_: &Request| {
            let body = if requests.fetch_add(1, Ordering::SeqCst) == 0 {
                OVERLOADED_EVENT.to_owned()
            } else {
                format!("{TEXT_EVENT}data: {{\"type\":\"message_stop\"}}\n\n")
            };
            ResponseTemplate::new(200).set_body_string(body)
        })
        .expect(2)
        .mount(&server)
        .await;
    let mut stream = anthropic(&server)
        .stream(
            &ModelConfig::new("claude-sonnet-4-5"),
            "System",
            &[Message::user().with_text("Hi")],
            &[],
        )
        .await
        .unwrap();
    let message = stream.next().await.unwrap().unwrap().0.unwrap();
    assert!(matches!(&message.content[0], MessageContent::Text(text) if text.text == "Answer"));
    assert_eq!(attempts.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn anthropic_reports_a_mid_stream_overload_without_replaying_partial_work() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/messages"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string(format!("{TEXT_EVENT}{OVERLOADED_EVENT}")),
        )
        .expect(1)
        .mount(&server)
        .await;
    let mut stream = anthropic(&server)
        .stream(
            &ModelConfig::new("claude-sonnet-4-5"),
            "System",
            &[Message::user().with_text("Hi")],
            &[],
        )
        .await
        .unwrap();
    assert!(stream.next().await.unwrap().unwrap().0.is_some());
    assert!(matches!(
        stream.next().await.unwrap(),
        Err(ProviderError::ServerError(_))
    ));
}

#[test]
fn mistral_non_streaming_content_retains_thinking_and_answer() {
    let message = response_to_message(&json!({"choices": [{"message": {"content": [
        {"type": "thinking", "thinking": [{"type": "text", "text": "Reasoning"}]},
        {"type": "text", "text": "Answer"}
    ]}}]}))
    .unwrap();
    assert!(
        matches!(&message.content[0], MessageContent::Thinking(thinking) if thinking.thinking == "Reasoning")
    );
    assert!(matches!(&message.content[1], MessageContent::Text(text) if text.text == "Answer"));
}

#[tokio::test]
async fn mistral_streaming_retains_thinking_through_the_transition_to_plain_text() {
    let mut lines = Vec::new();
    for delta in [
        json!({"content": [{"type": "thinking", "thinking": [{"type": "text", "text": "Think "}]}]}),
        json!({"content": [{"type": "thinking", "thinking": [{"type": "text", "text": "more"}]}, {"type": "text", "text": "Answer "}]}),
        json!({"content": "continues"}),
    ] {
        lines.push(Ok(format!(
            "data: {}",
            json!({"id": "response", "choices": [{"delta": delta}]})
        )));
    }
    lines.push(Ok("data: [DONE]".to_owned()));
    let mut stream = std::pin::pin!(response_to_streaming_message(tokio_stream::iter(lines)));
    let mut thinking = String::new();
    let mut text = String::new();
    while let Some(item) = stream.next().await {
        if let Some(message) = item.unwrap().0 {
            for part in message.content {
                match part {
                    MessageContent::Thinking(part) => thinking.push_str(&part.thinking),
                    MessageContent::Text(part) => text.push_str(&part.text),
                    _ => {}
                }
            }
        }
    }
    assert_eq!(thinking, "Think more");
    assert_eq!(text, "Answer continues");
}

#[test]
fn responses_forwards_request_parameters_and_keeps_app_owned_input_fields() {
    let mut model = ModelConfig::new("gpt-6.1-sol");
    model.request_params = Some(std::collections::HashMap::from([
        ("service_tier".to_owned(), json!("auto")),
        ("text".to_owned(), json!({"verbosity": "low"})),
        ("prompt_cache_options".to_owned(), json!({"ttl": "30m"})),
        ("thinking_effort".to_owned(), json!("medium")),
        ("input".to_owned(), json!(["replacement"])),
        ("model".to_owned(), json!("replacement")),
        ("stream".to_owned(), json!(false)),
    ]));
    let payload =
        create_responses_request(&model, "System", &[Message::user().with_text("Hi")], &[])
            .unwrap();
    assert_eq!(payload["service_tier"], "auto");
    assert_eq!(payload["text"]["verbosity"], "low");
    assert_eq!(payload["prompt_cache_options"]["ttl"], "30m");
    assert_eq!(payload["model"], "gpt-6.1-sol");
    assert_eq!(payload["input"][1]["content"][0]["text"], "Hi");
    assert!(payload.get("thinking_effort").is_none());
    assert!(payload.get("stream").is_none());
}

#[test]
fn responses_rejects_background_work_that_has_no_poll_or_cancel_handoff() {
    let mut model = ModelConfig::new("gpt-6.1-sol");
    model.request_params = Some(std::collections::HashMap::from([(
        "background".to_owned(),
        json!(true),
    )]));
    let error = create_responses_request(&model, "System", &[Message::user().with_text("Hi")], &[])
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ProviderError>(),
        Some(ProviderError::UsageError(_))
    ));
}

fn responses_completion(arguments: &str, status: &str) -> serde_json::Value {
    json!({
        "id": "response-1", "object": "response", "created_at": 0,
        "model": "gpt-6.1-sol", "status": status,
        "output": [{"type": "function_call", "call_id": "call-1", "name": "read", "arguments": arguments}]
    })
}

#[tokio::test]
async fn responses_rejects_malformed_and_non_object_tool_arguments_in_both_modes() {
    for arguments in ["{bad", "[1]", "null", "\"scalar\""] {
        let raw = responses_completion(arguments, "completed");
        let response: ResponsesApiResponse = serde_json::from_value(raw.clone()).unwrap();
        assert!(
            responses_api_to_message(&response).is_err(),
            "accepted {arguments}"
        );
        let event = json!({"type": "response.completed", "sequence_number": 1, "response": raw});
        let mut stream = std::pin::pin!(responses_api_to_streaming_message(tokio_stream::iter(
            vec![Ok(format!("data: {event}"))]
        )));
        let error = stream.next().await.unwrap().unwrap_err();
        assert!(matches!(
            error.downcast_ref::<ProviderError>(),
            Some(ProviderError::RequestFailed(_))
        ));
    }
}

#[test]
fn responses_rejects_an_incomplete_non_streaming_result() {
    let response: ResponsesApiResponse =
        serde_json::from_value(responses_completion("{}", "incomplete")).unwrap();
    assert!(responses_api_to_message(&response)
        .unwrap_err()
        .to_string()
        .contains("incomplete"));
}

#[tokio::test]
async fn responses_reports_incomplete_events_and_premature_eof() {
    let event = json!({"type": "response.incomplete", "sequence_number": 1, "response": {"incomplete_details": {"reason": "max_output_tokens"}}});
    let mut stream = std::pin::pin!(responses_api_to_streaming_message(tokio_stream::iter(
        vec![Ok(format!("data: {event}"))]
    )));
    assert!(stream
        .next()
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("max_output_tokens"));

    let event = json!({"type": "response.output_text.delta", "sequence_number": 1, "item_id": "item", "output_index": 0, "content_index": 0, "delta": "Partial answer"});
    let mut stream = std::pin::pin!(responses_api_to_streaming_message(tokio_stream::iter(
        vec![Ok(format!("data: {event}"))]
    )));
    assert!(stream.next().await.unwrap().unwrap().0.is_some());
    let error = stream.next().await.unwrap().unwrap_err();
    assert!(matches!(
        error.downcast_ref::<ProviderError>(),
        Some(ProviderError::NetworkError(_))
    ));
}

#[tokio::test]
async fn responses_rejects_an_empty_completed_result_in_both_modes() {
    let mut raw = responses_completion("{}", "completed");
    raw["output"] = json!([]);
    let response: ResponsesApiResponse = serde_json::from_value(raw.clone()).unwrap();
    assert!(responses_api_to_message(&response)
        .unwrap_err()
        .to_string()
        .contains("no content"));
    let event = json!({"type": "response.completed", "sequence_number": 1, "response": raw});
    let mut stream = std::pin::pin!(responses_api_to_streaming_message(tokio_stream::iter(
        vec![Ok(format!("data: {event}"))]
    )));
    assert!(stream
        .next()
        .await
        .unwrap()
        .unwrap_err()
        .to_string()
        .contains("no content"));
}
