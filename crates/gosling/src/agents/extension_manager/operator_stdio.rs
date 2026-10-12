// Owns operator-registered stdio startup, supervision, and frame bounds.
// ACP domain adapters keep the original public client and exit-status surface.
// The extension_manager compatibility facade re-exports those names unchanged.

use super::*;

/// Starts an operator-registered stdio MCP process using the same environment
/// resolution and subprocess hardening as configured extensions.
pub async fn connect_operator_stdio_client(
    registration: &AdapterRegistration,
    working_dir: &Path,
) -> ExtensionResult<OperatorStdioClient> {
    extension_malware_check::deny_if_malicious_cmd_args(&registration.cmd, &registration.args)
        .await?;

    let config = Config::global();
    let mut all_envs = merge_environments(
        &registration.envs,
        &registration.env_keys,
        &registration.domain_id,
        config,
    )
    .await?;
    let process_working_dir = registration
        .cwd
        .as_deref()
        .map(|raw| {
            let substituted = PathBuf::from(substitute_env_vars(raw, &all_envs));
            if substituted.is_relative() {
                working_dir.join(substituted)
            } else {
                substituted
            }
        })
        .unwrap_or_else(|| working_dir.to_path_buf());
    for (key, value) in minimal_child_environment() {
        all_envs.entry(key).or_insert(value);
    }

    let mut command = Command::new(resolve_command(&registration.cmd)).configure(|command| {
        command.env_clear().args(&registration.args).envs(all_envs);
    });
    let provider: SharedProvider = Arc::new(Mutex::new(None));
    configure_shell_owned_subprocess(&mut command);
    if let Ok(path) = SearchPaths::builder().path() {
        command.env("PATH", path);
    }
    if process_working_dir.is_dir() {
        command.current_dir(&process_working_dir);
    } else {
        return Err(ExtensionError::SetupError(format!(
            "domain adapter working directory is not a directory: {}",
            process_working_dir.display()
        )));
    }

    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn()?;
    let stdout = child.stdout.take().ok_or_else(|| {
        ExtensionError::SetupError("failed to attach domain adapter stdout".to_owned())
    })?;
    let stdin = child.stdin.take().ok_or_else(|| {
        ExtensionError::SetupError("failed to attach domain adapter stdin".to_owned())
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| {
        ExtensionError::SetupError("failed to attach domain adapter stderr".to_owned())
    })?;
    let mut stderr_task = tokio::spawn(async move {
        const MAX_STDERR_CAPTURE: usize = 64 * 1024;
        let mut captured = Vec::new();
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

    let transport = AsyncRwTransport::new(
        BoundedLineReader::new(stdout, registration.max_message_bytes),
        stdin,
    );
    let timeouts = ExtensionTimeouts::resolve(registration.timeout);
    let started = std::time::Instant::now();
    let client_result = McpClient::connect(
        transport,
        timeouts.mcp_client(),
        provider,
        "gosling-domain-adapter".to_string(),
        GoslingMcpClientCapabilities {
            mcpui: false,
            host_info: None,
        },
        process_working_dir,
    )
    .await;

    match client_result {
        Ok(client) => {
            let cancellation_token = CancellationToken::new();
            let (exit_status, _) = tokio::sync::watch::channel(OperatorProcessExit::Running);
            spawn_operator_child_supervisor(child, cancellation_token.clone(), exit_status.clone());
            Ok(OperatorStdioClient {
                client,
                _child: OperatorChildGuard { cancellation_token },
                exit_status,
            })
        }
        Err(error) => {
            let timed_out = timeouts.startup_expired(started);
            let exit_status = child.try_wait().ok().flatten();
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
            let stderr_content =
                match tokio::time::timeout(Duration::from_secs(1), &mut stderr_task).await {
                    Ok(error_task_out) => match error_task_out? {
                        Ok(stderr_content) => stderr_content,
                        Err(error) => return Err(error.into()),
                    },
                    Err(_) => {
                        stderr_task.abort();
                        String::new()
                    }
                };
            if timed_out {
                return Err(timeouts.startup_timeout_error(stderr_content));
            }
            Err(ProcessExit::new(stderr_content, exit_status, error).into())
        }
    }
}

pub struct OperatorStdioClient {
    pub client: McpClient,
    _child: OperatorChildGuard,
    exit_status: tokio::sync::watch::Sender<OperatorProcessExit>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperatorProcessExit {
    Running,
    Exited,
}

impl OperatorStdioClient {
    pub fn subscribe_exit(&self) -> tokio::sync::watch::Receiver<OperatorProcessExit> {
        self.exit_status.subscribe()
    }
}

struct OperatorChildGuard {
    cancellation_token: CancellationToken,
}

impl Drop for OperatorChildGuard {
    fn drop(&mut self) {
        self.cancellation_token.cancel();
    }
}

fn spawn_operator_child_supervisor(
    mut child: tokio::process::Child,
    cancellation_token: CancellationToken,
    exit_status: tokio::sync::watch::Sender<OperatorProcessExit>,
) {
    tokio::spawn(async move {
        let result = tokio::select! {
            result = child.wait() => Some(result),
            _ = cancellation_token.cancelled() => None,
        };
        if !matches!(result, Some(Ok(_))) {
            let _ = child.start_kill();
            let _ = tokio::time::timeout(Duration::from_secs(1), child.wait()).await;
        }
        exit_status.send_replace(OperatorProcessExit::Exited);
    });
}

struct BoundedLineReader<R> {
    inner: R,
    max_frame_bytes: usize,
    line_bytes: usize,
}

impl<R> BoundedLineReader<R> {
    fn new(inner: R, max_frame_bytes: usize) -> Self {
        Self {
            inner,
            max_frame_bytes,
            line_bytes: 0,
        }
    }
}

impl<R: AsyncRead + Unpin> AsyncRead for BoundedLineReader<R> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        if buf.remaining() == 0 {
            return Poll::Ready(Ok(()));
        }
        let permitted = self
            .max_frame_bytes
            .saturating_sub(self.line_bytes)
            .saturating_add(1)
            .min(buf.remaining())
            .min(8192);
        if permitted == 0 {
            return Poll::Ready(Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "MCP message exceeded configured maximum size",
            )));
        }
        let mut scratch = [0u8; 8192];
        let mut read = ReadBuf::new(&mut scratch[..permitted]);
        match Pin::new(&mut self.inner).poll_read(cx, &mut read) {
            Poll::Pending => Poll::Pending,
            Poll::Ready(Err(error)) => Poll::Ready(Err(error)),
            Poll::Ready(Ok(())) => {
                for byte in read.filled() {
                    if *byte == b'\n' {
                        self.line_bytes = 0;
                    } else {
                        self.line_bytes += 1;
                        if self.line_bytes > self.max_frame_bytes {
                            return Poll::Ready(Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "MCP message exceeded configured maximum size",
                            )));
                        }
                    }
                }
                buf.put_slice(read.filled());
                Poll::Ready(Ok(()))
            }
        }
    }
}

