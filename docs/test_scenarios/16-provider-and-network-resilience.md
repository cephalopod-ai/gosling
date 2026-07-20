# 16 — Provider and Network Resilience

Use a controllable provider fixture whenever possible so status codes, delays,
stream chunks, and usage metadata are deterministic. Real-provider variants
must use capped test credentials and record external request IDs.

---

### PN-01 — Rate-limit response and retry recovery
- Goal: HTTP 429 or provider-equivalent throttling is bounded and recoverable.
- Category: recovery / boundary
- Preconditions: fixture returns 429 with known retry metadata twice, then success; unique session marker.
- Steps: send one turn; observe automatic retry timing; cancel during backoff once; repeat to success; send a follow-up.
- Expected: UI names rate limiting rather than auth/network failure; retries respect provider hints and a finite cap; cancel stops retries within 10 seconds; exactly one assistant turn is committed on success; follow-up works.
- Observe: retry attempt count and duplicated provider billing/request IDs.

### PN-02 — Network disconnect during streaming
- Goal: a dropped stream cannot leave partial output masquerading as complete.
- Category: interruption / recovery
- Preconditions: fixture streams known chunks, drops connection before terminal event, then accepts retry.
- Steps: send marker prompt; drop after chunk 3; inspect status/history/export; retry or send a new turn; relaunch.
- Expected: interrupted response is visibly incomplete/failed; no fabricated terminal event; retry does not duplicate committed chunks into two assistant turns; session remains usable and relaunch preserves honest state.
- Observe: whether tools requested before the drop ran once or twice.

### PN-03 — Slow provider timeout and cancellation
- Goal: connection and response stalls have separate, controllable outcomes.
- Category: boundary / interruption
- Preconditions: fixture modes for connect delay, headers-with-no-body, and stream stall after one chunk; known configured timeout if exposed.
- Steps: exercise each stall; cancel one manually; allow one to hit timeout; restore fast mode and retry in the same session.
- Expected: local cancel meets 10-second deadline; configured timeout is enforced within tolerance; error distinguishes connection from mid-stream stall; no background retries/output after terminal state; restored provider succeeds.
- Observe: Desktop spinner and CLI terminal restoration.

### PN-04 — Context-window exhaustion and compaction
- Goal: an oversized history is compacted or rejected without losing the session.
- Category: boundary / persistence
- Preconditions: fixture with a small declared context limit; session with ordered marker turns near threshold; auto-compact settings recorded.
- Steps: send a turn below limit, one crossing limit, and one after recovery; repeat with auto-compact disabled/enabled; export before and after; relaunch.
- Expected: below-limit succeeds; crossing result is a named context error or explicit compaction; markers retained/removed match the documented compaction policy; no retry loop; post-recovery turn and export work.
- Observe: token estimates versus provider-reported usage.

### PN-05 — Empty and malformed provider responses
- Goal: invalid upstream payloads fail as provider errors, not successful blank turns.
- Category: invalid input / recovery
- Preconditions: fixture modes for empty body, invalid JSON, unknown event, invalid UTF-8/chunk, missing finish event, and valid response.
- Steps: invoke each mode in a separate fresh session; inspect exit/status/history; switch fixture to valid and retry.
- Expected: malformed cases terminate within deadline with bounded sanitized errors; no blank successful assistant turn; raw body is not dumped if secret-like; valid retry works without deleting config.
- Observe: JSON/stream-json output remains syntactically valid on failure.

### PN-06 — Provider/model override precedence
- Goal: CLI flags, environment, config, workspace profile, and session selection resolve predictably.
- Category: settings / boundary
- Preconditions: fixture exposes requested provider/model; distinct valid values configured at each applicable layer.
- Steps: run baseline config; add env overrides; add `run --provider/--model`; use Desktop workspace profile; resume a session after default changes; remove layers one at a time.
- Expected: captured requests follow documented precedence; UI labels match effective values; run flags do not mutate defaults; invalid highest-priority value errors instead of silently using a lower layer unless fallback is explicit.
- Observe: planner versus main model layers separately.

### PN-07 — Model-list failure and custom model ID
- Goal: inability to enumerate models does not prevent a valid explicit model when supported.
- Category: recovery / settings
- Preconditions: provider fixture whose list-models endpoint fails but chat accepts custom ID `fixture/custom-v1`.
- Steps: open configure/Desktop model picker; enter/select custom ID if allowed; send a turn; then use an invalid ID; restore model listing.
- Expected: list failure is visible and bounded; custom model route is discoverable where supported; request uses exact custom ID; invalid ID is named; restored listing does not overwrite saved custom selection silently.
- Observe: stale cached model list age and refresh control.

### PN-08 — Local provider stops and restarts
- Goal: loss of a local engine such as Ollama is diagnosed and recoverable.
- Category: recovery / lifecycle
- Preconditions: disposable local provider/model; process control; one successful baseline turn.
- Steps: stop provider before a turn; restart and retry; stop during streaming; run `info --check` and `doctor` in each state.
- Expected: stopped state is connection/unavailable, not bad credentials; checks exit consistently with reality; restart succeeds without reconfiguration; mid-stream stop leaves honest failed state and no orphan client process.
- Observe: model download/missing-model error remains distinct from daemon unavailable.

### PN-09 — OAuth expiry, refresh, and user cancellation
- Goal: OAuth-backed providers handle expired credentials and cancelled login without corrupting prior state.
- Category: recovery / authorization
- Preconditions: sandbox OAuth provider or fixture; tokens with known expiry/refresh outcomes; no production account.
- Steps: use valid token; expire access token with valid refresh; force refresh rejection; start re-auth and cancel; complete re-auth; relaunch.
- Expected: refresh succeeds without exposing tokens; rejection requests re-auth once; cancellation returns to usable settings and does not spin browser windows; completed auth persists; unrelated providers remain configured.
- Observe: callback port cleanup and browser error pages.

### PN-10 — Usage, cost, and statistics consistency
- Goal: displayed and machine-readable usage derives from provider metadata consistently.
- Category: persistence / boundary
- Preconditions: fixture returns exact input/output/cache token counts and price metadata for success, retry, tool call, and failed turn.
- Steps: run with `--stats`, text, JSON, and stream-json; inspect Desktop usage/cost; resume session and export; include a retried request.
- Expected: all surfaces agree within documented rounding; failed/retried requests are counted according to stated policy without double counting; units/currency are labeled; missing metadata shows unknown rather than zero.
- Observe: planner/subagent usage attribution.
