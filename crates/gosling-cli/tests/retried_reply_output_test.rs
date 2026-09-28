//! A reply attempt that breaks off mid-stream and is retried is not part of
//! the history, and `gosling run` output says so: `-q` never prints it,
//! stream-json retracts it, text output marks it (GSL-PT-20260927-B07).

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

fn chunk(delta: Value, finish: Value) -> String {
    let chunk = json!({
        "id": "r",
        "object": "chat.completion.chunk",
        "created": 0,
        "model": "gpt-4o",
        "choices": [{"index": 0, "delta": delta, "finish_reason": finish}],
    });
    format!("data: {chunk}\n\n")
}

/// The turn's first reply sends `BROKEN-` and then drops the connection;
/// every later reply (and the session title request) is `COMPLETE`.
fn start_flaky_provider() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let streamed = Arc::new(AtomicUsize::new(0));
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let streamed = Arc::clone(&streamed);
            std::thread::spawn(move || serve(stream, &streamed));
        }
    });
    port
}

fn serve(mut stream: TcpStream, streamed: &AtomicUsize) {
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
    let request: Value = serde_json::from_slice(&body).unwrap_or_default();

    if request_line.contains("/chat/completions") && request["stream"] == json!(true) {
        let turn_request = request["tools"]
            .as_array()
            .is_some_and(|tools| !tools.is_empty());
        if turn_request && streamed.fetch_add(1, Ordering::SeqCst) == 0 {
            let _ =
                write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n{}",
                chunk(json!({"role": "assistant", "content": "BROKEN-"}), Value::Null)
            );
            let _ = stream.flush();
            std::thread::sleep(Duration::from_millis(300));
            return;
        }
        let events = chunk(
            json!({"role": "assistant", "content": "COMPLETE"}),
            Value::Null,
        ) + &chunk(json!({}), json!("stop"))
            + "data: [DONE]\n\n";
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{events}",
            events.len()
        );
        return;
    }
    let payload = if request_line.contains("/chat/completions") {
        json!({
            "id": "r",
            "object": "chat.completion",
            "created": 0,
            "model": "gpt-4o",
            "choices": [{"index": 0, "message": {"role": "assistant", "content": "Title"}, "finish_reason": "stop"}],
        })
    } else {
        json!({"object": "list", "data": []})
    }
    .to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
}

fn run(args: &[&str]) -> Output {
    let root = TempDir::new().unwrap();
    let config_dir = root.path().join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.yaml"),
        "GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\nGOSLING_MODE: auto\n",
    )
    .unwrap();
    let work = root.path().join("work");
    std::fs::create_dir_all(&work).unwrap();
    let port = start_flaky_provider();
    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(["run", "-t", "Tell me something"])
        .args(args)
        .current_dir(&work)
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .env("OPENAI_HOST", format!("http://127.0.0.1:{port}"))
        .env("OPENAI_API_KEY", "sk-test")
        .env_remove("GOSLING_MODE")
        .env_remove("GOSLING_PROVIDER")
        .env_remove("GOSLING_MODEL")
        .output()
        .unwrap()
}

fn stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn quiet_output_is_only_the_reply_that_was_kept() {
    assert_eq!(stdout(&run(&["-q"])), "COMPLETE\n");
}

#[test]
fn stream_json_retracts_the_attempt_that_was_retried() {
    let events: Vec<Value> = stdout(&run(&["--output-format", "stream-json"]))
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let shown = |text: &str| {
        events
            .iter()
            .position(|event| {
                event["type"] == "message" && event["message"]["content"][0]["text"] == text
            })
            .unwrap_or_else(|| panic!("{text} was not emitted: {events:?}"))
    };
    let broken = shown("BROKEN-");
    let complete = shown("COMPLETE");
    let retracted = events
        .iter()
        .position(|event| event["type"] == "messages_retracted")
        .expect("the retried attempt is retracted");

    assert!(broken < retracted && retracted < complete, "{events:?}");
    assert_eq!(
        events[retracted]["message_ids"],
        json!([events[broken]["message"]["id"]])
    );
    assert_eq!(events.last().unwrap()["type"], "complete");
}

#[test]
fn text_output_marks_the_discarded_attempt() {
    let out = stdout(&run(&[]));
    let broken = out.find("BROKEN- (discarded)").expect(&out);
    let complete = out.find("COMPLETE").expect(&out);
    assert!(broken < complete, "{out}");
}
