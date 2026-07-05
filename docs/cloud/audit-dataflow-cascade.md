# Gosling Audit — Cascade & Failure Propagation Lens

Lens: `audit-dataflow-cascade` (v3.1). Domain prefix: **CAS**. IDs `CAS-GSL-NNN`.
Authority: **audit-only / read-only** (per `00-orientation.md`). No source modified.
Builds on `docs/cloud/00-orientation.md` (Phase 1/2 shared orientation).

> The supplied prompt is treated as a draft. I preserved the intended mission
> (blast radius, retry amplification, state poisoning, one-fault-takes-down-the-turn)
> but expanded review to adjacent seams: the grind/goal feedback loop, the
> poisoned-session thinking-block path, mid-stream partial persistence, and the
> optional-extension degrade path.

## Effort budget

~34 tool calls, prioritized on the retry/backoff module, the agent execution
loop (`agents/agent.rs` reply loop), subagent handler, execution manager, and
the extension/tool dispatch seams. Surfaces read fully or in the relevant
region are listed; everything else is in Validation Limits.

---

## 1. Propagation inventory (handoff → containment → consumer)

| # | Origin failure | Propagation path | Containment point | Downstream consumer | Amplifier | Blast radius |
|---|---|---|---|---|---|---|
| A | Deterministic 4xx (`RequestFailed` 400/404/422) | provider `stream()` → `with_retry_config` | `should_retry` + `max_retries=3` | provider API, turn latency | 3× retry + backoff on a request that can never succeed | Service (provider load), Workflow (latency) |
| B | Model stops with grind goal still set | reply loop `no_tools_called` → grind nudge re-inject | **only** `max_turns` (default 1000) | provider (每turn full resend) | growing-context resend, no convergence/no-progress escape | Workflow / cost |
| C | Signed thinking-block mismatch (model/config switch mid-session) | poisoned history → every `stream()` rebuilds same payload → permanent 400 | non-retry marker list (2 strings) | agent loop general-error arm → user | user "retry" resends same poisoned history | Workflow (session stuck) |
| D | One MCP extension dead/slow | `get_prefixed_tools` per-ext future | `Ok/Err` match → empty tools + `warn!` | model tool set (silently reduced) | none (contained) | Local (capability loss, silent) |
| E | Subagent reply stream errors / panics | `run_subagent_task` → `CallToolResult::error` | match arms (1030-1039, panic catch) | parent model as tool result | none (contained) | Local |
| F | Provider `Retry-After: 1e30` / far-future date | `extract_retry_after` | `MAX_RETRY_AFTER_SECS=3600` clamp | retry sleep | none (capped) | Local |
| G | Rate-limit / server error mid-turn | `with_retry_config` transient retry | cap 3 + backoff + jitter | provider | bounded | Service |

Rows A, B, C are findings. Rows D–G are non-findings (containment present) but D
carries an operator-signal escalation.

## 2. Boundary map (intended vs present)

- **Retry bulkhead** (`gosling-providers/src/retry.rs`): present, bounded (cap 3,
  exp backoff, jitter, `MAX_RETRY_AFTER` clamp) — but its *retryability
  predicate* treats deterministic 4xx as retryable by default (Finding 001).
- **Turn bulkhead** (`agents/agent.rs` reply loop): provider errors `break` the
  loop and are surfaced as a message, not re-thrown — good containment. But the
  *self-feeding* nudge path (grind) has only the coarse `max_turns` bulkhead
  (Finding 002).
- **Recovery bulkhead** (ContextLengthExceeded → compact): capped at
  `compaction_attempts >= 2` (agent.rs:2451) — present and testable.
- **Poisoned-payload bulkhead**: partial — a 2-string marker list stops the
  *retry*, but nothing repairs/quarantines the poisoned history, and the
  user-facing recovery advice misclassifies it as transient (Finding 003).
- **Delegation bulkhead** (`subagent_handler` / `summon`): present — failures and
  panics become error tool-results, never fail the parent turn.
- **Extension bulkhead** (`extension_manager::get_prefixed_tools`): present —
  a dead extension degrades to empty tools rather than failing tool listing.

## 3. Findings table

