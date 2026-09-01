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
constructor, `add_extension`, and inline behavioral tests remain anchored there.

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

