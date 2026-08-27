# Deep Research regression playtest and flow audits

Date: 2026-08-27  
Repository: `/Users/eric/Work/vscode/forked/gosling`  
Baseline: `eceaa5b3d76cb567563394b1336513fd9a632160` (`main`, synchronized with `origin/main`)  
Authority: playtest/test-only followed by read-only workflow, pipeline, and input/output audits

## Repair closure — 2026-08-27

This report records the pre-repair audit. All seven findings were repaired and
revalidated in the subsequent governed campaign. See
`reports/2026-08-27-deep-research-repair-verification.md` for the post-repair
workflow, data-flow, file-flow, adversarial review, and test evidence.

| Finding | Closure | Repair evidence |
|---|---|---|
| WFG-DR-001 | Resolved | Persisted research roots; terminal-message artifact provenance, placement, filename, and SHA-256 pair verification before `Completed`. |
| WFG-DR-002 | Resolved | Shared Desktop byte constants and preflight validation match ACP per-item and aggregate limits. |
| WFG-DR-003 | Resolved | Per-session repetition state; exact previously failed payload is denied on first retry; generic identical calls remain capped. |
| IOP-DR-004 | Resolved | Suffix/content agreement at link and resolve for supported images, PDF, JSON, and UTF-8 text. |
| WFG-DR-005 | Resolved | Limit-plus-one scan returns `truncated`; renderer displays `500+` and a complete-folder warning. |
| IOP-DR-005 | Resolved | PDF object/page gates and incremental extraction stop at the prompt budget. |
| WFG-DR-006 | Resolved | Visible start error includes cleanup failure and manual Session History recovery. |

## Executive verdict

At the time of this pre-repair audit, the build passed the packaged long-input GUI replay and eleven focused
Rust plus twenty-six focused Desktop regressions. The connection-pool guard,
external-provider mode normalization, exact `source: "dummy"` compatibility,
and external-delegate Chat mode all hold at their tested boundaries. One High
workflow gap and five Medium defects remain. Release use should not call Deep
Research artifact delivery verified until the High gap is fixed: ACP marks a
run completed when streaming ends without error, but it never verifies that the
required session report and Research Library copy exist or match.

The playtest used no personal data, credentials, or live research services.
Provider-backed full orchestration was therefore partially blocked; runtime
claims below distinguish current-pass observation, prior supplied observation,
and static confirmation.

## Scope

- Lenses: application playtest, Workflow/GUI Integrity (WFG), pipeline graph,
  and Input/Output Path (IOP).
- Focus: Desktop New Research intake through ACP prompt execution, session
  storage, Summon delegation, tool activity, session artifacts, and the durable
  Research Library.
- Non-scope: repairing new audit findings, destructive provider/network drills,
  unrelated chat/CLI workflows, and live credential use.
- Existing scenario cards used unchanged: `docs/test_scenarios/19-deep-research.md`.
- User-requested regression cards added before execution:
  `docs/test_scenarios/20-deep-research-regressions.md` (DR-03–DR-08).

## Playtest environment and execution

- App: packaged Electron Desktop `/Applications/Gosling.app`.
- Installed backend SHA-256:
  `ac2ad6e90baea4530ff89f486de540a200db7611fc070c4954787fc35a227045`.
- Signature: `codesign --verify --deep --strict` passed.
- GUI sandbox: disposable Gosling root and Electron profile under
  `/tmp/gosling-dr-playtest.DqZeGk`; the operator's running app was not stopped
  or modified.
- Packaged GUI input: multiline Markdown, Unicode, and one unbroken token over
  8,000 characters.
- Geometry result: dialog `655/655`, list `607/607`, item `605/605`, preview
  `494/494` (`clientWidth/scrollWidth`); preview computed
  `white-space: pre-wrap`, `overflow-wrap: break-word`. No horizontal overflow.
- Screenshot: `/tmp/gosling-initial-input-wrap.png`.

### Scenario results

