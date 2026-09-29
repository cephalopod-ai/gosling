//! Every request except `initialize` must be refused on a stdio/WebSocket connection until
//! `initialize` has negotiated a supported protocol version (GSL-PT-20260927-F16).

#[allow(dead_code)]
#[path = "acp_common_tests/mod.rs"]
mod common_tests;

use agent_client_protocol::schema::v1::{
    ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest, TextContent,
};
use agent_client_protocol::schema::ProtocolVersion;
use agent_client_protocol::{Agent, Client, ConnectionTo, ErrorCode};
use common_tests::fixtures::{
    run_test, send_custom, spawn_acp_server_in_process, AcpTestServerSettings, OpenAiFixture,
};
use gosling::config::{CodeExecutionRuntime, GoslingMode};
use gosling_test_support::{IgnoreSessionId, TEST_MODEL};
use std::future::Future;
use std::sync::Arc;

fn with_raw_connection<F, Fut>(scenario: F)
where
    F: FnOnce(ConnectionTo<Agent>, std::path::PathBuf) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    run_test(async move {
        let openai = OpenAiFixture::new(vec![], Arc::new(IgnoreSessionId)).await;
        let data_root = tempfile::tempdir().unwrap();
        let cwd = tempfile::tempdir().unwrap();
        let (transport, _handle, _permissions) = spawn_acp_server_in_process(
            openai.uri(),
            &[],
            data_root.path(),
            GoslingMode::Auto,
            None,
            AcpTestServerSettings {
                current_model: TEST_MODEL,
                code_execution_runtime: CodeExecutionRuntime::Disabled,
                disable_session_naming: true,
            },
        )
        .await;
        let cwd_path = cwd.path().to_path_buf();
        Client
            .builder()
            .connect_with(transport, async move |cx: ConnectionTo<Agent>| {
                scenario(cx, cwd_path).await;
                Ok(())
            })
            .await
            .unwrap();
    });
}

async fn initialize(
    cx: &ConnectionTo<Agent>,
    version: ProtocolVersion,
) -> Result<(), agent_client_protocol::Error> {
    cx.send_request(InitializeRequest::new(version))
        .block_task()
        .await
        .map(|_| ())
}

async fn new_session(
    cx: &ConnectionTo<Agent>,
    cwd: &std::path::Path,
) -> Result<String, agent_client_protocol::Error> {
    cx.send_request(NewSessionRequest::new(cwd))
        .block_task()
        .await
        .map(|response| response.session_id.0.to_string())
}

fn assert_not_initialized(error: agent_client_protocol::Error, method: &str) {
    assert_eq!(error.code, ErrorCode::InvalidRequest, "{method}: {error:?}");
    let data = error.data.map(|data| data.to_string()).unwrap_or_default();
    assert!(
        data.contains("not initialized") && data.contains(method),
        "{method}: {data}"
    );
}

async fn assert_session_methods_refused(cx: &ConnectionTo<Agent>, cwd: &std::path::Path) {
    assert_not_initialized(new_session(cx, cwd).await.unwrap_err(), "session/new");
    let prompt = cx
        .send_request(PromptRequest::new(
            "unnegotiated",
            vec![ContentBlock::Text(TextContent::new("Say UNNEGOTIATED"))],
        ))
        .block_task()
        .await
        .unwrap_err();
    assert_not_initialized(prompt, "session/prompt");
    let custom = send_custom(
        cx,
        "_gosling/unstable/config/extensions/list",
        serde_json::json!({}),
    )
    .await
    .unwrap_err();
    assert_not_initialized(custom, "_gosling/unstable/config/extensions/list");
}

#[test]
fn session_methods_are_refused_without_initialize() {
    with_raw_connection(|cx, cwd| async move {
        assert_session_methods_refused(&cx, &cwd).await;
    });
}

#[test]
fn session_methods_are_refused_after_a_rejected_protocol_version() {
    with_raw_connection(|cx, cwd| async move {
        let unsupported: ProtocolVersion = serde_json::from_value(serde_json::json!(2)).unwrap();
        for version in [ProtocolVersion::V0, unsupported] {
            let error = initialize(&cx, version).await.unwrap_err();
            assert_eq!(error.code, ErrorCode::InvalidParams);
            assert_session_methods_refused(&cx, &cwd).await;
        }
    });
}

#[test]
fn session_new_succeeds_once_initialize_negotiates_the_latest_version() {
    with_raw_connection(|cx, cwd| async move {
        initialize(&cx, ProtocolVersion::V0).await.unwrap_err();
        initialize(&cx, ProtocolVersion::LATEST).await.unwrap();
        let session_id = new_session(&cx, &cwd).await.unwrap();
        assert!(!session_id.is_empty());
    });
}
