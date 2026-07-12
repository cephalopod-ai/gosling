use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt::Write;
use std::path::{Component, Path, PathBuf};

pub const TAGTEAM_CONTRACT_VERSION: u32 = 1;
const MAX_PROMPT_BYTES: usize = 128 * 1024;
const MAX_ALLOWED_PATHS: usize = 128;
const MAX_PATH_BYTES: usize = 1024;
const MAX_ROLE_TARGET_BYTES: usize = 256;
const MAX_ROUNDS: u32 = 20;
const MAX_TIMEOUT_SECONDS: u64 = 24 * 60 * 60;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum ContractError {
    #[error("unsupported Tagteam contract version {actual}; expected {expected}")]
    UnsupportedVersion { actual: u32, expected: u32 },
    #[error("repository root does not exist or is not a Git worktree: {0}")]
    InvalidRepository(String),
    #[error("prompt must be non-empty and at most {MAX_PROMPT_BYTES} bytes")]
    InvalidPrompt,
    #[error("Tagteam launch requires between 1 and {MAX_ALLOWED_PATHS} allowed paths")]
    InvalidAllowedPathCount,
    #[error("invalid allowed path {0:?}: use a normalized repository-relative path")]
    InvalidAllowedPath(String),
    #[error("invalid role target: {0}")]
    InvalidRoleTarget(String),
    #[error("rounds must be between 1 and {MAX_ROUNDS}")]
    InvalidRounds,
    #[error("timeouts must be non-zero and no more than {MAX_TIMEOUT_SECONDS} seconds")]
    InvalidTimeout,
    #[error("test preset identifier must be non-empty and contain no control characters")]
    InvalidTestPreset,
    #[error("failed to serialize normalized launch action: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryIdentity {
    #[schemars(with = "String")]
    pub canonical_root: PathBuf,
}

impl RepositoryIdentity {
    pub fn from_path(root: impl AsRef<Path>) -> Result<Self, ContractError> {
        let canonical_root = std::fs::canonicalize(root.as_ref())
            .map_err(|_| ContractError::InvalidRepository(root.as_ref().display().to_string()))?;
        if !canonical_root.join(".git").exists() {
            return Err(ContractError::InvalidRepository(
                canonical_root.display().to_string(),
            ));
        }
        Ok(Self { canonical_root })
    }

