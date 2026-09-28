//! What `gosling run` prints, and where: tool cards and their outcomes, output
//! modes, and exit codes, observed through the real binary and a scripted
//! OpenAI-compatible provider.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Replies to the newest message of each chat request:
/// - a user prompt with a line `CALL <tool> <json-arguments>` gets that tool call;
/// - a tool result gets the text `AFTER-TOOL` (no trailing newline, so glued
///   output is visible);
/// - any other prompt gets `REPLY`.
fn start_provider() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            std::thread::spawn(move || serve(stream));
        }
    });
    port
}

fn serve(mut stream: TcpStream) {
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

    let (content_type, payload) = if request_line.contains("/chat/completions") {
        let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let delta = reply_delta(&request);
        if request["stream"] == json!(true) {
            let chunk = |delta: &Value, finish: Value| {
                let chunk = json!({
                    "id": "r",
                    "object": "chat.completion.chunk",
                    "created": 0,
                    "model": "gpt-4o",
                    "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
                });
                format!("data: {chunk}\n\n")
            };
            let finish = if delta.get("tool_calls").is_some() {
                "tool_calls"
            } else {
                "stop"
            };
            (
                "text/event-stream",
                format!(
                    "{}{}data: [DONE]\n\n",
                    chunk(&delta, Value::Null),
                    chunk(&json!({}), json!(finish))
                ),
            )
        } else {
            let completion = json!({
                "id": "r",
                "object": "chat.completion",
                "created": 0,
                "model": "gpt-4o",
                "choices": [{"index": 0, "message": {"role": "assistant", "content": "Title"}, "finish_reason": "stop"}],
            });
            ("application/json", completion.to_string())
        }
    } else {
        (
            "application/json",
            json!({"object": "list", "data": []}).to_string(),
        )
    };
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
}

fn reply_delta(request: &Value) -> Value {
    let last = request["messages"]
        .as_array()
        .and_then(|messages| messages.last())
        .cloned()
        .unwrap_or(Value::Null);
    if last["role"] == "tool" {
        return json!({"role": "assistant", "content": "AFTER-TOOL"});
    }
    let text = match &last["content"] {
        Value::String(text) => text.clone(),
        other => other.to_string(),
    };
    let call = text
        .lines()
        .find_map(|line| line.trim().strip_prefix("CALL "))
        .and_then(|call| call.split_once(' '));
    match call {
        Some((name, arguments)) => json!({
            "role": "assistant",
            "tool_calls": [{
                "index": 0,
                "id": "call_1",
                "type": "function",
                "function": {"name": name, "arguments": arguments},
            }],
        }),
        None => json!({"role": "assistant", "content": "REPLY"}),
    }
}

struct Env {
    root: TempDir,
    port: u16,
}

impl Env {
    fn new() -> Self {
        let root = TempDir::new().unwrap();
        let config_dir = root.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("config.yaml"),
            "GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\nGOSLING_MODE: auto\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.path().join("work")).unwrap();
        Self {
            root,
            port: start_provider(),
        }
    }

    fn deny_tools(&self, tools: &[&str]) {
        let never_allow: Vec<String> = tools.iter().map(|tool| format!("  - {tool}\n")).collect();
        std::fs::write(
            self.root.path().join("config").join("permission.yaml"),
            format!(
                "user:\n  always_allow: []\n  ask_before: []\n  never_allow:\n{}",
                never_allow.concat()
            ),
        )
        .unwrap();
    }

    fn gosling(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_gosling"))
            .args(args)
            .current_dir(self.root.path().join("work"))
            .env("GOSLING_PATH_ROOT", self.root.path())
            .env("GOSLING_DISABLE_KEYRING", "1")
            .env("OPENAI_HOST", format!("http://127.0.0.1:{}", self.port))
            .env("OPENAI_API_KEY", "sk-test")
            .env_remove("GOSLING_MODE")
            .env_remove("GOSLING_PROVIDER")
            .env_remove("GOSLING_MODEL")
            .output()
            .unwrap()
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// GSL-PT-20260927-E05: a call the permission policy refused printed its
/// tool card and nothing else, as if it had run.
#[test]
fn a_denied_tool_call_is_shown_as_denied_with_its_reason() {
    let env = Env::new();
    env.deny_tools(&["write"]);

    let output = env.gosling(&[
        "run",
        "-t",
        r#"CALL write {"path": "denied.txt", "content": "x"}"#,
    ]);

    let stdout = stdout(&output);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(
        stdout.contains("✗ denied by policy: User permission denies this tool"),
        "{stdout}"
    );
    assert!(!env.root.path().join("work").join("denied.txt").exists());
}

/// GSL-PT-20260927-H03: a tool that reported an error looked the same as one
/// that succeeded.
#[test]
fn a_failed_tool_call_is_marked_failed_and_a_successful_one_is_not() {
    let env = Env::new();

    let failed = env.gosling(&[
        "run",
        "-t",
        r#"CALL shell {"command": "echo partial-output; exit 3"}"#,
    ]);
    let failed_stdout = stdout(&failed);
    assert!(failed.status.success(), "stderr: {}", stderr(&failed));
    assert!(failed_stdout.contains("partial-output"), "{failed_stdout}");
    assert!(
        failed_stdout.contains("Command exited with code 3\n    ✗ failed\n"),
        "{failed_stdout}"
    );

    let succeeded = env.gosling(&["run", "-t", r#"CALL shell {"command": "echo fine-output"}"#]);
    let succeeded_stdout = stdout(&succeeded);
    assert!(succeeded.status.success(), "stderr: {}", stderr(&succeeded));
    assert!(
        succeeded_stdout.contains("fine-output\n"),
        "{succeeded_stdout}"
    );
    assert!(!succeeded_stdout.contains('✗'), "{succeeded_stdout}");
}
