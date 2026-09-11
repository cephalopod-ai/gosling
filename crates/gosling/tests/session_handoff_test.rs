use gosling::config::GoslingMode;
use gosling::conversation::message::{Message, MessageContent};
use gosling::conversation::Conversation;
use gosling::providers::base::{CapabilitySupport, ContextOwnership, ProviderCapabilities};
use gosling::session::handoff::SessionHandoffBuilder;
use gosling::session::{SessionManager, SessionType};
use gosling_providers::model::ModelConfig;
use gosling_sdk_types::session_handoff::{SessionHandoffStatusDto, SessionHandoffTriggerDto};
use rmcp::model::{CallToolRequestParams, CallToolResult, Content};

async fn source_session(manager: &SessionManager) -> String {
    let session = manager
        .create_session(
            std::path::PathBuf::from("/tmp/handoff-workspace"),
            "Continuity source".to_string(),
            SessionType::User,
            GoslingMode::Approve,
        )
        .await
        .unwrap();
    manager
        .update(&session.id)
        .provider_name("openai")
        .model_config(ModelConfig::new("gpt-4o"))
        .apply()
        .await
        .unwrap();
    session.id
}

#[tokio::test]
async fn deterministic_checkpoint_is_bounded_redacted_and_provider_independent() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(temp_dir.path().to_path_buf());
    let session_id = source_session(&manager).await;
    manager
        .add_message(
            &session_id,
            &Message::user().with_text(
                "Finish continuity support. Authorization: Bearer sk-super-secret-token",
            ),
        )
        .await
        .unwrap();
    let call = CallToolRequestParams::new("developer__shell").with_arguments(rmcp::object!({
        "cmd": "cargo test",
        "env": { "API_KEY": "secret-value" }
    }));
    manager
        .add_message(
            &session_id,
            &Message::assistant()
                .with_generated_id()
                .with_tool_request("tool-1", Ok(call)),
        )
        .await
        .unwrap();
    manager
        .add_message(
            &session_id,
            &Message::user().with_tool_response(
                "tool-1",
                Ok(CallToolResult::success(vec![Content::text(
                    "tests passed; password=hunter2",
                )])),
            ),
        )
        .await
        .unwrap();

    let builder = SessionHandoffBuilder::new(&manager);
    let first = builder
        .build(
            &session_id,
            "anthropic",
            "claude-sonnet",
            128_000,
            ProviderCapabilities::gosling_managed(),
            SessionHandoffTriggerDto::ProviderFailure,
        )
        .await
        .unwrap();
    let second = builder
        .build(
            &session_id,
            "anthropic",
            "claude-sonnet",
            128_000,
            ProviderCapabilities::gosling_managed(),
            SessionHandoffTriggerDto::ProviderFailure,
        )
        .await
        .unwrap();

    assert_eq!(first.coverage.source_hash, second.coverage.source_hash);
    assert!(first.coverage.estimated_tokens <= 12_800);
    assert_eq!(
        first
            .latest_user_intent
            .as_ref()
            .map(|item| item.content.as_str()),
        Some("Finish continuity support. [REDACTED]")
    );
    assert!(first
        .commands_and_checks
        .iter()
        .any(|item| item.content == "cargo test — succeeded"));
    let persisted = serde_json::to_string(&first).unwrap();
    assert!(!persisted.contains("super-secret"));
    assert!(!persisted.contains("hunter2"));
    assert!(!persisted.contains("secret-value"));
    assert!(first.redaction_report.redaction_count >= 2);
}

