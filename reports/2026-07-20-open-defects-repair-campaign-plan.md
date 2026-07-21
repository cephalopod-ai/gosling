# Open Defects Repair Campaign Plan - 2026-07-20

Mode: Existing Findings Mode  
Authority: governed repair  
Skill: `020_repair/repair-defect-campaign`

## Frozen inventory

| Record | Disposition |
| --- | --- |
| ORCH-002 | Current source already satisfies the record; reconcile and close. |
| REC-001 | Current source already satisfies the record; reconcile and close. |
| REC-002 | Current source already satisfies the record; reconcile and close. |
| RES-002 | Current source already satisfies the record; reconcile and close. |
| RES-003 | Current source already satisfies the record; reconcile and close. |
| AUD-GOS-011 / GEM-005 / DEF-002 | Repair ACP runtime path scoping and path-keyed global state. |
| Desktop lint debt | Repair browser globals and unstable workspace-filter dependencies. |
| ACP schema wrapper | Repair caller-directory dependence. |
| Provider inventory residual | Cache/coalesce reads and invalidate on mutation. |
| Chat scroll/persistence TODOs | Reconcile as satisfied by current source and focused coverage. |

## Repair groups

1. Desktop/tooling locality: browser globals, hook stability, provider inventory, and schema recipe.
2. ACP boundary locality: task-local runtime paths plus path-keyed configuration and identity.

## Scope routing

`crates/gosling/src/acp/server.rs` exceeds 2,000 lines. This campaign applies only the narrow boundary repair; broad modularization remains routed to maintenance backlog.

Feature work is excluded: Session Handoff, Tagteam expansion, CLI usage reporting, release execution, and broad modularization. Giles's internal uniqueness-constraint failure is external tool debt. The bounded ACP response policy remains intentional.