    fn validate(&self) -> Result<(), ContractError> {
        let canonical = Self::from_path(&self.canonical_root)?;
        if canonical == *self {
            Ok(())
        } else {
            Err(ContractError::InvalidRepository(
                self.canonical_root.display().to_string(),
            ))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct AllowedPath(String);

impl AllowedPath {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        let path = Path::new(&value);
        let invalid_component = path.components().any(|component| {
            matches!(
                component,
                Component::Prefix(_) | Component::RootDir | Component::ParentDir
            )
        });
        if value.is_empty()
            || value.len() > MAX_PATH_BYTES
            || value.contains('\\')
            || has_windows_drive_prefix(&value)
            || value.chars().any(char::is_control)
            || invalid_component
            || value == "."
            || value.starts_with("./")
            || value.contains("//")
        {
            return Err(ContractError::InvalidAllowedPath(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<(), ContractError> {
        Self::new(self.0.clone()).map(|_| ())
    }
}

fn has_windows_drive_prefix(value: &str) -> bool {
    let bytes = value.as_bytes();
    bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct RoleTarget(String);

impl RoleTarget {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.trim().is_empty()
            || value.len() > MAX_ROLE_TARGET_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(ContractError::InvalidRoleTarget(value));
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<(), ContractError> {
        Self::new(self.0.clone()).map(|_| ())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TeamSpec {
    Supervisor {
        worker: RoleTarget,
        supervisor: RoleTarget,
    },
    Relay {
        coder: RoleTarget,
        supervisor: RoleTarget,
        scout: RoleTarget,
    },
    Adversarial {
        coder: RoleTarget,
        reviewer: RoleTarget,
    },
    Solo {
        worker: RoleTarget,
    },
}

impl TeamSpec {
    fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::Supervisor { worker, supervisor } => {
                worker.validate()?;
                supervisor.validate()
            }
            Self::Relay {
                coder,
                supervisor,
                scout,
            } => {
                coder.validate()?;
                supervisor.validate()?;
                scout.validate()
            }
            Self::Adversarial { coder, reviewer } => {
                coder.validate()?;
                reviewer.validate()
            }
            Self::Solo { worker } => worker.validate(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryPolicy {
    Assist,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct TestPresetRef(String);

impl TestPresetRef {
    pub fn new(value: impl Into<String>) -> Result<Self, ContractError> {
        let value = value.into();
        if value.trim().is_empty() || value.len() > 128 || value.chars().any(char::is_control) {
            return Err(ContractError::InvalidTestPreset);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<(), ContractError> {
        Self::new(self.0.clone()).map(|_| ())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimeBudget {
    pub invocation_timeout_seconds: u64,
    pub watchdog_timeout_seconds: u64,
    pub wall_timeout_seconds: u64,
}

impl TimeBudget {
    fn validate(self) -> Result<(), ContractError> {
        let values = [
            self.invocation_timeout_seconds,
            self.watchdog_timeout_seconds,
            self.wall_timeout_seconds,
        ];
        if values
            .iter()
            .any(|value| *value == 0 || *value > MAX_TIMEOUT_SECONDS)
        {
            return Err(ContractError::InvalidTimeout);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TagteamLaunchSpecV1 {
    pub schema_version: u32,
    pub repository: RepositoryIdentity,
    pub prompt: String,
    pub team: TeamSpec,
    pub allowed_paths: Vec<AllowedPath>,
    pub rounds: u32,
    pub time_budget: TimeBudget,
    pub test_preset: Option<TestPresetRef>,
    pub recovery_policy: RecoveryPolicy,
}

impl TagteamLaunchSpecV1 {
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.schema_version != TAGTEAM_CONTRACT_VERSION {
            return Err(ContractError::UnsupportedVersion {
                actual: self.schema_version,
                expected: TAGTEAM_CONTRACT_VERSION,
            });
        }
        if self.prompt.trim().is_empty() || self.prompt.len() > MAX_PROMPT_BYTES {
            return Err(ContractError::InvalidPrompt);
        }
        self.repository.validate()?;
        self.team.validate()?;
        if self.allowed_paths.is_empty() || self.allowed_paths.len() > MAX_ALLOWED_PATHS {
            return Err(ContractError::InvalidAllowedPathCount);
        }
        for path in &self.allowed_paths {
            path.validate()?;
        }
        if !(1..=MAX_ROUNDS).contains(&self.rounds) {
            return Err(ContractError::InvalidRounds);
        }
        self.time_budget.validate()?;
        if let Some(test_preset) = &self.test_preset {
            test_preset.validate()?;
        }
        Ok(())
    }

    pub fn action_digest(&self) -> Result<String, ContractError> {
        self.validate()?;
        let normalized = serde_json::to_vec(self)
            .map_err(|error| ContractError::Serialization(error.to_string()))?;
        let digest = Sha256::digest(normalized);
        let mut encoded = String::with_capacity(digest.len() * 2);
        for byte in digest {
            write!(&mut encoded, "{byte:02x}")
                .map_err(|error| ContractError::Serialization(error.to_string()))?;
        }
        Ok(encoded)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Completeness {
    Complete,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Retryability {
    Retryable,
    Terminal,
    ApprovalRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TagteamCapabilitySet {
    pub schema_version: u32,
    pub producer_version: String,
    pub capabilities: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunHandle {
    pub schema_version: u32,
    pub run_id: String,
    pub producer_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Page<T> {
    pub schema_version: u32,
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub completeness: Completeness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanItem {
    pub id: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FindingSummary {
    pub id: String,
    pub severity: String,
    pub status: String,
    pub location: Option<String>,
    pub issue: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiagnosticSummary {
    pub schema_version: u32,
    pub status: String,
    pub details: Vec<String>,
    pub completeness: Completeness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecoveryAssessment {
    pub schema_version: u32,
    pub run_id: String,
    pub resumable: bool,
    pub reason: String,
    pub action_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Approval {
    pub action_digest: String,
    pub approved_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub nonce: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn launch_spec(root: &Path) -> TagteamLaunchSpecV1 {
        std::fs::create_dir_all(root.join(".git")).unwrap();
        TagteamLaunchSpecV1 {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            repository: RepositoryIdentity::from_path(root).unwrap(),
            prompt: "repair the parser".to_string(),
            team: TeamSpec::Supervisor {
                worker: RoleTarget::new("agy:worker").unwrap(),
                supervisor: RoleTarget::new("codex:supervisor").unwrap(),
            },
            allowed_paths: vec![AllowedPath::new("internal/").unwrap()],
            rounds: 2,
            time_budget: TimeBudget {
                invocation_timeout_seconds: 900,
                watchdog_timeout_seconds: 300,
                wall_timeout_seconds: 3600,
            },
            test_preset: Some(TestPresetRef::new("go-test").unwrap()),
            recovery_policy: RecoveryPolicy::Assist,
        }
    }

    #[test]
    fn launch_digest_is_stable() {
        let repo = TempDir::new().unwrap();
        let spec = launch_spec(repo.path());
        assert_eq!(spec.action_digest().unwrap(), spec.action_digest().unwrap());
    }

    #[test]
    fn rejects_escaping_and_absolute_paths() {
        assert!(AllowedPath::new("../secret").is_err());
        assert!(AllowedPath::new("/tmp/secret").is_err());
        assert!(AllowedPath::new("./internal/").is_err());
        assert!(AllowedPath::new("C:/secret").is_err());
    }

    #[test]
    fn rejects_unknown_major_contract() {
        let repo = TempDir::new().unwrap();
        let mut spec = launch_spec(repo.path());
        spec.schema_version += 1;
        assert!(matches!(
            spec.validate(),
            Err(ContractError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn validation_rejects_nested_values_constructed_by_deserialization() {
        let repo = TempDir::new().unwrap();
        let spec = launch_spec(repo.path());
        let mut value = serde_json::to_value(spec).unwrap();
        value["allowed_paths"] = serde_json::json!(["../outside"]);
        let malformed: TagteamLaunchSpecV1 = serde_json::from_value(value).unwrap();

        assert!(matches!(
            malformed.validate(),
            Err(ContractError::InvalidAllowedPath(_))
        ));
    }
}
