# Audit Lens — Performance Profile

Lens: `audit-performance-profile` (measure-first). One lens of a multi-lens,
audit-only / read-only engagement. Builds on `docs/cloud/00-orientation.md`.

## Authority & evidence posture

- **Read-only.** No source modified; only this report written.
- **MEASURE-FIRST skill in a no-measurement environment.** A full release build
  (`cargo build --release`, ~11m per the README) and any provider-backed agent
  run were **not** performed here — the build toolchain, provider credentials,
  and the network-blocked `v8-goose`/local-model stacks are unavailable. **Every
  timing/throughput claim in this report is therefore `simulation-reasoned`
  (static hotspot reasoning), never `runtime-observed`.** No before/after
  numbers, percentiles, or profile shares are asserted. Per the skill's Evidence
  Rules, static-only bottleneck claims are capped at `Likely`; code *patterns*
  are quoted `file:line` and may be `Confirmed` as patterns, but their **runtime
  cost is not measured**.
- The one supplied evidence source with numbers is the **README footprint table**
  (`README.md:29-43`); it is scored against the measurement-quality rubric below
  and is the subject of the first two findings.

---

## 1. Target-metric definition

The prompt asks for two metric classes. Neither has a project-supplied baseline
usable as ground truth (the README's numbers are a *claim under audit*, not a
trusted baseline), so targets are constructed and the assumption stated.

| Field | Metric A — per-turn added latency | Metric B — agent cold start |
|---|---|---|
| Metric class | user latency (local CPU overhead the agent adds *around* the provider call) | startup / cold-start to "ready for first prompt" |
| Percentile | p50 **and** p95 across turns of one session | p50 + p95 across process launches |
| Measurement point | wall time from provider-stream-end of turn *k* to first byte sent for turn *k+1* (i.e. context re-assembly, excluding model inference/network) | `exec()` → agent initialized (config, provider registry, tokenizer BPE table, extension/MCP handshakes) ready to accept input |
| Workload | a real tool-loop session: conversation growing toward the model context limit (hundreds of messages, multi-KB tool outputs) | first launch, OS page cache cold, real config with ≥1 provider + typical extensions |
| Baseline | **unmeasured** (cannot build) | README claims 6.1 ms `--version` — see PERF-GSL-001 for why that is not this metric |
| Target | local per-turn overhead ≪ provider round-trip (stay < ~1% of turn wall); no super-linear growth over session length | ready-to-prompt within a small multiple of `--version` |

**Why it matters.** For Metric A the honest first-order fact is that an agent
turn's wall time is dominated by the **provider streaming call** (network +
model inference: hundreds of ms to tens of seconds). Local context re-assembly is
CPU over in-memory data. So the leverage question is not "is the local code
optimal" but "does the local per-turn work stay a rounding error, and does it
avoid super-linear growth as the session lengthens." That framing drives the
Amdahl prioritization: several real inefficiencies exist but sit behind a tiny
`p`, so they are correctly ranked Low.

---

## 2. Workload realism critique

- **Metric A realism:** the realistic agent workload is a *growing* conversation
  under a tool loop. The per-turn code re-processes the **entire** history each
  turn (tokenize + clone), so cost per turn grows with history length and total
  session cost is ~quadratic in turn count. This only matters at large scale
  (long sessions, big tool outputs); short chats never feel it. A benchmark that
  used a fixed 3-message conversation would be a PERF-003 toy-input lie and hide
  the scaling term.
- **Metric B realism:** the README benchmarks `--version` and `doctor`, which do
  essentially none of the real startup work (no provider handshake, no MCP
  subprocess spawn, no model catalog resolution against a configured provider).
  Real cold start is a different, larger workload — see PERF-GSL-001.

---

## 3. Measurement-quality rubric — README footprint table (`README.md:29-43`)

The only numeric evidence source supplied. Scored pass/fail; any FAIL caps what
the number can support.

