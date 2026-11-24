# 2026-11-24 — Muse Code modernization

## Task and baseline

- Repository `/Users/eric/Work/vscode/forked/gosling`, clean `main` at
  `61eb3399a5a5b3f6ad54b86f4c84bde20c0b4f75`, tracking `origin/main`.
  Muse provider history starts at `340efa6d0` with fixture fixes at `9b72051bd`.
- Host macOS arm64, repository Rust toolchain `1.92`, and installed Muse Code
  `1.4.25 (1.4.25-R9361.1)`. Meta's changelog and the SDK mirror both listed
  1.4.25 during qualification.
- Reviewed the provider registry and the separate Meta API format, canonical
  metadata, external adapters for Claude, Codex, Copilot, Pi and Vibe,
  the shared ACP provider and subprocess helpers, Desktop and CLI setup,
  session usage persistence, plan and permission boundaries, compaction,
  Provider Transitions and Full Session Handoff.
- The private catalog supplied `plan-rust-app`. The independent critic reviewed
  auth, replay, compatibility, settings and usage semantics. Findings and
  dispositions are in the [transport ADR](../../adr/0024-muse-code-transport.md).

## Qualification evidence

| Surface | Evidence and limits |
| --- | --- |
| Headless JSONL | Echo through installed Muse yielded schema 1, linked and started run IDs, incremental Unicode text, a zero-token root snapshot, and matching completion. The opt-in Rust test repeated this through gosling's provider. |
| Models and reasoning | `exec --list-models` is documented from 1.4.24. A no-inference read using the existing sign-in listed Wonder 1.0, Ember 1.0, Spark 1.3 and Spark 1.2. An empty profile lacked credentials. The CLI help advertises effort values, and gosling retains the existing model-specific mapping. Per-model effort entitlements were not exercised live. |
| Native continuity | The documented `exec --session-id` resumed the same isolated echo session from a new process. A different workspace was refused, and combining an explicit ID with `--no-session-log` failed at CLI usage validation. No durable gosling binding was added. |
| Billing | `usage --json` returned an API-key state with a synthetic environment key. The installed signed-in profile returned `server_sent_no_usage`, with no allowance. A clean profile returned a sign-in error. A positive allowance shape is covered by a fixture based on the SDK schema. Stored-key precedence is documented. The CLI refused to store an invalid synthetic key in a disposable profile, so persistent-key reproduction remains unverified. |
| ACP | Protocol v1, `session/new`, text streaming and a context update were observed in isolation. The host advertised images and four modes: `allowAll`, `promptUnmatched`, `onRequest`, `denyUnmatched`. Memory-only operation reported `loadSession: false`. The example run returned no prompt token totals. Real approval requests, actions and image inference were not exercised. |
| MSP | The installed schema identifies stable model discovery, resume, approval, usage and context surfaces and experimental account, refresh and admission calls. A sandboxed echo stdio host negotiated schema v1 and experimental opt-in, answered `account/read` and `usage/read`, started a session and accepted an echo turn. The empty usage result and bundled catalog are not evidence of account entitlement. |
| Restrictions and richer inputs | Help and the SDK describe read-only posture, shell/write disabling, image flags, structured final response, step limits, tools and MCP, per-session controls, hooks, plugins, skills and subagents. The echo probe supplied explicit shell and write disable flags. No side-effecting operation, inherited-configuration containment, schema-constrained Meta generation, branching, crash recovery, or host-managed tool execution was qualified. |

Primary references are [Meta's changelog](https://dev.meta.ai/docs/muse-code/changelog),
[headless guide](https://dev.meta.ai/docs/muse-code/extending),
[authentication guide](https://dev.meta.ai/docs/muse-code/auth),
[permissions guide](https://dev.meta.ai/docs/muse-code/permissions), and
the [published 1.4.25 SDK source](https://github.com/meta-models/muse-code-sdk).
The installed schema was exported using `muse schema generate-json-schema
--out <temporary-directory> --experimental`. All native probes used disposable
workspaces and settings. The no-inference account queries used the existing
CLI sign-in without printing account details. No authenticated model turn,
paid request, install, deployment or publication was performed.

## Implementation

- `crates/gosling/src/providers/muse_code.rs` and its `cli` and `output`
  modules own capability and billing probes, the guarded subprocess, and
  root-run JSONL streaming. The existing config, history reconstruction,
  naming fast path, private prompt and external-tool boundaries remain.
- ACP model validation permits manual IDs for CLI verification. The shared
  Desktop configuration dialog retains browser sign-in steps with the optional
  path field. Registry identity and the separate Meta API provider stay as before.
- Session and agent changes store cumulative run usage separately from active
  context and API pricing. A terminal Muse error also stops goal/grind/research
  continuations to avoid repeating invisible external tool effects.
- Windows process-tree ownership uses Job Object features of the existing
  `process-wrap` crate. Cargo.lock has no dependency or version changes.
- As a reversible compatibility choice, the provider keeps one headless process
  per turn, uses a 30-minute run limit, and accepts an explicit host-environment
  acknowledgment when the CLI cannot confirm subscription billing. The
  transport ADR and [user guide](../../../documentation/docs/guides/meta-muse.md)
  document the visible changes and remaining constraints.

## Validation checkpoint

- Baseline focused Meta suite: eight passed with `rustls-tls`.
- Implementation milestone: twelve focused Rust tests passed. The separate
  installed-Muse echo test passed when invoked explicitly. The session storage
  accounting test passed.
- Desktop SDK was current. Desktop TypeScript and the focused provider settings
  suite passed after correcting the test fixture's required OAuth flag.
  Both edited TypeScript files passed Prettier.
- The user guide compiled with MDX.
- The debug CLI built with `cargo build --locked -p gosling-cli
  --no-default-features --features rustls-tls --bin gosling`. A hermetic
  `gosling run --provider muse-code --model muse-spark-1.3 --no-session` with
  an echo shim exited 1 before the model without a billing acknowledgment,
  then exited 0 and returned the synthetic Unicode prompt once with the
  acknowledgment. This exercises dispatch and process handling without
  a subscription inference or API request.
- An isolated gosling ACP stdio round trip exposed both provider setup entries,
  kept the Meta and Muse Code configuration keys distinct, and returned the
  subprocess fixture's dynamic Muse models and the known Spark context limit.
- The existing context-estimate test passed after the usage write change.
- Formatting, whitespace, local documentation links and the Cargo and pnpm
  lockfile comparisons passed.
- The isolated Windows Job Object spawn helper cross-checked for
  `x86_64-pc-windows-msvc` using the existing process-wrap version. Native
  Windows and Linux process behavior and packaged Desktop launch remain
  unverified.
- Strict Clippy found existing `clippy::string_slice` failures at
  `crates/gosling/src/mail/workflow.rs:502` and `:543`. The mail file matches
  the baseline. Scoped Clippy without warning promotion finished with
  those two warnings.
