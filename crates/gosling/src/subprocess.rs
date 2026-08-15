use std::time::Duration;
use tokio::process::Command;

#[cfg(windows)]
const CREATE_NO_WINDOW_FLAG: u32 = 0x08000000;

#[cfg(target_os = "linux")]
fn configure_parent_death_signal(command: &mut Command) {
    let parent_pid = unsafe { libc::getpid() };

    unsafe {
        command.pre_exec(move || {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(std::io::Error::last_os_error());
            }

            if libc::getppid() != parent_pid {
                return Err(std::io::Error::from_raw_os_error(libc::ESRCH));
            }

            Ok(())
        });
    }
}

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

#[allow(unused_variables)]
fn configure_subprocess_with_process_group(command: &mut Command, isolate_process_group: bool) {
    // Kill the child when its handle is dropped (graceful shutdown, agent eviction
    // from the session LRU, or extension reconfigure) so MCP servers and spawned
    // provider CLIs don't leak. On Linux this is backstopped by PR_SET_PDEATHSIG
    // below for abnormal parent death; macOS has no in-process equivalent, so a
    // hard parent SIGKILL can still orphan children.
    command.kill_on_drop(true);
    // Most subprocesses are isolated so they do not receive SIGINT when the
    // user presses Ctrl+C in the terminal. A shell-owned adapter instead stays
    // in its backend's group so forced desktop cleanup can terminate the whole
    // backend tree on platforms without parent-death signals.
    #[cfg(unix)]
    if isolate_process_group {
        command.process_group(0);
    }
    #[cfg(target_os = "linux")]
    configure_parent_death_signal(command);
    command.set_no_window();
}

pub fn configure_subprocess(command: &mut Command) {
    configure_subprocess_with_process_group(command, true);
}

pub fn configure_shell_owned_subprocess(command: &mut Command) {
    configure_subprocess_with_process_group(command, false);
}

#[cfg(unix)]
pub async fn process_is_alive(pid: u32) -> bool {
    // SAFETY: kill(pid, 0) only probes for the process's existence; it sends no signal.
    let result = unsafe { libc::kill(pid as libc::pid_t, 0) };
    result == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

#[cfg(windows)]
pub async fn process_is_alive(pid: u32) -> bool {
    let mut command = Command::new("tasklist");
    command
        .args(["/FI", &format!("PID eq {pid}"), "/NH"])
        .set_no_window();
    match command.output().await {
        // Fail open: if we can't determine liveness, assume the process is still
        // alive rather than risk shutting ourselves down on a transient error.
        Ok(output) => String::from_utf8_lossy(&output.stdout).contains(&pid.to_string()),
        Err(_) => true,
    }
}

/// Resolves once the process identified by `pid` is no longer running.
///
/// Used to detect when a supervising process (e.g. the Electron app that
/// launched `goslingd`) has died without giving this process a chance to shut
/// down gracefully, so it can self-terminate instead of surviving as an
/// orphan. This is a polling fallback for platforms (macOS, Windows) with no
/// in-process parent-death notification equivalent to Linux's
/// `PR_SET_PDEATHSIG`.
pub async fn wait_for_process_exit(pid: u32, poll_interval: Duration) {
    while process_is_alive(pid).await {
        tokio::time::sleep(poll_interval).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Stdio;

    #[cfg(unix)]
    #[tokio::test]
    async fn wait_for_process_exit_resolves_promptly_after_the_process_dies() {
        let mut child = Command::new("sleep")
            .arg("30")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("spawn sleep helper");
        let pid = child.id().expect("child pid");

        assert!(
            process_is_alive(pid).await,
            "freshly spawned process should be alive"
        );

        let poll_interval = Duration::from_millis(20);
        let wait_future = wait_for_process_exit(pid, poll_interval);
        tokio::pin!(wait_future);
        assert!(
            tokio::time::timeout(Duration::from_millis(200), &mut wait_future)
                .await
                .is_err(),
            "wait_for_process_exit resolved before the process was killed"
        );

        child.kill().await.expect("kill sleep helper");
        child.wait().await.expect("reap sleep helper");

        tokio::time::timeout(Duration::from_secs(2), wait_future)
            .await
            .expect("wait_for_process_exit did not resolve after the process exited");

        assert!(
            !process_is_alive(pid).await,
            "reaped process should no longer be reported alive"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn process_is_alive_is_false_for_an_unused_pid() {
        // pid 1 is always alive (init/launchd); reusing the reaped test-helper
        // pid space is flaky, so instead assert the negative directly against a
        // pid that cannot belong to a live process: i32::MAX is never a valid pid.
        assert!(!process_is_alive(i32::MAX as u32).await);
    }
}