| Dimension | Verdict | Basis |
|---|---|---|
| Repeatability (≥5 runs, median+spread, CV) | **FAIL** | All timings are "avg" with **no run count, no spread/CV**. `--version` = 8.4 ms→6.1 ms and `doctor` = 8.8→6.3 ms are sub-10 ms deltas where scheduler jitter and CPU frequency scaling dominate; a 2.3 ms gap is unsupported without run count + variance. Build time is a **single** "17m12s"/"11m26s" wall number. |
| Warm-up / cold-vs-warm stated | **FAIL** | Labeled "cold start" but no evidence the page cache was dropped between runs. A truly cold read of a 117–151 MB binary from disk alone is typically tens of ms; 6–8 ms strongly implies a **warm** binary — i.e. the number is warm exec latency mislabeled "cold start" (PERF-004). |
| Workload realism | **FAIL (timing rows)** | `--version`/`doctor` are toy commands, not the "quicker startup" of Vision (`README.md:27`). Static rows (packages, binary size, build dir) are realistic build artifacts. |
| Percentile sample size | **FAIL** | Bare averages; no p50/p95, no counts (PERF-005). For cold-vs-warm the distribution is bimodal by nature and an average hides it. |
| Observer effect | N/A-ish | Timing method (`/usr/bin/time`? `hyperfine`?) unstated; cannot assess. |
| Environment control | **PARTIAL** | "same host" and "matched Cargo feature flags, code-mode excluded symmetrically" are good hygiene. Governor/turbo/thermal pinning **not** stated — decisive at sub-10 ms. |
| Single variable | **FAIL for the deltas' narrative** | The table compares **goose v1.41.0** against **gosling v1.40.0**, but Provenance (`README.md:23`) states gosling is a fork of **goose v1.38**. The baseline is a *newer, diverged* goose, so "-186 packages / -22% binary" conflates gosling's intentional trimming with 3 minor-versions of upstream drift. Not a single-variable comparison. |

**Net:** the **static build-artifact rows** (Cargo.lock package count, stripped
binary size, `target/release` size, absent `libstdc++`) are the defensible part —
they are deterministic, measurable, and the −15%/−22%/−37% deltas are mechanistically
plausible given removal of the candle/llama.cpp/MLX/HF local-inference stack the
README itself attributes (148 crates, `README.md:43`). The **timing rows**
(cold-start ms, RSS, build time) are **quarantined**: they may motivate but cannot
support a performance conclusion. To the README's credit, it explicitly discloses
that **agent conversation/tool-calling throughput was not benchmarked and is not
expected to differ** (`README.md:43`) — this is honest and correctly bounds the
claim away from "faster agent," containing the overclaim risk to startup/footprint.

---

## 4. Static hotspot review (bottleneck classification)

No profile was obtainable, so the decision tree is applied statically and every
branch's runtime manifestation is downgraded to `Likely`/`Potential`.

**Where does a turn's wall time go?** Overwhelmingly **outside the process** — the
provider streaming call (`stream_response_from_provider`, `agent.rs:2047`). The
in-process context-assembly work per turn is CPU over in-memory `Vec<Message>`.
Thus the dominant class is **external-dependency bound (the model API)**, which no
local code change can help. The local hotspots below are real code patterns but
live behind a small Amdahl `p`.

### Per-turn local work in `reply_internal` loop (`agent.rs:1931-2075`)

Each loop iteration (one provider turn) does, over the **full** conversation:

1. `check_if_compaction_needed(...)` → `count_chat_tokens("", messages, &[])` — a
   full tokenization pass over every message (`context_mgmt/mod.rs:266`).
2. `session_manager.get_session(...)` reload (`agent.rs:1974`) — **and again**
   inside `inject_moim` (`agents/moim.rs:50-55`) — two session reloads/turn.
3. `inject_moim(&id, conversation.clone(), ...)` — a **full `Conversation` clone**
   (`agent.rs:2029`; `moim.rs:41` takes it by value).
4. `apply_context_manager` → `ContextManager::build` which:
   - clones all messages into the request (`agent.rs:1777`),
   - a full `count_chat_tokens` estimate (`context_mgmt/packet.rs:243`),
   - `selector::classify_blocks`, which tokenizes **per message** via
     `count_chat_tokens("", from_ref(msg), &[])` (`context_mgmt/selector.rs:13-14,101`).
5. `maybe_summarize_tool_pairs(..., conversation.clone(), ...)` — **another full
   `Conversation` clone** (`agent.rs:2071`).

So per turn: **~3+ full-history tokenization passes and ~3 full `Vec<Message>`
clones**. With history length `n` growing ~linearly in turn count `T`, per-turn
cost is `O(n)` and whole-session cost `O(T·n) = O(T²)` — an accidental-quadratic
scaling term (PERF-008 shape). See PERF-GSL-003.