| Scenario | Result | Evidence / limit |
|---|---|---|
| DR-01 prompt-only scope | Not executed | Requires a deterministic provider plus research extensions; no live credentials were used. Static flow was audited. |
| DR-02 invalid delegate source | Partial pass | Schema, prompt, and source-validation paths inspected; exact current-pass end-to-end lead behavior not run. |
| DR-03 queued writers | Pass | `test_queued_writers_do_not_exhaust_the_connection_pool` passed. |
| DR-04 Solo external ACP mode | Partial pass | Current mode-normalization regression passed; earlier installed-app evidence records `ACP_SOLO_OK`, but this pass did not reuse live credentials. |
| DR-05 `source: "dummy"` | Pass | Both ad-hoc normalization and named-source preservation regressions passed. |
| DR-06 external delegate Chat | Partial pass | Mode/tool-disable regression passed; provider-backed async delegate lifecycle was not run. |
| DR-07 long Initial Inputs | Pass | Packaged GUI geometry and wrapping replay passed; component regression passed. |
| DR-08 invalid extension parameters | Known fail, not re-executed | Supplied runtime screenshot shows six repeated `-32602 InvalidParams` Markdown calls; current source still instantiates repetition protection with no limit. |

## Workflow truth inventory

| View/control | Backend effect | Shown status | Actual source of truth | Failure feedback | Bypass |
|---|---|---|---|---|---|
| New Research submit | Creates session, appends three system prompts, stores/resolves Initial Inputs, dispatches ACP prompt | Composer transitions into research chat | Each awaited request; incomplete session deleted on failure | Start error rendered in Hub | Generic ACP can create a session, but there is no separate backend “research” endpoint |
| Prompt run | Persists InProgress, streams agent events, persists terminal state | Spinner or Task failed banner | Stream error/cancellation only | Exact ACP error text shown | Artifact/report postconditions are not checked |
| Delegate launch | Validates payload, creates subagent, starts task | Activity tool call | Launch result; async completion obtained later through `load` | Tool result carries error/Chat notice | External provider cannot use Auto; Chat disables tools |
| Initial Inputs | Writes session-scoped library rows, resolves selected IDs into prompt context | Input count/list | ACP library rows and resolver | Start error; cleanup failure only in console | UI limits differ from resolver aggregate limits |
| Outputs | Discovers successful tool/file references into artifact inventory | Outputs count/cards | `session_artifacts` | Preview errors and missing status shown | No required-report or dual-copy verification |
| Research Library | Filesystem scan on pane open/tab selection | File count/list | Depth/extension/500-file bounded scan | Generic load failure | Limit truncation is not disclosed |

## WFG inventory result

| Code | Result | Evidence |
|---|---|---|
| WFG-001 Fake Success | Finding WFG-DR-001 | `researchLibrary.ts:10-15`; `server.rs:3184-3192` |
| WFG-002 UI/API Mismatch | Finding WFG-DR-002 | `ResearchInitialInputsDialog.tsx:78-80,154-176`; `shell_handlers.rs:9-14,430-452` |
| WFG-003 CLI/API Mismatch | Not applicable | Deep Research intake/team selection is a Desktop experience; the CLI exposes generic ACP/chat, not this product workflow. |
| WFG-004 Stale Display | Held | Library refreshes at mount and whenever its tab is selected (`ArtifactPane.tsx:336-356,484-487`); artifacts arrive as ACP updates. |
| WFG-005 Hidden Failure | Finding WFG-DR-006 | Cleanup failure is console-only (`Hub.tsx:468-480`). |
| WFG-006 Destructive Ambiguity | Held in scope | Initial Inputs removal is pre-submit; incomplete-session cleanup targets only the newly created ID. No library-file delete action is exposed here. |
| WFG-007 Approval Gate Bypass | Held | External-tool providers normalize away from Auto; external delegates use Chat (`delegate_config.rs:7-20`). |
| WFG-008 Status Lies | Same root as WFG-DR-001 | ACP Completed means stream completion, not research deliverable completion. Counted once. |
| WFG-009 Partial Success Complete | Finding WFG-DR-005 | Library scan stops at 500 but returns only an array (`researchLibrary.ts:5,20-59`). |
| WFG-010 Disabled Control Active Backend | Not confirmed | Research submit gating is renderer composition over generic ACP calls; no backend research mutation endpoint claims the same UI gate. |
| WFG-011 Backend Mutation No Feedback | Same root as WFG-DR-006 | Failed cleanup can leave a created session without operator feedback. |
| WFG-012 Workflow Step Skipped | Same root as WFG-DR-001 | Required report writes can be skipped without affecting terminal Completed. |
| WFG-013 Operator Cannot Diagnose | Held for ACP errors | Prompt failures render `describeAcpError` in the Task failed banner (`chatSessionController.ts:296-304`; `BaseChat.tsx:635-654`). |
| WFG-014 Derived Data Confirmed | Held | Research prompts require source provenance and explicitly classify prior library reports as secondary context. |
| WFG-015 Bulk Selection Mismatch | Not applicable | No bulk mutation exists in the audited Deep Research surfaces. |

