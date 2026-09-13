# 2026-09-13 — Consolidated audit repair campaign

## Objective and authority

- Consolidate and reconcile the two audits in commit `829b09054`: the live all-scenarios
  playtest and the independent Gemini architecture/dataflow audit.
- Repair confirmed, in-scope defects using the catalog `repair-defect-campaign` workflow.
- Involvement level: L2 (standard). Authority: governed local repair. No push, PR, merge, or
  other remote mutation is authorized.
- Campaign branch: `codex/consolidated-audit-repair-20260913`, created from clean `main` at
  `829b09054`.

## Governing material

- Repository contract: `AGENTS.md`, with the required documentation read order completed for the
  preceding playtest and confirmed unchanged by the audit-only commit.
- Catalog workflows: `audit-repo-state-reconciliation` followed by `repair-defect-campaign`.
- Binding shared contracts: source readability, architecture-contract drift, record closure,
  involvement, preflight, audit execution, and repair governance.
- Source reports:
  - `docs/cloud/2026-09-13-live-all-scenarios-playtest.md`
  - `docs/cloud/20260913_Gemini_Audit_Data_gosling.md`

## Gate 0 — orientation, safety, and baseline

- Initial worktree: clean. The source commit differs from its parent only by the two audit reports.
- Source editing is gated until the complete candidate inventory and locality-group plan are
  recorded.
- Git policy: one local commit per independently verifiable repair group; historical audit reports
  remain intact; no remote mutation.
- Baseline `source bin/activate-hermit && cargo test -p gosling-cli`: pass. This includes 260 CLI
  unit tests and all `gosling-cli` integration and doc tests.
- Planned validation inventory: targeted Rust and UI regressions per group, `cargo fmt --check`,
  relevant package tests, Desktop typecheck/tests for Desktop changes, full clippy, final diff and
  adversarial review, and documentation structural checks.

## Gate 1 — canonical inventory

The reports contain eighteen distinct IDs. None are exact duplicates. Three pairs share a broad
seam (`GSL-PT-20260913-002`/`AUD-DAT-005`, `GSL-PT-20260913-004`/`AUD-DAT-001`, and
`GSL-PT-20260913-005`/`AUD-DAT-004`) but have different mechanisms and retain their original IDs.

