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
fn selectable_catalog_credential_policy_alone_is_accepted() {
    let root = TempDir::new().unwrap();
    let output = validate(
        &root,
        serde_json::json!({
            "schemaVersion": 1,
            "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" },
            "session": { "credentialPolicy": "selectable_catalog" }
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
fn selectable_catalog_combined_with_a_fixed_credential_profile_id_is_rejected() {
    let root = TempDir::new().unwrap();
    let output = validate(
        &root,
        serde_json::json!({
            "schemaVersion": 1,
            "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" },
            "session": {
                "credentialPolicy": "selectable_catalog",
                "credentialProfileId": "some-fixed-profile"
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
    assert!(
        codes.contains(&"invalid_credential_policy"),
        "missing invalid_credential_policy: {report}"
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

fn validate_in(
    root: &TempDir,
    cwd: &std::path::Path,
    provisioning: serde_json::Value,
    extra_args: &[&str],
) -> Output {
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
        .args(extra_args)
        .current_dir(cwd)
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .output()
        .expect("failed to run gosling binary")
}

fn minimal_provisioning() -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": 1,
        "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" }
    })
}

#[test]
fn validation_does_not_create_workspace_state_in_a_fresh_root() {
    let root = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();

    let output = validate_in(&root, cwd.path(), minimal_provisioning(), &[]);

    assert!(output.status.success(), "{output:?}");
    assert!(
        !root.path().join("data").join("workspaces").exists(),
        "validation must not create the workspace store"
    );
    assert!(!cwd.path().join("Outputs").exists());
}

#[tokio::test]
async fn validation_resolves_workspaces_from_an_existing_store() {
    let root = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();
    let service =
        gosling::workspace::WorkspaceService::initialize(&root.path().join("data"), cwd.path())
            .await
            .unwrap();
    let (_, _, default_workspace_id) = service.list().unwrap();

    let output = validate_in(
        &root,
        cwd.path(),
        serde_json::json!({
            "schemaVersion": 1,
            "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" },
            "session": { "workspaceId": default_workspace_id }
        }),
        &[],
    );

    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["resolution"]["workspaceId"], default_workspace_id);
    assert!(
        !report["issues"]
            .as_array()
            .unwrap()
            .iter()
            .any(|issue| issue["code"] == "missing_workspace"),
        "{report}"
    );
}

#[test]
fn unknown_builtins_are_reported() {
    let root = TempDir::new().unwrap();
    let cwd = TempDir::new().unwrap();

    let output = validate_in(
        &root,
        cwd.path(),
        minimal_provisioning(),
        &["--with-builtin", "developer,bogus-ext"],
    );

    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["valid"], false);
    assert_eq!(
        report["issues"],
        serde_json::json!([{
            "code": "missing_extension",
            "severity": "error",
            "path": "--with-builtin[1]",
            "message": "builtin extension 'bogus-ext' does not exist"
        }])
    );
}

#[test]
fn unreadable_or_invalid_documents_are_named_in_the_error() {
    let root = TempDir::new().unwrap();
    let missing = root.path().join("missing.json");
    let invalid = root.path().join("invalid.json");
    std::fs::write(&invalid, "{}").unwrap();

    for document in [&missing, &invalid] {
        let output = Command::new(env!("CARGO_BIN_EXE_gosling"))
            .args([
                "shell-validate",
                "--shell-id",
                "test_shell",
                "--shell-display-name",
                "Test Shell",
                "--shell-provisioning",
                document.to_str().unwrap(),
            ])
            .env("GOSLING_PATH_ROOT", root.path())
            .env("GOSLING_DISABLE_KEYRING", "1")
            .output()
            .expect("failed to run gosling binary");

        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains(document.to_str().unwrap()), "{stderr}");
    }
}
