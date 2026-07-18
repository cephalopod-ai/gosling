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
    let normalized_working_folder = validate_path(
        &workspace.working_folder,
        true,
        None,
        WorkspaceIssueCode::MissingPrimaryFolder,
        &mut issues,
    );

    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    if let Some(path) = normalized_working_folder.as_ref() {
        paths.insert(comparison_path(path));
    }

    for folder in &workspace.folders {
        validate_identifier(&folder.id, "folder", &mut ids, &mut issues);
        if let Some(path) = validate_path(
            &folder.path,
            false,
            Some(&folder.id),
            WorkspaceIssueCode::MissingFolder,
            &mut issues,
        ) {
            validate_unique_path(path, &folder.id, &mut paths, &mut issues);
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
        if let Some(path) = validate_path(
            &output.path,
            false,
            Some(&output.id),
            WorkspaceIssueCode::MissingOutputFolder,
            &mut issues,
        ) {
            validate_unique_path(path, &output.id, &mut paths, &mut issues);
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

fn validate_path(
    raw: &str,
    required: bool,
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
                if required {
                    "primary working folder is unavailable; relink it before starting a session"
                } else {
                    "optional workspace folder is unavailable"
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
                "workspace path is not a directory",
                target_id.map(str::to_string),
                Some(normalized.clone()),
            ));
        } else if let Ok(canonical) = path.canonicalize() {
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
            "workspace path is unavailable on this platform; relink it before use",
            target_id.map(str::to_string),
            Some(normalized.clone()),
        ));
    }
    Some(normalized)
}

fn validate_unique_path(
    path: String,
    target_id: &str,
    paths: &mut HashSet<String>,
    issues: &mut Vec<WorkspaceIssue>,
) {
    if !paths.insert(comparison_path(&path)) {
        issues.push(issue(
            WorkspaceIssueCode::DuplicatePath,
            WorkspaceIssueSeverity::Warning,
            "workspace folder duplicates another configured path",
            Some(target_id.to_string()),
            Some(path),
        ));
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
        value.replace_range(0..1, &value[0..1].to_ascii_uppercase());
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
        ProductType,
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
}
