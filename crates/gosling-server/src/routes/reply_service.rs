use crate::routes::history_override::{
    apply_conversation_override, is_early_provider_failure_message, rollback_conversation_override,
    ConversationOverrideRollback,
};
use crate::state::AppState;
use async_trait::async_trait;
use futures::StreamExt;
use gosling::agents::{AgentEvent, SessionConfig};
use gosling::conversation::message::{ActionRequiredData, Message, MessageContent, TokenState};
use gosling::conversation::Conversation;
use gosling::session::SessionManager;
use rmcp::model::ServerNotification;
use serde::Serialize;
use std::{
    fmt::Display,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

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
    ActiveRequests {
        request_ids: Vec<String>,
    },
    Ping,
}

pub struct ReplyTaskConfig {
    pub state: Arc<AppState>,
    pub session_id: String,
    pub user_message: Message,
    pub override_conversation: Option<Vec<Message>>,
    pub cancel_token: CancellationToken,
    pub session_start: Instant,
    pub heartbeat_interval: Option<Duration>,
}

#[async_trait]
pub trait ReplyEventSink: Send {
    async fn publish(&mut self, event: MessageEvent) -> bool;
}

pub fn log_session_start() {
    tracing::info!(
        monotonic_counter.gosling.session_starts = 1,
        session_type = "app",
        interface = "ui",
        "Session started"
    );
}

pub fn is_elicitation_response(message: &Message) -> bool {
    message.content.iter().any(|content| {
        matches!(
            content,
            MessageContent::ActionRequired(action_required)
                if matches!(
                    action_required.data,
                    ActionRequiredData::ElicitationResponse { .. }
                )
        )
    })
}

pub fn track_tool_telemetry(content: &MessageContent, all_messages: &[Message]) {
    match content {
        MessageContent::ToolRequest(tool_request) => {
            if let Ok(tool_call) = &tool_request.tool_call {
                tracing::info!(
                    monotonic_counter.gosling.tool_calls = 1,
                    tool_name = %tool_call.name,
                    "Tool call started"
                );
            }
        }
        MessageContent::ToolResponse(tool_response) => {
            let tool_name = all_messages
                .iter()
                .rev()
                .find_map(|message| {
                    message.content.iter().find_map(|content| {
                        if let MessageContent::ToolRequest(request) = content {
                            if request.id == tool_response.id {
                                request
                                    .tool_call
                                    .as_ref()
                                    .ok()
                                    .map(|tool_call| tool_call.name.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    })
                })
                .unwrap_or_else(|| "unknown".to_string().into());

            tracing::info!(
                monotonic_counter.gosling.tool_completions = 1,
                tool_name = %tool_name,
                result = if tool_response.tool_result.is_ok() { "success" } else { "error" },
                "Tool call completed"
            );
        }
        _ => {}
    }
}

pub async fn get_token_state(session_manager: &SessionManager, session_id: &str) -> TokenState {
    session_manager
        .get_session(session_id, false)
        .await
        .map(|session| TokenState::from(&session))
        .inspect_err(|error| {
            tracing::warn!(
                "Failed to fetch session token state for {}: {}",
                session_id,
                error
            );
        })
        .unwrap_or_default()
}

pub async fn run_reply_task<S>(config: ReplyTaskConfig, sink: &mut S)
where
    S: ReplyEventSink,
{
    let ReplyTaskConfig {
        state,
        session_id,
        user_message,
        override_conversation,
        cancel_token,
        session_start,
        heartbeat_interval,
    } = config;

    let agent = match state.get_agent(session_id.clone()).await {
        Ok(agent) => agent,
        Err(error) => {
            tracing::error!("Failed to get session agent: {}", error);
            let _ = sink
                .publish(MessageEvent::Error {
                    error: format!("Failed to get session agent: {}", error),
                })
                .await;
            return;
        }
    };

    let session = match state.session_manager().get_session(&session_id, true).await {
        Ok(session) => session,
        Err(error) => {
            tracing::error!("Failed to read session for {}: {}", session_id, error);
            let _ = sink
                .publish(MessageEvent::Error {
                    error: format!("Failed to read session: {}", error),
                })
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
        if let Some((override_conversation, rollback)) =
            apply_conversation_override(state.session_manager(), &session, &user_message, history)
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
            Some(cancel_token.clone()),
        )
        .await
    {
        Ok(stream) => stream,
        Err(error) => {
            rollback_if_pending(state.session_manager(), &session_id, &mut rollback_state).await;
            tracing::error!("Failed to start reply stream: {:?}", error);
            let _ = sink
                .publish(MessageEvent::Error {
                    error: error.to_string(),
                })
                .await;
            return;
        }
    };

    let mut reply_progressed = false;
    let mut heartbeat = heartbeat_interval.map(tokio::time::interval);

    loop {
        if let Some(heartbeat_interval) = heartbeat.as_mut() {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    if !reply_progressed {
                        rollback_if_pending(state.session_manager(), &session_id, &mut rollback_state).await;
                    }
                    tracing::info!("Agent task cancelled for session {}", session_id);
                    break;
                }
                _ = heartbeat_interval.tick() => {
                    if !sink.publish(MessageEvent::Ping).await {
                        break;
                    }
                }
                response = timeout(Duration::from_millis(500), stream.next()) => {
                    if handle_stream_result(
                        response,
                        &state,
                        &session_id,
                        &mut all_messages,
                        &mut rollback_state,
                        &mut reply_progressed,
                        sink,
                    )
                    .await
                    {
                        break;
                    }
                }
            }
        } else {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    if !reply_progressed {
                        rollback_if_pending(state.session_manager(), &session_id, &mut rollback_state).await;
                    }
                    tracing::info!("Agent task cancelled for session {}", session_id);
                    break;
                }
                response = timeout(Duration::from_millis(500), stream.next()) => {
                    if handle_stream_result(
                        response,
                        &state,
                        &session_id,
                        &mut all_messages,
                        &mut rollback_state,
                        &mut reply_progressed,
                        sink,
                    )
                    .await
                    {
                        break;
                    }
                }
            }
        }
    }

    log_session_completion(
        state.session_manager(),
        &session_id,
        session_start,
        all_messages.len(),
    )
    .await;

    let final_token_state = get_token_state(state.session_manager(), &session_id).await;
    let _ = sink
        .publish(MessageEvent::Finish {
            reason: "stop".to_string(),
            token_state: final_token_state,
        })
        .await;
}

