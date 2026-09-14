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

## Changes-only walkthrough follow-up

Status: verified locally on 2026-09-14

The original discussion identified the lack of a true changes-only view and a
source-message action as the highest-priority walkthrough gap. That recommendation
existed only in the discussion, so this dated addendum is its durable closure record:
open -> closed. The Desktop now computes a bounded word- or line-level diff only
after comparison is enabled, collapses unchanged content, and loads the source
messages recorded by a snapshot only after **View source messages** is selected.
Source loading reuses the existing paged transcript interface, preserves recorded
message order, stops after reaching the oldest recorded source row, and reports
unavailable rows. No diff, raw message copy, delta, or ancestor dependency is added
to `sessions.db`.

Changed surfaces:

- `ui/desktop/src/components/conversation/ContextHistoryDiff.tsx` and focused tests
- `ui/desktop/src/components/conversation/ContextHistoryDialog.tsx` and focused tests
- `ui/desktop/src/acp/contextHistory.ts` and focused tests
- Desktop locale catalogs and source hashes
- ADR-0021 and the Smart Context Management guide

Validation on the final UI state:

- Focused Context History tests: 11 passed across 3 files.
- Complete Desktop suite: 1,366 tests passed across 175 files.
- Desktop TypeScript, ESLint, translation synchronization, and all 15 locale checks passed.
- Scoped Prettier checks for every touched TypeScript file passed. The repository-wide
  Prettier check remains nonzero on 51 unrelated pre-existing files; none is part of
  this change and none was rewritten.
- Documentation tests: 16 passed; documentation TypeScript and production build passed.
- `cargo fmt --check` passed; no Rust source or generated ACP contract changed.

Architecture comparison: accepted ADR-0021 and the existing Desktop ACP adapter
boundary were conformant before the repair. The final change derives presentation
state from independent snapshots and the existing transcript pager, so the result is
**no new architecture or contract drift**.

## Revised improvement order

1. Decide and test `/clear` retention semantics. It currently removes the session's
   entire Context History, including pins; either preserve walkthrough history or make
   the destructive scope explicit before clearing.
2. Verify `summary_hash` when reading and exporting snapshots so stored integrity
   metadata actively detects corruption instead of remaining descriptive.
3. Make expiration timing explicit in the UI and guide: automatic cleanup is
   event-driven, so `purge_after` is eligibility time rather than a scheduled deletion.
4. Rename **Maximum local storage** to **Maximum summary payload** because the cap
   counts snapshot payload bytes, not SQLite pages, WAL files, indexes, or backups.
5. Measure real snapshot size and similarity distributions before adding an independent
   compression codec; retain standalone readability and avoid ancestor chains.
6. Batch CLI full-history export and startup expiration reconciliation only if measured
   session sizes show those paths are material bottlenecks.
