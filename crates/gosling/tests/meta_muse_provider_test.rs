use gosling::providers::base::{Provider, ProviderDef, ProviderDescriptor};
use gosling::providers::muse_code::MuseCodeProvider;
use gosling_providers::model::ModelConfig;
use gosling_providers::thinking::ThinkingEffort;
use serde_json::{json, Value};

#[tokio::test]
async fn api_and_cli_have_distinct_setup_and_execution_contracts() {
    use gosling::providers::catalog::{get_setup_catalog_entries, ProviderSetupCategory};

    let api = gosling::providers::get_from_registry("meta").await.unwrap();
    let cli = gosling::providers::get_from_registry("muse-code")
        .await
        .unwrap();
    assert!(!api.executes_tools_outside_gosling());
    assert!(cli.executes_tools_outside_gosling());
    assert!(!cli.manages_own_context());
    let key = &api.metadata().config_keys[0];
    assert_eq!(key.name, "MODEL_API_KEY");
    assert!(key.secret && key.required);
    assert!(cli.metadata().config_keys.iter().all(|key| !key.secret));
    let cli_keys = MuseCodeProvider::metadata().config_keys;
    assert_eq!(cli_keys.len(), 1);
    assert_eq!(cli_keys[0].name, "MUSE_CODE_COMMAND");
    assert!(!cli_keys[0].required);
    assert!(!MuseCodeProvider::metadata().setup_steps.is_empty());
    for entry in [api, cli] {
        let model = entry
            .normalize_model_config(ModelConfig::new("muse-spark-1.3"))
            .unwrap();
        assert_eq!(model.context_limit, Some(1_048_576));
        assert_eq!(model.reasoning, Some(true));
    }
    let catalog = get_setup_catalog_entries().await;
    assert_eq!(
        catalog
            .iter()
            .find(|entry| entry.provider_id == "meta")
            .unwrap()
            .category,
        ProviderSetupCategory::Model
    );
    assert_eq!(
        catalog
            .iter()
            .find(|entry| entry.provider_id == "muse-code")
            .unwrap()
            .category,
        ProviderSetupCategory::Agent
    );
}

#[test]
fn reasoning_levels_stay_within_metas_hosted_model_contract() {
    use gosling_providers::formats::meta::{prepare_request, reasoning_effort};

    for name in [
        "muse-wonder-1.0",
        "muse-spark-1.3",
        "muse-spark-1.2",
        "muse-spark-1.1",
        "muse-spark-1.3-contributor",
    ] {
        assert_eq!(reasoning_effort(name, ThinkingEffort::Off), "minimal");
        assert_eq!(reasoning_effort(name, ThinkingEffort::Low), "low");
        assert_eq!(reasoning_effort(name, ThinkingEffort::Medium), "medium");
        assert_eq!(reasoning_effort(name, ThinkingEffort::High), "high");
        let highest = if matches!(name, "muse-wonder-1.0" | "muse-spark-1.3") {
            "max"
        } else {
            "xhigh"
        };
        assert_eq!(reasoning_effort(name, ThinkingEffort::Max), highest);
        assert_eq!(reasoning_effort(name, ThinkingEffort::Ultra), highest);
        let model = ModelConfig::new(name).with_thinking_effort(ThinkingEffort::Off);
        assert_eq!(
            prepare_request(json!({}), &model)["reasoning_effort"],
            "minimal"
        );
        assert_eq!(
            prepare_request(json!({"reasoning_effort":"high"}), &model)["reasoning_effort"],
            "high"
        );
        assert!(prepare_request(json!({}), &ModelConfig::new(name))
            .get("reasoning_effort")
            .is_none());
    }
}