## Pipeline graph

### Nodes

| Node | Stage | File / function | Inputs | Outputs / side effects | Branches |
|---|---|---|---|---|---|
| N1 | Entry | `Hub.handleSubmit` | prompt, images, Initial Inputs, roster, workspace | session request | empty/invalid/valid |
| N2 | Validation | Hub + team prompt builder | UI counts, workspace, extensions, distinct roster | disabled reason or configuration | Solo/Dual/Trio |
| N3 | Routing | `createSession` / ACP new session | provider/model/folders/extensions | active session, folder policy | hosted/external provider |
| N4 | Policy | system-prompt append | scientific method, library, team contract | persisted prompt fragments | inputs/no inputs; Solo/multi |
| N5 | Persistence | `addResearchInitialInputs` + library handlers/store | pasted text and canonical file path | scoped SQLite items | text/file; success/partial failure |
| N6 | Adapter | library resolver | ordered item IDs | assistant text context/images | missing, malformed, oversize, success |
| N7 | Execution | ACP `prompt` / `Agent.reply` | user message + context | stream events/messages | error/cancel/complete |
| N8 | Delegation | Summon delegate | source/provider/model/extensions | subagent session/task | hosted Auto; external Chat; bad source |
| N9 | Tool adapter | MCP/research extensions | generated tool arguments | result or `InvalidParams` | success/retry/fail |
| N10 | Persistence | prompt state/message storage | events and terminal state | SQLite prompt state/messages | writer contention |
| N11 | Artifact generation | tool writes + artifact discovery | output paths/results | workspace report and artifact row | write/no write/overwrite |
| N12 | Durable product | model-directed Research Library copy | session report | second filesystem copy | write/skip/mismatch |
| N13 | Presentation | BaseChat/ArtifactPane | prompt error, artifacts, library scan | banner, counts, previews | error/truncated/stale/fresh |

### Major edges

| Edge | From → To | Condition / data |
|---|---|---|
| E1 | N1 → N2 | local intake state |
| E2 | N2 → N3 | all renderer gates pass |
| E3 | N3 → N4 | session creation succeeds |
| E4 | N4 → N5 | optional Initial Inputs exist |
| E5 | N5 → N6 | ordered created library IDs |
| E6 | N6 → N7 | resolved assistant context/images |
| E7 | N7 → N8 | Dual/Trio lead emits delegate calls |
| E8 | N7/N8 → N9 | provider emits research tool calls |
| E9 | N7/N9 → N10 | messages and prompt state persist |
| E10 | N9 → N11 | successful write/reference is discovered |
| E11 | N11 → N12 | prompt instructs a second identical copy; no enforced transaction |
| E12 | N10/N11/N12 → N13 | terminal state and inventory projection |

### Path inventory and selection

| Path | Risk | Shape | Existing/current evidence |
|---|---|---|---|
| P-01 | P1 | Solo, prompt-only, hosted, complete | static only this pass |
| P-02 | P1 | Solo with Initial Inputs | UI/ACP tests |
| P-03 | P1 | Dual with hosted delegate | prompt/schema tests; no current E2E |
| P-04 | P1 | Dual external delegate in Chat | mode test; no current E2E |
| P-05 | P1 | Legacy dummy source normalized | two Rust tests |
| P-06 | P1 | Prompt under queued writers | Rust contention test |
| P-07 | P2 | Linked file missing at resolve | source fail-closed |
| P-08 | P2 | Extension InvalidParams repeats | prior runtime evidence; current guard absent |
| P-09 | P1 | Outputs + Research Library copies | prompt-only postcondition |

