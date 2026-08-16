# Gosling Audit — Workflow / GUI Integrity + Design Webapp + Compliance Posture

**Date:** 2026-08-15  
**Target:** `/Users/eric/Work/vscode/forked/gosling`  
**HEAD:** `073d19428509ea6eb317924b1856a1fe7e9002c8` (`refine XAI auth settings and OAuth handling`)  
**Authority:** read-only / audit-only. Source was not modified. This file is the assigned report.  
**Lenses:** `audit-workflow-gui` (WFG-001..015), `audit-design-webapp` (Gates 1–6), `audit-compliance-posture` (CMP-001..015)  
**Surfaces:** `ui/desktop`, `ui/text`, CLI output, README / SECURITY / Giles / `docs/cloud` posture language.

The supplied prompt is treated as a draft. The intended mission is preserved: re-verify operator-facing truth, approval-button honesty, TUI auto-allow, settings persistence, README performance claims, SECURITY.md vs actual controls, and stale audit reports treated as current. Review is expanded to adjacent producer/consumer seams (ACP option order vs client selection; Settings UI vs `GoslingMode::default`; historical `docs/cloud` reports vs HEAD).

---

## Executive Verdict

No **Critical** defect is confirmed on this HEAD for the three assigned lenses. The previously ship-gating TUI auto-allow (`options[0]` = `AllowAlways`) is **repaired** in current source: interactive TUI blocks on `PermissionPrompt`, and `--text` declines unless `--yes`. Desktop no longer paints a missing tool response as green success.

Confirmed High/Medium defects remain on **approval-button truth**, **settings persistence vs backend default**, **bulk Always-Allow partial commit**, **undiagnosable tool errors**, and **stale audit/README posture**. The most dangerous operator lie is that Chat Settings highlights **Autonomous** when `GOSLING_MODE` is unset while the Rust default is **SmartApprove** — a confirmatory click persists Auto and drops the default approval gate. The most dangerous documentation lie is that `docs/cloud/99-master-report.md` and `docs/cloud/audit-workflow-gui.md` still describe TUI auto-allow and default Auto as current; those reports are 2026-07 evidence, not HEAD.

Do not treat historical `docs/cloud/` reports as the live security posture. Patch the settings default and the stale-report banners before any v1.0.0 claim that “approval gating is repaired.” Desktop/TUI were not launched in this run; findings are `source-evidenced` unless noted.

---

## Scope

- Repository / branch / commit: `gosling` `main` @ `073d19428509ea6eb317924b1856a1fe7e9002c8`
- Orientation: `docs/cloud/2026-08-15-orientation.md` (historical reports are seeds, not current verdicts)
- Skills: `audit-workflow-gui` v3.1, `audit-design-webapp` v0.3, `audit-compliance-posture` v3.2
- Files/directories inspected (prioritized sample):
  - `ui/text/src/tui.tsx`, `ui/text/src/components/PermissionPrompt.tsx`, `ui/text/src/toolcall.tsx`
  - `ui/desktop/src/components/ToolApprovalButtons.tsx` (+ test), `ToolCallWithResponse.tsx` (+ test), `ToolCallConfirmation.tsx`, `ToolCallStatusIndicator.tsx`, `GoslingMessage.tsx`, `ElicitationRequest.tsx`
  - `ui/desktop/src/acp/permissionRequests.ts`, `ui/desktop/src/acp/adapter/tools.ts`, `ui/desktop/src/preload.ts`, `ui/desktop/src/main.ts` settings IPC, `ui/desktop/src/utils/settings.ts`
  - `ui/desktop/src/components/settings/mode/*`, `chat/ChatSettingsSection.tsx`, `chat/SpellcheckToggle.tsx`, `security/SecurityToggle.tsx`, `SettingsView.tsx`
  - `ui/desktop/src/theme-init.ts`, `contexts/ThemeContext.tsx`, `theme/theme-tokens.ts`, `index.html`
  - `crates/gosling-cli/src/session/mod.rs`, `crates/gosling-providers/src/gosling_mode.rs`
  - `crates/gosling/src/acp/server.rs`, `crates/gosling/src/acp/common.rs`, `crates/gosling/src/tool_inspection.rs`, `crates/gosling/src/permission/permission_inspector.rs`, `crates/gosling/src/config/permission.rs`
  - `README.md`, `SECURITY.md`, `RELEASE_CHECKLIST.md`, `documentation/docs/release-notes/v1.0.0.md`
  - `docs/cloud/audit-workflow-gui.md`, `docs/cloud/audit-design-webapp.md`, `docs/cloud/audit-compliance-posture.md`, `docs/cloud/99-master-report.md`, `docs/cloud/2026-08-12-live-all-scenarios-playtest.md`, `.giles/repo.yaml`
- Commands/tests run: none (static read-only). Oracle-integrity note: no fresh-process UI/CLI run; no in-process fixture was treated as production evidence.
- Effort budget: ~90 targeted reads/greps across the named surfaces. Bought: full WFG-001..015 and CMP-001..015 inventory, six design gates scored with coverage caveats, re-verification of every 2026-07 WFG finding cited in `docs/cloud/audit-workflow-gui.md`.
- Constraints: no source edits; no app launch; contrast/keyboard/responsive not measured live. Behavior gates (Design 1/4/5) cap at partial coverage.

---

## Draft Prompt Assessment

- **Intended mission:** operator-truth + GUI design + honest posture at this HEAD, with explicit focus on fake success, approval buttons, TUI auto-allow, settings persistence, README perf, SECURITY.md, stale audits.
- **Under-specified:** whether “approval button truth” includes session-mode vs default-mode, bulk extension allow, and CLI/TUI option-set parity. Those were added.
- **Overly narrow if followed literally:** desktop-only would miss TUI `--text --yes` and CLI planner Auto persist.
- **Assumptions challenged:** that prior `docs/cloud/audit-workflow-gui.md` findings still hold; that README v1.0.0 language is the running version; that SECURITY.md is the enforcement contract.

---

## Surface Inventory

| Surface | Actor | Input/Trigger | State/Output | Boundary | Reviewed |
|---|---|---|---|---|---|
| Desktop `ToolApprovalButtons` | Operator | Allow Once / Always Allow / Always Allow all ext / Deny | ACP `requestPermission` resolve + optional `setToolPermissions` | Human gate; persist AlwaysAllow | Yes |
| Desktop `ToolCallConfirmation` | Operator | Approve without inline card | Name + first arg summary + buttons | Disclosure at decision point | Yes |
| Desktop tool result card | Operator | Stream complete / toolResponse | Status dot + results | Shown status vs backend result | Yes |
| Desktop Chat Settings Mode | Operator | Radio row click | `upsert('GOSLING_MODE')` | Default mode for *new* sessions | Yes |
| Desktop session ModeSwitcher | Operator | Dropdown | `setSessionMode` | Per-session mode | Yes |
| Desktop theme / spellcheck / settings IPC | Operator | Toggle | electron-store + leftover localStorage | Persistence truth | Yes |
| Desktop session delete/import | Operator | Confirm / file pick | ACP delete/import + toasts | Destructive + fake success | Yes (sampled) |
| TUI `requestPermission` (interactive) | Operator | Keyboard on `PermissionPrompt` | Selected `optionId` | Human gate | Yes |
| TUI `--text` / `--yes` | Operator / script | Permission request, no TTY prompt | reject_once or allow_once | Fail-closed vs opt-in | Yes |
| CLI interactive confirm | Operator | cliclack select | `Permission` to agent | Human gate; hides Always Allow on security prompt | Yes |
| CLI non-interactive Approve/SmartApprove | Script | Tool confirmation | Process error, no auto-allow | Fail-closed | Yes |
| CLI plan-act | Operator | Confirm act-on-plan | Temporarily writes `GoslingMode::Auto` to global config | Approval gate | Yes |
| README / SECURITY / release notes | Reader | Claims | Belief about version, perf, controls | Posture honesty | Yes |
| `docs/cloud/*` historical audits | Reader / later auditor | Findings as if current | False current posture | CMP-015 | Yes |
| Electron a11y / semantics / tokens | User | Keyboard, SR, theme | Usable GUI | Gates 2–5 | Sampled |

---

## Workflow Truth Table

