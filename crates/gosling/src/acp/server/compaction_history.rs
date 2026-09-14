use super::*;
use crate::session::{CompactionHistoryError, CompactionHistoryPolicyV1};

const POLICY_CONFIG_KEY: &str = "GOSLING_COMPACTION_HISTORY_POLICY";

impl GoslingAcpAgent {
    pub(super) async fn on_read_compaction_history_policy(
        &self,
        _req: ReadCompactionHistoryPolicyRequest,
    ) -> Result<ReadCompactionHistoryPolicyResponse, agent_client_protocol::Error> {
        let policy = CompactionHistoryPolicyV1::configured_result()
            .map_err(compaction_history_validation_error)?;
        let stats = self
            .session_manager
            .compaction_history_stats(None)
            .await
            .map_err(compaction_history_error)?;
        Ok(ReadCompactionHistoryPolicyResponse {
            policy: policy.into(),
            stats,
            managed_by_environment: std::env::var_os(POLICY_CONFIG_KEY).is_some(),
        })
    }

    pub(super) async fn on_preview_compaction_history_policy(
        &self,
        req: PreviewCompactionHistoryPolicyRequest,
    ) -> Result<PreviewCompactionHistoryPolicyResponse, agent_client_protocol::Error> {
        let policy = CompactionHistoryPolicyV1::try_from(req.policy)
            .map_err(compaction_history_validation_error)?;
        self.session_manager
            .preview_compaction_history_policy(policy)
            .await
            .map_err(compaction_history_error)
    }

    pub(super) async fn on_apply_compaction_history_policy(
        &self,
        req: ApplyCompactionHistoryPolicyRequest,
    ) -> Result<ApplyCompactionHistoryPolicyResponse, agent_client_protocol::Error> {
        if std::env::var_os(POLICY_CONFIG_KEY).is_some() {
            return Err(agent_client_protocol::Error::invalid_params().data(
                "Context History is managed by GOSLING_COMPACTION_HISTORY_POLICY in the environment",
            ));
        }
        let policy = CompactionHistoryPolicyV1::try_from(req.policy)
            .map_err(compaction_history_validation_error)?;
        let config = self.config()?;
        self.session_manager
            .apply_compaction_history_policy_if_unchanged(
                policy.clone(),
                &req.expected_preview_hash,
                || {
                    config
                        .set_param(POLICY_CONFIG_KEY, &policy)
                        .map_err(|error| {
                            anyhow::anyhow!("Failed to save Context History policy: {error}")
                        })
                },
            )
            .await
            .map_err(compaction_history_error)
    }
}

pub(super) fn compaction_history_error(error: anyhow::Error) -> agent_client_protocol::Error {
    let (mut response, code) = match error.downcast_ref::<CompactionHistoryError>() {
        Some(CompactionHistoryError::NotFound(_)) => (
            agent_client_protocol::Error::resource_not_found(None),
            "compaction_revision_not_found",
        ),
        Some(CompactionHistoryError::Conflict(_)) => (
            agent_client_protocol::Error::invalid_params(),
            "compaction_history_conflict",
        ),
        Some(CompactionHistoryError::Validation(_)) => (
            agent_client_protocol::Error::invalid_params(),
            "compaction_history_validation",
        ),
        None if matches!(
            error.downcast_ref::<sqlx::Error>(),
            Some(sqlx::Error::RowNotFound)
        ) =>
        {
            (
                agent_client_protocol::Error::resource_not_found(None),
                "session_not_found",
            )
        }
        None => (
            agent_client_protocol::Error::internal_error(),
            "compaction_history_storage",
        ),
    };
    response.message = error.to_string();
    response.data(serde_json::json!({"code": code, "message": error.to_string()}))
}

fn compaction_history_validation_error(error: anyhow::Error) -> agent_client_protocol::Error {
    compaction_history_error(CompactionHistoryError::Validation(error.to_string()).into())
}
