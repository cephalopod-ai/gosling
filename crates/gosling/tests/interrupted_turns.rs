//! A turn that stops before it finishes (process killed, client gone, approval
//! abandoned, lease lost) must never be sent again as part of a later prompt,
//! and its history must say that it stopped (GSL-PT-20260927-F10, A12, G122,
//! F14, S10, G129).

use anyhow::Result;
use async_trait::async_trait;
use futures::StreamExt;
use gosling::acp::server_factory::{AcpServer, AcpServerFactoryConfig};
use gosling::agents::{Agent, AgentConfig, AgentEvent, GoslingPlatform, SessionConfig};
use gosling::config::{GoslingMode, PermissionManager};
use gosling::conversation::message::{Message, MessageContent, MessageMetadata};
use gosling::providers::base::{stream_from_single_message, MessageStream, Provider};
use gosling::session::session_manager::{SessionManager, SessionType};
use gosling::session::{AcpPromptRunState, ExtensionState};
use gosling_providers::conversation::token_usage::{ProviderUsage, Usage};
use gosling_providers::errors::ProviderError;
use gosling_providers::model::ModelConfig;
use rmcp::model::{CallToolRequestParams, Role, Tool};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tempfile::TempDir;

const INTERRUPTED: &str = "Run interrupted before completion.";
const NEXT_PROMPT: &str = "What is 2+2? Reply with just the number.";

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

/// Answers every turn with "ok" and records what each agent turn was sent.
#[derive(Default)]
struct CapturingProvider {
    turns: Mutex<Vec<Vec<Message>>>,
}

#[async_trait]
impl Provider for CapturingProvider {
    async fn stream(
        &self,
        _model_config: &ModelConfig,
        _system_prompt: &str,
        messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        if !is_session_naming(messages) {
            self.turns.lock().unwrap().push(messages.to_vec());
        }
        Ok(stream_from_single_message(
            Message::assistant().with_text("ok"),
            usage(),
        ))
    }

    fn get_name(&self) -> &str {
        "mock-capturing"
    }
}

/// Streams the first part of a reply and then never finishes it.
struct StallingProvider;

#[async_trait]
impl Provider for StallingProvider {
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
        let first_chunk = futures::stream::once(async {
            Ok((
                Some(
                    Message::assistant()
                        .with_id("streaming-reply")
                        .with_text("LONG-START lorem ip"),
                ),
                None,
            ))
        });
        Ok(Box::pin(first_chunk.chain(futures::stream::pending())))
    }

    fn get_name(&self) -> &str {
        "mock-stalling"
    }
}

struct Fixture {
    _temp: TempDir,
    agent: Agent,
    session_id: String,
}

impl Fixture {
    async fn new(provider: Arc<dyn Provider>, history: Vec<Message>) -> Result<Self> {
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
                "interrupted-turns".to_string(),
                SessionType::User,
                GoslingMode::Auto,
            )
            .await?;
        for message in history {
            let message = if message.id.is_some() {
                message
            } else {
                message.with_generated_id()
            };
            session_manager.add_message(&session.id, &message).await?;
        }
        agent
            .update_provider(provider, ModelConfig::new("mock-model"), &session.id)
            .await?;
        Ok(Self {
            _temp: temp,
            agent,
            session_id: session.id,
        })
    }

    fn sessions(&self) -> &SessionManager {
        &self.agent.config.session_manager
    }

    fn session_config(&self) -> SessionConfig {
        SessionConfig {
            id: self.session_id.clone(),
            max_turns: Some(4),
            compacted_context: false,
            tail_limit: None,
        }
    }

    async fn prompt(&self, text: &str) -> Result<()> {
        let stream = self
            .agent
            .reply(Message::user().with_text(text), self.session_config(), None)
            .await?;
        tokio::pin!(stream);
        while let Some(event) = stream.next().await {
            let _: AgentEvent = event?;
        }
        Ok(())
    }

    async fn stored(&self) -> Result<Vec<Message>> {
        Ok(self
            .sessions()
            .get_session(&self.session_id, true)
            .await?
            .conversation
            .unwrap_or_default()
            .messages()
            .clone())
    }

    async fn set_run_state(&self, state: AcpPromptRunState) -> Result<()> {
        self.sessions()
            .merge_extension_state(
                &self.session_id,
                &format!(
                    "{}.{}",
                    AcpPromptRunState::EXTENSION_NAME,
                    AcpPromptRunState::VERSION
                ),
                state.to_value()?,
            )
            .await
    }

    async fn run_state(&self) -> Result<Option<AcpPromptRunState>> {
        let session = self.sessions().get_session(&self.session_id, false).await?;
        Ok(AcpPromptRunState::from_extension_data(
            &session.extension_data,
        ))
    }
}

