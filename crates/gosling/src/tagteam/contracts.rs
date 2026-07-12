use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

pub const TAGTEAM_CONTRACT_VERSION: u32 = 1;
pub const MAX_PAGE_ITEMS: usize = 100;
pub const MAX_CURSOR_BYTES: usize = 1024;
pub const MAX_IDENTIFIER_BYTES: usize = 256;
pub const MAX_PRODUCER_VERSION_BYTES: usize = 128;
pub const MAX_CAPABILITIES: usize = 64;
pub const MAX_CAPABILITY_BYTES: usize = 128;
pub const MAX_TITLE_BYTES: usize = 4096;
pub const MAX_STATUS_BYTES: usize = 128;
pub const MAX_LOCATION_BYTES: usize = 2048;
pub const MAX_ISSUE_BYTES: usize = 16 * 1024;
pub const MAX_REASON_BYTES: usize = 8 * 1024;
pub const MAX_DIAGNOSTIC_DETAILS: usize = 64;
pub const MAX_DIAGNOSTIC_DETAIL_BYTES: usize = 4096;
pub const MAX_DIAGNOSTIC_TOTAL_BYTES: usize = 64 * 1024;
pub const MAX_APPROVAL_TOKEN_BYTES: usize = 256;
pub const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;

const MAX_PROMPT_BYTES: usize = 128 * 1024;
const MAX_ALLOWED_PATHS: usize = 128;
const MAX_PATH_BYTES: usize = 1024;
const MAX_ROLE_TARGET_BYTES: usize = 256;
const MAX_TEST_PRESET_BYTES: usize = 128;
const MAX_ROUNDS: u32 = 20;
const MAX_TIMEOUT_SECONDS: u64 = 24 * 60 * 60;
const SHA256_HEX_BYTES: usize = 64;

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
    #[error("allowed path escapes the canonical repository root: {0:?}")]
    AllowedPathEscapesRepository(String),
    #[error("multiple allowed paths resolve to the same repository location: {0:?}")]
    DuplicateAllowedPath(String),
    #[error("invalid role target: {0}")]
    InvalidRoleTarget(String),
    #[error("rounds must be between 1 and {MAX_ROUNDS}")]
    InvalidRounds,
    #[error("timeouts must be non-zero and no more than {MAX_TIMEOUT_SECONDS} seconds")]
    InvalidTimeout,
    #[error("test preset identifier must be non-empty and contain no control characters")]
    InvalidTestPreset,
    #[error("invalid page request: {0}")]
    InvalidPageRequest(String),
    #[error("invalid approval contract: {0}")]
    InvalidApproval(String),
    #[error("invalid Tagteam response: {0}")]
    InvalidResponse(String),
    #[error("failed to serialize normalized Tagteam action: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RepositoryIdentity {
    #[schemars(with = "String")]
    canonical_root: PathBuf,
}

impl RepositoryIdentity {
    pub fn from_path(root: impl AsRef<Path>) -> Result<Self, ContractError> {
        let supplied_root = root.as_ref();
        let canonical_root = std::fs::canonicalize(supplied_root)
            .map_err(|_| ContractError::InvalidRepository(supplied_root.display().to_string()))?;
        if !canonical_root.is_dir() {
            return Err(ContractError::InvalidRepository(
                canonical_root.display().to_string(),
            ));
        }
        let output = Command::new("git")
            .arg("-C")
            .arg(&canonical_root)
            .args(["rev-parse", "--show-toplevel"])
            .env("GIT_OPTIONAL_LOCKS", "0")
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_COMMON_DIR")
            .env_remove("GIT_INDEX_FILE")
            .output()
            .map_err(|_| ContractError::InvalidRepository(canonical_root.display().to_string()))?;
        if !output.status.success() {
            return Err(ContractError::InvalidRepository(
                canonical_root.display().to_string(),
            ));
        }
        let discovered = String::from_utf8(output.stdout)
            .ok()
            .map(|root| PathBuf::from(root.trim_end()))
            .and_then(|root| std::fs::canonicalize(root).ok());
        if discovered.as_deref() != Some(canonical_root.as_path()) {
            return Err(ContractError::InvalidRepository(
                canonical_root.display().to_string(),
            ));
        }
        Ok(Self { canonical_root })
    }

