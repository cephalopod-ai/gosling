# Gosling Audit — Reliability Lens (`audit-reliability` v3.0)

Authority: **audit-only / read-only**. No source modified. Builds on
`docs/cloud/00-orientation.md`. Domain prefix: `REL`.

Draft-prompt note: the supplied prompt is treated as a draft. I preserved the
intended mission (failure behavior on provider/MCP/agent-loop/doctor paths) and
expanded to the adjacent security-inspector degradation seam, which is where the
system's "reports healthy while a subsystem failed" risk actually lives.

## Effort budget & method

~28 tool calls, source-only (no build/run). Prioritized per
`audit_method.md §Prioritization`: provider request/retry path, MCP client,
agent reply loop, doctor, then the security-inspector degradation seam. Runtime
manifestations (turn hang, live fail-open) are `simulation-reasoned`, not
reproduced. Static confirmations quote a line actually read.

## Failure-mode inventory (REL-001 … REL-015)

| ID | Mode | Result |
|---|---|---|
| REL-001 | Startup failure hidden | Partial — REL-GSL-002 (classifier init swallowed) |
| REL-002 | False-healthy state | Non-finding — doctor actively tests provider (held) |
| REL-003 | Silent degradation | **REL-GSL-002, REL-GSL-003** |
| REL-004 | Retry storm | Non-finding (backoff/jitter/cap held); minor REL-GSL-004 |
| REL-005 | Resource exhaustion | Non-finding — loop/compaction bounded (held) |
| REL-006 | Missing timeout | **REL-GSL-001** (uncapped delay); other timeouts held |
| REL-007 | Crash mid-operation | REL-GSL-005 (asserts/expects on MCP path) |
| REL-008 | Non-atomic recovery | Not reviewed (session persistence out of budget) |
| REL-009 | Failure misclassified | Non-finding — agent loop + http_status classify well (held) |
| REL-010 | Error swallowed | **REL-GSL-002**; minor REL-GSL-006 (ApiResponse) |
| REL-011 | Partial output treated success | Non-finding — handle_response requires valid JSON (held) |
| REL-012 | Cleanup failure | Non-finding — ActiveToolCallGuard Drop cleans up (held) |
| REL-013 | Unbounded work | Non-finding — max_turns default 1000 enforced (held) |
| REL-014 | Fragile default config | Note — default `transient_only=false` retries 4xx (REL-GSL-004) |
| REL-015 | Missing operator signal | Partial — REL-GSL-002 init failure is unlogged |

---

## Findings

### REL-GSL-001: Google rate-limit `retryDelay` is parsed uncapped, freezing the agent turn

Severity: Medium
Confidence: Confirmed (missing cap); runtime hang simulation-reasoned
Evidence basis: source-evidenced
Domain: Reliability

Evidence:
- `crates/gosling/src/providers/utils.rs:79-102` — `parse_google_retry_delay`
  parses `retryDelay` (`"…s"`) as `u64` seconds → `Duration::from_secs(num)`
  with **no upper bound**.
- `crates/gosling/src/providers/utils.rs:148-153` — feeds it straight into
  `ProviderError::RateLimitExceeded { retry_delay }`.
- `crates/gosling-providers/src/retry.rs:233-251` — the retry loop honors a
  provider-supplied delay verbatim: `RateLimitExceeded { retry_delay: Some(d) } => *d`
  then `sleep(delay).await`. No clamp.
- Contrast: the generic/OpenRouter/RFC-7231 path caps at `MAX_RETRY_AFTER_SECS`
  = 3600s and rejects NaN/inf/absurd values (`http_status.rs:37,64-70,383-393`).
  The Google path bypasses that clamp entirely.

Observed behavior:
- A Google/Gemini-compatible endpoint returning `429` with
  `{"error":{"details":[{"@type":".../RetryInfo","retryDelay":"999999999s"}]}}`
  produces a multi-year `Duration`; the retry loop calls `sleep()` on it, hanging
  the current agent turn with no user-visible progress (the `sleep` is outside any
  request timeout).

Expected boundary:
- Provider-supplied retry delays are attacker-influenceable (proxy, compromised
  or buggy endpoint) and must be clamped to the same `MAX_RETRY_AFTER_SECS`
  bound the other 429 path already enforces.

