# Modularize `session_manager.rs` — gated run log

Skill: `repair-source-modularization` (agent-skills `020_repair`).
Branch: `repair/modularize-session-manager-2026-08-22`.
Routed by: `docs/TODO.md:455-462` (ARC-GSL-001 follow-up — explicit pre-authorized
target, first of six >=2000-line files listed there).

## Gate 0 — Orientation

- `git status --short`: clean. Branch created from `main` at `9ce0988b8`.
- Repo contract: `AGENTS.md` read (build/test/lint commands, crate layout,
  "Test: prefer tests/ folder" — this file's tests are inline `#[cfg(test)]`,
  pre-existing convention, not changed by this run).
- Docs/ledger layout: `docs/logs/session/` is the established session-log
  location (19 prior entries, e.g. `2026-08-19-shell-defect-campaign.md`).
  No `docs/modularity-logs/` or similar exists yet — using
  `docs/logs/session/` per repo convention instead of inventing a new
  directory.
- No repo-documented explicit line-count threshold in `CONTRIBUTING.md` or
  `docs/architecture.md`; the repo's own convention (`docs/TODO.md`) uses
  ">=2000 lines" / ">=4000 lines" as its own working thresholds for this
  exact routing decision.
- Process/port snapshot: no dev servers, watchers, or test runners for this
  repo were running. Only pre-existing, unrelated processes: an already
  packaged `/Applications/Gosling.app` instance (installed earlier this
  session, unrelated task) and this machine's normal VSCode/Claude Electron
  helpers. None of these are attributable to this run.

## Gate 1 — Target lock and full-file read

Target: `crates/gosling/src/session/session_manager.rs` (9349 lines).
Explicit target mode (user-supplied). No other candidate files considered
in this run — one-file-per-run.

