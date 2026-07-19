# Playtest repair session — 2026-07-19

Authority: `repair-defect-patchset`, supplied findings PLY-GOS-001 and
PLY-GOS-002 from the 2026-07-19 `audit-playtest-app` report.

## Stage plan

- Stage 1 — artifact authorization and readiness: `ui/desktop/src/main.ts`,
  `ui/desktop/src/utils/artifactFileAccess.ts`, and `ArtifactPane`. The safe
  state is fail-visible for genuinely unapproved paths while valid workspace
  output routes become readable after asynchronous publication.
- Stage 2 — window chrome hit-testing: `AppLayout` and `ArtifactPane`. The
  safe state is pointer/keyboard-accessible pane controls without changing
  pane layout or navigation behavior.

The stages share only the artifact workbench boundary. Stage 1 landed before
Stage 2's UI verification; no unrelated source surfaces were edited.

## Repairs and regression evidence

- PLY-GOS-001: canonical existing workspace output roots now participate in
  artifact-file authorization, and the pane retries only the specific
  approved-root race three times with bounded backoff. Direct file grants and
  outside-root rejection remain covered.
- PLY-GOS-002: the Outputs pane stacking context is above the titlebar drag
  overlay and header controls are `no-drag`.
- Focused tests: 4 files, 11 tests passed.
- Direct TypeScript compiler: passed (`./ui/node_modules/.bin/tsc --noEmit -p
  ui/desktop/tsconfig.json`).
- Changed-file ESLint and Prettier checks: passed.
- Isolated Electron replay: persisted artifact content rendered after restart;
  Outputs close control accepted a pointer click.

## Whole-stack and follow-up inspection

The original two reproductions were replayed after the patch. The main process
and renderer processes were stopped, test ports were checked, and no test
mock-provider process remained. The pre-existing installed Gosling backend was
left untouched. Remaining playtest permutations (queue steering, alternate
model, native Save a copy, and successful archive write) remain follow-up
coverage, not silently marked complete.

## Final status

`completed_verified` for the supplied two findings.

