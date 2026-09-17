#[allow(dead_code)]
#[path = "acp_common_tests/mod.rs"]
mod common_tests;

use agent_client_protocol::schema::v1::NewSessionRequest;
use common_tests::fixtures::server::AcpServerConnection;
use common_tests::fixtures::{
    run_test, send_custom, Connection, OpenAiFixture, TestConnectionConfig,
};
use gosling::config::GoslingMode;
use gosling::session::{
    NewSessionLibraryContent, SessionLibraryScope, SessionManager, SessionType,
};
use std::path::Path;
use std::sync::Arc;

const COMPILE: &str = "_gosling/unstable/shell/session/library/compile";
const RESOLVE: &str = "_gosling/unstable/shell/session/library/resolve";

async fn active_session(
    root: &Path,
    cwd: &Path,
) -> (AcpServerConnection, Arc<SessionManager>, String) {
    let manager = Arc::new(SessionManager::new(root.to_path_buf()));
    let fixture = OpenAiFixture::new(
        vec![],
        <AcpServerConnection as Connection>::expected_session_id(),
    )
    .await;
    let conn = AcpServerConnection::new(
        TestConnectionConfig {
            data_root: root.to_path_buf(),
            session_manager: Some(manager.clone()),
            ..Default::default()
        },
        fixture,
    )
    .await;
    let session = conn
        .cx()
        .send_request(NewSessionRequest::new(cwd))
        .block_task()
        .await
        .unwrap();
    (conn, manager, session.session_id.0.to_string())
}

async fn add_text(manager: &SessionManager, session_id: &str, name: &str, text: &str) -> String {
    manager
        .add_session_library_item(
            session_id,
            SessionLibraryScope::Session,
            name.into(),
            NewSessionLibraryContent::Text(text.into()),
        )
        .await
        .unwrap()
        .id
}

#[test]
fn compiles_all_nineteen_sources_verbatim_with_citations_and_private_unique_outputs() {
    run_test(async {
        let root = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let (conn, manager, session_id) = active_session(root.path(), cwd.path()).await;
        let mut ids = Vec::new();
        let mut sources = Vec::new();
        for index in 0..19 {
            let text = format!("  Source {index}: Résumé 😀\n```md\n{}\n```\n[Citation](https://example.com/{index})\nsource_{index}_end  ", "evidence ".repeat(6000));
            ids.push(add_text(&manager, &session_id, &format!("Source {index}"), &text).await);
            sources.push(text);
        }
        let response = send_custom(
            conn.cx(),
            COMPILE,
            serde_json::json!({ "sessionId": session_id, "itemIds": ids }),
        )
        .await
        .unwrap();
        assert_eq!(response["sourceCount"], 19);
        let path = Path::new(response["filePath"].as_str().unwrap());
        assert_eq!(path.parent().unwrap(), cwd.path().canonicalize().unwrap());
        let document = std::fs::read_to_string(path).unwrap();
        assert!(document.len() > 512 * 1024);
        assert_eq!(
            response["sizeBytes"].as_u64().unwrap(),
            document.len() as u64
        );
        for source in &sources {
            assert_eq!(document.matches(source.as_str()).count(), 1);
        }
        assert!(document.contains("S019"));
        assert!(document.contains("SHA-256"));
        assert!(response["promptText"]
            .as_str()
            .unwrap()
            .contains("NOT been read yet"));
        assert!(!response["promptText"]
            .as_str()
            .unwrap()
            .contains("source_18_end"));
        let artifacts = manager
            .list_session_artifacts(&session_id, None, 100)
            .await
            .unwrap();
        assert!(artifacts
            .artifacts
            .iter()
            .any(|item| item.resolved_path == path.to_string_lossy()));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let second = send_custom(
            conn.cx(),
            COMPILE,
            serde_json::json!({ "sessionId": session_id, "itemIds": ids }),
        )
        .await
        .unwrap();
        assert_ne!(second["filePath"], response["filePath"]);
        assert_eq!(std::fs::read_to_string(path).unwrap(), document);
        assert_eq!(
            manager
                .list_session_library_items(&session_id)
                .await
                .unwrap()
                .len(),
            19
        );
    });
}

