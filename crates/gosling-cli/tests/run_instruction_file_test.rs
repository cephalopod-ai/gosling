//! `gosling run -i <file>` only claims the file is missing when it is; other read failures name
//! the file and the reason (GSL-PT-20260927-E10).

use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

const NOT_FOUND: &str = "Instruction file not found — did you mean to use gosling run --text?";

fn run_with_instructions(root: &TempDir, file: &Path) -> Output {
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(["run", "-i"])
        .arg(file)
        .env("HOME", &home)
        .env("GOSLING_PATH_ROOT", root.path().join("root"))
        .env("GOSLING_DISABLE_KEYRING", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run gosling binary")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn unreadable_instruction_files_are_not_reported_as_missing() {
    let root = TempDir::new().unwrap();
    let invalid_utf8 = root.path().join("invalid.bin");
    std::fs::write(&invalid_utf8, [0xff, 0xfe, 0xfd]).unwrap();
    let directory = root.path().join("a-directory");
    std::fs::create_dir(&directory).unwrap();

    for (file, reason) in [(&invalid_utf8, "valid UTF-8"), (&directory, "directory")] {
        let output = run_with_instructions(&root, file);
        let stderr = stderr(&output);
        assert_eq!(output.status.code(), Some(1), "{stderr}");
        assert!(!stderr.contains(NOT_FOUND), "{stderr}");
        assert!(
            stderr.contains(&format!(
                "Could not read instruction file {}:",
                file.display()
            )) && stderr.contains(reason),
            "{stderr}"
        );
    }
}

#[test]
fn a_missing_instruction_file_keeps_the_text_hint() {
    let root = TempDir::new().unwrap();
    let output = run_with_instructions(&root, &root.path().join("missing.md"));
    let stderr = stderr(&output);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(stderr.contains(NOT_FOUND), "{stderr}");
}