| ID | Priority | Complexity | Evidence and touch set | Validation | Disposition |
|---|---:|---:|---|---|---|
| GSL-PT-20260913-001 | P1 | S | `doctor.rs` intentionally skips the live provider request although CLI help and LC-03/PN-08 describe a working-setup check | CLI doctor integration fixture | repair |
| GSL-PT-20260913-002 | P0 | M | text-only Ctrl-C truncates the submitted turn and adds a local canned assistant line in CLI `session/mod.rs` | CLI unit plus resume/history regression | repair |
| GSL-PT-20260913-003 | P0 | M | cumulative subagent message events can rebroadcast one tool request to multiple active subscribers; CLI renders every notification | subagent notification dedup regression | repair |
| GSL-PT-20260913-004 | P0 | L | `--no-session` creates/deletes a durable hidden session and leaves ordinary full-content request logging enabled, contrary to CX-07 | success/failure marker scan and CLI state tests | repair |
| GSL-PT-20260913-005 | P1 | S | ACP already returns the missing-folder detail in JSON-RPC data; `Hub.tsx` renders only `Error.message` | focused Hub test, Desktop typecheck | repair |
| GSL-PT-20260913-006 | P3 | L | confirmed prior C-9; extension configs record references, not exclusive secret ownership, so blind deletion can remove a shared credential | ownership design required | excluded: intentionally deferred with risk |
| AUD-DAT-001 | P0 | L | the global file and cross-session recall are real, but cross-session recall is an explicit `MEMORY_MANAGER_EVAL.md` contract and no workspace-memory ADR selects replacement semantics | architecture decision and migration tests required | excluded: architecture decision with high privacy risk |
| AUD-DAT-002 | P1 | S | `FileMemorySource` reads from byte zero although its bounded-read contract says newest append-only entries | >8 MiB tail-read unit test | repair |
| AUD-DAT-003 | P2 | S | primary ACP verification already fails closed above 2,000 artifacts; a residual `Err(_) => true` in the core nudge check can still suppress the nudge | research helper regression | repair as low hardening; High/silent-success claim rejected |
| AUD-DAT-004 | P1 | S | closeout filters missing output roots; verifier instead canonicalizes every configured root and aborts | valid-plus-missing root regression | repair |
| AUD-DAT-005 | P1 | S | both truncate methods retain current token counters while context resolution uses `stored.max(estimated)` | atomic truncation/usage tests | repair |
| AUD-DAT-006 | P2 | S | tool dispatch wakes every 100 ms solely to inspect cancellation | cancellation-select regression or explicit static proof | repair; severity reduced to Low |
| AUD-DAT-007 | P0 | M | 40 fresh-start CLI processes across five roots produced three immediate SQLite `database is locked` failures under `/tmp/gsl-aud007.mFxEG4` | repeated multi-process launch plus session tests | repair; promoted to confirmed |
| AUD-DAT-008 | — | — | ADR-0018 deliberately keys history by canonical path across sessions and retains it independently of chat deletion | existing shared-history regression | verified-not-a-defect |
| AUD-DAT-009 | P2 | M | both import paths deserialize all sessions before comparing provenance | duplicate/source-change import tests and query proof | repair |
| AUD-DAT-010 | P3 | L | unused legacy columns are schema maintenance; removing them only from fresh schema would create fresh/upgraded drift and full removal needs a migration decision | fresh/upgraded schema parity | excluded: maintenance feature |
| AUD-DAT-011 | P2 | S | handoff activation performs one metadata update query per covered row inside an atomic transaction | handoff integration tests | repair |
| AUD-DAT-012 | P3 | L | the `fs2` cross-process lock is required by ADR-0001; scheduler impact is unmeasured and moving all service boundaries needs a coherent async design | contention benchmark/design | excluded: unconfirmed architecture/performance work |

The playtest notes (`--version` presentation, invalid-JSON classification, moved-workspace git
diagnostics, and scenario command-list drift) remain notes rather than promoted defects and are not
silently added to this repair scope.

## Gate 2 — locality groups

| Group | Findings | Primary touch set | Planned proof |
|---|---|---|---|
| R1 memory read window | AUD-DAT-002 | `context_mgmt/memory.rs` | focused memory unit tests |
| R2 research fail-closed paths | AUD-DAT-003, AUD-DAT-004 | `session/research.rs`, `acp/server/research_completion.rs` | focused research unit tests |
| R3 chat cancellation and usage | GSL-PT-20260913-002, AUD-DAT-005, AUD-DAT-006 | CLI `session/mod.rs`, `message_storage.rs`, `reply_stream.rs` | CLI, compaction, and session tests |
| R4 subagent notification identity | GSL-PT-20260913-003 | `subagent_handler.rs` and summon routing if required | focused subagent tests |
| R5 stateless CLI retention | GSL-PT-20260913-004 | CLI builder/session lifetime, request-log context | CLI integration and marker scans |
| R6 provider health diagnostics | GSL-PT-20260913-001 | CLI doctor command/tests | loopback provider integration test |
| R7 multi-process session initialization | AUD-DAT-007 | `pool_lifecycle.rs` | repeated external startup race plus session tests |
| R8 storage query efficiency | AUD-DAT-009, AUD-DAT-011 | import lookup and handoff storage | import and handoff tests |
| R9 Desktop error fidelity | GSL-PT-20260913-005 | `Hub.tsx`, `Hub.test.tsx` | focused UI test and typecheck |

Groups are independently reversible and will be committed only after their targeted gate passes.
Large files (`session_manager.rs`, CLI `session/mod.rs`, `reply_stream.rs`) receive narrow changes
only; no mid-campaign modularization. Gate 1 and Gate 2 are complete, so source repair may begin.

## R1 — memory read window

- Gate 3: preserve the existing 8 MiB allocation bound and shared file lock; seek to a
  line-aligned tail window so only complete recent JSONL records are parsed.