| View/Control | Backend Effect | Shown Status | Actual Status | Feedback On Failure | Bypass If Disabled? |
|---|---|---|---|---|---|
| TUI interactive permission | Forwards chosen `optionId` | Prompt + cursor | Operator keypress | Esc → cancelled | No — blocks on prompt |
| TUI `--text` without `--yes` | Prefer `reject_once` | stderr declined | Declined | stderr reason | N/A |
| TUI `--text --yes` | Prefer `allow_once`; fallback `allow_always` then `options[0]` | No prompt | Auto allow-once (usually) | none | Opt-in only |
| Desktop Allow Once / Deny | `resolveAcpPermissionRequest` | “Allowed once” / “Denied once” if `true` | ACP selected kind | Stale line if false; **throw swallowed** | Always Allow hidden when `prompt` set; ACP still offers it |
| Desktop Always Allow all ext | Resolve current call **then** `setToolPermissions` | Error if persist fails | Current call already AlwaysAllow | Error text; buttons remain | Resolve already committed |
| Desktop tool no-response | — | `unknown` → pending/amber | Incomplete | Distinct from success | n/a |
| Desktop tool `status:error` | — | Red dot | Error | **error string not rendered** | n/a |
| Settings Default Mode (unset) | Backend `SmartApprove` | **Autonomous selected** | SmartApprove | none | Clicking highlighted Auto persists Auto |
| Theme first paint | electron-store theme | localStorage / system | Durable setting may differ | console warn | n/a |
| CLI Approve/SmartApprove headless | Abort | Error | Error | Error text | No |
| CLI plan-act | `set_gosling_mode(Auto)` then restore | “act on this plan” | Global Auto during (and after crash) | Restore on success path | Temporarily disables gate |

---

## Posture Inventory

| Framework/Control | Evidence Source | Evidence Grade | Mapping Confidence | Gap Language | Enforcement? |
|---|---|---|---|---|---|
| Fail-closed inspector errors | `tool_inspection.rs:130-154` + unit test | Strong (code + test) | High | README states the mechanism | Runtime, yes |
| Default mode SmartApprove | `gosling_mode.rs:28-29` `#[default]` | Strong | High | README “Safer Defaults” is vague | Runtime default |
| SECURITY.md precautions | `SECURITY.md:1-16` | Advisory only | High | “consider”, “if possible” | No |
| README footprint table | `README.md:31-41` dated 2026-07-04 vs v0.0.5 | Historical measurement | High | Explicitly “not v1.0.0 benchmark claims” | No |
| gosling version identity | README v1.0.0 vs `Cargo.toml`/`package.json` `0.1.0` | Contradicted | High | Mixed marketing vs build | Checklist wants 1.0.0 |
| Historical WFG TUI auto-allow | `docs/cloud/audit-workflow-gui.md`, `99-master-report.md` | **Stale vs HEAD** | High | Written as current | None; misleads auditors |
| Giles repo.yaml | `.giles/repo.yaml` schema 1.0 / canon 1.4 | Advisory mirror | High | No certification language | No |
| Prompt-injection toggle | `SecurityToggle` + config default-off historically | UI exists; default not re-traced here | Med | SECURITY.md does not claim it is on | Config |

---

## Boundary Map

| Surface | Intended Boundary | Enforced At | Status |
|---|---|---|---|
| Tool execution | Human approval unless Auto / cached AlwaysAllow | Core inspectors + client `requestPermission` | Holds in core and interactive TUI/Desktop; Desktop default-mode UI can persist Auto |
| Security-inspector prompt | Hide persistent grant | CLI + Desktop hide Always Allow when `prompt`; TUI shows all options | Partial / client-divergent |
| Settings persistence | Durable electron-store / ACP config | `set-setting` IPC; `upsert` | Shadowed by leftover localStorage on read; Mode UI default wrong |
| Shown tool success | Verified tool result | `deriveLoadingStatus` | Holds for missing response; fails for error-string display |
| Public posture | Claim ≤ evidence | README hedges perf; `docs/cloud` historical files do not | Stale reports overclaim |
| Release identity | One version | Checklist wants 1.0.0; tree is 0.1.0 | Ambiguous |

---

## Findings Table

| ID | Severity | Confidence | Evidence Basis | Domain | Title | Patch Priority | Blast Radius | Complexity | Cost | Nominal Agent |
|---|---|---|---|---|---|---|---|---|---|---|
| WFG-GOS-001 | High | Confirmed | source-evidenced | Workflow-GUI | Settings Default Mode shows Autonomous when backend default is SmartApprove | 1 | Workflow | operator_ux | S | codex |
| WFG-GOS-002 | High | Confirmed | source-evidenced | Workflow-GUI | Bulk “Always Allow all ext tools” resolves the live call before persist; persist failure is a lie | 1 | Local | operator_ux | S | codex |
| WEB-GOS-001 | High | Confirmed | source-evidenced | Workflow-GUI / Gate 1 | Always Allow is equal-weight to Allow Once; Deny is the faintest control | 2 | Workflow | operator_ux | S | claude |
| WFG-GOS-003 | Medium | Confirmed | source-evidenced | Workflow-GUI | Desktop drops the tool `error` string — operator cannot diagnose | 2 | Workflow | operator_ux | S | codex |
| WFG-GOS-004 | Medium | Confirmed | source-evidenced | Workflow-GUI | Approval click throw path is silent | 3 | Workflow | local_guardrail | XS | codex |
| WFG-GOS-005 | Medium | Confirmed | source-evidenced | Workflow-GUI | `getSetting` leftover localStorage permanently shadows electron-store | 2 | Workflow | persistence_recovery | S | codex |
| WFG-GOS-006 | Medium | Confirmed | source-evidenced | Workflow-GUI | TUI still offers Allow-always on security prompts; Desktop/CLI hide it | 2 | Workflow | operator_ux | S | claude |
| WFG-GOS-007 | Medium | Confirmed | source-evidenced | Workflow-GUI | CLI plan-act persists global Auto, restores only on the success path | 2 | Service | workflow_protocol | M | claude |
| WFG-GOS-008 | Low | Confirmed | source-evidenced | Workflow-GUI | `--yes` comment says never `allow_always`; code falls back to it | 4 | Workflow | local_guardrail | XS | codex |
| WFG-GOS-009 | Medium | Confirmed | source-evidenced | Workflow-GUI | Theme/spellcheck optimistic UI; save failure leaves a lying control | 3 | Workflow | operator_ux | S | codex |
| WEB-GOS-002 | High | Confirmed | source-evidenced | Workflow-GUI / Gate 1 | Approval disclosure is name + truncated first-line arg | 2 | Local | operator_ux | S | claude |
| WEB-GOS-003 | Medium | Confirmed | source-evidenced | Gate 3/4 | `index.html` has no `lang`; approval/mode controls are weakly named | 4 | Workflow | operator_ux | S | claude |
| WEB-GOS-004 | Low | Confirmed | source-evidenced | Gate 1 | `ConfigureApproveMode` is dead — `isDialogOpen` is never set | 5 | Workflow | operator_ux | XS | codex |
| CMP-GOS-001 | High | Confirmed | source-evidenced | Compliance-Posture | Stale `docs/cloud` audits still assert TUI auto-allow and default Auto as current | 1 | Repo | operator_ux | S | claude |
| CMP-GOS-002 | Medium | Confirmed | source-evidenced | Compliance-Posture | Version identity split: docs say v1.0.0, tree ships 0.1.0 | 3 | Repo | governance_decision | S | human-owner |
| CMP-GOS-003 | Low | Confirmed | source-evidenced | Compliance-Posture | README perf table is a 2026-07-04 / v0.0.5 baseline, still in the current product README | 5 | Repo | operator_ux | XS | claude |

---

## Detailed Findings

### WFG-GOS-001: Settings Default Mode shows Autonomous when backend default is SmartApprove

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `crates/gosling-providers/src/gosling_mode.rs:24-33` — `#[default] SmartApprove`
- `crates/gosling-providers/src/gosling_mode.rs:41-43` — unit test `default_mode_is_smart_approve`
- `ui/desktop/src/components/settings/mode/ModeSection.tsx:8` — `useState('auto')`
- `ui/desktop/src/components/settings/mode/ModeSection.tsx:22-26` — only overwrites when `config.GOSLING_MODE` is truthy
- `ui/desktop/src/components/settings/mode/ModeSelectionItem.tsx:94` — row `onClick` always calls `handleModeChange(mode.key)` even if already checked
- `ui/desktop/src/components/settings/chat/ChatSettingsSection.tsx:14-19` — labels this as “Default Mode” for new sessions
- `crates/gosling-cli/src/cli.rs:42-44` — invalid mode falls back to `smart_approve`

Observed behavior:
- A fresh or unset `GOSLING_MODE` is SmartApprove in the agent. The Settings radio paints Autonomous as selected. Clicking that already-highlighted row upserts `'auto'` and turns off the default approval gate for future sessions.

Expected boundary:
- Shown default must equal `GoslingMode::default()` (`smart_approve`) until a stored value exists. Confirming the highlighted option must be a no-op.

Failure mechanism:
- UI initial state is a hardcoded `'auto'` leftover from when Auto was the Rust default. Absence of a config key is treated as “keep auto” instead of “use backend default.”

