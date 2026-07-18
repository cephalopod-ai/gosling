# Defect Ledger — Gosling Desktop Workspaces

No finding disappears. Dispositions: fixed, verified-not-a-defect, deferred-with-risk,
blocked-by-missing-input, or blocked-by-tooling. Every fixed defect names a regression test.

Last updated: Gate 1

| ID | Gate | Source | Severity | Finding | Root cause | Disposition | Patch | Regression test | Validation evidence | Residual risk |
|---|---|---|---|---|---|---|---|---|---|---|

## Root-cause patterns and prevention rules

| Pattern | Prevention rule adopted | Plan-change ref |
|---|---|---|
| Ambient session inputs (ARC-GOS-002) | Every workspace-sensitive provider/session input is explicit or scoped and pinned. | initial charter |
| Oversized persistence owner (ARC-GOS-001) | Workspace metadata receives a dedicated store; session manager stores snapshots only. | initial charter |