    pub fn canonical_root(&self) -> &Path {
        &self.canonical_root
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
        if value.is_empty()
            || value.len() > MAX_PATH_BYTES
            || value.contains('\\')
            || has_windows_drive_prefix(&value)
            || value.chars().any(char::is_control)
        {
            return Err(ContractError::InvalidAllowedPath(value));
        }

        let mut normalized_components = Vec::new();
        for component in Path::new(&value).components() {
            match component {
                Component::Normal(component) => {
                    let component = component
                        .to_str()
                        .ok_or_else(|| ContractError::InvalidAllowedPath(value.clone()))?;
                    normalized_components.push(component);
                }
                Component::CurDir => {}
                Component::Prefix(_) | Component::RootDir | Component::ParentDir => {
                    return Err(ContractError::InvalidAllowedPath(value));
                }
            }
        }

        let normalized = normalized_components.join("/");
        if normalized.is_empty() || normalized.len() > MAX_PATH_BYTES {
            return Err(ContractError::InvalidAllowedPath(value));
        }
        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn resolve_within(
        &self,
        repository: &RepositoryIdentity,
    ) -> Result<PathBuf, ContractError> {
        self.validate()?;
        repository.validate()?;

        let candidate = repository.canonical_root.join(&self.0);
        let mut existing = candidate.as_path();
        let mut missing_components = Vec::<OsString>::new();
        while std::fs::symlink_metadata(existing).is_err() {
            let name = existing
                .file_name()
                .ok_or_else(|| ContractError::AllowedPathEscapesRepository(self.0.clone()))?;
            missing_components.push(name.to_os_string());
            existing = existing
                .parent()
                .ok_or_else(|| ContractError::AllowedPathEscapesRepository(self.0.clone()))?;
        }
        if !missing_components.is_empty() && !existing.is_dir() {
            return Err(ContractError::InvalidAllowedPath(self.0.clone()));
        }

        let mut resolved = std::fs::canonicalize(existing)
            .map_err(|_| ContractError::AllowedPathEscapesRepository(self.0.clone()))?;
        if !resolved.starts_with(&repository.canonical_root) {
            return Err(ContractError::AllowedPathEscapesRepository(self.0.clone()));
        }
        for component in missing_components.iter().rev() {
            resolved.push(component);
        }
        if resolved == repository.canonical_root
            || !resolved.starts_with(&repository.canonical_root)
        {
            return Err(ContractError::AllowedPathEscapesRepository(self.0.clone()));
        }
        Ok(resolved)
    }

    fn validate(&self) -> Result<(), ContractError> {
        let normalized = Self::new(self.0.clone())?;
        if normalized == *self {
            Ok(())
        } else {
            Err(ContractError::InvalidAllowedPath(self.0.clone()))
        }
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
            || value.trim() != value
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
        if Self::new(self.0.clone())? == *self {
            Ok(())
        } else {
            Err(ContractError::InvalidRoleTarget(self.0.clone()))
        }
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
        if value.trim().is_empty()
            || value.trim() != value
            || value.len() > MAX_TEST_PRESET_BYTES
            || value.chars().any(char::is_control)
        {
            return Err(ContractError::InvalidTestPreset);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn validate(&self) -> Result<(), ContractError> {
        if Self::new(self.0.clone())? == *self {
            Ok(())
        } else {
            Err(ContractError::InvalidTestPreset)
        }
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
        self.resolved_allowed_paths().map(|_| ())
    }

    pub fn action_digest(&self) -> Result<String, ContractError> {
        let resolved_allowed_paths = self.resolved_allowed_paths()?;
        #[derive(Serialize)]
        struct NormalizedLaunchAction<'a> {
            schema_version: u32,
            repository: &'a RepositoryIdentity,
            prompt: &'a str,
            team: &'a TeamSpec,
            allowed_paths: &'a [AllowedPath],
            resolved_allowed_paths: &'a [PathBuf],
            rounds: u32,
            time_budget: TimeBudget,
            test_preset: &'a Option<TestPresetRef>,
            recovery_policy: RecoveryPolicy,
        }

        let normalized = NormalizedLaunchAction {
            schema_version: self.schema_version,
            repository: &self.repository,
            prompt: &self.prompt,
            team: &self.team,
            allowed_paths: &self.allowed_paths,
            resolved_allowed_paths: &resolved_allowed_paths,
            rounds: self.rounds,
            time_budget: self.time_budget,
            test_preset: &self.test_preset,
            recovery_policy: self.recovery_policy,
        };
        digest_serialized(b"gosling.tagteam.launch.v1\0", &normalized)
    }

    fn resolved_allowed_paths(&self) -> Result<Vec<PathBuf>, ContractError> {
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

        let mut seen = HashSet::with_capacity(self.allowed_paths.len());
        let mut resolved = Vec::with_capacity(self.allowed_paths.len());
        for path in &self.allowed_paths {
            let resolved_path = path.resolve_within(&self.repository)?;
            if !seen.insert(resolved_path.clone()) {
                return Err(ContractError::DuplicateAllowedPath(
                    path.as_str().to_string(),
                ));
            }
            resolved.push(resolved_path);
        }
        if !(1..=MAX_ROUNDS).contains(&self.rounds) {
            return Err(ContractError::InvalidRounds);
        }
        self.time_budget.validate()?;
        if let Some(test_preset) = &self.test_preset {
            test_preset.validate()?;
        }
        Ok(resolved)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum ApprovalAction {
    Start {
        launch_digest: String,
        idempotency_key: String,
    },
    Resume {
        run_id: String,
        recovery_digest: String,
    },
    Cancel {
        run_id: String,
    },
}

impl ApprovalAction {
    pub fn for_start(
        launch: &TagteamLaunchSpecV1,
        idempotency_key: impl Into<String>,
    ) -> Result<Self, ContractError> {
        let idempotency_key = idempotency_key.into();
        validate_bounded_identifier(
            &idempotency_key,
            MAX_IDEMPOTENCY_KEY_BYTES,
            "start idempotency key",
            ContractError::InvalidApproval,
        )?;
        Ok(Self::Start {
            launch_digest: launch.action_digest()?,
            idempotency_key,
        })
    }

    pub fn for_resume(
        run_id: impl Into<String>,
        recovery_digest: impl Into<String>,
    ) -> Result<Self, ContractError> {
        let run_id = run_id.into();
        let recovery_digest = recovery_digest.into();
        validate_bounded_identifier(
            &run_id,
            MAX_IDENTIFIER_BYTES,
            "resume run id",
            ContractError::InvalidApproval,
        )?;
        validate_sha256_hex(&recovery_digest, "recovery digest")?;
        Ok(Self::Resume {
            run_id,
            recovery_digest,
        })
    }

    pub fn for_cancel(run_id: impl Into<String>) -> Result<Self, ContractError> {
        let run_id = run_id.into();
        validate_bounded_identifier(
            &run_id,
            MAX_IDENTIFIER_BYTES,
            "cancel run id",
            ContractError::InvalidApproval,
        )?;
        Ok(Self::Cancel { run_id })
    }

    pub fn digest(&self) -> Result<String, ContractError> {
        self.validate()?;
        digest_serialized(b"gosling.tagteam.approval-action.v1\0", self)
    }

    fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::Start {
                launch_digest,
                idempotency_key,
            } => {
                validate_sha256_hex(launch_digest, "launch digest")?;
                validate_bounded_identifier(
                    idempotency_key,
                    MAX_IDEMPOTENCY_KEY_BYTES,
                    "start idempotency key",
                    ContractError::InvalidApproval,
                )
            }
            Self::Resume {
                run_id,
                recovery_digest,
            } => {
                validate_bounded_identifier(
                    run_id,
                    MAX_IDENTIFIER_BYTES,
                    "resume run id",
                    ContractError::InvalidApproval,
                )?;
                validate_sha256_hex(recovery_digest, "recovery digest")
            }
            Self::Cancel { run_id } => validate_bounded_identifier(
                run_id,
                MAX_IDENTIFIER_BYTES,
                "cancel run id",
                ContractError::InvalidApproval,
            ),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ApprovalRequest {
    pub schema_version: u32,
    pub action: ApprovalAction,
    pub action_digest: String,
}

impl ApprovalRequest {
    pub fn from_action(action: ApprovalAction) -> Result<Self, ContractError> {
        let action_digest = action.digest()?;
        Ok(Self {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            action,
            action_digest,
        })
    }

    pub fn for_start(
        launch: &TagteamLaunchSpecV1,
        idempotency_key: impl Into<String>,
    ) -> Result<Self, ContractError> {
        Self::from_action(ApprovalAction::for_start(launch, idempotency_key)?)
    }

    pub fn for_resume(
        run_id: impl Into<String>,
        recovery_digest: impl Into<String>,
    ) -> Result<Self, ContractError> {
        Self::from_action(ApprovalAction::for_resume(run_id, recovery_digest)?)
    }

    pub fn for_cancel(run_id: impl Into<String>) -> Result<Self, ContractError> {
        Self::from_action(ApprovalAction::for_cancel(run_id)?)
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        validate_schema_version(self.schema_version)?;
        self.action.validate()?;
        validate_sha256_hex(&self.action_digest, "approval action digest")?;
        if self.action.digest()? != self.action_digest {
            return Err(ContractError::InvalidApproval(
                "action digest does not match the requested action".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Approval(String);

impl Approval {
    pub fn from_token(token: impl Into<String>) -> Result<Self, ContractError> {
        let approval = Self(token.into());
        approval.validate()?;
        Ok(approval)
    }

    pub fn token(&self) -> &str {
        &self.0
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if self.0.is_empty()
            || self.0.len() > MAX_APPROVAL_TOKEN_BYTES
            || !self.0.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(ContractError::InvalidApproval(
                "approval token is malformed".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PageRequest {
    cursor: Option<String>,
    limit: usize,
}

impl PageRequest {
    pub fn new(cursor: Option<String>, limit: usize) -> Result<Self, ContractError> {
        let request = Self { cursor, limit };
        request.validate()?;
        Ok(request)
    }

    pub fn first(limit: usize) -> Result<Self, ContractError> {
        Self::new(None, limit)
    }

    pub fn cursor(&self) -> Option<&str> {
        self.cursor.as_deref()
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

    pub fn validate(&self) -> Result<(), ContractError> {
        if !(1..=MAX_PAGE_ITEMS).contains(&self.limit) {
            return Err(ContractError::InvalidPageRequest(format!(
                "limit must be between 1 and {MAX_PAGE_ITEMS}"
            )));
        }
        if let Some(cursor) = &self.cursor {
            validate_bounded_text(
                cursor,
                MAX_CURSOR_BYTES,
                false,
                "cursor",
                ContractError::InvalidPageRequest,
            )?;
        }
        Ok(())
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

impl TagteamCapabilitySet {
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_schema_version(self.schema_version)?;
        validate_bounded_identifier(
            &self.producer_version,
            MAX_PRODUCER_VERSION_BYTES,
            "producer version",
            ContractError::InvalidResponse,
        )?;
        if self.capabilities.len() > MAX_CAPABILITIES {
            return Err(ContractError::InvalidResponse(format!(
                "capability count exceeds {MAX_CAPABILITIES}"
            )));
        }
        let mut seen = HashSet::with_capacity(self.capabilities.len());
        for capability in &self.capabilities {
            validate_bounded_identifier(
                capability,
                MAX_CAPABILITY_BYTES,
                "capability",
                ContractError::InvalidResponse,
            )?;
            if !seen.insert(capability) {
                return Err(ContractError::InvalidResponse(format!(
                    "duplicate capability {capability:?}"
                )));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RunHandle {
    pub schema_version: u32,
    pub run_id: String,
    pub producer_version: String,
}

impl RunHandle {
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_schema_version(self.schema_version)?;
        validate_response_identifier(&self.run_id, "run id")?;
        validate_bounded_identifier(
            &self.producer_version,
            MAX_PRODUCER_VERSION_BYTES,
            "producer version",
            ContractError::InvalidResponse,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Page<T> {
    pub schema_version: u32,
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub completeness: Completeness,
}

pub trait PageItemContract {
    fn validate_page_item(&self) -> Result<(), ContractError>;
}

impl<T: PageItemContract> Page<T> {
    pub fn validate(&self, request: &PageRequest) -> Result<(), ContractError> {
        validate_schema_version(self.schema_version)?;
        request.validate()?;
        if self.items.len() > request.limit() || self.items.len() > MAX_PAGE_ITEMS {
            return Err(ContractError::InvalidResponse(
                "page contains more items than requested".to_string(),
            ));
        }
        if let Some(cursor) = &self.next_cursor {
            validate_bounded_text(
                cursor,
                MAX_CURSOR_BYTES,
                false,
                "next cursor",
                ContractError::InvalidResponse,
            )?;
        }
        for item in &self.items {
            item.validate_page_item()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PlanItem {
    pub id: String,
    pub title: String,
    pub status: String,
}

impl PageItemContract for PlanItem {
    fn validate_page_item(&self) -> Result<(), ContractError> {
        validate_response_identifier(&self.id, "plan item id")?;
        validate_bounded_text(
            &self.title,
            MAX_TITLE_BYTES,
            false,
            "plan item title",
            ContractError::InvalidResponse,
        )?;
        validate_bounded_identifier(
            &self.status,
            MAX_STATUS_BYTES,
            "plan item status",
            ContractError::InvalidResponse,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FindingSummary {
    pub id: String,
    pub severity: String,
    pub status: String,
    pub location: Option<String>,
    pub issue: String,
}

impl PageItemContract for FindingSummary {
    fn validate_page_item(&self) -> Result<(), ContractError> {
        validate_response_identifier(&self.id, "finding id")?;
        validate_bounded_identifier(
            &self.severity,
            MAX_STATUS_BYTES,
            "finding severity",
            ContractError::InvalidResponse,
        )?;
        validate_bounded_identifier(
            &self.status,
            MAX_STATUS_BYTES,
            "finding status",
            ContractError::InvalidResponse,
        )?;
        if let Some(location) = &self.location {
            validate_bounded_text(
                location,
                MAX_LOCATION_BYTES,
                false,
                "finding location",
                ContractError::InvalidResponse,
            )?;
        }
        validate_bounded_text(
            &self.issue,
            MAX_ISSUE_BYTES,
            false,
            "finding issue",
            ContractError::InvalidResponse,
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DiagnosticSummary {
    pub schema_version: u32,
    pub status: String,
    pub details: Vec<String>,
    pub completeness: Completeness,
}

impl DiagnosticSummary {
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_schema_version(self.schema_version)?;
        validate_bounded_identifier(
            &self.status,
            MAX_STATUS_BYTES,
            "diagnostic status",
            ContractError::InvalidResponse,
        )?;
        if self.details.len() > MAX_DIAGNOSTIC_DETAILS {
            return Err(ContractError::InvalidResponse(format!(
                "diagnostic detail count exceeds {MAX_DIAGNOSTIC_DETAILS}"
            )));
        }
        let mut total_bytes = 0usize;
        for detail in &self.details {
            validate_bounded_text(
                detail,
                MAX_DIAGNOSTIC_DETAIL_BYTES,
                false,
                "diagnostic detail",
                ContractError::InvalidResponse,
            )?;
            total_bytes = total_bytes.saturating_add(detail.len());
        }
        if total_bytes > MAX_DIAGNOSTIC_TOTAL_BYTES {
            return Err(ContractError::InvalidResponse(format!(
                "diagnostic details exceed {MAX_DIAGNOSTIC_TOTAL_BYTES} bytes"
            )));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecoveryAssessment {
    pub schema_version: u32,
    pub run_id: String,
    pub resumable: bool,
    pub reason: String,
    pub approval_request: Option<ApprovalRequest>,
}

impl RecoveryAssessment {
    pub fn validate(&self) -> Result<(), ContractError> {
        validate_schema_version(self.schema_version)?;
        validate_response_identifier(&self.run_id, "recovery run id")?;
        validate_bounded_text(
            &self.reason,
            MAX_REASON_BYTES,
            false,
            "recovery reason",
            ContractError::InvalidResponse,
        )?;
        match (&self.resumable, &self.approval_request) {
            (true, Some(request)) => {
                request.validate()?;
                match &request.action {
                    ApprovalAction::Resume { run_id, .. } if run_id == &self.run_id => Ok(()),
                    _ => Err(ContractError::InvalidResponse(
                        "recovery approval is not bound to this resume action".to_string(),
                    )),
                }
            }
            (false, None) => Ok(()),
            _ => Err(ContractError::InvalidResponse(
                "resumable assessments require exactly one matching approval request".to_string(),
            )),
        }
    }
}

fn validate_schema_version(actual: u32) -> Result<(), ContractError> {
    if actual == TAGTEAM_CONTRACT_VERSION {
        Ok(())
    } else {
        Err(ContractError::UnsupportedVersion {
            actual,
            expected: TAGTEAM_CONTRACT_VERSION,
        })
    }
}

fn validate_response_identifier(value: &str, name: &str) -> Result<(), ContractError> {
    validate_bounded_identifier(
        value,
        MAX_IDENTIFIER_BYTES,
        name,
        ContractError::InvalidResponse,
    )
}

fn validate_bounded_identifier(
    value: &str,
    max_bytes: usize,
    name: &str,
    error: impl FnOnce(String) -> ContractError,
) -> Result<(), ContractError> {
    if value.trim() != value {
        return Err(error(format!(
            "{name} must not have leading or trailing whitespace"
        )));
    }
    validate_bounded_text(value, max_bytes, false, name, error)
}

fn validate_bounded_text(
    value: &str,
    max_bytes: usize,
    allow_empty: bool,
    name: &str,
    error: impl FnOnce(String) -> ContractError,
) -> Result<(), ContractError> {
    if (!allow_empty && value.is_empty())
        || value.len() > max_bytes
        || value.chars().any(char::is_control)
    {
        return Err(error(format!(
            "{name} must be non-empty, contain no control characters, and be at most {max_bytes} bytes"
        )));
    }
    Ok(())
}

fn validate_sha256_hex(value: &str, name: &str) -> Result<(), ContractError> {
    if value.len() != SHA256_HEX_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ContractError::InvalidApproval(format!(
            "{name} must be a lowercase SHA-256 digest"
        )));
    }
    Ok(())
}

fn digest_serialized(
    domain: &[u8],
    value: &(impl Serialize + ?Sized),
) -> Result<String, ContractError> {
    let serialized = serde_json::to_vec(value)
        .map_err(|error| ContractError::Serialization(error.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(domain);
    hasher.update(serialized);
    Ok(hex_digest(hasher.finalize()))
}

pub(crate) fn hex_digest(bytes: impl AsRef<[u8]>) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let bytes = bytes.as_ref();
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn launch_spec(root: &Path) -> TagteamLaunchSpecV1 {
        assert!(Command::new("git")
            .args(["init", "-q"])
            .arg(root)
            .status()
            .unwrap()
            .success());
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
    fn launch_digest_is_stable_and_approval_actions_are_domain_bound() {
        let repo = TempDir::new().unwrap();
        let spec = launch_spec(repo.path());
        assert_eq!(spec.action_digest().unwrap(), spec.action_digest().unwrap());

        let start = ApprovalRequest::for_start(&spec, "session-1").unwrap();
        let cancel = ApprovalRequest::for_cancel("run-session-1").unwrap();
        assert_ne!(start.action_digest, cancel.action_digest);
        assert!(start.validate().is_ok());
    }

    #[test]
    fn normalizes_safe_relative_paths_and_rejects_lexical_escape() {
        assert_eq!(
            AllowedPath::new("./internal//parser/").unwrap().as_str(),
            "internal/parser"
        );
        assert!(AllowedPath::new("../secret").is_err());
        assert!(AllowedPath::new("/tmp/secret").is_err());
        assert!(AllowedPath::new("C:/secret").is_err());
    }

    #[test]
    fn repository_identity_rejects_fake_git_metadata() {
        let root = TempDir::new().unwrap();
        std::fs::create_dir(root.path().join(".git")).unwrap();

        assert!(RepositoryIdentity::from_path(root.path()).is_err());
    }

    #[test]
    fn allowed_path_rejects_missing_suffix_below_file() {
        let repo = TempDir::new().unwrap();
        let spec = launch_spec(repo.path());
        std::fs::write(repo.path().join("file"), "content").unwrap();

        assert!(AllowedPath::new("file/child")
            .unwrap()
            .resolve_within(&spec.repository)
            .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn launch_validation_rejects_symlinks_that_escape_the_repository() {
        use std::os::unix::fs::symlink;

        let repo = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let mut spec = launch_spec(repo.path());
        symlink(outside.path(), repo.path().join("outside-link")).unwrap();
        spec.allowed_paths = vec![AllowedPath::new("outside-link/child").unwrap()];

        assert!(matches!(
            spec.validate(),
            Err(ContractError::AllowedPathEscapesRepository(_))
        ));
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
    fn validation_rejects_non_normalized_deserialized_paths() {
        let repo = TempDir::new().unwrap();
        let spec = launch_spec(repo.path());
        let mut value = serde_json::to_value(spec).unwrap();
        value["allowed_paths"] = serde_json::json!(["internal/"]);
        let malformed: TagteamLaunchSpecV1 = serde_json::from_value(value).unwrap();

        assert!(matches!(
            malformed.validate(),
            Err(ContractError::InvalidAllowedPath(_))
        ));
    }

    #[test]
    fn response_and_page_bounds_are_enforced() {
        assert!(PageRequest::first(0).is_err());
        assert!(PageRequest::first(MAX_PAGE_ITEMS + 1).is_err());

        let request = PageRequest::first(1).unwrap();
        let oversized = Page {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            items: vec![
                PlanItem {
                    id: "P1".to_string(),
                    title: "inspect".to_string(),
                    status: "complete".to_string(),
                },
                PlanItem {
                    id: "P2".to_string(),
                    title: "repair".to_string(),
                    status: "pending".to_string(),
                },
            ],
            next_cursor: None,
            completeness: Completeness::Complete,
        };
        assert!(matches!(
            oversized.validate(&request),
            Err(ContractError::InvalidResponse(_))
        ));
    }
}