Deliberate paths: A=P-02 canonical intake; B=P-04 controlled tool-disabled
delegate; C=P-07 controlled file rejection. Expanded-path cap was not reached.
Provider-backed branches were deferred because no test credentials were used.

Random selection seed: `831f1602d79aef093d90faad968261fb`.

| Selected | Signature hash |
|---|---|
| P-07 | `fdba1dc4999fa0928cc906cea755b854132b540010ba7d651e45eb156effe19f` |
| P-08 | `39520eb8cbaa1ca36067ec6786e657d58d95165e63b41165e8d817ab34d604c9` |
| P-09 | `ff04931d29ca35931f4db5b5c67aa7b8e82a108e0dd370b32cd9700fc49ba1f7` |
| P-03 | `f23323a4234478a5fc150cfb34961eee0baa89eef772bb7be7eeff042f91f18c` |
| P-01 | `0d0345f5e187333fb924914611417c56ce3863148aa5f1edea7d52f337a40dd0` |

Replay method: initialize Python `random.Random(int(seed, 16))`, use the path
list in P-01…P-09 order, and call `sample(paths, 5)`.

### Invariants

| Invariant | Result |
|---|---|
| Accepted prompt gets a persisted terminal state | Held in focused contention test |
| Rejected/missing input fails closed | Held statically and by handler tests |
| Invalid input creates no durable private library residue after cleanup | Cleanup behavior tested; cleanup-failure visibility remains WFG-DR-006 |
| External providers never execute delegated tools outside inspection | Held by Chat mode/tool-disable test |
| Named delegate sources are not broadly rewritten | Held by named `dummy` preservation test |
| Final product traces to input | Partially held through messages/artifact provenance; full report not required |
| Required report exists in Outputs | Not enforced (WFG-DR-001) |
| Required library copy exists and is byte-identical | Not enforced (WFG-DR-001) |
| Identical invalid tool calls terminate promptly | Failed in supplied run; guard disabled (WFG-DR-003) |
| Random replay metadata exists | Held in this report |

## Input/output surface inventory

| Surface | Direction | Format | Trust | Validation | Sink | Bound |
|---|---|---|---|---|---|---|
| Pasted Initial Input | In | UTF-8 text | user | client maxLength; backend non-empty/per-item limit | SQLite then prompt context | 256 KiB/item; 512 KiB aggregate |
| Selected Initial Input file | In | PDF/image/text/data by suffix | local user file | canonical path, regular/non-empty, suffix allowlist | linked path then prompt context | 20 MiB file; tighter resolver bounds |
| Composer images | In | ACP image | user | generic ACP conversion | prompt | outside focused file picker |
| Delegate payload | In | JSON | model-generated | Serde/schema plus normalization | subagent config/session | max turns; task slots |
| MCP tool call | In/out | JSON-RPC | model/extension | extension schema | tool result/activity | max turns; repetition limit disabled |
| Session artifact | Out | path/metadata | tool/provider | successful-result discovery and path checks | SQLite inventory/UI | paginated backend, renderer total cap |
| Research Library | Out/list | filesystem files | model/tool | native selected root; extension/depth scan | durable directory/UI | 500 listed, depth 6; truncation undisclosed |

## IOP inventory result

