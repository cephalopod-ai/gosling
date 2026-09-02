# 2026-09-02 — Performance repair for PERF-GSL-006 through PERF-GSL-010

**Branch:** `main`  
**Task:** Confirm the five supplied performance findings, PERF-GSL-006 through
PERF-GSL-010, apply the smallest behavior-preserving repairs, and record
reproducible evidence.

## Confirmation evidence

| Finding      | Before                                                                                                                                             | After                                                                                                                                                                 | Preserved behavior                                                                                                                                            |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| PERF-GSL-006 | A 10,000-message/50-hit SQLite `scanstats` fixture executed the correlated count 50 times and visited 500,000 rows through `idx_messages_session`. | The bounded FTS match set is materialized once; the matched-session aggregate executed once and visited 10,000 rows.                                                  | Both queries returned an aggregate count of 500,000. The Rust regression also verifies that a one-hit result reports the target session's two total messages. |
| PERF-GSL-007 | `EXPLAIN QUERY PLAN` selected `idx_messages_session` and reported `USE TEMP B-TREE FOR ORDER BY`.                                                  | Migration 31 adds `idx_messages_session_time_asc`; the plan selects it without a temporary sort.                                                                      | Migration and query-plan regression covers an existing v30 database.                                                                                          |
| PERF-GSL-008 | Each event created and discarded one JSON output `Vec` solely to read its length.                                                                  | JSON is written to a byte-counting sink without retaining output bytes.                                                                                               | Tests compare exact encoded lengths for an empty event and escaped/unicode content; the existing 1,024-byte serialization-error fallback remains.             |
| PERF-GSL-009 | Every tool response scanned prior messages and content blocks in reverse, O(N) per response.                                                       | The reply builds the latest request-ID/name map once, updates it on streamed requests, rebuilds it on history replacement, and performs one hash lookup per response. | Duplicate IDs retain the latest request name and missing IDs still report `unknown`.                                                                          |
| PERF-GSL-010 | Each resolved-model message performed two backward history scans, yielding O(N²) inspections over a render pass.                                   | The existing memoized render index computes both states in one O(N) pass; rendering performs two O(1) array reads.                                                    | UI regression covers synthetic changes, recorded-switch suppression, and the next synthetic change.                                                           |

These are operation-count and query-plan measurements, not wall-clock speedup
claims. The SQLite fixture used SQLite 3.51.0 and the same 10,000-message input
for both query forms.

## Files changed

- `crates/gosling/src/session/chat_history_search.rs`
- `crates/gosling/src/session/session_manager.rs`
- `crates/gosling/src/session/session_manager/migrations.rs`
- `crates/gosling/src/session/session_manager/schema.rs`
- `crates/gosling-server/src/session_event_bus.rs`
- `crates/gosling-server/src/routes/reply_service.rs`
- `ui/desktop/src/components/ProgressiveMessageList.tsx`
- `ui/desktop/src/components/ProgressiveMessageList.test.tsx`
- `docs/architecture.md`
- `docs/TODO.md`
- `docs/logs/session/2026-09-02-performance-repair.md`
- `.gitignore` (keeps this required session record trackable)

## Validation

- `cargo test -p gosling -p gosling-server` — passed; all executed tests and
  doc-tests succeeded, with the suite's existing ignored tests unchanged.
- `cargo clippy --all-targets -- -D warnings` — passed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- `pnpm test:run src/components/ProgressiveMessageList.test.tsx` — passed, 5/5.
- `pnpm run typecheck` — passed.
- Focused ESLint and Prettier checks for the two changed UI files — passed.
- SQLite 3.51.0 `scanstats` and `EXPLAIN QUERY PLAN` fixture — passed with the
  before/after work counts and plans shown above.
- The required `GILES:DOCS-GOVERNANCE:START` marker remains present in
  `AGENTS.md`.

The UI commands were run through `source bin/activate-hermit`; one discarded
invocation used the system pnpm 10.6.4 and stopped at the repository's pnpm
engine guard before running tests.

## Contract and record closure

- The persisted message format and public ACP/HTTP contracts are unchanged.
- The session database advances from schema version 30 to 31 with an additive
  index migration; `docs/architecture.md` now reflects the live version.
- The historical source audit remains unchanged as retained evidence.
- `docs/TODO.md` records PERF-GSL-006 through PERF-GSL-010 as closed. The
  active-only ledger is unchanged because none of these findings remains open.
- A second-pass diff review found no unrelated source changes or new
  architecture/ADR divergence.

## Residual costs and rollback

- The FTS query still ranks and materializes up to its existing 50-row cap and
  still serializes normal SQLite access behind the same read operation.
- The composite index adds database storage, write amplification, and a one-time
  index-build cost when migration 31 runs.
- The event bus still pays JSON serialization CPU; only the discarded output
  allocation is removed.
- The telemetry map adds one O(N) initialization pass and transient storage per
  reply. The UI render index adds two O(N) arrays.
- Before release, the source changes can be reverted together. Once migration 31
  has run on a user database, removing the additive index would require an
  explicit later migration; no destructive rollback is performed automatically.
