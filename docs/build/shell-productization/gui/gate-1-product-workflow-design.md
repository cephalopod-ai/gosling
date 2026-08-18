# Default Shell GUI — Gate 1: product and workflow design

Status: design deliverable; no renderer implementation is authorized by this document
Date: 2026-08-18
Gate: `plan-webapp-design` Gate 1
Authority: DS-7 acceptance ([`../audits/ds-7-operator-acceptance.md`](../audits/ds-7-operator-acceptance.md)),
ADR-0014, ADR-0010–0013, and
[`../../../architecture/default-shell-template.md`](../../../architecture/default-shell-template.md)
Requirements: SHP-REQ-055, SHP-REQ-056

## 0. Rules this document follows

1. **Nothing is invented.** Every state, operation, field, and bound below cites committed source in
   `ui/desktop/src/shell/**` or `ui/sdk/src/generated/**`. Where the design needs something the host
   does not expose, it appears in §12 as a numbered gap, never as an assumption.
2. **The generic template is the product.** No DAWES, math, Project ABC, or Physics/CST concept
   appears. Domain surfaces exist only as declaration-gated empty slots.
3. **Failure states are first-class.** The foundation can produce 11 lifecycle states, 12 failure
   codes, 10 recovery actions, 4 directory states, 5 settings-recovery states, 3 credential catalog
   statuses, 4 credential selection statuses, 3 module statuses, and 4 adapter statuses. Every one
   has a designed surface. A state with no happy path still gets honest copy.
4. **Honest copy precedes polish.** Copy in this document is the specification, not a placeholder.

## 1. What this product is

A reduced Gosling desktop application, larger than a workspace chat window, that a user opens to do
one task at a time inside one folder they chose, with an agent whose instructions the product owns.

It is *not* a Gosling replacement, a workspace manager, a settings editor, a file browser, an IDE, or
a multi-session console. Every one of those is deliberately absent from the v1 capability envelope,
and the UI must not imply their existence.

The single defining constraint on the whole design: **the renderer has no authority.** It cannot read
a file, resolve a path, hold a secret, open a backend, spawn a process, or answer a question the main
process did not hand it. It renders projections and returns fenced decisions. Every screen below is
designed from what main will actually give it.

## 2. Who uses it and what they are trying to do

| User | Their job | What the design owes them |
| ---- | --------- | ------------------------- |
| Operator running the neutral template | Prove the shell works end to end before a named product exists | Every state reachable and legible; diagnostics one click away |
| A future shell product's end user | Get one task done in one folder without learning Gosling | A first-run path with no dead ends and no jargon |
| A future shell product's builder | See exactly which surfaces are theirs to replace | Declaration-gated slots that visibly disappear when undeclared |

## 3. Window and information architecture

One window. No tabs, no multi-pane workspace, no floating panels. Minimum window size is an
acceptance requirement (§11).

```
┌──────────────────────────────────────────────────────────────────────┐
│ TITLE BAR — product displayName + version   [status pill]            │  A
├──────────────────────────────────────────────────────────────────────┤
│ CONTEXT BAR                                                          │  B
│  📁 <folder label>  ▾ Change    🔑 <account name>  ▾ Change          │
│  ⓘ <provider> · <model>            [Sessions ▾]  [Settings] [Help ▾] │
├──────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  CONVERSATION                                                        │  C
│   · history block (delivery: 'history')                              │
│   · ─── resumed here ───                                             │
│   · live blocks (delivery: 'live')                                   │
│   · tool activity rows                                               │
│   · [transcript gap notice + Repair]                                 │
│                                                                      │
├──────────────────────────────────────────────────────────────────────┤
│  INTERACTION DOCK  (permission / form / confirmation — at most one)   │  D
├──────────────────────────────────────────────────────────────────────┤
│  COMPOSER   [ text area ]                      [Send] / [Stop]       │  E
│  <failure banner, inline, draft preserved>                            │
├──────────────────────────────────────────────────────────────────────┤
│  MODULES STRIP (only when non-core modules or an adapter exist)       │  F
└──────────────────────────────────────────────────────────────────────┘
```

Region ownership:

| Region | Source of truth | Notes |
| ------ | --------------- | ----- |
| A title bar | `snapshot.identity`, `snapshot.lifecycleState` | `identity` is `null` until ACP is verified — show the packaged name from the profile only, never a guess |
| B context bar | `snapshot.directory`, `snapshot.credentials`, `snapshot.session` | provider/model come from `session.providerId`/`session.modelId` and are `null` before a session exists |
| C conversation | `session.transcript.read` + `session.onUpdated` | reconciled by `updateSeq`, never by arrival order |
| D interaction dock | `snapshot.pendingInteractions` + the three `on*Requested` events | at most one visible; queue depth shown if >1 |
| E composer | renderer-local draft + `snapshot.session.promptAttempt` | draft is renderer-owned and survives generation bumps |
| F modules strip | `snapshot.modules`, `snapshot.adapter` | hidden entirely when only `core:session` is present |

## 4. Screen and state inventory

Screens are shells around the same window. IDs are stable for Gate 2 traceability.

| ID | Screen / state | Entry condition (source) | Primary action | Secondary |
| -- | -------------- | ------------------------ | -------------- | --------- |
| S-01 | Starting | `lifecycleState` `booting` | none | Stop |
| S-02 | Checking your setup | `validating` | none | Stop, Save diagnostics |
| S-03 | Choose a folder | `ready` and `directory.state === 'unselected'` | Choose folder | Settings, Help |
| S-04 | Choose an account | `ready`, directory selected, `credentials.selectionStatus === 'none'` and `catalogStatus === 'available'` | Choose account | Change folder |
| S-05 | Sessions | `ready`, directory + credential settled, no active session | Start new task | Resume a listed session |
| S-05b | Sessions — empty | as S-05, list length 0 | Start new task | — |
| S-06 | Conversation (idle) | `session.status === 'active'`, `promptAttempt === null` | Send | Stop task, Sessions, Settings |
| S-07 | Conversation (streaming) | `promptAttempt.phase === 'streaming'` | Stop | — |
| S-08 | Conversation (cancelling) | `promptAttempt.phase === 'cancelling'` | none | — |
| S-09 | Permission request | pending interaction `kind: 'permission'` | Allow once | Deny |
| S-10 | Form request | pending interaction `kind: 'elicitation'` | Submit | Decline, Cancel |
| S-11 | Action confirmation | pending interaction `kind: 'confirm'` | Approve | Reject |
| S-09q | Queued interactions | `pendingInteractions.length > 1` | resolve the front one | — |
| S-12 | Transcript gap | `ShellTranscriptSnapshot.integrity !== 'complete'` or `truncated` | Repair | Continue anyway |
| S-13 | Resume uncertainty | `session.resumeIntegrity === 'uncertain'` | Continue | Start new task instead |
| S-14 | Needs attention (degraded) | `degraded` | Retry | Save diagnostics, Open in Gosling |
| S-15 | Reconnect an account | `relink_required` | Open in Gosling | Save diagnostics, Quit |
| S-16 | Version mismatch | `incompatible` | Open in Gosling | Save diagnostics, Quit |
| S-17 | Can't reach the backend | `offline` | Retry | Save diagnostics, Quit |
| S-18 | Something went wrong | `fatal` | Save diagnostics | Quit |
| S-19 | Shutting down | `stopping` | none | Save diagnostics |
| S-20 | Stopped | `stopped` | Restart | Quit |
| S-21 | Settings | user opens from B | Close | Reset settings |
| S-22 | Settings need review | `settingsRecovery.status !== 'loaded'` and `!== 'absent'` | Reset settings | Continue without saving |
| S-23 | Folder unavailable | `directory.state` `missing` or `invalid` | Choose folder | Save diagnostics |
| S-24 | Account unavailable | `credentials.selectionStatus` `missing` or `relink_required` | Choose account | Open in Gosling |
| S-25 | Setup problems | `provisioningIssues.length > 0` while otherwise usable | Save diagnostics | Open in Gosling |
| S-26 | Module unavailable | any `modules[].status !== 'ready'` | Save diagnostics | dismiss |
| S-27 | Adapter unavailable | `adapter.status !== 'ready'` | Save diagnostics | dismiss |
| S-28 | Hand off to Gosling | user chose handoff | Open Gosling | Cancel |
| S-29 | Outputs | **gated — see §12 gap G-1** | — | — |

Three failure overlays are not screens but states any conversation route can enter. They are
enumerated separately because Gate 2 gives them their own component (C-25) and the wireframe shows
them:

| ID | Overlay | Entry condition | Affordance |
| -- | ------- | --------------- | ---------- |
| F-01 | Failure, draft preserved | decoded failure with `preservesDraft: true` | the `recovery` affordance; composer text retained |
| F-02 | Failure, stale request | `code === 'STALE_REQUEST'` | quiet Refresh; never a modal |
| F-03 | Failure, capability unavailable | `code === 'CAPABILITY_UNAVAILABLE'` | Open in Gosling |

## 5. The nine-step path, walked

This is the workflow the corrective audit certified the backend can support. Each step names the real
operation and the real thing that can go wrong.

### Step 1 — Start and show verified facts

`booting` → `validating` → `ready`. The title bar shows the packaged product name immediately, but
the *verified* identity badge appears only once `snapshot.identity !== null`, which happens only
after ACP preflight succeeds. `compatibility.status` moves `unverified` → `compatible`.

Design consequence: never show "connected" during `validating`. S-02 says what is happening —
"Checking your setup" — and offers Save diagnostics after the first few seconds, because
`validating` allows `diagnostics` and a stuck validate is the most likely first-run complaint.
ACP preflight has a 10-second timeout (`ACP_PREFLIGHT_TIMEOUT_MS`), so S-02 must not look hung
before then.

### Step 2 — Choose or restore one validated folder