- Gate 4: added a greater-than-8-MiB regression. It failed before the implementation change with
  zero recalled records, then passed after the reader was corrected.
- Gate 5: `cargo test -p gosling --lib context_mgmt::memory::tests` — 7 passed.
- Gates 6/7: diff review found no change to cross-session memory scope, ranking, malformed-line
  tolerance, or lock behavior. This group repairs `AUD-DAT-002` only; `AUD-DAT-001` stays routed.
- Gate 8: committed locally as `84e0b58fe` (`fix(memory): read recent bounded entries`).

## R2 — research fail-closed paths

- Gate 3: make artifact-inventory errors fall back to the bounded on-disk deliverable check rather
  than claiming success; give closeout and verification the same available-output-root policy.
- Gate 4: both regressions failed before repair: the artifact-limit case returned `true`, and one
  unavailable secondary output root made an otherwise valid pair fail.
- Gate 5: `cargo test -p gosling --lib research` — 27 passed.
- Gates 6/7: primary ACP verification continues to fail closed on inventory errors; the fallback
  only recognizes a recent, mentioned deliverable under an available output root. Missing all
  output roots still fails. `AUD-DAT-003` closes as Low hardening, not the report's claimed High
  silent-completion path; `AUD-DAT-004` closes as confirmed.
- Gate 8: committed locally as `67f764f4f` (`fix(research): keep deliverable checks fail closed`).

## R3 — chat cancellation and current usage

- Gate 3: keep the submitted user prompt, delete only its incomplete suffix, persist an explicit
  cancellation terminator, reset current-context usage in the same truncation transaction, and
  replace the 100 ms tool polling wake-up with the existing cancellation future.
- Gate 4: the cancellation regression first failed with only the earlier message persisted; the
  usage regression first failed with all five current token counters still populated.
- Gate 5: the two truncation regressions, both interactive interruption regressions, the cancelled
  machine-output regression, and all 37 utility tests passed. `cargo test -p gosling-cli` passed
  all 260 unit tests and every CLI integration/doc test.
- Gates 6/7: partial assistant text is removed without removing user intent; dispatched tool turns
  retain their ledger recovery and now end with the same durable cancellation notice. Accumulated
  usage and cost are unchanged. Static diff review confirms the polling sleep is gone and the
  optional cancellation helper remains pending forever only when no token exists.
- Gate 8: committed locally as `327039c85` (`fix(session): preserve cancelled turns honestly`).

## R4 — subagent notification identity

- Gate 3: attach the subagent's durable tool-request ID to its notification and deduplicate only
  identical `(subagent_id, tool_request_id)` deliveries in the agent's per-batch notification
  stream. Do not suppress arbitrary equal-looking tool calls or deduplicate in a frontend.
- Gate 4: the notification-shape regression failed before repair because no tool-request identity
  was emitted. The rebroadcast regression now proves three deliveries of one identity forward once
  while a distinct request still forwards.
- Gate 5: all 8 subagent-handler tests and all 46 Summon tests passed.
- Gates 6/7: malformed, legacy, and non-subagent notifications remain pass-through. The seen set is
  bounded to one tool batch and keys by both subagent and request, so legitimate repeated commands
  with distinct request IDs remain visible. Fan-out overhead can remain, but duplicate user events
  from that fan-out cannot cross the agent boundary.
- Gate 8: committed locally as `4aab6a257` (`fix(subagent): deduplicate notification identity`).

## R5 — stateless CLI retention

- Gate 3: construct the entire no-session agent graph with a temporary session manager, retain its
  temp directory for exactly the `CliSession` lifetime, and hold a process-wide transcript
  suppression guard so spawned provider/tool work cannot escape the boundary. Disable CLI history
  and project-instruction persistence for that session; remove deletion of a global hidden row.
- Gate 4: the success marker regression first found the exact prompt in both
  `state/logs/llm_request.0.jsonl` and `data/sessions/sessions.db-wal`.
- Gate 5: the seven run-state integration tests passed. Unique prompt markers were absent after a
  successful run, forced provider failure, and live SIGINT cancellation; the successful run's
  unique response marker was also absent. All 260 CLI unit tests and every CLI integration/doc
  test passed, as did all 13 provider-utility tests.
