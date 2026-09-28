#[allow(dead_code)]
#[path = "acp_common_tests/mod.rs"]
mod common_tests;
use agent_client_protocol::schema::v1::{
    ContentBlock, ListSessionsRequest, ListSessionsResponse, NewSessionRequest, PromptRequest,
    SessionConfigKind, SessionConfigOptionCategory, SessionConfigOptionValue, SessionInfo,
    SessionUpdate, SetSessionConfigOptionRequest, StopReason, TextContent,
};
use agent_client_protocol::ErrorCode;
use common_tests::fixtures::server::AcpServerConnection;
use common_tests::fixtures::{
    run_test, Connection, OpenAiFixture, PermissionDecision, Session, SessionData,
    TestConnectionConfig,
};
#[cfg(feature = "code-mode")]
use common_tests::run_prompt_codemode;
use common_tests::{
    run_close_session, run_config_mcp, run_config_option_mode_set, run_config_option_model_set,
    run_delete_session, run_fs_read_text_file_true, run_fs_write_text_file_false,
    run_fs_write_text_file_true, run_initialize_doesnt_hit_provider, run_list_sessions,
    run_load_mode, run_load_model, run_load_session_error, run_load_session_mcp,
    run_load_session_replays_image_attachment, run_mode_set, run_model_list, run_model_set,
    run_model_set_error_session_not_found, run_new_session_returns_initial_config,
    run_new_session_uses_current_config_mode, run_permission_persistence, run_prompt_basic,
    run_prompt_error, run_prompt_image, run_prompt_image_attachment, run_prompt_mcp,
    run_prompt_model_mismatch, run_prompt_skill, run_session_name_update_notification,
    run_shell_terminal_false, run_shell_terminal_true,
};
use gosling::config::GoslingMode;
use gosling::conversation::message::{Message, MessageMetadata};
use gosling::custom_requests::{GetSessionInfoRequest, GetSessionInfoResponse};
use gosling::session::{SessionManager, SessionType};
use gosling_test_support::TEST_MODEL;
use std::path::Path;

tests_config_option_set_error!(AcpServerConnection);
tests_mode_set_error!(AcpServerConnection);

