# 2026-08-14 Default Shell DS-3 through DS-6 foundation

- Task: implement the DS-3 (working-directory authority), DS-4 (credential metadata and
  use-without-ownership), DS-5 (module and backend composition), and DS-6 (launcher, icon, identity,
  and scaffold) work packages from
  [`../../architecture/default-shell-template.md`](../../architecture/default-shell-template.md),
  without starting the Default Shell GUI or any named shell.
- Commits: `9dee4b6c1` (DS-3 through DS-6 foundation), `7a5253018`, `69b3e5132`, `7a5e001db`,
  `0bdc3302b`, `4c83ac8f6`, `db00ae58d`, `a51f2c661` — merged to `main` via PR #52
  (`claude/default-shell-ds3-ds7-7bsret`) at `216143682`.
- Files changed: `crates/gosling-sdk-types/src/shell.rs`; `crates/gosling/src/acp/shell_handlers.rs`,
  `shell_validation.rs`, `domain_adapter.rs`; `crates/gosling-cli/src/cli.rs`;
  `ui/desktop/src/shell/directoryController.ts`, `credentialController.ts`, `runtimeSnapshot.ts`,
  `ipcMain.ts`; `ui/desktop/scripts/shell-scaffold*.{js,test.js}`;
  `fixtures/shell-{products,consumers}/default-shell-template/**`; ADR-0014 amendment.
- Evidence (local, this worktree, not yet revision-bound to one exact CI-checked SHA — see DS-7):
  - DS-3: `_gosling/unstable/shell/directory/validate` canonicalizes without side effects;
    `directory.select`/`session.detach` are typed main-owned operations that never accept a
    renderer-supplied path. Local Rust, Desktop, and live-child tests pass.
  - DS-4: `session.credentialPolicy` gates a four-field safe projection behind
    `_gosling/unstable/shell/credentials/list`; credential selection persists only an opaque ID and
    the backend re-resolves it at session creation. Sentinel-secret and revoke/relink/mismatch tests
    pass locally.
  - DS-5: `_gosling/unstable/shell/modules/list` reports the intersection of provisioned selection
    and live backend resolution as one bounded inventory; the v1 one-adapter limit is explicit.
    Local unit and live-child tests pass; the full DS-5.4 crash/hang/malformed-adapter matrix is
    deferred to R6.
  - DS-6: `pnpm run shell:scaffold` generates a non-destructive neutral template into an approved
    root through a staged temporary directory; `pnpm run shell:conformance` refuses to certify an
    incomplete one. `fixtures/shell-{products,consumers}/default-shell-template` is the committed
    neutral sample.
- Decision: DS-3 through DS-6 are **built**, not verified. This log does not itself constitute DS-7
  acceptance — DS-7 requires one exact clean revision with current CI, which remains open (see
  [`../../build/shell-productization/traceability-matrix.md`](../../build/shell-productization/traceability-matrix.md)
  and the DS-7 acceptance audit tracked under CA-6 of the Default Shell corrective closure campaign).
- Follow-up: this log resolves the dangling reference at
  `default-shell-template.md`'s DS-7 section, which cited this filename before it existed.
