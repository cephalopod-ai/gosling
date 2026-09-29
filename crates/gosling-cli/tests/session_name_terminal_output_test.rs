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

fn import(root: &TempDir, file_name: &str, transcript: &str) -> String {
    let path = root.path().join(file_name);
    std::fs::write(&path, transcript).unwrap();
    let working_dir = root.path().canonicalize().unwrap();
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
    String::from_utf8(output.stdout).unwrap()
}

fn codex_transcript(first_user_text: &str) -> String {
    let meta = serde_json::json!({
        "timestamp": "2026-05-22T13:37:22Z",
        "type": "session_meta",
        "payload": {"id": "inj", "cwd": "/w"}
    });
    let message = serde_json::json!({
        "timestamp": "2026-05-22T13:37:23Z",
        "type": "response_item",
        "payload": {
            "type": "message",
            "role": "user",
            "content": [{"type": "input_text", "text": first_user_text}]
        }
    });
    format!("{meta}\n{message}\n")
}

fn claude_code_transcript_titled(ai_title: &str) -> String {
    let user = serde_json::json!({
        "type": "user",
        "sessionId": "titled",
        "uuid": "u1",
        "timestamp": "2026-01-01T00:00:01Z",
        "cwd": "/tmp",
        "message": {"role": "user", "content": "hi"}
    });
    let title = serde_json::json!({
        "type": "ai-title",
        "sessionId": "titled",
        "aiTitle": ai_title
    });
    format!("{user}\n{title}\n")
}

fn session_list_text(root: &TempDir) -> String {
    let output = gosling(root, &["session", "list"]);
    assert!(output.status.success());
    String::from_utf8(output.stdout).unwrap()
}

fn session_names_from_json(root: &TempDir) -> Vec<String> {
    let output = gosling(root, &["session", "list", "--format", "json"]);
    assert!(output.status.success());
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    sessions
        .iter()
        .map(|s| s["name"].as_str().unwrap().to_string())
        .collect()
}

fn assert_no_terminal_controls(text: &str) {
    assert!(
        !text.contains('\u{1b}') && !text.contains('\u{7}'),
        "raw ESC/BEL reached the terminal: {text:?}"
    );
}

const INJECTED: &str = "hello \u{1b}]0;PWNED-BY-IMPORT\u{7}\u{1b}[2J\u{1b}[31mfake";

#[test]
fn imported_session_name_escape_sequences_are_inert_in_import_and_list_output() {
    let root = TempDir::new().unwrap();

    let import_output = import(&root, "inject-codex.jsonl", &codex_transcript(INJECTED));
    assert_no_terminal_controls(&import_output);
    assert!(
        import_output.contains(r"hello \u{1b}]0;PWNED-BY-IMPORT\u{7}\u{1b}[2J\u{1b}[31mfake"),
        "import line should show the escaped name: {import_output:?}"
    );

    let list = session_list_text(&root);
    assert_no_terminal_controls(&list);
    assert!(list.contains(r"hello \u{1b}]0;PWNED-BY-IMPORT"));
}

#[test]
fn session_name_with_newline_stays_on_one_list_line() {
    let root = TempDir::new().unwrap();
    import(
        &root,
        "titled.jsonl",
        &claude_code_transcript_titled("line1\nline2"),
    );

    let list = session_list_text(&root);
    let session_lines: Vec<&str> = list.lines().skip(1).collect();
    assert_eq!(session_lines.len(), 1, "name split the row: {list:?}");
    assert!(session_lines[0].contains(r"line1\nline2"));
}

#[test]
fn unicode_session_names_print_unchanged() {
    let root = TempDir::new().unwrap();
    let name = "café naïve 🪿 日本語";

    let import_output = import(&root, "unicode.jsonl", &codex_transcript(name));
    assert!(import_output.contains(&format!(" - {name}\n")));
    assert!(session_list_text(&root).contains(&format!(" - {name} - ")));
}

#[test]
fn json_session_list_keeps_the_exact_stored_name() {
    let root = TempDir::new().unwrap();
    import(&root, "inject-codex.jsonl", &codex_transcript(INJECTED));
    import(
        &root,
        "titled.jsonl",
        &claude_code_transcript_titled("line1\nline2"),
    );

    let mut names = session_names_from_json(&root);
    names.sort();
    let mut expected = vec![INJECTED.to_string(), "line1\nline2".to_string()];
    expected.sort();
    assert_eq!(names, expected);
}
