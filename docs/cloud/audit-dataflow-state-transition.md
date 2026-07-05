# Gosling Audit — Lens: Dataflow / State-Transition (STT)

Lens authority: **audit-only / read-only**. Builds on `docs/cloud/00-orientation.md`.
Skill: `audit-dataflow-state-transition` v3.1. Scope focus: lifecycle & status
models — illegal transitions, gate/review bypass, partial/non-durable
transitions, ambiguous/missing status. Priority state machines audited:
**permission mode & tool-approval flow**, agent turn/session mode lifecycle,
OAuth callback flow, extension enable/malware-check.

Evidence discipline per `evidence_discipline.md`: every `Confirmed` cites a
line actually read. Severity argued from mechanism, independent of confidence.

---

## 1. State model inventory

| Object | States | Legal transitions | Gate/Guard | Mutation layer | Durable? | Idempotent? |
|---|---|---|---|---|---|---|
| Tool request | new → {approved, needs_approval, denied} → executed / declined | new→approved (auto), new→needs_approval→(confirmed)→executed, new→denied | `PermissionInspector::inspect` + security inspectors → `apply_inspection_results_to_permissions`; execution gated in `handle_approved_and_denied_tools` / `handle_approval_tool_requests` | domain (agent + inspectors) | executed effect not persisted as state; confirmation channel in-mem | approval is one-shot per request id |
| GoslingMode | Auto / Approve / SmartApprove / Chat | any→any via `update_gosling_mode` | none (user choice) | agent + session_manager | persisted to session (agent.rs:2803) | n/a |
| Smart-approve permission cache | none / AlwaysAllow / AskBefore / NeverAllow (per tool **name**) | LLM judge or annotation writes | none — write is unconditional | `PermissionManager` | **persisted to disk** (permission.rs:183) | last-write-wins by name |
| MCP extension | disabled → (malware check) → enabled/launched | config enable; launch on demand | `deny_if_malicious_cmd_args` (fail-open) | `ExtensionManager` | config persisted | n/a |
| MCP OAuth (extension auth) | request → authorize → callback(code,state) → token | delegated to `OAuthState` lib | CSRF `state` validated inside `handle_callback` (external lib) | `oauth/mod.rs` | token persisted (`persist.rs`) | n/a |

