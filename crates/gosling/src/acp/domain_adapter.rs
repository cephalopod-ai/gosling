use crate::acp::shell::DomainAdapter;
use crate::agents::extension_manager::{connect_operator_stdio_client, OperatorStdioClient};
use crate::agents::mcp_client::McpClientTrait;
use crate::agents::ToolCallContext;
use crate::config::AdapterRegistration;
use anyhow::{bail, Result};
use futures::future::BoxFuture;
use gosling_sdk_types::custom_requests::{
    DomainActionRequest, DomainActionResponse, DomainAdapterDescriptor, DomainSnapshotRequest,
    DomainSnapshotResponse,
};
use gosling_sdk_types::shell::DomainAdapterStatus;
use rmcp::model::CallToolResult;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;

const ADAPTER_SESSION_ID: &str = "domain-adapter";

pub struct McpDomainAdapter {
    descriptor: DomainAdapterDescriptor,
    client: Arc<OperatorStdioClient>,
    max_message_bytes: usize,
    request_timeout: Duration,
    working_dir: PathBuf,
    status: tokio::sync::watch::Sender<DomainAdapterStatus>,
}

impl McpDomainAdapter {
    pub async fn connect(
        registration: AdapterRegistration,
        expected_descriptor: DomainAdapterDescriptor,
        working_dir: PathBuf,
    ) -> Result<Self> {
        registration.validate().map_err(anyhow::Error::msg)?;
        let client = connect_operator_stdio_client(&registration, &working_dir).await?;
        let client = Arc::new(client);
        let request_timeout = Duration::from_secs(
            registration
                .timeout
                .unwrap_or(crate::config::DEFAULT_EXTENSION_TIMEOUT),
        );
        let descriptor: DomainAdapterDescriptor = Self::call_tool(
            &client,
            registration.max_message_bytes,
            request_timeout,
            &working_dir,
            "descriptor",
            Map::new(),
        )
        .await?;
        if descriptor != expected_descriptor {
            bail!("ADAPTER_DESCRIPTOR_MISMATCH");
        }

        let (status, _) = tokio::sync::watch::channel(DomainAdapterStatus::Ready);
        let mut process_exit = client.subscribe_exit();
        let process_status = status.clone();
        tokio::spawn(async move {
            while process_exit.changed().await.is_ok() {
                if *process_exit.borrow()
                    == crate::agents::extension_manager::OperatorProcessExit::Exited
                {
                    process_status.send_replace(DomainAdapterStatus::Crashed);
                    return;
                }
            }
        });

        Ok(Self {
            descriptor,
            client,
            max_message_bytes: registration.max_message_bytes,
            request_timeout,
            working_dir,
            status,
        })
    }

    async fn call_tool<T>(
        client: &OperatorStdioClient,
        max_message_bytes: usize,
        request_timeout: Duration,
        working_dir: &Path,
        name: &str,
        arguments: Map<String, Value>,
    ) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let encoded_arguments = serde_json::to_vec(&arguments)?;
        if encoded_arguments.len() > max_message_bytes {
            bail!("ADAPTER_REQUEST_TOO_LARGE");
        }
        let context = ToolCallContext::new(
            ADAPTER_SESSION_ID.to_string(),
            Some(working_dir.to_path_buf()),
            None,
        );
        let result = tokio::time::timeout(
            request_timeout,
            client
                .client
                .call_tool(&context, name, Some(arguments), CancellationToken::new()),
        )
        .await
        .map_err(|_| anyhow::anyhow!("ADAPTER_OPERATION_TIMEOUT"))?
        .map_err(|_| anyhow::anyhow!("ADAPTER_OPERATION_FAILED"))?;
        Self::decode_result(result, max_message_bytes)
    }

    fn decode_result<T>(result: CallToolResult, max_message_bytes: usize) -> Result<T>
    where
        T: DeserializeOwned,
    {
        let encoded_result = serde_json::to_vec(&result)?;
        if encoded_result.len() > max_message_bytes {
            bail!("ADAPTER_RESPONSE_TOO_LARGE");
        }
        if result.is_error == Some(true) {
            bail!("ADAPTER_OPERATION_FAILED");
        }
        let payload = match result.structured_content {
            Some(payload) => payload,
            None if result.content.len() == 1 => {
                let text = result.content[0]
                    .as_text()
                    .ok_or_else(|| anyhow::anyhow!("ADAPTER_RESPONSE_INVALID"))?;
                serde_json::from_str(&text.text)
                    .map_err(|_| anyhow::anyhow!("ADAPTER_RESPONSE_INVALID"))?
            }
            None => bail!("ADAPTER_RESPONSE_INVALID"),
        };
        serde_json::from_value(payload).map_err(|_| anyhow::anyhow!("ADAPTER_RESPONSE_INVALID"))
    }
}

