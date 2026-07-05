# Audit — Dependency Criticality (Single Points of Failure & Absent-Dependency Behavior)

Lens: `audit-dependency-criticality` (DEP inventory + NASA SPF / NIST resilience addendum).
Scope authority: **audit-only / read-only** per `docs/cloud/00-orientation.md`. Only this
file was written.

Headline question for this lens: *what does one missing or failing dependency take down,
and is there a second way through or an honest refusal?*

Bottom line: gosling handles the **hard, obvious** absences well — the bundled server
binary, the OS keyring, MCP/CLI subprocesses, `git`, the bind port, and an unset provider
all fail **visibly** at startup or first use, several with bounded timeouts and named
paths. The two material findings are in the **security-control dependency chain**: the ML
prompt-injection classifier is a *network* dependency whose absence degrades **silently**
(only a `warn!` log), and the tool-inspector manager **fails open** — any inspector error
is swallowed and the tool proceeds. The single-LLM-provider-per-session runtime outage is
an intentional, owned SPF recorded in the Residual Risk Register.

---

## Dependency Criticality Register

Derived from the per-dependency absence walks below — not estimated. Absence behavior uses
the playbook vocabulary (`refuse-clear` / `degrade-honest` / `degrade-silent` / `crash` /
`hang` / `corrupt`).

| Dependency | Class | Consumers | Absence behavior | Detected? | Detection time | Is SPF | Alternate / fallback | Safe state | Finding |
|---|---|---|---|---|---|---|---|---|---|
| Bundled `gosling` server binary | DEP-010 | Desktop app (all功能) | `refuse-clear` (names all searched paths) | yes (fs check) | startup | no | binary-not-found error + 30s bounded readiness | fail_visible | non-finding |
| `gosling serve` local HTTP/WS port | DEP-005 | Desktop ⇄ server ACP/HTTP | refuse-clear (desktop auto-assigns port 0; CLI `bind` errors) | yes | startup | no | OS-assigned ephemeral port | fail_visible | non-finding |
| OS keyring (Linux Secret Service / macOS Keychain) | DEP-003/004 | secret storage (API keys, tokens) | `degrade-honest` → file `secrets.yaml` (0600) | yes (error-string match) | first-use | no | file storage fallback | fail_degraded | non-finding (visibility note) |
| ML prompt-injection classifier endpoint | DEP-008/009 | `SecurityManager` scanner (opt-in ML) | **`degrade-silent`** (command→patterns; conversation→0.0) | log-only (`warn!`) | never | **partial (ML portion)** | pattern fallback (command path only) | fail_degraded | **DEP-GSL-001** |
| Tool inspectors (security + permission) as a set | DEP-004 | agent tool-call gating | **fail-open** on inspector `Err` (results dropped, tool proceeds) | log-only (`error!`) | mid-operation | **yes** | none for the failing inspector | fail_closed | **DEP-GSL-002** |
| Selected LLM provider (runtime reachability) | DEP-008 | every generation turn | request error propagates (no outage failover) | yes (exception) | mid-operation | **yes (intentional)** | none for outage; config-mismatch fallback only | fail_visible | RR-001 |
| MCP stdio command (`uvx`/`npx`/`docker`/custom) | DEP-010 | extension start | refuse (spawn error + captured stderr) | yes | first-use | no | user picks another extension | fail_visible | non-finding |
| Provider CLI (`claude`/`codex`/`gemini`/`cursor-agent`) | DEP-010 | ACP subprocess providers | refuse (spawn error naming command) | yes | first-use | no | user picks API provider | fail_visible | non-finding |
| `git` | DEP-010 | plugin install / `/review` | refuse (`bail!` with stderr) | yes | first-use | no | none (feature unavailable) | fail_visible | non-finding |
| `bash` | DEP-010 | developer shell tool | `degrade-honest` → `sh` | yes (`which`) | first-use | no | `sh` (or `GOSLING_SHELL`) | fail_degraded | non-finding |
| Provider set (`GOSLING_PROVIDER`/model) | DEP-002 | agent boot | refuse-clear (`"Provider not set"`) | yes | first-use | no | n/a | fail_closed | non-finding |
| Secrets/config file (`config.yaml`, `secrets.yaml`) | DEP-001 | config reads | `degrade-honest` (missing → empty map / `NotFound`) | yes | first-use | no | env-var overrides | fail_visible | non-finding |

