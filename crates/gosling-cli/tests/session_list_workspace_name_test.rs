//! `session list` names a session's workspace by its current name, so a
//! renamed workspace whose old name was taken by another workspace is never
//! misattributed (GSL-PT-20260927-G105).

use gosling::config::GoslingMode;
use gosling::session::{SessionManager, SessionType};
use gosling::workspace::WorkspaceSessionContext;
use std::path::{Path, PathBuf};
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

fn workspace(id: &str, name: &str, folder: &Path) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "schemaVersion": 1,
        "name": name,
        "workingFolder": folder,
        "productOutputFolders": [{
            "id": format!("{id}-outputs"),
            "label": "Outputs",
            "path": folder,
            "productTypes": ["document"],
            "isDefault": true,
            "createIfMissing": false
        }],
        "createdAt": "2026-09-27T00:00:00Z",
        "updatedAt": "2026-09-27T00:00:00Z",
        "lastOpenedAt": "2026-09-27T00:00:00Z"
    })
}

fn write_workspaces(data_dir: &Path, folder: &Path) {
    let directory = data_dir.join("workspaces");
    std::fs::create_dir_all(&directory).unwrap();
    let document = serde_json::json!({
        "schema_version": 1,
        "active_workspace_id": "default",
        "default_workspace_id": "default",
        "migration_completed": true,
        "templates_materialized": true,
        "workspaces": [
            workspace("default", "Default", folder),
            workspace("renamed", "Alpha Renamed", folder),
            workspace("reused", "Alpha", folder),
        ]
    });
    std::fs::write(
        directory.join("workspaces.json"),
        serde_json::to_vec(&document).unwrap(),
    )
    .unwrap();
}

async fn seed_session(data_dir: &Path, folder: &Path, workspace_id: &str, snapshot: &str) {
    let sessions = SessionManager::new(data_dir.to_path_buf());
    let session = sessions
        .create_session(
            folder.to_path_buf(),
            format!("chat in {workspace_id}"),
            SessionType::Acp,
            GoslingMode::default(),
        )
        .await
        .unwrap();
    sessions
        .update(&session.id)
        .workspace_snapshot(
            workspace_id.to_string(),
            snapshot.to_string(),
            None,
            None,
            None,
            WorkspaceSessionContext {
                workspace_id: workspace_id.to_string(),
                workspace_name: snapshot.to_string(),
                primary_working_folder: folder.to_string_lossy().to_string(),
                ..Default::default()
            },
        )
        .apply()
        .await
        .unwrap();
}

fn listed_workspace_names(root: &TempDir) -> Vec<(String, String)> {
    let output = gosling(root, &["session", "list", "--format", "json"]);
    assert!(
        output.status.success(),
        "session list failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    let mut names: Vec<(String, String)> = sessions
        .iter()
        .map(|session| {
            (
                session["workspace_id"].as_str().unwrap().to_string(),
                session["workspace_name"].as_str().unwrap().to_string(),
            )
        })
        .collect();
    names.sort();
    names
}

#[tokio::test]
async fn session_list_reports_the_current_workspace_name() {
    let root = TempDir::new().unwrap();
    let data_dir = root.path().join("data");
    let folder = PathBuf::from("/tmp/session-list-workspace-name");
    write_workspaces(&data_dir, &folder);
    seed_session(&data_dir, &folder, "renamed", "Alpha").await;
    seed_session(&data_dir, &folder, "deleted", "Gamma").await;

    assert_eq!(
        listed_workspace_names(&root),
        vec![
            ("deleted".to_string(), "Gamma (removed)".to_string()),
            ("renamed".to_string(), "Alpha Renamed".to_string()),
        ]
    );
}
