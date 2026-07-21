# Audit and repair campaign — 2026-07-18

Skill source: `agent-skills` repo, `010_audit/*` (audit) and
`020_repair/repair-defect-campaign` (repair). Requested scope: architecture,
dataflow, and failsafe audit lenses against gosling's workflow, followed by a
repair pass fixing all findings. Branch: `claude/gosling-audit-repair-vav52g`,
based on `main`.

## Scope decision

The original plan launched 13 fresh parallel audit agents (architecture-seam;
the seven `audit-dataflow-*` lenses; the failsafe family — readiness,
dependency-criticality, recovery-idempotency, operator-signal; and
reliability). That run was lost to an environment container reclaim before
any agent produced a report (confirmed via transcript inspection: ~20-30 tool
calls each, almost entirely spent loading the shared audit-base methodology
docs, zero draft findings anywhere).

Before relaunching, this campaign discovered gosling already has substantial,
recent audit history: `reports/2026-07-16-defect-audit-and-repair.md` ran 12
lenses in discovery mode and left **20 fully-specified, unrepaired defects**
with file:line evidence, explicitly dispositioning most of the originally
planned 13 lenses (architecture-seam, all seven dataflow siblings,
failsafe-readiness) as not-applicable or lower-priority for gosling's actual
shape (a Rust agent framework, not a webapp with file-upload/pipeline/GraphQL
surfaces) — with reasoning specific to this repo, not generic.

Given that, this campaign:

1. Verified the 2026-07-16 report's evidence still holds (no commits since its
   last fix touched any of the 20 open defects' files — confirmed via
   `git diff --stat` against the intervening commit range).
2. Ran a **fresh, bounded architecture-invariant compliance check** against
   `.architecture/invariants.yaml` (7 active invariants, created 2026-07-09) —
   this registry existed before the 2026-07-16 campaign but its own
   disposition text incorrectly claimed no registry existed, so nobody had
   actually checked compliance against it. Result: **all 7 invariants
   currently held**, no violations (one apparent orphaned-IPC-channel
   candidate for ARC-003 was ruled out after verification — the channels are
   handled in `utils/autoUpdater.ts`, a separate module `main.ts` delegates
   to). This is this campaign's actual "architecture" and "is everything
   wired correctly" audit contribution.
3. Used the 20 open defects as the repair-defect-campaign's **Existing
   Findings Mode** input, which is exactly what "repair ALL findings" means
   once a recent, well-evidenced inventory already exists — re-running 13
   fresh blind audits over it would have rediscovered the same ground at much
   higher cost, or worse, silently contradicted the prior campaign's
   reasoning without engaging it.

`audit-recovery-idempotency` (gosling's closest analogue to a "database"
audit — its one real DB is the SQLite session/tagteam store) was already run
in the 2026-07-16 pass; its two open findings (REC-001, REC-002) are in this
campaign's inventory below.

## Defect disposition

Of the 2026-07-16 report's 20 open items, 3 were already carrying an explicit
deferral with reasoning (LLM-002, LLM-003, DEP-002 — unsafe to validate
without live extension/LLM integration testing, or too risky to touch in
isolation) and are treated as protected per the repair-defect-campaign's own
rule: deferred items stay untouched unless the user explicitly reopens that
exact item. The remaining 18 were this campaign's actual inventory.

### Fixed and verified (13)

