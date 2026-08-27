use chrono::{DateTime, Utc};
use rmcp::model::{CallToolRequestParams, CallToolResult, RawContent, ResourceContents};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionArtifactRelation {
    Created,
    Modified,
    Referenced,
}

impl std::fmt::Display for SessionArtifactRelation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::Modified => write!(f, "modified"),
            Self::Referenced => write!(f, "referenced"),
        }
    }
}

impl std::str::FromStr for SessionArtifactRelation {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "created" => Ok(Self::Created),
            "modified" => Ok(Self::Modified),
            "referenced" => Ok(Self::Referenced),
            _ => anyhow::bail!("invalid session artifact relation {value}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SessionArtifactProvenance {
    BuiltInTool,
    McpResourceLink,
    ToolMetadata,
    ToolArgument,
    AssistantMessage,
    CompatibilityInference,
}

impl std::fmt::Display for SessionArtifactProvenance {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::BuiltInTool => write!(f, "built_in_tool"),
            Self::McpResourceLink => write!(f, "mcp_resource_link"),
            Self::ToolMetadata => write!(f, "tool_metadata"),
            Self::ToolArgument => write!(f, "tool_argument"),
            Self::AssistantMessage => write!(f, "assistant_message"),
            Self::CompatibilityInference => write!(f, "compatibility_inference"),
        }
    }
}