#[tokio::test]
async fn handoff_creation_commits_new_session_and_active_checkpoint_together() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(temp_dir.path().to_path_buf());
    let source_session_id = source_session(&manager).await;
    manager
        .add_message(
            &source_session_id,
            &Message::user().with_text("Finish the handoff implementation"),
        )
        .await
        .unwrap();
    let snapshot = SessionHandoffBuilder::new(&manager)
        .build(
            &source_session_id,
            "openai",
            "gpt-4o",
            128_000,
            ProviderCapabilities::gosling_managed(),
            SessionHandoffTriggerDto::SessionFork,
        )
        .await
        .unwrap();

    let (target, snapshot) = manager
        .create_handoff_session(
            &source_session_id,
            "Continuity target".to_string(),
            "openai".to_string(),
            ModelConfig::new("gpt-4o"),
            snapshot,
        )
        .await
        .unwrap();
    assert_eq!(snapshot.session_id, target.id);
    assert_eq!(
        snapshot.source_session_id.as_deref(),
        Some(source_session_id.as_str())
    );
    assert_eq!(snapshot.status, SessionHandoffStatusDto::Active);
    assert_eq!(snapshot.generation, 1);
    let stored = manager
        .latest_handoff_snapshot(&target.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.snapshot_id, snapshot.snapshot_id);
    let conversation = target.conversation.unwrap();
    assert_eq!(conversation.messages().len(), 1);
    assert!(!conversation.messages()[0].metadata.user_visible);
    assert!(conversation.messages()[0].metadata.agent_visible);
    assert!(matches!(
        conversation.messages()[0].content.first(),
        Some(MessageContent::Text(_))
    ));
}

#[tokio::test]
async fn stale_generation_is_rejected_and_failure_state_is_durable() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(temp_dir.path().to_path_buf());
    let session_id = source_session(&manager).await;
    let snapshot = SessionHandoffBuilder::new(&manager)
        .build(
            &session_id,
            "openai",
            "gpt-4o",
            128_000,
            ProviderCapabilities::gosling_managed(),
            SessionHandoffTriggerDto::ManualCheckpoint,
        )
        .await
        .unwrap();
    let prepared = manager
        .prepare_handoff_snapshot(snapshot.clone(), Some(0))
        .await
        .unwrap();
    assert_eq!(prepared.generation, 1);
    assert!(manager
        .prepare_handoff_snapshot(snapshot, Some(0))
        .await
        .unwrap_err()
        .to_string()
        .contains("stale handoff generation"));

    manager
        .update_handoff_status(
            &prepared.snapshot_id,
            SessionHandoffStatusDto::Failed,
            Some("target initialization failed; Authorization: Bearer sk-hidden-secret-token"),
        )
        .await
        .unwrap();
    let latest = manager
        .latest_handoff_snapshot(&session_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(latest.status, SessionHandoffStatusDto::Failed);
    assert_eq!(
        latest.failure.as_deref(),
        Some("target initialization failed; [REDACTED]")
    );
    assert!(latest.redaction_report.redaction_count > 0);
}

#[tokio::test]
async fn large_history_is_hard_bounded_by_the_target_context() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(temp_dir.path().to_path_buf());
    let session_id = source_session(&manager).await;
    for index in 0..100 {
        manager
            .add_message(
                &session_id,
                &Message::user().with_text(format!("message {index}: {}", "x".repeat(5_000))),
            )
            .await
            .unwrap();
    }

    let snapshot = SessionHandoffBuilder::new(&manager)
        .build(
            &session_id,
            "openai",
            "small-context-model",
            8_192,
            ProviderCapabilities::gosling_managed(),
            SessionHandoffTriggerDto::ManualCheckpoint,
        )
        .await
        .unwrap();

    assert!(snapshot.coverage.estimated_tokens <= 819);
    assert!(snapshot.coverage.recent_tail_message_count < 100);
    assert!(snapshot.redaction_report.truncated_item_count > 0);
}

#[tokio::test]
async fn pending_handoff_delivers_checkpoint_and_latest_user_once() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(temp_dir.path().to_path_buf());
    let source_session_id = source_session(&manager).await;
    manager
        .add_message(
            &source_session_id,
            &Message::user().with_text("old raw history must not be replayed"),
        )
        .await
        .unwrap();
    let snapshot = SessionHandoffBuilder::new(&manager)
        .build(
            &source_session_id,
            "openai",
            "gpt-4o",
            128_000,
            ProviderCapabilities::gosling_managed(),
            SessionHandoffTriggerDto::SessionFork,
        )
        .await
        .unwrap();
    let (target, snapshot) = manager
        .create_handoff_session(
            &source_session_id,
            "Target".to_string(),
            "openai".to_string(),
            ModelConfig::new("gpt-4o"),
            snapshot,
        )
        .await
        .unwrap();
    let current_user = Message::user().with_text("continue the current task");
    manager
        .add_message(&target.id, &current_user)
        .await
        .unwrap();
    let stored = manager.get_session(&target.id, true).await.unwrap();
    let conversation = stored.conversation.unwrap_or_else(Conversation::default);

    let (delivered, pending_id) = gosling::session::handoff::conversation_for_pending_handoff(
        &manager,
        &target.id,
        &conversation,
    )
    .await
    .unwrap();

    assert_eq!(pending_id.as_deref(), Some(snapshot.snapshot_id.as_str()));
    assert_eq!(delivered.messages().len(), 2);
    assert!(!delivered
        .messages()
        .iter()
        .any(|message| message.as_concat_text() == "old raw history must not be replayed"));
    assert_eq!(
        delivered
            .messages()
            .iter()
            .filter(|message| message.as_concat_text() == "continue the current task")
            .count(),
        1
    );
}

