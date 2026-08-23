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

Pending final baseline-equivalent rerun and MOD-V01..10 evidence capture.
