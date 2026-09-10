#[allow(dead_code)]
#[path = "acp_common_tests/mod.rs"]
mod common_tests;

use common_tests::fixtures::server::AcpServerConnection;
use common_tests::fixtures::{
    run_test, send_custom, Connection, OpenAiFixture, PermissionDecision, Session,
    TestConnectionConfig,
};
use gosling::config::permission::PermissionLevel;
use gosling::config::PermissionManager;
use gosling::conversation::message::Message;
use gosling::session::SessionManager;
use gosling_test_support::TEST_MODEL;

async fn new_connection(data_root: &std::path::Path) -> AcpServerConnection {
    new_connection_with_exchanges(data_root, Vec::new()).await
}

async fn new_connection_with_exchanges(
    data_root: &std::path::Path,
    exchanges: Vec<(String, &'static str)>,
) -> AcpServerConnection {
    let openai = OpenAiFixture::new(
        exchanges,
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

#[test]
fn handoff_session_returns_a_bounded_checkpoint_and_a_history_free_new_session() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let mut conn = new_connection(data_root.path()).await;
        let session_manager = SessionManager::new(data_root.path().to_path_buf());

        let session_data = conn.new_session().await.unwrap();
        let session_id = session_data.session.session_id().0.to_string();
        session_manager
            .add_message(
                &session_id,
                &Message::user().with_text("let's do the thing"),
            )
            .await
            .unwrap();
        session_manager
            .add_message(
                &session_id,
                &Message::assistant().with_text("sure, working on it"),
            )
            .await
            .unwrap();

        let response = send_custom(
            conn.cx(),
            "_gosling/unstable/session/handoff",
            serde_json::json!({ "sessionId": session_id }),
        )
        .await
        .unwrap();

        let new_session_id = response
            .get("sessionId")
            .and_then(|v| v.as_str())
            .expect("missing sessionId")
            .to_string();
        assert_ne!(new_session_id, session_id);
        let snapshot = response.get("snapshot").expect("missing snapshot");
        assert_eq!(
            snapshot
                .get("latestUserIntent")
                .and_then(|value| value.get("content"))
                .and_then(|value| value.as_str()),
            Some("let's do the thing")
        );
        assert_eq!(
            snapshot.get("status").and_then(|value| value.as_str()),
            Some("active")
        );
        assert!(response
            .get("continuationPrompt")
            .and_then(|value| value.as_str())
            .is_some_and(|prompt| prompt.contains("saved session checkpoint")));

        let new_session = session_manager
            .get_session(&new_session_id, true)
            .await
            .unwrap();
        let handoff_messages = new_session.conversation.unwrap();
        assert_eq!(handoff_messages.messages().len(), 1);
        assert!(!handoff_messages.messages()[0].metadata.user_visible);
        assert!(handoff_messages.messages()[0].metadata.agent_visible);

        // The original session's own conversation is untouched.
        let original = session_manager
            .get_session(&session_id, true)
            .await
            .unwrap();
        assert_eq!(original.conversation.unwrap().messages().len(), 2);
    });
}

#[test]
fn handoff_session_rejects_an_empty_session_id() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let conn = new_connection(data_root.path()).await;

        let error = send_custom(
            conn.cx(),
            "_gosling/unstable/session/handoff",
            serde_json::json!({ "sessionId": "" }),
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("sessionId"));
    });
}

