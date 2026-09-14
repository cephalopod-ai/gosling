# Context compaction history core

Date: 2026-09-14

## Task

Implement the first reviewable Context History changeset: freeze the contract,
add the database ledger, and atomically capture successful durable and temporary
compactions without duplicating the raw transcript.

## Files changed

- `docs/adr/0021-context-compaction-history.md` and `docs/INDEX.md`
- `crates/gosling/src/context_mgmt/mod.rs`
- `crates/gosling/src/session/session_manager.rs`
- `crates/gosling/src/session/session_manager/{schema,migrations,message_storage,compaction_history_storage}.rs`
- compaction agent persistence seams and focused tests

## Validation

Validation commands and exact outcomes are recorded in the commit/PR summary.

## Risks and follow-ups

- Typed ACP, preference impact preview, pin/delete/purge operations, CLI, and
  Desktop timeline controls are intentionally staged for the next changeset.
- This local ledger is not an externally anchored or compliance-grade audit log.
- Secure page reclamation remains a separate explicit idle-time operation.