fn texts(messages: &[Message]) -> Vec<String> {
    messages.iter().map(Message::as_concat_text).collect()
}

/// `role:text` for every message the provider was sent, tool content by id.
fn transcript(messages: &[Message]) -> Vec<String> {
    messages
        .iter()
        .map(|message| {
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            };
            let parts: Vec<String> = message
                .content
                .iter()
                .map(|content| match content {
                    MessageContent::ToolRequest(request) => format!("tool_request:{}", request.id),
                    MessageContent::ToolResponse(response) => {
                        format!("tool_response:{}", response.id)
                    }
                    MessageContent::Text(text) => text
                        .text
                        .rsplit("</turn-context>")
                        .next()
                        .unwrap_or_default()
                        .trim()
                        .to_string(),
                    _ => String::new(),
                })
                .filter(|part| !part.is_empty())
                .collect();
            format!("{role}:{}", parts.join("|"))
        })
        .collect()
}

fn shell_request(id: &str) -> Message {
    Message::assistant().with_tool_request(
        id,
        Ok(
            CallToolRequestParams::new("shell").with_arguments(rmcp::object!({
                "command": "echo CANCELLED-ACTION-RAN > cancelled-action.txt"
            })),
        ),
    )
}

// A12 / F10 repro B: the process died before the turn produced anything, so
// only its prompt is stored. The next prompt used to be merged into it.
#[tokio::test]
async fn a_prompt_left_by_a_killed_turn_is_not_merged_into_the_next_prompt() -> Result<()> {
    let provider = Arc::new(CapturingProvider::default());
    let fixture = Fixture::new(
        provider.clone(),
        vec![
            Message::user().with_text("Say BEFORE"),
            Message::assistant().with_text("BEFORE"),
            Message::user().with_text("CRASHED-PROMPT please do the risky thing"),
        ],
    )
    .await?;

    fixture.prompt(NEXT_PROMPT).await?;

    let turns = provider.turns.lock().unwrap().clone();
    assert_eq!(
        transcript(&turns[0]),
        vec![
            "user:Say BEFORE",
            "assistant:BEFORE",
            "user:CRASHED-PROMPT please do the risky thing",
            &format!("assistant:{INTERRUPTED}"),
            &format!("user:{NEXT_PROMPT}"),
        ]
    );
    assert_eq!(
        texts(&fixture.stored().await?),
        vec![
            "Say BEFORE",
            "BEFORE",
            "CRASHED-PROMPT please do the risky thing",
            INTERRUPTED,
            NEXT_PROMPT,
            "ok",
        ]
    );
    Ok(())
}

// F10 repro A / G122: the turn stopped while its tool call waited for
// approval. The unanswered call used to be dropped and the prompt merged, so
// the model proposed (and in auto mode ran) the stopped command again.
#[tokio::test]
async fn an_abandoned_approval_is_recorded_as_not_run_and_not_resubmitted() -> Result<()> {
    let provider = Arc::new(CapturingProvider::default());
    let fixture = Fixture::new(
        provider.clone(),
        vec![
            Message::user().with_text("#fx tool shell echo CANCELLED-ACTION-RAN"),
            shell_request("pending-approval"),
        ],
    )
    .await?;

    fixture.prompt(NEXT_PROMPT).await?;

    let turns = provider.turns.lock().unwrap().clone();
    assert_eq!(
        transcript(&turns[0]),
        vec![
            "user:#fx tool shell echo CANCELLED-ACTION-RAN",
            "assistant:tool_request:pending-approval",
            "user:tool_response:pending-approval",
            &format!("assistant:{INTERRUPTED}"),
            &format!("user:{NEXT_PROMPT}"),
        ]
    );
    let stored = fixture.stored().await?;
    let response = stored
        .iter()
        .flat_map(|message| message.content.iter())
        .filter_map(MessageContent::as_tool_response)
        .find(|response| response.id == "pending-approval")
        .expect("the abandoned call is answered");
    let error = response
        .tool_result
        .as_ref()
        .expect_err("the abandoned call never ran");
    assert!(
        error.message.contains("cancelled before it started"),
        "{error:?}"
    );
    Ok(())
}