**Mitigations already present (credit):** the tokenizer BPE table is built once
via `OnceCell` off the async workers (`token_counter.rs:206-221`); a **process-wide
LRU token cache survives across turns** (`shared_token_counter`,
`token_counter.rs:227-236`) so an unchanged message's *encode* is not repeated —
this removes the expensive part of the quadratic term. What remains is (a) a
blake3 hash of every content string on **every** count call to form the cache key
(`token_counter.rs:35-40`, `count_tokens` at `:54-55`) — `O(total bytes)` per pass
even on cache hits — and (b) the `Vec<Message>` clones and per-block allocations,
which the cache does not help.

### Security pattern scanner (`security/patterns.rs`, `security/scanner.rs`)

Regex set is compiled **once** via `LazyLock` (`patterns.rs:310-318`) — the "per
call vs once" question resolves cleanly to *once* (non-finding). On the scan path,
`scan_for_patterns` runs **all 43 regexes** over the input and, on any match, scans
a **second time** (`is_match` then `find_iter`, `patterns.rs:337-339`). This is the
**fallback** path used only when no ML classifier is configured
(`scanner.rs:200-215`), invoked per tool call over tool content whose size is
attacker/tool-influenced. See PERF-GSL-004.

---

## 5. Findings

### PERF-GSL-001: Cold-start claim rests on a toy, mislabeled, un-replicated benchmark

Severity: Medium
Confidence: Confirmed (methodology defect) / Likely (real-startup divergence)
Evidence basis: simulation-reasoned
Domain: Compliance-Posture (performance claim vs evidence)

Evidence:
- `README.md:27` — Vision claims "quicker startup".
- `README.md:40-41` — cold-start row: `--version` 8.4 ms avg / 24.0 MB RSS (goose)
  vs 6.1 ms / 17.7 MB (gosling); `doctor` 8.8 vs 6.3 ms. "avg" only, no run count,
  no percentiles.
- Real startup work not exercised by `--version`: provider registry init
  (`providers/init.rs:168-178`), tokenizer BPE build (`token_counter.rs:206-221`),
  MCP/extension subprocess spawning (per orientation §4).

Observed behavior:
- The headline "quicker startup" is supported only by `--version`/`doctor` timing.

Expected boundary:
- A startup claim should measure the metric it claims (agent ready-to-prompt),
  over ≥5 runs with a stated cold/warm protocol and spread, on a pinned host.

Failure mechanism:
- `--version` returns before doing agent init; its time reflects dynamic-linker +
  binary page-in, not startup. Sub-10 ms on a 117–151 MB binary indicates a
  **warm** binary mislabeled "cold start" (PERF-004). Bare averages hide the
  bimodal cold/warm distribution (PERF-005). The smaller-binary → fewer-pages →
  faster-exec mechanism is directionally plausible, but 0.1 ms precision is
  unsupported without run count + variance (PERF-003).

Break-it angle:
- Drop the page cache (`echo 3 > drop_caches`) and re-run: a genuinely cold read
  of the binary alone should dwarf 6–8 ms, exposing the label. Measure `gosling`
  reaching an interactive prompt with a real provider configured; that is the
  claimed metric and is expected to be materially larger and dominated by
  provider/MCP handshakes, not binary size.

Impact:
- The user-facing "quicker startup" claim is not evidenced by the cited numbers;
  a reader cannot trust the 27–29% startup deltas as representative of real use.

Operational impact:
- Blast radius: Repo (public claim). Side-effect class: user-visible (docs).
  Reversibility: reversible. Operator visibility: UI-visible (README).
  Rerun safety: safe.

Adjacent failure modes: PERF-GSL-002 (same table, baseline confound).

Recommended mitigation:
- Minimal: relabel the row as "`--version` warm exec latency", or replace with a
  ready-to-prompt measurement using `hyperfine` (≥10 runs, report median + σ,
  state cold/warm). Report p50 and p95. State CPU governor pinning.
- Guardrail: a docs check that any "cold start"/"startup" number cites a harness,
  run count, and cold/warm protocol.

Implementation assessment:
- Complexity: operator_ux. Cost: S. Cost drivers: re-run benchmark, docs.
  Nominal agent: human-owner (publishes a claim). Rationale: it is a published
  comparative claim, not a code defect.

Validation:
- Re-measure with a pinned harness; assert the reported metric is ready-to-prompt
  and carries run count + spread. Wall-clock budget with wide headroom, never on a
  shared CI runner.

Non-goals:
- Not disputing the static footprint rows (see §3).

---

