use crate::routes::errors::ErrorResponse;
use crate::routes::reply_service::{
    log_session_start, run_reply_task, MessageEvent, ReplyEventSink, ReplyTaskConfig,
};
use crate::session_event_bus::RequestGuard;
use crate::state::AppState;
#[cfg(test)]
use axum::http::StatusCode;
use axum::{
    extract::{DefaultBodyLimit, State},
    http::{self},
    response::IntoResponse,
    routing::post,
    Json, Router,
};
use bytes::Bytes;
use futures::Stream;
use gosling::conversation::message::Message;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct ChatRequest {
    user_message: Message,
    /// Override the server's conversation history. Only use this when you need absolute control
    /// over the conversation state (e.g., administrative tools). For normal operations, the server
    /// is the source of truth - use truncate/fork endpoints to modify conversation history instead.
    #[serde(default)]
    override_conversation: Option<Vec<Message>>,
    session_id: String,
}

pub struct SseResponse {
    rx: ReceiverStream<String>,
}

impl SseResponse {
    fn new(rx: ReceiverStream<String>) -> Self {
        Self { rx }
    }
}

impl Stream for SseResponse {
    type Item = Result<Bytes, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx)
            .poll_next(cx)
            .map(|opt| opt.map(|s| Ok(Bytes::from(s))))
    }
}

impl IntoResponse for SseResponse {
    fn into_response(self) -> axum::response::Response {
        let stream = self;
        let body = axum::body::Body::from_stream(stream);

        http::Response::builder()
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(body)
            .unwrap()
    }
}

async fn stream_event(
    event: MessageEvent,
    tx: &mpsc::Sender<String>,
    cancel_token: &CancellationToken,
) -> bool {
    let json = serde_json::to_string(&event).unwrap_or_else(|e| {
        format!(
            r#"{{"type":"Error","error":"Failed to serialize event: {}"}}"#,
            e
        )
    });

    if tx.send(format!("data: {}\n\n", json)).await.is_err() {
        tracing::info!("client hung up");
        cancel_token.cancel();
        return false;
    }

    true
}

struct SseReplySink {
    tx: mpsc::Sender<String>,
    cancel_token: CancellationToken,
}

#[async_trait::async_trait]
impl ReplyEventSink for SseReplySink {
    async fn publish(&mut self, event: MessageEvent) -> bool {
        stream_event(event, &self.tx, &self.cancel_token).await
    }
}

