use std::collections::HashSet;
use std::fs::Metadata;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::{Duration, SystemTime};

use gosling_providers::errors::ProviderError;
use process_wrap::tokio::ChildWrapper;
use serde_json::Value;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::process::Command;
use tokio::sync::Mutex;

use super::output::protocol_error;
use crate::agents::mcp_client::ExtensionProcessGroup;
use crate::config::search_path::SearchPaths;
use crate::subprocess::configure_subprocess;

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PROBE_BYTES: usize = 128 * 1024;
const MAX_DIAGNOSTIC_BYTES: usize = 16 * 1024;
const BILLING_ACK: &str = "MUSE_CODE_ACKNOWLEDGE_UNVERIFIED_BILLING";

#[derive(PartialEq)]
struct ExecutableStamp {
    path: PathBuf,
    modified: Option<SystemTime>,
    len: u64,
    #[cfg(unix)]
    identity: (u64, u64, i64, i64),
}

impl ExecutableStamp {
    fn new(path: PathBuf, metadata: &Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Self {
            path,
            modified: metadata.modified().ok(),
            len: metadata.len(),
            #[cfg(unix)]
            identity: (
                metadata.dev(),
                metadata.ino(),
                metadata.ctime(),
                metadata.ctime_nsec(),
            ),
        }
    }
}

#[derive(Clone)]
pub(super) struct Capabilities {
    pub(super) executable: PathBuf,
    version: String,
    list_models: bool,
    usage: bool,
    reasoning_effort: bool,
    pub(super) accurate_run_usage: bool,
}

impl Capabilities {
    pub(super) fn require_reasoning_effort(&self) -> Result<(), ProviderError> {
        if !self.reasoning_effort {
            return Err(protocol_error(&format!(
                "{} does not advertise --reasoning-effort; update Muse or clear the effort override",
                self.version
            )));
        }
        Ok(())
    }
}

pub(super) struct MuseCli {
    command: PathBuf,
    working_dir: PathBuf,
    capabilities: Mutex<Option<(ExecutableStamp, Capabilities)>>,
}

impl MuseCli {
    pub(super) fn new(command: PathBuf, working_dir: PathBuf) -> Self {
        Self {
            command,
            working_dir,
            capabilities: Mutex::new(None),
        }
    }

    pub(super) fn command(&self, executable: &Path) -> Command {
        let mut command = Command::new(executable);
        configure_subprocess(&mut command);
        command
            .current_dir(&self.working_dir)
            .env_remove("META_API_KEY")
            .env_remove("MODEL_API_KEY")
            .env_remove(BILLING_ACK)
            .env_remove("MUSE_PERMISSION_PROFILE_FAIL_OPEN")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Ok(path) = SearchPaths::builder().path() {
            command.env("PATH", path);
        }
        command
    }

    pub(super) async fn capabilities(&self) -> Result<Capabilities, ProviderError> {
        let mut cached = self.capabilities.lock().await;
        let path = tokio::fs::canonicalize(&self.command)
            .await
            .map_err(|_| missing_executable())?;
        let metadata = tokio::fs::metadata(&path)
            .await
            .map_err(|_| missing_executable())?;
        if !metadata.is_file() {
            return Err(missing_executable());
        }
        let stamp = ExecutableStamp::new(path, &metadata);
        if let Some((_, capabilities)) = cached.as_ref().filter(|(old, _)| *old == stamp) {
            return Ok(capabilities.clone());
        }

        let (version, help, exec_help) = tokio::try_join!(
            self.probe(&stamp.path, &["--version"]),
            self.probe(&stamp.path, &["--help"]),
            self.probe(&stamp.path, &["exec", "--help"]),
        )?;
        let (version, version_parts) = parse_version(&version)
            .ok_or_else(|| protocol_error("did not report a recognizable CLI version"))?;
        let flags = |flag| exec_help.split_whitespace().any(|word| word == flag);
        for flag in [
            "--provider",
            "--json",
            "--model",
            "--prompt-file",
            "--no-session-log",
            "--disable-approval",
        ] {
            if !flags(flag) {
                return Err(protocol_error(&format!(
                    "{version} does not advertise exec {flag}; install a compatible Muse CLI"
                )));
            }
        }
        let capabilities = Capabilities {
            executable: stamp.path.clone(),
            version,
            list_models: flags("--list-models"),
            usage: help
                .lines()
                .any(|line| line.split_whitespace().next() == Some("usage")),
            reasoning_effort: flags("--reasoning-effort"),
            // Earlier versions could report only the calls after auto-compaction.
            accurate_run_usage: version_parts >= [1, 4, 24],
        };
        *cached = Some((stamp, capabilities.clone()));
        Ok(capabilities)
    }

