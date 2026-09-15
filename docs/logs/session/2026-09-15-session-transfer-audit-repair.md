# 2026-09-15 — Session-transfer dataflow/workflow/architecture audit and repair

## Task and authority

The user asked to re-run the dataflow, workflow, and architecture audit skills, thoroughly, and
patch the findings. Read order completed: `AGENTS.md` (`GEMINI.md` absent), the 2026-09-15
multi-lens report already on file for this repository
(`/Users/eric/Work/vscode/agent-skills/010_audit/audit-architecture-seam/generated/2026-09-15-report.md`,
audited `main` at `c5cddc428`), and the architecture-drift reports for host-enforced planning. That
report explicitly named its unreviewed frontier; the highest-value item matching all three requested
lenses was **"session import/copy/fork, handoff and planning ledger state transitions."** This pass
re-verified that report's three repaired findings at current HEAD, then ran a fresh, evidence-driven
audit of that named frontier using the loaded `audit-architecture-seam`, `audit-architecture-drift`,
`audit-workflow-gui`, `audit-dataflow-{integrity,concurrency,state-transition,cascade,input-output,
temporal}` skill instructions and their shared `000_common/audit-base` contracts (`audit_method.md`,
`finding_format.md`, `severity_matrix.md`, `confidence_calibration.md`, `evidence_discipline.md`).

This was a targeted deep pass on one named surface, not a mechanical whole-repo walk of every
taxonomy code across all nine lenses — the audit method's own effort-budgeting rule ("prioritization
order," "sampling rule for large surfaces") calls for exactly that trade-off, and it is stated
explicitly here rather than implied.

## Verification of prior findings

All three findings from the 2026-09-15 report are confirmed repaired at current HEAD (verified by
source read, not by re-running the skill):

- **WFG-GSL-001** — `recall_brief/render.rs:32-33` now derives `retrieval_partial` from
  `receipt_reports_partial`, not a bare `partial == Some(true)` check.
- **STT-GSL-001** — `compaction_history_storage.rs`/`acp/server/compaction_history.rs` now report
  `compaction_history_partial_apply` and commit reconciliation before the config setter.
- **REL-GSL-001** — the stale `CURRENT_SCHEMA_VERSION == 35` assertion in `plan_storage.rs` no longer
  exists.

## New findings and repairs

### CAS/CON-1: `export_session` held the process-global write gate for a pure read

**Domain:** Cascade (blast radius) / Concurrency. **Confidence:** Confirmed (source-evidenced).