---

## Findings

| ID | Title | Severity | Confidence |
|----|-------|----------|------------|
| DEP-GSL-001 | ML prompt-injection classifier network outage degrades the security scan silently | Medium | Confirmed (source) |
| DEP-GSL-002 | Tool-inspector manager fails open: any inspector error drops its verdict and the tool proceeds | Medium (High if the failing inspector is the permission/security gate) | Confirmed (source) |

---

### DEP-GSL-001: ML prompt-injection classifier network outage degrades the security scan silently

Severity: Medium
Confidence: Confirmed (deterministic error-to-None substitution path)
Evidence basis: source-evidenced
Domain: Failsafe (DEP-008 / DEP-009)

Evidence:
- `crates/gosling/src/security/scanner.rs:298-304` — `scan_with_classifier`: on any
  classifier error (network unreachable, connect/read timeout, 5xx, 4xx) the result is
  swallowed to `None` with only `tracing::warn!("{} classifier scan failed: {:#}", ...)`.
- `crates/gosling/src/security/scanner.rs:200-222` — `analyze_text` (command path): a
  `None` from the classifier falls back to `pattern_based_scanning` (degraded but real).
- `crates/gosling/src/security/scanner.rs:245-262` — `scan_conversation`
  (prompt-injection over user messages): `result.unwrap_or(0.0)` (line 252) means a failed
  classifier contributes **confidence 0.0** with **no pattern fallback** on this path.
- `crates/gosling/src/security/classification_client.rs:52` — client timeout default 5000ms;
  `:162` request send `?`, `:176-179` parse `?` — all errors flow back up as the `Err`
  swallowed above.
- `crates/gosling/src/security/mod.rs:57-71` — ML scanning is opt-in
  (`is_ml_scanning_enabled` defaults `false`), so this affects users who *believe they
  enabled* ML detection.

Observed behavior:
- With the classifier endpoint absent/unreachable, the command-injection scan silently
  reverts to pattern-only, and the conversation/prompt-injection ML signal silently drops
  to 0.0. The only trace is a `warn!` line; no operator-visible status says "ML security
  scanning is currently degraded."

Expected boundary:
- `fail_degraded` with **honest** status: when a security control's remote dependency is
  down, the operator (and ideally an event stream) should be told the control is running in
  reduced mode, not left to infer it from a log line.

Failure mechanism:
- Error→`None`→`unwrap_or(0.0)` collapses "I could not evaluate this" into "this is safe"
  on the conversation path, and "I could not evaluate this" into "patterns only" on the
  command path, with no signal surfaced above the log.

Break-it angle:
- Point `classification` at a closed localhost port or a TEST-NET address (RFC 5737
  `192.0.2.1`) in a sandbox and issue a shell tool call whose maliciousness only the ML
  model catches (patterns miss it): the call is scored 0.0/pattern-only and is not flagged.

Impact:
- The claimed ML safety uplift silently disappears during a provider/network outage.
  Practical blast radius is bounded: the command path keeps patterns, and context
  confidence is weighted only 0.2 in `combine_confidences` (mod-adjacent
  `scanner.rs:264-284`) — but the operator is not told detection degraded.

Operational impact:
- Blast radius: Workflow. Side-effect class: none (detection quality). Reversibility:
  reversible. Operator visibility: log-only. Rerun safety: safe.

Adjacent failure modes:
- DEP-GSL-002 (a *propagated* classifier error would be dropped entirely, not just
  degraded).

Recommended mitigation:
- Remediation patterns: `dependency_health_probe`, `degraded_mode_contract`,
  `fail_closed_refusal` (configurable).
- Minimal repair: distinguish "classifier unavailable" from "classified safe"; emit a
  security-degraded event/status once per session and let policy choose degrade-honest vs
  fail-closed (require-approval) for high-risk tools while ML is down.
- Behavior test: classifier pointed at a closed port → a pattern-invisible malicious
  command is either flagged or the run surfaces an explicit "security scanning degraded"
  signal (assert the signal, not just the log).

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: tests, one status/event field.
  Nominal agent: codex.
- Rationale: contained to the scanner + one status surface; deterministic test.

Resilience mapping:
- Phase: withstand. Objective(s): continue, understand. Safe state: fail_degraded.

