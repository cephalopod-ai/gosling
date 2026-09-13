use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Output};
use tempfile::TempDir;

fn gosling(root: &TempDir, config: &str) -> Output {
    let config_dir = root.path().join("config");
    std::fs::create_dir_all(&config_dir).unwrap();
    std::fs::write(
        config_dir.join("config.yaml"),
        format!("GOSLING_PROVIDER: openai\nGOSLING_MODEL: gpt-4o\n{config}"),
    )
    .unwrap();

    let host = healthy_openai_server();

    Command::new(env!("CARGO_BIN_EXE_gosling"))
        .arg("doctor")
        .env("GOSLING_PATH_ROOT", root.path())
        .env("GOSLING_DISABLE_KEYRING", "1")
        .env("OPENAI_HOST", host)
        .env("OPENAI_API_KEY", "sk-test")
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
fn invalid_runtime_config_values_emit_actionable_warnings() {
    let cases = [
        ("GOSLING_MODE: yolo\n", "Invalid GOSLING_MODE"),
        ("GOSLING_MAX_TURNS: plenty\n", "Invalid GOSLING_MAX_TURNS"),
        (
            "GOSLING_AUTO_COMPACT_THRESHOLD: 5\n",
            "Invalid GOSLING_AUTO_COMPACT_THRESHOLD",
        ),
        (
            "GOSLING_AUTO_COMPACT_REDUCTION: 5\n",
            "Invalid GOSLING_AUTO_COMPACT_REDUCTION",
        ),
    ];

    for (config, expected_warning) in cases {
        let root = TempDir::new().unwrap();
        let output = gosling(&root, config);
        assert!(output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_warning),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn valid_runtime_config_values_do_not_warn() {
    let root = TempDir::new().unwrap();
    let output = gosling(
        &root,
        "GOSLING_MODE: auto\nGOSLING_MAX_TURNS: 5\nGOSLING_AUTO_COMPACT_THRESHOLD: 0.8\nGOSLING_AUTO_COMPACT_REDUCTION: 0.15\n",
    );

    assert!(output.status.success());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("Invalid GOSLING_"));
}
