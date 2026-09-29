//! `gosling serve` stopped by SIGTERM answers the prompts still running and
//! closes their turns before it exits (GSL-PT-20260927-F15).
#![cfg(unix)]

use agent_client_protocol::schema::v1::{
    ClientCapabilities, ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest,
    SessionNotification, SessionUpdate, StopReason, TextContent,
};
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::{Agent, Client, ConnectionTo};
use agent_client_protocol_http::HttpClient;
use gosling::session::{AcpPromptRunState, ExtensionState, SessionManager};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const SECRET: &str = "serve-shutdown-test-secret";

struct ServeProcess(Child);

impl Drop for ServeProcess {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// Streams the first part of every reply and then never finishes it.
fn start_stalling_provider() -> (u16, mpsc::Receiver<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let (streaming_tx, streaming_rx) = mpsc::channel();
    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let streaming_tx = streaming_tx.clone();
            std::thread::spawn(move || serve(stream, streaming_tx));
        }
    });
    (port, streaming_rx)
}

fn serve(mut stream: TcpStream, streaming: mpsc::Sender<()>) {
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

    if request_line.contains("/chat/completions") && request["stream"] == serde_json::json!(true) {
        let chunk = serde_json::json!({
            "id": "r",
            "object": "chat.completion.chunk",
            "created": 0,
            "model": "gpt-4o",
            "choices": [{"index": 0, "delta": {"role": "assistant", "content": "PARTIAL-"}, "finish_reason": null}],
        });
        let _ = write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: {chunk}\n\n"
        );
        let _ = stream.flush();
        let _ = streaming.send(());
        std::thread::sleep(Duration::from_secs(120));
        return;
    }
    let payload = if request_line.contains("/chat/completions") {
        serde_json::json!({
            "id": "r",
            "object": "chat.completion",
            "created": 0,
            "model": "gpt-4o",
            "choices": [{"index": 0, "message": {"role": "assistant", "content": "Title"}, "finish_reason": "stop"}],
        })
    } else {
        serde_json::json!({"object": "list", "data": [{"id": "gpt-4o", "object": "model", "created": 0, "owned_by": "openai"}]})
    }
    .to_string();
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn wait_for_server(port: u16) {
    let deadline = Instant::now() + Duration::from_secs(30);
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

async fn connect(
    port: u16,
    updates: Arc<Mutex<Vec<SessionUpdate>>>,
) -> (ConnectionTo<Agent>, tokio::task::JoinHandle<()>) {
    let http = reqwest::Client::builder()
        .default_headers({
            let mut headers = reqwest::header::HeaderMap::new();
            headers.insert("X-Secret-Key", SECRET.parse().unwrap());
            headers
        })
        .build()
        .unwrap();
    let transport = HttpClient::with_client(format!("http://127.0.0.1:{port}"), http).unwrap();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(async move {
        let _ = Client
            .builder()
            .on_receive_notification(
                async move |notification: SessionNotification, _cx| {
                    updates.lock().unwrap().push(notification.update);
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_with(transport, move |cx: ConnectionTo<Agent>| async move {
                cx.send_request(
                    InitializeRequest::new(ProtocolVersion::LATEST)
                        .client_capabilities(ClientCapabilities::default()),
                )
                .block_task()
                .await?;
                let _ = ready_tx.send(cx.clone());
                std::future::pending::<Result<(), agent_client_protocol::Error>>().await
            })
            .await;
    });
    (ready_rx.await.unwrap(), task)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn sigterm_answers_a_running_prompt_and_closes_its_turn() {
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
    let (provider_port, streaming) = start_stalling_provider();
    let port = free_port();
    let mut serve = ServeProcess(
        Command::new(env!("CARGO_BIN_EXE_gosling"))
            .args(["serve", "--host", "127.0.0.1", "--port", &port.to_string()])
            .current_dir(&work)
            .env("GOSLING_PATH_ROOT", root.path())
            .env("GOSLING_DISABLE_KEYRING", "1")
            .env("GOSLING_SERVER__SECRET_KEY", SECRET)
            .env("OPENAI_HOST", format!("http://127.0.0.1:{provider_port}"))
            .env("OPENAI_API_KEY", "sk-test")
            .env_remove("GOSLING_MODE")
            .env_remove("GOSLING_PROVIDER")
            .env_remove("GOSLING_MODEL")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap(),
    );
    wait_for_server(port).await;

    let updates = Arc::new(Mutex::new(Vec::new()));
    let (cx, client) = connect(port, Arc::clone(&updates)).await;
    let session_id = cx
        .send_request(NewSessionRequest::new(&work))
        .block_task()
        .await
        .unwrap()
        .session_id;
    let prompt = tokio::spawn({
        let cx = cx.clone();
        let request = PromptRequest::new(
            session_id.clone(),
            vec![ContentBlock::Text(TextContent::new("Write a long answer"))],
        );
        async move { cx.send_request(request).block_task().await }
    });
    tokio::task::spawn_blocking(move || streaming.recv_timeout(Duration::from_secs(60)))
        .await
        .unwrap()
        .expect("the turn reached the provider");
    let deadline = Instant::now() + Duration::from_secs(10);
    while !updates
        .lock()
        .unwrap()
        .iter()
        .any(|update| matches!(update, SessionUpdate::AgentMessageChunk(_)))
    {
        assert!(
            Instant::now() < deadline,
            "the reply never started streaming"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    let status = Command::new("kill")
        .args(["-TERM", &serve.0.id().to_string()])
        .status()
        .unwrap();
    assert!(status.success());
    let response = tokio::time::timeout(Duration::from_secs(3), prompt)
        .await
        .expect("a running prompt must be answered when the server stops")
        .unwrap()
        .unwrap();
    assert_eq!(response.stop_reason, StopReason::Cancelled);
    client.abort();

    let deadline = Instant::now() + Duration::from_secs(15);
    let exit = loop {
        if let Some(exit) = serve.0.try_wait().unwrap() {
            break exit;
        }
        assert!(Instant::now() < deadline, "gosling serve did not exit");
        tokio::time::sleep(Duration::from_millis(50)).await;
    };
    assert!(exit.success(), "{exit:?}");

    let session = SessionManager::new(root.path().join("data"))
        .get_session(&session_id.0, true)
        .await
        .unwrap();
    let texts: Vec<String> = session
        .conversation
        .unwrap()
        .messages()
        .iter()
        .map(|message| message.as_concat_text())
        .collect();
    assert_eq!(
        texts,
        vec![
            "Write a long answer",
            "PARTIAL-",
            "Run interrupted before completion."
        ]
    );
    assert_eq!(
        AcpPromptRunState::from_extension_data(&session.extension_data),
        Some(AcpPromptRunState::Interrupted)
    );
}