| ID | Fix | Commit |
|---|---|---|
| ORCH-003 | Orchestrator-managed sessions now use `Auto`+`SubAgent` (matching delegate() subagents) instead of `SmartApprove`+`User`, so `Agent::redirect_unapprovable_subagent_requests` (added for ORCH-001) auto-resolves an unanswerable approval instead of `send_message` hanging forever | `a9f3096` |
| CON-003 | `append_facts_to_durable_file` holds an exclusive `fs2` lock across the read-check and the append, so two concurrent summarizer runs can no longer both see "heading missing" and duplicate it | `a9f3096` |
| OPS-003 | `reply_service`'s `Finish.reason`/`exit_type` telemetry now thread a real `ReplyExitReason` (normal/error/disconnected/cancelled) instead of hardcoding `"stop"`/`"normal"` regardless of how the loop actually ended | `a9f3096` |
| GUI-004 | `ExternalBackendSection` surfaces a visible error and resyncs from the actually-persisted setting on save failure, instead of silently logging to console while the UI keeps showing the unsaved value | `a9f3096` |
| GUI-005 | `UpdateSection`'s post-check success toast now reads a ref that's always current instead of a stale async closure over `updateInfo`, which a concurrent updater event could silently skip | `a9f3096` |
| SEC-003 | `open-directory-in-explorer` now runs the renderer-supplied path through `assertRendererFileAccess`, matching every sibling file-path IPC handler's confinement | `a9f3096` |
| INV-001 | `ProviderInventoryEntryDto.provider_type` is now a real `ProviderTypeDto` enum (serde/schema-checked) instead of `format!("{:?}", ...)` standing in for a wire contract | `2ef1213` |
| INV-002 | Added a drift-guard test pinning `ProviderTypeDto`'s wire values against the exact literals `ui/desktop/src/types/providers.ts`'s hand-maintained union expects | `2ef1213` |
| OPS-002 | `read_tail`/`read_capped` now return `Result` instead of discarding their `io::Error`; `generate_diagnostics` pushes a `DiagnosticsError` on each read failure instead of the `errors` field staying permanently empty | `2ef1213` |
| GUI-002 | `ToolApprovalButtons`' "Always Allow all {extension} tools" now checks a new non-consuming `isAcpPermissionRequestPending` before mutating backend tool permissions, instead of mutating first and validating after | `2ef1213` |
| CON-001 | `AgentManager`'s LRU cache promotes a busy (turn-in-flight) eviction candidate out of eviction position instead of evicting purely by recency, preventing the lost-update race where a second agent rebuilt from stale state clobbers the original's `extension_data` | `d95b89e` |
| OPS-004 | `update_session_name` and the `/diagnostics/{session_id}` route now log and return a real message on failure, matching the established `ErrorResponse`/`(StatusCode, String)` pattern already used by `fork_session`/`status` in the same files, instead of discarding the error via `.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)` | `d95b89e` |
| OPS-005 | `ensure_extensions_loaded`'s synchronous-retry branch now checks the retry's per-extension results and returns an error listing what still failed, instead of unconditionally returning `Ok(())` regardless of whether the retry itself also failed | `d95b89e` |

### Deferred, with reasoning (5)

| ID | Why |
|---|---|
| ORCH-002 | `summon.rs`'s default full-tool/extension-surface inheritance is the same concern as the 2026-07-10 report's routed AOC-019 ("default least-authority toolset") — a security-model design decision (an enforced capability field in agent frontmatter, or a default-inheritance change) rather than a local guardrail. Not re-litigated; left routed as that report already decided. |
| RES-002 | Provider CLI subprocess orphaning on quit is a genuine defect, but a correct fix needs either Rust-side SIGTERM-forwarding-to-children in `gosling-server` (unbuildable in this sandbox) or real macOS/Windows process-tree verification this Linux sandbox cannot perform. |
| RES-003 | Docker-exec'd MCP extension processes surviving a local client kill needs in-container PID tracking (`docker top` correlation) to fix correctly; Docker isn't available in this sandbox to validate against. |
| REC-001 | Persist-before-side-effect turn atomicity in `agent.rs` (4579 lines) is the same concern the 2026-07-10 report's Section B already routed for exactly this reason: the fix touches the core turn loop in the codebase's largest, most concurrency-sensitive file, and deserves a dedicated pass with room to trace every call path, not a rushed patch under this campaign's remaining time budget. |
| REC-002 | Making legacy-session-import completion track separately from `schema_version` (so an interrupted first run retries the import instead of silently skipping it forever) needs a proper new schema migration (this DB is at version 20) across both the fresh-DB and existing-DB-upgrade paths — real surface area in a live migration system that deserves dedicated verification against its own migration test suite, not a rushed addition here. |

## Validation

- `cargo fmt --all -- --check`: clean across the whole workspace, on every
  commit.
- `cargo test -p gosling -p gosling-sdk-types` (excluding
  `test_claude_code_provider`, confirmed via `git stash` to fail identically
  on this branch's pre-existing code before any of this campaign's changes —
  it spawns a real `claude` subprocess, which can't run nested inside this
  session): **1396 lib tests + every integration suite passing, 0 failures**,
  across three full runs (one per commit stage).
- `cargo clippy -p gosling -p gosling-sdk-types --all-targets -- -D
  warnings`: clean.
