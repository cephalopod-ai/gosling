# PC-001 artifact-router workflow and input/output audit

Skills: `audit-workflow-gui` v3.1, `audit-dataflow-input-output` v3.1, and
`audit-dataflow-concurrency` v3.1
Authority: read-only audit; implementation authority resumed only after the inventory closed
Scope: the new resolver, React router context, session/workspace exports, Outputs pane, Electron
save/copy IPC, native downloads, source-file authorization, and adjacent direct-write surfaces
Budget: every changed router file and every Desktop save/download call site, including async
cross-process config publication; unrelated updater, configuration, and automatic
transcript-archive internals were classified but not deep-audited

The supplied request was treated as a draft. Its universal-artifact mission was preserved while
the audit expanded to malformed content, simultaneous downloads, failure signaling, and bypass
entry points implied by the cross-process design.

## Surface and boundary inventory

| Surface                               | Direction / format                      | Trust and validation                                                       | Destination / effect                                               | Operator truth                                          |
| ------------------------------------- | --------------------------------------- | -------------------------------------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------- |
| Workspace metadata export             | backend JSON → file                     | backend non-secret export; shared router                                   | pinned workspace `export`/default output; user may override dialog | success only after main-process write                   |
| Session export                        | backend JSON → file                     | ACP export; session ID and pinned workspace                                | pinned session `export`/default output                             | cancel is neutral; failure is a toast                   |
| Outputs Save a copy                   | file or in-memory content → file        | source path uses canonical approved-root guard; base64 and size validated  | tab's pinned workspace type/default output                         | copies full source; success only after write            |
| Native download                       | network response → file                 | per-window config canonicalized against approved roots; portable leaf name | visible session's pinned workspace or active workspace             | unroutable download warns; collision-safe path reserved |
| Missing output                        | workspace metadata → directory mutation | backend validation plus explicit `createIfMissing` policy                  | selected output only                                               | confirmation precedes creation                          |
| App update                            | network → update staging                | updater contract                                                           | OS/app update location                                             | excluded: not a user artifact                           |
| Automatic session archive             | ACP export → configured history folder  | dedicated archive setting                                                  | transcript archive                                                 | excluded: retention/history contract                    |
| `.goslinghints`, settings, registries | configuration/state → app paths         | existing file-access/config boundaries                                     | config/data directory                                              | excluded: not a product artifact                        |

Producer/consumer pairs were read together: router request/main handler, workspace validation/native
config, artifact tab/Save Copy, session row/session export, and `will-download`/renderer feedback.

## Findings and repairs

### IOP-GOS-001: Permissive base64 decoding could report a corrupted artifact as saved

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced and regression-tested
Domain: Input-Output-Path

Before repair, the main save helper passed renderer content directly to permissive
`Buffer.from(..., 'base64')`; malformed characters can be ignored while the resulting bytes are
written and the UI reports success. The repaired boundary validates alphabet, padding, and a
decode/re-encode round trip before the write (`ui/desktop/src/utils/artifactSave.ts:14-27`).

Expected boundary: invalid encoded content aborts before any target file exists.
Break-it angle: save `not!base64` as an image.
Impact: a generated image/document could be silently truncated or corrupted.
Repair: strict decoder plus an on-disk negative regression.
Validation: `artifactSave.test.ts` asserts the error and `ENOENT` target postcondition.

### IOP-GOS-002: Simultaneous native downloads could select the same available path

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Input-Output-Path; secondary Concurrency

Before repair, each `will-download` event checked only current filesystem existence. Two events
arriving before either created its file could both choose the same destination. The session router
now reserves a destination before `setSavePath` and releases it on `done`
(`ui/desktop/src/utils/artifactDownloads.ts:49-71`).

Expected boundary: a path selected for an in-flight download participates in collision checks.
Break-it angle: start two `brief.pdf` downloads in the same event turn.
Impact: downloads could contend for or overwrite the same user artifact.
Repair: per-session in-flight reservation set in addition to filesystem existence.
Validation: the regression obtains `brief.pdf` and `brief (1).pdf` before either completes.

### WFG-GOS-008: Unroutable native downloads silently fell back outside the workspace

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI; secondary Input-Output-Path

