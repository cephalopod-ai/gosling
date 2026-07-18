use crate::config::GoslingMode;
use crate::conversation::message::{Message, ToolRequest};
use crate::session::SessionManager;
use crate::tool_inspection::{InspectionAction, InspectionResult, ToolInspector};
use anyhow::Result;
use async_trait::async_trait;
use rmcp::model::CallToolRequestParams;
use std::ffi::OsString;
use std::io::ErrorKind;
use std::path::Component;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Flags tool calls that touch a path outside every working directory
/// configured for the session. Opt-in and off by default (see
/// `Session::restrict_tools_to_working_dirs`); when off this inspector never
/// produces a result, so it never changes behavior. When on, it only ever
/// requires approval with an explanatory message — it never denies outright.
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
        if !session.restrict_tools_to_working_dirs {
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
    let path = Path::new(value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    }
}

/// Returns the first out-of-scope path referenced by this tool call, if any
/// can be confidently determined. Only flags calls with an explicit `path`
/// argument, or shell commands referencing an absolute path — ambiguous
/// calls (e.g. a shell command with no absolute-path token) are left alone
/// rather than guessed at.
fn out_of_scope_path(
    tool_call: &CallToolRequestParams,
    working_dir: &Path,
    allowed_dirs: &[PathBuf],
) -> Result<Option<PathBuf>> {
    let Some(args) = tool_call.arguments.as_ref() else {
        return Ok(None);
    };

    for key in ["path", "file", "file_path", "filePath"] {
        if let Some(value) = args.get(key).and_then(|v| v.as_str()) {
            let resolved = resolve(value, working_dir);
            if !is_within_any(&resolved, allowed_dirs)? {
                return Ok(Some(canonicalize_potential_path(&resolved)?));
            }
        }
    }

    if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
        for token in shell_words::split(command).unwrap_or_default() {
            if !token.starts_with('/') {
                continue;
            }
            let resolved = PathBuf::from(&token);
            if !is_within_any(&resolved, allowed_dirs)? {
                return Ok(Some(canonicalize_potential_path(&resolved)?));
            }
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