#[tokio::test]
async fn api_sends_authenticated_chat_requests_with_gosling_tools_and_usage() {
    use gosling::config::DeclarativeProviderConfig;
    use gosling::conversation::message::Message;
    use gosling::providers::openai_def::from_custom_config;
    use wiremock::matchers::{header, method, path};
    use wiremock::{Mock, MockServer, Request, ResponseTemplate};

    let root = tempfile::tempdir().unwrap();
    let _env = env_lock::lock_env([
        ("GOSLING_PATH_ROOT", Some(root.path().to_str().unwrap())),
        ("MODEL_API_KEY", Some("meta-fixture-key")),
    ]);
    let server = MockServer::start().await;
    for streaming in [false, true] {
        Mock::given(method("POST"))
            .and(path("/v1/chat/completions"))
            .and(header("authorization", "Bearer meta-fixture-key"))
            .respond_with(move |request: &Request| {
                let body: Value = serde_json::from_slice(&request.body).unwrap();
                assert_eq!(body["model"], "muse-spark-1.3");
                assert_eq!(body["reasoning_effort"], "minimal");
                assert_eq!(body["max_tokens"], 1024);
                assert_eq!(body["tools"][0]["function"]["name"], "read_file");
                assert!(body["messages"][1].get("reasoning_content").is_none());
                if streaming {
                    assert_eq!(body["stream"], true);
                    ResponseTemplate::new(200).set_body_string(concat!(
                        "data: {\"model\":\"muse-spark-1.3\",\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
                        "data: {\"model\":\"muse-spark-1.3\",\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}\n\n",
                        "data: [DONE]\n\n"
                    ))
                } else {
                    ResponseTemplate::new(200).set_body_json(json!({
                        "model": "muse-spark-1.3",
                        "choices": [{"message":{"role":"assistant", "content":"Hello"}, "finish_reason":"stop"}],
                        "usage": {"prompt_tokens":10,"completion_tokens":2,"total_tokens":12}
                    }))
                }
            }).expect(1).mount(&server).await;
        let mut config: DeclarativeProviderConfig =
            serde_json::from_str(include_str!("../src/providers/declarative/meta.json")).unwrap();
        config.base_url = format!("{}/v1/chat/completions", server.uri());
        config.supports_streaming = Some(streaming);
        let provider = from_custom_config(config, None).unwrap();
        let tool = rmcp::model::Tool::new(
            "read_file",
            "Read a file",
            rmcp::object!({"type":"object", "properties":{}}),
        );
        let (message, usage) = provider
            .complete(
                &ModelConfig::new("muse-spark-1.3")
                    .with_max_tokens(Some(1024))
                    .with_thinking_effort(ThinkingEffort::Off),
                "System",
                &[
                    Message::assistant()
                        .with_thinking("private", "signature")
                        .with_text("Earlier"),
                    Message::user().with_text("Hi"),
                ],
                &[tool],
            )
            .await
            .unwrap();
        assert_eq!(message.as_concat_text(), "Hello");
        assert_eq!(usage.usage.total_tokens, Some(12));
        server.verify().await;
        server.reset().await;
    }
}

#[cfg(unix)]
mod cli {
    use super::*;
    use futures::StreamExt;
    use gosling::config::GoslingMode;
    use gosling::conversation::message::{Message, MessageContent};
    use std::os::unix::fs::PermissionsExt;
    use std::path::Path;
    use std::time::Duration;

    fn event(kind: &str, terminal: Option<&str>, text: &str) -> String {
        json!({
            "schema_version":1, "payload_schema_version":1, "payload_type":kind,
            "payload":{"run_stream":{"kind":"run","id":"root"}, "terminal":terminal, "text":text}
        })
        .to_string()
    }

    fn successful_events() -> String {
        format!(
            "{}\n{}\n",
            event("run.lifecycle.started", None, ""),
            event("run.terminal.completed", Some("completed"), "Done")
        )
    }

    fn script(root: &Path, body: &str) -> std::path::PathBuf {
        let command = root.join("muse-fixture");
        let probe_handlers = r#"
test "${META_API_KEY-unset}" = unset
test "${MODEL_API_KEY-unset}" = unset
test "${MUSE_PERMISSION_PROFILE_FAIL_OPEN-unset}" = unset
case "$*" in
  --version) echo 'Muse Code 1.4.25 (fixture)'; exit 0 ;;
  --help) echo '  usage  Show your subscription allowance'; exit 0 ;;
  'exec --help')
    echo '--json --provider --model --prompt-file --no-session-log --disable-approval --reasoning-effort --list-models'
    exit 0 ;;
  'usage --json')
    if [ -f billing.json ]; then cat billing.json; else
      echo '{"schemaVersion":1,"allowance":null,"reason":"server_sent_no_usage"}'
    fi
    exit 0 ;;
  'exec --provider meta --list-models')
    if [ -f model-list.txt ]; then cat model-list.txt; fi
    exit 0 ;;
