# Modularize `summon.rs` — gated run log

Skill: `repair-source-modularization` (agent-skills `020_repair`).
Branch: `repair/modularize-summon-2026-08-23`.
Routed by: user request plus `docs/TODO.md:466-473` (ARC-GSL-001 follow-up).

## Gate 0 — Orientation

- `git status --short`: clean. Branch created from `main` at `502d79825`.
- Required repo instructions and documentation read: `AGENTS.md`, `README.md`,
  `docs/INDEX.md`, `docs/architecture.md`, `docs/TODO.md`, `.giles/repo.yaml`,
  and the prior modularization log. `GEMINI.md` is absent.
- The private catalog matched `repair-source-modularization`; its complete
  instructions, hazards, contracts, plan/ledger templates, and verification
  protocol were loaded before edits.
- Process snapshot: no repo dev server, Cargo, Rust compiler, or test process was
  running. A pre-existing packaged `/Applications/Gosling.app` instance and its
  `gosling serve` child on port 62477 are unrelated and must remain untouched.

## Gate 1 — Target lock and whole-file read

Discovery considered the five remaining production files routed in
`docs/TODO.md:466-473`. Selected
`crates/gosling/src/agents/platform_extensions/summon.rs` (2,772 lines): it is
the shortest candidate and has the smallest focused symbol surface among the
remaining Rust candidates. Rejected for this run:

- `crates/gosling/src/acp/server.rs` — 4,981 lines and broader protocol surface.
- `crates/gosling/src/agents/agent.rs` — 5,369 lines and central loop surface.
- `crates/gosling/src/agents/extension_manager.rs` — 4,344 lines and broader
  extension lifecycle surface.
- `ui/desktop/src/main.ts` — 3,488 lines and order-sensitive Electron lifecycle.

The entire target was read in overlapping chunks, lines 1-2,772, before this
plan or any source edit. Its production inventory is 30 top-level symbols:

- retained facade constants/types: `EXTENSION_NAME`,
  `SUBAGENT_DESCRIPTION_BUDGET`, `TASK_LABEL_BUDGET`, `DelegateParams`, and
  `SummonClient`;
- source discovery/policy: `kind_plural`, `AgentMetadata`,
  `DelegateCapabilityPolicy`, `DelegateSpec`, `validate_capability_policy`,
  `resolve_delegate_extensions`, `delegate_authority_summary`,
  `parse_agent_content`, `scan_agents_from_dir`,
  `discover_filesystem_sources`, `build_instructions_with_context`, and
  `build_subagent_instructions`;
- task tracking: `BackgroundTask`, `CompletedTask`, `TaskLoadResult`,
  `round_duration`, `current_epoch_millis`, `max_background_tasks`,
  `completed_task_ttl`, `is_session_id`, and `Drop for SummonClient`;
- client behavior: the inherent `SummonClient` implementation and
  `McpClientTrait` implementation;
- path policy: `resolve_working_dir`;
- the unchanged inline `tests` module.

Public compatibility names found by repo-wide search are `EXTENSION_NAME`,
`SummonClient`, and `discover_filesystem_sources`; the already-public
`DelegateParams`, `BackgroundTask`, and `CompletedTask` paths are also preserved
with facade re-exports. Direct callers are the platform-extension registry and
ACP agent-mention discovery. Second-level workflows are platform extension
initialization/MCP dispatch and ACP mention completion.

No serialization type-path, logger namespace, reflection, string registry, or
import-time side-effect hazard was found. Rust field privacy requires extracted
modules to remain children of `summon`; split inherent implementations are
therefore behavior-neutral and compiler-checked. Private helpers used across
child seams or by the existing sibling test module will use `pub(super)` only,
which does not widen the public facade.

## Gate 2 — Baseline

One exact pre-edit command chain completed with exit 0:

```text
source bin/activate-hermit
cargo fmt --check -p gosling
cargo build -p gosling
cargo test -p gosling --lib agents::platform_extensions::summon::tests
  36 passed; 0 failed; 0 ignored; 1,660 filtered out
cargo test -p gosling --lib
  1,696 passed; 0 failed; 0 ignored
cargo clippy -p gosling --all-targets -- -D warnings
  clean, exit 0
```

No pre-existing validation failures.

## Gate 2 — Extraction plan

The original module remains a **compatibility facade**. Every moved body is
deleted from the facade in the same edit that installs its child implementation;
Rust rejects duplicate inherent methods, so copy/delegate/delete is compiler
enforced rather than wrapper based.

1. `summon/source_discovery.rs` — agent metadata parsing, source discovery,
   capability resolution, and instruction composition. Facade re-exports
   `discover_filesystem_sources`.
2. `summon/task_tracking.rs` — background/completed task state, slot and
   notification lifecycle, task-result collection, cleanup, and time labels.
   Facade re-exports `BackgroundTask` and `CompletedTask`.