Break-it angle:
- New install → open Settings → Chat → Autonomous is filled → click it to “leave it” → `GOSLING_MODE=auto` is written → new chats auto-execute tools.

Impact:
- Operator believes the default is already Autonomous *or* believes they did not change policy. Either way the human gate disappears.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible + process (subsequent tool auto-exec)
- Reversibility: compensatable (set mode back)
- Operator visibility: UI-visible-but-wrong
- Rerun safety: unsafe (persists)

Adjacent failure modes:
- WEB-GOS-001 (easy Auto / Always Allow clicks)
- CMP-GOS-001 (stale reports still say default *is* Auto; current UI makes that look true)

Recommended mitigation:
- Remediation patterns: default-from-backend; no-op if already selected.
- Minimal repair: `useState('smart_approve')`; treat missing `GOSLING_MODE` as `smart_approve`; skip upsert when `newMode === currentMode`.
- Local guardrail: test that unset config renders Smart selected and does not write Auto on remount.
- Behavior test: unset key → Smart checked; click Smart → no upsert; click Auto → upsert `auto`.

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Cost drivers: 1 component + test
- Nominal implementation agent: codex
- Rationale: local default + click guard.

Validation:
- Assert Settings radio selection equals `GoslingMode::default()` when config omits `GOSLING_MODE`.

Non-goals:
- Do not change Auto-mode semantics themselves.

---

### WFG-GOS-002: Bulk “Always Allow all ext tools” resolves the live call before persist; persist failure is a lie

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/components/ToolApprovalButtons.tsx:142-166`
- `ui/desktop/src/components/ToolApprovalButtons.tsx:148-151` — `resolveAcpPermissionRequest(..., 'always_allow')` **before** `listTools` / `setToolPermissions`
- `ui/desktop/src/acp/permissionRequests.ts:35-52` — resolve deletes the pending request and sends the ACP outcome immediately
- `ui/desktop/src/components/ToolApprovalButtons.test.tsx:105-143` — encodes order `resolve` → `listTools` → `setToolPermissions` as the desired contract
- On persist throw: `setApprovalError(failedToAllowExtension)` while the current call is already AlwaysAllow

Observed behavior:
- Clicking “Always Allow all {extension} tools” first grants AlwaysAllow for the in-flight tool (ACP + permission store via the agent), then tries to persist the rest. If `listTools`/`setToolPermissions` fails, the UI says the action failed and leaves the buttons up. The current tool has already been approved and persisted as AlwaysAllow.

Expected boundary:
- Either persist the extension grant first (or transactionally) and only then resolve the live request, or surface **partial success**: “This tool was always-allowed; extension-wide persist failed.”

Failure mechanism:
- Liveness check was moved before the bulk mutation to avoid stale-request side effects (good for the stale path, test at `:77-103`). That same resolve is a state mutation, not a check.

Break-it angle:
- Fail `setToolPermissions` (network/ACP error). Operator retries Deny; request is already gone (stale). Tool already ran.

Impact:
- False failure after a standing grant. Operator may assume the tool did not run.

Operational impact:
- Blast radius: Local
- Side-effect class: process / file / network (whatever the tool does) + persisted AlwaysAllow
- Reversibility: compensatable for the grant; tool effects may be irreversible
- Operator visibility: UI-visible-but-wrong
- Rerun safety: unsafe

Adjacent failure modes:
- WFG-GOS-004 (other silent approval errors)
- WFG-015 (bulk set is server-listed tools, not a displayed selection — label is honest if persist succeeds)

Recommended mitigation:
- Remediation patterns: check-then-mutate without committing; split “liveness peek” from resolve.
- Minimal repair: add `isAcpPermissionRequestPending` (already exists at `permissionRequests.ts:31-33`) as the pre-check; call `resolve` only after `setToolPermissions` succeeds; if persist fails after a forced resolve, show partial-success copy.
- Behavior test: `setToolPermissions` reject → no “Always allowed” label, **and** either no ACP resolve or an explicit partial-success message.

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Cost drivers: button handler + existing test rewrite
- Nominal implementation agent: codex
- Rationale: local control-flow change; product copy for partial success.

Validation:
- Test persist-fail does not report blanket failure after AlwaysAllow already applied, or does not resolve until persist succeeds.

Non-goals:
- Do not remove the extension-wide grant feature.

---

### WEB-GOS-001: Always Allow is equal-weight to Allow Once; Deny is the faintest control

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI (Gate 1 layout/hierarchy; security control)

Evidence:
- `ui/desktop/src/components/ToolApprovalButtons.tsx:194-226`
- Allow Once `variant="secondary"` (`:197`)
- Always Allow `variant="secondary"` (`:204`) — identical
- Always Allow all ext `variant="secondary"` (`:213`)
- Deny `variant="outline"` (`:222`) — least prominent
- CLI does put Always Allow in the list but **omits it when a security prompt is present** (`crates/gosling-cli/src/session/mod.rs:1928-1952`)

Observed behavior:
- Four same-size pills; the two durable grants are filled like the one-time grant; Deny is outline. Prompt-fatigue clicks land on the left filled buttons, including persistent grants.

Expected boundary:
- Gate 1: one primary action; destructive / high-blast actions separated. Safe default (Deny or Allow Once) should dominate; Always Allow needs extra friction.

Failure mechanism:
- Consequence is not encoded in visual weight.

Break-it angle:
- Rapid approval stream (map capped at 500, `ToolApprovalButtons.tsx:75`) → habituated left-click → standing grant.

Impact:
- Accidental AlwaysAllow of shell/file/network tools.

Operational impact:
- Blast radius: Local
- Side-effect class: process / file / network
- Reversibility: compensatable for the grant
- Operator visibility: UI-visible
- Rerun safety: n/a

Adjacent failure modes:
- WEB-GOS-002 (approving without seeing the command)
- WFG-GOS-006 (TUI still shows Allow-always under security prompts)

Recommended mitigation:
- Remediation pattern: consequence-ranked actions.
- Minimal repair: Deny or Allow Once as primary; Always Allow outline / separated / explicit “skips future prompts” copy.
- Behavior test: snapshot variants; Always Allow not styled identically to Allow Once.

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Cost drivers: 1 component + i18n + snapshot
- Nominal implementation agent: claude
- Rationale: security-affordance copy needs product judgment.

Validation:
- DOM assert deny/allow-once visual weight ≥ always-allow.

Non-goals:
- Backend permission model.

---

### WFG-GOS-003: Desktop drops the tool `error` string — operator cannot diagnose

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/acp/adapter/tools.ts:76-79` — ACP `failed` → `{ status: 'error', error: toolError(update) }`
- `ui/desktop/src/components/ToolCallWithResponse.tsx:154-157` — `getToolResultContent` returns `[]` unless `status === 'success'`
- `ui/desktop/src/components/ToolCallWithResponse.tsx:567-570` — `toolResults` only on success
- `ui/desktop/src/components/ToolCallWithResponse.tsx:889-904` — only `toolResults.map`; no `toolResult.error` branch
- Status mapping at `:792-805` shows a red `error` dot only

Observed behavior:
- Failed tools get a red indicator and the tool label. The serialized error string is discarded.

Expected boundary:
- WFG-013: failure yields an actionable reason.

Failure mechanism:
- Content extraction is success-gated; the error variant has no renderer.

Break-it angle:
- Extension throws / ACP `failed`. Operator sees a red dot and retries blind.

Impact:
- Undiagnosable failures; mis-attribution.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: red-dot-only
- Rerun safety: safe

Adjacent failure modes:
- WFG-GOS-004 (approval errors also under-surfaced)

Recommended mitigation:
- Minimal repair: when `status === 'error'`, render `error` in the card.
- Behavior test: error-variant fixture shows the message.

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Nominal implementation agent: codex
- Cost drivers: 1 renderer + test
- Rationale: local render branch.

Validation:
- Assert the error string is in the document.

Non-goals:
- Change ACP adapter status mapping.

---

