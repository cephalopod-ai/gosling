//! An unknown provider named by `GOSLING_PROVIDER` is reported as an unknown provider, not as a
//! missing model (GSL-PT-20260927-A02).

use std::process::{Command, Output};
use tempfile::TempDir;

const CONFIG: &str = "active_provider: openai
providers:
  openai:
    enabled: true
    configured: true
    model: playtest-model
OPENAI_HOST: http://127.0.0.1:9
GOSLING_DISABLE_KEYRING: true
";

fn run(envs: &[(&str, &str)], args: &[&str]) -> String {
    let root = TempDir::new().unwrap();
    let config_dir = root.path().join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(config_dir.join("config.yaml"), CONFIG).unwrap();
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();

    let output: Output = Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(["run", "--no-session", "-t", "Say READY"])
        .args(args)
        .env("HOME", &home)
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .env_remove("GOSLING_PROVIDER")
        .env_remove("GOSLING_MODEL")
        .envs(envs.iter().copied())
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run gosling binary");
    assert!(!output.status.success());
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
}

#[test]
fn an_unknown_provider_from_the_environment_is_named_with_its_source() {
    let output = run(&[("GOSLING_PROVIDER", "nonsense-prov")], &[]);
    assert!(
        output.contains("Unknown provider 'nonsense-prov' (from GOSLING_PROVIDER)"),
        "{output}"
    );
    assert!(!output.contains("No model configured"), "{output}");
}

#[test]
fn an_unknown_provider_flag_is_still_reported_as_unknown() {
    let output = run(&[], &["--provider", "nosuchprovider"]);
    assert!(output.contains("Unknown provider"), "{output}");
    assert!(output.contains("nosuchprovider"), "{output}");
}

#[test]
fn a_known_provider_without_a_model_still_asks_for_a_model() {
    let output = run(&[("GOSLING_PROVIDER", "anthropic")], &[]);
    assert!(output.contains("No model configured"), "{output}");
    assert!(!output.contains("Unknown provider"), "{output}");
}
