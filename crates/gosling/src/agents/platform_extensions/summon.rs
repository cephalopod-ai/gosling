use crate::agents::extension::PlatformExtensionContext;
use crate::agents::mcp_client::{Error, McpClientTrait};
use crate::agents::subagent_handler::{
    run_subagent_task, OnMessageCallback, SubagentRunParams, SubagentTask,
};
use crate::agents::subagent_task_config::{TaskConfig, DEFAULT_SUBAGENT_MAX_TURNS};
use crate::agents::tool_execution::ToolCallContext;
use crate::agents::AgentConfig;
use crate::config::paths::Paths;
use crate::config::{Config, GoslingMode};
use crate::providers;
use crate::session::extension_data::EnabledExtensionsState;
use crate::session::SessionType;
use crate::sources::parse_frontmatter;
use crate::utils::safe_truncate;
use anyhow::Result;
use async_trait::async_trait;
use gosling_sdk_types::custom_requests::{SourceEntry, SourceType};
use rmcp::model::{
    CallToolResult, Content, Implementation, InitializeResult, JsonObject, ListToolsResult, Meta,
    ServerCapabilities, ServerNotification, Tool,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::{mpsc, Mutex, OwnedSemaphorePermit, Semaphore};

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{info, warn};

mod delegate_config;
mod delegation;
mod loading;
mod source_discovery;
mod task_tracking;

#[cfg(test)]
use delegate_config::resolve_working_dir;
pub use source_discovery::discover_filesystem_sources;
use source_discovery::{
    build_instructions_with_context, build_subagent_instructions, delegate_authority_summary,
    kind_plural, resolve_delegate_extensions, validate_capability_policy, AgentMetadata,
    DelegateSpec,
};
#[cfg(test)]
use source_discovery::{parse_agent_content, scan_agents_from_dir, DelegateCapabilityPolicy};
use task_tracking::{current_epoch_millis, is_session_id, max_background_tasks, round_duration};
pub use task_tracking::{BackgroundTask, CompletedTask};

// This compatibility facade preserves the original `summon` module path and public names while
// cohesive implementation seams live in child modules.

pub static EXTENSION_NAME: &str = "summon";

const SUBAGENT_DESCRIPTION_BUDGET: usize = 160;

const TASK_LABEL_BUDGET: usize = 60;

#[derive(Debug, Default, Deserialize)]
pub struct DelegateParams {
    pub instructions: Option<String>,
    pub source: Option<String>,
    pub extensions: Option<Vec<String>>,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub temperature: Option<f32>,
    pub max_turns: Option<usize>,
    pub context: Option<String>,
    pub working_dir: Option<String>,
    #[serde(default)]
    pub r#async: bool,
}

pub struct SummonClient {
    info: InitializeResult,
    context: PlatformExtensionContext,
    source_cache: Mutex<Option<(Instant, PathBuf, Vec<SourceEntry>)>>,
    background_task_slots: Arc<Semaphore>,
    max_background_tasks: usize,
    background_tasks: Mutex<HashMap<String, BackgroundTask>>,
    completed_tasks: Mutex<HashMap<String, CompletedTask>>,
    notification_subscribers: Arc<Mutex<Vec<mpsc::Sender<ServerNotification>>>>,
}

impl SummonClient {
    pub fn new(context: PlatformExtensionContext) -> Result<Self> {
        Self::with_background_task_limit(context, max_background_tasks())
    }

    async fn handle_async_delegate(
        &self,
        session_id: &str,
        params: DelegateParams,
    ) -> Result<(Vec<Content>, String), String> {
        let task_slot = self.try_reserve_background_task_slot()?;

        let session = self
            .context
            .session_manager
            .get_session(session_id, false)
            .await
            .map_err(|e| format!("Failed to get session: {}", e))?;

        let working_dir = session.working_dir.clone();
        let spec = self.build_delegate_spec(&params, &working_dir).await?;

        let task_config = self
            .build_task_config(&params, &spec, &session)
            .await
            .map_err(|e| format!("Failed to build task config: {}", e))?;
        let authority_summary = delegate_authority_summary(&task_config.extensions);

        let description = safe_truncate(&Self::get_task_description(&params), TASK_LABEL_BUDGET);

        // Subagents must use Auto until get_agent_messages forwards
        // ActionRequired messages to the parent. Until then, any mode
        // that requires approval will hang on the subagent's confirmation_rx.
        let agent_config = AgentConfig::new(
            self.context.session_manager.clone(),
            crate::config::permission::PermissionManager::instance(),
            GoslingMode::Auto,
            true, // disable session naming for subagents
            crate::agents::GoslingPlatform::GoslingCli,
        )
        .with_code_execution_runtime(self.context.code_execution_runtime)
        .with_use_login_shell_path(self.context.use_login_shell_path);

        let subagent_session = self
            .context
            .session_manager
            .create_session(
                task_config.parent_working_dir.clone(),
                description.clone(),
                SessionType::SubAgent,
                GoslingMode::Auto,
            )
            .await
            .map_err(|e| format!("Failed to create subagent session: {}", e))?;

        let task_id = subagent_session.id.clone();

        let turns = Arc::new(AtomicU32::new(0));
        let last_activity = Arc::new(AtomicU64::new(current_epoch_millis()));

        let turns_clone = Arc::clone(&turns);
        let last_activity_clone = Arc::clone(&last_activity);

        let on_message: OnMessageCallback = Arc::new(move |_msg| {
            turns_clone.fetch_add(1, Ordering::Relaxed);
            last_activity_clone.store(current_epoch_millis(), Ordering::Relaxed);
        });

        let task_token = CancellationToken::new();
        let task_token_clone = task_token.clone();

        let notification_buffer = Arc::new(Mutex::new(Vec::new()));

        let (notif_tx, notif_rx) = tokio::sync::mpsc::unbounded_channel::<ServerNotification>();
        Self::spawn_notification_bridge(
            notif_rx,
            Arc::clone(&self.notification_subscribers),
            Arc::clone(&notification_buffer),
        );

        let mut background_tasks = self.background_tasks.lock().await;
        let handle = tokio::spawn(async move {
            run_subagent_task(SubagentRunParams {
                config: agent_config,
                task: SubagentTask {
                    instructions: spec.instructions.clone(),
                    prompt: spec.prompt.clone(),
                },
                task_config,
                return_last_only: true,
                session_id: subagent_session.id,
                cancellation_token: Some(task_token_clone),
                on_message: Some(on_message),
                notification_tx: Some(notif_tx),
            })
            .await
        });

        let task = BackgroundTask {
            id: task_id.clone(),
            description: description.clone(),
            started_at: Instant::now(),
            turns,
            last_activity,
            handle,
            cancellation_token: task_token,
            notification_buffer,
            _slot: task_slot,
        };

        background_tasks.insert(task_id.clone(), task);

        let content = vec![Content::text(format!(
            "Task {} started in background: \"{}\"\n\
             Resolved delegate authority: extensions = {}.\n\
             Continue with other work. When you need the result, use load(source: \"{}\").",
            task_id, description, authority_summary, task_id
        ))];
        Ok((content, task_id))
    }
}

#[async_trait]
impl McpClientTrait for SummonClient {
    async fn list_tools(
        &self,
        session_id: &str,
        _next_cursor: Option<String>,
        _cancellation_token: CancellationToken,
    ) -> Result<ListToolsResult, Error> {
        self.cleanup_completed_tasks().await;

        let is_subagent = self
            .context
            .session_manager
            .get_session(session_id, false)
            .await
            .map(|s| s.session_type == SessionType::SubAgent)
            .unwrap_or(false);

        let mut tools = vec![self.create_load_tool()];

        if !is_subagent {
            tools.push(self.create_delegate_tool());
        }

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        ctx: &ToolCallContext,
        name: &str,
        arguments: Option<JsonObject>,
        cancellation_token: CancellationToken,
    ) -> Result<CallToolResult, Error> {
        let session_id = &ctx.session_id;
        match name {
            "load" => match self.handle_load(session_id, arguments).await {
                Ok(result) => Ok(result),
                Err(error) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Error: {}",
                    error
                ))])),
            },
            "delegate" => {
                match self
                    .handle_delegate(session_id, arguments, cancellation_token)
                    .await
                {
                    Ok(result) => Ok(result),
                    Err(error) => Ok(CallToolResult::error(vec![Content::text(format!(
                        "Error: {}",
                        error
                    ))])),
                }
            }
            _ => Ok(CallToolResult::error(vec![Content::text(format!(
                "Error: Unknown tool: {}",
                name
            ))])),
        }
    }

    fn get_info(&self) -> Option<&InitializeResult> {
        Some(&self.info)
    }

    fn get_instructions(&self) -> Option<String> {
        let instructions = build_subagent_instructions(self.context.session.as_deref());
        if instructions.is_empty() {
            None
        } else {
            Some(instructions)
        }
    }

    async fn subscribe(&self) -> mpsc::Receiver<ServerNotification> {
        let (tx, rx) = mpsc::channel(16);
        self.notification_subscribers.lock().await.push(tx);
        rx
    }

    async fn get_moim(&self, _session_id: &str) -> Option<String> {
        self.cleanup_completed_tasks().await;

        let running = self.background_tasks.lock().await;
        let completed = self.completed_tasks.lock().await;

        if running.is_empty() && completed.is_empty() {
            return None;
        }

        let mut lines = vec!["Background tasks:".to_string()];
        let now = current_epoch_millis();

        let mut sorted_running: Vec<_> = running.values().collect();
        sorted_running.sort_by_key(|t| &t.id);

        for task in sorted_running {
            let elapsed = task.started_at.elapsed();
            let idle_ms = now.saturating_sub(task.last_activity.load(Ordering::Relaxed));

            lines.push(format!(
                "• {}: \"{}\" - running {}, {} turns, idle {}",
                task.id,
                task.description,
                round_duration(elapsed),
                task.turns.load(Ordering::Relaxed),
                round_duration(Duration::from_millis(idle_ms)),
            ));
        }

        let mut sorted_completed: Vec<_> = completed.values().collect();
        sorted_completed.sort_by_key(|t| &t.id);

        for task in sorted_completed {
            let status = if task.result.is_ok() {
                "completed"
            } else {
                "failed"
            };
            lines.push(format!(
                "• {}: \"{}\" - {} in {} ({} turns) - use load(\"{}\") to get result",
                task.id,
                task.description,
                status,
                round_duration(task.duration),
                task.turns_taken,
                task.id
            ));
        }

        if !running.is_empty() {
            lines.push(
                "\n→ Use load(source: \"<id>\") to wait for a task, or load(source: \"<id>\", cancel: true) to stop it"
                    .to_string(),
            );
        }

        Some(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::collections::{HashMap, HashSet};
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn create_test_context() -> PlatformExtensionContext {
        PlatformExtensionContext {
            extension_manager: None,
            session_manager: Arc::new(crate::session::SessionManager::instance()),
            session: None,
            use_login_shell_path: false,
            code_execution_runtime: crate::config::CodeExecutionRuntime::Enabled,
        }
    }

    fn test_extension(name: &str) -> crate::agents::ExtensionConfig {
        crate::agents::ExtensionConfig::Builtin {
            name: name.to_string(),
            description: name.to_string(),
            display_name: None,
            timeout: None,
            bundled: None,
            available_tools: Vec::new(),
        }
    }

    #[test]
    fn test_agent_frontmatter_parsing() {
        let agent = r#"---
name: reviewer
model: sonnet
---
You review code."#;
        let source = parse_agent_content(agent, Path::new(""), false).unwrap();
        assert_eq!(source.name, "reviewer");
        assert!(source.description.contains("sonnet"));
    }

    #[test]
    fn test_delegate_capability_policy_is_versioned_and_deduplicated() {
        let extensions = validate_capability_policy(Some(DelegateCapabilityPolicy {
            version: 1,
            extensions: vec![
                "developer".to_string(),
                "summarize".to_string(),
                "developer".to_string(),
            ],
        }))
        .unwrap();
        assert_eq!(extensions, vec!["developer", "summarize"]);

        let error = validate_capability_policy(Some(DelegateCapabilityPolicy {
            version: 2,
            extensions: Vec::new(),
        }))
        .unwrap_err();
        assert!(error.contains("version 2"));
    }

    #[test]
    fn test_adhoc_delegate_defaults_to_no_extensions() {
        let parent = vec![test_extension("developer"), test_extension("summarize")];
        let resolved = resolve_delegate_extensions(parent, &DelegateSpec::default(), None).unwrap();
        assert!(resolved.is_empty());
    }

    #[test]
    fn test_source_delegate_is_bounded_by_role_and_explicit_request() {
        let parent = vec![
            test_extension("developer"),
            test_extension("summarize"),
            test_extension("summon"),
        ];
        let spec = DelegateSpec {
            role_extensions: Some(vec!["developer".to_string(), "summarize".to_string()]),
            ..Default::default()
        };

        let role_default = resolve_delegate_extensions(parent.clone(), &spec, None).unwrap();
        assert_eq!(
            role_default
                .iter()
                .map(|ext| ext.name())
                .collect::<Vec<_>>(),
            vec!["developer", "summarize"]
        );

        let narrowed =
            resolve_delegate_extensions(parent.clone(), &spec, Some(&["summarize".to_string()]))
                .unwrap();
        assert_eq!(narrowed[0].name(), "summarize");

        let error =
            resolve_delegate_extensions(parent, &spec, Some(&["summon".to_string()])).unwrap_err();
        assert!(error.contains("outside the role capability policy"));
    }

    #[test]
    fn test_delegate_extension_must_exist_in_parent_session() {
        let error = resolve_delegate_extensions(
            vec![test_extension("developer")],
            &DelegateSpec::default(),
            Some(&["summarize".to_string()]),
        )
        .unwrap_err();
        assert!(error.contains("unavailable in the parent session"));
    }

    #[tokio::test]
    async fn test_legacy_source_without_capability_policy_gets_no_extensions() {
        let temp_dir = TempDir::new().unwrap();
        let agents = temp_dir.path().join(".gosling/agents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(
            agents.join("reviewer.md"),
            "---\nname: reviewer\n---\nReview without tools.",
        )
        .unwrap();

        let client = SummonClient::new(create_test_context()).unwrap();
        let params = DelegateParams {
            source: Some("reviewer".to_string()),
            ..Default::default()
        };
        let spec = client
            .build_delegate_spec(&params, temp_dir.path())
            .await
            .unwrap();
        assert_eq!(spec.role_extensions, Some(Vec::new()));
    }

    #[tokio::test]
    async fn test_repo_committed_agent_capability_policy_is_ignored() {
        // AOC-GOS-004: a repo-committed agent file cannot grant itself
        // extensions by declaring a `capabilities` policy, even one the
        // parent session happens to have enabled.
        let temp_dir = TempDir::new().unwrap();
        let agents = temp_dir.path().join(".gosling/agents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(
            agents.join("helper.md"),
            "---\nname: helper\ncapabilities:\n  version: 1\n  extensions: [developer]\n---\nHelp.",
        )
        .unwrap();

        let client = SummonClient::new(create_test_context()).unwrap();
        let params = DelegateParams {
            source: Some("helper".to_string()),
            ..Default::default()
        };
        let spec = client
            .build_delegate_spec(&params, temp_dir.path())
            .await
            .unwrap();
        assert_eq!(spec.role_extensions, Some(Vec::new()));

        let resolved =
            resolve_delegate_extensions(vec![test_extension("developer")], &spec, None).unwrap();
        assert!(resolved.is_empty());
    }

    #[test]
    fn test_global_agent_capability_policy_is_honored() {
        // Companion to the untrusted-source test above: an operator-authored
        // global agent file (`source.global == true`) is still trusted to
        // declare a capability policy, so `build_spec_from_agent` itself
        // must honor it rather than only the discovery-layer flag.
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("helper.md");
        fs::write(
            &path,
            "---\nname: helper\ncapabilities:\n  version: 1\n  extensions: [developer]\n---\nHelp.",
        )
        .unwrap();

        let source = SourceEntry {
            source_type: SourceType::Agent,
            name: "helper".to_string(),
            description: "Global helper".to_string(),
            content: "Help.".to_string(),
            path: path.to_string_lossy().into_owned(),
            global: true,
            writable: true,
            supporting_files: Vec::new(),
            properties: std::collections::HashMap::new(),
        };

        let client = SummonClient::new(create_test_context()).unwrap();
        let spec = client
            .build_spec_from_agent(&source, &DelegateParams::default())
            .unwrap();
        assert_eq!(spec.role_extensions, Some(vec!["developer".to_string()]));
    }

    #[test]
    fn test_resolve_working_dir_relative_subdir() {
        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path().canonicalize().unwrap();
        let subdir = parent.join("sub");
        fs::create_dir(&subdir).unwrap();

        let resolved = resolve_working_dir(&parent, "sub").unwrap();
        assert_eq!(resolved, subdir.canonicalize().unwrap());
    }

    #[test]
    fn test_resolve_working_dir_rejects_traversal_outside_parent() {
        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path().join("parent");
        let sibling = temp_dir.path().join("sibling");
        fs::create_dir(&parent).unwrap();
        fs::create_dir(&sibling).unwrap();

        let err = resolve_working_dir(&parent, "../sibling").unwrap_err();
        assert!(
            err.to_string()
                .contains("outside the parent session directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_resolve_working_dir_rejects_file_path() {
        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path().canonicalize().unwrap();
        let file = parent.join("a.txt");
        fs::write(&file, "hello").unwrap();

        let err = resolve_working_dir(&parent, "a.txt").unwrap_err();
        assert!(
            err.to_string().contains("is not a directory"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_resolve_working_dir_rejects_nonexistent_path() {
        let temp_dir = TempDir::new().unwrap();
        let parent = temp_dir.path().canonicalize().unwrap();

        let err = resolve_working_dir(&parent, "does-not-exist").unwrap_err();
        assert!(
            err.to_string().contains("could not be resolved"),
            "unexpected error: {err}"
        );
    }
    #[test]
    fn test_agent_scan_skips_non_agent_markdown() {
        let temp_dir = TempDir::new().unwrap();
        let agents_dir = temp_dir.path().join("agents");
        fs::create_dir_all(&agents_dir).unwrap();
        fs::write(
            agents_dir.join("README.md"),
            "---\ntitle: Notes\n---\nThis is not an agent.",
        )
        .unwrap();
        fs::write(
            agents_dir.join("notes.md"),
            "---\nauthor: someone\ntags: [docs]\n---\nJust documentation.",
        )
        .unwrap();
        fs::write(
            agents_dir.join("reviewer.md"),
            "---\nname: reviewer\nmodel: sonnet\n---\nYou review code.",
        )
        .unwrap();
        fs::write(agents_dir.join("plain.md"), "No frontmatter at all.").unwrap();
        fs::write(
            agents_dir.join("broken.md"),
            "---\nname: [unterminated\n---\nBroken YAML.",
        )
        .unwrap();

        let mut sources = Vec::new();
        let mut seen = HashSet::new();
        scan_agents_from_dir(&agents_dir, &mut sources, &mut seen, false);

        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].name, "reviewer");
    }

    #[tokio::test]
    async fn test_discover_agents() {
        let temp_dir = TempDir::new().unwrap();

        let agents = temp_dir.path().join(".gosling/agents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(
            agents.join("reviewer.md"),
            "---\nname: reviewer\nmodel: sonnet\ndescription: Code reviewer\n---\nYou review code.",
        )
        .unwrap();

        let sources = discover_filesystem_sources(temp_dir.path());

        let agent = sources
            .iter()
            .find(|s| s.name == "reviewer" && s.source_type == SourceType::Agent)
            .unwrap();
        assert_eq!(agent.description, "Code reviewer");
        assert!(agent.content.contains("You review code"));
    }

    #[tokio::test]
    async fn test_agent_deduplication_local_wins() {
        let temp_dir = TempDir::new().unwrap();

        let local = temp_dir.path().join(".gosling/agents");
        fs::create_dir_all(&local).unwrap();
        fs::write(
            local.join("reviewer.md"),
            "---\nname: reviewer\ndescription: Local reviewer\n---\nlocal steps",
        )
        .unwrap();

        let also_local = temp_dir.path().join(".agents/agents");
        fs::create_dir_all(&also_local).unwrap();
        fs::write(
            also_local.join("reviewer.md"),
            "---\nname: reviewer\ndescription: Agents reviewer\n---\nagents steps",
        )
        .unwrap();

        let sources = discover_filesystem_sources(temp_dir.path());

        let reviewers: Vec<_> = sources.iter().filter(|s| s.name == "reviewer").collect();
        assert_eq!(reviewers.len(), 1);
    }

    #[tokio::test]
    async fn test_load_agent_source() {
        let temp_dir = TempDir::new().unwrap();

        let agents = temp_dir.path().join(".gosling/agents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(
            agents.join("reviewer.md"),
            "---\nname: reviewer\nmodel: sonnet\ndescription: Code reviewer\n---\nYou review code carefully.",
        )
        .unwrap();

        let client = SummonClient::new(create_test_context()).unwrap();
        let result = client
            .handle_load_source("reviewer", temp_dir.path())
            .await
            .unwrap();

        let text = &result[0].as_text().expect("expected text content").text;
        assert!(text.contains("reviewer"));
        assert!(text.contains("You review code carefully"));
        assert!(text.contains("now available in your context"));
    }

    #[tokio::test]
    async fn test_load_nonexistent_source_suggests_similar() {
        let temp_dir = TempDir::new().unwrap();

        let agents = temp_dir.path().join(".gosling/agents");
        fs::create_dir_all(&agents).unwrap();
        fs::write(
            agents.join("deploy.md"),
            "---\nname: deploy\ndescription: Deploy to production\n---\nsteps",
        )
        .unwrap();

        let client = SummonClient::new(create_test_context()).unwrap();
        let err = client
            .handle_load_source("deploy-prod", temp_dir.path())
            .await
            .unwrap_err();

        assert!(err.contains("not found"));
        assert!(err.contains("deploy"), "should suggest 'deploy': {}", err);
    }

    #[tokio::test]
    async fn test_load_completely_unknown_source() {
        let temp_dir = TempDir::new().unwrap();

        let client = SummonClient::new(create_test_context()).unwrap();
        let err = client
            .handle_load_source("zzz-nonexistent", temp_dir.path())
            .await
            .unwrap_err();

        assert!(err.contains("not found"));
        assert!(err.contains("Use load()"));
    }

    #[tokio::test]
    async fn test_client_tools_and_unknown_tool() {
        let client = SummonClient::new(create_test_context()).unwrap();

        let result = client
            .list_tools("test", None, CancellationToken::new())
            .await
            .unwrap();
        let names: Vec<_> = result.tools.iter().map(|t| t.name.as_ref()).collect();
        assert!(names.contains(&"load") && names.contains(&"delegate"));

        let ctx = ToolCallContext::new("test".to_string(), None, None);
        let result = client
            .call_tool(&ctx, "unknown", None, CancellationToken::new())
            .await
            .unwrap();
        assert!(result.is_error.unwrap_or(false));
    }

    #[test]
    fn test_duration_rounding_for_moim() {
        assert_eq!(round_duration(Duration::from_secs(5)), "0s");
        assert_eq!(round_duration(Duration::from_secs(15)), "10s");
        assert_eq!(round_duration(Duration::from_secs(59)), "50s");

        assert_eq!(round_duration(Duration::from_secs(60)), "1m");
        assert_eq!(round_duration(Duration::from_secs(90)), "1m");
        assert_eq!(round_duration(Duration::from_secs(120)), "2m");
    }

    #[test]
    fn test_task_description_formatting() {
        let make_params = |source: Option<&str>, instructions: Option<&str>| DelegateParams {
            source: source.map(String::from),
            instructions: instructions.map(String::from),
            ..Default::default()
        };

        assert_eq!(
            SummonClient::get_task_description(&make_params(Some("reviewer"), None)),
            "reviewer"
        );
        assert_eq!(
            SummonClient::get_task_description(&make_params(None, Some("do stuff"))),
            "do stuff"
        );
        assert_eq!(
            SummonClient::get_task_description(&make_params(Some("r"), Some("task"))),
            "r: task"
        );
        assert_eq!(
            SummonClient::get_task_description(&make_params(None, None)),
            "Unknown task"
        );
    }

    #[tokio::test]
    async fn test_context_injected_into_adhoc_spec() {
        let temp_dir = TempDir::new().unwrap();
        let client = SummonClient::new(create_test_context()).unwrap();

        let params = DelegateParams {
            instructions: Some("do the task".to_string()),
            context: Some("background info".to_string()),
            ..Default::default()
        };

        let spec = client
            .build_delegate_spec(&params, temp_dir.path())
            .await
            .unwrap();

        assert_eq!(
            spec.instructions.as_deref(),
            Some("# Reference Context\n\nbackground info")
        );
        assert_eq!(spec.prompt.as_deref(), Some("do the task"));
    }

    #[test]
    fn test_build_instructions_with_context_wraps_existing_instructions() {
        assert_eq!(
            build_instructions_with_context("background info", "Run deploy steps"),
            "# Reference Context\n\nbackground info\n\n# Task Instructions\n\nRun deploy steps"
        );
        assert_eq!(
            build_instructions_with_context("background info", ""),
            "# Reference Context\n\nbackground info"
        );
    }

    #[test]
    fn test_validate_delegate_params_rejects_zero_max_turns() {
        let context = create_test_context();
        let client = SummonClient::new(context).unwrap();

        let params = DelegateParams {
            instructions: Some("do something".to_string()),
            max_turns: Some(0),
            ..Default::default()
        };
        let result = client.validate_delegate_params(&params);
        assert_eq!(result, Err("'max_turns' must be at least 1".to_string()));
    }

    #[test]
    fn test_validate_delegate_params_accepts_positive_max_turns() {
        let context = create_test_context();
        let client = SummonClient::new(context).unwrap();

        let params = DelegateParams {
            instructions: Some("do something".to_string()),
            max_turns: Some(5),
            ..Default::default()
        };
        assert!(client.validate_delegate_params(&params).is_ok());
    }

    #[test]
    #[serial]
    fn test_resolve_max_turns_falls_back_to_env_var() {
        let context = create_test_context();
        let client = SummonClient::new(context).unwrap();

        std::env::set_var("GOSLING_SUBAGENT_MAX_TURNS", "7");
        let result = client.resolve_max_turns();
        std::env::remove_var("GOSLING_SUBAGENT_MAX_TURNS");

        assert_eq!(
            result, 7,
            "should fall back to GOSLING_SUBAGENT_MAX_TURNS env var"
        );
    }

    #[test]
    #[serial]
    fn test_resolve_max_turns_falls_back_to_default() {
        let context = create_test_context();
        let client = SummonClient::new(context).unwrap();

        std::env::remove_var("GOSLING_SUBAGENT_MAX_TURNS");
        let result = client.resolve_max_turns();

        assert_eq!(
            result,
            crate::agents::subagent_task_config::DEFAULT_SUBAGENT_MAX_TURNS,
            "should fall back to DEFAULT_SUBAGENT_MAX_TURNS"
        );
    }

    fn empty_spec() -> DelegateSpec {
        DelegateSpec::default()
    }

    const PARENT_MODEL: &str = "claude-3-5-sonnet-20241022";
    const OVERRIDE_MODEL: &str = "claude-opus-4-6";
    const PROVIDER: &str = "anthropic";

    fn session_with(parent: gosling_providers::model::ModelConfig) -> crate::session::Session {
        crate::session::Session {
            provider_name: Some(PROVIDER.to_string()),
            model_config: Some(parent),
            ..Default::default()
        }
    }

    fn resolve_with_override(
        model: Option<&str>,
        parent: gosling_providers::model::ModelConfig,
    ) -> gosling_providers::model::ModelConfig {
        let client = SummonClient::new(create_test_context()).unwrap();
        let params = DelegateParams {
            model: model.map(String::from),
            ..Default::default()
        };
        client
            .resolve_model_config(&params, &empty_spec(), &session_with(parent), PROVIDER)
            .expect("resolve_model_config")
    }

    fn parent_config() -> gosling_providers::model::ModelConfig {
        gosling_providers::model::ModelConfig::new(PARENT_MODEL).with_canonical_limits(PROVIDER)
    }

    #[tokio::test]
    #[serial]
    async fn test_resolve_model_config_applies_canonical_limits_to_overridden_model() {
        let _env = env_lock::lock_env([
            ("GOSLING_CONTEXT_LIMIT", None::<&str>),
            ("GOSLING_MAX_TOKENS", None::<&str>),
            ("GOSLING_SUBAGENT_MODEL", None::<&str>),
        ]);

        let parent = parent_config();
        let overridden = gosling_providers::model::ModelConfig::new(OVERRIDE_MODEL)
            .with_canonical_limits(PROVIDER);
        assert_ne!(parent.context_limit, overridden.context_limit);
        assert_ne!(parent.reasoning, overridden.reasoning);

        let resolved = resolve_with_override(Some(OVERRIDE_MODEL), parent);

        assert_eq!(resolved.model_name, OVERRIDE_MODEL);
        assert_eq!(resolved.context_limit, overridden.context_limit);
        assert_eq!(resolved.max_tokens, overridden.max_tokens);
        assert_eq!(resolved.reasoning, overridden.reasoning);
    }

    #[tokio::test]
    #[serial]
    async fn test_resolve_model_config_preserves_parent_request_params_on_override() {
        let _env = env_lock::lock_env([
            ("GOSLING_CONTEXT_LIMIT", None::<&str>),
            ("GOSLING_MAX_TOKENS", None::<&str>),
            ("GOSLING_SUBAGENT_MODEL", None::<&str>),
        ]);

        let mut parent = parent_config();
        parent.request_params = Some(HashMap::from([(
            "anthropic_beta".to_string(),
            serde_json::json!("custom-beta-header"),
        )]));

        let resolved = resolve_with_override(Some(OVERRIDE_MODEL), parent);

        assert_eq!(
            resolved
                .request_params
                .as_ref()
                .and_then(|p| p.get("anthropic_beta")),
            Some(&serde_json::json!("custom-beta-header")),
        );
    }

    fn extract_text(content: &Content) -> &str {
        use rmcp::model::RawContent;
        match &content.raw {
            RawContent::Text(t) => t.text.as_str(),
            _ => panic!("Expected text content"),
        }
    }

    #[test]
    fn test_is_session_id() {
        assert!(is_session_id("20260204_1"));
        assert!(is_session_id("20260204_42"));
        assert!(is_session_id("20260204_999"));
        assert!(!is_session_id("task_12345_0001"));
        assert!(!is_session_id("my-agent"));
        assert!(!is_session_id("2026020_1"));
        assert!(!is_session_id("20260204"));
    }

    #[tokio::test]
    async fn background_task_slot_reservation_is_atomic() {
        async fn attempt(
            client: &SummonClient,
            start: Arc<tokio::sync::Barrier>,
            winner_ready: Arc<tokio::sync::Barrier>,
            mut release: tokio::sync::watch::Receiver<bool>,
        ) -> bool {
            start.wait().await;
            let Ok(_slot) = client.try_reserve_background_task_slot() else {
                return false;
            };
            winner_ready.wait().await;
            release.changed().await.unwrap();
            true
        }

        let client = SummonClient::with_background_task_limit(create_test_context(), 1).unwrap();
        let start = Arc::new(tokio::sync::Barrier::new(3));
        let winner_ready = Arc::new(tokio::sync::Barrier::new(2));
        let (release_tx, release_rx) = tokio::sync::watch::channel(false);

        let first = attempt(
            &client,
            Arc::clone(&start),
            Arc::clone(&winner_ready),
            release_rx.clone(),
        );
        let second = attempt(
            &client,
            Arc::clone(&start),
            Arc::clone(&winner_ready),
            release_rx,
        );
        let driver = async {
            start.wait().await;
            winner_ready.wait().await;
            release_tx.send(true).unwrap();
        };

        let (first_reserved, second_reserved, ()) = tokio::join!(first, second, driver);
        assert_ne!(first_reserved, second_reserved);
    }

    #[tokio::test]
    async fn test_async_task_result_lifecycle() {
        let client = SummonClient::new(create_test_context()).unwrap();
        let temp_dir = TempDir::new().unwrap();

        let result = client
            .handle_load_task_result("20260204_999", false, false)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));

        {
            use crate::agents::subagent_handler::create_tool_notification;
            use crate::conversation::message::MessageContent;
            use rmcp::model::CallToolRequestParams;

            let tool_call = CallToolRequestParams::new("developer__shell").with_arguments(
                serde_json::json!({"command": "ls"})
                    .as_object()
                    .unwrap()
                    .clone(),
            );
            let content = MessageContent::tool_request("req1", Ok(tool_call));
            let notif = create_tool_notification(&content, "20260204_1").unwrap();

            let buffer = Arc::new(Mutex::new(vec![notif]));

            let mut running = client.background_tasks.lock().await;
            running.insert(
                "20260204_1".to_string(),
                BackgroundTask {
                    id: "20260204_1".to_string(),
                    description: "Running task".to_string(),
                    started_at: Instant::now(),
                    turns: Arc::new(AtomicU32::new(2)),
                    last_activity: Arc::new(AtomicU64::new(current_epoch_millis())),
                    handle: tokio::spawn(async {
                        tokio::time::sleep(Duration::from_millis(50)).await;
                        Ok("done".to_string())
                    }),
                    cancellation_token: CancellationToken::new(),
                    notification_buffer: buffer,
                    _slot: client.try_reserve_background_task_slot().unwrap(),
                },
            );
        }

        let mut subscriber = client.subscribe().await;

        let result = client
            .handle_load_task_result("20260204_1", false, false)
            .await
            .expect("load should wait and return result");
        let text = extract_text(&result.content[0]);
        assert!(text.contains("Completed"));
        assert!(text.contains("done"));

        let notif = subscriber
            .try_recv()
            .expect("subscriber should receive buffered notification");
        if let ServerNotification::LoggingMessageNotification(log) = notif {
            let data = log.params.data.as_object().unwrap();
            assert_eq!(
                data.get("subagent_id").and_then(|v| v.as_str()),
                Some("20260204_1")
            );
        } else {
            panic!("expected logging notification");
        }

        {
            let mut completed = client.completed_tasks.lock().await;
            completed.insert(
                "20260204_2".to_string(),
                CompletedTask {
                    id: "20260204_2".to_string(),
                    description: "Successful task".to_string(),
                    result: Ok("Task completed successfully with output".to_string()),
                    turns_taken: 5,
                    duration: Duration::from_secs(60),
                    completed_at: Instant::now(),
                },
            );
            completed.insert(
                "20260204_3".to_string(),
                CompletedTask {
                    id: "20260204_3".to_string(),
                    description: "Failed task".to_string(),
                    result: Err("Something went wrong".to_string()),
                    turns_taken: 3,
                    duration: Duration::from_secs(30),
                    completed_at: Instant::now(),
                },
            );
        }

        let moim = client.get_moim("test").await.unwrap();
        assert!(moim.contains("20260204_2"));
        assert!(moim.contains("20260204_3"));
        assert!(moim.contains(r#"use load("20260204_2") to get result"#));
        assert!(moim.contains(r#"use load("20260204_3") to get result"#));

        let discovery = client.handle_load_discovery(temp_dir.path()).await.unwrap();
        let discovery_text = extract_text(&discovery[0]);
        assert!(discovery_text.contains("Completed Tasks (awaiting retrieval)"));
        assert!(discovery_text.contains("20260204_2"));
        assert!(discovery_text.contains("20260204_3"));

        let result = client
            .handle_load_task_result("20260204_2", false, false)
            .await
            .unwrap();
        let text = extract_text(&result.content[0]);
        assert!(text.contains("20260204_2"));
        assert!(text.contains("Successful task"));
        assert!(text.contains("✓ Completed"));
        assert!(text.contains("1m"));
        assert!(text.contains("5 turns"));
        assert!(text.contains("Task completed successfully with output"));
        assert_eq!(result.status, "completed");
        assert_eq!(result.turns, Some(5));

        assert!(!client
            .completed_tasks
            .lock()
            .await
            .contains_key("20260204_2"));

        let result = client
            .handle_load_task_result("20260204_3", false, false)
            .await
            .unwrap();
        let text = extract_text(&result.content[0]);
        assert!(text.contains("✗ Failed"));
        assert!(text.contains("Error: Something went wrong"));
        assert_eq!(result.status, "failed");

        let result = client
            .handle_load_task_result("20260204_3", false, false)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));

        // All tasks consumed -- moim should be empty
        assert!(client.get_moim("test").await.is_none());
    }

    #[tokio::test]
    async fn test_cancel_running_task() {
        let client = SummonClient::new(create_test_context()).unwrap();
        let token = CancellationToken::new();

        {
            let mut running = client.background_tasks.lock().await;
            running.insert(
                "20260204_1".to_string(),
                BackgroundTask {
                    id: "20260204_1".to_string(),
                    description: "Cancellable task".to_string(),
                    started_at: Instant::now(),
                    turns: Arc::new(AtomicU32::new(3)),
                    last_activity: Arc::new(AtomicU64::new(current_epoch_millis())),
                    handle: tokio::spawn(async {
                        tokio::time::sleep(Duration::from_secs(1000)).await;
                        Ok("should not see this".to_string())
                    }),
                    cancellation_token: token.clone(),
                    notification_buffer: Arc::new(Mutex::new(Vec::new())),
                    _slot: client.try_reserve_background_task_slot().unwrap(),
                },
            );
        }

        let result = client
            .handle_load_task_result("20260204_1", true, false)
            .await
            .unwrap();
        let text = extract_text(&result.content[0]);
        assert!(text.contains("Cancelled"));
        assert!(text.contains("20260204_1"));
        assert!(text.contains("Cancellable task"));
        assert_eq!(result.status, "cancelled");
        assert_eq!(result.turns, Some(3));
        assert!(token.is_cancelled());
        assert!(!client
            .background_tasks
            .lock()
            .await
            .contains_key("20260204_1"));
    }

    #[tokio::test]
    async fn test_peek_running_task() {
        let client = SummonClient::new(create_test_context()).unwrap();

        {
            let mut running = client.background_tasks.lock().await;
            running.insert(
                "20260204_1".to_string(),
                BackgroundTask {
                    id: "20260204_1".to_string(),
                    description: "Long running analysis".to_string(),
                    started_at: Instant::now(),
                    turns: Arc::new(AtomicU32::new(7)),
                    last_activity: Arc::new(AtomicU64::new(current_epoch_millis())),
                    handle: tokio::spawn(async {
                        tokio::time::sleep(Duration::from_secs(1000)).await;
                        Ok("eventual result".to_string())
                    }),
                    cancellation_token: CancellationToken::new(),
                    notification_buffer: Arc::new(Mutex::new(Vec::new())),
                    _slot: client.try_reserve_background_task_slot().unwrap(),
                },
            );
        }

        // Peek should return status without removing the task
        let result = client
            .handle_load_task_result("20260204_1", false, true)
            .await
            .unwrap();
        let text = extract_text(&result.content[0]);
        assert!(text.contains("Running"));
        assert!(text.contains("Long running analysis"));
        assert!(text.contains("7")); // turns taken

        // Task should still be in background_tasks (not consumed)
        assert!(client
            .background_tasks
            .lock()
            .await
            .contains_key("20260204_1"));
    }

    #[tokio::test]
    async fn test_peek_nonexistent_task() {
        let client = SummonClient::new(create_test_context()).unwrap();

        let result = client
            .handle_load_task_result("20260204_999", false, true)
            .await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[tokio::test]
    async fn test_peek_completed_task_returns_result() {
        let client = SummonClient::new(create_test_context()).unwrap();

        {
            let mut completed = client.completed_tasks.lock().await;
            completed.insert(
                "20260204_1".to_string(),
                CompletedTask {
                    id: "20260204_1".to_string(),
                    description: "Finished task".to_string(),
                    result: Ok("final output".to_string()),
                    turns_taken: 4,
                    duration: Duration::from_secs(30),
                    completed_at: Instant::now(),
                },
            );
        }

        // Peek on a completed task should return the full result (same as non-peek)
        let result = client
            .handle_load_task_result("20260204_1", false, true)
            .await
            .unwrap();
        let text = extract_text(&result.content[0]);
        assert!(text.contains("Completed"));
        assert!(text.contains("final output"));

        // Peek must be non-destructive: the result is still retrievable afterwards.
        assert!(client
            .completed_tasks
            .lock()
            .await
            .contains_key("20260204_1"));
        let result = client
            .handle_load_task_result("20260204_1", false, false)
            .await
            .unwrap();
        assert!(extract_text(&result.content[0]).contains("final output"));
    }
}
