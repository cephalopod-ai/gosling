use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

fn gosling(root: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(args)
        .env("GOSLING_PATH_ROOT", root)
        .env("HOME", home)
        .env("GOSLING_DISABLE_KEYRING", "1")
        .output()
        .expect("failed to run gosling binary")
}

fn files_under(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .flat_map(|entry| {
            let path = entry.path();
            if path.is_dir() {
                files_under(&path)
            } else {
                vec![path]
            }
        })
        .collect()
}

#[test]
fn help_version_and_usage_errors_leave_the_root_untouched() {
    let root = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();

    for args in [
        &["--version"][..],
        &["--help"],
        &["help"],
        &["session", "--help"],
        &["--no-such-flag"],
    ] {
        gosling(root.path(), home.path(), args);
        let created = fs::read_dir(root.path()).unwrap().count();
        assert_eq!(
            created,
            0,
            "`gosling {}` created {:?}",
            args.join(" "),
            files_under(root.path())
        );
    }
}

#[test]
fn commands_still_write_their_log_file() {
    let root = TempDir::new().unwrap();
    let home = TempDir::new().unwrap();

    let output = gosling(root.path(), home.path(), &["info"]);

    assert!(output.status.success(), "{output:?}");
    let logs = files_under(&root.path().join("state").join("logs").join("cli"));
    assert!(
        logs.iter().any(|path| {
            path.extension().is_some_and(|ext| ext == "log")
                && fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0)
        }),
        "{logs:?}"
    );
}