`SessionStorage::export_session` (`session_transfer.rs`) acquired `acquire_write_guard()` — a single
`tokio::sync::Mutex<()>` shared by the whole `SessionStorage` instance (one per process; `SessionManager::instance()`
clones a `static SESSION_STORAGE`, so this is process-wide in `gosling serve`) — and a `BEGIN
IMMEDIATE` transaction, even though its only work is three read helpers
(`get_session_with_messages_in_tx`, `native_plan_history_in_tx`, and their nested SELECTs) with no
INSERT/UPDATE anywhere in the path. Since every mutating session operation
(`apply_update`, `import_session`, `copy_session`, plan approval, compaction commit, tool-operation
begin) also goes through the same guard, exporting one large session blocked every other session's
writes in the same process for the export's full duration — the same class of stall previously
diagnosed as the "write-gate stall" pattern (see the operator's own prior diagnosis notes).

**Repair:** `export_session` now uses `pool.begin()` (a plain deferred transaction, matching the
existing read-only idiom already used by `compaction_history_stats`) and no longer takes the write
guard. Read consistency across the several SELECTs is unaffected.

**Evidence:** `crates/gosling/src/session/session_manager/session_transfer.rs`, `pool_lifecycle.rs:112-115`
(`write_gate` field), `session_manager.rs:613-616` (`SessionManager::instance()` sharing one `Arc`).

### CON-2 / WFG-2: concurrent identical imports could create duplicate sessions, and the primary Desktop import UI could not tell

**Domain:** Concurrency (primary) / Workflow-GUI. **Confidence:** Confirmed (source-evidenced;
reproduced under real concurrency in a new regression test).

`SessionManager::import_session`'s content-hash dedup (`imported_session_by_sha256`) ran as a plain
`SELECT` **before** the write-guarded creation transaction — a textbook check-then-act. No database
constraint enforces uniqueness on the import provenance hash (it lives inside a JSON
`extension_data` blob, not a first-class indexed column). Two concurrent imports of identical
content could both pass the pre-check and both create a session. This path is reachable from the
primary Desktop import UI, not just the CLI: ACP's `on_import_session`
(`crates/gosling/src/acp/server/manage_sessions.rs`) is the sole backing for all three Desktop import
entry points (`handleImportClick`, `handleImportSession`, `handleImportNostrLink` in
`SessionListView.tsx`) plus the CLI's Nostr branch — any double-click, client retry after a slow
response, or two near-simultaneous requests would race. There was no existing test for this; the
sibling CLI-file import path (`import_session_file`) had the identical check-then-act shape.

Compounding this, `import_session`'s `Result<Session>` return type carried no signal distinguishing
"created" from "the dedup matched and returned the pre-existing session," so **every** Desktop import
call site unconditionally showed `toast.success("Session imported successfully")` regardless of
which happened — a status-truthfulness gap (the operator cannot tell whether a new session was
actually created) that also masked the race above. The CLI's separate file-import path already got
this right (`SessionFileImportResult::Imported/AlreadyImported/SourceChanged`); the JSON-string/ACP/
Nostr path did not — the "sibling implementation" angle the audit method's shared baseline calls out.

**Repair:**

- The dedup lookup (renamed `imported_session_id_by_provenance_in_tx`) now runs **inside** the same
  `BEGIN IMMEDIATE` transaction as session creation, so the check and the act share one atomic unit;
  a losing concurrent caller reads back the winner's committed session instead of creating a
  duplicate.
- `SessionFileImportResult` is generalized to `SessionImportOutcome` (`Imported` /
  `AlreadyImported` / `SourceChanged`) and used by both `import_session` and `import_session_file`,
  closing the same gap for both transports with one mechanism instead of two divergent ones.
- `ImportSessionResponse` (ACP/SDK types) gained an additive `alreadyImported: bool` field
  (`#[serde(default)]`, backward compatible). `on_import_session` populates it from the outcome.
  Desktop's `acpImportSession` wrapper now returns `{ alreadyImported }`, and all three
  `SessionListView.tsx` import handlers show a distinct, honest toast
  (`sessions.toast.alreadyImported`) when the backend reused an existing session instead of creating
  one. The CLI's Nostr branch now reports the same three outcomes the file-import branch always did.
- New regression test `concurrent_identical_imports_create_exactly_one_session`: 20 concurrently
  spawned imports of identical content produce exactly 1 `Imported` and 19 `AlreadyImported`, all
  resolving to the same session id, and `list_all_sessions` shows exactly one row.

**Evidence:** `session_transfer.rs` (dedup-in-tx, `SessionImportOutcome`), `session_manager.rs`
(wrapper simplification, new test), `crates/gosling/src/acp/server/manage_sessions.rs::on_import_session`,
`crates/gosling-sdk-types/src/custom_requests.rs::ImportSessionResponse`, `ui/desktop/src/acp/sessions.ts`,
`ui/desktop/src/components/sessions/SessionListView.tsx`.

### Bonus, discovered while regenerating the schema for the fix above: the checked-in ACP schema/SDK types were stale

Regenerating `acp-schema.json`/`acp-meta.json`/`ui/sdk/src/generated/*` for the `alreadyImported`
field produced a much larger diff than that one field — it also added the entire Recall Brief
method family (`RecallBriefRequest_unstable`/`RecallBriefResponse_unstable` and related schema
entries). `git log -- crates/gosling/acp-schema.json` shows its last regeneration was commit
`97ffa1077`, which predates `c5cddc428` ("feat(core): add opt-in ACP recall brief..."). The Recall
Brief method is genuinely registered (`#[custom_method(RecallBriefRequest)]` in
`acp/server/custom_dispatch.rs`), `Cargo.lock` did not change as part of this regeneration (ruling
out dependency-version drift as the explanation), and CI's `.github/workflows/ci.yml` runs `just
check-acp-schema` as a named, gating step ("Check ACP Schema is Up-to-Date"). This is a genuine
generated-artifact/source drift: the checked-in schema and generated SDK types were out of date
relative to source for at least three feature commits. This audit did not investigate *why* CI did
not catch it (a separate, out-of-scope question — possibly the check ran and was overridden, or the
job was skipped on the relevant PR); that is recorded as an open follow-up, not asserted as
understood.

**Repair:** the regeneration is committed alongside this change (`just generate-acp-types`, run
twice, byte-identical both times — confirms determinism and that no further drift remains).

## Validation

- `cargo fmt --all -- --check`: clean.
- `cargo clippy --all-targets --locked -- -D warnings` (full workspace): clean.
- `cargo test -p gosling --lib --locked` (excluding the pre-existing, environment-dependent
  `prompt_manager` snapshot baseline failures documented in the 2026-09-15 audit-findings-repair
  log): 1,978 passed, 0 failed, 4 ignored.
- `cargo test -p gosling --lib session:: --locked` under a clean `GOSLING_PATH_ROOT`: 219 passed
  (includes the new concurrency regression test and the updated import/plan-transfer tests).
- `cargo test -p gosling --lib plan_storage:: --locked`: 22 passed, 1 ignored.
- `cargo test -p gosling-cli -p gosling-sdk-types --locked`: all passed, including the 3 focused
  Recall Brief wire-contract tests (untouched by this change, confirming no regression there).
- `just generate-acp-types` run twice: identical output both times.
- Desktop: `pnpm run lint:check` (typecheck, ESLint, and 21 i18n sync/validation tests across all 15
  non-English locales, 1,312 messages) passed after rebuilding `@repo-makeover/gosling-sdk`'s `dist/`
  output, which Desktop consumes instead of `ui/sdk/src/generated` directly. `pnpm test -- --run`:
  175 files, 1,367 tests passed.
- Linking on this machine requires `DEVELOPER_DIR=/Library/Developer/CommandLineTools` (unaccepted
  Xcode license); used as a per-command override only, per the existing session-log precedent.
- Not run: the complete workspace test suite (`cargo test --workspace`), a packaged Desktop build,
  and no new Vitest file was added for `SessionListView.tsx` (it has no existing test harness to
  extend; the full 1,367-test suite still passed unchanged, and the new behavior is exercised
  end-to-end by the Rust-side regression test plus typecheck/lint on the TS side).

## Risks and follow-ups

- Why CI's `check-acp-schema` gate did not catch the pre-existing schema drift is unexplained and
  left open; if it matters, it deserves its own short investigation rather than a guess here.
- `import_session_file`'s dedup is now atomic with creation, but its `SourceChanged` outcome (a
  changed local file re-imported at the same path) is CLI-only; ACP's `on_import_session` never
  passes a source path, so that variant cannot occur there and is defensively folded into
  `alreadyImported: true` rather than treated as unreachable.
- This pass covered one named, high-value surface in depth rather than the full nine-lens taxonomy
  across the whole repository. The prior report's remaining "Not Reviewed" frontier (full ARC-001..025
  graph, provider adapter contract matrix, Desktop/Electron IPC, CI release/branch-protection
  posture, `summon` delegation, MCP server protocol) is unchanged by this pass.
