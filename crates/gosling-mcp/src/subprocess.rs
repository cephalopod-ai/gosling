#[cfg(not(windows))]
use std::sync::OnceLock;
use tokio::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW_FLAG: u32 = 0x08000000;

pub trait SubprocessExt {
    fn set_no_window(&mut self) -> &mut Self;
}

impl SubprocessExt for Command {
    fn set_no_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            self.creation_flags(CREATE_NO_WINDOW_FLAG);
        }
        self
    }
}

impl SubprocessExt for std::process::Command {
    fn set_no_window(&mut self) -> &mut Self {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            self.creation_flags(CREATE_NO_WINDOW_FLAG);
        }
        self
    }
}

#[cfg(not(windows))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UnixShellFlavor {
    Posix,
    Nushell,
    Fish,
}

#[cfg(not(windows))]
fn unix_shell_flavor(shell: &str) -> UnixShellFlavor {
    let name = std::path::Path::new(shell)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(shell)
        .to_ascii_lowercase();

    match name.as_str() {
        "nu" | "nushell" => UnixShellFlavor::Nushell,
        "fish" => UnixShellFlavor::Fish,
        _ => UnixShellFlavor::Posix,
    }
}

#[cfg(not(windows))]
fn unix_login_shell_command_args(shell: &str) -> [&'static str; 4] {
    let probe = match unix_shell_flavor(shell) {
        UnixShellFlavor::Nushell => "print ($env.PATH | str join (char esep))",
        UnixShellFlavor::Fish => "printf '%s\\n' \"$PATH\"",
        UnixShellFlavor::Posix => "echo $PATH",
    };

    ["-l", "-i", "-c", probe]
}

#[cfg(not(windows))]
fn login_probe_shell_name(shell: &str, explicit: bool, flatpak: bool) -> Option<String> {
    if flatpak && !explicit {
        // Sandbox paths may not exist on the host; explicit overrides remain verbatim.
        Some(
            std::path::Path::new(shell)
                .file_name()?
                .to_str()?
                .to_string(),
        )
    } else {
        Some(shell.to_string())
    }
}

/// Resolve the user's full PATH by running a login shell.
///
/// When gosling is launched from a desktop app (e.g. Electron), it may inherit
/// a minimal PATH like `/usr/bin:/bin`. This function spawns a login shell to
/// source the user's profile and recover the full PATH.
///
/// Shared by Developer, hooks and computer control. Honor GOSLING_SHELL first,
/// otherwise source the user's login profile; Flatpak probes run on the host.
#[cfg(not(windows))]
fn resolve_login_shell_path() -> Option<String> {
    use process_wrap::std::{CommandWrap, ProcessSession};
    use std::path::PathBuf;
    use std::process::Stdio;

    let configured_shell = std::env::var("GOSLING_SHELL").ok();
    let is_flatpak = std::path::Path::new("/.flatpak-info").exists();
    let shell = configured_shell
        .clone()
        .or_else(|| {
            std::env::var("SHELL")
                .ok()
                .filter(|shell| PathBuf::from(shell).is_file())
        })
        .unwrap_or_else(|| {
            if PathBuf::from("/bin/bash").is_file() {
                "/bin/bash".to_string()
            } else {
                "sh".to_string()
            }
        });

    let shell = login_probe_shell_name(&shell, configured_shell.is_some(), is_flatpak)?;
    let mut command = if is_flatpak {
        let mut command = std::process::Command::new("flatpak-spawn");
        command.args(["--host", "--watch-bus"]).arg(&shell);
        command
    } else {
        std::process::Command::new(&shell)
    };
    command.args(unix_login_shell_command_args(&shell));
    let mut cmd = CommandWrap::from(command);
    cmd.command_mut()
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    // Spawn in a new session so that interactive shell job-control setup
    // cannot steal the terminal foreground from the parent gosling process.
    cmd.wrap(ProcessSession);

    let mut child = cmd.spawn().ok()?;
    capture_login_path(&mut *child, std::time::Duration::from_secs(5))
}

/// Capture a bounded login-PATH probe, killing and reaping it on failure or timeout.
#[cfg(not(windows))]
pub fn capture_login_path(
    child: &mut dyn process_wrap::std::ChildWrapper,
    timeout: std::time::Duration,
) -> Option<String> {
    use std::io::Read;
    use std::sync::mpsc::TryRecvError;
    use std::time::{Duration, Instant};

    const MAX_PATH_OUTPUT_BYTES: usize = 64 * 1024;
    let deadline = Instant::now() + timeout;
    let result = (|| {
        let stdout = child.stdout().take()?;
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = stdout
                .take((MAX_PATH_OUTPUT_BYTES + 1) as u64)
                .read_to_end(&mut bytes)
                .map(|_| bytes);
            let _ = tx.send(result);
        });

        let mut captured = None;
        loop {
            let status = child.try_wait().ok()?;
            if status.is_some_and(|status| !status.success()) {
                return None;
            }
            if captured.is_none() {
                match rx.try_recv() {
                    Ok(Ok(bytes)) if bytes.len() <= MAX_PATH_OUTPUT_BYTES => captured = Some(bytes),
                    Ok(_) | Err(TryRecvError::Disconnected) => return None,
                    Err(TryRecvError::Empty) => {}
                }
            }
            // EOF alone is insufficient: profiles may close stdout and then hang.
            if status.is_some() {
                if let Some(bytes) = captured {
                    return Some(bytes);
                }
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return None;
            }
            std::thread::sleep(remaining.min(Duration::from_millis(20)));
        }
    })();

    if result.is_none() {
        let _ = child.start_kill();
        let _ = child.wait();
    }
    let output = result?;

    // Take the last non-empty line — interactive shells may emit
    // extra output from profile scripts before our echo.
    String::from_utf8_lossy(&output)
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .filter(|path| !path.is_empty())
}

