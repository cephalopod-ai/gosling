# Gosling Audit — Lens: Workflow / GUI Integrity (operator-facing truth)

Domain prefix: **WFG** · Skill: `audit-workflow-gui` v3.1 · Authority: **audit-only /
read-only** (only this file was written). Builds on
`docs/cloud/00-orientation.md`.

Lens focus (as tasked): operator-facing TRUTH across **CLI** (`crates/gosling-cli`),
**TUI** (`ui/text`, Ink) and **desktop** (`ui/desktop`, Electron/React) — fake success,
UI/API & CLI/API mismatch, stale display, hidden failure, destructive ambiguity, and
**approval-gate display-vs-enforcement mismatch**. Central cross-check: does each UI's
rendering of tool approval and tool outcome match what the core actually decided/did?

## Effort budget & method

~30 tool calls (within the 30–45 budget). Prioritized, per instruction: tool-approval
display, error surfacing, and success/failure rendering in both UIs. Read the three
approval entry points (CLI interactive/headless, TUI two handlers, desktop
`ToolApprovalButtons` + ACP permission bridge), the desktop and TUI tool-result
renderers, and one destructive flow (session delete). Ink fixed-height rules from
CLAUDE.md were applied to `ui/text` as correctness risks. Unreviewed surfaces are listed
under Validation Limits.

## Surface / boundary inventory (tool-truth table)

| View / control | Backend effect | Shown status source | Source of truth? | Failure feedback | Gate bypass? |
|---|---|---|---|---|---|
| CLI tool confirm (`session/mod.rs:1163`) | runs/denies tool | interactive picker result | actual decision | yellow "cancelled" + tool_response Err | No — fails closed non-interactive (WFG non-finding NF-1) |
| **TUI permission (`tui.tsx:763` & `:1315`)** | runs tool + persists AlwaysAllow | **auto-selects `options[0]`** | **request-submission, NOT operator** | **none — no prompt shown** | **YES (WFG-GSL-001)** |
| Desktop approval buttons (`ToolApprovalButtons.tsx`) | resolves ACP permission | button click | operator click | stale-request line; throw swallowed (WFG-GSL-005) | No (options ignored → WFG-GSL-004) |
| Desktop tool result (`ToolCallWithResponse.tsx:511`) | — (render) | streaming-done + response presence | **launch/absence, not result** (WFG-GSL-002) | error `.error` string dropped (WFG-GSL-003) | n/a |
| TUI tool box (`toolcall.tsx:36`) | — (render) | ACP `tool_call_update.status` | actual status | red ✗ + red border on `failed` (NF-3) | n/a |
| Desktop session delete (`SessionListPane.tsx:595`) | deletes session + file | try/catch → toast | actual result | error toast + partial-failure toast (NF-2) | n/a |

---

## Findings

### WFG-GSL-001: TUI silently auto-approves every tool call with the most-permissive option (AllowAlways), bypassing the approval gate, the configured GoslingMode, and security-inspector prompts

