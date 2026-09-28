//! What the CLI prints, and where: tool cards and their outcomes, `run` output
//! modes, error streams and exit codes, observed through the real binary and a
//! scripted OpenAI-compatible provider.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Output};
use tempfile::TempDir;

/// Replies to the newest message of each chat request. A user prompt is read
/// for these lines:
/// - `STATUS <code>`: answer with that HTTP status;
/// - `CALL <tool> <json-arguments>`: call that tool;
/// - `SAY <text>`: reply with that text (alongside a `CALL`, before the call).
///
/// A prompt without either gets `REPLY`, and a tool result gets `AFTER-TOOL`.
/// Replies never end with a newline, so glued output stays visible.
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

    let (status, content_type, payload) = if request_line.contains("/chat/completions") {
        let request: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
        let prompt = prompt(&request);
        if let Some(code) = prompt.as_deref().and_then(|p| directive(p, "STATUS")) {
            let error = json!({"error": {"message": "scripted failure"}});
            (
                format!("{code} Scripted"),
                "application/json",
                error.to_string(),
            )
        } else if request["stream"] == json!(true) {
            let delta = reply_delta(prompt.as_deref());
            let chunk = |delta: Value, finish: Value| {
                let chunk = json!({
                    "id": "r",
                    "object": "chat.completion.chunk",
                    "created": 0,
                    "model": "gpt-4o",
                    "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
                });
                format!("data: {chunk}\n\n")
            };
            let mut events = String::new();
            if let Some(content) = delta.get("content") {
                events += &chunk(
                    json!({"role": "assistant", "content": content}),
                    Value::Null,
                );
            }
            let finish = match delta.get("tool_calls") {
                Some(tool_calls) => {
                    events += &chunk(
                        json!({"role": "assistant", "tool_calls": tool_calls}),
                        Value::Null,
                    );
                    "tool_calls"
                }
                None => "stop",
            };
            events += &chunk(json!({}), json!(finish));
            (
                "200 OK".to_string(),
                "text/event-stream",
                events + "data: [DONE]\n\n",
            )
        } else {
            let completion = json!({
                "id": "r",
                "object": "chat.completion",
                "created": 0,
                "model": "gpt-4o",
                "choices": [{"index": 0, "message": {"role": "assistant", "content": "Title"}, "finish_reason": "stop"}],
            });
            (
                "200 OK".to_string(),
                "application/json",
                completion.to_string(),
            )
        }
    } else {
        (
            "200 OK".to_string(),
            "application/json",
            json!({"object": "list", "data": []}).to_string(),
        )
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
}

/// The newest message's text when it is the user's prompt; `None` after a tool result.
fn prompt(request: &Value) -> Option<String> {
    let last = request["messages"].as_array()?.last()?;
    if last["role"] != "user" {
        return None;
    }
    Some(match &last["content"] {
        Value::String(text) => text.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part["text"].as_str())
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    })
}

fn directive<'a>(prompt: &'a str, name: &str) -> Option<&'a str> {
    prompt
        .lines()
        .find_map(|line| line.trim().strip_prefix(name)?.strip_prefix(' '))
}

