use std::process::{Command, Output};
use tempfile::TempDir;

fn gosling(root: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(args)
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .output()
        .expect("failed to run gosling binary")
}

#[test]
fn diagnostics_rejects_missing_session_without_creating_output() {
    let root = TempDir::new().unwrap();
    let output_path = root.path().join("missing-session-diagnostics.json");
    let output = gosling(
        &root,
        &[
            "session",
            "diagnostics",
            "--session-id",
            "session-that-does-not-exist",
            "--output",
            output_path.to_str().unwrap(),
        ],
    );

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Session 'session-that-does-not-exist'")
    );
    assert!(!output_path.exists());
}
