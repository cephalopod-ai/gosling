use super::*;
use rmcp::model::Content as RmcpContent;
use serde::Serialize;

pub(super) const ACP_HISTORY_PAGE_LIMIT: usize = DEFAULT_SESSION_TAIL_LIMIT;
const ACP_HISTORY_MESSAGE_MAX_BYTES: usize = 100_000;
const ACP_LIVE_TEXT_MAX_BYTES: usize = 512_000;
const ACP_LIVE_IMAGE_MAX_BYTES: usize = 4_000_000;
const ACP_TOOL_RESULT_MAX_BYTES: usize = 1_000_000;
const ACP_TOOL_RAW_OUTPUT_MAX_BYTES: usize = 256_000;
const ACP_TOOL_CONTENT_MAX_BYTES: usize = 512_000;
const ACP_TOOL_META_MAX_BYTES: usize = 64_000;
const ACP_TOOL_INPUT_MAX_BYTES: usize = 256_000;
const ACP_NOTIFICATION_VALUE_MAX_BYTES: usize = 256_000;
const ACP_TOOL_TITLE_MAX_BYTES: usize = 512;
const ACP_TOOL_CHAIN_SUMMARY_MAX_BYTES: usize = 2_048;
const ACP_IDENTIFIER_MAX_BYTES: usize = 1_024;
const ACP_LOCATION_MAX_BYTES: usize = 4_096;
const ACP_CUSTOM_RESPONSE_MAX_BYTES: usize = 6_000_000;
const MAX_HISTORY_CONTENT_ITEMS: usize = 64;

fn serialized_len<T: Serialize>(value: &T) -> usize {
    serde_json::to_vec(value).map_or(usize::MAX, |serialized| serialized.len())
}

fn prefix_at_byte_boundary(value: &str, max_bytes: usize) -> &str {
    let mut end = value.len().min(max_bytes);
    while end > 0 && !value.is_char_boundary(end) {
        end -= 1;
    }
    value.get(..end).unwrap_or_default()
}

pub(super) fn bounded_text(value: &str, max_bytes: usize, label: &str) -> String {
    if value.len() <= max_bytes {
        return value.to_string();
    }

    let notice = format!(
        "\n\n[{label} truncated for ACP presentation: {} bytes total]",
        value.len()
    );
    let prefix_budget = max_bytes.saturating_sub(notice.len());
    format!(
        "{}{}",
        prefix_at_byte_boundary(value, prefix_budget),
        notice
    )
}

pub(super) fn project_json(value: &serde_json::Value, max_bytes: usize) -> serde_json::Value {
    let Ok(serialized) = serde_json::to_string(value) else {
        return serde_json::json!({
            "_gosling": {
                "truncated": true,
                "reason": "structured output could not be serialized",
            }
        });
    };
    if serialized.len() <= max_bytes {
        return value.clone();
    }

    let original_bytes = serialized.len();
    let mut preview_bytes = max_bytes / 8;
    loop {
        let preview = prefix_at_byte_boundary(&serialized, preview_bytes);
        let projected = serde_json::json!({
            "_gosling": {
                "truncated": true,
                "originalBytes": original_bytes,
                "limitBytes": max_bytes,
            },
            "preview": preview,
        });
        if serialized_len(&projected) <= max_bytes || preview_bytes == 0 {
            return projected;
        }
        preview_bytes /= 2;
    }
}

fn project_rmcp_meta(meta: &rmcp::model::Meta) -> rmcp::model::Meta {
    if serialized_len(meta) <= ACP_TOOL_META_MAX_BYTES {
        return meta.clone();
    }

    let mut projected = rmcp::model::JsonObject::new();
    for key in ["_gosling/acp-aware", TRUSTED_TOOL_UPDATE_META_KEY] {
        if let Some(value) = meta.0.get(key) {
            projected.insert(
                key.to_string(),
                project_json(value, ACP_TOOL_META_MAX_BYTES / 2),
            );
        }
    }
    rmcp::model::Meta(projected)
}

fn omitted_content_notice(original_bytes: usize) -> RmcpContent {
    RmcpContent::text(format!(
        "[Tool content omitted from ACP presentation: {original_bytes} bytes total]"
    ))
}

