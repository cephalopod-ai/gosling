use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
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

    let (content_type, payload) = if request_line.contains("/chat/completions") {
        chat_requests.fetch_add(1, Ordering::SeqCst);
        if body.contains("\"stream\":true") {
            let chunk = |delta: &str, finish: &str| {
                format!(
                    "data: {{\"id\":\"r\",\"object\":\"chat.completion.chunk\",\"created\":0,\"model\":\"gpt-4o\",\"choices\":[{{\"index\":0,\"delta\":{delta},\"finish_reason\":{finish}}}]}}\n\n"
                )
            };
            (
                "text/event-stream",
                format!(
                    "{}{}data: [DONE]\n\n",
                    chunk(
                        "{\"role\":\"assistant\",\"content\":\"MOCK-REPLY\"}",
                        "null"
                    ),
                    chunk("{}", "\"stop\"")
                ),
            )
        } else {
            (
                "application/json",
                "{\"id\":\"r\",\"object\":\"chat.completion\",\"created\":0,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,\"message\":{\"role\":\"assistant\",\"content\":\"MOCK-REPLY\"},\"finish_reason\":\"stop\"}]}".to_string(),
            )
        }
    } else {
        (
            "application/json",
            "{\"object\":\"list\",\"data\":[]}".to_string(),
        )
    };
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
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
    assert!(String::from_utf8_lossy(&moved.stderr).contains("Staying in current directory"));
    assert_eq!(env.export(&id)["working_dir"], dir_b.to_str().unwrap());
}

#[test]
fn no_session_run_leaves_nothing_resumable() {
    let env = Env::new();
    let cwd = env.root.path();

    let ephemeral = env.run_ok(cwd, &["run", "--no-session", "-t", "secret-ish prompt"]);
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

    let kept = env.run_ok(cwd, &["run", "-n", "kept", "-t", "hi"]);
    assert_eq!(
        env.export(&banner_session_id(&kept))["name"],
        serde_json::Value::String("kept".to_string())
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
