//! Tool-call identity, location, chain, and fallback presentation metadata.
//!
//! Maintainers: derive metadata from trusted tool state and bound client-facing values.
//! Clients: tool titles, locations, and chain metadata retain their existing wire shape.

use super::*;

pub(super) fn get_requested_line(arguments: Option<&rmcp::model::JsonObject>) -> Option<u32> {
    arguments
        .and_then(|args| args.get("line"))
        .and_then(|v| v.as_u64())
        .map(|l| l as u32)
}

pub(super) fn is_developer_file_tool(tool_name: &str) -> bool {
    matches!(tool_name, "read" | "write" | "edit")
}

pub(super) fn extract_locations_from_meta(
    tool_response: &crate::conversation::message::ToolResponse,
) -> Option<Vec<ToolCallLocation>> {
    let result = tool_response.tool_result.as_ref().ok()?;
    let meta = result.meta.as_ref()?;
    let locations_val = meta.get("tool_locations")?;
    let entries: Vec<serde_json::Value> = serde_json::from_value(locations_val.clone()).ok()?;
    let locations = entries
        .into_iter()
        .filter_map(|entry| {
            let path = entry.get("path")?.as_str()?;
            let line = entry.get("line").and_then(|v| v.as_u64()).map(|l| l as u32);
            Some(ToolCallLocation::new(presentation::project_location(path)).line(line))
        })
        .collect::<Vec<_>>();
    if locations.is_empty() {
        None
    } else {
        Some(locations)
    }
}

pub(super) fn extract_tool_locations(
    tool_request: &crate::conversation::message::ToolRequest,
    tool_response: &crate::conversation::message::ToolResponse,
) -> Vec<ToolCallLocation> {
    let mut locations = Vec::new();

    if let Ok(tool_call) = &tool_request.tool_call {
        if !is_developer_file_tool(tool_call.name.as_ref()) {
            return locations;
        }

        let tool_name = tool_call.name.as_ref();
        let path_str = tool_call
            .arguments
            .as_ref()
            .and_then(|args| args.get("path"))
            .and_then(|p| p.as_str());

        if let Some(path_str) = path_str {
            if matches!(tool_name, "read") {
                let line = get_requested_line(tool_call.arguments.as_ref());
                locations.push(
                    ToolCallLocation::new(presentation::project_location(path_str)).line(line),
                );
                return locations;
            }

            if matches!(tool_name, "write" | "edit") {
                locations
                    .push(ToolCallLocation::new(presentation::project_location(path_str)).line(1));
                return locations;
            }

            let command = tool_call
                .arguments
                .as_ref()
                .and_then(|args| args.get("command"))
                .and_then(|c| c.as_str());

            if let Ok(result) = &tool_response.tool_result {
                for content in &result.content {
                    if let RawContent::Text(text_content) = &content.raw {
                        let text = &text_content.text;

                        match command {
                            Some("view") => {
                                let line = extract_view_line_range(text)
                                    .map(|range| range.0 as u32)
                                    .or(Some(1));
                                locations.push(
                                    ToolCallLocation::new(presentation::project_location(path_str))
                                        .line(line),
                                );
                            }
                            Some("str_replace") | Some("insert") => {
                                let line = extract_first_line_number(text)
                                    .map(|l| l as u32)
                                    .or(Some(1));
                                locations.push(
                                    ToolCallLocation::new(presentation::project_location(path_str))
                                        .line(line),
                                );
                            }
                            Some("write") => {
                                locations.push(
                                    ToolCallLocation::new(presentation::project_location(path_str))
                                        .line(1),
                                );
                            }
                            _ => {
                                locations.push(
                                    ToolCallLocation::new(presentation::project_location(path_str))
                                        .line(1),
                                );
                            }
                        }
                        break;
                    }
                }
            }

            if locations.is_empty() {
                locations
                    .push(ToolCallLocation::new(presentation::project_location(path_str)).line(1));
            }
        }
    }

    locations
}