Failure analysis (FMECA row):
- Failure mode: classifier endpoint unreachable / times out. Likely cause: network egress
  blocked, endpoint outage, expired auth token. Operational phase: tool-call inspection.
- Local effect: `None`/0.0 confidence. Workflow effect: reduced/zeroed ML detection.
  System/operator effect: operator unaware the control degraded.
- Detection method: `warn!` log only. Detection latency: post-hoc. Operator visible: no.
- Compensating provision: pattern fallback on the command path only.

Single point of failure:
- is_spf: partial (SPF for the ML portion of the control; command path retains patterns).
  missing_alternate: true (conversation path). redundancy_or_fallback: pattern matcher
  (command path only). required_owner_decision: whether ML-down should fail-closed for
  high-risk tools (policy).

Criticality:
- Likelihood: plausible (egress restriction / outage / token expiry). Detectability: silent.

Validation:
- Test asserts an explicit degraded signal or a fail-closed refusal when the classifier is
  unreachable — not merely that no exception escaped.

Non-goals:
- Do not add a second classifier provider in this slice.

---

### DEP-GSL-002: Tool-inspector manager fails open — an inspector error drops its verdict and the tool proceeds

Severity: Medium (High if the failing inspector is the permission/security gate for a
destructive tool)
Confidence: Confirmed (traced control path)
Evidence basis: source-evidenced
Domain: Failsafe (DEP-004)

Evidence:
- `crates/gosling/src/tool_inspection.rs:95-116` — for each inspector, `Ok(results)`
  extends the verdict set; `Err(e)` logs `tracing::error!(... "Tool inspector failed")` and
  the comment `// Continue with other inspectors even if one fails`. The manager then
  returns `Ok(all_results)` (`:118`) with the failed inspector contributing **nothing**.
- `crates/gosling/src/security/security_inspector.rs:59-82` — `SecurityInspector::inspect`
  returns `analyze_tool_requests(...).await?`; a propagated error (e.g. a non-classifier
  failure from `scanner.rs:153`) becomes the dropped `Err` above, so any
  `RequireApproval` this inspector would have raised is lost and the tool is not gated.
- The `PermissionInspector` runs through the same loop, so the same drop-on-error semantics
  apply to permission gating.

Observed behavior:
- If an inspector returns `Err` mid-operation, its findings (including
  `RequireApproval`/block verdicts) are discarded and execution continues as if that
  inspector had approved.

Expected boundary:
- `fail_closed` for a safety inspector: an inspector that cannot render a verdict should
  cause the tool call to require approval / be refused, not silently pass. The current
  behavior is an availability-over-safety choice made implicitly in a shared loop.

Failure mechanism:
- One `Err` arm treats "inspector could not evaluate" as "inspector had nothing to say,"
  conflating an error with an allow.

Break-it angle:
- Force the SecurityInspector (or PermissionInspector) to error (e.g. a config/serialize
  failure, or a propagated scanner error) while a tool call that it would have gated is in
  flight: the call proceeds unblocked with only an `error!` log.

Impact:
- A dependency or internal failure inside a *safety* inspector can silently remove the
  gate in front of a tool call. Severity depends on which inspector and which tool.

Operational impact:
- Blast radius: Workflow (potentially Service — a shell/file/network tool). Side-effect
  class: process/file/network (the tool that runs). Reversibility: depends on tool.
  Operator visibility: log-only. Rerun safety: unknown.

Adjacent failure modes:
- DEP-GSL-001 (the classifier outage that most plausibly triggers an inspector error path).

Recommended mitigation:
- Remediation patterns: `fail_closed_refusal`, `degraded_mode_contract`.
- Minimal repair: classify inspectors as fail-open vs fail-closed; for a fail-closed
  (safety) inspector, convert `Err` into a synthetic `RequireApproval`/block verdict rather
  than dropping it, and surface the degradation.
- Behavior test: a safety inspector that returns `Err` must cause its tool request to be
  blocked or require approval, and the operator signal must say why.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Cost drivers: tests, one policy branch.
  Nominal agent: codex (policy owner decides fail-open vs fail-closed default → human-owner
  sign-off).
- Rationale: single loop, but the default-safe-state choice is a policy decision.

Resilience mapping:
- Phase: withstand. Objective(s): prevent_avoid, constrain, understand. Safe state:
  fail_closed.

