use super::contracts::{
    Approval, DiagnosticSummary, FindingSummary, Page, PlanItem, RecoveryAssessment, RunHandle,
    TagteamCapabilitySet, TagteamLaunchSpecV1,
};
use super::reducer::TagteamRunSnapshot;
use async_trait::async_trait;

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
    ) -> Result<RunHandle, TagteamClientError>;

    async fn status(&self, run_id: &str) -> Result<TagteamRunSnapshot, TagteamClientError>;

    async fn plan(
        &self,
        run_id: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<Page<PlanItem>, TagteamClientError>;

    async fn findings(
        &self,
        run_id: &str,
        cursor: Option<&str>,
        limit: usize,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tagteam::contracts::{
        AllowedPath, Completeness, RecoveryPolicy, RepositoryIdentity, RoleTarget, TeamSpec,
        TestPresetRef, TimeBudget, TAGTEAM_CONTRACT_VERSION,
    };
    use crate::tagteam::reducer::{RunClass, TagteamRunSnapshot};
    use chrono::{Duration, Utc};
    use std::collections::HashMap;
    use std::path::Path;
    use std::sync::Mutex;
    use tempfile::TempDir;

    struct FixtureClient {
        schema_version: u32,
        runs: Mutex<HashMap<String, TagteamRunSnapshot>>,
    }

    impl FixtureClient {
        fn new() -> Self {
            Self {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                runs: Mutex::new(HashMap::new()),
            }
        }

        fn with_schema_version(schema_version: u32) -> Self {
            Self {
                schema_version,
                runs: Mutex::new(HashMap::new()),
            }
        }

        fn set_state(&self, run_id: &str, sequence: u64, class: RunClass) {
            let mut runs = self.runs.lock().unwrap();
            let snapshot = runs.get_mut(run_id).unwrap();
            snapshot.last_sequence = sequence;
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
            Ok(TagteamCapabilitySet {
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
            })
        }

        async fn validate_launch(
            &self,
            spec: &TagteamLaunchSpecV1,
        ) -> Result<(), TagteamClientError> {
            spec.validate()
                .map_err(|error| TagteamClientError::InvalidRequest(error.to_string()))
        }

        async fn start(
            &self,
            spec: TagteamLaunchSpecV1,
            idempotency_key: &str,
        ) -> Result<RunHandle, TagteamClientError> {
            self.validate_launch(&spec).await?;
            if idempotency_key.trim().is_empty() {
                return Err(TagteamClientError::InvalidRequest(
                    "idempotency key is required".to_string(),
                ));
            }
            let run_id = format!("run-{idempotency_key}");
            let mut runs = self.runs.lock().unwrap();
            runs.entry(run_id.clone())
                .or_insert_with(|| TagteamRunSnapshot::configured(run_id.clone(), Utc::now()));
            Ok(RunHandle {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                run_id,
                producer_version: "fixture-v1".to_string(),
            })
        }

        async fn status(&self, run_id: &str) -> Result<TagteamRunSnapshot, TagteamClientError> {
            self.runs
                .lock()
                .unwrap()
                .get(run_id)
                .cloned()
                .ok_or_else(|| TagteamClientError::RunNotFound(run_id.to_string()))
        }

        async fn plan(
            &self,
            run_id: &str,
            cursor: Option<&str>,
            limit: usize,
        ) -> Result<Page<PlanItem>, TagteamClientError> {
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
                cursor,
                limit,
            )
        }

        async fn findings(
            &self,
            run_id: &str,
            cursor: Option<&str>,
            limit: usize,
        ) -> Result<Page<FindingSummary>, TagteamClientError> {
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
                cursor,
                limit,
            )
        }

        async fn prepare_resume(
            &self,
            run_id: &str,
        ) -> Result<RecoveryAssessment, TagteamClientError> {
            self.status(run_id).await?;
            Ok(RecoveryAssessment {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                run_id: run_id.to_string(),
                resumable: true,
                reason: "fixture is resumable".to_string(),
                action_digest: Some(format!("resume:{run_id}")),
            })
        }

        async fn resume(
            &self,
            run_id: &str,
            approval: Approval,
        ) -> Result<RunHandle, TagteamClientError> {
            self.status(run_id).await?;
            validate_approval(&approval, &format!("resume:{run_id}"))?;
            Ok(RunHandle {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                run_id: run_id.to_string(),
                producer_version: "fixture-v1".to_string(),
            })
        }

        async fn cancel(&self, run_id: &str, approval: Approval) -> Result<(), TagteamClientError> {
            self.status(run_id).await?;
            validate_approval(&approval, &format!("cancel:{run_id}"))?;
            let mut runs = self.runs.lock().unwrap();
            let snapshot = runs
                .get_mut(run_id)
                .ok_or_else(|| TagteamClientError::RunNotFound(run_id.to_string()))?;
            snapshot.last_sequence += 1;
            snapshot.class = RunClass::Cancelled;
            snapshot.producer_status = "cancelled".to_string();
            snapshot.completeness = Completeness::Complete;
            snapshot.updated_at = Utc::now();
            Ok(())
        }

        async fn diagnostics(&self) -> Result<DiagnosticSummary, TagteamClientError> {
            Ok(DiagnosticSummary {
                schema_version: TAGTEAM_CONTRACT_VERSION,
                status: "ready".to_string(),
                details: vec!["fixture".to_string()],
                completeness: Completeness::Complete,
            })
        }
    }

    fn validate_approval(
        approval: &Approval,
        expected_digest: &str,
    ) -> Result<(), TagteamClientError> {
        let now = Utc::now();
        if approval.action_digest != expected_digest
            || approval.nonce.is_empty()
            || approval.approved_at > now
            || approval.expires_at <= now
        {
            return Err(TagteamClientError::ApprovalRequired);
        }
        Ok(())
    }

    fn paginate<T: Clone>(
        items: Vec<T>,
        cursor: Option<&str>,
        limit: usize,
    ) -> Result<Page<T>, TagteamClientError> {
        let start = cursor
            .unwrap_or("0")
            .parse::<usize>()
            .map_err(|_| TagteamClientError::InvalidRequest("invalid cursor".to_string()))?;
        if start > items.len() {
            return Err(TagteamClientError::InvalidRequest(
                "cursor is outside the result set".to_string(),
            ));
        }
        let end = start.saturating_add(limit.min(100)).min(items.len());
        Ok(Page {
            schema_version: TAGTEAM_CONTRACT_VERSION,
            items: items[start..end].to_vec(),
            next_cursor: (end < items.len()).then(|| end.to_string()),
            completeness: Completeness::Complete,
        })
    }

    fn spec(root: &Path) -> TagteamLaunchSpecV1 {
        std::fs::create_dir_all(root.join(".git")).unwrap();
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

    #[tokio::test]
    async fn fixture_implementation_satisfies_consumer_contract() {
        let repo = TempDir::new().unwrap();
        let client = FixtureClient::new();
        let capabilities = client.capabilities().await.unwrap();
        assert_eq!(capabilities.schema_version, TAGTEAM_CONTRACT_VERSION);
        assert!(capabilities.capabilities.contains(&"status".to_string()));

        let handle = client
            .start(spec(repo.path()), "session-1-generation-1")
            .await
            .unwrap();
        let snapshot = client.status(&handle.run_id).await.unwrap();
        assert_eq!(snapshot.class, RunClass::Configured);
        assert_eq!(
            client
                .plan(&handle.run_id, None, 10)
                .await
                .unwrap()
                .items
                .len(),
            2
        );
        assert_eq!(
            client
                .findings(&handle.run_id, None, 10)
                .await
                .unwrap()
                .items
                .len(),
            2
        );

        let recovery = client.prepare_resume(&handle.run_id).await.unwrap();
        let approval = Approval {
            action_digest: recovery.action_digest.unwrap(),
            approved_at: Utc::now() - Duration::seconds(1),
            expires_at: Utc::now() + Duration::minutes(1),
            nonce: "nonce-1".to_string(),
        };
        client.resume(&handle.run_id, approval).await.unwrap();
    }

    #[tokio::test]
    async fn expired_or_mutated_approval_is_rejected() {
        let repo = TempDir::new().unwrap();
        let client = FixtureClient::new();
        let handle = client
            .start(spec(repo.path()), "session-1-generation-1")
            .await
            .unwrap();
        let approval = Approval {
            action_digest: "wrong".to_string(),
            approved_at: Utc::now() - Duration::minutes(2),
            expires_at: Utc::now() - Duration::minutes(1),
            nonce: "nonce-1".to_string(),
        };
        assert_eq!(
            client.resume(&handle.run_id, approval).await,
            Err(TagteamClientError::ApprovalRequired)
        );
    }

    #[tokio::test]
    async fn invalid_launch_and_incompatible_schema_fail_deterministically() {
        let repo = TempDir::new().unwrap();
        let client = FixtureClient::new();
        let mut invalid = spec(repo.path());
        invalid.prompt.clear();
        assert!(matches!(
            client.start(invalid, "generation-1").await,
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
    async fn progress_terminal_pagination_and_cancellation_are_observable() {
        let repo = TempDir::new().unwrap();
        let client = FixtureClient::new();
        let handle = client
            .start(spec(repo.path()), "session-1-generation-1")
            .await
            .unwrap();

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

        let first_page = client.plan(&handle.run_id, None, 1).await.unwrap();
        assert_eq!(first_page.items.len(), 1);
        let second_page = client
            .plan(&handle.run_id, first_page.next_cursor.as_deref(), 1)
            .await
            .unwrap();
        assert_eq!(second_page.items[0].id, "P2");
        assert!(client.plan(&handle.run_id, Some("bad"), 1).await.is_err());

        let cancellable = client
            .start(spec(repo.path()), "session-1-generation-2")
            .await
            .unwrap();
        let approval = Approval {
            action_digest: format!("cancel:{}", cancellable.run_id),
            approved_at: Utc::now() - Duration::seconds(1),
            expires_at: Utc::now() + Duration::minutes(1),
            nonce: "nonce-2".to_string(),
        };
        client.cancel(&cancellable.run_id, approval).await.unwrap();
        assert_eq!(
            client.status(&cancellable.run_id).await.unwrap().class,
            RunClass::Cancelled
        );
    }
}