fn project_rmcp_content(content: &RmcpContent, max_bytes: usize) -> RmcpContent {
    let original_bytes = serialized_len(content);
    if original_bytes <= max_bytes {
        return content.clone();
    }

    let mut projected = content.clone();
    match &mut projected.raw {
        RawContent::Text(text) => {
            text.text = bounded_text(&text.text, max_bytes / 8, "Tool text");
        }
        RawContent::Resource(resource) => {
            if let ResourceContents::TextResourceContents { text, .. } = &mut resource.resource {
                *text = bounded_text(text, max_bytes / 8, "Tool resource");
            } else {
                return omitted_content_notice(original_bytes);
            }
        }
        RawContent::Image(_) | RawContent::Audio(_) | RawContent::ResourceLink(_) => {
            return omitted_content_notice(original_bytes);
        }
    }

    if serialized_len(&projected) <= max_bytes {
        projected
    } else {
        omitted_content_notice(original_bytes)
    }
}

fn project_call_tool_result(result: &CallToolResult, max_bytes: usize) -> CallToolResult {
    let mut projected = result.clone();
    projected.meta = projected.meta.as_ref().map(project_rmcp_meta);
    projected.structured_content = projected.structured_content.as_ref().map(|value| {
        project_json(
            value,
            ACP_TOOL_RAW_OUTPUT_MAX_BYTES.min(max_bytes.saturating_div(3).max(1_024)),
        )
    });

    let content_budget = ACP_TOOL_CONTENT_MAX_BYTES.min(max_bytes.saturating_div(2).max(1_024));
    let mut remaining = content_budget;
    let mut content = Vec::new();
    for (index, item) in result.content.iter().enumerate() {
        if remaining < 256 {
            content.push(RmcpContent::text(format!(
                "[{} additional tool content block(s) omitted from ACP presentation]",
                result.content.len() - index
            )));
            break;
        }
        let remaining_items = result.content.len() - index;
        let item_budget = (remaining / remaining_items.max(1)).max(256);
        let item = project_rmcp_content(item, item_budget);
        let item_bytes = serialized_len(&item);
        if item_bytes > remaining {
            content.push(omitted_content_notice(item_bytes));
            break;
        }
        remaining = remaining.saturating_sub(item_bytes);
        content.push(item);
    }
    projected.content = content;

    if serialized_len(&projected) <= max_bytes {
        return projected;
    }

    let mut fallback = CallToolResult::success(vec![RmcpContent::text(format!(
        "[Tool result omitted from ACP presentation: {} bytes total]",
        serialized_len(result)
    ))]);
    fallback.is_error = result.is_error;
    fallback.meta = projected.meta;
    fallback
}

pub(super) fn project_tool_result_for_update(
    result: &ToolResult<CallToolResult>,
) -> ToolResult<CallToolResult> {
    result
        .as_ref()
        .map(|result| project_call_tool_result(result, ACP_TOOL_RESULT_MAX_BYTES))
        .map_err(Clone::clone)
}

pub(super) fn project_tool_input(value: &serde_json::Value) -> serde_json::Value {
    project_json(value, ACP_TOOL_INPUT_MAX_BYTES)
}

pub(super) fn project_notification_value(value: &serde_json::Value) -> serde_json::Value {
    project_json(value, ACP_NOTIFICATION_VALUE_MAX_BYTES)
}

pub(super) fn project_tool_title(value: &str) -> String {
    bounded_text(value, ACP_TOOL_TITLE_MAX_BYTES, "Tool title")
}

pub(super) fn project_tool_chain_summary(value: &str) -> String {
    bounded_text(
        value,
        ACP_TOOL_CHAIN_SUMMARY_MAX_BYTES,
        "Tool chain summary",
    )
}

pub(super) fn project_identifier(value: &str) -> String {
    if value.len() <= ACP_IDENTIFIER_MAX_BYTES {
        return value.to_string();
    }
    let prefix_bytes = ACP_IDENTIFIER_MAX_BYTES / 2;
    let suffix_bytes = ACP_IDENTIFIER_MAX_BYTES.saturating_sub(prefix_bytes + 3);
    let prefix = prefix_at_byte_boundary(value, prefix_bytes);
    let mut suffix_start = value.len().saturating_sub(suffix_bytes);
    while suffix_start < value.len() && !value.is_char_boundary(suffix_start) {
        suffix_start += 1;
    }
    format!(
        "{}...{}",
        prefix,
        value.get(suffix_start..).unwrap_or_default()
    )
}

pub(super) fn project_location(value: &str) -> String {
    bounded_text(value, ACP_LOCATION_MAX_BYTES, "Location")
}

pub(super) fn project_live_text(value: &str, label: &str) -> String {
    bounded_text(value, ACP_LIVE_TEXT_MAX_BYTES, label)
}

pub(super) fn live_image_fits(data: &str) -> bool {
    data.len() <= ACP_LIVE_IMAGE_MAX_BYTES
}