| ID | Title | Severity | Confidence |
|---|---|---|---|
| CAS-GSL-001 | Default retry config retries deterministic 4xx client errors (4× provider load, delayed surfacing) | Medium | Confirmed |
| CAS-GSL-002 | `grind` nudge is a self-feeding loop bounded only by `max_turns=1000`, with no convergence/no-progress escape | Medium | Confirmed |
| CAS-GSL-003 | Poisoned thinking-block history permanently fails the session; recovery advice misclassifies it as transient | Medium | Likely |

No Critical/High cascade was traced in this lens: every amplifier I confirmed is
bounded (retry cap, `max_turns`, compaction cap, `Retry-After` clamp). The blast
radius of the confirmed findings is provider load / token cost / a stuck session,
not durable corruption or security posture. Stated honestly rather than inflated.

---

## CAS-GSL-001: Default retry config retries deterministic 4xx client errors

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Cascade

Evidence:
- `crates/gosling-providers/src/retry.rs:99-108` — `should_retry`:
  `ProviderError::RequestFailed(_) => !config.transient_only`.
- `crates/gosling-providers/src/retry.rs:28-38` — `Default` sets
  `transient_only: false`.
- `crates/gosling-providers/src/retry.rs:266-271` — test
  `default_config_retries_request_failed` asserts a `RequestFailed("Bad request
  (400): model not found")` **is** retried under the default config.
- `crates/gosling-providers/src/base.rs:440-442` — the `Provider::retry_config`
  default is `RetryConfig::default()`.
- `crates/gosling-providers/src/ollama.rs:393-400` — the *only* provider that
  opts into `.transient_only()`. Anthropic (`anthropic.rs:328`), OpenAI
  (`openai.rs:662,724`), Google (`google.rs:187`), OpenRouter, litellm, nanogpt,
  kimicode, tetrate, githubcopilot, snowflake, chatgpt_codex, sagemaker all call
  `with_retry` with the default config.

Observed behavior:
- A 4xx `RequestFailed` (e.g. HTTP 400 invalid/oversized payload, 404 unknown
  model, 422 unprocessable) is retried up to `max_retries=3` times with
  exponential backoff (~1s, 2s, 4s + jitter) before the error surfaces.

Expected boundary:
- The retry predicate should treat deterministic client errors (4xx other than
  429) as non-retryable — the identical payload is rebuilt each attempt, so a
  4xx can never become a 2xx.

Failure mechanism:
- The retryability default is inverted: `transient_only` defaults to `false`, so
  the *permissive* branch (`!config.transient_only` = retry) is the default for
  every provider except ollama. The permanent-failure escape hatch is only a
  2-string marker list (`PERMANENT_REQUEST_FAILURE_MARKERS`, lines 88-97) scoped
  to Anthropic thinking-block rejections; every other deterministic 4xx falls
  through to "retry".

Break-it angle:
- Introduce a payload-construction bug (or an invalid configured model name):
  every affected call now hits the provider 4 times instead of once, each after
  a growing backoff, and the real error is delayed by ~7s. If the same bad call
  is issued by N parallel subagents (`summon`/delegate), the amplification is 4×N
  against the provider for a request that cannot succeed.

Impact:
- 4× provider request load and ~7s added latency per deterministic-failure call;
  masks/​delays the true error class. On metered providers this is 4× the failed
  spend. Bounded (cap 3), so not a runaway — an amplifier, not a storm.

Operational impact:
- Blast radius: Service (provider), Workflow (latency)
- Side-effect class: network (external API)
- Reversibility: reversible
- Operator visibility: log-only (`tracing::warn!` per retry, retry.rs:226-231)
- Rerun safety: safe

Adjacent failure modes:
- CAS-GSL-003 (the thinking-block permanent 400 is the *one* case the marker list
  catches — proving the class exists but is under-covered).
- Feeds CAS-GSL-002: a grind loop that keeps issuing a deterministic-failing call
  multiplies 4× per turn × up to 1000 turns.

