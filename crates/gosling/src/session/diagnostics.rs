use crate::config::base::Config;
use crate::config::extensions::get_enabled_extensions;
use crate::config::paths::Paths;
use crate::prompt_template::list_templates;
use crate::providers::utils::{LLM_LOG_SESSION_ID_KEY, LOGS_TO_KEEP};
use crate::session::SessionManager;
use gosling_providers::secret_redaction::redact_secrets;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::PathBuf;
use utoipa::ToSchema;

const SERVER_LOG_TAIL_LINES: usize = 400;
const SERVER_LOG_MAX_BYTES: usize = 2 * 1024 * 1024;
const LLM_LOG_MAX_BYTES: usize = 2 * 1024 * 1024;
const CONFIG_MAX_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticsLevel {
    #[default]
    Summary,
    Full,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, schemars::JsonSchema)]
pub struct SystemInfo {
    pub app_version: String,
    pub os: String,
    pub os_version: String,
    pub architecture: String,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub enabled_extensions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsConfig {
    pub config_path: String,
    pub config_yaml: Option<String>,
    pub truncated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsExtensions {
    pub enabled: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsTextFile {
    pub path: String,
    pub content: String,
    pub truncated: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsLogs {
    pub server: Option<DiagnosticsTextFile>,
    pub llm: Vec<DiagnosticsTextFile>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsPrompt {
    pub name: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsError {
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiagnosticsReport {
    pub schema_version: u32,
    pub generated_at: String,
    pub level: DiagnosticsLevel,
    pub system: SystemInfo,
    pub config: Option<DiagnosticsConfig>,
    pub extensions: DiagnosticsExtensions,
    pub session: Option<serde_json::Value>,
    pub logs: DiagnosticsLogs,
    pub prompts: Vec<DiagnosticsPrompt>,
    pub errors: Vec<DiagnosticsError>,
}

impl SystemInfo {
    pub fn collect() -> Self {
        let config = Config::global();
        let provider = config.get_gosling_provider().ok();
        let model = config.get_gosling_model().ok();
        let enabled_extensions = get_enabled_extensions()
            .into_iter()
            .map(|ext| ext.name().to_string())
            .collect();

        Self {
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            os: std::env::consts::OS.to_string(),
            os_version: sys_info::os_release().unwrap_or_else(|_| "unknown".to_string()),
            architecture: std::env::consts::ARCH.to_string(),
            provider,
            model,
            enabled_extensions,
        }
    }

    pub fn to_text(&self) -> String {
        format!(
            "App Version: {}\n\
             OS: {}\n\
             OS Version: {}\n\
             Architecture: {}\n\
             Provider: {}\n\
             Model: {}\n\
             Enabled Extensions: {}\n\
             Timestamp: {}\n",
            self.app_version,
            self.os,
            self.os_version,
            self.architecture,
            self.provider.as_deref().unwrap_or("unknown"),
            self.model.as_deref().unwrap_or("unknown"),
            self.enabled_extensions.join(", "),
            chrono::Utc::now().to_rfc3339()
        )
    }
}

pub fn get_system_info() -> SystemInfo {
    SystemInfo::collect()
}

pub fn config_path() -> PathBuf {
    Paths::config_dir().join("config.yaml")
}

pub fn latest_server_log_path() -> Option<PathBuf> {
    let server_dir = Paths::in_state_dir("logs").join("server");
    let latest_date_dir = latest_entry_by_name(&server_dir)?;
    latest_entry_by_name(&latest_date_dir)
}

pub fn latest_llm_log_path() -> Option<PathBuf> {
    let path = Paths::in_state_dir("logs").join("llm_request.0.jsonl");
    path.exists().then_some(path)
}

fn recent_llm_log_paths() -> Vec<PathBuf> {
    let logs_dir = Paths::in_state_dir("logs");
    let paths: Vec<_> = fs::read_dir(logs_dir)
        .ok()
        .into_iter()
        .flatten()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("llm_request.") && name.ends_with(".jsonl"))
        })
        .collect();

    let (mut numbered, mut temp): (Vec<_>, Vec<_>) = paths
        .into_iter()
        .partition(|path| llm_log_index(path).is_some());

    numbered.sort_by_key(|path| llm_log_index(path).unwrap_or(usize::MAX));
    temp.sort_by(|left, right| {
        llm_log_modified(right)
            .cmp(&llm_log_modified(left))
            .then_with(|| llm_log_name(left).cmp(&llm_log_name(right)))
    });

    if temp.is_empty() || numbered.len() < LOGS_TO_KEEP {
        numbered.extend(temp);
        numbered.truncate(LOGS_TO_KEEP);
        numbered
    } else {
        let temp_slots = 1;
        let numbered_slots = LOGS_TO_KEEP.saturating_sub(temp_slots);
        temp.truncate(temp_slots);
        numbered.truncate(numbered_slots);
        temp.extend(numbered);
        temp
    }
}

const LLM_LOG_SESSION_LINE_MAX_BYTES: u64 = 4096;

fn llm_log_session_id(path: &std::path::Path) -> Option<String> {
    let file = fs::File::open(path).ok()?;
    let mut first_line = String::new();
    BufReader::new(file.take(LLM_LOG_SESSION_LINE_MAX_BYTES))
        .read_line(&mut first_line)
        .ok()?;
    serde_json::from_str::<serde_json::Value>(&first_line)
        .ok()?
        .get(LLM_LOG_SESSION_ID_KEY)?
        .as_str()
        .map(String::from)
}

fn redact_json_strings(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::String(text) => serde_json::Value::String(redact_secrets(&text)),
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.into_iter().map(redact_json_strings).collect())
        }
        serde_json::Value::Object(fields) => serde_json::Value::Object(
            fields
                .into_iter()
                .map(|(key, value)| (key, redact_json_strings(value)))
                .collect(),
        ),
        value => value,
    }
}

fn llm_log_index(path: &std::path::Path) -> Option<usize> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    name.strip_prefix("llm_request.")
        .and_then(|name| name.strip_suffix(".jsonl"))
        .and_then(|name| name.parse::<usize>().ok())
}

