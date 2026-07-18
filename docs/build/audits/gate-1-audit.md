# Gate 1 Negative Space audit — Workspaces requirements

## Verdict

The normalized charter covers the hidden actors and alternate paths most likely to
invalidate the feature: two Desktop windows, active-workspace changes during new-session
creation, provider recreation after a switch, filesystem changes after validation,
deleted workspace/profile metadata, and interrupted writes. No unresolved material
negative-space finding remains in the requirements. The audit added explicit guardrails
to REQ-008, REQ-012, REQ-020, and INV-008 before implementation.

## Scope and assumption ledger

Lens: `audit-negative-space` 3.1, read-only. Surface: `docs/INTENT.md` requirements and
the existing new/load/session/config paths cited by the Gate 0 audit.

| Assumption | Where | What relies on it | If false | Disposition |
|---|---|---|---|---|
| Active workspace cannot change mid-create | Desktop active state | new session selection | wrong workspace pinned | REQ-008 passes explicit ID |
| Only one Desktop window edits workspaces | renderer context | visible active/list state | stale or lost UI state | REQ-007/027 require broadcast/backend ownership |
| Folder stays valid after editor validation | filesystem | session cwd/tool scope | wrong/missing directory | INV-008/REQ-006 require pre-use backend validation |
| Profile remains present forever | secure store | resume/recreate | silent fallback/account drift | REQ-012/020/021 require relink/fail-closed |
| Store write completes | process/filesystem | workspace definitions | corrupt/empty store | INV-006/REQ-002 require atomic recovery |
| Workspace deletion owns its folders | operator interpretation | delete action | user file loss | INV-004/010 and REQ-021 forbid file deletion |
| Current active workspace identifies visible chat | sidebar/header | user belief | wrong-account/project confusion | REQ-011/017 separate active from pinned |

## NEG-001..015 disposition

| Code | Result |
|---|---|
| NEG-001 Impossible State Possible | Held — only-workspace deletion must atomically create/reassign Default (REQ-021). |
| NEG-002 Hidden Actor | Held — multi-window broadcast and backend authority are explicit (REQ-007/027). |
| NEG-003 Unmodeled Input | Held — filesystem/env/template inputs are validated and normalized (REQ-006/023, INV-008). |
| NEG-004 Cross-Boundary Composition | Held — delete+resume and switch+recreate are explicit acceptance cases (REQ-012/020/021). |
| NEG-005 Assumption Collapse | Held — active and pinned workspace identities are modeled separately. |
| NEG-006 Rare Timing Window | Held by contract — explicit workspace ID closes switch/create race; atomic persistence closes write interruption. Runtime tests remain required. |
| NEG-007 Catastrophic Low Probability | Held by invariant — workspace/profile deletion cannot delete files/sessions and secret writes reuse existing hardened storage. |
| NEG-008 Negative Test Missing | Open test obligation, not a defect — REQ-030 enumerates negative tests before verification. |
| NEG-009 Safety Bypassed By Alternate Path | Held by contract — every new-session route must funnel through one create seam (REQ-008). |
| NEG-010 Human/Operator Misuse | Held — referenced-profile deletion and missing-output creation require explicit confirmation (REQ-005/014). |
| NEG-011 Model/Provider Output Trusted | N/A — provider output is not used to choose workspace paths or credentials. |
| NEG-012 Future Integration Breaks Invariant | Held — cloud sync/multi-user are explicit non-goals; backend ownership avoids renderer split brain. |
| NEG-013 Local-First Assumption Fails | Held for current scope — multi-window is modeled; network/team access is out of scope. |
| NEG-014 Compliance Language Over-Trusted | N/A — no compliance claim gates this feature. |
| NEG-015 Recovery Mechanism Causes Damage | Held by contract — recovery may replace metadata files only and never selected folders/sessions (REQ-002/021). |

## Break-it and required tests

- Switch active workspace while a delayed new-session request is in flight.
- Construct providers concurrently for two profiles of the same provider using distinct sentinels.
- Rename/delete a workspace or profile between list and resume.
- Replace a validated folder with a symlink before session creation.
- Interrupt a workspace write before rename and reopen the store.
- Invoke every new-session entry path and assert the same workspace request contract.
- Double-submit destructive confirmations and assert idempotent metadata-only behavior.

## Validation limits

This gate audited requirements and existing static seams, not implemented behavior.
Every “held by contract” item remains planned until its regression test is executed.

