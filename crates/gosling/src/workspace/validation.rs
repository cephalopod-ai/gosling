use super::{
    CredentialProfile, WorkspaceIssue, WorkspaceIssueCode, WorkspaceIssueSeverity,
    WorkspaceMutation, WorkspaceValidationReport,
};
use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

pub fn normalize_workspace_path(raw: &str) -> Result<String, String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err("path cannot be empty".to_string());
    }
    if contains_parent_component(trimmed) {
        return Err("path traversal components are not allowed".to_string());
    }
    if !is_platform_absolute(trimmed) {
        return Err("path must be absolute".to_string());
    }

    if is_windows_absolute(trimmed) {
        return Ok(normalize_windows_path(trimmed));
    }

    let mut normalized = PathBuf::new();
    for component in Path::new(trimmed).components() {
        match component {
            Component::CurDir => {}
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized.to_string_lossy().to_string())
}

pub fn validate_workspace_mutation(
    workspace: &WorkspaceMutation,
    profiles: &[CredentialProfile],
) -> WorkspaceValidationReport {
    let mut issues = Vec::new();
    let primary_subject = "primary working folder".to_string();
    let normalized_working_folder = validate_path(
        &workspace.working_folder,
        true,
        &primary_subject,
        None,
        WorkspaceIssueCode::MissingPrimaryFolder,
        &mut issues,
    );

    let mut ids = HashSet::new();
    let mut paths = HashMap::new();
    if let Some(path) = normalized_working_folder.as_ref() {
        paths.insert(comparison_path(path), primary_subject);
    }

    for folder in &workspace.folders {
        validate_identifier(&folder.id, "folder", &mut ids, &mut issues);
        let subject = folder_subject("folder", &folder.label);
        if let Some(path) = validate_path(
            &folder.path,
            false,
            &subject,
            Some(&folder.id),
            WorkspaceIssueCode::MissingFolder,
            &mut issues,
        ) {
            validate_unique_path(path, subject, &folder.id, &mut paths, &mut issues);
        }
    }

    let default_output_count = workspace
        .product_output_folders
        .iter()
        .filter(|output| output.is_default)
        .count();
    if workspace.product_output_folders.is_empty() || default_output_count != 1 {
        issues.push(issue(
            WorkspaceIssueCode::InvalidOutputConfiguration,
            WorkspaceIssueSeverity::Error,
            "a workspace must have at least one output folder and exactly one default output",
            None,
            None,
        ));
    }
    for output in &workspace.product_output_folders {
        validate_identifier(&output.id, "output folder", &mut ids, &mut issues);
        if output.product_types.is_empty() {
            issues.push(issue(
                WorkspaceIssueCode::InvalidOutputConfiguration,
                WorkspaceIssueSeverity::Error,
                "each output folder must support at least one product type",
                Some(output.id.clone()),
                Some(output.path.clone()),
            ));
        }
        let subject = folder_subject("output folder", &output.label);
        if let Some(path) = validate_path(
            &output.path,
            false,
            &subject,
            Some(&output.id),
            WorkspaceIssueCode::MissingOutputFolder,
            &mut issues,
        ) {
            validate_unique_path(path, subject, &output.id, &mut paths, &mut issues);
        }
    }

    let profiles_by_id: HashMap<_, _> = profiles
        .iter()
        .map(|profile| (profile.id.as_str(), profile))
        .collect();
    for binding in &workspace.credential_bindings {
        validate_identifier(&binding.id, "credential binding", &mut ids, &mut issues);
        let profile = profiles_by_id.get(binding.credential_profile_id.as_str());
        if profile.is_none() {
            issues.push(issue(
                WorkspaceIssueCode::MissingCredentialProfile,
                WorkspaceIssueSeverity::Warning,
                "credential profile is missing and must be relinked",
                Some(binding.id.clone()),
                None,
            ));
        } else if profile
            .is_some_and(|profile| profile.status != super::CredentialProfileStatus::Configured)
        {
            issues.push(issue(
                WorkspaceIssueCode::CredentialNeedsAuthentication,
                WorkspaceIssueSeverity::Warning,
                "credential profile requires setup or authentication",
                Some(binding.id.clone()),
                None,
            ));
        }
    }
    let default_flags = workspace
        .credential_bindings
        .iter()
        .filter(|binding| binding.is_default)
        .count();
    if let Some(default_id) = workspace.default_credential_binding_id.as_deref() {
        if default_flags != 1
            || !workspace
                .credential_bindings
                .iter()
                .any(|binding| binding.id == default_id && binding.is_default)
        {
            issues.push(issue(
                WorkspaceIssueCode::InvalidCredentialBinding,
                WorkspaceIssueSeverity::Error,
                "default credential binding does not exist",
                Some(default_id.to_string()),
                None,
            ));
        }
    } else if default_flags != 0 {
        issues.push(issue(
            WorkspaceIssueCode::InvalidCredentialBinding,
            WorkspaceIssueSeverity::Error,
            "credential binding default flags do not match the default binding reference",
            None,
            None,
        ));
    }

    let valid_for_session = !issues
        .iter()
        .any(|item| item.severity == WorkspaceIssueSeverity::Error);
    WorkspaceValidationReport {
        valid_for_session,
        issues,
        normalized_working_folder,
    }
}

