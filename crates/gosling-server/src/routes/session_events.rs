use crate::routes::errors::ErrorResponse;
use crate::routes::reply_service::{
    is_elicitation_response, log_session_start, run_reply_task, MessageEvent, ReplyEventSink,
    ReplyTaskConfig,
};
use crate::session_event_bus::RequestGuard;
use crate::state::AppState;
use async_trait::async_trait;
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{self, HeaderMap},
    response::IntoResponse,
    routing::{get, post},
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

// ── Request / Response types ────────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct SessionReplyRequest {
    /// Client-generated UUIDv7 identifying this request.
    pub request_id: String,
    pub user_message: Message,
    #[serde(default)]
    pub override_conversation: Option<Vec<Message>>,
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct SessionReplyResponse {
    pub request_id: String,
}

#[derive(Debug, Deserialize, Serialize, utoipa::ToSchema)]
pub struct CancelRequest {
    pub request_id: String,
}

// ── SSE Event Stream Response ───────────────────────────────────────────

/// An SSE response that includes `id:` lines for Last-Event-ID reconnection.
pub struct SseEventStream {
    rx: ReceiverStream<String>,
}

impl SseEventStream {
    fn new(rx: ReceiverStream<String>) -> Self {
        Self { rx }
    }
}

impl Stream for SseEventStream {
    type Item = Result<Bytes, Infallible>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.rx)
            .poll_next(cx)
            .map(|opt| opt.map(|s| Ok(Bytes::from(s))))
    }
}

