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
