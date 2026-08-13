use agent_client_protocol::schema::v1::{ClientCapabilities, InitializeRequest, NewSessionRequest};
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::{Agent, Client, ConnectionTo};
use agent_client_protocol_http::HttpClient;
use gosling::acp::custom_requests::{
    GetSessionInfoRequest, GetSessionInfoResponse, GetToolsRequest, GetToolsResponse,
    GoslingToolCallRequest, GoslingToolCallResponse, SetToolPermissionsRequest,
    SetToolPermissionsResponse, ShellProvisioningIssueCode, ShellProvisioningReadRequest,
    ShellProvisioningReadResponse, ToolPermissionEntry, ToolPermissionLevel,
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
    spawn_server_from(root, namespace, provisioning, port, None)
}

fn spawn_server_from(
    root: &Path,
    namespace: &str,
    provisioning: &Path,
    port: u16,
    startup_dir: Option<&Path>,
) -> ServeProcess {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gosling"));
    command
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
        .stderr(Stdio::null());
    if let Some(startup_dir) = startup_dir {
        command.current_dir(startup_dir);
    }
    ServeProcess(command.spawn().expect("failed to spawn gosling serve"))
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

fn write_project_skill(working_dir: &Path, name: &str, description: &str) {
    let skill_dir = working_dir.join(".agents/skills").join(name);
    std::fs::create_dir_all(&skill_dir).unwrap();
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            "---\nname: {name}\ndescription: {description}\n---\n\n# {name}\n\n{description}\n"
        ),
    )
    .unwrap();
}

fn write_skill_provisioning(path: &Path, skill_id: &str) {
    std::fs::write(
        path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" },
            "session": {
                "extensions": [{ "name": "skills" }],
                "skillIds": [skill_id]
            }
        }))
        .unwrap(),
    )
    .unwrap();
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

#[tokio::test(flavor = "multi_thread")]
async fn session_preflight_and_runtime_use_the_requested_working_directory() {
    let root = TempDir::new().unwrap();
    let startup_dir = TempDir::new().unwrap();
    let requested_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(root.path().join("config")).unwrap();
    std::fs::write(
        root.path().join("config/config.yaml"),
        "GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\nGOSLING_DISABLE_KEYRING: true\n",
    )
    .unwrap();
    write_project_skill(
        requested_dir.path(),
        "requested-only-skill",
        "Only the requested session directory provides this skill.",
    );
    let provisioning_path = root.path().join("requested-provisioning.json");
    write_skill_provisioning(&provisioning_path, "requested-only-skill");

    let port = free_port();
    let _server = spawn_server_from(
        root.path(),
        "requested-cwd",
        &provisioning_path,
        port,
        Some(startup_dir.path()),
    );
    wait_for_server(port).await;
    let (cx, client_task) = connect(port).await;

    let startup_preflight: ShellProvisioningReadResponse = custom(
        &cx,
        "_gosling/unstable/shell/provisioning/read",
        serde_json::to_value(ShellProvisioningReadRequest {}).unwrap(),
    )
    .await;
    assert!(!startup_preflight.validation.valid);
    assert!(startup_preflight.validation.issues.iter().any(|issue| {
        issue.code == ShellProvisioningIssueCode::MissingSkill
            && issue.path == "session.skillIds[0]"
    }));

    let session = cx
        .send_request(NewSessionRequest::new(requested_dir.path()))
        .block_task()
        .await
        .expect("session-specific preflight should use the requested directory");
    let session_id = session.session_id.0.to_string();
    let tools: GetToolsResponse = custom(
        &cx,
        "_gosling/unstable/tools/list",
        serde_json::to_value(GetToolsRequest {
            session_id: session_id.clone(),
            extension_name: Some("skills".into()),
        })
        .unwrap(),
    )
    .await;
    assert!(tools
        .tools
        .iter()
        .any(|tool| tool.name.ends_with("load_skill")));
    let load_skill_name = tools
        .tools
        .iter()
        .find(|tool| tool.name.ends_with("load_skill"))
        .unwrap()
        .name
        .clone();
    let _: SetToolPermissionsResponse = custom(
        &cx,
        "_gosling/unstable/tools/permissions/set",
        serde_json::to_value(SetToolPermissionsRequest {
            tool_permissions: vec![ToolPermissionEntry {
                tool_name: load_skill_name.clone(),
                permission: ToolPermissionLevel::AlwaysAllow,
            }],
        })
        .unwrap(),
    )
    .await;

    let loaded: GoslingToolCallResponse = custom(
        &cx,
        "_gosling/unstable/tools/call",
        serde_json::to_value(GoslingToolCallRequest {
            session_id,
            name: load_skill_name,
            arguments: serde_json::json!({"name": "requested-only-skill"}),
        })
        .unwrap(),
    )
    .await;
    assert!(!loaded.is_error);
    assert!(serde_json::to_string(&loaded.content)
        .unwrap()
        .contains("requested-only-skill"));
    client_task.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn session_preflight_rejects_skills_available_only_in_the_startup_directory() {
    let root = TempDir::new().unwrap();
    let startup_dir = TempDir::new().unwrap();
    let requested_dir = TempDir::new().unwrap();
    std::fs::create_dir_all(root.path().join("config")).unwrap();
    std::fs::write(
        root.path().join("config/config.yaml"),
        "GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\nGOSLING_DISABLE_KEYRING: true\n",
    )
    .unwrap();
    write_project_skill(
        startup_dir.path(),
        "startup-only-skill",
        "Only the server startup directory provides this skill.",
    );
    let provisioning_path = root.path().join("startup-provisioning.json");
    write_skill_provisioning(&provisioning_path, "startup-only-skill");

    let port = free_port();
    let _server = spawn_server_from(
        root.path(),
        "startup-cwd",
        &provisioning_path,
        port,
        Some(startup_dir.path()),
    );
    wait_for_server(port).await;
    let (cx, client_task) = connect(port).await;

    let startup_preflight: ShellProvisioningReadResponse = custom(
        &cx,
        "_gosling/unstable/shell/provisioning/read",
        serde_json::to_value(ShellProvisioningReadRequest {}).unwrap(),
    )
    .await;
    assert!(startup_preflight.validation.valid);

    let error = cx
        .send_request(NewSessionRequest::new(requested_dir.path()))
        .block_task()
        .await
        .expect_err("session preflight must reject startup-only selections");
    let data = error
        .data
        .expect("invalid provisioning should include a report");
    assert_eq!(data["validation"]["valid"], false);
    assert_eq!(data["validation"]["issues"][0]["code"], "missing_skill");
    assert_eq!(
        gosling::session::SessionManager::new(root.path().join("data/shells/startup-cwd"))
            .list_sessions()
            .await
            .unwrap()
            .len(),
        0,
        "invalid session-specific provisioning must fail before durable session creation"
    );
    client_task.abort();
}
