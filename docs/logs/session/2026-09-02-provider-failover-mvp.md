# 2026-09-02 — Provider failover MVP

## Task

Add the first safe provider/model failover path for transient outages, including local Ollama and
OpenRouter routes, without replaying tool side effects or permanently changing a session's selected
provider.

## Root cause

Provider adapters and the agent stream loop had bounded same-provider retries, and partial assistant
messages were checkpointed and rolled back between safe retries. Once those retries were exhausted,
the final transient error always ended the turn because the stream loop had no secondary provider
route.

## Implementation

- Added an opt-in fallback pair through `GOSLING_FAILOVER_PROVIDER` and
  `GOSLING_FAILOVER_MODEL`, plus an `AgentConfig` builder for embedded callers.
- Snapshot the fallback at turn start and activate it only after the primary transient retry budget
  is exhausted.
- Roll back the interrupted assistant stream before sending the provider-neutral conversation to the
  fallback.
- Keep fallback selection turn-local; the persisted session provider and model remain unchanged.
- Carry the active provider/model through context management, compaction, inference metadata, and
  tool-pair summarization after the switch.

## Safety boundary

Failover is attempted once per turn and only before any tool from the interrupted response has run.
It rejects credential-pinned sessions, providers that manage their own context or execute tools
outside Gosling, action-required provider routes, identical primary/fallback pairs, and mismatched
toolshim modes. Incomplete or unusable fallback configuration does not prevent the primary from
running; it is surfaced if the primary later needs failover.

Configuring a cloud fallback is an explicit data-egress decision because that provider receives the
provider-neutral conversation, system and project instructions, and tool definitions. A local
Ollama fallback keeps that provider request on the configured Ollama endpoint.

## Validation

- `cargo fmt --all -- --check`: passed.
- `cargo build`: passed.
- `cargo test`: passed across the full Rust workspace, including doc tests and the focused agent
  failover/retry suite (3 passed).
- `cargo clippy --all-targets -- -D warnings`: passed.
- Desktop `pnpm run typecheck`: passed through the repository Hermit toolchain.
- Desktop `pnpm test -- --run`: 148 files and 1,105 tests passed.
- Documentation governance marker checks and `git diff --check`: passed.
- The exhaustion regression proves four primary attempts, one fallback attempt, rollback of all
  partial text, successful fallback completion, no terminal error, and an unchanged persisted
  primary route.
- The post-tool regression runs with a fallback configured and proves the fallback receives zero
  requests after a tool has executed.

The provider behavior is verified with deterministic test doubles. A live Ollama or OpenRouter
outage/fallback drill was not run and remains an operator acceptance check.

## Deliberate non-scope

- No automatic ranking, health probing, multi-hop chain, or implicit fallback.
- No Desktop settings UI in this MVP.
- No cross-provider failover for self-managed CLI/ACP providers or isolated credential profiles.