fn project_history_content(content: &MessageContent, max_bytes: usize) -> MessageContent {
    if serialized_len(content) <= max_bytes {
        return content.clone();
    }

    let mut projected = content.clone();
    match &mut projected {
        MessageContent::Text(text) => {
            text.text = bounded_text(&text.text, max_bytes / 8, "Message text");
        }
        MessageContent::Thinking(thinking) => {
            thinking.thinking = bounded_text(&thinking.thinking, max_bytes / 8, "Thinking content");
            thinking.signature.clear();
        }
        MessageContent::RedactedThinking(thinking) => {
            thinking.data =
                bounded_text(&thinking.data, max_bytes / 8, "Redacted thinking content");
        }
        MessageContent::Image(image) => {
            return MessageContent::text(format!(
                "[Image omitted from paged history: {} bytes total]",
                image.data.len()
            ));
        }
        MessageContent::ToolRequest(request) => {
            request.id = project_identifier(&request.id);
            request.metadata = None;
            request.tool_meta = request
                .tool_meta
                .as_ref()
                .map(|value| project_json(value, max_bytes / 4));
            if let Ok(tool_call) = &mut request.tool_call {
                tool_call.meta = None;
                tool_call.task = None;
                tool_call.arguments = tool_call.arguments.as_ref().and_then(|arguments| {
                    project_json(&serde_json::Value::Object(arguments.clone()), max_bytes / 2)
                        .as_object()
                        .cloned()
                });
            }
        }
        MessageContent::ToolResponse(response) => {
            response.id = project_identifier(&response.id);
            response.metadata = None;
            if let Ok(result) = &response.tool_result {
                response.tool_result = Ok(project_call_tool_result(result, max_bytes));
            }
        }
        MessageContent::SystemNotification(notification) => {
            notification.msg =
                bounded_text(&notification.msg, max_bytes / 8, "System notification");
            notification.data = notification
                .data
                .as_ref()
                .map(|value| project_json(value, max_bytes / 4));
        }
        MessageContent::ToolConfirmationRequest(_)
        | MessageContent::ActionRequired(_)
        | MessageContent::FrontendToolRequest(_) => {
            return MessageContent::text(format!(
                "[Interactive message omitted from paged history: {} bytes total]",
                serialized_len(content)
            ));
        }
    }

    if serialized_len(&projected) <= max_bytes {
        projected
    } else {
        MessageContent::text(format!(
            "[Message content omitted from paged history: {} bytes total]",
            serialized_len(content)
        ))
    }
}

pub(super) fn project_history_message(message: &Message) -> Message {
    if serialized_len(message) <= ACP_HISTORY_MESSAGE_MAX_BYTES {
        return message.clone();
    }

    let mut projected = message.clone();
    projected.id = projected.id.as_deref().map(project_identifier);
    projected.metadata.terminal_error = projected
        .metadata
        .terminal_error
        .as_deref()
        .map(|value| project_live_text(value, "Terminal error"));
    if let Some(inference) = &mut projected.metadata.inference {
        inference.provider = project_identifier(&inference.provider);
        inference.requested_model = project_identifier(&inference.requested_model);
        inference.resolved_model = inference.resolved_model.as_deref().map(project_identifier);
    }
    let visible_count = message.content.len().min(MAX_HISTORY_CONTENT_ITEMS);
    let per_item_budget = ACP_HISTORY_MESSAGE_MAX_BYTES
        .saturating_sub(4_096)
        .checked_div(visible_count.max(1))
        .unwrap_or(1_024)
        .max(1_024);
    projected.content = message
        .content
        .iter()
        .take(MAX_HISTORY_CONTENT_ITEMS)
        .map(|content| project_history_content(content, per_item_budget))
        .collect();
    if message.content.len() > MAX_HISTORY_CONTENT_ITEMS {
        projected.content.push(MessageContent::text(format!(
            "[{} additional content item(s) omitted from paged history]",
            message.content.len() - MAX_HISTORY_CONTENT_ITEMS
        )));
    }

    if serialized_len(&projected) <= ACP_HISTORY_MESSAGE_MAX_BYTES {
        projected
    } else {
        Message {
            content: vec![MessageContent::text(format!(
                "[Message omitted from paged history: {} bytes total]",
                serialized_len(message)
            ))],
            ..projected
        }
    }
}

pub(super) fn project_history_page(messages: Vec<Message>) -> Vec<Message> {
    messages
        .into_iter()
        .take(ACP_HISTORY_PAGE_LIMIT)
        .map(|message| project_history_message(&message))
        .collect()
}

