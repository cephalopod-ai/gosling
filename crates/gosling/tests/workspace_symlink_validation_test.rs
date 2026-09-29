#![cfg(unix)]

use gosling::workspace::{
    validate_workspace_mutation, ProductOutputFolder, ProductType, WorkspaceFolder,
    WorkspaceFolderAccess, WorkspaceFolderKind, WorkspaceIssue, WorkspaceIssueCode,
    WorkspaceIssueSeverity, WorkspaceMutation,
};
use std::os::unix::fs::symlink;
use std::path::Path;

fn output(id: &str, path: &Path, is_default: bool) -> ProductOutputFolder {
    ProductOutputFolder {
        id: id.into(),
        label: id.into(),
        path: path.to_string_lossy().to_string(),
        product_types: vec![ProductType::Document],
        is_default,
        create_if_missing: false,
    }
}

fn escapes(issues: &[WorkspaceIssue]) -> Vec<&WorkspaceIssue> {
    issues
        .iter()
        .filter(|issue| issue.code == WorkspaceIssueCode::FolderResolvesOutside)
        .collect()
}

#[test]
fn symlinked_output_resolving_outside_its_folder_is_reported_as_a_warning() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("ws");
    let real = workspace.join("real");
    let outside = root.path().join("outside");
    std::fs::create_dir_all(real.join("docs")).unwrap();
    std::fs::create_dir_all(&outside).unwrap();
    symlink(&real, workspace.join("link-internal")).unwrap();
    symlink(&outside, workspace.join("link-escape")).unwrap();

    let mutation = WorkspaceMutation {
        name: "Links".into(),
        working_folder: workspace
            .join("link-internal")
            .to_string_lossy()
            .to_string(),
        product_output_folders: vec![
            output("escape", &workspace.join("link-escape"), true),
            output(
                "inside",
                &workspace.join("link-internal").join("docs"),
                false,
            ),
        ],
        ..WorkspaceMutation::default()
    };
    let report = validate_workspace_mutation(&mutation, &[]);

    assert!(report.valid_for_session, "{:?}", report.issues);
    let escapes = escapes(&report.issues);
    assert_eq!(escapes.len(), 1, "{:?}", report.issues);
    let escape = escapes[0];
    assert_eq!(escape.severity, WorkspaceIssueSeverity::Warning);
    assert_eq!(escape.target_id.as_deref(), Some("escape"));
    let link_path = workspace.join("link-escape").to_string_lossy().to_string();
    let resolved = outside
        .canonicalize()
        .unwrap()
        .to_string_lossy()
        .to_string();
    assert_eq!(escape.path.as_deref(), Some(link_path.as_str()));
    assert!(escape.message.contains(&link_path), "{}", escape.message);
    assert!(escape.message.contains(&resolved), "{}", escape.message);
}

#[test]
fn primary_and_additional_folders_leaving_their_folder_through_a_link_are_reported() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("ws");
    let outside = root.path().join("outside");
    std::fs::create_dir_all(workspace.join("out")).unwrap();
    std::fs::create_dir_all(outside.join("nested")).unwrap();
    symlink(&outside, workspace.join("linked-dir")).unwrap();

    let primary = workspace.join("linked-dir");
    let additional = workspace.join("linked-dir").join("nested");
    let mutation = WorkspaceMutation {
        name: "Links".into(),
        working_folder: primary.to_string_lossy().to_string(),
        folders: vec![WorkspaceFolder {
            id: "reference".into(),
            label: "Reference".into(),
            path: additional.to_string_lossy().to_string(),
            kind: WorkspaceFolderKind::Reference,
            access: WorkspaceFolderAccess::Read,
            description: None,
        }],
        product_output_folders: vec![output("out", &workspace.join("out"), true)],
        ..WorkspaceMutation::default()
    };
    let report = validate_workspace_mutation(&mutation, &[]);

    assert!(report.valid_for_session, "{:?}", report.issues);
    let escapes = escapes(&report.issues);
    assert_eq!(escapes.len(), 2, "{:?}", report.issues);
    assert!(escapes
        .iter()
        .all(|issue| issue.severity == WorkspaceIssueSeverity::Warning));
    assert_eq!(escapes[0].target_id, None);
    assert_eq!(escapes[1].target_id.as_deref(), Some("reference"));
}

#[test]
fn plain_folders_and_internal_links_are_not_reported() {
    let root = tempfile::tempdir().unwrap();
    let workspace = root.path().join("ws");
    std::fs::create_dir_all(workspace.join("real").join("out")).unwrap();
    symlink(workspace.join("real"), workspace.join("link-internal")).unwrap();

    let mutation = WorkspaceMutation {
        name: "Plain".into(),
        working_folder: workspace.to_string_lossy().to_string(),
        product_output_folders: vec![output(
            "out",
            &workspace.join("link-internal").join("out"),
            true,
        )],
        ..WorkspaceMutation::default()
    };
    let report = validate_workspace_mutation(&mutation, &[]);

    assert!(report.valid_for_session);
    assert!(report.issues.is_empty(), "{:?}", report.issues);
}