Intended machine for the tool-approval gate is recovered from the `GoslingMode`
enum (`crates/gosling-providers/src/gosling_mode.rs:24-34`) with its documented
per-mode contract ("Auto: automatically approve", "Approve: ask before every
tool call", "SmartApprove: ask only for sensitive tool calls", "Chat: no tool
calls") and the `InspectionAction` enum (`tool_inspection.rs:23-30`).

## 2. Boundary map (mutation boundary = `dispatch_tool_call`)

`dispatch_tool_call` (actual tool execution) is reached from exactly two sites:

1. `handle_approved_and_denied_tools` (agent.rs:740-771) — iterates
   `permission_check_result.approved` and dispatches directly, no confirmation.
2. `handle_approval_tool_requests` (tool_execution.rs:134-154) — dispatches only
   when `confirmation.permission == AllowOnce || AlwaysAllow`.

`denied` never reaches dispatch (error response only, agent.rs:777-792). The gate
is therefore enforced at the mutation boundary; the question for each finding is
**what puts a request into `approved` without a user confirmation**.

---

## 3. Findings

### STT-GSL-001: SmartApprove auto-executes tools by self-declared `read_only_hint` (attacker-controlled for MCP extensions)

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: State-Transition

Evidence:
- `crates/gosling/src/permission/permission_inspector.rs:38-53` — `apply_tool_annotations` populates `readonly_tools` directly from each tool's own annotation: `if anns.read_only_hint == Some(true) { readonly_annotated.insert(tool.name...) }`.
- `crates/gosling/src/permission/permission_inspector.rs:164-169` — in `Approve | SmartApprove`, `else if self.is_readonly_annotated_tool(tool_name) || (SmartApprove && cached AlwaysAllow) { InspectionAction::Allow }`.
- `crates/gosling/src/agents/agent.rs:678-680` — annotations are applied (populating the read-only set) only when `gosling_mode == SmartApprove`.
- Orientation §4: "MCP extensions / ACP subprocesses (third-party code) → tool results are attacker-influenceable content; the framework spawns and trusts them." Tool definitions (incl. annotations) originate from the extension.

Observed behavior:
- In SmartApprove mode a tool whose definition declares `read_only_hint: true` transitions straight to `approved` and executes with **no user confirmation** (`InspectionAction::Allow`, reason "Tool annotated as read-only").

Expected boundary:
- The read-only classification used to skip the approval gate must derive from a trusted source. An untrusted MCP extension must not be able to self-certify its own destructive tool as read-only to bypass the SmartApprove gate.

Failure mechanism:
- `read_only_hint` is taken verbatim from the tool's annotation with no validation and no cross-check against the tool's actual behavior; a malicious or careless extension sets it to `true` on a write/destructive tool.

Break-it angle:
- Enable an MCP extension exposing a `delete_all`/`shell` tool annotated `read_only_hint: true`; in SmartApprove the agent runs it without prompting. (Enabling the extension is itself approval-gated via `MANAGE_EXTENSIONS`, but that approval says nothing about individual tool annotations.)

Impact:
- Approval gate for the "sensitive tool calls" mode is bypassed for any enabled extension; blast radius = user workstation (shell/file/network per SECURITY.md).

Operational impact:
- Blast radius: Service (workstation) / Cross-system if the tool egresses.
- Side-effect class: process / file / network.
- Reversibility: irreversible (destructive tool executes).
- Operator visibility: silent (no prompt shown).
- Rerun safety: unsafe.

Adjacent failure modes:
- STT-GSL-002 (the LLM-judge cache is the other path into `Allow`).
- A mislabeled **built-in** tool would inherit the same trust; annotation-source integrity is the shared root.

Recommended mitigation:
- Remediation pattern: trusted-classification / allowlist. Only honor `read_only_hint` for first-party/built-in tools; for extension tools, ignore self-declared read-only and fall through to LLM judge or explicit approval.
- Local guardrail: tag each `Tool` with a provenance flag at registration; `is_readonly_annotated_tool` consults provenance.
- Behavior test: SmartApprove + extension tool annotated read-only + `read_only_hint` true ⇒ `RequireApproval`, not `Allow`.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: modules (inspector + tool registration), tests
- Nominal implementation agent: claude
- Rationale: needs a provenance concept threaded from extension registration to the inspector; touches trust-boundary logic, not a one-line guard.

Validation:
- Negative test asserting persisted decision path is `RequireApproval` for extension-sourced read-only-annotated tools in SmartApprove.

Non-goals:
- Do not change SmartApprove's treatment of genuinely first-party read-only tools.

---

### STT-GSL-002: SmartApprove LLM read-only verdict is cached AlwaysAllow by tool NAME, persisted, and ignores arguments (derived state becomes durable authority)

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: State-Transition

Evidence:
- `crates/gosling/src/permission/permission_inspector.rs:239-254` — after LLM detection, `permission_manager.update_smart_approve_permission(&tc.name, level)` where `level = AlwaysAllow` if judged read-only.
- `crates/gosling/src/permission/permission_judge.rs:90-116` — `create_check_messages` builds the judge prompt from `tool_call.name` **only** (`tool_names.join(", ")`); arguments are never sent to the judge.
- `crates/gosling/src/permission/permission_inspector.rs:165-169` — a cached `SmartApprove` `AlwaysAllow` for the name short-circuits to `InspectionAction::Allow` on every subsequent call.
- `crates/gosling/src/config/permission.rs:218-220,157-184` — smart-approve permission is stored per name and `self.persist()`ed to disk.

Observed behavior:
- The first time a tool name is judged read-only, `AlwaysAllow` is written for that **name** and persisted. Every later invocation of the same name — with any arguments — is auto-approved without re-judging.

Expected boundary:
- A read-only determination that gates execution must be a function of the actual operation (name + arguments), or must be re-derived rather than cached as durable authority; a per-name cache must not authorize argument sets that were never evaluated.

Failure mechanism:
- The judge classifies by name with no arguments; the derived verdict is persisted as authority keyed by name; consumers trust the cached verdict instead of recomputing per call.

Break-it angle:
- A tool whose read-only-ness depends on arguments (e.g. a generic `query`/`fetch`/`run` tool that is read-only for a benign arg and destructive for another). Once any call gets it cached `AlwaysAllow`, all future destructive-arg calls auto-execute. Also: a first-party tool judged read-only stays read-only across upgrades that add write behavior under the same name.

Impact:
- Durable, silent widening of the auto-approve set; a name judged read-only once is permanently ungated regardless of arguments.

Operational impact:
- Blast radius: Service (workstation), persists across sessions/restarts.
- Side-effect class: process / file / network on the mis-approved calls.
- Reversibility: cache is reversible (edit config) but the executed effects are not.
- Operator visibility: silent (reason logged as "SmartApprove cached as read-only").
- Rerun safety: unsafe (replay auto-approves).

Adjacent failure modes:
- STT-GSL-001 (annotation path into `Allow`).
- Cross-session propagation: the disk-persisted cache authorizes future sessions the operator never reviewed.

Recommended mitigation:
- Remediation pattern: recompute-on-read / argument-scoped decision. Either (a) include arguments in the judge input and cache keyed by an argument-shape/hash, or (b) treat the LLM verdict as advisory per-call and do not persist AlwaysAllow.
- Local guardrail: cap the cache to session lifetime, or add a freshness/provenance stamp so a name whose tool definition changed is re-judged.
- Behavior test: judge tool `run` read-only for arg A, then call with arg B ⇒ still `RequireApproval`.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: judge prompt + cache key change, migration of persisted config, tests
- Nominal implementation agent: claude
- Rationale: changes the caching key/semantics and the judge contract; needs care to avoid regressing SmartApprove UX.

Validation:
- Test that argument variation is not auto-approved by a name-level read-only cache; test that persisted cache is not blindly trusted after tool definition change.

Non-goals:
- Do not remove SmartApprove; do not change the judge model.

---

### STT-GSL-003: Extension-launch malware gate fails open (skips non-npx/uvx and all OSV errors)

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: State-Transition

Evidence:
- `crates/gosling/src/agents/extension_manager.rs:1084` — `deny_if_malicious_cmd_args(cmd, args).await?;` immediately before launching a stdio extension process.
- `crates/gosling/src/agents/extension_malware_check.rs:49-56` — only `*uvx`→PyPI and `*npx`→npm are checked; every other command `return Ok(())` ("fail open").
- `crates/gosling/src/agents/extension_malware_check.rs:211-233` — OSV request error, HTTP error, and JSON parse error all `return Ok(())` (fail open).

Observed behavior:
- The disabled→launched transition proceeds even when the malware check could not run: any non-npx/uvx launcher, or any transient/blocked OSV network condition, yields "allowed".

Expected boundary:
- A malware gate on a state transition that spawns third-party code should fail closed (or at minimum surface an explicit un-verified state) rather than silently transitioning to launched.

Failure mechanism:
- The check maps every uncertain outcome to `Ok(())`; the caller cannot distinguish "verified clean" from "not verified".

Break-it angle:
- Block/degrade `api.osv.dev` (offline, proxy 403), or wrap the same malicious package behind a non-npx/uvx wrapper command; the extension launches unchecked.

Impact:
- Defense-in-depth control provides no guarantee under the exact conditions (offline / novel launcher) an attacker can arrange.

Operational impact:
- Blast radius: Service (spawns process) / Cross-system.
- Side-effect class: process.
- Reversibility: irreversible once the process runs.
- Operator visibility: log-only (debug/error), no state surfaced to operator.
- Rerun safety: unsafe.

Adjacent failure modes:
- Version `None` queries by name only (broadens/narrows depending on OSV); pinned-version bypass if attacker publishes clean version then swaps.

Recommended mitigation:
- Remediation pattern: fail-closed gate with explicit override. On check-unavailable, block launch or require explicit user approval carrying an "unverified" banner; do not silently allow.
- Local guardrail: distinguish `Verified` / `Unverified` / `Malicious` as three outcomes; only `Verified` (or explicit user override) transitions to launched.
- Behavior test: OSV 500 ⇒ launch blocked or approval-required (currently `is_ok()`), asserting the transition does not silently proceed.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: change gate contract, operator UX for unverified state, tests
- Nominal implementation agent: human-owner
- Rationale: fail-open→fail-closed is a policy decision (breaks offline installs); needs owner sign-off, not just a code patch.

Validation:
- Tests replacing the current `assert!(res.is_ok())` fail-open expectations (extension_malware_check.rs:463-482) with fail-closed/require-approval semantics.

Non-goals:
- Do not add new ecosystems in this slice.

---

### STT-GSL-004: `gosling_mode` snapshotted per turn; mid-turn tightening does not apply to the in-flight turn

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: State-Transition

Evidence:
- `crates/gosling/src/agents/agent.rs:676` — `let gosling_mode = *self.current_gosling_mode.lock().await;` read once during `prepare_reply_context`.
- `crates/gosling/src/agents/agent.rs:1839-1847` — `reply_internal` destructures `gosling_mode` from `ReplyContext` and uses this single snapshot for the whole tool loop (used at agent.rs:2168,2192).
- `crates/gosling/src/agents/agent.rs:2789-2811` — `update_gosling_mode` updates `current_gosling_mode` + persists to session, but an already-running turn holds the old value.

Observed behavior:
- Changing mode mid-turn (e.g. Auto→Approve or →Chat to halt tool execution) has no effect until the next turn; the running turn keeps auto-approving.

Expected boundary:
- A tightening of the approval mode should take effect for subsequent tool calls in the same turn, or the operator control should document that it applies next turn only.

Failure mechanism:
- The mode is captured as a per-turn snapshot rather than re-read at each tool-inspection point.

Break-it angle:
- During a long multi-tool Auto turn, the operator switches to Approve to stop a runaway; remaining queued tool calls still execute unprompted.

Impact:
- Operator control lag; bounded because the operator can cancel the turn (cancel_token path exists).

Operational impact:
- Blast radius: Workflow.
- Side-effect class: process/file/network for the remaining auto-approved calls.
- Reversibility: irreversible per executed call.
- Operator visibility: UI-visible mode change that appears applied but is not, for the current turn.
- Rerun safety: safe.

Adjacent failure modes:
- `principal_type` on `PermissionConfirmation` is accepted from the confirmation request but never gates execution (see non-findings) — an ambiguous-status smell in the same subsystem.

Recommended mitigation:
- Re-read `current_gosling_mode` at each tool-inspection iteration, or explicitly document snapshot semantics and rely on cancellation for hard stops.
- Behavior test: switch to Approve after first tool call in a multi-tool turn ⇒ second tool call requires approval.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: one read-site change + test
- Nominal implementation agent: codex
- Rationale: narrow, mechanical; only risk is the SmartApprove annotation-apply step keying off the same snapshot.

Validation:
- Test asserting mid-turn mode tightening changes the persisted approval decision for the next tool call.

Non-goals:
- Do not attempt to retroactively unwind already-dispatched calls.

---

## 4. Non-findings (seams checked and held)

- **Core approval gate is at the mutation boundary.** `dispatch_tool_call` is reached only from `approved` (agent.rs:740-749) or a confirmed `AllowOnce/AlwaysAllow` (tool_execution.rs:134-135). `denied` yields an error response and never dispatches (agent.rs:781-791). Held.
- **Inspection override is monotonic toward stricter.** `apply_inspection_results_to_permissions` (tool_inspection.rs:213-253): a security `Deny`/`RequireApproval` removes a request from `approved`; an inspector `Allow` explicitly does **not** downgrade an existing deny/approval (lines 249-252). Security cannot loosen a stricter decision. Held.
- **Fail-closed default when no permission decision exists.** `process_inspection_results` pushes any request lacking a permission result to `needs_approval` (permission_inspector.rs:102-105), and the top-level loop defaults the whole batch to `needs_approval` if the permission inspector is absent (agent.rs:2201-2209). Held.
- **Confirmation channel is fail-closed on drop/cancel.** `handle_approval_tool_requests` awaits `confirmation_rx` and propagates an error (no dispatch) if the channel closes (tool_execution.rs:115-116); the router prunes stale/closed senders on register (tool_confirmation_router.rs:22-24). Held.
- **Chat mode does not run tools.** Chat is handled before inspection: tool calls get `CHAT_MODE_TOOL_SKIPPED_RESPONSE` and are never dispatched (agent.rs:2168-2184). The `GoslingMode::Chat => continue` arm in the inspector (permission_inspector.rs:152) is defensive/unreached. Held.
- **Denied/AlwaysDeny caches NeverAllow durably.** `AlwaysDeny` writes `NeverAllow` (tool_execution.rs:164-167); terminal deny is respected on later calls via `get_user_permission` (permission_inspector.rs:156-159). Held.
- **Server confirmation route requires the server secret.** `confirm_tool_action` (routes/action_required.rs:35-52) is reached only through the authenticated router; the LLM emits tool calls, not authenticated localhost HTTP, so the model cannot self-approve via this route in the traced path. Held (see Validation Limits for un-traced ACP/CLI callers).

## 5. Break-it review summary

- Drive tool→executed skipping the gate: **reachable in SmartApprove** via self-declared read-only annotation (STT-GSL-001) and name-cached LLM verdict (STT-GSL-002); **not reachable** in Approve mode (annotations only applied in SmartApprove, agent.rs:678) and **not reachable** for `denied`.
- Replay the same approval twice: router delivers once (`remove` on deliver, tool_confirmation_router.rs:28); one-shot. No double-dispatch found.
- Confirmation for a foreign request id: keyed by unique request id + session; delivery to unknown id is a no-op warning (tool_confirmation_router.rs:38-44). Held.
- Feed a status not in the enum: `GoslingMode`/`Permission` are Rust enums deserialized with serde; unknown values are rejected at parse. Held.
- Mutate a terminal (denied) request: no path re-opens a denied request within a turn. Held.

## 6. Validation Limits (what was NOT reviewed)

- Not executed live — no build/run; all findings are `source-evidenced` / `simulation-reasoned`, not `runtime-observed`.
- ACP-provider permission routing (`PermissionRouting::ActionRequired`, `handle_permission_confirmation`, agent.rs:1372-1381) was not traced into each ACP provider (`claude_acp.rs`, `codex_acp.rs`, etc.); those providers own their own approval state and may have distinct gates/bypasses. Un-reviewed.
- Full caller set of `handle_confirmation` (CLI `session/mod.rs`, `acp/server.rs`) was not exhaustively authz-traced; the self-approval non-finding is scoped to the server route.
- OAuth CSRF `state` validation is delegated to the external `OAuthState`/`handle_callback` library (`oauth/mod.rs:178`); the library's state check was not inspected. Provider device-flow OAuth (providers/auth) not reviewed under this lens.
- Extension enable/disable persistence and the `MANAGE_EXTENSIONS` approval-to-enable transition were only spot-checked; the full config-mutation atomicity (partial-write on crash between config update and process launch) was not traced (candidate STT-003, un-assessed).
- Concurrency manifestations (two turns racing the smart-approve cache, `current_gosling_mode` read/write races) are left to the concurrency lens; kept out of confidence here.
- `principal_type` on `PermissionConfirmation` appears unused as a gate; not deeply traced across all consumers (potential STT-005 ambiguous-status, Plausible only).

## 7. Patch order (this lens)

1. STT-GSL-001 (High) — provenance-gate the read-only annotation trust.
2. STT-GSL-002 (High) — argument-scope / de-persist the LLM read-only cache.
3. STT-GSL-003 (Medium) — fail-closed (or explicit unverified state) on malware-check unavailability [owner decision].
4. STT-GSL-004 (Low) — re-read mode per tool iteration or document snapshot semantics.
</content>
</invoke>
