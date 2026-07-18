# Workspaces build state

Last updated: 2026-07-18

## Objective

Implement production-quality Desktop Workspaces with backend-owned metadata, secure
credential-profile references, session pinning, folder/output routing, migration,
accessible sidebar management, and regression coverage.

## Baseline

- Repository: `cephalopod-ai/gosling`
- Branch: `main`
- Baseline commit: `9b9571febf06f7fc6dfddea32267b5c0d325b369`
- Remote state: `main` matched `origin/main` before Workspaces changes began.
- Execution profile: existing-repository/Giles, governed repair.

## Gate status

| Gate                        | Status   | Exit evidence                                                                |
| --------------------------- | -------- | ---------------------------------------------------------------------------- |
| 0 — Orientation             | complete | `evidence/gate-0.md`, `audits/gate-0-audit.md`                               |
| 1 — Intent and traceability | complete | `docs/INTENT.md`, `traceability-matrix.md`, `evidence/gate-1.md`             |
| 2 — Contracts and design    | complete | `docs/architecture.md`, ADR-0001–0005, `io-contract.md`, `execution-plan.md` |
| 3 — Boundaries and harness  | complete | canonical SDK DTOs; `evidence/gate-3.md`                                     |
| 4 — Backend vertical slice  | complete | workspace/session/credential backend; `evidence/gate-4.md`                   |
| 5 — Desktop vertical slice  | complete | Desktop UI/session integration; `evidence/gate-5.md`                         |
| 6 — Audit and repair        | complete | security/concurrency/LLM audit; `evidence/gate-6.md`                          |
| 7 — Documentation           | active   | operator/user/distribution documentation                                    |
| 8 — Acceptance and handoff  | pending  | —                                                                            |

## Current decisions

- Workspace metadata and active selection will be owned by a backend workspace store.
- Workspace credentials will use metadata-only profiles plus namespaced keys in the
  existing `Config` secret-storage abstraction.
- Session rows will pin workspace, credential-profile, effective working folder, and
  a non-secret workspace-context snapshot.
- Provider construction will receive a session-scoped credential resolution context;
  workspace switching will never rewrite global provider secrets.
- Workspace React state will live in a focused context; sidebar components will only
  present and invoke that state.
- Optional extension defaults are excluded unless implementation discovery reveals a
  clean session-scoped hook.

## Continuation point

Document user, operator, persistence, distribution-template, credential-security, and known
limitation behavior for Gate 7, then run the Gate 8 acceptance matrix.
