use crate::config::GoslingMode;
use crate::conversation::message::{Message, ToolRequest};
use crate::session::SessionManager;
use crate::tool_inspection::{InspectionAction, InspectionResult, ToolInspector};
use crate::workspace::{WorkspaceFolderAccess, WorkspaceFolderPolicy};
use anyhow::Result;
use async_trait::async_trait;
use rmcp::model::CallToolRequestParams;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Enforces the session's pinned filesystem boundary. Ordinary sessions opt
/// in through `Session::restrict_tools_to_working_dirs`; workspace sessions
/// always enforce their saved policy. Out-of-scope paths require approval,
/// while mutations under read-only workspace roots are denied outright.
pub struct WorkingDirScopeInspector {
    session_manager: Arc<SessionManager>,
}

impl WorkingDirScopeInspector {
    pub fn new(session_manager: Arc<SessionManager>) -> Self {
        Self { session_manager }
    }
}

#[async_trait]
impl ToolInspector for WorkingDirScopeInspector {
    fn name(&self) -> &'static str {
        "working_dir_scope"
    }

    async fn inspect(
        &self,
        session_id: &str,
        tool_requests: &[ToolRequest],
        _messages: &[Message],
        _gosling_mode: GoslingMode,
    ) -> Result<Vec<InspectionResult>> {
        let Ok(session) = self.session_manager.get_session(session_id, false).await else {
            return Ok(Vec::new());
        };
        if !session.restrict_tools_to_working_dirs && session.workspace_context.is_none() {
            return Ok(Vec::new());
        }

        let mut allowed_dirs = Vec::with_capacity(1 + session.additional_working_dirs.len());
        allowed_dirs.push(session.working_dir.clone());
        allowed_dirs.extend(session.additional_working_dirs.iter().cloned());

        let mut results = Vec::new();
        for request in tool_requests {
            let Ok(tool_call) = &request.tool_call else {
                continue;
            };
            if let Some(context) = &session.workspace_context {
                let policy = context.effective_folder_policy();
                if is_mutating_tool_call(tool_call) {
                    if is_shell_tool(tool_call)
                        && policy
                            .roots
                            .iter()
                            .any(|root| root.access == WorkspaceFolderAccess::Read)
                    {
                        results.push(InspectionResult {
                            tool_request_id: request.id.clone(),
                            action: InspectionAction::Deny,
                            reason: "mutating shell commands cannot be safely scoped while the workspace has read-only roots; use a structured file tool or a workspace without read-only folders".to_string(),
                            confidence: 1.0,
                            inspector_name: self.name().to_string(),
                            finding_id: Some("AUD-GOS-003".to_string()),
                        });
                        continue;
                    }
                    if let Some(path) =
                        first_read_only_path(tool_call, &session.working_dir, &policy)?
                    {
                        results.push(InspectionResult {
                            tool_request_id: request.id.clone(),
                            action: InspectionAction::Deny,
                            reason: format!(
                                "workspace folder policy forbids mutation under {}",
                                path.display()
                            ),
                            confidence: 1.0,
                            inspector_name: self.name().to_string(),
                            finding_id: Some("AUD-GOS-003".to_string()),
                        });
                        continue;
                    }
                }
            }
            let Some(path) = out_of_scope_path(tool_call, &session.working_dir, &allowed_dirs)?
            else {
                continue;
            };

            let dirs_list = allowed_dirs
                .iter()
                .map(|d| d.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            results.push(InspectionResult {
                tool_request_id: request.id.clone(),
                action: InspectionAction::RequireApproval(Some(format!(
                    "\"{}\" touches {}, which is outside your working directories ({}). \
                     This session has \"restrict tools to working directories\" turned on.",
                    tool_call.name,
                    path.display(),
                    dirs_list
                ))),
                reason: "path outside configured working directories".to_string(),
                confidence: 1.0,
                inspector_name: self.name().to_string(),
                finding_id: None,
            });
        }
        Ok(results)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn auto_downgrades_require_approval(&self) -> bool {
        false
    }
}

fn normalize_resolved_path(path: PathBuf) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }
    normalized
}