#[tokio::test]
async fn provider_managed_bootstrap_combines_checkpoint_and_current_request_once() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(temp_dir.path().to_path_buf());
    let source_session_id = source_session(&manager).await;
    manager
        .add_message(
            &source_session_id,
            &Message::user().with_text("preserve the provider-owned objective"),
        )
        .await
        .unwrap();
    let snapshot = SessionHandoffBuilder::new(&manager)
        .build(
            &source_session_id,
            "acp-provider",
            "managed-model",
            128_000,
            ProviderCapabilities::provider_managed(),
            SessionHandoffTriggerDto::SessionFork,
        )
        .await
        .unwrap();
    let (target, snapshot) = manager
        .create_handoff_session(
            &source_session_id,
            "Managed target".to_string(),
            "acp-provider".to_string(),
            ModelConfig::new("managed-model"),
            snapshot,
        )
        .await
        .unwrap();
    manager
        .add_message(
            &target.id,
            &Message::user().with_text("continue without replaying side effects"),
        )
        .await
        .unwrap();
    let conversation = manager
        .get_session(&target.id, true)
        .await
        .unwrap()
        .conversation
        .unwrap();

    let (delivered, pending_id) = gosling::session::handoff::conversation_for_pending_handoff(
        &manager,
        &target.id,
        &conversation,
    )
    .await
    .unwrap();

    assert_eq!(pending_id.as_deref(), Some(snapshot.snapshot_id.as_str()));
    assert_eq!(delivered.messages().len(), 1);
    let prompt = delivered.messages()[0].as_concat_text();
    assert!(prompt.contains("preserve the provider-owned objective"));
    assert_eq!(
        prompt
            .matches("continue without replaying side effects")
            .count(),
        1
    );
}

#[tokio::test]
async fn new_context_delivery_excludes_the_checkpoint() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(temp_dir.path().to_path_buf());
    let source_session_id = source_session(&manager).await;
    manager
        .add_message(
            &source_session_id,
            &Message::user().with_text("history that must stay behind"),
        )
        .await
        .unwrap();
    let capabilities = ProviderCapabilities {
        context_ownership: ContextOwnership::Provider,
        bootstrap_handoff: CapabilitySupport::Unsupported,
        ..ProviderCapabilities::provider_managed()
    };
    let snapshot = SessionHandoffBuilder::new(&manager)
        .build(
            &source_session_id,
            "new-context-provider",
            "new-context-model",
            128_000,
            capabilities,
            SessionHandoffTriggerDto::SessionFork,
        )
        .await
        .unwrap();
    let (target, _) = manager
        .create_handoff_session(
            &source_session_id,
            "New context target".to_string(),
            "new-context-provider".to_string(),
            ModelConfig::new("new-context-model"),
            snapshot,
        )
        .await
        .unwrap();
    manager
        .add_message(
            &target.id,
            &Message::user().with_text("start clean from this request"),
        )
        .await
        .unwrap();
    let conversation = manager
        .get_session(&target.id, true)
        .await
        .unwrap()
        .conversation
        .unwrap();

    let (delivered, pending_id) = gosling::session::handoff::conversation_for_pending_handoff(
        &manager,
        &target.id,
        &conversation,
    )
    .await
    .unwrap();

    assert!(pending_id.is_none());
    assert_eq!(delivered.messages().len(), 1);
    assert_eq!(
        delivered.messages()[0].as_concat_text(),
        "start clean from this request"
    );
    assert!(!delivered.messages()[0]
        .as_concat_text()
        .contains("history that must stay behind"));
}

