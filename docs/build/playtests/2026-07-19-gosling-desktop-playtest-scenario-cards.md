# Gosling Desktop scenario cards — 2026-07-19

These cards are the executed and partially executed representatives of the
500-case matrix. Statuses distinguish observed behavior from unexecuted
permutations.

## PLY-001 — create workspace

- Goal: create a workspace with a primary folder and a named output.
- Category: workspace / P0
- Preconditions: disposable app root; local provider configured.
- Inputs: `Playtest Workspace`, primary `/tmp/gosling-playtest.zVEbFP/work`, output `Outputs`.
- Steps: open Workspaces `+`; fill General and Product outputs; choose Validate; save.
- Expected: validation succeeds, workspace persists, sidebar row shows output count.
- Actual: PASS. Row showed `Playtest Workspace`, `0 refs · 1 outputs`.
- Confirmation: directly observed through Electron UI.
- Reproducibility: deterministic in isolated root.

## PLY-002 — explicit workspace on new chat

- Goal: select a workspace for a new chat without changing an open session.
- Category: session / P0
- Preconditions: PLY-001 complete.
- Steps: open New Chat; select Playtest Workspace; inspect working-folder text and submit.
- Expected: selected folder is used and session header identifies the workspace.
- Actual: PASS. Folder displayed `/tmp/gosling-playtest.zVEbFP/work`; header displayed Playtest Workspace.

## PLY-003 — archive recovery

- Goal: archive a session when no archive folder is configured.
- Category: persistence / P1
- Steps: open session actions; choose Archive session.
- Expected: archive is either saved or a clear setup/retry state is shown.
- Actual: PASS recovery state. Settings opened and displayed `No archive folder configured yet` with `Choose folder`.
- Follow-up: configure a disposable archive folder and rerun the successful-write branch.

## PLY-004 — credential form secrecy

- Goal: ensure profile forms use transient password state.
- Category: credentials / security / P0
- Steps: Manage profiles → New profile; select Sandbox Local; enter sentinel `PLAYTEST_SENTINEL_ONLY_NOT_REAL`; cancel.
- Expected: sentinel is never persisted or shown after cancel.
- Actual: PASS. After cancel, no password input remained and the sentinel was absent from body text.

## PLY-005 — model switch cancel

- Goal: open the in-session model picker and cancel without reassigning the session.
- Category: model/session / P1
- Steps: click current model; open Change Model; cancel.
- Expected: dialog is accessible and the session remains on its current model.
- Actual: PASS for open/cancel. A full alternate-provider switch was not confirmed because the disposable fixture inventory initially did not match its configured model; this is recorded as partial, not a product failure.

## PLY-006 — slow generation and queue

- Goal: queue a follow-up while a response is streaming.
- Category: queue/steer / P0
- Steps: send a slow prompt; while `gosling is working on it…` is visible, send a follow-up.
- Expected: current generation remains active and the follow-up appears in Message Queue.
- Actual: PASS. UI showed `Message Queue`, `1 message queued`, and the follow-up text.
- Follow-up: rerun with a clean compatible fixture and capture the send-now/steer action.

## PLY-007 — artifact link and preview (repaired)

- Goal: open a generated document directly in the right sidebar and preview it.
- Category: artifacts / P0
- Preconditions: workspace output exists at `/tmp/gosling-playtest.zVEbFP/work/Outputs/report.md`.
- Steps: submit a response containing the canonical artifact path; click `Open report.md in Outputs`.
- Expected: Outputs pane opens and renders the file content.
- Initial actual: FAIL; see PLY-GOS-001. The pane opened but showed `Preview unavailable` and `Renderer file access denied for path outside approved roots`.
- Repair verification: PASS after canonical workspace-output authorization and bounded retry; the persisted report content rendered after restart.

## PLY-008 — close Outputs pane (repaired)

- Goal: close the pane opened by an artifact link.
- Category: artifacts / P1
- Steps: click `Close outputs pane` after PLY-007.
- Expected: pane closes by pointer or keyboard activation.
- Initial actual: FAIL; see PLY-GOS-002. The pointer target was intercepted by the titlebar drag region; Enter did not close it.
- Repair verification: PASS after the pane received a higher stacking context and `no-drag` controls; pointer activation closed the pane.

## PLY-009 — restart and resume

- Goal: verify workspace/session persistence across a desktop restart.
- Category: loading/resume / P0
- Steps: stop the dev app; relaunch with the same disposable user-data root; inspect sidebar and current thread.
- Expected: workspace and prior session remain readable without migration prompts.
- Actual: PASS. Playtest Workspace and the prior artifact-thread content were present after relaunch.

## PLY-010 — provider outage recovery

- Goal: expose a recoverable send failure.
- Category: recovery / P1
- Steps: stop the local fixture and submit a prompt.
- Expected: visible error and resend/retry action, no silent loss.
- Actual: PASS recovery UI. Gosling showed a network error and `Please resend your message to try again`. The outage was intentionally induced.
