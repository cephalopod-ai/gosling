# 20 — Deep Research Regressions

These cards replay failures observed during the 2026-08-27 Deep Research
campaign. Use a disposable `GOSLING_PATH_ROOT`, deterministic providers, and
ACP/tool-call capture unless a card explicitly requires an installed-app UI
check. Preserve the exact payloads and visible status text as evidence.

---

### DR-03 — ACP prompt persistence survives queued writers
- Goal: submit a Deep Research prompt while session metadata writers are queued without exhausting the SQLite connection pool.
- Category: concurrency / persistence / recovery
- Preconditions: disposable root; deterministic provider; test hook or fixture that holds one immediate-write transaction and queues at least twenty additional writers; ACP prompt-state telemetry.
- Steps:
  1. Hold one immediate-write transaction open.
  2. Queue twenty session metadata writes through the normal storage API.
  3. Submit a new ACP Deep Research prompt while the writers are queued.
  4. Release the held transaction and wait for a terminal prompt state.
- Expected: the ACP prompt state acquires a connection within its deadline; no `pool timed out while waiting for an open connection` error appears; the prompt and queued writes persist exactly once; the run reaches a terminal success, failure, or cancelled state with diagnostics.
- Observe: connection-pool occupancy, write ordering, duplicate messages, missing prompt state, spinner without a terminal state, and WAL/SHM residue after shutdown.
- Variations: cancel the prompt before releasing the writer; repeat with forty queued writes while retaining the same bounded deadline.

### DR-04 — Solo external ACP provider selects an approval-capable mode
- Goal: run Solo research with an ACP provider whose tools execute outside Gosling inspection without submitting the session in Auto mode.
- Category: happy path / permissions / recovery
- Preconditions: disposable root; scripted ACP provider marked as externally executing tools; provider returns the exact token `ACP_SOLO_OK`; protocol capture for advertised and selected modes.
- Steps:
  1. Open New Research, select Solo, and choose the external-tool ACP provider.
  2. Submit `Reply with exactly ACP_SOLO_OK and do not call tools.`
  3. Record the mode shown in the composer and the mode sent through ACP.
  4. Wait for the terminal response.
- Expected: Auto is not offered or selected for the provider; Gosling uses an approval-capable mode; the provider returns `ACP_SOLO_OK`; no error says the provider cannot run in Auto mode.
- Observe: a stale Autonomous label, UI/protocol disagreement, silent mode substitution after submission, or repeated retries.
- Variations: switch from a hosted provider in Auto to the external provider before sending; attempt an explicit Auto mode request and verify it fails closed.

### DR-05 — Legacy `source: "dummy"` becomes an ad-hoc delegate only
- Goal: tolerate the exact legacy Deep Research payload emitted by a lead model without weakening named-source validation.
- Category: invalid input / recovery / concurrency
- Preconditions: disposable root; deterministic lead and delegate providers; Summon protocol capture; no registered source named `dummy`.
- Steps:
  1. Submit a delegate call containing `instructions`, explicit `provider`, explicit `model`, `async: true`, and `source: "dummy"`.
  2. Observe normalization, selected provider/model, delegate launch count, and returned activity text.
  3. In a fresh fixture, submit a source-only call with `source: "dummy"` and no explicit provider/model.
- Expected: the complete ad-hoc payload normalizes by omitting only the sentinel source and launches exactly one delegate using the explicit provider/model; the source-only payload remains a named-source request and returns one clear not-found error; no request is silently routed to a different provider.
- Observe: `Source 'dummy' not found` on the ad-hoc payload, broad conversion of genuine named sources, duplicate launches, or retry churn.
- Variations: use `source: "missing-researcher"` with explicit provider/model and verify it remains a named-source error; use `source: "Dummy"` and verify sentinel matching is exact.

### DR-06 — External-provider delegate runs bounded and tool-disabled
- Goal: include an external-tool ACP provider as a Deep Research peer without bypassing inspection or failing because subagents require Auto.
- Category: permissions / degraded path / recovery
- Preconditions: disposable root; deterministic lead; one hosted delegate and one external-tool ACP delegate; ACP mode and tool-advertisement capture.
- Steps:
  1. Start Dual or Trio research whose roster includes the external-tool provider.
  2. Let the lead launch that seat with explicit provider/model and bounded instructions.
  3. Record the delegate mode, advertised tools, launch result, and lead-visible activity.
  4. Wait for the lead to incorporate the peer result or declare a bounded degraded outcome.
- Expected: the external delegate runs in Chat mode with no delegated tool definitions; the launch/result text discloses the bounded tool-disabled mode; hosted delegates remain in Auto; the lead reaches a terminal result without weakening the external-provider Auto guard.
- Observe: approval bypass, hidden mode downgrade, external tool execution, blanket success when a seat failed, or an orphan subagent.
- Variations: make the external delegate return an error and verify the lead names only that seat as degraded; use a hosted-only roster and verify all seats remain Auto.

### DR-07 — Initial Inputs contain long and multiline content
- Goal: paste and queue long research material without horizontally expanding or corrupting the Initial Inputs dialog.
- Category: boundary / files / accessibility
- Preconditions: installed or development Desktop build at a normal window size; Initial Inputs dialog open; browser/UI geometry capture.
- Steps:
  1. Paste a multiline Markdown input containing headings, paragraphs, Unicode, and an unbroken token of at least 8,000 characters.
  2. Queue the pasted input and inspect both the textarea and queued preview.
  3. Resize the window narrower, scroll the queued preview vertically, and close/reopen the dialog.
- Expected: textarea and preview use soft wrapping; every card remains within the dialog's client width; no horizontal dialog scrollbar appears; long content remains readable through vertical scrolling; controls remain visible and operable.
- Observe: `scrollWidth > clientWidth`, clipped remove/browse buttons, single-line truncation that hides the content, overlapping cards, or lost pasted text.
- Variations: paste a single URL-like token; paste Markdown tables and fenced code; queue two long inputs.

### DR-08 — Invalid extension parameters fail once with diagnosable feedback
- Goal: prevent a repeated tool-call loop when a research extension rejects a malformed parameter object.
- Category: invalid input / error recovery / interrupted workflow
- Preconditions: disposable root; deterministic lead; scripted research extension whose Markdown operation rejects the observed argument set `selector`, `backendNodeId`, `maxBytes`, `url`, and `timeout` with JSON-RPC `-32602 InvalidParams`; tool-call count capture.
- Steps:
  1. Start research that causes the scripted lead to issue the exact malformed Markdown call.
  2. Record the first extension error returned to the lead and rendered in Activity.
  3. Continue until the lead chooses a different valid method or reaches a terminal degraded result.
- Expected: the invalid call produces one actionable error that identifies the parameter contract; the lead does not repeat the identical rejected payload; Activity distinguishes the failed call from successful calls; the run terminates or recovers within the normal deadline.
- Observe: six or more identical failed cards, generic labels that expose only parameter names, hidden failures under a success summary, or unbounded retry/token growth.
- Variations: omit one required field; include one unknown field; make a corrected second call and verify it succeeds exactly once.
