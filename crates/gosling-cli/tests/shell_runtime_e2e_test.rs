use agent_client_protocol::schema::v1::{ClientCapabilities, InitializeRequest, NewSessionRequest};
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::{Agent, Client, ConnectionTo};
use agent_client_protocol_http::HttpClient;
use gosling::acp::custom_requests::{
    GetSessionInfoRequest, GetSessionInfoResponse, GetToolsRequest, GetToolsResponse,
    GoslingToolCallRequest, GoslingToolCallResponse, SetToolPermissionsRequest,
    SetToolPermissionsResponse, ShellCredentialCatalogStatus, ShellCredentialListResponse,
    ShellDirectoryReason, ShellDirectoryStatus, ShellDirectoryValidateResponse,
    ShellModuleListResponse, ShellProvisioningIssueCode, ShellProvisioningReadRequest,
    ShellProvisioningReadResponse, ToolPermissionEntry, ToolPermissionLevel,
    SHELL_MODULE_CONTRACT_VERSION,
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
                        let available_methods = shell["availableMethods"]
                            .as_array()
                            .expect("shell metadata omitted available custom methods");
                        assert!(available_methods.iter().any(|method| {
                            method == "_gosling/unstable/shell/provisioning/read"
                        }));
                        assert!(available_methods
                            .iter()
                            .any(|method| { method == "_gosling/unstable/shell/handoff/prepare" }));
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
        serde_json::to_value(ShellProvisioningReadRequest::default()).unwrap(),
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
        serde_json::to_value(ShellProvisioningReadRequest::default()).unwrap(),
    )
    .await;
    assert!(!startup_preflight.validation.valid);
    assert!(startup_preflight.validation.issues.iter().any(|issue| {
        issue.code == ShellProvisioningIssueCode::MissingSkill
            && issue.path == "session.skillIds[0]"
    }));

    // The same provisioning is valid once judged against the directory the shell selected, which is
    // what lets a shell restore its remembered directory instead of failing the compatibility gate.
    let scoped_preflight: ShellProvisioningReadResponse = custom(
        &cx,
        "_gosling/unstable/shell/provisioning/read",
        serde_json::json!({ "workingDir": requested_dir.path() }),
    )
    .await;
    assert!(
        scoped_preflight.validation.valid,
        "{:?}",
        scoped_preflight.validation.issues
    );

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
        serde_json::to_value(ShellProvisioningReadRequest::default()).unwrap(),
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

