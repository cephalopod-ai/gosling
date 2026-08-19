# Default Shell GUI — Gate 2: front-end handoff

Status: design deliverable; no renderer implementation is authorized by this document
Date: 2026-08-18
Gate: `plan-webapp-design` Gate 2
Depends on: [`gate-1-product-workflow-design.md`](gate-1-product-workflow-design.md)
Authority: ADR-0010–0014, DS-7 acceptance
Requirements: SHP-REQ-055, SHP-REQ-057

## 0. How to read this

This is the contract an implementer builds against in Gate 3. Every row cites committed source. If a
row and the source disagree, **the source wins and this document is wrong** — file it as a defect
rather than coding around it.

Source of record:

| Concern                                      | File                                                                     |
| -------------------------------------------- | ------------------------------------------------------------------------ |
| Renderer API shape                           | `ui/desktop/src/shell/preloadApi.ts`                                     |
| Channels, request/response types             | `ui/desktop/src/shell/ipc.ts`                                            |
| Capability gating, byte bounds, sender trust | `ui/desktop/src/shell/ipcMain.ts`                                        |
| Snapshot shape                               | `ui/desktop/src/shell/runtimeSnapshot.ts`                                |
| Lifecycle states and legal transitions       | `ui/desktop/src/shell/lifecycle.ts`                                      |
| Failure taxonomy                             | `ui/desktop/src/shell/operationFailure.ts`                               |
| Session record, updates, transcript          | `ui/desktop/src/shell/sessionController.ts`                              |
| Stream projection                            | `ui/desktop/src/shell/sessionUpdateProjection.ts`                        |
| Interaction records                          | `ui/desktop/src/shell/interactionController.ts`                          |
| Directory / credential snapshots             | `ui/desktop/src/shell/directoryController.ts`, `credentialController.ts` |
| Settings schema and bounds                   | `ui/desktop/src/shell/localSettings.ts`                                  |
| Module summary                               | `ui/sdk/src/generated/types.gen.ts` (`ShellModuleSummary`)               |

The renderer's entire world is `window.goslingShell`. There is no `require`, no Node integration, no
`fetch` to a backend, no filesystem. Anything not on that object does not exist.

## 1. Operation inventory

Twenty-four invoke channels. "Capability" is the string the consumer manifest must declare in
`declaredCapabilities`; a blank cell means the operation is ungated. Response bound is enforced in
main and a violation throws.

