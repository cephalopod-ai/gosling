# TODO

## Open items from the 2026-08-15 exhaustive audit — recorded 2026-08-16

The repair campaign (`docs/logs/session/2026-08-16-audit-repair-campaign.md`)
closed roughly 62 of the 94 live High/Medium findings plus most Low items across
19 gated groups, merged as `c828a5895`. What remains is listed here so it is not
rediscovered from scratch.

### Needs a design decision, not a patch

- [ ] **SEC-GOS-002** — the live ACP MCP-app guest installs a client-supplied
      CSP; goslingd builds one server-side. Port the server-side builder.
- [ ] **SEC-GOS-007** — `/mcp-app-proxy` and `/mcp-app-guest` are exempt from
      token auth because browsers cannot set headers on an iframe load. Needs a
      nonce or equivalent, the same constraint class SEC-GOS-001 hit.
- [x] **SEC-GOS-012** — closed in `6a02881fb`. The combination is refused with
      an actionable error; verified live across unauth+0.0.0.0 (refused),
      authenticated+0.0.0.0 (serves), and unauth+loopback (serves).
- [ ] **CON-GSL-001** — cross-process recover can mark a live peer's tool
      `in_doubt`. The obvious `updated_at` guard was implemented and rejected:
      `updated_at` only moves on state transitions, so it means "started
      recently", not "owner alive". Needs a real liveness source — a heartbeat
      on `updated_at`, or an owner PID/lease the recovering process can probe.
- [ ] **AOC-GOS-002** — tagteam puts the full system+conversation prompt on
      argv, where `ps` exposes it. The fix is stdin, but the tagteam CLI is an
      external binary whose input contract is not verifiable from this repo.
- [ ] **DAT-GSL-003** — `UNIQUE(session_id, message_id)` needs a schema
      migration plus dedupe of existing rows on live databases.
- [ ] **TMP-GOS-005 / TMP-GOS-006** — pinned-vs-live workspace folder policy is
      a documented product invariant; config migration versioning needs a
      `config_version` scheme and a dual-read deprecation window.
- [ ] **REC-GSL-001** — `Cargo.toml` declares `repo-makeover/gosling`; the
      remote is `cephalopod-ai/gosling`. **Nine workflows are gated on
      `github.repository == 'repo-makeover/gosling'` and therefore never run
      here**, including `cargo-deny`, `scorecard`, and `dependabot-auto-merge`.
      Correcting the slug activates all nine at once — an operator decision.

### Blocked on tooling

- [ ] **RSP-GSL-002 / RSP-GSL-003** — a secret-scanning job and
      `[licenses]`/`[bans]`/`[sources]` in `deny.toml`. `cargo-deny` is not
      available in the dev environment, so neither could be validated; shipping
      unverified CI config that fails on first run is worse than the open item.

### Deliberately not fixed, with reasoning recorded in-tree

- **SEC-GOS-011** — failing closed on an absent WebSocket `Origin` was
  implemented and tested live: it returns 403 to every non-browser ACP client
  while blocking no browser attack, because the spec requires browsers to send
  `Origin`. Reverted; reasoning is at the call site.

### Closed in the second repair batch — 2026-08-16

- [x] **STT-GOS-001** (`886c8df8b`) — Chat mode executed frontend tool
      requests because the execution loop sat above the Chat branch. Residual:
      verified structurally and by compile, not by a runtime test.
- [x] **STT-GOS-005** (`886c8df8b`) — permission write failures were swallowed;
      `persist` and the mutators now return the error and every call site
      handles it deliberately.
- [x] **ARCN-GSL-001** (`6a02881fb`) — the CSP handler keyed the ACP lease
      lookup by webContents id instead of `BrowserWindow.id`, so the CSP
      omitted the local ACP origin.
- [x] **SECN-GSL-002** (`6a02881fb`) — the extension allowlist fetch now
      requires https and is bounded by timeout and size.

See `docs/logs/session/2026-08-16-audit-repair-batch2.md`.

### Lower priority, mechanical but needs a judgement call