fn canonicalize_potential_path(path: &Path) -> Result<PathBuf> {
    let mut existing_ancestor = path.to_path_buf();
    let mut missing_segments: Vec<OsString> = Vec::new();

    loop {
        match std::fs::canonicalize(&existing_ancestor) {
            Ok(canonical_ancestor) => {
                missing_segments.reverse();
                let resolved = missing_segments
                    .into_iter()
                    .fold(canonical_ancestor, |path, segment| path.join(segment));
                return Ok(normalize_resolved_path(resolved));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                match std::fs::symlink_metadata(&existing_ancestor) {
                    Ok(metadata) if metadata.file_type().is_symlink() => {
                        anyhow::bail!(
                            "cannot authorize path through dangling symbolic link: {}",
                            path.display()
                        );
                    }
                    Ok(_) => return Err(error.into()),
                    Err(metadata_error) if metadata_error.kind() == ErrorKind::NotFound => {}
                    Err(metadata_error) => return Err(metadata_error.into()),
                }

                let Some(name) = existing_ancestor.file_name().map(OsString::from) else {
                    return Err(error.into());
                };
                let Some(parent) = existing_ancestor.parent() else {
                    return Err(error.into());
                };
                missing_segments.push(name);
                existing_ancestor = parent.to_path_buf();
            }
            Err(error) => return Err(error.into()),
        }
    }
}

fn canonical_allowed_dirs(dirs: &[PathBuf]) -> Vec<PathBuf> {
    dirs.iter()
        .filter_map(|dir| canonicalize_potential_path(dir).ok())
        .collect()
}

fn is_within_any(path: &Path, dirs: &[PathBuf]) -> Result<bool> {
    let canonical_path = canonicalize_potential_path(path)?;
    let canonical_dirs = canonical_allowed_dirs(dirs);
    if canonical_dirs.is_empty() {
        anyhow::bail!("no working directory could be canonicalized");
    }
    Ok(canonical_dirs
        .iter()
        .any(|dir| canonical_path.starts_with(dir)))
}

fn resolve(value: &str, working_dir: &Path) -> PathBuf {
    if let Ok(url) = url::Url::parse(value) {
        if url.scheme() == "file" {
            if let Ok(path) = url.to_file_path() {
                return path;
            }
        }
    }
    for prefix in ["~/", "$HOME/", "${HOME}/"] {
        if let Some(relative) = value.strip_prefix(prefix) {
            if let Some(home) = dirs::home_dir() {
                return home.join(relative);
            }
        }
    }
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    }
}

fn argument_key_tokens(key: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut token = String::new();
    let mut previous_was_lowercase = false;
    for character in key.chars() {
        if !character.is_ascii_alphanumeric() {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
            previous_was_lowercase = false;
            continue;
        }
        if character.is_ascii_uppercase() && previous_was_lowercase && !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
        }
        token.push(character.to_ascii_lowercase());
        previous_was_lowercase = character.is_ascii_lowercase();
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    tokens
}

fn argument_key_has_path_semantics(key: &str) -> bool {
    argument_key_tokens(key).iter().any(|token| {
        matches!(
            token.as_str(),
            "path"
                | "paths"
                | "file"
                | "files"
                | "filename"
                | "filenames"
                | "directory"
                | "directories"
                | "dir"
                | "dirs"
                | "folder"
                | "folders"
                | "root"
                | "roots"
                | "cwd"
                | "uri"
                | "uris"
        )
    })
}

fn argument_key_is_text_payload(key: &str) -> bool {
    argument_key_tokens(key).iter().any(|token| {
        matches!(
            token.as_str(),
            "body" | "content" | "prompt" | "query" | "replacement" | "template" | "text"
        )
    })
}

