use agent_client_protocol::schema::v1::{ClientCapabilities, InitializeRequest, NewSessionRequest};
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::{Agent, Client, ConnectionTo};
use agent_client_protocol_http::HttpClient;
use gosling::acp::custom_requests::{
    GetSessionInfoRequest, GetSessionInfoResponse, ShellProvisioningReadRequest,
    ShellProvisioningReadResponse,
};
use gosling::session::{EnabledExtensionsState, ExtensionState, ShellSkillSelectionState};
use std::net::TcpListener;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const SECRET: &str = "shell-runtime-e2e-secret";

struct ServeProcess(Child);

impl Drop for ServeProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn spawn_server(root: &Path, namespace: &str, provisioning: &Path, port: u16) -> ServeProcess {
    let child = Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args([
            "serve",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--shell-id",
            "test_shell",
            "--shell-display-name",
            "Test Shell",
            "--shell-version",
            "7",
            "--shell-runtime-namespace",
            namespace,
            "--shell-provisioning",
            provisioning.to_str().unwrap(),
            "--with-builtin",
            "developer",
        ])
        .env("GOSLING_PATH_ROOT", root)
        .env("GOSLING_DISABLE_KEYRING", "1")
        .env("GOSLING_SERVER__SECRET_KEY", SECRET)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("failed to spawn gosling serve");
    ServeProcess(child)
}

async fn wait_for_server(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(20);
    let url = format!("http://127.0.0.1:{port}/health");
    while Instant::now() < deadline {
        if reqwest::get(&url)
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            return;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    panic!("gosling serve did not become ready");
}

fn authenticated_client(port: u16) -> HttpClient {
    let http = reqwest::Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("X-Secret-Key", SECRET.parse().unwrap());
            headers
        })
        .build()
        .unwrap();
    HttpClient::with_client(format!("http://127.0.0.1:{port}"), http).unwrap()
}

async fn connect(port: u16) -> (ConnectionTo<Agent>, tokio::task::JoinHandle<()>) {
    let holder = Arc::new(Mutex::new(None));
    let holder_for_task = Arc::clone(&holder);
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let result = Client
            .builder()
            .connect_with(
                authenticated_client(port),
                move |cx: ConnectionTo<Agent>| {
                    let holder = Arc::clone(&holder_for_task);
                    async move {
                        let response = cx
                            .send_request(
                                InitializeRequest::new(ProtocolVersion::LATEST)
                                    .client_capabilities(ClientCapabilities::default()),
                            )
                            .block_task()
                            .await?;
                        let shell = response
                            .agent_capabilities
                            .meta
                            .as_ref()
                            .and_then(|meta| meta.get("goslingShell"))
                            .expect("initialize response omitted shell metadata");
                        assert_eq!(shell["identity"]["id"], "test_shell");
                        assert_eq!(shell["identity"]["version"], "7");
                        *holder.lock().unwrap() = Some(cx.clone());
                        let _ = ready_tx.send(());
                        std::future::pending::<Result<(), agent_client_protocol::Error>>().await
                    }
                },
            )
            .await;
        if let Err(error) = result {
            eprintln!("ACP client exited: {error}");
        }
    });
    ready_rx.await.unwrap();
    let cx = holder.lock().unwrap().take().unwrap();
    (cx, task)
}

async fn custom<T: serde::de::DeserializeOwned>(
    cx: &ConnectionTo<Agent>,
    method: &str,
    params: serde_json::Value,
) -> T {
    let message = agent_client_protocol::UntypedMessage::new(method, params).unwrap();
    let value = cx.send_request(message).block_task().await.unwrap();
    serde_json::from_value(value).unwrap()
}