### PERF-GSL-002: Footprint deltas confound gosling's trimming with upstream goose drift; version labels are inconsistent

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- `README.md:23` — "gosling **v0.0.1** is a fork of goose **v1.38**".
- `README.md:31` — footprint comparison is "goose v1.41.0 (commit 181cbbe)" vs
  "gosling v1.40.0 (commit 5b7d039)".
- `README.md:5,54` — gosling referred to as "v0.0.1" elsewhere.

Observed behavior:
- Three version identities for gosling appear (v0.0.1 in prose, v1.40.0 in the
  benchmark); the goose baseline (v1.41.0) is 3 minor versions **ahead** of the
  stated fork point (v1.38).

Expected boundary:
- A "what gosling removed vs goose" delta must be single-variable: same goose
  version the fork derives from, or an explicit statement that upstream drift is
  included.

Failure mechanism:
- Comparing against v1.41.0 folds packages goose added in v1.38→v1.41 into the
  "-186 packages / -22% binary / -37% build dir" deltas, over-attributing the
  reduction to gosling's intentional removals. Violates the rubric's single-variable
  row.

Break-it angle:
- Rebuild the exact fork-point goose (v1.38) with matched flags and re-diff; the
  package/size deltas attributable purely to gosling will differ from the table.

Impact:
- Overstates gosling's footprint win and undermines trust in the otherwise
  defensible static rows.

Operational impact:
- Blast radius: Repo. Side-effect class: user-visible (docs). Reversibility:
  reversible. Operator visibility: UI-visible. Rerun safety: safe.

Adjacent failure modes: PERF-GSL-001.

Recommended mitigation:
- Minimal: reconcile version labels; either compare against the fork-point goose
  or add a one-line caveat that the baseline includes upstream v1.38→v1.41 drift.

Implementation assessment:
- Complexity: operator_ux. Cost: XS. Cost drivers: docs. Nominal agent:
  human-owner. Rationale: published claim reconciliation.

Validation:
- Version strings in prose and table agree; baseline provenance stated.

Non-goals:
- Not re-running the full build comparison in this lens (cannot build here).

---

### PERF-GSL-003: Per-turn full-history re-tokenization and multiple `Conversation` clones — O(n)/turn, ~O(T²)/session local overhead

Severity: Low
Confidence: Likely (code pattern Confirmed; runtime cost unmeasured)
Evidence basis: simulation-reasoned
Domain: Performance (PERF-008 / PERF-011 shape)

Evidence:
- `agent.rs:2029` — `inject_moim(&id, conversation.clone(), ...)` full clone/turn;
  `agents/moim.rs:41` consumes `Conversation` by value.
- `agent.rs:2071` — `maybe_summarize_tool_pairs(..., conversation.clone(), ...)`
  second full clone/turn.
- `agent.rs:1777` — `conversation_messages: conversation.messages().clone()` into
  the context-build request.
- `context_mgmt/mod.rs:266` — `count_chat_tokens("", messages, &[])` full pass in
  the per-turn compaction check.
- `context_mgmt/packet.rs:243` + `context_mgmt/selector.rs:13-14,101` — another
  full tokenization estimate plus per-message tokenization in `classify_blocks`.
- `token_counter.rs:35-40,54-55` — cache key = blake3 over full text every call,
  so `O(total bytes)` per pass even on cache hits.

Observed behavior:
- Each provider turn rebuilds the context packet from scratch and clones the whole
  message vector ~3×; token counting walks the whole history several times.

Expected boundary:
- Per-turn local overhead should stay well below the provider round-trip and
  should not grow super-linearly over a session.

Failure mechanism:
- History `n` grows ~linearly with turns `T`; `O(n)` work per turn ⇒ `O(T²)`
  local CPU + allocation churn over a session. The LRU encode-cache
  (`token_counter.rs:227-236`) removes the expensive re-encode, leaving blake3
  hashing + `Vec<Message>` clones + block-classification allocation as the
  residual quadratic term.

Break-it angle:
- Drive a synthetic 300-turn session with multi-KB tool outputs and instrument
  per-turn local time (turn-end→next-request), excluding the provider call; fit
  t(2T)/t(T). A ratio ≫2 confirms the quadratic term. (Not executed here.)

Impact:
- Negligible for short chats; measurable CPU/allocation pressure only for very
  long, tool-heavy sessions — and even then likely dwarfed by provider latency.

Operational impact:
- Blast radius: Workflow (one session). Side-effect class: none (CPU/alloc).
  Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- Two `get_session` reloads/turn (`agent.rs:1974`, `moim.rs:50-55`) — chatty
  lookup (PERF-009 shape), same Low tier.