### WFG-GOS-004: Approval click throw path is silent

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/components/ToolApprovalButtons.tsx:130-139`
- `catch` only `console.error('Error confirming tool action:', err)` — no `setApprovalError`
- Stale `false` path *does* set `staleApprovalRequest` (`:134-136`)

Observed behavior:
- If `resolveAcpPermissionRequest` throws, the click does nothing visible.

Expected boundary:
- Every approval action yields a visible result.

Failure mechanism:
- Throw is treated as log-only; buttons remain live.

Break-it angle:
- Inject a throw from resolve. Operator re-clicks; no diagnosis.

Impact:
- Hidden failure on the security control.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: silent (console-only)
- Rerun safety: unknown

Adjacent failure modes:
- WFG-GOS-002

Recommended mitigation:
- Set `approvalError` in the catch, same as stale.
- Test: mocked throw → alert text.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Nominal implementation agent: codex
- Cost drivers: 1 catch + test
- Rationale: already have the error element (`:228-232`).

Validation:
- Force throw; assert `role="alert"` text.

Non-goals:
- Change resolve implementation.

---

### WFG-GOS-005: `getSetting` leftover localStorage permanently shadows electron-store

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/preload.ts:17-24` — “lazy migration” map (`theme`, `useSystemTheme`, `responseStyle`, `showPricing`, `seenAnnouncementIds`)
- `ui/desktop/src/preload.ts:236-248` — `getSetting` returns parsed localStorage if present and **does not write it through or clear it**
- `ui/desktop/src/preload.ts:255-272` — same for `getSettings`
- `ui/desktop/src/preload.ts:287-293` — only `setSetting` clears localStorage
- `ui/desktop/src/theme-init.ts:8-15` — first paint reads **only** localStorage, never electron-store
- Durable store is `ipcMain.handle('get-setting')` → `getSettings()` (`ui/desktop/src/main.ts:2036-2039`)

Observed behavior:
- Comment says lazy migration. Implementation is “localStorage wins forever until that key is written via `setSetting`.” First paint can disagree with the durable theme. Another window / main-process write to electron-store is invisible while leftover LS exists.

Expected boundary:
- Read path migrates LS → store → delete LS, or ignores LS after store exists.

Failure mechanism:
- Producer (electron-store) and consumer (preload getSetting / theme-init) disagree on source of truth.

Break-it angle:
- Leave `localStorage.theme=light` from an old build; set dark via a path that writes only electron-store (or a second profile). UI stays light.

Impact:
- Settings appear not to persist, or persist only in the renderer partition.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: reversible (clear LS or toggle theme)
- Operator visibility: UI-visible-but-wrong
- Rerun safety: safe

Adjacent failure modes:
- WFG-GOS-009 (optimistic theme/spellcheck)

Recommended mitigation:
- On get: if LS present, `setSetting` (or IPC write) then `removeItem`.
- `theme-init` cannot await IPC; accept FOUC or inject the store value from main at window create.
- Test: LS + store disagree → returned value is store after one get, LS gone.

Implementation assessment:
- Complexity: persistence_recovery
- Cost: S
- Nominal implementation agent: codex
- Cost drivers: preload + theme-init + test
- Rationale: two readers, one writer.

Validation:
- Unit test the migrate-on-read path.

Non-goals:
- Redesign the settings schema.

---

### WFG-GOS-006: TUI still offers Allow-always on security prompts; Desktop/CLI hide it

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- Core always sends all four options, AllowAlways first: `crates/gosling/src/acp/server.rs:2306-2310`
- Desktop hides Always Allow when `prompt` is set: `ToolApprovalButtons.tsx:201-209`
- CLI omits Always Allow when `security_prompt` is Some: `crates/gosling-cli/src/session/mod.rs:1928-1952`
- TUI `PermissionPrompt` maps **every** `request.options` (`ui/text/src/components/PermissionPrompt.tsx:102-111`) including `allow_always` (`:16`)
- Default cursor prefers `allow_once` (`PermissionPrompt.tsx:50-57`) — good — but Enter on navigated Allow-always still persists

Observed behavior:
- The same security-inspector prompt that Desktop/CLI treat as “no standing grant” is a fully offered Allow-always in the TUI.

Expected boundary:
- Clients must agree on whether a security finding forbids persistent grant. WFG-002/003: UI/CLI parity.

Failure mechanism:
- TUI is a faithful option-list renderer; Desktop/CLI apply an extra policy the server does not encode (they still *send* AllowAlways).

Break-it angle:
- Trigger an egress/injection finding. Desktop user cannot Always Allow. TUI user arrows to “Allow always for this tool” and persists it.

Impact:
- Security-prompt policy is client-optional. TUI operators can persist a grant the other UIs refuse to offer.

Operational impact:
- Blast radius: Local
- Side-effect class: process + persisted AlwaysAllow
- Reversibility: compensatable
- Operator visibility: UI-visible
- Rerun safety: unsafe (persists)

Adjacent failure modes:
- WEB-GOS-001
- CMP-GOS-001 (old report said TUI auto-selected AllowAlways; now it is offered, not auto)

Recommended mitigation:
- Prefer server-side: omit `AllowAlways` when a security prompt is attached. Then every client matches.
- Local TUI guard: hide/disable `allow_always` when prompt/content indicates a security finding.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: S
- Nominal implementation agent: claude
- Cost drivers: ACP option construction + TUI filter + tests
- Rationale: producer/consumer pair; fix the producer if possible.

Validation:
- Security-prompt request contains no `allow_always`, or TUI does not render it.

Non-goals:
- Remove Allow-always for ordinary (non-security) confirms.

---

### WFG-GOS-007: CLI plan-act persists global Auto, restores only on the success path

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `crates/gosling-cli/src/session/mod.rs:1075-1098`
- `:1081-1082` — `config.set_gosling_mode(GoslingMode::Auto)` on the global Config
- `:1092-1093` — then `process_agent_response`
- `:1097-1098` — restore previous mode only after that call returns

Observed behavior:
- Confirming “act on this plan” writes Auto to durable config for the duration of the run. A crash, kill, or early return between set and restore leaves the user in Auto. Concurrent CLI/Desktop sessions reading global config also see Auto.

Expected boundary:
- Acting on a plan may use Auto **in-process** without mutating the stored default. Restore must be fail-safe (defer/drop guard).

Failure mechanism:
- Session-scoped intent is written to the process-global persisted mode.

Break-it angle:
- Confirm plan-act; kill the process mid-run; next `gosling` session is Auto with no approval prompts.

Impact:
- Silent standing change of the approval gate after a planner workflow.

Operational impact:
- Blast radius: Service (all sessions sharing that config root)
- Side-effect class: file (config) + process
- Reversibility: compensatable if noticed
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- WFG-GOS-001 (another Auto-default lie)

Recommended mitigation:
- Override agent mode in memory only; do not `set_gosling_mode` on Config. If persist is required, use a drop-guard restore.
- Test: simulated error after set still restores; or no persist occurs.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Nominal implementation agent: claude
- Cost drivers: CLI session + config tests
- Rationale: crash path needs an explicit restore protocol.

Validation:
- After a failed plan-act, `get_gosling_mode()` equals the pre-plan value.

Non-goals:
- Remove plan-act.

---

### WFG-GOS-008: `--yes` comment says never `allow_always`; code falls back to it

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/text/src/tui.tsx:1366-1381`
- Comment: “never ‘allow always’”
- Code: `allow_once ?? allow_always ?? options[0]`
- Server always includes `allow_once` today (`server.rs:2306-2310`), so the fallback is latent for gosling’s own server

Observed behavior:
- Documented policy and code disagree. Against gosling-core the `--yes` path is AllowOnce. Against a reduced option set it can persist AlwaysAllow.

Expected boundary:
- Non-interactive opt-in is AllowOnce or cancel — never AlwaysAllow.

Failure mechanism:
- Defensive fallback walks toward the most persistent grant.

Break-it angle:
- Point `--text --yes` at an ACP agent that omits `allow_once`.

Impact:
- Standing grant from a “one-shot yes” flag.

Operational impact:
- Blast radius: Workflow
- Side-effect class: process
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- WFG-GOS-006

Recommended mitigation:
- If no `allow_once`, cancel. Delete the `allow_always` fallback. Align the comment.
- Test: options without `allow_once` → cancelled.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Nominal implementation agent: codex
- Cost drivers: 1 function + test
- Rationale: comment already states the policy.

Validation:
- Fixture options=`[allow_always]` + `--yes` → cancelled, not selected.

Non-goals:
- Change `--yes` AllowOnce behavior when present.

---

### WFG-GOS-009: Theme/spellcheck optimistic UI; save failure leaves a lying control

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/contexts/ThemeContext.tsx:72-88` — `setUserThemePreferenceState` / `setResolvedTheme` **before** `setSetting`; catch is `console.warn` only
- `ui/desktop/src/components/settings/chat/SpellcheckToggle.tsx:18-31` — `useState(true)` before load; `setEnabled(checked)` then `setSpellcheck` with no catch
- Spellcheck copy says restart required (`:12`) — good — but a failed write still shows the new value

Observed behavior:
- Theme and spellcheck switches move immediately. If IPC persist fails, the control stays on the new value. Theme also broadcasts to other windows (`:91-95`) after a failed save.

Expected boundary:
- Persist then paint, or roll back the control and toast on failure.