fn extract_view_line_range(text: &str) -> Option<(usize, usize)> {
    let re = regex::Regex::new(r"\(lines (\d+)-(\d+|end)\)").ok()?;
    if let Some(caps) = re.captures(text) {
        let start = caps.get(1)?.as_str().parse::<usize>().ok()?;
        let end = if caps.get(2)?.as_str() == "end" {
            start
        } else {
            caps.get(2)?.as_str().parse::<usize>().ok()?
        };
        return Some((start, end));
    }
    None
}

fn extract_first_line_number(text: &str) -> Option<usize> {
    let re = regex::Regex::new(r"```[^\n]*\n(\d+):").ok()?;
    if let Some(caps) = re.captures(text) {
        return caps.get(1)?.as_str().parse::<usize>().ok();
    }
    None
}

pub(super) fn read_resource_link(link: ResourceLink) -> Option<String> {
    let url = Url::parse(&link.uri).ok()?;
    if url.scheme() == "file" {
        let path = url.to_file_path().ok()?;
        let contents = fs::read_to_string(&path).ok()?;

        Some(format!(
            "\n\n# {}\n```\n{}\n```",
            path.to_string_lossy(),
            contents
        ))
    } else {
        None
    }
}

pub(super) fn format_tool_name(tool_name: &str) -> String {
    if let Some((extension, tool)) = tool_name.split_once("__") {
        format!(
            "{}: {}",
            extension.replace('_', " "),
            tool.replace('_', " ")
        )
    } else {
        tool_name.replace('_', " ")
    }
}

/// Build a short fallback title from the tool name and arguments by extracting
/// the most useful value (file path, command, query, url, etc.).
pub(super) fn summarize_tool_call(
    tool_name: &str,
    arguments: Option<&serde_json::Value>,
) -> String {
    let base = format_tool_name(tool_name);

    let detail = arguments.and_then(|args| {
        let obj = args.as_object()?;
        let keys = [
            "path", "file", "command", "query", "url", "uri", "name", "pattern", "source",
        ];
        for key in &keys {
            if let Some(v) = obj.get(*key) {
                let s = match v {
                    serde_json::Value::String(s) => s.clone(),
                    other => other.to_string(),
                };
                if !s.is_empty() {
                    let first_line = s.lines().next().unwrap_or(&s);
                    if first_line.len() > 60 {
                        return Some(format!("{}…", crate::utils::safe_truncate(first_line, 57)));
                    }
                    return Some(first_line.to_string());
                }
            }
        }
        None
    });

    match detail {
        Some(d) => format!("{base} · {d}"),
        None => base,
    }
}

pub(super) fn tool_call_identity_meta(tool_request: &ToolRequest) -> Option<Meta> {
    let tool_call = tool_request.tool_call.as_ref().ok()?;
    let tool_name = presentation::project_identifier(tool_call.name.as_ref());
    let extension_name = tool_request
        .tool_meta
        .as_ref()
        .and_then(|meta| meta.get("gosling_extension"))
        .and_then(serde_json::Value::as_str)
        .map(presentation::project_identifier)
        .or_else(|| {
            tool_name
                .split_once("__")
                .map(|(extension_name, _)| presentation::project_identifier(extension_name))
        });

    let mut tool_call_meta = serde_json::Map::new();
    tool_call_meta.insert("toolName".to_string(), serde_json::Value::String(tool_name));
    if let Some(extension_name) = extension_name {
        tool_call_meta.insert(
            "extensionName".to_string(),
            serde_json::Value::String(extension_name),
        );
    }

    let mut gosling_meta = serde_json::Map::new();
    gosling_meta.insert(
        "toolCall".to_string(),
        serde_json::Value::Object(tool_call_meta),
    );

    let mut meta = serde_json::Map::new();
    meta.insert(
        "gosling".to_string(),
        serde_json::Value::Object(gosling_meta),
    );
    Some(meta)
}

