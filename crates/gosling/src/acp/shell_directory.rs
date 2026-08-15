use crate::acp::custom_requests::{
    ShellDirectoryReason, ShellDirectoryStatus, ShellDirectoryValidateResponse,
};
use std::path::{Path, PathBuf};

pub const MAX_SHELL_DIRECTORY_PATH_BYTES: usize = 4096;

fn invalid(reason: ShellDirectoryReason) -> ShellDirectoryValidateResponse {
    ShellDirectoryValidateResponse {
        status: ShellDirectoryStatus::Invalid,
        canonical_path: None,
        reason: Some(reason),
    }
}

fn unavailable(reason: ShellDirectoryReason) -> ShellDirectoryValidateResponse {
    ShellDirectoryValidateResponse {
        status: ShellDirectoryStatus::Unavailable,
        canonical_path: None,
        reason: Some(reason),
    }
}

/// Canonicalizes an operator-selected directory without creating, mutating, or activating anything.
///
/// The response never echoes the requested path: an inaccessible private location is reported only
/// as a stable reason code, and a canonical path is returned only for a directory the caller may
/// already reach.
pub fn canonicalize_shell_directory(raw: &str) -> ShellDirectoryValidateResponse {
    if raw.is_empty() || raw.contains('\0') {
        return invalid(ShellDirectoryReason::InvalidPath);
    }
    if raw.len() > MAX_SHELL_DIRECTORY_PATH_BYTES {
        return invalid(ShellDirectoryReason::PathTooLong);
    }
    let requested = PathBuf::from(raw);
    if !requested.is_absolute() {
        return invalid(ShellDirectoryReason::NotAbsolute);
    }
    let metadata = match std::fs::metadata(&requested) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return invalid(ShellDirectoryReason::NotFound)
        }
        Err(_) => return unavailable(ShellDirectoryReason::Inaccessible),
    };
    if !metadata.is_dir() {
        return invalid(ShellDirectoryReason::NotADirectory);
    }
    let Ok(canonical) = std::fs::canonicalize(&requested) else {
        return unavailable(ShellDirectoryReason::Inaccessible);
    };
    if std::fs::read_dir(&canonical).is_err() {
        return unavailable(ShellDirectoryReason::Inaccessible);
    }
    let Some(canonical) = canonical.to_str() else {
        return invalid(ShellDirectoryReason::InvalidPath);
    };
    if canonical.len() > MAX_SHELL_DIRECTORY_PATH_BYTES {
        return invalid(ShellDirectoryReason::PathTooLong);
    }
    ShellDirectoryValidateResponse {
        status: ShellDirectoryStatus::Valid,
        canonical_path: Some(canonical.to_string()),
        reason: None,
    }
}

/// Re-resolves a directory that main already accepted, immediately before a session is created.
pub fn accepted_shell_directory(path: &Path) -> Result<PathBuf, ShellDirectoryReason> {
    let raw = path.to_str().ok_or(ShellDirectoryReason::InvalidPath)?;
    let response = canonicalize_shell_directory(raw);
    match response.canonical_path {
        Some(canonical) => Ok(PathBuf::from(canonical)),
        None => Err(response.reason.unwrap_or(ShellDirectoryReason::InvalidPath)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn rejects_relative_empty_and_oversized_paths() {
        assert_eq!(
            canonicalize_shell_directory("relative/path").reason,
            Some(ShellDirectoryReason::NotAbsolute)
        );
        assert_eq!(
            canonicalize_shell_directory("").reason,
            Some(ShellDirectoryReason::InvalidPath)
        );
        assert_eq!(
            canonicalize_shell_directory("/tmp/\0/escape").reason,
            Some(ShellDirectoryReason::InvalidPath)
        );
        let oversized = format!("/{}", "a".repeat(MAX_SHELL_DIRECTORY_PATH_BYTES));
        assert_eq!(
            canonicalize_shell_directory(&oversized).reason,
            Some(ShellDirectoryReason::PathTooLong)
        );
    }

    #[test]
    fn rejects_missing_directories_and_files() {
        let directory = TempDir::new().unwrap();
        let file = directory.path().join("not-a-directory.txt");
        std::fs::write(&file, "x").unwrap();

        assert_eq!(
            canonicalize_shell_directory(directory.path().join("missing").to_str().unwrap()).reason,
            Some(ShellDirectoryReason::NotFound)
        );
        assert_eq!(
            canonicalize_shell_directory(file.to_str().unwrap()).reason,
            Some(ShellDirectoryReason::NotADirectory)
        );
    }

    #[test]
    fn resolves_symlink_aliases_to_one_canonical_directory() {
        let directory = TempDir::new().unwrap();
        let target = directory.path().join("target");
        std::fs::create_dir(&target).unwrap();
        let alias = directory.path().join("alias");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &alias).unwrap();
        #[cfg(not(unix))]
        std::fs::create_dir(&alias).unwrap();

        let accepted = canonicalize_shell_directory(alias.to_str().unwrap());
        assert_eq!(accepted.status, ShellDirectoryStatus::Valid);
        let canonical = accepted.canonical_path.unwrap();
        #[cfg(unix)]
        assert_eq!(
            canonical,
            std::fs::canonicalize(&target).unwrap().to_str().unwrap()
        );
        assert!(accepted_shell_directory(Path::new(&canonical)).is_ok());
    }

    #[test]
    fn validation_creates_nothing_on_disk() {
        let directory = TempDir::new().unwrap();
        let missing = directory.path().join("should-not-be-created");
        assert_eq!(
            canonicalize_shell_directory(missing.to_str().unwrap()).status,
            ShellDirectoryStatus::Invalid
        );
        assert!(!missing.exists());
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 0);
    }
}