| Code | Result | Evidence |
|---|---|---|
| IOP-001 Unvalidated Input | Finding WFG-DR-002 | UI and backend enforce incompatible size contracts. |
| IOP-002 Unsafe Output Path | Held in scope | Library root comes from a native directory chooser; workspace/session folders are canonicalized and made explicit policy roots. |
| IOP-003 Path Traversal | Held | Input paths must be absolute and canonicalized; artifact discovery rejects traversal/remote paths. |
| IOP-004 Archive Slip | Not applicable | Initial Inputs do not accept archives. |
| IOP-005 Extension/Format Confusion | Finding IOP-DR-004 | MIME is selected only from the filename suffix (`shell_handlers.rs:586-606`). |
| IOP-006 Malformed Payload Accepted | Held | Serde/ACP errors fail closed; invalid/missing selected items abort resolution. |
| IOP-007 Dangerous Export Formula | Not confirmed in scope | The app does not own a structured CSV/XLSX research exporter; model-authored files remain general tool output. |
| IOP-008 Provider Output Trusted | Held at canonical-data boundary | Provider output remains report/artifact content, not a canonical structured record; prompts require provenance. |
| IOP-009 Log/Report Leakage | Not confirmed | No secret value is intentionally copied into the audited report/list projections; library summaries omit payloads and paths. |
| IOP-010 Generated Artifact Reuse | Not confirmed | Artifact identity includes session and resolved path; Research Library is rescanned rather than cached by a weak key. |
| IOP-011 Output Overwrite | Same root as WFG-DR-001 | “Do not overwrite” exists only in the model prompt; no exclusive dual-copy publication is verified. |
| IOP-012 Partial Output Complete | Finding WFG-DR-005 | Research Library scan silently stops at 500. |
| IOP-013 Unbounded Processing | Finding IOP-DR-005 | PDF bytes are capped, but `lopdf::Document::load` and all-page extraction happen before prompt truncation. |
| IOP-014 Hidden Input Source | Held | The Research Library path and role are visible in New Research and added explicitly to session folders/system context. |
| IOP-015 Inconsistent CLI/API/UI | Same root as WFG-DR-002 | The relevant mismatch is Desktop versus ACP resolver; no equivalent Deep Research CLI intake exists. |

## Findings

### WFG-DR-001 — Research completion does not verify required deliverables

- Severity: **High**
- Confidence: **Confirmed** missing postcondition; runtime omission is **Likely** until deliberately reproduced.
- Evidence basis: source-evidenced.
- Evidence: `ui/desktop/src/prompts/researchLibrary.ts:10-15` requires two
  byte-identical copies. `crates/gosling/src/acp/server.rs:3184-3192` chooses
  Completed solely from stream error/cancellation and persists it before
  artifact listing. No report/copy/hash check is present.
- Impact: a Deep Research run can appear terminally complete with no durable
  report, one copy, mismatched copies, or a silently overwritten library file.
- Recommendation: define a research completion manifest (session artifact,
  library artifact, hashes, provenance, completion/degraded state); validate it
  before research-specific completion and show missing/mismatched output as
  degraded/failed.
- Validation: deterministic test where a provider returns final prose without
  files; missing one copy and mismatched-hash variants must not be Completed.
- Complexity/cost/agent: high / medium-high / Rust ACP + Desktop workflow.

### WFG-DR-002 — Initial Input limits disagree across UI and ACP resolution

- Severity: **Medium**
- Confidence: **Confirmed** deterministic code property.
- Evidence basis: source-evidenced.
- Evidence: UI says every file may be 20 MB and accepts that limit
  (`ResearchInitialInputsDialog.tsx:78-80,162-176`). ACP limits images to 5 MB,
  aggregate images to 10 MB, and aggregate text to 512 KiB
  (`shell_handlers.rs:9-14,430-452`). Pasted text allows 256 KiB each, so two
  maximum items plus library labels exceed the aggregate. Text/PDF truncation
  adds a label and truncation marker before the same aggregate check
  (`shell_handlers.rs:651-677`).
- Impact: inputs presented as valid are stored, then research creation fails and
  the new session is deleted. An 8 MB image or sufficiently large text/PDF is a
  direct example.
- Recommendation: share one contract across Desktop/SDK/server; validate
  aggregate prompt contribution before creating/storing a session; distinguish
  file-size from extracted prompt-size and preview truncation.
- Validation: 8 MB PNG, two 256 KiB pastes, 1 MB text, large-text PDF, and exact
  boundary variants through the Desktop and direct ACP calls.
- Complexity/cost/agent: medium / medium / Desktop + ACP contracts.

### WFG-DR-003 — Repetition protection is installed with no limit

