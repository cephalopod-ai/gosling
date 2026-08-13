use std::process::{Command, Output};
use tempfile::TempDir;

fn validate(root: &TempDir, provisioning: serde_json::Value) -> Output {
    let path = root.path().join("shell-provisioning.json");
    std::fs::write(&path, serde_json::to_vec_pretty(&provisioning).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args([
            "shell-validate",
            "--shell-id",
            "test_shell",
            "--shell-display-name",
            "Test Shell",
            "--shell-provisioning",
            path.to_str().unwrap(),
        ])
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .output()
        .expect("failed to run gosling binary")
}

#[test]
fn valid_minimal_shell_provisioning_emits_structured_report() {
    let root = TempDir::new().unwrap();
    let output = validate(
        &root,
        serde_json::json!({
            "schemaVersion": 1,
            "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" }
        }),
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], true);
    assert_eq!(report["issues"], serde_json::json!([]));
}

#[test]
fn dynamic_provider_model_is_not_rejected_by_static_preflight() {
    let root = TempDir::new().unwrap();
    let output = validate(
        &root,
        serde_json::json!({
            "schemaVersion": 1,
            "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" },
            "session": {
                "provider": "openai",
                "model": "future-model-from-provider-catalog"
            }
        }),
    );

    assert!(
        output.status.success(),
        "stderr: {}\nstdout: {}",
        String::from_utf8_lossy(&output.stderr),
        String::from_utf8_lossy(&output.stdout)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["resolution"]["model"],
        "future-model-from-provider-catalog"
    );
}

#[test]
fn invalid_references_are_reported_without_starting_a_server() {
    let root = TempDir::new().unwrap();
    let output = validate(
        &root,
        serde_json::json!({
            "schemaVersion": 1,
            "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" },
            "session": {
                "workspaceId": "missing-workspace",
                "credentialProfileId": "missing-profile",
                "provider": "missing-provider",
                "model": "missing-model",
                "extensions": [{ "name": "missing-extension" }],
                "skillIds": ["missing-skill"]
            },
            "protocolPolicy": {
                "mode": "restricted",
                "deniedMethods": ["not/a/gosling/method"]
            }
        }),
    );

    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let codes = report["issues"]
        .as_array()
        .unwrap()
        .iter()
        .map(|issue| issue["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    for expected in [
        "missing_workspace",
        "missing_credential_profile",
        "missing_provider",
        "missing_extension",
        "missing_skill",
        "invalid_denied_method",
    ] {
        assert!(codes.contains(&expected), "missing {expected}: {report}");
    }
    let report_text = serde_json::to_string(&report).unwrap();
    assert!(!report_text.contains("secretValue"));
}
