use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

struct MockProvider {
    port: u16,
    chat_requests: Arc<AtomicUsize>,
}

impl MockProvider {
    fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let chat_requests = Arc::new(AtomicUsize::new(0));
        let counter = chat_requests.clone();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let counter = counter.clone();
                std::thread::spawn(move || serve(stream, &counter));
            }
        });
        Self {
            port,
            chat_requests,
        }
    }
}

fn serve(mut stream: TcpStream, chat_requests: &AtomicUsize) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut request_line = String::new();
    if reader.read_line(&mut request_line).is_err() {
        return;
    }
    let mut content_length = 0usize;
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
            break;
        }
        if let Some((name, value)) = line.split_once(':') {
            if name.eq_ignore_ascii_case("content-length") {
                content_length = value.trim().parse().unwrap_or(0);
            }
        }
    }
    let mut body = vec![0u8; content_length];
    let _ = reader.read_exact(&mut body);
    let body = String::from_utf8_lossy(&body);

    let (status, content_type, payload) = if request_line.contains("/chat/completions") {
        chat_requests.fetch_add(1, Ordering::SeqCst);
        if body.contains("CX07-NO-SESSION-CANCELLED-20260913") {
            std::thread::sleep(Duration::from_secs(10));
            (
                "200 OK",
                "application/json",
                "{\"id\":\"late\",\"choices\":[]}".to_string(),
            )
        } else if body.contains("CX07-NO-SESSION-FAILURE-20260913") {
            (
                "400 Bad Request",
                "application/json",
                "{\"error\":{\"message\":\"forced provider failure\"}}".to_string(),
            )
        } else if body.contains("C11-CALL-TREE")
            && body.contains("\"tools\"")
            && body.contains("\"stream\":true")
        {
            let tool_call = serde_json::json!({
                "id": "r",
                "object": "chat.completion.chunk",
                "created": 0,
                "model": "gpt-4o",
                "choices": [{
                    "index": 0,
                    "delta": {"role": "assistant", "tool_calls": [{
                        "index": 0,
                        "id": "call_c11",
                        "type": "function",
                        "function": {"name": "tree", "arguments": "{\"path\": \".\"}"},
                    }]},
                    "finish_reason": "tool_calls",
                }],
            });
            (
                "200 OK",
                "text/event-stream",
                format!("data: {tool_call}\n\ndata: [DONE]\n\n"),
            )
        } else if body.contains("\"stream\":true") {
            let chunk = |delta: &str, finish: &str| {
                format!(
                    "data: {{\"id\":\"r\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4o\",\"choices\":[{{\"index\":0,\"delta\":{delta},\"finish_reason\":{finish}}}]}}\n\n"
                )
            };
            let response_text = if body.contains("CX07-NO-SESSION-SUCCESS-20260913") {
                "CX07-NO-SESSION-RESPONSE-20260913"
            } else {
                "MOCK-REPLY"
            };
            let response_delta = serde_json::json!({
                "role": "assistant",
                "content": response_text,
            })
            .to_string();
            (
                "200 OK",
                "text/event-stream",
                format!(
                    "{}{}data: [DONE]\n\n",
                    chunk(&response_delta, "null"),
                    chunk("{}", "\"stop\"")
                ),
            )
        } else {
            (
                "200 OK",
                "application/json",
                "{\"id\":\"r\",\"object\":\"chat.completion\",\"created\":0,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":\"MOCK-REPLY\"},\"finish_reason\":\"stop\"}]}".to_string(),
            )
        }
    } else {
        (
            "200 OK",
            "application/json",
            "{\"object\":\"list\",\"data\":[]}".to_string(),
        )
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
}

struct Env {
    root: TempDir,
    mock: MockProvider,
}