#[cfg(all(test, not(windows)))]
mod tests {
    use super::*;
    use process_wrap::std::{CommandWrap, ProcessSession};
    use std::process::Stdio;
    use std::time::{Duration, Instant};

    #[cfg(not(windows))]
    #[test]
    fn unix_shell_flavor_detects_nushell_names() {
        assert_eq!(unix_shell_flavor("nu"), UnixShellFlavor::Nushell);
        assert_eq!(unix_shell_flavor("nushell"), UnixShellFlavor::Nushell);
        assert_eq!(
            unix_shell_flavor("/etc/profiles/per-user/can/bin/nu"),
            UnixShellFlavor::Nushell
        );
        assert_eq!(unix_shell_flavor("/bin/bash"), UnixShellFlavor::Posix);
    }

    #[cfg(not(windows))]
    #[test]
    fn unix_login_shell_command_args_use_nushell_probe() {
        assert_eq!(
            unix_login_shell_command_args("nu"),
            ["-l", "-i", "-c", "print ($env.PATH | str join (char esep))"]
        );
        assert_eq!(
            unix_login_shell_command_args("/bin/bash"),
            ["-l", "-i", "-c", "echo $PATH"]
        );
        assert_eq!(
            unix_login_shell_command_args("/usr/bin/fish"),
            ["-l", "-i", "-c", "printf '%s\\n' \"$PATH\""]
        );
    }

    #[test]
    fn flatpak_auto_shell_uses_host_lookup_and_explicit_shell_stays_verbatim() {
        assert_eq!(
            login_probe_shell_name("/app/bin/bash", false, true).as_deref(),
            Some("bash")
        );
        assert_eq!(
            login_probe_shell_name("/custom/bin/nu", true, true).as_deref(),
            Some("/custom/bin/nu")
        );
        assert_eq!(
            login_probe_shell_name("/usr/bin/fish", false, false).as_deref(),
            Some("/usr/bin/fish")
        );
    }

    fn probe(script: &str, timeout: Duration) -> Option<String> {
        let mut command = std::process::Command::new("/bin/sh");
        command
            .args(["-c", script])
            .stdin(Stdio::null())
            .stdout(Stdio::piped());
        let mut wrapped = CommandWrap::from(command);
        wrapped.wrap(ProcessSession);
        let mut child = wrapped.spawn().unwrap();
        let result = capture_login_path(&mut *child, timeout);
        assert!(
            child.try_wait().unwrap().is_some(),
            "probe child must be reaped"
        );
        result
    }

    #[test]
    fn login_path_probe_keeps_the_last_nonempty_profile_line() {
        assert_eq!(
            probe(
                "printf 'profile greeting\\n/usr/bin:/bin\\n\\n'",
                Duration::from_secs(1)
            )
            .as_deref(),
            Some("/usr/bin:/bin")
        );
        assert!(probe("printf '/usr/bin\\n'; exit 1", Duration::from_secs(1)).is_none());
    }

    #[test]
    fn login_path_probe_times_out_even_after_stdout_closes() {
        let started = Instant::now();
        assert!(probe("exec 1>&-; exec sleep 30", Duration::from_millis(100)).is_none());
        assert!(started.elapsed() < Duration::from_secs(2));
    }

    #[test]
    fn login_path_probe_rejects_excessive_output() {
        assert!(probe("head -c 1048576 /dev/zero", Duration::from_secs(1)).is_none());
    }
}

/// Returns the user's full login shell PATH, resolved once and cached.
///
/// Call this before spawning subprocesses to ensure they inherit the user's
/// full PATH rather than the restricted one from the desktop app launcher.
#[cfg(not(windows))]
pub fn user_login_path() -> Option<&'static str> {
    // Cache failure as well so a hung profile is not retried by each consumer.
    static CACHED: OnceLock<Option<String>> = OnceLock::new();
    CACHED.get_or_init(resolve_login_shell_path).as_deref()
}

/// Merge the login shell PATH with the current process PATH.
///
/// Prepends login shell entries so user tools are found first, while
/// preserving any runtime PATH additions (e.g. from direnv, nix, or
/// auto-install helpers like ensure_peekaboo).
#[cfg(not(windows))]
pub fn merged_path() -> Option<String> {
    let login = user_login_path()?;
    let current = std::env::var("PATH").unwrap_or_default();
    if current.is_empty() {
        return Some(login.to_string());
    }
    // Deduplicate: login shell entries first, then any current entries not already present.
    let login_entries: Vec<&str> = login.split(':').collect();
    let mut seen: std::collections::HashSet<&str> = login_entries.iter().copied().collect();
    let mut merged = login_entries;
    for entry in current.split(':') {
        if seen.insert(entry) {
            merged.push(entry);
        }
    }
    Some(merged.join(":"))
}