fn looks_like_explicit_path(value: &str) -> bool {
    let bytes = value.as_bytes();
    value.starts_with('/')
        || value.starts_with("./")
        || value.starts_with("../")
        || value.starts_with("~/")
        || value.starts_with("$HOME/")
        || value.starts_with("${HOME}/")
        || value.starts_with("file://")
        || value.starts_with('\\')
        || value.starts_with(".\\")
        || value.starts_with("..\\")
        || (bytes.len() >= 3
            && bytes[0].is_ascii_alphabetic()
            && bytes[1] == b':'
            && matches!(bytes[2], b'/' | b'\\'))
}

fn path_from_shell_token(token: &str) -> Option<&str> {
    let candidate = if token.starts_with('-') {
        token.split_once('=')?.1
    } else if let Some((_, value)) = token.split_once('=') {
        if looks_like_explicit_path(value) {
            value
        } else {
            token
        }
    } else {
        token
    };
    let candidate = candidate.trim_start_matches(|character: char| {
        character.is_ascii_digit() || matches!(character, '<' | '>' | '&')
    });
    looks_like_explicit_path(candidate).then_some(candidate)
}

fn collect_referenced_paths(
    value: &serde_json::Value,
    key: &str,
    inherited_path_semantics: bool,
    working_dir: &Path,
    paths: &mut Vec<PathBuf>,
) {
    let path_semantics = inherited_path_semantics || argument_key_has_path_semantics(key);
    match value {
        serde_json::Value::String(value) => {
            if path_semantics
                || (!argument_key_is_text_payload(key) && looks_like_explicit_path(value))
            {
                paths.push(resolve(value, working_dir));
            }
        }
        serde_json::Value::Array(values) => {
            for value in values {
                collect_referenced_paths(value, key, path_semantics, working_dir, paths);
            }
        }
        serde_json::Value::Object(values) => {
            for (nested_key, value) in values {
                collect_referenced_paths(value, nested_key, false, working_dir, paths);
            }
        }
        _ => {}
    }
}

fn referenced_paths(tool_call: &CallToolRequestParams, working_dir: &Path) -> Vec<PathBuf> {
    let Some(args) = tool_call.arguments.as_ref() else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    for (key, value) in args {
        if key != "command" {
            collect_referenced_paths(value, key, false, working_dir, &mut paths);
        }
    }
    if let Some(command) = args.get("command").and_then(|value| value.as_str()) {
        let tokens = shell_words::split(command).unwrap_or_default();
        for segment in tokens
            .split(|token| matches!(token.as_str(), "|" | "&&" | "||" | ";"))
            .filter(|segment| !segment.is_empty())
        {
            for token in segment.iter().skip(1) {
                if let Some(path) = path_from_shell_token(token) {
                    paths.push(resolve(path, working_dir));
                }
            }
        }
    }
    paths
}

fn first_read_only_path(
    tool_call: &CallToolRequestParams,
    working_dir: &Path,
    policy: &WorkspaceFolderPolicy,
) -> Result<Option<PathBuf>> {
    let canonical_roots = policy
        .roots
        .iter()
        .filter_map(|root| {
            canonicalize_potential_path(Path::new(&root.path))
                .ok()
                .map(|path| (path, root.access))
        })
        .collect::<Vec<_>>();
    for path in referenced_paths(tool_call, working_dir) {
        let canonical_path = canonicalize_potential_path(&path)?;
        let access = canonical_roots
            .iter()
            .filter(|(root, _)| canonical_path.starts_with(root))
            .max_by_key(|(root, _)| root.components().count())
            .map(|(_, access)| *access);
        if access == Some(WorkspaceFolderAccess::Read) {
            return Ok(Some(canonical_path));
        }
    }
    Ok(None)
}

