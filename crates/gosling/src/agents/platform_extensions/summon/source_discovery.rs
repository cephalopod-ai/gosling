// Owns agent source discovery, capability-policy resolution, and subagent instruction text.
// Extracted from `summon.rs` in a behavior-preserving modularization.
// The `summon` compatibility facade re-exports `discover_filesystem_sources`.

use super::*;

pub(super) fn kind_plural(kind: SourceType) -> &'static str {
    match kind {
        SourceType::Agent => "Agents",
        _ => "Other",
    }
}

#[derive(Debug, Deserialize)]
pub(super) struct AgentMetadata {
    pub(super) name: String,
    #[serde(default)]
    pub(super) description: Option<String>,
    #[serde(default)]
    pub(super) model: Option<String>,
    #[serde(default)]
    pub(super) capabilities: Option<DelegateCapabilityPolicy>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct DelegateCapabilityPolicy {
    pub(super) version: u32,
    #[serde(default)]
    pub(super) extensions: Vec<String>,
}

/// What a delegate runs: instructions/prompt for the subagent plus an
/// optional model override from the agent file's frontmatter.
#[derive(Debug, Default, Clone)]
pub(super) struct DelegateSpec {
    pub(super) instructions: Option<String>,
    pub(super) prompt: Option<String>,
    pub(super) model: Option<String>,
    pub(super) role_extensions: Option<Vec<String>>,
}

pub(super) fn validate_capability_policy(
    policy: Option<DelegateCapabilityPolicy>,
) -> Result<Vec<String>, String> {
    let Some(policy) = policy else {
        return Ok(Vec::new());
    };
    if policy.version != 1 {
        return Err(format!(
            "Unsupported delegate capability policy version {} (expected 1)",
            policy.version
        ));
    }

    let mut extensions = Vec::with_capacity(policy.extensions.len());
    for name in policy.extensions {
        let name = name.trim();
        if name.is_empty() {
            return Err("Delegate capability policy contains an empty extension name".to_string());
        }
        if !extensions.iter().any(|existing| existing == name) {
            extensions.push(name.to_string());
        }
    }
    Ok(extensions)
}

pub(super) fn resolve_delegate_extensions(
    parent_extensions: Vec<crate::agents::ExtensionConfig>,
    spec: &DelegateSpec,
    requested_extensions: Option<&[String]>,
) -> Result<Vec<crate::agents::ExtensionConfig>, String> {
    let role_extensions = spec.role_extensions.as_deref();
    let desired = match (role_extensions, requested_extensions) {
        (Some(role), Some(requested)) => {
            let denied: Vec<&str> = requested
                .iter()
                .filter(|name| !role.iter().any(|allowed| allowed == *name))
                .map(String::as_str)
                .collect();
            if !denied.is_empty() {
                return Err(format!(
                    "Delegate requested extension(s) outside the role capability policy: {}",
                    denied.join(", ")
                ));
            }
            requested
        }
        (Some(role), None) => role,
        (None, Some(requested)) => {
            // No role policy declared, so nothing bounds this list except the
            // parent's own extensions — the *model* chose it. Since Auto no
            // longer grants execution or write authority without an explicit
            // user permission (SEC-GOS-003), the child cannot turn a
            // model-picked extension into shell or write access on its own.
            // What remained wrong was that the choice was invisible: record it
            // so an operator reviewing a session can see an unbounded grant
            // happened and which extensions it covered. (LLM-GSL-010)
            tracing::warn!(
                security.event_type = "delegate_extensions_unbounded",
                security.extensions = %requested.join(","),
                "delegate spec declares no role_extensions policy; granting the \
                 model-requested extension list, bounded only by the parent session"
            );
            requested
        }
        (None, None) => &[],
    };

    let available_names: Vec<String> = parent_extensions
        .iter()
        .map(crate::agents::ExtensionConfig::name)
        .collect();
    let unavailable: Vec<&str> = desired
        .iter()
        .filter(|name| !available_names.iter().any(|available| available == *name))
        .map(String::as_str)
        .collect();
    if !unavailable.is_empty() {
        return Err(format!(
            "Delegate requested extension(s) unavailable in the parent session: {}",
            unavailable.join(", ")
        ));
    }

    Ok(parent_extensions
        .into_iter()
        .filter(|extension| desired.iter().any(|name| name == &extension.name()))
        .collect())
}

pub(super) fn delegate_authority_summary(extensions: &[crate::agents::ExtensionConfig]) -> String {
    if extensions.is_empty() {
        "none".to_string()
    } else {
        extensions
            .iter()
            .map(crate::agents::ExtensionConfig::name)
            .collect::<Vec<_>>()
            .join(", ")
    }
}

pub(super) fn parse_agent_content(content: &str, path: &Path, global: bool) -> Option<SourceEntry> {
    let (metadata, body): (AgentMetadata, String) = match parse_frontmatter(content) {
        Ok(Some(parsed)) => parsed,
        Ok(None) => return None,
        Err(e) => {
            // Missing fields means this file has valid YAML but isn't an agent — skip silently.
            // Only warn on actual YAML syntax errors.
            if e.to_string().contains("missing field") {
                return None;
            }
            warn!("Failed to parse agent file {}: {}", path.display(), e);
            return None;
        }
    };

    let description = metadata.description.unwrap_or_else(|| {
        let model_info = metadata
            .model
            .as_ref()
            .map(|m| format!(" ({})", m))
            .unwrap_or_default();
        format!("Agent{}", model_info)
    });

    Some(SourceEntry {
        source_type: SourceType::Agent,
        name: metadata.name,
        description,
        content: body,
        path: path.to_string_lossy().into_owned(),
        global,
        writable: true,
        supporting_files: Vec::new(),
        properties: std::collections::HashMap::new(),
    })
}

pub(super) fn scan_agents_from_dir(
    dir: &Path,
    sources: &mut Vec<SourceEntry>,
    seen: &mut std::collections::HashSet<String>,
    global: bool,
) {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext != "md" {
            continue;
        }

        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                warn!("Failed to read agent file {}: {}", path.display(), e);
                continue;
            }
        };

        if let Some(source) = parse_agent_content(&content, &path, global) {
            if !seen.contains(&source.name) {
                seen.insert(source.name.clone());
                sources.push(source);
            }
        }
    }
}