Failure analysis (FMECA row):
- Failure mode: safety inspector returns `Err`. Likely cause: dependency outage, serialize
  error, internal panic-to-error. Operational phase: pre-tool-execution gating.
- Local effect: inspector verdict dropped. Workflow effect: tool runs ungated.
  System/operator effect: silent loss of a control.
- Detection method: `error!` log. Detection latency: post-hoc. Operator visible: no.
- Compensating provision: other inspectors still run (but each is independently droppable).

Single point of failure:
- is_spf: yes (for the specific gate that errored — no alternate covers that inspector's
  verdict). missing_alternate: true. redundancy_or_fallback: null.
  required_owner_decision: the fail-open-vs-fail-closed default for safety inspectors.

Criticality:
- Likelihood: plausible. Detectability: silent (log-only).

Validation:
- Test asserts the tool is blocked/needs-approval when a fail-closed inspector errors.

Non-goals:
- Do not change fail-open behavior for genuinely advisory (non-gating) inspectors.

---

## Non-Findings (dependencies checked with a real, cited alternate or clear refusal)

These are `is_spf: no` (or handled) with the alternate/refusal cited.

1. **Bundled `gosling` server binary (desktop) — DEP-010.** Absence refuses clearly at
   startup listing every searched path (`ui/desktop/src/goslingServe.ts:100-104`).
   Readiness is bounded to 30s with fatal-stderr and spawn-error detection
   (`goslingServe.ts:236-298`, `:556-561`, `:642-655`); failure throws with startup
   diagnostics. `degrade`/`hang` avoided → `fail_visible`. Held.

2. **`gosling serve` port — DEP-005.** Desktop selects an OS-assigned ephemeral port
   (`goslingServe.ts:106-118`, `listen(0)`), so port contention cannot occur; the CLI path
   `TcpListener::bind(addr).await?` (`crates/gosling-server/src/commands/agent.rs:128`)
   refuses on `EADDRINUSE`. Held.

3. **OS keyring — DEP-003/004.** On headless Linux (no D-Bus/Secret Service) or a locked
   macOS keychain, availability errors are matched
   (`crates/gosling/src/config/base.rs:1116-1128`) and secrets fall back to a 0600
   `secrets.yaml` (`:1094-1107`, `:42-60`); a process-wide `AtomicBool` avoids re-hitting
   the slow failing keyring (`:28`, `:1168-1173`). `degrade-honest` — but the only signal
   is `tracing::warn!("Keyring unavailable. Using file storage for secrets.")` (`:1145`),
   i.e. **log-only** visibility. Held; minor operator-signal note (route to
   `audit-operator-signal`).

4. **MCP stdio command absent — DEP-010.** `TokioChildProcess::builder(command).spawn()?`
   (`crates/gosling/src/agents/extension_manager.rs:400-402`) propagates ENOENT; captured
   stderr is attached to the error (`:407-451`). Refuse at first-use. Held.

5. **Provider CLI subprocess (`claude`/`codex`/`gemini`/`cursor-agent`) — DEP-010.** Spawn
   failure maps to `ProviderError::RequestFailed` naming the command
   (`crates/gosling/src/providers/claude_code.rs:429-434`). Defaults are bare names on PATH
   (`config/base.rs:1191-1194`). The generic ACP spawn is less specific
   (`crates/gosling/src/acp/provider.rs:1048`, `.context("failed to spawn ACP process")` —
   does not name the binary) but still refuses, not hangs. Held (ACP message could name the
   command).

6. **`git` absent — DEP-010.** Plugin clone `bail!`s with captured stderr
   (`crates/gosling/src/plugins/mod.rs:301-308`); `/review` uses `.context(...)` on each
   `git` invocation. Refuse at first-use. Held.

7. **`bash` absent — DEP-010.** `unix_shell()` falls back to `sh`
   (`crates/gosling/src/agents/platform_extensions/developer/shell.rs:137-153`), with
   `GOSLING_SHELL` opt-in. `degrade-honest`. Held.

8. **Provider unset — DEP-002.** `Agent::provider()` returns `Err("Provider not set")`
   (`crates/gosling/src/agents/agent.rs:795-799`); boot refuses on missing provider/model
   (`:2872-2887`). Held.

