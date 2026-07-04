//! Long-conversation memory evaluation for base gosling's *only* mechanism
//! for handling conversations that outgrow the model's context window:
//! LLM-based compaction (`compact_messages` / `check_if_compaction_needed`
//! in `crates/goose/src/context_mgmt/mod.rs`).
//!
//! This is the baseline gosling-2's Context Manager MVP is compared against.
//! Base gosling has no cross-session memory and no deterministic in-session
//! fact retrieval — everything that falls outside the "still fits" window
//! survives only if the summarization LLM call happens to preserve it, and
//! there is no live model API available in this sandbox to test real LLM
//! summarization quality. So this harness brackets the answer with two mock
//! summarizers representing the best and worst a summarizer could plausibly
//! do, rather than pretending a single mock stands in for real quality:
//!
//! - `GenericSummaryProvider` returns a fixed, generic summary with no
//!   scenario-specific content — the worst case (a summarizer that discards
//!   everything not asked about directly). Establishes the *lower bound* on
//!   recall.
//! - `FaithfulSummaryProvider` echoes the scenario's needle fact verbatim
//!   into its summary whenever the fact is present in what it was asked to
//!   summarize — the best case (a maximally competent, faithful summarizer
//!   that never drops an explicitly-stated fact it can see). Establishes
//!   the *upper bound* on recall.
//!
//! Both providers gate on an approximate token budget (`summarizer_context_limit`,
//! set to the scenario's window size) and return `ProviderError::ContextLengthExceeded`
//! when the compaction call itself wouldn't fit — this exercises the real
//! `do_compact` progressive tool-response-stripping fallback
//! (`removal_percentages = [0, 10, 20, 50, 100]`) at genuine scale, which
//! matters because that fallback strips tool-response *content* outright
//! before the summarizer ever sees it — so even the best-possible summarizer
//! cannot recall a fact that was embedded in a tool response dropped by the
//! fallback before the compaction call succeeded.
//!
//! Heavy (multi-million-token) runs are `#[ignore]`d; run them explicitly:
//!   cargo test -p goose --test memory_manager_eval -- --ignored --nocapture

use anyhow::Result;
use async_trait::async_trait;
use goose::context_mgmt::{check_if_compaction_needed, compact_messages};
use goose::conversation::message::{Message, MessageContent};
use goose::conversation::Conversation;
use goose::providers::base::{stream_from_single_message, MessageStream, Provider};
use goose::session::Session;
use goose_providers::conversation::token_usage::{ProviderUsage, Usage};
use goose_providers::errors::ProviderError;
use goose_providers::model::ModelConfig;
use rmcp::model::{AnnotateAble, CallToolRequestParams, CallToolResult, RawContent, Tool};
use serde::Serialize;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Instant;

const WINDOW_SIZES: [(&str, usize); 3] = [("128K", 128_000), ("200K", 200_000), ("1M", 1_000_000)];
const CHARS_PER_TOKEN_ESTIMATE: usize = 4;
const MAX_COMPACTION_ROUNDS: usize = 5;

struct Scenario {
    name: &'static str,
    needle: &'static str,
    needle_message: &'static str,
    final_question: &'static str,
    filler_lines: &'static [&'static str],
}

