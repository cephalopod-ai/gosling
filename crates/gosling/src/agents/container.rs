#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Container {
    /// The Docker container ID
    id: String,
}

impl Container {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Identifies a single process started with `docker exec` inside a shared,
/// externally managed [`Container`], so it can be explicitly terminated when
/// the extension that owns it shuts down.
///
/// A `docker exec` client (the local `docker` CLI process gosling spawns and
/// tracks) is not the process it starts inside the container — it is only
/// attached to it. Killing the local client (e.g. via `kill_on_drop`, or a
/// SIGKILL sent because the process didn't exit gracefully in time) has no
/// effect on the exec'd process: SIGKILL cannot be caught or forwarded, and
/// `docker exec` does not proxy signals the way `docker run`/`docker attach`
/// can. Left alone, this leaves the exec'd process running as an orphan
/// inside the container. `kill()` targets just that process instead of
/// stopping the container, which other extensions in the same session may
/// still be using.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerExecProcess {
    container_id: String,
    /// The argv passed to `docker exec` after the container id — i.e. the
    /// command line the process has inside the container. Used to identify
    /// it for cleanup, since `docker exec` never surfaces its in-container
    /// PID to the host.
    argv: Vec<String>,
}

impl DockerExecProcess {
    pub fn new(container: &Container, argv: Vec<String>) -> Self {
        Self {
            container_id: container.id().to_string(),
            argv,
        }
    }

    /// Best-effort: send SIGKILL to the exec'd process inside the container
    /// by matching its exact command line, without stopping the container
    /// itself. Idempotent — a container that's already gone, or a process
    /// that already exited (e.g. it noticed stdin close on its own), are
    /// both treated as success.
    pub async fn kill(&self) {
        let Some(pattern) = kill_pattern(&self.argv) else {
            return;
        };

        let result = tokio::process::Command::new("docker")
            .arg("exec")
            .arg(&self.container_id)
            .arg("pkill")
            .arg("-f")
            .arg(&pattern)
            .kill_on_drop(true)
            .output()
            .await;

        match result {
            // pkill: 0 = a process was matched and signaled, 1 = no process
            // matched (already exited). Both are the expected outcomes.
            Ok(output) if output.status.success() || output.status.code() == Some(1) => {}
            Ok(output) => tracing::debug!(
                container = %self.container_id,
                status = ?output.status,
                stderr = %String::from_utf8_lossy(&output.stderr),
                "docker exec pkill did not confirm the containerized process was cleaned up"
            ),
            Err(error) => tracing::debug!(
                container = %self.container_id,
                %error,
                "failed to run docker exec pkill to clean up containerized process"
            ),
        }
    }
}

/// Builds a `pkill -f` extended-regex pattern that matches `argv` exactly,
/// escaping regex metacharacters in each token so command/argument text
/// containing them (paths, URLs, flags like `--foo=1.2`) is matched
/// literally rather than as a pattern.
fn kill_pattern(argv: &[String]) -> Option<String> {
    if argv.is_empty() {
        return None;
    }
    Some(
        argv.iter()
            .map(|arg| escape_regex(arg))
            .collect::<Vec<_>>()
            .join(r"\s+"),
    )
}

