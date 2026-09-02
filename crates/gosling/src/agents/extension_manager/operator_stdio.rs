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
    let client_result = McpClient::connect(
        transport,
        Duration::from_secs(resolve_timeout(registration.timeout)),
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
            let child = Arc::new(std::sync::Mutex::new(Some(child)));
            let (exit_status, _) = tokio::sync::watch::channel(OperatorProcessExit::Running);
            spawn_operator_child_supervisor(Arc::clone(&child), exit_status.clone());
            Ok(OperatorStdioClient {
                client,
                _child: OperatorChildGuard { child },
                exit_status,
            })
        }
        Err(error) => {
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
            Err(ProcessExit::new(stderr_content, error).into())
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
    child: Arc<std::sync::Mutex<Option<tokio::process::Child>>>,
}

impl Drop for OperatorChildGuard {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            if let Some(mut child) = child.take() {
                let _ = child.start_kill();
                if let Ok(runtime) = tokio::runtime::Handle::try_current() {
                    runtime.spawn(async move {
                        let _ = child.wait().await;
                    });
                }
            }
        }
    }
}

fn spawn_operator_child_supervisor(
    child: Arc<std::sync::Mutex<Option<tokio::process::Child>>>,
    exit_status: tokio::sync::watch::Sender<OperatorProcessExit>,
) {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));
        loop {
            interval.tick().await;
            let exited = match child.lock() {
                Ok(mut child) => {
                    let exited = matches!(
                        child.as_mut().map(tokio::process::Child::try_wait),
                        Some(Ok(Some(_))) | Some(Err(_)) | None
                    );
                    if exited {
                        child.take();
                    }
                    exited
                }
                Err(_) => true,
            };
            if exited {
                exit_status.send_replace(OperatorProcessExit::Exited);
                return;
            }
        }
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
