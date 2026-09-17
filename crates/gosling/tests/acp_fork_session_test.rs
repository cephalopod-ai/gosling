#[allow(dead_code)]
#[path = "acp_common_tests/mod.rs"]
mod common_tests;

use agent_client_protocol::schema::v1::{
    ForkSessionRequest, ForkSessionResponse, McpServer, McpServerSse, SessionId,
};
use common_tests::fixtures::server::AcpServerConnection;
use common_tests::fixtures::{
    run_test, send_custom, Connection, OpenAiFixture, TestConnectionConfig,
};
use gosling::config::GoslingMode;
use gosling::conversation::message::{Message, MessageContent};
use gosling::session::library::{NewSessionLibraryContent, SessionLibraryScope};
use gosling::session::{SessionManager, SessionType};
use std::path::Path;

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

async fn fork_session_request(
    conn: &AcpServerConnection,
    request: ForkSessionRequest,
) -> anyhow::Result<ForkSessionResponse> {
    conn.cx()
        .send_request(request)
        .block_task()
        .await
        .map_err(Into::into)
}

async fn seed_session_with_messages(
    session_manager: &SessionManager,
    cwd: &Path,
    messages: &[(&str, i64)],
) -> gosling::session::Session {
    let session = session_manager
        .create_session(
            cwd.to_path_buf(),
            "Fork before".to_string(),
            SessionType::Acp,
            GoslingMode::default(),
        )
        .await
        .unwrap();

    for (text, created) in messages {
        let mut message = Message::user().with_text(*text);
        message.created = *created;
        session_manager
            .add_message(&session.id, &message)
            .await
            .unwrap();
    }

    session
}

async fn session_texts(session_manager: &SessionManager, session_id: &str) -> Vec<String> {
    session_manager
        .get_session(session_id, true)
        .await
        .unwrap()
        .conversation
        .unwrap()
        .messages()
        .iter()
        .flat_map(|message| {
            message.content.iter().filter_map(|content| match content {
                MessageContent::Text(text) => Some(text.text.clone()),
                _ => None,
            })
        })
        .collect()
}

fn conversation_before_meta(timestamp: i64) -> serde_json::Map<String, serde_json::Value> {
    let mut meta = serde_json::Map::new();
    meta.insert(
        "conversationBefore".to_string(),
        serde_json::Value::Number(timestamp.into()),
    );
    meta
}

#[test]
fn fork_session_conversation_before_matches_rest_cutoff() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let session_manager = SessionManager::new(data_root.path().to_path_buf());
        let session = seed_session_with_messages(
            &session_manager,
            cwd.path(),
            &[
                ("first", 1_718_000_000),
                ("second", 1_718_000_060),
                ("third", 1_718_000_120),
            ],
        )
        .await;
        let conn = new_connection(data_root.path()).await;

        let response = fork_session_request(
            &conn,
            ForkSessionRequest::new(SessionId::new(session.id.clone()), cwd.path())
                .meta(conversation_before_meta(1_718_000_120)),
        )
        .await
        .unwrap();

        assert_eq!(
            session_texts(&session_manager, response.session_id.0.as_ref()).await,
            vec!["first", "second"]
        );
        assert_eq!(
            session_texts(&session_manager, &session.id).await,
            vec!["first", "second", "third"]
        );
        let branch = session_manager
            .get_session(response.session_id.0.as_ref(), false)
            .await
            .unwrap();
        assert_eq!(branch.name, "branch: Fork before");
        assert!(branch.user_set_name);
        session_manager
            .update(&branch.id)
            .system_generated_name("Automatic title")
            .apply()
            .await
            .unwrap();
        assert_eq!(
            session_manager
                .get_session(&branch.id, false)
                .await
                .unwrap()
                .name,
            "branch: Fork before"
        );
    });
}