/// Session start keeps only the pinned names (plus platform tools), so a name that
/// matches no installed extension silently starts chats without that MCP server.
/// This is a warning, not an error: an extension uninstalled after it was pinned
/// must not make the workspace impossible to save or open.
pub(super) fn validate_default_extensions(
    workspace: &WorkspaceMutation,
    known_extension_names: &HashSet<String>,
    report: &mut WorkspaceValidationReport,
) {
    let Some(pinned) = &workspace.default_extensions else {
        return;
    };
    let mut reported = HashSet::new();
    for name in pinned.iter().map(|name| name.trim()) {
        let is_platform = crate::agents::extension::PLATFORM_EXTENSIONS
            .contains_key(crate::config::extensions::name_to_key(name).as_str());
        if name.is_empty()
            || is_platform
            || known_extension_names.contains(name)
            || !reported.insert(name)
        {
            continue;
        }
        report.issues.push(issue(
            WorkspaceIssueCode::UnknownExtension,
            WorkspaceIssueSeverity::Warning,
            "default extension is not installed; new chats in this workspace will start without it",
            Some(name.to_string()),
            None,
        ));
    }
}

fn folder_subject(kind: &str, label: &str) -> String {
    let label = label.trim();
    if label.is_empty() {
        kind.to_string()
    } else {
        format!("{kind} \"{label}\"")
    }
}

