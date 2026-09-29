//! `session remove -r` matches session IDs, not names, and says so (GSL-PT-20260927-D12).

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

fn import_named(root: &TempDir, label: &str) {
    let meta = serde_json::json!({
        "timestamp": "2026-05-22T13:37:22Z",
        "type": "session_meta",
        "payload": {"id": label, "cwd": "/w"}
    });
    let message = serde_json::json!({
        "timestamp": "2026-05-22T13:37:23Z",
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": format!("project-{label}")}]
        }
    });
    let path = root.path().join(format!("{label}.jsonl"));
    std::fs::write(&path, format!("{meta}\n{message}\n")).unwrap();
    let working_dir = root.path().to_str().unwrap();
    let output = gosling(
        root,
        &[
            "session",
            "import",
            path.to_str().unwrap(),
            "--working-dir",
            working_dir,
        ],
    );
    assert!(
        output.status.success(),
        "import failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn listed(root: &TempDir) -> Vec<(String, String)> {
    let output = gosling(root, &["session", "list", "--format", "json"]);
    assert!(output.status.success());
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let mut listed: Vec<(String, String)> = sessions
        .iter()
        .map(|s| {
            (
                s["id"].as_str().unwrap().to_string(),
                s["name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    listed.sort();
    listed
}

#[test]
fn regex_matches_ids_and_says_names_are_not_matched() {
    let root = TempDir::new().unwrap();
    import_named(&root, "alpha");
    import_named(&root, "beta");
    let before = listed(&root);
    assert_eq!(before.len(), 2);
    assert!(before.iter().all(|(_, name)| name.starts_with("project-")));

    let by_name = gosling(&root, &["session", "remove", "-r", "^project-", "-y"]);
    assert!(by_name.status.success());
    let stdout = String::from_utf8_lossy(&by_name.stdout);
    assert!(
        stdout.contains("does not match any session IDs")
            && stdout.contains("--regex matches IDs, not names"),
        "unexpected stdout: {stdout}"
    );
    assert_eq!(listed(&root), before);

    let (first_id, _) = &before[0];
    let by_id = gosling(
        &root,
        &["session", "remove", "-r", &format!("^{first_id}$"), "-y"],
    );
    assert!(
        by_id.status.success(),
        "remove by id failed: {}",
        String::from_utf8_lossy(&by_id.stderr)
    );
    assert_eq!(listed(&root), before[1..].to_vec());
}

#[test]
fn regex_help_states_that_it_matches_session_ids() {
    let root = TempDir::new().unwrap();
    let output = gosling(&root, &["session", "remove", "--help"]);
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(
        help.contains("-r, --regex <PATTERN>")
            && help.contains("whose ID")
            && help.contains("names are not matched"),
        "unexpected help: {help}"
    );
}
