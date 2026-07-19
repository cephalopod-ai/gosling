# Playtest continuation repair session — 2026-07-19

Authority: `repair-defect-campaign`, supplied findings PLY-GOS-003 and
PLY-GOS-004 from the continuation of the 2026-07-19 `audit-playtest-app` run.

## Gate 0–2: inventory and grouping

- Baseline: `main` at `8664292bc`; the only pre-existing worktree change was
  the related continuation report.
- Group 1 — desktop new-chat and workspace-switch feedback:
  - PLY-GOS-003 (Medium, frontend correctness): invalid primary workspace
    submission returned silently. Touch set: `Hub.tsx`, `ChatInput.tsx` and
    their tests.
  - PLY-GOS-004 (Low, frontend/UX-bug): future-session indication was only an
    ephemeral toast. Touch set: `WorkspaceSidebarSection.tsx` and its test.
- No file required opportunistic modularization; the edits are narrow guards
  and presentation state, not heavy changes to a 1000–2000 line surface.
- Both defects share the new-chat/workspace interaction boundary and were
  repaired as one stage.

## Stage result

### PLY-GOS-003 — fixed

Hub derives the highest-severity workspace validation issue, renders it as an
inline alert, and passes a submit-disabled reason into ChatInput. ChatInput
blocks button, form, and Enter-key submission paths and keeps the reason
available to the send tooltip. `Hub.test.tsx` verifies that an invalid active
workspace cannot call `createSession` or navigate.

### PLY-GOS-004 — fixed

WorkspaceSidebarSection retains the existing toast and also renders an
accessible `role="status"`/`aria-live="polite"` notice outside the workspace
list. `WorkspaceSidebarSection.test.tsx` verifies the notice after a switch.
The visible session remains pinned; this change only communicates the new-chat
default.

## Verification and adversarial review

- Focused Desktop tests: 3 files, 9 tests passed.
- Full Desktop unit suite: 77 files, 531 tests passed.
- Direct Desktop TypeScript compiler: passed (`tsc --noEmit -p tsconfig.json`).
- Changed-file ESLint with `--max-warnings 0`: passed.
- Changed-file Prettier check: passed.
- `git diff --check`: passed.
- Adversarial review checked click, form-submit, and keyboard submission paths;
  invalid optional-only warnings remain non-blocking, while invalid required
  workspace validation blocks session creation. The notice is outside the
  list role to avoid adding a non-list child to the workspace list semantics.

## Record closure

- `docs/build/playtests/2026-07-19-gosling-desktop-playtest-continuation.md`:
  PLY-GOS-003 and PLY-GOS-004 changed from open findings to `RESOLVED` with
  repair evidence.
- `docs/build/defects.md`: added PC-003 ledger rows for both findings with
  root causes, patches, regression tests, and residual risk.
- No in-code TODO/FIXME/HACK marker described either finding.

## Status

`completed_verified` pending the stage commit.