#[tokio::test(flavor = "multi_thread")]
async fn shell_directory_credential_and_module_surfaces_are_live_and_bounded() {
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
            "settingsSchemaVersion": 1,
            "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" },
            "session": { "credentialPolicy": "fixed", "extensions": [], "skillIds": [] },
            "instructions": { "systemPrompt": "You are the neutral template assistant." }
        }))
        .unwrap(),
    )
    .unwrap();

    let port = free_port();
    let _server = spawn_server(root.path(), "directory", &provisioning_path, port);
    wait_for_server(port).await;
    let (cx, client_task) = connect(port).await;

    let accepted: ShellDirectoryValidateResponse = custom(
        &cx,
        "_gosling/unstable/shell/directory/validate",
        serde_json::json!({ "path": work.path() }),
    )
    .await;
    assert_eq!(accepted.status, ShellDirectoryStatus::Valid);
    let canonical = accepted.canonical_path.expect("accepted directory");
    assert_eq!(
        std::path::Path::new(&canonical),
        std::fs::canonicalize(work.path()).unwrap()
    );

    for (path, status, reason) in [
        (
            work.path().join("missing").display().to_string(),
            ShellDirectoryStatus::Invalid,
            ShellDirectoryReason::NotFound,
        ),
        (
            "relative/path".to_string(),
            ShellDirectoryStatus::Invalid,
            ShellDirectoryReason::NotAbsolute,
        ),
    ] {
        let rejected: ShellDirectoryValidateResponse = custom(
            &cx,
            "_gosling/unstable/shell/directory/validate",
            serde_json::json!({ "path": path }),
        )
        .await;
        assert_eq!(rejected.status, status);
        assert_eq!(rejected.reason, Some(reason));
        assert!(rejected.canonical_path.is_none(), "no path may be echoed");
    }
    assert!(
        !work.path().join("missing").exists(),
        "validation must not create the directory it was asked about"
    );

    let credentials: ShellCredentialListResponse = custom(
        &cx,
        "_gosling/unstable/shell/credentials/list",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(credentials.status, ShellCredentialCatalogStatus::Denied);
    assert!(credentials.profiles.is_empty());

    let modules: ShellModuleListResponse = custom(
        &cx,
        "_gosling/unstable/shell/modules/list",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(modules.contract_version, SHELL_MODULE_CONTRACT_VERSION);
    assert_eq!(
        modules
            .modules
            .iter()
            .map(|module| module.id.as_str())
            .collect::<Vec<_>>(),
        ["core:session"],
        "an empty shell selection resolves to no extension, skill, or adapter module"
    );

    let session = cx
        .send_request(NewSessionRequest::new(work.path()))
        .block_task()
        .await
        .unwrap();
    let info: GetSessionInfoResponse = custom(
        &cx,
        "_gosling/unstable/session/info",
        serde_json::to_value(GetSessionInfoRequest {
            session_id: session.session_id.0.to_string(),
        })
        .unwrap(),
    )
    .await;
    assert_eq!(
        info.session.cwd.to_string_lossy(),
        canonical,
        "session creation must pin the canonical directory"
    );

    let tools: GetToolsResponse = custom(
        &cx,
        "_gosling/unstable/tools/list",
        serde_json::to_value(GetToolsRequest {
            session_id: session.session_id.0.to_string(),
            extension_name: None,
        })
        .unwrap(),
    )
    .await;
    assert!(
        tools.tools.is_empty(),
        "an empty shell extension selection must yield no developer tools: {:?}",
        tools
            .tools
            .iter()
            .map(|tool| &tool.name)
            .collect::<Vec<_>>()
    );

    for smuggled in [
        serde_json::json!({ "shellCredentialProfileId": "smuggled-profile" }),
        serde_json::json!({
            "workspaceId": "any-workspace",
            "workspaceCredentialProfileId": "smuggled-profile"
        }),
    ] {
        let denied = cx
            .send_request(
                agent_client_protocol::UntypedMessage::new(
                    "session/new",
                    serde_json::json!({
                        "cwd": work.path(),
                        "mcpServers": [],
                        "_meta": smuggled
                    }),
                )
                .unwrap(),
            )
            .block_task()
            .await
            .expect_err("fixed provisioning must refuse any caller-selected credential");
        assert_eq!(
            denied.data.unwrap()["code"],
            "SHELL_CREDENTIAL_SELECTION_DENIED"
        );
    }

    client_task.abort();
}

#[tokio::test(flavor = "multi_thread")]
async fn selectable_catalog_credential_policy_opens_the_catalog_and_caller_selection() {
    let root = TempDir::new().unwrap();
    let work = TempDir::new().unwrap();
    let provisioning_path = write_neutral_provisioning(root.path(), "selectable_catalog");
    std::fs::write(
        root.path().join("config/secrets.yaml"),
        "OPENAI_API_KEY: shell-profile-test-key\n",
    )
    .unwrap();

    let port = free_port();
    let _server = spawn_server(
        root.path(),
        "selectable-credential",
        &provisioning_path,
        port,
    );
    wait_for_server(port).await;
    let (cx, client_task) = connect(port).await;

    // The catalog itself must be reachable rather than fail-closed the way `fixed` policy denies
    // it — whatever profiles this sandbox happens to have configured, the policy gate must open.
    let credentials: ShellCredentialListResponse = custom(
        &cx,
        "_gosling/unstable/shell/credentials/list",
        serde_json::json!({}),
    )
    .await;
    assert_eq!(credentials.status, ShellCredentialCatalogStatus::Available);
    let profile = credentials
        .profiles
        .iter()
        .find(|profile| profile.provider_or_service_id == "openai")
        .expect("test configuration must produce a safe OpenAI profile summary")
        .clone();

    let unknown = cx
        .send_request(
            agent_client_protocol::UntypedMessage::new(
                "session/new",
                serde_json::json!({
                    "cwd": work.path(),
                    "mcpServers": [],
                    "_meta": { "shellCredentialProfileId": "does-not-exist" }
                }),
            )
            .unwrap(),
        )
        .block_task()
        .await
        .expect_err("an unknown selected profile must fail closed before session creation");
    assert_eq!(
        unknown.data.unwrap()["code"],
        "SHELL_CREDENTIAL_PROFILE_UNAVAILABLE"
    );

    let created = cx
        .send_request(
            agent_client_protocol::UntypedMessage::new(
                "session/new",
                serde_json::json!({
                    "cwd": work.path(),
                    "mcpServers": [],
                    "_meta": { "shellCredentialProfileId": profile.id.clone() }
                }),
            )
            .unwrap(),
        )
        .block_task()
        .await
        .expect("a configured catalog profile must create a session");
    let session_id = created["sessionId"].as_str().unwrap();
    let stored = read_session(root.path(), "selectable-credential", session_id).await;
    assert_eq!(
        stored.credential_profile_id.as_deref(),
        Some(profile.id.as_str())
    );
    assert_eq!(
        stored.credential_profile_name.as_deref(),
        Some(profile.name.as_str())
    );

    client_task.abort();
}

fn write_neutral_provisioning(root: &Path, credential_policy: &str) -> std::path::PathBuf {
    std::fs::create_dir_all(root.join("config")).unwrap();
    std::fs::write(
        root.join("config/config.yaml"),
        "GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\nGOSLING_DISABLE_KEYRING: true\n",
    )
    .unwrap();
    let provisioning_path = root.join("provisioning.json");
    std::fs::write(
        &provisioning_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "settingsSchemaVersion": 1,
            "identity": { "id": "ignored", "displayName": "Ignored", "version": "0" },
            "session": { "credentialPolicy": credential_policy, "extensions": [], "skillIds": [] },
            "instructions": { "systemPrompt": "You are the neutral template assistant." }
        }))
        .unwrap(),
    )
    .unwrap();
    provisioning_path
}