Before repair, a missing/rejected routing config made the `will-download` listener return without
feedback, allowing Electron's normal destination behavior while the feature promised workspace
routing. The listener now emits an unrouted event for every failure branch
(`ui/desktop/src/utils/artifactDownloads.ts:50-72`), and the renderer names the file and relink
action (`ui/desktop/src/contexts/ArtifactRouterContext.tsx:72-80`).

Expected boundary: fallback never looks like successful workspace routing.
Break-it angle: remove or make the matching output inaccessible, then start a download.
Impact: the operator can look in the workspace while the file landed elsewhere.
Repair: typed main-to-renderer failure signal and warning toast.
Validation: download-router callback and renderer warning regressions.

### IOP-GOS-003: In-memory artifact saves had no resource bound

Severity: Medium
Confidence: Likely
Evidence basis: simulation-reasoned
Domain: Input-Output-Path; secondary Reliability

Before repair, the renderer could send an arbitrarily large content string and base64 decoding
could allocate an additional buffer in the main process. Runtime exhaustion was not reproduced, so
its manifestation remains Likely. The helper now refuses content above 256 MiB before opening a
save dialog (`ui/desktop/src/utils/artifactSave.ts:7,34-41`); file-source copies remain streamed by
Node and are not subject to the in-memory limit.

Expected boundary: in-memory content has an explicit limit, while large existing files use copy.
Break-it angle: submit a very large embedded tool result.
Impact: Desktop responsiveness or main-process availability could degrade.
Repair: bounded content input with actionable failure; preserve full-file copy.
Validation: injected small-limit regression proves refusal occurs before the dialog.

### IOP-GOS-004: Unicode-heavy artifact names could exceed filesystem byte limits

Severity: Low
Confidence: Likely
Evidence basis: source-evidenced with a pure boundary regression
Domain: Input-Output-Path

Before repair, the portable filename cap used JavaScript string length. A name containing many
multi-byte code points could satisfy the 180-character cap while exceeding the byte limit of a
common filesystem component. The sanitizer now measures UTF-8 bytes, truncates only at code-point
boundaries, and preserves a bounded extension (`ui/desktop/src/utils/artifactRouting.ts:103-140`).

Expected boundary: every proposed filename fits the router's 180-byte portability budget without
splitting a Unicode code point or losing the recognized extension.
Break-it angle: route an 80-emoji PNG title.
Impact: the save or native download could fail for an otherwise valid generated artifact.
Repair: shared UTF-8 byte truncation before every joined/default/download path.
Validation: the sanitizer regression asserts at most 180 encoded bytes, a complete `.png`
extension, and no replacement character.

The prior over-limit name was source-confirmed, but no host-filesystem rejection was exercised, so
the runtime manifestation remains Likely.

### CON-GOS-002: Older async validation could restore a stale workspace download route

Severity: Medium
Confidence: Likely
Evidence basis: source-evidenced with a forced-interleave regression
Domain: Concurrency

Before repair, each `set-artifact-routing-config` handler awaited filesystem validation and then
wrote directly to a shared per-window map. An earlier workspace update could validate after a newer
update and overwrite the newer route. Window cleanup had the same ordering hazard with an in-flight
validation. The repaired registry assigns a revision before every await and commits only if that
revision is still current (`ui/desktop/src/utils/artifactRoutingRegistry.ts:20-42`).

Expected boundary: the most recently received config or clear operation is authoritative regardless
of validation completion order.
Break-it angle: hold validation for workspace A, validate workspace B, then release A.
Impact: a subsequent native download could land in the previously active workspace.
Repair: revision-checked per-window registry; cleanup also advances the revision.
Validation: a forced interleave proves B remains selected and a pending update cannot resurrect a
destroyed window's route.

Runtime manifestation in a packaged Electron window was not observed, so the concurrency outcome
remains Likely even though the unguarded stale-write mechanism was source-confirmed.

## Concurrency surface inventory

| State / artifact               | Writers                                     | Readers                         | Transaction / atomicity                              | Idempotency / guard                          | Race disposition                                |
| ------------------------------ | ------------------------------------------- | ------------------------------- | ---------------------------------------------------- | -------------------------------------------- | ----------------------------------------------- |
| Per-window routing config      | renderer IPC updates and window cleanup     | native `will-download` listener | revision-checked commit after validation             | monotonically increasing per-window revision | CON-GOS-002 repaired                            |
| Native destination reservation | `will-download` events and `done` callbacks | collision selector              | synchronous event-loop mutation                      | in-flight path set plus filesystem check     | IOP-GOS-002 repaired                            |
| Explicit save target           | native dialog followed by one write/copy    | user and filesystem             | one main-process operation; no dependent state write | native overwrite confirmation                | held; no shared automatic target                |
| Missing output directory       | concurrent confirmed create calls           | workspace validator/router      | idempotent recursive `mkdir` through backend         | backend canonical path/access validation     | held; duplicate create has one directory effect |

