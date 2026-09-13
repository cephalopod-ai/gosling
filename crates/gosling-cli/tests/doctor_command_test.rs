use std::process::{Command, Output};
use tempfile::TempDir;

fn doctor(root: &TempDir, config: Option<&str>, envs: &[(&str, &str)]) -> Output {
    if let Some(config) = config {
        let config_dir = root.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.yaml"), config).unwrap();
    }

    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .arg("doctor")
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .env_remove("GOSLING_PROVIDER")
        .env_remove("GOSLING_MODEL")
        .envs(envs.iter().copied())
        .output()
        .expect("failed to run gosling binary")
}

#[test]
fn doctor_fails_without_configuration() {
    let root = TempDir::new().unwrap();
    let output = doctor(&root, None, &[]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("no provider configured"));
}

#[test]
fn doctor_fails_for_unknown_provider() {
    let root = TempDir::new().unwrap();
    let output = doctor(
        &root,
        None,
        &[
            ("GOSLING_PROVIDER", "nonsense-prov"),
            ("GOSLING_MODEL", "m1"),
        ],
    );

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Unknown provider: nonsense-prov"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn doctor_succeeds_for_a_configured_known_provider_without_verifying_it() {
    let root = TempDir::new().unwrap();
    let output = doctor(
        &root,
        Some("GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\n"),
        &[],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("not verified"));
}
