//! `session list` shows every session a person had, including ones created
//! over `gosling acp` / `gosling serve`, and keeps internal sessions hidden
//! (GSL-PT-20260927-S11).

use gosling::config::GoslingMode;
use gosling::conversation::message::Message;
use gosling::session::{SessionManager, SessionType};
use std::path::PathBuf;
use std::process::{Command, Output};
use tempfile::TempDir;

fn gosling(root: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(args)
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run gosling binary")
}

async fn seed_sessions(root: &TempDir) {
    let sessions = SessionManager::new(root.path().join("data"));
    for (name, session_type) in [
        ("cli chat", SessionType::User),
        ("scheduled job", SessionType::Scheduled),
        ("editor chat", SessionType::Acp),
        ("subagent run", SessionType::SubAgent),
        ("hidden helper", SessionType::Hidden),
        ("terminal shell", SessionType::Terminal),
    ] {
        let session = sessions
            .create_session(
                PathBuf::from("/tmp/session-list-types"),
                name.to_string(),
                session_type,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        sessions
            .add_message(&session.id, &Message::user().with_text("hello"))
            .await
            .unwrap();
    }
}

fn listed(root: &TempDir) -> Vec<(String, String)> {
    let output = gosling(root, &["session", "list", "--format", "json"]);
    assert!(
        output.status.success(),
        "session list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let mut listed: Vec<(String, String)> = sessions
        .iter()
        .map(|session| {
            (
                session["name"].as_str().unwrap().to_string(),
                session["session_type"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    listed.sort();
    listed
}

#[tokio::test]
async fn session_list_shows_acp_sessions_and_hides_internal_ones() {
    let root = TempDir::new().unwrap();
    seed_sessions(&root).await;

    assert_eq!(
        listed(&root),
        vec![
            ("cli chat".to_string(), "user".to_string()),
            ("editor chat".to_string(), "acp".to_string()),
            ("scheduled job".to_string(), "scheduled".to_string()),
        ]
    );

    let text = gosling(&root, &["session", "list"]);
    let stdout = String::from_utf8_lossy(&text.stdout);
    assert!(stdout.contains("editor chat"), "{stdout}");
    assert!(stdout.contains("cli chat"), "{stdout}");
    for hidden in ["subagent run", "hidden helper", "terminal shell"] {
        assert!(!stdout.contains(hidden), "{hidden} leaked: {stdout}");
    }
}

#[tokio::test]
async fn session_remove_regex_covers_acp_but_not_internal_sessions() {
    let root = TempDir::new().unwrap();
    seed_sessions(&root).await;

    let output = gosling(&root, &["session", "remove", "--regex", ".*", "-y"]);
    assert!(
        output.status.success(),
        "session remove failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(listed(&root).is_empty());

    let remaining: Vec<SessionType> = SessionManager::new(root.path().join("data"))
        .list_all_sessions()
        .await
        .unwrap()
        .into_iter()
        .map(|session| session.session_type)
        .collect();
    assert_eq!(remaining.len(), 3, "internal sessions must not be removed");
    assert!(remaining.iter().all(|t| matches!(
        t,
        SessionType::SubAgent | SessionType::Hidden | SessionType::Terminal
    )));
}