- Severity: **Medium**
- Confidence: **Confirmed**, runtime-observed in supplied screenshot and source-evidenced on current baseline.
- Evidence: `crates/gosling/src/agents/agent.rs:752-754` constructs
  `RepetitionInspector::new(None)`. `tool_monitor.rs:59-63,88-96` makes `None`
  unconditionally allow repeats. The supplied run showed six identical failed
  Markdown calls with `selector`, `backendNodeId`, `maxBytes`, `url`, and
  `timeout` after `-32602 InvalidParams`.
- Impact: provider turns, time, and cost are consumed repeating a payload known
  to be invalid; Deep Research may terminate degraded or at max turns instead
  of adapting once.
- Recommendation: apply a bounded identical-call limit, with a stricter path
  after a deterministic schema/InvalidParams failure; return the schema error
  and forbid an identical retry while still permitting a corrected call.
- Validation: exact rejected payload executes once; corrected second payload
  succeeds; interleaved distinct calls do not false-positive.
- Complexity/cost/agent: low-medium / low / Rust agent reliability.

### IOP-DR-004 — Initial Input content type trusts the extension

- Severity: **Medium**
- Confidence: **Confirmed** missing boundary; runtime provider behavior is untested.
- Evidence: `shell_handlers.rs:586-606` maps only path suffix to MIME. The
  canonical file is never checked for magic/content agreement before an image,
  PDF, JSON, or text path is selected.
- Impact: renamed or replaced files are decoded by the wrong path and can fail
  late, produce misleading context, or be sent to a provider with a false MIME.
- Recommendation: sniff supported image/PDF formats, require suffix/type/bytes
  agreement, parse structured text before labeling where appropriate, and
  revalidate at resolve time.
- Validation: PNG-as-JSON, HTML-as-PNG, malformed PDF, and correct controls.
- Complexity/cost/agent: medium / medium / Rust I/O boundary.

### WFG-DR-005 — Research Library silently truncates after 500 files

- Severity: **Medium**
- Confidence: **Confirmed** deterministic code property.
- Evidence: `ui/desktop/src/utils/researchLibrary.ts:5,20-59` stops scanning at
  500 and returns only `ResearchLibraryFile[]`; `ArtifactPane.tsx:560-593`
  renders the array length as the library count with no completeness flag.
- Impact: operators can believe the visible inventory is complete and miss
  older or lexically later research artifacts.
- Recommendation: return `{files, truncated, scannedCount}` or paginate; render
  an explicit “500+ / truncated” status and refresh affordance.
- Validation: 501 allowed files across subdirectories must show incompleteness.
- Complexity/cost/agent: low / low / Electron main + Desktop.

### IOP-DR-005 — PDF expansion is unbounded before prompt truncation

- Severity: **Medium** (local/single-user); higher only if untrusted remote file intake becomes reachable.
- Confidence: **Likely** runtime resource manifestation; **Confirmed** absence of an expansion/page/text bound.
- Evidence: input bytes are capped at 20 MB (`shell_handlers.rs:362-367`), but
  `lopdf::Document::load`, all-page enumeration, and `extract_text` complete
  before truncation (`shell_handlers.rs:651-665`).
- Impact: a compact but expansion-heavy PDF can consume disproportionate CPU or
  memory and delay/fail research startup.
- Recommendation: cap page count, extracted bytes, object/decompression work,
  and wall time during parsing; stream or stop extraction at the prompt budget.
- Validation: bounded high-expansion and high-page-count fixtures with measured
  memory/time; normal PDF still resolves.
- Complexity/cost/agent: medium / medium / Rust I/O/reliability.

### WFG-DR-006 — Failed incomplete-session cleanup is hidden from the operator

- Severity: **Low**
- Confidence: **Confirmed** source property; actual orphaning requires a second failure.
- Evidence: `Hub.tsx:468-480` catches deletion failure only with
  `console.error`, then shows only the original start failure.
- Impact: a failed research start can leave an orphan session that the operator
  is not told to remove.
- Recommendation: include cleanup failure and session ID in the visible error,
  or queue reliable backend cleanup with a visible recovery state.