async fn seed_list_sessions(data_root: &Path, working_dir: &Path, count: usize) {
    let session_manager = SessionManager::new(data_root.to_path_buf());
    for index in 0..count {
        let session = session_manager
            .create_session(
                working_dir.to_path_buf(),
                format!("Seed session {index}"),
                SessionType::Acp,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        session_manager
            .add_message(&session.id, &Message::user().with_text("hello"))
            .await
            .unwrap();
    }
}

async fn seed_list_session_with_message(
    data_root: &Path,
    working_dir: &Path,
    name: &str,
    session_type: SessionType,
    message: &str,
) {
    let session_manager = SessionManager::new(data_root.to_path_buf());
    let session = session_manager
        .create_session(
            working_dir.to_path_buf(),
            name.to_string(),
            session_type,
            GoslingMode::default(),
        )
        .await
        .unwrap();
    session_manager
        .add_message(&session.id, &Message::user().with_text(message))
        .await
        .unwrap();
}

async fn new_connection(data_root: &Path) -> AcpServerConnection {
    let openai = OpenAiFixture::new(
        vec![],
        <AcpServerConnection as Connection>::expected_session_id(),
    )
    .await;
    <AcpServerConnection as Connection>::new(
        TestConnectionConfig {
            data_root: data_root.to_path_buf(),
            ..Default::default()
        },
        openai,
    )
    .await
}

async fn list_sessions_request(
    conn: &AcpServerConnection,
    request: ListSessionsRequest,
) -> anyhow::Result<ListSessionsResponse> {
    conn.cx()
        .send_request(request)
        .block_task()
        .await
        .map_err(Into::into)
}

async fn get_session_info_request(
    conn: &AcpServerConnection,
    request: GetSessionInfoRequest,
) -> anyhow::Result<GetSessionInfoResponse> {
    conn.cx()
        .send_request(request)
        .block_task()
        .await
        .map_err(Into::into)
}

fn assert_invalid_params(error: anyhow::Error) {
    let acp_error = error.downcast::<agent_client_protocol::Error>().unwrap();
    assert_eq!(acp_error.code, ErrorCode::InvalidParams);
}

fn include_last_message_snippet_meta(
    value: serde_json::Value,
) -> serde_json::Map<String, serde_json::Value> {
    gosling_meta([("includeLastMessageSnippet", value)])
}

fn archive_state_meta(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    gosling_meta([("archiveState", value)])
}

fn gosling_meta<const N: usize>(
    entries: [(&str, serde_json::Value); N],
) -> serde_json::Map<String, serde_json::Value> {
    let mut gosling = serde_json::Map::new();
    for (key, value) in entries {
        gosling.insert(key.to_string(), value);
    }

    let mut meta = serde_json::Map::new();
    meta.insert("gosling".to_string(), serde_json::Value::Object(gosling));
    meta
}

fn last_message_snippet(session: &SessionInfo) -> Option<&str> {
    session
        .meta
        .as_ref()
        .and_then(|meta| meta.get("lastMessageSnippet"))
        .and_then(serde_json::Value::as_str)
}

#[test]
fn test_config_mcp() {
    run_test(async { run_config_mcp::<AcpServerConnection>().await });
}

#[test]
fn test_prompt_repeating_a_failing_tool_call_stops_with_max_turn_requests() {
    run_test(async {
        let prompt = "Keep calling get_code until it works.";
        // One tool call that fails (no such tool), then three repetition denials. A fifth
        // provider request would find no exchange and fail the prompt instead.
        let exchanges = (0..4)
            .map(|_| {
                (
                    prompt.to_string(),
                    include_str!("acp_test_data/openai_tool_call.txt"),
                )
            })
            .collect();
        let openai = OpenAiFixture::new(
            exchanges,
            <AcpServerConnection as Connection>::expected_session_id(),
        )
        .await;
        let mut conn = <AcpServerConnection as Connection>::new(
            TestConnectionConfig {
                gosling_mode: GoslingMode::Auto,
                ..Default::default()
            },
            openai,
        )
        .await;
        let SessionData { session, .. } = conn.new_session().await.unwrap();

        let response = conn
            .cx()
            .send_request(PromptRequest::new(
                session.session_id().clone(),
                vec![ContentBlock::Text(TextContent::new(prompt))],
            ))
            .block_task()
            .await
            .unwrap();

        assert_eq!(response.stop_reason, StopReason::MaxTurnRequests);
    });
}

#[test]
fn test_config_option_mode_set() {
    run_test(async { run_config_option_mode_set::<AcpServerConnection>().await });
}

#[test]
fn test_list_sessions() {
    run_test(async { run_list_sessions::<AcpServerConnection>().await });
}

#[test]
fn test_list_sessions_emits_computed_snippet() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let cwd = Path::new("/tmp/acp-session-list-snippet");
        let session_manager = SessionManager::new(data_root.path().to_path_buf());
        let session = session_manager
            .create_session(
                cwd.to_path_buf(),
                "Live subtitle".to_string(),
                SessionType::Acp,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        session_manager
            .add_message(
                &session.id,
                &Message::user().with_text("**raw** _markdown_ subtitle"),
            )
            .await
            .unwrap();
        session_manager
            .add_message(
                &session.id,
                &Message::assistant()
                    .with_text("hidden newer text")
                    .with_metadata(MessageMetadata::agent_only()),
            )
            .await
            .unwrap();

        let conn = new_connection(data_root.path()).await;
        let response = list_sessions_request(
            &conn,
            ListSessionsRequest::new()
                .meta(include_last_message_snippet_meta(serde_json::Value::Null)),
        )
        .await
        .unwrap();

        assert_eq!(response.sessions.len(), 1);
        assert_eq!(last_message_snippet(&response.sessions[0]), None);

        let response = list_sessions_request(
            &conn,
            ListSessionsRequest::new().meta(include_last_message_snippet_meta(
                serde_json::Value::Bool(false),
            )),
        )
        .await
        .unwrap();

        assert_eq!(response.sessions.len(), 1);
        assert_eq!(last_message_snippet(&response.sessions[0]), None);

        let response = list_sessions_request(
            &conn,
            ListSessionsRequest::new().meta(include_last_message_snippet_meta(
                serde_json::Value::Bool(true),
            )),
        )
        .await
        .unwrap();

        assert_eq!(response.sessions.len(), 1);
        assert_eq!(
            last_message_snippet(&response.sessions[0]),
            Some("**raw** _markdown_ subtitle")
        );
    });
}

