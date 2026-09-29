#[allow(dead_code)]
#[path = "acp_common_tests/mod.rs"]
mod common_tests;

use agent_client_protocol::schema::v1::{
    CancelNotification, ContentBlock, PromptRequest, PromptResponse, SessionId, SessionUpdate,
    StopReason, TextContent,
};
use agent_client_protocol::{Agent, ConnectionTo, ErrorCode};
use common_tests::fixtures::server::{AcpServerConnection, AcpServerSession};
use common_tests::fixtures::{
    run_test, Connection, OpenAiFixture, PermissionDecision, Session, SessionData,
    TestConnectionConfig,
};
use gosling::config::base::CONFIG_YAML_NAME;
use gosling::conversation::message::Message;
use gosling::session::{AcpPromptRunState, ExtensionState, SessionManager};
use gosling_test_support::TEST_MODEL;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::{ConnectOptions, SqliteConnection};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Notify;

/// A second writer holding the session store, like another Gosling process.
async fn lock_session_store(data_root: &Path) -> SqliteConnection {
    let mut connection = SqliteConnectOptions::new()
        .filename(data_root.join("sessions").join("sessions.db"))
        .connect()
        .await
        .unwrap();
    sqlx::query("BEGIN IMMEDIATE")
        .execute(&mut connection)
        .await
        .unwrap();
    connection
}

async fn release_session_store(mut connection: SqliteConnection) {
    sqlx::query("ROLLBACK")
        .execute(&mut connection)
        .await
        .unwrap();
}

fn spawn_prompt(
    cx: &ConnectionTo<Agent>,
    session_id: &SessionId,
    text: &str,
) -> tokio::task::JoinHandle<Result<PromptResponse, agent_client_protocol::Error>> {
    let cx = cx.clone();
    let request = PromptRequest::new(
        session_id.clone(),
        vec![ContentBlock::Text(TextContent::new(text))],
    );
    tokio::spawn(async move { cx.send_request(request).block_task().await })
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

async fn stored_run_state(data_root: &Path, session_id: &SessionId) -> Option<AcpPromptRunState> {
    let session = SessionManager::new(data_root.to_path_buf())
        .get_session(&session_id.0, false)
        .await
        .unwrap();
    AcpPromptRunState::from_extension_data(&session.extension_data)
}

async fn wait_for_agent_text(session: &AcpServerSession) {
    tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if session
                .session_updates()
                .iter()
                .any(|update| matches!(update, SessionUpdate::AgentMessageChunk(_)))
            {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the reply started streaming");
}

#[test]
fn a_prompt_cancelled_while_the_session_store_is_locked_ends_without_waiting_for_it() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let openai = OpenAiFixture::new(
            vec![(
                "what is 1+1".to_string(),
                include_str!("acp_test_data/openai_basic.txt"),
            )],
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
        let SessionData { mut session, .. } = conn.new_session().await.unwrap();
        let session_id = session.session_id().clone();

        let store_lock = lock_session_store(data_root.path()).await;
        let prompt = spawn_prompt(conn.cx(), &session_id, "CANCELLED-WHILE-LOCKED");
        tokio::time::sleep(Duration::from_millis(300)).await;
        conn.cx()
            .send_notification(CancelNotification::new(session_id.clone()))
            .unwrap();

        let response = tokio::time::timeout(Duration::from_secs(3), prompt)
            .await
            .expect("a cancel must not wait for the session store to be released")
            .unwrap()
            .unwrap();
        assert_eq!(response.stop_reason, StopReason::Cancelled);

        release_session_store(store_lock).await;
        assert!(
            !stored_texts(data_root.path(), &session_id)
                .await
                .iter()
                .any(|text| text.contains("CANCELLED-WHILE-LOCKED")),
            "the cancelled prompt never started, so it must not be in the history"
        );

        let output = session
            .prompt("what is 1+1", PermissionDecision::Cancel)
            .await
            .unwrap();
        assert_eq!(output.text, "2");
    });
}

