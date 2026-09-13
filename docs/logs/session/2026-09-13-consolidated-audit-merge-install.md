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

Pending. This section must record exact build outputs, rollback locations, hashes, signature
checks, installed versions, process shutdown/startup, and any runtime acceptance limits before the
run is closed.