#[test]
fn test_list_sessions_pagination() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        seed_list_sessions(data_root.path(), Path::new("/tmp/acp-session-list"), 51).await;
        let conn = new_connection(data_root.path()).await;

        let first = list_sessions_request(&conn, ListSessionsRequest::new())
            .await
            .unwrap();
        assert_eq!(first.sessions.len(), 50);
        assert!(first
            .sessions
            .iter()
            .all(|session| last_message_snippet(session).is_none()));

        let second = list_sessions_request(
            &conn,
            ListSessionsRequest::new()
                .cursor(first.next_cursor.clone().unwrap())
                .meta(include_last_message_snippet_meta(serde_json::Value::Bool(
                    true,
                ))),
        )
        .await
        .unwrap();
        assert_eq!(second.sessions.len(), 1);
        assert!(second.next_cursor.is_none());
        assert_eq!(last_message_snippet(&second.sessions[0]), Some("hello"));

        let second_id = &second.sessions[0].session_id;
        assert!(first
            .sessions
            .iter()
            .all(|session| session.session_id != *second_id));
    });
}

#[test]
fn test_list_sessions_query_filters_results() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let cwd = Path::new("/tmp/acp-session-list");
        seed_list_session_with_message(
            data_root.path(),
            cwd,
            "Postgres session",
            SessionType::Acp,
            "Discuss Postgres migrations",
        )
        .await;
        seed_list_session_with_message(
            data_root.path(),
            cwd,
            "Mobile session",
            SessionType::Acp,
            "Plan the mobile release",
        )
        .await;
        let conn = new_connection(data_root.path()).await;

        let mut meta = serde_json::Map::new();
        meta.insert(
            "query".to_string(),
            serde_json::Value::String("postgres".to_string()),
        );
        let response = list_sessions_request(&conn, ListSessionsRequest::new().meta(meta))
            .await
            .unwrap();

        assert_eq!(response.sessions.len(), 1);
        assert_eq!(
            response.sessions[0].title.as_deref(),
            Some("Postgres session")
        );
        assert!(response.next_cursor.is_none());
    });
}

#[test]
fn test_list_sessions_types_override_filters_results() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let cwd = Path::new("/tmp/acp-session-list");
        seed_list_session_with_message(
            data_root.path(),
            cwd,
            "ACP session",
            SessionType::Acp,
            "ACP message",
        )
        .await;
        seed_list_session_with_message(
            data_root.path(),
            cwd,
            "User session",
            SessionType::User,
            "User message",
        )
        .await;
        let conn = new_connection(data_root.path()).await;

        let mut meta = serde_json::Map::new();
        meta.insert(
            "types".to_string(),
            serde_json::Value::Array(vec![serde_json::Value::String("user".to_string())]),
        );
        let response = list_sessions_request(&conn, ListSessionsRequest::new().meta(meta))
            .await
            .unwrap();

        assert_eq!(response.sessions.len(), 1);
        assert_eq!(response.sessions[0].title.as_deref(), Some("User session"));
        assert!(response.next_cursor.is_none());
    });
}

#[test]
fn test_list_sessions_types_rejects_internal_session_types() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let conn = new_connection(data_root.path()).await;

        for session_type in ["hidden", "sub_agent"] {
            let mut meta = serde_json::Map::new();
            meta.insert(
                "types".to_string(),
                serde_json::Value::Array(vec![serde_json::Value::String(session_type.to_string())]),
            );

            let error = list_sessions_request(&conn, ListSessionsRequest::new().meta(meta))
                .await
                .unwrap_err();
            assert_invalid_params(error);
        }
    });
}

