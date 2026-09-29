//! A `gosling run` whose turn lease another process takes over stops within
//! one lease heartbeat, without waiting for the provider, and reports the lost
//! lease once (GSL-PT-20260927-A08).

use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Answers every request except a streamed chat completion, which it accepts
/// and then never answers, like a provider stuck on a long call.
fn start_stalling_provider() -> (u16, mpsc::Receiver<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (stalled_tx, stalled_rx) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let stalled_tx = stalled_tx.clone();
            std::thread::spawn(move || serve(stream, stalled_tx));
        }
    });
    (port, stalled_rx)
}

fn serve(mut stream: TcpStream, stalled: mpsc::Sender<()>) {
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
    let request: serde_json::Value = serde_json::from_slice(&body).unwrap_or_default();

    let payload = if request_line.contains("/chat/completions") {
        if request["stream"] == serde_json::json!(true) {
            let _ = stalled.send(());
            std::thread::sleep(Duration::from_secs(120));
            return;
        }
        serde_json::json!({
            "id": "r",
            "object": "chat.completion",
            "created": 0,
            "model": "gpt-4o",
            "choices": [{"index": 0, "message": {"role": "assistant", "content": "Title"}, "finish_reason": "stop"}],
        })
    } else {
        serde_json::json!({"object": "list", "data": []})
    }
    .to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
}

/// What another process taking the session over does to this one's lease.
fn revoke_turn_leases(root: &std::path::Path) {
    let database = root.join("data").join("sessions").join("sessions.db");
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        use sqlx::ConnectOptions;
        let mut connection = sqlx::sqlite::SqliteConnectOptions::new()
            .filename(database)
            .connect()
            .await
            .unwrap();
        let revoked = sqlx::query("DELETE FROM session_turn_leases")
            .execute(&mut connection)
            .await
            .unwrap();
        assert_eq!(revoked.rows_affected(), 1, "the running turn held a lease");
    });
}

#[test]
fn a_run_whose_lease_is_taken_over_stops_and_says_so_once() {
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
    let (port, stalled) = start_stalling_provider();

    let mut child = Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(["run", "-t", "Write a long answer"])
        .current_dir(&work)
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .env("OPENAI_HOST", format!("http://127.0.0.1:{port}"))
        .env("OPENAI_API_KEY", "sk-test")
        .env_remove("GOSLING_MODE")
        .env_remove("GOSLING_PROVIDER")
        .env_remove("GOSLING_MODEL")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    stalled
        .recv_timeout(Duration::from_secs(60))
        .expect("the turn reached the provider");

    revoke_turn_leases(root.path());
    let revoked_at = Instant::now();
    let deadline = revoked_at + Duration::from_secs(30);
    while child.try_wait().unwrap().is_none() {
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("a run whose lease was taken over kept waiting for the provider");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    let stopped_after = revoked_at.elapsed();
    let output = child.wait_with_output().unwrap();
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(!output.status.success(), "{stderr}");
    assert!(
        stopped_after < Duration::from_secs(20),
        "stopped {stopped_after:?} after the takeover"
    );
    assert_eq!(
        stderr.matches("Session turn lease was lost").count(),
        1,
        "{stderr}"
    );
}
