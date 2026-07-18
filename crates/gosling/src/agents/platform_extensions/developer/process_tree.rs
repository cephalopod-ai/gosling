pub(super) struct ProcessTreeGuard {
    root_pid: Option<u32>,
    armed: bool,
}

impl ProcessTreeGuard {
    pub(super) fn new(root_pid: Option<u32>) -> Self {
        Self {
            root_pid,
            armed: true,
        }
    }

    pub(super) fn terminate(&mut self) {
        if self.armed {
            terminate_process_tree(self.root_pid);
            self.armed = false;
        }
    }
}

impl Drop for ProcessTreeGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(unix)]
fn terminate_process_tree(root_pid: Option<u32>) {
    let Some(root_pid) = root_pid.and_then(|pid| i32::try_from(pid).ok()) else {
        return;
    };
    unsafe {
        libc::kill(-root_pid, libc::SIGKILL);
    }
}

#[cfg(windows)]
fn terminate_process_tree(root_pid: Option<u32>) {
    use std::os::windows::process::CommandExt;

    let Some(root_pid) = root_pid else { return };
    let root_pid = root_pid.to_string();
    let _ = std::process::Command::new("taskkill")
        .args(["/PID", root_pid.as_str(), "/T", "/F"])
        .creation_flags(0x08000000)
        .spawn();
}

#[cfg(not(any(unix, windows)))]
fn terminate_process_tree(_root_pid: Option<u32>) {}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn process_exists(pid: i32) -> bool {
        unsafe { libc::kill(pid, 0) == 0 }
    }

    #[tokio::test]
    async fn dropping_guard_kills_background_descendants() {
        let directory = tempfile::tempdir().unwrap();
        let pid_file = directory.path().join("background.pid");
        let mut command = tokio::process::Command::new("sh");
        command
            .arg("-c")
            .arg(format!(
                "sleep 30 & echo $! > '{}' && wait",
                pid_file.display()
            ))
            .process_group(0)
            .kill_on_drop(true);
        let mut child = command.spawn().unwrap();
        let guard = ProcessTreeGuard::new(child.id());

        let deadline = Instant::now() + Duration::from_secs(2);
        while !pid_file.exists() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let descendant_pid: i32 = std::fs::read_to_string(&pid_file)
            .unwrap()
            .trim()
            .parse()
            .unwrap();

        drop(guard);
        let _ = tokio::time::timeout(Duration::from_secs(2), child.wait()).await;
        let deadline = Instant::now() + Duration::from_secs(2);
        while process_exists(descendant_pid) && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(!process_exists(descendant_pid));
    }
}