fn reply_delta(prompt: Option<&str>) -> Value {
    let Some(prompt) = prompt else {
        return json!({"role": "assistant", "content": "AFTER-TOOL"});
    };
    let mut delta = json!({"role": "assistant"});
    if let Some(text) = directive(prompt, "SAY") {
        delta["content"] = json!(text);
    }
    match directive(prompt, "CALL").and_then(|call| call.split_once(' ')) {
        Some((name, arguments)) => {
            delta["tool_calls"] = json!([{
                "index": 0,
                "id": "call_1",
                "type": "function",
                "function": {"name": name, "arguments": arguments},
            }]);
        }
        None if delta.get("content").is_none() => delta["content"] = json!("REPLY"),
        None => {}
    }
    delta
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

/// GSL-PT-20260927-A17: `run -q` promises only the model's reply on stdout but
/// printed tool cards and raw tool output, glued to the reply.
#[test]
fn quiet_mode_prints_only_the_reply_text() {
    let env = Env::new();

    let output = env.gosling(&[
        "run",
        "-q",
        "-t",
        "SAY Checking first.\nCALL shell {\"command\": \"echo tool-output-marker\"}",
    ]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert_eq!(stdout(&output), "Checking first.\nAFTER-TOOL\n");
}

/// GSL-PT-20260927-A17: a provider failure under `run -q` printed the error
/// text on stdout as if it were the answer.
#[test]
fn quiet_mode_reports_a_failed_run_on_stderr_only() {
    let env = Env::new();

    let output = env.gosling(&["run", "-q", "-t", "STATUS 401"]);

    assert!(!output.status.success());
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains("Authentication"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn text_mode_still_shows_tool_cards_and_output() {
    let env = Env::new();

    let output = env.gosling(&[
        "run",
        "-t",
        "CALL shell {\"command\": \"echo tool-output-marker\"}",
    ]);

    let stdout = stdout(&output);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(stdout.contains("▸ shell"), "{stdout}");
    assert!(stdout.contains("tool-output-marker\n"), "{stdout}");
    assert!(stdout.contains("AFTER-TOOL"), "{stdout}");
}

/// GSL-PT-20260927-A16 / F02: a run that failed before its first turn printed a
/// human error on stdout, even for `--output-format json|stream-json`, and
/// nothing on stderr.
#[test]
fn a_run_that_cannot_start_reports_on_stderr_and_leaves_stdout_empty() {
    let env = Env::new();

    for args in [
        vec!["--output-format", "text"],
        vec!["--output-format", "json"],
        vec!["--output-format", "stream-json"],
        vec!["-q"],
    ] {
        let mut run = vec!["run", "--provider", "bogus-prov", "-t", "hi"];
        run.extend(args.iter().copied());
        let output = env.gosling(&run);

        assert!(!output.status.success(), "{args:?}");
        assert_eq!(stdout(&output), "", "{args:?}");
        assert!(
            stderr(&output).contains("error: Unknown provider: bogus-prov."),
            "{args:?}: {}",
            stderr(&output)
        );
    }
}

/// GSL-PT-20260927-E14: resuming a session that does not exist printed the
/// error on stdout.
#[test]
fn resuming_a_session_that_does_not_exist_fails_on_stderr() {
    let env = Env::new();

    let output = env.gosling(&[
        "run",
        "--output-format",
        "json",
        "-r",
        "--session-id",
        "20990101_99",
        "-t",
        "x",
    ]);

    assert!(!output.status.success());
    assert_eq!(stdout(&output), "");
    assert!(
        stderr(&output).contains("Cannot resume session 20990101_99 - no such session exists"),
        "{}",
        stderr(&output)
    );
}

/// GSL-PT-20260927-A22 / D11: without a selector and without a terminal for
/// the picker, these commands printed "Error: not connected" and exited 0, so
/// `gosling session export > backup.md` wrote an empty backup.
#[test]
fn session_commands_without_a_selector_or_terminal_fail_with_a_hint() {
    let env = Env::new();
    let run = env.gosling(&["run", "-t", "hi"]);
    assert!(run.status.success(), "stderr: {}", stderr(&run));

    for command in [
        vec!["session", "export"],
        vec!["session", "diagnostics"],
        vec!["session", "context-history", "list"],
        vec!["session", "remove"],
    ] {
        let output = env.gosling(&command);

        assert!(!output.status.success(), "{command:?}");
        assert_eq!(stdout(&output), "", "{command:?}");
        let stderr = stderr(&output);
        assert!(
            stderr.contains("no interactive terminal for the session picker"),
            "{command:?}: {stderr}"
        );
        assert!(
            stderr.contains("--session-id <ID>"),
            "{command:?}: {stderr}"
        );
    }
}

fn stream_events(output: &Output) -> Vec<Value> {
    stdout(output)
        .lines()
        .map(|line| serde_json::from_str(line).unwrap_or_else(|e| panic!("not JSON ({e}): {line}")))
        .collect()
}

/// GSL-PT-20260927-B15: a stream-json run that the provider failed ended with
/// an ordinary `message` event; only a success had a terminal event.
#[test]
fn a_failed_stream_json_run_ends_with_an_error_event() {
    let env = Env::new();

    let output = env.gosling(&["run", "--output-format", "stream-json", "-t", "STATUS 401"]);

    assert!(!output.status.success());
    let events = stream_events(&output);
    let last = events.last().expect("at least one event");
    assert_eq!(last["type"], "error", "{events:?}");
    assert!(
        last["error"].as_str().unwrap().contains("Authentication"),
        "{last}"
    );
    assert!(!events.iter().any(|event| event["type"] == "complete"));
}

#[test]
fn a_successful_stream_json_run_still_ends_with_complete() {
    let env = Env::new();

    let output = env.gosling(&["run", "--output-format", "stream-json", "-t", "hi"]);

    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let events = stream_events(&output);
    assert_eq!(events.last().unwrap()["type"], "complete", "{events:?}");
    assert!(!events.iter().any(|event| event["type"] == "error"));
}
