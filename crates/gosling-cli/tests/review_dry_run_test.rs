//! `gosling review --dry-run` shows the prompts the chosen options would send
//! (GSL-PT-20260927-D26).

use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

const CHECK_MARKER: &str = "SEC-ROOT-MARKER";
const INSTRUCTIONS_MARKER: &str = "INSTR-MARKER-ZZ";
const MAIN_PASS_MARKER: &str = "Path: `src/lib.rs`";

fn git(repo: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args([
            "-c",
            "user.name=playtest",
            "-c",
            "user.email=playtest@example.invalid",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(repo)
        .env("HOME", repo)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .status()
        .expect("git");
    assert!(status.success(), "git {args:?}");
}

fn fixture_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let repo = dir.path().join("repo");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    std::fs::create_dir_all(repo.join(".agents/checks")).unwrap();
    std::fs::write(repo.join("src/lib.rs"), "pub fn a() {}\n").unwrap();
    std::fs::write(
        repo.join(".agents/checks/security.md"),
        format!("---\nname: security\ndescription: root security\n---\n{CHECK_MARKER}\n"),
    )
    .unwrap();
    git(&repo, &["init", "-q"]);
    git(&repo, &["add", "."]);
    git(&repo, &["commit", "-q", "-m", "base"]);
    std::fs::write(repo.join("src/lib.rs"), "pub fn a() { let _ = 1; }\n").unwrap();
    dir
}

fn review(dir: &TempDir, args: &[&str]) -> String {
    let home = dir.path().join("home");
    let root = dir.path().join("root");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::create_dir_all(&root).unwrap();
    let output: Output = Command::new(env!("CARGO_BIN_EXE_gosling"))
        .arg("review")
        .arg("--dry-run")
        .args(args)
        .current_dir(dir.path().join("repo"))
        .env("HOME", &home)
        .env("GOSLING_PATH_ROOT", &root)
        .env("GOSLING_DISABLE_KEYRING", "1")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .stdin(std::process::Stdio::null())
        .output()
        .expect("failed to run gosling binary");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn default_dry_run_shows_main_pass_and_check_prompts() {
    let dir = fixture_repo();
    let out = review(&dir, &[]);
    assert!(out.contains(MAIN_PASS_MARKER), "{out}");
    assert!(out.contains(CHECK_MARKER), "{out}");
    assert!(
        out.contains("# orchestrator: main pass would run 1 subprocess(es), one per touched file"),
        "{out}"
    );
    assert!(
        out.contains("# orchestrator: 1 check(s) would run as parallel subprocesses"),
        "{out}"
    );
}

#[test]
fn checks_only_dry_run_has_no_main_pass() {
    let dir = fixture_repo();
    for args in [
        &["--checks-only"][..],
        &["--checks-only", "--no-orchestrate"][..],
    ] {
        let out = review(&dir, args);
        assert!(!out.contains(MAIN_PASS_MARKER), "{args:?}: {out}");
        assert!(out.contains(CHECK_MARKER), "{args:?}: {out}");
        assert!(
            out.contains("# orchestrator: main pass skipped (--checks-only)"),
            "{args:?}: {out}"
        );
        assert!(!out.contains("main pass would run"), "{args:?}: {out}");
    }
}

#[test]
fn instructions_appear_in_every_dry_run_prompt() {
    let dir = fixture_repo();
    let out = review(&dir, &["-i", INSTRUCTIONS_MARKER]);
    assert_eq!(out.matches(INSTRUCTIONS_MARKER).count(), 2, "{out}");
    assert_ne!(out, review(&dir, &[]));
}

#[test]
fn no_orchestrate_dry_run_still_prints_the_single_prompt() {
    let dir = fixture_repo();
    let out = review(&dir, &["--no-orchestrate", "-i", INSTRUCTIONS_MARKER]);
    assert!(out.contains(CHECK_MARKER), "{out}");
    assert!(out.contains(INSTRUCTIONS_MARKER), "{out}");
    assert!(!out.contains("# orchestrator:"), "{out}");
}

#[cfg(unix)]
#[test]
fn review_collects_git_once_and_summary_skips_the_patch() {
    use std::os::unix::fs::PermissionsExt;

    let dir = fixture_repo();
    let repo = dir.path().join("repo");
    std::fs::write(repo.join("new.txt"), "untracked content\n").unwrap();
    let real_git = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .unwrap();
    assert!(real_git.status.success());
    let real_git = String::from_utf8(real_git.stdout)
        .unwrap()
        .trim()
        .to_string();
    let bin = dir.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let wrapper = bin.join("git");
    std::fs::write(&wrapper, "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$GOSLING_TEST_GIT_CALLS\"\nexec \"$GOSLING_TEST_REAL_GIT\" \"$@\"\n").unwrap();
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = std::env::join_paths(
        std::iter::once(bin).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
    )
    .unwrap();
    let expected_summary = Command::new(&real_git)
        .args(["-c", "core.quotePath=off", "diff", "--stat", "HEAD"])
        .current_dir(&repo)
        .env("HOME", dir.path())
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .output()
        .unwrap()
        .stdout;

    for summary in [false, true] {
        for explicit_range in [false, true] {
            let log = dir.path().join(format!("calls-{summary}-{explicit_range}"));
            let mut command = Command::new(env!("CARGO_BIN_EXE_gosling"));
            command.args(["review", "--dry-run", "--quiet"]);
            if summary {
                command.arg("--summary-only");
            }
            if explicit_range {
                command.arg("HEAD");
            }
            let output = command
                .current_dir(&repo)
                .env("PATH", &path)
                .env("GOSLING_TEST_GIT_CALLS", &log)
                .env("GOSLING_TEST_REAL_GIT", &real_git)
                .env("HOME", dir.path())
                .env("GOSLING_PATH_ROOT", dir.path().join("root"))
                .env("GOSLING_DISABLE_KEYRING", "1")
                .env("GIT_CONFIG_NOSYSTEM", "1")
                .stdin(std::process::Stdio::null())
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            let calls = std::fs::read_to_string(log).unwrap();
            assert_eq!(
                calls.lines().count(),
                if explicit_range { 2 } else { 3 },
                "{calls}"
            );
            assert!(!calls.contains("--name-only"), "{calls}");
            if summary {
                assert!(!calls.contains("--patch"), "{calls}");
                assert!(calls.contains("--stat"), "{calls}");
                assert_eq!(output.stdout, expected_summary);
            } else {
                assert!(calls.contains("--raw -z --patch"), "{calls}");
                let prompt = String::from_utf8(output.stdout).unwrap();
                assert_eq!(prompt.contains("untracked content"), !explicit_range);
            }
        }
    }
}