/// Addresses SHP-DEF-007: no test previously launched two shell identities as live concurrent
/// processes and observed them. Two full server processes, each with its own path root, run at
/// the same time and must not observe or disturb each other's sessions, and neither may survive
/// the other's independent shutdown.
///
/// `multi_thread` matters here, not just as a nicety: `ServeProcess::drop` does a blocking
/// `Child::wait()` with no timeout, and this test drops two server handles mid-test. On the
/// default current-thread runtime that blocking wait would tie up the only executor thread; a
/// dedicated worker thread keeps it from stalling the runtime the rest of the test still needs.
#[tokio::test(flavor = "multi_thread")]
async fn two_shell_identities_coexist_as_live_concurrent_processes() {
    let root_a = TempDir::new().unwrap();
    let root_b = TempDir::new().unwrap();
    let work_a = TempDir::new().unwrap();
    let work_b = TempDir::new().unwrap();
    let provisioning_a = write_neutral_provisioning(root_a.path(), "fixed");
    let provisioning_b = write_neutral_provisioning(root_b.path(), "fixed");

    let port_a = free_port();
    let mut port_b = free_port();
    while port_b == port_a {
        port_b = free_port();
    }
    let server_a = spawn_server(root_a.path(), "coexist-a", &provisioning_a, port_a);
    let server_b = spawn_server(root_b.path(), "coexist-b", &provisioning_b, port_b);
    let pid_a = server_a.0.id();
    let pid_b = server_b.0.id();
    assert_ne!(pid_a, pid_b, "each identity must be its own OS process");

    tokio::join!(wait_for_server(port_a), wait_for_server(port_b));
    let ((cx_a, task_a), (cx_b, task_b)) = tokio::join!(connect(port_a), connect(port_b));

    let (session_a, session_b) = tokio::join!(
        cx_a.send_request(NewSessionRequest::new(work_a.path()))
            .block_task(),
        cx_b.send_request(NewSessionRequest::new(work_b.path()))
            .block_task(),
    );
    let session_a = session_a.unwrap();
    let session_b = session_b.unwrap();
    // Session ids are a per-store day counter, so two freshly isolated stores legitimately mint
    // the same first id independently — that's a feature of isolation, not a collision. The real
    // proof is that each store only ever holds its own session, checked below.

    // Each server persisted its own session under its own path root. Both stores independently
    // minted the same first-of-the-day session id (a per-store counter, not a global one), so
    // resolving that same id against each root's own store correctly and only ever returning
    // that root's own working directory is what proves the two stores are genuinely separate
    // rather than sharing state.
    let (found_a, found_b) = tokio::join!(
        read_session(root_a.path(), "coexist-a", &session_a.session_id.0),
        read_session(root_b.path(), "coexist-b", &session_b.session_id.0),
    );
    assert_eq!(
        found_a.working_dir,
        std::fs::canonicalize(work_a.path()).unwrap()
    );
    assert_eq!(
        found_b.working_dir,
        std::fs::canonicalize(work_b.path()).unwrap()
    );

    // Each server independently answers a live request while the other is also active.
    let (info_a, info_b): (GetSessionInfoResponse, GetSessionInfoResponse) = tokio::join!(
        custom(
            &cx_a,
            "_gosling/unstable/session/info",
            serde_json::to_value(GetSessionInfoRequest {
                session_id: session_a.session_id.0.to_string(),
            })
            .unwrap(),
        ),
        custom(
            &cx_b,
            "_gosling/unstable/session/info",
            serde_json::to_value(GetSessionInfoRequest {
                session_id: session_b.session_id.0.to_string(),
            })
            .unwrap(),
        ),
    );
    assert_eq!(
        info_a.session.cwd,
        std::fs::canonicalize(work_a.path()).unwrap()
    );
    assert_eq!(
        info_b.session.cwd,
        std::fs::canonicalize(work_b.path()).unwrap()
    );

    task_a.abort();
    task_b.abort();

    // Shut down identity A only; identity B must survive untouched.
    drop(server_a);
    tokio::time::timeout(
        Duration::from_secs(5),
        gosling::subprocess::wait_for_process_exit(pid_a, Duration::from_millis(20)),
    )
    .await
    .expect("identity A's process must not survive its own shutdown");
    assert!(
        gosling::subprocess::process_is_alive(pid_b).await,
        "identity B must keep running after only identity A is shut down"
    );

    drop(server_b);
    tokio::time::timeout(
        Duration::from_secs(5),
        gosling::subprocess::wait_for_process_exit(pid_b, Duration::from_millis(20)),
    )
    .await
    .expect("identity B's process must not survive its own shutdown");
}
