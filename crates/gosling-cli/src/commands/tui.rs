use anyhow::{anyhow, Context, Result};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

const TUI_SCRIPT_ENV: &str = "GOSLING_TUI_SCRIPT";
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

fn script_override(value: Option<OsString>) -> Result<Option<PathBuf>> {
    let Some(value) = value.filter(|v| !v.is_empty()) else {
        return Ok(None);
    };
    let path = PathBuf::from(value);
    if !path.is_file() {
        return Err(anyhow!(
            "{TUI_SCRIPT_ENV} is set to {}, which is not an existing file",
            path.display()
        ));
    }
    Ok(Some(path))
}

fn resolve_source() -> Result<TuiSource> {
    if let Some(script) = script_override(std::env::var_os(TUI_SCRIPT_ENV))? {
        return Ok(TuiSource::LocalScript(script));
    }
    if let Some(script) = find_local_script() {
        return Ok(TuiSource::LocalScript(script));
    }
    let spec = std::env::var(TUI_NPM_SPEC_ENV).unwrap_or_else(|_| DEFAULT_NPM_SPEC.to_string());
    Ok(TuiSource::Npx(spec))
}

fn launch_error(source: &TuiSource, descriptor: &str, err: std::io::Error) -> anyhow::Error {
    if err.kind() == std::io::ErrorKind::NotFound {
        let program = match source {
            TuiSource::LocalScript(_) => "node",
            TuiSource::Npx(_) => "npx",
        };
        return anyhow!(
            "`{program}` was not found on PATH; `gosling tui` needs Node.js installed to run ({descriptor})"
        );
    }
    anyhow!("failed to exec TUI ({descriptor}): {err}")
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
    let source = resolve_source()?;

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
        Err(launch_error(&source, &descriptor, err))
    }

    #[cfg(not(unix))]
    {
        let status = cmd
            .status()
            .map_err(|err| launch_error(&source, &descriptor, err))?;
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
    fn script_override_uses_an_existing_script() {
        let dir = tempfile::tempdir().unwrap();
        let script = dir.path().join("tui.js");
        std::fs::write(&script, "").unwrap();
        assert_eq!(
            script_override(Some(script.clone().into_os_string())).unwrap(),
            Some(script)
        );
    }

    #[test]
    fn script_override_rejects_a_missing_script() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing/tui.js");
        let err = script_override(Some(missing.into_os_string())).unwrap_err();
        assert!(err.to_string().contains("GOSLING_TUI_SCRIPT"), "{err}");
    }

    #[test]
    fn script_override_unset_or_empty_falls_through() {
        assert_eq!(script_override(None).unwrap(), None);
        assert_eq!(script_override(Some(OsString::new())).unwrap(), None);
    }

    #[test]
    fn launch_error_names_the_missing_runtime() {
        let local = TuiSource::LocalScript(PathBuf::from("tui.js"));
        let err = launch_error(
            &local,
            "node tui.js",
            std::io::Error::from(std::io::ErrorKind::NotFound),
        );
        assert!(
            err.to_string().contains("`node` was not found on PATH"),
            "{err}"
        );

        let npx = TuiSource::Npx(DEFAULT_NPM_SPEC.to_string());
        let err = launch_error(
            &npx,
            "npx",
            std::io::Error::from(std::io::ErrorKind::NotFound),
        );
        assert!(
            err.to_string().contains("`npx` was not found on PATH"),
            "{err}"
        );

        let err = launch_error(
            &local,
            "node tui.js",
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        assert!(
            err.to_string()
                .starts_with("failed to exec TUI (node tui.js)"),
            "{err}"
        );
    }

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