fn openai_text_chunk(text: &str) -> String {
    let chunk = serde_json::json!({
        "id": "chatcmpl-staged",
        "object": "chat.completion.chunk",
        "created": 1766229303,
        "model": TEST_MODEL,
        "choices": [{"index": 0, "delta": {"role": "assistant", "content": text}, "finish_reason": null}],
    });
    format!("data: {chunk}\n\n")
}

/// An OpenAI-compatible endpoint whose reply streams one chunk, a second one
/// when released, and then stalls.
async fn staged_openai_endpoint(release_second_chunk: Arc<Notify>) -> String {
    use axum::http::header::CONTENT_TYPE;
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
                let release_second_chunk = Arc::clone(&release_second_chunk);
                async move {
                    let body = async_stream::stream! {
                        yield Ok::<_, std::io::Error>(bytes::Bytes::from(openai_text_chunk("partial ")));
                        release_second_chunk.notified().await;
                        yield Ok(bytes::Bytes::from(openai_text_chunk("more")));
                        std::future::pending::<()>().await;
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

#[test]
fn a_cancel_while_the_reply_waits_on_the_session_store_still_closes_the_turn() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let release_second_chunk = Arc::new(Notify::new());
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
        let staged = staged_openai_endpoint(Arc::clone(&release_second_chunk)).await;
        point_openai_host_at(data_root.path(), &staged);
        let SessionData { session, .. } = conn.new_session().await.unwrap();
        let session_id = session.session_id().clone();

        let prompt = spawn_prompt(conn.cx(), &session_id, "write a long answer");
        wait_for_agent_text(&session).await;
        // Past the streaming checkpoint interval, so the next chunk is saved.
        tokio::time::sleep(Duration::from_millis(400)).await;
        let store_lock = lock_session_store(data_root.path()).await;
        release_second_chunk.notify_one();
        tokio::time::sleep(Duration::from_millis(300)).await;
        conn.cx()
            .send_notification(CancelNotification::new(session_id.clone()))
            .unwrap();
        tokio::time::sleep(Duration::from_millis(500)).await;
        release_session_store(store_lock).await;

        let response = tokio::time::timeout(Duration::from_secs(5), prompt)
            .await
            .expect("the cancelled prompt must answer once the session store is free")
            .unwrap()
            .unwrap();
        assert_eq!(response.stop_reason, StopReason::Cancelled);
        assert_eq!(
            stored_texts(data_root.path(), &session_id).await,
            vec![
                "write a long answer".to_string(),
                "partial ".to_string(),
                "Run cancelled by user before completion.".to_string(),
            ]
        );
        assert_eq!(
            stored_run_state(data_root.path(), &session_id).await,
            Some(AcpPromptRunState::Cancelled)
        );
    });
}

async fn connect(
    data_root: &Path,
    session_manager: Option<Arc<SessionManager>>,
) -> AcpServerConnection {
    let openai = OpenAiFixture::new(
        vec![],
        <AcpServerConnection as Connection>::expected_session_id(),
    )
    .await;
    <AcpServerConnection as Connection>::new(
        TestConnectionConfig {
            data_root: data_root.to_path_buf(),
            session_manager,
            ..Default::default()
        },
        openai,
    )
    .await
}