Recommended mitigation:
- Only if a profile shows it matters: borrow instead of clone into `inject_moim`
  / summarizer (take `&Conversation` or `Cow`); compute the token estimate once
  per turn and thread it through compaction-check + packet build rather than
  three independent full passes; dedupe the per-turn session reload.
- Guardrail: an instrumented per-turn "local overhead" counter and a
  scaling-ratio test (operation counts, not wall clock).

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: modules, one scaling test.
  Nominal agent: codex. Rationale: localized to the turn loop and context build.

Validation:
- Instrument clone/tokenize call counts per turn; assert they do not scale with
  history length after the fix (count-based, immune to timing noise).

Non-goals:
- Do not micro-optimize while unprofiled; per Amdahl (§6) this is Low.

---

### PERF-GSL-004: Command-injection regex fallback scans full tool output with 43 patterns (double scan on match)

Severity: Low
Confidence: Likely (pattern Confirmed; cost unmeasured)
Evidence basis: simulation-reasoned
Domain: Performance (PERF-012 shape)

Evidence:
- `security/patterns.rs:335-349` — loops all 43 `THREAT_PATTERNS`; on a match runs
  the regex **twice** (`is_match` then `find_iter`).
- `security/scanner.rs:200-215` — pattern scan is the fallback when no ML
  classifier is configured; runs over tool content.
- `security/patterns.rs:310-318` — regexes compiled once via `LazyLock` (the good
  part; see non-findings).

Observed behavior:
- 43 case-insensitive regex passes (44+ when a pattern matches) over each scanned
  tool content, whose size is tool/attacker-influenced.

Expected boundary:
- Threat scanning cost should scale gracefully with content size and not double-scan.

Failure mechanism:
- Large tool outputs (e.g. a multi-MB file read routed to the scanner) incur
  `~43 × |text|` regex work; the `is_match`+`find_iter` pair adds a redundant pass
  on match. Most passes are single (no match), so the dominant term is the 43×
  full scans, not the double-scan.

Break-it angle:
- Feed a 5–10 MB benign tool output through the fallback path and time it; scaling
  is linear in size but with a 43× constant. (Not executed.)

Impact:
- CPU spike proportional to tool-output size on the (classifier-absent) security
  path; user-perceptible only for very large scanned payloads.

Operational impact:
- Blast radius: Workflow. Side-effect class: none (CPU). Reversibility: reversible.
  Operator visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Only if profiled as hot: replace the per-pattern loop with a single
  `regex::RegexSet` pass to find which patterns match, then `find_iter` only those;
  and/or cap scanned length via the existing `large_response_handler` before
  scanning. Drop the redundant `is_match` before `find_iter`.
- Guardrail: a size cap assertion on scanner input (PERF-012 test).

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: one module, one test.
  Nominal agent: codex. Rationale: contained to the scanner.

Validation:
- Benchmark with a pinned harness on fixed large input; assert scan count/time
  scales with a `RegexSet` single pass. Also assert a max-scan-length cap.

Non-goals:
- No change to detection semantics (patterns/thresholds) in this slice.

---

## 6. Prioritization (Amdahl arithmetic)

`p` = fraction of the **target metric** attributable to the component. For Metric
A (turn latency), the provider streaming call dominates; local context assembly is
estimated `p ≲ 0.01` for typical sessions (unmeasured — the honest number requires
the §5 PERF-GSL-003 break-it harness).

| Rec | Metric | p (evidence) | assumed s | max_win = 1/(1−p) | projected movement |
|---|---|---|---|---|---|
| PERF-GSL-001 fix | Metric B claim honesty | n/a (claim, not runtime) | n/a | n/a | corrects a public claim; no runtime change |
| PERF-GSL-002 fix | footprint claim honesty | n/a | n/a | n/a | corrects attribution |
| PERF-GSL-003 (dedupe clones/tokenize) | Metric A | ≲0.01 typical; grows with session length (unmeasured) | ~2–3× on local overhead | ≈1.01× (short) | <1% turn latency; matters only for pathologically long sessions |
| PERF-GSL-004 (`RegexSet`) | scanner CPU on tool result | only on classifier-absent path, scales with payload | ~10–43× fewer passes | bounded by scan share of the turn | negligible unless huge scanned payloads |

