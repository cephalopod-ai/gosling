# 2026-09-02 — Branch integration

## Task

Integrate every outstanding local and remote branch tip into `main`, resolve overlapping Desktop
modularization work, validate the combined result, and synchronize `main` with `origin` before
starting the provider-failover MVP.

## Inventory and merge order

- `repair/modularize-desktop-main-2026-08-23`
- `codex/fix-markdown-math-rendering`
- `codex/modularize-agent-2026-09-01`, which already contained the Desktop, extension-manager, and
  ACP-server modularization branch tips
- `origin/version-bump/1.2.0`

All other local and remote branch tips were already reachable from the original `origin/main`.
The integration used recoverable refs under `refs/integration-backups/` before each merge.

## Conflict resolution

- Preserved the later renderer file and artifact access checks while integrating the first Desktop
  extraction.
- Kept the newer inline/display math parser and added the Markdown branch's combined rendering and
  code-fence regressions.
- Used the later consolidated Desktop decomposition where both modularization branches extracted the
  same responsibilities, and normalized the case-only `allowList.ts`/`allowlist.ts` duplicate.
- Merged the 1.2.0 version and canonical-provider-data update after the code branches.

No branches were deleted. Deletion remains an explicit operator decision after remote reachability
has been verified.

## Validation

- Pre-integration Rust clippy on the consolidated modularization tip: passed.
- Desktop typecheck after the first Desktop merge: passed.
- Renderer-access and certificate-trust focused tests: 4 passed.
- Desktop typecheck after the Markdown merge: passed.
- Markdown rendering focused tests: 31 passed.
- Desktop typecheck after the consolidated modularization merge: passed.
- Extracted Desktop main-process module tests: 22 passed.

The final combined build, test, clippy, Desktop suite, push, and remote reachability checks are
recorded in the completion handoff for this session.