#[test]
fn fork_session_inherits_independent_inputs_and_original_file_pointers() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let manager = SessionManager::new(data_root.path().to_path_buf());
        let source = seed_session_with_messages(&manager, cwd.path(), &[("first", 100)]).await;
        let file = cwd.path().join("reference.txt");
        std::fs::write(&file, "original reference").unwrap();
        let contents = [
            ("File", NewSessionLibraryContent::File {
                path: file.to_str().unwrap().to_string(),
                mime_type: "text/plain".to_string(),
            }),
            ("Notes", NewSessionLibraryContent::Text("pasted notes".to_string())),
            ("Image", NewSessionLibraryContent::Image {
                data: "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aD1sAAAAASUVORK5CYII=".to_string(),
                mime_type: "image/png".to_string(),
            }),
        ];
        let mut originals = Vec::new();
        for (name, content) in contents {
            originals.push(
                manager
                    .add_session_library_item(
                        &source.id,
                        SessionLibraryScope::Session,
                        name.to_string(),
                        content,
                    )
                    .await
                    .unwrap(),
            );
        }
        let shared = manager
            .add_session_library_item(
                &source.id,
                SessionLibraryScope::Project,
                "Shared".to_string(),
                NewSessionLibraryContent::Text("project reference".to_string()),
            )
            .await
            .unwrap();
        let unrelated = seed_session_with_messages(&manager, cwd.path(), &[]).await;
        let conn = new_connection(data_root.path()).await;
        let response = fork_session_request(
            &conn,
            ForkSessionRequest::new(SessionId::new(source.id.clone()), cwd.path())
                .meta(conversation_before_meta(100)),
        )
        .await
        .unwrap();
        let branch_id = response.session_id.0.to_string();
        let inputs = manager
            .list_session_library_items(&branch_id)
            .await
            .unwrap();
        assert_eq!(inputs.len(), 4);
        assert!(inputs.contains(&shared));
        for original in &originals {
            let inherited = inputs
                .iter()
                .find(|item| item.name == original.name)
                .unwrap();
            assert_ne!(inherited.id, original.id);
            let mut expected = original.clone();
            expected.id = inherited.id.clone();
            assert_eq!(inherited, &expected);
            assert_eq!(
                manager
                    .get_session_library_items(&branch_id, std::slice::from_ref(&inherited.id))
                    .await
                    .unwrap(),
                vec![inherited.clone()]
            );
            assert!(manager
                .get_session_library_items(&branch_id, std::slice::from_ref(&original.id))
                .await
                .is_err());
            assert!(manager
                .get_session_library_items(&unrelated.id, std::slice::from_ref(&inherited.id))
                .await
                .is_err());
        }
        assert_eq!(
            manager
                .list_session_library_items(&unrelated.id)
                .await
                .unwrap(),
            vec![shared]
        );

        let linked = inputs.iter().find(|item| item.name == "File").unwrap();
        std::fs::write(&file, "updated reference").unwrap();
        let resolved = send_custom(
            conn.cx(),
            "_gosling/unstable/shell/session/library/resolve",
            serde_json::json!({ "sessionId": branch_id, "itemIds": [linked.id] }),
        )
        .await
        .unwrap();
        assert!(resolved["items"][0]["content"]["text"]
            .as_str()
            .unwrap()
            .contains("updated reference"));

        let nested_response = fork_session_request(
            &conn,
            ForkSessionRequest::new(SessionId::new(branch_id.clone()), cwd.path()),
        )
        .await
        .unwrap();
        let nested_id = nested_response.session_id.0.to_string();
        assert_eq!(
            manager.get_session(&nested_id, false).await.unwrap().name,
            "branch: Fork before"
        );
        assert!(manager
            .remove_session_library_item(&branch_id, &linked.id)
            .await
            .unwrap());
        manager.delete_session(&source.id).await.unwrap();
        assert_eq!(
            manager
                .list_session_library_items(&branch_id)
                .await
                .unwrap()
                .len(),
            3
        );
        let nested_inputs = manager
            .list_session_library_items(&nested_id)
            .await
            .unwrap();
        assert_eq!(nested_inputs.len(), 4);
        assert_eq!(
            nested_inputs
                .iter()
                .find(|item| item.name == "File")
                .unwrap()
                .file_path,
            linked.file_path
        );
        let reopened = SessionManager::new(data_root.path().to_path_buf());
        assert_eq!(
            reopened
                .list_session_library_items(&nested_id)
                .await
                .unwrap(),
            nested_inputs
        );
    });
}

#[test]
fn fork_session_does_not_duplicate_an_empty_branch_label() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let manager = SessionManager::new(data_root.path().to_path_buf());
        let source = seed_session_with_messages(&manager, cwd.path(), &[]).await;
        manager
            .update(&source.id)
            .system_generated_name("branch:")
            .apply()
            .await
            .unwrap();
        let conn = new_connection(data_root.path()).await;
        let response = fork_session_request(
            &conn,
            ForkSessionRequest::new(SessionId::new(source.id), cwd.path()),
        )
        .await
        .unwrap();
        let branch = manager
            .get_session(response.session_id.0.as_ref(), false)
            .await
            .unwrap();
        assert_eq!(branch.name, "branch:");
        assert!(branch.user_set_name);
    });
}

#[test]
fn fork_session_failure_does_not_leave_a_copied_session() {
    run_test(async {
        let data_root = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let session_manager = SessionManager::new(data_root.path().to_path_buf());
        let session = seed_session_with_messages(&session_manager, cwd.path(), &[]).await;
        let input = session_manager
            .add_session_library_item(
                &session.id,
                SessionLibraryScope::Session,
                "Retained input".to_string(),
                NewSessionLibraryContent::Text("source input".to_string()),
            )
            .await
            .unwrap();
        let conn = new_connection(data_root.path()).await;
        let before = session_manager
            .list_sessions_by_types(&[SessionType::Acp])
            .await
            .unwrap()
            .len();

        let error = fork_session_request(
            &conn,
            ForkSessionRequest::new(SessionId::new(session.id.clone()), cwd.path()).mcp_servers(
                vec![McpServer::Sse(McpServerSse::new(
                    "legacy-sse",
                    "https://example.com/sse",
                ))],
            ),
        )
        .await
        .unwrap_err();

        assert!(error.to_string().contains("SSE is unsupported"));
        assert_eq!(
            session_manager
                .list_sessions_by_types(&[SessionType::Acp])
                .await
                .unwrap()
                .len(),
            before
        );
        assert_eq!(
            session_manager
                .list_session_library_items(&session.id)
                .await
                .unwrap(),
            vec![input]
        );
    });
}
