use super::*;

impl GoslingAcpAgent {
    pub(super) async fn shell_provisioning_validation(
        &self,
        provisioning: &ShellProvisioning,
    ) -> ShellProvisioningValidationReport {
        crate::acp::shell_validation::validate_shell_provisioning(
            provisioning,
            Config::global(),
            &self.workspace_service,
            &self.builtins,
            &self.default_working_folder,
        )
        .await
    }

    pub(super) async fn shell_provisioning_validation_for_working_dir(
        &self,
        provisioning: &ShellProvisioning,
        working_dir: &std::path::Path,
    ) -> ShellProvisioningValidationReport {
        crate::acp::shell_validation::validate_shell_provisioning_for_working_dir(
            provisioning,
            Config::global(),
            &self.workspace_service,
            &self.builtins,
            &self.default_working_folder,
            Some(working_dir),
        )
        .await
    }

    pub(super) async fn on_read_shell_provisioning(&self) -> ShellProvisioningReadResponse {
        let provisioning = self.shell_runtime.provisioning().clone();
        let validation = self.shell_provisioning_validation(&provisioning).await;
        ShellProvisioningReadResponse {
            provisioning,
            validation,
        }
    }

    pub(super) async fn on_validate_shell_provisioning(
        &self,
        request: ShellProvisioningValidateRequest,
    ) -> ShellProvisioningValidateResponse {
        let provisioning = request
            .provisioning
            .unwrap_or_else(|| self.shell_runtime.provisioning().clone());
        let validation = self.shell_provisioning_validation(&provisioning).await;
        ShellProvisioningValidateResponse {
            provisioning,
            validation,
        }
    }

    pub(super) async fn on_domain_snapshot(
        &self,
        request: DomainSnapshotRequest,
    ) -> Result<DomainSnapshotResponse, agent_client_protocol::Error> {
        self.shell_runtime.domain_snapshot(request).await
    }

    pub(super) async fn on_domain_action(
        &self,
        request: DomainActionRequest,
    ) -> Result<DomainActionResponse, agent_client_protocol::Error> {
        self.require_active_domain_session(&request.session_id)
            .await?;
        self.shell_runtime.perform_domain_action(request).await
    }

    pub(super) async fn on_domain_action_confirm(
        &self,
        request: DomainActionConfirmRequest,
    ) -> Result<DomainActionConfirmResponse, agent_client_protocol::Error> {
        self.require_active_domain_session(&request.session_id)
            .await?;
        self.shell_runtime.confirm_domain_action(request).await
    }

    async fn require_active_domain_session(
        &self,
        session_id: &str,
    ) -> Result<(), agent_client_protocol::Error> {
        if session_id.is_empty() || session_id.len() > 512 {
            return Err(agent_client_protocol::Error::invalid_params()
                .data(serde_json::json!({ "code": "DOMAIN_SESSION_INVALID" })));
        }
        if self.sessions.lock().await.contains_key(session_id) {
            return Ok(());
        }
        Err(agent_client_protocol::Error::invalid_params()
            .data(serde_json::json!({ "code": "DOMAIN_SESSION_UNAVAILABLE" })))
    }

    pub(super) fn on_prepare_shell_handoff(
        &self,
        request: ShellHandoffPrepareRequest,
    ) -> ShellHandoffPrepareResponse {
        ShellHandoffPrepareResponse {
            handoff: self.shell_runtime.prepare_handoff(request),
        }
    }
}
