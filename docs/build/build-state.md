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
- Remote state: the completed Workspaces gates are synchronized to `origin/main`.
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
| 7 — Documentation           | complete | user/operator/distribution docs; `evidence/gate-7.md`                         |
| 8 — Acceptance and handoff  | complete | `evidence/gate-8.md`; all traceability rows closed                            |

## Current decisions

- Workspace metadata and active selection are owned by a backend workspace store.
- Workspace credentials use metadata-only profiles plus namespaced keys in the
  existing `Config` secret-storage abstraction.
- Session rows pin workspace, credential-profile, effective working folder, and
  a non-secret workspace-context snapshot.
- Provider construction receives a session-scoped credential resolution context;
  workspace switching will never rewrite global provider secrets.
- Workspace React state lives in a focused context; sidebar components only
  present and invoke that state.
- Optional extension defaults are excluded unless implementation discovery reveals a
  clean session-scoped hook.

## Continuation point

All gates are complete, the acceptance evidence is committed, and local `main` matches
`origin/main`. No continuation action is required for this campaign.
