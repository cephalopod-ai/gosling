//! A reply attempt that breaks off mid-stream and is retried is dropped from
//! the history; ACP clients that were shown it are told to drop it too, so
//! the live view matches a replay (GSL-PT-20260927-B06).

#[allow(dead_code)]
#[path = "acp_common_tests/mod.rs"]
mod common_tests;

use agent_client_protocol::schema::v1::{
    ContentBlock, PromptRequest, SessionId, SessionUpdate, StopReason, TextContent,
};
use common_tests::fixtures::server::{AcpServerConnection, AcpServerSession};
use common_tests::fixtures::{
    run_test, Connection, OpenAiFixture, Session, SessionData, TestConnectionConfig,
};
use gosling::config::base::CONFIG_YAML_NAME;
use gosling::conversation::message::Message;
use gosling::session::SessionManager;
use gosling_test_support::TEST_MODEL;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

fn openai_chunk(id: &str, delta: serde_json::Value, finish: serde_json::Value) -> String {
    let chunk = serde_json::json!({
        "id": id,
        "object": "chat.completion.chunk",
        "created": 1766229303,
        "model": TEST_MODEL,
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
    });
    format!("data: {chunk}\n\n")
}

/// An OpenAI-compatible endpoint whose first `broken_replies` replies break
/// off after one chunk; every later reply is `COMPLETE`.
async fn flaky_openai_endpoint(broken_replies: usize) -> String {
    use axum::http::header::CONTENT_TYPE;
    let replies = Arc::new(AtomicUsize::new(0));
    let app = axum::Router::new()
        .route(
            "/v1/models",
            axum::routing::get(|| async {
                (
                    [(CONTENT_TYPE, "application/json")],
                    include_str!("acp_test_data/openai_models.json"),
                )
            }),
        )
        .route(
            "/v1/chat/completions",
            axum::routing::post(move || {
                let broken = replies.fetch_add(1, Ordering::SeqCst) < broken_replies;
                async move {
                    let body = async_stream::stream! {
                        if broken {
                            yield Ok::<_, std::io::Error>(bytes::Bytes::from(openai_chunk(
                                "chatcmpl-broken",
                                serde_json::json!({"role": "assistant", "content": "BROKEN-"}),
                                serde_json::Value::Null,
                            )));
                            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                            yield Err(std::io::Error::other("connection reset mid-reply"));
                        } else {
                            yield Ok(bytes::Bytes::from(openai_chunk(
                                "chatcmpl-complete",
                                serde_json::json!({"role": "assistant", "content": "COMPLETE"}),
                                serde_json::Value::Null,
                            )));
                            yield Ok(bytes::Bytes::from(openai_chunk(
                                "chatcmpl-complete",
                                serde_json::json!({}),
                                serde_json::json!("stop"),
                            )));
                            yield Ok(bytes::Bytes::from("data: [DONE]\n\n"));
                        }
                    };
                    (
                        [(CONTENT_TYPE, "text/event-stream")],
                        axum::body::Body::from_stream(body),
                    )
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    format!("http://{address}")
}

/// Sends the session's provider requests to `host` from now on.
fn point_openai_host_at(data_root: &Path, host: &str) {
    let global = gosling::config::paths::Paths::config_dir().join(CONFIG_YAML_NAME);
    for config_path in [data_root.join(CONFIG_YAML_NAME), global] {
        let contents = std::fs::read_to_string(&config_path).unwrap();
        let mut config: serde_yaml::Mapping = serde_yaml::from_str(&contents).unwrap();
        config.insert("OPENAI_HOST".into(), host.into());
        std::fs::write(&config_path, serde_yaml::to_string(&config).unwrap()).unwrap();
    }
}

fn gosling_meta(update: &SessionUpdate) -> Option<&serde_json::Map<String, serde_json::Value>> {
    let meta = match update {
        SessionUpdate::AgentMessageChunk(chunk) => chunk.meta.as_ref(),
        SessionUpdate::SessionInfoUpdate(info) => info.meta.as_ref(),
        _ => None,
    }?;
    meta.get("gosling")?.as_object()
}

/// The shown text of each streamed message, by message id, in order.
fn streamed_messages(updates: &[SessionUpdate]) -> Vec<(String, String)> {
    let mut messages: Vec<(String, String)> = Vec::new();
    for update in updates {
        let SessionUpdate::AgentMessageChunk(chunk) = update else {
            continue;
        };
        let ContentBlock::Text(text) = &chunk.content else {
            continue;
        };
        let id = gosling_meta(update)
            .and_then(|gosling| gosling.get("messageId"))
            .and_then(|id| id.as_str())
            .unwrap_or_default()
            .to_string();
        match messages.last_mut() {
            Some((last_id, shown)) if *last_id == id => shown.push_str(&text.text),
            _ => messages.push((id, text.text.clone())),
        }
    }
    messages
}

fn retractions(updates: &[SessionUpdate]) -> Vec<Vec<String>> {
    updates
        .iter()
        .filter_map(|update| gosling_meta(update)?.get("retractedMessageIds"))
        .map(|ids| serde_json::from_value(ids.clone()).unwrap())
        .collect()
}

/// The session's updates, collected until a chunk showing `text` arrived.
async fn updates_through(session: &AcpServerSession, text: &str) -> Vec<SessionUpdate> {
    let mut updates = Vec::new();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            updates.extend(session.session_updates());
            if streamed_messages(&updates)
                .iter()
                .any(|(_, shown)| shown == text)
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the reply was shown");
    updates
}

async fn stored_texts(data_root: &Path, session_id: &SessionId) -> Vec<String> {
    SessionManager::new(data_root.to_path_buf())
        .get_session(&session_id.0, true)
        .await
        .unwrap()
        .conversation
        .unwrap_or_default()
        .messages()
        .iter()
        .map(Message::as_concat_text)
        .collect()
}

#[test]
fn a_retried_reply_attempt_is_retracted_from_the_live_view() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let openai = OpenAiFixture::new(
            vec![],
            <AcpServerConnection as Connection>::expected_session_id(),
        )
        .await;
        let mut conn = <AcpServerConnection as Connection>::new(
            TestConnectionConfig {
                data_root: data_root.path().to_path_buf(),
                ..Default::default()
            },
            openai,
        )
        .await;
        point_openai_host_at(data_root.path(), &flaky_openai_endpoint(1).await);
        let SessionData { session, .. } = conn.new_session().await.unwrap();
        let session_id = session.session_id().clone();

        let prompt = |text: &str| {
            conn.cx().send_request(PromptRequest::new(
                session_id.clone(),
                vec![ContentBlock::Text(TextContent::new(text))],
            ))
        };
        let response = prompt("tell me something").block_task().await.unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        let updates = updates_through(&session, "COMPLETE").await;

        let streamed = streamed_messages(&updates);
        let [(broken_id, broken), (complete_id, complete)] = streamed.as_slice() else {
            panic!("expected the broken attempt and the retry, got {streamed:?}");
        };
        assert_eq!(
            (broken.as_str(), complete.as_str()),
            ("BROKEN-", "COMPLETE")
        );
        assert_ne!(broken_id, complete_id);
        assert_eq!(retractions(&updates), vec![vec![broken_id.clone()]]);
        let retracted_at = updates
            .iter()
            .position(|update| {
                gosling_meta(update)
                    .is_some_and(|gosling| gosling.contains_key("retractedMessageIds"))
            })
            .unwrap();
        let retry_shown_at = updates
            .iter()
            .position(|update| {
                matches!(update, SessionUpdate::AgentMessageChunk(_))
                    && gosling_meta(update)
                        .and_then(|gosling| gosling.get("messageId"))
                        .and_then(|id| id.as_str())
                        == Some(complete_id.as_str())
            })
            .unwrap();
        assert!(
            retracted_at < retry_shown_at,
            "retracted before the retry is shown"
        );
        assert_eq!(
            stored_texts(data_root.path(), &session_id).await,
            vec!["tell me something".to_string(), "COMPLETE".to_string()]
        );

        let response = prompt("and another thing").block_task().await.unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        assert!(
            retractions(&updates_through(&session, "COMPLETE").await).is_empty(),
            "a reply that streams through is not retracted"
        );
    });
}

#[test]
fn replacing_the_history_without_dropping_shown_messages_retracts_nothing() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let openai = OpenAiFixture::new(
            vec![],
            <AcpServerConnection as Connection>::expected_session_id(),
        )
        .await;
        let mut conn = <AcpServerConnection as Connection>::new(
            TestConnectionConfig {
                data_root: data_root.path().to_path_buf(),
                ..Default::default()
            },
            openai,
        )
        .await;
        point_openai_host_at(data_root.path(), &flaky_openai_endpoint(0).await);
        let SessionData { session, .. } = conn.new_session().await.unwrap();
        let session_id = session.session_id().clone();
        let prompt = |text: &str| {
            conn.cx().send_request(PromptRequest::new(
                session_id.clone(),
                vec![ContentBlock::Text(TextContent::new(text))],
            ))
        };
        prompt("remember this").block_task().await.unwrap();
        updates_through(&session, "COMPLETE").await;

        let response = prompt("/compact").block_task().await.unwrap();
        assert_eq!(response.stop_reason, StopReason::EndTurn);
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        let updates = session.session_updates();
        assert!(
            !streamed_messages(&updates).is_empty(),
            "the command's reply was shown"
        );
        assert!(retractions(&updates).is_empty(), "{updates:?}");
    });
}
