# Modularize `extension_manager.rs` — gated run log

Skill: `repair-source-modularization` (private agent-skills catalog).
Branch: `codex/modularize-extension-manager-2026-09-01`.
Routed by: `docs/TODO.md` / `MOD-GSL-001`.

## Gate 0 — Orientation

- Target repository: `/Users/eric/Work/vscode/forked/gosling`.
- Starting state: clean branch based on the completed Desktop-main modularization.
- Involvement: L2 standard; authority is governed, behavior-preserving source repair.
- Required sources read: `AGENTS.md`; root `GEMINI.md` is absent; `README.md`;
  `docs/INDEX.md`; relevant architecture, ADR, Goose-compatibility, active-ledger,
  advisory Giles, and prior modularization records.
- Architecture baseline: extension lifecycle and transport remain backend-owned.
  Accepted ADR-0012 requires operator domain adapters to reuse the existing stdio
  child-process supervision path. Public module paths and transport behavior must
  remain unchanged.
- Process baseline: no repository build/test process was running. Pre-existing
  Node PID 1054 on loopback port 8888 and installed Gosling PID 16232 on ports
  53273/53275 are outside this run and remain untouched.
- Execution shape: one original production source file, sequential source edits,
  one validated commit per seam. Independent reads/searches are batched.

## Gate 1 — Candidate inventory and target lock

| Candidate | Lines | Disposition this run |
|---|---:|---|
| `crates/gosling/src/agents/extension_manager.rs` | 4,531 | **selected**: shortest remaining routed file |
| `crates/gosling/src/acp/server.rs` | 5,136 | excluded by the one-file rule |
| `crates/gosling/src/agents/agent.rs` | 5,521 | excluded by the one-file rule |

The entire target was read before planning, in overlapping bounded chunks covering
lines 1–4,531. Direct callers were inventoried through `crates/gosling/src` and
`crates/gosling/tests`. Public names used outside the file are
`ExtensionManager`, `ExtensionManagerCapabilities`,
`TRUSTED_TOOL_UPDATE_META_KEY`, `connect_operator_stdio_client`,
`OperatorStdioClient`, `OperatorProcessExit`, `get_parameter_names`,
`get_tool_owner`, `is_first_class_extension`, `is_hidden_extension`,
`merge_environments`, and `substitute_env_vars`.

### Responsibility inventory

- paginated MCP discovery and pagination guards;
- action-required stream registration cleanup;
- extension state, manager state, lifecycle, and cache invalidation;
- child-process environment hardening, Docker env files, and stdio clients;
- operator stdio supervision and bounded line decoding;
- OAuth/static-client resolution and streamable HTTP/unix-socket transports;
- tool metadata, ownership, MCP App attachment trust, and visibility;
- tool catalog caching/filtering, resource/prompt discovery, tool dispatch;
- extension discovery, planning prompt, and platform MOIM aggregation;
- two inline test modules containing 37 focused unit tests.

### Bug ledger (MOD-B)

#### BUG-001 — Tool discovery cache is not keyed by session

- Code: MOD-B09 (state/cache scope mismatch).
- Original location: `extension_manager.rs:2108-2128`.
- Evidence: `get_all_tools_cached(session_id)` calls session-aware
  `fetch_all_tools(session_id)`, but stores the result in one
  `Mutex<Option<Arc<Vec<Tool>>>>` shared by every session.
- Why it looks wrong: `McpClientTrait::list_tools` receives a session ID, so a
  provider may expose session-specific tools; the first successful lookup can be
  returned to later sessions without querying their catalog.
- Why it might be intentional: most current extensions expose a stable catalog,
  and lifecycle changes invalidate the cache globally.
- Severity if real: medium; confidence: high; fix complexity: small/medium.
- Routed to: a later correctness/concurrency repair. Extraction impact: none;
  the cache behavior and field shape move unchanged in this run.

## Gate 2 — Baseline and extraction plan

### Pre-edit validation baseline

| Check | Result |
|---|---|
| `cargo fmt --check -p gosling` | pass |
| `cargo build -p gosling` | pass |
| `cargo test -p gosling --lib agents::extension_manager` | pass: 37/37 |
| `cargo test -p gosling --lib` | pass: 1,762/1,762 |
| `cargo test -p gosling --test mcp_integration_test` | pass: 0 failed; 4 replay cases intentionally ignored |
| `cargo clippy -p gosling --all-targets -- -D warnings` | pass |

### Extraction plan

`extension_manager.rs` remains the literal **compatibility facade** and preserves
all existing public module paths with re-exports. The manager type, its fields,
constructor, `add_extension`, and the `tests` module path remain anchored there.
The unchanged test body may move to a test-only sibling after production seams.

1. `extension_manager/pagination.rs` — pagination guard and typed MCP collectors.
2. `extension_manager/action_required_stream.rs` — action-required receiver and
   unregister-on-drop lifecycle.
3. `extension_manager/child_process.rs` — command resolution, minimal child/Docker
   environments, Docker env-file encoding, and general stdio MCP connection.
4. `extension_manager/operator_stdio.rs` — neutral operator adapter connection,
   process supervisor, exit status, and bounded stdout reader. Facade re-exports
   its three public names to preserve ADR-0012 callers.
5. `extension_manager/oauth.rs` — OAuth fallback, environment substitution,
   registered clients, streamable HTTP, and unix-socket transport. Facade
   re-exports the two `pub(crate)` environment helpers.
6. `extension_manager/tool_metadata.rs` — parameter schemas, tool owner/visibility,
   MCP App metadata trust, and resolved-tool metadata. Facade re-exports public
   and crate-visible names.