Failure mechanism:
- Shown state is bound to the click, not the store postcondition.

Break-it angle:
- Fail `set-setting` / `set-spellcheck`. Operator thinks dark/spellcheck-on is saved; next launch reverts (or LS shadow, WFG-GOS-005).

Impact:
- Fake settings persistence.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- WFG-GOS-005

Recommended mitigation:
- Await persist; on failure revert + toast.
- Spellcheck: initialize from store without assuming `true`.
- Test: rejected setSetting leaves previous theme selected.

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Nominal implementation agent: codex
- Cost drivers: ThemeContext + SpellcheckToggle + tests
- Rationale: two local handlers.

Validation:
- Mock persist reject → previous value shown + error toast.

Non-goals:
- Theme token redesign.

---

### WEB-GOS-002: Approval disclosure is name + truncated first-line arg

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI (Gate 1 AI-agent review)

Evidence:
- `ui/desktop/src/components/ToolCallConfirmation.tsx:23-42` — first matching key of `path|file|command|query|url|…`, first line only, cap 140 + ellipsis
- `ToolCallConfirmation.tsx:72-76` — detail in `truncate` CSS
- `ui/desktop/src/components/ToolCallWithResponse.tsx:543-551` — default `responseStyle==='concise'` collapses tool details
- `ToolCallWithResponse.tsx:826` — label `truncate`
- TUI `PermissionPrompt.tsx:26-38, 93-99` — JSON summary truncated to width, `wrap="truncate"`

Observed behavior:
- Standalone confirmation improved since 2026-07 (no longer name-only) but still hides a hostile suffix after 140 chars / first line / CSS truncate. Concise style keeps full args behind an expander.

Expected boundary:
- Gate 1 §10: the concrete command/path/URL is reviewable at the decision point without an extra click, and without clipping the dangerous tail.

Failure mechanism:
- Progressive disclosure applied to the security payload.

Break-it angle:
- Benign prefix + `&& curl … | sh` past 140 chars or after a newline.

Impact:
- Uninformed approval.

Operational impact:
- Blast radius: Local
- Side-effect class: process
- Reversibility: often irreversible
- Operator visibility: incomplete
- Rerun safety: n/a

Adjacent failure modes:
- WEB-GOS-001

Recommended mitigation:
- Always show full `command`/`path`/`url` in a wrap/scroll region on the approval card; do not CSS-truncate the security payload.
- Test: long command fully present in the approval DOM (not `…`).

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Nominal implementation agent: claude
- Cost drivers: confirmation + concise-style exception + Ink width budget
- Rationale: disclosure policy, not a new backend field.

Validation:
- Fixture command length > 140 appears in full on the approval card.

Non-goals:
- Change default response style for completed (non-approval) tool cards.

---

### WEB-GOS-003: `index.html` has no `lang`; approval/mode controls are weakly named

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture / Gate 3–4 (WCAG 3.1.1, 4.1.2)

Evidence:
- `ui/desktop/index.html:2` — `<html>` with no `lang`
- `ModeSelectionItem.tsx:91-95` — whole row is a `div` with `onClick`; radio is `sr-only` (`:116-123`)
- Gear button for permission rules (`:107-114`) has no accessible name
- `ToolApprovalButtons` uses native `<Button>` (keyboard OK) but no group label / no default focus on Deny
- `ToolCallStatusIndicator.tsx:26-36` — status is a color dot; `aria-label` exists (`:45`) so color is not the only channel for that widget
- Contrast was **not measured** (below Gate 4 floor for a Fail on contrast)

Observed behavior:
- Document language undeclared. Mode change is a clickable div. Permission-rules gear is an unnamed button.

Expected boundary:
- WCAG 2.1 AA: `lang`, name/role/value, visible focus. Gate 3 semantic controls.

Failure mechanism:
- Div-as-button and unlabeled icon button.

Break-it angle:
- Screen reader on Settings → “button” with no name next to Manual/Smart.

Impact:
- Settings and the security-adjacent gear are harder or impossible to operate via AT.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: AT-only
- Rerun safety: safe

Adjacent failure modes:
- WEB-GOS-001 (focus order / default action)

Recommended mitigation:
- `lang="en"` (or active locale). Mode row as `<label>` for the radio. `aria-label` on the gear.
- Test: axe/testing-library names.

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Nominal implementation agent: claude
- Cost drivers: markup + i18n names
- Rationale: a11y names, not a redesign.

Validation:
- `document.documentElement.lang` set; gear has an accessible name.

Non-goals:
- Full WCAG certification.

---

### WEB-GOS-004: `ConfigureApproveMode` is dead — `isDialogOpen` is never set

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI (Gate 1; WFG-012 dead step)

Evidence:
- `ui/desktop/src/components/settings/mode/ModeSelectionItem.tsx:83` — `isDialogOpen` state
- `:134-142` — renders `ConfigureApproveMode` only when `isDialogOpen`
- No `setIsDialogOpen(true)` anywhere in the repo (grep: only the useState and the conditional)
- Gear opens `PermissionRulesModal` instead (`:106-114`, `:146-149`)

Observed behavior:
- The configure-approve-mode dialog component exists and is imported but is unreachable.

Expected boundary:
- Either wire it or delete it. Dead workflow UI is a maintainability and IA hazard (operators looking at code/docs think a configure step exists).

Failure mechanism:
- Incomplete wiring after the gear was retargeted to permission rules.

Break-it angle:
- None at runtime; the step is simply skipped.

Impact:
- Dead code; possible doc/code drift.

Operational impact:
- Blast radius: Local
- Side-effect class: none
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- None material.

Recommended mitigation:
- Remove `ConfigureApproveMode` from this tree or open it from an explicit control.
- Test: unused export lint / storybook gone.

Implementation assessment:
- Complexity: operator_ux
- Cost: XS
- Nominal implementation agent: codex
- Cost drivers: delete or one click handler
- Rationale: dead UI.

Validation:
- Component unreferenced or reachable via a named control.

Non-goals:
- Redesign approve-mode configuration.

---

### CMP-GOS-001: Stale `docs/cloud` audits still assert TUI auto-allow and default Auto as current

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- `docs/cloud/99-master-report.md:83` — “Default `GoslingMode` = **Auto** ⇒ every tool auto-approved” with “lead-verified ✔”
- `docs/cloud/99-master-report.md:86` — “Tool inspectors **fail open**”
- `docs/cloud/99-master-report.md:90` — “**TUI silently auto-approves** every call by selecting `options[0]` = `AllowAlways`”
- `docs/cloud/audit-workflow-gui.md:38-50` — WFG-GSL-001 as a live High finding citing `tui.tsx:763-773` and `:1315-1325`
- Current HEAD contradicts all three:
  - default is SmartApprove (`gosling_mode.rs:28-29`)
  - inspector errors fail closed (`tool_inspection.rs:130-154`)
  - TUI interactive relays to `PermissionPrompt` (`tui.tsx:784-793`, `1182-1188`); `--text` declines (`tui.tsx:1390-1407`)
- `docs/cloud/2026-08-15-orientation.md:72-74` already warns historical reports are seeds
- `README.md:147` hedges historical audits; `README.md:135-137` still points readers at the **2026-07-20** playtest as “Release validation status”
- A later playtest exists: `docs/cloud/2026-08-12-live-all-scenarios-playtest.md` (61/4/37/8)

Observed behavior:
- The master report and the previous WFG lens read as current posture. An auditor or release owner who starts there will believe TUI auto-allows and default is Auto. That is exactly the failure this pass was asked to catch: **stale audit reports treated as current**.

Expected boundary:
- CMP-015: stale evidence is flagged. Historical reports must banner their commit and “superseded” status. README release-validation must not cite a 46/32/32 playtest as the live gate when a later pass exists and HEAD has repaired the cited defects.

Failure mechanism:
- Point-in-time reports live beside current docs without expiry. Strong wording (“lead-verified ✔”, present tense) outlives the code.

Break-it angle:
- New auditor reads `99-master-report.md` CLUSTER A and writes a security exception or a wrong patch.

Impact:
- Systemic false security/compliance posture (severity_matrix: High/Critical when it becomes the believed control state). Here it is High: it is repo-local and not an automated release gate, but it is the file later lenses are told to build on (`audit-workflow-gui.md:5`, `00-orientation.md`).

Operational impact:
- Blast radius: Repo
- Side-effect class: user-visible
- Reversibility: reversible (banner/archive)
- Operator visibility: silent unless dates are noticed
- Rerun safety: safe

Adjacent failure modes:
- CMP-GOS-002, CMP-GOS-003
- WFG-GOS-001 (UI still *looks* like default Auto, so the stale report appears confirmed)