pub(super) fn ensure_response_fits<T: Serialize>(
    response: T,
    label: &str,
) -> Result<T, agent_client_protocol::Error> {
    let response_bytes = serialized_len(&response);
    if response_bytes <= ACP_CUSTOM_RESPONSE_MAX_BYTES {
        return Ok(response);
    }
    Err(agent_client_protocol::Error::internal_error().data(format!(
        "{label} response exceeded the ACP safety limit: {response_bytes} bytes"
    )))
}

pub(super) fn ensure_custom_result_fits(
    result: Result<serde_json::Value, agent_client_protocol::Error>,
    label: &str,
) -> Result<serde_json::Value, agent_client_protocol::Error> {
    match result {
        Ok(response) => ensure_response_fits(response, label),
        Err(error) => {
            let error_bytes = serialized_len(&error);
            if error_bytes <= ACP_CUSTOM_RESPONSE_MAX_BYTES {
                return Err(error);
            }
            Err(agent_client_protocol::Error::internal_error().data(format!(
                "{label} error exceeded the ACP safety limit: {error_bytes} bytes"
            )))
        }
    }
}

#[cfg(test)]
pub(super) fn history_message_max_bytes() -> usize {
    ACP_HISTORY_MESSAGE_MAX_BYTES
}

#[cfg(test)]
mod tests {
    use super::*;

    fn oversized_tool_result() -> CallToolResult {
        let mut result = CallToolResult::success(vec![RmcpContent::text("visible result")]);
        result.structured_content = Some(serde_json::json!({
            "stdout": "x".repeat(12_000_000),
            "stderr": "",
            "exitCode": 0,
        }));
        result
    }

    #[test]
    fn projects_oversized_live_tool_output_below_the_transport_budget() {
        let original = oversized_tool_result();

        let projected = project_tool_result_for_update(&Ok(original.clone())).unwrap();

        assert!(serialized_len(&projected) <= ACP_TOOL_RESULT_MAX_BYTES);
        assert_eq!(
            projected
                .structured_content
                .as_ref()
                .and_then(|value| value.get("_gosling"))
                .and_then(|value| value.get("truncated"))
                .and_then(serde_json::Value::as_bool),
            Some(true)
        );
        assert_eq!(
            original
                .structured_content
                .as_ref()
                .and_then(|value| value.get("stdout"))
                .and_then(serde_json::Value::as_str)
                .map(str::len),
            Some(12_000_000)
        );
    }

    #[test]
    fn projects_legacy_history_rows_without_mutating_durable_content() {
        let message = Message::user().with_tool_response("tool-1", Ok(oversized_tool_result()));

        let projected = project_history_message(&message);

        assert!(serialized_len(&projected) <= history_message_max_bytes());
        assert!(serialized_len(&message) > 12_000_000);
        let serialized = serde_json::to_string(&projected).unwrap();
        assert!(serialized.contains("truncated"));
    }

    #[test]
    fn bounded_text_preserves_utf8_boundaries_and_budget() {
        let projected = bounded_text(&"🐥".repeat(10_000), 1_024, "Unicode output");

        assert!(projected.len() <= 1_024);
        assert!(projected.contains("truncated for ACP presentation"));
    }

    #[test]
    fn a_maximum_history_page_remains_below_the_desktop_frame_limit() {
        let mut result = CallToolResult::success(vec![RmcpContent::text("visible result")]);
        result.structured_content = Some(serde_json::json!({
            "stdout": "x".repeat(150_000),
        }));
        let message = Message::user().with_tool_response("tool-1", Ok(result));
        let page = project_history_page(vec![message; ACP_HISTORY_PAGE_LIMIT]);

        assert_eq!(page.len(), ACP_HISTORY_PAGE_LIMIT);
        assert!(serialized_len(&page) < 8_000_000);
    }

    #[test]
    fn rejects_custom_responses_before_the_transport_limit() {
        let response = serde_json::json!({
            "models": "x".repeat(ACP_CUSTOM_RESPONSE_MAX_BYTES),
        });

        let error = ensure_response_fits(response, "Provider inventory").unwrap_err();

        assert!(serde_json::to_string(&error)
            .unwrap()
            .contains("ACP safety limit"));
    }

    #[test]
    fn replaces_oversized_custom_errors_before_the_transport_limit() {
        let error = agent_client_protocol::Error::internal_error()
            .data("x".repeat(ACP_CUSTOM_RESPONSE_MAX_BYTES));

        let projected = ensure_custom_result_fits(Err(error), "Tool call").unwrap_err();
        let serialized = serde_json::to_string(&projected).unwrap();

        assert!(serialized.len() < 1_000);
        assert!(serialized.contains("Tool call error exceeded the ACP safety limit"));
    }
}