| API call                       | Channel                      | Request                                              | Response                                         | Capability                | Resp. bound |
| ------------------------------ | ---------------------------- | ---------------------------------------------------- | ------------------------------------------------ | ------------------------- | ----------- |
| `runtime.read()`               | `runtime.read`               | _(none — passing one throws)_                        | `ShellRuntimeSnapshot`                           | —                         | 64 KiB      |
| `runtime.retry(r)`             | `runtime.retry`              | `{generation}`                                       | `ShellActionResult`                              | —                         | 8 KiB       |
| `runtime.stop(r)`              | `runtime.stop`               | `{generation}`                                       | `ShellActionResult`                              | —                         | 8 KiB       |
| `directory.select(r)`          | `directory.select`           | `{generation, userGesture: true}`                    | `ShellDirectorySelectResult`                     | `directory.select`        | 8 KiB       |
| `credential.select(r)`         | `credential.select`          | `{generation, profileId: string\|null}`              | `ShellCredentialSnapshot`                        | `credential.select`       | 64 KiB      |
| `session.create(r)`            | `session.create`             | `{generation}`                                       | `ShellSessionRecord`                             | `session.create`          | 8 KiB       |
| `session.list(r)`              | `session.list`               | `{generation}`                                       | `ShellSessionSummary[]`                          | `session.list`            | 64 KiB      |
| `session.resume(r)`            | `session.resume`             | `{generation, sessionId}`                            | `ShellSessionRecord`                             | `session.resume`          | 8 KiB       |
| `session.readTranscript(r)`    | `session.transcript.read`    | `{generation, sessionId}`                            | `ShellTranscriptSnapshot`                        | `session.transcript.read` | 64 KiB      |
| `session.detach(r)`            | `session.detach`             | `{generation}`                                       | `ShellSessionDetachResult`                       | `session.detach`          | 8 KiB       |
| `prompt.submit(r)`             | `prompt.submit`              | `{generation, sessionId, text}`                      | `{promptAttemptId}`                              | `prompt.submit`           | 8 KiB       |
| `prompt.cancel(r)`             | `prompt.cancel`              | `{generation, sessionId, promptAttemptId}`           | `void`                                           | `prompt.cancel`           | 8 KiB       |
| `permission.respond(r)`        | `permission.respond`         | `{generation, sessionId, actionId, allowOnce}`       | `void`                                           | `permission.respond`      | 8 KiB       |
| `elicitation.respond(r)`       | `elicitation.respond`        | `{generation, sessionId, actionId, action, fields?}` | `void`                                           | `elicitation.respond`     | 8 KiB       |
| `domain.snapshot(r)`           | `domain.snapshot`            | `{generation, input?}`                               | `DomainSnapshotResponse_unstable`                | `domain.snapshot`         | 64 KiB      |
| `domain.action(r)`             | `domain.action`              | `{generation, sessionId, action, input?}`            | `DomainActionResponse_unstable`                  | `domain.action`           | 64 KiB      |
| `domain.confirm(r)`            | `confirmation.respond`       | `{generation, sessionId, actionId, approve}`         | `DomainActionConfirmResponse_unstable`           | `confirmation.respond`    | 64 KiB      |
| `diagnostics.save(r)`          | `diagnostics.save`           | `{generation, userGesture: true}`                    | `{status:'canceled'}\|{status:'saved',fileName}` | —                         | 8 KiB       |
| `handoff.prepare(r)`           | `handoff.prepare`            | `ShellHandoffPrepareRequest`                         | `{generation, handoff}`                          | —                         | 64 KiB      |
| `handoff.confirm(r)`           | `handoff.confirm`            | `{generation, handoffId}`                            | `{opened}`                                       | —                         | 8 KiB       |
| `external.open(url)`           | `external.open`              | `string` (≤2 KiB, allowlisted origin)                | `{opened}`                                       | —                         | 8 KiB       |
| `settings.read()`              | `settings.read`              | _(none — passing one throws)_                        | `ShellSettingsSnapshot`                          | —                         | 8 KiB       |
| `settings.updateAppearance(r)` | `settings.appearance.update` | `{generation, theme?, textScale?}`                   | `ShellSettingsSnapshot`                          | —                         | 8 KiB       |
| `settings.reset(r)`            | `settings.reset`             | `{generation, userGesture: true}`                    | `ShellSettingsSnapshot`                          | —                         | 8 KiB       |

Requests are bounded at 64 KiB (2 KiB for `external.open`). Every rejection arrives as an `Error`
whose message carries an encoded `ShellOperationFailure`; use `decodeShellOperationFailure`. **Never
show a raw error string** — if decoding returns `null`, treat it as `OPERATION_FAILED`.

The committed neutral template declares eleven capabilities: `credential.select`,
`directory.select`, `elicitation.respond`, `permission.respond`, `prompt.cancel`, `prompt.submit`,
`session.create`, `session.detach`, `session.list`, `session.resume`, `session.transcript.read`. The
three `domain.*` capabilities are **not** declared, so the generic GUI ships the module/adapter slot
in its undeclared state. Building it must not require a manifest change.

Capability closure, enforced in `runtimeController`: if `permission.respond` is undeclared, ACP
permission callbacks resolve `cancelled` without ever reaching the renderer; if
`elicitation.respond` is undeclared, elicitations resolve `cancel`. A GUI that renders a permission
dock without declaring the response capability would show a request nobody can answer — so the
component must be gated on the declaration, not on the event.

## 2. Event inventory

| API                              | Channel                  | Payload                           | Bound  |
| -------------------------------- | ------------------------ | --------------------------------- | ------ |
| `runtime.onChanged`              | `runtime.changed`        | `ShellRuntimeSnapshot`            | 64 KiB |
| `session.onUpdated`              | `session.updated`        | `ShellSessionUpdate`              | 64 KiB |
| `permission.onRequested`         | `permission.requested`   | interaction, `kind:'permission'`  | 8 KiB  |
| `elicitation.onRequested`        | `elicitation.requested`  | interaction, `kind:'elicitation'` | 8 KiB  |
| `domain.onConfirmationRequested` | `confirmation.requested` | interaction, `kind:'confirm'`     | 8 KiB  |