- Gates 6/7: the ordinary agent path remains on the global manager and retains request logging.
  No-session provider construction, extension loading, tool ledgers, cancellation notices, and
  summaries all share the temporary manager. Request JSON and trace input/output/tool arguments
  are suppressed while the guard lives; bounded metadata-only process logs remain allowed.
- Gate 8: committed locally as `9be94b4a9` (`fix(cli): make no-session runs ephemeral`).

## R6 — provider health diagnostics

- Gate 3: keep the bounded doctor report but require the existing provider-configuration probe
  before returning success; report verified only after the first provider event arrives and name
  provider/model in the failure.
- Gate 4: the unreachable-provider regression first failed because doctor exited zero with no
  stderr despite a closed loopback endpoint.
- Gate 5: all 4 doctor integration tests passed, including a real loopback OpenAI-compatible
  stream and a closed endpoint; all 3 doctor unit tests passed.
- Gates 6/7: missing configuration and unknown-provider behavior remain distinct. The implementation
  reuses the same provider/model resolution and live test used by `configure`; no second diagnostic
  protocol or provider-specific shortcut was introduced. This deliberately supersedes the earlier
  REL-GSL-011 configuration-only decision to satisfy the current CLI help and LC-03/PN-08 contract.
- Gate 8: committed locally as `249b08e49` (`fix(doctor): verify the configured provider`).

## R7 — multi-process session initialization

- Gate 3: retain the process-local first-init mutex and add an owner-only advisory file lock held
  across WAL setup, schema creation/migration, and legacy-import marking. Acquire the potentially
  blocking OS lock through `spawn_blocking`.
- Gate 4: the pre-fix live reproducer ran 40 fresh-start processes across five roots and observed
  three immediate SQLite `database is locked` failures.
- Gate 5: both pool-lifecycle tests passed, including the `0600` lock-file regression; a built CLI
  then completed 80 concurrent launches across ten fresh roots with zero failures. Evidence root:
  `/tmp/gsl-aud007-fixed.R4pGmb`.
- Gates 6/7: the lazy SQL pool still opens only after the lock is held; every success/error path
  releases the OS lock by dropping the file guard. Existing `busy_timeout`, WAL, migration, and
  retryable legacy-import semantics remain unchanged.
- Gate 8: committed locally as `4acf5d5ce` (`fix(session): serialize database initialization`).

## R8 — storage query efficiency

- Gate 3: query import provenance inside SQLite and materialize only a matching session; retain
  handoff metadata validation but replace the per-row serialization/update loop with one scoped
  `json_set` update.
- Gate 4: the import regression first failed because an unrelated malformed session row was
  deserialized by the full-table scan. The handoff regression first failed because round-tripping
  each metadata object through the current Rust type discarded a forward-compatible field.
- Gate 5: both new regressions passed, as did the existing file-import idempotence regression, all
  14 session-handoff tests, and all 6 ACP handoff tests.
- Gates 6/7: fingerprint lookup still precedes canonical-path lookup, malformed provenance JSON is
  ignored as before, and only the matching session is loaded. Handoff validation still occurs
  before any metadata write in the same immediate transaction; the bulk update is bounded by both
  session ID and covered row ID and preserves unknown JSON fields.
- Gate 8: committed locally as `4cada69ce` (`fix(session): streamline import and handoff
  storage`).

## R9 — Desktop error fidelity

- Gate 3: route new-session and incomplete-session-cleanup failures through the existing ACP error
  renderer so JSON-RPC `data` reaches the inline recovery message.
- Gate 4: the new Hub regression first rendered the structured failure as `[object Object]` and
  omitted the missing-folder/relink detail.
- Gate 5: all 16 Hub tests passed and `pnpm run typecheck` completed successfully.
- Gates 6/7: ordinary `Error` messages retain their existing text, while direct or nested JSON-RPC
  failures gain their structured detail through the already-tested shared helper. No generated API
  types or transport behavior changed.
