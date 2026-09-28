use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use gosling::agents::{Agent, AgentConfig, AgentEvent, GoslingPlatform, SessionConfig};
use gosling::config::permission::PermissionManager;
use gosling::config::GoslingMode;
use gosling::conversation::message::{Message, MessageContent};
use gosling::providers::base::{
    stream_from_single_message, MessageStream, Provider, ProviderDef, ProviderMetadata,
};
use gosling::session::session_manager::SessionType;
use gosling::session::SessionManager;
use gosling_providers::conversation::token_usage::{ProviderUsage, Usage};
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use rmcp::model::{CallToolRequestParams, Tool};
use rmcp::object;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tempfile::TempDir;

const SENTINEL: &str = "SENTINEL_SUBDIR_HINT_CONTENT";

/// Drives two tool calls that both touch `sub/`, then finishes with text. The
/// tool name is irrelevant: `record_tool_arguments` runs at the top of
/// `dispatch_tool_call`, before tool resolution, so an unresolved tool still
/// records the directory (and returns an error result quickly). Records how
/// many of its incoming requests already carried the injected hint. Tool call
/// ids are unique per provider, as a real model's are: a repeated id in the
/// same session replays the stored tool result instead of dispatching.
struct ToolCallingProvider {
    id: usize,
    call_count: AtomicUsize,
    requests_with_hint: AtomicUsize,
}

static NEXT_PROVIDER_ID: AtomicUsize = AtomicUsize::new(0);

