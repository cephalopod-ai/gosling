//! The provider HTTP client must bound *stalls*, not turns.
//!
//! The agent runs tool calls inside the loop that consumes the assistant's SSE
//! stream, so the response body stays open for as long as the tools run. A
//! total request deadline therefore killed any turn whose streaming plus tool
//! execution outlived it, surfacing as `Stream decode error: error decoding
//! response body` and discarding the turn's work.

use gosling_providers::api_client::inference_client_builder;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const READ_TIMEOUT: Duration = Duration::from_millis(300);
const GAP: Duration = Duration::from_millis(60);
const BODY_LEN: usize = 10;

/// Serves one response whose body is written a byte at a time with `GAP`
/// between writes, stalling forever after `stall_after` bytes. The total body
/// duration deliberately exceeds `READ_TIMEOUT` while every individual gap
/// stays under it.
async fn serve_trickled_body(stall_after: Option<usize>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/", listener.local_addr().unwrap());

    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();

        let mut request = [0u8; 1024];
        let _request_len = socket.read(&mut request).await.unwrap();

        socket
            .write_all(format!("HTTP/1.1 200 OK\r\nContent-Length: {BODY_LEN}\r\n\r\n").as_bytes())
            .await
            .unwrap();
        socket.flush().await.unwrap();

        let writable = stall_after.unwrap_or(BODY_LEN);
        for _ in 0..writable {
            tokio::time::sleep(GAP).await;
            socket.write_all(b"x").await.unwrap();
            socket.flush().await.unwrap();
        }

        if stall_after.is_some() {
            // Hold the connection open with nothing more to read.
            std::future::pending::<()>().await;
        }
    });

    url
}

#[tokio::test]
async fn a_body_slower_than_the_timeout_still_completes() {
    let url = serve_trickled_body(None).await;

    let client = inference_client_builder(READ_TIMEOUT).build().unwrap();
    let body = client.get(&url).send().await.unwrap().text().await.unwrap();

    assert_eq!(body.len(), BODY_LEN);
}

#[tokio::test]
async fn a_stalled_body_is_still_cut_off() {
    let url = serve_trickled_body(Some(1)).await;

    let client = inference_client_builder(READ_TIMEOUT).build().unwrap();
    let error = client
        .get(&url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .expect_err("a connection that stops delivering bytes must not hang");

    assert!(error.is_timeout(), "expected a timeout, got {error:?}");
}

// The shape this replaced, kept as the regression's counter-example: reqwest's
// `timeout` is a total deadline "from when the request starts connecting until
// the response body has finished", so the same slow-but-alive body fails.
#[tokio::test]
async fn a_total_deadline_would_have_failed_the_same_body() {
    let url = serve_trickled_body(None).await;

    let client = reqwest::Client::builder()
        .timeout(READ_TIMEOUT)
        .build()
        .unwrap();
    let error = client
        .get(&url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .expect_err("a total deadline caps the whole body read");

    assert!(error.is_timeout());
    assert!(error.to_string().contains("error decoding response body"));
}