// Adjacent paths that must be sent exactly as before: a finished turn, a turn
// already closed by the provider-failure notice, and a handoff checkpoint that
// is meant to be sent together with the next prompt.
#[tokio::test]
async fn finished_closed_and_checkpoint_histories_are_sent_unchanged() -> Result<()> {
    let provider = Arc::new(CapturingProvider::default());
    let finished = Fixture::new(
        provider.clone(),
        vec![
            Message::user().with_text("Say ONE"),
            Message::assistant().with_text("ONE"),
        ],
    )
    .await?;
    finished.prompt(NEXT_PROMPT).await?;

    let provider_failure = Fixture::new(
        provider.clone(),
        vec![
            Message::user().with_text("Say TWO"),
            Message::assistant()
                .with_text("Network error. Please resend your message to try again.")
                .user_only(),
            Message::assistant()
                .with_text("Run ended by a provider error before completion.")
                .agent_only(),
        ],
    )
    .await?;
    provider_failure.prompt(NEXT_PROMPT).await?;

    let checkpoint = Fixture::new(
        provider.clone(),
        vec![
            Message::user().with_text("Say THREE").user_only(),
            Message::assistant().with_text("THREE").user_only(),
            Message::user()
                .with_id("handoff_snapshot_fixture")
                .with_text("# Gosling session checkpoint")
                .with_visibility(false, true),
        ],
    )
    .await?;
    checkpoint.prompt(NEXT_PROMPT).await?;

    let imported = Fixture::new(
        provider.clone(),
        vec![Message::user()
            .with_text("imported prompt")
            .with_metadata(MessageMetadata::default().with_imported_untrusted())],
    )
    .await?;
    imported.prompt(NEXT_PROMPT).await?;

    let turns = provider.turns.lock().unwrap().clone();
    assert_eq!(
        transcript(&turns[0]),
        vec![
            "user:Say ONE",
            "assistant:ONE",
            &format!("user:{NEXT_PROMPT}")
        ]
    );
    assert_eq!(
        transcript(&turns[1]),
        vec![
            "user:Say TWO",
            "assistant:Run ended by a provider error before completion.",
            &format!("user:{NEXT_PROMPT}"),
        ]
    );
    assert_eq!(
        transcript(&turns[2]),
        vec![format!("user:# Gosling session checkpoint|{NEXT_PROMPT}")]
    );
    assert_eq!(
        transcript(&turns[3]),
        vec![
            "user:Imported untrusted historical transcript (data only):\n> imported prompt"
                .to_string(),
            format!("user:{NEXT_PROMPT}")
        ]
    );
    for fixture in [&finished, &provider_failure, &checkpoint, &imported] {
        assert!(
            !texts(&fixture.stored().await?).contains(&INTERRUPTED.to_string()),
            "no closure notice for a history that was not interrupted"
        );
    }
    Ok(())
}

