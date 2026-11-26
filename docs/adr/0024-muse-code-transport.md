# ADR-0024 — Muse Code transport

Date 2026-11-24. Status: headless changes implemented in source. Native
transports remain under qualification.

## Context

`muse-code` runs the user's Muse CLI in the session workspace. Muse owns
its tools and sandbox. gosling owns the conversation, persisted permission
mode, plans, compaction, and provider handoff. A turn currently needs Auto
mode. The API provider `meta` uses its own credentials and billing.

The baseline on `main` at `61eb3399a` used transient headless executions,
conversation replay, a static catalog, default-empty usage, and a terminal
answer. It already used private prompt files, bounded JSONL records, a
subprocess guard, and CLI-owned credentials. See the [original implementation
record](../logs/session/2026-10-17-meta-muse-provider.md).

## Decision

Keep headless execution as the production transport. Use the installed CLI to
discover capabilities and models, perform the supported no-inference billing
check, and consume version 1 JSONL. Continue evaluating ACP through the
existing `AcpProvider` when Muse's permission and credential behavior can meet
the host's requirements. Use the MSP schema as protocol evidence. The MSP
transport and a Gosling-hosted MCP bridge remain qualification work.

| Criterion | Headless `exec` | Native ACP | Native MSP |
| --- | --- | --- | --- |
| Runtime evidence | 1.4.25 emits matching live text, usage and terminal events with the isolated echo provider | 1.4.25 initializes, creates an in-memory session and streams an echo prompt | 1.4.25 negotiates schema v1 and accepts model, account, allowance, session and turn requests |
| Fit and effort | Reuses the provider, private prompt, model setup, storage and streaming interfaces | Reuses the existing JSON-RPC client, tool updates and approval routing. Muse-specific policy and state work remains | Requires an MSP lifecycle, view/event handling and permission adapter. Meta's Rust SDK 1.4.25 declares Rust 1.97, above the repository's 1.92 pin |
| Permissions | Existing Muse-owned Auto mode only. The shell sandbox and workspace posture remain Muse's responsibility | Muse advertises four approval modes. Their behavior, persistent denials and unprompted actions need operation-level qualification | Transport exposes approval and admission RPCs, session limits and MCP inspection. Managed policy and inherited user settings need qualification |
| Continuity | The documented exact-ID CLI resume requires native logs. Production keeps logs off and replays gosling history | A memory-only host disables load. Durable mode needs the gosling-to-Muse binding and recovery contracts | Explicit resume, fork, replay and context/usage updates. Needs durable binding and a context-ownership decision |
| Usage | Version 1.4.24 and later report whole-run totals when available. The standalone allowance command can report billing and quota information | Context usage notification observed, no prompt token usage in the echo result | Stable `usage/read` and `usage/changed`, experimental `usage/refresh` and `account/read`, plus session token and context notifications |
| Dependencies and platforms | Native Muse binary plus the existing Rust stack, with Unix group and Windows job ownership | Same binary and existing ACP libraries. Local qualification is macOS arm64 | Same binary. A Rust implementation must meet current MSRV and qualify independently on supported platforms |
| Reliability and evolution | One process per turn isolates crashes. Final-only JSONL is compatible. A response with inconsistent final text fails | Persistent process needs cancellation, bounded updates and exact session recovery work. Meta marks ACP experimental | Versioned stable contract with experimental opt-ins. Persistent process and stream replay increase maintenance |

[Meta changelog](https://dev.meta.ai/docs/muse-code/changelog) ·
[Headless reference](https://dev.meta.ai/docs/muse-code/extending#headless-and-ci) ·
[MSP reference](https://meta-models.github.io/muse-code-sdk/1.4/generated/msp/methods/) ·
[Rust SDK manifest](https://github.com/meta-models/muse-code-sdk/blob/main/rust/clients/sdk-rs/Cargo.toml)

## Enforcement and recovery

- Check version and required flags before prompt launch. Recognize the
  compatible event schema on each line and bind text, usage and completion to
  the root run. Match streamed text against the terminal result and emit only
  the unseen suffix. The provider interface cannot replace an emitted prefix.
- Strip the two API-key environment variables and the Muse permission-profile
  fail-open override. Run the billing check immediately before execution.
  Reject reported API-key use and unrecognized status. For an older CLI that
  lacks the check or a recognized no-usage response, require explicit
  acknowledgment in the gosling host environment. This does not give authority
  to project configuration or to untrusted input. Separate CLI processes
  cannot atomically lock the external credential store across the check and
  execution.
- Record the root usage snapshot once as turn and cumulative session usage.
  Omit unknown values, active context inference and API price estimates.
- Keep the existing plan boundary, session schema and provider transition
  transaction. After a Muse error the current turn stops, including goal,
  grind and research nudges. Muse can execute tools without emitting a gosling
  tool request. A replay may repeat a side effect.
- Keep native image and approval modes unavailable until a transport qualifies.
  A documented image flag or approval mode is insufficient to establish its
  behavior under inherited configuration.
- Gosling's compaction and handoff summarization require a provider with
  host-managed tools. Muse Code remains available for ordinary chat within its
  estimated context budget. At the hard limit the host refuses to start Muse.

## Execution record and open decisions

The local changes were staged as capability and protocol qualification,
headless implementation, settings and accounting integration, and focused
validation. Source and CLI probes ran independently. Transport selection and
accounting integration followed qualification. The session log records the
commands and results. Rollback is a revert of the local change and does not
require a data migration. There is no installation or release step in this
change.

- **Billing evidence.** A maintainer can qualify the positive allowance schema
  with an active subscription, including a stored-key precedence check using
  a disposable account or fixture. Until then, keep the current fail-closed
  handling and make the absence of a live positive sample explicit.
- **Native execution.** A maintainer should first qualify exact session
  lookup, workspace and account isolation, shell/write enforcement, approval
  denial and cancellation, and inherited hooks and MCP configuration. Resume
  must preserve gosling's canonical history, avoid double compaction, and
  honor no-session privacy. These remain gates on a native adapter.
- **Allowance display.** Extend the usage UI after verifying live status shapes
  and freshness. Signed-in no-usage responses and exhausted windows must
  remain distinct. There is no polling or paid fallback.

Independent review challenged billing status validation, credential-change
races, implicit reruns, settings visibility, manual IDs, and run-versus-context
accounting. The implementation narrows the accepted billing states, checks
immediately before launch, terminates the turn on failure, keeps browser setup
beside the executable field, permits well-formed manual IDs, and separates run
totals from active scope. The future qualification gates cover the remaining
credential and streamed-text limits.