- Validation: force input resolution failure and session-delete failure; banner
  names both outcomes and the orphan is discoverable.
- Complexity/cost/agent: low / low / Desktop workflow.

## Break-it review and non-findings

- Exact `source: "dummy"` with explicit provider/model normalizes; source-only
  `dummy` remains a named-source request.
- External delegates select Chat and disclose that delegated tools are disabled;
  hosted delegates remain Auto.
- A linked file must be absolute, canonical, regular, non-empty, and within the
  file-size limit at link time; existence and size are checked again at resolve.
- Selected library IDs must be unique, in scope, available, and at most sixteen.
- Private library rows are deleted with their owning session; project rows
  remain shared, verified by focused tests.
- Session artifact storage deduplicates and paginates; Desktop caps renderer
  accumulation separately.
- Prompt errors surface exact ACP detail in a Task failed banner.
- Research Library enumeration skips hidden and symlink entries, caps depth,
  and refreshes when its tab is selected.

## Skill escalation

| Finding | Primary lens | Secondary lens | Why |
|---|---|---|---|
| WFG-DR-001 | Workflow/GUI | Data integrity, state transition, I/O | Terminal state and dual-copy artifact integrity are one contract. |
| WFG-DR-002 | Workflow/GUI | I/O | UI acceptance diverges from server validation. |
| WFG-DR-003 | Workflow/GUI | Reliability, cost | Retry churn turns a diagnosable tool error into degraded execution. |
| IOP-DR-004 | I/O | Reliability | Wrong decoder/provider payload follows suffix confusion. |
| WFG-DR-005 | Workflow/GUI | I/O | A bounded listing is shown as a complete inventory. |
| IOP-DR-005 | I/O | Reliability | Expansion happens before the resource/prompt bound. |
| WFG-DR-006 | Workflow/GUI | State transition | A failed compensating deletion can leave hidden durable state. |

## Recommended patch order and tests

1. Enforce a research completion manifest and dual-copy hash verification
   (WFG-DR-001).
2. Unify and preflight Initial Input limits (WFG-DR-002).
3. Bound identical InvalidParams retries (WFG-DR-003).
4. Add content sniffing and bounded PDF extraction (IOP-DR-004/005).
5. Surface library truncation and cleanup failure (WFG-DR-005/006).

| Regression | Purpose |
|---|---|
| final prose/no files; one copy; mismatched copies | Completion truth |
| 8 MB PNG; two max text inputs; 1 MB text; large PDF | UI/ACP parity |
| identical InvalidParams then corrected call | Retry guard |
| suffix/content mismatch matrix | Format validation |
| 501 research-library files | Completeness signal |
| cleanup delete failure | Orphan-session feedback |

## Validation

- Packaged GUI long/multiline/unbroken-token geometry replay: passed.
- Desktop focused regression: 2 files / 5 tests passed.
- Desktop intake/library regression: 4 files / 21 tests passed.
- Rust focused regression: 11 named tests passed, including writer contention,
  mode/source/delegate guards, library scoping/deletion, and artifact persistence/pagination.
- Earlier validation on the same committed repair baseline: Desktop 133 files /
  1,062 tests, typecheck, Rust Summon 42 tests, warning-denying Clippy, formatting,
  package, signature, install hash, and installed launch all passed.
- `git diff --check`: passed before report creation.

## Validation limits and residual risk

- No current-pass live LLM credentials or research extensions were used.
- DR-01 and provider-backed DR-02/04/06/08 were not executed end to end in the
  disposable profile. Prior installed-app evidence and supplied screenshots are
  identified separately rather than rounded up to a pass.
- The pipeline audit did not add a new harness during its read-only phase.
- Network timeout, cancellation during a real research extension call, and
  byte-identical dual output behavior require a later credentialed sandbox run.
- Final confidence: **High** for the static boundaries and executed focused
  tests; **Medium** for end-to-end orchestration posture because those provider
  paths were partially blocked.

## Next action

Repair WFG-DR-001 first by introducing an application-owned research completion
manifest and validating the two required artifacts before research-specific
completion is shown.