impl ToolCallingProvider {
    fn new() -> Self {
        Self {
            id: NEXT_PROVIDER_ID.fetch_add(1, Ordering::SeqCst),
            call_count: AtomicUsize::new(0),
            requests_with_hint: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Provider for ToolCallingProvider {
    async fn stream(
        &self,
        _model_config: &ModelConfig,
        _system_prompt: &str,
        messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        let request_has_hint = messages.iter().any(|m| {
            m.content
                .iter()
                .any(|c| matches!(c, MessageContent::Text(t) if t.text.contains(SENTINEL)))
        });
        if request_has_hint {
            self.requests_with_hint.fetch_add(1, Ordering::SeqCst);
        }

        let call = self.call_count.fetch_add(1, Ordering::SeqCst);
        let usage = ProviderUsage::new(
            "mock-model".to_string(),
            Usage::new(Some(10), Some(5), Some(15)),
        );

        // Calls 0 and 1 each touch `sub/` via a tool; call 2 ends with text.
        let message = if call < 2 {
            let path = if call == 0 { "sub/a.txt" } else { "sub/b.txt" };
            Message::assistant().with_tool_request(
                format!("call_{}_{call}", self.id),
                Ok(CallToolRequestParams::new("inspect").with_arguments(object!({ "path": path }))),
            )
        } else {
            Message::assistant().with_text("All done.")
        };

        Ok(stream_from_single_message(message, usage))
    }

    fn get_name(&self) -> &str {
        "mock-tool-calling"
    }
}

impl gosling::providers::base::ProviderDescriptor for ToolCallingProvider {
    fn metadata() -> ProviderMetadata {
        ProviderMetadata {
            name: "mock".to_string(),
            display_name: "Mock Tool Calling Provider".to_string(),
            description: "Mock provider for subdirectory hint testing".to_string(),
            default_model: "mock-model".to_string(),
            known_models: vec![],
            model_doc_link: "".to_string(),
            config_keys: vec![],
            setup_steps: vec![],
            model_selection_hint: None,
            fast_model: None,
        }
    }
}

impl ProviderDef for ToolCallingProvider {
    type Provider = Self;

    fn from_env(
        _extensions: Vec<gosling::config::ExtensionConfig>,
        _tls_config: Option<gosling::providers::api_client::TlsConfig>,
    ) -> futures::future::BoxFuture<'static, anyhow::Result<Self>> {
        Box::pin(async { Ok(Self::new()) })
    }
}

/// When tool calls touch a subdirectory containing `.goslinghints`, the agent
/// injects those hints as an agent-only message in the live conversation
/// (reaching the in-flight turn, not just the session store) exactly once even
/// across repeated calls to the same directory.
#[tokio::test]
async fn subdirectory_hints_injected_once_agent_only() -> Result<()> {
    let workdir = TempDir::new()?;
    let sub = workdir.path().join("sub");
    std::fs::create_dir_all(&sub)?;
    std::fs::write(sub.join(".goslinghints"), SENTINEL)?;

    let data_dir = TempDir::new()?;
    let session_manager = Arc::new(SessionManager::new(data_dir.path().to_path_buf()));
    let config = AgentConfig::new(
        session_manager.clone(),
        PermissionManager::instance(),
        GoslingMode::Auto,
        true, // disable session naming so it doesn't consume a provider call
        GoslingPlatform::GoslingCli,
    );
    let agent = Agent::with_config(config);

    let session = session_manager
        .create_session(
            workdir.path().to_path_buf(),
            "subdir-hints-test".to_string(),
            SessionType::Hidden,
            GoslingMode::Auto,
        )
        .await?;

    let provider = Arc::new(ToolCallingProvider::new());
    agent
        .update_provider(
            provider.clone(),
            ModelConfig::new("mock-model"),
            &session.id,
        )
        .await?;

    let session_config = SessionConfig {
        id: session.id.clone(),
        max_turns: Some(5),
        compacted_context: false,
        tail_limit: None,
    };

    let reply_stream = agent
        .reply(
            Message::user().with_text("Look at the files under sub/"),
            session_config,
            None,
        )
        .await?;
    tokio::pin!(reply_stream);
    while let Some(event) = reply_stream.next().await {
        match event {
            Ok(AgentEvent::Message(_)) | Ok(_) => {}
            Err(e) => return Err(e),
        }
    }

    let conversation = session_manager
        .get_session(&session.id, true)
        .await?
        .conversation
        .expect("session has a conversation");

    let hint_messages: Vec<&Message> = conversation
        .messages()
        .iter()
        .filter(|m| {
            m.content
                .iter()
                .any(|c| matches!(c, MessageContent::Text(t) if t.text.contains(SENTINEL)))
        })
        .collect();

    assert_eq!(
        hint_messages.len(),
        1,
        "subdirectory hint should be injected exactly once across both tool calls to sub/, got {}",
        hint_messages.len()
    );

    let hint = hint_messages[0];
    assert!(hint.is_agent_visible(), "hint must be visible to the agent");
    assert!(
        !hint.is_user_visible(),
        "hint must not be dumped into the user-visible transcript"
    );
    let hint_text = hint.as_concat_text();
    assert!(
        hint_text.starts_with("### Subdirectory Project Hints (untrusted:")
            && hint_text.contains("not from the operator"),
        "repo-authored subdirectory hints must carry the untrusted project-hints framing: {hint_text}"
    );

    assert!(
        provider.requests_with_hint.load(Ordering::SeqCst) >= 1,
        "the injected hint must reach the live conversation (a later provider call must see it), \
         not just be written to the session store"
    );

    Ok(())
}

async fn reply_with_new_agent(
    session_manager: &Arc<SessionManager>,
    session_id: &str,
) -> Result<Arc<ToolCallingProvider>> {
    let agent = Agent::with_config(AgentConfig::new(
        session_manager.clone(),
        PermissionManager::instance(),
        GoslingMode::Auto,
        true,
        GoslingPlatform::GoslingCli,
    ));
    let provider = Arc::new(ToolCallingProvider::new());
    agent
        .update_provider(provider.clone(), ModelConfig::new("mock-model"), session_id)
        .await?;
    let reply_stream = agent
        .reply(
            Message::user().with_text("Look at the files under sub/"),
            SessionConfig {
                id: session_id.to_string(),
                max_turns: Some(5),
                compacted_context: false,
                tail_limit: None,
            },
            None,
        )
        .await?;
    tokio::pin!(reply_stream);
    while let Some(event) = reply_stream.next().await {
        event?;
    }
    Ok(provider)
}

/// Resuming a session creates a new `Agent` (a new CLI process for `--resume`,
/// or an ACP `session/load`). Hints the stored conversation already carries
/// must not be appended again when a later turn touches the same directory.
#[tokio::test]
async fn subdirectory_hints_are_not_reinjected_after_resume() -> Result<()> {
    let workdir = TempDir::new()?;
    let sub = workdir.path().join("sub");
    std::fs::create_dir_all(&sub)?;
    std::fs::write(sub.join(".goslinghints"), SENTINEL)?;

    let data_dir = TempDir::new()?;
    let session_manager = Arc::new(SessionManager::new(data_dir.path().to_path_buf()));
    let session = session_manager
        .create_session(
            workdir.path().to_path_buf(),
            "subdir-hints-resume".to_string(),
            SessionType::Hidden,
            GoslingMode::Auto,
        )
        .await?;

    reply_with_new_agent(&session_manager, &session.id).await?;
    let resumed_provider = reply_with_new_agent(&session_manager, &session.id).await?;

    let conversation = session_manager
        .get_session(&session.id, true)
        .await?
        .conversation
        .expect("session has a conversation");
    let hint_messages = conversation
        .messages()
        .iter()
        .filter(|m| m.as_concat_text().contains(SENTINEL))
        .count();
    assert_eq!(
        hint_messages, 1,
        "a resumed session must not append hints its conversation already carries"
    );
    assert!(
        resumed_provider.requests_with_hint.load(Ordering::SeqCst) >= 1,
        "the resumed turn still sees the hint from the stored conversation"
    );

    Ok(())
}
