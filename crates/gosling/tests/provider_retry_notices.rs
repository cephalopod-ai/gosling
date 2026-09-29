//! A provider that backs off before retrying (a 429 with Retry-After, a 5xx)
//! must tell the user why the reply is delayed instead of leaving an
//! unexplained spinner (GSL-PT-20260927-B04).

use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use gosling::agents::{Agent, AgentConfig, AgentEvent, GoslingPlatform, SessionConfig};
use gosling::config::{GoslingMode, PermissionManager};
use gosling::conversation::message::{Message, MessageContent};
use gosling::providers::base::{stream_from_single_message, MessageStream, Provider};
use gosling::session::session_manager::{SessionManager, SessionType};
use gosling_providers::conversation::token_usage::{ProviderUsage, Usage};
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use gosling_providers::retry::ProviderRetry;
use rmcp::model::Tool;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;

fn usage() -> ProviderUsage {
    ProviderUsage::new("mock-model".to_string(), Usage::default())
}

fn is_session_naming(messages: &[Message]) -> bool {
    messages.iter().any(|message| {
        message
            .as_concat_text()
            .contains("---BEGIN USER MESSAGES---")
    })
}

/// Rejects the first `rate_limited_attempts` requests with a 429 carrying a
/// Retry-After, then answers "ok".
struct RateLimitedProvider {
    rate_limited_attempts: usize,
    attempts: AtomicUsize,
}

impl RateLimitedProvider {
    fn new(rate_limited_attempts: usize) -> Self {
        Self {
            rate_limited_attempts,
            attempts: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Provider for RateLimitedProvider {
    async fn stream(
        &self,
        _model_config: &ModelConfig,
        _system_prompt: &str,
        messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        if is_session_naming(messages) {
            return Ok(stream_from_single_message(
                Message::assistant().with_text("title"),
                usage(),
            ));
        }
        self.with_retry(|| async {
            if self.attempts.fetch_add(1, Ordering::SeqCst) < self.rate_limited_attempts {
                return Err(ProviderError::RateLimitExceeded {
                    details: "fixture forced HTTP 429".to_string(),
                    retry_delay: Some(Duration::from_millis(1200)),
                });
            }
            Ok(stream_from_single_message(
                Message::assistant().with_text("ok"),
                usage(),
            ))
        })
        .await
    }

    fn get_name(&self) -> &str {
        "mock-rate-limited"
    }
}

/// Every notification text and assistant text the reply stream yielded, in order.
async fn reply_texts(provider: Arc<dyn Provider>) -> Result<Vec<String>> {
    let temp = TempDir::new()?;
    let session_manager = Arc::new(SessionManager::new(temp.path().join("data")));
    let agent = Agent::with_config(AgentConfig::new(
        session_manager.clone(),
        Arc::new(PermissionManager::new(temp.path().join("config"))),
        GoslingMode::Auto,
        true,
        GoslingPlatform::GoslingCli,
    ));
    let session = session_manager
        .create_session(
            temp.path().to_path_buf(),
            "provider-retry-notices".to_string(),
            SessionType::User,
            GoslingMode::Auto,
        )
        .await?;
    agent
        .update_provider(provider, ModelConfig::new("mock-model"), &session.id)
        .await?;

    let stream = agent
        .reply(
            Message::user().with_text("hello"),
            SessionConfig {
                id: session.id.clone(),
                max_turns: Some(2),
                compacted_context: false,
                tail_limit: None,
            },
            None,
        )
        .await?;
    tokio::pin!(stream);
    let mut texts = Vec::new();
    while let Some(event) = stream.next().await {
        if let AgentEvent::Message(message) = event? {
            for content in &message.content {
                match content {
                    MessageContent::SystemNotification(notice) => {
                        texts.push(notice.msg.clone());
                    }
                    MessageContent::Text(text) => texts.push(text.text.clone()),
                    _ => {}
                }
            }
        }
    }
    Ok(texts)
}

#[tokio::test]
async fn rate_limit_backoff_is_announced_before_the_reply() -> Result<()> {
    let texts = reply_texts(Arc::new(RateLimitedProvider::new(2))).await?;

    assert_eq!(
        texts,
        vec![
            "The provider is rate limiting requests. Retrying in 2s (1/3)...".to_string(),
            "The provider is rate limiting requests. Retrying in 2s (2/3)...".to_string(),
            "ok".to_string(),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn a_first_try_success_announces_no_retry() -> Result<()> {
    let texts = reply_texts(Arc::new(RateLimitedProvider::new(0))).await?;

    assert_eq!(texts, vec!["ok".to_string()]);
    Ok(())
}