Failure mechanism:
- The 3600s clamp lives only in `http_status::duration_from_finite_secs`; the
  Google-specific parser predates/parallels it and never adopted the cap.

Break-it angle:
- Hostile or misconfigured Gemini-compatible base URL (declarative provider /
  self-hosted proxy) sets a huge `retryDelay`; the turn stalls silently.

Impact:
- Silent, effectively-unbounded stall of an agent turn on the hot path; no
  operator signal during the sleep. Bounded per attempt but each of up to
  `max_retries` (3) retries can re-stall.

Operational impact:
- Blast radius: Workflow. Side-effect class: none (time). Reversibility:
  reversible (cancel). Operator visibility: silent during sleep. Rerun safety: safe.

Adjacent failure modes:
- REL-GSL-004 (4xx retried by default) shares the "provider response drives
  retry behavior without a hard bound" weakness.

Recommended mitigation:
- Route `parse_google_retry_delay` through `duration_from_finite_secs` (or apply
  `min(MAX_RETRY_AFTER_SECS)`), matching the existing cap.
- Behavior test: a `RetryInfo` with `retryDelay:"1000000s"` yields a delay
  clamped to 3600s.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: one module + one test.
  Nominal agent: codex.

Validation:
- Unit test asserting the clamp; assert parity with `retry_after_clamps_absurd_body_seconds`.

Non-goals:
- Do not change legitimate short-delay honoring or the OpenRouter path.

---

### REL-GSL-002: Prompt/command classifier init failure is swallowed unlogged; ML scanning silently degrades

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Reliability (escalate: Security)

Evidence:
- `crates/gosling/src/security/scanner.rs:50-55` — `with_ml_detection`:
  `command_classifier = Self::create_classifier(Command).ok();`
  `prompt_classifier = Self::create_classifier(Prompt).ok();` — each `create_classifier`
  `Result` error is discarded by `.ok()` with **no log**; the function only
  `bail!`s when *both* are `None`.
- `crates/gosling/src/security/scanner.rs:227-234` — `scan_conversation` with a
  `None` prompt classifier returns `confidence: 0.0` (treated clean), no pattern
  fallback.

Observed behavior:
- Operator enables `SECURITY_PROMPT_CLASSIFIER_ENABLED` but the prompt
  classifier fails to initialize (bad `SECURITY_PROMPT_CLASSIFIER_MODEL`/
  `_ENDPOINT`, unreachable host). Command classifier still initializes, so the
  `both None` bail does not trigger. The scanner comes up "successfully" with
  conversation prompt-injection scanning permanently off and no error emitted.

Expected boundary:
- A safety control that fails to initialize a component the operator explicitly
  enabled must surface that failure (log/error), not silently run degraded while
  reporting success.

Failure mechanism:
- `.ok()` collapses the descriptive `Err` from `create_classifier` (which
  distinguishes not-enabled vs missing-config vs client-build failure) into a
  bare `None`; the only survivor check is the both-None `bail!`.

Break-it angle:
- Point `SECURITY_PROMPT_CLASSIFIER_ENDPOINT` at a dead host; ML detection reports
  enabled, conversation scans return 0.0 forever.

Impact:
- Prompt-injection ML defense silently absent under partial misconfiguration;
  operator believes it is active (matches the `SECURITY.md` posture concern in
  orientation §6).

Operational impact:
- Blast radius: Service (security posture). Side-effect class: none (control gap).
  Reversibility: reversible. Operator visibility: **silent** (init path unlogged).
  Rerun safety: safe.

Adjacent failure modes:
- REL-GSL-003 (runtime fail-open of the same scan).

Recommended mitigation:
- When a classifier is `enabled` but init fails, `tracing::error!`/`warn!` with
  the reason before falling back to `None` (distinguish "not enabled" — expected,
  silent — from "enabled but failed" — must surface).
- Test: enabled + unresolvable endpoint logs an error and the resulting scanner
  reports the prompt classifier as unavailable.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: scanner module + tests.
  Nominal agent: claude (security-context judgement on enabled-vs-failed).

Validation:
- Assert a surfaced signal (not just absence of panic) when an enabled classifier
  fails to init.

