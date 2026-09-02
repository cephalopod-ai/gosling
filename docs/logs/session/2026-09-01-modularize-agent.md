# 2026-09-01 agent modularization

## Task

- Skill: `repair-source-modularization` from the private agent-skills catalog.
- Target lock: `crates/gosling/src/agents/agent.rs` only.
- Branch: `codex/modularize-agent-2026-09-01`.
- Involvement: L2 standard; behavior-preserving source modularization.

## Architecture and caller baseline

`Agent` owns the core turn lifecycle: provider/session state, hook and steering
fences, extension/tool projection, durable tool-operation dispatch, context
assembly, streaming response processing, and provider transitions. Public paths
are re-exported from `agents::mod`; `tool_execution.rs` also consumes
`tool_stream`, `ToolOperationGuard`, and `ToolStream` through the existing
`agents::agent` path. Those paths and all persistence/error ordering are frozen.

The complete 5,521-line target was read before edits. Operator-owned processes
remain untouched; no repository Cargo or rustc process was active at preflight.

## Untouched baseline

- `cargo fmt --all -- --check`: passed.
- `cargo build -p gosling`: passed.
- `cargo test -p gosling --lib agents::agent`: 27 passed.
- `cargo test -p gosling --lib`: 1,762 passed.
- `cargo clippy -p gosling --all-targets -- -D warnings`: passed.

## Gate 2 extraction plan

The original module remains the compatibility facade. Each seam is copied,
wired, verified, then deleted from the facade and committed independently.

| Order | Responsibility seam | Destination | Primary invariants |
|---|---|---|---|
| 1 | Inline regression suite | `agent/tests.rs` | Same test names and module path |
| 2 | Hooks and steering | `agent/hooks.rs` | Hook order, fail-closed denial, steer fencing |
| 3 | Frontend extension state | `agent/frontend_extensions.rs` | Deterministic projection and persistence |
| 4 | Durable tool dispatch | `agent/tool_dispatch.rs` | Begin/replay/in-doubt/complete ordering |
| 5 | Extension lifecycle | `agent/extensions.rs` | Parallel load and single persistence semantics |
| 6 | Reply setup and context | `agent/reply_context.rs` | Conversation repair, compaction, context fallback |
| 7 | Streaming turn loop | `agent/reply_stream.rs` | Retry, cancellation, tool, stop-hook, persistence ordering |
| 8 | Provider transitions | `agent/provider_transitions.rs` | Persist-before-live swap and rollback behavior |
| 9 | Prompt and frontend result APIs | `agent/prompt_apis.rs` | Existing public methods and timeout semantics |

## MOD-B bug ledger

| ID | Evidence | Classification | Route | Status |
|---|---|---|---|---|
| — | No suspect surfaced during extraction or verification | — | No repair route required | closed audit |

## Validation and closure

The source was reduced from 5,521 lines to a 532-line compatibility facade.
The implementation is split across hooks, frontend extensions, durable tool
dispatch, extension lifecycle, reply context/entry, provider transitions, prompt
APIs, and the regression suite. `reply_stream.rs` is a documented cohesion
exception at 1,124 lines: it contains the pre-existing single streaming turn-loop
state machine intact so the modularization does not cut through or alter its
retry, cancellation, tool, persistence, compaction, and stop-hook control flow.

Extraction commits:

- `974bad19e` regression tests
- `928d72b6b` hooks and steering
- `3e5115ffe` frontend extension state
- `6e5ddb000` durable tool dispatch
- `acd7ddadc` extension lifecycle
- `e7e8a00e6` reply context
- `679bc27d5` reply entry
- `b9864b936` reply stream state machine
- `36854b863` provider transitions
- `2c5da1b9f` prompt APIs
- `64fe753d7` compatibility facade declaration

MOD-V evidence:

- MOD-V01 scope: only `agent.rs` and its new `agent/*.rs` modules changed in
  production source.
- MOD-V02 facade: `agent.rs` contains the literal `compatibility facade`
  declaration and retains the public construction, event, and stream symbols.
- MOD-V03 ownership: extracted methods have one implementation owner; existing
  `agents::mod` re-exports and `tool_execution.rs` imports remain unchanged.
- MOD-V04 formatting: `cargo fmt --all -- --check` passed.
- MOD-V05 build: `cargo build -p gosling` passed.
- MOD-V06 focused regression: `cargo test -p gosling --lib agents::agent`
  passed, 27 tests.
- MOD-V07 full library: `cargo test -p gosling --lib` passed, 1,762 tests.
- MOD-V08 integration: `cargo test -p gosling --test acp_server_test`
  passed, 47 tests with one intentional ignore.
- MOD-V09 lint: `cargo clippy -p gosling --all-targets -- -D warnings`
  passed.
- MOD-V10 hygiene: `git diff --check`, public caller searches, symbol ownership
  searches, module line counts, governance checks, and process-residue inspection
  passed. The installed Gosling process remained operator-owned and untouched;
  no repository Cargo or rustc process remained after validation.

Result: behavior-preserving modularization complete. No MOD-B suspect was
recorded. This closes the routed >=2,000-line modularization campaign.