fn llm_log_modified(path: &std::path::Path) -> std::time::SystemTime {
    path.metadata()
        .and_then(|metadata| metadata.modified())
        .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
}

fn llm_log_name(path: &std::path::Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_string()
}

pub fn read_tail(path: &std::path::Path, max_lines: usize) -> std::io::Result<String> {
    read_tail_capped(path, max_lines, SERVER_LOG_MAX_BYTES).map(|(content, _)| content)
}

fn read_tail_capped(
    path: &std::path::Path,
    max_lines: usize,
    max_bytes: usize,
) -> std::io::Result<(String, bool)> {
    let mut file = fs::File::open(path)?;
    let file_len = file.metadata()?.len();
    let start = file_len.saturating_sub(max_bytes as u64);
    file.seek(SeekFrom::Start(start))?;
    let mut bytes = Vec::with_capacity(max_bytes.min(file_len as usize));
    file.take(max_bytes as u64).read_to_end(&mut bytes)?;
    let content = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = content.lines().collect();
    let start = lines.len().saturating_sub(max_lines);
    Ok((
        lines[start..].join("\n"),
        file_len > bytes.len() as u64 || start > 0,
    ))
}

pub fn read_capped(path: &std::path::Path, max_bytes: usize) -> std::io::Result<String> {
    let mut file = fs::File::open(path)?;
    let file_len = file.metadata()?.len();
    if file_len <= max_bytes as u64 {
        let mut bytes = Vec::with_capacity(file_len as usize);
        file.take(max_bytes as u64).read_to_end(&mut bytes)?;
        return String::from_utf8(bytes)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error));
    }

    let half = max_bytes / 2;
    let mut head = Vec::with_capacity(half);
    file.by_ref().take(half as u64).read_to_end(&mut head)?;
    let tail_budget = max_bytes.saturating_sub(head.len());
    file.seek(SeekFrom::Start(file_len.saturating_sub(tail_budget as u64)))?;
    let mut tail = Vec::with_capacity(tail_budget);
    file.take(tail_budget as u64).read_to_end(&mut tail)?;
    let omitted = file_len.saturating_sub((head.len() + tail.len()) as u64);
    Ok(format!(
        "{}\n\n... ({} bytes omitted) ...\n\n{}",
        String::from_utf8_lossy(&head),
        omitted,
        String::from_utf8_lossy(&tail),
    ))
}

fn was_truncated(content: &str) -> bool {
    content.contains("... (") && content.contains(" bytes omitted) ...")
}

fn latest_entry_by_name(dir: &std::path::Path) -> Option<PathBuf> {
    let mut entries: Vec<_> = fs::read_dir(dir).ok()?.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| e.file_name());
    entries.last().map(|e| e.path())
}

