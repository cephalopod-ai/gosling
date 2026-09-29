# 2026-09-29 — Remaining playtest repairs

## Task

Continue the 2026-09-27 playtest closeout, repair findings that do not require product direction,
and preserve unresolved decisions for the operator.

## Repairs

- G127: a Desktop process launched with `GOSLING_PATH_ROOT` now creates and grants its default
  Research Library inside that isolated root. An explicitly selected library remains authoritative.
- G220: permission dropdowns now use Radix's `asChild` composition, so their existing buttons are
  the triggers instead of being nested inside generated trigger buttons.

## Validation

- `cargo fmt --check` passed.
- `git diff --check` passed.
- Desktop formatting passed for every touched TypeScript/TSX file.
- The targeted Desktop test could not start because this checkout has no installed `vitest`
  executable.
- Workspace Clippy could not download the missing `buffer-redux` crate because the environment's
  network proxy returned HTTP 403.

## Pending product decisions

- E01/S19: decide whether `.goslingignore` should become an access-control boundary. Recommended:
  add enforcement behind an explicit setting that defaults **on**, while retaining a visible
  compatibility escape hatch for workflows that intentionally need ignored files.
- G108: decide whether relinking a moved workspace should re-home pinned historical chats.
  Recommended: offer a confirmation toggle in the relink flow, default **on**, and preserve the old
  path in recovery metadata.
- H02: decide whether failed delegate launch shapes should suppress equivalent retries.
  Recommended: deduplicate normalized launch attempts within a turn by default, with a per-session
  opt-out for diagnostic work.
- D24: decide whether importing a grown transcript should merge new turns into the existing import
  or create a new session. Recommended: merge only an append-only continuation and otherwise create
  a separate session.
- D21: decide whether terminal integration should forward typed shell commands to the model.
  Recommended: expose an explicit opt-in setting that defaults **off**.
- H08/H09: decide whether Gosling should wrap extension contract errors and reserve the normalized
  `dummy` delegate sentinel. Recommended: provide Gosling-owned contract detail and replace the
  sentinel with an unambiguous internal representation.

Other queued Desktop diagnostics remain open in the authoritative playtest register and require a
separate runtime-focused pass; this patch does not represent them as complete.
