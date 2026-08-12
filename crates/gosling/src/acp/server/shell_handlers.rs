use super::*;

impl GoslingAcpAgent {
    pub(super) fn on_read_shell_provisioning(&self) -> ShellProvisioningReadResponse {
        self.shell_runtime.read_provisioning()
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
