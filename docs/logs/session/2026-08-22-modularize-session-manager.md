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

All 12 seams completed, each checkpointed as its own commit on
`repair/modularize-session-manager-2026-08-22`, verified after every seam
with `cargo build -p gosling`, `cargo test -p gosling --lib` (1696/1696,
matching baseline, at every single checkpoint), `cargo fmt -p gosling`, and
`cargo clippy -p gosling --all-targets -- -D warnings`, plus a repo-wide
grep-based two-deep connection check per seam confirming every external
caller still resolves through the unchanged `SessionManager`/`SessionStorage`
public API.

| # | Seam | Commit | Lines moved |
|---|---|---|---|
| 1 | `schema.rs` | `4a18315f8` | ~517 |
| 2 | `migrations.rs` | `3d0ae83b9` | ~722 |
| 3 | `legacy_import.rs` | `c2f6da27d` | ~122 |
| 4 | `pool_lifecycle.rs` | `3d89c76bb` | ~106 |
| 5 | `tool_operations.rs` | `5c0f39b26` | ~495 |
| 6 | `message_storage.rs` | `07e0c3729` | ~684 |
| 7 | `artifacts_storage.rs` | `567d91041` | ~318 |
| 8 | `library_storage.rs` | `a93c549b7` | ~207 |
| 9 | `summary_storage.rs` | `39c6f2ffb` | ~204 |
| 10 | `session_crud.rs` | `2fdbe0168` | ~467 |
| 11 | `session_listing.rs` | `f11ec202b` | ~222 |
| 12 | `session_transfer.rs` | `0ae9cd607` | ~175 |

### Rust-specific hazard confirmed during extraction (not hypothetical — hit twice)

`super::x` resolves relative to the *immediate* parent module. The facade's
own code freely wrote `super::import_formats::...` and
`super::last_message_snippet::...` meaning `crate::session::import_formats`/
`crate::session::last_message_snippet` (since the facade's parent is
`crate::session`). Moving that code one level deeper, into
`session_manager::session_listing`/`session_manager::session_transfer`,
changes what `super::` means (now `session_manager`, not `crate::session`).
Both were compile errors (`E0433`), caught immediately and fixed by
rewriting to the explicit `crate::session::...` path — recorded here as
this run's second Rust-specific hazard finding (the first, field-privacy
direction, is in the Gate 1 inventory above).

### Bug ledger

No MOD-B suspects were found. Every moved function was read in full during
extraction (Gate 1) and again while fixing each seam's compile errors; none
showed copy-drift, swallowed errors, unchecked nulls, resource leaks,
mutable-state hazards, races, off-by-ones, dead branches,
success-reported-as-error, silent-fallback misconfiguration, injection,
stale TODOs describing present defects, numeric/unit hazards, or
docstring/behavior contract mismatches. This is a negative finding, stated
explicitly rather than by omission.

### Mistakes made and caught during this run (transparency, not polish)

Three mistakes occurred during mechanical extraction, all caught by the
compiler or the test suite before verification passed — recorded here
because "verify" in copy-delegate-verify-delete is only meaningful if
failures are visible, not smoothed over:

1. **Seam 6** (`message_storage.rs`): `has_orphaned_tool_responses` was
   concatenated inside the `impl SessionStorage` block instead of after it
   (a shell-script assembly-order mistake). Surfaced immediately as
   `cannot find function in this scope` on `cargo build`. Fixed before any
   other check ran.
2. **Seam 11 / Seam 12**: two `super::` path hazards (above), each a
   compile error, fixed on sight.
3. **Seam 12** (`session_transfer.rs`): `copy_session`'s true closing brace
   was one line past where boundary computation placed it (a manual
   off-by-one in reading a `sed` preview), leaving the facade with a
   premature stray `}` and the extracted module missing `copy_session`'s
   own closing brace. Surfaced as `unexpected closing delimiter` on
   `cargo build`. Fixed, then re-verified with a full
   `cargo test -p gosling --lib` run (1696/1696) specifically because a
   brace-boundary bug is exactly the kind of mistake a compiler catches
   syntactically but a test suite is needed to confirm didn't also
   silently relocate a line of logic.

