# Gate 5 evidence — Gosling Desktop Workspaces

Date: 2026-07-18

## Implemented

- Backend-owned `WorkspaceContext` with typed generated SDK calls, loading/error state, CRUD,
  validation, credential-profile metadata, harmless filter preference, and multi-window refetch.
- Sidebar Workspaces section with persisted collapse preference, All/workspace chat filtering,
  active and warning states, create/edit/duplicate/reveal/export/delete actions, confirmations,
  keyboard-reachable Radix menus, focus styles, and accessible names.
- Workspace editor for general metadata, primary/additional folders, read/read-write access,
  credential bindings, multiple product outputs/types, exactly one default output, explicit output
  creation, validation, reveal, and relinking.
- Secure credential-profile manager with password inputs, no prepopulation, configured placeholders,
  create/update/delete/dependency confirmation, redacted errors, and secret-state clearing.
- New-session workspace propagation through Hub, pair-route, grouped extension recovery, ACP request,
  and optimistic session projection; existing sessions retain their pinned workspace/header badge.
- Workspace-aware session listing with All-workspaces support and legacy/unassigned rows included under
  Default.

## Workflow audit and repairs

`audit-workflow-gui` v3.1 covered every WFG-001 through WFG-015 check. Seven confirmed defects were
fixed: hidden folder failures, raw secret-shaped errors, missing optimistic workspace metadata,
temporary working-directory reset, non-diagnostic warnings, invalid default-output deletion, and an
existing false-stale extension approval path.

## Commands and results

| Command                                                             | Result                                           |
| ------------------------------------------------------------------- | ------------------------------------------------ |
| `source bin/activate-hermit && cd ui/desktop && pnpm run typecheck` | pass                                             |
| focused workspace/header/session/working-dir tests                  | pass: 37, then 25 and 14 after added regressions |
| `source bin/activate-hermit && cd ui/desktop && pnpm run test:run`  | pass: 63 files, 476 tests                        |
| `git diff --check`                                                  | pass                                             |
| `jq empty docs/build/audits/gate-5-workflow-gui-findings.json`      | pass                                             |

The first typecheck attempt outside Hermit was rejected by the package engine guard because that
shell exposed pnpm 10.6.4. All recorded checks use the repository Hermit environment and pnpm
10.30.3.

## Gate decision

Gate 5 passes after JSON/schema validation and checkpoint hygiene. Desktop can create and manage
workspaces and secure profile references, make future-session defaults explicit, preserve visible
session pinning, and diagnose missing dependencies without displaying stored secrets.
