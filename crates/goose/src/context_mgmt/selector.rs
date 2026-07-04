use std::collections::HashMap;

use rmcp::model::Role;

use crate::conversation::message::{Message, MessageContent};
use crate::token_counter::TokenCounter;

use super::block::{ContextBlock, ContextPriority, ContextSlot};
use super::policy::{LONG_TOOL_OUTPUT_TOKEN_THRESHOLD, RECENT_MESSAGE_WINDOW};

fn message_estimated_tokens(msg: &Message, token_counter: &TokenCounter) -> usize {
    token_counter.count_chat_tokens("", std::slice::from_ref(msg), &[])
}

fn has_tool_content(msg: &Message) -> bool {
    msg.content.iter().any(|c| {
        matches!(
            c,
            MessageContent::ToolRequest(_) | MessageContent::ToolResponse(_)
        )
    })
}

/// A signature identifying a tool response's content, used to detect
/// duplicate tool output (the same result repeated verbatim later).
fn tool_response_signature(msg: &Message) -> Option<String> {
    msg.content.iter().find_map(|c| {
        let resp = c.as_tool_response()?;
        let result = resp.tool_result.as_ref().ok()?;
        let text = result
            .content
            .iter()
            .filter_map(|item| item.as_text().map(|t| t.text.clone()))
            .collect::<Vec<_>>()
            .join("\n");
        if text.is_empty() {
            None
        } else {
            Some(text)
        }
    })
}

fn tool_response_failed(msg: &Message) -> bool {
    msg.content.iter().any(|c| {
        c.as_tool_response()
            .map(|resp| resp.tool_result.is_err())
            .unwrap_or(false)
    })
}

/// Classify agent-visible messages into [`ContextBlock`]s.
///
/// Messages from the most recent user message onward are `Required` (current
/// user message, most recent assistant response, unresolved tool state). The
/// window just before that is `High` priority (recent conversation / recent
/// tool results). Anything older is `Medium` priority (a summarization
/// candidate) unless it's a duplicate tool response or a failed tool attempt,
/// in which case it's `Low` priority (droppable).
///
/// The returned blocks are in chronological order.
pub fn classify_blocks(messages: &[Message], token_counter: &TokenCounter) -> Vec<ContextBlock> {
    let indexed: Vec<&Message> = messages.iter().filter(|m| m.is_agent_visible()).collect();
    if indexed.is_empty() {
        return Vec::new();
    }

    let last_pos = indexed.len() - 1;
    let last_user_pos = indexed.iter().rposition(|m| m.role == Role::User);
    let required_start = last_user_pos.unwrap_or(last_pos);

    // Only the most recent occurrence of an identical tool response is kept;
    // earlier ones are duplicates.
    let mut latest_signature_pos: HashMap<String, usize> = HashMap::new();
    for (pos, msg) in indexed.iter().enumerate() {
        if let Some(sig) = tool_response_signature(msg) {
            latest_signature_pos.insert(sig, pos);
        }
    }

    let mut blocks = Vec::with_capacity(indexed.len());
    let mut recent_window_remaining = RECENT_MESSAGE_WINDOW;

    for pos in (0..=last_pos).rev() {
        let msg = indexed[pos];
        let estimated_tokens = message_estimated_tokens(msg, token_counter);
        let is_tool_content = has_tool_content(msg);

        let (priority, slot, reason) = if pos >= required_start {
            let slot = if is_tool_content {
                ContextSlot::RecentToolResults
            } else {
                ContextSlot::RecentConversation
            };
            (ContextPriority::Required, slot, None)
        } else if let Some(sig) = tool_response_signature(msg) {
            if latest_signature_pos.get(&sig).copied() != Some(pos) {
                (
                    ContextPriority::Low,
                    ContextSlot::RecentToolResults,
                    Some("duplicate_tool_output".to_string()),
                )
            } else {
                classify_non_duplicate(
                    msg,
                    is_tool_content,
                    estimated_tokens,
                    &mut recent_window_remaining,
                )
            }
        } else {
            classify_non_duplicate(
                msg,
                is_tool_content,
                estimated_tokens,
                &mut recent_window_remaining,
            )
        };

        blocks.push(ContextBlock {
            slot,
            priority,
            messages: vec![msg.clone()],
            estimated_tokens,
            label: format!("{}#{}", slot.as_str(), pos),
            reason,
        });
    }

    blocks.reverse();
    blocks
}

