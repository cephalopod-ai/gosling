use super::search::search_skills;
use super::{discover_skills, hydrate_skill_entry, loaded_skill_context_with_args};
use crate::agents::extension::PlatformExtensionContext;
use crate::agents::mcp_client::{Error, McpClientTrait};
use crate::agents::ToolCallContext;
use async_trait::async_trait;
use gosling_sdk_types::custom_requests::{SourceEntry, SourceType};
use rmcp::model::{
    CallToolResult, Content, Implementation, InitializeResult, JsonObject, ListToolsResult,
    ServerCapabilities, ServerNotification, Tool,
};
use std::path::{Path, PathBuf};
use std::sync::RwLock;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub static EXTENSION_NAME: &str = "skills";
const DIRECT_SKILL_ADVERTISEMENT_LIMIT: usize = 40;
const DEFAULT_SEARCH_LIMIT: usize = 5;
const MAX_SEARCH_LIMIT: usize = 20;

pub struct SkillsClient {
    info: InitializeResult,
    working_dir: PathBuf,
    skills: RwLock<Vec<SourceEntry>>,
}

impl SkillsClient {
    pub fn new(context: PlatformExtensionContext) -> anyhow::Result<Self> {
        let working_dir = context
            .session
            .as_ref()
            .map(|s| s.working_dir.clone())
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());

        let info = InitializeResult::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(EXTENSION_NAME, "1.0.0").with_title("Skills"));

        let skills = RwLock::new(discover_skills(Some(&working_dir)));

        Ok(Self {
            info,
            working_dir,
            skills,
        })
    }

    fn snapshot(&self) -> Vec<SourceEntry> {
        self.skills.read().unwrap().clone()
    }

    fn refresh(&self) -> Vec<SourceEntry> {
        let skills = discover_skills(Some(&self.working_dir));
        *self.skills.write().unwrap() = skills.clone();
        skills
    }
}

