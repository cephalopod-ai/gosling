use anyhow::{anyhow, Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const TUI_NPM_SPEC_ENV: &str = "GOSLING_TUI_NPM_SPEC";
const TUI_REL_PATH: &str = "ui/text/dist/tui.js";
const DEFAULT_NPM_SPEC: &str = "@repo-makeover/gosling@latest";
const NPM_BIN_NAME: &str = "gosling-tui";

enum TuiSource {
    LocalScript(PathBuf),
    Npx(String),
}

fn find_local_script() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let exe_dir = exe.parent().unwrap_or_else(|| Path::new("."));

    let mut dir = Some(exe_dir.to_path_buf());
    for _ in 0..6 {
        if let Some(d) = dir.clone() {
            let candidate = d.join(TUI_REL_PATH);
            if candidate.is_file() {
                return Some(candidate);
            }
            dir = d.parent().map(Path::to_path_buf);
        }
    }

    if let Ok(cwd) = std::env::current_dir() {
        let candidate = cwd.join(TUI_REL_PATH);
        if candidate.is_file() && is_gosling_workspace_root(&cwd) {
            return Some(candidate);
        }
    }

    None
}

/// Whether `dir` looks like the root of an actual gosling source checkout,
/// not just some directory that happens to contain a file at
/// `ui/text/dist/tui.js`. Without this, `gosling tui` run from any
/// directory containing an attacker-supplied `ui/text/dist/tui.js` (e.g. an
/// extracted archive or a cloned repo) would exec `node <that file>`
/// unprompted, inheriting the invoking process's full environment
/// (including any exported provider API keys). Checking for a `[workspace]`
/// `Cargo.toml` whose member actually declares the `gosling` package name
/// is a much higher bar for an attacker to spoof than dropping one JS file.
fn is_gosling_workspace_root(dir: &Path) -> bool {
    let Ok(cargo_toml) = std::fs::read_to_string(dir.join("Cargo.toml")) else {
        return false;
    };
    if !cargo_toml.contains("[workspace]") {
        return false;
    }
    std::fs::read_to_string(dir.join("crates/gosling/Cargo.toml"))
        .is_ok_and(|member| member.lines().any(|l| l.trim() == "name = \"gosling\""))
}

fn resolve_source() -> TuiSource {
    if let Some(script) = find_local_script() {
        return TuiSource::LocalScript(script);
    }
    let spec = std::env::var(TUI_NPM_SPEC_ENV).unwrap_or_else(|_| DEFAULT_NPM_SPEC.to_string());
    TuiSource::Npx(spec)
}

fn build_command(source: &TuiSource, args: &[String]) -> Result<Command> {
    match source {
        TuiSource::LocalScript(script) => {
            let mut cmd = Command::new("node");
            cmd.arg(script).args(args);
            Ok(cmd)
        }
        TuiSource::Npx(spec) => {
            let mut cmd = Command::new("npx");
            cmd.arg("--yes")
                .arg("--package")
                .arg(spec)
                .arg("--")
                .arg(NPM_BIN_NAME)
                .args(args);
            Ok(cmd)
        }
    }
}

pub fn handle_tui(args: Vec<String>) -> Result<()> {
    let source = resolve_source();

    let gosling_binary = std::env::current_exe()
        .context("could not determine current gosling executable to expose as GOSLING_BINARY")?;

    let mut cmd = build_command(&source, &args)?;
    cmd.env("GOSLING_BINARY", &gosling_binary);

    let descriptor = match &source {
        TuiSource::LocalScript(p) => format!("node {}", p.display()),
        TuiSource::Npx(spec) => format!("npx --package {} -- {}", spec, NPM_BIN_NAME),
    };

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        let err = cmd.exec();
        Err(anyhow!("failed to exec TUI ({descriptor}): {err}"))
    }

    #[cfg(not(unix))]
    {
        let status = cmd
            .status()
            .with_context(|| format!("failed to run `{descriptor}`"))?;
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_gosling_workspace_root_rejects_directory_with_no_cargo_toml() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_gosling_workspace_root(dir.path()));
    }

    #[test]
    fn is_gosling_workspace_root_rejects_attacker_supplied_tui_js_with_no_workspace() {
        // Simulates the actual attack this check exists to prevent: an
        // archive/repo that contains only a `ui/text/dist/tui.js` file (the
        // relative path find_local_script looks for) and nothing that
        // proves it's a real gosling checkout.
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("ui/text/dist")).unwrap();
        std::fs::write(
            dir.path().join("ui/text/dist/tui.js"),
            "console.log('not the real tui')",
        )
        .unwrap();
        assert!(!is_gosling_workspace_root(dir.path()));
    }

    #[test]
    fn is_gosling_workspace_root_rejects_unrelated_cargo_workspace() {
        // A [workspace] Cargo.toml alone (e.g. an unrelated Rust monorepo
        // that happens to sit at the attacker-controlled cwd) must not be
        // enough; the gosling package itself must be present.
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();
        assert!(!is_gosling_workspace_root(dir.path()));
    }

    #[test]
    fn is_gosling_workspace_root_accepts_real_workspace_layout() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[workspace]\nmembers = [\"crates/*\"]\n",
        )
        .unwrap();
        std::fs::create_dir_all(dir.path().join("crates/gosling")).unwrap();
        std::fs::write(
            dir.path().join("crates/gosling/Cargo.toml"),
            "[package]\nname = \"gosling\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        assert!(is_gosling_workspace_root(dir.path()));
    }
}
