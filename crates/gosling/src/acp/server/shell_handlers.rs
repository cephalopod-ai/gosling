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
        self.shell_runtime.perform_domain_action(request).await
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