esac
"#;
        std::fs::write(
            &command,
            format!("#!/bin/sh\nset -eu\n{probe_handlers}\n{body}\n"),
        )
        .unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
        command
    }

    async fn provider(
        root: &Path,
        command: &Path,
    ) -> (MuseCodeProvider, env_lock::EnvGuard<'static>) {
        provider_with_billing_ack(root, command, Some("true")).await
    }

    async fn provider_with_billing_ack(
        root: &Path,
        command: &Path,
        acknowledged: Option<&str>,
    ) -> (MuseCodeProvider, env_lock::EnvGuard<'static>) {
        let guard = env_lock::lock_env([
            ("GOSLING_PATH_ROOT", Some(root.to_str().unwrap())),
            ("MUSE_CODE_COMMAND", Some(command.to_str().unwrap())),
            ("GOSLING_MODE", Some("auto")),
            ("META_API_KEY", Some("must-not-reach-muse")),
            ("MODEL_API_KEY", Some("must-not-reach-muse-either")),
            ("MUSE_PERMISSION_PROFILE_FAIL_OPEN", Some("1")),
            ("MUSE_CODE_ACKNOWLEDGE_UNVERIFIED_BILLING", acknowledged),
        ]);
        let provider =
            MuseCodeProvider::from_env_with_working_dir(vec![], root.to_path_buf(), None)
                .await
                .unwrap();
        (provider, guard)
    }

    #[tokio::test]
    async fn compaction_never_launches_muse_code() -> anyhow::Result<()> {
        use gosling::agents::{Agent, AgentConfig, AgentEvent, GoslingPlatform, SessionConfig};
        use gosling::config::{Config, ConfigResolutionScope, PermissionManager};
        use gosling::context_mgmt::{compact_messages, generate_handoff_summary};
        use gosling::conversation::Conversation;
        use gosling::session::{SessionManager, SessionType};
        use gosling_providers::errors::ProviderError;
        use std::sync::Arc;

        let root = tempfile::tempdir()?;
        let launches = root.path().join("launches");
        std::fs::write(root.path().join("events.jsonl"), successful_events())?;
        let command = script(
            root.path(),
            "printf 'exec\\n' >> launches\ncat events.jsonl",
        );
        let (provider, _env) = provider(root.path(), &command).await;
        let model = ModelConfig::new("muse-spark-1.3").with_context_limit(Some(16_384));
        let conversation =
            Conversation::new_unvalidated([Message::user().with_text("Earlier request")]);
        let manager = Arc::new(SessionManager::new(root.path().join("sessions")));
        let agent = Agent::with_config(AgentConfig::new(
            manager.clone(),
            Arc::new(PermissionManager::new(root.path().join("permissions"))),
            GoslingMode::Auto,
            true,
            GoslingPlatform::GoslingCli,
        ));
        let session = manager
            .create_session(
                root.path().into(),
                "Muse compaction".into(),
                SessionType::Hidden,
                GoslingMode::Auto,
            )
            .await?;

        for manual in [true, false] {
            let error =
                compact_messages(&provider, &model, &session.id, &conversation, manual, None)
                    .await
                    .unwrap_err();
            assert!(error
                .to_string()
                .contains("cannot be used for tool-free summarization"));
        }
        assert!(
            generate_handoff_summary(&provider, &model, &session.id, &conversation)
                .await
                .unwrap_err()
                .to_string()
                .contains("cannot be used for tool-free summarization")
        );

        let error = provider
            .complete(
                &model.clone().with_context_limit(Some(16)),
                "System prompt",
                &[Message::user().with_text("Hello")],
                &[],
            )
            .await
            .unwrap_err();
        assert!(
            matches!(error, ProviderError::ContextLengthExceeded(message) if message.contains("Muse Code was not started"))
        );

        manager
            .replace_conversation(&session.id, &conversation)
            .await?;
        agent
            .update_provider(Arc::new(provider), model, &session.id)
            .await?;

        for (prompt, threshold, expected_launches, expected_error) in [
            ("/compact".to_string(), 0.8, 0, true),
            (" normal".repeat(512), 0.01, 1, false),
            (" oversized".repeat(18_000), 0.0, 1, true),
        ] {
            let settings = ConfigResolutionScope::unsaved(
                [(
                    "GOSLING_AUTO_COMPACT_THRESHOLD".to_string(),
                    json!(threshold),
                )]
                .into(),
                Default::default(),
            );
            let events = Config::with_resolution_scope(settings, async {
                let stream = agent
                    .reply(
                        Message::user().with_text(&prompt),
                        SessionConfig {
                            id: session.id.clone(),
                            max_turns: Some(2),
                            compacted_context: false,
                            tail_limit: None,
                        },
                        None,
                    )
                    .await?;
                futures::TryStreamExt::try_collect::<Vec<_>>(stream).await
            })
            .await?;
            let mut errors = Vec::new();
            let mut notices = Vec::new();
            for event in events {
                match event {
                    AgentEvent::HistoryReplaced(_) => panic!("unexpected history rewrite"),
                    AgentEvent::Message(message) => {
                        errors.extend(message.metadata.terminal_error);
                        notices.extend(message.content.into_iter().filter_map(|item| match item {
                            MessageContent::SystemNotification(notice) => Some(notice.msg),
                            _ => None,
                        }));
                    }
                    _ => {}
                }
            }
            assert_eq!(errors.len(), usize::from(expected_error), "{errors:?}");
            if expected_error {
                let expected = if prompt == "/compact" {
                    "tool-free summarization"
                } else {
                    "The request was not sent"
                };
                assert!(errors[0].contains(expected), "{errors:?}");
            } else {
                assert_eq!(
                    notices
                        .iter()
                        .filter(|notice| notice.contains("Automatic compaction is unavailable"))
                        .count(),
                    1,
                    "{notices:?}"
                );
            }
            let count = if launches.exists() {
                std::fs::read_to_string(&launches)?.lines().count()
            } else {
                0
            };
            assert_eq!(count, expected_launches);
            assert!(manager
                .list_compaction_revisions(&session.id, 10, None)
                .await?
                .is_empty());
        }
        let stored = manager
            .get_session(&session.id, true)
            .await?
            .conversation
            .unwrap();
        assert!(stored
            .messages()
            .iter()
            .any(|message| message.is_agent_visible()
                && message.as_concat_text() == "Earlier request"));
        assert!(stored
            .messages()
            .iter()
            .any(|message| !message.is_agent_visible()
                && message.is_user_visible()
                && message.as_concat_text().starts_with(" oversized")));
        Ok(())
    }

    #[tokio::test]
    async fn wrapper_preserves_cwd_history_and_private_prompt_transport_while_draining_stderr() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("events.jsonl"), successful_events()).unwrap();
        let command = script(
            root.path(),
            r#"
test "${META_API_KEY-unset}" = unset
test "${MODEL_API_KEY-unset}" = unset
test "$1" = exec
printf '%s\n' "$@" > args.txt
while [ "$#" -gt 0 ]; do
  if [ "$1" = --prompt-file ]; then shift; cat "$1" > prompt.txt; fi
  shift
done
i=0
while [ "$i" -lt 4000 ]; do
  printf 'stderr-flood-01234567890123456789\n' >&2
  i=$((i + 1))
done
cat events.jsonl
"#,
        );
        let (provider, _env) = provider(root.path(), &command).await;
        let (message, usage) = tokio::time::timeout(
            Duration::from_secs(10),
            provider.complete(
                &ModelConfig::new("muse-spark-1.3"),
                "System\n# Extensions\nnot-a-muse-tool",
                &[
                    Message::user().with_text("Earlier turn"),
                    Message::assistant().with_text("Prior answer"),
                    Message::user()
                        .with_text("Private prompt: quotes ' and \" and Unicode 🪿\nsecond line"),
                ],
                &[],
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(message.as_concat_text(), "Done");
        assert_eq!(usage.usage.total_tokens, None);
        let prompt = std::fs::read_to_string(root.path().join("prompt.txt")).unwrap();
        assert!(
            prompt.contains("Earlier turn")
                && prompt.contains("Prior answer")
                && prompt.contains("🪿")
        );
        assert!(!prompt.contains("not-a-muse-tool"));
        let args = std::fs::read_to_string(root.path().join("args.txt")).unwrap();
        assert!(!args.contains("Private prompt"));
        assert!(args.contains("--disable-approval") && args.contains("--no-session-log"));
        assert!(!args.contains("--yolo") && !args.contains("--disable-sandbox"));
        let path = args
            .lines()
            .skip_while(|arg| *arg != "--prompt-file")
            .nth(1)
            .unwrap();
        assert!(!Path::new(path).exists());
    }

    #[tokio::test]
    async fn wrapper_requires_a_valid_successful_terminal_and_successful_exit() {
        let root = tempfile::tempdir().unwrap();
        let command = script(root.path(), "cat events.jsonl\nexit 0");
        let (provider, _env) = provider(root.path(), &command).await;
        let started = event("run.lifecycle.started", None, "");
        let completed = event("run.terminal.completed", Some("completed"), "Done");
        for events in [
            "not JSON".to_string(),
            String::new(),
            started.clone(),
            completed.clone(),
            format!(
                "{started}\n{}",
                event("run.terminal.completed", Some("completed"), "")
            ),
            format!(
                "{started}\n{}",
                event("run.terminal.failed", Some("failed"), "")
            ),
            format!(
                "{started}\n{}",
                event("run.terminal.cancelled", Some("cancelled"), "")
            ),
            format!("{started}\n{completed}\n{completed}"),
            format!(
                "{started}\n{}\n{completed}",
                event("session.run.linked", None, "")
                    .replace("\"id\":\"root\"", "\"id\":\"other\"")
            ),
            format!(
                "{started}\n{}\n{completed}",
                event("run.output.delta", None, "Unattributed text")
                    .replace("\"id\":\"root\"", "\"id\":null")
            ),
            format!(
                "{started}\n{}\n{completed}",
                event("run.output.delta", None, "Conflicting prefix")
            ),
            successful_events().replace("\"schema_version\":1", "\"schema_version\":2"),
            successful_events().replace("\"id\":\"root\"", "\"id\":null"),
        ] {
            std::fs::write(root.path().join("events.jsonl"), format!("{events}\n")).unwrap();
            assert!(
                provider
                    .complete(
                        &ModelConfig::new("muse-spark-1.3"),
                        "System",
                        &[Message::user().with_text("Hi")],
                        &[]
                    )
                    .await
                    .is_err(),
                "accepted invalid events: {events}"
            );
        }
        script(
            root.path(),
            "cat events.jsonl\nprintf 'network error: private-sentinel\\n' >&2\nexit 1",
        );
        std::fs::write(root.path().join("events.jsonl"), successful_events()).unwrap();
        let error = provider
            .complete(
                &ModelConfig::new("muse-spark-1.3"),
                "System",
                &[Message::user().with_text("Hi")],
                &[],
            )
            .await
            .unwrap_err();
        assert!(error.to_string().contains("network failure"));
        assert!(!error.to_string().contains("private-sentinel"));
    }

    #[tokio::test]
    async fn wrapper_reports_a_conflicting_terminal_without_waiting_for_stdout_to_close() {
        let root = tempfile::tempdir().unwrap();
        let command = script(root.path(), "cat events.jsonl\nsleep 30");
        std::fs::write(
            root.path().join("events.jsonl"),
            format!(
                "{}\n{}\n{}\n",
                event("run.lifecycle.started", None, ""),
                event("run.output.delta", None, "Partial"),
                event("run.terminal.completed", Some("completed"), "Different")
            ),
        )
        .unwrap();
        let (provider, _env) = provider(root.path(), &command).await;
        let error = tokio::time::timeout(
            Duration::from_secs(5),
            provider.complete(
                &ModelConfig::new("muse-spark-1.3"),
                "System",
                &[Message::user().with_text("Hi")],
                &[],
            ),
        )
        .await
        .unwrap()
        .unwrap_err();
        assert!(error
            .to_string()
            .contains("inconsistent with its streamed text"));
    }

    #[tokio::test]
    async fn wrapper_rejects_unavailable_permissions_and_hosted_tools_before_launch() {
        let root = tempfile::tempdir().unwrap();
        let command = script(root.path(), "touch was-launched\nexit 1");
        let (provider, _env) = provider(root.path(), &command).await;
        for mode in [
            GoslingMode::Approve,
            GoslingMode::SmartApprove,
            GoslingMode::Chat,
        ] {
            assert!(provider.update_mode("session", mode).await.is_err());
        }
        let model = ModelConfig::new("muse-spark-1.3");
        let tool = rmcp::model::Tool::new("read", "Read", rmcp::object!({}));
        assert!(provider
            .stream(
                &model,
                "System",
                &[Message::user().with_text("Hi")],
                &[tool]
            )
            .await
            .is_err());
        let mut message = Message::user();
        message
            .content
            .push(MessageContent::image("aGVsbG8=", "image/png"));
        assert!(provider
            .stream(&model, "System", &[message], &[])
            .await
            .is_err());
        assert!(!root.path().join("was-launched").exists());
    }

    #[tokio::test]
    async fn model_discovery_precedes_session_mode_selection_without_running_a_turn() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("model-list.txt"),
            "muse-ember-1.0\nmuse-spark-1.3\nmuse-spark-1.3\n",
        )
        .unwrap();
        let command = script(root.path(), "touch was-launched\nexit 1");
        let _env = env_lock::lock_env([
            ("GOSLING_PATH_ROOT", Some(root.path().to_str().unwrap())),
            ("MUSE_CODE_COMMAND", Some(command.to_str().unwrap())),
            ("GOSLING_MODE", Some("approve")),
            ("META_API_KEY", Some("must-not-reach-muse")),
            ("MODEL_API_KEY", Some("must-not-reach-muse-either")),
        ]);
        let provider =
            MuseCodeProvider::from_env_with_working_dir(vec![], root.path().to_path_buf(), None)
                .await
                .unwrap();
        assert_eq!(
            provider.fetch_supported_models().await.unwrap(),
            ["muse-ember-1.0", "muse-spark-1.3"]
        );
        let models = provider.fetch_supported_model_info().await.unwrap();
        assert_eq!(models[0].name, "muse-ember-1.0");
        assert_eq!(models[1].context_limit, 1_048_576);
        assert!(provider
            .stream(
                &ModelConfig::new("muse-spark-1.3"),
                "System",
                &[Message::user().with_text("Hi")],
                &[]
            )
            .await
            .is_err());
        provider
            .update_mode("session", GoslingMode::Auto)
            .await
            .unwrap();
        assert!(!root.path().join("was-launched").exists());
    }

    #[tokio::test]
    async fn paid_api_keys_are_rejected_even_with_a_billing_acknowledgment() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("billing.json"),
            r#"{"schemaVersion":1,"allowance":null,"reason":"api_key"}"#,
        )
        .unwrap();
        let command = script(root.path(), "touch was-launched\nexit 1");
        let (provider, _env) = provider(root.path(), &command).await;
        let result = provider
            .complete(
                &ModelConfig::new("muse-spark-1.3"),
                "System",
                &[Message::user().with_text("Hi")],
                &[],
            )
            .await;
        assert!(matches!(
            result,
            Err(gosling_providers::errors::ProviderError::Authentication(message))
                if message.contains("selected an API key")
        ));
        assert!(!root.path().join("was-launched").exists());
    }

    #[tokio::test]
    async fn missing_or_malformed_billing_evidence_stops_before_exec() {
        let root = tempfile::tempdir().unwrap();
        let command = script(root.path(), "touch was-launched\ncat events.jsonl");
        let (provider, _env) = provider_with_billing_ack(root.path(), &command, None).await;
        let model = ModelConfig::new("muse-spark-1.3");
        let messages = [Message::user().with_text("Hi")];
        let billing = root.path().join("billing.json");
        for (status, diagnostic) in [
            (
                r#"{"schemaVersion":1,"allowance":null,"reason":"server_sent_no_usage"}"#,
                "MUSE_CODE_ACKNOWLEDGE_UNVERIFIED_BILLING",
            ),
            (
                r#"{"schemaVersion":1,"allowance":{"unexpected":true}}"#,
                "recognized subscription status",
            ),
            (
                r#"{"schemaVersion":1,"allowance":null,"reason":"future_reason"}"#,
                "recognized subscription status",
            ),
        ] {
            std::fs::write(&billing, status).unwrap();
            let failure = provider
                .complete(&model, "System", &messages, &[])
                .await
                .unwrap_err();
            assert!(failure.to_string().contains(diagnostic));
            assert!(!root.path().join("was-launched").exists());
        }

        std::fs::write(
            &billing,
            json!({
                "schemaVersion": 1,
                "allowance": {
                    "tier":"fixture", "observedAtMs":1,
                    "window":{"usedPercent":0,"resetsAtMs":2,"windowDurationMins":300},
                    "weekly":{"usedPercent":5,"resetsAtMs":3}
                }
            })
            .to_string(),
        )
        .unwrap();
        std::fs::write(root.path().join("events.jsonl"), successful_events()).unwrap();
        let (response, _) = provider
            .complete(&model, "System", &messages, &[])
            .await
            .unwrap();
        assert_eq!(response.as_concat_text(), "Done");
    }

    #[tokio::test]
    async fn executable_updates_invalidate_cached_capabilities() {
        let root = tempfile::tempdir().unwrap();
        let command = script(root.path(), "cat events.jsonl");
        let (provider, _env) = provider(root.path(), &command).await;
        std::fs::write(root.path().join("events.jsonl"), successful_events()).unwrap();
        let model = ModelConfig::new("muse-spark-1.3");
        let messages = [Message::user().with_text("Hi")];
        assert!(provider
            .complete(&model, "System", &messages, &[])
            .await
            .is_ok());
        let script = std::fs::read_to_string(&command)
            .unwrap()
            .replace("1.4.25", "1.5.0")
            .replace(" --no-session-log", "");
        std::fs::write(&command, script).unwrap();
        let error = provider
            .complete(&model, "System", &messages, &[])
            .await
            .unwrap_err();
        assert!(error
            .to_string()
            .contains("1.5.0 does not advertise exec --no-session-log"));
    }

    #[tokio::test]
    async fn stream_emits_root_deltas_before_exit_and_reconciles_final_usage_and_text() {
        let root = tempfile::tempdir().unwrap();
        let delta = event("run.output.delta", None, "Hello 🪿");
        let nested = event("run.output.delta", None, "Internal").replace("root", "child");
        let usage = |run_id: &str, input: u64, output: u64| {
            json!({
                "schema_version": 1, "payload_schema_version": 1, "payload_type": "run.usage",
                "payload": {
                    "run_stream": {"kind":"run", "id":run_id},
                    "models": [
                        {"model":"muse-spark-1.3", "input_tokens":input, "output_tokens":output, "cached_tokens":2, "future_field":7},
                        {"model":"muse-spark-1.2", "input_tokens":4, "output_tokens":3, "cached_tokens":1}
                    ]
                }
            })
        };
        std::fs::write(
            root.path().join("deltas.jsonl"),
            format!(
                "{}\n{}\n{nested}\n{}\n{delta}\n",
                event("session.run.linked", None, ""),
                event("run.lifecycle.started", None, "").replace("root", "child"),
                event("run.lifecycle.started", None, "")
            ),
        )
        .unwrap();
        std::fs::write(
            root.path().join("terminal.jsonl"),
            format!(
                "{}\n{}\n{}\n{}\n",
                usage("root", 10, 2),
                usage("child", 1000, 500),
                usage("root", 18, 5),
                event(
                    "run.terminal.completed",
                    Some("completed"),
                    "Hello 🪿 world"
                )
            ),
        )
        .unwrap();
        let command = script(
            root.path(),
            "cat deltas.jsonl\nwhile [ ! -f finish ]; do sleep 0.01; done\ncat terminal.jsonl",
        );
        let (provider, _env) = provider(root.path(), &command).await;
        let mut stream = provider
            .stream(
                &ModelConfig::new("muse-spark-1.3"),
                "System",
                &[Message::user().with_text("Hi")],
                &[],
            )
            .await
            .unwrap();
        let (first, usage) = tokio::time::timeout(Duration::from_secs(3), stream.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(first.unwrap().as_concat_text(), "Hello 🪿");
        assert!(usage.is_none());
        std::fs::write(root.path().join("finish"), "").unwrap();
        let (last, usage) = stream.next().await.unwrap().unwrap();
        assert_eq!(last.unwrap().as_concat_text(), " world");
        let usage = usage.unwrap().usage;
        assert_eq!(usage.input_tokens, Some(22));
        assert_eq!(usage.output_tokens, Some(8));
        assert_eq!(usage.total_tokens, Some(30));
        assert_eq!(usage.cache_read_input_tokens, Some(3));
        assert!(stream.next().await.is_none());
    }

    #[tokio::test]
    #[ignore = "requires an installed Muse executable named by GOSLING_TEST_MUSE_CLI"]
    async fn installed_muse_echo_round_trip_through_provider() {
        let root = tempfile::tempdir().unwrap();
        let command = root.path().join("muse-echo");
        std::fs::write(
            &command,
            r#"#!/bin/sh
set -eu
export HOME="$PWD"
export XDG_CONFIG_HOME="$PWD/config"
export XDG_DATA_HOME="$PWD/data"
export XDG_STATE_HOME="$PWD/state"
export MUSE_HOME="$PWD/muse"
case "$*" in
  'usage --json')
    echo '{"schemaVersion":1,"allowance":null,"reason":"server_sent_no_usage"}'
    exit 0 ;;
esac
if [ "$1" = exec ] && [ "${2-}" = --provider ]; then
  test "$3" = meta
  test "$4" = --json
  test "$5" = --model
  shift 6
  exec "$GOSLING_TEST_MUSE_CLI" exec --provider echo --json --disable-shell --disable-write "$@"
fi
exec "$GOSLING_TEST_MUSE_CLI" "$@"
"#,
        )
        .unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
        let (provider, _env) = provider(root.path(), &command).await;
        let (message, usage) = provider
            .complete(
                &ModelConfig::new("muse-spark-1.3"),
                "System",
                &[Message::user().with_text("echo qualification 🪿")],
                &[],
            )
            .await
            .unwrap();
        let text = message.as_concat_text();
        assert!(text.starts_with("echo: System"));
        assert_eq!(text.matches("echo qualification 🪿").count(), 1);
        assert_eq!(usage.usage.total_tokens, Some(0));
    }

    #[tokio::test]
    async fn dropping_the_stream_terminates_the_owned_cli() {
        let root = tempfile::tempdir().unwrap();
        let command = script(
            root.path(),
            "echo $$ > pid\nsleep 30 &\necho $! > descendant-pid\nwait",
        );
        let (provider, _env) = provider(root.path(), &command).await;
        let mut stream = provider
            .stream(
                &ModelConfig::new("muse-spark-1.3"),
                "System",
                &[Message::user().with_text("Hi")],
                &[],
            )
            .await
            .unwrap();
        let (pid, descendant) = tokio::time::timeout(Duration::from_secs(5), async {
            tokio::select! {
                result = stream.next() => panic!("Muse fixture exited before cancellation: {result:?}"),
                pids = async {
                    loop {
                        let read_pid = |name| std::fs::read_to_string(root.path().join(name))
                            .ok().and_then(|text| text.trim().parse::<u32>().ok());
                        if let (Some(pid), Some(descendant)) = (read_pid("pid"), read_pid("descendant-pid")) {
                            break (pid, descendant);
                        }
                        tokio::time::sleep(Duration::from_millis(20)).await;
                    }
                } => pids,
            }
        }).await.expect("Muse fixture did not become ready for cancellation");
        drop(stream);
        tokio::time::timeout(Duration::from_secs(3), async {
            while gosling::subprocess::process_is_alive(pid).await
                || gosling::subprocess::process_is_alive(descendant).await
            {
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await
        .expect("Muse child survived cancellation");
    }
}
