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

fn assert_success(output: &Output) {
    assert!(
        output.status.success(),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn list_shows_servers_but_not_extension_secrets() {
    let root = TempDir::new().unwrap();
    assert_success(&gosling(
        &root,
        &[
            "mcp",
            "install",
            "sshx",
            "--cmd",
            "server",
            "--secret",
            "VPS_PASSWORD=extension-owned",
        ],
    ));
    assert_success(&gosling(
        &root,
        &["secret", "set", "myvps", "--password", "server-owned"],
    ));

    let output = gosling(&root, &["secret", "list"]);
    assert_success(&output);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(stdout.lines().collect::<Vec<_>>(), ["MYVPS: PASSWORD"]);
}
