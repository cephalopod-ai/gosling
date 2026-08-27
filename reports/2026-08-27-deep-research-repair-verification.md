# Deep Research repair verification and flow audit

Date: 2026-08-27  
Campaign: `repair-defect-campaign`  
Repository: `/Users/eric/Work/vscode/forked/gosling`  
Initial baseline: `cecbe4680`  
Automation-created repair commits observed during execution: `dc2bfbf26`, `4f6ccba30`

## Verdict

All seven supplied findings are repaired at their owning boundary and covered by
focused regressions. The completion path now fails closed unless the terminal
assistant response reports a provenance-backed report in a configured workspace
output root and a byte-identical Research Library copy. No new Critical, High,
or Medium defect was found in the post-patch workflow, data-flow, file-flow, or
adversarial review.

The live-credential/provider research orchestration path was not exercised. The
real Electron Initial Inputs geometry scenario passed in an isolated profile;
the file and completion failure matrices ran deterministically below the UI.

## Closure inventory

| ID | Result | Owning control | Regression evidence |
|---|---|---|---|
| WFG-DR-001 | Resolved | ACP prompt terminal-state gate | identical pair passes; missing/unreported, mismatched, and misplaced output fail |
| WFG-DR-002 | Resolved | Desktop preflight plus ACP resolver | UTF-8 item, aggregate text, image item/aggregate, and input-count limits |
| WFG-DR-003 | Resolved | per-session `RepetitionInspector` | exact failed payload denied on first retry; corrected call passes; fourth generic repeat denied |
| IOP-DR-004 | Resolved | Rust link/resolve content validation | disguised PDF and malformed JSON rejected; recorded type rechecked on resolve |
| WFG-DR-005 | Resolved | typed Electron listing result | limit-plus-one returns `truncated`; pane renders `500+` and warning |
| IOP-DR-005 | Resolved | bounded Rust PDF extraction | object/page caps checked before per-page extraction; text stops at budget |
| WFG-DR-006 | Resolved | Hub compensating-action error | primary failure plus delete failure shown with manual recovery |

## Workflow audit

```text
New Research UI
  -> validates team, workspace, library and byte/count limits
  -> creates ACP workspace session with explicit library metadata/folder grant
  -> Rust canonicalizes library + product-output roots and persists research state
  -> stores/resolves selected Initial Inputs
  -> streams agent turn through permission/security/repetition inspection
  -> registers created/modified artifacts in the session inventory
  -> checks terminal assistant paths against output/library provenance
  -> hashes same-name pair
  -> Completed | Failed | Cancelled
```

Audit result: every transition with durable side effects has an awaited result.
Session-start compensation remains scoped to the just-created ID, and a failed
compensation is now operator-visible. Research completion is no longer inferred
from stream exhaustion.

## Data-flow audit

| Stage | Accepted data | Boundary/control | Failure behavior |
|---|---|---|---|
| Desktop intake | pasted Unicode text; native file metadata | UTF-8 bytes, 16 items, per-kind and aggregate limits | inline error; Done disabled |
| ACP storage | bounded text, images, canonical linked path | typed methods and session scope | invalid params; no prompt resolution |
| Resolve | selected opaque IDs | scope/uniqueness, current content/type, aggregate bytes | selection unavailable/too large |
| Prompt | labeled text and standard image blocks | labels excluded from source-byte accounting | no partial oversized prompt |
| Artifact inventory | created/modified filesystem evidence | backend provenance and canonical path | unproven paths ignored |
| Completion | terminal assistant text plus inventory | exact path reference, roots, filename, SHA-256 | prompt state `Failed` |

Audit result: the renderer does not become source of truth for either file type
or completion. Limits are duplicated for early UX but enforced independently by
Rust at the trust boundary.

## File-flow audit

```text
Native picker -> canonical linked path -> session-library row
             -> resolve-time stat + suffix/content agreement
             -> bounded text/PDF extraction or bounded image encoding

Agent tool write -> workspace output file -> session artifact provenance --+
                                                                        +-> bounded SHA-256 equality
Agent tool write -> Research Library file -> session artifact provenance --+

Main directory scan -> limit + 1 metadata rows -> preload typed result
                    -> 500 entries + explicit truncation state
```

Audit result: no renderer-supplied arbitrary read primitive was added. Symbolic
links remain excluded from library browsing; completion canonicalizes paths
before root comparison; linked content is revalidated after mutation; hashing
uses a fixed buffer and rejects deliverables over 100 MB.

## Adversarial review

- A terminal response that only claims success fails without created/modified
  artifact provenance.
- A matching file in scratch space does not satisfy the workspace Outputs copy.
- A stale same-name library file fails unless its bytes match.
- Mentioning a path in an intermediate message does not satisfy the terminal
  response contract.
- A renamed binary/text file or a linked file mutated after selection fails at
  the Rust boundary.
- An exact tool payload already associated with an error is denied on its first
  retry; a corrected payload and another session do not inherit that denial.
- A 501st library file cannot be silently represented as a complete inventory.
- A failed cleanup cannot leave the operator with only the primary error.

## Play-test and validation evidence

- Real Electron/Playwright long unbroken input scenario: passed; no dialog
  horizontal overflow, dialog remained inside viewport, wrapped preview visible.
- Focused Desktop scenarios: 7 files / 50 tests passed before the final full run.
- Focused Rust scenarios: completion, research state, input content/PDF bounds,
  and repetition tests passed before the final full run.
- The first Electron attempts discovered a Vite dependency-optimization race in
  the play-test fixture. The fixture now waits/reloads until the mounted renderer
  is outside the dynamic-import error boundary; the same scenario then passed.

Final full-suite, Clippy, build, packaging, signature, installation, and launch
results are recorded in the campaign session log rather than predicted here.

## Residual risk and follow-up

- Provider-backed solo/dual/trio research still needs a credentialed disposable
  environment for a complete external-service lifecycle replay.
- `server.rs`, `agent.rs`, and Desktop `main.ts` exceed the campaign's
  modularization threshold. This repair kept their edits to call sites; a
  dedicated source-modularization campaign remains appropriate.
- Completion intentionally verifies rather than auto-copies deliverables. A
  provider must still create and report both files; failure is now truthful and
  recoverable instead of a false success.