Recommended mitigation:
- Remediation pattern: correct the retryability default.
- Minimal repair: flip the default so non-429 4xx are not retried — either make
  `transient_only` default `true`, or change `should_retry` so `RequestFailed`
  is retried only when the mapped HTTP status is 408/425/429/5xx. (429 already
  routes through `RateLimitExceeded`.)
- Local guardrail: keep `RateLimitExceeded`/`ServerError`/`NetworkError`
  retryable; classify `RequestFailed` by status, not by a global boolean.
- Behavior test: assert a 400/404/422 `RequestFailed` returns after exactly one
  attempt under the default config; assert 429/500/network still retry.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: one predicate + status plumbing into `RequestFailed`, provider
  opt-out audit, tests
- Nominal implementation agent: codex
- Rationale: single-module logic change; the risk is that some providers rely on
  4xx-retry to paper over flaky gateways, so the opt-out list must be audited —
  hence S not XS.

Validation:
- Unit test on `should_retry` for each status class; regression that ollama's
  existing `transient_only` behavior is unchanged.

Non-goals:
- Do not change backoff math or the `Retry-After` clamp.

---

## CAS-GSL-002: `grind` nudge is a self-feeding loop bounded only by max_turns

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Cascade

Evidence:
- `crates/gosling/src/agents/agent.rs:2610-2624` — when `no_tools_called` and a
  grind goal is set, the loop injects `"Keep working. The grind goal is not yet
  complete… Continue until it is fully done."` and continues.
- `crates/gosling/src/agents/agent.rs:2588-2595` — the **goal** path damps itself
  with `goal_check_pending = true` (one re-check), then stops.
- `crates/gosling/src/agents/agent.rs:2625-2628` — `set_goal(None)/set_grind(None)`
  and `exit_chat=true` live in the `else` arm, only reached when *both* goal and
  grind nudges are `None`. While `grind` is `Some`, control always takes the
  grind branch, so this clearing arm is unreachable.
- `crates/gosling/src/agents/agent.rs:1916-1967` — the only bound: `turns_taken`
  increments per iteration, breaks at `max_turns`.
- `crates/gosling/src/agents/agent.rs:63` — `DEFAULT_MAX_TURNS: u32 = 1000`.
- `crates/gosling/src/agents/execute_commands.rs:466,473` — grind is
  set/cleared *only* by the `/grind` slash command.

Observed behavior:
- Once a user sets a grind goal, every turn where the model stops without calling
  a tool re-injects the "keep working" nudge and re-enters the loop, resending the
  full (monotonically growing) conversation to the provider, until `max_turns`
  (default 1000) is hit or the user manually runs `/grind` to clear it.

Expected boundary:
- A "keep working" feedback loop needs a damping/convergence check: no-progress
  detection, a nudge counter, or a token/cost budget — the same class of damping
  the sibling **goal** path already has (`goal_check_pending`).

Failure mechanism:
- Output (the nudge) re-enters input (next turn) with no terminal condition other
  than the coarse `max_turns`. The model asserting "done" (no tool call) is
  exactly the trigger that re-nudges, so a model that believes the task is
  complete cannot end the loop; it is pushed up to 1000 times.

Break-it angle:
- Set a grind goal the model considers already satisfied (or impossible). It will
  be re-nudged every turn; each turn resends a larger conversation. Combined with
  CAS-GSL-001, a deterministic-failing call inside the loop is retried 4× per
  turn × up to 1000 turns.

Impact:
- Up to ~1000 provider calls with growing context for a single user request; on
  metered providers this is a large, silent cost/token amplifier. No corruption,
  but a real blast-radius/cost cascade. The asymmetry with the damped goal path
  shows the missing bulkhead is an omission, not a deliberate design.

Operational impact:
- Blast radius: Workflow (cost/token)
- Side-effect class: network (external API), user-visible (nudge messages)
- Reversibility: reversible (user can `/grind` off or cancel)
- Operator visibility: UI-visible (grind nudge notifications) but the *cost*
  trajectory is not surfaced
- Rerun safety: safe

Adjacent failure modes:
- CAS-GSL-001 (per-turn 4× multiplier inside the loop).
- Compaction cost: each grind turn can re-trigger auto-compaction
  (agent.rs:1975), adding fast-model calls per iteration.

