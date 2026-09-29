use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use futures::StreamExt;
use gosling::agents::{Agent, AgentConfig, AgentEvent, GoslingPlatform, SessionConfig};
use gosling::config::permission::PermissionManager;
use gosling::config::GoslingMode;
use gosling::conversation::message::Message;
use gosling::session::session_manager::SessionType;
use gosling::session::{Session, SessionManager};
use rmcp::model::Role;
use tempfile::TempDir;

const SKILL_NAME: &str = "sentinel-skill";
const SKILL_BODY: &str = "SENTINEL-C18-INSTRUCTIONS";

struct Harness {
    _data_dir: TempDir,
    _project: TempDir,
    agent: Agent,
    session: Session,
}

impl Harness {
    async fn new() -> Result<Self> {
        let data_dir = TempDir::new()?;
        let project = TempDir::new()?;
        write_project_skill(project.path())?;

        let session_manager = Arc::new(SessionManager::new(data_dir.path().to_path_buf()));
        let agent = Agent::with_config(AgentConfig::new(
            session_manager.clone(),
            PermissionManager::instance(),
            GoslingMode::Auto,
            true,
            GoslingPlatform::GoslingCli,
        ));
        let session = session_manager
            .create_session(
                project.path().to_path_buf(),
                "skills-slash-command".to_string(),
                SessionType::Hidden,
                GoslingMode::default(),
            )
            .await?;
        Ok(Self {
            _data_dir: data_dir,
            _project: project,
            agent,
            session,
        })
    }

    async fn command(&self, text: &str) -> Result<Message> {
        Ok(self
            .agent
            .execute_command(text, &self.session.id)
            .await?
            .expect("slash command should produce a message"))
    }
}

fn write_project_skill(project: &Path) -> Result<()> {
    let skill_dir = project.join(".agents").join("skills").join(SKILL_NAME);
    std::fs::create_dir_all(&skill_dir)?;
    std::fs::write(
        skill_dir.join("SKILL.md"),
        format!(
            "---\nname: {SKILL_NAME}\ndescription: Sentinel skill for C18\n---\n{SKILL_BODY}\n"
        ),
    )?;
    Ok(())
}

#[tokio::test]
async fn skills_command_with_unknown_name_reports_it_locally() -> Result<()> {
    let harness = Harness::new().await?;

    for name in ["does-not-exist", "\"\""] {
        let reply = harness.command(&format!("/skills {name}")).await?;
        assert_eq!(reply.role, Role::Assistant);
        assert_eq!(
            reply.as_concat_text(),
            format!("No skill named '{name}'. Run /skills to list installed skills.")
        );
    }
    Ok(())
}

// No provider is configured, so any attempt to reach the model would fail the turn.
#[tokio::test]
async fn skills_command_with_unknown_name_sends_nothing_to_the_model() -> Result<()> {
    let harness = Harness::new().await?;

    let mut stream = harness
        .agent
        .reply(
            Message::user().with_text("/skills does-not-exist"),
            SessionConfig {
                id: harness.session.id.clone(),
                max_turns: Some(1),
                compacted_context: false,
                tail_limit: None,
            },
            None,
        )
        .await?;
    let mut assistant_texts = Vec::new();
    while let Some(event) = stream.next().await {
        if let AgentEvent::Message(message) = event? {
            if message.role == Role::Assistant {
                assistant_texts.push(message.as_concat_text());
            }
        }
    }
    assert_eq!(
        assistant_texts,
        vec!["No skill named 'does-not-exist'. Run /skills to list installed skills.".to_string()]
    );
    Ok(())
}

#[tokio::test]
async fn skills_command_with_known_name_matches_the_per_skill_command() -> Result<()> {
    let harness = Harness::new().await?;

    let via_skills = harness
        .command(&format!("/skills {SKILL_NAME} focus on tests"))
        .await?;
    let via_skill_command = harness
        .command(&format!("/{SKILL_NAME} focus on tests"))
        .await?;

    for message in [&via_skills, &via_skill_command] {
        assert_eq!(message.role, Role::User);
        let text = message.as_concat_text();
        assert!(text.contains(SKILL_BODY));
        assert!(text.contains("focus on tests"));
        assert!(text.contains("## Host Admission"));
    }
    Ok(())
}

#[tokio::test]
async fn bare_skills_command_still_lists_installed_skills() -> Result<()> {
    let harness = Harness::new().await?;

    let reply = harness.command("/skills").await?;
    assert_eq!(reply.role, Role::Assistant);
    assert!(reply.as_concat_text().contains("Installed skills"));
    assert!(reply.as_concat_text().contains(SKILL_NAME));
    Ok(())
}