No mistake reached a commit — each was caught by `cargo build`/`cargo test`
within the same seam's verification step, before checkpointing.

## Gate 5 — Intermediary audit

- **Behavior drift**: none. Every seam is a pure code move; no MOD-B was
  silently fixed (ledger above is empty by inspection, not by skipping the
  check).
- **Import cycles**: not applicable in the language-specific sense — see
  the Rust-specific note in the Gate 2 plan. No `session_manager::*`
  submodule imports another in a way that could cycle (they only import
  `super::` items and occasionally call sibling `Self::` methods, which is
  not an import-graph edge).
- **Stale references to moved symbols**: swept via `grep -rn` per seam
  (recorded in each commit message) plus a final repo-wide pass below
  (MOD-V02/V03).
- **Facade/re-export omissions**: verified per seam; the two cases needing
  an explicit re-export (`ToolOperationStart` via
  `pub(crate) use tool_operations::ToolOperationStart;`, and
  `summary_covers_history_before` via a `#[cfg(test)]`-gated `use` for the
  inline test module) are both in place and build-verified.
- **Redundancy from the seam**: checked via a repo-wide duplicate-`fn`-name
  scan across `session_manager.rs` and all 12 submodules — zero duplicates
  found (see MOD-V06 below for the exact command).
- **Tests still asserting monolith internals**: none exist. Every one of
  the ~80 inline tests calls through `SessionManager`'s public API against
  a real temp-file SQLite store; none reference `SessionStorage`'s internal
  method locations by path.
- **Docs/ledgers describing the old structure**: `docs/TODO.md:455-462` was
  the one living-doc reference (bare file path, no line numbers) and is now
  updated (Gate 6, below). No other living doc (`architecture.md`,
  `docs/architecture/*`, ADRs, `README.md`, `AGENTS.md`, `docs/INDEX.md`,
  `docs/SHELL_PRODUCTS.md`, `docs/INTENT.md`) cites a
  `session_manager.rs:<line>` reference. `docs/cloud/*.md` and `reports/*`
  are historical, dated audit/campaign snapshots (27 files cite
  `session_manager.rs:<line>` across them) — these are frozen records of
  what was true when each audit ran and are correctly left untouched, not
  silently ignored; rewriting their line numbers would falsify the
  historical record they exist to preserve.
- **Bug ledger completeness for code moved**: covered above — empty,
  explicitly.

No findings required patching beyond what each seam's own commit already
fixed (import gaps, visibility, the three mistakes above). No feature, GUI,
deferred, high-risk, or broad-rewrite findings arose, so nothing needed
routing elsewhere.

## Gate 6 — Tests, docs, ledgers, and reality sync

- **Tests**: no test changes needed or made. All ~80 inline tests already
  verify behavior (not monolith internals) and needed no rewriting; every
  seam's checkpoint reran the full suite unchanged.
- **New structural regression checks**: none added — this run introduces
  no new product behavior to test, only code motion.
- **Docs/links/ledgers updated**: `docs/TODO.md` — see the diff in this
  commit range; the `session_manager.rs` bullet in the six-file
  modularization list is now `[x]` with a closure note and a pointer to
  this log, and the remaining five files are split into their own still-open
  bullet.
- **Record closure**: no other source record (defect ledger, audit report,
  `TODO`/`FIXME` comment) named this specific modularization as an open
  item beyond `docs/TODO.md`, which is now closed for this file.

## Gate 7 — Final verification sweep

Full baseline command set rerun after all 12 seams, compared against Gate
2:

```
cargo build                                        # exit 0, clean
cargo build -p gosling                             # exit 0, clean
cargo test -p gosling --lib                        # 1696 passed; 0 failed (matches baseline exactly)
cargo test -p gosling --lib -p gosling-cli -p gosling-server
                                                    # 1829 + 246 + 36 passed; 0 failed on the
                                                    # confirming rerun (one non-reproducing flake
                                                    # on the first combined run, see MOD-V01 below)
cargo fmt -p gosling && cargo fmt --check -p gosling  # clean, no pending diff
cargo clippy -p gosling --all-targets -- -D warnings   # clean
cargo clippy --workspace --all-targets -- -D warnings  # clean
```

