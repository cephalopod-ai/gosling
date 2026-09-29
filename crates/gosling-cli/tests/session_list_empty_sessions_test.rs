//! `session list` applies the rule ACP `session/list` uses: a session that never
//! recorded a message is not listed, and any session with a message is
//! (GSL-PT-20260927-S16).

use gosling::config::GoslingMode;
use gosling::conversation::message::Message;
use gosling::session::{SessionManager, SessionType};
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;

fn gosling(root: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(args)
        .env("HOME", root.path())
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run gosling binary")
}

async fn seed(root: &TempDir) {
    let sessions = SessionManager::new(root.path().join("data"));
    for (name, message) in [
        ("never used", None),
        ("chatted", Some(Message::user().with_text("hello"))),
        (
            "agent-only history",
            Some(Message::user().with_text("context").agent_only()),
        ),
    ] {
        let session = sessions
            .create_session(
                PathBuf::from("/tmp/session-list-empty"),
                name.to_string(),
                SessionType::User,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        if let Some(message) = message {
            sessions.add_message(&session.id, &message).await.unwrap();
        }
    }
}

fn listed_names(root: &TempDir) -> Vec<String> {
    let output = gosling(root, &["session", "list", "--format", "json"]);
    assert!(
        output.status.success(),
        "session list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let mut names: Vec<String> = sessions
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn session_list_hides_sessions_without_messages_like_acp() {
    let root = TempDir::new().unwrap();
    seed(&root).await;

    assert_eq!(listed_names(&root), vec!["agent-only history", "chatted"]);

    let text = gosling(&root, &["session", "list"]);
    let stdout = String::from_utf8_lossy(&text.stdout);
    assert!(stdout.contains("chatted"), "{stdout}");
    assert!(!stdout.contains("never used"), "{stdout}");
}

#[tokio::test]
async fn an_unlisted_empty_session_can_still_be_removed_by_name() {
    let root = TempDir::new().unwrap();
    seed(&root).await;

    let output = gosling(&root, &["session", "remove", "--name", "never used", "-y"]);
    assert!(
        output.status.success(),
        "remove failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let remaining: Vec<String> = SessionManager::new(root.path().join("data"))
        .list_all_sessions()
        .await
        .unwrap()
        .into_iter()
        .map(|session| session.name)
        .collect();
    assert!(!remaining.contains(&"never used".to_string()));
    assert_eq!(remaining.len(), 2);
}
