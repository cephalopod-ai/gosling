# 2026-09-01 ACP server modularization

## Task

- Skill: `repair-source-modularization` from the private agent-skills catalog.
- Target lock: `crates/gosling/src/acp/server.rs` only.
- Excluded production source: `crates/gosling/src/agents/agent.rs` and every other
  original production file.
- Branch: `codex/modularize-acp-server-2026-09-01`.
- Involvement: L2 standard; authority ceiling is governed, behavior-preserving
  source modularization.

## Architecture and caller baseline

The ACP server is the protocol/interface boundary over core agent, session,
workspace, provider, extension, shell, and transport services. Accepted
architecture requires main-owned ACP authority, server-owned domain confirmation,
exact custom-method capability derivation, session/generation fencing, bounded ACP
presentation, and fail-closed shell/session activation. Those behaviors are frozen
for this run.

Direct public callers use the existing `gosling::acp::server` paths for
`AcpProviderFactory`, `GoslingAcpAgent`, `GoslingAcpAgentOptions`,
`GoslingAgentConnection`, `serve`, `run`, and `agent_request_schemas`. Internal
second-level callers use the facade helpers for shell extension selection, working
directory validation, usage notifications, and custom-method schema derivation.
All of those paths must remain unchanged.

Pre-existing processes are operator-owned and remain untouched:

- Node PID 1054 listens on loopback port 8888.
- Installed Gosling PID 16232 listens on loopback ports 53273 and 53275.
- No repository Cargo or rustc process was active at preflight.

## Untouched baseline

Before production edits:

- `cargo fmt --all -- --check`: passed.
- `cargo build -p gosling`: passed.
- `cargo test -p gosling --lib acp::server`: 140 passed.
- `cargo test -p gosling --lib`: 1,762 passed.
- `cargo test -p gosling --test acp_server_test`: 47 passed, 1 intentional
  ignore.
- `cargo clippy -p gosling --all-targets -- -D warnings`: passed.

## Gate 2 extraction plan

The 5,136-line source contains 3,843 production lines and a 1,293-line inline
test module. The original module will remain the compatibility facade. Public
paths, protocol schemas, notification ordering, response bounds, persistence,
and error mapping remain unchanged.

| Order | Responsibility seam | Destination | Primary invariants |
|---|---|---|---|
| 1 | Inline regression suite | `server/tests.rs` | Same test names and module paths |
| 2 | MCP/client extension selection | `server/extension_selection.rs` | Exact endpoint-bound secret rehydration and shell filtering |
| 3 | Session agent activation | `server/session_activation.rs` | Workspace/import instructions, provider inventory refresh, extension overrides |
| 4 | Client capability negotiation | `server/initialization.rs` | Exact custom-method registry and shell metadata |
| 5 | Tool locations and display metadata | `server/tool_metadata.rs` | Trusted metadata only, bounded identifiers and replay markers |
| 6 | Live tool/message events | `server/tool_events.rs` | Notification order, raw-output projection, permission/elicitation flow |
| 7 | Tool and chain summaries | `server/tool_summaries.rs` | Retry, persistence anchor, idempotent chain completion |
| 8 | Active prompt registry | `server/active_runs.rs` | AgentManager busy pin, cancellation, close cleanup, steer fences |
| 9 | Prompt execution | `server/prompt_execution.rs` | Durable in-progress/terminal state, stream ordering, completion verification |
| 10 | Provider/model session mutation | `server/session_configuration.rs` | Between-turn switch, workspace defaults, model validation |
| 11 | ACP transport lifecycle | `server/transport.rs` | EOF drain, public connection types, stdio and HTTP behavior |

Each seam is copied, wired, checked against the original, then deleted from the
facade. Formatting, focused ACP tests, and a scoped diff review run at every
checkpoint; each cohesive checkpoint receives its own commit.

## MOD-B bug ledger

| ID | Evidence | Classification | Route | Status |
|---|---|---|---|---|
| — | No suspect surfaced during extraction or verification | — | No repair route required | closed audit |

## Validation and closure

The source was reduced from 5,136 lines to a 655-line compatibility facade.
Behavior moved into responsibility modules without changing the public
`gosling::acp::server` paths. The largest newly extracted production seam is
`tool_events.rs` at 485 lines; `prompt_execution.rs` is 463 lines.

Extraction commits:

- `775517320` regression tests
- `86cf8705d` extension selection
- `03f9863b5` session activation
- `eba410cae` client initialization
- `474e28c5d` tool presentation metadata
- `9f5a3d05e` message projection
- `cd4afef2d` active prompt runs
- `aca619dd2` session configuration
- `07be0a94c` prompt execution
- `6a7142814` tool chain summaries
- `eda875b23` live tool events
- `da03c4cee` transport wiring

MOD-V evidence:

- MOD-V01 scope: only `server.rs` and its new `server/*.rs` modules changed in
  production source.
- MOD-V02 facade: `server.rs` contains the literal `compatibility facade`
  declaration and retains the existing public types and re-exports.
- MOD-V03 ownership: extracted symbols have one implementation owner; facade
  imports/re-exports are wiring only.
- MOD-V04 formatting: `cargo fmt --all -- --check` passed.
- MOD-V05 build: `cargo build -p gosling` passed.
- MOD-V06 focused regression: `cargo test -p gosling --lib acp::server`
  passed, 140 tests.
- MOD-V07 full library: `cargo test -p gosling --lib` passed, 1,762 tests.
- MOD-V08 integration: `cargo test -p gosling --test acp_server_test`
  passed, 47 tests with one intentional ignore.
- MOD-V09 lint: `cargo clippy -p gosling --all-targets -- -D warnings`
  passed.
- MOD-V10 hygiene: `git diff --check`, public caller searches, governance
  marker checks, module line counts, and process-residue inspection passed.
  The installed Gosling process remained operator-owned and untouched; no
  repository Cargo or rustc process remained after validation.

Result: behavior-preserving modularization complete. No MOD-B suspect was
recorded. `crates/gosling/src/agents/agent.rs` remains outside this run and is
the final routed large-file target.
