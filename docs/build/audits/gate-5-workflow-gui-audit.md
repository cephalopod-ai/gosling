# Gate 5 workflow / GUI integrity audit

Skill: `audit-workflow-gui` v3.1
Authority: read-only audit; repairs were applied only after the audit inventory closed
Scope: Desktop Workspaces sidebar, editor, credential manager, session filter/header,
new-chat defaults, Electron folder/export operations, multi-window invalidation, and the
full Desktop regression suite
Budget: all changed Desktop workflow files plus their ACP/backend guards

## Workflow truth inventory

| View or control           | Backend effect                              | Shown status                            | Actual source                                  | Failure feedback                   | Disabled-path backend guard                 |
| ------------------------- | ------------------------------------------- | --------------------------------------- | ---------------------------------------------- | ---------------------------------- | ------------------------------------------- |
| Workspace row             | set active workspace                        | future-chat toast + active marker       | ACP response then refetch                      | actionable toast                   | unknown IDs rejected                        |
| All / workspace filter    | list query only                             | pressed/selected row                    | local preference + ACP query                   | list load error                    | not a mutation                              |
| Workspace save            | create/update metadata                      | saved or saved-with-warnings            | validation + completed ACP mutation            | inline error                       | backend validates invariants                |
| Duplicate                 | create one copy                             | named success toast                     | returned duplicate                             | actionable toast                   | backend ID/name checks                      |
| Delete workspace          | delete metadata only                        | preservation confirmation + success     | completed ACP mutation                         | actionable toast                   | only/default workspace rejected             |
| Reveal folder             | shell reveal                                | no success claim                        | Electron boolean                               | actionable toast                   | no mutation                                 |
| Export metadata           | save one non-secret JSON file               | success only after write                | ACP document + Electron write boolean          | actionable toast                   | secret fields rejected backend-side         |
| Create output             | create selected directory                   | named success                           | ACP result validation                          | actionable toast                   | `create_if_missing`, native path, ID checks |
| Credential save           | secure profile create/update                | refreshed profile status                | backend-derived secure presence                | redacted inline error              | source/profile guards                       |
| Credential delete         | secure delete after dependency confirmation | profile disappears after refetch        | usage response + completed delete              | redacted inline error              | `confirmReferenced` checked backend-side    |
| New chat                  | create pinned session                       | chat opens + pinned header badge        | ACP session response / SQL snapshot            | existing create-session error path | backend revalidates workspace/profile       |
| Switch while chat visible | future default only                         | explicit pinned/new-default distinction | session snapshot + active workspace            | switch failure toast               | backend never rewrites session              |
| Extension always-allow    | resolve one request + persist tool policy   | always-allowed status                   | pending request consumption + permission write | stale/failure status               | consuming resolve is the liveness guard     |

## Boundary map

- `WorkspaceContext` is the renderer state boundary; every durable mutation goes through typed
  generated SDK calls and refetches backend truth.
- local storage contains only the harmless collapsed/filter preferences. It never stores a
  workspace or credential profile.
- the session list remains the existing navigation-session store. Workspace filtering is a query
  input, and optimistic rows project the pinned session snapshot.
- Electron directory/save/reveal bridges return cancel/boolean/error results; success must derive
  from those results.
- destructive workspace/profile operations require a user confirmation and are independently
  rejected by backend invariants.

## Findings

### WFG-GOS-001: Folder operations could fail without operator feedback

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow / GUI Integrity

Pre-repair evidence: `WorkspaceEditorDialog.tsx` awaited reveal without checking the returned
boolean, while chooser and output creation ran from unhandled async callbacks. The sidebar reveal
also allowed `addRecentDir` rejection to escape.

Observed behavior: unavailable folders or rejected IPC calls could do nothing while leaving the
editor apparently usable, and “Create now” could reject without an actionable result.
Expected boundary: every folder operation reflects the Electron/ACP result and names a recovery
action.
Failure mechanism: fire-and-forget promise paths were treated as UI completion.
Break-it angle: reject `directoryChooser`, return `false` from reveal, or reject output creation.
Impact: the operator cannot distinguish cancellation, failure, and completion and may start a
session with an unresolved folder.
Recommended mitigation: catch every path, require the reveal/write boolean, and emit actionable
feedback from the real result.
Validation: focused component tests plus the full Desktop suite.

### WFG-GOS-002: Secret-shaped provider errors were rendered verbatim

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow / GUI Integrity; secondary Security