Recommended mitigation:
- Remediation pattern: freshness banner + pointer to the 2026-08-15 orientation/master.
- Minimal repair: add a one-line header to `99-master-report.md`, `audit-workflow-gui.md`, `audit-design-webapp.md`, `audit-compliance-posture.md`: “Historical @ &lt;old SHA&gt;; not HEAD. Re-verify.” Point README release-validation at 2026-08-12/15.
- Do **not** silently rewrite historical ledgers.
- Test: docs lint that historical reports contain a superseded banner, or that README does not link them as current validation.

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Nominal implementation agent: claude
- Cost drivers: docs banners
- Rationale: wording/freshness only; no code.

Validation:
- Grep of `docs/cloud/99-master-report.md` shows a superseded banner and the original date/SHA.

Non-goals:
- Do not delete the 2026-07 evidence.

---

### CMP-GOS-002: Version identity split: docs say v1.0.0, tree ships 0.1.0

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- `Cargo.toml:11` — `version = "0.1.0"`
- `ui/desktop/package.json:4` — `"version": "0.1.0"`
- `README.md:84` — “What's included in gosling v1.0.0”
- `README.md:31` — benchmark artifact is “gosling v0.0.5 (commit `5b7d039`)”
- `README.md:139` — “Before publishing v1.0.0…”
- `RELEASE_CHECKLIST.md:1-8` — publish gate requires every artifact to report `1.0.0`
- `documentation/docs/release-notes/v1.0.0.md:9-11` — honestly says the page *prepares* notes and does not prove the tag exists

Observed behavior:
- Build identity is 0.1.0. Marketing/docs speak in v1.0.0 present tense in the README feature list, future tense in the checklist, and historical v0.0.5 in the benchmark. Old CMP-GSL-001 (0.0.1 vs 1.40.0) is **stale**; the split moved, it did not disappear.

Expected boundary:
- CMP-001: one authoritative version per artifact, labeled.

Failure mechanism:
- Release-prep prose landed in the root README before versions were bumped.

Break-it angle:
- Pin “gosling v1.0.0” from README; `gosling --version` is 0.1.0.

Impact:
- Provenance/version confusion; checklist cannot be honestly checked off on this commit.

Operational impact:
- Blast radius: Repo
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: UI-visible (About vs CLI vs README)
- Rerun safety: safe

Adjacent failure modes:
- CMP-GOS-003

Recommended mitigation:
- README: “current tree is 0.1.0; v1.0.0 is the planned release line.” Or bump versions when claiming v1.0.0.
- Docs check: README version tokens ⊆ `{Cargo.toml, package.json}` or are labeled “planned.”

Implementation assessment:
- Complexity: governance_decision
- Cost: S
- Nominal implementation agent: human-owner
- Cost drivers: product versioning decision + docs
- Rationale: whether to ship 0.1.0 or retag 1.0.0 is an owner call.

Validation:
- README does not present 1.0.0 as the running tree version unless `Cargo.toml` matches.

Non-goals:
- Do not invent a tag.

---

### CMP-GOS-003: README perf table is a 2026-07-04 / v0.0.5 baseline, still in the current product README

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- `README.md:29-41` — comparison date **2026-07-04**, gosling **v0.0.5** / `5b7d039`, goose v1.41.0 / `181cbbe`
- Explicit hedge: “These are historical baseline measurements, not v1.0.0 benchmark claims; rerun them before publishing current performance deltas.”
- Same README then lists current-product features under “What's included in gosling v1.0.0” (`:84-93`)

Observed behavior:
- The numbers are honestly caveated (better than the 2026-07 CMP report’s “unchanged functionality” claim, which is gone). They still sit in the primary product README next to current-tense claims, so readers treat −27% cold start as a live v1.0.0 fact.

Expected boundary:
- CMP-015 / CMP-003: historical measurements live in a dated note, not the top-level feature narrative, unless rerun on HEAD.

Failure mechanism:
- Well-hedged evidence in a high-authority location.

Break-it angle:
- Cite README deltas in a release blog without rerunning.

Impact:
- Stale performance posture.

Operational impact:
- Blast radius: Repo
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: UI-visible
- Rerun safety: safe

Adjacent failure modes:
- CMP-GOS-002

Recommended mitigation:
- Move the table under “Historical measurements (2026-07-04)” or to `docs/`. Keep the hedge.
- Or rerun on this HEAD and replace the commit IDs.

Implementation assessment:
- Complexity: operator_ux
- Cost: XS
- Nominal implementation agent: claude
- Cost drivers: docs move
- Rationale: wording/location.

Validation:
- Current-tense README sections do not embed the 2026-07-04 table without a “historical” heading.

Non-goals:
- Do not invent new benchmark numbers.

---

## WFG inventory (every code)

| Code | Verdict | Note |
|---|---|---|
| WFG-001 Fake Success | **Finding** (partial) / **Held** (old path) | Missing tool response is now `unknown` (`ToolCallWithResponse.tsx:371-377`, test `:18-21`). Remaining: bulk Always-Allow persist-fail (WFG-GOS-002), theme/spellcheck optimistic (WFG-GOS-009), `deriveLoadingStatus` treats any non-`error` as success (`:379-381`) — ACP adapter only writes success/error (`adapter/tools.ts:76-79`) so latent. |
| WFG-002 UI/API Mismatch | **Finding** | WFG-GOS-002 (shown fail vs AlwaysAllow sent); WFG-GOS-004 (click vs no status); Mode UI vs backend (WFG-GOS-001). Buttons ignore `request.options` and can show “Always allowed” while `permissionRequests.ts:72-75` sends `cancelled` if kind missing — still **Plausible** for third-party ACP (gosling server sends all four). |
| WFG-003 CLI/API Mismatch | **Finding** | WFG-GOS-006: CLI/Desktop hide Always Allow on security prompt; TUI offers it. Non-interactive: CLI errors in Approve/SmartApprove (`session/mod.rs:1207-1214`); TUI `--text` declines; aligned fail-closed. |
| WFG-004 Stale Display | **Finding** | WFG-GOS-001, WFG-GOS-005, theme-init vs store. Workspace filter persist is repaired (`WorkspaceContext.tsx` + test). |
| WFG-005 Hidden Failure | **Finding** | WFG-GOS-003, WFG-GOS-004. ModeSection max-turns catch is log-only (`ModeSection.tsx:44-46`) — Low, not separately filed. |
| WFG-006 Destructive Ambiguity | **Held** | Session delete uses a named confirm + partial-failure toast (`SessionListPane.tsx:593-630`). Extension install has typed confirm (`ExtensionInstallModal.tsx`). Switching to Auto has **no** extra confirm — covered via WFG-GOS-001/WEB-GOS-001 rather than unlabeled delete. |
| WFG-007 Approval Gate Bypass | **Finding** / **Held** (old TUI auto-allow) | Old TUI `options[0]` path is **gone**. Residual: Settings default click → Auto (WFG-GOS-001); plan-act persist Auto (WFG-GOS-007); `--yes` fallback (WFG-GOS-008); Auto mode itself still Allow (`permission_inspector.rs:162-166`) — by design. |
| WFG-008 Status Lies | **Finding** | Mode radio vs SmartApprove default; stale audits as current (CMP-GOS-001). Health/doctor not re-audited. |
| WFG-009 Partial Success Presented Complete | **Finding** | WFG-GOS-002. Session delete **Held**: archive-file failure is a second toast (`SessionListPane.tsx:620-622`). Import success is bound to `acpImportSession` resolve (`SessionListView.tsx:118-119`). |
| WFG-010 Disabled Control Active Backend | **Finding** | Always Allow hidden when `prompt` set, but ACP options still include `allow_always` (`server.rs:2306-2310`). Another client or TUI can still select it (WFG-GOS-006). |
| WFG-011 Backend Mutation No Feedback | **Finding** | WFG-GOS-004; plan-act mode write is silent (WFG-GOS-007). Config `upsert` reloads after write (`ConfigContext.tsx:61-66`) — Held for provider/mode when upsert succeeds. |
| WFG-012 Workflow Step Skipped | **Finding** (Low) | WEB-GOS-004 dead ConfigureApproveMode. Onboarding still guards provider (`OnboardingGuard`). |
| WFG-013 Operator Cannot Diagnose | **Finding** | WFG-GOS-003. Elicitation expired/stale **Held** (`ElicitationRequest.tsx:17-40`). |
| WFG-014 Derived Data Shown Confirmed | **Held** (sampled) | Tool cards show model/tool output as conversation content (inherent). Approval now labels options. README perf is caveated (CMP-GOS-003). LLM SmartApprove “read-only” auto-allow is labeled in inspector reason (`permission_inspector.rs:244-248`) but Desktop does not show that provenance on a skipped prompt — **Plausible**, not filed (needs a UI path that shows the skip). |
| WFG-015 Bulk Selection Semantics | **Held** with note | Extension “Always Allow all X tools” applies to `listTools` names, matching the label (`ToolApprovalButtons.tsx:155-159`). Empty list falls back to the one shown tool. Session list bulk not present. |