#[test]
fn test_list_sessions_archive_state_filters_results() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let cwd = Path::new("/tmp/acp-session-list-archive");
        let session_manager = SessionManager::new(data_root.path().to_path_buf());

        let active_session = session_manager
            .create_session(
                cwd.to_path_buf(),
                "Active session".to_string(),
                SessionType::Acp,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        session_manager
            .add_message(
                &active_session.id,
                &Message::user().with_text("hello active"),
            )
            .await
            .unwrap();

        let archived_session = session_manager
            .create_session(
                cwd.to_path_buf(),
                "Archived session".to_string(),
                SessionType::Acp,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        session_manager
            .add_message(
                &archived_session.id,
                &Message::user().with_text("hello archived"),
            )
            .await
            .unwrap();
        session_manager
            .update(&archived_session.id)
            .archived_at(Some(chrono::Utc::now()))
            .apply()
            .await
            .unwrap();

        let conn = new_connection(data_root.path()).await;

        let active = list_sessions_request(
            &conn,
            ListSessionsRequest::new().meta(archive_state_meta(serde_json::Value::String(
                "active".to_string(),
            ))),
        )
        .await
        .unwrap();
        assert_eq!(active.sessions.len(), 1);
        assert_eq!(active.sessions[0].title.as_deref(), Some("Active session"));

        let archived = list_sessions_request(
            &conn,
            ListSessionsRequest::new().meta(archive_state_meta(serde_json::Value::String(
                "archived".to_string(),
            ))),
        )
        .await
        .unwrap();
        assert_eq!(archived.sessions.len(), 1);
        assert_eq!(
            archived.sessions[0].title.as_deref(),
            Some("Archived session")
        );

        let all = list_sessions_request(
            &conn,
            ListSessionsRequest::new().meta(archive_state_meta(serde_json::Value::String(
                "all".to_string(),
            ))),
        )
        .await
        .unwrap();
        assert_eq!(all.sessions.len(), 2);
    });
}

#[test]
fn test_list_sessions_invalid_params() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let other_cwd = tempfile::tempdir().unwrap();
        seed_list_sessions(data_root.path(), cwd.path(), 51).await;
        let conn = new_connection(data_root.path()).await;

        let error =
            list_sessions_request(&conn, ListSessionsRequest::new().cursor("*".to_string()))
                .await
                .unwrap_err();
        assert_invalid_params(error);

        let error = list_sessions_request(
            &conn,
            ListSessionsRequest::new().cwd(std::path::PathBuf::from("relative/path")),
        )
        .await
        .unwrap_err();
        assert_invalid_params(error);

        let first = list_sessions_request(&conn, ListSessionsRequest::new().cwd(cwd.path()))
            .await
            .unwrap();

        let error = list_sessions_request(
            &conn,
            ListSessionsRequest::new()
                .cwd(other_cwd.path())
                .cursor(first.next_cursor.unwrap()),
        )
        .await
        .unwrap_err();
        assert_invalid_params(error);

        let error = list_sessions_request(
            &conn,
            ListSessionsRequest::new().meta(include_last_message_snippet_meta(
                serde_json::Value::String("true".to_string()),
            )),
        )
        .await
        .unwrap_err();
        assert_invalid_params(error);

        let error = list_sessions_request(
            &conn,
            ListSessionsRequest::new().meta(archive_state_meta(serde_json::Value::String(
                "later".to_string(),
            ))),
        )
        .await
        .unwrap_err();
        assert_invalid_params(error);
    });
}

#[test]
fn test_get_session_info() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let cwd = Path::new("/tmp/acp-session-info");
        let session_manager = SessionManager::new(data_root.path().to_path_buf());
        let session = session_manager
            .create_session(
                cwd.to_path_buf(),
                "Session info".to_string(),
                SessionType::Acp,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        session_manager
            .add_message(&session.id, &Message::user().with_text("hello"))
            .await
            .unwrap();
        let conn = new_connection(data_root.path()).await;

        let response = get_session_info_request(
            &conn,
            GetSessionInfoRequest {
                session_id: session.id.clone(),
            },
        )
        .await
        .unwrap();

        assert_eq!(
            response.session.session_id,
            agent_client_protocol::schema::v1::SessionId::new(session.id)
        );
        assert_eq!(response.session.cwd, cwd.to_path_buf());
        assert_eq!(response.session.title.as_deref(), Some("Session info"));
        assert!(response.session.updated_at.is_some());

        let meta = response
            .session
            .meta
            .expect("session info should include meta");
        assert!(meta.get("createdAt").and_then(|v| v.as_str()).is_some());
        assert_eq!(meta.get("messageCount"), Some(&serde_json::json!(1)));
        assert_eq!(meta.get("userSetName"), Some(&serde_json::json!(false)));
        assert_eq!(meta.get("sessionType"), Some(&serde_json::json!("acp")));
    });
}