3. `summon/loading.rs` — load tool schema, source cache/lookup, discovery output,
   and source loading.
4. `summon/delegation.rs` — delegate tool schema, validation, source/ad-hoc spec
   construction, and foreground delegation.
5. `summon/delegate_config.rs` — task/provider/model/turn/working-directory
   configuration.
6. `summon/async_delegation.rs` — background delegation startup and bookkeeping.
7. `summon/mcp.rs` — the `McpClientTrait` dispatch/subscription/status adapter.

Per seam: inspect direct and second-level callers, move bodies verbatim, run
format check + build + 36 targeted tests, inspect diff/stat/symbol ownership,
then checkpoint. Full baseline-equivalent validation and MOD-V01..10 follow the
last seam. The existing test module remains unchanged; it directly exercises
the extracted responsibilities through the original `summon` facade.

## Bug ledger (MOD-B)

No suspected defects observed so far. Any defect noticed while moving code will
be recorded and routed without changing behavior.

## Gates 3-4 — Seam checkpoints and connection checks

Every seam was moved into a child of `summon`, formatted, built, exercised by
the unchanged 36-test Summon suite, statically checked for a single symbol
owner, and committed as a rollback point:

| Seam | Module | Checkpoint | Direct / second-level connections checked |
|---|---|---|---|
| Source discovery and policy | `source_discovery.rs` | `c0a33b04b` | load cache, MCP instructions, delegate agent parsing; ACP mention completion and server slash-command discovery via the facade |
| Task tracking | `task_tracking.rs` | `d8d6daeb6` | load result collection, async task registration, MCP status; load/delegate tool lifecycle |
| Loading | `loading.rs` | `21f461409` | MCP `load`, delegate source resolution; platform-extension tool dispatch |
| Delegation | `delegation.rs` | `205cf74a8` | MCP `delegate`, async path, task config; platform-extension registry/client construction |
| Delegate configuration | `delegate_config.rs` | `7708b96ff` | foreground/background delegate execution; provider/model/session configuration |
| Async delegation | `async_delegation.rs` | `dd5143a4b` | delegate async branch and task tracking; MCP load/status collection |
| MCP adapter | `mcp.rs` | `674331e9e` | load/delegate implementations and client state; `PLATFORM_EXTENSIONS` registration and extension-manager workflows |

The Rust compiler exposed two child-privacy connections during extraction:
`kind_plural`/`AgentMetadata` and `resolve_source`. They were given
`pub(super)` visibility only, preserving the public API. One test-only import
was gated with `#[cfg(test)]`. These were structural wiring findings, not
behavioral defects and not MOD-B entries.

The external facade callers were inspected directly:

- `agents/platform_extensions/mod.rs` constructs `SummonClient` and reads
  `EXTENSION_NAME`; second level is platform-extension loading and MCP dispatch.
- `acp/server/agent_mentions.rs` calls `discover_filesystem_sources`; second
  level is ACP custom request dispatch.
- `gosling-server/routes/config_management.rs` calls the same discovery facade;
  second level is the HTTP slash-command route.

## Gate 5 — Intermediary audit

| Seam | Finding | Disposition | Complexity | Cost | Nominal agent |
|---|---|---|---|---|---|
| Source discovery | No duplicate owner, caller break, or behavior drift | proceed | medium | low | primary Rust agent |
| Task tracking | Spawn/reap/cancel state remains connected at one module altitude | proceed | high | medium | primary Rust agent |
| Loading | Cache, discovery, and task-result paths remain covered | proceed | medium | low | primary Rust agent |
| Delegation | Trust policy, no-nesting rule, and foreground run unchanged | proceed | high | medium | primary Rust agent |
| Delegate config | Provider/model/turn/path policies unchanged | proceed | medium | low | primary Rust agent |
| Async delegation | Spawn and registration moved together; cleanup remains reachable | proceed | medium | low | primary Rust agent |
| MCP adapter | Trait dispatch and facade registration compile unchanged | proceed | medium | low | primary Rust agent |

No MOD-B suspects and no rollback triggers surfaced.

## Gate 6 — Tests, docs, and ledgers

- Existing Summon behavior tests were not weakened, skipped, deleted, or moved.
- Added `test_original_module_path_compatibility_facade` to compile and assert
  the original public paths (`EXTENSION_NAME`, `SummonClient`,
  `DelegateParams`, `BackgroundTask`, `CompletedTask`, and
  `discover_filesystem_sources`). The targeted suite is now 37 tests.
- `cargo check -p gosling-server` passes, covering the separate crate caller of
  the discovery facade.
- Updated `docs/TODO.md` with the completed seven-module map and removed
  `summon.rs` from the remaining routed list.
- Active user docs contain no `summon.rs:<line>` references. Forty-five such
  references exist in immutable point-in-time audit/session reports; their
  line citations already targeted historical snapshots and were not rewritten
  as if they described the current tree. `documentation/static/servers.json`
  links to the original facade path, which remains valid.

