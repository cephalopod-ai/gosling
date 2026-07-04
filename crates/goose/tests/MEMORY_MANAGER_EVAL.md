# Baseline long-conversation memory evaluation (compaction)

This harness ([`memory_manager_eval.rs`](./memory_manager_eval.rs) in this
file's directory) evaluates base gosling's only mechanism for handling
conversations that outgrow a model's context window: LLM-driven compaction
(`compact_messages` / `check_if_compaction_needed` in
`crates/goose/src/context_mgmt/mod.rs`).

It exists to establish the **baseline** for a comparative evaluation against
gosling-2's fork-specific "Context Manager" feature — the full comparative
report (five scenarios, three context-window sizes, recall/performance
metrics for both repos) lives in gosling-2's
`crates/goose/tests/MEMORY_MANAGER_EVAL.md`. This file documents only what's
specific to running the baseline half in this repo.

Run explicitly (heavy, `#[ignore]`d by default):

```
cargo test -p goose --test memory_manager_eval -- --ignored --nocapture
```

## Method summary

Five scenarios, each with a "needle" fact stated early in a ~2.1M–3.6M-token
synthetic conversation, evaluated at 128K/200K/1M-token context-window sizes,
with the needle at two positions (`Start`/`Middle`) and two shapes
(`IsolatedText` — its own message — vs. `BuriedInToolOutput` — embedded
inside a large tool-response body, as a config value would appear in a log).

No live model API is available in this environment. `compact_messages`
requires a real (mocked) `Provider` to perform its summarization step, and
recall of an isolated-text fact is entirely dependent on the quality of that
summarizer — a single mock can't honestly represent that, so the harness
uses two bounds instead: `generic_worst_case` (a summarizer that never
preserves specifics) and `faithful_best_case` (echoes the needle if it's
present in what it was asked to summarize). Both gate on an approximate
token budget matching the scenario's window size and return
`ProviderError::ContextLengthExceeded` when the compaction call itself
wouldn't fit its own simulated context — exercising the real progressive
tool-response-removal fallback (`removal_percentages = [0, 10, 20, 50, 100]`
in `do_compact`) at genuine multi-million-token scale.

## Headline result

| Needle shape | `generic_worst_case` | `faithful_best_case` |
|---|---|---|
| IsolatedText | 0/30 (0%) | 30/30 (100%) |
| BuriedInToolOutput | 0/30 (0%) | 0/30 (0%) |

Zero errors, zero failed compactions across all 120 runs — compaction always
converged in a single round for these scenarios. The structural finding: an
isolated-text fact's survival is **100% dependent on summarizer quality**
with no architectural guarantee in either direction; a fact **embedded
inside tool output cannot be recalled even by a hypothetically perfect
summarizer**, because the progressive-removal fallback strips tool-response
content before the summarizer ever sees it once the raw history is too large
for the compaction call itself to fit.

See gosling-2's `MEMORY_MANAGER_EVAL.md` for the full head-to-head against
the Context Manager feature, including where it does and doesn't close this
gap.