#[test]
#[ignore = "ACP server session naming updates are not emitted in this harness path"]
fn test_session_name_update_notification() {
    run_test(async { run_session_name_update_notification::<AcpServerConnection>().await });
}

#[test]
fn test_close_session() {
    run_test(async { run_close_session::<AcpServerConnection>().await });
}

#[test]
fn test_config_option_model_set() {
    run_test(async { run_config_option_model_set::<AcpServerConnection>().await });
}

#[test]
fn test_config_option_reselecting_the_current_model_keeps_the_session_as_is() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let mut conn = new_connection(data_root.path()).await;
        let session_manager = SessionManager::new(data_root.path().to_path_buf());
        let data = conn.new_session().await.unwrap();
        let session_id = data.session.session_id().0.to_string();
        let generation = || async {
            session_manager
                .latest_handoff_generation(&session_id)
                .await
                .unwrap()
        };
        let initial = generation().await;

        conn.set_config_option(&session_id, "model", TEST_MODEL)
            .await
            .unwrap();
        let after_reselecting_the_initial_model = generation().await;
        conn.set_config_option(&session_id, "model", "gpt-4o")
            .await
            .unwrap();
        let after_switch = generation().await;
        conn.set_config_option(&session_id, "model", "gpt-4o")
            .await
            .unwrap();
        let after_reselect = generation().await;

        assert_eq!(after_reselecting_the_initial_model, initial);
        assert_eq!(after_switch, initial + 1);
        assert_eq!(after_reselect, after_switch);
    });
}

#[test]
fn test_config_option_thinking_effort_set() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        std::fs::write(
            data_root
                .path()
                .join(gosling::config::base::CONFIG_YAML_NAME),
            "GOSLING_MODEL: gpt-5.1\nGOSLING_PROVIDER: openai\n",
        )
        .unwrap();
        let openai = OpenAiFixture::new(
            vec![],
            <AcpServerConnection as Connection>::expected_session_id(),
        )
        .await;
        let mut conn = <AcpServerConnection as Connection>::new(
            TestConnectionConfig {
                current_model: "gpt-5.1".to_string(),
                data_root: data_root.path().to_path_buf(),
                ..Default::default()
            },
            openai,
        )
        .await;
        let data = conn.new_session().await.unwrap();

        let response = conn
            .cx()
            .send_request(SetSessionConfigOptionRequest::new(
                data.session.session_id().clone(),
                "thinking_effort".to_string(),
                SessionConfigOptionValue::value_id("high".to_string()),
            ))
            .block_task()
            .await
            .unwrap();

        let option = response
            .config_options
            .iter()
            .find(|option| option.id.0.as_ref() == "thinking_effort")
            .expect("thinking_effort option");
        assert_eq!(
            option.category,
            Some(SessionConfigOptionCategory::ThoughtLevel)
        );
        let select = match &option.kind {
            SessionConfigKind::Select(select) => select,
            _ => panic!("thinking_effort should be a select option"),
        };

        assert_eq!(select.current_value.0.as_ref(), "high");
    });
}

#[test]
fn test_delete_session() {
    run_test(async { run_delete_session::<AcpServerConnection>().await });
}

#[test]
fn test_fs_read_text_file_true() {
    run_test(async { run_fs_read_text_file_true::<AcpServerConnection>().await });
}

#[test]
fn test_fs_write_text_file_false() {
    run_test(async { run_fs_write_text_file_false::<AcpServerConnection>().await });
}

#[test]
fn test_fs_write_text_file_true() {
    run_test(async { run_fs_write_text_file_true::<AcpServerConnection>().await });
}

#[test]
fn test_initialize_doesnt_hit_provider() {
    run_test(async { run_initialize_doesnt_hit_provider::<AcpServerConnection>().await });
}

#[test]
fn test_load_mode() {
    run_test(async { run_load_mode::<AcpServerConnection>().await });
}

#[test]
fn test_load_model() {
    run_test(async { run_load_model::<AcpServerConnection>().await });
}