/// `subject` names the folder in every message ("output folder \"Outputs\""), and the
/// path follows it, so a list of issues says which folder each one is about.
fn validate_path(
    raw: &str,
    required: bool,
    subject: &str,
    target_id: Option<&str>,
    missing_code: WorkspaceIssueCode,
    issues: &mut Vec<WorkspaceIssue>,
) -> Option<String> {
    let normalized = match normalize_workspace_path(raw) {
        Ok(path) => path,
        Err(message) => {
            let code = if message.contains("traversal") {
                WorkspaceIssueCode::PathTraversal
            } else {
                WorkspaceIssueCode::RelativePath
            };
            let message = if raw.trim().is_empty() {
                format!("{subject}: {message}")
            } else {
                format!("{subject} ({}): {message}", raw.trim())
            };
            issues.push(issue(
                code,
                WorkspaceIssueSeverity::Error,
                &message,
                target_id.map(str::to_string),
                Some(raw.to_string()),
            ));
            return None;
        }
    };

    if is_native_path(&normalized) {
        let path = Path::new(&normalized);
        if !path.exists() {
            issues.push(issue(
                missing_code,
                if required {
                    WorkspaceIssueSeverity::Error
                } else {
                    WorkspaceIssueSeverity::Warning
                },
                &if required {
                    format!(
                        "{subject} ({normalized}) is unavailable; relink it before starting a session"
                    )
                } else {
                    format!("{subject} ({normalized}) is unavailable")
                },
                target_id.map(str::to_string),
                Some(normalized.clone()),
            ));
        } else if !path.is_dir() {
            issues.push(issue(
                WorkspaceIssueCode::NotDirectory,
                if required {
                    WorkspaceIssueSeverity::Error
                } else {
                    WorkspaceIssueSeverity::Warning
                },
                &format!("{subject} ({normalized}) is not a directory"),
                target_id.map(str::to_string),
                Some(normalized.clone()),
            ));
        } else if let Ok(canonical) = path.canonicalize() {
            if let Some(link_folder) = symlink_leaving_its_folder(path) {
                issues.push(issue(
                    WorkspaceIssueCode::FolderResolvesOutside,
                    WorkspaceIssueSeverity::Warning,
                    &format!(
                        "folder {} resolves to {} outside {}; access applies to the resolved folder",
                        normalized,
                        canonical.display(),
                        link_folder.display()
                    ),
                    target_id.map(str::to_string),
                    Some(normalized.clone()),
                ));
            }
            return Some(canonical.to_string_lossy().to_string());
        }
    } else {
        issues.push(issue(
            WorkspaceIssueCode::InaccessibleFolder,
            if required {
                WorkspaceIssueSeverity::Error
            } else {
                WorkspaceIssueSeverity::Warning
            },
            &format!(
                "{subject} ({normalized}) is unavailable on this platform; relink it before use"
            ),
            target_id.map(str::to_string),
            Some(normalized.clone()),
        ));
    }
    Some(normalized)
}

/// Finds a symlink along `path` whose target leaves the folder that holds the link, e.g.
/// `ws/out -> /elsewhere`. Such a folder looks like part of `ws` in the workspace, but
/// sessions are granted the resolved location. Links that stay inside their folder, and
/// system links such as macOS `/var -> /private/var`, are not reported.
fn symlink_leaving_its_folder(path: &Path) -> Option<PathBuf> {
    path.ancestors().find_map(|link| {
        let is_symlink = std::fs::symlink_metadata(link)
            .map(|metadata| metadata.file_type().is_symlink())
            .unwrap_or(false);
        if !is_symlink {
            return None;
        }
        let folder = link.parent()?;
        let canonical_folder = folder.canonicalize().ok()?;
        let target = link.canonicalize().ok()?;
        (!target.starts_with(&canonical_folder)).then(|| folder.to_path_buf())
    })
}

fn validate_unique_path(
    path: String,
    subject: String,
    target_id: &str,
    paths: &mut HashMap<String, String>,
    issues: &mut Vec<WorkspaceIssue>,
) {
    match paths.entry(comparison_path(&path)) {
        std::collections::hash_map::Entry::Occupied(existing) => issues.push(issue(
            WorkspaceIssueCode::DuplicatePath,
            WorkspaceIssueSeverity::Warning,
            &format!(
                "{subject} ({path}) is the same folder as the {}",
                existing.get()
            ),
            Some(target_id.to_string()),
            Some(path),
        )),
        std::collections::hash_map::Entry::Vacant(slot) => {
            slot.insert(subject);
        }
    }
}

fn validate_identifier(
    id: &str,
    label: &str,
    ids: &mut HashSet<String>,
    issues: &mut Vec<WorkspaceIssue>,
) {
    if id.trim().is_empty() || !ids.insert(id.to_string()) {
        issues.push(issue(
            WorkspaceIssueCode::InvalidOutputConfiguration,
            WorkspaceIssueSeverity::Error,
            &format!("{label} identifiers must be non-empty and unique"),
            (!id.is_empty()).then(|| id.to_string()),
            None,
        ));
    }
}

