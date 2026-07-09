use crate::routes::errors::ErrorResponse;
use crate::routes::history_override::{
    apply_conversation_override, is_early_provider_failure_message, rollback_conversation_override,
};
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
use futures::{stream::StreamExt, Stream};
use gosling::agents::{AgentEvent, SessionConfig};
use gosling::conversation::message::{Message, MessageContent, TokenState};
use gosling::conversation::Conversation;
use gosling::session::SessionManager;
use rmcp::model::ServerNotification;
use serde::{Deserialize, Serialize};
use std::{
    convert::Infallible,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Duration,
};
use tokio::sync::mpsc;
use tokio::time::timeout;
use tokio_stream::wrappers::ReceiverStream;
use tokio_util::sync::CancellationToken;

pub fn track_tool_telemetry(content: &MessageContent, all_messages: &[Message]) {
    match content {
        MessageContent::ToolRequest(tool_request) => {
            if let Ok(tool_call) = &tool_request.tool_call {
                tracing::info!(monotonic_counter.gosling.tool_calls = 1,
                    tool_name = %tool_call.name,
                    "Tool call started"
                );
            }
        }
        MessageContent::ToolResponse(tool_response) => {
            let tool_name = all_messages
                .iter()
                .rev()
                .find_map(|msg| {
                    msg.content.iter().find_map(|c| {
                        if let MessageContent::ToolRequest(req) = c {
                            if req.id == tool_response.id {
                                if let Ok(tool_call) = &req.tool_call {
                                    Some(tool_call.name.clone())
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| "unknown".to_string().into());

            let success = tool_response.tool_result.is_ok();
            let result_status = if success { "success" } else { "error" };

            tracing::info!(
                monotonic_counter.gosling.tool_completions = 1,
                tool_name = %tool_name,
                result = %result_status,
                "Tool call completed"
            );
        }
        _ => {}
    }
}

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

#[derive(Debug, Clone, Serialize, utoipa::ToSchema)]
#[serde(tag = "type")]
pub enum MessageEvent {
    Message {
        message: Message,
        token_state: TokenState,
    },
    Error {
        error: String,
    },
    Finish {
        reason: String,
        token_state: TokenState,
    },
    Notification {
        request_id: String,
        #[schema(value_type = Object)]
        message: ServerNotification,
    },
    UpdateConversation {
        conversation: Conversation,
    },
    /// Sent at the start of an SSE stream to inform the client about
    /// in-flight requests it can reattach to.
    ActiveRequests {
        request_ids: Vec<String>,
    },
    Ping,
}

pub async fn get_token_state(session_manager: &SessionManager, session_id: &str) -> TokenState {
    session_manager
        .get_session(session_id, false)
        .await
        .map(|session| TokenState::from(&session))
        .inspect_err(|e| {
            tracing::warn!(
                "Failed to fetch session token state for {}: {}",
                session_id,
                e
            );
        })
        .unwrap_or_default()
}

async fn stream_event(
    event: MessageEvent,
    tx: &mpsc::Sender<String>,
    cancel_token: &CancellationToken,
) {
    let json = serde_json::to_string(&event).unwrap_or_else(|e| {
        format!(
            r#"{{"type":"Error","error":"Failed to serialize event: {}"}}"#,
            e
        )
    });

    if tx.send(format!("data: {}\n\n", json)).await.is_err() {
        tracing::info!("client hung up");
        cancel_token.cancel();
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

    tracing::info!(
        monotonic_counter.gosling.session_starts = 1,
        session_type = "app",
        interface = "ui",
        "Session started"
    );

    let session_id = request.session_id.clone();

    let (tx, rx) = mpsc::channel(100);
    let stream = ReceiverStream::new(rx);
    let cancel_token = CancellationToken::new();

    let user_message = request.user_message;
    let override_conversation = request.override_conversation;

    let task_cancel = cancel_token.clone();
    let task_tx = tx.clone();

    drop(tokio::spawn(async move {
        let agent = match state.get_agent(session_id.clone()).await {
            Ok(agent) => agent,
            Err(e) => {
                tracing::error!("Failed to get session agent: {}", e);
                let _ = stream_event(
                    MessageEvent::Error {
                        error: format!("Failed to get session agent: {}", e),
                    },
                    &task_tx,
                    &task_cancel,
                )
                .await;
                return;
            }
        };

        let session = match state.session_manager().get_session(&session_id, true).await {
            Ok(metadata) => metadata,
            Err(e) => {
                tracing::error!("Failed to read session for {}: {}", session_id, e);
                let _ = stream_event(
                    MessageEvent::Error {
                        error: format!("Failed to read session: {}", e),
                    },
                    &task_tx,
                    &cancel_token,
                )
                .await;
                return;
            }
        };

        let session_config = SessionConfig {
            id: session_id.clone(),
            max_turns: None,
            compacted_context: false,
            tail_limit: None,
        };

        let mut rollback_state = None;
        let mut all_messages = session.conversation.clone().unwrap_or_default();
        if let Some(history) = override_conversation {
            if let Some((override_conversation, rollback)) = apply_conversation_override(
                state.session_manager(),
                &session,
                &user_message,
                history,
            )
            .await
            {
                all_messages = override_conversation;
                rollback_state = Some(rollback);
            }
        }
        all_messages.push(user_message.clone());

        let mut stream = match agent
            .reply(
                user_message.clone(),
                session_config,
                Some(task_cancel.clone()),
            )
            .await
        {
            Ok(stream) => stream,
            Err(e) => {
                if let Some(rollback) = rollback_state.take() {
                    rollback_conversation_override(state.session_manager(), &session_id, rollback)
                        .await;
                }
                tracing::error!("Failed to start reply stream: {:?}", e);
                stream_event(
                    MessageEvent::Error {
                        error: e.to_string(),
                    },
                    &task_tx,
                    &cancel_token,
                )
                .await;
                return;
            }
        };

        let mut heartbeat_interval = tokio::time::interval(Duration::from_millis(500));
        let mut reply_progressed = false;
        loop {
            tokio::select! {
                _ = task_cancel.cancelled() => {
                    if !reply_progressed {
                        if let Some(rollback) = rollback_state.take() {
                            rollback_conversation_override(
                                state.session_manager(),
                                &session_id,
                                rollback,
                            )
                            .await;
                        }
                    }
                    tracing::info!("Agent task cancelled");
                    break;
                }
                _ = heartbeat_interval.tick() => {
                    stream_event(MessageEvent::Ping, &tx, &cancel_token).await;
                }
                response = timeout(Duration::from_millis(500), stream.next()) => {
                    match response {
                        Ok(Some(Ok(AgentEvent::Message(message)))) => {
                            let rollback_for_message = !reply_progressed
                                && rollback_state.is_some()
                                && is_early_provider_failure_message(&message);
                            if rollback_for_message {
                                if let Some(rollback) = rollback_state.take() {
                                    rollback_conversation_override(
                                        state.session_manager(),
                                        &session_id,
                                        rollback,
                                    )
                                    .await;
                                }
                            } else {
                                reply_progressed = true;
                            }
                            for content in &message.content {
                                track_tool_telemetry(content, all_messages.messages());
                            }

                            all_messages.push(message.clone());

                            let token_state = get_token_state(state.session_manager(), &session_id).await;

                            stream_event(MessageEvent::Message { message, token_state }, &tx, &cancel_token).await;
                        }
                        Ok(Some(Ok(AgentEvent::Usage(_)))) => {}
                        Ok(Some(Ok(AgentEvent::HistoryReplaced(new_messages)))) => {
                            reply_progressed = true;
                            all_messages = new_messages.clone();
                            stream_event(MessageEvent::UpdateConversation {conversation: new_messages}, &tx, &cancel_token).await;

                        }
                        Ok(Some(Ok(AgentEvent::McpNotification((request_id, n))))) => {
                            reply_progressed = true;
                            stream_event(MessageEvent::Notification{
                                request_id: request_id.clone(),
                                message: n,
                            }, &tx, &cancel_token).await;
                        }

                        Ok(Some(Err(e))) => {
                            if !reply_progressed {
                                if let Some(rollback) = rollback_state.take() {
                                    rollback_conversation_override(
                                        state.session_manager(),
                                        &session_id,
                                        rollback,
                                    )
                                    .await;
                                }
                            }
                            tracing::error!("Error processing message: {}", e);
                            stream_event(
                                MessageEvent::Error {
                                    error: e.to_string(),
                                },
                                &tx,
                                &cancel_token,
                            ).await;
                            break;
                        }
                        Ok(None) => {
                            if !reply_progressed {
                                if let Some(rollback) = rollback_state.take() {
                                    rollback_conversation_override(
                                        state.session_manager(),
                                        &session_id,
                                        rollback,
                                    )
                                    .await;
                                }
                            }
                            break;
                        }
                        Err(_) => {
                            if tx.is_closed() {
                                break;
                            }
                            continue;
                        }
                    }
                }
            }
        }

        let session_duration = session_start.elapsed();

        if let Ok(session) = state.session_manager().get_session(&session_id, true).await {
            let total_tokens = session.usage.total_tokens.unwrap_or(0);
            tracing::info!(
                monotonic_counter.gosling.session_completions = 1,
                session_type = "app",
                interface = "ui",
                exit_type = "normal",
                duration_ms = session_duration.as_millis() as u64,
                total_tokens = total_tokens,
                message_count = session.message_count,
                "Session completed"
            );

            tracing::info!(
                monotonic_counter.gosling.session_duration_ms = session_duration.as_millis() as u64,
                session_type = "app",
                interface = "ui",
                "Session duration"
            );

            if total_tokens > 0 {
                tracing::info!(
                    monotonic_counter.gosling.session_tokens = total_tokens,
                    session_type = "app",
                    interface = "ui",
                    "Session tokens"
                );
            }
        } else {
            tracing::info!(
                monotonic_counter.gosling.session_completions = 1,
                session_type = "app",
                interface = "ui",
                exit_type = "normal",
                duration_ms = session_duration.as_millis() as u64,
                total_tokens = 0u64,
                message_count = all_messages.len(),
                "Session completed"
            );

            tracing::info!(
                monotonic_counter.gosling.session_duration_ms = session_duration.as_millis() as u64,
                session_type = "app",
                interface = "ui",
                "Session duration"
            );
        }

        let final_token_state = get_token_state(state.session_manager(), &session_id).await;

        let _ = stream_event(
            MessageEvent::Finish {
                reason: "stop".to_string(),
                token_state: final_token_state,
            },
            &task_tx,
            &cancel_token,
        )
        .await;
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