#[test]
fn test_load_session_error_session_not_found() {
    run_test(async { run_load_session_error::<AcpServerConnection>().await });
}

#[test]
fn test_load_session_names_a_working_folder_that_was_moved_away() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let parent = tempfile::tempdir().unwrap();
        let working_dir = parent.path().join("playtest-primary");
        std::fs::create_dir(&working_dir).unwrap();
        seed_list_sessions(data_root.path(), &working_dir, 1).await;
        let session_id = SessionManager::new(data_root.path().to_path_buf())
            .list_all_sessions()
            .await
            .unwrap()
            .remove(0)
            .id;
        std::fs::rename(&working_dir, parent.path().join("playtest-primary-moved")).unwrap();
        let conn = new_connection(data_root.path()).await;
        let load = |cwd: std::path::PathBuf| {
            conn.cx()
                .send_request(agent_client_protocol::schema::v1::LoadSessionRequest::new(
                    agent_client_protocol::schema::v1::SessionId::new(session_id.clone()),
                    cwd,
                ))
        };

        let error: anyhow::Error = load(working_dir.clone())
            .block_task()
            .await
            .unwrap_err()
            .into();
        let error = error.downcast::<agent_client_protocol::Error>().unwrap();
        assert_eq!(error.code, ErrorCode::InvalidParams);
        let reason = error.data.as_ref().and_then(|data| data.as_str()).unwrap();
        assert!(
            reason.contains(&working_dir.display().to_string()),
            "{reason}"
        );

        let error: anyhow::Error = load("relative/folder".into())
            .block_task()
            .await
            .unwrap_err()
            .into();
        let error = error.downcast::<agent_client_protocol::Error>().unwrap();
        assert_eq!(
            error.data.as_ref().and_then(|data| data.as_str()),
            Some("cwd must be an absolute path")
        );
    });
}

#[test]
fn test_load_session_mcp() {
    run_test(async { run_load_session_mcp::<AcpServerConnection>().await });
}

#[test]
fn test_load_session_replays_image_attachment() {
    run_test(async { run_load_session_replays_image_attachment::<AcpServerConnection>().await });
}

#[test]
fn test_mode_set() {
    run_test(async { run_mode_set::<AcpServerConnection>().await });
}

#[test]
fn test_model_list() {
    run_test(async { run_model_list::<AcpServerConnection>().await });
}

#[test]
fn test_new_session_returns_initial_config() {
    run_test(async { run_new_session_returns_initial_config::<AcpServerConnection>().await });
}

#[test]
fn test_new_session_uses_current_config_mode() {
    run_test(async { run_new_session_uses_current_config_mode::<AcpServerConnection>().await });
}

#[test]
fn test_new_session_cleans_up_when_config_fails() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let conn = new_connection(data_root.path()).await;
        let work_dir = tempfile::tempdir().unwrap();
        let mut meta = serde_json::Map::new();
        meta.insert(
            "enabledExtensions".to_string(),
            serde_json::Value::String("invalid".to_string()),
        );

        let error: anyhow::Error = conn
            .cx()
            .send_request(NewSessionRequest::new(work_dir.path()).meta(meta))
            .block_task()
            .await
            .unwrap_err()
            .into();

        assert_invalid_params(error);

        let sessions = SessionManager::new(data_root.path().to_path_buf())
            .list_all_sessions()
            .await
            .unwrap();
        assert!(sessions.is_empty());
    });
}