#[test]
fn linked_text_over_inline_limit_is_never_silently_truncated() {
    run_test(async {
        let root = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let (conn, manager, session_id) = active_session(root.path(), cwd.path()).await;
        let text = format!("{}\nEND_OF_COMPLETE_SOURCE", "é".repeat(300_000));
        let path = cwd.path().join("source.txt");
        std::fs::write(&path, &text).unwrap();
        let item = manager
            .add_session_library_item(
                &session_id,
                SessionLibraryScope::Session,
                "Large source".into(),
                NewSessionLibraryContent::File {
                    path: path.to_string_lossy().into_owned(),
                    mime_type: "text/plain".into(),
                },
            )
            .await
            .unwrap();
        let request = serde_json::json!({ "sessionId": session_id, "itemIds": [item.id] });
        let error = send_custom(conn.cx(), RESOLVE, request.clone())
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("SHELL_LIBRARY_SELECTION_TOO_LARGE"));
        let result = send_custom(conn.cx(), COMPILE, request).await.unwrap();
        let compiled = std::fs::read_to_string(result["filePath"].as_str().unwrap()).unwrap();
        assert!(compiled.contains(&text));
        assert!(!compiled.contains("[Content truncated]"));
    });
}

#[test]
fn rejects_foreign_missing_duplicate_and_planning_inputs_without_publishing() {
    run_test(async {
        let root = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let (conn, manager, session_id) = active_session(root.path(), cwd.path()).await;
        let own = add_text(&manager, &session_id, "Own", "owned source").await;
        let other = manager
            .create_session(
                cwd.path().into(),
                "Other".into(),
                SessionType::Acp,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        let foreign = add_text(&manager, &other.id, "Private", "must not leak").await;
        for ids in [
            vec![foreign],
            vec!["missing".into()],
            vec![own.clone(), own.clone()],
            vec![],
        ] {
            assert!(send_custom(
                conn.cx(),
                COMPILE,
                serde_json::json!({ "sessionId": session_id, "itemIds": ids })
            )
            .await
            .is_err());
        }
        assert_eq!(std::fs::read_dir(cwd.path()).unwrap().count(), 0);
        send_custom(
            conn.cx(),
            "_gosling/unstable/session/plan/start",
            serde_json::json!({ "sessionId": session_id }),
        )
        .await
        .unwrap();
        let error = send_custom(
            conn.cx(),
            COMPILE,
            serde_json::json!({ "sessionId": session_id, "itemIds": [own] }),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("planning_capability_denied"));
        assert_eq!(std::fs::read_dir(cwd.path()).unwrap().count(), 0);
    });
}

#[test]
fn failed_linked_source_and_oversized_compilation_leave_no_partial_output() {
    run_test(async {
        let root = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let (conn, manager, session_id) = active_session(root.path(), cwd.path()).await;
        let first = add_text(&manager, &session_id, "First", "complete first source").await;
        let path = cwd.path().join("missing.txt");
        std::fs::write(&path, "will disappear").unwrap();
        let missing = manager
            .add_session_library_item(
                &session_id,
                SessionLibraryScope::Session,
                "Missing".into(),
                NewSessionLibraryContent::File {
                    path: path.to_string_lossy().into_owned(),
                    mime_type: "text/plain".into(),
                },
            )
            .await
            .unwrap();
        std::fs::remove_file(path).unwrap();
        assert!(send_custom(
            conn.cx(),
            COMPILE,
            serde_json::json!({ "sessionId": session_id, "itemIds": [first, missing.id] })
        )
        .await
        .is_err());
        assert_eq!(std::fs::read_dir(cwd.path()).unwrap().count(), 0);
        let mut ids = Vec::new();
        for index in 0..2 {
            let path = cwd.path().join(format!("large-{index}.txt"));
            std::fs::write(&path, "x".repeat(17 * 1024 * 1024)).unwrap();
            ids.push(
                manager
                    .add_session_library_item(
                        &session_id,
                        SessionLibraryScope::Session,
                        format!("Large {index}"),
                        NewSessionLibraryContent::File {
                            path: path.to_string_lossy().into_owned(),
                            mime_type: "text/plain".into(),
                        },
                    )
                    .await
                    .unwrap()
                    .id,
            );
        }
        let error = send_custom(
            conn.cx(),
            COMPILE,
            serde_json::json!({ "sessionId": session_id, "itemIds": ids }),
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains("32 MiB"));
        assert_eq!(std::fs::read_dir(cwd.path()).unwrap().count(), 2);
    });
}
