//! A turn whose lease another process took over must stop within one lease
//! heartbeat with the lease-lost error, without waiting for the provider to
//! answer (GSL-PT-20260927-A08, F11).

use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use gosling::agents::{Agent, AgentConfig, GoslingPlatform, SessionConfig};
use gosling::config::{GoslingMode, PermissionManager};
use gosling::conversation::message::Message;
use gosling::providers::base::{MessageStream, Provider};
use gosling::session::session_manager::{SessionManager, SessionType};
use gosling_providers::conversation::token_usage::ProviderUsage;
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use rmcp::model::Tool;
use sqlx::sqlite::SqliteConnectOptions;
use sqlx::ConnectOptions;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

type StreamItem = Result<(Option<Message>, Option<ProviderUsage>), ProviderError>;

struct SetOnDrop(Arc<AtomicBool>);

impl Drop for SetOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::SeqCst);
    }
}

/// Takes the request and never answers, like a provider stuck on a long call.
struct UnansweredProvider {
    request_started: Arc<Notify>,
    request_abandoned: Arc<AtomicBool>,
}

#[async_trait]
impl Provider for UnansweredProvider {
    async fn stream(
        &self,
        _model_config: &ModelConfig,
        _system_prompt: &str,
        _messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        self.request_started.notify_one();
        let abandoned = SetOnDrop(Arc::clone(&self.request_abandoned));
        Ok(Box::pin(futures::stream::poll_fn(
            move |_| -> std::task::Poll<Option<StreamItem>> {
                let _ = &abandoned;
                std::task::Poll::Pending
            },
        )))
    }

    fn get_name(&self) -> &str {
        "mock-unanswered"
    }
}

/// What another process taking the session over does to this one's lease.
async fn revoke_turn_lease(data_dir: &Path, session_id: &str) {
    let mut connection = SqliteConnectOptions::new()
        .filename(data_dir.join("sessions").join("sessions.db"))
        .connect()
        .await
        .unwrap();
    let revoked = sqlx::query("DELETE FROM session_turn_leases WHERE session_id = ?")
        .bind(session_id)
        .execute(&mut connection)
        .await
        .unwrap();
    assert_eq!(revoked.rows_affected(), 1, "the turn held a lease");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_turn_whose_lease_was_taken_over_stops_without_waiting_for_the_provider() -> Result<()> {
    let temp = TempDir::new()?;
    let data_dir = temp.path().join("data");
    let session_manager = Arc::new(SessionManager::new(data_dir.clone()));
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
            "lease-revocation".to_string(),
            SessionType::User,
            GoslingMode::Auto,
        )
        .await?;
    let request_started = Arc::new(Notify::new());
    let request_abandoned = Arc::new(AtomicBool::new(false));
    agent
        .update_provider(
            Arc::new(UnansweredProvider {
                request_started: Arc::clone(&request_started),
                request_abandoned: Arc::clone(&request_abandoned),
            }),
            ModelConfig::new("mock-model"),
            &session.id,
        )
        .await?;

    let mut reply = agent
        .reply(
            Message::user().with_text("P1-LATE"),
            SessionConfig {
                id: session.id.clone(),
                max_turns: None,
                compacted_context: false,
                tail_limit: None,
            },
            Some(CancellationToken::new()),
        )
        .await?;
    let read_reply = async {
        let mut last = None;
        while let Some(event) = reply.next().await {
            last = Some(event);
        }
        last
    };
    let take_over = async {
        request_started.notified().await;
        revoke_turn_lease(&data_dir, &session.id).await;
        Instant::now()
    };
    let (last, revoked_at) = tokio::time::timeout(
        Duration::from_secs(30),
        futures::future::join(read_reply, take_over),
    )
    .await
    .expect("a revoked turn must not wait for the provider to answer");

    assert!(
        revoked_at.elapsed() < Duration::from_secs(20),
        "the turn stops within one lease heartbeat of the takeover"
    );
    let error = match last {
        Some(Err(error)) => error,
        other => panic!("expected the lease-lost error, got {other:?}"),
    };
    assert!(
        error.to_string().starts_with("Session turn lease was lost"),
        "{error}"
    );
    assert!(
        request_abandoned.load(Ordering::SeqCst),
        "the provider request is abandoned with the turn"
    );
    Ok(())
}