ARCN-GSL-002 (49 scattered `process.env` reads), ARC-GSL-005 (duplicated
`GOOSE_EXCLUDED_SKILL_IDS` across a TS/JS boundary), INV-GSL-003 (slash
`COMMANDS` vs dispatch `match` exhaustiveness), MEM-GSL-004 (TUI `turns` grows
unbounded; capping changes reachable scrollback), PERF-GSL-002 (Desktop
`performance.spec.ts` is a single-run smoke script presented as a benchmark),
SIG-GSL-005 (ML-classifier init failure is log-only), DAT-GSL-006 (session
create + extension apply are two commits), NEG-GSL-003 (`GOSLING_SHELL` flavor
is unmodeled scanner input), ARC-GSL-001 (three files over 4000 lines, routed to
a dedicated modularization pass rather than split mid-repair).

### Known-failing test predating this work

- [ ] `context_mgmt::summarizer::tests::defaults_to_off` fails on clean HEAD and
      was left untouched throughout the campaign. Diagnose separately.

## Provider follow-up — observed 2026-08-16

- [x] **Grok / xAI OAuth tool-schema rejection.** Fixed in `11806887c` —
      `formats/openai.rs::object_rooted_parameters` coerces union-rooted MCP
      tool schemas to an object root before they reach the provider. Residual:
      this is a compatibility shim at the provider seam; `math_mcp__math_analyze`
      still declares an `anyOf`/`oneOf` root upstream.
      Original report:

- [ ] ~~**Grok / xAI OAuth tool-schema rejection.**~~ `gosling` with the
      `xai_oauth` provider fails a tool call with
      `Bad request (400): math_mcp__math_analyze: tool parameter root must be an
      object type (root schema is an anyOf/oneOf union with a non-object
      branch)`. Reported repeatable. Investigate whether Gosling forwards MCP
      tool schemas that xAI rejects, and normalize them at the provider seam.
- [x] **Mistral (`vibe`) CLI as an ACP provider option.** Done —
      `crates/gosling/src/providers/vibe_acp.rs`. Uses the `vibe-acp` console
      script the `mistral-vibe` package ships, so it is a normal `AcpProvider`
      registration rather than a CLI scraper. Verified end to end through the
      built binary. Follow-up worth knowing: Gosling's `Chat` maps to Vibe's
      `plan`, which writes a plan file under `~/.vibe/plans/` instead of
      running nothing — usable for planning, not a no-side-effects mode.

## Provider authentication follow-up — observed 2026-08-15

- [ ] Investigate and repair Gemini provider OAuth configuration failing with
      `OAuth login failed: Internal error`. The operator supplied a Desktop provider-configuration
      screenshot showing the failure. This item is recorded only; no diagnosis or login-flow change was
      attempted as part of the Default Shell pre-GUI corrective work.

## Shared project-shell readiness — reassessed 2026-08-13

The host/process ACP foundation is merged, but the post-Gate-4
[readiness reassessment](build/shell-productization/readiness-reassessment.md) found that it is not
yet consumable by separate project shells. The renderer is hard-coded and lifecycle-only; main-owned
ACP exposes no safe renderer prompt/update/permission service; the Rust domain-adapter trait has no
production registration path; package metadata/resources are not fully project-neutral; and reusable
shell workflows are absent. R0 repaired the Linux V8 helper and restored the baseline; three
successive `main` CI runs through `31744291492` completed successfully. Reverify current CI before
execution, but do not mistake baseline health for project-shell readiness.

Forward Gates 5–8 are superseded by the
[project-shell readiness plan](build/shell-productization/project-shell-readiness-plan.md). R0 is
complete. Follow the focused
[pre-GUI backend implementation plan](build/shell-productization/pre-gui-backend-implementation-plan.md)
to freeze and implement R1–R4 before adding shared UI or widening preload. Named adapters, prompts,
workflows, UI, branding, real publication, and updater promotion remain outside this campaign. A
DAWES, math, or other named shell begins only after milestone M5 proves a copy-free neutral consumer
end to end, unless the operator explicitly accepts a narrower development-only exception.