/// A cancelled turn releases its lease after it has answered.
async fn wait_for_turn_lease_release(data_root: &Path, session_id: &SessionId) {
    let mut connection = SqliteConnectOptions::new()
        .filename(data_root.join("sessions").join("sessions.db"))
        .connect()
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let leases: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM session_turn_leases WHERE session_id = ?")
                    .bind(session_id.0.as_ref())
                    .fetch_one(&mut connection)
                    .await
                    .unwrap();
            if leases == 0 {
                return;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("the cancelled turn released its lease");
}

async fn cancel_and_wait(
    cx: &ConnectionTo<Agent>,
    session_id: &SessionId,
    prompt: tokio::task::JoinHandle<Result<PromptResponse, agent_client_protocol::Error>>,
) {
    cx.send_notification(CancelNotification::new(session_id.clone()))
        .unwrap();
    let response = tokio::time::timeout(Duration::from_secs(5), prompt)
        .await
        .expect("the cancelled prompt answered")
        .unwrap()
        .unwrap();
    assert_eq!(response.stop_reason, StopReason::Cancelled);
}

// F12: the connections of one server share its session store. A prompt from a
// second connection while the session's turn runs on another one is refused
// before it starts, says why, and leaves the running turn alone.
#[test]
fn a_prompt_on_a_session_busy_on_another_connection_is_refused_before_it_starts() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let store = Arc::new(SessionManager::new(data_root.path().to_path_buf()));
        let mut owner = connect(data_root.path(), Some(Arc::clone(&store))).await;
        let mut other = connect(data_root.path(), Some(Arc::clone(&store))).await;
        let staged = staged_openai_endpoint(Arc::new(Notify::new())).await;
        point_openai_host_at(data_root.path(), &staged);
        let SessionData {
            session: owner_session,
            ..
        } = owner.new_session().await.unwrap();
        let session_id = owner_session.session_id().clone();
        let SessionData {
            session: other_session,
            ..
        } = other.load_session(&session_id.0, vec![]).await.unwrap();

        let running = spawn_prompt(other.cx(), &session_id, "a long turn");
        wait_for_agent_text(&other_session).await;

        let error = tokio::time::timeout(
            Duration::from_secs(5),
            spawn_prompt(owner.cx(), &session_id, "SECOND-PROMPT"),
        )
        .await
        .expect("the second prompt is answered without waiting for the running turn")
        .unwrap()
        .expect_err("the second prompt is refused");
        assert_eq!(error.code, ErrorCode::InvalidRequest, "{error:?}");
        assert_eq!(
            error.data,
            Some(serde_json::json!(format!(
                "session {} already has a prompt running on another connection to this server",
                session_id.0
            )))
        );
        assert_eq!(
            stored_run_state(data_root.path(), &session_id).await,
            Some(AcpPromptRunState::InProgress),
            "the refused prompt must not record anything for the running turn"
        );
        assert!(!stored_texts(data_root.path(), &session_id)
            .await
            .iter()
            .any(|text| text.contains("SECOND-PROMPT")));

        cancel_and_wait(other.cx(), &session_id, running).await;
        wait_for_turn_lease_release(data_root.path(), &session_id).await;

        let idle = spawn_prompt(owner.cx(), &session_id, "the owner's turn");
        wait_for_agent_text(&owner_session).await;
        cancel_and_wait(owner.cx(), &session_id, idle).await;
    });
}

// Connections that do not share a store are separate owners, like another
// Gosling process or window: the turn lease still refuses the second prompt,
// with its own message.
#[test]
fn a_prompt_on_a_session_running_on_another_store_keeps_the_turn_lease_refusal() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let mut owner = connect(data_root.path(), None).await;
        let mut other = connect(data_root.path(), None).await;
        let staged = staged_openai_endpoint(Arc::new(Notify::new())).await;
        point_openai_host_at(data_root.path(), &staged);
        let SessionData {
            session: owner_session,
            ..
        } = owner.new_session().await.unwrap();
        let session_id = owner_session.session_id().clone();
        let SessionData {
            session: other_session,
            ..
        } = other.load_session(&session_id.0, vec![]).await.unwrap();

        let running = spawn_prompt(other.cx(), &session_id, "a long turn");
        wait_for_agent_text(&other_session).await;

        let error = tokio::time::timeout(
            Duration::from_secs(5),
            spawn_prompt(owner.cx(), &session_id, "SECOND-PROMPT"),
        )
        .await
        .expect("the second prompt is answered without waiting for the running turn")
        .unwrap()
        .expect_err("the second prompt is refused");
        assert_eq!(error.code, ErrorCode::InternalError, "{error:?}");
        assert_eq!(
            error.data,
            Some(serde_json::json!(format!(
                "Error getting agent reply: session {} already has an active turn in another Gosling process or window",
                session_id.0
            )))
        );

        cancel_and_wait(other.cx(), &session_id, running).await;
    });
}
