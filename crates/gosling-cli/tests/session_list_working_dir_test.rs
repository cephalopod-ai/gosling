//! `session list -w` must match whole path components (GSL-PT-20260927-D07).

use std::path::Path;
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

fn import_into(root: &TempDir, id: &str, working_dir: &Path) {
    let meta = serde_json::json!({
        "timestamp": "2026-05-22T13:37:22Z",
        "type": "session_meta",
        "payload": {"id": id, "cwd": "/w"}
    });
    let message = serde_json::json!({
        "timestamp": "2026-05-22T13:37:23Z",
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": format!("session {id}")}]
        }
    });
    let path = root.path().join(format!("{id}.jsonl"));
    std::fs::write(&path, format!("{meta}\n{message}\n")).unwrap();
    let output = gosling(
        root,
        &[
            "session",
            "import",
            path.to_str().unwrap(),
            "--working-dir",
            working_dir.to_str().unwrap(),
        ],
    );
    assert!(
        output.status.success(),
        "import failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn listed_names(root: &TempDir, filter: &str) -> Vec<String> {
    let output = gosling(root, &["session", "list", "--format", "json", "-w", filter]);
    assert!(output.status.success());
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let mut names: Vec<String> = sessions
        .iter()
        .map(|session| session["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    names
}

#[test]
fn working_dir_filter_lists_only_that_directory_and_below() {
    let root = TempDir::new().unwrap();
    let base = root.path().canonicalize().unwrap();
    let proj = base.join("proj");
    let nested = proj.join("nested");
    let sibling = base.join("proj-b ünï space");
    for dir in [&nested, &sibling] {
        std::fs::create_dir_all(dir).unwrap();
    }
    import_into(&root, "in-proj", &proj);
    import_into(&root, "in-nested", &nested);
    import_into(&root, "in-sibling", &sibling);

    let expected = vec![
        "session in-nested".to_string(),
        "session in-proj".to_string(),
    ];
    assert_eq!(listed_names(&root, proj.to_str().unwrap()), expected);
    assert_eq!(
        listed_names(&root, &format!("{}/", proj.display())),
        expected
    );
    assert_eq!(
        listed_names(&root, sibling.to_str().unwrap()),
        vec!["session in-sibling".to_string()]
    );
}