## v1.0.0 release readiness - 2026-07-20

- [x] Prepare the README, release notes, release process, release checklist, user-manual entry points, documentation index, inventory, and stewardship status for v1.0.0.
- [x] Preserve the historical v0.0.6 note and audit/playtest evidence as point-in-time records.
- [ ] Change all source, lockfile, generated API, Desktop package, About, and runtime version surfaces from `0.1.0` to `1.0.0` in a dedicated reviewed release change.
- [ ] Complete every source, documentation, packaged-GUI, signing, checksum, scenario, and clean-install gate in `RELEASE_CHECKLIST.md`.
- [ ] Tag, publish, verify, and announce v1.0.0. These actions remain maintainer-owned and were not performed by documentation stewardship.

## Chat reliability and CLI usage backlog — 2026-07-17

- [ ] Keep the chat view pinned to the bottom while a new user input is typed
      and while new content is appended, so the most recent chat item remains
      visible instead of the scroll position jumping to the middle of the window.
- [x] Make chat persistence incremental and crash-resilient: store each user
      message as soon as Enter is submitted, and store assistant output as it is
      written to the chat window, so an abrupt Gosling exit does not erase the last
      chat item.
- [ ] For CLI usage with subscription-backed providers where usage data is
      available, including Codex and Claude, add a way to inspect current usage
      during a session.

## Tagteam workflow, MCP control plane, and Run Steward

**Status:** Phase 1 foundation and an isolated Phase 2 Unix-socket MCP adapter
are implemented behind a disabled feature. The required lint and MSRV jobs
compile it explicitly, and the CI test job runs its feature-specific tests.
Product UI activation, workflow-service integration, legacy-provider
replacement, durable lifecycle ownership, and fleet features remain deferred
until their gates are met. The detailed staged plan is in
[`reports/2026-07-12-tagteam-future-integration-plan.md`](../reports/2026-07-12-tagteam-future-integration-plan.md).

Phase 1 is intentionally producer-independent while Tagteam remains in its
debug-use loop. It provides architecture contracts, internal types, additive
persistence, deterministic event reduction, steward capability policy,
test-only consumer fixtures, and a disabled feature gate. The Phase 2 adapter
adds a strict `McpTagteamClient` for Tagteam's durable Unix-socket MCP daemon:
it verifies protocol, producer schema/capabilities, and canonical repository
identity; accepts structured content only; forwards producer-prepared approval
records unchanged; and fails closed on malformed or ambiguous producer data. It
does not spawn Tagteam, select models, duplicate its profile catalog, or add a
visible control with no live handler.

Gosling should treat Tagteam as a session workflow, not as an LLM provider.
The user selects a normal Gosling provider, model, and reasoning effort for the
outer **Run Steward**, then separately selects the Tagteam mode and models for
each implementation, review, supervision, or scout role. Execution authority
belongs to a deterministic controller exposed by Tagteam's versioned MCP
contract; the steward only monitors, explains, reports, and prepares recovery.

### Deferred live implementation horizon

- [x] Add a typed Unix-socket MCP control client for Tagteam's published
      control-plane contract. The feature-gated adapter covers validate,
      prepare-start, start, status, plan, findings, prepare-resume, resume, cancel,
      diagnostics, structured producer errors, and reconnecting a fresh client to
      the same daemon fixture.
- [ ] Add a `Standard` versus `Tagteam` session-workflow distinction without
      overloading Gosling's existing tool-permission mode. Keep the selected Run
      Steward provider/model in the normal session model configuration.
- [ ] Replace the current hardcoded Tagteam-profile/provider path with a typed
      workflow service that consumes the typed MCP control client and reason codes.
      Do not copy Tagteam's profile registry, model catalog, flag validation, or
      recovery state machine into Gosling.
- [ ] Persist a versioned launch specification and run binding containing the
      Gosling session, repository identity, Tagteam run ID, run directory, state
      root, sanitized normalized arguments, last event sequence, and last
      trustworthy snapshot. Do not treat a persisted PID as execution authority.
