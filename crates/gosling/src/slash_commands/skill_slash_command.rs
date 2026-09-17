use std::path::Path;

use gosling_sdk_types::custom_requests::{SourceEntry, SourceType};

use super::types::{SlashCommandEntry, SlashCommandSource};
use super::util::normalize_command_name;

pub fn list_commands(working_dir: Option<&Path>) -> Vec<SlashCommandEntry> {
    commands_from_sources(crate::skills::list_installed_skills(working_dir))
}

pub fn format_installed_skills(working_dir: Option<&Path>) -> String {
    let sources = crate::skills::list_installed_skills(working_dir);
    let skills: Vec<_> = sources
        .iter()
        .filter(|s| matches!(s.source_type, SourceType::Skill | SourceType::BuiltinSkill))
        .collect();

    let mut output = String::new();
    if skills.is_empty() {
        output.push_str("No skills installed.\n\n");
        output.push_str("Skills are loaded from SKILL.md files in:\n");
        output.push_str("  - ~/.agents/skills/ (global)\n");
        output.push_str("  - ~/.agents/plugins/*/skills/ (installed plugins)\n");
        output.push_str("  - .agents/skills/ (in current project)\n");
    } else {
        output.push_str(&format!("**Installed skills ({}):**\n\n", skills.len()));
        for skill in &skills {
            let kind_label = if skill.source_type == SourceType::BuiltinSkill {
                " *(builtin)*"
            } else {
                ""
            };
            output.push_str(&format!(
                "- **{}**{}: {}\n",
                skill.name, kind_label, skill.description
            ));
        }
    }
    output
}

/// A skill the user explicitly selected with a slash command, hydrated and
/// admitted but not yet recorded against the turn.
pub(crate) struct PreparedSkillCommand {
    command: String,
    skill: SourceEntry,
    pub(crate) admission: crate::skills::admission::SkillAdmission,
}

pub(crate) fn prepare_command(
    command: &str,
    working_dir: Option<&Path>,
) -> Result<Option<PreparedSkillCommand>, String> {
    let wd_fallback;
    let working_dir = match working_dir {
        Some(path) => Some(path),
        None => {
            wd_fallback = std::env::current_dir().ok();
            wd_fallback.as_deref()
        }
    };
    let Some(discovered) = crate::skills::discover_skills_with_origin(working_dir)
        .into_iter()
        .find(|skill| skill.entry.name.eq_ignore_ascii_case(command))
    else {
        return Ok(None);
    };
    prepare_discovered(&discovered, command).map(Some)
}

fn prepare_discovered(
    discovered: &crate::skills::DiscoveredSkill,
    command: &str,
) -> Result<PreparedSkillCommand, String> {
    let (skill, bytes) = crate::skills::hydrate_skill_entry_with_bytes(&discovered.entry)
        .map_err(|_| format!("Skill /{} is no longer available", command))?;
    let admission = crate::skills::skill_admission_for(
        discovered,
        &skill,
        &bytes,
        crate::skills::admission::AdmissionChannel::UserSlashCommand,
    )
    .map_err(|refusal| format!("Skill /{command} was not admitted: {refusal}."))?;
    Ok(PreparedSkillCommand {
        command: command.to_string(),
        skill,
        admission,
    })
}

pub(crate) fn render_prepared_command(
    prepared: &PreparedSkillCommand,
    params_str: &str,
    scope: crate::skills::admission::AdmissionScope,
) -> Result<String, String> {
    let args = (!params_str.is_empty()).then_some(params_str);
    crate::skills::admitted_skill_context(&prepared.skill, args, &prepared.admission, scope)
        .map_err(|e| format!("Skill /{}: {}", prepared.command, e))
}

pub(super) fn commands_from_sources(sources: Vec<SourceEntry>) -> Vec<SlashCommandEntry> {
    sources
        .into_iter()
        .filter_map(|source| {
            let name = normalize_command_name(&source.name);
            if name.is_empty() {
                return None;
            }
            let input_hint = crate::skills::skill_argument_hint(&source);

            Some(SlashCommandEntry {
                name,
                description: source.description,
                source: SlashCommandSource::Skill,
                source_path: None,
                input_hint,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gosling_sdk_types::custom_requests::SourceType;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn commands_from_sources_marks_entries_as_skill() {
        let commands = commands_from_sources(vec![
            source_entry(SourceType::Skill, "review", "Review code"),
            source_entry(SourceType::Skill, "summarize", "Summarize text"),
        ]);

        let names: Vec<_> = commands.iter().map(|c| c.name.as_str()).collect();
        assert_eq!(names, vec!["review", "summarize"]);
        assert!(commands
            .iter()
            .all(|c| c.source == SlashCommandSource::Skill));
    }

    #[test]
    fn commands_from_sources_normalizes_names() {
        let commands = commands_from_sources(vec![source_entry(
            SourceType::Skill,
            "/Code-Review",
            "Review",
        )]);

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].name, "code-review");
    }

    #[test]
    fn commands_from_sources_skips_empty_names() {
        let commands =
            commands_from_sources(vec![source_entry(SourceType::Skill, "/", "Empty name")]);

        assert!(commands.is_empty());
    }

    #[test]
    fn list_commands_loads_project_skill_from_disk() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp
            .path()
            .join(".agents")
            .join("skills")
            .join("code-review");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: code-review\ndescription: Review changed code\nmetadata:\n  argument-hint: \"[task]\"\n  arguments:\n    - task\n---\nReview the diff.",
        )
        .unwrap();

        let commands = list_commands(Some(tmp.path()));
        let command = commands
            .iter()
            .find(|command| command.name == "code-review")
            .expect("project skill should be listed");

        assert_eq!(command.description, "Review changed code");
        assert_eq!(command.source, SlashCommandSource::Skill);
        assert_eq!(command.input_hint.as_deref(), Some("[task]"));
    }

    #[test]
    fn catalog_skill_command_hydrates_content_on_demand() {
        let tmp = TempDir::new().unwrap();
        let skill_dir = tmp.path().join("catalog").join("plan-example");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: plan-example\ndescription: File description\n---\nPlan carefully.",
        )
        .unwrap();

        let lightweight = crate::skills::DiscoveredSkill {
            entry: SourceEntry {
                source_type: SourceType::Skill,
                name: "plan-example".to_string(),
                description: "Catalog description".to_string(),
                path: skill_dir.to_string_lossy().into_owned(),
                global: true,
                writable: false,
                ..Default::default()
            },
            origin: crate::skills::admission::SkillOrigin::new(
                crate::skills::admission::SkillSourceKind::User,
            ),
        };

        let prepared = prepare_discovered(&lightweight, "plan-example").unwrap();
        let prompt = render_prepared_command(
            &prepared,
            "",
            crate::skills::admission::AdmissionScope::Turn,
        )
        .unwrap();

        assert!(prompt.contains("Catalog description"));
        assert!(prompt.contains("Plan carefully."));
        assert!(prompt.contains("## Host Admission"));
    }

    fn source_entry(source_type: SourceType, name: &str, description: &str) -> SourceEntry {
        SourceEntry {
            source_type,
            name: name.to_string(),
            description: description.to_string(),
            content: String::new(),
            path: String::new(),
            global: false,
            writable: false,
            supporting_files: Vec::new(),
            properties: HashMap::new(),
        }
    }
}
