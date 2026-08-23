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

## Gate 3-7 evidence

Pending extraction.
