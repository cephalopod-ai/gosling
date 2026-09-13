use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Output};
use tempfile::TempDir;

fn doctor(root: &TempDir, config: Option<&str>, envs: &[(&str, &str)]) -> Output {
    if let Some(config) = config {
        let config_dir = root.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(config_dir.join("config.yaml"), config).unwrap();
    }

    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .arg("doctor")
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .env_remove("GOSLING_PROVIDER")
        .env_remove("GOSLING_MODEL")
        .envs(envs.iter().copied())
        .output()
        .expect("failed to run gosling binary")
}

fn healthy_openai_server() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        if let Ok((stream, _)) = listener.accept() {
            serve_openai_response(stream);
        }
    });
    format!("http://{address}")
}

fn serve_openai_response(mut stream: TcpStream) {
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
    let mut body = vec![0; content_length];
    let _ = reader.read_exact(&mut body);
    let payload = concat!(
        "data: {\"id\":\"doctor\",\"object\":\"chat.completion.chunk\",",
        "\"created\":0,\"model\":\"gpt-4o\",\"choices\":[{\"index\":0,",
        "\"delta\":{\"role\":\"assistant\",\"content\":\"ok\"},",
        "\"finish_reason\":null}]}\n\n",
        "data: [DONE]\n\n"
    );
    let _ = write!(
        stream,
        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
        payload.len()
    );
}

#[test]
fn doctor_fails_without_configuration() {
    let root = TempDir::new().unwrap();
    let output = doctor(&root, None, &[]);

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("no provider configured"));
}

#[test]
fn doctor_fails_for_unknown_provider() {
    let root = TempDir::new().unwrap();
    let output = doctor(
        &root,
        None,
        &[
            ("GOSLING_PROVIDER", "nonsense-prov"),
            ("GOSLING_MODEL", "m1"),
        ],
    );

    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("Unknown provider: nonsense-prov"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn doctor_fails_when_the_configured_provider_is_unreachable() {
    let root = TempDir::new().unwrap();
    let output = doctor(
        &root,
        Some("GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\n"),
        &[
            ("OPENAI_HOST", "http://127.0.0.1:1"),
            ("OPENAI_API_KEY", "sk-test"),
        ],
    );

    assert!(
        !output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stderr).contains("provider check failed"));
}

#[test]
fn doctor_verifies_a_healthy_configured_provider() {
    let root = TempDir::new().unwrap();
    let host = healthy_openai_server();
    let output = doctor(
        &root,
        Some("GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\n"),
        &[("OPENAI_HOST", &host), ("OPENAI_API_KEY", "sk-test")],
    );

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("provider request verified"));
}
