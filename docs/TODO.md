# TODO

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

## Defect-repair campaign — 2026-07-16

Full inventory, skill disposition, and repair log:
[`reports/2026-07-16-defect-audit-and-repair.md`](../reports/2026-07-16-defect-audit-and-repair.md).
42 defects found across 12 audit lenses, grouped into locality-based repair
stages. Follow-up campaign (branch
`claude/gosling-defect-repair-followup-6093`, pushed, PR not yet opened)
repaired 19 of the remaining 21 under `repair-defect-campaign` gates across
13 stages; 2 (ORCH-002, REC-001) are dispositioned as needing dedicated
follow-up work with reasons, not silently dropped. A real regression
introduced mid-campaign (the LLM-002 fix over-broadly removing a fast path)
was caught by the campaign's own Gate 9 full-suite verification and fixed
before closeout. Combined with the three passes already on `main`: 41 of 42
original defects now repaired or honestly dispositioned. Track per-stage
status in that report rather than duplicating it here.

Corroborates two previously-deferred, still-open findings from
`reports/2026-07-10-audit-skills-pack-report.md`: the `/status` static-200
health lie (there: FSR-SRV-001, here: OPS-001 — repaired in this pass) and
the hardcoded `exit_type="normal"` telemetry (there: FSR-SRV-002, here:
OPS-003 — not yet repaired, carried into the follow-up backlog). Correction:
this session's sandbox cannot build `gosling-server` either (`cargo build -p
gosling-server` fails downloading `v8-goose`'s prebuilt V8 binary from a
blocked GitHub-releases host) — the underlying `gosling` crate change
(`SessionManager::healthy()`) is compiled and tested, but the
`gosling-server` route handlers themselves are
unverified by `cargo build`/`test`/`clippy` in this environment. Recommend
CI confirm both before merge.
