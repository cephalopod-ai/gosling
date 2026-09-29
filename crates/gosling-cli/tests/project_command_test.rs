//! `gosling project` fails when the newest project's folder is gone and passes on the exit status
//! of the session it starts (GSL-PT-20260927-D14).

use std::process::{Command, Output};
use tempfile::TempDir;

fn gosling(root: &TempDir, args: &[&str]) -> Output {
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(args)
        .current_dir(root.path())
        .env("HOME", &home)
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .env_remove("GOSLING_PROVIDER")
        .env_remove("GOSLING_MODEL")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run gosling binary")
}

fn track_project(root: &TempDir, path: &str) {
    let data_dir = root.path().join("data");
    std::fs::create_dir_all(&data_dir).unwrap();
    let projects = serde_json::json!({
        "projects": {
            path: {
                "path": path,
                "last_accessed": "2026-09-28T00:00:00Z",
                "last_instruction": null,
                "last_session_id": "20260928_1"
            }
        }
    });
    std::fs::write(data_dir.join("projects.json"), projects.to_string()).unwrap();
}

#[test]
fn a_missing_latest_project_folder_is_an_error() {
    let root = TempDir::new().unwrap();
    let gone = root.path().join("p3-gone");
    track_project(&root, gone.to_str().unwrap());

    let output = gosling(&root, &["project"]);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stderr.contains(&format!(
            "Most recent project directory '{}' no longer exists",
            gone.display()
        )),
        "{stderr}"
    );
    assert!(stderr.contains("gosling projects"), "{stderr}");

    let listing = gosling(&root, &["projects"]);
    assert!(listing.status.success());
    assert!(String::from_utf8_lossy(&listing.stdout).contains("p3-gone"));
}

#[test]
fn a_failing_session_fails_the_project_command() {
    let root = TempDir::new().unwrap();

    let output = gosling(&root, &["project"]);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("No previous projects found"),
        "{stdout}{stderr}"
    );
    assert!(
        stderr.contains("No provider configured"),
        "{stdout}{stderr}"
    );
    assert_eq!(output.status.code(), Some(1), "{stdout}{stderr}");
}
