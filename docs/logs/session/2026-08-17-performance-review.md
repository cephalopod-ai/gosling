# 2026-08-17 — Performance review and optimization opportunity log

**Branch:** detached `HEAD` at `main` @ `1a5f470c` (no remote mutation)
**Skill:** documentation/performance stewardship (read-only review)
**Scope:** re-assess the performance findings from
[`docs/cloud/audit-performance-profile.md`](../../cloud/audit-performance-profile.md),
verify each cited source path against current HEAD, scan the areas the
audit explicitly did not review for additional opportunities, and record the
result in the to-do ledger and this log. **No source code was modified**; this
is an analysis-only pass consistent with the audit's read-only posture.

## Gate 0 — baseline

Working tree clean, `main` in sync with origin at `1a5f470c`. No build, run, or
profile was executed in this environment (matching the original audit's
no-measurement constraint); all claims here are `simulation-reasoned` against
current source, with code patterns quoted `file:line` and marked `Confirmed`
as patterns. Runtime cost remains `Likely`/`Potential`, never `Confirmed`.

## Gate 1 — inventory of existing PERF findings

The performance audit froze four findings (PERF-GSL-001 through 004) plus a
§7 list of explicit non-findings. `docs/TODO.md` previously tracked only
PERF-GSL-002 (as a one-line entry in the "Lower priority" paragraph). This
review carries the full series into the ledger so none is rediscovered from
scratch.

## Gate 2 — re-verification against current HEAD

Each cited path was re-read. Findings below record the current state, not the
audit-time state, so the ledger does not stale-cite line numbers that have
drifted.

### PERF-GSL-001 / PERF-GSL-002 — README and Desktop harness (claim hygiene)

**Unchanged.** No code touched. These remain `human-owner` / docs decisions:
the README cold-start row is still a historical v0.0.5 run, and
`ui/desktop/tests/e2e/performance.spec.ts` is still a single-run smoke script.
Carried as open `[ ]` in the ledger; no re-measurement was performed here.

### PERF-GSL-003 — per-turn full-history re-tokenization and Conversation clones

**Re-verified open.** Line numbers drifted (audit cited `agent.rs:1931-2075`;
the reply loop is now around `agent.rs:2330-2440`) but every cited pattern is
present:

- `agent.rs:2391` — `inject_moim(&session_config.id, conversation.clone(), …)`,
  a full `Vec<Message>` clone per turn. `inject_moim` takes `Conversation` by
  value (`moim.rs:41`), so the clone is forced at the call site.
- `agent.rs:2435` — `maybe_summarize_tool_pairs(…, conversation.clone(), …)`,
  a second full clone per turn.
- `agent.rs:2339` — `session_manager.get_session(&session_config.id, false)`
  reload per turn, **plus** a second `get_session` inside `inject_moim`
  (`moim.rs:53`). Two session reloads per turn.
- `token_counter.rs:35-40,54-55` — `count_tokens` computes
  `TokenCacheKey::from_text` (a `blake3::hash` over the full text) on **every**
  call, including cache hits. The LRU eliminates the expensive re-encode; the
  blake3 keying plus the `Vec<Message>` clones are the residual quadratic term.

**Diagnosis.** History length `n` grows ~linearly with turn count `T`; the
per-turn work is `O(n)`, so whole-session local cost is `O(T·n) = O(T²)`. The
process-wide LRU encode-cache (`token_counter.rs:227-236`) already removed the
expensive part of the quadratic (re-encoding unchanged prefixes); what remains
is the per-call blake3 hash (which is `O(total bytes)` per pass even on hits)
and the `Conversation` clones (which the cache cannot help).

**Walkthrough of a fix (not applied).** Take `&Conversation` (or `Cow`) into
`inject_moim` and the summarizer instead of consuming by value, eliminating
the two forced clones per turn. Thread a single token estimate computed once
per turn through the compaction-check and the context-packet build, rather
than the three independent full passes observed (`context_mgmt/mod.rs:436`,
`packet.rs:243`, `selector.rs:13-14`). Deduplicate the two `get_session`
reloads. Optionally, skip the blake3 hash for short texts where a direct
`encode_with_special_tokens` would be cheaper than hashing.

**Justification for leaving open.** Per the audit's own §6 Amdahl arithmetic,
this sits behind `p ≪ 0.01` of a turn's wall time (the provider streaming call
dominates). The audit's §9 explicitly says **do not route to
`repair-performance-bottleneck` yet** — first obtain a profile via the
PERF-GSL-003 break-it harness, and only act if it shows a non-trivial share.
Premature micro-optimization of a sub-1% path is the failure mode this guards
against. Recorded as open `[ ]` with that guardrail restated.

### PERF-GSL-004 — security scanner double-scan

**Status changed: the double-scan is resolved.** The audit reported
`scan_for_patterns` (`patterns.rs:335-349` at audit time) running `is_match`
then `find_iter` per pattern — a redundant second regex pass on every match.
Current `patterns.rs:334-348` runs `find_iter` directly in the loop with **no**
preceding `is_match`; the redundant pass is gone. (Verified: the only
`is_match` calls in `crates/gosling/src/security/` are in `egress_inspector.rs`,
on different, non-scanner paths.)

What remains open is the **37-pattern sequential loop** over each scanned tool
output without a single `regex::RegexSet` pass (the audit cited 43 patterns;
the set was trimmed). This is the same Low tier and the same "profile first"
guardrail. Recorded as `[~]` (partially addressed) in the ledger with the
status change called out, so a future reader does not re-report the already-
fixed double-scan.