Recommended mitigation:
- Remediation pattern: add loop damping + convergence check.
- Minimal repair: add a grind-specific no-progress / consecutive-nudge counter
  (mirror `goal_check_pending`) that clears grind and exits after K nudges
  without a tool call, or gate on a token/turn budget distinct from `max_turns`.
- Local guardrail: cap consecutive grind nudges (e.g. reuse the
  `stop_hook_block_cap` pattern already in this loop, agent.rs:2699-2706).
- Behavior test: with a model stub that always stops without tools, assert the
  grind loop exits after K nudges rather than running to `max_turns`.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: loop-state field, exit semantics, tests against a stub model,
  UX decision on the cap value
- Nominal implementation agent: claude
- Rationale: touches turn-loop control flow with several interacting exit paths
  (steers, stop-hook, goal, compaction); needs careful reasoning about existing
  breaks, so M and context-heavy.

Validation:
- Loop test asserting bounded grind nudges under a no-tool model; assert goal
  path unchanged.

Non-goals:
- Do not remove the grind feature or change `max_turns`.

---

## CAS-GSL-003: Poisoned thinking-block history permanently fails the session; recovery advice misclassifies it as transient

Severity: Medium
Confidence: Likely
Evidence basis: source-evidenced (retry + surfacing); simulation-reasoned (persistence)
Domain: Cascade

Evidence:
- `crates/gosling-providers/src/retry.rs:84-97` — comment + marker list document
  the mechanism: Anthropic rejects signed `thinking`/`redacted_thinking` blocks
  "once a thinking model's config changes mid-conversation, and the identical
  payload is rebuilt on every retry — so retrying can never succeed."
- `crates/gosling-providers/src/retry.rs:104` — such a `RequestFailed` is
  correctly *not* retried.
- `crates/gosling/src/agents/agent.rs:2547-2557` — the general provider-error arm
  yields `"Ran into this error: {provider_err}. Please retry if you think this is
  a transient or recoverable error."` and `break`s.
- No repair/quarantine of the offending thinking blocks was found on this path
  (the ContextLengthExceeded arm compacts, agent.rs:2446-2497; there is no
  analogous strip/repair for the permanent thinking-block 400).

Observed behavior:
- A thinking-block mismatch (introduced by switching thinking model/config
  mid-session) makes the rebuilt payload fail with a permanent 400 on *every*
  subsequent provider call. The retry layer correctly stops retrying, but the
  agent surfaces a generic "please retry if transient" message and the poisoned
  blocks remain in session history, so the next user turn rebuilds the same
  payload and fails identically.

Expected boundary:
- A recognized-permanent poisoned-payload error should (a) be classified as
  non-transient in the user-facing message, and (b) trigger a repair path (strip
  the offending signed blocks / quarantine the message) or a clear "start a new
  session" instruction — the way `Refusal` (agent.rs:2522-2534) and
  `ContextLengthExceeded` are handled with tailored guidance.

Failure mechanism:
- The knowledge that this failure class is *permanent* exists in the retry layer
  (the marker list) but is not propagated to the agent-loop error arm, which
  applies generic transient-retry advice (downstream misclassification), and no
  stage repairs the poisoned history (state poisoning persists).

Break-it angle:
- Start a session on a thinking model, switch model/config mid-conversation to
  invalidate the signed blocks; every following turn fails and the user is told
  to "retry", which resends the same poisoned history and fails again — a stuck
  session with no in-product recovery except starting over.

Impact:
- A single mid-session config change permanently bricks that session; the
  operator is actively misdirected toward a retry that cannot work. Bounded to
  one session (no cross-session spread), hence Medium.

Operational impact:
- Blast radius: Workflow (one session)
- Side-effect class: user-visible
- Reversibility: compensatable (new session) but not in-place recoverable
- Operator visibility: UI-visible but misleading (says transient)
- Rerun safety: unsafe (rerun reproduces the failure)

Adjacent failure modes:
- CAS-GSL-001 (same 4xx-permanence class, under-covered generally).
- CAS-008 error-context: the specific provider cause is flattened into generic
  advice at the agent boundary.

