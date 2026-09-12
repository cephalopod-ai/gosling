use gosling::config::GoslingMode;
use gosling::conversation::message::Message;
use gosling::session::{SessionManager, SessionType};
use serde_json::{json, Value};
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

#[tokio::test]
async fn session_scoped_mcp_bridge_searches_and_reads_prior_messages() {
    let root = TempDir::new().unwrap();
    let data_dir = root.path().join("data");
    let manager = SessionManager::new(data_dir.clone());
    let session = manager
        .create_session(
            root.path().to_path_buf(),
            "history bridge".to_string(),
            SessionType::User,
            GoslingMode::Auto,
        )
        .await
        .unwrap();
    manager
        .add_message(
            &session.id,
            &Message::user().with_id("prior-message").with_text(format!(
                "{}bridge-search-marker",
                "older context ".repeat(80)
            )),
        )
        .await
        .unwrap();
    manager
        .add_message(
            &session.id,
            &Message::user()
                .with_id("current-request")
                .with_text("find bridge-search-marker"),
        )
        .await
        .unwrap();

    let mut child = Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args([
            "session-history-mcp",
            "--session-id",
            &session.id,
            "--data-dir",
            data_dir.to_str().unwrap(),
        ])
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let requests = [
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "bridge-test", "version": "1"}
            }
        }),
        json!({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}),
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "tools/call",
            "params": {
                "name": "session_search",
                "arguments": {"query": "bridge-search-marker"}
            }
        }),
        json!({
            "jsonrpc": "2.0",
            "id": 4,
            "method": "tools/call",
            "params": {
                "name": "session_read",
                "arguments": {"message_id": "prior-message"}
            }
        }),
    ];
    for request in requests {
        writeln!(stdin, "{}", request).unwrap();
    }
    drop(stdin);

    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .collect::<Vec<_>>();
    let tools = &responses
        .iter()
        .find(|response| response["id"] == 2)
        .unwrap()["result"]["tools"];
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "session_search"));
    assert!(tools
        .as_array()
        .unwrap()
        .iter()
        .any(|tool| tool["name"] == "session_read"));

    let search = responses
        .iter()
        .find(|response| response["id"] == 3)
        .unwrap()["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(search.contains("prior-message"));
    assert!(search.contains("bridge-search-marker"));
    assert!(!search.contains("current-request"));

    let read = responses
        .iter()
        .find(|response| response["id"] == 4)
        .unwrap()["result"]["content"][0]["text"]
        .as_str()
        .unwrap();
    assert!(read.contains("prior-message"));
    assert!(read.contains("bridge-search-marker"));
}
