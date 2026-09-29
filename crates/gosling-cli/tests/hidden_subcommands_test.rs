//! Hidden internal subcommands stay out of shell completion scripts and typo suggestions, but
//! still run when invoked by name (GSL-PT-20260927-A19).

use std::process::{Command, Output};
use tempfile::TempDir;

const HIDDEN: [&str; 2] = ["session-history-mcp", "validate-extensions"];

fn gosling(args: &[&str]) -> Output {
    let root = TempDir::new().unwrap();
    let home = root.path().join("home");
    std::fs::create_dir_all(&home).unwrap();
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(args)
        .env("HOME", &home)
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run gosling binary")
}

#[test]
fn completion_scripts_omit_hidden_subcommands() {
    for shell in ["bash", "zsh", "fish", "nu", "elvish", "powershell"] {
        let output = gosling(&["completion", shell]);
        assert!(output.status.success(), "completion {shell} failed");
        let script = String::from_utf8(output.stdout).unwrap();
        for hidden in HIDDEN {
            assert!(!script.contains(hidden), "{shell} script offers {hidden}");
        }
        for visible in ["session", "completion", "term"] {
            assert!(script.contains(visible), "{shell} script lost {visible}");
        }
    }
}

#[test]
fn bash_completion_omits_hidden_term_log() {
    let output = gosling(&["completion", "bash"]);
    let script = String::from_utf8(output.stdout).unwrap();
    assert!(
        !script.contains("term__subcmd__log"),
        "bash offers term log"
    );
    assert!(script.contains("term__subcmd__init"));
}

#[test]
fn typo_suggestions_omit_hidden_subcommands() {
    let output = gosling(&["sesion"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("'session'"), "{stderr}");
    assert!(!stderr.contains("session-history-mcp"), "{stderr}");

    let output = gosling(&["validate-extension"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("unrecognized subcommand"), "{stderr}");
    assert!(!stderr.contains("validate-extensions"), "{stderr}");
    assert!(!stderr.contains("tip: some similar"), "{stderr}");
}

#[test]
fn hidden_subcommands_still_run_by_name() {
    let output = gosling(&["validate-extensions", "--help"]);
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("bundled-extensions.json"), "{stdout}");
}