---

## Design gates

### Fast Design Gate

| Q | Verdict | Evidence |
|---|---|---|
| Where am I? | Pass | Settings tabs (`SettingsView.tsx:152+`), chat stream, TUI header |
| What matters? | Partial | Approval consequences not ranked (WEB-GOS-001); payload truncated (WEB-GOS-002) |
| What to do next? | Pass | Onboarding, New Chat, empty-session affordances (sampled) |
| What just happened? | Partial | Toasts on session CRUD; tool error string dropped (WFG-GOS-003); missing response no longer green |
| How to recover? | Partial | Stale approval / elicitation copy exists; approval throw is silent |

### Gate scorecard

| Gate | Coverage | Worst finding | Verdict | Evidence line |
|---|---|---|---|---|
| 1 Product / Workflow | partial (code, no live click-trail) | High (WEB-GOS-001/002, WFG-GOS-001) | **Fail** | Approval hierarchy + truncated disclosure + Mode default lie |
| 2 Handoff | partial | none High | **Partial** | Tokens exist (`theme-tokens.ts:1-15`); no separate design spec / 404-500 mockups (Electron SPA) |
| 3 Web Standards | partial | Medium (WEB-GOS-003) | **Partial** | `<!doctype html>` + CSP (`index.html:1-8`); no `lang`; buttons mostly semantic; Mode row is a div |
| 4 Accessibility | thin/partial (no live keyboard/SR/contrast) | Medium (WEB-GOS-003) | **Partial** (coverage caveat; contrast not measured so cannot Fail on 1.4.3) | Focus styles on `button.tsx:7`; gear unnamed; lang missing |
| 5 Device / Browser | thin | none measured | **Partial** (coverage caveat) | Electron Chromium desktop; no mobile breakpoints audited; not launched |
| 6 Production Readiness | partial | High (approval/settings) | **Fail** | Permissions enforced in core, not only UI; settings persist can lie; no live perf measurement |

Tokens/handoff non-finding: `theme-tokens.ts` is an explicit light/dark semantic system applied to `:root` and MCP host styles.

---

## CMP inventory (every code)

| Code | Verdict | Note |
|---|---|---|
| CMP-001 Framework Version Ambiguity | **Finding** | CMP-GOS-002 (0.1.0 vs v1.0.0 vs v0.0.5). Giles `canon_version: "1.4"` is pinned (`.giles/repo.yaml:3`) — Held for Giles. |
| CMP-002 Draft Treated As Final | **Held** | v1.0.0 notes say they *prepare* content (`v1.0.0.md:9-11`). README feature list is more final-sounding (CMP-GOS-002). |
| CMP-003 Evidence Overclaim | **Finding** (mild) | README fail-closed inspector claim matches `tool_inspection.rs:130-154` — **Held**. “Safer Defaults” (`README.md:68`) is vague but now true-ish (SmartApprove). Stale master report overclaims (CMP-GOS-001). Perf location (CMP-GOS-003). |
| CMP-004 Missing Evidence As Violation | **Held / N/A** | No SSDF collector. SECURITY.md uses precaution language, not “noncompliant.” |
| CMP-005 Advisory As Enforcement | **Held** | SECURITY.md is advisory. `RELEASE_CHECKLIST.md:3` is a human publish gate, not an automated fail on advisory gaps. |
| CMP-006 Wrong Control Mapping | **Held / N/A** | No SSDF mapper. |
| CMP-007 Evidence Grade Inflation | **Finding** | `99-master-report.md` “lead-verified ✔” on default Auto / TUI auto-allow / fail-open is now false at HEAD (CMP-GOS-001). |
| CMP-008 Report Format Drift | **N/A** | No paired JSON/MD posture emitter in-repo. |
| CMP-009 Collector Scope Violation | **N/A** | No passive compliance collector. |
| CMP-010 Certification Language | **Held** | SECURITY.md does not say certified/compliant/guaranteed. README does not claim certification. `.giles/repo.yaml` is `mode: canonical` (advisory mirror per AGENTS.md) — not a cert. |
| CMP-011 Tool Output As Ground Truth | **Finding** | Historical playtest/audit files are reused as current validation in README `:135-137` (CMP-GOS-001). |
| CMP-012 Policy/Practice Confusion | **Held** | SECURITY.md is policy/advice; README hardening bullets are backed by current code for fail-closed inspection and 0o600/0o700 claims were not re-traced in this slice (Not Reviewed). |
| CMP-013 Profile Misapplied | **Held** | No SSDF profile applied as a pass/fail in-tree. |
| CMP-014 Release Gate Semantics | **Held** with note | Checklist is explicit and human-owned. It requires 1.0.0 identity this tree does not have — a readiness gap, not a silent advisory-fail. |
| CMP-015 Stale Compliance Evidence | **Finding** | CMP-GOS-001, CMP-GOS-003. Orientation correctly says seeds-not-verdicts; the older files themselves do not. |

---

## Non-Findings / Checked But Not Confirmed

- **TUI interactive auto-allow is repaired.** `tui.tsx:784-793` stores `resolve` until `PermissionPrompt` keypress (`:1185-1188`). Comment cites ARC-GOS-001. This **denies** the 2026-07 WFG-GSL-001 on this HEAD.
- **TUI `--text` fail-closed without `--yes`.** `tui.tsx:1390-1407` writes a reason to stderr and selects `reject_once`.
- **Missing tool response is not green success.** `deriveLoadingStatus` → `unknown` (`ToolCallWithResponse.tsx:371-377`); mapped to `pending` (`:800-804`); `Dot.tsx:13-16` uses amber for `unknown`. Regression test at `ToolCallWithResponse.test.tsx:18-21`.
- **CLI non-interactive Approve/SmartApprove fails closed.** `session/mod.rs:1207-1214`.
- **CLI/Desktop hide Always Allow when a security prompt is present.** `session/mod.rs:1943-1951`; `ToolApprovalButtons.tsx:201`.
- **Inspector errors fail closed.** `tool_inspection.rs:130-154`; Auto downgrade does **not** apply to the `Err` branch. README `:62` matches.
- **`read_only_hint: true` cannot self-authorize.** `permission.rs:143-160` — annotations only tighten (`read_only_hint == Some(false)` → AskBefore).
- **Rust default mode is SmartApprove**, not Auto (`gosling_mode.rs:28-29`).
- **Session delete is labeled + confirmed + partial-aware.** `SessionListPane.tsx:593-630`.
- **Session diagnostics missing-id fake success is repaired.** `session_diagnostics_command_test.rs:14-33` — non-zero exit, no output file.
- **Chat Settings copy** correctly says default is for *new* sessions (`ChatSettingsSection.tsx:17-19`); session ModeSwitcher is labeled “Mode for this session” (`ModeSwitcher.tsx:21-24`).
- **SECURITY.md does not certify or claim enforcement.** It is a caution + reporting pointer (`SECURITY.md:1-16`).
- **v1.0.0 release notes** do not claim the tag exists (`v1.0.0.md:9-11`).
- **ACP resolve stale path is honest** when `resolve` returns `false` (`ToolApprovalButtons.tsx:134-136` + test `:53-75`).
- **TUI tool failed state is red**, not success (`toolcall.tsx:36-41, 87-92`).
- **Config upsert reloads** after a successful write (`ConfigContext.tsx:61-66`).
- **External backend secret** is stripped before renderer persist (`main.ts:2026-2031, 2059-2066`).

---

## Break-It Review

| Attack | Result |
|---|---|
| Backend tool result missing after stream end | **Survives** — `unknown` / pending / amber, not success |
| Backend tool `status:error` | **Fails** — red dot, no reason (WFG-GOS-003) |
| TUI Approve mode, no operator | **Survives** (interactive blocks; `--text` declines) |
| TUI `--yes` | AllowOnce; latent AlwaysAllow fallback (WFG-GOS-008) |
| Desktop Always Allow all ext + persist fail | **Fails** — call already resolved (WFG-GOS-002) |
| Approval resolve throws | **Fails** — silent (WFG-GOS-004) |
| Unset GOSLING_MODE + Settings open | **Fails** — Auto highlighted; click persists Auto (WFG-GOS-001) |
| Leftover localStorage vs electron-store | **Fails** — LS wins (WFG-GOS-005) |
| Theme setSetting reject | **Fails** — UI stays new (WFG-GOS-009) |
| Security prompt + Always Allow | Desktop/CLI hide; TUI still offers (WFG-GOS-006) |
| CLI plan-act + kill | **Likely fail** — Auto left persisted (WFG-GOS-007); crash not executed |
| Session delete | **Survives** |
| Missing session diagnostics | **Survives** (test) |
| Treat `99-master-report.md` as current | **Fails** (CMP-GOS-001) |
| Disabled Always Allow via `prompt` + other client | Backend still accepts AllowAlways |
| Bulk session selection | N/A — no bulk session mutate |