fn issue(
    code: WorkspaceIssueCode,
    severity: WorkspaceIssueSeverity,
    message: &str,
    target_id: Option<String>,
    path: Option<String>,
) -> WorkspaceIssue {
    WorkspaceIssue {
        code,
        severity,
        message: message.to_string(),
        target_id,
        path,
    }
}

fn contains_parent_component(path: &str) -> bool {
    path.replace('\\', "/").split('/').any(|part| part == "..")
}

fn is_windows_absolute(path: &str) -> bool {
    let bytes = path.as_bytes();
    (bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b':'
        && matches!(bytes[2], b'/' | b'\\'))
        || path.starts_with("\\\\")
        || path.starts_with("//")
}

fn is_platform_absolute(path: &str) -> bool {
    Path::new(path).is_absolute() || is_windows_absolute(path)
}

fn normalize_windows_path(path: &str) -> String {
    let mut value = path.replace('/', "\\");
    while value.contains("\\.\\") {
        value = value.replace("\\.\\", "\\");
    }
    if value.as_bytes().get(1) == Some(&b':') {
        if let Some(drive_letter) = value.get_mut(..1) {
            drive_letter.make_ascii_uppercase();
        }
    }
    while value.len() > 3 && value.ends_with('\\') {
        value.pop();
    }
    value
}

fn is_native_path(path: &str) -> bool {
    cfg!(windows) == is_windows_absolute(path)
}

pub(super) fn is_native_workspace_path(path: &str) -> bool {
    is_native_path(path) && Path::new(path).is_absolute()
}