Non-goals:
- Do not change the not-enabled (opt-in default false) silent path.

---

### REL-GSL-003: Prompt classifier runtime error fails open to "clean" (log-only)

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Reliability (escalate: Security)

Evidence:
- `crates/gosling/src/security/scanner.rs:287-305` — `scan_with_classifier`
  maps any `classify` error to `None` (warn-logged at :301).
- `crates/gosling/src/security/scanner.rs:245-253` — `scan_conversation` folds
  per-message results with `result.unwrap_or(0.0)`, so a `None` (network/HTTP
  error to the classifier endpoint) contributes 0.0 (clean) to the max. Unlike
  the command path (`analyze_text:200-221`), there is **no pattern-matcher
  fallback** for the conversation prompt scan.

Observed behavior:
- Transient failure of the classification endpoint makes conversation
  prompt-injection scanning return "clean" for the affected messages; the tool
  call proceeds as if scanned safe.

Expected boundary:
- A scan that could not run should be distinguishable from a scan that ran and
  found nothing; safety-relevant fail-open should at minimum degrade to the
  pattern matcher, not silently to 0.0.

Failure mechanism:
- `unwrap_or(0.0)` conflates "no signal" with "clean signal."

Break-it angle:
- Kill the classifier endpoint mid-session; injection content in conversation
  passes the ML gate.

Impact:
- Time-boxed fail-open of one safety control; warn-logged so operator can detect
  post-hoc. Command scanning and pattern matching still cover the tool content.

Operational impact:
- Blast radius: Workflow. Side-effect class: none. Reversibility: reversible.
  Operator visibility: log-only. Rerun safety: safe.

Recommended mitigation:
- On classifier error in `scan_conversation`, fall back to
  `pattern_based_scanning` (as the command path does) rather than 0.0, or mark
  the result `scanned:false` so the caller can choose a stricter posture.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal agent: claude.

Validation:
- Test: classifier error yields pattern-fallback confidence, not 0.0.

Non-goals:
- Do not block the turn on classifier outage (availability tradeoff is a policy
  decision).

---

### REL-GSL-004: Default retry config retries deterministic 4xx client errors

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Reliability

Evidence:
- `crates/gosling-providers/src/retry.rs:99-108` — `should_retry`:
  `RequestFailed(_) => !config.transient_only`.
- `crates/gosling-providers/src/retry.rs:28-37` — `Default` sets
  `transient_only: false`. A deterministic 400 (e.g. "model not found",
  malformed request) is therefore retried `max_retries` (3) times with backoff
  (~1+2+4s) before surfacing.

Observed behavior:
- Non-transient 4xx (except the two hardcoded permanent-thinking markers,
  :88-97) are retried, wasting ~7s and 4× request volume against an error that
  cannot succeed on replay.

Expected boundary:
- Deterministic client errors should not be retried; retry should default to
  transient classes.

Failure mechanism:
- The default keeps `transient_only=false` for backward compatibility; only
  call sites that opt into `.transient_only()` avoid it.

Impact:
- Latency and redundant load, not a storm (bounded by `max_retries`). Amplifies
  under many parallel subagents hitting the same deterministic 4xx.

Operational impact:
- Blast radius: Workflow. Side-effect class: network. Reversibility: reversible.
  Operator visibility: log (warn per retry). Rerun safety: safe.

Adjacent failure modes: REL-GSL-001.

Recommended mitigation:
- Flip the default to `transient_only`, or expand the permanent-4xx marker set;
  keep explicit opt-in for the rare retry-4xx cases.

Implementation assessment:
- Complexity: governance_decision (behavior change). Cost: S. Nominal agent:
  human-owner (default-behavior policy) then codex.

Validation:
- Test: default config does not retry a generic 400.

Non-goals:
- Do not remove transient (5xx/network/429) retries.

---

### REL-GSL-005: `assert!`/`expect` on the MCP client hot path can panic a tool call

Severity: Low
Confidence: Plausible
Evidence basis: source-evidenced
Domain: Reliability (escalate: Concurrency)