impl Env {
    fn new() -> Self {
        let root = TempDir::new().unwrap();
        let config_dir = root.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.yaml"),
            "GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\n",
        )
        .unwrap();
        Self {
            root,
            mock: MockProvider::start(),
        }
    }

    fn command(&self, cwd: &Path, args: &[&str]) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_gosling"));
        command
            .args(args)
            .current_dir(cwd)
            .env("GOSLING_PATH_ROOT", self.root.path())
            .env("GOSLING_DISABLE_KEYRING", "1")
            .env(
                "OPENAI_HOST",
                format!("http://127.0.0.1:{}", self.mock.port),
            )
            .env("OPENAI_API_KEY", "sk-test")
            .env_remove("GOSLING_MODE")
            .env_remove("GOSLING_PROVIDER")
            .env_remove("GOSLING_MODEL");
        command
    }

    fn gosling(&self, cwd: &Path, args: &[&str]) -> Output {
        self.command(cwd, args).output().unwrap()
    }

    fn run_ok(&self, cwd: &Path, args: &[&str]) -> Output {
        let output = self.gosling(cwd, args);
        assert!(
            output.status.success(),
            "gosling {args:?} failed\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        output
    }

    fn export(&self, session_id: &str) -> serde_json::Value {
        let output = self.run_ok(
            self.root.path(),
            &[
                "session",
                "export",
                "--session-id",
                session_id,
                "--format",
                "json",
            ],
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

fn banner_session_id(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .split(|c: char| !(c.is_ascii_digit() || c == '_'))
        .find(|token| {
            token.len() > 9
                && token.as_bytes()[8] == b'_'
                && token.bytes().take(8).all(|b| b.is_ascii_digit())
        })
        .unwrap_or_else(|| panic!("no session id in output: {stdout}"))
        .to_string()
}

fn files_containing(root: &Path, marker: &[u8]) -> Vec<std::path::PathBuf> {
    let mut matches = Vec::new();
    let Ok(entries) = std::fs::read_dir(root) else {
        return matches;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            matches.extend(files_containing(&path, marker));
        } else if std::fs::read(&path).is_ok_and(|contents| {
            contents
                .windows(marker.len())
                .any(|window| window == marker)
        }) {
            matches.push(path);
        }
    }
    matches
}

#[test]
fn resume_keeps_the_sessions_stored_permission_mode() {
    let env = Env::new();
    let cwd = env.root.path();

    let created = env
        .command(cwd, &["run", "-n", "approve-me", "-t", "hi"])
        .env("GOSLING_MODE", "approve")
        .output()
        .unwrap();
    assert!(created.status.success());
    let approve_id = banner_session_id(&created);
    assert_eq!(env.export(&approve_id)["gosling_mode"], "approve");

    env.run_ok(cwd, &["run", "-r", "-n", "approve-me", "-t", "again"]);
    assert_eq!(env.export(&approve_id)["gosling_mode"], "approve");

    let fresh = env.run_ok(cwd, &["run", "-n", "default-mode", "-t", "hi"]);
    assert_eq!(
        env.export(&banner_session_id(&fresh))["gosling_mode"],
        "auto"
    );
}

fn listed_sessions(env: &Env) -> Vec<(String, u64)> {
    let output = env.run_ok(env.root.path(), &["session", "list", "--format", "json"]);
    let sessions: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
    sessions
        .iter()
        .map(|session| {
            (
                session["id"].as_str().unwrap().to_string(),
                session["message_count"].as_u64().unwrap(),
            )
        })
        .collect()
}

#[test]
fn a_start_that_fails_before_its_first_turn_leaves_no_session() {
    let env = Env::new();
    let cwd = env.root.path();
    let kept = env.run_ok(cwd, &["run", "-n", "kept", "-t", "hi"]);
    let kept_id = banner_session_id(&kept);

    for args in [
        &["run", "-t", "Say READY"][..],
        &["run", "-n", "named-start", "-t", "Say READY"][..],
        &["session"][..],
    ] {
        let failed = env
            .command(cwd, args)
            .env("GOSLING_PROVIDER", "nonsense-prov")
            .output()
            .unwrap();
        assert!(!failed.status.success(), "{args:?} must fail to start");
    }
    std::fs::write(
        env.root.path().join("config").join("config.yaml"),
        "GOSLING_MODEL: gpt-4o\n",
    )
    .unwrap();
    let unconfigured = env.gosling(cwd, &["run", "-t", "Say READY"]);
    assert!(!unconfigured.status.success());

    let resume_failure = env.gosling(
        cwd,
        &[
            "run",
            "-r",
            "--session-id",
            &kept_id,
            "--provider",
            "nonsense-prov",
            "-t",
            "again",
        ],
    );
    assert!(!resume_failure.status.success());

    assert_eq!(listed_sessions(&env), vec![(kept_id, 2)]);
}

/// GSL-PT-20260927-G130: an unreadable GOSLING_MODE used to start Autonomous
/// sessions silently; it must fall back to asking before every tool call.
#[test]
fn invalid_mode_starts_sessions_that_ask_before_tools() {
    let env = Env::new();
    let cwd = env.root.path();

    for (value, session_name) in [("yolo", "invalid-yolo"), ("aprove", "invalid-typo")] {
        let created = env
            .command(cwd, &["run", "-n", session_name, "-t", "hi"])
            .env("GOSLING_MODE", value)
            .output()
            .unwrap();
        assert!(created.status.success());
        let stderr = String::from_utf8_lossy(&created.stderr);
        assert!(
            stderr.contains("Invalid GOSLING_MODE") && stderr.contains("New sessions use approve"),
            "stderr: {stderr}"
        );
        assert_eq!(
            env.export(&banner_session_id(&created))["gosling_mode"],
            "approve",
            "{value}"
        );
    }

    let unset = env.run_ok(cwd, &["run", "-n", "unset-mode", "-t", "hi"]);
    assert!(!String::from_utf8_lossy(&unset.stderr).contains("Invalid GOSLING_MODE"));
    assert_eq!(
        env.export(&banner_session_id(&unset))["gosling_mode"],
        "auto"
    );
}

#[test]
fn resuming_from_another_directory_moves_the_session_to_it() {
    let env = Env::new();
    let dir_a = env.root.path().join("dir-a");
    let dir_b = env.root.path().join("dir-b");
    std::fs::create_dir_all(&dir_a).unwrap();
    std::fs::create_dir_all(&dir_b).unwrap();
    let dir_a = dir_a.canonicalize().unwrap();
    let dir_b = dir_b.canonicalize().unwrap();

    let created = env.run_ok(&dir_a, &["run", "-n", "wd", "-t", "hi"]);
    let id = banner_session_id(&created);

    env.run_ok(&dir_a, &["run", "-r", "-n", "wd", "-t", "same dir"]);
    assert_eq!(env.export(&id)["working_dir"], dir_a.to_str().unwrap());

    let moved = env.run_ok(&dir_b, &["run", "-r", "-n", "wd", "-t", "other dir"]);
    let warning = String::from_utf8_lossy(&moved.stderr);
    assert!(warning.contains("Staying in current directory"));
    assert!(warning.contains(&format!(
        "the session's working directory is now {}",
        dir_b.display()
    )));
    assert_eq!(env.export(&id)["working_dir"], dir_b.to_str().unwrap());
}

#[test]
fn resuming_a_restricted_session_elsewhere_keeps_its_working_directory() {
    let env = Env::new();
    let trusted = env.root.path().join("trusted");
    let elsewhere = env.root.path().join("elsewhere");
    std::fs::create_dir_all(&trusted).unwrap();
    std::fs::create_dir_all(&elsewhere).unwrap();
    let trusted = trusted.canonicalize().unwrap();
    let elsewhere = elsewhere.canonicalize().unwrap();

    let source = env.run_ok(&trusted, &["run", "-n", "source", "-t", "hi"]);
    let export_path = env.root.path().join("source.json");
    env.run_ok(
        env.root.path(),
        &[
            "session",
            "export",
            "--session-id",
            &banner_session_id(&source),
            "--format",
            "json",
            "-o",
            export_path.to_str().unwrap(),
        ],
    );
    let imported = env.run_ok(
        &elsewhere,
        &[
            "session",
            "import",
            export_path.to_str().unwrap(),
            "--working-dir",
            trusted.to_str().unwrap(),
        ],
    );
    let imported_id = banner_session_id(&imported);
    assert_eq!(
        env.export(&imported_id)["working_dir"],
        trusted.to_str().unwrap()
    );

    let resumed = env.run_ok(
        &elsewhere,
        &["run", "-r", "--session-id", &imported_id, "-t", "again"],
    );

    assert!(String::from_utf8_lossy(&resumed.stderr)
        .contains("restricted to its working directory; switching to"));
    let after = env.export(&imported_id);
    assert_eq!(after["working_dir"], trusted.to_str().unwrap());
    assert_eq!(after["restrict_tools_to_working_dirs"], true);
}

#[test]
fn no_session_run_leaves_nothing_resumable() {
    let env = Env::new();
    let cwd = env.root.path();
    let marker = "CX07-NO-SESSION-SUCCESS-20260913";
    let response_marker = "CX07-NO-SESSION-RESPONSE-20260913";

    let ephemeral = env.run_ok(cwd, &["run", "--no-session", "-t", marker]);
    let id = banner_session_id(&ephemeral);
    assert_eq!(
        env.mock.chat_requests.load(Ordering::SeqCst),
        1,
        "a --no-session run must not make a title-generation request"
    );

    let export = env.gosling(cwd, &["session", "export", "--session-id", &id]);
    assert!(!export.status.success());
    let resume = env.gosling(cwd, &["run", "-r", "--session-id", &id, "-t", "probe"]);
    assert!(!resume.status.success());
    assert_eq!(
        files_containing(env.root.path(), marker.as_bytes()),
        Vec::<std::path::PathBuf>::new(),
        "--no-session prompt bytes must not remain anywhere in the durable path root"
    );
    assert_eq!(
        files_containing(env.root.path(), response_marker.as_bytes()),
        Vec::<std::path::PathBuf>::new(),
        "--no-session response bytes must not remain anywhere in the durable path root"
    );

    let kept = env.run_ok(cwd, &["run", "-n", "kept", "-t", "hi"]);
    assert_eq!(
        env.export(&banner_session_id(&kept))["name"],
        serde_json::Value::String("kept".to_string())
    );
}

#[test]
fn failed_no_session_run_leaves_no_prompt_bytes() {
    let env = Env::new();
    let marker = "CX07-NO-SESSION-FAILURE-20260913";

    let failed = env.gosling(env.root.path(), &["run", "--no-session", "-t", marker]);

    assert!(!failed.status.success());
    assert_eq!(
        files_containing(env.root.path(), marker.as_bytes()),
        Vec::<std::path::PathBuf>::new(),
        "failed --no-session prompt bytes must not remain in the durable path root"
    );
}

#[cfg(unix)]
#[test]
fn cancelled_no_session_run_leaves_no_prompt_bytes() {
    let env = Env::new();
    let marker = "CX07-NO-SESSION-CANCELLED-20260913";
    let mut child = env
        .command(env.root.path(), &["run", "--no-session", "-t", marker])
        .spawn()
        .unwrap();

    for _ in 0..500 {
        if env.mock.chat_requests.load(Ordering::SeqCst) > 0 {
            break;
        }
        if let Some(status) = child.try_wait().unwrap() {
            panic!("--no-session child exited before reaching the provider: {status}");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(env.mock.chat_requests.load(Ordering::SeqCst) > 0);
    unsafe {
        libc::kill(child.id() as libc::pid_t, libc::SIGINT);
    }
    let status = child.wait().unwrap();

    assert!(!status.success());
    assert_eq!(
        files_containing(env.root.path(), marker.as_bytes()),
        Vec::<std::path::PathBuf>::new(),
        "cancelled --no-session prompt bytes must not remain in the durable path root"
    );
}

#[test]
fn project_tracking_records_session_runs_not_other_commands() {
    let env = Env::new();
    let project_dir = env.root.path().join("project");
    let elsewhere = env.root.path().join("elsewhere");
    std::fs::create_dir_all(&project_dir).unwrap();
    std::fs::create_dir_all(&elsewhere).unwrap();

    let listing = env.run_ok(&elsewhere, &["projects"]);
    assert_eq!(
        String::from_utf8_lossy(&listing.stdout).trim(),
        "No projects found."
    );

    let run = env.run_ok(&project_dir, &["run", "-t", "hi"]);
    let id = banner_session_id(&run);
    env.run_ok(&elsewhere, &["doctor"]);

    let listing = env.run_ok(&elsewhere, &["projects"]);
    let listing = String::from_utf8_lossy(&listing.stdout);
    let project_path = project_dir.canonicalize().unwrap();
    assert!(
        listing.contains(project_path.to_str().unwrap()),
        "{listing}"
    );
    assert!(!listing.contains("elsewhere"), "{listing}");

    let tracker: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(env.root.path().join("data").join("projects.json")).unwrap(),
    )
    .unwrap();
    let entry = &tracker["projects"][project_path.to_str().unwrap()];
    assert_eq!(entry["last_session_id"], serde_json::Value::String(id));
}

#[test]
fn configure_refuses_a_config_file_it_cannot_parse() {
    let env = Env::new();
    let config_file = env.root.path().join("config").join("config.yaml");
    let valid = env.gosling(env.root.path(), &["configure"]);
    assert!(!valid.status.success());
    assert!(String::from_utf8_lossy(&valid.stderr).contains("requires an interactive terminal"));

    std::fs::write(
        &config_file,
        "GOSLING_PROVIDER: openai\n  GOSLING_MODEL: [gpt-4o\n",
    )
    .unwrap();
    let broken = env.gosling(env.root.path(), &["configure"]);
    let stderr = String::from_utf8_lossy(&broken.stderr);
    assert!(!broken.status.success());
    assert!(stderr.contains("could not be parsed"), "{stderr}");
    assert!(stderr.contains(config_file.to_str().unwrap()), "{stderr}");
    assert_eq!(env.mock.chat_requests.load(Ordering::SeqCst), 0);
}

/// GSL-PT-20260927-S18: a corrupt permission policy used to panic `run`; it
/// now runs with tools denied and names the unreadable file.
#[test]
fn corrupt_permission_policy_is_reported_instead_of_panicking() {
    let env = Env::new();
    let policy = env.root.path().join("config").join("permission.yaml");
    std::fs::write(&policy, "user: [unclosed\n  - : :\n").unwrap();

    let output = env.gosling(env.root.path(), &["run", "--no-session", "-t", "hi"]);

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stdout.contains("MOCK-REPLY"), "stdout: {stdout}");
    assert!(!stderr.contains("panicked"), "stderr: {stderr}");
    assert!(
        stderr.contains(&format!(
            "Permission policy {} could not be read",
            policy.display()
        )),
        "stderr: {stderr}"
    );
}

/// GSL-PT-20260927-C11 / E-N4: the non-interactive denial named the configured
/// default mode, not the mode stored on the session that actually asked.
#[test]
fn non_interactive_denial_names_the_sessions_own_mode() {
    let env = Env::new();
    let cwd = env.root.path();
    let created = env
        .command(cwd, &["run", "-n", "c11", "-t", "hi"])
        .env("GOSLING_MODE", "approve")
        .output()
        .unwrap();
    assert!(created.status.success());

    let denied = env
        .command(cwd, &["run", "-r", "-n", "c11", "-t", "C11-CALL-TREE"])
        .stdin(std::process::Stdio::null())
        .output()
        .unwrap();

    let stderr = String::from_utf8_lossy(&denied.stderr);
    assert!(!denied.status.success(), "stderr: {stderr}");
    assert!(
        stderr.contains("Tool approval required in non-interactive mode with GoslingMode::approve"),
        "stderr: {stderr}"
    );
}

/// GSL-PT-20260927-C11: imports start in approve mode with tools restricted to
/// their working directory; the command must say so.
#[test]
fn import_reports_the_imported_sessions_mode() {
    let env = Env::new();
    let cwd = env.root.path();
    let created = env.run_ok(cwd, &["run", "-n", "to-export", "-t", "hi"]);
    let exported = env.export(&banner_session_id(&created));
    let file = env.root.path().join("export.json");
    std::fs::write(&file, exported.to_string()).unwrap();

    let imported = env.run_ok(cwd, &["session", "import", file.to_str().unwrap()]);

    let stdout = String::from_utf8_lossy(&imported.stdout);
    assert!(
        stdout.contains("Mode: approve, tools restricted to its working directory"),
        "stdout: {stdout}"
    );
}