fn is_mutating_tool_call(tool_call: &CallToolRequestParams) -> bool {
    let name = tool_call.name.to_ascii_lowercase();
    let operation = name.rsplit("__").next().unwrap_or(&name);
    let mutation_markers = [
        "write", "edit", "delete", "remove", "create", "mkdir", "move", "copy", "rename", "patch",
        "replace", "append", "truncate", "chmod", "chown", "upload", "save",
    ];
    if mutation_markers
        .iter()
        .any(|marker| operation.contains(marker))
    {
        return true;
    }

    if is_shell_tool(tool_call) {
        let command = tool_call
            .arguments
            .as_ref()
            .and_then(|args| args.get("command"))
            .and_then(|value| value.as_str())
            .unwrap_or_default();
        return !is_confidently_read_only_shell(command);
    }

    let read_markers = [
        "read", "list", "search", "find", "grep", "query", "get", "inspect", "view", "stat",
        "preview", "fetch",
    ];
    !read_markers.iter().any(|marker| operation.contains(marker))
}

fn is_shell_tool(tool_call: &CallToolRequestParams) -> bool {
    let name = tool_call.name.to_ascii_lowercase();
    name.contains("shell") || name.contains("command") || name.contains("terminal")
}

fn is_confidently_read_only_shell(command: &str) -> bool {
    let Ok(tokens) = shell_words::split(command) else {
        return false;
    };
    if tokens.is_empty()
        || tokens
            .iter()
            .any(|token| token == ">" || token == ">>" || token.starts_with('>'))
    {
        return false;
    }
    let mut commands = tokens
        .split(|token| matches!(token.as_str(), "|" | "&&" | "||" | ";"))
        .filter(|segment| !segment.is_empty());
    commands.all(|segment| {
        let executable = Path::new(&segment[0])
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or_default();
        match executable {
            "cat" | "ls" | "pwd" | "rg" | "grep" | "head" | "tail" | "wc" | "stat" | "file" => true,
            "find" => !segment.iter().any(|token| {
                matches!(
                    token.as_str(),
                    "-delete" | "-exec" | "-execdir" | "-ok" | "-okdir"
                )
            }),
            "sed" => !segment
                .iter()
                .any(|token| token == "-i" || token.starts_with("-i")),
            "git" => segment.get(1).is_some_and(|subcommand| {
                matches!(
                    subcommand.as_str(),
                    "diff" | "grep" | "log" | "show" | "status"
                )
            }),
            _ => false,
        }
    })
}