## Concurrency inventory disposition

| Check                                | Disposition                                                                                              |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------- |
| CON-001 Race Condition               | CON-GOS-002 repaired; IOP-GOS-002 is the adjacent filename race.                                         |
| CON-002 Lost Update                  | Held after repair: stale validators cannot overwrite the latest config revision.                         |
| CON-003 Double Processing            | Held: repeated routing config updates have one current value; saves remain explicit user operations.     |
| CON-004 Replay Hazard                | Held: config replay is last-received-wins and creates no artifact effect.                                |
| CON-005 Retry Collision              | Held: repeated config publication is side-effect-free; native downloads reserve independent paths.       |
| CON-006 Stale Read                   | Held: the download listener reads the registry at event time, not a captured config snapshot.            |
| CON-007 Stale Write                  | CON-GOS-002, repaired.                                                                                   |
| CON-008 Ordering Dependency          | CON-GOS-002, repaired with explicit revision ordering.                                                   |
| CON-009 Partial Commit               | Not applicable: route publication has one in-memory commit after validation.                             |
| CON-010 Missing Transaction Boundary | Not applicable: no dependent database or multi-store write exists in this router.                        |
| CON-011 Lock Inversion               | Not applicable: the TypeScript router owns no locks.                                                     |
| CON-012 Shared Mutable State         | Held after repair: per-window configs and destination reservations have explicit ownership/guards.       |
| CON-013 Non-Atomic File Output       | Held for owned flows: saves complete before success; native downloads use Electron's download lifecycle. |
| CON-014 Duplicate Canonical Creation | Not applicable: artifact copies/downloads are not canonical database entities.                           |
| CON-015 Check-Then-Act Hazard        | IOP-GOS-002 repaired by including in-flight reservations in the name predicate.                          |
| CON-016 Concurrent Bulk Scope Drift  | Not applicable: no bulk artifact operation exists.                                                       |
| CON-017 Artifact Reuse Race          | IOP-GOS-002 repaired; explicit dialog overwrites remain user-authorized.                                 |
| CON-018 Watcher/Event Reentrancy     | Held: the router installs once per Electron session and `done` only releases reservations.               |

## Concurrency break-it review and patch order

1. Forced validation completion to reverse workspace-update order; the revisioned registry retained
   the newer config.
2. Cleared a window while validation was pending; the late result could not restore routing state.
3. Fired duplicate same-name download events before either completed; the reservations produced
   distinct paths.
4. Replayed config publication and inspected the single current registry value; no artifact or
   directory effect was duplicated.

The two routing-state races were repaired before UI refinements because they could put output in
the wrong workspace. Regression tests assert final routing/path state, not only the absence of an
exception.

## Workflow/GUI inventory disposition

| Check                                   | Disposition                                                                              |
| --------------------------------------- | ---------------------------------------------------------------------------------------- |
| WFG-001 Fake success                    | Held after repair: every explicit save toast follows the completed main write.           |
| WFG-002 UI/API mismatch                 | Held: cancel, success, and error responses are projected distinctly.                     |
| WFG-003 CLI/API mismatch                | Not applicable: the universal router is Desktop-only and changes no CLI operation.       |
| WFG-004 Stale display                   | Held: workspace refetch updates config; visible sessions publish their pinned workspace. |
| WFG-005 Hidden failure                  | WFG-GOS-008, repaired.                                                                   |
| WFG-006 Destructive ambiguity           | Held: Save Copy does not move; overwrite is an explicit native-dialog choice.            |
| WFG-007 Approval bypass                 | Held: missing-directory creation uses confirmation and the backend guard.                |
| WFG-008 Status lies                     | Held after WFG-GOS-008 repair.                                                           |
| WFG-009 Partial success complete        | IOP-GOS-001 adjacent case, repaired before write.                                        |
| WFG-010 Disabled control active backend | Held: unavailable outputs are rejected independently of UI state.                        |
| WFG-011 Backend mutation no feedback    | Held: save/export/create paths surface completion or failure.                            |
| WFG-012 Workflow step skipped           | Held: all Gosling-owned artifact entry points call the shared router.                    |
| WFG-013 Operator cannot diagnose        | Held after WFG-GOS-008; messages name relink/create/deleted-workspace recovery.          |
| WFG-014 Derived data shown confirmed    | Held: inferred product type selects a default; it is not presented as artifact truth.    |
| WFG-015 Bulk selection mismatch         | Not applicable: no bulk artifact operation.                                              |

