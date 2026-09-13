# 2026-09-13 — Consolidated audit merge, rebuild, and local reinstall

## Task and authority

- Merge and deconflict `codex/consolidated-audit-repair-20260913` into local `main`.
- Refresh README, release, validation, installation, index, and inventory documentation for the
  repaired source.
- Rebuild the release CLI and Desktop package, then replace the local CLI and
  `/Applications/Gosling.app` with rollback copies retained until verification completes.
- No push, tag, GitHub release, notarization, updater promotion, or remote mutation is authorized.

The repository-required private catalog was searched. It returned adjacent repair and macOS
planning workflows but no exact local Electron merge/reinstall workflow, so the repository's
`RELEASE.md`, `RELEASE_CHECKLIST.md`, `justfile`, Desktop README, and prior local-install records
govern this run.

## Merge

- Starting repair head: `b484255d0` on `codex/consolidated-audit-repair-20260913`.
- Local and `origin/main` both pointed to its direct ancestor `829b09054`.
- `git merge --ff-only codex/consolidated-audit-repair-20260913` advanced local `main` without a
  content merge or conflicts. Local `main` became 12 commits ahead of `origin/main`; nothing was
  pushed.
- The repair branch had already passed final `cargo clippy --all-targets -- -D warnings`, Rust
  formatting, targeted/broad Rust and Desktop tests, and adversarial review before the merge.

## Pre-install state

- `/Users/eric/.local/bin/gosling` reported `1.1.0`, SHA-256
  `6bd721b0797716960c13821f75dc38739b2e0cb611a9080dddafb071f6cf16d5`.
- `/Applications/Gosling.app` reported `1.2.5`; its embedded backend SHA-256 was
  `6e0a0f9dc435b18ef857ab8504b60587948eb5814dd090b6fc29ed85d4853d73`.
- Deep/strict code-signature verification of the installed app passed. No installed Gosling app or
  Desktop backend process was running.

## Documentation refresh

The pre-build documentation patch updates:

- `README.md`, `RELEASE.md`, and the v1.2.5 candidate notes with the reconciled audit repairs and
  current 127-card/source-validation limits;
- the installation guide's stale v1.2.2 source-candidate notice to v1.2.5 while preserving the
  local-build versus published-release boundary;
- both documentation indexes and inventories; and
- the current test ledger with the 2026-09-13 source evidence and its baseline/workspace limits.

Documentation validation before the application build:

- `npm test` — 16 passed.
- `npm run typecheck` — passed.
- The first `npm run build` exported 171 pages but correctly failed its broken-link gate because a
  release-note link escaped the Docusaurus content root. The link now uses the canonical GitHub
  source URL; the rerun built and exported all 171 pages successfully.

## Build, install, and verification

- `source bin/activate-hermit && just package-ui` completed successfully. pnpm printed expected
  unsupported-platform warnings for the non-arm64 binary packages while selecting the arm64
  package.
- `target/release/gosling`, `ui/desktop/src/bin/gosling`, and the packaged app backend all reported
  1.2.5 and shared SHA-256
  `75751874133dd603c1781dc6393d6d452a633188c634ea3e9f9eb137556ddc50`.
- The packaged bundle reported `CFBundleShortVersionString=1.2.5` and
  `CFBundleVersion=1.2.5`; `codesign --verify --deep --strict --verbose=2` passed.
- Rollback copies were created at `/tmp/gosling-install-backup-20260913.npOTjt`. The previous CLI
  hash remains `6bd721b0797716960c13821f75dc38739b2e0cb611a9080dddafb071f6cf16d5`; the
  previous installed backend hash remains
  `6e0a0f9dc435b18ef857ab8504b60587948eb5814dd090b6fc29ed85d4853d73`. The copied app's
  deep/strict signature verification passed before replacement.
- The new bundle and CLI were staged and verified before replacement. After installation,
  `/Users/eric/.local/bin/gosling`, `/Applications/Gosling.app/Contents/Resources/bin/gosling`,
  and `target/release/gosling` shared the release hash above; all reported 1.2.5. Installed bundle
  metadata and deep/strict signature verification passed. `gosling --help` exited successfully.

## Installed Desktop smoke limit

Installed UI acceptance is blocked, not passed. A normal launch and a second launch with a fresh
Electron user-data directory each kept the main/GPU processes alive but produced no renderer,
Desktop backend, or visible window. The UI inspection hook timed out, and AppleScript observed no
window. A one-second `sample` of the fresh-profile process showed the main thread waiting in the
macOS Security `SecItemCopyMatching` Keychain path. This narrows the same no-renderer packaged-app
limitation already recorded by the 127-card source playtest.

AppleScript quit and a normal TERM did not complete while the process was waiting. Only the two
smoke-test process trees were killed; the final process check found no installed Gosling app or
Desktop backend still running. No provider request, updater promotion, notarization, release tag,
publication, or clean-machine acceptance was attempted.

## Final validation

- `source bin/activate-hermit && cargo fmt --all -- --check` — passed.
- `source bin/activate-hermit && cargo clippy --all-targets -- -D warnings` — passed.
- `source ../bin/activate-hermit && npm test` from `documentation/` — 16 passed.
- `source ../bin/activate-hermit && npm run typecheck` from `documentation/` — passed.
- `source ../bin/activate-hermit && npm run build` from `documentation/` — passed; 171 Markdown
  pages exported.
- `git diff --check` — passed.
- The required AGENTS documentation-governance marker remains present. `GEMINI.md` is absent, so
  there is no Gemini marker to validate.