/// Returns the first out-of-scope path referenced by this tool call, if any
/// can be confidently determined. Only flags calls with an explicit `path`
/// argument, or shell commands referencing an explicit absolute or relative
/// path. Ambiguous path-free calls are left alone rather than guessed at.
fn out_of_scope_path(
    tool_call: &CallToolRequestParams,
    working_dir: &Path,
    allowed_dirs: &[PathBuf],
) -> Result<Option<PathBuf>> {
    for resolved in referenced_paths(tool_call, working_dir) {
        if !is_within_any(&resolved, allowed_dirs)? {
            return Ok(Some(canonicalize_potential_path(&resolved)?));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::model::JsonObject;

    fn tool_call(name: &str, args: JsonObject) -> CallToolRequestParams {
        CallToolRequestParams::new(name.to_string()).with_arguments(args)
    }

    fn json_args(pairs: &[(&str, &str)]) -> JsonObject {
        pairs
            .iter()
            .map(|(k, v)| {
                (
                    (*k).to_string(),
                    serde_json::Value::String((*v).to_string()),
                )
            })
            .collect()
    }

    #[test]
    fn flags_path_outside_working_dirs() {
        let working_dir = PathBuf::from("/home/user/project");
        let allowed = vec![working_dir.clone()];
        let call = tool_call(
            "developer__text_editor__write",
            json_args(&[("path", "/etc/passwd")]),
        );

        let result = out_of_scope_path(&call, &working_dir, &allowed).unwrap();
        assert_eq!(
            result,
            Some(canonicalize_potential_path(Path::new("/etc/passwd")).unwrap())
        );
    }

    #[test]
    fn allows_path_inside_working_dir() {
        let working_dir = PathBuf::from("/home/user/project");
        let allowed = vec![working_dir.clone()];
        let call = tool_call(
            "developer__text_editor__write",
            json_args(&[("path", "/home/user/project/src/main.rs")]),
        );

        assert_eq!(
            out_of_scope_path(&call, &working_dir, &allowed).unwrap(),
            None
        );
    }

    #[test]
    fn allows_path_inside_additional_working_dir() {
        let working_dir = PathBuf::from("/home/user/project");
        let allowed = vec![working_dir.clone(), PathBuf::from("/home/user/other")];
        let call = tool_call(
            "developer__text_editor__write",
            json_args(&[("path", "/home/user/other/file.txt")]),
        );

        assert_eq!(
            out_of_scope_path(&call, &working_dir, &allowed).unwrap(),
            None
        );
    }

    #[test]
    fn allows_relative_path() {
        let working_dir = PathBuf::from("/home/user/project");
        let allowed = vec![working_dir.clone()];
        let call = tool_call(
            "developer__text_editor__write",
            json_args(&[("path", "src/main.rs")]),
        );

        assert_eq!(
            out_of_scope_path(&call, &working_dir, &allowed).unwrap(),
            None
        );
    }

    #[test]
    fn flags_relative_parent_traversal() {
        let root = tempfile::tempdir().unwrap();
        let working_dir = root.path().join("project");
        std::fs::create_dir(&working_dir).unwrap();
        let allowed = vec![working_dir.clone()];
        let call = tool_call(
            "developer__text_editor__write",
            json_args(&[("path", "../outside.txt")]),
        );

        assert_eq!(
            out_of_scope_path(&call, &working_dir, &allowed).unwrap(),
            Some(
                std::fs::canonicalize(root.path())
                    .unwrap()
                    .join("outside.txt")
            )
        );
    }

    #[test]
    fn flags_nested_path_aliases_and_explicit_paths_under_unknown_keys() {
        let root = tempfile::tempdir().unwrap();
        let working_dir = root.path().join("project");
        std::fs::create_dir(&working_dir).unwrap();
        let outside = root.path().join("outside.txt");
        let call = tool_call(
            "third_party__export",
            serde_json::from_value(serde_json::json!({
                "options": {
                    "outputFile": outside,
                    "secondaryTarget": "../also-outside.txt"
                }
            }))
            .unwrap(),
        );

        assert!(
            out_of_scope_path(&call, &working_dir, std::slice::from_ref(&working_dir))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn checks_arrays_under_path_semantic_keys() {
        let root = tempfile::tempdir().unwrap();
        let working_dir = root.path().join("project");
        std::fs::create_dir(&working_dir).unwrap();
        let call = tool_call(
            "third_party__batch",
            serde_json::from_value(serde_json::json!({
                "input_files": ["inside.txt", "../outside.txt"]
            }))
            .unwrap(),
        );

        assert!(
            out_of_scope_path(&call, &working_dir, std::slice::from_ref(&working_dir))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn does_not_treat_text_payloads_as_paths() {
        let root = tempfile::tempdir().unwrap();
        let working_dir = root.path().join("project");
        std::fs::create_dir(&working_dir).unwrap();
        let call = tool_call(
            "developer__text_editor__write",
            serde_json::from_value(serde_json::json!({
                "path": "inside.txt",
                "content": "/outside-looking prose"
            }))
            .unwrap(),
        );

        assert_eq!(
            out_of_scope_path(&call, &working_dir, std::slice::from_ref(&working_dir)).unwrap(),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn flags_existing_and_missing_paths_through_symlink_escape() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let working_dir = root.path().join("project");
        let outside = root.path().join("outside");
        std::fs::create_dir(&working_dir).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "secret").unwrap();
        symlink(&outside, working_dir.join("redirect")).unwrap();
        let allowed = vec![working_dir.clone()];

        for path in ["redirect/secret.txt", "redirect/new.txt"] {
            let call = tool_call(
                "developer__text_editor__write",
                json_args(&[("path", path)]),
            );
            assert!(out_of_scope_path(&call, &working_dir, &allowed)
                .unwrap()
                .is_some());
        }
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_fails_closed() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let working_dir = root.path().join("project");
        std::fs::create_dir(&working_dir).unwrap();
        symlink(
            working_dir.join("missing-target"),
            working_dir.join("dangling"),
        )
        .unwrap();
        let call = tool_call(
            "developer__text_editor__write",
            json_args(&[("path", "dangling/new.txt")]),
        );

        assert!(
            out_of_scope_path(&call, &working_dir, std::slice::from_ref(&working_dir)).is_err()
        );
    }

    #[test]
    fn flags_shell_command_with_absolute_path_outside_scope() {
        let working_dir = PathBuf::from("/home/user/project");
        let allowed = vec![working_dir.clone()];
        let call = tool_call(
            "developer__shell",
            json_args(&[("command", "cat /etc/passwd")]),
        );

        let result = out_of_scope_path(&call, &working_dir, &allowed).unwrap();
        assert_eq!(
            result,
            Some(canonicalize_potential_path(Path::new("/etc/passwd")).unwrap())
        );
    }

    #[test]
    fn does_not_guess_at_relative_shell_command() {
        let working_dir = PathBuf::from("/home/user/project");
        let allowed = vec![working_dir.clone()];
        let call = tool_call("developer__shell", json_args(&[("command", "ls -la src")]));

        assert_eq!(
            out_of_scope_path(&call, &working_dir, &allowed).unwrap(),
            None
        );
    }

    #[test]
    fn flags_shell_command_with_explicit_relative_parent_path() {
        let root = tempfile::tempdir().unwrap();
        let working_dir = root.path().join("project");
        std::fs::create_dir(&working_dir).unwrap();
        let call = tool_call(
            "developer__shell",
            json_args(&[("command", "rm ../valuable.txt")]),
        );

        assert!(
            out_of_scope_path(&call, &working_dir, std::slice::from_ref(&working_dir))
                .unwrap()
                .is_some()
        );
    }

    #[test]
    fn flags_shell_home_expansion_outside_scope() {
        let working_dir = tempfile::tempdir().unwrap();
        let call = tool_call(
            "developer__shell",
            json_args(&[("command", "cat ~/.ssh/id_rsa")]),
        );

        assert!(out_of_scope_path(
            &call,
            working_dir.path(),
            std::slice::from_ref(&working_dir.path().to_path_buf())
        )
        .unwrap()
        .is_some());
    }

    #[test]
    fn flags_shell_option_and_redirection_paths_outside_scope() {
        let working_dir = tempfile::tempdir().unwrap();
        let allowed = vec![working_dir.path().to_path_buf()];
        for command in [
            "tool --output=/etc/gosling-output",
            "echo data >/etc/gosling-output",
        ] {
            let call = tool_call("developer__shell", json_args(&[("command", command)]));
            assert!(out_of_scope_path(&call, working_dir.path(), &allowed)
                .unwrap()
                .is_some());
        }
    }

    #[test]
    fn flags_file_uri_outside_scope() {
        let working_dir = tempfile::tempdir().unwrap();
        let call = tool_call(
            "third_party__resource",
            json_args(&[("resourceUri", "file:///etc/passwd")]),
        );

        assert!(out_of_scope_path(
            &call,
            working_dir.path(),
            std::slice::from_ref(&working_dir.path().to_path_buf())
        )
        .unwrap()
        .is_some());
    }

    #[test]
    fn no_arguments_never_flagged() {
        let working_dir = PathBuf::from("/home/user/project");
        let allowed = vec![working_dir.clone()];
        let call = CallToolRequestParams::new("developer__todo__read".to_string());

        assert_eq!(
            out_of_scope_path(&call, &working_dir, &allowed).unwrap(),
            None
        );
    }

    fn write_request(id: &str, path: &str) -> ToolRequest {
        ToolRequest {
            id: id.to_string(),
            tool_call: Ok(tool_call(
                "developer__text_editor__write",
                json_args(&[("path", path)]),
            )),
            metadata: None,
            tool_meta: None,
        }
    }

    fn read_request(id: &str, path: &str) -> ToolRequest {
        ToolRequest {
            id: id.to_string(),
            tool_call: Ok(tool_call(
                "developer__text_editor__read",
                json_args(&[("path", path)]),
            )),
            metadata: None,
            tool_meta: None,
        }
    }

    fn shell_request(id: &str, command: &str) -> ToolRequest {
        ToolRequest {
            id: id.to_string(),
            tool_call: Ok(tool_call(
                "developer__shell",
                json_args(&[("command", command)]),
            )),
            metadata: None,
            tool_meta: None,
        }
    }

    #[tokio::test]
    async fn workspace_policy_denies_read_only_mutation_and_allows_reads_and_outputs() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        let reference = root.path().join("reference");
        let output = root.path().join("output");
        std::fs::create_dir_all(&project).unwrap();
        std::fs::create_dir_all(&reference).unwrap();
        std::fs::create_dir_all(&output).unwrap();

        let session_manager = Arc::new(SessionManager::new(root.path().to_path_buf()));
        let session = session_manager
            .create_session(
                project.clone(),
                "workspace".into(),
                crate::session::SessionType::User,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        let context = crate::workspace::WorkspaceSessionContext {
            workspace_id: "workspace".into(),
            workspace_name: "Workspace".into(),
            primary_working_folder: project.to_string_lossy().to_string(),
            folders: Vec::new(),
            product_output_folders: Vec::new(),
            folder_policy: WorkspaceFolderPolicy {
                roots: vec![
                    crate::workspace::WorkspaceFolderPolicyRoot {
                        path: project.to_string_lossy().to_string(),
                        access: WorkspaceFolderAccess::ReadWrite,
                    },
                    crate::workspace::WorkspaceFolderPolicyRoot {
                        path: reference.to_string_lossy().to_string(),
                        access: WorkspaceFolderAccess::Read,
                    },
                    crate::workspace::WorkspaceFolderPolicyRoot {
                        path: output.to_string_lossy().to_string(),
                        access: WorkspaceFolderAccess::ReadWrite,
                    },
                ],
            },
        };
        session_manager
            .update(&session.id)
            .workspace_snapshot(
                "workspace".into(),
                "Workspace".into(),
                None,
                None,
                None,
                context,
            )
            .apply()
            .await
            .unwrap();

        let inspector = WorkingDirScopeInspector::new(session_manager.clone());
        let results = inspector
            .inspect(
                &session.id,
                &[
                    write_request(
                        "write-reference",
                        reference.join("valuable.txt").to_str().unwrap(),
                    ),
                    read_request(
                        "read-reference",
                        reference.join("valuable.txt").to_str().unwrap(),
                    ),
                    write_request("write-output", output.join("report.md").to_str().unwrap()),
                    shell_request(
                        "shell-write-output",
                        &format!("touch {}", output.join("shell-report.md").display()),
                    ),
                ],
                &[],
                GoslingMode::Auto,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].tool_request_id, "write-reference");
        assert_eq!(results[0].action, InspectionAction::Deny);
        assert_eq!(results[1].tool_request_id, "shell-write-output");
        assert_eq!(results[1].action, InspectionAction::Deny);
        assert!(results[1].reason.contains("cannot be safely scoped"));
        let reloaded = session_manager
            .get_session(&session.id, false)
            .await
            .unwrap();
        assert!(reloaded.restrict_tools_to_working_dirs);
        assert!(reloaded.additional_working_dirs.contains(&reference));
        assert!(reloaded.additional_working_dirs.contains(&output));
    }

    #[test]
    fn nested_read_only_root_overrides_writable_parent_and_shell_is_fail_closed() {
        let root = tempfile::tempdir().unwrap();
        let project = root.path().join("project");
        let reference = project.join("reference");
        std::fs::create_dir_all(&reference).unwrap();
        let policy = WorkspaceFolderPolicy {
            roots: vec![
                crate::workspace::WorkspaceFolderPolicyRoot {
                    path: project.to_string_lossy().to_string(),
                    access: WorkspaceFolderAccess::ReadWrite,
                },
                crate::workspace::WorkspaceFolderPolicyRoot {
                    path: reference.to_string_lossy().to_string(),
                    access: WorkspaceFolderAccess::Read,
                },
            ],
        };
        let target = reference.join("valuable.txt");
        let shell_write = tool_call(
            "developer__shell",
            json_args(&[("command", &format!("rm {}", target.display()))]),
        );
        let shell_read = tool_call(
            "developer__shell",
            json_args(&[("command", &format!("cat {}", target.display()))]),
        );

        assert!(is_mutating_tool_call(&shell_write));
        assert_eq!(
            first_read_only_path(&shell_write, &project, &policy).unwrap(),
            Some(canonicalize_potential_path(&target).unwrap())
        );
        assert!(!is_mutating_tool_call(&shell_read));
    }

    #[tokio::test]
    async fn off_by_default_never_flags_anything() {
        let dir = tempfile::tempdir().unwrap();
        let session_manager = Arc::new(SessionManager::new(dir.path().to_path_buf()));
        let session = session_manager
            .create_session(
                dir.path().to_path_buf(),
                "test".into(),
                crate::session::SessionType::User,
                GoslingMode::default(),
            )
            .await
            .unwrap();

        let inspector = WorkingDirScopeInspector::new(session_manager);
        let results = inspector
            .inspect(
                &session.id,
                &[write_request("req-1", "/etc/passwd")],
                &[],
                GoslingMode::Auto,
            )
            .await
            .unwrap();

        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn flags_out_of_scope_write_when_restriction_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let session_manager = Arc::new(SessionManager::new(dir.path().to_path_buf()));
        let session = session_manager
            .create_session(
                dir.path().to_path_buf(),
                "test".into(),
                crate::session::SessionType::User,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        session_manager
            .update(&session.id)
            .restrict_tools_to_working_dirs(true)
            .apply()
            .await
            .unwrap();

        let inspector = WorkingDirScopeInspector::new(session_manager);
        let results = inspector
            .inspect(
                &session.id,
                &[write_request("req-1", "/etc/passwd")],
                &[],
                GoslingMode::Auto,
            )
            .await
            .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].tool_request_id, "req-1");
        match &results[0].action {
            InspectionAction::RequireApproval(Some(message)) => {
                assert!(message.contains("/etc/passwd"));
                assert!(message.contains(&dir.path().display().to_string()));
            }
            other => panic!("expected RequireApproval with a message, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn allows_in_scope_write_when_restriction_enabled() {
        let dir = tempfile::tempdir().unwrap();
        let session_manager = Arc::new(SessionManager::new(dir.path().to_path_buf()));
        let session = session_manager
            .create_session(
                dir.path().to_path_buf(),
                "test".into(),
                crate::session::SessionType::User,
                GoslingMode::default(),
            )
            .await
            .unwrap();
        session_manager
            .update(&session.id)
            .restrict_tools_to_working_dirs(true)
            .apply()
            .await
            .unwrap();

        let in_scope_path = dir.path().join("file.txt");
        let inspector = WorkingDirScopeInspector::new(session_manager);
        let results = inspector
            .inspect(
                &session.id,
                &[write_request("req-1", in_scope_path.to_str().unwrap())],
                &[],
                GoslingMode::Auto,
            )
            .await
            .unwrap();

        assert!(results.is_empty());
    }
}