Each subscribe returns an unsubscribe function. Main drops deliveries to a destroyed renderer and
suppresses snapshots from a superseded generation, so the renderer never needs to defend against
those — but it must still ignore payloads whose `generation` is below its own current one, because
event and snapshot ordering across channels is not guaranteed.

## 3. State ownership

Exactly one store, seeded by `runtime.read()` and `settings.read()` at mount, then advanced only by
events and operation results.

| Slice          | Owner             | Rule                                                                                     |
| -------------- | ----------------- | ---------------------------------------------------------------------------------------- |
| `snapshot`     | main              | replace wholesale on `runtime.changed`; never patch a field locally                      |
| `settings`     | main              | replace wholesale on each settings operation result                                      |
| `transcript`   | renderer, derived | ordered by `updateSeq`; built from `readTranscript` then appended from `session.updated` |
| `interactions` | main + events     | keyed by `actionId`; union of `snapshot.pendingInteractions` and arriving events         |
| `draft`        | **renderer only** | must survive generation bumps; never derived from the snapshot                           |
| `route`        | renderer, derived | computed from snapshot per Gate 1 §4; never independently assigned                       |

Anti-requirement: the renderer must not maintain a parallel copy of directory, credential, session,
module, or lifecycle state. Those exist once, in `snapshot`. Optimistic local mutation of any of them
is a defect.

## 4. Reducer rules

**R-1 Generation fencing.** Every request carries `snapshot.generation`. Drop any event whose
`generation` is lower. On an increase, clear `transcript` and `interactions`, keep `draft` and
`settings`, and re-read the snapshot.

**R-2 Monotonic sequence.** `ShellSessionUpdate.updateSeq` is monotonic per runtime. Insert by
`updateSeq`; discard a duplicate; a gap sets a `hasGap` flag that offers Repair (re-call
`readTranscript`). Never renumber and never sort by arrival.

**R-3 Delivery seam.** `delivery: 'history'` updates precede `'live'` ones after a resume. Render one
seam at the boundary. A `'history'` update arriving after live content has begun is a bug in the
renderer's ordering, not in main.

**R-4 Prompt attempt lifecycle.** `kind` is `started` | `stream` | `completed` | `cancelled` |
`failed`. Only the three terminal kinds end an attempt. `snapshot.session.promptAttempt.phase`
(`idle` | `streaming` | `cancelling`) drives the Send/Stop control; do not infer it from stream
traffic.

**R-5 Interaction lifetime.** Add on event or snapshot; remove only on explicit response or on a
terminal session outcome. A `stream` update must never clear the dock (SHP-DEF-052). Responses are
single-use: disable the controls immediately on click and never resend the same `actionId`.

**R-6 Stream kinds.** `ShellSessionStream` is exactly four shapes: `content` (with `role`
`user`/`assistant`, `messageId`, `text` ≤60 KiB), `tool` (`toolCallId`, `title` ≤4 KiB, `toolKind`,
`status`), `session_info` (`title`), `usage` (`used`, `size`). `agent_thought_chunk` is dropped in
main and will never arrive — the renderer must have no "thinking" view. Non-text content blocks are
dropped too, so the renderer needs no image or blob path.

**R-7 Failures.** Decode, then branch on `recovery` for the affordance and `preservesDraft` for the
composer. Honor `retrySafe`: only `RUNTIME_UNAVAILABLE` sets it true today, and the UI must not offer
retry where it is false.

**R-8 Cancel is success.** `directory.select` → `cancelled`, `elicitation.respond` with `cancel`,
and `diagnostics.save` → `canceled` are all normal outcomes. No error styling, no logging, no toast.

## 5. Component inventory

Props are derived types, not new shapes. `Snap = ShellRuntimeSnapshot`.