`directory.select` opens Electron's native chooser in main. The renderer sends `{ generation,
userGesture: true }` and no path, ever. Three results, all normal:

- `cancelled` — return to S-03 unchanged, no error styling. Cancel is success.
- `selected` — the context bar shows `directory.label` (a basename, max 128 chars), with the full
  `directory.path` available on hover/focus for disambiguation.
- `rejected` — S-23 with the `reasonCode`, and the design must not editorialize: show the honest
  reason and re-offer the chooser.

On relaunch, main restores the remembered directory *before* the compatibility gate. If the
remembered folder is gone, `directory.state` is `missing` and `remembered` may be `true` — S-23 must
distinguish "the folder you used last time is gone" from "you haven't picked one yet". Switching
folders while a session exists requires `session.detach` first; the UI must ask for that explicitly
(§6, D-3) rather than silently detaching.

### Step 3 — Choose a safe credential reference, or hand off

Behavior depends on the product's provisioned `credentialPolicy`:

| `catalogStatus` | Meaning | Surface |
| --------------- | ------- | ------- |
| `available` | `selectable_catalog`; up to 128 four-field profiles | S-04 picker, and a Change control in B |
| `denied` | `fixed` profile; the product chose the account | No picker at all. B shows the pinned account name, not a menu |
| `unavailable` | catalog read failed or timed out fail-closed | B shows "Accounts unavailable", Retry, and Save diagnostics |

Each profile shows exactly `name`, `providerOrServiceId`, and `status`. There is no fourth column,
no secret, no field list, and no "test connection" — those are Gosling's.

`selectionStatus` drives S-24: `relink_required` and `missing` both mean the shell cannot fix it.
The only honest action is Open in Gosling (handoff), and the copy must say so rather than offering a
retry that cannot work.

### Step 4 — Discover, then create or resume

`session.list` returns at most **20** active ACP sessions whose `workingDir` equals the accepted
canonical directory. S-05 is therefore a short list, not a searchable archive, and the copy must not
promise history beyond it: "Recent tasks in this folder (up to 20)".

Each row shows `title` (or a neutral fallback), `updatedAt`, and `messageCount`. `resume` may fail
with `SESSION_UNAVAILABLE` — including when a renderer-known ID points outside the accepted
directory, which main refuses. S-05 treats that as "That task isn't available here" and refreshes
the list, never as a crash.

### Step 5 — Reconcile history, live updates, and gaps

Resume buffers bounded ACP history, activates, then emits explicit `history`-delivery updates before
live ones. The conversation renders a single visual seam between them.

`readTranscript` returns `integrity` of `complete`, `incomplete`, or `resume_uncertain`, plus
`truncated`, `firstSeq`, `lastSeq`. Ledger bounds are **256 updates / 48 KiB**. S-12 shows a
persistent, non-modal notice — "Part of this conversation isn't shown" — with Repair. S-13 handles
`resumeIntegrity === 'uncertain'`, which means an earlier prompt was interrupted and its outcome is
unknown. The copy must not claim the work completed or failed: "The last request may not have
finished. Check the result before repeating it."

### Step 6 — Send and cancel while preserving the draft

`prompt.submit` takes `{ generation, sessionId, text }`, max **64 KiB**, and returns a
`promptAttemptId`. `prompt.cancel` needs that exact ID.

`preservesDraft` on a failure is a contract the UI must honor literally: when it is `true`, the
composer keeps the text and the failure appears as an inline banner above it. When `false`, the
failure is about the shell rather than the message. **Retry always clears the generation** (§7), so
the renderer must hold the draft in its own state, not in anything generation-scoped.

### Step 7 — Answer permissions, forms, and confirmations

At most one interaction is presented at a time; extras show as "1 more waiting". Every response is
single-use and fenced by `actionId` + `generation` + `sessionId`.

- **Permission (S-09).** Shows `toolTitle`, an `effect` badge of `read`/`write`/`execute`/`network`/
  `other`, `targets` as basenames, and `inputFields` as names only. Buttons appear only when
  `summary.allowOnce` / `summary.deny` are true. There is deliberately no "always allow" — the shell
  has no persistent permission store, and offering one would be a lie.
- **Form (S-10).** Renders only the supported field types: `string`, `number`, `integer`, `boolean`,
  `multi_select`, with optional `format` of `email`/`uri`/`date`/`date-time` and min/max bounds.
  Bounds are **32 fields / 64 options / 8 KiB**. Secret-shaped or schema-invalid forms are cancelled
  by main and never reach the renderer, so S-10 needs no "this looks sensitive" state.
- **Confirmation (S-11).** Shows `action` and `inputFields` names. This is the mutation gate; the
  copy names the action and states that it is not reversible by the shell.

Critical rule from SHP-DEF-052: streamed tool progress must never dismiss a pending interaction. The
dock clears only on a *terminal* session outcome — `completed`, `cancelled`, or `failed`.

Interaction IDs are capped at **4,096 per runtime**. On exhaustion, new interactions fail closed and
the only honest surface is S-14 with Retry, which restarts the runtime.

### Step 8 — Show declared modules and Outputs

The modules strip is hidden when the inventory is only `core:session`. Otherwise each module shows
`id`, `kind` (`core`/`extension`/`skill`/`adapter`), `status` (`ready`/`unavailable`/`incompatible`),
and optional `version`. A provisioned-but-unresolved module appears as `unavailable` rather than
vanishing — that visibility is the point (S-26).

An adapter, when present, shows `descriptorId`, `protocolVersion`, its `actions` names, and status.
Adapter mutations route through S-11. Adapter payload rendering is consumer-owned and data-only; the
generic template ships an empty slot.

**Outputs is not designed in this gate.** See §12 gap G-1.

### Step 9 — Recover, or leave honestly

Three exits, in escalating order:

1. **Retry** — only where the lifecycle allows it (`degraded`, `offline`).
2. **Save diagnostics** — user-initiated, requires `userGesture: true`, writes a redacted bundle
   (max 1 MiB) and refuses to overwrite. Result is `saved` with a filename, or `canceled`. Telemetry
   is off; this is the only export.
3. **Open in Gosling** — `handoff.prepare` then `handoff.confirm`, then `external.open`. S-28 shows
   what travels: the question, the origin identity, and reference IDs. It must state plainly that
   the shell cannot complete the task and Gosling can.

## 6. Decisions the UI must ask rather than assume

| ID | Situation | Why it cannot be silent |
| -- | --------- | ----------------------- |
| D-1 | Reset settings | `settings.reset` needs `userGesture: true` and discards the local document |
| D-2 | Save diagnostics | needs `userGesture: true`; writes a file to the user's disk |
| D-3 | Change folder with an active session | requires `session.detach` first, which releases the local slot |
| D-4 | Any adapter mutation | server-owned single-use confirmation; the shell cannot undo it |
| D-5 | Hand off to Gosling | leaves the shell and carries context to another application |
| D-6 | Restart after `stopped` | begins a new generation and abandons runtime state |

## 7. Lifecycle → surface map

`retry` is not a reconnect. Source (`runtimeController.retry`) stops the runtime, increments the
generation, re-enters `booting`, and starts fresh. Everything generation-fenced is lost: the session,
all pending interactions, the transcript ledger. Only the renderer's own draft and the persisted
settings survive. The UI must set that expectation before the user clicks Retry, and must restore
focus to the composer with the draft intact afterwards.

| State | Screen | Actions the host allows | Copy | Recoverable in place? |
| ----- | ------ | ----------------------- | ---- | --------------------- |
| `booting` | S-01 | stop | "Starting <name>…" | — |
| `validating` | S-02 | stop, diagnostics | "Checking your setup…" | — |
| `ready` | S-03…S-06 | stop, diagnostics, handoff | — | — |
| `busy` | S-07 | stop, diagnostics | — | — |
| `degraded` | S-14 | retry, stop, diagnostics, handoff | "<name> started with a problem it couldn't work around." | Retry restarts the runtime |
| `relink_required` | S-15 | stop, diagnostics, handoff | "An account this shell needs isn't connected any more. Reconnect it in Gosling, then start <name> again." | **No.** No retry is allowed |
| `incompatible` | S-16 | stop, diagnostics, handoff | "This version of <name> doesn't match the Gosling core it shipped with." | **No** |
| `offline` | S-17 | retry, stop, diagnostics | "<name> can't reach its backend." | Retry restarts the runtime |
| `stopping` | S-19 | diagnostics | "Shutting down…" | — |
| `stopped` | S-20 | none | "<name> has stopped." | Restart begins a new generation |
| `fatal` | S-18 | stop, diagnostics | "<name> hit a problem it can't recover from." | **No** |

Note the asymmetry: `degraded`, `relink_required`, `incompatible`, and `offline` can only transition
onward to `stopping` or `fatal`. There is no path back to `ready` within a generation. Any UI that
implies "we'll reconnect shortly" would be false.

Startup failures classify by cause (`startupFailureName`): a compatibility error becomes
`incompatible`; a `PROVISIONING_INVALID` error becomes `relink_required` when its issues include
`missing_credential_profile`, `credential_profile_unavailable`, or `credential_provider_mismatch`,
and `degraded` otherwise; an adapter descriptor mismatch becomes `incompatible`; everything else
becomes `offline`.

## 8. Failure code → copy and recovery map

All twelve `ShellOperationFailure` codes. `message` from main is already safe and user-facing; the
UI shows it and adds only the affordance named by `recovery`.

| Code | `recovery` value | Where it surfaces | Affordance the UI shows | Draft |
| ---- | ---------------- | ----------------- | ----------------------- | ----- |
| `CAPABILITY_UNAVAILABLE` | `open_gosling` | inline at the attempted control | Open in Gosling | no |
| `CREDENTIAL_REQUIRED` | `select_credential` | composer banner / B | Choose account | preserved on submit |
| `DIRECTORY_REQUIRED` | `choose_directory` | composer banner / B | Choose folder | preserved on submit |
| `INTERACTION_PENDING` | `review_session` | composer banner | Scroll to the pending request | preserved on submit |
| `INVALID_INPUT` | `none` | composer, field-level | none — fix the text | always preserved |
| `INVALID_REQUEST` | `none` | inline | none | preserved on submit |
| `OPERATION_FAILED` | `save_diagnostics` | inline | Save diagnostics | preserved on submit |
| `RUNTIME_UNAVAILABLE` | `retry` | banner across C | Retry (`retrySafe: true`) | preserved on submit |
| `SESSION_BUSY` | `review_session` | composer banner | Review the current task | preserved on submit |
| `SESSION_UNAVAILABLE` | `review_session` | S-05 or C | Back to Sessions | preserved on submit |
| `SETTINGS_RECOVERY_REQUIRED` | `reset_settings` | S-21/S-22 | Reset settings | no |
| `STALE_REQUEST` | `refresh` | quiet inline notice | Refresh | preserved on submit |

`classifyShellOperationFailure` emits nine of the ten `ShellRecoveryAction` values. The tenth,
`restart`, is never produced by a failure — it is reachable only through the lifecycle path, as
S-20's Restart control. An implementer must still map all ten, because a future classifier change
must not be able to produce an affordance the UI cannot render.

`STALE_REQUEST` deserves care: it means the generation moved under a pending action. It is normal
during retry and shutdown, so it must be quiet — a low-emphasis inline line, never a modal.

## 9. Settings

Four editable things, and nothing else: theme (`system`/`light`/`dark`), text scale (**0.8–2.0**),
plus the remembered folder and preferred account reference, which are set by choosing them rather
than typed. There is no key-value editor, no global Gosling settings, no telemetry toggle, and no
developer-tools switch. S-21 must look small on purpose.

Recovery states (`settingsRecovery.status`):

| Status | Surface | Copy |
| ------ | ------- | ---- |
| `loaded` | normal | — |
| `absent` | normal | first run; defaults apply silently |
| `unsupported_schema` | S-22 | "These settings were written by a newer version of <name>." |
| `malformed` | S-22 | "<name>'s local settings can't be read." |
| `unreadable` | S-22 | "<name> doesn't have permission to read its own settings." |

The last three refuse writes until an explicit reset. Crucially, the shell must stay usable: a
directory chosen during recovery applies to the running process and reports `remembered: false`. The
UI must say that out loud — "This folder won't be remembered until settings are reset" — rather than
appearing to save and silently forgetting.

## 10. First run

The honest first-run sequence, with no wizard chrome and no marketing:

1. S-01/S-02 while the backend starts.
2. S-03 — one sentence explaining why a folder is needed: "<name> works inside one folder you
   choose. It can read and change files there, and nowhere else."
3. S-04 if and only if `catalogStatus === 'available'` and nothing is selected.
4. S-05 with an empty list — "No tasks here yet" and a single Start new task.
5. S-06 with a composer placeholder describing the shell's actual scope.

If any of steps 2–4 cannot be satisfied, the user lands on the matching failure screen with a real
action. There is no state in which the window shows an enabled composer that cannot send.

## 11. Cross-cutting requirements

These are acceptance criteria, not polish, per `default-shell-template.md`. Gate 2 makes each one
testable.

- Keyboard: every action reachable in a documented tab order; the interaction dock takes focus when
  an interaction arrives and returns it to the composer on resolution.
- Focus restoration across generation bumps, since retry destroys runtime state.
- Reduced motion honored; streaming must not require animation to be legible.
- Text scale 0.8–2.0 without clipping or horizontal scrolling.
- Minimum window size below which the layout is still usable.
- Screen-reader announcements for arriving interactions, terminal prompt outcomes, and lifecycle
  changes into a failure state.
- Contrast sufficient in all three themes.
- No copy that names a capability the envelope excludes.

## 12. Gaps — design decisions the host cannot yet support

| ID | Gap | Consequence | Owner |
| -- | --- | ----------- | ----- |
| G-1 | Outputs has no renderer projection. `_gosling/unstable/session/artifacts/list` exists under ADR-0013, but there is no shell IPC channel, no `GoslingShellAPI` namespace, no capability entry, and no snapshot field. | S-29 is specified as a slot only. The Outputs surface cannot be designed in detail or built until a narrow main-owned operation exists. A directory scan or generic passthrough is forbidden. | Gate 3, tracked as SHP-DEF-054 |
| G-2 | No operation clears a selected directory. `ShellDirectoryController.clear()` is internal. | S-23 can re-choose but cannot return to `unselected`. Accepted; the design does not offer "forget this folder". | none — recorded as intentional |
| G-3 | No in-shell credential relink. Relink is handoff plus `external.open`. | S-15 and S-24 must route to Gosling rather than offering an inline fix. | none — matches ADR-0014 |
| G-4 | Module inventory refreshes only via the runtime snapshot; there is no `modules.refresh`. | S-26 shows state but offers no direct re-check; Retry is the only refresh. | acceptable for v1 |
| G-5 | Gemini OAuth provider configuration fails with `Internal error` (`docs/TODO.md`). | No claim of a polished credential/relink experience until closed. | outside this campaign |

## 13. Negative space — what this UI must never offer

Direct from the v1 envelope. A control matching any of these is a design defect, not a feature:

developer tools by default; global Gosling setting mutation; credential creation, editing, or
testing; secret display; arbitrary file access or a file tree; arbitrary backend URLs; process
spawning; generic IPC/RPC; updater controls; plugin installation; more than one concurrent session;
autonomous background execution; "always allow" permissions; telemetry opt-in; any named-domain
behavior.

## 14. Exit criteria for Gate 1

- [x] Every lifecycle, failure, recovery, directory, credential, settings-recovery, module, and
      adapter state has a designed surface.
- [x] Every operation referenced exists in `GoslingShellAPI`.
- [x] Every bound quoted matches a committed constant.
- [x] Cancel, decline, and stale are designed as normal outcomes.
- [x] Gaps are recorded as gaps.
- [x] No named-shell or out-of-envelope surface appears.
- [ ] Operator review of this document.

Gate 2 handoff: [`gate-2-frontend-handoff.md`](gate-2-frontend-handoff.md).
Wireframe: [`default-shell-wireframe.html`](default-shell-wireframe.html).