**Highest-leverage next action:** the two **claim-accuracy** findings
(PERF-GSL-001/002) are the highest-value output of this lens — they are Confirmed
against README text and correct a public performance/footprint claim, whereas the
code hotspots are real but sit behind `p ≲ 0.01` and must not be optimized before
a profile justifies them. **Do not** touch the turn-loop clones or the scanner
until a profile (PERF-GSL-003 break-it harness) shows a non-trivial `p`.

---

## 7. Explicit non-findings (checked and held)

- **Regex compile-once:** security threat patterns compile a single time via
  `LazyLock` (`security/patterns.rs:310-318`); OpenAI/thinking format regexes use
  `OnceLock`/`LazyLock` (`gosling-providers/src/formats/openai.rs:1472+`,
  `thinking.rs:249-252`). No per-call regex compilation on these paths. **Held.**
- **Tokenizer build-once:** the tiktoken `o200k_base` BPE table is built once via
  `OnceCell` and off the async workers with `spawn_blocking`
  (`token_counter.rs:206-221`). Not rebuilt per turn. **Held.**
- **Token encode cache across turns:** `shared_token_counter` is a process-wide
  `OnceCell<Arc<TokenCounter>>` whose LRU survives turns
  (`token_counter.rs:227-236`), so unchanged prefixes are not re-encoded — the
  expensive part of the potential quadratic is already mitigated. **Held** (residual
  is blake3 keying + clones, PERF-GSL-003).
- **Canonical model / provider metadata catalogs:** embedded via `include_str!`
  and parsed once behind `Lazy`/`OnceCell`
  (`gosling-providers/src/canonical/catalog.rs:7`, `registry.rs:8-11`,
  `providers/init.rs:48,168`); no per-turn disk read or re-parse of the catalog.
  `providers()` clones its metadata vec per call but is a setup/UI path, not
  per-turn. **Held.**
- **`get_context_limit` default:** returns an in-memory value from the model config
  (`gosling-providers/src/base.rs:436-438`); no synchronous network call on the
  default per-turn path. (Provider overrides may enrich it; not traced per provider.)
- **Sync-in-async (PERF-010) on the turn loop:** the CPU-heavy tokenizer build is
  explicitly pushed to `spawn_blocking`; the summarizer is `tokio::spawn`-ed off the
  critical path (`agent.rs:1824`, `maybe_summarize_tool_pairs`). No obvious blocking
  syscall found on the hot turn path in the sampled code. **Held (sampled).**

---

## 8. Validation Limits (what was NOT done / NOT reviewed)

- **No build, no run, no profile, no benchmark.** All timing/scaling claims are
  `simulation-reasoned`; none are `runtime-observed`. The README's own numbers were
  not reproduced (cannot build).
- **PERF-001..016 coverage:** searched. Reported: PERF-003/004/005 (README),
  PERF-008/011 (turn loop), PERF-009 (session reloads), PERF-012 (scanner). Not
  observed in sampled code / not applicable: PERF-006/007 (N+1, DB index — no SQL
  hot path in the sampled agent loop; `session/` persistence not profiled),
  PERF-013 (lock contention — the `Mutex`/`RwLock` on provider/registry/token-cache
  were read but not stress-tested; token-cache `Mutex` is held only around LRU
  get/put, short), PERF-014 (per-op connection setup — provider HTTP client reuse
  not traced), PERF-015 (queueing — not applicable to single-user local turn loop),
  PERF-016 (bundle bloat — the `ui/desktop` Electron/JS startup and bundle size were
  **not** reviewed in this lens; that is a separate front-end startup surface).
- **Not reviewed:** `context_mgmt/summarizer/*`, `session/` persistence I/O costs,
  `providers/formats/*` per-request serialization cost, streaming decode path in
  `gosling-providers`, `ui/**` JS/Electron cold start and render performance, the
  `execution/manager.rs` multi-agent/subagent fan-out cost.
- **Provider-override `get_context_limit`** implementations (acp, litellm, openai)
  were not each traced for hidden network/blocking calls on the per-turn path.
- **Confidence ceiling applied:** per `confidence_calibration.md`, no
  resource-exhaustion or scaling outcome is marked Confirmed from static evidence;
  code *patterns* are Confirmed, their runtime cost is Likely/Potential.

## 9. Route

- Claim corrections (PERF-GSL-001/002): owner decision → docs update (human-owner).
- Code hotspots (PERF-GSL-003/004): **do not route to `repair-performance-bottleneck`
  yet** — they lack a measured `p`. First obtain a profile via the PERF-GSL-003
  break-it harness; route only if the profile shows a non-trivial share.
</content>
</invoke>