#[async_trait]
impl McpClientTrait for SkillsClient {
    async fn list_tools(
        &self,
        _session_id: &str,
        _next_cursor: Option<String>,
        _cancellation_token: CancellationToken,
    ) -> Result<ListToolsResult, Error> {
        let schema = serde_json::json!({
            "type": "object",
            "required": ["name"],
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the skill to load. Use \"skill-name/path\" to load a supporting file."
                },
                "args": {
                    "type": "string",
                    "description": "Optional arguments to provide when loading the skill."
                }
            }
        });

        let load_tool = Tool::new(
            "load_skill",
            "Load a skill's full content into your context so you can follow its instructions.\n\n\
             Skills are listed in your system instructions. When you need to use one, \
             load it first to get the detailed instructions.\n\n\
             Examples:\n\
             - load_skill(name: \"gdrive\") → Loads the gdrive skill instructions\n\
             - load_skill(name: \"my-skill\", args: \"the arguments for the skill\") → Loads a skill with arguments\n\
             - load_skill(name: \"my-skill/template.md\") → Loads a supporting file"
                .to_string(),
            schema.as_object().unwrap().clone(),
        );

        let search_schema = serde_json::json!({
            "type": "object",
            "required": ["query"],
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Task or routing terms to match against skill actions, roles, surface, targets, keywords, names, and descriptions."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_SEARCH_LIMIT,
                    "default": DEFAULT_SEARCH_LIMIT
                }
            }
        });
        let search_tool = Tool::new(
            "find_skills",
            "Find relevant skills without loading their full instructions. Use this when the catalog is large or the exact skill name is unknown."
                .to_string(),
            search_schema.as_object().unwrap().clone(),
        );

        let refresh_schema = serde_json::json!({
            "type": "object",
            "properties": {}
        });
        let refresh_tool = Tool::new(
            "refresh_skills",
            "Refresh skill discovery after external catalog or SKILL.md files change.".to_string(),
            refresh_schema.as_object().unwrap().clone(),
        );

        Ok(ListToolsResult {
            tools: vec![load_tool, search_tool, refresh_tool],
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        _ctx: &ToolCallContext,
        name: &str,
        arguments: Option<JsonObject>,
        _cancellation_token: CancellationToken,
    ) -> Result<CallToolResult, Error> {
        if name == "find_skills" {
            let query = arguments
                .as_ref()
                .and_then(|args| args.get("query"))
                .and_then(|value| value.as_str())
                .unwrap_or("");
            if query.trim().is_empty() {
                return Ok(CallToolResult::error(vec![Content::text(
                    "Missing required parameter: query",
                )]));
            }
            let limit = arguments
                .as_ref()
                .and_then(|args| args.get("limit"))
                .and_then(|value| value.as_u64())
                .map(|value| value as usize)
                .unwrap_or(DEFAULT_SEARCH_LIMIT)
                .clamp(1, MAX_SEARCH_LIMIT);

            let mut skills = self.snapshot();
            if search_skills(&skills, query, limit).is_empty() {
                skills = self.refresh();
            }
            let matches = search_skills(&skills, query, limit);
            if matches.is_empty() {
                return Ok(CallToolResult::success(vec![Content::text(format!(
                    "No skills matched '{}'.",
                    query
                ))]));
            }

            let mut output = format!("# Skill matches for '{}'\n", query);
            for skill_match in matches {
                output.push_str(&format!(
                    "\n- **{}** — {}\n",
                    skill_match.skill.name, skill_match.skill.description
                ));
            }
            output.push_str("\nLoad the best match with `load_skill`.");
            return Ok(CallToolResult::success(vec![Content::text(output)]));
        }

        if name == "refresh_skills" {
            let count = self.refresh().len();
            return Ok(CallToolResult::success(vec![Content::text(format!(
                "Refreshed {} skills.",
                count
            ))]));
        }

        if name != "load_skill" {
            return Ok(CallToolResult::error(vec![Content::text(format!(
                "Unknown tool: {}",
                name
            ))]));
        }

        let skill_name = arguments
            .as_ref()
            .and_then(|args| args.get("name"))
            .and_then(|v| v.as_str())
            .unwrap_or("");

        if skill_name.is_empty() {
            return Ok(CallToolResult::error(vec![Content::text(
                "Missing required parameter: name",
            )]));
        }
        let args = arguments
            .as_ref()
            .and_then(|args| args.get("args"))
            .and_then(|v| v.as_str());

        let mut skills = self.snapshot();

        if !skills.iter().any(|skill| {
            skill.name == skill_name
                || skill_name
                    .split_once('/')
                    .is_some_and(|(parent, _)| skill.name == parent)
        }) {
            skills = self.refresh();
        }

        if let Some(skill) = skills.iter().find(|s| s.name == skill_name) {
            let Some(skill) = hydrate_skill_entry(skill) else {
                return Ok(CallToolResult::error(vec![Content::text(format!(
                    "Skill '{}' is no longer available. Refresh the skill catalog and try again.",
                    skill_name
                ))]));
            };
            return match loaded_skill_context_with_args(&skill, args) {
                Ok(rendered) => Ok(CallToolResult::success(vec![Content::text(rendered)])),
                Err(e) => Ok(CallToolResult::error(vec![Content::text(format!(
                    "Failed to parse skill arguments: {}",
                    e
                ))])),
            };
        }

        if let Some((parent_skill_name, raw_relative_path)) = skill_name.split_once('/') {
            let relative_path = raw_relative_path.replace('\\', "/");
            if let Some(skill) = skills.iter().find(|s| {
                s.name == parent_skill_name
                    && matches!(s.source_type, SourceType::Skill | SourceType::BuiltinSkill)
            }) {
                let Some(skill) = hydrate_skill_entry(skill) else {
                    return Ok(CallToolResult::error(vec![Content::text(format!(
                        "Skill '{}' is no longer available. Refresh the skill catalog and try again.",
                        parent_skill_name
                    ))]));
                };
                let skill_dir = PathBuf::from(&skill.path);
                let canonical_skill_dir = skill_dir
                    .canonicalize()
                    .unwrap_or_else(|_| skill_dir.clone());

                for file_path in &skill.supporting_files {
                    let file_path_buf = Path::new(file_path);
                    let Ok(rel) = file_path_buf.strip_prefix(&skill_dir) else {
                        continue;
                    };
                    if rel.to_string_lossy().replace('\\', "/") != relative_path {
                        continue;
                    }

                    return Ok(match file_path_buf.canonicalize() {
                        Ok(canonical) if canonical.starts_with(&canonical_skill_dir) => {
                            match std::fs::read_to_string(&canonical) {
                                Ok(content) => {
                                    CallToolResult::success(vec![Content::text(format!(
                                        "# Loaded: {}\n\n{}\n\n---\nFile loaded into context.",
                                        skill_name, content
                                    ))])
                                }
                                Err(e) => CallToolResult::error(vec![Content::text(format!(
                                    "Failed to read '{}': {}",
                                    skill_name, e
                                ))]),
                            }
                        }
                        Ok(_) => CallToolResult::error(vec![Content::text(format!(
                            "Refusing to load '{}': resolves outside the skill directory",
                            skill_name
                        ))]),
                        Err(e) => CallToolResult::error(vec![Content::text(format!(
                            "Failed to resolve '{}': {}",
                            skill_name, e
                        ))]),
                    });
                }

                let available: Vec<String> = skill
                    .supporting_files
                    .iter()
                    .filter_map(|f| {
                        Path::new(f)
                            .strip_prefix(&skill_dir)
                            .ok()
                            .map(|r| r.to_string_lossy().replace('\\', "/"))
                    })
                    .take(10)
                    .collect();

                return Ok(if available.is_empty() {
                    CallToolResult::error(vec![Content::text(format!(
                        "Skill '{}' has no supporting files.",
                        skill.name
                    ))])
                } else {
                    CallToolResult::error(vec![Content::text(format!(
                        "File '{}' not found. Available: {}",
                        skill_name,
                        available.join(", ")
                    ))])
                });
            }
        }

        let suggestions: Vec<&str> = skills
            .iter()
            .filter(|s| {
                s.name.to_lowercase().contains(&skill_name.to_lowercase())
                    || skill_name.to_lowercase().contains(&s.name.to_lowercase())
            })
            .take(3)
            .map(|s| s.name.as_str())
            .collect();

        Ok(if suggestions.is_empty() {
            CallToolResult::error(vec![Content::text(format!(
                "Skill '{}' not found.",
                skill_name
            ))])
        } else {
            CallToolResult::error(vec![Content::text(format!(
                "Skill '{}' not found. Did you mean: {}?",
                skill_name,
                suggestions.join(", ")
            ))])
        })
    }

    fn get_info(&self) -> Option<&InitializeResult> {
        Some(&self.info)
    }

    fn get_instructions(&self) -> Option<String> {
        let sources = self.snapshot();
        let mut skills: Vec<&SourceEntry> = sources
            .iter()
            .filter(|s| {
                s.source_type == SourceType::Skill || s.source_type == SourceType::BuiltinSkill
            })
            .collect();
        skills.sort_by(|a, b| (&a.name, &a.path).cmp(&(&b.name, &b.path)));

        if skills.is_empty() {
            return None;
        }

        if skills.len() > DIRECT_SKILL_ADVERTISEMENT_LIMIT {
            return Some(format!(
                "\n\nYou have a searchable catalog of {} skills. When a reusable workflow may help or the user asks for a skill, call `find_skills` with the task intent, then call `load_skill` with the best match. Do not guess a skill name.",
                skills.len()
            ));
        }

        let mut instructions = String::from(
            "\n\nYou have these skills at your disposal, when it is clear they can help you solve a problem or you are asked to use them:",
        );
        for skill in &skills {
            instructions.push_str(&format!("\n• {} - {}", skill.name, skill.description));
        }
        Some(instructions)
    }

    async fn subscribe(&self) -> mpsc::Receiver<ServerNotification> {
        let (_tx, rx) = mpsc::channel(1);
        rx
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_skill_from_filesystem() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join(".gosling/skills/my-skill");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: my-skill\ndescription: A test skill\n---\nDo the thing.",
        )
        .unwrap();

        let session = std::sync::Arc::new(crate::session::Session {
            working_dir: temp_dir.path().to_path_buf(),
            ..crate::session::Session::default()
        });
        let client = SkillsClient::new(PlatformExtensionContext {
            extension_manager: None,
            session_manager: Arc::new(crate::session::SessionManager::instance()),
            session: Some(session),
            use_login_shell_path: false,
            code_execution_runtime: crate::config::CodeExecutionRuntime::Enabled,
        })
        .unwrap();

        let ctx = ToolCallContext::new("test".to_string(), None, None);
        let args: JsonObject =
            serde_json::from_value(serde_json::json!({"name": "my-skill"})).unwrap();
        let result = client
            .call_tool(&ctx, "load_skill", Some(args), CancellationToken::new())
            .await
            .unwrap();

        assert!(!result.is_error.unwrap_or(false));
        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("expected text"),
        };
        assert!(text.contains("my-skill"));
        assert!(text.contains("Do the thing"));
    }

    #[test]
    fn catalog_entry_loads_content_and_validates_identity_on_demand() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join("catalog/plan-example");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: plan-example\ndescription: File description\n---\nPlan carefully.",
        )
        .unwrap();
        fs::write(skill_dir.join("reference.md"), "Supporting details.").unwrap();
        let mut entry = SourceEntry {
            source_type: SourceType::Skill,
            name: "plan-example".to_string(),
            description: "Catalog description".to_string(),
            path: skill_dir.to_string_lossy().into_owned(),
            global: true,
            writable: false,
            ..Default::default()
        };

        let loaded = hydrate_skill_entry(&entry).unwrap();

        assert_eq!(loaded.description, "Catalog description");
        assert!(loaded.content.contains("Plan carefully."));
        assert_eq!(loaded.supporting_files.len(), 1);

        entry.name = "different-id".to_string();
        assert!(hydrate_skill_entry(&entry).is_none());
    }

    #[tokio::test]
    async fn test_load_skill_not_found_returns_error() {
        let client = SkillsClient::new(PlatformExtensionContext {
            extension_manager: None,
            session_manager: Arc::new(crate::session::SessionManager::instance()),
            session: None,
            use_login_shell_path: false,
            code_execution_runtime: crate::config::CodeExecutionRuntime::Enabled,
        })
        .unwrap();

        let ctx = ToolCallContext::new("test".to_string(), None, None);
        let args: JsonObject =
            serde_json::from_value(serde_json::json!({"name": "nonexistent"})).unwrap();
        let result = client
            .call_tool(&ctx, "load_skill", Some(args), CancellationToken::new())
            .await
            .unwrap();

        assert!(result.is_error.unwrap_or(false));
    }

    #[tokio::test]
    async fn test_find_skills_uses_structured_routing_metadata() {
        let temp_dir = TempDir::new().unwrap();
        let skill_dir = temp_dir.path().join(".agents/skills/plan-example");
        fs::create_dir_all(&skill_dir).unwrap();
        fs::write(
            skill_dir.join("SKILL.md"),
            "---\nname: plan-example\ndescription: A synthetic planning skill\nmetadata:\n  routing:\n    actions: [plan]\n    roles: [architect]\n    surface: registry\n    targets: [skills]\n    keywords: [index]\n---\nPlan the example.",
        )
        .unwrap();
        let session = Arc::new(crate::session::Session {
            working_dir: temp_dir.path().to_path_buf(),
            ..crate::session::Session::default()
        });
        let client = SkillsClient::new(PlatformExtensionContext {
            extension_manager: None,
            session_manager: Arc::new(crate::session::SessionManager::instance()),
            session: Some(session),
            use_login_shell_path: false,
            code_execution_runtime: crate::config::CodeExecutionRuntime::Enabled,
        })
        .unwrap();
        let ctx = ToolCallContext::new("test".to_string(), None, None);
        let args: JsonObject = serde_json::from_value(serde_json::json!({
            "query": "plan architect registry skills index"
        }))
        .unwrap();

        let result = client
            .call_tool(&ctx, "find_skills", Some(args), CancellationToken::new())
            .await
            .unwrap();

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(text) => &text.text,
            _ => panic!("expected text"),
        };
        assert!(text.contains("plan-example"));
    }

    #[tokio::test]
    async fn large_catalog_uses_bounded_search_instructions() {
        let client = SkillsClient::new(PlatformExtensionContext {
            extension_manager: None,
            session_manager: Arc::new(crate::session::SessionManager::instance()),
            session: None,
            use_login_shell_path: false,
            code_execution_runtime: crate::config::CodeExecutionRuntime::Enabled,
        })
        .unwrap();
        *client.skills.write().unwrap() = (0..=DIRECT_SKILL_ADVERTISEMENT_LIMIT)
            .map(|index| SourceEntry {
                source_type: SourceType::Skill,
                name: format!("synthetic-skill-{index}"),
                description: "Synthetic description".to_string(),
                ..Default::default()
            })
            .collect();

        let instructions = client.get_instructions().unwrap();

        assert!(instructions.contains("searchable catalog"));
        assert!(instructions.contains("find_skills"));
        assert!(!instructions.contains("synthetic-skill-0"));
    }
}