#[test]
fn checkpoint_preview_is_non_mutating_and_stale_transition_is_rejected() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let mut conn = new_connection(data_root.path()).await;
        let manager = SessionManager::new(data_root.path().to_path_buf());
        let session_data = conn.new_session().await.unwrap();
        let session_id = session_data.session.session_id().0.to_string();
        manager
            .add_message(
                &session_id,
                &Message::user().with_text("preserve this objective across the switch"),
            )
            .await
            .unwrap();

        let preview = send_custom(
            conn.cx(),
            "_gosling/unstable/session/handoff/checkpoint/preview",
            serde_json::json!({
                "sessionId": session_id,
                "targetProvider": "openai",
                "targetModel": TEST_MODEL,
            }),
        )
        .await
        .unwrap();
        assert_eq!(
            preview
                .get("expectedCurrentGeneration")
                .and_then(serde_json::Value::as_u64),
            Some(0)
        );
        assert_eq!(
            preview
                .pointer("/snapshot/generation")
                .and_then(serde_json::Value::as_u64),
            Some(1)
        );
        assert!(preview.get("queuedAfterRunId").is_none());
        assert_eq!(
            preview
                .pointer("/toolContinuity/authorizationMode")
                .and_then(serde_json::Value::as_str),
            Some("smart_approve")
        );
        assert!(preview
            .pointer("/toolContinuity/stateHash")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|hash| hash.len() == 64));
        assert!(preview
            .pointer("/toolContinuity/enabledExtensionNames")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|extensions| extensions
                .iter()
                .any(|name| name.as_str() == Some("extensionmanager"))));
        assert!(manager
            .latest_handoff_snapshot(&session_id)
            .await
            .unwrap()
            .is_none());

        let preview_source_hash = preview
            .pointer("/snapshot/coverage/sourceHash")
            .and_then(serde_json::Value::as_str)
            .unwrap()
            .to_string();
        manager
            .add_message(
                &session_id,
                &Message::user().with_text("message added after the preview"),
            )
            .await
            .unwrap();
        let changed_session_error = send_custom(
            conn.cx(),
            "_gosling/unstable/session/provider/transition",
            serde_json::json!({
                "sessionId": session_id,
                "targetProvider": "openai",
                "targetModel": TEST_MODEL,
                "expectedCurrentGeneration": 0,
                "expectedSourceHash": preview_source_hash,
                "confirmNewContext": false,
            }),
        )
        .await
        .unwrap_err();
        assert!(changed_session_error
            .to_string()
            .contains("session changed after the handoff checkpoint was previewed"));

        let refreshed_preview = send_custom(
            conn.cx(),
            "_gosling/unstable/session/handoff/checkpoint/preview",
            serde_json::json!({
                "sessionId": session_id,
                "targetProvider": "openai",
                "targetModel": TEST_MODEL,
            }),
        )
        .await
        .unwrap();
        let refreshed_source_hash = refreshed_preview
            .pointer("/snapshot/coverage/sourceHash")
            .and_then(serde_json::Value::as_str)
            .unwrap();

        let transition = serde_json::json!({
            "sessionId": session_id,
            "targetProvider": "openai",
            "targetModel": TEST_MODEL,
            "expectedCurrentGeneration": 0,
            "expectedSourceHash": refreshed_source_hash,
            "confirmNewContext": false,
        });
        let activated = send_custom(
            conn.cx(),
            "_gosling/unstable/session/provider/transition",
            transition.clone(),
        )
        .await
        .unwrap();
        assert_eq!(
            activated
                .pointer("/snapshot/status")
                .and_then(serde_json::Value::as_str),
            Some("active")
        );
        let committed = manager.get_session(&session_id, true).await.unwrap();
        let committed_messages = committed.conversation.unwrap();
        let original = committed_messages
            .messages()
            .iter()
            .find(|message| message.as_concat_text() == "preserve this objective across the switch")
            .unwrap();
        assert!(original.metadata.user_visible);
        assert!(!original.metadata.agent_visible);
        assert_eq!(
            committed_messages
                .messages()
                .iter()
                .filter(|message| message
                    .id
                    .as_deref()
                    .is_some_and(|id| id.starts_with("handoff_snapshot_")))
                .count(),
            1
        );

        let error = send_custom(
            conn.cx(),
            "_gosling/unstable/session/provider/transition",
            transition,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("stale handoff generation"));
        let latest = manager
            .latest_handoff_snapshot(&session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(latest.generation, 1);
    });
}

#[test]
fn transition_rejects_tool_or_authorization_changes_after_preview() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let openai = OpenAiFixture::new(
            Vec::new(),
            <AcpServerConnection as Connection>::expected_session_id(),
        )
        .await;
        let mut conn = <AcpServerConnection as Connection>::new(
            TestConnectionConfig {
                data_root: data_root.path().to_path_buf(),
                builtins: vec!["developer".to_string()],
                ..Default::default()
            },
            openai,
        )
        .await;
        let manager = SessionManager::new(data_root.path().to_path_buf());
        let session_data = conn.new_session().await.unwrap();
        let session_id = session_data.session.session_id().0.to_string();
        let preview = send_custom(
            conn.cx(),
            "_gosling/unstable/session/handoff/checkpoint/preview",
            serde_json::json!({
                "sessionId": session_id,
                "targetProvider": "openai",
                "targetModel": TEST_MODEL,
            }),
        )
        .await
        .unwrap();
        let tool_state_hash = preview
            .pointer("/toolContinuity/stateHash")
            .and_then(serde_json::Value::as_str)
            .unwrap();
        PermissionManager::for_config_dir(data_root.path().to_path_buf())
            .update_user_permission("shell", PermissionLevel::AlwaysAllow)
            .unwrap();

        let error = send_custom(
            conn.cx(),
            "_gosling/unstable/session/provider/transition",
            serde_json::json!({
                "sessionId": session_id,
                "targetProvider": "openai",
                "targetModel": TEST_MODEL,
                "expectedCurrentGeneration": 0,
                "expectedToolStateHash": tool_state_hash,
                "confirmNewContext": false,
            }),
        )
        .await
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("tools or authorization mode changed"));
        assert!(manager
            .latest_handoff_snapshot(&session_id)
            .await
            .unwrap()
            .is_none());
    });
}