## Gate 7 — Verification sweep

Final status: `completed_verified`.

### Symbol parity and line reconciliation

- Original inventory: 30 production top-level symbols plus the inline test
  module's nested fixtures/tests.
- Retained in the facade: the three constants, `DelegateParams`,
  `SummonClient`, its public `new` constructor, and the unchanged test module.
- Moved/split: every other type, helper, trait implementation, and inherent
  method body is accounted for in exactly one of the seven child modules.
- Removed as duplicate: zero. Unaccounted symbols: zero.
- Original: 2,772 lines. Final facade: 1,090 lines. Facade plus child modules:
  2,866 lines, a +94 delta consisting of seven four-line module headers,
  module declarations/import/re-export wiring, parent-only visibility and
  rustfmt line expansion, and the 19-line facade compatibility test.

### MOD-V01..10 coverage

| Code | Check | Status | Evidence |
|---|---|---|---|
| MOD-V01 | Regression vs baseline | pass | Exact baseline chain rerun: format check and build exit 0; focused suite 37/37 (36 baseline tests plus one compatibility test); full library suite 1,697/1,697; all-target Clippy with `-D warnings` clean. No prior passing test regressed. |
| MOD-V02 | Broken source references | pass | Repo-wide facade search finds the platform registry plus ACP and server discovery callers unchanged; every moved implementation symbol has exactly one child-module owner; `cargo check -p gosling-server` passes. |
| MOD-V03 | Broken doc links | pass | Active README/index/architecture/TODO/product docs have no `summon.rs:<line>` references. The static server link still resolves to the compatibility facade. Historical audit reports retain snapshot-specific citations. |
| MOD-V04 | Orphaned modules/symbols | pass | Seven `mod` declarations match seven child files; compiler and ownership grep confirm all are reachable and every planned symbol is owned once. |
| MOD-V05 | Orphaned processes | pass | Post-validation snapshot contains no Cargo, rustc, test runner, watcher, or repo dev server. The same pre-existing packaged Gosling processes and port 62477 remain untouched. Spawn and task cancel/reap code remain connected through `SummonClient` state. |
| MOD-V06 | Redundancy | pass | Distinctive-body searches for capability warning, cancellation timeout, load discovery, delegate schema text, and async authority output each return one owner; no facade implementation copies remain. |
| MOD-V07 | Module identity | n/a | No serialized Rust type paths, module-derived logger names, reflection, registry strings, or import-order registrations apply. Serde field/wire shapes and public Rust paths remain unchanged. |
| MOD-V08 | Doc/comment freshness | pass | Every child has responsibility, exact extraction provenance, and facade/export notes; facade contains the literal `compatibility facade`; TODO and this log describe the new map. |
| MOD-V09 | Import graph health | pass | All child modules are descendants of the facade and depend through `super`; format/build/tests/Clippy compile the full graph with no cycles or lazy-import workarounds. |
| MOD-V10 | Test integrity | pass | No existing test/assertion was changed, removed, skipped, or weakened. Added one original-module-path test. Existing tests directly execute the moved inherent methods and free functions; Rust inherent-method identity has no separate child import surface. |

### Adversarial walkthrough

- Source path: repo/global agent discovery → frontmatter parse → untrusted
  capability policy suppression → load or delegate spec creation.
- Foreground path: MCP dispatch → validation/no-nesting check → provider/model/
  working-directory resolution → subagent run → result/error mapping.
- Background path: MCP dispatch → atomic slot reservation → spawned task and
  notification bridge → peek/cancel/wait → completed-map retention/TTL → MOIM.
- Compatibility paths: platform registry construction, ACP agent mentions, and
  server slash-command discovery all enter through the original module path.
- Error paths exercised by existing tests include missing sources, invalid turn
  limits, unavailable/out-of-policy extensions, directory traversal/file/
  missing paths, nonexistent tasks, task cancellation, and peek semantics.
- No persistence or serialized module identity exists in this component. A live
  provider delegation was not run because it would add external cost/network
  state without increasing confidence beyond the deterministic unit and compile
  coverage for a body-preserving move.

### Decisions, residual risk, and follow-up

- Kept the original module as a facade because three independent workflows and
  public downstream crate code use it.
- Used child modules so Rust privacy protects `SummonClient` state while
  `pub(super)` exposes only the cross-seam edges required inside the facade.
- Kept all behavior tests in the established inline module; they execute the
  actual split inherent implementations, while the added test pins facade paths.
- MOD-B ledger count: zero. No suspected defect was changed or routed.
- Residual risk is limited to a live provider call not being performed; public
  compile coverage, full deterministic tests, and unchanged bodies bound it.
- Remaining one-file-per-run candidates are `acp/server.rs`, `agents/agent.rs`,
  `agents/extension_manager.rs`, and `ui/desktop/src/main.ts`.