#[test]
fn test_workspace_rejects_model_from_another_provider_before_session_activation() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let work_dir = tempfile::tempdir().unwrap();
        let workspace_dir = data_root.path().join("workspaces");
        std::fs::create_dir_all(&workspace_dir).unwrap();
        std::fs::write(
            workspace_dir.join("workspaces.json"),
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "active_workspace_id": "workspace-1",
                "default_workspace_id": "workspace-1",
                "migration_completed": true,
                "templates_materialized": true,
                "workspaces": [{
                    "id": "workspace-1",
                    "schemaVersion": 1,
                    "name": "Invalid provider model pair",
                    "workingFolder": work_dir.path(),
                    "productOutputFolders": [{
                        "id": "output",
                        "label": "Outputs",
                        "path": work_dir.path(),
                        "productTypes": ["document"],
                        "isDefault": true,
                        "createIfMissing": false
                    }],
                    "credentialBindings": [],
                    "defaultProvider": "chatgpt_codex",
                    "defaultModel": "claude-opus-4-8",
                    "createdAt": "2026-07-20T00:00:00Z",
                    "updatedAt": "2026-07-20T00:00:00Z",
                    "lastOpenedAt": "2026-07-20T00:00:00Z"
                }],
                "credential_profiles": [],
                "distribution_profile_secret_fields": {},
                "workspace_profile_required_secret_fields": {},
                "pending_secret_deletions": []
            }))
            .unwrap(),
        )
        .unwrap();
        let conn = new_connection(data_root.path()).await;
        let mut meta = serde_json::Map::new();
        meta.insert(
            "workspaceId".to_string(),
            serde_json::Value::String("workspace-1".to_string()),
        );

        let error: anyhow::Error = conn
            .cx()
            .send_request(NewSessionRequest::new(work_dir.path()).meta(meta))
            .block_task()
            .await
            .unwrap_err()
            .into();

        assert_invalid_params(error);
        let sessions = SessionManager::new(data_root.path().to_path_buf())
            .list_all_sessions()
            .await
            .unwrap();
        assert!(sessions.is_empty());
    });
}

#[test]
fn test_model_set() {
    run_test(async { run_model_set::<AcpServerConnection>().await });
}

#[test]
fn test_model_set_error_session_not_found() {
    run_test(async { run_model_set_error_session_not_found::<AcpServerConnection>().await });
}

#[test]
fn test_permission_persistence() {
    run_test(async { run_permission_persistence::<AcpServerConnection>().await });
}

#[test]
fn test_prompt_basic() {
    run_test(async { run_prompt_basic::<AcpServerConnection>().await });
}

#[test]
#[cfg(feature = "code-mode")]
fn test_prompt_codemode() {
    run_test(async { run_prompt_codemode::<AcpServerConnection>().await });
}

#[test]
fn test_prompt_error_session_not_found() {
    run_test(async { run_prompt_error::<AcpServerConnection>().await });
}

#[test]
fn test_prompt_image() {
    run_test(async { run_prompt_image::<AcpServerConnection>().await });
}

#[test]
fn test_prompt_image_attachment() {
    run_test(async { run_prompt_image_attachment::<AcpServerConnection>().await });
}

#[test]
fn test_prompt_mcp() {
    run_test(async { run_prompt_mcp::<AcpServerConnection>().await });
}

#[test]
fn test_prompt_model_mismatch() {
    run_test(async { run_prompt_model_mismatch::<AcpServerConnection>().await });
}

#[test]
fn test_prompt_skill() {
    run_test(async { run_prompt_skill::<AcpServerConnection>().await });
}

#[test]
fn test_shell_terminal_false() {
    run_test(async { run_shell_terminal_false::<AcpServerConnection>().await });
}

#[test]
fn test_shell_terminal_true() {
    run_test(async { run_shell_terminal_true::<AcpServerConnection>().await });
}

// GSL-PT-20260927-B10: the usage update sent when a prompt ended reused the
// context snapshot taken before the turn's provider call, so the gauge showed
// a local message estimate instead of the request the provider had just measured.
#[test]
fn test_prompt_end_usage_update_reports_the_last_provider_request() {
    run_test(async {
        let expected_session_id = AcpServerConnection::expected_session_id();
        let openai = OpenAiFixture::new(
            vec![(
                "what is 1+1".to_string(),
                include_str!("acp_test_data/openai_basic.txt"),
            )],
            expected_session_id.clone(),
        )
        .await;
        let mut conn = AcpServerConnection::new(TestConnectionConfig::default(), openai).await;
        let SessionData { mut session, .. } = conn.new_session().await.unwrap();
        expected_session_id.set(&session.session_id().0);

        session
            .prompt("what is 1+1", PermissionDecision::Cancel)
            .await
            .unwrap();

        let last_used = session
            .session_updates()
            .into_iter()
            .filter_map(|update| match update {
                SessionUpdate::UsageUpdate(usage) => Some(usage.used),
                _ => None,
            })
            .last();
        assert_eq!(last_used, Some(110));
    });
}