Recommended mitigation:
- Remediation pattern: propagate permanence classification + add repair/guidance.
- Minimal repair: map the permanent-marker case (or a dedicated
  `ProviderError::PermanentRequestFailure`) to a tailored agent-loop arm that
  says the conversation cannot continue and offers a new session — mirroring the
  `Refusal` arm — and/or strip the offending signed thinking blocks before the
  next request.
- Local guardrail: reuse `is_permanent_request_failure` at the agent boundary so
  the message class is consistent with the retry decision.
- Behavior test: given a permanent thinking-block 400, assert the surfaced
  message does not advise a plain retry and (if repair chosen) that the next
  request omits the offending blocks.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: a new error variant or shared classifier, an agent-loop arm,
  optional history-repair, provider-specific tests
- Nominal implementation agent: claude
- Rationale: crosses the provider→agent boundary and touches conversation
  repair; needs care to avoid stripping blocks other providers require echoed.

Validation:
- Test the surfacing message + (optional) repair on a simulated permanent 400.

Non-goals:
- Do not broaden the marker list in this slice (that is CAS-GSL-001's status-based
  fix); scope here is surfacing + recovery.

---

## 4. Non-findings (checked and held)

- **Retry-After abuse (CAS-003)** — `Not Confirmed`. `extract_retry_after` clamps
  server-dictated delays at `MAX_RETRY_AFTER_SECS = 3600`
  (`http_status.rs:37,64-70,79-88`) and rejects NaN/negative/infinite; a hostile
  `retry_after_seconds: 1e30` degrades to ≤1h, not a freeze/panic.
- **Nested provider retries (CAS-003)** — `Not Confirmed`. The two `with_retry`
  calls in `openai.rs:662` and `:724` are mutually exclusive if/else branches;
  `databricks.rs:685` and `:702` are a first-try + one-shot `stream_options`
  fallback, not a retry-inside-retry. No compounding `max_retries²`.
- **Agent loop re-throwing provider errors (CAS-001)** — `Not Confirmed`. Every
  provider-error arm (`agent.rs:2446-2557`) `break`s and yields a message; the
  loop does not wrap the provider call in its own retry, so provider retries are
  not multiplied by a turn-level retry.
- **Subagent failure fails the parent turn (CAS-010)** — `Not Confirmed`.
  `run_subagent_task` maps errors and panics to `CallToolResult::error`
  (`summon.rs:1030-1039`; panic catches at `summon.rs:730,780,1331`); a failing
  delegate is a tool-result error, not a parent-turn abort.
- **One dead MCP extension takes down tool listing (CAS-012)** — `Not Confirmed`.
  `get_prefixed_tools` catches per-extension `list_tools` errors and returns
  empty tools with a `warn!` (`extension_manager.rs:1393-1402,1441-1445`); other
  extensions still contribute. See operator-signal escalation below.
- **Compaction recovery runs away (CAS-014)** — `Not Confirmed`. ContextLength
  recovery is capped at `compaction_attempts >= 2` (`agent.rs:2449-2460`) and
  the counter resets only on a successful chunk.
- **Stop-hook denial loop (CAS-002)** — `Not Confirmed`. Bounded by
  `consecutive_stop_hook_blocks > stop_hook_block_cap` (`agent.rs:2699-2706`).
- **Orphaned tool_request poisoning history (CAS-004)** — `Not Confirmed`.
  Tool requests are pushed to history paired with their response in the same
  iteration (`agent.rs:2435-2437`); streaming yields tool calls only once
  complete (`base.rs:282-287`), so a mid-stream break leaves no half-request.

## 5. Break-it review (per skill checklist)

- Poison one input record → traced C (thinking-block) and the orphaned-request
  case; C poisons the session (finding), orphan case is contained.
- Fail one item in a batch → subagent/extension failures are isolated
  (non-findings D, E).
- Report/finding into a generated patch prompt → N/A (no report→patch chain in
  this runtime; that seam belongs to the audit tooling, not gosling).
- Trigger a retry loop → A (4xx retried, bounded 4×) and G (transient, bounded).
- Break the optional integration → D (extension degrades to empty tools).
- Force the recovery path → compaction cap holds; thinking-block "recovery"
  advice is wrong (C).