fn classify_non_duplicate(
    msg: &Message,
    is_tool_content: bool,
    estimated_tokens: usize,
    recent_window_remaining: &mut usize,
) -> (ContextPriority, ContextSlot, Option<String>) {
    if tool_response_failed(msg) {
        return (
            ContextPriority::Low,
            ContextSlot::RecentToolResults,
            Some("stale_failed_attempt".to_string()),
        );
    }

    if *recent_window_remaining > 0 {
        *recent_window_remaining -= 1;
        if is_tool_content && estimated_tokens > LONG_TOOL_OUTPUT_TOKEN_THRESHOLD {
            return (
                ContextPriority::Medium,
                ContextSlot::SummarizedToolResults,
                Some("long_tool_output".to_string()),
            );
        }
        let slot = if is_tool_content {
            ContextSlot::RecentToolResults
        } else {
            ContextSlot::RecentConversation
        };
        return (ContextPriority::High, slot, None);
    }

    if is_tool_content {
        (
            ContextPriority::Medium,
            ContextSlot::SummarizedToolResults,
            Some("older_conversation".to_string()),
        )
    } else {
        (
            ContextPriority::Medium,
            ContextSlot::OlderConversationSummary,
            Some("older_conversation".to_string()),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token_counter::create_token_counter;
    use rmcp::model::{AnnotateAble, CallToolRequestParams};

    async fn counter() -> TokenCounter {
        create_token_counter().await.unwrap()
    }

    #[tokio::test]
    async fn latest_user_message_and_tail_are_required() {
        let tc = counter().await;
        let messages = vec![
            Message::user().with_text("hello"),
            Message::assistant().with_text("hi there"),
            Message::user().with_text("what's next?"),
        ];
        let blocks = classify_blocks(&messages, &tc);

        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[2].priority, ContextPriority::Required);
        assert!(blocks[2].messages[0]
            .content
            .iter()
            .any(|c| c.as_text() == Some("what's next?")));
    }

    #[tokio::test]
    async fn unresolved_tool_call_after_last_user_message_is_required() {
        let tc = counter().await;
        let messages = vec![
            Message::user().with_text("read the file"),
            Message::assistant()
                .with_tool_request("call1", Ok(CallToolRequestParams::new("read_file"))),
        ];
        let blocks = classify_blocks(&messages, &tc);

        assert!(blocks
            .iter()
            .all(|b| b.priority == ContextPriority::Required));
    }

    #[tokio::test]
    async fn duplicate_tool_output_is_low_priority() {
        let tc = counter().await;
        let messages = vec![
            Message::user().with_text("look this up twice"),
            Message::assistant()
                .with_tool_request("call1", Ok(CallToolRequestParams::new("search"))),
            Message::user().with_tool_response(
                "call1",
                Ok(rmcp::model::CallToolResult::success(vec![
                    rmcp::model::RawContent::text("same result").no_annotation(),
                ])),
            ),
            Message::assistant().with_text("let me check again"),
            Message::assistant()
                .with_tool_request("call2", Ok(CallToolRequestParams::new("search"))),
            Message::user().with_tool_response(
                "call2",
                Ok(rmcp::model::CallToolResult::success(vec![
                    rmcp::model::RawContent::text("same result").no_annotation(),
                ])),
            ),
            Message::user().with_text("thanks"),
        ];
        let blocks = classify_blocks(&messages, &tc);

        let duplicate = blocks
            .iter()
            .find(|b| b.reason.as_deref() == Some("duplicate_tool_output"));
        assert!(
            duplicate.is_some(),
            "expected the earlier identical tool response to be flagged as a duplicate"
        );
    }

    #[tokio::test]
    async fn failed_tool_attempt_is_low_priority() {
        let tc = counter().await;
        let messages = vec![
            Message::user().with_text("try the flaky tool"),
            Message::assistant()
                .with_tool_request("call1", Ok(CallToolRequestParams::new("flaky"))),
            Message::user().with_tool_response(
                "call1",
                Err(rmcp::model::ErrorData::new(
                    rmcp::model::ErrorCode::INTERNAL_ERROR,
                    "boom".to_string(),
                    None,
                )),
            ),
            Message::assistant().with_text("that failed, trying something else"),
            Message::user().with_text("ok, thanks"),
        ];
        let blocks = classify_blocks(&messages, &tc);

        let failed = blocks
            .iter()
            .find(|b| b.reason.as_deref() == Some("stale_failed_attempt"));
        assert!(failed.is_some());
    }

    #[tokio::test]
    async fn old_messages_beyond_window_are_medium_priority() {
        let tc = counter().await;
        let mut messages = Vec::new();
        for i in 0..(RECENT_MESSAGE_WINDOW + 5) {
            messages.push(Message::user().with_text(format!("turn {i}")));
            messages.push(Message::assistant().with_text(format!("reply {i}")));
        }
        messages.push(Message::user().with_text("final question"));
        let blocks = classify_blocks(&messages, &tc);

        assert!(blocks.iter().any(|b| b.priority == ContextPriority::Medium
            && b.reason.as_deref() == Some("older_conversation")));
        assert!(blocks.iter().any(|b| b.priority == ContextPriority::High));
        assert_eq!(blocks.last().unwrap().priority, ContextPriority::Required);
    }
}