Severity: **High**
Confidence: **Confirmed**
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/text/src/tui.tsx:763-773` (interactive TUI client) and `ui/text/src/tui.tsx:1315-1325`
  (headless `--text` mode) — both `requestPermission` handlers:
  ```ts
  const optionId = params.options?.[0]?.optionId ?? "approve";
  return { outcome: { outcome: "selected", optionId } };
  ```
- `crates/gosling/src/acp/server.rs:1891-1896` — the core sends options in a fixed order,
  `AllowAlways` **first**: `vec![option(AllowAlways), option(AllowOnce), option(RejectOnce),
  option(RejectAlways)]`. `option()` (`:1883-1889`) sets `optionId` to the serialized kind,
  i.e. `"allow_always"`.
- `crates/gosling/src/acp/server.rs:3430-3432` (test) — `"allow_always"` maps to
  `Permission::AlwaysAllow`.
- `crates/gosling/src/agents/tool_execution.rs:150-153` — `AlwaysAllow` calls
  `update_permission_manager(&tool_call.name, PermissionLevel::AlwaysAllow)` → the decision
  is **persisted**, not just per-call.
- `crates/gosling/src/agents/tool_execution.rs:118-131` — a security-inspector finding for
  this call is only `tracing::info!`-logged as `ALLOW`; the `security_prompt` that the core
  attached to the confirmation request is never rendered by the TUI.

Observed behavior:
- Every tool-permission request the core raises — including ones raised specifically because
  the tool is destructive or because a prompt-injection / egress finding fired — is answered
  by the TUI by selecting `options[0]`, which is `AllowAlways`. No approval UI is ever shown
  to the TUI operator, and the decision is persisted for all future calls of that tool.

Expected boundary:
- When the core requests confirmation (mode `Approve`/`SmartApprove`, or a security finding),
  the front end must surface the request to the operator and forward the operator's actual
  choice — never auto-select, and never silently escalate to the most-permissive/persistent
  option. Compare the CLI, which prompts interactively and *fails closed* in non-interactive
  Approve/SmartApprove (`crates/gosling-cli/src/session/mod.rs:1172-1179`), and the desktop,
  which shows real buttons and even hides "Always Allow" when a security prompt is present
  (`ui/desktop/src/components/ToolApprovalButtons.tsx:145`).

Failure mechanism:
- The handler binds the response to `options[0]` (request submission) instead of to operator
  input. Because the core orders `AllowAlways` first, "pick the first option" is the worst
  possible default: it both allows and persists.

Break-it angle:
- Run `gosling` in the text UI under `GoslingMode::Approve` (the mode whose entire purpose is
  to gate tools) and have the model call `shell rm -rf …` or trigger an egress finding: the
  tool runs with zero operator interaction and `AlwaysAllow` is written to the permission
  store, so it will never prompt again.

Impact:
- The permission system and the security inspectors — the app's primary safety controls per
  `SECURITY.md` / orientation §5 — are fully defeated for any operator using the text UI.
  Destructive/exfiltration tool calls execute with no visibility and no abort path, and the
  most-permissive decision is durably persisted.

Operational impact:
- Blast radius: Cross-system (agent gains unattended shell/file/network on the workstation)
- Side-effect class: process / file / network / external API
- Reversibility: irreversible (executed side effects; persisted AlwaysAllow)
- Operator visibility: silent (log-only at info level)
- Rerun safety: unsafe (persisted always-allow compounds)

Adjacent failure modes:
- Same root as WFG-GSL-004 (front end ignores the semantic meaning of the offered options).
- Cross-lens: escalates the permission/security-inspector lenses — the gate holds in core
  but is nullified by this client.

Recommended mitigation:
- Remediation pattern: *forward real operator decision; fail closed on non-interactive*.
- Minimal repair: render an approval prompt in the interactive TUI and send the operator's
  chosen `optionId`; in headless `--text` mode, respect `GoslingMode` and either reject the
  session (like the CLI) or select `RejectOnce`, never `options[0]`.
- Local guardrail: never index `options[0]`; map an explicit operator action → option kind,
  and treat a missing interactive channel as deny.
- Behavior test: TUI under Approve mode with a pending tool confirmation must not resolve
  without operator input, and must never emit `allow_always` unprompted.

Implementation assessment:
- Complexity: operator_ux
- Cost: M
- Cost drivers: new TUI approval component, headless-mode policy, tests
- Nominal implementation agent: claude
- Rationale: front-end UX plus a policy decision mirroring the CLI's fail-closed contract.

Validation:
- Test asserts: (a) interactive TUI shows a prompt and forwards the selected kind; (b)
  headless text mode in Approve/SmartApprove does not auto-allow; (c) no path emits
  `allow_always` without an explicit operator "always" choice.

Non-goals:
- Do not redesign the core permission model; the core already computes the correct gate.

---

### WFG-GSL-002: Desktop renders a tool with no response after streaming ends as green "success" (fake success)

Severity: **Medium**
Confidence: **Confirmed**
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/components/ToolCallWithResponse.tsx:509-520`:
  ```ts
  // This is a workaround for cases where the backend doesn't send tool responses
  const isStreamingComplete = !isStreamingMessage;
  const shouldShowAsComplete = isStreamingComplete && !toolResponse;
  const loadingStatus = !toolResponse
    ? shouldShowAsComplete ? 'success' : 'loading'
    : ...status === 'error' ? 'error' : 'success';
  ```
- `ui/desktop/src/components/ToolCallStatusIndicator.tsx:26-27` — `'success'` renders a green
  dot.

