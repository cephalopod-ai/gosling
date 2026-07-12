use super::contracts::{
    Approval, ContractError, DiagnosticSummary, FindingSummary, Page, PageRequest, PlanItem,
    RecoveryAssessment, RunHandle, TagteamCapabilitySet, TagteamLaunchSpecV1,
};
use super::reducer::TagteamRunSnapshot;
use async_trait::async_trait;

#[cfg(test)]
use super::contracts::{MAX_IDENTIFIER_BYTES, TAGTEAM_CONTRACT_VERSION};

pub const MAX_SNAPSHOT_TEXT_BYTES: usize = 4096;
pub const MAX_SNAPSHOT_CHANGED_PATHS: usize = 128;

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TagteamClientError {
    #[error("invalid Tagteam request: {0}")]
    InvalidRequest(String),
    #[error("Tagteam producer schema {actual} is incompatible with supported schema {supported}")]
    IncompatibleSchema { actual: u32, supported: u32 },
    #[error("Tagteam control plane is unavailable: {0}")]
    Unavailable(String),
    #[error("Tagteam action requires a matching unexpired approval")]
    ApprovalRequired,
    #[error("Tagteam run was not found: {0}")]
    RunNotFound(String),
    #[error("Tagteam operation failed and may be retried: {0}")]
    Retryable(String),
    #[error("Tagteam operation failed terminally: {0}")]
    Terminal(String),
}

#[async_trait]
pub trait TagteamClient: Send + Sync {
    async fn capabilities(&self) -> Result<TagteamCapabilitySet, TagteamClientError>;

    async fn validate_launch(&self, spec: &TagteamLaunchSpecV1) -> Result<(), TagteamClientError>;

    async fn start(
        &self,
        spec: TagteamLaunchSpecV1,
        idempotency_key: &str,
        approval: Approval,
    ) -> Result<RunHandle, TagteamClientError>;

    async fn status(&self, run_id: &str) -> Result<TagteamRunSnapshot, TagteamClientError>;

    async fn plan(
        &self,
        run_id: &str,
        request: &PageRequest,
    ) -> Result<Page<PlanItem>, TagteamClientError>;

    async fn findings(
        &self,
        run_id: &str,
        request: &PageRequest,
    ) -> Result<Page<FindingSummary>, TagteamClientError>;

    async fn prepare_resume(&self, run_id: &str) -> Result<RecoveryAssessment, TagteamClientError>;

    async fn resume(
        &self,
        run_id: &str,
        approval: Approval,
    ) -> Result<RunHandle, TagteamClientError>;

    async fn cancel(&self, run_id: &str, approval: Approval) -> Result<(), TagteamClientError>;

    async fn diagnostics(&self) -> Result<DiagnosticSummary, TagteamClientError>;
}

pub fn validate_run_snapshot(snapshot: &TagteamRunSnapshot) -> Result<(), ContractError> {
    super::reducer::validate_snapshot(snapshot)
}

#[cfg(test)]
fn request_error(error: ContractError) -> TagteamClientError {
    TagteamClientError::InvalidRequest(error.to_string())
}

#[cfg(test)]
fn response_error(error: ContractError) -> TagteamClientError {
    match error {
        ContractError::UnsupportedVersion { actual, expected } => {
            TagteamClientError::IncompatibleSchema {
                actual,
                supported: expected,
            }
        }
        error => TagteamClientError::Terminal(format!(
            "producer response failed contract validation: {error}"
        )),
    }
}