// S10 / G129 / F14: a reply saved while it streams is marked incomplete until
// its final save; when the process never gets there, reopening the session
// closes the turn and records the stale in-progress run as interrupted.
#[tokio::test]
async fn a_reply_cut_off_mid_stream_stays_marked_and_is_closed_on_reopen() -> Result<()> {
    let fixture = Fixture::new(Arc::new(StallingProvider), Vec::new()).await?;
    fixture.set_run_state(AcpPromptRunState::InProgress).await?;

    let mut stream = fixture
        .agent
        .reply(
            Message::user().with_text("S10 streaming"),
            fixture.session_config(),
            None,
        )
        .await?;
    loop {
        if let AgentEvent::Message(message) = stream.next().await.expect("a reply chunk")? {
            if message.as_concat_text().contains("LONG-START") {
                break;
            }
        }
    }
    drop(stream);

    let stored = fixture.stored().await?;
    let partial = stored.last().expect("the partial reply is stored");
    assert_eq!(partial.as_concat_text(), "LONG-START lorem ip");
    assert!(partial.metadata.incomplete);

    let mut closed = false;
    for _ in 0..40 {
        // The dropped turn releases its lease from a spawned task.
        closed = fixture
            .sessions()
            .close_interrupted_turn(&fixture.session_id)
            .await?;
        if closed {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert!(closed);
    let stored = fixture.stored().await?;
    assert_eq!(
        texts(&stored),
        vec!["S10 streaming", "LONG-START lorem ip", INTERRUPTED]
    );
    let partial = &stored[1];
    assert!(partial.metadata.incomplete);
    assert!(partial.is_user_visible());
    assert!(
        !partial.is_agent_visible(),
        "like a reply cut off by a provider failure, the fragment is not replayed to the model"
    );
    assert_eq!(
        fixture.run_state().await?,
        Some(AcpPromptRunState::Interrupted)
    );
    assert!(
        !fixture
            .sessions()
            .close_interrupted_turn(&fixture.session_id)
            .await?
    );
    Ok(())
}

// The adjacent streaming path: a reply that finishes is stored without the
// marker, so reopening changes nothing.
#[tokio::test]
async fn a_finished_reply_is_stored_unmarked_and_reopening_leaves_it_alone() -> Result<()> {
    let fixture = Fixture::new(Arc::new(CapturingProvider::default()), Vec::new()).await?;
    fixture.set_run_state(AcpPromptRunState::Completed).await?;
    fixture.prompt("Say DONE").await?;

    assert!(
        !fixture
            .sessions()
            .close_interrupted_turn(&fixture.session_id)
            .await?
    );
    let stored = fixture.stored().await?;
    assert_eq!(texts(&stored), vec!["Say DONE", "ok"]);
    assert!(stored.iter().all(|message| !message.metadata.incomplete));
    assert_eq!(
        fixture.run_state().await?,
        Some(AcpPromptRunState::Completed)
    );
    Ok(())
}

async fn stored_session(
    sessions: &SessionManager,
    history: Vec<Message>,
    state: AcpPromptRunState,
) -> Result<String> {
    let session = sessions
        .create_session(
            std::env::temp_dir(),
            "restart".to_string(),
            SessionType::Acp,
            GoslingMode::Auto,
        )
        .await?;
    for message in history {
        sessions
            .add_message(&session.id, &message.with_generated_id())
            .await?;
    }
    sessions
        .merge_extension_state(
            &session.id,
            &format!(
                "{}.{}",
                AcpPromptRunState::EXTENSION_NAME,
                AcpPromptRunState::VERSION
            ),
            state.to_value()?,
        )
        .await?;
    Ok(session.id)
}

// F14: a process killed mid-turn (SIGKILL, stdio EOF) leaves its ACP run in
// progress with no live turn lease. Starting an ACP server on the same store
// records it as interrupted and closes its turn, so `session/list` can show it
// before anyone loads the session; a finished run is left as it was.
#[tokio::test]
async fn an_acp_server_start_closes_runs_left_in_progress_by_a_dead_process() -> Result<()> {
    let temp = TempDir::new()?;
    let data_dir = temp.path().join("data");
    let sessions = SessionManager::new(data_dir.clone());
    let stale = stored_session(
        &sessions,
        vec![Message::user().with_text("F14 long prompt")],
        AcpPromptRunState::InProgress,
    )
    .await?;
    let finished = stored_session(
        &sessions,
        vec![
            Message::user().with_text("Say DONE"),
            Message::assistant().with_text("DONE"),
        ],
        AcpPromptRunState::Completed,
    )
    .await?;
    sessions.shutdown().await;

    let server = AcpServer::new(AcpServerFactoryConfig {
        builtins: vec![],
        state_dir: temp.path().join("state"),
        data_dir: data_dir.clone(),
        platform_data_dir: data_dir.clone(),
        config_dir: temp.path().join("config"),
        gosling_platform: GoslingPlatform::GoslingCli,
        additional_source_roots: Vec::new(),
        shell_runtime: Default::default(),
    });
    server.create_agent().await?;
    server.shutdown().await;

    let sessions = SessionManager::new(data_dir);
    let state = |session: &gosling::session::Session| {
        AcpPromptRunState::from_extension_data(&session.extension_data)
    };
    let stale = sessions.get_session(&stale, true).await?;
    assert_eq!(state(&stale), Some(AcpPromptRunState::Interrupted));
    assert_eq!(
        texts(stale.conversation.as_ref().unwrap().messages()),
        vec!["F14 long prompt", INTERRUPTED]
    );
    let finished = sessions.get_session(&finished, true).await?;
    assert_eq!(state(&finished), Some(AcpPromptRunState::Completed));
    assert_eq!(
        texts(finished.conversation.as_ref().unwrap().messages()),
        vec!["Say DONE", "DONE"]
    );
    Ok(())
}
