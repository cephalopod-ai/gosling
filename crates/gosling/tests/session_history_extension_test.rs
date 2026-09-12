use gosling::agents::mcp_client::McpClientTrait;
use gosling::agents::platform_extensions::{
    session_history::SessionHistoryClient, PlatformExtensionContext,
};
use gosling::agents::ToolCallContext;
use gosling::config::{CodeExecutionRuntime, GoslingMode};
use gosling::conversation::message::Message;
use gosling::providers::base::ProviderCapabilities;
use gosling::session::handoff::SessionHandoffBuilder;
use gosling::session::{SessionManager, SessionType};
use gosling_providers::model::ModelConfig;
use gosling_sdk_types::session_handoff::SessionHandoffTriggerDto;
use rmcp::model::RawContent;
use rmcp::object;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;

fn first_text(result: &rmcp::model::CallToolResult) -> &str {
    match &result.content[0].raw {
        RawContent::Text(text) => &text.text,
        _ => panic!("expected text content"),
    }
}

#[tokio::test]
async fn handoff_target_can_search_and_read_redacted_source_history() {
    let temp = tempfile::tempdir().unwrap();
    let manager = Arc::new(SessionManager::new(temp.path().join("data")));
    let source = manager
        .create_session(
            temp.path().to_path_buf(),
            "source".to_string(),
            SessionType::User,
            GoslingMode::Auto,
        )
        .await
        .unwrap();
    manager
        .add_message(
            &source.id,
            &Message::user()
                .with_id("source-history-message")
                .with_text(format!(
                    "{} continuity-marker api_key=secret-value",
                    "older context ".repeat(80)
                )),
        )
        .await
        .unwrap();
    let snapshot = SessionHandoffBuilder::new(&manager)
        .build(
            &source.id,
            "target-provider",
            "target-model",
            128_000,
            ProviderCapabilities::gosling_managed(),
            SessionHandoffTriggerDto::SessionFork,
        )
        .await
        .unwrap();
    let (target, _) = manager
        .create_handoff_session(
            &source.id,
            "target".to_string(),
            "target-provider".to_string(),
            ModelConfig::new("target-model"),
            snapshot,
        )
        .await
        .unwrap();
    manager
        .add_message(
            &target.id,
            &Message::user()
                .with_id("target-history-message")
                .with_text("current-session-marker"),
        )
        .await
        .unwrap();
    manager
        .add_message(
            &target.id,
            &Message::user()
                .with_id("current-request-message")
                .with_text("recover earlier context"),
        )
        .await
        .unwrap();
    let client = SessionHistoryClient::new(PlatformExtensionContext {
        extension_manager: None,
        session_manager: manager,
        session: None,
        use_login_shell_path: false,
        code_execution_runtime: CodeExecutionRuntime::Disabled,
    });
    let ctx = ToolCallContext::new(target.id, Some(temp.path().to_path_buf()), None);

    let search = client
        .call_tool(
            &ctx,
            "session_search",
            Some(object!({"query": "continuity-marker"})),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(search.is_error, Some(false));
    assert!(first_text(&search).contains("source-history-message"));
    assert!(first_text(&search).contains("continuity-marker"));
    assert!(first_text(&search).contains("\"source_session\": true"));

    let current_search = client
        .call_tool(
            &ctx,
            "session_search",
            Some(object!({"query": "current-session-marker"})),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(current_search.is_error, Some(false));
    assert!(first_text(&current_search).contains("target-history-message"));
    assert!(first_text(&current_search).contains("\"source_session\": false"));

    let current_request_search = client
        .call_tool(
            &ctx,
            "session_search",
            Some(object!({"query": "recover earlier context"})),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert!(!first_text(&current_request_search).contains("current-request-message"));

    let read = client
        .call_tool(
            &ctx,
            "session_read",
            Some(object!({"message_id": "source-history-message"})),
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(read.is_error, Some(false));
    assert!(first_text(&read).contains("continuity-marker"));
    assert!(first_text(&read).contains("[REDACTED]"));
    assert!(!first_text(&read).contains("secret-value"));
}

#[tokio::test]
async fn session_history_tools_are_declared_read_only() {
    let temp = tempfile::tempdir().unwrap();
    let client = SessionHistoryClient::new(PlatformExtensionContext {
        extension_manager: None,
        session_manager: Arc::new(SessionManager::new(temp.path().join("data"))),
        session: None,
        use_login_shell_path: false,
        code_execution_runtime: CodeExecutionRuntime::Disabled,
    });

    let tools = client
        .list_tools("session", None, CancellationToken::new())
        .await
        .unwrap()
        .tools;
    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.name.as_ref())
            .collect::<Vec<_>>(),
        vec!["session_search", "session_read"]
    );
    assert!(tools
        .iter()
        .all(|tool| tool.annotations.as_ref().and_then(|a| a.read_only_hint) == Some(true)));
}