Pre-repair evidence: the credential manager and WorkspaceContext passed `Error.message` directly
to inline UI through the general `errorMessage` helper.

Observed behavior: a provider or secure-storage error containing `api_key=...`, a bearer token,
or a serialized secret-field value would be displayed to the renderer/operator.
Expected boundary: renderer feedback remains useful but never reproduces secret-shaped values.
Failure mechanism: an unsanitized backend/provider diagnostic crossed the renderer boundary.
Break-it angle: reject credential creation with `api_key=SENTINEL_WORKSPACE_SECRET`.
Impact: credentials can appear in screenshots, screen sharing, or copied diagnostics.
Recommended mitigation: cap and redact secret assignments, bearer tokens, provider tokens, and
serialized `value` fields before display.
Validation: the sentinel error test asserts the original value is absent and the form value is
cleared after failure.

### WFG-GOS-003: Optimistic session rows dropped pinned workspace identity

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow / GUI Integrity

Pre-repair evidence: `sessionToListItem` projected provider/model/name fields but omitted
`workspace_id` and `workspace_name` even though both existed on the new `Session`.

Observed behavior: a just-created local sidebar row temporarily lost its workspace snapshot until
a backend poll replaced it.
Expected boundary: optimistic and backend-projected session rows expose the same pinned identity.
Failure mechanism: one producer-to-consumer projection was not extended with the new fields.
Break-it angle: inspect a new session before the first list poll.
Impact: workspace grouping/header truth can transiently disagree with the persisted session.
Recommended mitigation: project both snapshot fields and test the optimistic conversion.
Validation: `sessionToListItem` pinned-workspace regression test.

### WFG-GOS-004: Unrelated workspace refreshes reset a temporary chat directory

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow / GUI Integrity

Pre-repair evidence: Hub reset `workingDir` to `activeWorkspace.workingFolder` whenever the
workspace object changed; every mutation/refetch produces a new object.

Observed behavior: editing a credential, duplicating a workspace, or receiving a multi-window
refresh could silently replace a user-selected one-off directory before session creation.
Expected boundary: a workspace switch changes future defaults, a followed primary path tracks an
edit, and an intentional temporary override survives unrelated refreshes.
Failure mechanism: object identity was conflated with a workspace/default change.
Break-it angle: choose `/tmp/one-off`, then trigger a profile refresh before submitting the chat.
Impact: an agent can start in the wrong directory and read or write the wrong project.
Recommended mitigation: reconcile by workspace ID and previous primary path rather than object
identity.
Validation: three reconciliation tests cover switch, primary edit, and temporary override.

### WFG-GOS-005: Warning indicators hid the actionable reason

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow / GUI Integrity

Pre-repair evidence: every validation issue rendered the same icon with only “Workspace needs
attention.”

Observed behavior: keyboard and screen-reader users could detect a warning but not learn whether
the folder, platform path, or credential required repair.
Expected boundary: the warning itself exposes the backend validation message and the Edit action
remains keyboard reachable.
Failure mechanism: structured validation was reduced to a boolean.
Break-it angle: load a missing primary folder and inspect the accessible name.
Impact: diagnosis requires trial-and-error navigation.
Recommended mitigation: preserve the validation messages in the warning title/accessibility name.
Validation: sidebar accessibility test asserts the exact relink reason.

### WFG-GOS-006: Removing the default output left an invalid draft

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow / GUI Integrity

Pre-repair evidence: the remove action filtered the selected output without transferring
`isDefault` when another output remained.

Observed behavior: deleting the current default made a later save fail the backend’s exactly-one-
default invariant despite an apparently valid remaining destination.
Expected boundary: the editor cannot create an avoidably invalid default state through a single
supported action.
Failure mechanism: collection deletion ignored the dependent selection invariant.
Break-it angle: add a second output, remove the first/default, then save.
Impact: avoidable save failure and unclear repair work.
Recommended mitigation: transfer default status to the first remaining output.
Validation: editor regression asserts the remaining output is selected.

### WFG-GOS-007: Extension always-allow performed an impossible second liveness check

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced and test-observed
Domain: Workflow / GUI Integrity

Pre-repair evidence: `resolveAcpPermissionRequest` deletes the pending request, after which the
always-allow handler called `isAcpPermissionRequestPending` and necessarily observed `false`.