Evidence:
- `crates/gosling/src/agents/mcp_client.rs:211-215` — `set_session_id` `assert!`s
  that a `GoslingClient` is only ever used by one session ("McpClient received
  requests from different sessions") — a panic, called on every
  `send_request_with_context` (:655).
- `crates/gosling/src/agents/mcp_client.rs:164,253,276` —
  `.expect("active_tool_calls mutex poisoned")` on the `StdMutex` guarding
  active tool calls; a prior panic while holding the lock turns subsequent tool
  calls into panics.

Observed behavior:
- If the per-session client invariant is ever violated (shared client across
  sessions), or the std mutex is poisoned by an earlier panic, the next MCP
  request panics rather than returning an `Err`.

Expected boundary:
- Hot-path request handling should degrade to a returned error, not abort.

Failure mechanism:
- Invariant expressed as `assert!` (crashes) rather than a typed error; poisoned
  `StdMutex` surfaced with `.expect`.

Break-it angle:
- A refactor that reuses one client across sessions, or any panic inside a code
  block holding `active_tool_calls`, converts into a cascading panic on the next
  call.

Impact:
- Turn/process crash instead of a recoverable error. Low reachability today
  (clients are constructed per extension/session), hence Plausible.

Operational impact:
- Blast radius: Workflow (turn). Side-effect class: process. Reversibility:
  reversible (restart). Operator visibility: log (panic). Rerun safety: safe.

Recommended mitigation:
- Convert the session-mismatch `assert!` to a returned `ServiceError`; consider
  recovering poisoned locks with `into_inner()` on poison rather than `.expect`.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal agent: codex.

Validation:
- Test: a cross-session call returns `Err`, not a panic.

Non-goals:
- Do not relax the single-session invariant itself.

---

### REL-GSL-006: `ApiResponse::from_response` discards body-parse errors (partial-success seam)

Severity: Low
Confidence: Plausible
Evidence basis: source-evidenced
Domain: Reliability (escalate: Input/Output-Path)

Evidence:
- `crates/gosling-providers/src/api_client.rs:220-226` — `from_response` builds
  `ApiResponse { status, payload }` with `payload = response.json().await.ok()`:
  a non-JSON / truncated body silently becomes `payload: None` while `status`
  stays `200`.

Observed behavior:
- Any caller of `api_post`/`api_get` (the `ApiResponse` variants, e.g. auxiliary
  model-listing/metadata calls) that keys success on `status` alone would treat
  a 200-with-unparseable-body as success with empty payload.

Expected boundary:
- A 200 with a body the caller needs but cannot parse should be a distinct
  failure, not a `None` payload folded into success.

Failure mechanism:
- `.ok()` collapses the JSON decode error.

Notes / scope:
- The primary completion/streaming paths do **not** use this — they go through
  `handle_response` (`http_status.rs:270-276`), which errors on invalid JSON
  (held, see non-findings). Impact is limited to the `ApiResponse` consumers; I
  did not enumerate every caller, hence Plausible.

Recommended mitigation:
- Preserve the decode error (e.g. `payload: Result<Value, _>` or a `text`
  fallback) so a 200-with-bad-body is not indistinguishable from an empty
  success.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal agent: codex.

Validation:
- Test: `api_get` on a 200 with a non-JSON body does not silently yield `None`.

Non-goals:
- Do not touch `handle_response`/streaming paths (already correct).

---

## Non-findings (checked and held)

- **MCP request timeout** — `await_response` (`mcp_client.rs:671-692`) is a
  `tokio::select!` over the response, `sleep(timeout)` (sends a cancel + returns
  `ServiceError::Timeout`), and the cancel token. Bounded and distinct.
- **MCP connection timeout** — every `McpClient::connect*` call routes through
  `resolve_timeout` → `DEFAULT_EXTENSION_TIMEOUT` (`extension_manager.rs:110-116,
  430-432, 696-702, 830-834, 1009-1051`). No unbounded connect.
- **Provider HTTP timeout** — `DEFAULT_PROVIDER_TIMEOUT_SECS = 600` applied to
  the reqwest builder (`api_client.rs:17,254`); bounds the whole request incl.
  streamed body.
- **Classification client timeout** — 5000ms default
  (`classification_client.rs:52,54-55`); scans cannot hang.
- **Retry backoff/jitter/cap** — exponential with `backoff_multiplier`, jitter
  0.8–1.2, `max_interval_ms` 30s, `max_retries` 3, and a `GOSLING_PROVIDER_SKIP_BACKOFF`
  test escape (`retry.rs:65-81,224-252`). Auth-refresh retry is capped at exactly
  1, independent of `max_retries` (`retry.rs:197,205-222`). No storm.
- **Retry-after clamp (generic path)** — `MAX_RETRY_AFTER_SECS` 3600s + NaN/inf/
  negative/absurd handling, well-tested (`http_status.rs:37,64-88,383-393`).
- **Agent-loop error classification** — distinct arms surface user-visible
  messages and break for `ContextLengthExceeded` (with capped 2 compaction
  attempts), `CreditsExhausted`, `Refusal` (terminal), `NetworkError`, and a
  generic catch-all (`agent.rs:2446-2557`). Failures are loud, not swallowed.
- **Agent loop bounded** — `turns_taken > max_turns` (default `DEFAULT_MAX_TURNS`
  = 1000) yields `MAX_TURNS_MESSAGE` and breaks (`agent.rs:63,1916-1967`).
- **Stop-hook infinite-loop guard** — `stop_hook_block_cap` overrides after N
  consecutive blocks (`agent.rs:123,1927-1928`).
- **Doctor is not false-healthy** — `/doctor` actively runs a live completion
  against the configured provider (`doctor.rs:151-167`), classifies errors
  distinctly (`describe_error` :244-271), and falls back to other models/
  providers, persisting the working one. It reports "No working provider found"
  rather than a green light on failure. (Minor: `test_provider` does not assert
  the completion is non-empty — acceptable for a connectivity/auth probe.)
- **Partial output not treated as success** — `handle_response` requires a valid
  JSON body and errors otherwise (`http_status.rs:270-276`); streaming
  mid-stream errors propagate via `from_stream_error` (`anthropic.rs:346-350`).
- **Cleanup on cancel/drop** — `ActiveToolCallGuard::drop` unregisters the active
  tool call, covering cancellation and dropped reply streams
  (`mcp_client.rs:159-174,652-665`).

## Break-it review (per skill checklist)

- Kill dependency + read health → doctor re-tests live, reports failure (held).
- Empty/failed provider output as success → completion path errors on non-JSON
  (held); ApiResponse path is the one seam (REL-GSL-006).
- Force timeout/network vs auth → classified distinctly at both `http_status`
  and agent-loop layers (held).
- Unbounded input/work → turn loop and compaction are capped (held).
- Repeated failures → backoff/jitter/cap present; the one unbounded delay is the
  Google `retryDelay` (REL-GSL-001).
- Error swallowed → classifier init `.ok()` (REL-GSL-002) and conversation
  fail-open (REL-GSL-003) are the swallow/degrade cases; agent loop does not
  swallow.

## Validation Limits (not reviewed)

- `gosling-server` request/health paths; session persistence & crash-recovery
  atomicity (REL-008 unassessed); OAuth token-refresh reliability under
  concurrent turns.
- Per-provider streaming error handling beyond Anthropic (ollama,
  openai_compatible, bedrock, gcpvertexai, databricks, codex/ACP bridges) —
  only the Anthropic stream and the shared `http_status`/`retry` layers traced.
- `context_mgmt` truncation/summarization correctness; ACP subprocess lifecycle;
  desktop/TUI reliability surfaces.
- No build/run in this pass: all runtime manifestations (turn hang under
  REL-GSL-001, live fail-open under REL-GSL-003) are `simulation-reasoned`, not
  reproduced. Upgrading them to Confirmed-runtime needs a harness with a stub
  Gemini endpoint / a downed classifier endpoint.
- REL-GSL-006 caller enumeration incomplete (Plausible).

## Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| REL-GSL-001 | Reliability | Negative-Space / Temporal | Hostile provider drives an uncapped delay |
| REL-GSL-002 | Reliability | Security | Safety control silently degrades; operator-signal gap |
| REL-GSL-003 | Reliability | Security | Fail-open of prompt-injection ML gate |
| REL-GSL-004 | Reliability | — | Retry policy default |
| REL-GSL-005 | Reliability | Concurrency | Panic under cross-session / poisoned lock |
| REL-GSL-006 | Reliability | Input/Output-Path | Body-parse error folded into success |
