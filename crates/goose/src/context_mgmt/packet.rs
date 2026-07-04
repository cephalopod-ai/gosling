use std::collections::HashMap;

use serde::Serialize;

use crate::conversation::message::{Message, MessageMetadata};
use crate::conversation::{fix_conversation, Conversation};
use crate::token_counter::TokenCounter;

use super::block::{ContextBlock, ContextPriority, ContextSlot};
use super::budget::ContextBudgetPolicy;
use super::policy::ContextManagerMode;
use super::selector;

/// Input to [`ContextManager::build`] — everything needed to assemble a
/// [`ContextPacket`] for one provider call.
#[derive(Debug, Clone)]
pub struct ContextBuildRequest {
    pub system_prompt: String,
    pub project_instructions: Option<String>,
    pub conversation_messages: Vec<Message>,
    pub context_limit: usize,
    pub reserved_response_tokens: usize,
}

/// Strategy the Context Manager used to fit the conversation into budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextStrategy {
    /// Everything fit within budget once clearly low-value blocks (duplicates,
    /// stale failed attempts) were dropped; nothing needed summarizing.
    FullContext,
    /// Older conversation and/or long tool output had to be summarized to fit
    /// the budget.
    RecentPlusSummary,
}

impl ContextStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            ContextStrategy::FullContext => "full_context",
            ContextStrategy::RecentPlusSummary => "recent_plus_summary",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextSlotUsage {
    pub slot: ContextSlot,
    pub estimated_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextDropRecord {
    pub slot: ContextSlot,
    pub reason: String,
    pub count: usize,
    pub estimated_tokens_saved: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextSummaryRecord {
    pub slot: ContextSlot,
    pub original_message_count: usize,
    pub original_tokens: usize,
    pub summary_tokens: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContextPacketMetadata {
    pub estimated_tokens_before: usize,
    pub estimated_tokens_after: usize,
    pub context_limit: usize,
    pub reserved_response_tokens: usize,
    pub strategy: ContextStrategy,
    pub slots: Vec<ContextSlotUsage>,
    pub dropped_blocks: Vec<ContextDropRecord>,
    pub summarized_blocks: Vec<ContextSummaryRecord>,
    /// Always true in this MVP: the retrieved-memory slot is reserved but
    /// never populated. Flipping this requires wiring an actual memory
    /// retrieval step into `ContextManager::build`.
    pub retrieved_memory_empty: bool,
}

/// The assembled, inspectable input to a provider call.
#[derive(Debug, Clone)]
pub struct ContextPacket {
    pub system_prompt: String,
    pub messages: Vec<Message>,
    pub metadata: ContextPacketMetadata,
}

const SUMMARY_PREVIEW_CHARS: usize = 600;

fn summarize_group(
    slot: ContextSlot,
    blocks: &[ContextBlock],
    token_counter: &TokenCounter,
) -> (ContextBlock, ContextSummaryRecord) {
    let original_message_count: usize = blocks.iter().map(|b| b.messages.len()).sum();
    let original_tokens: usize = blocks.iter().map(|b| b.estimated_tokens).sum();

    let rendered: Vec<String> = blocks
        .iter()
        .flat_map(|b| b.messages.iter())
        .map(super::format_message_for_compacting)
        .collect();
    let joined = rendered.join("\n");
    let mut chars = joined.chars();
    let preview: String = chars.by_ref().take(SUMMARY_PREVIEW_CHARS).collect();
    let truncated = chars.next().is_some();

    let summary_text = format!(
        "[Context Manager summary of {} earlier message(s)]: {}{}",
        original_message_count,
        preview,
        if truncated { " ... [truncated]" } else { "" }
    );

    let summary_tokens = token_counter.count_tokens(&summary_text);
    let message = Message::user()
        .with_text(summary_text)
        .with_metadata(MessageMetadata::agent_only());

    let block = ContextBlock {
        slot,
        priority: ContextPriority::Medium,
        messages: vec![message],
        estimated_tokens: summary_tokens,
        label: format!("{}#summary", slot.as_str()),
        reason: Some("summarized".to_string()),
    };
    let record = ContextSummaryRecord {
        slot,
        original_message_count,
        original_tokens,
        summary_tokens,
    };

    (block, record)
}

/// Collapses a contiguous run of same-slot `Medium` blocks into a single
/// summary block appended to `result`, then clears the run. No-op if `run`
/// is empty.
fn flush_medium_run(
    run: &mut Vec<ContextBlock>,
    result: &mut Vec<ContextBlock>,
    summarized_blocks: &mut Vec<ContextSummaryRecord>,
    token_counter: &TokenCounter,
) {
    let Some(first) = run.first() else {
        return;
    };
    let (summary, record) = summarize_group(first.slot, run, token_counter);
    summarized_blocks.push(record);
    result.push(summary);
    run.clear();
}

/// Builds a [`ContextPacket`] from raw request inputs, applying the default
/// MVP budget policy: drop clearly low-value blocks unconditionally, then
/// summarize older conversation / long tool output only if the remaining
/// content still exceeds the available budget.
pub struct ContextManager;

impl ContextManager {
    /// Async convenience wrapper that resolves the shared token counter.
    pub async fn build(request: ContextBuildRequest) -> Result<ContextPacket, String> {
        let token_counter = crate::token_counter::shared_token_counter().await?;
        Ok(Self::build_with_counter(request, &token_counter))
    }

    pub fn build_with_counter(
        request: ContextBuildRequest,
        token_counter: &TokenCounter,
    ) -> ContextPacket {
        let ContextBuildRequest {
            system_prompt,
            project_instructions,
            conversation_messages,
            context_limit,
            reserved_response_tokens,
        } = request;

        let system_tokens = token_counter.count_tokens(&system_prompt);
        let project_tokens = project_instructions
            .as_deref()
            .map(|s| token_counter.count_tokens(s))
            .unwrap_or(0);
        let estimated_tokens_before = system_tokens
            + project_tokens
            + token_counter.count_chat_tokens("", &conversation_messages, &[]);

        let policy = ContextBudgetPolicy::new(context_limit, reserved_response_tokens);
        let mut blocks = selector::classify_blocks(&conversation_messages, token_counter);

        // Unconditionally drop clearly low-value blocks (duplicates, stale
        // failed attempts) — these never add value regardless of budget.
        let mut dropped_totals: HashMap<(ContextSlot, String), (usize, usize)> = HashMap::new();
        blocks.retain(|b| {
            if b.priority == ContextPriority::Low {
                let reason = b.reason.clone().unwrap_or_else(|| "low_value".to_string());
                let entry = dropped_totals.entry((b.slot, reason)).or_insert((0, 0));
                entry.0 += 1;
                entry.1 += b.estimated_tokens;
                false
            } else {
                true
            }
        });
        let dropped_blocks: Vec<ContextDropRecord> = dropped_totals
            .into_iter()
            .map(|((slot, reason), (count, tokens))| ContextDropRecord {
                slot,
                reason,
                count,
                estimated_tokens_saved: tokens,
            })
            .collect();

        let required_tokens: usize = blocks
            .iter()
            .filter(|b| b.priority == ContextPriority::Required)
            .map(|b| b.estimated_tokens)
            .sum();
        let fixed_tokens = system_tokens + project_tokens + required_tokens;
        let available = policy.available_for_conversation(fixed_tokens);

        let candidate_tokens: usize = blocks
            .iter()
            .filter(|b| b.priority != ContextPriority::Required)
            .map(|b| b.estimated_tokens)
            .sum();

        let mut summarized_blocks = Vec::new();
        let mut strategy = ContextStrategy::FullContext;

        let final_blocks: Vec<ContextBlock> = if candidate_tokens <= available {
            blocks
        } else {
            strategy = ContextStrategy::RecentPlusSummary;

            // Summarize in place rather than grouping all Medium blocks
            // together: a long tool output can be Medium priority while still
            // sitting inside the recent window, interleaved with High/Required
            // blocks. Collapsing every Medium block to the front would hoist
            // that summary ahead of its own tool request, breaking the
            // request/response order the provider expects. Instead, only
            // contiguous runs of same-slot Medium blocks are collapsed,
            // in place, preserving chronological order everywhere else.
            let mut result = Vec::with_capacity(blocks.len());
            let mut run: Vec<ContextBlock> = Vec::new();

            for block in blocks {
                if block.priority == ContextPriority::Medium {
                    if run.last().is_some_and(|b| b.slot != block.slot) {
                        flush_medium_run(
                            &mut run,
                            &mut result,
                            &mut summarized_blocks,
                            token_counter,
                        );
                    }
                    run.push(block);
                } else {
                    flush_medium_run(&mut run, &mut result, &mut summarized_blocks, token_counter);
                    result.push(block);
                }
            }
            flush_medium_run(&mut run, &mut result, &mut summarized_blocks, token_counter);

            result
        };

        let mut slot_totals: HashMap<ContextSlot, usize> = HashMap::new();
        for block in &final_blocks {
            *slot_totals.entry(block.slot).or_insert(0) += block.estimated_tokens;
        }

        let slots = vec![
            ContextSlotUsage {
                slot: ContextSlot::System,
                estimated_tokens: system_tokens,
            },
            ContextSlotUsage {
                slot: ContextSlot::ProjectInstructions,
                estimated_tokens: project_tokens,
            },
            ContextSlotUsage {
                slot: ContextSlot::RecentConversation,
                estimated_tokens: slot_totals
                    .get(&ContextSlot::RecentConversation)
                    .copied()
                    .unwrap_or(0),
            },
            ContextSlotUsage {
                slot: ContextSlot::OlderConversationSummary,
                estimated_tokens: slot_totals
                    .get(&ContextSlot::OlderConversationSummary)
                    .copied()
                    .unwrap_or(0),
            },
            ContextSlotUsage {
                slot: ContextSlot::RecentToolResults,
                estimated_tokens: slot_totals
                    .get(&ContextSlot::RecentToolResults)
                    .copied()
                    .unwrap_or(0),
            },
            ContextSlotUsage {
                slot: ContextSlot::SummarizedToolResults,
                estimated_tokens: slot_totals
                    .get(&ContextSlot::SummarizedToolResults)
                    .copied()
                    .unwrap_or(0),
            },
            ContextSlotUsage {
                slot: ContextSlot::RetrievedMemory,
                estimated_tokens: 0,
            },
        ];

        let estimated_tokens_after: usize = slots.iter().map(|s| s.estimated_tokens).sum();

        let final_system_prompt = match &project_instructions {
            Some(addendum) if !addendum.is_empty() => format!("{system_prompt}\n\n{addendum}"),
            _ => system_prompt,
        };

        let assembled: Vec<Message> = final_blocks.into_iter().flat_map(|b| b.messages).collect();

        // Dropping a duplicate/stale-failed tool response (Low priority) can
        // leave its assistant tool-request orphaned, which providers like
        // Anthropic/OpenAI reject. Reuse the same repair pass the rest of the
        // pipeline already runs before a provider call rather than
        // re-implementing tool-call pairing here.
        let (fixed, fix_issues) = fix_conversation(Conversation::new_unvalidated(assembled));
        if !fix_issues.is_empty() {
            tracing::debug!(
                target: "goose::context_mgmt",
                issues = ?fix_issues,
                "context manager repaired tool call/response pairing after dropping low-value blocks"
            );
        }
        let messages = fixed.messages().clone();

        ContextPacket {
            system_prompt: final_system_prompt,
            messages,
            metadata: ContextPacketMetadata {
                estimated_tokens_before,
                estimated_tokens_after,
                context_limit,
                reserved_response_tokens,
                strategy,
                slots,
                dropped_blocks,
                summarized_blocks,
                retrieved_memory_empty: true,
            },
        }
    }
}

/// Decides what to actually hand to the provider based on the Context
/// Manager's mode: `On` uses the packet's assembled input; `Off`/`Shadow`
/// leave the existing (pre-Context-Manager) input untouched.
pub fn resolve_provider_input(
    mode: ContextManagerMode,
    packet: &ContextPacket,
    fallback_system_prompt: &str,
    fallback_messages: &[Message],
) -> (String, Vec<Message>) {
    match mode {
        ContextManagerMode::On => (packet.system_prompt.clone(), packet.messages.clone()),
        ContextManagerMode::Off | ContextManagerMode::Shadow => (
            fallback_system_prompt.to_string(),
            fallback_messages.to_vec(),
        ),
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

    fn build(
        token_counter: &TokenCounter,
        system_prompt: &str,
        project_instructions: Option<&str>,
        messages: Vec<Message>,
        context_limit: usize,
        reserved_response_tokens: usize,
    ) -> ContextPacket {
        ContextManager::build_with_counter(
            ContextBuildRequest {
                system_prompt: system_prompt.to_string(),
                project_instructions: project_instructions.map(|s| s.to_string()),
                conversation_messages: messages,
                context_limit,
                reserved_response_tokens,
            },
            token_counter,
        )
    }

    #[tokio::test]
    async fn required_context_is_never_dropped() {
        let tc = counter().await;
        let mut messages = Vec::new();
        for i in 0..50 {
            messages.push(Message::user().with_text(format!("turn {i}")));
            messages.push(Message::assistant().with_text(format!("reply {i}")));
        }
        messages.push(Message::user().with_text("final question"));

        // Tiny budget forces summarization.
        let packet = build(&tc, "system prompt", None, messages, 2_000, 200);

        let last = packet.messages.last().expect("packet should have messages");
        assert!(last
            .content
            .iter()
            .any(|c| c.as_text() == Some("final question")));
    }

    #[tokio::test]
    async fn latest_user_message_is_preserved() {
        let tc = counter().await;
        let messages = vec![
            Message::user().with_text("hello"),
            Message::assistant().with_text("hi"),
            Message::user().with_text("what time is it?"),
        ];
        let packet = build(&tc, "system", None, messages, 128_000, 4_000);

        assert!(packet.messages.iter().any(|m| m
            .content
            .iter()
            .any(|c| c.as_text() == Some("what time is it?"))));
    }

    #[tokio::test]
    async fn system_and_project_instructions_are_preserved() {
        let tc = counter().await;
        let messages = vec![Message::user().with_text("hi")];
        let packet = build(
            &tc,
            "You are a helpful agent.",
            Some("# Project: widgets\nBuild widgets."),
            messages,
            128_000,
            4_000,
        );

        assert!(packet.system_prompt.contains("You are a helpful agent."));
        assert!(packet.system_prompt.contains("Build widgets."));
        let system_usage = packet
            .metadata
            .slots
            .iter()
            .find(|s| s.slot == ContextSlot::System)
            .unwrap();
        assert!(system_usage.estimated_tokens > 0);
        let project_usage = packet
            .metadata
            .slots
            .iter()
            .find(|s| s.slot == ContextSlot::ProjectInstructions)
            .unwrap();
        assert!(project_usage.estimated_tokens > 0);
    }

    #[tokio::test]
    async fn long_older_conversation_is_summarized_when_over_budget() {
        let tc = counter().await;
        let mut messages = Vec::new();
        for i in 0..50 {
            messages.push(Message::user().with_text(format!(
                "this is a fairly long older turn number {i} with some padding text to add tokens"
            )));
            messages.push(Message::assistant().with_text(format!(
                "this is a fairly long older reply number {i} with some padding text to add tokens"
            )));
        }
        messages.push(Message::user().with_text("final question"));

        let packet = build(&tc, "system", None, messages, 2_000, 200);

        assert_eq!(packet.metadata.strategy, ContextStrategy::RecentPlusSummary);
        assert!(!packet.metadata.summarized_blocks.is_empty());
        assert!(packet
            .metadata
            .summarized_blocks
            .iter()
            .any(|r| r.slot == ContextSlot::OlderConversationSummary));
        assert!(packet.metadata.estimated_tokens_after < packet.metadata.estimated_tokens_before);
    }

    #[tokio::test]
    async fn duplicate_tool_output_is_dropped() {
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
        let packet = build(&tc, "system", None, messages, 128_000, 4_000);

        assert!(packet
            .metadata
            .dropped_blocks
            .iter()
            .any(|d| d.reason == "duplicate_tool_output"));

        // Dropping the earlier duplicate ToolResponse must not orphan its
        // ToolRequest — the packet has to be valid provider input as-is.
        Conversation::new(packet.messages.clone())
            .expect("packet messages must not contain an orphaned tool call after dropping a duplicate response");
    }

    #[tokio::test]
    async fn long_tool_output_inside_recent_window_keeps_valid_ordering() {
        let tc = counter().await;
        let mut messages = Vec::new();
        // Enough older filler to push the conversation over budget.
        for i in 0..40 {
            messages.push(Message::user().with_text(format!(
                "older turn {i} with some padding text to add a few tokens"
            )));
            messages.push(Message::assistant().with_text(format!(
                "older reply {i} with some padding text to add a few tokens"
            )));
        }
        // A tool call/response pair inside the recent window, with a response
        // long enough to be classified Medium ("long_tool_output") even
        // though it sits among High/Required blocks.
        messages.push(
            Message::assistant()
                .with_tool_request("call1", Ok(CallToolRequestParams::new("search"))),
        );
        messages.push(Message::user().with_tool_response(
            "call1",
            Ok(rmcp::model::CallToolResult::success(vec![
                rmcp::model::RawContent::text("x".repeat(10_000)).no_annotation(),
            ])),
        ));
        messages.push(Message::assistant().with_text("here's what I found"));
        messages.push(Message::user().with_text("final question"));

        let packet = build(&tc, "system", None, messages, 2_000, 200);

        assert_eq!(packet.metadata.strategy, ContextStrategy::RecentPlusSummary);
        assert!(packet
            .metadata
            .summarized_blocks
            .iter()
            .any(|r| r.slot == ContextSlot::SummarizedToolResults));

        // Every ToolResponse must be preceded by its matching ToolRequest —
        // summarizing the long response in place must not hoist it ahead of
        // the request that produced it.
        Conversation::new(packet.messages.clone()).expect(
            "packet messages must keep tool request/response pairs in order after in-place summarization",
        );
    }

    #[tokio::test]
    async fn retrieved_memory_slot_exists_and_is_empty() {
        let tc = counter().await;
        let messages = vec![Message::user().with_text("hi")];
        let packet = build(&tc, "system", None, messages, 128_000, 4_000);

        assert!(packet.metadata.retrieved_memory_empty);
        let memory_usage = packet
            .metadata
            .slots
            .iter()
            .find(|s| s.slot == ContextSlot::RetrievedMemory)
            .expect("retrieved memory slot should be present");
        assert_eq!(memory_usage.estimated_tokens, 0);
        assert!(!packet.messages.iter().any(|m| m.content.iter().any(|c| c
            .as_text()
            .map(|t| t.contains("retrieved memory"))
            .unwrap_or(false))));
    }

    #[tokio::test]
    async fn shadow_mode_does_not_change_provider_input() {
        let tc = counter().await;
        let messages = vec![Message::user().with_text("hi")];
        let packet = build(&tc, "system prompt", None, messages.clone(), 2_000, 200);

        let (system_prompt, resolved_messages) = resolve_provider_input(
            ContextManagerMode::Shadow,
            &packet,
            "original system prompt",
            &messages,
        );

        assert_eq!(system_prompt, "original system prompt");
        assert_eq!(resolved_messages.len(), messages.len());
    }

    #[tokio::test]
    async fn on_mode_uses_the_new_packet() {
        let tc = counter().await;
        let messages = vec![Message::user().with_text("hi")];
        let packet = build(&tc, "system prompt", None, messages.clone(), 2_000, 200);

        let (system_prompt, resolved_messages) = resolve_provider_input(
            ContextManagerMode::On,
            &packet,
            "original system prompt",
            &messages,
        );

        assert_eq!(system_prompt, packet.system_prompt);
        assert_eq!(resolved_messages.len(), packet.messages.len());
        assert_ne!(system_prompt, "original system prompt");
    }
}