/// Add `gosling.toolChainSummary = { summary, count }` to a `Meta` blob,
/// preserving any existing `gosling.*` keys (e.g. `gosling.toolCall` set by
/// [`tool_call_identity_meta`]).
pub(super) fn with_tool_chain_summary_meta(
    base: Option<Meta>,
    summary: &str,
    count: usize,
) -> Option<Meta> {
    let summary = presentation::project_tool_chain_summary(summary);
    let mut meta = base.unwrap_or_default();
    let gosling_entry = meta
        .entry("gosling".to_string())
        .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));
    let gosling_obj = match gosling_entry {
        serde_json::Value::Object(obj) => obj,
        other => {
            *other = serde_json::Value::Object(serde_json::Map::new());
            match other {
                serde_json::Value::Object(obj) => obj,
                _ => unreachable!(),
            }
        }
    };
    let mut chain = serde_json::Map::new();
    chain.insert("summary".to_string(), serde_json::Value::String(summary));
    chain.insert(
        "count".to_string(),
        serde_json::Value::Number(serde_json::Number::from(count)),
    );
    gosling_obj.insert(
        "toolChainSummary".to_string(),
        serde_json::Value::Object(chain),
    );
    Some(meta)
}

pub(super) struct PendingToolCall {
    pub(super) tool_call: ToolCall,
    pub(super) identity_meta: Option<Meta>,
    pub(super) fallback_title: String,
}

/// If `buffer` holds a multi-tool run (≥ 2 tool requests), (re)register a
/// [`ToolChain`] in `chain_membership` anchored on the **first** tool's
/// message_id (the row [`SessionManager::update_tool_request_meta`] will patch
/// when persisting the LLM-generated summary). Does **not** clear the buffer
/// — chains can grow as more tools arrive (sequential tool use), so callers
/// keep accumulating and re-registering with the larger set of ids.
///
/// The buffer contains `(tool_call_id, message_id)` pairs in arrival order,
/// fed by the prompt stream loop. Sequential tool use (Bedrock/Anthropic)
/// interleaves request → response → request → response across separate
/// `AgentEvent::Message` events, so a per-event view would only see length-1
/// chains and miss the run. Tool responses are chain-neutral (they don't
/// split the run); only non-tool content (text, thinking, image, etc.) does,
/// matching the frontend's `groupContentSections` behavior.
pub(super) fn extend_chain_membership(
    buffer: &[(String, String)],
    chain_membership: &mut HashMap<String, Arc<ToolChain>>,
) {
    if buffer.len() >= 2 {
        let ids: Vec<String> = buffer.iter().map(|(id, _)| id.clone()).collect();
        let message_id = buffer[0].1.clone();
        let chain = Arc::new(ToolChain {
            ids: ids.clone(),
            message_id,
        });
        for id in ids {
            chain_membership.insert(id, chain.clone());
        }
    }
}

pub(super) fn pending_tool_call_from_request(tool_request: &ToolRequest) -> PendingToolCall {
    let tool_name = match &tool_request.tool_call {
        Ok(tool_call) => tool_call.name.to_string(),
        Err(_) => "error".to_string(),
    };
    let args_value = tool_request
        .tool_call
        .as_ref()
        .ok()
        .and_then(|tc| tc.arguments.as_ref())
        .map(|a| serde_json::Value::Object(a.clone()));
    let fallback_title = summarize_tool_call(&tool_name, args_value.as_ref());
    let identity_meta = tool_call_identity_meta(tool_request);

    // Prefer the persisted LLM-generated title when available so replay (and
    // any subsequent live initial ToolCall after the title task has already
    // resolved) emits the nice title up front, with no flash of the
    // deterministic fallback.
    let initial_title = presentation::project_tool_title(
        &tool_request
            .persisted_title()
            .map(|s| s.to_string())
            .unwrap_or_else(|| fallback_title.clone()),
    );

    let mut tool_call = ToolCall::new(
        ToolCallId::new(presentation::project_identifier(&tool_request.id)),
        initial_title,
    )
    .status(ToolCallStatus::Pending);
    if let Some(args) = args_value {
        tool_call = tool_call.raw_input(presentation::project_tool_input(&args));
    }

    PendingToolCall {
        tool_call,
        identity_meta,
        fallback_title,
    }
}