impl IntoResponse for SseEventStream {
    fn into_response(self) -> axum::response::Response {
        let body = axum::body::Body::from_stream(self);
        http::Response::builder()
            .header("Content-Type", "text/event-stream")
            .header("Cache-Control", "no-cache")
            .header("Connection", "keep-alive")
            .body(body)
            .unwrap()
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────

fn format_sse_event(seq: u64, json: &str) -> String {
    format!("id: {}\ndata: {}\n\n", seq, json)
}

fn serialize_session_event(seq: u64, request_id: Option<&str>, event: &MessageEvent) -> String {
    // Build JSON payload: { request_id?: string, ...event_fields }
    // We flatten request_id into the event JSON.
    let mut event_json = serde_json::to_value(event).unwrap_or_else(
        |e| serde_json::json!({"type": "Error", "error": format!("Serialization error: {}", e)}),
    );

    if let Some(rid) = request_id {
        if let serde_json::Value::Object(ref mut map) = event_json {
            // Always insert chat_request_id for routing (the chat UUID that
            // the frontend registered its listener under).
            map.insert(
                "chat_request_id".to_string(),
                serde_json::Value::String(rid.to_string()),
            );
            // Also set request_id if the event doesn't already carry one
            // (e.g. Notification events have their own request_id for tool-call matching)
            map.entry("request_id")
                .or_insert_with(|| serde_json::Value::String(rid.to_string()));
        }
    }

    let json_str = serde_json::to_string(&event_json).unwrap_or_default();
    format_sse_event(seq, &json_str)
}

async fn send_sse_frame(
    tx: &mpsc::Sender<String>,
    frame: String,
    shutdown: &CancellationToken,
) -> bool {
    tokio::select! {
        biased;
        _ = shutdown.cancelled() => false,
        result = tx.send(frame) => result.is_ok(),
    }
}

struct EventBusReplySink {
    bus: Arc<crate::session_event_bus::SessionEventBus>,
    request_id: String,
}

#[async_trait]
impl ReplyEventSink for EventBusReplySink {
    async fn publish(&mut self, event: MessageEvent) -> bool {
        self.bus.publish(Some(self.request_id.clone()), event).await;
        true
    }
}

// ── GET /sessions/{id}/events ───────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/sessions/{id}/events",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    responses(
        (status = 200, description = "SSE event stream",
         body = MessageEvent,
         content_type = "text/event-stream"),
        (status = 404, description = "Session not found"),
    )
)]
pub async fn session_events(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    headers: HeaderMap,
) -> Result<SseEventStream, axum::http::StatusCode> {
    // Validate the session exists before creating an event bus.
    state
        .session_manager()
        .get_session(&session_id, false)
        .await
        .map_err(|_| axum::http::StatusCode::NOT_FOUND)?;

    let last_event_id: Option<u64> = headers
        .get("Last-Event-ID")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse().ok());

    let bus = state.get_or_create_event_bus(&session_id).await;

    let (replay, replay_max_seq, mut live_rx) = match bus.subscribe(last_event_id).await {
        Ok(result) => result,
        Err(_) => {
            // Client's Last-Event-ID has been evicted from the replay buffer.
            // Send a single error event so the client knows to reload.
            let (tx, rx) = mpsc::channel::<String>(1);
            let stream = ReceiverStream::new(rx);
            let seq = 0;
            let error_event = MessageEvent::Error {
                error: "Client too far behind — reload conversation".to_string(),
            };
            let frame = serialize_session_event(seq, None, &error_event);
            tokio::spawn(async move {
                let _ = tx.send(frame).await;
            });
            return Ok(SseEventStream::new(stream));
        }
    };

    let (tx, rx) = mpsc::channel::<String>(256);
    let stream = ReceiverStream::new(rx);
    let task_bus = bus.clone();
    let shutdown = state.shutdown_token();

    tokio::spawn(async move {
        let bus = task_bus;

        // Notify the client about any in-flight requests BEFORE replay
        // so it can register event handlers before replayed events arrive.
        // Emitted without an SSE `id:` field so it doesn't regress the
        // client's Last-Event-ID cursor.
        let active_ids = bus.active_request_ids().await;
        if !active_ids.is_empty() {
            let event = MessageEvent::ActiveRequests {
                request_ids: active_ids,
            };
            let json_str = serde_json::to_string(&serde_json::to_value(&event).unwrap_or_default())
                .unwrap_or_default();
            let frame = format!("data: {}\n\n", json_str);
            if !send_sse_frame(&tx, frame, &shutdown).await {
                return;
            }
        }

        // Send replayed events
        for event in &replay {
            let frame =
                serialize_session_event(event.seq, event.request_id.as_deref(), &event.event);
            if !send_sse_frame(&tx, frame, &shutdown).await {
                return;
            }
        }

        // Send live events + heartbeat pings
        let mut heartbeat_interval = tokio::time::interval(Duration::from_millis(500));
        // Heartbeat uses a local counter — not stored in the replay buffer
        let mut heartbeat_seq = 0u64;

        loop {
            tokio::select! {
                biased;
                _ = shutdown.cancelled() => return,
                _ = heartbeat_interval.tick() => {
                    // Send heartbeat directly without publishing to the bus,
                    // so pings don't evict real events from the replay buffer.
                    // Use a comment-style SSE id so it won't interfere with Last-Event-ID.
                    let frame = format!(": ping {}\n\n", heartbeat_seq);
                    heartbeat_seq += 1;
                    if !send_sse_frame(&tx, frame, &shutdown).await {
                        return;
                    }
                }
                result = live_rx.recv() => {
                    match result {
                        Ok(event) => {
                            // Skip events already covered by replay to avoid duplicates
                            // at the replay/live handoff boundary.
                            if event.seq <= replay_max_seq {
                                continue;
                            }
                            let frame = serialize_session_event(
                                event.seq,
                                event.request_id.as_deref(),
                                &event.event,
                            );
                            if !send_sse_frame(&tx, frame, &shutdown).await {
                                return;
                            }
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                            tracing::warn!("SSE subscriber lagged by {} events, closing stream so client reconnects with Last-Event-ID", n);
                            // Close the stream so the client reconnects and
                            // replays missed events from the buffer.
                            return;
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                            return;
                        }
                    }
                }
            }
        }
    });

    Ok(SseEventStream::new(stream))
}