async fn handle_stream_result<S, E>(
    response: Result<Option<Result<AgentEvent, E>>, tokio::time::error::Elapsed>,
    state: &Arc<AppState>,
    session_id: &str,
    all_messages: &mut Conversation,
    rollback_state: &mut Option<ConversationOverrideRollback>,
    reply_progressed: &mut bool,
    sink: &mut S,
) -> bool
where
    S: ReplyEventSink,
    E: Display,
{
    match response {
        Ok(Some(Ok(AgentEvent::Message(message)))) => {
            let rollback_for_message = !*reply_progressed
                && rollback_state.is_some()
                && is_early_provider_failure_message(&message);
            if rollback_for_message {
                rollback_if_pending(state.session_manager(), session_id, rollback_state).await;
            } else {
                *reply_progressed = true;
            }

            for content in &message.content {
                track_tool_telemetry(content, all_messages.messages());
            }

            all_messages.push(message.clone());
            let token_state = get_token_state(state.session_manager(), session_id).await;

            !sink
                .publish(MessageEvent::Message {
                    message,
                    token_state,
                })
                .await
        }
        Ok(Some(Ok(AgentEvent::Usage(_)))) => false,
        Ok(Some(Ok(AgentEvent::HistoryReplaced(new_messages)))) => {
            *reply_progressed = true;
            *all_messages = new_messages.clone();
            !sink
                .publish(MessageEvent::UpdateConversation {
                    conversation: new_messages,
                })
                .await
        }
        Ok(Some(Ok(AgentEvent::McpNotification((request_id, notification))))) => {
            *reply_progressed = true;
            !sink
                .publish(MessageEvent::Notification {
                    request_id,
                    message: notification,
                })
                .await
        }
        Ok(Some(Err(error))) => {
            if !*reply_progressed {
                rollback_if_pending(state.session_manager(), session_id, rollback_state).await;
            }
            tracing::error!("Error processing message: {}", error);
            let _ = sink
                .publish(MessageEvent::Error {
                    error: error.to_string(),
                })
                .await;
            true
        }
        Ok(None) => {
            if !*reply_progressed {
                rollback_if_pending(state.session_manager(), session_id, rollback_state).await;
            }
            true
        }
        Err(_) => false,
    }
}

async fn rollback_if_pending(
    session_manager: &SessionManager,
    session_id: &str,
    rollback_state: &mut Option<ConversationOverrideRollback>,
) {
    if let Some(rollback) = rollback_state.take() {
        rollback_conversation_override(session_manager, session_id, rollback).await;
    }
}

async fn log_session_completion(
    session_manager: &SessionManager,
    session_id: &str,
    session_start: Instant,
    fallback_message_count: usize,
) {
    let session_duration = session_start.elapsed();

    if let Ok(session) = session_manager.get_session(session_id, true).await {
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
            message_count = fallback_message_count,
            "Session completed"
        );

        tracing::info!(
            monotonic_counter.gosling.session_duration_ms = session_duration.as_millis() as u64,
            session_type = "app",
            interface = "ui",
            "Session duration"
        );
    }
}
