# Gosling Desktop playtest continuation — 2026-07-19

Authority: `audit-playtest-app` report-only continuation of the capped
500-permutation matrix in `2026-07-19-gosling-desktop-playtest-matrix.json`.
This document records only interactions observed in the disposable Electron
profile; generated permutations are not implied to pass.

## Run boundary

- Repository: `/Users/eric/Work/vscode/forked/gosling`
- Runtime root: `/tmp/gosling-playtest.zVEbFP`
- Launch: existing Electron Forge development command with an isolated user
  data directory and Playwright remote debugging; no install, rebuild, or
  reinstall was performed.
- Inputs: fake local providers on loopback only, disposable folders, and a
  transient fake credential value. No real credentials or external sites were
  used.
- Cleanup: the Electron dev process and loopback fixtures were stopped. Ports
  9237 and 45678–45683 were free afterward. The pre-existing installed Gosling
  backend was left running intentionally.

## Continuation coverage

| Scenario | Result | Evidence |
| --- | --- | --- |
| Credential profile create, secure save, bind, and persistence | PASS | Created a profile through the workspace editor, entered a fake password value, saved it, bound it to Playtest Workspace, and observed only metadata (`configured`) in the renderer and `workspaces.json`. A repository-wide sandbox search found no fake value. |
| Missing primary and optional folder validation | PASS / PARTIAL | Validation displayed `primary working folder is unavailable; relink it before starting a session` and `optional workspace folder is unavailable`. The workspace row and New Chat selector showed `needs attention`. The selector still allowed text entry and the Send control became enabled; submitting produced no new session row or actionable visible error. See PLY-GOS-003. |
| Switch workspace while a session is visible | PASS / UX NOTE | Switching to Default changed the active workspace while the visible session header remained `Playtest Workspace`; the session was not reassigned. No visible `new chats use ...` indication was present. See PLY-GOS-004. |
| New-workspace defaults | PASS | A new workspace editor preselected `/Users/eric/Work`, `ChatGPT Codex`, and `gpt-5.6-terra`. The reasoning selector correctly displayed `Not available for this model` for that model. |
| Mid-session provider/model switch | PASS | The model picker exposed `Sandbox Alternate`; switching changed the session header from `custom_sandbox_local playtest-model` to `Sandbox Alternate playtest-alt`. A follow-up prompt returned `Alternate provider response.` The session remained the same workspace. |
| Queue steering/send-now | PARTIAL | A long loopback SSE fixture and follow-up prompts were exercised. The response completed/buffered before a stable queue control state could be captured, so no send-now/steer activation is credited. The earlier basic queue-display representative remains the only confirmed queue pass. |
| Archive confirmation and file write | PARTIAL / NOT CREDITED | The archive confirmation UI opened with the configured folder text. The folder was supplied through the existing settings bridge because the native chooser cannot be driven by the Playwright channel; the subsequent write was correctly rejected by the renderer access boundary and no archive file was claimed. Native chooser + successful write remains unexecuted. |
| Credential-profile deletion dependency warning | BLOCKED | Clicking Delete opened a native confirmation dialog that could not be safely driven through the available GUI channel. No deletion was performed and no result is credited. |
| Additional source/reference folders and multiple output destinations | NOT EXECUTED | The editor controls were discovered, but the remaining native directory-selection branches were not completed. No physical folder was removed or altered. |
| Native “Save a copy” | NOT EXECUTED | Deferred because it requires a native save chooser; no success or failure is claimed. |

## Findings

### PLY-GOS-003 — missing primary folder has no actionable start error (RESOLVED)

- Severity: Medium
- Confidence: Confirmed, runtime-observed
- Reproduction:
  1. Create a workspace with primary folder
     `/tmp/gosling-playtest.zVEbFP/work/MissingPrimary` and optional output
     `/tmp/gosling-playtest.zVEbFP/work/MissingOutputs`.
  2. Run Validate. The editor shows the required and optional unavailable
     messages and permits Save; the sidebar marks the workspace as needing
     attention.
  3. Select `Missing Paths Workspace` in New Chat, type a prompt, and submit.
- Expected: starting is blocked with a clear relink/replace action before or
  during submission.
- Actual: the workspace selector shows `needs attention`, but the composer
  accepts text and enables Send. The attempted submission produced no visible
  error and no new session row was observed in the isolated sessions database.
- Impact: a user can believe a chat started while the app silently does
  nothing, requiring them to infer that the folder must be relinked.
- Follow-up: disable submission for a required-invalid workspace or surface a
  blocking inline error/toast with a direct Relink action.
- Repair: Hub now renders the structured validation issue and passes a guard and
  reason into ChatInput. Click, form-submit, and Enter-key paths are blocked.
  Regression: `Hub.test.tsx` invalid-workspace case passes.

### PLY-GOS-004 — workspace switch lacks a visible future-session indication (RESOLVED)

- Severity: Low
- Confidence: Confirmed, runtime-observed
- Reproduction:
  1. Open a session pinned to `Playtest Workspace`.
  2. Switch the active workspace to `Default` from the sidebar.
- Expected: the visible session stays pinned and a small non-blocking notice
  says that new chats will use Default.
- Actual: the session header stayed correctly pinned to `Playtest Workspace`,
  but no future-session notice or equivalent text appeared in the visible UI.
- Impact: users may not realize that New Chat defaults changed while the open
  thread intentionally did not.
- Follow-up: show a transient status near the composer or workspace selector;
  keep it non-blocking and do not mutate the pinned session.
- Repair: the sidebar now retains an accessible `role=status` notice alongside
  the existing toast. Regression: `WorkspaceSidebarSection.test.tsx` verifies
  the notice after switching.

## Security observations

- The fake credential was entered into a password field and was cleared after
  save; renderer responses exposed profile metadata only.
- The fake value did not appear in workspace persistence, session metadata, or
  the isolated runtime search.
- Directly assigning an archive path through the settings bridge did not bypass
  the renderer file-access boundary; main-process logs showed the expected
  `Renderer file access denied for path outside approved roots` errors. This is
  a boundary observation, not a product finding, because the native chooser
  branch was not executed.

## Remaining matrix accounting

The matrix still contains 500 generated permutations. The original report
credits 10 interactive representatives. This continuation adds 10 bounded
scenario representatives (5 pass-oriented, 2 partial, 1 blocked, and 2 not
executed). These representatives overlap some matrix dimensions, so the
unique permutation count is not reduced mechanically; the remaining cases are
not credited as passes. The native chooser, archive-write,
credential-delete, product-output, and queue-steer branches require a
follow-up run with native UI control and a deterministic streaming fixture.
