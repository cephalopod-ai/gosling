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

/// GSL-PT-20260927-C24 / A20: `secret set` named config.yaml although the
/// values went to the secret store.
#[test]
fn set_names_the_secret_store_it_wrote() {
    let root = TempDir::new().unwrap();
    let output = gosling(
        &root,
        &[
            "secret",
            "set",
            "ptbox",
            "--password",
            "ptbox-password-7713",
        ],
    );
    assert_success(&output);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let secrets_file = root.path().join("config").join("secrets.yaml");
    assert!(
        stdout.contains(&format!(
            "Stored 1 field(s) for server 'PTBOX' in {}",
            secrets_file.display()
        )),
        "{stdout}"
    );
    assert!(!stdout.contains("config.yaml"), "{stdout}");
    assert!(std::fs::read_to_string(&secrets_file)
        .unwrap()
        .contains("ptbox-password-7713"));
}

/// GSL-PT-20260927-A20: removing a server with nothing stored reported success.
#[test]
fn remove_fails_for_an_unknown_server_and_succeeds_for_a_stored_one() {
    let root = TempDir::new().unwrap();
    let unknown = gosling(&root, &["secret", "remove", "nosuch"]);
    assert!(!unknown.status.success());
    assert!(String::from_utf8_lossy(&unknown.stderr)
        .contains("no stored credentials found for server 'NOSUCH'"));

    assert_success(&gosling(
        &root,
        &[
            "secret", "set", "myvps", "--login", "admin", "--port", "2222",
        ],
    ));
    let removed = gosling(&root, &["secret", "remove", "myvps"]);
    assert_success(&removed);
    assert!(
        String::from_utf8_lossy(&removed.stdout).contains("Removed credentials for server 'MYVPS'")
    );
    assert!(!gosling(&root, &["secret", "get", "myvps"]).status.success());
}