pub fn discover_filesystem_sources(working_dir: &Path) -> Vec<SourceEntry> {
    let mut sources: Vec<SourceEntry> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    let home = dirs::home_dir();
    let config = Paths::config_dir();

    let local_agent_dirs: Vec<PathBuf> = vec![
        working_dir.join(".gosling/agents"),
        working_dir.join(".claude/agents"),
        working_dir.join(".agents/agents"),
    ];

    let global_agent_dirs: Vec<PathBuf> = [
        home.as_ref().map(|h| h.join(".gosling/agents")),
        home.as_ref().map(|h| h.join(".agents/agents")),
        Some(config.join("agents")),
        home.as_ref().map(|h| h.join(".claude/agents")),
    ]
    .into_iter()
    .flatten()
    .collect();

    for dir in local_agent_dirs {
        // Repo-committed: `global: false`. `build_spec_from_agent` uses this to
        // refuse a capability policy declared here (AOC-GOS-004).
        scan_agents_from_dir(&dir, &mut sources, &mut seen, false);
    }

    for dir in global_agent_dirs {
        scan_agents_from_dir(&dir, &mut sources, &mut seen, true);
    }

    sources
}

pub(super) fn build_instructions_with_context(context: &str, instructions: &str) -> String {
    let mut result = format!("# Reference Context\n\n{}", context);
    if !instructions.is_empty() {
        result.push_str(&format!("\n\n# Task Instructions\n\n{}", instructions));
    }
    result
}

pub(super) fn build_subagent_instructions(session: Option<&crate::session::Session>) -> String {
    let Some(session) = session else {
        return String::new();
    };

    // filter the sources down to what we want even though currently that is what we get
    let mut sources: Vec<SourceEntry> = discover_filesystem_sources(&session.working_dir)
        .into_iter()
        .filter(|s| matches!(s.source_type, SourceType::Agent))
        .collect();

    if sources.is_empty() {
        return String::new();
    }

    sources.sort_by(|a, b| (&a.source_type, &a.name).cmp(&(&b.source_type, &b.name)));
    let subagents: Vec<&SourceEntry> = sources.iter().collect();

    let names = subagents
        .iter()
        .map(|s| s.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");

    let mut out = String::new();
    out.push_str(
        "\n\nThe following named subagents are available in this session and \
         can be invoked through the `delegate` tool (run as a subagent) or \
         the `load` tool (read their instructions into your own context):\n",
    );

    let mut current_kind: Option<SourceType> = None;
    for s in &subagents {
        if current_kind != Some(s.source_type) {
            out.push_str(&format!("\n{}:", kind_plural(s.source_type)));
            current_kind = Some(s.source_type);
        }
        out.push_str(&format!(
            "\n• {} — {}",
            s.name,
            safe_truncate(&s.description, SUBAGENT_DESCRIPTION_BUDGET)
        ));
    }

    out.push_str(&format!(
        "\n\nWhen to call a subagent (one of [{names}]):\n\
         • `@<name>` in the user's message — always call that subagent.\n\
         • The user mentions a subagent by name without `@` — infer from \
         context whether they want it invoked, and if so, call it.\n\
         • The user's request strongly matches a subagent's description — \
         call it.\n\n\
         Calling a subagent normally means `delegate(source: \"<name>\", \
         instructions: ...)`, which runs it as an isolated subagent and \
         returns its result. Use `load(source: \"<name>\")` instead if you \
         only want to read the subagent's instructions into your own \
         context. For long-running work, pass `async: true` to `delegate` — \
         it returns a task id immediately, and you collect the result later \
         with `load(source: \"<task_id>\")`, which waits for completion.",
    ));

    out
}