Observed behavior: the request was resolved as always-allow, but extension-wide permission
persistence stopped with a stale-request message.
Expected boundary: consume the request exactly once, then perform the promised permission write;
a truly stale request must mutate nothing.
Failure mechanism: a valid consuming liveness check was followed by a contradictory
non-consuming check.
Break-it angle: approve a live extension with “Always allow all tools.”
Impact: one tool proceeds while the promised durable extension policy is not applied, and the UI
reports the wrong outcome.
Recommended mitigation: retain the consuming resolve as the gate and remove only the impossible
post-consumption check.
Validation: stale requests mutate nothing; live request call order is resolve, list, persist; the
full suite now passes.

## WFG inventory disposition

| Check                                   | Disposition       | Evidence                                                                                         |
| --------------------------------------- | ----------------- | ------------------------------------------------------------------------------------------------ |
| WFG-001 Fake Success                    | Findings          | WFG-GOS-001 and WFG-GOS-007 tied UI outcome to incomplete async paths.                           |
| WFG-002 UI/API Mismatch                 | Findings          | WFG-GOS-003 and WFG-GOS-007.                                                                     |
| WFG-003 CLI/API Mismatch                | Not applicable    | Workspace management has no CLI surface; existing CLI behavior is unchanged.                     |
| WFG-004 Stale Display                   | Findings          | WFG-GOS-003 and WFG-GOS-004; cross-window invalidation itself held.                              |
| WFG-005 Hidden Failure                  | Finding           | WFG-GOS-001.                                                                                     |
| WFG-006 Destructive Ambiguity           | Held              | workspace/profile deletes are named, confirm exact effects, and state preservation/dependencies. |
| WFG-007 Approval Gate Bypass            | Held after repair | workspace/profile backend guards mirror UI confirmation/disable semantics.                       |
| WFG-008 Status Lies                     | Findings          | WFG-GOS-007; credential configured status is backend-derived and held.                           |
| WFG-009 Partial Success Complete        | Finding           | WFG-GOS-007 resolved the request but skipped policy persistence.                                 |
| WFG-010 Disabled Control Active Backend | Held              | only/default workspace and empty/default-output invariants are backend-enforced.                 |
| WFG-011 Backend Mutation No Feedback    | Finding           | WFG-GOS-001 output creation; all durable workspace mutations now refresh or report.              |
| WFG-012 Workflow Step Skipped           | Finding           | WFG-GOS-006; session preparation still enforces folder/credential readiness.                     |
| WFG-013 Operator Cannot Diagnose        | Findings          | WFG-GOS-001, WFG-GOS-002, and WFG-GOS-005.                                                       |
| WFG-014 Derived Data Shown Confirmed    | Held              | validation and credential readiness come from backend filesystem/secure-store checks.            |
| WFG-015 Bulk Selection Mismatch         | Not applicable    | no workspace bulk operation exists.                                                              |

## Break-it review

- Backend mutation fails: create/update/delete/export/output/profile paths now retain the form or
  show an actionable error; success is emitted only after the completed result.
- Disabled control driven directly: backend rejects default/only workspace deletion and invalid
  output/default collections.
- Out-of-band change: the Electron broadcast causes an ACP refetch in other windows; a stale local
  session filter is repaired to backend active truth.
- Workspace switch during a visible chat: the session header continues to show its pinned snapshot
  and explicitly names the different active default.
- Missing credential/folder: validation is visible in the row/editor and new-session preparation
  refuses to fall back silently.
- Secret-bearing error: sentinel value is redacted and cleared from component state.

## Patch order and regression guardrails

1. Preserve session/workspace identity and intentional Hub directory state.
2. Make path/output result feedback truthful and diagnostic.
3. Redact secret-shaped diagnostics at the workspace UI boundary.
4. Preserve dependent output/default invariants.
5. Remove the contradictory approval liveness check.
6. Add rendering, deletion, multi-window, pinned-header, sentinel, projection, and directory-state
   regressions; run the full Desktop suite.

## Validation limits

- The complete jsdom Desktop suite ran, but this gate did not authenticate a live provider or write
  a real OS keyring secret.
- Electron shell reveal/save behavior is mocked in component tests; the handler/result contracts
  were source-traced.
- No bulk workspace workflow or workspace CLI exists, so WFG-003 and WFG-015 are inapplicable.
- Visual pixel comparison was not used; accessibility behavior is asserted from roles, names,
  focusable native controls, and Radix menu/dialog primitives.

Stop condition: all WFG-001 through WFG-015 checks have a finding, held non-finding, or explicit
not-applicable disposition; every confirmed finding is repaired and regression-covered.
