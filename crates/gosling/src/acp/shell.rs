use crate::acp::custom_requests::{
    DomainActionConfirmRequest, DomainActionConfirmResponse, DomainActionConfirmationStatus,
    DomainActionRequest, DomainActionResponse, DomainAdapterDescriptor, DomainSnapshotRequest,
    DomainSnapshotResponse, ShellAuthorityMode, ShellHandoffEnvelope, ShellHandoffPrepareRequest,
    ShellIdentity, ShellProtocolPolicy, ShellProvisioning, SHELL_HANDOFF_SCHEMA_VERSION,
    SHELL_PROVISIONING_SCHEMA_VERSION,
};
use agent_client_protocol::Error;
use anyhow::Result;
use futures::future::BoxFuture;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::watch;
use uuid::Uuid;

const MAX_DOMAIN_REQUEST_BYTES: usize = 16 * 1024;
const MAX_DOMAIN_RESPONSE_BYTES: usize = 64 * 1024;
const MAX_DOMAIN_RESOURCES: usize = 64;
const MAX_DOMAIN_ACTION_ID_BYTES: usize = 512;
const MAX_PENDING_DOMAIN_ACTIONS: usize = 64;

fn domain_error(code: &str) -> Error {
    Error::invalid_params().data(serde_json::json!({ "code": code }))
}

#[derive(Clone)]
struct PendingDomainAction {
    request: DomainActionRequest,
}

fn bounded_domain_value(value: &impl Serialize, maximum: usize, code: &str) -> Result<(), Error> {
    let bytes = serde_json::to_vec(value).map_err(|_| domain_error(code))?;
    if bytes.len() > maximum {
        return Err(domain_error(code));
    }
    Ok(())
}

pub trait DomainAdapter: Send + Sync {
    fn descriptor(&self) -> DomainAdapterDescriptor;

    fn status(&self) -> crate::acp::custom_requests::DomainAdapterStatus {
        crate::acp::custom_requests::DomainAdapterStatus::Ready
    }

    fn subscribe_status(
        &self,
    ) -> Option<watch::Receiver<crate::acp::custom_requests::DomainAdapterStatus>> {
        None
    }

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
    pending_domain_actions: Arc<std::sync::Mutex<HashMap<String, PendingDomainAction>>>,
}