## Input/output inventory disposition

| Check                              | Disposition                                                                                                                                                      |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| IOP-001 Unvalidated input          | Held after repair: config shapes/types, encoded content, and source paths are validated.                                                                         |
| IOP-002 Unsafe output path         | Held after IOP-GOS-004 repair: native paths pass canonical approved-root validation, portable leaf names are byte-bounded, and dialog targets are user-selected. |
| IOP-003 Path traversal             | Held: filenames reduce to a portable leaf; source reads use canonical containment.                                                                               |
| IOP-004 Archive slip               | Not applicable: the router neither parses nor extracts archives.                                                                                                 |
| IOP-005 Extension/format confusion | Held: explicit type wins, known extension wins over MIME, MIME is fallback routing only.                                                                         |
| IOP-006 Malformed payload          | IOP-GOS-001, repaired.                                                                                                                                           |
| IOP-007 Dangerous export formula   | Not applicable: owned exports are JSON; CSV content is copied, not interpreted.                                                                                  |
| IOP-008 Provider output trusted    | Held: tool output requires explicit Save Copy and is never promoted to canonical state.                                                                          |
| IOP-009 Log/report leakage         | Held: artifact bodies are never logged or placed in routing config.                                                                                              |
| IOP-010 Generated artifact reuse   | Held: tabs distinguish base directory and workspace; no artifact cache is reused.                                                                                |
| IOP-011 Output overwrite           | IOP-GOS-002 repaired; explicit save-dialog overwrite remains user-authorized.                                                                                    |
| IOP-012 Partial output complete    | IOP-GOS-001 repaired; file tabs copy the full source rather than preview bytes.                                                                                  |
| IOP-013 Unbounded processing       | IOP-GOS-003 repaired; native downloads and file copies stream.                                                                                                   |
| IOP-014 Hidden input source        | Held: content, file, export, and native-download producers are enumerated above.                                                                                 |
| IOP-015 CLI/API/UI parity          | Not applicable: no equivalent CLI artifact-router operation exists.                                                                                              |

## Skill escalation

| Finding     | Primary lens      | Secondary lens    | Why                                                                     |
| ----------- | ----------------- | ----------------- | ----------------------------------------------------------------------- |
| IOP-GOS-001 | Input/Output Path | Workflow/GUI      | Corrupted bytes previously produced a false success.                    |
| IOP-GOS-002 | Input/Output Path | Concurrency       | Two in-flight events shared a check-then-act name decision.             |
| WFG-GOS-008 | Workflow/GUI      | Input/Output Path | Silent fallback changed the actual destination.                         |
| IOP-GOS-003 | Input/Output Path | Reliability       | The runtime consequence is resource pressure in the main process.       |
| IOP-GOS-004 | Input/Output Path | Reliability       | A byte-oversized portable leaf could make an otherwise valid save fail. |
| CON-GOS-002 | Concurrency       | Input/Output Path | Reordered async validation could change a native download destination.  |

## Validation limits

- The complete jsdom suite and pure main-process helpers ran; a packaged Electron window was not
  launched to observe an actual network download.
- macOS was the host. Windows path and reserved-name behavior is pure-tested, but the native save
  dialog was not exercised on Windows or Linux.
- Direct writes inside third-party tools/extensions cannot be intercepted transparently and are
  documented as outside the Desktop-owned router; their existing permission boundaries were not
  re-audited here.
- No destructive resource-exhaustion drill was run; IOP-GOS-003 therefore stays Likely.

Stop condition: every WFG-001–015, IOP-001–015, and CON-001–018 item has a finding, held
non-finding, or explicit not-applicable disposition; all discovered findings are repaired and
regression-covered.