- [ ] Add a setup surface with four explicit groups: Workflow, Run Steward,
      Tagteam Team, and Execution. Show role labels that change with supervisor,
      relay, adversarial, and solo modes so users can always see which model edits,
      reviews, supervises, or scouts.
- [ ] Include repository root, explicit allowed paths, rounds, invocation and
      watchdog timeouts, bounded test presets, and Assist recovery policy in the
      execution setup. Do not accept model-authored shell or unrestricted flag
      strings.
- [ ] Restrict Tagteam workflow sessions to the dedicated Tagteam MCP tools.
      Do not expose Developer, arbitrary shell/edit tools, subagent delegation,
      extension management, or unrelated external MCP tools to the Run Steward.
- [ ] Launch the validated Tagteam action from the user's Run action rather
      than depending on the steward to select the correct tool. Feed the steward
      normalized updates only when phase, role, diff, test, finding, fallback,
      stall, approval need, or terminal state materially changes.
- [ ] Render a persistent live run card in Desktop and the text UI: mode and
      role assignments, current phase/round, elapsed and idle time, diff counts,
      tests, findings, degraded/blocking reason, and artifact references. Keep raw
      transcripts and repository content opt-in and bounded.
- [ ] Implement Assist-only recovery first. The steward may inspect status,
      plan, findings, diagnostics, and prepare-resume results. Resume and cancel
      require an action-bound user approval; scope widening, finding deferral,
      transfer, branch cleanup, and unsafe Tagteam flags remain unavailable.
- [ ] Add deterministic fallback messages so monitoring still works if the Run
      Steward is unavailable or returns invalid output. The steward is never on the
      critical execution path.
- [ ] Validate the real Tagteam daemon with scratch-repository runs across all
      Tagteam modes. The current socket fixture covers protocol/capability checks,
      canonical-root matching, structured terminal errors, approval forwarding,
      malformed producer data, and reconnect. Include Ollama/low-capability,
      mid-tier, and frontier stewards; restart/reconnect, duplicate-launch,
      cancellation, stalled run, blocking findings, test failure, and unsafe resume
      cases before activation. The ignored `live_tagteam_socket_smoke_test` can
      validate the read-only adapter boundary against a locally started daemon with
      `TAGTEAM_MCP_SOCKET=<socket>`; it deliberately never launches a run.

### Future vision

- [ ] Connect Gosling to a durable Tagteam daemon for background execution,
      reconnectable event streaming, safe cross-restart cancellation, and one
      authoritative observer lease per run.
- [ ] Add local-first steward escalation policies: deterministic templates,
      then Ollama, then an optional explicitly configured cloud model for ambiguous
      diagnosis. Preserve strict per-run cost, call, timeout, and contention
      budgets.
- [ ] Add saved team configurations and organization policies only after
      Tagteam exposes machine-readable capability and profile provenance. Display
      resolved roles and versions rather than trusting stale labels.
- [ ] Add fleet monitoring for active, waiting, stalled, blocked, degraded, and
      recoverable runs while keeping repository content, prompts, secrets, and
      private reasoning local by default.
- [ ] Generalize the workflow/controller boundary for other long-running
      external orchestrators only after the Tagteam implementation demonstrates a
      stable contract; do not create a generic arbitrary-process launcher.

### Acceptance boundary

- The same Tagteam launch specification produces the same normalized action
  whether initiated from Desktop, text UI, CLI, or another MCP-capable host.
- Gosling can restart, reconnect to persisted status, and avoid a duplicate run
  without asking the steward to infer process state.
- A local low-capability steward can accurately report deterministic facts and
  request user help, while tests prove it cannot edit files, broaden scope,
  approve recovery, or recursively invoke Tagteam.
- The legacy Tagteam provider is not removed until workflow parity, migration
  guidance, runtime playtests, and rollback behavior are verified.

## Exhaustive defect-repair campaign — 2026-07-17