#[cfg(test)]
mod operator_stdio_tests {
    use super::BoundedLineReader;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[cfg(unix)]
    #[tokio::test]
    async fn supervisor_reaps_the_child_when_its_guard_is_dropped() {
        use super::*;
        let mut command = Command::new("sleep");
        command.arg("60");
        configure_shell_owned_subprocess(&mut command);
        let child = command.spawn().unwrap();
        let pid = child.id().unwrap();
        let cancellation_token = CancellationToken::new();
        let (exit_status, mut receiver) = tokio::sync::watch::channel(OperatorProcessExit::Running);
        spawn_operator_child_supervisor(child, cancellation_token.clone(), exit_status);
        drop(OperatorChildGuard { cancellation_token });
        tokio::time::timeout(Duration::from_secs(2), receiver.changed())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*receiver.borrow(), OperatorProcessExit::Exited);
        assert!(!crate::subprocess::process_is_alive(pid).await);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn supervisor_notifies_natural_exit_without_a_polling_tick() {
        use super::*;
        let mut command = Command::new("sh");
        command.args(["-c", "exit 0"]);
        configure_shell_owned_subprocess(&mut command);
        let child = command.spawn().unwrap();
        let (exit_status, mut receiver) = tokio::sync::watch::channel(OperatorProcessExit::Running);
        spawn_operator_child_supervisor(child, CancellationToken::new(), exit_status);
        tokio::time::timeout(Duration::from_secs(2), receiver.changed())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(*receiver.borrow(), OperatorProcessExit::Exited);
    }

    #[tokio::test]
    async fn bounded_reader_accepts_exactly_sized_json_lines() {
        let (reader, mut writer) = tokio::io::duplex(64);
        writer.write_all(b"abcd\nxy\n").await.unwrap();
        drop(writer);
        let mut reader = BoundedLineReader::new(reader, 4);
        let mut output = Vec::new();

        reader.read_to_end(&mut output).await.unwrap();

        assert_eq!(output, b"abcd\nxy\n");
    }

    #[tokio::test]
    async fn bounded_reader_rejects_an_overlong_json_line_before_decode() {
        let (reader, mut writer) = tokio::io::duplex(64);
        writer.write_all(b"abcde\n").await.unwrap();
        drop(writer);
        let mut reader = BoundedLineReader::new(reader, 4);
        let mut output = Vec::new();

        let error = reader.read_to_end(&mut output).await.unwrap_err();

        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(error.to_string().contains("configured maximum size"));
    }
}