#[tokio::test]
async fn provider_transition_rebases_current_usage_and_preserves_accumulated_usage() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(temp_dir.path().to_path_buf());
    let session_id = source_session(&manager).await;
    let accumulated = gosling_providers::conversation::token_usage::Usage::new(
        Some(1_000),
        Some(200),
        Some(1_200),
    );
    manager
        .update(&session_id)
        .usage(gosling_providers::conversation::token_usage::Usage::new(
            Some(900),
            Some(100),
            Some(1_000),
        ))
        .context_usage_estimated(false)
        .last_request_tokens(Some(1_000))
        .accumulated_usage(accumulated)
        .apply()
        .await
        .unwrap();
    manager
        .add_message(
            &session_id,
            &Message::user().with_text("continue with a fresh target context"),
        )
        .await
        .unwrap();
    let snapshot = SessionHandoffBuilder::new(&manager)
        .build(
            &session_id,
            "openai",
            "gpt-4o",
            128_000,
            ProviderCapabilities::gosling_managed(),
            SessionHandoffTriggerDto::UserRequestedSwitch,
        )
        .await
        .unwrap();
    let prepared = manager
        .prepare_handoff_snapshot(snapshot, None)
        .await
        .unwrap();
    manager
        .update_handoff_status(
            &prepared.snapshot_id,
            SessionHandoffStatusDto::Activating,
            None,
        )
        .await
        .unwrap();

    manager
        .commit_provider_transition(
            &prepared.snapshot_id,
            "openai",
            ModelConfig::new("gpt-4o"),
            GoslingMode::Approve,
        )
        .await
        .unwrap();

    let transitioned = manager.get_session(&session_id, false).await.unwrap();
    let checkpoint_tokens = prepared.coverage.estimated_tokens as i32;
    assert_eq!(
        transitioned.usage,
        gosling_providers::conversation::token_usage::Usage::new(
            Some(checkpoint_tokens),
            Some(0),
            Some(checkpoint_tokens),
        )
    );
    assert!(transitioned.context_usage_estimated);
    assert_eq!(transitioned.last_request_tokens, Some(1_000));
    assert_eq!(transitioned.accumulated_usage, accumulated);
}

#[tokio::test]
async fn retention_and_session_delete_keep_checkpoint_storage_bounded() {
    let temp_dir = tempfile::tempdir().unwrap();
    let manager = SessionManager::new(temp_dir.path().to_path_buf());
    let session_id = source_session(&manager).await;
    let builder = SessionHandoffBuilder::new(&manager);

    for _ in 0..7 {
        let snapshot = builder
            .build(
                &session_id,
                "openai",
                "gpt-4o",
                128_000,
                ProviderCapabilities::gosling_managed(),
                SessionHandoffTriggerDto::ManualCheckpoint,
            )
            .await
            .unwrap();
        let prepared = manager
            .prepare_handoff_snapshot(snapshot, None)
            .await
            .unwrap();
        manager
            .update_handoff_status(
                &prepared.snapshot_id,
                SessionHandoffStatusDto::Failed,
                Some("injected failure"),
            )
            .await
            .unwrap();
    }
    let snapshot = builder
        .build(
            &session_id,
            "openai",
            "gpt-4o",
            128_000,
            ProviderCapabilities::gosling_managed(),
            SessionHandoffTriggerDto::ManualCheckpoint,
        )
        .await
        .unwrap();
    let prepared = manager
        .prepare_handoff_snapshot(snapshot, None)
        .await
        .unwrap();
    manager
        .update_handoff_status(
            &prepared.snapshot_id,
            SessionHandoffStatusDto::Activating,
            None,
        )
        .await
        .unwrap();
    manager
        .commit_provider_transition(
            &prepared.snapshot_id,
            "openai",
            ModelConfig::new("gpt-4o"),
            GoslingMode::Approve,
        )
        .await
        .unwrap();

    let database_url = format!(
        "sqlite://{}",
        temp_dir.path().join("sessions/sessions.db").display()
    );
    let pool = sqlx::SqlitePool::connect(&database_url).await.unwrap();
    let retained: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM session_handoff_snapshots WHERE session_id = ?")
            .bind(&session_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(retained, 5);

    manager.delete_session(&session_id).await.unwrap();
    let remaining: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM session_handoff_snapshots WHERE session_id = ?")
            .bind(&session_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(remaining, 0);
}