- Gate 8: committed locally as `109a66ab7` (`fix(desktop): preserve session error details`).

## R10 — integrated doctor-test alignment

- Integrated CLI validation found that the older config-value integration test still invoked
  `doctor` without a reachable provider and therefore encoded the superseded configuration-only
  contract.
- The test now supplies a deterministic loopback OpenAI-compatible stream, so it continues to
  test runtime configuration warnings while satisfying the live provider-health contract.
- Both config-validation cases passed, followed by all 260 CLI unit tests and every CLI
  integration/doc test.
- Committed locally as `b0f387e1c` (`test(cli): keep doctor validation deterministic`).

## Gate 9 — integrated regression and closure

- `cargo fmt --all -- --check` — pass.
- `cargo clippy --all-targets -- -D warnings` — pass.
- `cargo test -p gosling-cli` — pass: 260 unit tests and all integration/doc tests.
- `cargo test -p gosling` with five documented baseline failures and four suite-order keychain
  cases filtered — pass for every remaining target. The library result was 1,888 passed, 3
  ignored, and 8 filtered; all remaining integration, binary, and doc tests passed.
- The four keychain cases passed immediately in isolation: three
  `test_agent_inherits_session_mode` variants and `test_session_mode_isolation`. In the unfiltered
  aggregate run, earlier extension tests mutate process-global secret-source state and these later
  tests wait in macOS `SecKeychainFindGenericPassword`; this is test isolation, not a campaign
  regression.
- The five baseline failures remain unchanged from the independently verified 2026-09-12 baseline:
  three prompt-manager snapshots plus `prompt_template::tests::test_get_template` incorporate the
  operator's customized prompt, and
  `test_custom_preferences_validate_resulting_compaction_pair` accepts a configuration pair the
  test expects to reject. Generated `.snap.new` files were removed without accepting them.
- 593 tests passed across `gosling-mcp`, `gosling-providers`, `gosling-sdk-types`, and provider
  integration suites; the other selected support, SDK, and macro targets had no runnable tests.
- A literal `cargo test --workspace` remains blocked by the previously recorded
  `gosling-test-support` compile failure: the active OpenTelemetry feature set omits metrics and
  global provider APIs used by `otel.rs`.
- Desktop validation passed: 167 files / 1,321 tests and `pnpm run typecheck`.
- Live repair evidence passed: no-session success/failure/SIGINT marker scans, loopback/closed
  doctor probes, and 80 concurrent process launches across 10 fresh roots with zero SQLite setup
  failures after a 3-of-40 pre-repair reproduction.

## Record closure

- Canonical report: `docs/cloud/2026-09-13-consolidated-audit-repair.md`.
- Both source audits retain their original findings and have dated disposition addenda.
- `docs/INDEX.md`, `docs/TODO.md`, and `docs/polish/active-todo-ledger.md` record the canonical
  report and four remaining decisions.
- The complete 127-card live playtest and signed installed Desktop were not rerun after repair.
  Validation is therefore source-complete for the repaired paths, with those live-product limits
  stated explicitly.
- No push, PR, merge, release, or other remote mutation was performed.

## R11 — adversarial-review evidence closure

- The required independent adversarial review found no production-code blocker and accepted the
  four routed dispositions and validation caveats. It did identify one evidence overclaim: the R4
  regression exercised the identity helper but not three inputs through the actual multi-stream
  fan-in primitive.
- Added a regression in `reply_stream.rs` that sends the same subagent request notification through
  three `ToolStreamItem::Message` streams combined with `select_all`, applies the production
  batch-deduplication path, and proves exactly one notification is forwarded.
- The new fan-in regression and all 8 subagent-handler tests passed.
- Committed locally as `a9d1f0322` (`test(subagent): cover multi-stream notification fan-in`).
- The reviewer confirmed that this closes its only blocker; no other code, routing, validation, or
  governance issue was reported.
- Final Rust format, full Clippy, and `git diff --check` passed after the evidence commit.
- Documentation diff review and local-link checks passed. The required AGENTS governance marker is
  present; `GEMINI.md` does not exist in this repository, so there was no Gemini marker to check.