#[cfg(test)]
fn validate_request_run_id(run_id: &str) -> Result<(), TagteamClientError> {
    if run_id.is_empty()
        || run_id.trim() != run_id
        || run_id.len() > MAX_IDENTIFIER_BYTES
        || run_id.chars().any(char::is_control)
    {
        return Err(TagteamClientError::InvalidRequest(
            "run id is malformed".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod conformance {
    use super::super::contracts::ApprovalRequest;
    use super::*;

    pub(crate) async fn assert_consumer_contract<C, I>(
        client: &C,
        launch: TagteamLaunchSpecV1,
        issue_approval: I,
    ) where
        C: TagteamClient,
        I: Fn(&ApprovalRequest) -> Approval,
    {
        let capabilities = client.capabilities().await.unwrap();
        capabilities.validate().unwrap();
        assert!(capabilities
            .capabilities
            .iter()
            .any(|capability| capability == "status"));

        let idempotency_key = "conformance-session-generation-1";
        let start_request = ApprovalRequest::for_start(&launch, idempotency_key).unwrap();
        let start_approval = issue_approval(&start_request);
        let handle = client
            .start(launch.clone(), idempotency_key, start_approval.clone())
            .await
            .unwrap();
        handle.validate().unwrap();
        assert_eq!(
            client
                .start(launch.clone(), idempotency_key, start_approval)
                .await,
            Err(TagteamClientError::ApprovalRequired)
        );
        let mut invalid_launch = launch;
        invalid_launch.prompt.clear();
        assert!(matches!(
            client
                .start(
                    invalid_launch,
                    "invalid-launch",
                    Approval::from_token("forged-conformance-token").unwrap(),
                )
                .await,
            Err(TagteamClientError::InvalidRequest(_))
        ));
        let snapshot = client.status(&handle.run_id).await.unwrap();
        validate_run_snapshot(&snapshot).unwrap();

        let first_page = PageRequest::first(10).unwrap();
        client
            .plan(&handle.run_id, &first_page)
            .await
            .unwrap()
            .validate(&first_page)
            .unwrap();
        client
            .findings(&handle.run_id, &first_page)
            .await
            .unwrap()
            .validate(&first_page)
            .unwrap();

        let recovery = client.prepare_resume(&handle.run_id).await.unwrap();
        recovery.validate().unwrap();
        let resume_request = recovery.approval_request.as_ref().unwrap();
        let resume_approval = issue_approval(resume_request);
        assert_eq!(
            client.cancel(&handle.run_id, resume_approval.clone()).await,
            Err(TagteamClientError::ApprovalRequired)
        );
        client
            .resume(&handle.run_id, resume_approval.clone())
            .await
            .unwrap();
        assert_eq!(
            client.resume(&handle.run_id, resume_approval).await,
            Err(TagteamClientError::ApprovalRequired)
        );

        let invalid_page: PageRequest =
            serde_json::from_value(serde_json::json!({"cursor": null, "limit": 0})).unwrap();
        assert!(matches!(
            client.plan(&handle.run_id, &invalid_page).await,
            Err(TagteamClientError::InvalidRequest(_))
        ));

        let cancel_request = ApprovalRequest::for_cancel(&handle.run_id).unwrap();
        client
            .cancel(&handle.run_id, issue_approval(&cancel_request))
            .await
            .unwrap();
        client.diagnostics().await.unwrap().validate().unwrap();
    }
}

#[cfg(test)]
mod tests {
    use super::super::contracts::{
        AllowedPath, ApprovalAction, ApprovalRequest, Completeness, PageItemContract,
        RecoveryPolicy, RepositoryIdentity, RoleTarget, TeamSpec, TestPresetRef, TimeBudget,
    };
    use super::super::reducer::{RunClass, TagteamRunSnapshot};
    use super::*;
    use chrono::{DateTime, Duration, Utc};
    use sha2::{Digest, Sha256};
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Mutex;
    use tempfile::TempDir;
    use uuid::Uuid;

    #[derive(Debug)]
    struct IssuedApproval {
        action_digest: String,
        expires_at: DateTime<Utc>,
        consumed: bool,
    }

    struct FixtureClient {
        schema_version: u32,
        runs: Mutex<HashMap<String, TagteamRunSnapshot>>,
        approvals: Mutex<HashMap<String, IssuedApproval>>,
    }

    impl FixtureClient {
        fn new() -> Self {
            Self {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                runs: Mutex::new(HashMap::new()),
                approvals: Mutex::new(HashMap::new()),
            }
        }

        fn with_schema_version(schema_version: u32) -> Self {
            Self {
                schema_version,
                runs: Mutex::new(HashMap::new()),
                approvals: Mutex::new(HashMap::new()),
            }
        }

        fn issue_approval(
            &self,
            request: &ApprovalRequest,
            lifetime: Duration,
        ) -> Result<Approval, TagteamClientError> {
            self.issue_approval_until(request, Utc::now() + lifetime)
        }

        fn issue_approval_until(
            &self,
            request: &ApprovalRequest,
            expires_at: DateTime<Utc>,
        ) -> Result<Approval, TagteamClientError> {
            request.validate().map_err(request_error)?;
            let token = format!("approval-{}", Uuid::new_v4().simple());
            self.approvals.lock().unwrap().insert(
                token.clone(),
                IssuedApproval {
                    action_digest: request.action_digest.clone(),
                    expires_at,
                    consumed: false,
                },
            );
            Approval::from_token(token).map_err(request_error)
        }

        fn consume_approval(
            &self,
            approval: &Approval,
            expected: &ApprovalRequest,
        ) -> Result<(), TagteamClientError> {
            approval
                .validate()
                .map_err(|_| TagteamClientError::ApprovalRequired)?;
            expected
                .validate()
                .map_err(|_| TagteamClientError::ApprovalRequired)?;
            let mut approvals = self.approvals.lock().unwrap();
            let issued = approvals
                .get_mut(approval.token())
                .ok_or(TagteamClientError::ApprovalRequired)?;
            if issued.consumed
                || issued.expires_at <= Utc::now()
                || issued.action_digest != expected.action_digest
            {
                return Err(TagteamClientError::ApprovalRequired);
            }
            issued.consumed = true;
            Ok(())
        }

        fn set_state(&self, run_id: &str, sequence: u64, class: RunClass) {
            let mut runs = self.runs.lock().unwrap();
            let snapshot = runs.get_mut(run_id).unwrap();
            snapshot.last_sequence = sequence;
            snapshot.last_observation_digest = Some("0".repeat(64));
            snapshot.class = class;
            snapshot.producer_status = format!("{class:?}").to_lowercase();
            snapshot.completeness = Completeness::Complete;
            snapshot.updated_at = Utc::now();
        }
    }

    #[async_trait]
    impl TagteamClient for FixtureClient {
        async fn capabilities(&self) -> Result<TagteamCapabilitySet, TagteamClientError> {
            if self.schema_version != TAGTEAM_CONTRACT_VERSION {
                return Err(TagteamClientError::IncompatibleSchema {
                    actual: self.schema_version,
                    supported: TAGTEAM_CONTRACT_VERSION,
                });
            }
            let capabilities = TagteamCapabilitySet {
                schema_version: self.schema_version,
                producer_version: "fixture-v1".to_string(),
                capabilities: vec![
                    "start".to_string(),
                    "status".to_string(),
                    "plan".to_string(),
                    "findings".to_string(),
                    "prepare_resume".to_string(),
                    "resume".to_string(),
                    "cancel".to_string(),
                    "diagnostics".to_string(),
                ],
            };
            capabilities.validate().map_err(response_error)?;
            Ok(capabilities)
        }

        async fn validate_launch(
            &self,
            spec: &TagteamLaunchSpecV1,
        ) -> Result<(), TagteamClientError> {
            spec.validate().map_err(request_error)
        }

        async fn start(
            &self,
            spec: TagteamLaunchSpecV1,
            idempotency_key: &str,
            approval: Approval,
        ) -> Result<RunHandle, TagteamClientError> {
            self.validate_launch(&spec).await?;
            let expected =
                ApprovalRequest::for_start(&spec, idempotency_key).map_err(request_error)?;
            self.consume_approval(&approval, &expected)?;

            let digest_prefix: String = expected.action_digest.chars().take(24).collect();
            let run_id = format!("run-{digest_prefix}");
            let handle = RunHandle {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                run_id: run_id.clone(),
                producer_version: "fixture-v1".to_string(),
            };
            handle.validate().map_err(response_error)?;
            self.runs.lock().unwrap().entry(run_id).or_insert_with(|| {
                TagteamRunSnapshot::configured(handle.run_id.clone(), Utc::now())
            });
            Ok(handle)
        }

        async fn status(&self, run_id: &str) -> Result<TagteamRunSnapshot, TagteamClientError> {
            validate_request_run_id(run_id)?;
            let snapshot = self
                .runs
                .lock()
                .unwrap()
                .get(run_id)
                .cloned()
                .ok_or_else(|| TagteamClientError::RunNotFound(run_id.to_string()))?;
            validate_run_snapshot(&snapshot).map_err(response_error)?;
            Ok(snapshot)
        }

        async fn plan(
            &self,
            run_id: &str,
            request: &PageRequest,
        ) -> Result<Page<PlanItem>, TagteamClientError> {
            request.validate().map_err(request_error)?;
            self.status(run_id).await?;
            paginate(
                vec![
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
                request,
            )
        }

        async fn findings(
            &self,
            run_id: &str,
            request: &PageRequest,
        ) -> Result<Page<FindingSummary>, TagteamClientError> {
            request.validate().map_err(request_error)?;
            self.status(run_id).await?;
            paginate(
                vec![
                    FindingSummary {
                        id: "F1".to_string(),
                        severity: "major".to_string(),
                        status: "open".to_string(),
                        location: Some("src/lib.rs:1".to_string()),
                        issue: "fixture finding".to_string(),
                    },
                    FindingSummary {
                        id: "F2".to_string(),
                        severity: "minor".to_string(),
                        status: "closed".to_string(),
                        location: None,
                        issue: "fixture follow-up".to_string(),
                    },
                ],
                request,
            )
        }

        async fn prepare_resume(
            &self,
            run_id: &str,
        ) -> Result<RecoveryAssessment, TagteamClientError> {
            let snapshot = self.status(run_id).await?;
            let assessment = RecoveryAssessment {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                run_id: run_id.to_string(),
                resumable: true,
                reason: "fixture is resumable".to_string(),
                approval_request: Some(recovery_request(&snapshot)?),
            };
            assessment.validate().map_err(response_error)?;
            Ok(assessment)
        }

        async fn resume(
            &self,
            run_id: &str,
            approval: Approval,
        ) -> Result<RunHandle, TagteamClientError> {
            let snapshot = self.status(run_id).await?;
            self.consume_approval(&approval, &recovery_request(&snapshot)?)?;
            let handle = RunHandle {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                run_id: run_id.to_string(),
                producer_version: "fixture-v1".to_string(),
            };
            handle.validate().map_err(response_error)?;
            Ok(handle)
        }

        async fn cancel(&self, run_id: &str, approval: Approval) -> Result<(), TagteamClientError> {
            self.status(run_id).await?;
            let expected = ApprovalRequest::for_cancel(run_id).map_err(request_error)?;
            self.consume_approval(&approval, &expected)?;
            let mut runs = self.runs.lock().unwrap();
            let snapshot = runs
                .get_mut(run_id)
                .ok_or_else(|| TagteamClientError::RunNotFound(run_id.to_string()))?;
            snapshot.last_sequence += 1;
            snapshot.last_observation_digest = Some("0".repeat(64));
            snapshot.class = RunClass::Cancelled;
            snapshot.producer_status = "cancelled".to_string();
            snapshot.completeness = Completeness::Complete;
            snapshot.updated_at = Utc::now();
            Ok(())
        }

        async fn diagnostics(&self) -> Result<DiagnosticSummary, TagteamClientError> {
            let diagnostics = DiagnosticSummary {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                status: "ready".to_string(),
                details: vec!["fixture".to_string()],
                completeness: Completeness::Complete,
            };
            diagnostics.validate().map_err(response_error)?;
            Ok(diagnostics)
        }
    }

    fn recovery_request(
        snapshot: &TagteamRunSnapshot,
    ) -> Result<ApprovalRequest, TagteamClientError> {
        let serialized = serde_json::to_vec(snapshot)
            .map_err(|error| TagteamClientError::Terminal(error.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(b"gosling.tagteam.fixture-recovery.v1\0");
        hasher.update(serialized);
        let digest = hasher.finalize();
        let mut recovery_digest = String::with_capacity(digest.len() * 2);
        for byte in digest {
            recovery_digest.push_str(&format!("{byte:02x}"));
        }
        ApprovalRequest::for_resume(snapshot.run_id.clone(), recovery_digest).map_err(request_error)
    }

    fn paginate<T: Clone + PageItemContract>(
        items: Vec<T>,
        request: &PageRequest,
    ) -> Result<Page<T>, TagteamClientError> {
        request.validate().map_err(request_error)?;
        let start = request
            .cursor()
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|_| TagteamClientError::InvalidRequest("invalid cursor".to_string()))?;
        if start > items.len() {
            return Err(TagteamClientError::InvalidRequest(
                "cursor is outside the result set".to_string(),
            ));
        }
        let end = start.saturating_add(request.limit()).min(items.len());
        let page = Page {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            items: items[start..end].to_vec(),
            next_cursor: (end < items.len()).then(|| end.to_string()),
            completeness: Completeness::Complete,
        };
        page.validate(request).map_err(response_error)?;
        Ok(page)
    }

    fn spec(root: &Path) -> TagteamLaunchSpecV1 {
        assert!(std::process::Command::new("git")
            .args(["init", "-q"])
            .arg(root)
            .status()
            .unwrap()
            .success());
        TagteamLaunchSpecV1 {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            repository: RepositoryIdentity::from_path(root).unwrap(),
            prompt: "fixture request".to_string(),
            team: TeamSpec::Solo {
                worker: RoleTarget::new("ollama:fixture").unwrap(),
            },
            allowed_paths: vec![AllowedPath::new("src/").unwrap()],
            rounds: 1,
            time_budget: TimeBudget {
                invocation_timeout_seconds: 60,
                watchdog_timeout_seconds: 30,
                wall_timeout_seconds: 120,
            },
            test_preset: Some(TestPresetRef::new("fixture-test").unwrap()),
            recovery_policy: RecoveryPolicy::Assist,
        }
    }

    async fn start_run(
        client: &FixtureClient,
        launch: TagteamLaunchSpecV1,
        idempotency_key: &str,
    ) -> RunHandle {
        let request = ApprovalRequest::for_start(&launch, idempotency_key).unwrap();
        let approval = client
            .issue_approval(&request, Duration::minutes(1))
            .unwrap();
        client
            .start(launch, idempotency_key, approval)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn fixture_implementation_satisfies_consumer_contract() {
        let repo = TempDir::new().unwrap();
        let client = FixtureClient::new();
        super::conformance::assert_consumer_contract(&client, spec(repo.path()), |request| {
            client
                .issue_approval(request, Duration::minutes(1))
                .unwrap()
        })
        .await;
    }

    #[tokio::test]
    async fn start_requires_an_exact_single_use_approval() {
        let repo = TempDir::new().unwrap();
        let client = FixtureClient::new();
        let launch = spec(repo.path());
        let key = "session-1-generation-1";
        let forged = Approval::from_token("forged-token").unwrap();
        assert_eq!(
            client.start(launch.clone(), key, forged).await,
            Err(TagteamClientError::ApprovalRequired)
        );

        let request = ApprovalRequest::for_start(&launch, key).unwrap();
        let approval = client
            .issue_approval(&request, Duration::minutes(1))
            .unwrap();
        assert_eq!(
            client
                .start(launch.clone(), "different-generation", approval.clone())
                .await,
            Err(TagteamClientError::ApprovalRequired)
        );
        client
            .start(launch.clone(), key, approval.clone())
            .await
            .unwrap();
        assert_eq!(
            client.start(launch, key, approval).await,
            Err(TagteamClientError::ApprovalRequired)
        );
    }

    #[tokio::test]
    async fn expired_and_cross_action_approvals_are_rejected() {
        let repo = TempDir::new().unwrap();
        let client = FixtureClient::new();
        let handle = start_run(&client, spec(repo.path()), "session-1-generation-1").await;
        let recovery = client.prepare_resume(&handle.run_id).await.unwrap();
        let expired = client
            .issue_approval_until(
                recovery.approval_request.as_ref().unwrap(),
                Utc::now() - Duration::seconds(1),
            )
            .unwrap();
        assert_eq!(
            client.resume(&handle.run_id, expired).await,
            Err(TagteamClientError::ApprovalRequired)
        );

        let cancel_request = ApprovalRequest::from_action(
            ApprovalAction::for_cancel(handle.run_id.clone()).unwrap(),
        )
        .unwrap();
        let cancel_approval = client
            .issue_approval(&cancel_request, Duration::minutes(1))
            .unwrap();
        assert_eq!(
            client.resume(&handle.run_id, cancel_approval.clone()).await,
            Err(TagteamClientError::ApprovalRequired)
        );
        client
            .cancel(&handle.run_id, cancel_approval)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn invalid_launch_and_incompatible_schema_fail_deterministically() {
        let repo = TempDir::new().unwrap();
        let client = FixtureClient::new();
        let mut invalid = spec(repo.path());
        invalid.prompt.clear();
        assert!(matches!(
            client
                .start(
                    invalid,
                    "generation-1",
                    Approval::from_token("unused-token").unwrap()
                )
                .await,
            Err(TagteamClientError::InvalidRequest(_))
        ));

        let incompatible = FixtureClient::with_schema_version(TAGTEAM_CONTRACT_VERSION + 1);
        assert_eq!(
            incompatible.capabilities().await,
            Err(TagteamClientError::IncompatibleSchema {
                actual: TAGTEAM_CONTRACT_VERSION + 1,
                supported: TAGTEAM_CONTRACT_VERSION,
            })
        );
    }

    #[tokio::test]
    async fn progress_pagination_response_bounds_and_cancellation_are_observable() {
        let repo = TempDir::new().unwrap();
        let client = FixtureClient::new();
        let handle = start_run(&client, spec(repo.path()), "session-1-generation-1").await;

        client.set_state(&handle.run_id, 1, RunClass::Running);
        assert_eq!(
            client.status(&handle.run_id).await.unwrap().class,
            RunClass::Running
        );
        client.set_state(&handle.run_id, 2, RunClass::Degraded);
        assert_eq!(
            client.status(&handle.run_id).await.unwrap().class,
            RunClass::Degraded
        );

        let first_request = PageRequest::first(1).unwrap();
        let first_page = client.plan(&handle.run_id, &first_request).await.unwrap();
        assert_eq!(first_page.items.len(), 1);
        let second_request = PageRequest::new(first_page.next_cursor.clone(), 1).unwrap();
        let second_page = client.plan(&handle.run_id, &second_request).await.unwrap();
        assert_eq!(second_page.items[0].id, "P2");
        let bad_cursor = PageRequest::new(Some("bad".to_string()), 1).unwrap();
        assert!(client.plan(&handle.run_id, &bad_cursor).await.is_err());
        let zero_limit: PageRequest =
            serde_json::from_value(serde_json::json!({"cursor": null, "limit": 0})).unwrap();
        assert!(matches!(
            client.plan(&handle.run_id, &zero_limit).await,
            Err(TagteamClientError::InvalidRequest(_))
        ));

        client
            .runs
            .lock()
            .unwrap()
            .get_mut(&handle.run_id)
            .unwrap()
            .summary = Some("x".repeat(MAX_SNAPSHOT_TEXT_BYTES + 1));
        assert!(matches!(
            client.status(&handle.run_id).await,
            Err(TagteamClientError::Terminal(_))
        ));

        let cancellable = start_run(&client, spec(repo.path()), "session-1-generation-2").await;
        let cancel_request = ApprovalRequest::for_cancel(&cancellable.run_id).unwrap();
        let cancel_approval = client
            .issue_approval(&cancel_request, Duration::minutes(1))
            .unwrap();
        client
            .cancel(&cancellable.run_id, cancel_approval)
            .await
            .unwrap();
        assert_eq!(
            client.status(&cancellable.run_id).await.unwrap().class,
            RunClass::Cancelled
        );
    }
}