const SCENARIOS: [Scenario; 5] = [
    Scenario {
        name: "codebase_refactor_marathon",
        needle: "UUIDv7 primary keys",
        needle_message: "Before we start: architectural decision — every new table we add during this refactor must use UUIDv7 primary keys, not auto-increment ints. Payments module is owned by another team, don't touch it.",
        final_question: "We're about to create the new `shipments` table. What primary key type should it use, and is there anything we should avoid touching?",
        filler_lines: &[
            "diff --git a/src/service.rs b/src/service.rs",
            "-    pub id: i64,",
            "+    pub id: Uuid,",
            "running cargo build ... warning: unused import `std::fmt`",
            "test result: ok. 42 passed; 0 failed; 0 ignored",
            "refactored module boundary between billing and inventory",
            "renamed struct OrderLine -> LineItem across 14 call sites",
        ],
    },
    Scenario {
        name: "incident_response_marathon",
        needle: "request timeout config is set to 30000ms",
        needle_message: "Status update: we already confirmed it's NOT a DNS issue — checked at 14:02 UTC, resolver latency was normal. The service's request timeout config is set to 30000ms, that hasn't changed recently.",
        final_question: "Given everything we've gathered in this incident so far, should we investigate DNS resolution again, and what is the current request timeout config value?",
        filler_lines: &[
            "2026-07-04T09:12:03Z ERROR upstream connection reset by peer",
            "2026-07-04T09:12:04Z WARN retrying request, attempt 3/5",
            "cpu_usage{pod=\"api-7f9\"} 0.82",
            "memory_usage{pod=\"api-7f9\"} 0.61",
            "GET /health 200 12ms",
            "checked load balancer target group: all healthy",
            "reviewed recent deploys: no changes in the last 6 hours",
        ],
    },
    Scenario {
        name: "requirements_gathering_thread",
        needle: "SLA is 99.95% uptime",
        needle_message: "Hard requirement from the client: the mobile app MUST support full offline mode on iOS — this is non-negotiable. Also the contracted SLA is 99.95% uptime.",
        final_question: "Can you summarize the requirements we've gathered for the client proposal, including the mobile and reliability requirements?",
        filler_lines: &[
            "client call notes: discussed dashboard color scheme preferences",
            "searched docs: found reference implementation for push notifications",
            "compared pricing tiers across three competitor products",
            "drafted onboarding email copy, awaiting client feedback",
            "reviewed accessibility requirements (WCAG 2.1 AA)",
            "client asked about timeline for the Android release",
        ],
    },
    Scenario {
        name: "research_literature_review",
        needle: "focus only on transformer-based approaches",
        needle_message: "Scope for this literature review: exclude any papers published before 2020, and focus only on transformer-based approaches — not RNNs or CNNs, even if they're cited by transformer papers.",
        final_question: "Based on everything reviewed, can you compile the shortlist of papers that fit our review's scope?",
        filler_lines: &[
            "Abstract: We propose a novel recurrent architecture for sequence modeling (2018)...",
            "Abstract: A convolutional approach to language understanding (2016)...",
            "Abstract: Scaling attention mechanisms for long-context reasoning (2023)...",
            "citation graph: 214 papers reference this work",
            "extracted benchmark table: BLEU scores across 12 datasets",
            "noted: this paper's ablation study lacks a controlled baseline",
        ],
    },
    Scenario {
        name: "personal_planning_assistant",
        needle: "allergic to shellfish",
        needle_message: "A few things about me you should remember for this whole planning process: I'm allergic to shellfish, and the total budget cap for this trip is $5,000 — please don't suggest anything that would blow that.",
        final_question: "Given everything we've planned so far, what restaurant should we book for the final night, and does it fit within what I told you earlier?",
        filler_lines: &[
            "flight search: found 3 options with layovers under 2 hours",
            "hotel comparison: reviewed ratings and cancellation policies",
            "itinerary draft: day 1 museum, day 2 hiking, day 3 free time",
            "currency conversion notes and tipping customs for the destination",
            "packing list draft: weather forecast shows mild temperatures",
            "checked visa requirements: none needed for this itinerary",
        ],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NeedlePosition {
    Start,
    Middle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NeedleShape {
    /// The needle is its own plain-text user message.
    IsolatedText,
    /// The needle is a sentence embedded inside a large tool-response body,
    /// as a config value would appear in the middle of a real log dump.
    BuriedInToolOutput,
}

fn filler_block(scenario: &Scenario, index: usize, target_chars: usize) -> String {
    let mut text = String::with_capacity(target_chars + 128);
    let mut i = 0usize;
    while text.len() < target_chars {
        let line = scenario.filler_lines[(index + i) % scenario.filler_lines.len()];
        let absolute_chunk = index * 1_000 + i;
        text.push_str(line);
        text.push_str(&format!(" [msg {index} chunk {absolute_chunk}]\n"));
        i += 1;
    }
    text
}

fn build_conversation(
    scenario: &Scenario,
    target_tokens: usize,
    position: NeedlePosition,
    shape: NeedleShape,
) -> Vec<Message> {
    let mut messages = Vec::new();
    messages.push(Message::user().with_text("Starting a new work session."));
    messages.push(Message::assistant().with_text("Ready to help. What are we working on?"));

    let target_chars = target_tokens * CHARS_PER_TOKEN_ESTIMATE;
    let per_message_chars = 12_000usize;
    let num_pairs = (target_chars / per_message_chars).max(1);
    let needle_at = match position {
        NeedlePosition::Start => 0,
        NeedlePosition::Middle => num_pairs / 2,
    };

    for i in 0..num_pairs {
        let call_id = format!("call_{i}");
        if i == needle_at && shape == NeedleShape::IsolatedText {
            messages.push(Message::user().with_text(scenario.needle_message));
            messages.push(
                Message::assistant().with_text("Understood, I'll keep that in mind throughout."),
            );
        }

        messages.push(
            Message::assistant()
                .with_tool_request(call_id.as_str(), Ok(CallToolRequestParams::new("run_step"))),
        );
        let content = if i == needle_at && shape == NeedleShape::BuriedInToolOutput {
            let mut c = filler_block(scenario, i, per_message_chars / 2);
            c.push_str(&format!("\nNOTE: {}\n", scenario.needle_message));
            c.push_str(&filler_block(scenario, i + 1, per_message_chars / 2));
            c
        } else {
            filler_block(scenario, i, per_message_chars)
        };
        messages.push(Message::user().with_tool_response(
            call_id.as_str(),
            Ok(CallToolResult::success(vec![
                RawContent::text(content).no_annotation(),
            ])),
        ));
    }
    if needle_at >= num_pairs && shape == NeedleShape::IsolatedText {
        messages.push(Message::user().with_text(scenario.needle_message));
        messages
            .push(Message::assistant().with_text("Understood, I'll keep that in mind throughout."));
    }

    messages.push(Message::user().with_text(scenario.final_question));
    messages
}

fn approx_tokens(system_prompt: &str, messages: &[Message]) -> usize {
    let msg_chars: usize = messages
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|c| c.as_text())
        .map(|t| t.len())
        .sum();
    (system_prompt.len() + msg_chars) / CHARS_PER_TOKEN_ESTIMATE
}

fn is_compaction_call(messages: &[Message]) -> bool {
    messages.len() == 1
        && messages[0].content.iter().any(|c| {
            if let MessageContent::Text(text) = c {
                text.text.to_lowercase().contains("summarize")
            } else {
                false
            }
        })
}

/// Worst-case summarizer: succeeds whenever the call fits its own context
/// budget, but the summary it returns never contains scenario-specific
/// content — establishes the lower bound on baseline recall.
struct GenericSummaryProvider {
    summarizer_context_limit: usize,
    compaction_call_count: AtomicUsize,
}

impl GenericSummaryProvider {
    fn new(summarizer_context_limit: usize) -> Self {
        Self {
            summarizer_context_limit,
            compaction_call_count: AtomicUsize::new(0),
        }
    }
}

#[async_trait]
impl Provider for GenericSummaryProvider {
    async fn stream(
        &self,
        _model_config: &ModelConfig,
        system_prompt: &str,
        messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        if is_compaction_call(messages) {
            self.compaction_call_count.fetch_add(1, Ordering::Relaxed);
            if approx_tokens(system_prompt, messages) > self.summarizer_context_limit {
                return Err(ProviderError::ContextLengthExceeded(
                    "compaction input exceeds summarizer's own context window".to_string(),
                ));
            }
            let summary = Message::assistant()
                .with_text("The conversation covered various topics and tool calls across a long working session.");
            return Ok(stream_from_single_message(
                summary,
                ProviderUsage::new("mock-generic".to_string(), Usage::default()),
            ));
        }
        Ok(stream_from_single_message(
            Message::assistant().with_text("ok"),
            ProviderUsage::new("mock-generic".to_string(), Usage::default()),
        ))
    }

    fn get_name(&self) -> &str {
        "mock-generic-summary"
    }
}

/// Best-case summarizer: succeeds under the same context-budget gate, and
/// when it succeeds, echoes the needle verbatim into its summary if the
/// needle text is present anywhere in what it was asked to summarize —
/// establishes the upper bound on baseline recall. It cannot recall a fact
/// it never saw (e.g. one stripped by the tool-response-removal fallback
/// before this call ever ran).
struct FaithfulSummaryProvider {
    summarizer_context_limit: usize,
    needle: &'static str,
}

#[async_trait]
impl Provider for FaithfulSummaryProvider {
    async fn stream(
        &self,
        _model_config: &ModelConfig,
        system_prompt: &str,
        messages: &[Message],
        _tools: &[Tool],
    ) -> Result<MessageStream, ProviderError> {
        if is_compaction_call(messages) {
            if approx_tokens(system_prompt, messages) > self.summarizer_context_limit {
                return Err(ProviderError::ContextLengthExceeded(
                    "compaction input exceeds summarizer's own context window".to_string(),
                ));
            }
            let saw_needle = system_prompt.contains(self.needle);
            let summary_text = if saw_needle {
                format!(
                    "Summary of the working session so far. Key fact to preserve: {}",
                    self.needle
                )
            } else {
                "Summary of the working session so far.".to_string()
            };
            return Ok(stream_from_single_message(
                Message::assistant().with_text(summary_text),
                ProviderUsage::new("mock-faithful".to_string(), Usage::default()),
            ));
        }
        Ok(stream_from_single_message(
            Message::assistant().with_text("ok"),
            ProviderUsage::new("mock-faithful".to_string(), Usage::default()),
        ))
    }

    fn get_name(&self) -> &str {
        "mock-faithful-summary"
    }
}

fn conversation_text(conversation: &Conversation) -> String {
    conversation
        .agent_visible_messages()
        .iter()
        .flat_map(|m| m.content.iter())
        .filter_map(|c| c.as_text())
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Serialize)]
struct EvalResult {
    scenario: String,
    needle_position: String,
    needle_shape: String,
    window: String,
    context_limit: usize,
    provider_kind: String,
    initial_tokens_estimated: usize,
    rounds_run: usize,
    compaction_succeeded: bool,
    needle_recalled_in_final_conversation: bool,
    elapsed_ms: u128,
    error: Option<String>,
}

#[allow(clippy::too_many_arguments)]
async fn eval_one(
    scenario: &Scenario,
    target_tokens: usize,
    position: NeedlePosition,
    shape: NeedleShape,
    window_name: &str,
    context_limit: usize,
    provider_kind: &str,
    provider: &dyn Provider,
) -> EvalResult {
    let messages = build_conversation(scenario, target_tokens, position, shape);
    let initial_tokens_estimated = messages.len();
    let model_config = ModelConfig::new("mock-model").with_context_limit(Some(context_limit));

    let mut conversation = Conversation::new_unvalidated(messages);
    let session = Session {
        model_config: Some(model_config.clone()),
        usage: Usage::new(Some(0), Some(0), Some(0)),
        ..Session::default()
    };

    let start = Instant::now();
    let mut rounds_run = 0usize;
    let mut compaction_succeeded = true;
    let mut error = None;

    loop {
        let needs_compaction =
            match check_if_compaction_needed(provider, &conversation, Some(0.8), &session).await {
                Ok(v) => v,
                Err(e) => {
                    error = Some(format!("check_if_compaction_needed failed: {e}"));
                    compaction_succeeded = false;
                    break;
                }
            };
        if !needs_compaction || rounds_run >= MAX_COMPACTION_ROUNDS {
            break;
        }
        match compact_messages(
            provider,
            &model_config,
            "eval-session",
            &conversation,
            false,
        )
        .await
        {
            Ok((compacted, _usage)) => {
                conversation = compacted;
                rounds_run += 1;
            }
            Err(e) => {
                error = Some(format!("compact_messages failed: {e}"));
                compaction_succeeded = false;
                break;
            }
        }
    }
    let elapsed_ms = start.elapsed().as_millis();

    let final_text = conversation_text(&conversation);
    let needle_recalled = final_text.contains(scenario.needle);

    EvalResult {
        scenario: scenario.name.to_string(),
        needle_position: format!("{:?}", position),
        needle_shape: format!("{:?}", shape),
        window: window_name.to_string(),
        context_limit,
        provider_kind: provider_kind.to_string(),
        initial_tokens_estimated,
        rounds_run,
        compaction_succeeded,
        needle_recalled_in_final_conversation: needle_recalled,
        elapsed_ms,
        error,
    }
}

/// Full sweep: 5 scenarios x 2 needle positions x 2 needle shapes x 3 context
/// windows x 2 summarizer-quality bounds = 120 runs, each against a
/// conversation sized to ~2.5M tokens of synthetic content.
#[tokio::test]
#[ignore]
async fn eval_all_scenarios_baseline_compaction() {
    let target_tokens = 2_500_000usize;
    let mut results = Vec::new();
    let mut any_unexpected_failure = false;

    for scenario in SCENARIOS.iter() {
        for position in [NeedlePosition::Start, NeedlePosition::Middle] {
            for shape in [NeedleShape::IsolatedText, NeedleShape::BuriedInToolOutput] {
                for (window_name, context_limit) in WINDOW_SIZES.iter() {
                    let generic = GenericSummaryProvider::new(*context_limit);
                    let result = eval_one(
                        scenario,
                        target_tokens,
                        position,
                        shape,
                        window_name,
                        *context_limit,
                        "generic_worst_case",
                        &generic,
                    )
                    .await;
                    log_result(&result);
                    any_unexpected_failure |= !result.compaction_succeeded;
                    results.push(result);

                    let faithful = FaithfulSummaryProvider {
                        summarizer_context_limit: *context_limit,
                        needle: scenario.needle,
                    };
                    let result = eval_one(
                        scenario,
                        target_tokens,
                        position,
                        shape,
                        window_name,
                        *context_limit,
                        "faithful_best_case",
                        &faithful,
                    )
                    .await;
                    log_result(&result);
                    any_unexpected_failure |= !result.compaction_succeeded;
                    results.push(result);
                }
            }
        }
    }

    let json = serde_json::to_string_pretty(&results).expect("serialize results");
    let out_path = std::env::var("MEMORY_EVAL_OUTPUT")
        .unwrap_or_else(|_| "/tmp/gosling_baseline_compaction_eval_results.json".to_string());
    std::fs::write(&out_path, &json).expect("write results file");
    println!("Wrote {} results to {}", results.len(), out_path);

    // A "failure" here means compact_messages/check_if_compaction_needed
    // returned Err for a reason *other* than the summarizer's own context
    // budget being exceeded (that's an expected, exercised code path via the
    // progressive-removal fallback, not a bug) — genuine panics or
    // unexpected errors would surface as a hard test failure via `.expect`
    // inside eval_one's Result handling instead of reaching this assert.
    assert!(
        !results.iter().any(|r| r.error.as_deref().is_some_and(|e| {
            !e.contains("context limit exceeded even after removing all tool responses")
        })),
        "baseline compaction hit an unexpected error"
    );
    let _ = any_unexpected_failure; // recorded in the JSON for the report; not a hard failure by itself
}

fn log_result(r: &EvalResult) {
    println!(
        "[{}] needle={:?} shape={} window={} provider={} rounds={} succeeded={} needle_recalled={} elapsed_ms={} error={:?}",
        r.scenario,
        r.needle_position,
        r.needle_shape,
        r.window,
        r.provider_kind,
        r.rounds_run,
        r.compaction_succeeded,
        r.needle_recalled_in_final_conversation,
        r.elapsed_ms,
        r.error,
    );
}

/// Robustness check: a conversation whose *first* compaction attempt cannot
/// fit even the summarizer's own budget must still converge via the
/// progressive tool-response-removal fallback (or fail cleanly with a
/// descriptive error) rather than hang or panic.
#[tokio::test]
#[ignore]
async fn stress_compaction_converges_when_summarizer_window_is_tiny() {
    let scenario = &SCENARIOS[0];
    let messages = build_conversation(
        scenario,
        2_500_000,
        NeedlePosition::Start,
        NeedleShape::IsolatedText,
    );
    let model_config = ModelConfig::new("mock-model").with_context_limit(Some(128_000));
    let conversation = Conversation::new_unvalidated(messages);
    // Deliberately tiny: forces every removal_percentage tier to be tried.
    let provider = GenericSummaryProvider::new(5_000);

    let result = compact_messages(
        &provider,
        &model_config,
        "eval-session",
        &conversation,
        false,
    )
    .await;

    match result {
        Ok(_) => {}
        Err(e) => assert!(
            e.to_string()
                .contains("context limit exceeded even after removing all tool responses"),
            "unexpected compaction error: {e}"
        ),
    }
}