9. **Config/secrets file absent — DEP-001.** `read_secrets_from_file` returns an empty map
   when the file is missing (`config/base.rs:1072-1084`); `get_param` returns
   `NotFound`. Env vars override both (`:810-815`). `degrade-honest`. Held.

---

## Break-It Review (summary)

- Remove server binary → desktop refuses at startup naming paths (held).
- Take the bind port → CLI refuses; desktop cannot hit it (auto-port) (held).
- Strip D-Bus/Secret Service on Linux → file fallback + warn (held, log-only).
- Point the classifier at a closed port / TEST-NET → **silent** ML degradation
  (DEP-GSL-001).
- Force a safety inspector to `Err` → tool proceeds ungated (DEP-GSL-002).
- Make the selected provider unreachable mid-turn → request error, no failover (RR-001).
- Remove `uvx`/`npx`/`docker`/`git`/CLI binary → refuse at first use (held).

## SPF Attention Ranking (driving axis stated)

1. **DEP-GSL-002** — driving axis: absence behavior (`fail-open`/`degrade-silent` of a
   *gating* control) combined with `mid-operation` detection and silent visibility; worst
   because it can remove a safety gate in front of a real side-effecting tool.
2. **DEP-GSL-001** — driving axis: `degrade-silent` at `never` detection, but blast radius
   is bounded (opt-in ML; command path keeps patterns), so it ranks below GSL-002.
3. **RR-001** (provider outage) — driving axis: consumer count (every turn) but detection
   is `mid-operation`/visible (request error), and it is an intentional, owned SPF.

## Cross-Lens Escalations

- **`audit-failsafe-readiness` / security family:** DEP-GSL-002 (fail-open inspector loop)
  and DEP-GSL-001 (silent security degradation) are the code-level realization of the
  `SECURITY.md` prompt-injection posture; whether the *default* should be fail-closed for
  gating inspectors is a failsafe/security policy call, not a local dependency patch.
- **`audit-operator-signal`:** keyring-fallback (log-only), classifier-degraded (log-only),
  and inspector-dropped (log-only) all lack an operator-visible status — signal-quality
  belongs to that lens.
- **`audit-recovery-idempotency`:** provider stream mid-turn failure recovery (RR-001)
  belongs to that lens.

## Residual Risk Register

| ID | Finding | Retained risk | Required control | Control present | Safe state | Owner | Review by |
|----|---------|---------------|------------------|-----------------|-----------|-------|-----------|
| RR-001 | DEP-008 (register) | Single LLM provider per session; runtime outage (5xx / network / 429) has **no failover** — the `restore_provider_from_session` "fallback" (`agents/agent.rs:2907-2943`) only covers a *removed/renamed* provider in config, not an outage | bounded_wait + operator-visible failed-turn status + rerun hint (recovery-idempotency scope) | partial (per-provider 429/retry exists in `providers/*`; no cross-provider failover) | fail_visible | human-owner | when a provider-failover seam is designed |

Note: the register entry is a *decision to retain with a control*, not a severity
downgrade. The single-provider design is intentional; it is recorded so the outage SPF is
not lost.

---

## Validation Limits

- **Static / read-only only.** No live drills: the classifier was not actually pointed at a
  closed port, no inspector was forced to error at runtime, no provider outage was induced,
  and no headless-Linux keyring absence was reproduced. Per `confidence_calibration.md`,
  the *missing guard / substitution paths* are `Confirmed` from source; the runtime
  *manifestations* (that the tool actually runs ungated, that ML actually zeroes in
  production) are Confirmed-at-code-level but would need `test-reproduced` evidence to
  confirm the live outcome. DEP-GSL-001/002 are wording-anchored to the code property.
- **Sampling.** Provider 429/retry handling was confirmed present across ~20 provider files
  by grep but not read line-by-line per provider; a per-provider read could surface a
  provider that hammers on 429 (DEP-012) — not audited here.
- **Not exhaustively walked:** every `getenv`/`env::var` read site (DEP-002 defaults),
  every MCP transport variant (SSE/streamable-HTTP reachability timeouts vs stdio), the
  OAuth device/callback endpoint absence paths (`oauth/`), and the desktop TLS
  fingerprint-pinning failure branch beyond the 5s timeout. These are candidates for a
  follow-up pass.
- **`ui/gosling-binary/*/bin` are git-ignored** and absent in this checkout; binary
  discovery logic was audited from `goslingServe.ts`, not from a packaged artifact.