### MOD-V01 — Regression vs baseline

Checked. `cargo test -p gosling --lib` matches the Gate 2 baseline exactly
(1696/1696) at every one of the 12 seam checkpoints and again here at the
end — zero deviation throughout the run.

The wider `cargo test -p gosling --lib -p gosling-cli -p gosling-server`
run (not part of the Gate 2 baseline, which was scoped to `-p gosling`
matching this repo's documented `cargo test -p gosling` convention) showed
1829 gosling-lib tests rather than 1696 — expected: Cargo's feature
unification activates additional `#[cfg(feature = ...)]`-gated tests when
built alongside other workspace members, unrelated to this run's changes.
One test, `tracing::langfuse_layer::tests::test_create_langfuse_observer`
(a Langfuse-observability test wholly unrelated to session storage),
failed on the first combined run and passed on an immediate rerun of the
identical command, and also passed in isolation
(`cargo test -p gosling --lib tracing::langfuse_layer::tests::test_create_langfuse_observer`).
Per the flaky-result protocol: both runs are recorded, and this is treated
as a pre-existing test-isolation flake (very likely a process-global
env-var race under parallel test execution, a known hazard for
observability-client tests), not a regression this run introduced —
nothing in the moved `session_manager` code touches tracing/Langfuse.

### MOD-V02 — Broken source references

Checked, per seam and again in aggregate. No import, re-export, `__all__`-
equivalent (`pub use` list in `session/mod.rs`), entry point, or
string-referenced module path was broken: `session/mod.rs`'s `pub use
session_manager::{...}` and `pub(crate) use session_manager::ToolOperationStart;`
lines resolve unchanged (verified by successful compilation of every
dependent crate). No config, DI wiring, or plugin registry references
`session_manager`'s internal file layout anywhere in the repo (session
storage has no plugin/registry surface).

### MOD-V03 — Broken doc links

Checked — see Gate 5 and Gate 6 above. One living-doc reference
(`docs/TODO.md`) updated; historical audit/campaign docs (`docs/cloud/*`,
`reports/*`, `docs/logs/session/*`) intentionally left with their original
line-number citations, which remain accurate as historical evidence of
what was true when each was written.

### MOD-V04 — Orphaned modules and symbols

Checked. Every one of the 12 new `session_manager/*.rs` files is declared
via `mod ...;` in the facade and is reachable (confirmed by the build
succeeding — an unreferenced `mod` with `#![warn(unused)]`-equivalent
lints, and this workspace runs `-D warnings`, would fail otherwise). No
facade name was left with zero callers: every `pub`/`pub(super)`/
`pub(crate)` item introduced by this run has at least one caller, verified
per-seam via grep and by the absence of any clippy `dead_code` finding
across `cargo clippy --workspace --all-targets -- -D warnings`.

### MOD-V05 — Orphaned processes

Checked. Gate 0 snapshot recorded no dev servers, watchers, or test
runners for this repo running before the run started. Process check
re-run now: no `cargo`/`rustc` processes remain running. Snapshot diff
clean — nothing this run spawned was left behind.

### MOD-V06 — Redundancy

Checked. Repo-wide duplicate-function-name scan across
`session_manager.rs` and all 12 submodules:

```
grep -hoE "^\s*(pub\(super\) |pub\(crate\) |pub )?(async )?fn [a-zA-Z_][a-zA-Z0-9_]*" \
  crates/gosling/src/session/session_manager.rs crates/gosling/src/session/session_manager/*.rs \
  | sed -E 's/^\s*//;s/^(pub\(super\)|pub\(crate\)|pub) //;s/^async //;s/^fn //' \
  | sort | uniq -c | sort -rn | awk '$1>1'
```

Zero duplicates. No facade body was left un-delegated alongside a module
copy (Rust's own "duplicate definition" compile error made this
impossible to get wrong silently — see the Gate 2 Rust-specific note).

### MOD-V07 — Module-identity spot checks

N/a, as flagged in the Gate 2 plan: `SessionStorage`'s fields are the only
module-identity-sensitive item, and every submodule housing a former
`impl SessionStorage` method is a direct child of `session_manager`
(verified by the compiler's field-privacy errors during extraction, which
is exactly the mechanical spot-check this code demanded — see Gate 2's
Rust-specific note). No serialization path stores a module name (`Session`/
`SessionSummary`/etc. serialize via serde field names), no
`getLogger(__name__)`-equivalent logger namespace exists in this file, and
no string-keyed registry references `session_manager`'s internal layout.

### MOD-V08 — Doc and comment freshness

Checked. Every one of the 12 new modules carries the required header
comment (single responsibility, `Extracted from ... in a behavior-preserving
modularization` provenance line, and — where applicable — which facade
re-export depends on it). No moved code contains a stale comment pointing
at a line number, section, or "above/below" location that no longer
applies (verified while reading each seam's extracted text; none of the
moved code used relative "see above" phrasing to begin with).

### MOD-V09 — Import graph health

Checked. No new import cycles are possible in the Rust sense used here
(see Gate 2's Rust-specific note — inherent `impl` blocks aren't subject to
the file-order cycle hazard the language-agnostic hazard catalog assumes).
No import-time side effects exist in any moved code (no module-level
`static`/`const` initializers with side effects were moved; `SESSION_STORAGE`
stays in the facade, untouched).

### MOD-V10 — Test integrity

Checked. Zero test files were edited. No assertion was weakened, no test
skipped or deleted, no tolerance widened, no snapshot regenerated. The
full inline suite (~80 tests) passed identically at every one of the 12
seam checkpoints and at this final sweep.

## Adversarial walkthrough

- **Workflow/data paths**: traced the session-create → message-write →
  tool-operation → artifact-discovery → summary → export/import/copy chain
  across its new module boundaries; every cross-module call is a
  `Self::method(...)` or `self.method(...)` call resolved by the compiler
  identically to before the move (Rust does not distinguish which file an
  inherent-impl method lives in).
- **Imports/re-exports/public APIs**: `crate::session::session_manager::*`
  (`SessionManager`, `SessionStorage`, `Session`, `SessionType`, and every
  other name grepped as an external dependency at Gate 2) all resolve
  unchanged — proven by every downstream crate (`gosling-cli`,
  `gosling-server`, and `gosling`'s own `acp`/`agents`/`tagteam`/`workspace`
  modules) compiling clean throughout.
- **Error paths and fallback behavior**: unchanged — no error-handling
  code was rewritten, only relocated; the one behavior-relevant read of
  every moved function (Gate 1) found no fallback logic worth flagging as
  a MOD-B suspect.
- **Test coverage for new modules and facade**: the existing inline suite
  already exercises every extracted responsibility through the facade's
  public API; no coverage gap was introduced because no new public surface
  was created (only re-exports of what already existed).
- **Docs/ledgers/logs describing the new structure**: this log, plus the
  `docs/TODO.md` closure entry.

## Final status: `completed_verified`

Summary: `crates/gosling/src/session/session_manager.rs` (9349 lines,
independently pre-authorized by `docs/TODO.md:455-462`) is modularized
into 12 responsibility-scoped submodules under
`crates/gosling/src/session/session_manager/`. The facade retains every
public type, constant, and the `SessionManager` delegating API unchanged;
`SessionStorage`'s ~4150-line `impl` monolith is fully decomposed. Zero
behavior changes, zero MOD-B findings, zero test edits, full workspace
build/test/clippy/fmt clean, one confirmed-flaky (not caused) test
recorded and explained. Three extraction mistakes were made and caught by
the compiler/test suite before any checkpoint — recorded above rather than
smoothed over.

Recommended follow-up: the remaining five files named in
`docs/TODO.md:459-462` (`acp/server.rs`, `agents/agent.rs`,
`agents/extension_manager.rs`, `agents/platform_extensions/summon.rs`,
`ui/desktop/src/main.ts`) remain open, each a separate one-file-per-run
modularization per this same skill.
