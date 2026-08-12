use crate::acp::custom_requests::{
    DomainActionRequest, DomainActionResponse, DomainAdapterDescriptor, DomainSnapshotRequest,
    DomainSnapshotResponse, ShellAuthorityMode, ShellHandoffEnvelope, ShellHandoffPrepareRequest,
    ShellIdentity, ShellProtocolPolicy, ShellProvisioning, ShellProvisioningReadResponse,
    SHELL_PROVISIONING_SCHEMA_VERSION,
};
use agent_client_protocol::Error;
use anyhow::Result;
use futures::future::BoxFuture;
use std::collections::HashSet;
use std::sync::Arc;
use uuid::Uuid;

pub trait DomainAdapter: Send + Sync {
    fn descriptor(&self) -> DomainAdapterDescriptor;

    fn snapshot(
        &self,
        request: DomainSnapshotRequest,
    ) -> BoxFuture<'static, Result<DomainSnapshotResponse>>;

    fn perform_action(
        &self,
        request: DomainActionRequest,
    ) -> BoxFuture<'static, Result<DomainActionResponse>>;
}

#[derive(Clone)]
pub struct ShellRuntime {
    provisioning: ShellProvisioning,
    denied_methods: Arc<HashSet<String>>,
    domain_adapter: Option<Arc<dyn DomainAdapter>>,
}

impl ShellRuntime {
    pub fn main_gosling() -> Self {
        Self::new(
            ShellProvisioning {
                identity: ShellIdentity {
                    id: "gosling".into(),
                    display_name: "Gosling".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                },
                ..ShellProvisioning::default()
            },
            None,
        )
    }

    pub fn new(
        mut provisioning: ShellProvisioning,
        domain_adapter: Option<Arc<dyn DomainAdapter>>,
    ) -> Self {
        if provisioning.schema_version == 0 {
            provisioning.schema_version = SHELL_PROVISIONING_SCHEMA_VERSION;
        }
        if let Some(adapter) = &domain_adapter {
            provisioning.domain_adapter = Some(adapter.descriptor());
        }
        let denied_methods = provisioning
            .protocol_policy
            .denied_methods
            .iter()
            .cloned()
            .collect();
        Self {
            provisioning,
            denied_methods: Arc::new(denied_methods),
            domain_adapter,
        }
    }

    pub fn provisioning(&self) -> &ShellProvisioning {
        &self.provisioning
    }

    pub fn identity(&self) -> &ShellIdentity {
        &self.provisioning.identity
    }

    pub fn protocol_policy(&self) -> &ShellProtocolPolicy {
        &self.provisioning.protocol_policy
    }

    pub fn enforce_custom_method(&self, method: &str) -> Result<(), Error> {
        if self.protocol_policy().mode == ShellAuthorityMode::Restricted
            && self.denied_methods.contains(method)
        {
            return Err(Error::new(-32003, "Method unavailable to this shell").data(
                serde_json::json!({
                    "shellId": self.identity().id,
                    "method": method,
                    "policy": "restricted"
                }),
            ));
        }
        Ok(())
    }

    pub fn read_provisioning(&self) -> ShellProvisioningReadResponse {
        ShellProvisioningReadResponse {
            provisioning: self.provisioning.clone(),
        }
    }

    pub async fn domain_snapshot(
        &self,
        request: DomainSnapshotRequest,
    ) -> Result<DomainSnapshotResponse, Error> {
        let adapter = self.domain_adapter.as_ref().ok_or_else(|| {
            Error::method_not_found().data("This shell has no domain adapter configured")
        })?;
        adapter
            .snapshot(request)
            .await
            .map_err(|error| Error::internal_error().data(error.to_string()))
    }

    pub async fn perform_domain_action(
        &self,
        request: DomainActionRequest,
    ) -> Result<DomainActionResponse, Error> {
        let adapter = self.domain_adapter.as_ref().ok_or_else(|| {
            Error::method_not_found().data("This shell has no domain adapter configured")
        })?;
        adapter
            .perform_action(request)
            .await
            .map_err(|error| Error::internal_error().data(error.to_string()))
    }

    pub fn prepare_handoff(&self, request: ShellHandoffPrepareRequest) -> ShellHandoffEnvelope {
        ShellHandoffEnvelope {
            handoff_id: Uuid::now_v7().to_string(),
            origin: self.identity().clone(),
            source_session_id: request.session_id,
            question: request.question,
            requested_capability: request.requested_capability,
            references: request.references,
            return_destination: request.return_destination,
            allow_mutation: request.allow_mutation,
        }
    }
}

impl Default for ShellRuntime {
    fn default() -> Self {
        Self::main_gosling()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(mode: ShellAuthorityMode) -> ShellRuntime {
        ShellRuntime::new(
            ShellProvisioning {
                identity: ShellIdentity {
                    id: "math".into(),
                    display_name: "Math".into(),
                    version: "1".into(),
                },
                protocol_policy: ShellProtocolPolicy {
                    mode,
                    denied_methods: vec!["_gosling/unstable/config/upsert".into()],
                },
                ..ShellProvisioning::default()
            },
            None,
        )
    }

    #[test]
    fn inherited_authority_does_not_apply_denials() {
        assert!(runtime(ShellAuthorityMode::Inherit)
            .enforce_custom_method("_gosling/unstable/config/upsert")
            .is_ok());
    }

    #[test]
    fn restricted_authority_denies_server_side() {
        let error = runtime(ShellAuthorityMode::Restricted)
            .enforce_custom_method("_gosling/unstable/config/upsert")
            .unwrap_err();
        assert_eq!(i32::from(error.code), -32003);
    }
}