Full file read in full (lines 1-5670, the production code) plus a
structural census of the inline test module (lines 5671-9349, ~80 test fns
+ fixtures) via full signature enumeration (every `fn`/`async fn`/
`struct`/`enum`/`impl`/`const`/`static` at every nesting level) plus direct
reading of representative tests. The test module is not an extraction seam
target (it's inline `#[cfg(test)]`, not a separate file) and every test in
it drives the file's already-public `SessionManager`/`SessionStorage` API
end-to-end against a real temp-file SQLite store — none of it is coupled to
internal layout, so it is expected to keep passing unchanged through every
seam (this is verified per-seam at Gate 4, not assumed).

### Symbol inventory (top-level, production code)

**Module-level (imports, consts, small types) — lines 1-124, retained in facade:**
`CURRENT_SCHEMA_VERSION`, `SESSIONS_FOLDER`, `DB_NAME`,
`MILLISECOND_TIMESTAMP_THRESHOLD`, `DEFAULT_SESSION_TAIL_LIMIT`,
`MAX_SESSION_MESSAGE_PAGE_LIMIT`, `TOOL_OPERATION_SCHEMA_VERSION`,
`SessionFileImportResult`, `validate_session_name` (+ its own test mod),
`SessionType`, `SessionWorkflow`, `SESSION_STORAGE` static.

**Session type + related (127-365), retained in facade:**
`Session`, `SessionSummaryStatus` (+`Display`/`FromStr`),
`summary_covers_history_before` (moves to `summary_storage.rs`, its only
caller), `SessionSummary`, `SessionSummaryFact`, `SessionMessagePage`,
`SessionMessageSearchMatch`, `SessionMessageSearchResults`,
`SessionArtifactPage`, `impl From<&Session> for TokenState`,
`SessionUpdateBuilder` (+ `impl`, 366-539), `SessionInsights`.

**SessionManager facade (541-1206), retained in facade — this is the public
API surface the whole run must not change:**
`SessionManager` struct, `ToolOperationStart` (moves to
`tool_operations.rs`, re-imported), `SessionListCursor`, `SessionListPage`,
`SessionArchiveState`, `SessionListFilters`, `SessionListPageQuery`,
`SessionListQuery` (all four move to `session_listing.rs`),
`keyword_terms`, `message_keyword_clause` (move to `session_listing.rs`),
`SessionNameUpdate` (retained), `impl SessionManager` — ~70 thin
`self.storage.foo(...)` delegating methods, unchanged by this run (they
call through `SessionStorage`, whose methods move; the delegation
signatures do not).

**SessionStorage struct + row-level impls (1208-1407), retained in facade:**
`SessionStorage` struct (fields: `pool`, `initialized`, `session_dir`,
`owner_id`, `active_tool_operations` — all private; submodules must be
descendants of `session_manager` to see them, see Rust-specific note
below), `role_to_string`, `message_timestamp_to_datetime`,
`normalized_message_timestamp_sql`, `session_sort_at`, `impl Default for
Session`, `impl Session` (`without_messages`), `impl FromRow for Session`.

**`impl SessionStorage` (1409-5557), ~4148 lines — the monolith body being
carved into 11 seams below.**

**Free functions after the impl block (5559-5669), distributed to seams:**
`serialize_tool_operation_result`, `tool_operation_request_digest`,
`deserialize_tool_operation_result` → `tool_operations.rs`;
`SessionArtifactRow` + `session_artifact_from_row` → `artifacts_storage.rs`;
`SessionLibraryItemRow` + `session_library_item_from_row` →
`library_storage.rs`; `has_orphaned_tool_responses`, `merge_tool_meta` →
`message_storage.rs`.

### Rust-specific extraction note (not in the language-agnostic hazard
catalog, recorded here as this run's own hazard finding)

Unlike the dynamic-language case the hazard catalog assumes, an inherent
`impl SessionStorage { ... }` block can be split across as many files as
needed in the same crate — callers write `storage.foo(...)` and never
reference which file implements `foo`. Two consequences change how
copy-delegate-verify-delete applies here:

- **No "thin wrapper" step.** There is no facade stub to leave in place;
  once a method's body is physically moved to a new file's `impl
  SessionStorage` block, the version in `session_manager.rs` is deleted in
  the same edit (Rust rejects two definitions of the same associated
  function for one type as a hard compile error — `error[E0592]: duplicate
  definitions`). Copy-then-immediately-delete is therefore safe: the
  compiler itself is the parity check, not a follow-up verification step.
- **Field privacy, not method placement, is the real hazard.** `pool`,
  `initialized`, `session_dir`, `owner_id`, `active_tool_operations` are
  private fields of `SessionStorage`, declared in the `session_manager`
  module. Rust's privacy model makes a private item visible to its
  defining module and that module's *descendants*. Every extracted
  submodule must therefore be declared as a child of `session_manager`
  (`crates/gosling/src/session/session_manager/<seam>.rs`, added via `mod
  <seam>;` inside `session_manager.rs`) — a sibling module (e.g. directly
  under `crate::session`) would not compile against the private fields.
  This is the one Rust-specific "module-identity" hazard for this run and
  is verified automatically: an incorrectly-placed submodule fails to
  compile (`error[E0616]: field ... is private`), it does not silently
  misbehave.

No pickle-equivalent serialization paths, logger-namespace
(`getLogger(__name__)`-equivalent) hazards, or reflection/registry
string-keyed lookups exist for this type: `Session`/`SessionSummary`/etc.
serialize via `serde` field names, not module paths, and nothing in the
two-deep sweep (below, repeated per seam) references `session_manager`'s
internal file layout as a string. MOD-V07 is therefore expected to be
`n/a` for every seam; each seam's Gate 4 still checks this rather than
assuming it.

## Gate 2 — Baseline

Commands run on branch `repair/modularize-session-manager-2026-08-22`
before any source edit:

```
source bin/activate-hermit
cargo build -p gosling            # exit 0
cargo test -p gosling --lib       # 1696 passed; 0 failed; 0 ignored
cargo clippy -p gosling --all-targets -- -D warnings   # clean, exit 0
```

No pre-existing failures. (The previously known-failing
`context_mgmt::summarizer::tests::defaults_to_off` is already resolved per
`docs/TODO.md`'s "Known-failing test predating this work" section, `93a19738d`
— reconfirmed clean here, not something this run needs to carry.)

## Gate 2 — Extraction plan

Target file: `crates/gosling/src/session/session_manager.rs`
Reason selected: explicit user instruction; independently pre-authorized by
`docs/TODO.md:455-462` (largest of six routed files, 9349 lines).
Current responsibilities: session CRUD/listing/import/export, message
read/write/paging/search, tool-operation ledger, session artifacts,
session library items, session summaries, schema definition and the full
migration ladder (v1..28), legacy on-disk import, and the thin
`SessionManager` facade that the rest of the crate calls.
Public compatibility names (verified via repo-wide grep of
`session_manager::`): `SessionManager`, `SessionStorage`, `Session`,
`SessionType`, `SessionWorkflow`, `SessionSummary`, `SessionSummaryFact`,
`SessionSummaryStatus`, `SessionUpdateBuilder`, `SessionInsights`,
`SessionNameUpdate`, `SessionArtifactPage`, `SessionFileImportResult`,
`ToolOperationStart` (`pub(crate)`), `DB_NAME`, `SESSIONS_FOLDER`,
`DEFAULT_SESSION_TAIL_LIMIT`, `MAX_SESSION_MESSAGE_PAGE_LIMIT`. All of
these stay declared directly in `session_manager.rs`; only `impl
SessionStorage` method bodies move.
Module-identity hazards: see the Rust-specific note above (field privacy →
submodules must be descendants of `session_manager`). No serialization,
logger-namespace, or registry hazards identified.

Extraction seams, in execution order (chosen for review clarity; Rust's
compiler — not import cycles — is what actually gates correctness here,
so strict leaf-first ordering is a readability choice, not a technical
requirement):

1. **`session_manager/schema.rs`** (~515 lines) — `create_schema`,
   `create_message_search_schema`, `create_tool_operations_schema`,
   `create_session_artifacts_schema`, `create_session_library_schema`,
   `backfill_session_artifacts`. One responsibility (schema DDL); kept as
   one seam rather than split further because splitting table-creation
   from its own migration-time callers would scatter one concept for no
   auditability gain. Two-deep check: grep `create_schema\(`,
   `create_.*_schema\(`, `backfill_session_artifacts\(` for all callers
   (expected: only inside `SessionStorage`, from `pool()` and
   `apply_migration`).
2. **`session_manager/migrations.rs`** (~725 lines) — `run_migrations`,
   `get_schema_version`, `update_schema_version`, `apply_migration`.
   Exceeds the ~400-line seam cap by design: `apply_migration` is a single
   ordered match over schema versions 1..28 (already carries
   `#[allow(clippy::too_many_lines)]`); splitting a version ladder across
   files would make the migration history harder to audit, not easier.
   Recorded here as a deliberate exception per the skill's "one cohesive
   responsibility" principle overriding the line-count guideline.
3. **`session_manager/legacy_import.rs`** (~125 lines) —
   `legacy_import_completed`, `mark_legacy_import_complete`,
   `import_legacy`, `import_legacy_session`. Depends on
   `replace_conversation_in_tx` (seam 6) — a same-crate `Self::` call, not
   a Rust import-cycle hazard (see Rust-specific note); sequencing here is
   for review order only.
4. **`session_manager/pool_lifecycle.rs`** (~150 lines) — `create_pool`,
   `new`, `pool`, `create`. Orchestrates seams 1-3.
5. **`session_manager/tool_operations.rs`** (~475 lines) —
   `begin_tool_operation`, `release_tool_operation`,
   `mark_tool_operation_in_doubt`, `complete_tool_operation`,
   `persist_tool_operation_response`, `recover_tool_operations`,
   `cancel_undispatched_tool_requests`, plus free fns
   `serialize_tool_operation_result`, `tool_operation_request_digest`,
   `deserialize_tool_operation_result`, and `ToolOperationStart` (moved
   from the facade, re-imported by `impl SessionManager` via `use`).
   `persist_tool_operation_response`/`cancel_undispatched_tool_requests`
   call `Self::upsert_message_in_tx` (seam 6) — same-crate `Self::` call.
6. **`session_manager/message_storage.rs`** (~930 lines) — all message
   read/write/paging/search plus conversation replace/truncate:
   `get_conversation`, `row_to_message`, `get_message_page_rows`,
   `get_session_message_page`, `get_session_tail_page`,
   `get_session_message_rows_between`, `get_session_message_window`,
   `add_message`, `upsert_message`, `upsert_message_in_tx`,
   `search_session_messages`, `update_message_metadata`,
   `update_tool_request_meta`, `replace_conversation_in_tx`,
   `replace_conversation_inner`, `replace_conversation` (pub),
   `truncate_conversation`, `truncate_conversation_from_message`, plus
   free fns `has_orphaned_tool_responses`, `merge_tool_meta`. Kept as one
   seam (exceeds ~400 lines) because "the message store" is one
   responsibility and `add_message`/`upsert_message_in_tx` share
   near-identical bodies that must not be pulled apart into two files
   (would manufacture MOD-B01 copy-drift risk per the hazard catalog).
7. **`session_manager/artifacts_storage.rs`** (~500 lines) —
   `discover_message_artifacts_in_tx`,
   `register_completed_assistant_artifacts`, `upsert_artifacts_in_tx`,
   `upsert_session_artifacts`, `get_session_artifact`,
   `list_session_artifacts`, `SessionArtifactRow`,
   `session_artifact_from_row`.
8. **`session_manager/library_storage.rs`** (~200 lines) —
   `session_library_scope_keys`, `list_session_library_items`,
   `add_session_library_item`, `remove_session_library_item`,
   `get_session_library_items`, `SessionLibraryItemRow`,
   `session_library_item_from_row`.
9. **`session_manager/summary_storage.rs`** (~260 lines) —
   `get_session_summary`, `get_session_summary_facts`,
   `upsert_session_summary`, `replace_session_summary_facts`,
   `get_session_for_compacted_resume`, `summary_covers_history_before`
   (moved from module level — its only caller lives here).
10. **`session_manager/session_crud.rs`** (~500 lines) — `create_session`,
    `create_session_in_tx`, `get_session`, `apply_update`,
    `apply_update_in_tx`, `record_usage`, `delete_session`,
    `get_insights`.
11. **`session_manager/session_listing.rs`** (~350 lines) —
    `list_sessions_matching`, `list_sessions_by_types`,
    `list_sessions_paged`, `list_sessions`, plus the private types
    `SessionListQuery`, `keyword_terms`, `message_keyword_clause` (moved
    from module level — only used here). `SessionListCursor`,
    `SessionListPage`, `SessionArchiveState`, `SessionListFilters`,
    `SessionListPageQuery` (declared near `SessionManager`, `pub(crate)`,
    used by the facade's `list_sessions_paged` delegating method) stay
    declared in the facade since `impl SessionManager` references them
    directly in its public-ish signature; `session_listing.rs` does
    `use super::{...}`.
12. **`session_manager/session_transfer.rs`** (~200 lines) —
    `export_session`, `import_session`, `copy_session`.

Tests/docs/ledgers:
- Tests to update: none of the ~80 inline tests need rewriting — they
  exercise `SessionManager`'s public API only. Verified per-seam at Gate 4
  by running `cargo test -p gosling --lib` after every seam.
- New structural regression checks: none planned beyond the existing
  behavior suite; this run does not introduce new product behavior to
  test.
- Docs/links/ledgers to update: `docs/TODO.md:455-462` (mark this file
  done in the routed list once all six... only this one file is in scope
  this run, see Stop Conditions below), this session log itself (MOD-V03
  sweep will check for any other doc referencing
  `session_manager.rs:<line>` whose line target moved).

Risks:
- Import cycle risk: none (Rust `impl` blocks aren't file-order
  sensitive; see Rust-specific note).
- Monkeypatch/import compatibility risk: none — Rust has no monkeypatching;
  the only compatibility surface is the `crate::session::session_manager::*`
  path, preserved by keeping all type/const declarations in the facade.
- Module-identity risk: none beyond private-field visibility (mitigated by
  submodule placement, compiler-verified).
- Workflow/data-path risk: none — no behavior changes, pure code motion.
- Validation gaps: none identified; full `cargo build`/`test`/`clippy` is
  available and cheap to rerun after each seam.

## Gate 3-4 — Seam log

(Filled in as each seam completes; see commits on this branch.)