fn escape_regex(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(
            c,
            '.' | '*' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '|' | '^' | '$' | '\\'
        ) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn kill_pattern_is_none_for_empty_argv() {
        assert_eq!(kill_pattern(&[]), None);
    }

    #[test]
    fn kill_pattern_joins_tokens_with_whitespace_regex() {
        let argv = vec!["gosling".to_string(), "mcp".to_string(), "dev".to_string()];
        assert_eq!(kill_pattern(&argv).as_deref(), Some(r"gosling\s+mcp\s+dev"));
    }

    #[test]
    fn kill_pattern_escapes_regex_metacharacters_in_each_token() {
        let argv = vec![
            "/usr/bin/server".to_string(),
            "--version=1.2+build".to_string(),
            "a(b)[c]".to_string(),
        ];
        let pattern = kill_pattern(&argv).unwrap();
        assert_eq!(
            pattern,
            r"/usr/bin/server\s+--version=1\.2\+build\s+a\(b\)\[c\]"
        );
    }

    #[test]
    fn escape_regex_leaves_ordinary_characters_untouched() {
        assert_eq!(escape_regex("simple-name_123"), "simple-name_123");
    }

    /// Returns `false` (and lets callers skip) rather than panicking when
    /// Docker isn't installed or the daemon isn't reachable, so this test
    /// doesn't fail CI environments without Docker.
    async fn docker_available() -> bool {
        tokio::process::Command::new("docker")
            .arg("info")
            .kill_on_drop(true)
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    async fn docker_exec_matches(container_id: &str, pattern: &str) -> bool {
        tokio::process::Command::new("docker")
            .arg("exec")
            .arg(container_id)
            .arg("pgrep")
            .arg("-f")
            .arg(pattern)
            .kill_on_drop(true)
            .output()
            .await
            .map(|output| output.status.success())
            .unwrap_or(false)
    }

    /// End-to-end against a real Docker daemon: demonstrates both the defect
    /// (killing the local `docker exec` client does not kill the process it
    /// started inside the container) and the fix (`DockerExecProcess::kill`
    /// terminates it without stopping the container).
    #[tokio::test]
    async fn kill_terminates_exec_process_without_stopping_container() {
        if !docker_available().await {
            eprintln!("skipping: docker is not available in this environment");
            return;
        }

        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or_default();
        let name = format!("gosling-res003-test-{}-{}", std::process::id(), nanos);

        // The container's own long-running main process must not match the
        // `sleep 300` pattern used for the exec'd process below — otherwise
        // this test can't distinguish "the exec'd process was killed" from
        // "the container's main process was killed", which is the exact
        // distinction the fix is responsible for preserving.
        let run = tokio::process::Command::new("docker")
            .args([
                "run",
                "-d",
                "--rm",
                "--name",
                &name,
                "busybox",
                "tail",
                "-f",
                "/dev/null",
            ])
            .kill_on_drop(true)
            .output()
            .await
            .expect("failed to invoke docker run");
        if !run.status.success() {
            eprintln!(
                "skipping: could not start busybox test container: {}",
                String::from_utf8_lossy(&run.stderr)
            );
            return;
        }

        // Guarantee container cleanup even if an assertion below fails.
        struct ContainerGuard(String);
        impl Drop for ContainerGuard {
            fn drop(&mut self) {
                let name = self.0.clone();
                if let Ok(handle) = tokio::runtime::Handle::try_current() {
                    handle.spawn(async move {
                        let _ = tokio::process::Command::new("docker")
                            .args(["rm", "-f", &name])
                            .kill_on_drop(true)
                            .output()
                            .await;
                    });
                }
            }
        }
        let _guard = ContainerGuard(name.clone());

        let container = Container::new(name.clone());

        // Start the process the same way the extension manager does: a local
        // `docker exec` client attached to the container.
        let mut exec_child = tokio::process::Command::new("docker")
            .arg("exec")
            .arg("-i")
            .arg(&name)
            .arg("sleep")
            .arg("300")
            .kill_on_drop(true)
            .spawn()
            .expect("failed to spawn docker exec");

        // Give the exec'd process a moment to actually start inside the container.
        tokio::time::sleep(Duration::from_millis(500)).await;
        let pattern = kill_pattern(&["sleep".to_string(), "300".to_string()]).unwrap();
        assert!(
            docker_exec_matches(&name, &pattern).await,
            "expected the exec'd sleep process to be running inside the container"
        );

        // Reproduce the defect: force-kill only the local docker exec client,
        // as rmcp's TokioChildProcess cleanup does when a process doesn't
        // exit gracefully within its timeout.
        exec_child
            .kill()
            .await
            .expect("failed to kill local docker exec client");
        let _ = exec_child.wait().await;
        tokio::time::sleep(Duration::from_millis(500)).await;
        assert!(
            docker_exec_matches(&name, &pattern).await,
            "defect not reproduced: killing the local docker exec client should not \
             have terminated the process inside the container"
        );

        // Apply the fix: explicitly target the in-container process.
        let docker_process =
            DockerExecProcess::new(&container, vec!["sleep".to_string(), "300".to_string()]);
        docker_process.kill().await;
        tokio::time::sleep(Duration::from_millis(300)).await;
        assert!(
            !docker_exec_matches(&name, &pattern).await,
            "DockerExecProcess::kill should have terminated the process inside the container"
        );

        // The container itself must still be running — other extensions in
        // the same session may depend on it.
        let inspect = tokio::process::Command::new("docker")
            .args(["inspect", "-f", "{{.State.Running}}", &name])
            .kill_on_drop(true)
            .output()
            .await
            .expect("failed to inspect container");
        assert_eq!(
            String::from_utf8_lossy(&inspect.stdout).trim(),
            "true",
            "the shared container must not be stopped by killing one exec'd process"
        );
    }

    /// `kill()` on a container that no longer exists must not panic or hang
    /// — the extension may be torn down after the container was already
    /// removed by whatever created it.
    #[tokio::test]
    async fn kill_is_a_noop_when_container_is_gone() {
        if !docker_available().await {
            eprintln!("skipping: docker is not available in this environment");
            return;
        }

        let container = Container::new("gosling-res003-nonexistent-container");
        let docker_process =
            DockerExecProcess::new(&container, vec!["sleep".to_string(), "300".to_string()]);
        docker_process.kill().await;
    }
}