impl ShellRuntime {
    pub fn main_gosling() -> Self {
        Self::new(
            ShellProvisioning {
                identity: ShellIdentity {
                    id: "gosling".into(),
                    display_name: "Gosling".into(),
                    version: env!("CARGO_PKG_VERSION").into(),
                    runtime_namespace: "gosling".into(),
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
            pending_domain_actions: Arc::new(std::sync::Mutex::new(HashMap::new())),
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

    pub fn domain_adapter_status(
        &self,
    ) -> Option<crate::acp::custom_requests::DomainAdapterStatus> {
        self.domain_adapter.as_ref().map(|adapter| adapter.status())
    }

    pub fn subscribe_domain_adapter_status(
        &self,
    ) -> Option<watch::Receiver<crate::acp::custom_requests::DomainAdapterStatus>> {
        self.domain_adapter
            .as_ref()
            .and_then(|adapter| adapter.subscribe_status())
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

    pub async fn domain_snapshot(
        &self,
        request: DomainSnapshotRequest,
    ) -> Result<DomainSnapshotResponse, Error> {
        let adapter = self.domain_adapter.as_ref().ok_or_else(|| {
            Error::method_not_found().data("This shell has no domain adapter configured")
        })?;
        bounded_domain_value(
            &request.input,
            MAX_DOMAIN_REQUEST_BYTES,
            "ADAPTER_REQUEST_TOO_LARGE",
        )?;
        let response = adapter
            .snapshot(request)
            .await
            .map_err(|_| domain_error("ADAPTER_OPERATION_FAILED"))?;
        if response.domain_id != adapter.descriptor().domain_id {
            return Err(domain_error("ADAPTER_RESPONSE_MISMATCH"));
        }
        if response.resources.len() > MAX_DOMAIN_RESOURCES {
            return Err(domain_error("ADAPTER_RESPONSE_RESOURCES_TOO_MANY"));
        }
        bounded_domain_value(
            &response,
            MAX_DOMAIN_RESPONSE_BYTES,
            "ADAPTER_RESPONSE_TOO_LARGE",
        )?;
        Ok(response)
    }

    pub async fn perform_domain_action(
        &self,
        request: DomainActionRequest,
    ) -> Result<DomainActionResponse, Error> {
        let adapter = self.domain_adapter.as_ref().ok_or_else(|| {
            Error::method_not_found().data("This shell has no domain adapter configured")
        })?;
        let descriptor = adapter.descriptor();
        if request.session_id.is_empty() || request.session_id.len() > MAX_DOMAIN_ACTION_ID_BYTES {
            return Err(domain_error("DOMAIN_SESSION_INVALID"));
        }
        if request.generation == 0 {
            return Err(domain_error("DOMAIN_GENERATION_INVALID"));
        }
        let action = descriptor
            .actions
            .iter()
            .find(|candidate| candidate.name == request.action)
            .ok_or_else(|| domain_error("ADAPTER_ACTION_UNAVAILABLE"))?;
        if action.kind == crate::acp::custom_requests::DomainAdapterActionKind::Mutate {
            let action_id = Uuid::now_v7().to_string();
            let mut pending_actions = self
                .pending_domain_actions
                .lock()
                .map_err(|_| domain_error("DOMAIN_CONFIRMATION_UNAVAILABLE"))?;
            if pending_actions.len() >= MAX_PENDING_DOMAIN_ACTIONS {
                return Err(domain_error("DOMAIN_CONFIRMATION_LIMIT"));
            }
            pending_actions.insert(action_id.clone(), PendingDomainAction { request });
            return Ok(DomainActionResponse {
                domain_id: descriptor.domain_id,
                action: action.name.clone(),
                payload: serde_json::Value::Null,
                resources: Vec::new(),
                confirmation_action_id: Some(action_id),
            });
        }
        self.execute_domain_action(adapter, descriptor, request)
            .await
    }

    pub async fn confirm_domain_action(
        &self,
        request: DomainActionConfirmRequest,
    ) -> Result<DomainActionConfirmResponse, Error> {
        if request.action_id.is_empty() || request.action_id.len() > MAX_DOMAIN_ACTION_ID_BYTES {
            return Err(domain_error("DOMAIN_CONFIRMATION_STALE"));
        }
        let pending = {
            let mut pending_actions = self
                .pending_domain_actions
                .lock()
                .map_err(|_| domain_error("DOMAIN_CONFIRMATION_UNAVAILABLE"))?;
            let Some(pending) = pending_actions.get(&request.action_id) else {
                return Err(domain_error("DOMAIN_CONFIRMATION_STALE"));
            };
            if pending.request.session_id != request.session_id
                || pending.request.generation != request.generation
            {
                return Err(domain_error("DOMAIN_CONFIRMATION_STALE"));
            }
            pending_actions
                .remove(&request.action_id)
                .expect("pending action exists after validation")
        };
        if !request.approve {
            return Ok(DomainActionConfirmResponse {
                status: DomainActionConfirmationStatus::Denied,
                result: None,
            });
        }
        let adapter = self.domain_adapter.as_ref().ok_or_else(|| {
            Error::method_not_found().data("This shell has no domain adapter configured")
        })?;
        let response = self
            .execute_domain_action(adapter, adapter.descriptor(), pending.request)
            .await?;
        Ok(DomainActionConfirmResponse {
            status: DomainActionConfirmationStatus::Approved,
            result: Some(response),
        })
    }

    async fn execute_domain_action(
        &self,
        adapter: &Arc<dyn DomainAdapter>,
        descriptor: DomainAdapterDescriptor,
        request: DomainActionRequest,
    ) -> Result<DomainActionResponse, Error> {
        bounded_domain_value(
            &request.input,
            MAX_DOMAIN_REQUEST_BYTES,
            "ADAPTER_REQUEST_TOO_LARGE",
        )?;
        let expected_action = request.action.clone();
        let response = adapter
            .perform_action(request)
            .await
            .map_err(|_| domain_error("ADAPTER_OPERATION_FAILED"))?;
        if response.domain_id != descriptor.domain_id || response.action != expected_action {
            return Err(domain_error("ADAPTER_RESPONSE_MISMATCH"));
        }
        if response.resources.len() > MAX_DOMAIN_RESOURCES {
            return Err(domain_error("ADAPTER_RESPONSE_RESOURCES_TOO_MANY"));
        }
        bounded_domain_value(
            &response,
            MAX_DOMAIN_RESPONSE_BYTES,
            "ADAPTER_RESPONSE_TOO_LARGE",
        )?;
        Ok(response)
    }

    pub fn prepare_handoff(&self, request: ShellHandoffPrepareRequest) -> ShellHandoffEnvelope {
        ShellHandoffEnvelope {
            schema_version: SHELL_HANDOFF_SCHEMA_VERSION,
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
    use crate::acp::custom_requests::{DomainAdapterAction, DomainAdapterActionKind};
    use std::sync::Mutex;

    struct TestAdapter {
        descriptor: DomainAdapterDescriptor,
        snapshot_response: Mutex<DomainSnapshotResponse>,
        action_response: Mutex<DomainActionResponse>,
        calls: Arc<Mutex<Vec<String>>>,
    }

    impl DomainAdapter for TestAdapter {
        fn descriptor(&self) -> DomainAdapterDescriptor {
            self.descriptor.clone()
        }

        fn snapshot(
            &self,
            _request: DomainSnapshotRequest,
        ) -> BoxFuture<'static, Result<DomainSnapshotResponse>> {
            self.calls.lock().unwrap().push("snapshot".into());
            let response = self.snapshot_response.lock().unwrap().clone();
            Box::pin(async move { Ok(response) })
        }

        fn perform_action(
            &self,
            request: DomainActionRequest,
        ) -> BoxFuture<'static, Result<DomainActionResponse>> {
            self.calls
                .lock()
                .unwrap()
                .push(format!("action:{}", request.action));
            let response = self.action_response.lock().unwrap().clone();
            Box::pin(async move { Ok(response) })
        }
    }

    fn adapter() -> (Arc<TestAdapter>, ShellRuntime) {
        let descriptor = DomainAdapterDescriptor {
            domain_id: "neutral-fixture".into(),
            display_name: "Neutral Fixture".into(),
            version: "0.1.0".into(),
            protocol_version: "1.0.0".into(),
            actions: vec![
                DomainAdapterAction {
                    name: "inspect".into(),
                    kind: DomainAdapterActionKind::Read,
                    schema_ref: "neutral-fixture/inspect@1".into(),
                },
                DomainAdapterAction {
                    name: "toggle".into(),
                    kind: DomainAdapterActionKind::Mutate,
                    schema_ref: "neutral-fixture/toggle@1".into(),
                },
            ],
        };
        let adapter = Arc::new(TestAdapter {
            snapshot_response: Mutex::new(DomainSnapshotResponse {
                domain_id: descriptor.domain_id.clone(),
                payload: serde_json::json!({ "state": "neutral" }),
                resources: Vec::new(),
            }),
            action_response: Mutex::new(DomainActionResponse {
                domain_id: descriptor.domain_id.clone(),
                action: "inspect".into(),
                payload: serde_json::json!({ "state": "neutral" }),
                resources: Vec::new(),
                confirmation_action_id: None,
            }),
            descriptor,
            calls: Arc::new(Mutex::new(Vec::new())),
        });
        let runtime = ShellRuntime::new(ShellProvisioning::default(), Some(adapter.clone()));
        (adapter, runtime)
    }

    fn runtime(mode: ShellAuthorityMode) -> ShellRuntime {
        ShellRuntime::new(
            ShellProvisioning {
                identity: ShellIdentity {
                    id: "math".into(),
                    display_name: "Math".into(),
                    version: "1".into(),
                    runtime_namespace: "math".into(),
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

    #[test]
    fn handoff_envelope_is_versioned_and_uses_server_identity() {
        let envelope =
            runtime(ShellAuthorityMode::Inherit).prepare_handoff(ShellHandoffPrepareRequest {
                session_id: "session-1".into(),
                question: "Continue this analysis".into(),
                requested_capability: "general_workspace".into(),
                ..ShellHandoffPrepareRequest::default()
            });

        assert_eq!(envelope.schema_version, SHELL_HANDOFF_SCHEMA_VERSION);
        assert_eq!(envelope.origin.id, "math");
        assert_eq!(envelope.source_session_id, "session-1");
    }

    #[tokio::test]
    async fn domain_actions_are_allowlisted_and_mutations_wait_for_confirmation() {
        let (adapter, runtime) = adapter();

        assert!(runtime
            .perform_domain_action(DomainActionRequest {
                action: "unknown".into(),
                session_id: "session-a".into(),
                generation: 4,
                ..DomainActionRequest::default()
            })
            .await
            .is_err());
        let pending = runtime
            .perform_domain_action(DomainActionRequest {
                action: "toggle".into(),
                session_id: "session-a".into(),
                generation: 4,
                ..DomainActionRequest::default()
            })
            .await
            .unwrap();
        let action_id = pending.confirmation_action_id.unwrap();
        adapter.action_response.lock().unwrap().action = "toggle".into();
        assert!(runtime
            .confirm_domain_action(DomainActionConfirmRequest {
                action_id: action_id.clone(),
                session_id: "session-b".into(),
                generation: 4,
                approve: true,
            })
            .await
            .is_err());
        assert!(adapter.calls.lock().unwrap().is_empty());

        let confirmation = runtime
            .confirm_domain_action(DomainActionConfirmRequest {
                action_id: action_id.clone(),
                session_id: "session-a".into(),
                generation: 4,
                approve: true,
            })
            .await
            .unwrap();
        assert_eq!(
            confirmation.status,
            DomainActionConfirmationStatus::Approved
        );
        assert_eq!(confirmation.result.unwrap().action, "toggle");
        assert_eq!(adapter.calls.lock().unwrap().as_slice(), ["action:toggle"]);
        assert!(runtime
            .confirm_domain_action(DomainActionConfirmRequest {
                action_id,
                session_id: "session-a".into(),
                generation: 4,
                approve: true,
            })
            .await
            .is_err());

        adapter.action_response.lock().unwrap().action = "inspect".into();
        let response = runtime
            .perform_domain_action(DomainActionRequest {
                action: "inspect".into(),
                session_id: "session-a".into(),
                generation: 4,
                ..DomainActionRequest::default()
            })
            .await
            .unwrap();
        assert_eq!(response.action, "inspect");
        assert_eq!(
            adapter.calls.lock().unwrap().as_slice(),
            ["action:toggle", "action:inspect"]
        );

        let denied = runtime
            .perform_domain_action(DomainActionRequest {
                action: "toggle".into(),
                session_id: "session-a".into(),
                generation: 4,
                ..DomainActionRequest::default()
            })
            .await
            .unwrap();
        let denied = runtime
            .confirm_domain_action(DomainActionConfirmRequest {
                action_id: denied.confirmation_action_id.unwrap(),
                session_id: "session-a".into(),
                generation: 4,
                approve: false,
            })
            .await
            .unwrap();
        assert_eq!(denied.status, DomainActionConfirmationStatus::Denied);
        assert!(denied.result.is_none());
        assert_eq!(
            adapter.calls.lock().unwrap().as_slice(),
            ["action:toggle", "action:inspect"]
        );
    }

    #[tokio::test]
    async fn domain_responses_must_match_the_negotiated_descriptor_and_bounds() {
        let (adapter, runtime) = adapter();
        adapter.action_response.lock().unwrap().action = "other".into();
        assert!(runtime
            .perform_domain_action(DomainActionRequest {
                action: "inspect".into(),
                session_id: "session-a".into(),
                generation: 4,
                ..DomainActionRequest::default()
            })
            .await
            .is_err());

        adapter.snapshot_response.lock().unwrap().payload =
            serde_json::Value::String("x".repeat(64 * 1024));
        assert!(runtime
            .domain_snapshot(DomainSnapshotRequest::default())
            .await
            .is_err());
        assert_eq!(
            adapter.calls.lock().unwrap().as_slice(),
            ["action:inspect", "snapshot"]
        );
    }

    #[tokio::test]
    async fn domain_responses_limit_resource_references_before_reaching_consumers() {
        let (adapter, runtime) = adapter();
        let resources = (0..=MAX_DOMAIN_RESOURCES)
            .map(
                |index| crate::acp::custom_requests::DomainResourceReference {
                    kind: "neutral".into(),
                    id: index.to_string(),
                    label: "Neutral resource".into(),
                    uri: None,
                },
            )
            .collect();
        adapter.snapshot_response.lock().unwrap().resources = resources;
        assert!(runtime
            .domain_snapshot(DomainSnapshotRequest::default())
            .await
            .is_err());

        adapter.action_response.lock().unwrap().resources = (0..=MAX_DOMAIN_RESOURCES)
            .map(
                |index| crate::acp::custom_requests::DomainResourceReference {
                    kind: "neutral".into(),
                    id: index.to_string(),
                    label: "Neutral resource".into(),
                    uri: None,
                },
            )
            .collect();
        assert!(runtime
            .perform_domain_action(DomainActionRequest {
                action: "inspect".into(),
                session_id: "session-a".into(),
                generation: 1,
                ..DomainActionRequest::default()
            })
            .await
            .is_err());
    }
}