| ID   | Component                | Props                                                                   | Renders when                                           | Emits                                                  |
| ---- | ------------------------ | ----------------------------------------------------------------------- | ------------------------------------------------------ | ------------------------------------------------------ |
| C-01 | `ShellFrame`             | `{snapshot, settings}`                                                  | always                                                 | route selection                                        |
| C-02 | `StatusPill`             | `{state: Snap['lifecycleState'], compatibility: Snap['compatibility']}` | always                                                 | —                                                      |
| C-03 | `IdentityBadge`          | `{identity: Snap['identity']}`                                          | always; unverified until non-null                      | —                                                      |
| C-04 | `DirectoryChip`          | `{directory: Snap['directory'], canChange: boolean}`                    | always                                                 | `directory.select`                                     |
| C-05 | `CredentialChip`         | `{credentials: Snap['credentials']}`                                    | always; menu only when `catalogStatus === 'available'` | `credential.select`                                    |
| C-06 | `SessionContext`         | `{session: Snap['session']}`                                            | when session non-null                                  | —                                                      |
| C-07 | `SessionPicker`          | `{directory, credentials, sessions, loading, declared actions}`         | route S-05; Workspace, Tasks, and Recent tasks panels  | existing directory/credential/settings/session actions |
| C-08 | `Transcript`             | `{blocks, hasGap, integrity, resumeIntegrity}`                          | route S-06+                                            | Repair → `session.readTranscript`                      |
| C-09 | `ToolActivityRow`        | `{toolCallId, title, toolKind, status}`                                 | per `tool` stream                                      | —                                                      |
| C-10 | `UsageMeter`             | `{used, size}`                                                          | on `usage` stream                                      | —                                                      |
| C-11 | `TranscriptGapNotice`    | `{integrity, truncated, firstSeq, lastSeq}`                             | `integrity !== 'complete' \|\| truncated`              | Repair                                                 |
| C-12 | `Composer`               | `{draft, phase, disabledReason, failure}`                               | route S-06+                                            | `prompt.submit`, `prompt.cancel`                       |
| C-13 | `InteractionDock`        | `{interactions: ShellInteraction[], queueDepth}`                        | any pending                                            | routes to C-14/15/16                                   |
| C-14 | `PermissionRequest`      | permission `summary`                                                    | dock                                                   | `permission.respond`                                   |
| C-15 | `ElicitationForm`        | elicitation `summary`                                                   | dock                                                   | `elicitation.respond`                                  |
| C-16 | `ConfirmationRequest`    | confirm `summary`                                                       | dock                                                   | `domain.confirm`                                       |
| C-17 | `ModulesStrip`           | `{modules: Snap['modules'], adapter: Snap['adapter']}`                  | any non-`core` module or adapter                       | —                                                      |
| C-18 | `DomainSlot`             | `{adapter, declared: boolean}`                                          | `declared && adapter !== null`                         | `domain.snapshot`/`action`                             |
| C-19 | `SettingsPanel`          | `{settings, recovery}`                                                  | route S-21                                             | `settings.updateAppearance`, `settings.reset`          |
| C-20 | `SettingsRecoveryNotice` | `{recovery}`                                                            | `status` not `loaded`/`absent`                         | `settings.reset`                                       |
| C-21 | `LifecycleFailureScreen` | `{state, reasonCode, allowedActions}`                                   | S-14…S-20                                              | `runtime.retry`, `runtime.stop`, diagnostics, handoff  |
| C-22 | `ProvisioningIssueList`  | `{issues: Snap['provisioningIssues']}`                                  | `issues.length > 0`                                    | diagnostics                                            |
| C-23 | `DiagnosticsAction`      | `{generation}`                                                          | wherever `allowedActions` includes `diagnostics`       | `diagnostics.save`                                     |
| C-24 | `HandoffDialog`          | `{envelope}`                                                            | route S-28                                             | `handoff.prepare`/`confirm`, `external.open`           |
| C-25 | `FailureBanner`          | `{failure: ShellOperationFailure}`                                      | on decoded failure                                     | recovery action                                        |
| C-26 | `OutputsPanel`           | `{artifacts, totalCount, truncated}`                                    | active session and declared `session.artifacts.read`   | `session.readArtifacts`                                |

C-21 must render buttons from `allowedActions` alone. Hard-coding Retry produces a dead button in
`relink_required`, `incompatible`, and `fatal`, where the host rejects it.

## 6. Bounds

Enforced in main; the UI must respect them proactively so the user never trips a limit blind.

