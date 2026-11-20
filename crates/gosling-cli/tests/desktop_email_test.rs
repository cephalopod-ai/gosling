use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use tempfile::TempDir;

#[test]
fn desktop_worker_reports_an_unconfigured_mailbox_without_creating_mail_state() {
    let root = TempDir::new().unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_gosling"))
        .args(["desktop-email", "--supervised"])
        .env("GOSLING_PATH_ROOT", root.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = child.stdin.take().unwrap();
    input.write_all(b"{\"op\":\"poll\"}\n").unwrap();
    let mut line = String::new();
    let mut output = BufReader::new(child.stdout.take().unwrap());
    output.read_line(&mut line).unwrap();
    let response: serde_json::Value = serde_json::from_str(&line).unwrap();
    assert!(response["data"]["account"].is_null());
    assert_eq!(response["data"]["revision"], 0);
    assert!(child.wait().unwrap().success());
    drop(input);
    assert!(!root.path().join("data").join("email").exists());
}