fn comparison_path(path: &str) -> String {
    if is_windows_absolute(path) {
        path.to_lowercase()
    } else {
        path.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{
        CredentialBinding, CredentialProfileSource, CredentialProfileStatus, ProductOutputFolder,
        ProductType, WorkspaceFolder,
    };

    #[test]
    fn normalizes_unix_and_windows_paths() {
        assert_eq!(
            normalize_workspace_path("/tmp/./project").unwrap(),
            "/tmp/project"
        );
        assert_eq!(
            normalize_workspace_path("c:/Projects/Work/").unwrap(),
            "C:\\Projects\\Work"
        );
        assert_eq!(
            normalize_workspace_path("\\\\server\\share\\folder").unwrap(),
            "\\\\server\\share\\folder"
        );
    }

    #[test]
    fn rejects_relative_and_traversal_paths() {
        assert!(normalize_workspace_path("relative/folder").is_err());
        assert!(normalize_workspace_path("/tmp/../secret").is_err());
        assert!(normalize_workspace_path("C:\\work\\..\\secret").is_err());
    }

    #[test]
    fn missing_primary_folder_blocks_sessions_but_missing_output_warns() {
        let mutation = WorkspaceMutation {
            name: "Test".into(),
            working_folder: "/definitely/missing/gosling-workspace".into(),
            product_output_folders: vec![ProductOutputFolder {
                id: "output".into(),
                label: "Outputs".into(),
                path: "/definitely/missing/gosling-output".into(),
                product_types: vec![ProductType::Document],
                is_default: true,
                create_if_missing: false,
            }],
            ..WorkspaceMutation::default()
        };
        let report = validate_workspace_mutation(&mutation, &[]);

        assert!(!report.valid_for_session);
        assert!(report.issues.iter().any(|issue| {
            issue.code == WorkspaceIssueCode::MissingPrimaryFolder
                && issue.severity == WorkspaceIssueSeverity::Error
        }));
        assert!(report.issues.iter().any(|issue| {
            issue.code == WorkspaceIssueCode::MissingOutputFolder
                && issue.severity == WorkspaceIssueSeverity::Warning
        }));
    }

    fn issue_message(report: &WorkspaceValidationReport, code: WorkspaceIssueCode) -> &str {
        report
            .issues
            .iter()
            .find(|issue| issue.code == code)
            .map(|issue| issue.message.as_str())
            .unwrap()
    }

    #[test]
    fn unavailable_folder_issues_name_the_folder_and_its_path() {
        let mutation = WorkspaceMutation {
            name: "Test".into(),
            working_folder: "/definitely/missing/gosling-primary".into(),
            folders: vec![WorkspaceFolder {
                id: "reference".into(),
                label: "Reference".into(),
                path: "/definitely/missing/gosling-reference".into(),
                ..WorkspaceFolder::default()
            }],
            product_output_folders: vec![ProductOutputFolder {
                id: "output".into(),
                label: "Outputs".into(),
                path: "/definitely/missing/gosling-output".into(),
                product_types: vec![ProductType::Document],
                is_default: true,
                create_if_missing: false,
            }],
            ..WorkspaceMutation::default()
        };

        let report = validate_workspace_mutation(&mutation, &[]);

        assert_eq!(
            issue_message(&report, WorkspaceIssueCode::MissingPrimaryFolder),
            "primary working folder (/definitely/missing/gosling-primary) is unavailable; relink it before starting a session"
        );
        assert_eq!(
            issue_message(&report, WorkspaceIssueCode::MissingFolder),
            "folder \"Reference\" (/definitely/missing/gosling-reference) is unavailable"
        );
        assert_eq!(
            issue_message(&report, WorkspaceIssueCode::MissingOutputFolder),
            "output folder \"Outputs\" (/definitely/missing/gosling-output) is unavailable"
        );
        let output_issue = report
            .issues
            .iter()
            .find(|issue| issue.code == WorkspaceIssueCode::MissingOutputFolder)
            .unwrap();
        assert_eq!(output_issue.target_id.as_deref(), Some("output"));
        assert_eq!(
            output_issue.path.as_deref(),
            Some("/definitely/missing/gosling-output")
        );
    }

    #[test]
    fn duplicate_path_warning_names_both_folders() {
        let root = tempfile::tempdir().unwrap();
        let canonical = root.path().canonicalize().unwrap();
        let mutation = WorkspaceMutation {
            name: "Test".into(),
            working_folder: root.path().to_string_lossy().into(),
            folders: vec![WorkspaceFolder {
                id: "reference".into(),
                label: "Reference".into(),
                path: root.path().to_string_lossy().into(),
                ..WorkspaceFolder::default()
            }],
            product_output_folders: vec![ProductOutputFolder {
                id: "output".into(),
                label: "".into(),
                path: root.path().to_string_lossy().into(),
                product_types: vec![ProductType::Document],
                is_default: true,
                create_if_missing: false,
            }],
            ..WorkspaceMutation::default()
        };

        let report = validate_workspace_mutation(&mutation, &[]);
        let messages: Vec<_> = report
            .issues
            .iter()
            .filter(|issue| issue.code == WorkspaceIssueCode::DuplicatePath)
            .map(|issue| issue.message.clone())
            .collect();

        assert_eq!(
            messages,
            vec![
                format!(
                    "folder \"Reference\" ({}) is the same folder as the primary working folder",
                    canonical.display()
                ),
                format!(
                    "output folder ({}) is the same folder as the primary working folder",
                    canonical.display()
                ),
            ]
        );
    }

    #[test]
    fn a_file_configured_as_a_folder_is_named_in_the_issue() {
        let root = tempfile::tempdir().unwrap();
        let file = root.path().join("notes.txt");
        std::fs::write(&file, "not a folder").unwrap();
        let mutation = WorkspaceMutation {
            name: "Test".into(),
            working_folder: root.path().to_string_lossy().into(),
            product_output_folders: vec![ProductOutputFolder {
                id: "output".into(),
                label: "Outputs".into(),
                path: file.to_string_lossy().into(),
                product_types: vec![ProductType::Document],
                is_default: true,
                create_if_missing: false,
            }],
            ..WorkspaceMutation::default()
        };

        let report = validate_workspace_mutation(&mutation, &[]);

        assert_eq!(
            issue_message(&report, WorkspaceIssueCode::NotDirectory),
            format!(
                "output folder \"Outputs\" ({}) is not a directory",
                file.display()
            )
        );
    }

    #[test]
    fn foreign_platform_primary_folder_blocks_sessions() {
        let foreign = if cfg!(windows) {
            "/tmp/gosling-workspace"
        } else {
            "C:\\Projects\\gosling-workspace"
        };
        let mutation = WorkspaceMutation {
            name: "Foreign".into(),
            working_folder: foreign.into(),
            product_output_folders: vec![ProductOutputFolder {
                id: "output".into(),
                label: "Output".into(),
                path: foreign.into(),
                product_types: vec![ProductType::Document],
                is_default: true,
                create_if_missing: true,
            }],
            ..WorkspaceMutation::default()
        };

        let report = validate_workspace_mutation(&mutation, &[]);

        assert!(!report.valid_for_session);
        assert!(report.issues.iter().any(|issue| {
            issue.code == WorkspaceIssueCode::InaccessibleFolder
                && issue.severity == WorkspaceIssueSeverity::Error
        }));
    }

    #[test]
    fn unavailable_credential_profile_is_visible_before_session_creation() {
        let root = tempfile::tempdir().unwrap();
        let binding = CredentialBinding {
            id: "binding".into(),
            label: "Provider".into(),
            credential_profile_id: "profile".into(),
            is_default: true,
            ..CredentialBinding::default()
        };
        let mutation = WorkspaceMutation {
            name: "Credentials".into(),
            working_folder: root.path().to_string_lossy().into(),
            product_output_folders: vec![ProductOutputFolder {
                id: "output".into(),
                label: "Output".into(),
                path: root.path().to_string_lossy().into(),
                product_types: vec![ProductType::Document],
                is_default: true,
                create_if_missing: false,
            }],
            credential_bindings: vec![binding],
            default_credential_binding_id: Some("binding".into()),
            ..WorkspaceMutation::default()
        };
        let profile = CredentialProfile {
            id: "profile".into(),
            name: "Missing".into(),
            status: CredentialProfileStatus::Missing,
            source: CredentialProfileSource::WorkspaceSecureStorage,
            ..CredentialProfile::default()
        };

        let report = validate_workspace_mutation(&mutation, &[profile]);

        assert!(report.issues.iter().any(|issue| {
            issue.code == WorkspaceIssueCode::CredentialNeedsAuthentication
                && issue.severity == WorkspaceIssueSeverity::Warning
        }));
    }

    fn unknown_extension_targets(
        default_extensions: Option<Vec<&str>>,
        known: &[&str],
    ) -> Vec<String> {
        let mutation = WorkspaceMutation {
            default_extensions: default_extensions
                .map(|names| names.into_iter().map(str::to_string).collect()),
            ..WorkspaceMutation::default()
        };
        let known = known.iter().map(|name| name.to_string()).collect();
        let mut report = WorkspaceValidationReport {
            valid_for_session: true,
            ..WorkspaceValidationReport::default()
        };

        validate_default_extensions(&mutation, &known, &mut report);

        assert!(report.valid_for_session);
        assert!(report
            .issues
            .iter()
            .all(|issue| issue.severity == WorkspaceIssueSeverity::Warning));
        report
            .issues
            .into_iter()
            .map(|issue| issue.target_id.unwrap())
            .collect()
    }

    #[test]
    fn unknown_default_extension_is_reported_once() {
        assert_eq!(
            unknown_extension_targets(Some(vec!["muninn", "nope-ext", " nope-ext "]), &["muninn"]),
            vec!["nope-ext"]
        );
    }

    #[test]
    fn configured_and_platform_default_extensions_are_not_reported() {
        assert!(
            unknown_extension_targets(Some(vec!["muninn", "developer"]), &["muninn"]).is_empty()
        );
        assert!(unknown_extension_targets(Some(vec![]), &[]).is_empty());
        assert!(unknown_extension_targets(None, &[]).is_empty());
    }
}