Audit checkpoint:
[`reports/2026-07-17-exhaustive-defect-audit-checkpoint.md`](../reports/2026-07-17-exhaustive-defect-audit-checkpoint.md).
Repair plan and evidence:
[`reports/2026-07-17-defect-campaign-plan.md`](../reports/2026-07-17-defect-campaign-plan.md)
and
[`reports/2026-07-17-defect-campaign-session-log.md`](../reports/2026-07-17-defect-campaign-session-log.md).

The synchronized audit froze 34 findings. The repair campaign fixed 33 and
left one explicitly dispositioned architectural residual; it also fixed one
post-freeze SDK request-shape defect found by verification. Only the audit
checkpoint is synchronized to the remote. All repair and closeout commits are
local until a separate push is authorized.

- [x] AUD-031: sessions.db schema v23 adds a durable tool-operation ledger with
      stable operation identities, explicit in-doubt recovery, terminal-result
      replay, and MCP operation-id propagation for servers that support external
      deduplication. Tool requests are checkpointed before dispatch and terminal
      responses are linked back to the ledger transactionally.
      Residual risk: Gosling cannot prove whether a non-idempotent external server
      committed an operation before a transport or process failure. Such operations
      remain visibly in doubt and require external verification; Gosling does not
      retry them automatically.
- [ ] Modularize the routed >=2000-line files in dedicated changes, preserving
      behavior and avoiding mixed repair/refactor commits:
      `crates/gosling/src/session/session_manager.rs`,
      `crates/gosling/src/acp/server.rs`,
      `crates/gosling/src/agents/agent.rs`,
      `crates/gosling/src/agents/extension_manager.rs`,
      `crates/gosling/src/agents/platform_extensions/summon.rs`, and
      `ui/desktop/src/main.ts`.
- [x] Run the added Rust regression suite, workspace build, and Clippy before
      merge when explicitly authorized. The 2026-07-18 twelve-lens follow-up ran
      the workspace build, serialized `gosling` library suite, related crate suites,
      and all-target Clippy successfully.

## Defect-repair campaign — 2026-07-16

Full inventory, skill disposition, and repair log:
[`reports/2026-07-16-defect-audit-and-repair.md`](../reports/2026-07-16-defect-audit-and-repair.md).
42 defects found across 12 audit lenses, grouped into locality-based repair
stages. 22 repaired under `repair-defect-campaign` gates (patch, regression
test, change review, commit per stage) across three passes; the remaining 20
were carried forward and repaired (13) or deferred with reasoning (5, plus
the 3 already-deferred from this pass) by the 2026-07-18 follow-up campaign
below. Track per-stage status in those reports rather than duplicating them
here.