Runtime manifestation of crash-during-plan-act is capped at **Likely** (not executed). All other material findings are deterministic code paths.

---

## Recommended Patch Order

1. Banner/archive stale `docs/cloud` reports and stop citing 2026-07-20 as current release validation (CMP-GOS-001).
2. Fix Settings Default Mode initial state + no-op click (WFG-GOS-001).
3. Fix bulk Always-Allow order / partial-success copy (WFG-GOS-002).
4. Approval visual hierarchy + full command disclosure (WEB-GOS-001, WEB-GOS-002).
5. Render tool error strings; surface approval throws (WFG-GOS-003, WFG-GOS-004).
6. Migrate-on-read settings; roll back theme/spellcheck on persist fail (WFG-GOS-005, WFG-GOS-009).
7. Omit `AllowAlways` from security-prompt ACP options (WFG-GOS-006); drop `--yes` AlwaysAllow fallback (WFG-GOS-008).
8. Plan-act in-memory Auto only (WFG-GOS-007).
9. `lang` + accessible names (WEB-GOS-003); remove dead ConfigureApproveMode (WEB-GOS-004).
10. Reconcile 0.1.0 vs v1.0.0 language (CMP-GOS-002); relocate historical perf table (CMP-GOS-003).

---

## Regression Test Strategy

| Test | Purpose | Finding |
|---|---|---|
| Unset `GOSLING_MODE` → Settings shows Smart; click Smart does not upsert | Default truth | WFG-GOS-001 |
| `setToolPermissions` reject → no silent AlwaysAllow / explicit partial | Bulk truth | WFG-GOS-002 |
| Approval resolve throw → `role="alert"` | Hidden failure | WFG-GOS-004 |
| Tool result `{status:'error', error:'boom'}` appears in DOM | Diagnose | WFG-GOS-003 |
| Already exists: `deriveLoadingStatus(undefined,false)==='unknown'` | Keep fake-success closed | WFG-001 Held |
| Already exists: TUI PermissionPrompt / `--text` decline comments; add `--yes` without `allow_once` → cancelled | Auto-allow | WFG-GOS-008 |
| localStorage + store disagree → store wins, LS cleared | Persist | WFG-GOS-005 |
| Security-prompt options omit `allow_always` | Client parity | WFG-GOS-006 |
| Plan-act error path leaves prior mode | Gate | WFG-GOS-007 |
| Docs grep: historical reports have superseded banner | Posture | CMP-GOS-001 |

---

## Deferred Risks

- Third-party ACP reduced option sets (WFG-GSL-004 latent): buttons not derived from `request.options`; missing kind → cancelled while UI may show the clicked label. Plausible until a non-gosling agent is connected.
- `map_permission_response` AllowOnce → AllowAlways fallback (`acp/common.rs:79-81`) if `allow_once` absent. Latent for gosling-core.
- `--text` decline `reject_once ?? options[0]` (`tui.tsx:1396-1398`): if `reject_once` missing, `options[0]` is AllowAlways on gosling-core. Latent.
- SmartApprove LLM read-only skip has no Desktop provenance chip (WFG-014 Plausible).
- Contrast, live keyboard order, mobile widths: not measured (Gate 4/5).
- README 0o600/0o700 / plugin `--` claims: not re-traced (CMP-012 Not Reviewed).
- Subagent Auto forcing: mentioned in `permission_inspector.rs:134-137`; not deep-reviewed (route to agent-orchestration).
- Shell-focused `requestPermission` cancel stubs (`acpRuntime.ts:193`) — sampled only.

---

## Validation Limits

- App was **not** built or launched. No live click, keyboard, SR, or contrast measurement.
- No `cargo test` / `pnpm test` executed in this pass. Existing tests were **read**, not run. Oracle-integrity check: fixtures were not treated as production proof; several tests encode the current (sometimes wrong) contract (bulk resolve-first).
- Ink TUI overflow beyond PermissionPrompt/toolcall was not exhaustively audited.
- `ui/desktop/src/shell/*` focused-product surface sampled only.
- Provider onboarding, MCP app renderer success paths, dictation, updater toasts: not exhaustively reviewed.
- Gate 5 cross-browser/print/CSS-off: N/A / not run.
- SECURITY.md “unique risk” narrative is accepted as advisory; no attempt to certify the product.

---

## Final Confidence

**Medium-High** for operator-truth on the named surfaces (approval, TUI, settings, README/SECURITY/stale reports): those paths were read end-to-end with file:line. **Medium** for design Gates 4–5 (static only). **High** that the 2026-07 TUI auto-allow and default-Auto *code* findings are not present on this HEAD, and **High** that the *documents* still say they are.

---

## v3.1 Calibration Addendum

Static review confirmed missing guards and lying copy. Runtime crash-during-plan-act and leftover-localStorage-in-a-real-profile are **Likely** until reproduced. No race/OOM/lock-storm was marked Confirmed.

### Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| WFG-GOS-001 | Workflow-GUI | State-Transition, Security | Default mode is the approval gate |
| WFG-GOS-002 | Workflow-GUI | State-Transition, Security | Partial AlwaysAllow persist |
| WEB-GOS-001/002 | Design / Workflow-GUI | Security | Human backstop affordance |
| WFG-GOS-005/009 | Workflow-GUI | Temporal, Data-Integrity | Settings freshness |
| WFG-GOS-006 | Workflow-GUI | Contract-InternalAPI | ACP options vs client policy |
| WFG-GOS-007 | Workflow-GUI | Failsafe, Temporal | Persist Auto without restore-on-crash |
| CMP-GOS-001 | Compliance-Posture | Workflow-GUI, Security | Stale audits become believed controls |
| CMP-GOS-002 | Compliance-Posture | Architecture | Version identity |

---

## What would falsify the strongest conclusion?

The strongest conclusion is: **TUI no longer auto-allows, but Settings + stale docs still create an Auto/AlwaysAllow lie.**

It would be falsified if:

1. First-run `acpReadAllConfig` always materializes `GOSLING_MODE=smart_approve` *before* `ModeSection` mounts, **and** the radio click is a no-op when already selected (then WFG-GOS-001 drops to Low FOUC).
2. Historical `docs/cloud` reports are bannered and README release-validation points at a HEAD playtest (then CMP-GOS-001 drops to Info).
3. A live launch shows `GOSLING_MODE` pre-populated and TUI PermissionPrompt unreachable on the default path (would require re-scoring).

None of those were observed in current source.

---

## Finding IDs (index)

| ID | Severity | Path |
|---|---|---|
| WFG-GOS-001 | High | `ui/desktop/src/components/settings/mode/ModeSection.tsx` |
| WFG-GOS-002 | High | `ui/desktop/src/components/ToolApprovalButtons.tsx` |
| WEB-GOS-001 | High | `ui/desktop/src/components/ToolApprovalButtons.tsx` |
| WEB-GOS-002 | High | `ui/desktop/src/components/ToolCallConfirmation.tsx` |
| CMP-GOS-001 | High | `docs/cloud/99-master-report.md`, `docs/cloud/audit-workflow-gui.md`, `README.md` |
| WFG-GOS-003 | Medium | `ui/desktop/src/components/ToolCallWithResponse.tsx` |
| WFG-GOS-004 | Medium | `ui/desktop/src/components/ToolApprovalButtons.tsx` |
| WFG-GOS-005 | Medium | `ui/desktop/src/preload.ts`, `ui/desktop/src/theme-init.ts` |
| WFG-GOS-006 | Medium | `ui/text/src/components/PermissionPrompt.tsx` vs Desktop/CLI |
| WFG-GOS-007 | Medium | `crates/gosling-cli/src/session/mod.rs` |
| WFG-GOS-009 | Medium | `ui/desktop/src/contexts/ThemeContext.tsx`, `SpellcheckToggle.tsx` |
| WEB-GOS-003 | Medium | `ui/desktop/index.html`, `ModeSelectionItem.tsx` |
| CMP-GOS-002 | Medium | `README.md`, `Cargo.toml`, `ui/desktop/package.json` |
| WFG-GOS-008 | Low | `ui/text/src/tui.tsx` |
| WEB-GOS-004 | Low | `ui/desktop/src/components/settings/mode/ModeSelectionItem.tsx` |
| CMP-GOS-003 | Low | `README.md` |