**Walkthrough of the remaining fix (not applied).** Replace the per-pattern
`find_iter` loop with a single `RegexSet` pass to discover which patterns match,
then `find_iter` only those — collapsing `~37 × |text|` regex work toward
`2 × |text|` in the common (few-match) case. The `large_response_handler`
already caps scanned length on another path; the scanner would benefit from
the same size cap as a guardrail.

## Gate 3 — areas the audit explicitly did not review

The audit's §8 listed unreviewed surfaces. This pass sampled three of them
for *additional* opportunities (still read-only, still un-profiled).

### New: PERF-GSL-005 — SSE streaming chunk double-deserialization

The audit's §8 said the streaming decode path in `gosling-providers` was "not
reviewed." Sampling it found a concrete, contained pattern:

`parse_streaming_chunk`
(`gosling-providers/src/formats/openai.rs:1088-1116`) deserializes every SSE
data line twice:

1. `serde_json::from_str::<Value>(line)` — to check for an `error` field or
   `object == "error"` shape (lines 1097-1113);
2. `serde_json::from_value::<StreamingChunk>(value)` — to produce the typed
   chunk (lines 1115-1116), re-walking the same `Value` tree.

`StreamingChunk` already derives `Deserialize` (`openai.rs:144`). This is on
the hot streaming path: `parse_streaming_chunk` is called per SSE data line via
`response_to_streaming_message` (`openai.rs:1165`), and again per tool-call
data line (`openai.rs:1214`).

**Diagnosis.** The first parse allocates a full `serde_json::Value` tree; the
second re-walks it. The `Value` intermediate exists only to discriminate the
error shape, which could be checked on the typed struct (an error response
fails to populate `choices`) or via a cheaper byte/`Deserializer` peek before
committing to the full parse.

**Walkthrough of a fix (not applied).** Deserialize straight to
`StreamingChunk` via `serde_json::from_str`, then inspect the typed struct
(empty `choices` + a present `error`/`object` field → treat as server error),
or use `serde_json::Deserializer::from_str` with `peek`/`byte_offset` to
detect the error shape before the full deserialize. This eliminates one full
`Value`-tree allocation + re-walk per chunk. The error-detection semantics
must be preserved exactly (both the `error` field and the
`object == "error"` variant).

**Justification for Low severity.** A single SSE chunk is small (typically a
few tokens of delta), and this is bounded CPU per token streamed. A turn's wall
time is dominated by provider network + model inference, so the local decode
share is small. It only matters under very high chunk throughput (long,
fast-streaming turns). It is a legitimate, contained optimization with a clear
guardrail, recorded as a new `[ ]` finding (PERF-GSL-005).

### Non-findings (checked and held)

- **Session persistence SQLite config** — `session_manager.rs:1399-1404` uses
  WAL journal mode with `Synchronous::Normal`, the standard corruption-safe /
  fast-commit pairing, and a 30s busy timeout. Already well-tuned; no change
  indicated. **Held.**
- **Regex compile-once / tokenizer build-once** — re-confirmed:
  `THREAT_PATTERNS` compile behind `LazyLock` (`patterns.rs:310-318`); the
  tiktoken BPE table builds once via `OnceCell` + `spawn_blocking`
  (`token_counter.rs`). **Held.**
- **FileMemorySource sync read** — `context_mgmt/memory.rs:131-153` still does
  a synchronous `read_to_string`, but it is now bounded by
  `MAX_MEMORY_FILE_BYTES` (the MEM-GSL-002 cap) and takes a shared file lock.
  Bounded and on the context-build path, not the per-token hot path. **Held.**
- **Provider catalog parse-once** — embedded via `include_str!` behind
  `Lazy`/`OnceCell`; not per-turn. **Held.**
- **Execution manager fan-out** — `execution/manager.rs` uses
  `tokio::spawn` + `join_all` for concurrent agent creation (lines 606-612,
  638-644), which is the right shape for parallel fan-out; no serializing loop
  found on the multi-agent path. **Held (sampled).**

## Validation

No build, test, or profile was run (read-only review, consistent with the
audit's no-measurement environment). Structural validation of the
documentation patch:

```sh
git diff -- docs/TODO.md docs/logs/session/2026-08-17-performance-review.md
grep -R "GILES:DOCS-GOVERNANCE:START" -n AGENTS.md
```

`git diff` confirms only `docs/TODO.md` (new performance section) and the new
session log are touched; no source, lockfile, or config change. The AGENTS.md
governance marker is intact.

## Risks and follow-ups

- **All code findings are un-profiled.** The audit's §6 and §9 guardrail
  applies: do not implement PERF-GSL-003/004/005 until a profile (the
  PERF-GSL-003 break-it harness) shows a non-trivial share of a turn. This log
  records the opportunities and their justifications so the work is not lost,
  not so it is done prematurely.
- **PERF-GSL-001/002 are claim-accuracy, not code.** They are `human-owner`
  decisions and were not modified here.
- **PERF-GSL-004 status change** is recorded as `[~]` so a future pass does not
  re-report the resolved double-scan; the remaining `RegexSet` opportunity is
  still open.
- **Line numbers will drift again.** The ledger cites current-HEAD line numbers
  with the function/file names alongside; re-verify by name if the numbers
  stale before acting.
