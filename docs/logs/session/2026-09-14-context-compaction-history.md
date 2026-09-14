# Context compaction history

Date: 2026-09-14

## Task

Implement ADR-0021 end to end: freeze the contract, add the database ledger,
atomically capture successful durable and temporary compactions without duplicating
the raw transcript, and expose bounded user controls for walkthrough history.

## Files changed

- `docs/adr/0021-context-compaction-history.md`, `docs/INDEX.md`, and user guides
- `crates/gosling/src/context_mgmt/mod.rs`
- `crates/gosling/src/session/session_manager.rs`
- `crates/gosling/src/session/session_manager/{schema,migrations,message_storage,compaction_history_storage}.rs`
- compaction agent persistence seams and focused tests
- typed ACP history and policy requests plus generated TypeScript clients
- CLI list/show/export/pin/unpin/delete/prune commands
- Desktop session timeline and App settings with policy impact preview

## Validation

Validation used the repository's pinned Hermit toolchain:

- Seven focused core tests passed for schema migration, append-only generations,
  policy bounds and preview/application, pinning, deletion, and purge accounting.
- Two SDK wire-contract tests, one ACP dispatch/policy integration test, and the CLI
  parser regression passed. The ACP test also proves a stale preview cannot persist
  a policy.
- The complete Desktop suite passed: 174 files and 1,359 tests. TypeScript, ESLint,
  translation extraction/synchronization, and all locale checks passed.
- Documentation unit tests (16), TypeScript checking, and the production Docusaurus
  build passed.
- `cargo clippy --all-targets --locked -- -D warnings` passed. After the final
  transaction refinement, scoped Gosling library and ACP integration clippy passed
  again with warnings denied.
- ACP schema and TypeScript client generation was byte-reproducible from the final
  source. `just check-acp-schema` regenerated successfully but then exited nonzero
  because that recipe compares generated files to committed `HEAD`, which necessarily
  differs for this uncommitted feature; the separate before/after digest check passed.
- `cargo fmt --check`, `git diff --check`, and the required AGENTS governance-marker
  check passed. `GEMINI.md` is absent.

## Risks and follow-ups

- This local ledger is not an externally anchored or compliance-grade audit log.
- SQLite deletion is logical. Secure reclamation across database pages, WAL files,
  backups, and filesystem snapshots remains a separate storage-level design.
- Pinned snapshots can exceed count or byte limits; previews report byte overage and
  cleanup does not silently remove pins.
