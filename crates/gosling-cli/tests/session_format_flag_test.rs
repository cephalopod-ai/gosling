//! Unknown `--format` values are usage errors, not a silent text fallback (GSL-PT-20260927-D19).

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

fn assert_usage_error(args: &[&str], accepted: &str) {
    let root = TempDir::new().unwrap();
    let output = gosling(&root, args);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(2),
        "{args:?} should be a usage error; stdout: {} stderr: {stderr}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        stderr.contains("invalid value 'xml'") && stderr.contains(accepted),
        "{args:?} stderr: {stderr}"
    );
    assert!(output.stdout.is_empty(), "{args:?} printed to stdout");
}

#[test]
fn unknown_format_values_are_rejected() {
    assert_usage_error(
        &["session", "list", "--format", "xml"],
        "[possible values: text, json]",
    );
    assert_usage_error(
        &["session", "export", "--session-id", "x", "--format", "xml"],
        "[possible values: markdown, json, yaml]",
    );
    assert_usage_error(
        &[
            "session",
            "context-history",
            "list",
            "--session-id",
            "x",
            "--format",
            "xml",
        ],
        "[possible values: text, json]",
    );
    assert_usage_error(
        &[
            "session",
            "context-history",
            "show",
            "--session-id",
            "x",
            "1",
            "--format",
            "xml",
        ],
        "[possible values: markdown, json]",
    );
    assert_usage_error(
        &[
            "session",
            "context-history",
            "export",
            "--session-id",
            "x",
            "--format",
            "xml",
        ],
        "[possible values: json, markdown]",
    );
}

#[test]
fn accepted_session_list_formats_still_work() {
    let root = TempDir::new().unwrap();
    let json = gosling(&root, &["session", "list", "--format", "json"]);
    assert!(json.status.success());
    assert_eq!(String::from_utf8_lossy(&json.stdout).trim(), "[]");

    let text = gosling(&root, &["session", "list", "-f", "text"]);
    assert!(text.status.success());
    let default = gosling(&root, &["session", "list"]);
    assert!(default.status.success());
    assert_eq!(text.stdout, default.stdout);
}