- Flood alerts → retries log `warn!` per attempt but are bounded; no alert-storm
  masking (no paging/health system in-process).

## 6. Patch order

1. **CAS-GSL-001** (S, codex) — smallest, highest leverage; also shrinks the
   amplifier feeding 002/003.
2. **CAS-GSL-003** (M, claude) — reuse `is_permanent_request_failure` at the agent
   boundary; depends conceptually on 001's classification.
3. **CAS-GSL-002** (M, claude) — loop damping; largest control-flow surface.

## 7. Regression / guardrail tests

- `should_retry`: 400/404/422 → one attempt; 408/425/429/5xx/network → retried
  (CAS-GSL-001).
- Grind loop with a no-tool model stub exits after K nudges, not `max_turns`
  (CAS-GSL-002).
- Permanent thinking-block 400 surfaces non-transient guidance and (if repair
  chosen) drops offending blocks from the next request (CAS-GSL-003).
- Regression: ollama `transient_only` unchanged; compaction cap, stop-hook cap,
  subagent error-result, extension degrade all still hold.

## 8. Skill escalation (cross-lens)

| Observation | Owning lens | Why |
|---|---|---|
| Dead MCP extension silently drops its tools with only a `warn!` (`extension_manager.rs:1399`), so the model loses capability with no user-visible signal and may hallucinate/misreport | `audit-operator-signal` | Silent degraded mode; cascade lens confirms containment but not the signal gap |
| Retry `warn!` per attempt is log-only; the *cost* trajectory of grind + 4×-retry is never surfaced to the operator | `audit-operator-signal` / `audit-performance-profile` | Cost blast radius is invisible pre-limit |
| `transient_only` default-false is an inverted-default footgun new providers inherit silently | `audit-negative-space` / `audit-architecture-seam` | Default-unsafe boundary; every new provider inherits the wrong retry posture |
| Grind loop cost (up to 1000 resends of growing context, plus per-turn compaction) | `audit-performance-profile` | Token/cost amplification magnitude is a runtime property |
| Mid-session model/config switch invalidating signed thinking blocks | `audit-state-transition` / `audit-temporal` | The state transition that plants the poison is upstream of this lens |

## 9. Validation Limits (NOT reviewed / not proven)

- **Not executed.** All findings are static/simulation-reasoned; no live agent
  run, no provider credentials, no reproduction of the 4× load, the grind loop, or
  the poisoned-session path. Magnitudes (actual cost, actual latency) are
  therefore Likely at most for runtime claims, per the lens confidence rule.
- **Providers sampled, not exhaustive.** I confirmed the default-retry inheritance
  for the providers listed in Finding 001 and the sole ollama opt-out; I did not
  read every one of the 15+ provider `stream()` bodies (bedrock/gcpvertexai/
  databricks_v2 load a config-driven `RetryConfig` whose `transient_only` I did
  not trace end-to-end — `databricks.rs:155`, `gcpvertexai.rs:198`,
  `databricks_v2.rs:134`). If any of those default `transient_only` true, they are
  already safe; the finding stands for the default-config majority regardless.
- **ACP subprocess providers** (`claude_acp`, `codex_acp`, `copilot_acp`,
  `gemini_cli`, `cursor_agent`, etc.) manage their own context/retry
  (`manages_own_context`); their internal retry/backoff and crash-restart
  behavior was **not** reviewed here and may harbor separate amplification.
- **`gosling-server` and CLI turn drivers** (`crates/gosling-server/src/routes/
  agent.rs`, CLI reply drivers) were not read; a server-level retry wrapping the
  agent turn (if any) would change the amplification math and is unverified.
- **Mid-stream SSE failure persistence** (partial assistant message written at
  `agent.rs:2680-2683` after a recovery-compact replaced `conversation`) was
  reasoned about but not reproduced; a possible duplicate/ordering edge exists
  under CLE-after-partial-Ok and is left for `audit-state-transition`.
- **Concurrency amplification** (many sessions each retrying 4× simultaneously
  against one provider, thundering herd despite jitter) is a runtime property
  and was not load-tested — `requires-authorized-drill`.