Observed behavior:
- When streaming finishes but **no tool response was ever received** (crash, cancellation,
  dropped result, backend that "doesn't send tool responses" per the code's own comment), the
  tool is shown with a green success indicator instead of an error/unknown state.

Expected boundary:
- Status must derive from a captured tool result. Absence of a result is "unknown/incomplete",
  not "success" (per the skill's worked example WFG-CH-001).

Failure mechanism:
- Success is bound to `!toolResponse && streamingComplete` (an absence) rather than to a
  verified result.

Break-it angle:
- Trigger a tool whose result never arrives; the operator sees green success and acts on an
  output that does not exist.

Impact:
- Operator believes a tool completed successfully when it may have failed or been dropped.

Operational impact:
- Blast radius: Workflow · Side-effect class: user-visible · Reversibility: reversible ·
  Operator visibility: UI-visible-but-wrong · Rerun safety: unknown

Adjacent failure modes: WFG-GSL-003 (error case also under-surfaced).

Recommended mitigation:
- Minimal repair: render a distinct "incomplete/unknown" state (not green) when
  `!toolResponse` after streaming; only show success on a received non-error result.
- Behavior test: streaming-complete + no response ⇒ status ≠ success.

Implementation assessment: Complexity operator_ux · Cost S · agent codex · Rationale: local
render-logic change + test.

Validation: assert `getToolCallStatus` output for the no-response case is not `success`.

Non-goals: do not change the backend response contract in this slice.

---

### WFG-GSL-003: Desktop drops the error string on a failed tool result — operator cannot diagnose

Severity: **Medium**
Confidence: **Likely**
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/components/ToolCallWithResponse.tsx:226-227` — server serializes a failed
  `ToolResult` as `{ status: "error", error: string }`.
- `:532-535` — `toolResults` is populated **only** when `loadingStatus === 'success'`.
- `:138-147` `getToolResultContent` returns `[]` when `toolResult.status !== 'success'`.
- The only content rendering is `toolResults.map(...)` (`:846-854`); there is no branch that
  renders `toolResult.error`.

Observed behavior:
- A tool whose execution errored (Rust `Err` variant) shows a red status dot and the tool
  label, but the `error` message itself is never displayed anywhere in the tool card.

Expected boundary:
- A failure must give the operator an actionable reason (skill WFG-013).

Failure mechanism:
- Content extraction is gated on `success`, and the `error` field of the error variant has no
  renderer, so the reason is silently discarded.

Break-it angle:
- Cause a tool to return the error variant (e.g. an extension that throws): operator sees a
  red dot with no message and cannot tell what failed.

Impact: undiagnosable failures; operator retries blind or misattributes cause.

Operational impact: Blast radius Workflow · Side-effect class user-visible · Reversibility
reversible · Operator visibility red-dot-only · Rerun safety safe.

Note / why Likely not Confirmed: MCP tools that report failure as `status:'success'` with
`isError`/error text in `content` **are** rendered (that path shows content). Only the
`ToolResult` `error` variant is dropped; I did not runtime-confirm which error path each tool
class takes.

Recommended mitigation:
- Minimal repair: when `toolResult.status === 'error'`, render `toolResult.error` in the card.
- Behavior test: error-variant result displays its message.

Implementation assessment: Complexity operator_ux · Cost S · agent codex.

Validation: assert the error string appears in the rendered card for an error-variant result.

Non-goals: none.

---

### WFG-GSL-004: Desktop approval buttons ignore the agent's offered options; an unoffered choice silently maps to "cancelled" while the UI reports the chosen action

Severity: **Low** (Medium if a reduced-option ACP agent is connected)
Confidence: **Plausible**
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/components/ToolApprovalButtons.tsx:135-157` renders Allow Once / (Always
  Allow if `!prompt`) / Deny from a fixed set — it never inspects `request.options`.
- `ui/desktop/src/acp/permissionRequests.ts:64-74` — `permissionResponseForAction`: if the
  requested action's kind is not in `request.options`, `permissionOptionIdForAction` returns
  `undefined` and the response falls back to `cancelledPermissionResponse()`.
- `ToolApprovalButtons.tsx:108-133` — `resolveAcpPermissionRequest` returning `true` sets the
  UI to the chosen action's resolved label (e.g. "Always allowed"), regardless of what was
  actually sent.

Observed behavior:
- If the connected agent offers a reduced option set (e.g. no `allow_always`), clicking
  "Always Allow" shows "Always allowed" to the operator while the ACP outcome sent is
  `cancelled` — a UI/API mismatch (shown allow vs actual cancel).

Why Plausible (latent for gosling itself): gosling's own ACP server always emits all four
options (`server.rs:1891-1896`), and the desktop connects to that server, so the fallback is
not reached in the default configuration. It becomes reachable only if the desktop is pointed
at an ACP agent that omits option kinds — which I did not confirm exists in this repo.

Expected boundary: buttons must be derived from `request.options`, and the resolved label must
reflect the outcome actually sent.

Recommended mitigation: build buttons from `request.options`; if a mapped option is missing,
disable the button rather than sending `cancelled` under a success label.

Implementation assessment: Complexity operator_ux · Cost S · agent codex.

Validation: given a request lacking `allow_always`, assert the "Always Allow" button is not
shown, and that no click reports "Always allowed" while sending `cancelled`.

Non-goals: none.

---

### WFG-GSL-005: Desktop approval handler swallows a thrown resolve error with no operator feedback

Severity: **Low**
Confidence: **Confirmed**
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/components/ToolApprovalButtons.tsx:108-118`:
  ```ts
  } catch (err) {
    console.error('Error confirming tool action:', err);
  }
  ```
  On a thrown error the component does not set `approvalError`, does not mark the decision,
  and leaves the buttons in place with no visible change.

Observed behavior: if `resolveAcpPermissionRequest` throws, the operator's click appears to do
nothing; only a console line is written. (The `false`/stale path *is* surfaced at `:113`; the
throw path is not.)

Expected boundary: every operator action on the approval control yields a visible result
(WFG-011/WFG-013).

Recommended mitigation: set `approvalError` in the catch, mirroring the stale-request branch.

Implementation assessment: Complexity local_guardrail · Cost XS · agent codex.

Validation: force a throw; assert an error message renders.

Non-goals: none.

---

## Non-findings (checked and held)

- **NF-1 — CLI tool approval is correctly gated.** `crates/gosling-cli/src/session/mod.rs:1163-1210`
  prompts interactively (`prompt_tool_confirmation`, `:1769-1817`), and in **non-interactive**
  mode it *fails closed* for `Approve`/`SmartApprove` (`:1172-1179`), only auto-allowing
  `AllowOnce` in `Auto` mode with a `tracing::warn!`. Cancel maps to `DenyOnce` and pushes a
  tool_response error (`:1187-1206`). This is the correct contrast to WFG-GSL-001. Denies
  WFG-007 for the CLI.
- **NF-2 — Desktop session delete is not destructively ambiguous.** `SessionListPane.tsx:98-131`
  uses a confirm dialog naming the session with "This action cannot be undone", and a distinct
  message when a tracked archive file will also be removed. `:595-626` surfaces failure via
  `deleteFailed` toast and surfaces **partial** failure (session deleted but archive file not
  removed) via an additional `deleteArchiveFileFailed` toast. Denies WFG-006 and WFG-009 for
  this flow.
- **NF-3 — TUI collapsed tool box surfaces failure.** `ui/text/src/toolcall.tsx:36-41,87-92` maps
  `failed` → red ✗ icon + `CRANBERRY` border. The `failed` status path is not a fake success.
  (The gap is WFG-GSL-001, which is about *approval*, and the collapsed box not showing the
  failure *reason* until expanded — expansion view not deep-reviewed; see Limits.)
- **NF-4 — TUI content/tool renderers respect Ink fixed-height rules.** `ContentRenderers.tsx`
  emits `height={1}` boxes with `wrap="truncate"`/`"truncate-end"` (`:70-98`), and `toolcall.tsx`
  uses `truncate-end` with `flexGrow={1}` on an **empty spacer** box, not on text (`:124-138`).
  The main layout is fixed-height with a virtualized/sliced line list (`tui.tsx:1204-1244`).
  No fixed-height `wrap="wrap"` on dynamic text was found in the tool/content path. One latent
  note: `renderUserPrompt`'s collapsed preview uses `wrap="wrap"` (`ContentRenderers.tsx:288,298`)
  inside an unbounded-height Box, but the text is pre-truncated to a preview + "…", so it stays
  ~1 line in practice. Recorded as an observation, not a finding.

## Cross-lens escalations (for the audit lead)

- **WFG-GSL-001 escalates the permission and security-inspector lenses.** The core gate and the
  inspectors (`crates/gosling/src/security/*`, `permission/*`) may be individually sound, but the
  text-UI client nullifies them: a correct core decision to *ask* is answered "AllowAlways" with
  no operator involvement, and inspector findings are downgraded to info-logs. Any lens rating
  the permission/security controls as effective must qualify that rating for the `ui/text`
  surface.
- **Contract/architecture-seam lens:** the ACP option *ordering* (`AllowAlways` first,
  `server.rs:1891`) is load-bearing for a client that does `options[0]`. A producer→consumer
  contract where "position 0 = most permissive" is a latent hazard worth flagging to the
  contract-internalapi lens.

## Validation Limits (not reviewed / not executed)

- **No app was built or run** (per orientation §7 deferral). All findings are source-evidenced /
  simulation-reasoned; none are runtime-observed.
- The TUI **expanded** tool-call detail view (`ToolCallExpanded`, `tui.tsx:1239`) was not
  deep-read for whether it renders the failure reason; NF-3 covers only the collapsed box.
- Desktop **elicitation** UI, `ProgressiveMessageList`, streaming-cancel rendering, and the full
  toast inventory were not exhaustively reviewed.
- WFG-GSL-004's reachability was not confirmed against any real reduced-option ACP agent; capped
  at Plausible accordingly.
- Concurrency/staleness of the desktop `globalApprovalState` map
  (`ToolApprovalButtons.tsx:46-70`) and of the pending-request map on session churn was not
  race-tested (per method v3.1, such staleness would cap at Likely until reproduced).
- `crates/gosling-server` ACP response construction beyond the permission-option block, and the
  full CLI streaming/error rendering, were sampled, not exhaustively read.
