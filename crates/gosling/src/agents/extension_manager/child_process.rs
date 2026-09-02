// Owns hardened child-process environments and stdio MCP client startup.
// Extension lifecycle and operator adapters share these process primitives.
// The extension_manager compatibility facade keeps all helpers non-public.

use super::*;

pub(super) fn resolve_command(cmd: &str) -> PathBuf {
    SearchPaths::builder()
        .with_npm()
        .resolve(cmd)
        .unwrap_or_else(|_| {
            // let the OS raise the error
            PathBuf::from(cmd)
        })
}

pub(super) fn minimal_child_environment() -> HashMap<String, String> {
    let mut env = HashMap::new();
    for key in [
        "PATH",
        "HOME",
        "USER",
        "TMPDIR",
        "TEMP",
        "TMP",
        // Spawned MCP servers and the desktop launcher shims resolve gosling's
        // config/data locations from these; dropping them makes children write
        // to a different tree than the one the agent reads.
        "GOSLING_PATH_ROOT",
        "XDG_CONFIG_HOME",
        "XDG_DATA_HOME",
        "XDG_STATE_HOME",
        "XDG_CACHE_HOME",
        // Custom registry settings consumed by the desktop npx/uvx shims.
        "GOSLING_NPM_REGISTRY",
        "GOSLING_NPM_CERT",
        "GOSLING_UV_REGISTRY",
    ] {
        if let Ok(value) = std::env::var(key) {
            env.insert(key.to_string(), value);
        }
    }

    #[cfg(windows)]
    for key in [
        "SystemRoot",
        "COMSPEC",
        "PATHEXT",
        "USERPROFILE",
        "APPDATA",
        "LOCALAPPDATA",
    ] {
        if let Ok(value) = std::env::var(key) {
            env.insert(key.to_string(), value);
        }
    }

    env
}

pub(super) fn apply_minimal_child_environment(command: &mut Command) {
    command.env_clear().envs(minimal_child_environment());
}

/// Like `apply_minimal_child_environment`, but also forwards the env vars
/// that select which Docker daemon/context the `docker` CLI talks to.
/// Clearing these would make `docker exec` silently fall back to the local
/// default daemon and fail to find a container selected via a non-default
/// `DOCKER_HOST`/`DOCKER_CONTEXT`.
/// See https://docs.docker.com/reference/cli/docker/#environment-variables
pub(super) fn apply_minimal_docker_client_environment(command: &mut Command) {
    let mut env = minimal_child_environment();
    for key in [
        "DOCKER_HOST",
        "DOCKER_CONTEXT",
        "DOCKER_CERT_PATH",
        "DOCKER_TLS_VERIFY",
        "DOCKER_CONFIG",
    ] {
        if let Ok(value) = std::env::var(key) {
            env.insert(key.to_string(), value);
        }
    }
    command.env_clear().envs(env);
}

/// Write resolved extension env vars (which may include keyring secrets) to a
/// `docker exec --env-file` compatible file instead of `-e KEY=VALUE` argv,
/// which would leak them to any local process via `ps`/`/proc/<pid>/cmdline`.
/// The env-file format is one `KEY=VALUE` pair per line with no quoting, so
/// invalid names and values containing line breaks are rejected.
pub(super) fn write_docker_env_file(
    path: &std::path::Path,
    envs: &HashMap<String, String>,
) -> std::io::Result<()> {
    let mut contents = String::new();
    let mut entries = envs.iter().collect::<Vec<_>>();
    entries.sort_unstable_by(|(left, _), (right, _)| left.cmp(right));
    for (key, value) in entries {
        if key.is_empty()
            || key.contains('=')
            || key.contains(['\r', '\n'])
            || value.contains(['\r', '\n'])
        {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("environment variable {key:?} cannot be represented in a Docker env file"),
            ));
        }
        contents.push_str(key);
        contents.push('=');
        contents.push_str(value);
        contents.push('\n');
    }

    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(path)?;
        file.write_all(contents.as_bytes())
    }
    #[cfg(not(unix))]
    {
        std::fs::write(path, contents)
    }
}

pub(super) async fn child_process_client(
    mut command: Command,
    timeout: &Option<u64>,
    provider: SharedProvider,
    working_dir: &PathBuf,
    docker_container: Option<String>,
    client_name: String,
    capabilities: GoslingMcpClientCapabilities,
) -> ExtensionResult<McpClient> {
    configure_subprocess(&mut command);

    if let Ok(path) = SearchPaths::builder().path() {
        command.env("PATH", path);
    }

    if working_dir.exists() && working_dir.is_dir() {
        tracing::info!("Setting MCP process working directory: {:?}", working_dir);
        command.current_dir(working_dir);
    } else {
        tracing::warn!(
            "Working directory doesn't exist or isn't a directory: {:?}",
            working_dir
        );
    }

    let (transport, mut stderr) = TokioChildProcess::builder(command)
        .stderr(Stdio::piped())
        .spawn()?;
    let mut stderr = stderr.take().ok_or_else(|| {
        ExtensionError::SetupError("failed to attach child process stderr".to_owned())
    })?;

    let mut stderr_task = tokio::spawn(async move {
        // This buffer is only consumed to build an error message if the connect
        // below fails. On the success path the task is detached and lives as long
        // as the MCP server, so an unbounded `read_to_end` would accumulate every
        // stderr byte a long-lived, chatty server emits for the whole session. Cap
        // what we retain (enough to diagnose a startup failure) but keep reading so
        // the child never blocks writing to a full stderr pipe.
        const MAX_STDERR_CAPTURE: usize = 64 * 1024;
        let mut captured: Vec<u8> = Vec::new();
        let mut chunk = [0u8; 8192];
        loop {
            let n = stderr.read(&mut chunk).await?;
            if n == 0 {
                break;
            }
            if captured.len() < MAX_STDERR_CAPTURE {
                let take = n.min(MAX_STDERR_CAPTURE - captured.len());
                captured.extend_from_slice(&chunk[..take]);
            }
        }
        Ok::<String, std::io::Error>(String::from_utf8_lossy(&captured).into())
    });

    let client_result = McpClient::connect_with_container(
        transport,
        Duration::from_secs(resolve_timeout(*timeout)),
        provider,
        docker_container,
        client_name,
        capabilities,
        working_dir.clone(),
    )
    .await;

    match client_result {
        Ok(client) => Ok(client),
        Err(error) => {
            let stderr_content =
                match tokio::time::timeout(Duration::from_secs(1), &mut stderr_task).await {
                    Ok(error_task_out) => match error_task_out? {
                        Ok(stderr_content) => stderr_content,
                        Err(e) => return Err(e.into()),
                    },
                    Err(_) => {
                        stderr_task.abort();
                        String::new()
                    }
                };
            Err(ProcessExit::new(stderr_content, error).into())
        }
    }
}