7. `extension_manager/tool_catalog.rs` — cached catalog enumeration/filtering,
   planning prompt, cache invalidation, and MCP App attachment hydration.
8. `extension_manager/resources.rs` — resource reading/listing and UI resources.
9. `extension_manager/tool_dispatch.rs` — owner resolution and tool-call dispatch.
10. `extension_manager/prompts.rs` — prompt discovery and retrieval.
11. `extension_manager/discovery.rs` — extension-search output and MOIM collection.
12. `extension_manager/lifecycle.rs` — lifecycle/query methods other than the
    ordering-sensitive multi-transport `add_extension` constructor path.

Each seam copies code unchanged, delegates through the facade, verifies direct and
second-level callers, deletes the old body only after the new owner compiles, runs
formatting plus focused tests, inspects the diff/single-owner search, and commits.
Final closure reruns the full baseline and MOD-V01–10. Public Rust paths, serde
field names, MCP metadata keys, cancellation/drop behavior, transport timeouts,
environment hardening, and process supervision are explicit preservation risks.

## Gates 3–5 — Seam checkpoints and intermediary audit

Every production module carries a three-line dual-audience header and remains
below 400 lines. The facade re-exports public or crate-visible names only where
the original module path requires it. Rust `pub(super)` is used solely for
cross-responsibility internals.

| Seam | New owner | Lines | Commit | Verification |
|---|---|---:|---|---|
| pagination | `pagination.rs` | 120 | `cc3abd4b1` | pagination collection/limit tests pass |
| action stream | `action_required_stream.rs` | 48 | `62586bd54` | focused 37-test oracle passes |
| tool metadata | `tool_metadata.rs` | 160 | `785ccd7a1` | metadata trust/owner tests pass |
| child process | `child_process.rs` | 217 | `71490fdbe` | environment/Docker cleanup tests pass |
| operator stdio | `operator_stdio.rs` | 295 | `b383aa1ac` | bounded-reader tests pass; public paths re-exported |
| environment | `environment.rs` | 140 | `bbe3890ef` | substitution/static-client tests pass |
| OAuth transport | `oauth.rs` | 346 | `d2abbf7e5` | OAuth/header/timeout tests pass |
| tool catalog | `tool_catalog.rs` | 232 | `1c69158e6` | filtering/cache tests pass |
| resources | `resources.rs` | 200 | `664633263` | focused 37-test oracle passes |
| tool dispatch | `tool_dispatch.rs` | 206 | `a550b4f4c` | dispatch/resolve tests pass |
| prompts | `prompts.rs` | 108 | `6fee0bd47` | focused 37-test oracle passes |
| discovery | `discovery.rs` | 115 | `c017d2426` | focused 37-test oracle passes |
| lifecycle | `lifecycle.rs` | 145 | `4221443b5` | lifecycle/cache/Docker tests pass |
| test body | `tests.rs` | 1,632 test-only | `b5b12f867` | same 37 test names and results |

The environment/OAuth plan item was split into two seams to keep each production
owner below the skill's approximate 400-line ceiling. The test module body moved
only after all production seams passed, reducing the original production facade
without changing its `extension_manager::tests::*` module path.

Intermediary audits found no duplicate implementation owner, stale import, public
contract change, weakened assertion, or new MOD-B suspect. Two compile-time
privacy/path adjustments were structural only: the private tool metadata key
became `pub(super)`, and dispatch now names the already imported
`ToolCallContext`. Delimiter and orphan-comment mistakes caught during seam
verification were corrected before their commits; no failed intermediate state
was checkpointed.

## Gate 6 — Final MOD-V closure

The original `extension_manager.rs` is 687 lines after final formatting, down
from 4,531 (84.8%). Its retained responsibility is explicit: type/field anchors,
stable re-exports, constructor/capability setup, and the ordering-sensitive
multi-transport `add_extension` path. Thirteen production responsibility modules
range from 48 to 346 lines; the test-only sibling is 1,632 lines.

| MOD-V check | Evidence | Result |
|---|---|---|
| V01 compile/type | `cargo build -p gosling`; strict Clippy | pass |
| V02 focused tests | `cargo test -p gosling --lib agents::extension_manager` | pass: 37/37 |
| V03 broad tests | `cargo test -p gosling --lib` | pass: 1,762/1,762 |
| V04 direct integration | `cargo test -p gosling --test mcp_integration_test` | pass: 0 failed; 4 harness-ignored |
| V05 format/diff | `cargo fmt --check -p gosling`; `git diff --check` | pass |
| V06 single ownership | symbol searches over facade and responsibility modules | pass |
| V07 public compatibility | caller searches plus facade `pub use`/`pub(crate) use` | pass |
| V08 wire/metadata stability | serde fields and both MCP metadata-key literals unchanged | pass |
| V09 lifecycle semantics | cancellation, Drop, timeout, environment, process code moved unchanged | pass |
| V10 documentation/closure | facade/module headers, bug ledger, this run log | pass |

Second-level caller checks cover ACP server presentation/tools/resources, the ACP
domain adapter required by ADR-0012, agent/reply/MOIM flows, platform code mode,
extension config resolution, and `mcp_integration_test`. The public paths
`ExtensionManager`, `ExtensionManagerCapabilities`, operator stdio names, metadata
helpers/key, visibility helpers, and environment helpers are unchanged.

BUG-001 remains open and was moved unchanged to `tool_catalog.rs`; it is not
silently fixed by this modularization. No additional source-evidenced defect was
found. The pre-existing Node listener on 8888 and installed Gosling listeners on
53273/53275 remained alive and untouched through closure.
