//! `session remove --path` removes the session named by the file stem, like the
//! other commands that accept the legacy `--path` (GSL-PT-20260927-D16 follow-up).

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

fn import_session(root: &TempDir, label: &str) {
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
            "content": [{"type": "input_text", "text": format!("hello {label}")}]
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

fn listed_ids(root: &TempDir) -> Vec<String> {
    let output = gosling(root, &["session", "list", "--format", "json"]);
    assert!(output.status.success());
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let mut ids: Vec<String> = sessions
        .iter()
        .map(|s| s["id"].as_str().unwrap().to_string())
        .collect();
    ids.sort();
    ids
}

#[test]
fn remove_by_legacy_path_removes_the_session_named_by_the_file_stem() {
    let root = TempDir::new().unwrap();
    import_session(&root, "alpha");
    import_session(&root, "beta");
    let before = listed_ids(&root);
    assert_eq!(before.len(), 2);

    let target = &before[0];
    let legacy_path = format!("/old/sessions/{target}.jsonl");
    let output = gosling(&root, &["session", "remove", "--path", &legacy_path, "-y"]);
    assert!(
        output.status.success(),
        "remove --path failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(listed_ids(&root), before[1..].to_vec());
}

#[test]
fn remove_by_legacy_path_reports_an_unknown_session() {
    let root = TempDir::new().unwrap();
    import_session(&root, "alpha");
    let before = listed_ids(&root);

    let output = gosling(
        &root,
        &[
            "session",
            "remove",
            "--path",
            "/old/sessions/nope_1.jsonl",
            "-y",
        ],
    );
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Session ID 'nope_1' not found"),
        "unexpected stderr: {stderr}"
    );
    assert_eq!(listed_ids(&root), before);
}