| Thing                    | Bound                          | UI obligation                                   |
| ------------------------ | ------------------------------ | ----------------------------------------------- |
| Prompt text              | 64 KiB                         | live counter near the limit; block Send past it |
| Transcript ledger        | 256 updates / 48 KiB           | gap notice; never claim completeness            |
| Session list             | 20                             | label the cap in the heading                    |
| Credential profiles      | 128                            | scrollable list                                 |
| Credential field length  | 256 chars                      | truncate with full value on focus               |
| Elicitation              | 32 fields / 64 options / 8 KiB | render all; no pagination needed                |
| Interaction IDs          | 4,096 per runtime              | on exhaustion route to S-14                     |
| Settings document        | 16 KiB                         | not user-visible                                |
| Directory path / label   | 4096 / 128 chars               | label in chip, path on hover                    |
| Stream text / tool title | 60 KiB / 4 KiB                 | virtualize long transcripts                     |
| Diagnostics bundle       | 1 MiB                          | show the saved filename                         |
| Text scale               | 0.8–2.0                        | clamp the control to this range                 |
| ACP preflight            | 10 s                           | S-02 must not look hung before it               |

## 7. Theme and tokens

Three themes (`system`, `light`, `dark`) × text scale 0.8–2.0. Tokens are semantic, never raw values
at call sites.

- Surface: `--surface-base`, `--surface-raised`, `--surface-sunken`
- Text: `--text-primary`, `--text-secondary`, `--text-inverse`
- Line: `--border-subtle`, `--border-strong`, `--focus-ring`
- Status: `--status-ok`, `--status-busy`, `--status-warn`, `--status-error`, `--status-neutral`
- Effect badges (permission): `read`, `write`, `execute`, `network`, `other` — five distinct tokens,
  each distinguishable without color alone
- Type scale: derived from a single `--text-scale` multiplier bound to the setting

`system` follows the OS. All three themes must pass contrast, and no status may be conveyed by hue
alone.

## 8. Accessibility acceptance criteria (SHP-REQ-057)

Each is a test, not an aspiration.

| ID   | Criterion                                                              | How it is verified                                    |
| ---- | ---------------------------------------------------------------------- | ----------------------------------------------------- |
| A-1  | Every action is keyboard reachable in documented order                 | keyboard-only walkthrough of every route in Gate 1 §4 |
| A-2  | An arriving interaction moves focus to the dock                        | assert `document.activeElement` after the event       |
| A-3  | Resolving an interaction returns focus to its prior owner              | focus assertion before/after                          |
| A-4  | A generation bump restores focus to the composer with the draft intact | simulate `runtime.retry`, assert draft and focus      |
| A-5  | Text scale 0.8–2.0 causes no clipping or horizontal scroll             | render each route at both bounds                      |
| A-6  | Reduced motion removes animation without losing information            | render with `prefers-reduced-motion`                  |
| A-7  | Layout usable at the declared minimum window size                      | render each route at that size                        |
| A-8  | Lifecycle failures, terminal outcomes, and interactions are announced  | assert live-region content                            |
| A-9  | Status is never color-only                                             | badge and pill snapshots include text or shape        |
| A-10 | Contrast passes in all three themes                                    | automated contrast check per theme                    |

## 9. Per-component acceptance criteria (selected)

- **C-04 DirectoryChip** — never sends a path; `cancelled` leaves state untouched with no error
  styling; `rejected` shows the `reasonCode`; a `missing` remembered folder reads differently from
  never having chosen one; `remembered: false` is stated out loud during settings recovery.
- **C-05 CredentialChip** — no menu when `catalogStatus === 'denied'`; `unavailable` offers retry and
  diagnostics, not a picker; `relink_required`/`missing` route to handoff, not retry; exactly the
  four safe fields render.
- **C-08 Transcript** — ordered by `updateSeq` under shuffled arrival; duplicates ignored; one
  history/live seam; a gap surfaces C-11; no code path can render an `agent_thought_chunk`.
- **C-12 Composer** — draft survives a generation bump; `preservesDraft: true` keeps the text;
  `preservesDraft: false` clears the banner but not the text; Send blocked past 64 KiB; Stop appears
  only when `phase === 'streaming'` and passes the exact `promptAttemptId`.
- **C-14 PermissionRequest** — buttons render only for the flags present in `summary`; no "always
  allow"; a streamed `tool` update does not dismiss it; double-click sends one response.
- **C-15 ElicitationForm** — renders all six supported types with their bounds; required fields block
  submit; Decline and Cancel are distinct and both terminal.