Corroborates two previously-deferred, still-open findings from
`reports/2026-07-10-audit-skills-pack-report.md`: the `/status` static-200
health lie (there: FSR-SRV-001, here: OPS-001 — repaired in this pass) and
the hardcoded `exit_type="normal"` telemetry (there: FSR-SRV-002, here:
OPS-003 — repaired in the 2026-07-18 follow-up campaign below). Correction:
this session's sandbox cannot build `gosling-server` either (`cargo build -p
gosling-server` fails downloading `v8-goose`'s prebuilt V8 binary from a
blocked GitHub-releases host) — the underlying `gosling` crate change
(`SessionManager::healthy()`) is compiled and tested, but the
`gosling-server` route handlers themselves are
unverified by `cargo build`/`test`/`clippy` in this environment. Recommend
CI confirm both before merge.

## Audit and repair campaign — 2026-07-18

Full disposition, architecture-invariant compliance check, and repair log:
[`reports/2026-07-18-audit-repair-campaign.md`](../reports/2026-07-18-audit-repair-campaign.md).
Repaired 13 of the 2026-07-16 campaign's 20 open defects (ORCH-003, CON-003,
OPS-002, OPS-003, OPS-004, OPS-005, INV-001, INV-002, GUI-002, GUI-004,
GUI-005, SEC-003, CON-001); deferred 5 with stated reasoning (ORCH-002,
RES-002, RES-003, REC-001, REC-002), same sandbox build limitations as
above for `gosling-server` and `ui/desktop`.

## Twelve-lens audit and defect-repair campaign — 2026-07-18

Audit report and machine-readable inventory:
[`reports/2026-07-18-twelve-lens-agent-skills-audit.md`](../reports/2026-07-18-twelve-lens-agent-skills-audit.md)
and
[`reports/2026-07-18-twelve-lens-agent-skills-findings.json`](../reports/2026-07-18-twelve-lens-agent-skills-findings.json).
Repair plan and execution evidence:
[`reports/2026-07-18-twelve-lens-defect-campaign-plan.md`](../reports/2026-07-18-twelve-lens-defect-campaign-plan.md)
and
[`reports/2026-07-18-twelve-lens-defect-campaign-session-log.md`](../reports/2026-07-18-twelve-lens-defect-campaign-session-log.md).

The catalog-driven audit froze 10 findings. All 10 were repaired: plaintext
prompt secret profiles, renderer filesystem self-authorization, unenforced
workspace folder access, delegated-role capability inheritance, lossy JSONL
imports, imported transcript authority, unvalidated settings IPC, tear-prone
Desktop JSON writes, unbounded import payloads, and invalid-host startup panic.
The reports retain the full threat analysis, repair stages, regression proof,
and the one upstream Nostr allocation limitation. No campaign commit or remote
mutation was performed.

## Open-defect campaign reconciliation (2026-07-20)

- [x] Chat auto-follow remains enabled while the user is at the bottom and pauses after upward user scrolling.
- [x] Interrupted chat/tool operations are durably recorded and recovered without redispatching an in-doubt side effect.
- [x] ACP runtime config, data, state, identity, and request execution are scoped to the server instance rather than the process default.
- [x] Desktop browser-global lint debt and unstable workspace-filter hook dependencies are repaired.
- [x] Provider inventory startup reads are cached and concurrent reads are coalesced; mutations invalidate the cache.
- [x] The ACP schema check resolves repository paths from the Justfile location.
- [ ] CLI usage reporting remains a feature backlog item, not an open defect.
- [ ] Session Handoff and expanded Tagteam remain feature backlog items, not open defects.
- [ ] Giles's internal uniqueness-constraint failure remains external tool debt.
- [ ] Release execution remains maintainer-owned.

### Provider inventory concurrency closure (2026-07-20)

- [x] Mutation epochs invalidate provider inventory at both mutation boundaries.
- [x] Reads superseded by an invalidation retry against the current generation.
- [x] Reads completing during a mutation cannot repopulate the shared cache.

### Open-defect campaign verification closure (2026-07-20)

- [x] Rust formatting, library tests, server tests, and workspace clippy are green.
- [x] Desktop typecheck, 547 tests, ESLint, and i18n validation are green.
- [x] ACP schema generation and generated TypeScript consistency are green.
- [x] Credential selector, chat scrolling, parent supervision, Claude permissions, and container cleanup regression cards pass.

## Six-lens audit and repair campaign — 2026-08-12

Full inventory and repair evidence:
[`reports/2026-08-12-six-lens-agent-skills-audit-repair.md`](../reports/2026-08-12-six-lens-agent-skills-audit-repair.md).

- [x] On explicit CLI turn cancellation, close undispatched sibling tool requests with terminal,
      idempotent responses while preserving ledger-only reconnect recovery and the existing in-doubt
      boundary after dispatch.
- [x] Bound diagnostics disk reads as well as returned content, report real truncation, and
      create full diagnostic bundles owner-only with an explicit sharing warning.
- [x] Serialize `projects.json` read-modify-write operations and atomically replace private
      tracker state.
- [x] Coordinate shared memory JSONL readers and batch writers with file locks and durable
      flushes.
- [ ] Replace Bedrock/SageMaker process-global AWS environment export with provider-instance
      AWS SDK configuration. Preserve and test environment/config precedence, live reload,
      concurrent different credentials, and subagent construction before deleting the adapter.
