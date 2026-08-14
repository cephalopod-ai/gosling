# Gosling Repair Campaign — 2026-08-14 (Architecture/Dataflow/Workflow/MCP Follow-Up)

Follow-up to the stress-test audit (`99-master-report.md`) and the earlier
`repair-campaign-log.md` (Stages 1–4, security/reliability-focused). Executed
with `020_repair/repair-defect-campaign`. Authority: patch-authorized on
branch `claude/state-audit-patch-20kthj` (branched from `main` @ `efe97b6`,
PR #50 already merged).

## Scope decision

The user asked to audit and patch, explicitly **excluding security findings**
(already the subject of the earlier campaign and its still-open,
human-owner-gated Cluster A/B items) and scoping to **architecture,
dataflow, workflow, and MCP** lenses. `repair-campaign-log.md`'s own
"Disposition ledger" left several non-security Cluster D/E/F items from
`99-master-report.md` unaddressed — this run re-verified each against the
current source (much had already changed since the July audit) and fixed
every one that was still genuinely open, non-security, and
compile+test-verifiable in this environment.

## Gate 0 — posture & baseline

- Git: branch `claude/state-audit-patch-20kthj`, clean, `== origin/main`.
- Toolchain: hermit cargo 1.92 available; `cargo check -p gosling --features
  nostr` green (unlike the July audit's environment, this one could build
  and run the full test suite).
- Findings source: `docs/cloud/99-master-report.md` Clusters D
  (reliability/external-API), E (crash/recovery integrity), F (operator
  truth/signal, backend-only subset), H (architecture, explicitly routed —
  not touched), plus an MCP-focused sweep of `audit-dataflow-pipeline-graph.md`,
  `audit-resource-lifecycle.md`, `audit-failsafe-readiness.md`,
  `audit-dataflow-state-transition.md`. Four parallel research passes verified
  each candidate against current `git log`-advanced source before anything
  was scheduled for a stage — the July report's line numbers had drifted and
  several items had already been fixed by unrelated later work.

## Gate 1 — verified inventory

| Candidate (99-master-report.md) | Status found | Action |
|---|---|---|
| `transient_only=false` default | already fixed (stale disposition in repair-campaign-log.md) | none — noted here for record closure |
| Corrupt config → silent "start fresh" | already fixed (`load_write_config` now refuses instead of silently emptying) | none |
| CLI logging is file-only | not actually a defect — server/CLI split is intentional; security-relevant degradations already reach the user via the approval-prompt path, not `tracing` | none |
| Unknown slash command sent to LLM | already fixed (`InputResult::Retry`, regression test already exists) | none |
| Bedrock nests a 6-retry budget over the AWS SDK's own retries | **open** | fixed, Group A |
| Missing HTTP timeouts on OAuth token clients | **open** | fixed, Group A |
| `grind` nudge self-feeds unboundedly | **open** | fixed, Group C |
| MCP extension removal doesn't await transport shutdown | **open** (RES-GSL-004) | fixed, Group E |
| Stream truncation accepted as complete | **open**, narrowed (empty-response case already guarded) | fixed, Group B |
| Session import spans 3 un-enclosed transactions | **open** for a true crash (a prior pass added a compensating delete that only covers the *handled-error* path) | fixed, Group D |
| Mid-turn crash replays tool side effects | still needs a dedicated repair with crash drills | **not touched** — already correctly dispositioned as deferred in `repair-campaign-log.md`; not reopened |
| Cluster H (architecture: inverted domain ownership, `agent.rs` god file, config singleton, type-parity gate) | architecture rewrites, excluded by the skill | **not touched** — remains routed to `repair-source-modularization` |
| macOS SIGKILL orphans MCP/ACP/provider subprocess children | real but cross-cutting (not MCP-specific) and needs a macOS runtime drill to verify a fix | **not touched** — flagged, not scheduled |

None of the five fixed items required modularization: every touched file
either stayed under the 1000-line threshold or only needed small, localized
edits relative to its size (the modularization rule's "heavily edit"
threshold), including the four files at or above 2000 lines
(`agent.rs`, `formats/openai.rs`, `session_manager.rs`,
`extension_manager.rs`) — each got the smallest safe fix, no split attempted.

## Stages executed (all compile + test verified, one commit per group)

| Group | Commit | Defects fixed | Files | Validation |
|---|---|---|---|---|
| A — Provider retry/timeout hardening | `97061ec` | Bedrock's app-level retry wrapper no longer races the AWS SDK's own retry loop (`.retry_config(RetryConfig::disabled())` on the loader); xai_oauth.rs (4 sites), oauth.rs (3 sites), gcpauth.rs (1 site) OAuth HTTP clients now set `DEFAULT_PROVIDER_TIMEOUT_SECS`, matching every other provider in the crate | `providers/bedrock.rs`, `providers/xai_oauth.rs`, `providers/oauth.rs`, `providers/gcpauth.rs` | `cargo check`/`clippy` clean |
| C — Grind nudge cap | `4b7109a` | `DEFAULT_MAX_GRIND_NUDGES=50` independent of `max_turns=1000`; exceeding it clears the grind goal and tells the user why instead of self-feeding to the turn limit | `agents/agent.rs`, `tests/agent.rs` | new `test_grind_nudge_cap_terminates_before_max_turns`; `goal_checking_tests` 6/6 |
| E — MCP transport shutdown | `930d90a` | `McpClientTrait::close()` (default no-op) + `McpClient` override awaiting `RunningService::close_with_timeout` (5s bound); `Extension::shutdown` now awaits it before the existing Docker branch | `agents/mcp_client.rs`, `agents/extension_manager.rs` | new `test_close_awaits_transport_shutdown` (real `McpClient` over a `tokio::io::duplex` transport); `extension_manager` 33/33, `mcp_client` 28/28 |
| B — Stream truncation detection | `61d799a` | `response_to_streaming_message` now tracks `saw_done` alongside `yielded_any_content`; a stream that yielded content but never saw `[DONE]` (dropped connection, truncated tool-call arguments) now errors instead of silently completing | `gosling-providers/formats/openai.rs` | new `test_streaming_connection_drop_without_done_is_error`; full `formats::openai` 132/132, `gosling-providers` 438/438 |
| D — Session import/copy atomicity | `1789066` | `create_session_in_tx`/`apply_update_in_tx` (reusing the file's existing `_in_tx`/wrapper convention already used by `replace_conversation_in_tx`); `import_session`/`copy_session` now run session creation, the metadata update, the conversation replace, and (for copy) the artifact-copy insert in one transaction, committed once — a crash or interrupt anywhere in between now rolls the whole operation back instead of relying on a compensating delete that only covered handled errors | `session/session_manager.rs` | renamed + new atomicity regression tests (trigger-forced failure on the final step, session count unchanged); `session_manager` 63/63 |

**Gate 9 final regression:** `cargo test --workspace --features gosling/nostr
--lib` = all crates green (`gosling` 1740, `gosling-providers` 438,
`gosling-sdk-types` 12, `gosling-server` 34, others 0/N-A). `cargo clippy
--workspace --features gosling/nostr --all-targets -D warnings` clean.
`cargo fmt` applied to every changed file.

## What each fix maps to in the audit

- Group A → master-report Cluster D (`bedrock.rs:122-123` nested retry;
  `xai_oauth.rs`/`oauth.rs`/`gcpauth.rs` missing timeouts, listed under
  "Deferred — needs an integration/drill environment" in
  `repair-campaign-log.md` but actually independently fixable per-site
  without a live integration environment).
- Group C → master-report Cluster D ("`grind` nudge self-feeds").
- Group E → not a master-report item; found via a targeted MCP-dataflow
  sweep of `audit-resource-lifecycle.md` (RES-GSL-004).
- Group B → master-report Cluster D ("Stream truncation accepted as
  complete").
- Group D → master-report Cluster E ("Session import spans 3 un-enclosed
  transactions").

## Disposition ledger — not touched this campaign (with reasons)

- **Mid-turn crash replays tool side effects** — needs an atomic turn
  boundary / idempotency key verified with a crash drill; already correctly
  deferred in `repair-campaign-log.md`, not reopened here (out of scope: a
  data-path redesign, not a bounded defect fix).
- **Cluster H (architecture)** — inverted domain ownership, `agent.rs`
  3.8K-plus-LOC orchestrator, 256-site config singleton, cross-language type
  parity. Architecture rewrites are explicitly excluded by
  `repair-defect-campaign`; route to `repair-source-modularization`.
- **macOS subprocess orphan on hard SIGKILL** (`subprocess.rs:52-63`) — real
  and self-documented in code, but cross-cutting (affects every subprocess
  type, not MCP-specific) and disproportionately costly to verify safely
  here (needs a macOS runtime drill). → routed, not patched blind.
- **Security/permission-control findings** (Clusters A, B, C-secrets, G) —
  explicitly out of scope for this pass per the user's request; already
  covered or dispositioned by the earlier `repair-campaign-log.md`.

## Record closure

- `transient_only` default, corrupt-config silent-reset, and unknown-slash-
  command findings are recorded above as **already fixed** by work that
  landed after the July audit and before this campaign — `repair-campaign-
  log.md`'s "Deferred — human-owner" disposition for `transient_only` is
  stale as of this run; noted here rather than editing that historical log.
- CLI file-only logging is recorded above as **verified-not-a-defect** with
  reasoning (intentional server/CLI split; degradations already reach the
  operator through the approval-prompt path).
- No in-code `TODO`/`FIXME` markers referenced any of the five fixed defects
  — nothing to scrub.

## Final status

`completed_verified` for the in-scope eligible-defect set (Groups A, C, E,
B, D — all compile+test verified, full workspace test suite and clippy
clean). Cluster H and the mid-turn-replay item remain correctly routed/
deferred with reasons; the macOS orphan gap is flagged for a future drill-
verified pass. No security findings were touched, per the user's scope.
