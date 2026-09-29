//! `gosling session import` reports the working directory only for a completed import, and a
//! failed import names the file and the formats it tried (GSL-PT-20260927-C12, GSL-PT-20260927-D13).

use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

const WORKING_DIR_LINE: &str = "Imported session working directory:";

const PI_TRANSCRIPT: &str = r#"{"type":"session","version":3,"id":"s","timestamp":"2024-12-03T14:00:00.000Z","cwd":"/w"}
{"type":"message","id":"a","parentId":null,"timestamp":"2024-12-03T14:00:01.000Z","message":{"role":"user","content":"hello"}}
"#;

fn import(root: &TempDir, file: &Path) -> Output {
    let home = root.path().join("home");
    let working_dir = root.path().join("wd");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&working_dir).unwrap();
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(["session", "import"])
        .arg(file)
        .arg("--working-dir")
        .arg(&working_dir)
        .env("HOME", &home)
        .env("GOSLING_PATH_ROOT", root.path().join("root"))
        .env("GOSLING_DISABLE_KEYRING", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run gosling binary")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn malformed_files_fail_with_file_and_format_context() {
    let root = TempDir::new().unwrap();
    let cases = [
        (
            "truncated.json",
            "{\"id\": \"x\", \"name\": \"trunc",
            "EOF while parsing",
        ),
        ("notsession.json", "{\"hello\": 1}", "missing field"),
        ("empty.json", "", "EOF while parsing"),
    ];
    for (name, content, cause) in cases {
        let file = root.path().join(name);
        std::fs::write(&file, content).unwrap();
        let output = import(&root, &file);
        let stdout = text(&output.stdout);
        let stderr = text(&output.stderr);
        assert!(!output.status.success(), "{name} imported: {stdout}");
        assert!(
            !stdout.contains(WORKING_DIR_LINE),
            "{name} announced a working directory: {stdout}"
        );
        assert!(
            stderr.contains(&format!("Could not import {}:", file.display())),
            "{name} stderr: {stderr}"
        );
        assert!(
            stderr.contains("gosling, Claude Code, Codex or Pi"),
            "{name} stderr does not name the formats tried: {stderr}"
        );
        assert!(stderr.contains(cause), "{name} lost the cause: {stderr}");
    }
}

#[test]
fn missing_file_error_names_the_file() {
    let root = TempDir::new().unwrap();
    let file = root.path().join("does-not-exist.json");
    let output = import(&root, &file);
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
    assert!(!output.status.success());
    assert!(!stdout.contains(WORKING_DIR_LINE), "{stdout}");
    assert!(
        stderr.contains(&format!("Could not import {}:", file.display()))
            && stderr.contains("No such file or directory"),
        "{stderr}"
    );
    assert!(!stderr.contains("Claude Code"), "{stderr}");
}

#[test]
fn working_directory_is_reported_only_when_the_import_happens() {
    let root = TempDir::new().unwrap();
    let file = root.path().join("pi.jsonl");
    std::fs::write(&file, PI_TRANSCRIPT).unwrap();

    let first = import(&root, &file);
    let first_stdout = text(&first.stdout);
    assert!(
        first.status.success(),
        "{first_stdout}{}",
        text(&first.stderr)
    );
    assert!(first_stdout.contains("Session imported:"), "{first_stdout}");
    assert!(first_stdout.contains(WORKING_DIR_LINE), "{first_stdout}");

    let second = import(&root, &file);
    let second_stdout = text(&second.stdout);
    assert!(second.status.success(), "{}", text(&second.stderr));
    assert!(
        second_stdout.contains("Session already imported"),
        "{second_stdout}"
    );
    assert!(
        !second_stdout.contains(WORKING_DIR_LINE),
        "an already-imported session announced a working directory: {second_stdout}"
    );
}