impl DomainAdapter for McpDomainAdapter {
    fn descriptor(&self) -> DomainAdapterDescriptor {
        self.descriptor.clone()
    }

    fn status(&self) -> DomainAdapterStatus {
        *self.status.borrow()
    }

    fn subscribe_status(&self) -> Option<tokio::sync::watch::Receiver<DomainAdapterStatus>> {
        Some(self.status.subscribe())
    }

    fn snapshot(
        &self,
        request: DomainSnapshotRequest,
    ) -> BoxFuture<'static, Result<DomainSnapshotResponse>> {
        let client = self.client.clone();
        let working_dir = self.working_dir.clone();
        let max_message_bytes = self.max_message_bytes;
        let request_timeout = self.request_timeout;
        let status = self.status.clone();
        Box::pin(async move {
            let mut arguments = Map::new();
            arguments.insert("input".to_string(), request.input);
            let result = Self::call_tool(
                &client,
                max_message_bytes,
                request_timeout,
                &working_dir,
                "snapshot",
                arguments,
            )
            .await;
            if result
                .as_ref()
                .err()
                .is_some_and(|error| error.to_string() == "ADAPTER_OPERATION_TIMEOUT")
            {
                status.send_replace(DomainAdapterStatus::Hung);
            }
            result
        })
    }

    fn perform_action(
        &self,
        request: DomainActionRequest,
    ) -> BoxFuture<'static, Result<DomainActionResponse>> {
        let client = self.client.clone();
        let working_dir = self.working_dir.clone();
        let max_message_bytes = self.max_message_bytes;
        let request_timeout = self.request_timeout;
        let status = self.status.clone();
        Box::pin(async move {
            let mut arguments = Map::new();
            arguments.insert("action".to_string(), Value::String(request.action));
            arguments.insert("input".to_string(), request.input);
            let result = Self::call_tool(
                &client,
                max_message_bytes,
                request_timeout,
                &working_dir,
                "action",
                arguments,
            )
            .await;
            if result
                .as_ref()
                .err()
                .is_some_and(|error| error.to_string() == "ADAPTER_OPERATION_TIMEOUT")
            {
                status.send_replace(DomainAdapterStatus::Hung);
            }
            result
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acp::shell::ShellRuntime;
    use crate::config::DEFAULT_ADAPTER_MAX_MESSAGE_BYTES;
    use gosling_sdk_types::custom_requests::{
        DomainActionConfirmRequest, DomainActionConfirmationStatus, DomainAdapterAction,
        DomainAdapterActionKind,
    };
    use rmcp::model::Content;
    use std::fs;
    use tempfile::TempDir;

    fn descriptor() -> DomainAdapterDescriptor {
        DomainAdapterDescriptor {
            domain_id: "neutral-fixture".to_string(),
            display_name: "Neutral Fixture".to_string(),
            version: "0.1.0".to_string(),
            protocol_version: "1.0.0".to_string(),
            actions: vec![
                DomainAdapterAction {
                    name: "inspect".to_string(),
                    kind: DomainAdapterActionKind::Read,
                    schema_ref: "neutral-fixture/inspect@1".to_string(),
                },
                DomainAdapterAction {
                    name: "toggle".to_string(),
                    kind: DomainAdapterActionKind::Mutate,
                    schema_ref: "neutral-fixture/toggle@1".to_string(),
                },
            ],
        }
    }

    fn registration(args: Vec<String>, timeout_secs: u64) -> AdapterRegistration {
        AdapterRegistration {
            domain_id: "neutral-fixture".to_string(),
            cmd: "node".to_string(),
            args,
            envs: Default::default(),
            env_keys: Vec::new(),
            timeout: Some(timeout_secs),
            cwd: None,
            max_message_bytes: DEFAULT_ADAPTER_MAX_MESSAGE_BYTES,
        }
    }

    fn write_fixture(directory: &TempDir) -> AdapterRegistration {
        let script = directory.path().join("adapter.mjs");
        fs::write(
            &script,
            r#"
import readline from 'node:readline';

const descriptor = {
  domainId: 'neutral-fixture',
  displayName: 'Neutral Fixture',
  version: '0.1.0',
  protocolVersion: '1.0.0',
  actions: [
    { name: 'inspect', kind: 'read', schemaRef: 'neutral-fixture/inspect@1' },
    { name: 'toggle', kind: 'mutate', schemaRef: 'neutral-fixture/toggle@1' },
  ],
};

const reply = (id, result) => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`);
readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') {
    reply(request.id, {
      protocolVersion: request.params.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: 'neutral-fixture', version: '0.1.0' },
    });
    return;
  }
  if (request.method !== 'tools/call') return;
  const tool = request.params.name;
  if (tool === 'descriptor') {
    reply(request.id, { content: [], structuredContent: descriptor, isError: false });
    return;
  }
  if (tool === 'snapshot') {
    reply(request.id, {
      content: [],
      structuredContent: { domainId: 'neutral-fixture', payload: request.params.arguments.input, resources: [] },
      isError: false,
    });
    return;
  }
  if (tool === 'action') {
    reply(request.id, {
      content: [],
      structuredContent: {
        domainId: 'neutral-fixture',
        action: request.params.arguments.action,
        payload: request.params.arguments.input,
        resources: [],
      },
      isError: false,
    });
  }
});
"#,
        )
        .unwrap();
        registration(vec![script.display().to_string()], 10)
    }

    fn write_hanging_fixture(directory: &TempDir) -> AdapterRegistration {
        let script = directory.path().join("hanging-adapter.mjs");
        fs::write(
            &script,
            r#"
import readline from 'node:readline';

const descriptor = {
  domainId: 'neutral-fixture',
  displayName: 'Neutral Fixture',
  version: '0.1.0',
  protocolVersion: '1.0.0',
  actions: [
    { name: 'inspect', kind: 'read', schemaRef: 'neutral-fixture/inspect@1' },
    { name: 'toggle', kind: 'mutate', schemaRef: 'neutral-fixture/toggle@1' },
  ],
};

const reply = (id, result) => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`);
readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') {
    reply(request.id, {
      protocolVersion: request.params.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: 'neutral-fixture', version: '0.1.0' },
    });
    return;
  }
  if (request.method === 'tools/call' && request.params.name === 'descriptor') {
    reply(request.id, { content: [], structuredContent: descriptor, isError: false });
  }
});
"#,
        )
        .unwrap();
        registration(vec![script.display().to_string()], 1)
    }

    fn write_malformed_fixture(directory: &TempDir) -> AdapterRegistration {
        let script = directory.path().join("malformed-adapter.mjs");
        fs::write(
            &script,
            r#"
import readline from 'node:readline';

const descriptor = {
  domainId: 'neutral-fixture',
  displayName: 'Neutral Fixture',
  version: '0.1.0',
  protocolVersion: '1.0.0',
  actions: [
    { name: 'inspect', kind: 'read', schemaRef: 'neutral-fixture/inspect@1' },
    { name: 'toggle', kind: 'mutate', schemaRef: 'neutral-fixture/toggle@1' },
  ],
};

const reply = (id, result) => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`);
readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') {
    reply(request.id, {
      protocolVersion: request.params.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: 'neutral-fixture', version: '0.1.0' },
    });
    return;
  }
  if (request.method === 'tools/call' && request.params.name === 'descriptor') {
    reply(request.id, { content: [], structuredContent: descriptor, isError: false });
    return;
  }
  if (request.method === 'tools/call' && request.params.name === 'snapshot') {
    reply(request.id, { content: [], structuredContent: { malformed: true }, isError: false });
  }
});
"#,
        )
        .unwrap();
        registration(vec![script.display().to_string()], 10)
    }

    fn write_crashing_fixture(directory: &TempDir) -> AdapterRegistration {
        let script = directory.path().join("crashing-adapter.mjs");
        fs::write(
            &script,
            r#"
import readline from 'node:readline';

const descriptor = {
  domainId: 'neutral-fixture',
  displayName: 'Neutral Fixture',
  version: '0.1.0',
  protocolVersion: '1.0.0',
  actions: [
    { name: 'inspect', kind: 'read', schemaRef: 'neutral-fixture/inspect@1' },
    { name: 'toggle', kind: 'mutate', schemaRef: 'neutral-fixture/toggle@1' },
  ],
};

const reply = (id, result) => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`);
readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') {
    reply(request.id, {
      protocolVersion: request.params.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: 'neutral-fixture', version: '0.1.0' },
    });
    return;
  }
  if (request.method === 'tools/call' && request.params.name === 'descriptor') {
    reply(request.id, { content: [], structuredContent: descriptor, isError: false });
    return;
  }
  if (request.method === 'tools/call') process.exit(23);
});
"#,
        )
        .unwrap();
        registration(vec![script.display().to_string()], 10)
    }

    fn write_startup_crashing_fixture(directory: &TempDir) -> AdapterRegistration {
        let script = directory.path().join("startup-crashing-adapter.mjs");
        fs::write(&script, "process.exit(9);\n").unwrap();
        registration(vec![script.display().to_string()], 10)
    }

    fn write_idle_crashing_fixture(directory: &TempDir) -> AdapterRegistration {
        let script = directory.path().join("idle-crashing-adapter.mjs");
        fs::write(
            &script,
            r#"
import readline from 'node:readline';

const descriptor = {
  domainId: 'neutral-fixture',
  displayName: 'Neutral Fixture',
  version: '0.1.0',
  protocolVersion: '1.0.0',
  actions: [
    { name: 'inspect', kind: 'read', schemaRef: 'neutral-fixture/inspect@1' },
    { name: 'toggle', kind: 'mutate', schemaRef: 'neutral-fixture/toggle@1' },
  ],
};

const reply = (id, result) => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`);
readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') {
    reply(request.id, {
      protocolVersion: request.params.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: 'neutral-fixture', version: '0.1.0' },
    });
    return;
  }
  if (request.method === 'tools/call' && request.params.name === 'descriptor') {
    reply(request.id, { content: [], structuredContent: descriptor, isError: false });
    // Exits on its own once negotiated, independent of any subsequent call — an idle crash
    // rather than one triggered by an in-flight snapshot/action request. The delay is generous
    // (well beyond local IPC latency) so the test's subscribe_status() call, which happens
    // immediately after connect() returns, cannot lose the race against this timer even under
    // CI scheduler contention — tokio::watch receivers only observe changes sent after they
    // subscribe, so a too-tight margin here could make the test hang until its own timeout.
    setTimeout(() => process.exit(23), 300);
  }
});
"#,
        )
        .unwrap();
        registration(vec![script.display().to_string()], 10)
    }

    fn write_idle_fixture(directory: &TempDir, pid_file: &Path) -> AdapterRegistration {
        let script = directory.path().join("idle-adapter.mjs");
        fs::write(
            &script,
            r#"
import fs from 'node:fs';
import readline from 'node:readline';

fs.writeFileSync(process.argv[2], String(process.pid));

const descriptor = {
  domainId: 'neutral-fixture',
  displayName: 'Neutral Fixture',
  version: '0.1.0',
  protocolVersion: '1.0.0',
  actions: [
    { name: 'inspect', kind: 'read', schemaRef: 'neutral-fixture/inspect@1' },
    { name: 'toggle', kind: 'mutate', schemaRef: 'neutral-fixture/toggle@1' },
  ],
};

const reply = (id, result) => process.stdout.write(`${JSON.stringify({ jsonrpc: '2.0', id, result })}\n`);
readline.createInterface({ input: process.stdin }).on('line', (line) => {
  const request = JSON.parse(line);
  if (request.method === 'initialize') {
    reply(request.id, {
      protocolVersion: request.params.protocolVersion,
      capabilities: { tools: {} },
      serverInfo: { name: 'neutral-fixture', version: '0.1.0' },
    });
    return;
  }
  if (request.method === 'tools/call' && request.params.name === 'descriptor') {
    reply(request.id, { content: [], structuredContent: descriptor, isError: false });
  }
});
"#,
        )
        .unwrap();
        registration(
            vec![script.display().to_string(), pid_file.display().to_string()],
            10,
        )
    }

    #[test]
    fn decodes_a_single_json_text_result() {
        let result = CallToolResult::success(vec![Content::text(r#"{"domainId":"fixture"}"#)]);

        let decoded: serde_json::Value = McpDomainAdapter::decode_result(result, 1024).unwrap();

        assert_eq!(decoded["domainId"], "fixture");
    }

    #[test]
    fn rejects_multiple_unstructured_content_blocks() {
        let result = CallToolResult::success(vec![Content::text("{}"), Content::text("{}")]);

        let error = McpDomainAdapter::decode_result::<serde_json::Value>(result, 1024).unwrap_err();

        assert!(error.to_string().contains("ADAPTER_RESPONSE_INVALID"));
    }

    #[tokio::test]
    async fn negotiates_and_invokes_a_neutral_stdio_adapter_process() {
        let directory = TempDir::new().unwrap();
        let expected = descriptor();
        let adapter = McpDomainAdapter::connect(
            write_fixture(&directory),
            expected.clone(),
            directory.path().to_path_buf(),
        )
        .await
        .unwrap();

        assert_eq!(adapter.descriptor(), expected);
        let snapshot = adapter
            .snapshot(DomainSnapshotRequest {
                input: serde_json::json!({ "scope": "neutral" }),
            })
            .await
            .unwrap();
        assert_eq!(snapshot.domain_id, "neutral-fixture");
        assert_eq!(snapshot.payload, serde_json::json!({ "scope": "neutral" }));

        let action = adapter
            .perform_action(DomainActionRequest {
                session_id: "session-a".to_string(),
                generation: 1,
                action: "inspect".to_string(),
                input: serde_json::json!({ "id": "resource-1" }),
            })
            .await
            .unwrap();
        assert_eq!(action.action, "inspect");
        assert_eq!(action.payload, serde_json::json!({ "id": "resource-1" }));
    }

    #[tokio::test]
    async fn executes_a_confirmed_mutation_through_the_live_neutral_adapter() {
        let directory = TempDir::new().unwrap();
        let adapter = Arc::new(
            McpDomainAdapter::connect(
                write_fixture(&directory),
                descriptor(),
                directory.path().to_path_buf(),
            )
            .await
            .unwrap(),
        );
        let runtime = ShellRuntime::new(Default::default(), Some(adapter));

        let pending = runtime
            .perform_domain_action(DomainActionRequest {
                session_id: "session-a".to_string(),
                generation: 7,
                action: "toggle".to_string(),
                input: serde_json::json!({ "enabled": true }),
            })
            .await
            .unwrap();
        let action_id = pending
            .confirmation_action_id
            .expect("mutation requires confirmation");
        assert!(pending.payload.is_null());

        let confirmed = runtime
            .confirm_domain_action(DomainActionConfirmRequest {
                session_id: "session-a".to_string(),
                generation: 7,
                action_id,
                approve: true,
            })
            .await
            .unwrap();

        assert_eq!(confirmed.status, DomainActionConfirmationStatus::Approved);
        let result = confirmed
            .result
            .expect("approval returns the live action result");
        assert_eq!(result.domain_id, "neutral-fixture");
        assert_eq!(result.action, "toggle");
        assert_eq!(result.payload, serde_json::json!({ "enabled": true }));
    }

    #[tokio::test]
    async fn times_out_an_adapter_call_that_stops_responding_after_negotiation() {
        let directory = TempDir::new().unwrap();
        let adapter = McpDomainAdapter::connect(
            write_hanging_fixture(&directory),
            descriptor(),
            directory.path().to_path_buf(),
        )
        .await
        .unwrap();

        let error = adapter
            .snapshot(DomainSnapshotRequest {
                input: serde_json::json!({ "scope": "neutral" }),
            })
            .await
            .unwrap_err();

        assert!(error.to_string().contains("ADAPTER_OPERATION_TIMEOUT"));
        assert_eq!(adapter.status(), DomainAdapterStatus::Hung);
    }

    #[tokio::test]
    async fn rejects_malformed_live_adapter_output_with_a_typed_error() {
        let directory = TempDir::new().unwrap();
        let adapter = McpDomainAdapter::connect(
            write_malformed_fixture(&directory),
            descriptor(),
            directory.path().to_path_buf(),
        )
        .await
        .unwrap();

        let error = adapter
            .snapshot(DomainSnapshotRequest {
                input: serde_json::json!({ "scope": "neutral" }),
            })
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "ADAPTER_RESPONSE_INVALID");
    }

    #[tokio::test]
    async fn maps_a_live_adapter_crash_to_a_typed_error() {
        let directory = TempDir::new().unwrap();
        let adapter = McpDomainAdapter::connect(
            write_crashing_fixture(&directory),
            descriptor(),
            directory.path().to_path_buf(),
        )
        .await
        .unwrap();
        let mut status = adapter
            .subscribe_status()
            .expect("MCP adapter exposes a process status subscription");

        let error = adapter
            .snapshot(DomainSnapshotRequest {
                input: serde_json::json!({ "scope": "neutral" }),
            })
            .await
            .unwrap_err();

        assert_eq!(error.to_string(), "ADAPTER_OPERATION_FAILED");
        tokio::time::timeout(Duration::from_secs(2), status.changed())
            .await
            .expect("adapter exit status arrived")
            .expect("adapter status channel remained open");
        assert_eq!(*status.borrow(), DomainAdapterStatus::Crashed);
    }

    #[tokio::test]
    async fn a_startup_crash_before_negotiation_fails_connect_instead_of_hanging() {
        let directory = TempDir::new().unwrap();

        let result = McpDomainAdapter::connect(
            write_startup_crashing_fixture(&directory),
            descriptor(),
            directory.path().to_path_buf(),
        )
        .await;

        // The exact wording comes from the MCP client's own handshake failure; what matters here
        // is that connect() resolves to an error at all rather than hanging forever on a process
        // that died before completing negotiation.
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn an_idle_crash_with_no_call_in_flight_still_updates_the_safe_status() {
        let directory = TempDir::new().unwrap();
        let adapter = McpDomainAdapter::connect(
            write_idle_crashing_fixture(&directory),
            descriptor(),
            directory.path().to_path_buf(),
        )
        .await
        .unwrap();
        assert_eq!(adapter.status(), DomainAdapterStatus::Ready);
        let mut status = adapter
            .subscribe_status()
            .expect("MCP adapter exposes a process status subscription");

        // No snapshot/action call is ever made — the fixture exits on its own once negotiated.
        tokio::time::timeout(Duration::from_secs(2), status.changed())
            .await
            .expect("adapter exit status arrived")
            .expect("adapter status channel remained open");
        assert_eq!(*status.borrow(), DomainAdapterStatus::Crashed);
    }

    #[tokio::test]
    async fn reconnecting_after_a_crash_starts_ready_with_no_stale_status() {
        let directory = TempDir::new().unwrap();
        let crashed = McpDomainAdapter::connect(
            write_crashing_fixture(&directory),
            descriptor(),
            directory.path().to_path_buf(),
        )
        .await
        .unwrap();
        let mut crashed_status = crashed
            .subscribe_status()
            .expect("MCP adapter exposes a process status subscription");
        let _ = crashed
            .snapshot(DomainSnapshotRequest {
                input: serde_json::json!({ "scope": "neutral" }),
            })
            .await;
        tokio::time::timeout(Duration::from_secs(2), crashed_status.changed())
            .await
            .expect("adapter exit status arrived")
            .expect("adapter status channel remained open");
        assert_eq!(*crashed_status.borrow(), DomainAdapterStatus::Crashed);
        drop(crashed);

        // A supervisor restarting the adapter connects a fresh instance, which must not inherit
        // the prior process's crashed status — each connection owns its own status channel.
        let restarted = McpDomainAdapter::connect(
            write_fixture(&directory),
            descriptor(),
            directory.path().to_path_buf(),
        )
        .await
        .unwrap();
        assert_eq!(restarted.status(), DomainAdapterStatus::Ready);
        let snapshot = restarted
            .snapshot(DomainSnapshotRequest {
                input: serde_json::json!({ "scope": "neutral" }),
            })
            .await
            .unwrap();
        assert_eq!(snapshot.domain_id, "neutral-fixture");
    }

    #[tokio::test]
    async fn a_forced_shutdown_leaves_no_orphaned_adapter_process() {
        let directory = TempDir::new().unwrap();
        let pid_file = directory.path().join("adapter.pid");
        let adapter = McpDomainAdapter::connect(
            write_idle_fixture(&directory, &pid_file),
            descriptor(),
            directory.path().to_path_buf(),
        )
        .await
        .unwrap();

        let pid: u32 = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                if let Ok(contents) = fs::read_to_string(&pid_file) {
                    let trimmed = contents.trim();
                    if let Ok(pid) = trimmed.parse() {
                        return pid;
                    }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("adapter wrote its pid before timing out");
        assert!(
            crate::subprocess::process_is_alive(pid).await,
            "adapter process should be running before shutdown"
        );

        drop(adapter);
        // OperatorChildGuard::drop spawns an async reap task on the current runtime; give it a
        // bounded window rather than asserting immediately.
        tokio::time::timeout(
            Duration::from_secs(2),
            crate::subprocess::wait_for_process_exit(pid, Duration::from_millis(20)),
        )
        .await
        .expect("adapter process must not survive a forced shutdown");
    }
}