// ── POST /sessions/{id}/reply ───────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/sessions/{id}/reply",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    request_body = SessionReplyRequest,
    responses(
        (status = 200, description = "Request accepted",
         body = SessionReplyResponse),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Session not found"),
        (status = 424, description = "Agent not initialized"),
        (status = 500, description = "Internal server error"),
    )
)]
pub async fn session_reply(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<SessionReplyRequest>,
) -> Result<Json<SessionReplyResponse>, ErrorResponse> {
    let request_id = request.request_id.clone();

    // Validate request_id is a valid UUID
    if uuid::Uuid::parse_str(&request_id).is_err() {
        return Err(ErrorResponse::bad_request(
            "request_id must be a valid UUID",
        ));
    }

    // Validate session exists before allocating a bus/registering work
    state
        .session_manager()
        .get_session(&session_id, false)
        .await
        .map_err(|_| ErrorResponse::not_found(format!("Session {} not found", session_id)))?;

    let session_start = std::time::Instant::now();
    log_session_start();

    let user_message = request.user_message;
    let override_conversation = request.override_conversation;

    if is_elicitation_response(&user_message) {
        let agent = state.get_agent_for_route(session_id.clone()).await?;
        let session_config = gosling::agents::types::SessionConfig {
            id: session_id.clone(),
            max_turns: None,
            compacted_context: false,
            tail_limit: None,
        };
        let _ = agent
            .reply(user_message, session_config, None)
            .await
            .map_err(|e| ErrorResponse::internal(e.to_string()))?;
        return Ok(Json(SessionReplyResponse { request_id }));
    }

    let bus = state.get_or_create_event_bus(&session_id).await;

    let cancel_token = bus
        .try_register_request(request_id.clone())
        .await
        .map_err(|_| {
            ErrorResponse::bad_request("Session already has an active request. Cancel it first.")
        })?;

    let task_state = state.clone();
    let task_session_id = session_id.clone();
    let task_request_id = request_id.clone();
    let task_cancel = cancel_token.clone();
    let task_bus = bus.clone();

    drop(tokio::spawn(async move {
        let mut _guard = RequestGuard::new(task_bus.clone(), task_request_id.clone());
        let mut sink = EventBusReplySink {
            bus: task_bus.clone(),
            request_id: task_request_id.clone(),
        };
        run_reply_task(
            ReplyTaskConfig {
                state: task_state,
                session_id: task_session_id,
                user_message,
                override_conversation,
                cancel_token: task_cancel,
                session_start,
                heartbeat_interval: None,
            },
            &mut sink,
        )
        .await;

        _guard.disarm();
        task_bus.cleanup_request(&task_request_id).await;
    }));

    Ok(Json(SessionReplyResponse { request_id }))
}

// ── POST /sessions/{id}/cancel ──────────────────────────────────────────

#[utoipa::path(
    post,
    path = "/sessions/{id}/cancel",
    params(
        ("id" = String, Path, description = "Session ID"),
    ),
    request_body = CancelRequest,
    responses(
        (status = 200, description = "Cancellation accepted"),
    )
)]
pub async fn session_cancel(
    State(state): State<Arc<AppState>>,
    Path(session_id): Path<String>,
    Json(request): Json<CancelRequest>,
) -> axum::http::StatusCode {
    let bus = match state.get_event_bus(&session_id).await {
        Some(bus) => bus,
        None => return axum::http::StatusCode::NOT_FOUND,
    };
    bus.cancel_request(&request.request_id).await;
    axum::http::StatusCode::OK
}

// ── Route registration ──────────────────────────────────────────────────

pub fn routes(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/sessions/{id}/events", get(session_events))
        .route(
            "/sessions/{id}/reply",
            post(session_reply).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route("/sessions/{id}/cancel", post(session_cancel))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn blocked_sse_send_wakes_for_server_shutdown() {
        let (tx, _rx) = mpsc::channel(1);
        tx.send("occupied".to_string()).await.unwrap();
        let shutdown = CancellationToken::new();
        shutdown.cancel();

        let sent = tokio::time::timeout(
            Duration::from_millis(100),
            send_sse_frame(&tx, "blocked".to_string(), &shutdown),
        )
        .await
        .unwrap();

        assert!(!sent);
    }
}