#[allow(clippy::too_many_lines)]
#[utoipa::path(
    post,
    path = "/reply",
    request_body = ChatRequest,
    responses(
        (status = 200, description = "Streaming response initiated",
         body = MessageEvent,
         content_type = "text/event-stream"),
        (status = 424, description = "Agent not initialized"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn reply(
    State(state): State<Arc<AppState>>,
    Json(request): Json<ChatRequest>,
) -> Result<SseResponse, ErrorResponse> {
    let session_start = std::time::Instant::now();
    log_session_start();

    let session_id = request.session_id.clone();

    // CON-GOS-100/REL-GOS-012: this legacy endpoint used to spawn a reply task
    // with no per-session single-flight guard at all, unlike the newer
    // `/sessions/{id}/reply`. Two concurrent requests for the same session
    // (a client retry, two tabs) ran unserialized turns against the same
    // conversation, including a lost-update race on accumulated usage/cost
    // (CON-GOS-101). Registering with the same per-session bus the newer
    // route uses closes the gap for both entry points at once and, as a
    // side effect, makes this endpoint's turn reachable from
    // `POST /sessions/{id}/cancel` too.
    let bus = state.get_or_create_event_bus(&session_id).await;
    let request_id = uuid::Uuid::new_v4().to_string();
    let cancel_token = bus
        .try_register_request(request_id.clone())
        .await
        .map_err(|_| {
            ErrorResponse::bad_request("Session already has an active request. Cancel it first.")
        })?;

    let (tx, rx) = mpsc::channel(100);
    let stream = ReceiverStream::new(rx);

    let user_message = request.user_message;
    let override_conversation = request.override_conversation;

    let task_cancel = cancel_token.clone();
    let task_bus = bus.clone();
    let task_request_id = request_id.clone();

    drop(tokio::spawn(async move {
        let mut guard = RequestGuard::new(task_bus.clone(), task_request_id.clone());
        let mut sink = SseReplySink {
            tx: tx.clone(),
            cancel_token: cancel_token.clone(),
        };
        run_reply_task(
            ReplyTaskConfig {
                state,
                session_id,
                user_message,
                override_conversation,
                cancel_token: task_cancel,
                session_start,
                heartbeat_interval: Some(Duration::from_millis(500)),
            },
            &mut sink,
        )
        .await;

        guard.disarm();
        task_bus.cleanup_request(&task_request_id).await;
    }));
    Ok(SseResponse::new(stream))
}

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route(
            "/reply",
            post(reply).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    mod integration_tests {
        use super::*;
        use async_trait::async_trait;
        use axum::body::{to_bytes, Body};
        use axum::http::Request;
        use gosling::conversation::message::Message;
        use gosling::conversation::Conversation;
        use gosling::providers::base::{MessageStream, Provider};
        use gosling::session::SessionType;
        use gosling_providers::errors::ProviderError;
        use gosling_providers::model::ModelConfig;
        use rmcp::model::Tool;
        use std::path::PathBuf;
        use tower::ServiceExt;

        struct FailingProvider;

        #[async_trait]
        impl Provider for FailingProvider {
            fn get_name(&self) -> &str {
                "failing-test-provider"
            }

            async fn stream(
                &self,
                _model_config: &ModelConfig,
                _system_prompt: &str,
                _messages: &[Message],
                _tools: &[Tool],
            ) -> Result<MessageStream, ProviderError> {
                Err(ProviderError::ExecutionError(
                    "intentional startup failure".into(),
                ))
            }
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn test_reply_endpoint() {
            let state = AppState::new(true).await.unwrap();

            let app = routes(state);

            let request = Request::builder()
                .uri("/reply")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-secret-key", "test-secret")
                .body(Body::from(
                    serde_json::to_string(&ChatRequest {
                        user_message: Message::user().with_text("test message"),
                        override_conversation: None,
                        session_id: "test-session".to_string(),
                    })
                    .unwrap(),
                ))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK);
        }

        // CON-GOS-100/REL-GOS-012 regression: the legacy `/reply` endpoint
        // must not spawn a second concurrent reply task for a session that
        // already has one active, matching `/sessions/{id}/reply`'s guard.
        #[tokio::test(flavor = "multi_thread")]
        async fn test_reply_rejected_when_session_has_active_request() {
            let state = AppState::new(true).await.unwrap();
            let bus = state
                .get_or_create_event_bus("concurrent-reply-session")
                .await;
            let _token = bus
                .try_register_request("existing-request".to_string())
                .await
                .unwrap();

            let app = routes(state);
            let request = Request::builder()
                .uri("/reply")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-secret-key", "test-secret")
                .body(Body::from(
                    serde_json::to_string(&ChatRequest {
                        user_message: Message::user().with_text("test message"),
                        override_conversation: None,
                        session_id: "concurrent-reply-session".to_string(),
                    })
                    .unwrap(),
                ))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test(flavor = "multi_thread")]
        async fn test_override_conversation_rolls_back_on_reply_start_failure() {
            let state = AppState::new(true).await.unwrap();
            let session = state
                .session_manager()
                .create_session(
                    PathBuf::default(),
                    "reply-override-rollback-test".to_string(),
                    SessionType::Hidden,
                    gosling::config::GoslingMode::default(),
                )
                .await
                .unwrap();

            let original_message = Message::user().with_text("original history");
            let original_conversation =
                Conversation::new_unvalidated(vec![original_message.clone()]);
            state
                .session_manager()
                .replace_conversation(&session.id, &original_conversation)
                .await
                .unwrap();

            let agent = state.get_agent(session.id.clone()).await.unwrap();
            agent
                .update_provider(
                    Arc::new(FailingProvider),
                    ModelConfig::new("failing-test-model"),
                    &session.id,
                )
                .await
                .unwrap();

            let failed_prompt = Message::user().with_text("failed prompt");
            let app = routes(state.clone());
            let request = Request::builder()
                .uri("/reply")
                .method("POST")
                .header("content-type", "application/json")
                .header("x-secret-key", "test-secret")
                .body(Body::from(
                    serde_json::to_string(&ChatRequest {
                        user_message: failed_prompt.clone(),
                        override_conversation: Some(vec![
                            Message::user().with_text("override history")
                        ]),
                        session_id: session.id.clone(),
                    })
                    .unwrap(),
                ))
                .unwrap();

            let response = app.oneshot(request).await.unwrap();

            assert_eq!(response.status(), StatusCode::OK);
            let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let body_text = String::from_utf8(body.to_vec()).unwrap();
            assert!(body_text.contains("intentional startup failure"));

            let persisted = state
                .session_manager()
                .get_session(&session.id, true)
                .await
                .unwrap();
            let messages = persisted
                .conversation
                .unwrap()
                .messages()
                .iter()
                .map(|message| message.as_concat_text())
                .collect::<Vec<_>>();

            assert_eq!(
                messages,
                vec!["original history".to_string(), "failed prompt".to_string()]
            );
        }
    }
}
