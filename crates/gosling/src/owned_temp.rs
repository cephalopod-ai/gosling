use fs2::FileExt;
use std::fs::{self, File, OpenOptions};
use std::io;
use std::path::Path;

const OWNER_LOCK: &str = ".gosling-owner.lock";

/// A private staging directory whose OS lock survives for exactly its live owner.
/// Field order closes the lock before TempDir removes it on Windows.
pub(crate) struct OwnedTempDir {
    _owner: File,
    directory: tempfile::TempDir,
}

impl OwnedTempDir {
    pub(crate) fn new_in(root: &Path, prefix: &str) -> io::Result<Self> {
        fs::create_dir_all(root)?;
        let _publication_lock = lock_root(root)?;
        sweep_entries(root, prefix);
        let directory = tempfile::Builder::new()
            .prefix(&format!("{prefix}{}-", std::process::id()))
            .tempdir_in(root)?;
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let owner = options.open(directory.path().join(OWNER_LOCK))?;
        owner.lock_exclusive()?;
        Ok(Self {
            _owner: owner,
            directory,
        })
    }

    pub(crate) fn path(&self) -> &Path {
        self.directory.path()
    }
}

pub(crate) fn sweep(root: &Path, prefix: &str) {
    let Ok(_publication_lock) = lock_root(root) else {
        return;
    };
    sweep_entries(root, prefix);
}

fn lock_root(root: &Path) -> io::Result<File> {
    if !fs::symlink_metadata(root)
        .is_ok_and(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
    {
        return Err(io::Error::other("Staging root must be a real directory"));
    }
    let marker = root.join(".gosling-staging-publication.lock");
    if fs::symlink_metadata(&marker).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(io::Error::other(
            "Staging publication lock must not be a symlink",
        ));
    }
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let lock = options.open(marker)?;
    // Sweepers cannot observe a directory until its owner lock is held.
    lock.lock_exclusive()?;
    Ok(lock)
}

fn sweep_entries(root: &Path, prefix: &str) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        if !entry.file_name().to_string_lossy().starts_with(prefix)
            || !entry.file_type().is_ok_and(|kind| kind.is_dir())
        {
            continue;
        }
        let marker = entry.path().join(OWNER_LOCK);
        if !fs::symlink_metadata(&marker)
            .is_ok_and(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
        {
            continue;
        }
        let Ok(owner) = OpenOptions::new().read(true).write(true).open(&marker) else {
            continue;
        };
        if owner.try_lock_exclusive().is_err() {
            continue;
        }
        // A staging owner never reopens an existing directory. Only sweepers can
        // contend here, so closing our handle before Windows deletion is safe.
        drop(owner);
        if let Err(error) = fs::remove_dir_all(entry.path()) {
            tracing::warn!(path = %entry.path().display(), %error, "Could not remove abandoned Gosling staging directory");
        }
    }
}

pub(crate) const EXTENSION_PREFIX: &str = ".gosling-extension-staging-";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sweep_preserves_live_and_unowned_directories_and_removes_crash_residue() {
        let root = tempfile::tempdir().unwrap();
        let live = OwnedTempDir::new_in(root.path(), EXTENSION_PREFIX).unwrap();
        let abandoned = root.path().join(format!("{EXTENSION_PREFIX}crashed"));
        fs::create_dir(&abandoned).unwrap();
        fs::write(abandoned.join(OWNER_LOCK), "").unwrap();
        fs::write(abandoned.join("test.env"), "test-only-secret").unwrap();
        let unowned = root.path().join(format!("{EXTENSION_PREFIX}unowned"));
        fs::create_dir(&unowned).unwrap();
        sweep(root.path(), EXTENSION_PREFIX);
        assert!(live.path().is_dir());
        assert!(unowned.is_dir());
        assert!(!abandoned.exists());
    }

    #[cfg(unix)]
    #[test]
    fn sweep_does_not_follow_a_replaced_root() {
        let root = tempfile::tempdir().unwrap();
        let outside = root.path().join("outside");
        let abandoned = outside.join(format!("{EXTENSION_PREFIX}crashed"));
        fs::create_dir_all(&abandoned).unwrap();
        fs::write(abandoned.join(OWNER_LOCK), "").unwrap();
        let link = root.path().join("linked-root");
        std::os::unix::fs::symlink(&outside, &link).unwrap();
        sweep(&link, EXTENSION_PREFIX);
        assert!(abandoned.is_dir());
    }
}
