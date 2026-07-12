use serde_yaml::Value;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn gosling(root: &TempDir, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(args)
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .env_remove("PR26_GOOSE_TOKEN")
        .output()
        .expect("failed to run gosling binary")
}

fn read_extensions(root: &TempDir) -> Value {
    let content = std::fs::read_to_string(root.path().join("config").join("config.yaml"))
        .expect("config.yaml should exist");
    let config: Value = serde_yaml::from_str(&content).unwrap();
    config.get("extensions").cloned().expect("extensions key")
}

fn write_config(root: &TempDir, content: &str) {
    let config_dir = root.path().join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.yaml"), content).unwrap();
}

#[test]
fn install_writes_stdio_extension() {
    let root = TempDir::new().unwrap();
    write_config(
        &root,
        r#"
extensions:
  existing:
    enabled: true
    type: builtin
    name: existing
    description: keep me
"#,
    );

    let output = gosling(
        &root,
        &[
            "mcp",
            "install",
            "my-server",
            "--cmd",
            "npx -y @block/gdrive",
            "--env",
            "FOO=bar",
            "--timeout",
            "42",
            "--description",
            "test server",
        ],
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let extensions = read_extensions(&root);
    let entry = extensions.get("my-server").expect("my-server entry");
    assert_eq!(entry.get("type").unwrap().as_str(), Some("stdio"));
    assert_eq!(entry.get("enabled").unwrap().as_bool(), Some(true));
    assert_eq!(entry.get("cmd").unwrap().as_str(), Some("npx"));
    assert_eq!(
        entry.get("args").unwrap().as_sequence().unwrap().len(),
        2,
        "args should be ['-y', '@block/gdrive']"
    );
    assert_eq!(
        entry.get("envs").unwrap().get("FOO").unwrap().as_str(),
        Some("bar")
    );
    assert_eq!(entry.get("timeout").unwrap().as_u64(), Some(42));
    assert_eq!(
        entry.get("description").unwrap().as_str(),
        Some("test server")
    );
    assert!(
        extensions.get("existing").is_some(),
        "pre-existing entries must be preserved"
    );
}

#[test]
fn install_stores_secrets_outside_config() {
    let root = TempDir::new().unwrap();
    let output = gosling(
        &root,
        &[
            "mcp",
            "install",
            "secret-server",
            "--cmd",
            "run-server",
            "--secret",
            "MY_TOKEN=hunter2",
        ],
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let extensions = read_extensions(&root);
    let entry = extensions.get("secret-server").unwrap();
    let env_keys = entry.get("env_keys").unwrap().as_sequence().unwrap();
    assert_eq!(env_keys[0].as_str(), Some("MY_TOKEN"));
    assert!(
        entry.get("envs").unwrap().get("MY_TOKEN").is_none(),
        "secret value must not be written to config.yaml"
    );

    let secrets = std::fs::read_to_string(root.path().join("config").join("secrets.yaml")).unwrap();
    assert!(secrets.contains("MY_TOKEN"));
}

#[test]
fn install_config_failure_restores_secret_values() {
    let root = TempDir::new().unwrap();
    write_config(&root, "extensions: {}\n");
    std::fs::write(
        root.path().join("config").join("secrets.yaml"),
        "ROLLBACK_TOKEN: old-value\n",
    )
    .unwrap();
    std::fs::create_dir(root.path().join("config").join("config.tmp")).unwrap();

    let output = gosling(
        &root,
        &[
            "mcp",
            "install",
            "failed-server",
            "--cmd",
            "server",
            "--secret",
            "ROLLBACK_TOKEN=new-value",
        ],
    );

    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Installed extension"));
    assert!(read_extensions(&root).get("failed-server").is_none());
    let secrets = std::fs::read_to_string(root.path().join("config").join("secrets.yaml")).unwrap();
    assert!(secrets.contains("ROLLBACK_TOKEN"));
    assert!(secrets.contains("old-value"));
    assert!(!secrets.contains("new-value"));
}

#[test]
fn install_rejects_malformed_env() {
    let root = TempDir::new().unwrap();
    let output = gosling(
        &root,
        &[
            "mcp", "install", "bad-env", "--cmd", "server", "--env", "NOVALUE",
        ],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("KEY=VALUE"));
}

#[test]
fn install_imports_from_goose_config() {
    let root = TempDir::new().unwrap();
    let goose_config = root.path().join("goose-config.yaml");
    std::fs::write(
        &goose_config,
        r#"
extensions:
  muninn:
    enabled: false
    type: stdio
    name: muninn
    cmd: /opt/muninn/bin/muninn
    args:
    - mcp
    envs:
      MUNINN_EMBED_PROVIDER: ollama
    timeout: 300
    description: memory fabric
    bundled: false
"#,
    )
    .unwrap();

    let output = gosling(
        &root,
        &[
            "mcp",
            "install",
            "muninn",
            "--from-goose",
            "--goose-config",
            goose_config.to_str().unwrap(),
            "--env",
            "MUNINN_EMBED_DIM=768",
        ],
    );
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let extensions = read_extensions(&root);
    let entry = extensions.get("muninn").expect("muninn entry");
    assert_eq!(
        entry.get("enabled").unwrap().as_bool(),
        Some(true),
        "installing should enable the imported extension"
    );
    assert_eq!(
        entry.get("cmd").unwrap().as_str(),
        Some("/opt/muninn/bin/muninn")
    );
    let envs = entry.get("envs").unwrap();
    assert_eq!(
        envs.get("MUNINN_EMBED_PROVIDER").unwrap().as_str(),
        Some("ollama")
    );
    assert_eq!(
        envs.get("MUNINN_EMBED_DIM").unwrap().as_str(),
        Some("768"),
        "--env additions should merge into the imported entry"
    );
}

#[test]
fn install_from_goose_rejects_unresolved_env_keys() {
    let root = TempDir::new().unwrap();
    let goose_config = root.path().join("goose-config.yaml");
    std::fs::write(
        &goose_config,
        r#"
extensions:
  protected:
    enabled: true
    type: stdio
    name: protected
    cmd: protected-server
    args: []
    env_keys:
    - PR26_GOOSE_TOKEN
    timeout: 300
"#,
    )
    .unwrap();

    let output = gosling(
        &root,
        &[
            "mcp",
            "install",
            "protected",
            "--from-goose",
            "--goose-config",
            goose_config.to_str().unwrap(),
        ],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("PR26_GOOSE_TOKEN"));
    assert!(stderr.contains("--secret KEY=VALUE"));
    let config_path = root.path().join("config").join("config.yaml");
    if config_path.exists() {
        assert!(!std::fs::read_to_string(config_path)
            .unwrap()
            .contains("protected"));
    }
}

#[test]
fn install_from_goose_accepts_explicit_secret_values() {
    let root = TempDir::new().unwrap();
    let goose_config = root.path().join("goose-config.yaml");
    std::fs::write(
        &goose_config,
        r#"
extensions:
  protected:
    enabled: true
    type: stdio
    name: protected
    cmd: protected-server
    args: []
    env_keys:
    - PR26_GOOSE_TOKEN
    timeout: 300
"#,
    )
    .unwrap();

    let output = gosling(
        &root,
        &[
            "mcp",
            "install",
            "protected",
            "--from-goose",
            "--goose-config",
            goose_config.to_str().unwrap(),
            "--secret",
            "PR26_GOOSE_TOKEN=secret-value",
        ],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let extensions = read_extensions(&root);
    let entry = extensions.get("protected").unwrap();
    assert_eq!(
        entry.get("env_keys").unwrap().as_sequence().unwrap()[0].as_str(),
        Some("PR26_GOOSE_TOKEN")
    );
    let config = std::fs::read_to_string(root.path().join("config").join("config.yaml")).unwrap();
    assert!(!config.contains("secret-value"));
    let secrets = std::fs::read_to_string(root.path().join("config").join("secrets.yaml")).unwrap();
    assert!(secrets.contains("PR26_GOOSE_TOKEN"));
    assert!(secrets.contains("secret-value"));
}

#[test]
fn install_from_goose_rejects_missing_entry() {
    let root = TempDir::new().unwrap();
    let goose_config = root.path().join("goose-config.yaml");
    std::fs::write(&goose_config, "extensions: {}\n").unwrap();

    let output = gosling(
        &root,
        &[
            "mcp",
            "install",
            "nonexistent",
            "--from-goose",
            "--goose-config",
            goose_config.to_str().unwrap(),
        ],
    );
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("no extension 'nonexistent'"));
}

#[test]
fn remove_deletes_configured_extension() {
    let root = TempDir::new().unwrap();
    write_config(
        &root,
        r#"
extensions:
  doomed:
    enabled: true
    type: stdio
    name: doomed
    description: about to go
    cmd: server
    args: []
    timeout: 300
"#,
    );

    let output = gosling(&root, &["mcp", "remove", "doomed"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(read_extensions(&root).get("doomed").is_none());

    let output = gosling(&root, &["mcp", "remove", "doomed"]);
    assert!(!output.status.success(), "second remove should fail");
}

#[test]
fn remove_reports_config_persistence_failure() {
    let root = TempDir::new().unwrap();
    write_config(
        &root,
        r#"
extensions:
  retained:
    enabled: true
    type: stdio
    name: retained
    description: keep me
    cmd: server
    args: []
    timeout: 300
"#,
    );
    std::fs::create_dir(root.path().join("config").join("config.tmp")).unwrap();

    let output = gosling(&root, &["mcp", "remove", "retained"]);

    assert!(!output.status.success());
    assert!(!String::from_utf8_lossy(&output.stdout).contains("Removed extension"));
    assert!(read_extensions(&root).get("retained").is_some());
}

#[test]
fn list_shows_configured_extensions() {
    let root = TempDir::new().unwrap();
    write_config(
        &root,
        r#"
extensions:
  my-server:
    enabled: true
    type: stdio
    name: my-server
    description: a server
    cmd: server
    args:
    - --flag
    timeout: 300
  off-server:
    enabled: false
    type: streamable_http
    name: off-server
    description: remote
    uri: http://localhost:8000/mcp
    timeout: 300
"#,
    );

    let output = gosling(&root, &["mcp", "list"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("my-server"));
    assert!(stdout.contains("server --flag"));
    assert!(stdout.contains("off-server"));
    assert!(stdout.contains("disabled"));
    assert!(stdout.contains("http://localhost:8000/mcp"));
}

#[test]
fn bare_server_form_still_parses() {
    let root = TempDir::new().unwrap();
    let output = gosling(&root, &["mcp", "not-a-real-server"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Invalid command"),
        "unknown bare server names should still reach the McpCommand parser"
    );
}

#[test]
fn plain_mcp_shows_help() {
    let root = TempDir::new().unwrap();
    let output = gosling(&root, &["mcp"]);
    assert!(!output.status.success());
    let help = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(help.contains("install"));
    assert!(help.contains("serve"));
}

#[test]
fn config_dir_is_isolated_by_path_root() {
    // Guard against the test binary silently writing to the real ~/.config/gosling:
    // GOSLING_PATH_ROOT must fully control where config.yaml lands.
    let root = TempDir::new().unwrap();
    let output = gosling(&root, &["mcp", "install", "isolated", "--cmd", "server"]);
    assert!(output.status.success());
    assert!(Path::new(&root.path().join("config").join("config.yaml")).exists());
}