    pub(super) async fn discover_models(&self, capabilities: &Capabilities) -> Option<Vec<String>> {
        if !capabilities.list_models {
            return None;
        }
        let output = self
            .probe(
                &capabilities.executable,
                &["exec", "--provider", "meta", "--list-models"],
            )
            .await
            .ok()?;
        let mut models = Vec::new();
        let mut seen = HashSet::new();
        for line in output
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            if !valid_model_id(line) {
                return None;
            }
            if seen.insert(line) {
                models.push(line.to_string());
            }
        }
        (!models.is_empty()).then_some(models)
    }

    pub(super) async fn require_subscription(
        &self,
        capabilities: &Capabilities,
    ) -> Result<(), ProviderError> {
        if capabilities.usage {
            let result = self
                .probe(&capabilities.executable, &["usage", "--json"])
                .await?;
            let status: Value = serde_json::from_str(&result)
                .map_err(|_| protocol_error("returned an unreadable billing status"))?;
            if status["schemaVersion"].as_u64() != Some(1) {
                return Err(protocol_error(
                    "returned an unsupported billing status schema",
                ));
            }
            match status["reason"].as_str() {
                Some("api_key") => {
                    return Err(ProviderError::Authentication(
                        "Muse Code selected an API key. Use a subscribed browser sign-in for muse-code, or select 'Meta Muse (API)' for separately billed requests."
                            .to_string(),
                    ));
                }
                Some("no_subscription" | "not_subscribed") => {
                    return Err(ProviderError::Authentication(
                        "Muse Code reported no subscription for the signed-in account.".to_string(),
                    ));
                }
                None if status["reason"].is_null() && valid_allowance(&status["allowance"]) => {
                    return Ok(());
                }
                Some("server_sent_no_usage") if status["allowance"].is_null() => {}
                _ => {
                    return Err(protocol_error(
                        "did not return a recognized subscription status",
                    ))
                }
            }
        }
        if std::env::var(BILLING_ACK).as_deref() == Ok("true") {
            return Ok(());
        }
        Err(ProviderError::Authentication(format!(
            "Muse Code did not confirm subscription billing. Check `muse usage --json` or update Muse. If you have verified that Muse uses only your subscription, set {BILLING_ACK}=true before launching gosling."
        )))
    }

    async fn probe(&self, executable: &Path, args: &[&str]) -> Result<String, ProviderError> {
        tokio::time::timeout(PROBE_TIMEOUT, self.run_probe(executable, args))
            .await
            .map_err(|_| {
                protocol_error("timed out while checking CLI capabilities or account status")
            })?
    }

    async fn run_probe(&self, executable: &Path, args: &[&str]) -> Result<String, ProviderError> {
        let mut command = self.command(executable);
        command.args(args);
        let mut child = spawn(command).map_err(|_| missing_executable())?;
        let _process_group = ExtensionProcessGroup::new(child.id());
        let stdout = child
            .stdout()
            .take()
            .ok_or_else(|| protocol_error("stdout is unavailable"))?;
        let stderr = child
            .stderr()
            .take()
            .ok_or_else(|| protocol_error("stderr is unavailable"))?;
        let mut output = Vec::new();
        let read_stdout = async {
            stdout
                .take((MAX_PROBE_BYTES + 1) as u64)
                .read_to_end(&mut output)
                .await?;
            if output.len() > MAX_PROBE_BYTES {
                return Err(std::io::Error::other("Muse probe output exceeds limit"));
            }
            Ok(())
        };
        let ((), diagnostic, status) =
            tokio::try_join!(read_stdout, diagnostic_stderr(stderr), child.wait())
                .map_err(|_| protocol_error("could not read the CLI probe result"))?;
        if !status.success() {
            return Err(super::run_failure(&diagnostic));
        }
        String::from_utf8(output)
            .map_err(|_| protocol_error("returned an invalid UTF-8 CLI probe result"))
    }
}

#[cfg(windows)]
pub(super) fn spawn(command: Command) -> std::io::Result<Box<dyn ChildWrapper>> {
    use process_wrap::tokio::{CommandWrap, CreationFlags, JobObject, KillOnDrop};

    let mut command = CommandWrap::from(command);
    let CreationFlags(mut flags) = CreationFlags(Default::default());
    flags.0 = crate::subprocess::CREATE_NO_WINDOW_FLAG;
    command.wrap(CreationFlags(flags));
    command.wrap(KillOnDrop);
    command.wrap(JobObject);
    command.spawn()
}

#[cfg(not(windows))]
pub(super) fn spawn(mut command: Command) -> std::io::Result<Box<dyn ChildWrapper>> {
    Ok(Box::new(command.spawn()?))
}

fn valid_allowance(allowance: &Value) -> bool {
    allowance["tier"]
        .as_str()
        .is_some_and(|tier| !tier.is_empty())
        && allowance["observedAtMs"]
            .as_u64()
            .is_some_and(|stamp| stamp > 0)
        && allowance["window"]["windowDurationMins"]
            .as_u64()
            .is_some_and(|duration| duration > 0)
        && ["window", "weekly"].iter().all(|block| {
            allowance[block]["usedPercent"].as_u64().is_some()
                && allowance[block]["resetsAtMs"]
                    .as_u64()
                    .is_some_and(|stamp| stamp > 0)
        })
}

fn parse_version(output: &str) -> Option<(String, [u32; 3])> {
    let output = output.trim();
    let version = output
        .strip_prefix("Muse Code ")?
        .split_whitespace()
        .next()?;
    let mut components = version.split('.');
    let parts = [
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
        components.next()?.parse().ok()?,
    ];
    components
        .next()
        .is_none()
        .then(|| (version.to_string(), parts))
}

pub(super) fn valid_model_id(model: &str) -> bool {
    model.len() <= 128
        && model
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphanumeric)
        && model
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'/' | b':'))
}

fn missing_executable() -> ProviderError {
    protocol_error("could not start the executable; check installation, MUSE_CODE_COMMAND and the working directory")
}

pub(super) async fn diagnostic_stderr(
    mut stderr: impl AsyncRead + Unpin,
) -> std::io::Result<String> {
    let mut buffer = [0; 4096];
    let mut tail = Vec::new();
    loop {
        let count = stderr.read(&mut buffer).await?;
        if count == 0 {
            return Ok(String::from_utf8_lossy(&tail).into_owned());
        }
        let excess = (tail.len() + count).saturating_sub(MAX_DIAGNOSTIC_BYTES);
        tail.drain(..excess);
        tail.extend_from_slice(&buffer[..count]);
    }
}