impl std::str::FromStr for SessionArtifactProvenance {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "built_in_tool" => Ok(Self::BuiltInTool),
            "mcp_resource_link" => Ok(Self::McpResourceLink),
            "tool_metadata" => Ok(Self::ToolMetadata),
            "tool_argument" => Ok(Self::ToolArgument),
            "assistant_message" => Ok(Self::AssistantMessage),
            "compatibility_inference" => Ok(Self::CompatibilityInference),
            _ => anyhow::bail!("invalid session artifact provenance {value}"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SessionArtifact {
    pub session_id: String,
    pub display_path: String,
    pub resolved_path: String,
    pub base_working_dir: String,
    pub workspace_id: Option<String>,
    pub mime_type: Option<String>,
    pub relation: SessionArtifactRelation,
    pub provenance: SessionArtifactProvenance,
    pub source_id: Option<String>,
    pub first_seen_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredArtifact {
    pub display_path: String,
    pub resolved_path: String,
    pub base_working_dir: String,
    pub workspace_id: Option<String>,
    pub mime_type: Option<String>,
    pub relation: SessionArtifactRelation,
    pub provenance: SessionArtifactProvenance,
    pub source_id: Option<String>,
}

impl DiscoveredArtifact {
    pub fn from_path(
        value: &str,
        working_dir: &Path,
        workspace_id: Option<&str>,
        mime_type: Option<String>,
        relation: SessionArtifactRelation,
        provenance: SessionArtifactProvenance,
        source_id: Option<&str>,
    ) -> Option<Self> {
        let display_path = local_path_from_uri(value)?;
        let path = Path::new(&display_path);
        if path
            .components()
            .any(|component| component == Component::ParentDir)
        {
            return None;
        }
        let resolved = if path.is_absolute() {
            lexical_normalize(path)
        } else {
            lexical_normalize(&working_dir.join(path))
        };
        let working_dir = lexical_normalize(working_dir);
        if matches!(
            provenance,
            SessionArtifactProvenance::AssistantMessage
                | SessionArtifactProvenance::CompatibilityInference
        ) && !resolved.starts_with(&working_dir)
        {
            return None;
        }
        Some(Self {
            display_path,
            resolved_path: resolved.to_string_lossy().to_string(),
            base_working_dir: working_dir.to_string_lossy().to_string(),
            workspace_id: workspace_id.map(str::to_string),
            mime_type,
            relation,
            provenance,
            source_id: source_id.map(str::to_string),
        })
    }
}

fn lexical_normalize(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            _ => normalized.push(component.as_os_str()),
        }
    }
    normalized
}

pub fn discover_from_successful_tool(
    tool_call: &CallToolRequestParams,
    result: &CallToolResult,
    working_dir: &Path,
    workspace_id: Option<&str>,
    source_id: Option<&str>,
) -> Vec<DiscoveredArtifact> {
    if result.is_error == Some(true) {
        return Vec::new();
    }

    let mut discovered = Vec::new();
    let short_name = tool_call
        .name
        .rsplit("__")
        .next()
        .unwrap_or(tool_call.name.as_ref());
    if matches!(short_name, "write" | "edit") {
        if let Some(path) = tool_call
            .arguments
            .as_ref()
            .and_then(|arguments| arguments.get("path"))
            .and_then(|path| path.as_str())
        {
            push_path(
                &mut discovered,
                path,
                working_dir,
                workspace_id,
                None,
                if short_name == "write" {
                    SessionArtifactRelation::Created
                } else {
                    SessionArtifactRelation::Modified
                },
                SessionArtifactProvenance::BuiltInTool,
                source_id,
            );
        }
    }

    for content in &result.content {
        match &content.raw {
            RawContent::ResourceLink(resource) => push_path(
                &mut discovered,
                &resource.uri,
                working_dir,
                workspace_id,
                resource.mime_type.clone(),
                SessionArtifactRelation::Referenced,
                SessionArtifactProvenance::McpResourceLink,
                source_id,
            ),
            RawContent::Resource(resource) => match &resource.resource {
                ResourceContents::TextResourceContents { uri, mime_type, .. }
                | ResourceContents::BlobResourceContents { uri, mime_type, .. } => push_path(
                    &mut discovered,
                    uri,
                    working_dir,
                    workspace_id,
                    mime_type.clone(),
                    SessionArtifactRelation::Referenced,
                    SessionArtifactProvenance::McpResourceLink,
                    source_id,
                ),
            },
            _ => {}
        }
    }

    if let Some(meta) = result.meta.as_ref().map(|meta| &meta.0) {
        discover_metadata_paths(meta, &mut discovered, working_dir, workspace_id, source_id);
    }

    if is_mutating_tool(tool_call) {
        if let Some(arguments) = tool_call.arguments.as_ref() {
            discover_argument_paths(
                arguments,
                &mut discovered,
                working_dir,
                workspace_id,
                source_id,
            );
        }
    }

    deduplicate(discovered)
}

pub fn discover_from_assistant_markdown(
    markdown: &str,
    working_dir: &Path,
    workspace_id: Option<&str>,
    source_id: Option<&str>,
    provenance: SessionArtifactProvenance,
) -> Vec<DiscoveredArtifact> {
    let mut candidates = Vec::new();
    for capture in regex::Regex::new(r#"\]\(\s*(?:<([^>\n]+)>|([^\n)]+))\s*\)|`([^`\n]+)`"#)
        .expect("artifact markdown regex")
        .captures_iter(markdown)
    {
        if let Some(value) = capture
            .get(1)
            .or_else(|| capture.get(2))
            .or_else(|| capture.get(3))
        {
            let value = value.as_str().trim();
            if looks_like_file_path(value) {
                push_path(
                    &mut candidates,
                    value,
                    working_dir,
                    workspace_id,
                    None,
                    SessionArtifactRelation::Referenced,
                    provenance,
                    source_id,
                );
            }
        }
    }
    deduplicate(candidates)
}

fn discover_metadata_paths(
    object: &serde_json::Map<String, serde_json::Value>,
    discovered: &mut Vec<DiscoveredArtifact>,
    working_dir: &Path,
    workspace_id: Option<&str>,
    source_id: Option<&str>,
) {
    for key in [
        "artifacts",
        "artifact",
        "output",
        "outputPath",
        "output_path",
    ] {
        if let Some(value) = object.get(key) {
            discover_metadata_value(value, discovered, working_dir, workspace_id, source_id);
        }
    }
}

fn discover_metadata_value(
    value: &serde_json::Value,
    discovered: &mut Vec<DiscoveredArtifact>,
    working_dir: &Path,
    workspace_id: Option<&str>,
    source_id: Option<&str>,
) {
    match value {
        serde_json::Value::String(path) => push_path(
            discovered,
            path,
            working_dir,
            workspace_id,
            None,
            SessionArtifactRelation::Referenced,
            SessionArtifactProvenance::ToolMetadata,
            source_id,
        ),
        serde_json::Value::Array(values) => {
            for value in values {
                discover_metadata_value(value, discovered, working_dir, workspace_id, source_id);
            }
        }
        serde_json::Value::Object(object) => {
            let mime_type = object
                .get("mimeType")
                .or_else(|| object.get("mime_type"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            for key in ["path", "file", "filePath", "file_path", "uri"] {
                if let Some(path) = object.get(key).and_then(serde_json::Value::as_str) {
                    push_path(
                        discovered,
                        path,
                        working_dir,
                        workspace_id,
                        mime_type.clone(),
                        SessionArtifactRelation::Referenced,
                        SessionArtifactProvenance::ToolMetadata,
                        source_id,
                    );
                }
            }
            for key in ["artifacts", "outputs", "resources"] {
                if let Some(nested) = object.get(key) {
                    discover_metadata_value(
                        nested,
                        discovered,
                        working_dir,
                        workspace_id,
                        source_id,
                    );
                }
            }
        }
        _ => {}
    }
}

fn discover_argument_paths(
    object: &serde_json::Map<String, serde_json::Value>,
    discovered: &mut Vec<DiscoveredArtifact>,
    working_dir: &Path,
    workspace_id: Option<&str>,
    source_id: Option<&str>,
) {
    for (key, value) in object {
        let normalized = normalize_argument_key(key);
        if matches!(
            normalized.as_str(),
            "destination"
                | "destination_path"
                | "file"
                | "file_path"
                | "output"
                | "output_file"
                | "output_path"
                | "path"
        ) {
            for path in string_values(value) {
                push_path(
                    discovered,
                    path,
                    working_dir,
                    workspace_id,
                    None,
                    SessionArtifactRelation::Modified,
                    SessionArtifactProvenance::ToolArgument,
                    source_id,
                );
            }
        }
        if let serde_json::Value::Object(nested) = value {
            discover_argument_paths(nested, discovered, working_dir, workspace_id, source_id);
        } else if let serde_json::Value::Array(values) = value {
            for nested in values.iter().filter_map(serde_json::Value::as_object) {
                discover_argument_paths(nested, discovered, working_dir, workspace_id, source_id);
            }
        }
    }
}

fn normalize_argument_key(key: &str) -> String {
    let mut normalized = String::new();
    for character in key.chars() {
        if character.is_ascii_uppercase() {
            if !normalized.is_empty() {
                normalized.push('_');
            }
            normalized.push(character.to_ascii_lowercase());
        } else if character == '-' || character == ' ' {
            normalized.push('_');
        } else {
            normalized.push(character.to_ascii_lowercase());
        }
    }
    normalized
}

fn string_values(value: &serde_json::Value) -> Vec<&str> {
    match value {
        serde_json::Value::String(value) => vec![value],
        serde_json::Value::Array(values) => {
            values.iter().filter_map(|value| value.as_str()).collect()
        }
        _ => Vec::new(),
    }
}

fn is_mutating_tool(tool_call: &CallToolRequestParams) -> bool {
    let name = tool_call.name.to_lowercase();
    [
        "write", "edit", "create", "save", "export", "render", "convert", "generate",
    ]
    .iter()
    .any(|verb| name.rsplit("__").next().unwrap_or(&name) == *verb || name.contains(verb))
}

fn looks_like_file_path(value: &str) -> bool {
    let value = value.trim();
    if value.is_empty() || value.contains('\n') || value.contains('\0') {
        return false;
    }
    if value.starts_with("file:") {
        return true;
    }
    let path = Path::new(value);
    path.extension().is_some() && !value.contains("://")
}

fn local_path_from_uri(value: &str) -> Option<String> {
    let value = value.trim();
    if let Some(path) = value.strip_prefix("file://") {
        return urlencoding::decode(path).ok().map(|path| path.into_owned());
    }
    if value.contains("://") {
        return None;
    }
    Some(value.to_string())
}

#[allow(clippy::too_many_arguments)]
fn push_path(
    discovered: &mut Vec<DiscoveredArtifact>,
    value: &str,
    working_dir: &Path,
    workspace_id: Option<&str>,
    mime_type: Option<String>,
    relation: SessionArtifactRelation,
    provenance: SessionArtifactProvenance,
    source_id: Option<&str>,
) {
    if let Some(artifact) = DiscoveredArtifact::from_path(
        value,
        working_dir,
        workspace_id,
        mime_type,
        relation,
        provenance,
        source_id,
    ) {
        discovered.push(artifact);
    }
}

fn deduplicate(discovered: Vec<DiscoveredArtifact>) -> Vec<DiscoveredArtifact> {
    let mut seen = HashSet::new();
    discovered
        .into_iter()
        .filter(|artifact| seen.insert(artifact.resolved_path.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::{
        CallToolRequestParams, CallToolResult, Content, JsonObject, Meta, RawResource,
    };

    #[test]
    fn session_artifact_successful_write_is_discovered_but_failed_write_is_not() {
        let mut arguments = JsonObject::new();
        arguments.insert("path".to_string(), serde_json::json!("src/main.rs"));
        let call = CallToolRequestParams::new("developer__write").with_arguments(arguments);
        let success = CallToolResult::success(vec![Content::text("Created src/main.rs")]);
        let artifacts = discover_from_successful_tool(
            &call,
            &success,
            Path::new("/workspace"),
            None,
            Some("tool-1"),
        );
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].resolved_path, "/workspace/src/main.rs");
        assert_eq!(artifacts[0].relation, SessionArtifactRelation::Created);

        let failed = CallToolResult::error(vec![Content::text("failed")]);
        assert!(discover_from_successful_tool(
            &call,
            &failed,
            Path::new("/workspace"),
            None,
            Some("tool-1")
        )
        .is_empty());
    }

    #[test]
    fn session_artifact_resource_links_and_markdown_references_are_discovered() {
        let call = CallToolRequestParams::new("reports__build");
        let resource = RawResource::new("file:///workspace/report.pdf", "report")
            .with_mime_type("application/pdf");
        let result = CallToolResult::success(vec![Content::resource_link(resource)]);
        let artifacts = discover_from_successful_tool(
            &call,
            &result,
            Path::new("/workspace"),
            None,
            Some("tool-1"),
        );
        assert_eq!(artifacts[0].mime_type.as_deref(), Some("application/pdf"));

        let markdown = discover_from_assistant_markdown(
            "See `src/lib.rs` and [report](<docs/report.md>).",
            Path::new("/workspace"),
            None,
            Some("message-1"),
            SessionArtifactProvenance::AssistantMessage,
        );
        assert_eq!(markdown.len(), 2);
    }

    #[test]
    fn session_artifact_structured_tool_metadata_is_discovered() {
        let call = CallToolRequestParams::new("reports__build");
        let mut result = CallToolResult::success(vec![]);
        result.meta = Some(Meta(
            serde_json::json!({
                "artifacts": [{"path": "output/report.pdf", "mimeType": "application/pdf"}]
            })
            .as_object()
            .unwrap()
            .clone(),
        ));

        let artifacts = discover_from_successful_tool(
            &call,
            &result,
            Path::new("/workspace"),
            None,
            Some("tool-1"),
        );
        assert_eq!(artifacts.len(), 1);
        assert_eq!(artifacts[0].display_path, "output/report.pdf");
        assert_eq!(artifacts[0].mime_type.as_deref(), Some("application/pdf"));
        assert_eq!(
            artifacts[0].provenance,
            SessionArtifactProvenance::ToolMetadata
        );
    }

    #[test]
    fn session_artifact_traversal_and_remote_uris_are_not_discovered() {
        assert!(DiscoveredArtifact::from_path(
            "../secret.txt",
            Path::new("/workspace"),
            None,
            None,
            SessionArtifactRelation::Referenced,
            SessionArtifactProvenance::AssistantMessage,
            None,
        )
        .is_none());
        assert!(DiscoveredArtifact::from_path(
            "https://example.com/report.pdf",
            Path::new("/workspace"),
            None,
            None,
            SessionArtifactRelation::Referenced,
            SessionArtifactProvenance::AssistantMessage,
            None,
        )
        .is_none());
    }

    #[test]
    fn assistant_absolute_paths_cannot_escape_the_working_directory() {
        assert!(DiscoveredArtifact::from_path(
            "/outside/private-notes.txt",
            Path::new("/workspace"),
            None,
            None,
            SessionArtifactRelation::Referenced,
            SessionArtifactProvenance::AssistantMessage,
            None,
        )
        .is_none());

        let inside = DiscoveredArtifact::from_path(
            "/workspace/reports/result.txt",
            Path::new("/workspace"),
            None,
            None,
            SessionArtifactRelation::Referenced,
            SessionArtifactProvenance::AssistantMessage,
            None,
        )
        .expect("an assistant reference inside the working directory remains discoverable");
        assert_eq!(inside.resolved_path, "/workspace/reports/result.txt");

        assert!(DiscoveredArtifact::from_path(
            "/outside/tool-output.txt",
            Path::new("/workspace"),
            None,
            None,
            SessionArtifactRelation::Created,
            SessionArtifactProvenance::BuiltInTool,
            None,
        )
        .is_some());
    }
}