pub async fn generate_diagnostics(
    session_manager: &SessionManager,
    session_id: &str,
    level: DiagnosticsLevel,
) -> anyhow::Result<DiagnosticsReport> {
    let config_path = config_path();
    let system_info = SystemInfo::collect();
    let is_full = matches!(level, DiagnosticsLevel::Full);
    let mut errors: Vec<DiagnosticsError> = Vec::new();

    // Session export/parse failures are recorded into `errors` instead of aborting
    // the whole report: a diagnostics report is meant to be a best-effort snapshot,
    // and a broken session shouldn't hide the system/config/log info that did
    // collect successfully.
    let session = if is_full {
        match session_manager.export_session(session_id).await {
            Ok(session_data) => match serde_json::from_str(&session_data) {
                Ok(value) => Some(redact_json_strings(value)),
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse exported session {} for diagnostics: {}",
                        session_id,
                        e
                    );
                    errors.push(DiagnosticsError {
                        path: None,
                        message: format!("Failed to parse session data: {}", e),
                    });
                    None
                }
            },
            Err(e) => {
                tracing::warn!(
                    "Failed to export session {} for diagnostics: {}",
                    session_id,
                    e
                );
                errors.push(DiagnosticsError {
                    path: None,
                    message: format!("Failed to export session: {}", e),
                });
                None
            }
        }
    } else {
        None
    };

    let config = if is_full {
        let config_yaml = if config_path.exists() {
            match read_capped(&config_path, CONFIG_MAX_BYTES) {
                Ok(content) => Some(redact_secrets(&content)),
                Err(e) => {
                    errors.push(DiagnosticsError {
                        path: Some(config_path.display().to_string()),
                        message: format!("failed to read config: {e}"),
                    });
                    None
                }
            }
        } else {
            None
        };
        let truncated = config_yaml.as_deref().is_some_and(was_truncated);
        Some(DiagnosticsConfig {
            config_path: config_path.display().to_string(),
            config_yaml,
            truncated,
        })
    } else {
        None
    };

    let logs = if is_full {
        let server = latest_server_log_path().and_then(|path| {
            match read_tail_capped(&path, SERVER_LOG_TAIL_LINES, SERVER_LOG_MAX_BYTES) {
                Ok((content, truncated)) => Some(DiagnosticsTextFile {
                    path: path.display().to_string(),
                    content: redact_secrets(&content),
                    truncated,
                }),
                Err(e) => {
                    errors.push(DiagnosticsError {
                        path: Some(path.display().to_string()),
                        message: format!("failed to read server log: {e}"),
                    });
                    None
                }
            }
        });
        let llm = recent_llm_log_paths()
            .into_iter()
            .filter(|path| llm_log_session_id(path).as_deref() == Some(session_id))
            .filter_map(|path| match read_capped(&path, LLM_LOG_MAX_BYTES) {
                Ok(content) => {
                    let truncated = was_truncated(&content);
                    Some(DiagnosticsTextFile {
                        path: path.display().to_string(),
                        content: redact_secrets(&content),
                        truncated,
                    })
                }
                Err(e) => {
                    errors.push(DiagnosticsError {
                        path: Some(path.display().to_string()),
                        message: format!("failed to read LLM log: {e}"),
                    });
                    None
                }
            })
            .collect();
        DiagnosticsLogs { server, llm }
    } else {
        DiagnosticsLogs::default()
    };

    let prompts = if is_full {
        list_templates()
            .into_iter()
            .map(|template| DiagnosticsPrompt {
                name: template.name,
                content: template.user_content.unwrap_or(template.default_content),
            })
            .collect()
    } else {
        Vec::new()
    };

    Ok(DiagnosticsReport {
        schema_version: 1,
        generated_at: chrono::Utc::now().to_rfc3339(),
        level,
        system: system_info.clone(),
        config,
        extensions: DiagnosticsExtensions {
            enabled: system_info.enabled_extensions,
        },
        session,
        logs,
        prompts,
        errors,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn read_tail_surfaces_the_io_error_instead_of_swallowing_it() {
        let missing = std::path::Path::new("/nonexistent/gosling-diagnostics-test-fixture.log");
        let error = read_tail(missing, 50).expect_err("missing file must return Err, not None");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn read_capped_surfaces_the_io_error_instead_of_swallowing_it() {
        let missing = std::path::Path::new("/nonexistent/gosling-diagnostics-test-fixture.yaml");
        let error = read_capped(missing, 1024).expect_err("missing file must return Err, not None");
        assert_eq!(error.kind(), std::io::ErrorKind::NotFound);
    }

    #[test]
    fn read_capped_still_succeeds_and_truncates_within_budget() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("small.txt");
        std::fs::write(&path, "hello").unwrap();
        assert_eq!(read_capped(&path, 1024).unwrap(), "hello");
    }

    #[test]
    fn read_capped_reads_only_the_bounded_head_and_tail() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.log");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(b"HEAD").unwrap();
        file.set_len(64 * 1024 * 1024).unwrap();
        file.seek(SeekFrom::End(-4)).unwrap();
        file.write_all(b"TAIL").unwrap();

        let content = read_capped(&path, 1024).unwrap();
        assert!(content.starts_with("HEAD"));
        assert!(content.ends_with("TAIL"));
        assert!(was_truncated(&content));
        assert!(content.len() < 1200);
    }

    #[test]
    fn read_tail_caps_bytes_and_reports_real_truncation() {
        use std::io::Write;

        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("server.log");
        let mut file = std::fs::File::create(&path).unwrap();
        file.set_len(4 * 1024 * 1024).unwrap();
        file.seek(SeekFrom::End(-17)).unwrap();
        file.write_all(b"older\nnewer\nlast\n").unwrap();

        let (content, truncated) = read_tail_capped(&path, 2, 1024).unwrap();
        assert_eq!(content, "newer\nlast");
        assert!(truncated);

        let small = dir.path().join("small.log");
        std::fs::write(&small, "one\ntwo").unwrap();
        let (content, truncated) = read_tail_capped(&small, 10, 1024).unwrap();
        assert_eq!(content, "one\ntwo");
        assert!(!truncated);
    }

    #[tokio::test]
    async fn generate_diagnostics_records_session_export_failure_in_errors() {
        let temp_dir = TempDir::new().unwrap();
        let session_manager = SessionManager::new(temp_dir.path().to_path_buf());

        let report = generate_diagnostics(
            &session_manager,
            "session-that-does-not-exist",
            DiagnosticsLevel::Full,
        )
        .await
        .expect("diagnostics generation should degrade gracefully instead of failing outright");

        assert!(report.session.is_none());
        assert_eq!(
            report.errors.len(),
            1,
            "expected exactly one recorded error, got: {:?}",
            report.errors
        );
        assert!(report.errors[0].message.contains("export session"));
    }

    // GSL-PT-20260912-D-4: the bundle carried other sessions' request logs and
    // unredacted tool secrets.
    #[tokio::test]
    async fn full_report_includes_only_the_requested_sessions_logs_redacted() {
        use crate::config::paths::RuntimePaths;
        use crate::config::GoslingMode;
        use crate::conversation::message::Message;
        use crate::providers::utils::RequestLog;
        use crate::session::SessionType;
        use gosling_providers::request_log::RequestLogger;

        let root = TempDir::new().unwrap();
        let runtime_paths = RuntimePaths::new(
            root.path().join("config"),
            root.path().join("data"),
            root.path().join("state"),
        );
        Paths::scope(runtime_paths, async {
            let session_manager = SessionManager::new(root.path().join("sessions"));
            let requested = session_manager
                .create_session(
                    root.path().to_path_buf(),
                    "requested".to_string(),
                    SessionType::User,
                    GoslingMode::Approve,
                )
                .await
                .unwrap();
            session_manager
                .add_message(
                    &requested.id,
                    &Message::assistant()
                        .with_text("tool said ghp_REQUESTEDabcdefghijklmnopqrstuvwxyz0123"),
                )
                .await
                .unwrap();

            let logger = RequestLog::new(LOGS_TO_KEEP).unwrap();
            for (session_id, line) in [
                (
                    Some("other-session".to_string()),
                    "OTHER-SESSION-MARKER ghp_OTHERabcdefghijklmnopqrstuvwxyz0123",
                ),
                (None, "UNSCOPED-MARKER"),
                (
                    Some(requested.id.clone()),
                    "REQUESTED-MARKER ghp_REQUESTEDabcdefghijklmnopqrstuvwxyz0123",
                ),
            ] {
                let mut handle = crate::session_context::with_session_id(session_id, async {
                    logger.start().unwrap()
                })
                .await;
                handle.write(line).unwrap();
            }

            let report =
                generate_diagnostics(&session_manager, &requested.id, DiagnosticsLevel::Full)
                    .await
                    .unwrap();

            assert_eq!(report.logs.llm.len(), 1, "{:?}", report.logs.llm);
            let llm = &report.logs.llm[0].content;
            assert!(llm.contains("REQUESTED-MARKER [REDACTED]"), "{llm}");
            let bundle = serde_json::to_string(&report).unwrap();
            assert!(!bundle.contains("OTHER-SESSION-MARKER"));
            assert!(!bundle.contains("UNSCOPED-MARKER"));
            assert!(!bundle.contains("abcdefghijklmnopqrstuvwxyz0123"));
            assert!(bundle.contains("tool said [REDACTED]"));
            assert!(report.errors.is_empty(), "{:?}", report.errors);
        })
        .await;
    }

    /// Summary-level reports never touch the session at all, so no error
    /// should be recorded even for a nonexistent session id.
    #[tokio::test]
    async fn generate_diagnostics_summary_level_has_no_session_errors() {
        let temp_dir = TempDir::new().unwrap();
        let session_manager = SessionManager::new(temp_dir.path().to_path_buf());

        let report = generate_diagnostics(
            &session_manager,
            "session-that-does-not-exist",
            DiagnosticsLevel::Summary,
        )
        .await
        .unwrap();

        assert!(report.session.is_none());
        assert!(report.errors.is_empty());
    }
}
