use super::contracts::{
    AllowedPath, Completeness, ContractError, PageRequest, RecoveryPolicy, RepositoryIdentity,
    RoleTarget, TagteamCapabilitySet, TagteamLaunchSpecV1, TeamSpec, TestPresetRef, TimeBudget,
    MAX_IDENTIFIER_BYTES, MAX_PAGE_ITEMS, MAX_REASON_BYTES, MAX_STATUS_BYTES,
    TAGTEAM_CONTRACT_VERSION,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use rmcp::{
    model::{CallToolRequestParams, ClientRequest, Request, ServerResult},
    service::{PeerRequestOptions, RunningService},
    transport::IntoTransport,
    ClientHandler, RoleClient, ServiceExt,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{collections::HashSet, path::Path, time::Duration as StdDuration};
use tokio::sync::Mutex;

const TAGTEAM_MCP_PROTOCOL_VERSION: &str = "2025-11-25";
const MAX_PRODUCER_TEXT_BYTES: usize = 4096;
const MAX_DIAGNOSTIC_DETAILS: usize = 64;
const MAX_APPROVAL_NONCE_BYTES: usize = 256;
const MAX_IDEMPOTENCY_KEY_BYTES: usize = 256;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TagteamControlError {
    #[error("invalid Tagteam control request: {0}")]
    InvalidRequest(String),
    #[error(
        "Tagteam control-plane schema {actual} is incompatible with supported schema {supported}"
    )]
    IncompatibleSchema { actual: u32, supported: u32 },
    #[error("Tagteam control plane is unavailable: {0}")]
    Unavailable(String),
    #[error("Tagteam action requires a matching unexpired approval")]
    ApprovalRequired,
    #[error("Tagteam control operation {code} may be retried: {reason}")]
    Retryable { code: String, reason: String },
    #[error("Tagteam control operation {code} failed: {reason}")]
    Terminal { code: String, reason: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlApproval {
    pub action_digest: String,
    pub approved_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub nonce: String,
}

impl ControlApproval {
    pub fn validate(&self) -> Result<(), TagteamControlError> {
        validate_request_digest(&self.action_digest, "approval action digest")?;
        if self.nonce.is_empty()
            || self.nonce.len() > MAX_APPROVAL_NONCE_BYTES
            || self.nonce.trim() != self.nonce
            || self.nonce.chars().any(char::is_control)
        {
            return Err(TagteamControlError::InvalidRequest(
                "approval nonce is malformed".to_string(),
            ));
        }
        if self.expires_at <= self.approved_at {
            return Err(TagteamControlError::InvalidRequest(
                "approval expiry must be after approval time".to_string(),
            ));
        }
        if self.expires_at <= Utc::now() {
            return Err(TagteamControlError::InvalidRequest(
                "approval has expired".to_string(),
            ));
        }
        if self.expires_at > self.approved_at + Duration::minutes(30) {
            return Err(TagteamControlError::InvalidRequest(
                "approval lifetime exceeds Tagteam's 30 minute maximum".to_string(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlLaunchValidation {
    pub schema_version: u32,
    pub normalized: TagteamLaunchSpecV1,
    pub launch_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStartPreparation {
    pub schema_version: u32,
    pub normalized: TagteamLaunchSpecV1,
    pub action_digest: String,
    pub approval_max_lifetime_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlRecoveryAssessment {
    pub schema_version: u32,
    pub run_id: String,
    pub resumable: bool,
    pub reason_code: String,
    pub reason: String,
    pub action_digest: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlRunStatus {
    pub schema_version: u32,
    pub run_id: String,
    pub status: String,
    pub phase: String,
    pub verdict: String,
    pub degraded: bool,
    pub degraded_reason: String,
    pub blocking_reason: String,
    pub current_round: u32,
    pub rounds_completed: u32,
    pub rounds_requested: u32,
    pub changed_files: Vec<String>,
    pub findings_count: u32,
    pub open_major_count: u32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlStatus {
    pub schema_version: u32,
    pub snapshot_id: String,
    pub completeness: Completeness,
    pub truncated: bool,
    pub run: ControlRunStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlPage<T> {
    pub schema_version: u32,
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
    pub completeness: Completeness,
    pub truncated: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlPlanItem {
    pub id: String,
    pub title: String,
    pub status: String,
    pub owner: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlFinding {
    pub id: String,
    pub source: String,
    pub severity: String,
    pub status: String,
    pub file: Option<String>,
    pub line: Option<u32>,
    pub issue: String,
    pub fix: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ControlDiagnostics {
    pub schema_version: u32,
    pub status: String,
    pub canonical_root: String,
    pub repo_id: String,
    pub state_root: String,
    pub details: Vec<String>,
    pub completeness: Completeness,
}

#[async_trait]
pub trait TagteamControlClient: Send + Sync {
    async fn capabilities(&self) -> Result<TagteamCapabilitySet, TagteamControlError>;

    async fn validate_launch(
        &self,
        launch: &TagteamLaunchSpecV1,
    ) -> Result<ControlLaunchValidation, TagteamControlError>;

    async fn prepare_start(
        &self,
        launch: &TagteamLaunchSpecV1,
        idempotency_key: &str,
    ) -> Result<ControlStartPreparation, TagteamControlError>;

    async fn start(
        &self,
        launch: &TagteamLaunchSpecV1,
        idempotency_key: &str,
        approval: ControlApproval,
    ) -> Result<super::contracts::RunHandle, TagteamControlError>;

    async fn status(&self, run_id: &str) -> Result<ControlStatus, TagteamControlError>;

    async fn plan(
        &self,
        run_id: &str,
        request: &PageRequest,
    ) -> Result<ControlPage<ControlPlanItem>, TagteamControlError>;

    async fn findings(
        &self,
        run_id: &str,
        request: &PageRequest,
    ) -> Result<ControlPage<ControlFinding>, TagteamControlError>;

    async fn prepare_resume(
        &self,
        run_id: &str,
    ) -> Result<ControlRecoveryAssessment, TagteamControlError>;

    async fn resume(
        &self,
        run_id: &str,
        approval: ControlApproval,
    ) -> Result<super::contracts::RunHandle, TagteamControlError>;

    async fn cancel(
        &self,
        run_id: &str,
        approval: ControlApproval,
    ) -> Result<(), TagteamControlError>;

    async fn diagnostics(&self) -> Result<ControlDiagnostics, TagteamControlError>;
}

struct TagteamMcpHandler;

impl ClientHandler for TagteamMcpHandler {}

pub struct McpTagteamClient {
    service: Mutex<RunningService<RoleClient, TagteamMcpHandler>>,
    timeout: StdDuration,
    capabilities: TagteamCapabilitySet,
    repository: WireRepository,
}

impl McpTagteamClient {
    #[cfg(unix)]
    pub async fn connect_unix_socket(
        socket_path: impl AsRef<Path>,
        timeout: StdDuration,
    ) -> Result<Self, TagteamControlError> {
        let stream = tokio::net::UnixStream::connect(socket_path.as_ref())
            .await
            .map_err(|error| TagteamControlError::Unavailable(error.to_string()))?;
        Self::connect(stream, timeout).await
    }

    async fn connect<T, E, A>(
        transport: T,
        timeout: StdDuration,
    ) -> Result<Self, TagteamControlError>
    where
        T: IntoTransport<RoleClient, E, A>,
        E: std::error::Error + From<std::io::Error> + Send + Sync + 'static,
    {
        let service = TagteamMcpHandler
            .serve(transport)
            .await
            .map_err(|error| TagteamControlError::Unavailable(error.to_string()))?;
        let server = service.peer_info().ok_or_else(|| {
            TagteamControlError::Unavailable(
                "MCP initialization returned no server information".to_string(),
            )
        })?;
        if server.server_info.name != "tagteam" {
            return Err(TagteamControlError::Unavailable(
                "MCP endpoint is not a Tagteam control-plane server".to_string(),
            ));
        }
        if server.protocol_version.as_str() != TAGTEAM_MCP_PROTOCOL_VERSION {
            return Err(TagteamControlError::Unavailable(format!(
                "MCP protocol {} is unsupported; expected {TAGTEAM_MCP_PROTOCOL_VERSION}",
                server.protocol_version
            )));
        }

        let client = Self {
            service: Mutex::new(service),
            timeout,
            capabilities: TagteamCapabilitySet {
                schema_version: 0,
                producer_version: String::new(),
                capabilities: Vec::new(),
            },
            repository: WireRepository::default(),
        };
        let capabilities = client.call_capabilities().await?;
        require_capabilities(
            &capabilities,
            [
                "capabilities",
                "validate_launch",
                "prepare_start",
                "prepare_resume",
                "status",
                "plan",
                "findings",
                "diagnostics",
            ],
        )?;
        let diagnostics = client.call_diagnostics().await?;
        let repository = WireRepository {
            canonical_root: diagnostics.canonical_root,
            repo_id: diagnostics.repo_id,
        };
        Ok(Self {
            service: client.service,
            timeout,
            capabilities,
            repository,
        })
    }

    async fn call_capabilities(&self) -> Result<TagteamCapabilitySet, TagteamControlError> {
        let response: WireCapabilities = self.call_tool("tagteam_capabilities", json!({})).await?;
        response.try_into()
    }

    async fn call_diagnostics(&self) -> Result<ControlDiagnostics, TagteamControlError> {
        let response: WireDiagnostics = self.call_tool("tagteam_diagnostics", json!({})).await?;
        response.try_into()
    }

    async fn call_tool<T: for<'de> Deserialize<'de>>(
        &self,
        name: &str,
        arguments: Value,
    ) -> Result<T, TagteamControlError> {
        let arguments = arguments.as_object().cloned().ok_or_else(|| {
            TagteamControlError::InvalidRequest("MCP tool arguments must be an object".to_string())
        })?;
        let request = ClientRequest::CallToolRequest(Request::new(
            CallToolRequestParams::new(name.to_string()).with_arguments(arguments),
        ));
        let handle = {
            let service = self.service.lock().await;
            service
                .send_cancellable_request(request, PeerRequestOptions::no_options())
                .await
                .map_err(|error| TagteamControlError::Unavailable(error.to_string()))?
        };
        let result = tokio::time::timeout(self.timeout, handle.rx)
            .await
            .map_err(|_| TagteamControlError::Unavailable("MCP tool call timed out".to_string()))?
            .map_err(|_| TagteamControlError::Unavailable("MCP transport closed".to_string()))?
            .map_err(|error| TagteamControlError::Unavailable(error.to_string()))?;
        let ServerResult::CallToolResult(result) = result else {
            return Err(TagteamControlError::Unavailable(
                "MCP endpoint returned an unexpected response".to_string(),
            ));
        };
        if result.is_error == Some(true) {
            let failure: WireFailure =
                serde_json::from_value(result.structured_content.ok_or_else(|| {
                    TagteamControlError::Terminal {
                        code: "malformed_error".to_string(),
                        reason: "Tagteam returned an error without structured details".to_string(),
                    }
                })?)
                .map_err(|error| TagteamControlError::Terminal {
                    code: "malformed_error".to_string(),
                    reason: format!("Tagteam returned an invalid structured error: {error}"),
                })?;
            return Err(failure.into_error());
        }
        let structured =
            result
                .structured_content
                .ok_or_else(|| TagteamControlError::Terminal {
                    code: "malformed_response".to_string(),
                    reason: "Tagteam returned no structured result".to_string(),
                })?;
        serde_json::from_value(structured).map_err(|error| TagteamControlError::Terminal {
            code: "malformed_response".to_string(),
            reason: format!("Tagteam returned an invalid structured result: {error}"),
        })
    }

    fn require_capability(&self, capability: &str) -> Result<(), TagteamControlError> {
        if self
            .capabilities
            .capabilities
            .iter()
            .any(|item| item == capability)
        {
            Ok(())
        } else {
            Err(TagteamControlError::Terminal {
                code: "unsupported_capability".to_string(),
                reason: format!("Tagteam endpoint does not advertise {capability}"),
            })
        }
    }

    fn launch_arguments(
        &self,
        launch: &TagteamLaunchSpecV1,
    ) -> Result<WireLaunch, TagteamControlError> {
        launch.validate().map_err(contract_error)?;
        let wire = WireLaunch::from_launch(launch)?;
        if wire.repository.canonical_root != self.repository.canonical_root {
            return Err(TagteamControlError::InvalidRequest(
                "launch repository does not match the connected Tagteam daemon".to_string(),
            ));
        }
        Ok(wire)
    }

    fn run_arguments(&self, run_id: &str) -> Result<Value, TagteamControlError> {
        validate_request_identifier(run_id, "run id")?;
        Ok(json!({
            "schema_version": TAGTEAM_CONTRACT_VERSION,
            "repository": self.repository,
            "run_id": run_id,
        }))
    }
}

#[async_trait]
impl TagteamControlClient for McpTagteamClient {
    async fn capabilities(&self) -> Result<TagteamCapabilitySet, TagteamControlError> {
        Ok(self.capabilities.clone())
    }

    async fn validate_launch(
        &self,
        launch: &TagteamLaunchSpecV1,
    ) -> Result<ControlLaunchValidation, TagteamControlError> {
        let response: WireLaunchValidation = self
            .call_tool(
                "tagteam_validate_launch",
                serde_json::to_value(self.launch_arguments(launch)?)
                    .map_err(|error| TagteamControlError::InvalidRequest(error.to_string()))?,
            )
            .await?;
        response.try_into()
    }

    async fn prepare_start(
        &self,
        launch: &TagteamLaunchSpecV1,
        idempotency_key: &str,
    ) -> Result<ControlStartPreparation, TagteamControlError> {
        validate_idempotency_key(idempotency_key)?;
        let response: WireStartPreparation = self
            .call_tool(
                "tagteam_prepare_start",
                json!({
                    "schema_version": TAGTEAM_CONTRACT_VERSION,
                    "launch": self.launch_arguments(launch)?,
                    "idempotency_key": idempotency_key,
                }),
            )
            .await?;
        response.try_into()
    }

    async fn start(
        &self,
        launch: &TagteamLaunchSpecV1,
        idempotency_key: &str,
        approval: ControlApproval,
    ) -> Result<super::contracts::RunHandle, TagteamControlError> {
        self.require_capability("start")?;
        validate_idempotency_key(idempotency_key)?;
        approval.validate()?;
        let response: WireRunHandle = self
            .call_tool(
                "tagteam_start",
                json!({
                    "schema_version": TAGTEAM_CONTRACT_VERSION,
                    "launch": self.launch_arguments(launch)?,
                    "idempotency_key": idempotency_key,
                    "approval": approval,
                }),
            )
            .await?;
        response.try_into()
    }

    async fn status(&self, run_id: &str) -> Result<ControlStatus, TagteamControlError> {
        validate_request_identifier(run_id, "run id")?;
        let response: WireStatus = self
            .call_tool("tagteam_status", json!({"run_id": run_id}))
            .await?;
        response.try_into()
    }

    async fn plan(
        &self,
        run_id: &str,
        request: &PageRequest,
    ) -> Result<ControlPage<ControlPlanItem>, TagteamControlError> {
        validate_request_identifier(run_id, "run id")?;
        request.validate().map_err(contract_error)?;
        let response: WirePage<WirePlanItem> = self
            .call_tool(
                "tagteam_plan",
                json!({"run_id": run_id, "cursor": request.cursor(), "limit": request.limit()}),
            )
            .await?;
        response.try_into()
    }

    async fn findings(
        &self,
        run_id: &str,
        request: &PageRequest,
    ) -> Result<ControlPage<ControlFinding>, TagteamControlError> {
        validate_request_identifier(run_id, "run id")?;
        request.validate().map_err(contract_error)?;
        let response: WirePage<WireFinding> = self
            .call_tool(
                "tagteam_findings",
                json!({"run_id": run_id, "cursor": request.cursor(), "limit": request.limit()}),
            )
            .await?;
        response.try_into()
    }

    async fn prepare_resume(
        &self,
        run_id: &str,
    ) -> Result<ControlRecoveryAssessment, TagteamControlError> {
        self.require_capability("prepare_resume")?;
        let response: WireRecoveryAssessment = self
            .call_tool("tagteam_prepare_resume", self.run_arguments(run_id)?)
            .await?;
        response.try_into()
    }

    async fn resume(
        &self,
        run_id: &str,
        approval: ControlApproval,
    ) -> Result<super::contracts::RunHandle, TagteamControlError> {
        self.require_capability("resume")?;
        approval.validate()?;
        let mut arguments = self.run_arguments(run_id)?;
        arguments["approval"] = serde_json::to_value(approval)
            .map_err(|error| TagteamControlError::InvalidRequest(error.to_string()))?;
        let response: WireRunHandle = self.call_tool("tagteam_resume", arguments).await?;
        response.try_into()
    }

    async fn cancel(
        &self,
        run_id: &str,
        approval: ControlApproval,
    ) -> Result<(), TagteamControlError> {
        self.require_capability("cancel")?;
        approval.validate()?;
        let mut arguments = self.run_arguments(run_id)?;
        arguments["approval"] = serde_json::to_value(approval)
            .map_err(|error| TagteamControlError::InvalidRequest(error.to_string()))?;
        let _: Value = self.call_tool("tagteam_cancel", arguments).await?;
        Ok(())
    }

    async fn diagnostics(&self) -> Result<ControlDiagnostics, TagteamControlError> {
        self.call_diagnostics().await
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct WireRepository {
    canonical_root: String,
    #[serde(default)]
    repo_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireRoleTarget {
    adapter: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireTeam {
    mode: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    worker: Option<WireRoleTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    coder: Option<WireRoleTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    supervisor: Option<WireRoleTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    reviewer: Option<WireRoleTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    scout: Option<WireRoleTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireTimeBudget {
    invocation_timeout_seconds: u64,
    watchdog_timeout_seconds: u64,
    wall_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WireLaunch {
    schema_version: u32,
    repository: WireRepository,
    prompt: String,
    team: WireTeam,
    allowed_paths: Vec<String>,
    rounds: u32,
    time_budget: WireTimeBudget,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    test_preset: Option<String>,
    recovery_policy: String,
}

impl WireLaunch {
    fn from_launch(launch: &TagteamLaunchSpecV1) -> Result<Self, TagteamControlError> {
        Ok(Self {
            schema_version: launch.schema_version,
            repository: WireRepository {
                canonical_root: launch.repository.canonical_root().display().to_string(),
                repo_id: String::new(),
            },
            prompt: launch.prompt.clone(),
            team: WireTeam::from_team(&launch.team)?,
            allowed_paths: launch
                .allowed_paths
                .iter()
                .map(|path| path.as_str().to_string())
                .collect(),
            rounds: launch.rounds,
            time_budget: WireTimeBudget {
                invocation_timeout_seconds: launch.time_budget.invocation_timeout_seconds,
                watchdog_timeout_seconds: launch.time_budget.watchdog_timeout_seconds,
                wall_timeout_seconds: launch.time_budget.wall_timeout_seconds,
            },
            test_preset: launch
                .test_preset
                .as_ref()
                .map(|preset| preset.as_str().to_string()),
            recovery_policy: "assist".to_string(),
        })
    }

    fn into_launch(self) -> Result<TagteamLaunchSpecV1, TagteamControlError> {
        let repository = RepositoryIdentity::from_path(&self.repository.canonical_root)
            .map_err(producer_contract_error)?;
        let allowed_paths = self
            .allowed_paths
            .into_iter()
            .map(AllowedPath::new)
            .collect::<Result<Vec<_>, _>>()
            .map_err(producer_contract_error)?;
        let test_preset = self
            .test_preset
            .map(TestPresetRef::new)
            .transpose()
            .map_err(producer_contract_error)?;
        if self.recovery_policy != "assist" {
            return Err(TagteamControlError::Terminal {
                code: "malformed_response".to_string(),
                reason: "Tagteam returned an unsupported recovery policy".to_string(),
            });
        }
        let launch = TagteamLaunchSpecV1 {
            schema_version: self.schema_version,
            repository,
            prompt: self.prompt,
            team: self.team.into_team()?,
            allowed_paths,
            rounds: self.rounds,
            time_budget: TimeBudget {
                invocation_timeout_seconds: self.time_budget.invocation_timeout_seconds,
                watchdog_timeout_seconds: self.time_budget.watchdog_timeout_seconds,
                wall_timeout_seconds: self.time_budget.wall_timeout_seconds,
            },
            test_preset,
            recovery_policy: RecoveryPolicy::Assist,
        };
        launch.validate().map_err(producer_contract_error)?;
        Ok(launch)
    }
}

impl WireTeam {
    fn from_team(team: &TeamSpec) -> Result<Self, TagteamControlError> {
        let mut wire = Self {
            mode: String::new(),
            worker: None,
            coder: None,
            supervisor: None,
            reviewer: None,
            scout: None,
        };
        match team {
            TeamSpec::Supervisor { worker, supervisor } => {
                wire.mode = "supervisor".to_string();
                wire.worker = Some(WireRoleTarget::from_role(worker)?);
                wire.supervisor = Some(WireRoleTarget::from_role(supervisor)?);
            }
            TeamSpec::Relay {
                coder,
                supervisor,
                scout,
            } => {
                wire.mode = "relay".to_string();
                wire.coder = Some(WireRoleTarget::from_role(coder)?);
                wire.supervisor = Some(WireRoleTarget::from_role(supervisor)?);
                wire.scout = Some(WireRoleTarget::from_role(scout)?);
            }
            TeamSpec::Adversarial { coder, reviewer } => {
                wire.mode = "adversarial".to_string();
                wire.coder = Some(WireRoleTarget::from_role(coder)?);
                wire.reviewer = Some(WireRoleTarget::from_role(reviewer)?);
            }
            TeamSpec::Solo { worker } => {
                wire.mode = "solo".to_string();
                wire.worker = Some(WireRoleTarget::from_role(worker)?);
            }
        }
        Ok(wire)
    }

    fn into_team(self) -> Result<TeamSpec, TagteamControlError> {
        let required = |role: Option<WireRoleTarget>, name: &str| {
            role.ok_or_else(|| TagteamControlError::Terminal {
                code: "malformed_response".to_string(),
                reason: format!("Tagteam normalized launch omitted required {name} role"),
            })?
            .into_role()
        };
        let absent = |role: &Option<WireRoleTarget>, name: &str| {
            if role.is_some() {
                Err(TagteamControlError::Terminal {
                    code: "malformed_response".to_string(),
                    reason: format!("Tagteam normalized launch included an unexpected {name} role"),
                })
            } else {
                Ok(())
            }
        };
        match self.mode.as_str() {
            "supervisor" => {
                absent(&self.coder, "coder")?;
                absent(&self.reviewer, "reviewer")?;
                absent(&self.scout, "scout")?;
                Ok(TeamSpec::Supervisor {
                    worker: required(self.worker, "worker")?,
                    supervisor: required(self.supervisor, "supervisor")?,
                })
            }
            "relay" => {
                absent(&self.worker, "worker")?;
                absent(&self.reviewer, "reviewer")?;
                Ok(TeamSpec::Relay {
                    coder: required(self.coder, "coder")?,
                    supervisor: required(self.supervisor, "supervisor")?,
                    scout: required(self.scout, "scout")?,
                })
            }
            "adversarial" => {
                absent(&self.worker, "worker")?;
                absent(&self.supervisor, "supervisor")?;
                absent(&self.scout, "scout")?;
                Ok(TeamSpec::Adversarial {
                    coder: required(self.coder, "coder")?,
                    reviewer: required(self.reviewer, "reviewer")?,
                })
            }
            "solo" => {
                absent(&self.coder, "coder")?;
                absent(&self.supervisor, "supervisor")?;
                absent(&self.reviewer, "reviewer")?;
                absent(&self.scout, "scout")?;
                Ok(TeamSpec::Solo {
                    worker: required(self.worker, "worker")?,
                })
            }
            _ => Err(TagteamControlError::Terminal {
                code: "malformed_response".to_string(),
                reason: "Tagteam normalized launch returned an unknown mode".to_string(),
            }),
        }
    }
}

impl WireRoleTarget {
    fn from_role(role: &RoleTarget) -> Result<Self, TagteamControlError> {
        let (adapter, model) = role
            .as_str()
            .split_once(':')
            .map_or((role.as_str(), None), |(adapter, model)| {
                (adapter, Some(model))
            });
        if adapter.is_empty() || model.is_some_and(str::is_empty) {
            return Err(TagteamControlError::InvalidRequest(
                "role targets must use adapter or adapter:model form".to_string(),
            ));
        }
        Ok(Self {
            adapter: adapter.to_string(),
            model: model.map(str::to_string),
        })
    }

    fn into_role(self) -> Result<RoleTarget, TagteamControlError> {
        let value = self
            .model
            .map(|model| format!("{}:{model}", self.adapter))
            .unwrap_or(self.adapter);
        RoleTarget::new(value).map_err(producer_contract_error)
    }
}

#[derive(Debug, Deserialize)]
struct WireCapabilities {
    schema_version: u32,
    producer_version: String,
    capabilities: Vec<String>,
}

impl TryFrom<WireCapabilities> for TagteamCapabilitySet {
    type Error = TagteamControlError;

    fn try_from(value: WireCapabilities) -> Result<Self, Self::Error> {
        let capabilities = TagteamCapabilitySet {
            schema_version: value.schema_version,
            producer_version: value.producer_version,
            capabilities: value.capabilities,
        };
        capabilities.validate().map_err(producer_contract_error)?;
        Ok(capabilities)
    }
}

#[derive(Debug, Deserialize)]
struct WireLaunchValidation {
    schema_version: u32,
    normalized: WireLaunch,
    launch_digest: String,
}

impl TryFrom<WireLaunchValidation> for ControlLaunchValidation {
    type Error = TagteamControlError;

    fn try_from(value: WireLaunchValidation) -> Result<Self, Self::Error> {
        validate_schema_version(value.schema_version)?;
        validate_response_digest(&value.launch_digest, "launch digest")?;
        Ok(Self {
            schema_version: value.schema_version,
            normalized: value.normalized.into_launch()?,
            launch_digest: value.launch_digest,
        })
    }
}

#[derive(Debug, Deserialize)]
struct WireStartPreparation {
    schema_version: u32,
    normalized: WireLaunch,
    action_digest: String,
    approval_max_lifetime_seconds: u64,
}

impl TryFrom<WireStartPreparation> for ControlStartPreparation {
    type Error = TagteamControlError;

    fn try_from(value: WireStartPreparation) -> Result<Self, Self::Error> {
        validate_schema_version(value.schema_version)?;
        validate_response_digest(&value.action_digest, "start action digest")?;
        if value.approval_max_lifetime_seconds == 0 || value.approval_max_lifetime_seconds > 30 * 60
        {
            return Err(TagteamControlError::Terminal {
                code: "malformed_response".to_string(),
                reason: "Tagteam returned an invalid approval lifetime".to_string(),
            });
        }
        Ok(Self {
            schema_version: value.schema_version,
            normalized: value.normalized.into_launch()?,
            action_digest: value.action_digest,
            approval_max_lifetime_seconds: value.approval_max_lifetime_seconds,
        })
    }
}

#[derive(Debug, Deserialize)]
struct WireRunHandle {
    schema_version: u32,
    run_id: String,
    producer_version: String,
}

impl TryFrom<WireRunHandle> for super::contracts::RunHandle {
    type Error = TagteamControlError;

    fn try_from(value: WireRunHandle) -> Result<Self, Self::Error> {
        let handle = super::contracts::RunHandle {
            schema_version: value.schema_version,
            run_id: value.run_id,
            producer_version: value.producer_version,
        };
        handle.validate().map_err(producer_contract_error)?;
        Ok(handle)
    }
}

#[derive(Debug, Deserialize)]
struct WireRecoveryAssessment {
    schema_version: u32,
    run_id: String,
    resumable: bool,
    reason_code: String,
    reason: String,
    #[serde(default)]
    action_digest: Option<String>,
}

impl TryFrom<WireRecoveryAssessment> for ControlRecoveryAssessment {
    type Error = TagteamControlError;

    fn try_from(value: WireRecoveryAssessment) -> Result<Self, Self::Error> {
        validate_schema_version(value.schema_version)?;
        validate_response_identifier(&value.run_id, "recovery run id")?;
        validate_text(&value.reason_code, MAX_STATUS_BYTES, "recovery reason code")?;
        validate_text(&value.reason, MAX_REASON_BYTES, "recovery reason")?;
        match (&value.resumable, &value.action_digest) {
            (true, Some(digest)) => validate_response_digest(digest, "resume action digest")?,
            (true, None) => {
                return Err(TagteamControlError::Terminal {
                    code: "malformed_response".to_string(),
                    reason: "resumable Tagteam response omitted an action digest".to_string(),
                });
            }
            (false, Some(_)) => {
                return Err(TagteamControlError::Terminal {
                    code: "malformed_response".to_string(),
                    reason: "non-resumable Tagteam response included an action digest".to_string(),
                });
            }
            (false, None) => {}
        }
        Ok(Self {
            schema_version: value.schema_version,
            run_id: value.run_id,
            resumable: value.resumable,
            reason_code: value.reason_code,
            reason: value.reason,
            action_digest: value.action_digest,
        })
    }
}

#[derive(Debug, Deserialize)]
struct WireStatus {
    schema_version: u32,
    snapshot_id: String,
    completeness: Completeness,
    truncated: bool,
    run: WireRunStatus,
}

#[derive(Debug, Deserialize)]
struct WireRunStatus {
    schema_version: u32,
    run_id: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    phase: String,
    #[serde(default)]
    verdict: String,
    #[serde(default)]
    degraded: bool,
    #[serde(default)]
    degraded_reason: String,
    #[serde(default)]
    blocking_reason: String,
    #[serde(default)]
    current_round: u32,
    #[serde(default)]
    rounds_completed: u32,
    #[serde(default)]
    rounds_requested: u32,
    #[serde(default)]
    changed_files: Vec<String>,
    #[serde(default)]
    findings_count: u32,
    #[serde(default)]
    open_major_count: u32,
    updated_at: DateTime<Utc>,
}

impl TryFrom<WireStatus> for ControlStatus {
    type Error = TagteamControlError;

    fn try_from(value: WireStatus) -> Result<Self, Self::Error> {
        validate_schema_version(value.schema_version)?;
        validate_response_digest(&value.snapshot_id, "snapshot id")?;
        let run = ControlRunStatus {
            schema_version: value.run.schema_version,
            run_id: value.run.run_id,
            status: value.run.status,
            phase: value.run.phase,
            verdict: value.run.verdict,
            degraded: value.run.degraded,
            degraded_reason: value.run.degraded_reason,
            blocking_reason: value.run.blocking_reason,
            current_round: value.run.current_round,
            rounds_completed: value.run.rounds_completed,
            rounds_requested: value.run.rounds_requested,
            changed_files: value.run.changed_files,
            findings_count: value.run.findings_count,
            open_major_count: value.run.open_major_count,
            updated_at: value.run.updated_at,
        };
        run.validate()?;
        Ok(Self {
            schema_version: value.schema_version,
            snapshot_id: value.snapshot_id,
            completeness: value.completeness,
            truncated: value.truncated,
            run,
        })
    }
}

impl ControlRunStatus {
    fn validate(&self) -> Result<(), TagteamControlError> {
        validate_schema_version(self.schema_version)?;
        validate_response_identifier(&self.run_id, "run id")?;
        validate_optional_text(&self.status, "status")?;
        validate_optional_text(&self.phase, "phase")?;
        validate_optional_text(&self.verdict, "verdict")?;
        validate_optional_text(&self.degraded_reason, "degraded reason")?;
        validate_optional_text(&self.blocking_reason, "blocking reason")?;
        if self.changed_files.len() > 128 {
            return Err(TagteamControlError::Terminal {
                code: "malformed_response".to_string(),
                reason: "Tagteam status contains too many changed files".to_string(),
            });
        }
        for path in &self.changed_files {
            AllowedPath::new(path.clone()).map_err(producer_contract_error)?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize)]
struct WirePage<T> {
    schema_version: u32,
    items: Vec<T>,
    #[serde(default)]
    next_cursor: Option<String>,
    completeness: Completeness,
    truncated: bool,
}

#[derive(Debug, Deserialize)]
struct WirePlanItem {
    id: String,
    title: String,
    status: String,
    #[serde(default)]
    owner: Option<String>,
    #[serde(default)]
    reason: Option<String>,
}

impl TryFrom<WirePage<WirePlanItem>> for ControlPage<ControlPlanItem> {
    type Error = TagteamControlError;

    fn try_from(value: WirePage<WirePlanItem>) -> Result<Self, Self::Error> {
        validate_page_metadata(
            value.schema_version,
            value.items.len(),
            value.next_cursor.as_deref(),
        )?;
        let items = value
            .items
            .into_iter()
            .map(|item| {
                validate_response_identifier(&item.id, "plan item id")?;
                validate_text(&item.title, MAX_PRODUCER_TEXT_BYTES, "plan item title")?;
                validate_text(&item.status, MAX_STATUS_BYTES, "plan item status")?;
                if let Some(owner) = &item.owner {
                    validate_text(owner, MAX_PRODUCER_TEXT_BYTES, "plan item owner")?;
                }
                if let Some(reason) = &item.reason {
                    validate_text(reason, MAX_PRODUCER_TEXT_BYTES, "plan item reason")?;
                }
                Ok(ControlPlanItem {
                    id: item.id,
                    title: item.title,
                    status: item.status,
                    owner: item.owner,
                    reason: item.reason,
                })
            })
            .collect::<Result<Vec<_>, TagteamControlError>>()?;
        Ok(ControlPage {
            schema_version: value.schema_version,
            items,
            next_cursor: value.next_cursor,
            completeness: value.completeness,
            truncated: value.truncated,
        })
    }
}

#[derive(Debug, Deserialize)]
struct WireFinding {
    id: String,
    source: String,
    severity: String,
    status: String,
    #[serde(default)]
    file: Option<String>,
    #[serde(default)]
    line: Option<u32>,
    issue: String,
    #[serde(default)]
    fix: Option<String>,
}

impl TryFrom<WirePage<WireFinding>> for ControlPage<ControlFinding> {
    type Error = TagteamControlError;

    fn try_from(value: WirePage<WireFinding>) -> Result<Self, Self::Error> {
        validate_page_metadata(
            value.schema_version,
            value.items.len(),
            value.next_cursor.as_deref(),
        )?;
        let items = value
            .items
            .into_iter()
            .map(|item| {
                validate_response_identifier(&item.id, "finding id")?;
                validate_text(&item.source, MAX_STATUS_BYTES, "finding source")?;
                validate_text(&item.severity, MAX_STATUS_BYTES, "finding severity")?;
                validate_text(&item.status, MAX_STATUS_BYTES, "finding status")?;
                validate_text(&item.issue, MAX_REASON_BYTES, "finding issue")?;
                if let Some(file) = &item.file {
                    validate_text(file, MAX_PRODUCER_TEXT_BYTES, "finding file")?;
                }
                if let Some(fix) = &item.fix {
                    validate_text(fix, MAX_REASON_BYTES, "finding fix")?;
                }
                Ok(ControlFinding {
                    id: item.id,
                    source: item.source,
                    severity: item.severity,
                    status: item.status,
                    file: item.file,
                    line: item.line,
                    issue: item.issue,
                    fix: item.fix,
                })
            })
            .collect::<Result<Vec<_>, TagteamControlError>>()?;
        Ok(ControlPage {
            schema_version: value.schema_version,
            items,
            next_cursor: value.next_cursor,
            completeness: value.completeness,
            truncated: value.truncated,
        })
    }
}

#[derive(Debug, Deserialize)]
struct WireDiagnostics {
    schema_version: u32,
    status: String,
    repository: WireRepository,
    state_root: String,
    details: Vec<String>,
    completeness: Completeness,
}

impl TryFrom<WireDiagnostics> for ControlDiagnostics {
    type Error = TagteamControlError;

    fn try_from(value: WireDiagnostics) -> Result<Self, Self::Error> {
        validate_schema_version(value.schema_version)?;
        validate_text(&value.status, MAX_STATUS_BYTES, "diagnostic status")?;
        validate_text(
            &value.repository.canonical_root,
            MAX_PRODUCER_TEXT_BYTES,
            "repository root",
        )?;
        validate_text(&value.repository.repo_id, MAX_STATUS_BYTES, "repository id")?;
        validate_text(&value.state_root, MAX_PRODUCER_TEXT_BYTES, "state root")?;
        if value.details.len() > MAX_DIAGNOSTIC_DETAILS {
            return Err(TagteamControlError::Terminal {
                code: "malformed_response".to_string(),
                reason: "Tagteam diagnostics contains too many details".to_string(),
            });
        }
        for detail in &value.details {
            validate_text(detail, MAX_PRODUCER_TEXT_BYTES, "diagnostic detail")?;
        }
        Ok(Self {
            schema_version: value.schema_version,
            status: value.status,
            canonical_root: value.repository.canonical_root,
            repo_id: value.repository.repo_id,
            state_root: value.state_root,
            details: value.details,
            completeness: value.completeness,
        })
    }
}

#[derive(Debug, Deserialize)]
struct WireFailure {
    code: String,
    reason: String,
    recoverable: bool,
}

impl WireFailure {
    fn into_error(self) -> TagteamControlError {
        if !is_valid_text(&self.code, MAX_STATUS_BYTES)
            || !is_valid_text(&self.reason, MAX_REASON_BYTES)
        {
            return TagteamControlError::Terminal {
                code: "malformed_error".to_string(),
                reason: "Tagteam returned an invalid structured error".to_string(),
            };
        }
        if self.code.starts_with("approval_") {
            return TagteamControlError::ApprovalRequired;
        }
        if self.recoverable {
            TagteamControlError::Retryable {
                code: self.code,
                reason: self.reason,
            }
        } else {
            TagteamControlError::Terminal {
                code: self.code,
                reason: self.reason,
            }
        }
    }
}

fn validate_schema_version(schema_version: u32) -> Result<(), TagteamControlError> {
    if schema_version == TAGTEAM_CONTRACT_VERSION {
        Ok(())
    } else {
        Err(TagteamControlError::IncompatibleSchema {
            actual: schema_version,
            supported: TAGTEAM_CONTRACT_VERSION,
        })
    }
}

fn is_valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn validate_request_digest(value: &str, label: &str) -> Result<(), TagteamControlError> {
    if is_valid_digest(value) {
        Ok(())
    } else {
        Err(TagteamControlError::InvalidRequest(format!(
            "{label} is malformed"
        )))
    }
}

fn validate_response_digest(value: &str, label: &str) -> Result<(), TagteamControlError> {
    if is_valid_digest(value) {
        Ok(())
    } else {
        Err(TagteamControlError::Terminal {
            code: "malformed_response".to_string(),
            reason: format!("Tagteam returned an invalid {label}"),
        })
    }
}

fn is_valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && value.trim() == value
        && !value.chars().any(char::is_control)
}

fn validate_request_identifier(value: &str, label: &str) -> Result<(), TagteamControlError> {
    if is_valid_identifier(value) {
        Ok(())
    } else {
        Err(TagteamControlError::InvalidRequest(format!(
            "{label} is malformed"
        )))
    }
}

fn validate_response_identifier(value: &str, label: &str) -> Result<(), TagteamControlError> {
    if is_valid_identifier(value) {
        Ok(())
    } else {
        Err(TagteamControlError::Terminal {
            code: "malformed_response".to_string(),
            reason: format!("Tagteam returned an invalid {label}"),
        })
    }
}

fn validate_idempotency_key(value: &str) -> Result<(), TagteamControlError> {
    if value.is_empty()
        || value.len() > MAX_IDEMPOTENCY_KEY_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(TagteamControlError::InvalidRequest(
            "idempotency key is malformed".to_string(),
        ));
    }
    Ok(())
}

fn validate_text(value: &str, max_bytes: usize, label: &str) -> Result<(), TagteamControlError> {
    if !is_valid_text(value, max_bytes) {
        return Err(TagteamControlError::Terminal {
            code: "malformed_response".to_string(),
            reason: format!("Tagteam returned an invalid {label}"),
        });
    }
    Ok(())
}

fn is_valid_text(value: &str, max_bytes: usize) -> bool {
    !value.is_empty() && value.len() <= max_bytes && !value.chars().any(char::is_control)
}

fn validate_optional_text(value: &str, label: &str) -> Result<(), TagteamControlError> {
    if value.is_empty() {
        return Ok(());
    }
    validate_text(value, MAX_PRODUCER_TEXT_BYTES, label)
}

fn validate_page_metadata(
    schema_version: u32,
    item_count: usize,
    next_cursor: Option<&str>,
) -> Result<(), TagteamControlError> {
    validate_schema_version(schema_version)?;
    if item_count > MAX_PAGE_ITEMS {
        return Err(TagteamControlError::Terminal {
            code: "malformed_response".to_string(),
            reason: "Tagteam returned too many page items".to_string(),
        });
    }
    if let Some(cursor) = next_cursor {
        validate_text(cursor, 1024, "page cursor")?;
    }
    Ok(())
}

fn require_capabilities<'a>(
    capabilities: &TagteamCapabilitySet,
    required: impl IntoIterator<Item = &'a str>,
) -> Result<(), TagteamControlError> {
    let advertised: HashSet<&str> = capabilities
        .capabilities
        .iter()
        .map(String::as_str)
        .collect();
    for capability in required {
        if !advertised.contains(capability) {
            return Err(TagteamControlError::Unavailable(format!(
                "Tagteam endpoint omitted required capability {capability}"
            )));
        }
    }
    Ok(())
}

fn contract_error(error: ContractError) -> TagteamControlError {
    match error {
        ContractError::UnsupportedVersion { actual, expected } => {
            TagteamControlError::IncompatibleSchema {
                actual,
                supported: expected,
            }
        }
        error => TagteamControlError::InvalidRequest(error.to_string()),
    }
}

fn producer_contract_error(error: ContractError) -> TagteamControlError {
    match error {
        ContractError::UnsupportedVersion { actual, expected } => {
            TagteamControlError::IncompatibleSchema {
                actual,
                supported: expected,
            }
        }
        error => TagteamControlError::Terminal {
            code: "malformed_response".to_string(),
            reason: format!("Tagteam returned an invalid normalized action: {error}"),
        },
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        process::Command,
        sync::{Arc, Mutex as StdMutex},
    };
    use tempfile::TempDir;
    use tokio::{
        io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
        net::UnixListener,
        task::JoinHandle,
    };

    struct FixtureServer {
        socket_path: PathBuf,
        requests: Arc<StdMutex<Vec<Value>>>,
        _socket_dir: TempDir,
        task: JoinHandle<()>,
    }

    impl FixtureServer {
        async fn start(repository: &std::path::Path, fail_status: bool) -> Self {
            let socket_dir = tempfile::Builder::new()
                .prefix("gt")
                .tempdir_in("/tmp")
                .unwrap();
            let socket_path = socket_dir.path().join("mcp.sock");
            let listener = UnixListener::bind(&socket_path).unwrap();
            let requests = Arc::new(StdMutex::new(Vec::new()));
            let captured_requests = requests.clone();
            let repository = std::fs::canonicalize(repository)
                .unwrap()
                .display()
                .to_string();
            let task = tokio::spawn(async move {
                loop {
                    let (stream, _) = listener.accept().await.unwrap();
                    let (reader, mut writer) = stream.into_split();
                    let mut reader = BufReader::new(reader);
                    let mut line = String::new();
                    loop {
                        line.clear();
                        if reader.read_line(&mut line).await.unwrap() == 0 {
                            break;
                        }
                        let request: Value = serde_json::from_str(&line).unwrap();
                        captured_requests.lock().unwrap().push(request.clone());
                        let Some(id) = request.get("id") else {
                            continue;
                        };
                        let response = match request["method"].as_str().unwrap() {
                            "initialize" => json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "protocolVersion": TAGTEAM_MCP_PROTOCOL_VERSION,
                                    "capabilities": {"tools": {}},
                                    "serverInfo": {"name": "tagteam", "version": "fixture"}
                                }
                            }),
                            "tools/call" => {
                                let name = request["params"]["name"].as_str().unwrap();
                                let result = match name {
                                    "tagteam_capabilities" => success(json!({
                                        "schema_version": 1,
                                        "producer_version": "fixture",
                                        "capabilities": [
                                            "capabilities", "validate_launch", "prepare_start",
                                            "prepare_resume", "status", "plan", "findings",
                                            "diagnostics", "start", "resume", "cancel"
                                        ]
                                    })),
                                    "tagteam_diagnostics" => success(json!({
                                        "schema_version": 1,
                                        "status": "ready",
                                        "repository": {"canonical_root": repository, "repo_id": "fixture-repo"},
                                        "state_root": "/tmp/tagteam-state",
                                        "details": ["repository identity verified"],
                                        "completeness": "complete"
                                    })),
                                    "tagteam_validate_launch" => {
                                        let launch = request["params"]["arguments"].clone();
                                        success(json!({
                                            "schema_version": 1,
                                            "normalized": launch,
                                            "launch_digest": digest('a')
                                        }))
                                    }
                                    "tagteam_prepare_start" => {
                                        let arguments = request["params"]["arguments"].clone();
                                        success(json!({
                                            "schema_version": 1,
                                            "normalized": arguments["launch"].clone(),
                                            "action_digest": digest('b'),
                                            "approval_max_lifetime_seconds": 1800
                                        }))
                                    }
                                    "tagteam_start" => success(json!({
                                        "schema_version": 1,
                                        "run_id": "run-fixture-1",
                                        "producer_version": "fixture"
                                    })),
                                    "tagteam_status" if fail_status => failure(
                                        "run_not_owned",
                                        "this daemon does not own the run",
                                        false,
                                    ),
                                    "tagteam_status" => success(json!({
                                        "schema_version": 1,
                                        "snapshot_id": digest('c'),
                                        "completeness": "complete",
                                        "truncated": false,
                                        "run": {
                                            "schema_version": 1,
                                            "run_id": "run-fixture-1",
                                            "status": "running",
                                            "phase": "implementation",
                                            "verdict": "",
                                            "degraded": false,
                                            "degraded_reason": "",
                                            "blocking_reason": "",
                                            "current_round": 1,
                                            "rounds_completed": 0,
                                            "rounds_requested": 2,
                                            "changed_files": ["src/lib.rs"],
                                            "findings_count": 0,
                                            "open_major_count": 0,
                                            "updated_at": "2026-07-14T00:00:00Z"
                                        }
                                    })),
                                    _ => failure(
                                        "unsupported",
                                        "fixture received an unsupported tool",
                                        false,
                                    ),
                                };
                                json!({"jsonrpc": "2.0", "id": id, "result": result})
                            }
                            _ => json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {"code": -32601, "message": "method not found"}
                            }),
                        };
                        writer
                            .write_all(
                                format!("{}\n", serde_json::to_string(&response).unwrap())
                                    .as_bytes(),
                            )
                            .await
                            .unwrap();
                    }
                }
            });
            Self {
                socket_path,
                requests,
                _socket_dir: socket_dir,
                task,
            }
        }
    }

    impl Drop for FixtureServer {
        fn drop(&mut self) {
            self.task.abort();
        }
    }

    fn success(value: Value) -> Value {
        json!({"content": [], "structuredContent": value, "isError": false})
    }

    fn failure(code: &str, reason: &str, recoverable: bool) -> Value {
        json!({
            "content": [],
            "structuredContent": {"code": code, "reason": reason, "recoverable": recoverable},
            "isError": true
        })
    }

    fn digest(character: char) -> String {
        std::iter::repeat_n(character, 64).collect()
    }

    fn launch(root: &std::path::Path) -> TagteamLaunchSpecV1 {
        let src = root.join("src");
        std::fs::create_dir_all(&src).unwrap();
        assert!(Command::new("git")
            .args(["init", "-q"])
            .arg(root)
            .status()
            .unwrap()
            .success());
        TagteamLaunchSpecV1 {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            repository: RepositoryIdentity::from_path(root).unwrap(),
            prompt: "implement the approved slice".to_string(),
            team: TeamSpec::Solo {
                worker: RoleTarget::new("codex:gpt-5.6-luna-high").unwrap(),
            },
            allowed_paths: vec![AllowedPath::new("src").unwrap()],
            rounds: 2,
            time_budget: TimeBudget {
                invocation_timeout_seconds: 60,
                watchdog_timeout_seconds: 30,
                wall_timeout_seconds: 300,
            },
            test_preset: None,
            recovery_policy: RecoveryPolicy::Assist,
        }
    }

    #[tokio::test]
    async fn socket_adapter_forwards_prepared_approval_and_decodes_status() {
        let repository = tempfile::Builder::new()
            .prefix("gr")
            .tempdir_in("/tmp")
            .unwrap();
        let launch = launch(repository.path());
        let fixture = FixtureServer::start(repository.path(), false).await;
        let client =
            McpTagteamClient::connect_unix_socket(&fixture.socket_path, StdDuration::from_secs(2))
                .await
                .unwrap();

        let validated = client.validate_launch(&launch).await.unwrap();
        assert_eq!(validated.launch_digest, digest('a'));
        let prepared = client
            .prepare_start(&launch, "session-1-generation-1")
            .await
            .unwrap();
        assert_eq!(prepared.action_digest, digest('b'));
        let approval = ControlApproval {
            action_digest: prepared.action_digest.clone(),
            approved_at: Utc::now(),
            expires_at: Utc::now() + Duration::minutes(10),
            nonce: "user-confirmed-approval-1".to_string(),
        };
        let handle = client
            .start(&launch, "session-1-generation-1", approval.clone())
            .await
            .unwrap();
        assert_eq!(handle.run_id, "run-fixture-1");
        let status = client.status(&handle.run_id).await.unwrap();
        assert_eq!(status.run.phase, "implementation");
        assert_eq!(status.run.changed_files, ["src/lib.rs"]);

        let requests = fixture.requests.lock().unwrap();
        let start = requests
            .iter()
            .find(|request| request["params"]["name"] == "tagteam_start")
            .unwrap();
        assert_eq!(
            start["params"]["arguments"]["approval"],
            serde_json::to_value(approval).unwrap()
        );
        assert_eq!(
            start["params"]["arguments"]["launch"]["team"]["worker"]["adapter"],
            "codex"
        );
        assert_eq!(
            start["params"]["arguments"]["launch"]["team"]["worker"]["model"],
            "gpt-5.6-luna-high"
        );
    }

    #[tokio::test]
    async fn socket_adapter_preserves_structured_terminal_failures() {
        let repository = tempfile::Builder::new()
            .prefix("gr")
            .tempdir_in("/tmp")
            .unwrap();
        let _launch = launch(repository.path());
        let fixture = FixtureServer::start(repository.path(), true).await;
        let client =
            McpTagteamClient::connect_unix_socket(&fixture.socket_path, StdDuration::from_secs(2))
                .await
                .unwrap();

        assert_eq!(
            client.status("run-fixture-1").await,
            Err(TagteamControlError::Terminal {
                code: "run_not_owned".to_string(),
                reason: "this daemon does not own the run".to_string(),
            })
        );
    }

    #[tokio::test]
    async fn socket_adapter_reconnects_to_the_same_daemon_run() {
        let repository = tempfile::Builder::new()
            .prefix("gr")
            .tempdir_in("/tmp")
            .unwrap();
        let _launch = launch(repository.path());
        let fixture = FixtureServer::start(repository.path(), false).await;

        let first =
            McpTagteamClient::connect_unix_socket(&fixture.socket_path, StdDuration::from_secs(2))
                .await
                .unwrap();
        assert_eq!(
            first.status("run-fixture-1").await.unwrap().run.status,
            "running"
        );
        drop(first);

        let second =
            McpTagteamClient::connect_unix_socket(&fixture.socket_path, StdDuration::from_secs(2))
                .await
                .unwrap();
        assert_eq!(
            second.status("run-fixture-1").await.unwrap().run.phase,
            "implementation"
        );

        let initialize_requests = fixture
            .requests
            .lock()
            .unwrap()
            .iter()
            .filter(|request| request["method"] == "initialize")
            .count();
        assert_eq!(initialize_requests, 2);
    }

    #[tokio::test]
    #[ignore = "requires TAGTEAM_MCP_SOCKET for a real local Tagteam daemon"]
    async fn live_tagteam_socket_smoke_test() {
        let socket = std::env::var_os("TAGTEAM_MCP_SOCKET")
            .expect("TAGTEAM_MCP_SOCKET must point to a running local Tagteam daemon");
        let client = McpTagteamClient::connect_unix_socket(socket, StdDuration::from_secs(5))
            .await
            .expect("real Tagteam daemon should satisfy the read-only control contract");
        let capabilities = client.capabilities().await.unwrap();
        let diagnostics = client.diagnostics().await.unwrap();

        assert_eq!(capabilities.schema_version, TAGTEAM_CONTRACT_VERSION);
        assert_eq!(diagnostics.schema_version, TAGTEAM_CONTRACT_VERSION);
        assert!(!diagnostics.canonical_root.is_empty());
    }

    #[test]
    fn malformed_producer_errors_and_team_shapes_fail_closed() {
        assert_eq!(
            WireFailure {
                code: "approval_\nrequired".to_string(),
                reason: "invalid".to_string(),
                recoverable: true,
            }
            .into_error(),
            TagteamControlError::Terminal {
                code: "malformed_error".to_string(),
                reason: "Tagteam returned an invalid structured error".to_string(),
            }
        );

        let team = WireTeam {
            mode: "solo".to_string(),
            worker: Some(WireRoleTarget {
                adapter: "codex".to_string(),
                model: Some("gpt-5.6-luna-high".to_string()),
            }),
            coder: Some(WireRoleTarget {
                adapter: "grok".to_string(),
                model: Some("grok-4.5".to_string()),
            }),
            supervisor: None,
            reviewer: None,
            scout: None,
        };
        assert_eq!(
            team.into_team(),
            Err(TagteamControlError::Terminal {
                code: "malformed_response".to_string(),
                reason: "Tagteam normalized launch included an unexpected coder role".to_string(),
            })
        );
    }

    #[test]
    fn control_approval_rejects_expired_or_overlong_records() {
        let now = Utc::now();
        let expired = ControlApproval {
            action_digest: digest('a'),
            approved_at: now,
            expires_at: now,
            nonce: "approval".to_string(),
        };
        assert!(expired.validate().is_err());
        let overlong = ControlApproval {
            action_digest: digest('a'),
            approved_at: now,
            expires_at: now + Duration::minutes(31),
            nonce: "approval".to_string(),
        };
        assert!(overlong.validate().is_err());
    }
}