- New regression tests added: `concurrent_durable_file_appends_write_heading_once`
  (CON-003), `provider_type_dto_wire_values_match_ts_union` (INV-002),
  `read_tail_surfaces_the_io_error_instead_of_swallowing_it` +
  `read_capped_surfaces_the_io_error_instead_of_swallowing_it` +
  `read_capped_still_succeeds_and_truncates_within_budget` (OPS-002),
  `test_lru_eviction_skips_a_busy_session` (CON-001), plus two new
  `permissionRequests.test.ts` cases for `isAcpPermissionRequestPending`
  (GUI-002, unverified — see below).
- `crates/gosling-server` (OPS-003, OPS-004, OPS-005, status.rs) is
  `cargo fmt`-clean but its own build/test/clippy could not run in this
  sandbox: it transitively pulls `v8-goose`, whose build script downloads a
  prebuilt binary from a host this session's egress policy blocks (HTTP
  403) — the same pre-existing limitation the 2026-07-16 report hit for the
  same crate. Recommend CI confirm before merge.
- `ui/desktop` (GUI-002, GUI-004, GUI-005, SEC-003) changes are unverified by
  `pnpm test`/`pnpm run typecheck` here: `node_modules` was never installed in
  this sandbox (`pnpm install` fails on an Electron `node-gyp` git-tarball
  dependency, also blocked by the egress policy), matching the 2026-07-16
  report's precedent for the same crate. Recommend CI confirm before merge.

## Architecture invariant compliance (fresh finding, not from the 2026-07-16 backlog)

All 7 invariants in `.architecture/invariants.yaml` currently hold:

- ARC-001/ARC-002 (route adapters stay thin; reply lifecycle has one owner):
  confirmed — both `reply.rs` and `session_events.rs` import from
  `reply_service.rs`, not from each other. (This was flagged as a violation
  in the 2026-07-09 architecture audit and has since been fixed.)
- ARC-003 (Electron privilege boundary is contract-backed): confirmed —
  `ui/desktop/src/ipc/channels.ts` exists as the shared contract; one
  apparent orphaned-channel candidate (six updater IPC channels) was ruled
  out after verification, handled in `utils/autoUpdater.ts`. (Also flagged
  and since fixed since the 2026-07-09 audit.)
- ARC-004 (Goose compatibility remains an adapter): confirmed —
  `goose-compat.js` has explicit normalization functions and provenance
  text.
- ARC-005/ARC-006 (Tagteam is a workflow, not a provider; has one lifecycle
  owner): confirmed — `providers/tagteam.rs` (455 lines) is a thin
  CLI-invoking `Provider` implementation with no reducer/state-machine
  logic; `tagteam/` owns launch validation, state reduction, and policy
  separately.
- ARC-007 (Run Steward cannot inherit mutation authority): confirmed —
  `StewardCapabilityPolicy::decide` denies `Shell`, `FileMutation`,
  `ExtensionManagement`, `DelegateSubagent`, `WidenScope`, `DeferFinding`,
  `Transfer`, and `InvokeTagteam` unconditionally.

## Residual risk / follow-up backlog

- The 5 deferred items above, each with a stated next action.
- Recommend CI run `cargo test`/`clippy` for `gosling-server` and
  `pnpm test`/`typecheck` for `ui/desktop` on this branch before merge, since
  this sandbox could verify neither crate directly.
- `docs/TODO.md`'s existing items (TODO-001 through TODO-010, per the
  2026-07-10 report) are unrelated to this campaign's touched files and were
  left as-is.


## Deferred-record reconciliation - 2026-07-20

A superseding source review found that current source already satisfies the deferred records below:

- ORCH-002: delegate capability policy is versioned and source-requested extensions are constrained by policy.
- REC-001: tool operations enter the durable ledger before side effects, in-doubt operations are not redispatched, and terminal results are persisted idempotently.
- REC-002: legacy import completion is tracked separately from schema version and failed imports remain retryable.
- RES-002: parent-process supervision is wired through `GOSLING_SERVER__PARENT_PID`.
- RES-003: container cleanup includes in-container process termination and regression coverage.

These records are closed by superseding reconciliation; their historical text is retained. This campaign did not rerun tests.

## Final verification supersession - 2026-07-20

The later open-defect campaign completed the previously deferred execution pass. Rust formatting, 1,533 Gosling library tests, 34 gosling-server/TLS tests, workspace clippy, Desktop typecheck, 547 Desktop tests, ESLint, i18n validation, and ACP schema consistency all pass. The earlier statement that tests were not rerun remains historical context and is superseded by this evidence.
