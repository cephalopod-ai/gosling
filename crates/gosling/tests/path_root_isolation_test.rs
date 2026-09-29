use gosling::agents::platform_extensions::summon::discover_filesystem_sources;
use gosling::checks::global_checks_dirs;
use gosling::skills::{discover_skills, global_skills_dir};
use gosling::sources::create_source;
use gosling_sdk_types::custom_requests::{SourceEntry, SourceType};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use tempfile::TempDir;

struct Layout {
    home: TempDir,
    root: TempDir,
    project: TempDir,
}

fn write_skill(base: &Path, dir: &str, name: &str) {
    let skill_dir = base.join(dir).join(name);
    fs::create_dir_all(&skill_dir).unwrap();
    fs::write(
        skill_dir.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {name} description\n---\nbody\n"),
    )
    .unwrap();
}

fn write_agent(base: &Path, dir: &str, name: &str) {
    let agent_dir = base.join(dir);
    fs::create_dir_all(&agent_dir).unwrap();
    fs::write(
        agent_dir.join(format!("{name}.md")),
        format!("---\nname: {name}\ndescription: {name} description\n---\nYou are {name}.\n"),
    )
    .unwrap();
}

fn layout() -> Layout {
    let layout = Layout {
        home: TempDir::new().unwrap(),
        root: TempDir::new().unwrap(),
        project: TempDir::new().unwrap(),
    };
    let home = layout.home.path();
    write_skill(home, ".agents/skills", "home-agents-skill");
    write_skill(home, ".claude/skills", "home-claude-skill");
    write_skill(home, ".config/agents/skills", "home-config-agents-skill");
    write_agent(home, ".agents/agents", "home-agents-agent");
    write_agent(home, ".gosling/agents", "home-gosling-agent");
    write_agent(home, ".claude/agents", "home-claude-agent");

    let root = layout.root.path();
    write_skill(root, ".agents/skills", "root-agents-skill");
    write_skill(root, ".claude/skills", "root-claude-skill");
    write_agent(root, ".agents/agents", "root-agents-agent");
    write_agent(root, ".claude/agents", "root-claude-agent");
    layout
}

fn names(entries: Vec<SourceEntry>) -> Vec<String> {
    entries.into_iter().map(|entry| entry.name).collect()
}

fn has_prefix(names: &[String], prefix: &str) -> bool {
    names.iter().any(|name| name.starts_with(prefix))
}

fn write_global_skill(name: &str) {
    create_source(
        SourceType::Skill,
        name,
        "written by the test",
        "body",
        true,
        None,
        HashMap::new(),
    )
    .unwrap();
}

#[test]
fn path_root_replaces_home_for_skill_agent_and_check_discovery() {
    let layout = layout();
    let home = layout.home.path();
    let root = layout.root.path();
    let _env = env_lock::lock_env([
        ("HOME", Some(home.to_str().unwrap())),
        ("GOSLING_PATH_ROOT", Some(root.to_str().unwrap())),
        ("GOSLING_DISABLE_KEYRING", Some("1")),
    ]);

    let skills = names(discover_skills(Some(layout.project.path())));
    assert!(
        skills.contains(&"root-agents-skill".to_string()),
        "{skills:?}"
    );
    assert!(
        skills.contains(&"root-claude-skill".to_string()),
        "{skills:?}"
    );
    assert!(!has_prefix(&skills, "home-"), "{skills:?}");

    let agents = names(discover_filesystem_sources(layout.project.path()));
    assert!(
        agents.contains(&"root-agents-agent".to_string()),
        "{agents:?}"
    );
    assert!(
        agents.contains(&"root-claude-agent".to_string()),
        "{agents:?}"
    );
    assert!(!has_prefix(&agents, "home-"), "{agents:?}");

    assert_eq!(
        global_checks_dirs(),
        vec![
            root.join(".config").join("gosling").join("checks"),
            root.join(".config").join("agents").join("checks"),
        ]
    );

    assert_eq!(
        global_skills_dir(),
        Some(root.join(".agents").join("skills"))
    );
    write_global_skill("root-written-skill");
    assert!(root
        .join(".agents/skills/root-written-skill/SKILL.md")
        .is_file());
    assert!(!home.join(".agents/skills/root-written-skill").exists());
}

#[test]
fn without_path_root_discovery_still_uses_home() {
    let layout = layout();
    let home = layout.home.path();
    let _env = env_lock::lock_env([
        ("HOME", Some(home.to_str().unwrap())),
        ("GOSLING_PATH_ROOT", None),
        ("XDG_CONFIG_HOME", None),
        ("XDG_DATA_HOME", None),
        ("XDG_STATE_HOME", None),
        ("GOSLING_DISABLE_KEYRING", Some("1")),
    ]);

    let skills = names(discover_skills(Some(layout.project.path())));
    for expected in [
        "home-agents-skill",
        "home-claude-skill",
        "home-config-agents-skill",
    ] {
        assert!(skills.contains(&expected.to_string()), "{skills:?}");
    }
    assert!(!has_prefix(&skills, "root-"), "{skills:?}");

    let agents = names(discover_filesystem_sources(layout.project.path()));
    for expected in [
        "home-agents-agent",
        "home-gosling-agent",
        "home-claude-agent",
    ] {
        assert!(agents.contains(&expected.to_string()), "{agents:?}");
    }
    assert!(!has_prefix(&agents, "root-"), "{agents:?}");

    assert_eq!(
        global_checks_dirs(),
        vec![
            home.join(".config").join("gosling").join("checks"),
            home.join(".config").join("agents").join("checks"),
        ]
    );

    assert_eq!(
        global_skills_dir(),
        Some(home.join(".agents").join("skills"))
    );
    write_global_skill("home-written-skill");
    assert!(home
        .join(".agents/skills/home-written-skill/SKILL.md")
        .is_file());
}