#[test]
fn transition_queues_behind_the_previewed_active_turn_and_includes_its_output() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let mut conn = new_connection_with_exchanges(
            data_root.path(),
            vec![(
                "finish before switching".to_string(),
                include_str!("acp_test_data/openai_basic.txt"),
            )],
        )
        .await;
        let session_data = conn.new_session().await.unwrap();
        let session_id = session_data.session.session_id().0.to_string();
        let mut active_session = session_data.session;
        let active_prompt = tokio::spawn(async move {
            active_session
                .prompt("finish before switching", PermissionDecision::Cancel)
                .await
        });

        let preview = tokio::time::timeout(std::time::Duration::from_secs(2), async {
            loop {
                let preview = send_custom(
                    conn.cx(),
                    "_gosling/unstable/session/handoff/checkpoint/preview",
                    serde_json::json!({
                        "sessionId": session_id,
                        "targetProvider": "openai",
                        "targetModel": TEST_MODEL,
                    }),
                )
                .await
                .unwrap();
                if preview.get("queuedAfterRunId").is_some() {
                    break preview;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the active run should become visible to the preview");
        let active_run_id = preview
            .get("queuedAfterRunId")
            .and_then(serde_json::Value::as_str)
            .unwrap();
        let tool_state_hash = preview
            .pointer("/toolContinuity/stateHash")
            .and_then(serde_json::Value::as_str)
            .unwrap();

        let transition = send_custom(
            conn.cx(),
            "_gosling/unstable/session/provider/transition",
            serde_json::json!({
                "sessionId": session_id,
                "targetProvider": "openai",
                "targetModel": TEST_MODEL,
                "expectedCurrentGeneration": 0,
                "expectedActiveRunId": active_run_id,
                "expectedToolStateHash": tool_state_hash,
                "confirmNewContext": false,
            }),
        )
        .await
        .unwrap();
        let output = active_prompt.await.unwrap().unwrap();

        assert_eq!(output.text, "2");
        assert_eq!(
            transition
                .pointer("/snapshot/coverage/totalMessageCount")
                .and_then(serde_json::Value::as_u64),
            Some(2)
        );
        assert_eq!(
            transition
                .pointer("/snapshot/latestUserIntent/content")
                .and_then(serde_json::Value::as_str),
            Some("finish before switching")
        );
    });
}

#[test]
fn transition_commit_failure_preserves_stored_and_live_provider() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let exchanges = vec![(
            "still works".to_string(),
            include_str!("acp_test_data/openai_basic.txt"),
        )];
        let mut conn = new_connection_with_exchanges(data_root.path(), exchanges).await;
        let manager = SessionManager::new(data_root.path().to_path_buf());
        let mut session_data = conn.new_session().await.unwrap();
        let session_id = session_data.session.session_id().0.to_string();
        let before = manager.get_session(&session_id, false).await.unwrap();

        let database_url = format!(
            "sqlite://{}",
            data_root.path().join("sessions/sessions.db").display()
        );
        let pool = sqlx::SqlitePool::connect(&database_url).await.unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_handoff_commit BEFORE UPDATE ON sessions \
             BEGIN SELECT RAISE(FAIL, 'injected transition commit failure'); END",
        )
        .execute(&pool)
        .await
        .unwrap();

        let error = send_custom(
            conn.cx(),
            "_gosling/unstable/session/provider/transition",
            serde_json::json!({
                "sessionId": session_id,
                "targetProvider": "openai",
                "targetModel": TEST_MODEL,
                "expectedCurrentGeneration": 0,
                "confirmNewContext": false,
            }),
        )
        .await
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("injected transition commit failure"));

        let after = manager.get_session(&session_id, false).await.unwrap();
        assert_eq!(after.provider_name, before.provider_name);
        assert_eq!(
            serde_json::to_value(&after.model_config).unwrap(),
            serde_json::to_value(&before.model_config).unwrap()
        );
        let latest = manager
            .latest_handoff_snapshot(&session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            latest.status,
            gosling::session_handoff::SessionHandoffStatusDto::Failed
        );

        sqlx::query("DROP TRIGGER fail_handoff_commit")
            .execute(&pool)
            .await
            .unwrap();
        let output = session_data
            .session
            .prompt("still works", PermissionDecision::Cancel)
            .await
            .unwrap();
        assert_eq!(output.text, "2");
    });
}