- **C-21 LifecycleFailureScreen** — buttons derive from `allowedActions`; no retry in
  `relink_required`, `incompatible`, or `fatal`; each state's copy matches Gate 1 §7 verbatim.
- **C-25 FailureBanner** — no raw error string ever reaches the DOM; an undecodable error renders as
  `OPERATION_FAILED`.

## 10. Test plan for Gate 3

1. **Contract tests** — a typed fake of `window.goslingShell` returning each documented shape;
   assert no component reads a field outside the snapshot types.
2. **Reducer property tests** — shuffled/duplicated/gapped `updateSeq` streams always converge on the
   same ordered transcript.
3. **Negative-space tests** — assert the renderer bundle contains no `require`, no Node built-in, no
   direct `ipcRenderer`, and no filesystem or network access; assert no component references an
   undeclared capability channel.
4. **State-matrix rendering** — every state enumerated in Gate 1 renders without throwing, with a
   snapshot per theme × text-scale bound.
5. **Accessibility suite** — A-1…A-10 above.
6. **Live-child integration** — extend the existing `gosling serve` harness so the real renderer
   drives create → prompt → permission → cancel → resume, with no ACP authority in the renderer.
7. **Packaged replay** — fold the GUI into the existing package/readback and close/restart replay;
   assert no orphaned backend and an empty product-local process registry.

## 11. What Gate 3 must add to the host

| ID  | Need                                      | Disposition after Gate 3                                                                                                                                                                                                                                                                            |
| --- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| H-1 | Outputs projection (SHP-DEF-054)          | **implemented.** Rust builds a field-by-field, active-session-only projection with `name`, coarse `kind`, and `relation`; the response is bounded to 100 items and carries `totalCount`/`truncated`. Generated SDK and capability-gated main/preload/UI paths expose no location or file authority. |
| H-2 | Declared minimum window size              | **no change needed.** `createMinimalShellWindowOptions` in `ui/desktop/src/shellHost.ts` already sets `minWidth: 760`, `minHeight: 520`. This gap was wrong; the stylesheet targets those bounds.                                                                                                   |
| H-3 | Renderer-visible capability list          | **implemented.** `ShellRuntimeSnapshot.declaredCapabilities` is a sorted projection of the consumer manifest. Components gate on it rather than inferring from failures.                                                                                                                            |
| H-4 | Handoff preconditions vs `allowedActions` | **closed.** `relink_required` and `incompatible` no longer advertise `handoff`; live-session states retain it. This preserves ADR-0012's server-owned envelope authority.                                                                                                                           |

H-3 mattered because without it C-13/C-14/C-15 could not distinguish "no interaction pending" from
"this consumer cannot answer interactions", and C-18 could not hide cleanly.

Gate 3's build record is [`gate-3-build-record.md`](gate-3-build-record.md). Two design decisions in
this document were overtaken by the shell's CSP during implementation and are recorded there: the
renderer ships without `@vitejs/plugin-react` (its dev preamble is an inline script) and the
stylesheet is linked from `shell.html` rather than imported from JavaScript (a JS import becomes an
inline `<style>` in dev). Both would violate `script-src 'self'` / `style-src 'self'`.

## 12. Exit criteria for Gate 2

- [x] Every operation, event, capability, and bound traced to source.
- [x] Component inventory covers every Gate 1 state.
- [x] Reducer rules cover fencing, ordering, delivery, interaction lifetime, and failures.
- [x] Accessibility criteria are testable.
- [x] Required host additions are named and scoped.
- [x] Accepted and merged as PR #58; implemented by Gate 3.
- [ ] SHP-DEF-053 closed — still open, independent of this document.

## 13. Temporary desktop-parity handoff amendment (2026-08-19)

The renderer now mirrors the normal Gosling desktop frame while retaining `GoslingShellAPI`, its
isolated preload, and the existing declaration-gated store/actions. `SettingsPanel` and all
user-opened settings routing are removed. Local settings may still be read by the store for runtime
appearance and recovered through the bounded reset action, but no Settings, Skills, Extensions, or
global-settings control is rendered.

For the committed Default Shell template, `credential.select` is also absent and fixed credential
policy has no profile ID. `ContextBar` and `SessionPicker` omit Account content in that state. The
credential controller ignores any stale product-local preferred profile when the backend denies
catalog access, so switching a previously selectable development template back to fixed cannot
retain or submit that selection.