async fn read_session(
    path_root: &Path,
    namespace: &str,
    session_id: &str,
) -> gosling::session::Session {
    let data_dir = path_root.join("data/shells").join(namespace);
    gosling::session::SessionManager::new(data_dir)
        .get_session(session_id, false)
        .await
        .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn spawned_shell_runtime_applies_provisioning_and_isolates_sessions() {
    let root = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    std::fs::create_dir_all(root.path().join("config")).unwrap();
    std::fs::write(
        root.path().join("config/config.yaml"),
        "GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\nGOSLING_DISABLE_KEYRING: true\n",
    )
    .unwrap();
    let provisioning_path = root.path().join("provisioning.json");
    std::fs::write(
        &provisioning_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" },
            "session": {
                "extensions": [
                    { "name": "developer", "availableTools": ["shell"] },
                    { "name": "skills" }
                ],
                "skillIds": ["gosling-doc-guide"]
            },
            "protocolPolicy": {
                "mode": "restricted",
                "deniedMethods": ["_gosling/unstable/config/upsert"]
            }
        }))
        .unwrap(),
    )
    .unwrap();

    let port = free_port();
    let _server = spawn_server(root.path(), "alpha", &provisioning_path, port);
    wait_for_server(port).await;
    let (cx, client_task) = connect(port).await;

    let provisioning: ShellProvisioningReadResponse = custom(
        &cx,
        "_gosling/unstable/shell/provisioning/read",
        serde_json::to_value(ShellProvisioningReadRequest {}).unwrap(),
    )
    .await;
    assert!(
        provisioning.validation.valid,
        "{:?}",
        provisioning.validation.issues
    );
    assert_eq!(provisioning.provisioning.identity.id, "test_shell");

    let session = cx
        .send_request(NewSessionRequest::new(work.path()))
        .block_task()
        .await
        .unwrap();
    let session_id = session.session_id.0.to_string();
    let info: GetSessionInfoResponse = custom(
        &cx,
        "_gosling/unstable/session/info",
        serde_json::to_value(GetSessionInfoRequest {
            session_id: session_id.clone(),
        })
        .unwrap(),
    )
    .await;
    assert_eq!(info.session.session_id.0.as_ref(), session_id);
    assert_eq!(
        gosling::session::SessionManager::new(root.path().join("data"))
            .list_sessions()
            .await
            .unwrap()
            .len(),
        0,
        "main Gosling session storage must remain isolated"
    );
    assert_eq!(
        gosling::session::SessionManager::new(root.path().join("data/shells/beta"))
            .list_sessions()
            .await
            .unwrap()
            .len(),
        0,
        "unrelated shell namespaces must remain isolated"
    );

    let denied = agent_client_protocol::UntypedMessage::new(
        "_gosling/unstable/config/upsert",
        serde_json::json!({"key": "TEST", "value": "no"}),
    )
    .unwrap();
    let error = cx.send_request(denied).block_task().await.unwrap_err();
    assert_eq!(i32::from(error.code), -32003);

    drop(cx);
    client_task.abort();
    drop(_server);

    let stored = read_session(root.path(), "alpha", &session_id).await;
    let extensions = EnabledExtensionsState::from_extension_data(&stored.extension_data).unwrap();
    let names = extensions
        .extensions
        .iter()
        .map(|extension| extension.name())
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        names,
        ["developer", "skills"]
            .into_iter()
            .map(String::from)
            .collect()
    );
    let developer = extensions
        .extensions
        .iter()
        .find(|extension| extension.name() == "developer")
        .unwrap();
    assert!(!developer.is_tool_available("read_file"));
    let skills = ShellSkillSelectionState::from_extension_data(&stored.extension_data).unwrap();
    assert_eq!(skills.skill_ids, vec!["gosling-doc-guide"]);

    let restart_port = free_port();
    let _restart = spawn_server(root.path(), "alpha", &provisioning_path, restart_port);
    wait_for_server(restart_port).await;
    let (restart_cx, restart_task) = connect(restart_port).await;
    let reloaded: GetSessionInfoResponse = custom(
        &restart_cx,
        "_gosling/unstable/session/info",
        serde_json::to_value(GetSessionInfoRequest {
            session_id: session_id.clone(),
        })
        .unwrap(),
    )
    .await;
    assert_eq!(reloaded.session.session_id.0.as_ref(), session_id);
    restart_task.abort();
}
