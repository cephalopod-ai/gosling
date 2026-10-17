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
    assert!(MuseCodeProvider::metadata().config_keys.is_empty());
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
                        "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"Hello\"},\"finish_reason\":null}]}\n\n",
                        "data: {\"choices\":[{\"index\":0,\"delta\":{},\"finish_reason\":\"stop\"}],\"usage\":{\"prompt_tokens\":10,\"completion_tokens\":2,\"total_tokens\":12}}\n\n",
                        "data: [DONE]\n\n"
                    ))
                } else {
                    ResponseTemplate::new(200).set_body_json(json!({
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
        std::fs::write(&command, format!("#!/bin/sh\nset -eu\n{body}\n")).unwrap();
        std::fs::set_permissions(&command, std::fs::Permissions::from_mode(0o700)).unwrap();
        command
    }

    async fn provider(
        root: &Path,
        command: &Path,
    ) -> (MuseCodeProvider, env_lock::EnvGuard<'static>) {
        let guard = env_lock::lock_env([
            ("GOSLING_PATH_ROOT", Some(root.to_str().unwrap())),
            ("MUSE_CODE_COMMAND", Some(command.to_str().unwrap())),
            ("GOSLING_MODE", Some("auto")),
            ("META_API_KEY", Some("must-not-reach-muse")),
            ("MODEL_API_KEY", Some("must-not-reach-muse-either")),
        ]);
        let provider =
            MuseCodeProvider::from_env_with_working_dir(vec![], root.to_path_buf(), None)
                .await
                .unwrap();
        (provider, guard)
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
        script(root.path(), "cat events.jsonl\nexit 1");
        std::fs::write(root.path().join("events.jsonl"), successful_events()).unwrap();
        assert!(provider
            .complete(
                &ModelConfig::new("muse-spark-1.3"),
                "System",
                &[Message::user().with_text("Hi")],
                &[]
            )
            .await
            .is_err());
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
    async fn model_discovery_precedes_session_mode_selection_without_launching_muse() {
        let root = tempfile::tempdir().unwrap();
        let command = script(root.path(), "touch was-launched\nexit 1");
        let _env = env_lock::lock_env([
            ("GOSLING_PATH_ROOT", Some(root.path().to_str().unwrap())),
            ("MUSE_CODE_COMMAND", Some(command.to_str().unwrap())),
            ("GOSLING_MODE", Some("approve")),
        ]);
        let provider =
            MuseCodeProvider::from_env_with_working_dir(vec![], root.path().to_path_buf(), None)
                .await
                .unwrap();
        assert!(provider
            .fetch_supported_models()
            .await
            .unwrap()
            .iter()
            .any(|name| name == "muse-spark-1.3"));
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
        assert!(
            tokio::time::timeout(Duration::from_millis(250), stream.next())
                .await
                .is_err()
        );
        let pid: u32 = std::fs::read_to_string(root.path().join("pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let descendant: u32 = std::fs::read_to_string(root.path().join("descendant-pid"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
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
