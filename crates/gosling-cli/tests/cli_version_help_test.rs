//! `--version` names the program, and `session diagnostics` / `shell-validate` describe their
//! options (GSL-PT-20260927-A15).

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

fn help(args: &[&str]) -> String {
    let root = TempDir::new().unwrap();
    let output = gosling(&root, args);
    assert!(output.status.success(), "{args:?} failed");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn version_output_names_the_program() {
    let root = TempDir::new().unwrap();
    let expected = format!("gosling {}\n", env!("CARGO_PKG_VERSION"));
    for flag in ["--version", "-V"] {
        let output = gosling(&root, &[flag]);
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout), expected, "{flag}");
    }
}

#[test]
fn session_diagnostics_help_describes_the_command_and_output() {
    let session_help = help(&["session", "--help"]);
    let diagnostics_line = session_help
        .lines()
        .find(|line| line.trim_start().starts_with("diagnostics"))
        .expect("session --help lists diagnostics");
    assert!(
        diagnostics_line.contains("diagnostics report"),
        "{diagnostics_line}"
    );

    let diagnostics_help = help(&["session", "diagnostics", "--help"]);
    assert!(
        diagnostics_help.starts_with("Write a JSON diagnostics report for a session"),
        "{diagnostics_help}"
    );
    assert!(
        diagnostics_help.contains("-o, --output <FILE>")
            && diagnostics_help.contains("default: diagnostics_<session_id>.json"),
        "{diagnostics_help}"
    );
}

#[test]
fn shell_validate_help_describes_every_option() {
    let shell_help = help(&["shell-validate", "--help"]);
    for option in [
        "--shell-id <ID>",
        "--shell-display-name <NAME>",
        "--shell-version <VERSION>",
        "--shell-provisioning <PATH>",
        "--with-builtin <NAME>",
    ] {
        let line_index = shell_help
            .lines()
            .position(|line| line.trim_start().starts_with(option))
            .unwrap_or_else(|| panic!("{option} missing from:\n{shell_help}"));
        let description = shell_help
            .lines()
            .nth(line_index + 1)
            .unwrap_or_default()
            .trim();
        assert!(
            !description.is_empty() && !description.starts_with('-'),
            "{option} has no description:\n{shell_help}"
        );
    }
    assert!(shell_help.contains("Prints a JSON report and exits non-zero"));
}
