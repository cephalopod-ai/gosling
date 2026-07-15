# Gosling integration plan for future Tagteam

**Status:** Phase 1 implemented and locally validated; Phase 2 is in progress
behind the disabled feature; Phases 3-4 remain gated and deferred.
**Plan date:** 2026-07-12.
**Target baseline:** Gosling branch `codex/external-skill-catalog` at
`c896d778f`; clean except for the related untracked `docs/TODO.md` ledger.
**Producer state:** Tagteam is actively changing through a debug-use and repair
loop. Its versioned MCP control-plane contract is now consumed by an isolated
Gosling Unix-socket adapter, but real-daemon compatibility remains a release
gate rather than an assumed guarantee.
**Authority:** Plan and Gosling Phase 1 only. This plan does not authorize
changes in Tagteam or activation of incomplete Gosling UI.

## 1. Intent

Make Gosling a first-class consumer of a future, versioned Tagteam MCP control
plane while preserving a strict split between:

- the **deterministic controller**, which validates and executes Tagteam
  actions;
- the **Run Steward**, which is a normal selectable Gosling provider/model that
  monitors normalized evidence and communicates with the user; and
- the **Tagteam roles**, whose worker, coder, reviewer, supervisor, and scout
  models are selected separately and remain owned by Tagteam's contract.

The feature belongs under a Tagteam orchestration capability, not under the
provider capability. The current `tagteam` provider remains a compatibility
adapter until the new workflow has runtime parity and migration evidence.

## 2. Constraints and non-goals

- Phase 1 must not call the changing Tagteam binary or assume an unpublished
  MCP tool schema.
- Gosling must not duplicate Tagteam profiles, model catalogs, reason codes,
  mode-role validation, or recovery rules.
- No generic external-process abstraction, arbitrary shell wrapper, raw flag
  passthrough, or unrestricted artifact reader is introduced.
- No visible Desktop, text UI, slash command, or CLI workflow selector ships
  before a real backend passes its contract tests.
- The Run Steward is not an execution fallback. Missing or invalid steward
  output degrades to deterministic status text and never changes a run.
- Existing Gosling sessions remain `Standard` by default, and all schema
  changes are additive and backward compatible.
- Existing Tagteam provider behavior is preserved until a separate, verified
  deprecation stage.

## 3. Target architecture

```text
Desktop / text UI / CLI
        |
        v
Gosling TagteamWorkflowService
        |-- TagteamClient port ---------------- future Tagteam MCP endpoint
        |-- TagteamRunStore -------------------- SQLite projection/binding
        |-- TagteamEventReducer ---------------- normalized deterministic state
        |-- StewardCapabilityPolicy ------------ restricted tool inventory
        `-- Run Steward provider/model --------- advisory summaries only
```

Dependency direction is inward: UI and transports depend on the workflow
service; the workflow service depends on a Gosling-owned port; the future MCP
adapter implements that port. Tagteam transport types do not leak into session,
provider, or UI modules.

## 4. Stable Gosling-side contracts

Phase 1 defines an internal, versioned neutral model that a future MCP adapter
can map onto without making its wire schema Gosling's domain model.

### 4.1 Session and launch types

```text
SessionWorkflow
  Standard
  Tagteam

TagteamLaunchSpecV1
  repository identity and canonical root
  prompt reference/content
  mode-specific TeamSpec
  explicit allowed paths
  rounds and bounded time budgets
  approved test preset reference
  recovery policy = Assist
```

`TeamSpec` is an enum with mode-specific variants. Invalid combinations such as
a scout in supervisor mode or a reviewer in solo mode should not be
representable after validation. Adapter/model identifiers remain opaque strings
until the producer returns capability metadata.

### 4.2 Client port

The internal `TagteamClient` port has outcome-level methods corresponding to
capabilities, not command strings:

```text
capabilities
validate_launch
start
status
plan
findings
prepare_resume
resume
cancel
diagnostics
```

Every response carries a producer schema version, run identity where
applicable, result completeness, and typed retryability. Unknown major versions
fail closed; compatible minor additions are ignored unless marked required.

The Phase 2 `TagteamControlClient` is intentionally separate from the Phase 1
`TagteamClient` port. The earlier port carries a draft approval abstraction;
the live producer requires a prepared action digest and approval record. A
workflow service will converge those boundaries only after it can persist the
producer's approval and run-binding semantics without translating or
recomputing authority-bearing fields.

### 4.3 Normalized state

`TagteamEventReducer` produces a bounded `TagteamRunSnapshot` from ordered
producer observations. It owns legal transitions, deduplication, stale-update
rejection, terminal-state precedence, and deterministic fallback messages.

Minimum state classes:

```text
configured -> validating -> ready -> starting -> running
running -> waiting | implementing | reviewing | testing | recovering
active -> passed | degraded | blocked | failed | quarantined | cancelled
```

The producer remains authoritative for its detailed phase and reason codes.
Gosling maps them to presentation classes without inventing success or recovery
eligibility.

### 4.4 Persistence

Use additive storage rather than putting volatile orchestration fields directly
on `Session`:

- add `workflow_kind` to sessions, defaulting to `standard`;
- add `tagteam_run_bindings` keyed by Gosling session and launch generation;
- persist the versioned launch spec, Tagteam run ID/run directory/state root
  when known, sanitized action digest, last producer sequence, last valid
  snapshot, and terminal/recovery class;
- do not treat a stored PID as proof of process ownership;
- cap retained completed bindings per session and delete them with the session.

Raw transcripts, private reasoning, secrets, and unbounded event streams do not
belong in this store.

## 5. Phase 1 - cleared Gosling foundation

Phase 1 has no live Tagteam dependency and no user-visible activation. Each
stage is independently reviewable and reversible.

### Stage 1.1 - architecture ownership

- Add an ADR for Tagteam-as-workflow and the controller/steward authority split.
- Register `orchestration.tagteam_workflow` in `.architecture/components.yaml`.
- Add invariants that Tagteam is not modeled as a provider, the workflow has
  one lifecycle owner, and the steward cannot inherit arbitrary mutation tools.
- Verification: registry syntax check and architecture review against ARC-001
  through ARC-004.
- Rollback: documentation-only removal before code depends on the declarations.
- Estimate: S; architectural risk Low.

### Stage 1.2 - domain contracts

- Add the versioned workflow, launch, team, capability, observation, snapshot,
  error, completeness, and retryability types in a cohesive Tagteam module.
- Keep MCP/wire DTOs out of this module.
- Verification: serialization round trips, unknown-version rejection,
  mode-role validation, bounds, canonical path, and redaction tests.
- Rollback: remove the isolated module; no session behavior changes yet.
- Estimate: M; architectural risk Medium.

### Stage 1.3 - additive persistence

- Add the next SQLite migration for `workflow_kind` and
  `tagteam_run_bindings`.
- Extend `Session` and update builders with a default-safe workflow value.
- Implement compare-and-set updates for launch generation and producer event
  sequence so stale observers cannot overwrite newer state.
- Verification: fresh database, upgrade database, standard-session regression,
  cascade deletion, stale-write rejection, retention, and export/import policy
  tests.
- Rollback: old binaries ignore the additive table/column; the feature flag
  remains off and every existing row remains `standard`.
- Estimate: M; architectural risk Medium.

### Stage 1.4 - event reducer and deterministic reporting

- Implement the pure reducer and deterministic fallback report renderer.
- Reject out-of-order events, illegal terminal regressions, malformed required
  fields, and success claims without authoritative terminal evidence.
- Bound changed paths, findings summaries, diagnostics, and rendered text.
- Verification: table-driven transition tests, replay determinism, malformed
  event fixtures, truncation markers, and byte-identical repeated reductions.
- Rollback: isolated pure logic with no active runtime caller.
- Estimate: M; architectural risk High because false status is the core hazard.

### Stage 1.5 - steward capability policy

- Define a Tagteam Run Steward capability profile as deterministic policy data.
- Allow only status, plan, findings, diagnostics, and prepare-resume reads in
  Phase 1 fixtures. Reserve start, resume, and cancel as approval-bound actions
  for later phases.
- Explicitly deny Developer, shell/edit, extension management, subagents,
  unrelated MCP tools, scope changes, finding deferral, transfer, and recursive
  Tagteam invocation.
- Verification: exact tool-inventory golden tests and adversarial policy tests
  proving inherited/default extensions cannot reappear.
- Rollback: remove the unused policy module before activation.
- Estimate: S; security risk High.

### Stage 1.6 - consumer contract fixture harness

- Define a `TagteamClient` test contract and an in-memory fixture implementation
  used only by tests.
- Cover capabilities, valid/invalid launch, immediate run-handle return,
  progress, findings pagination, degraded and terminal states, resumable and
  unsafe recovery, cancellation ownership, malformed responses, and unknown
  schema versions.
- Record fixtures as Gosling consumer expectations, clearly labeled draft until
  Tagteam publishes matching producer fixtures.
- Do not add a production mock, canned-success command, or visible UI.
- Verification: every client implementation must pass the same contract suite;
  Phase 2 cannot merge an MCP adapter that bypasses it.
- Rollback: fixture-only code can change with the producer contract without a
  user migration.
- Estimate: M; compatibility risk Medium.

### Stage 1.7 - feature gate and handoff

- Add a compile/runtime feature gate that remains disabled and undiscoverable
  in production surfaces until Phase 2 passes.
- Document the draft consumer contract and create a producer handoff checklist.
- Verification: normal Gosling provider/model/session behavior is byte- and
  behavior-equivalent with the gate disabled; no new UI control or advertised
  command exists.
- Rollback: disable or remove the gate without touching persisted standard
  sessions.
- Estimate: S; release risk Low.

## 6. Phase 1 completion gate

Phase 1 is complete only when:

- architecture ownership and invariants are committed;
- all internal types are versioned and bounded;
- fresh and upgraded databases preserve existing sessions;
- reducer replay is deterministic and cannot regress a terminal state;
- the steward capability inventory contains no repository mutation path;
- the consumer contract suite covers every planned MCP capability and failure
  class;
- the feature remains invisible and inactive in production builds; and
- `cargo fmt`, targeted Gosling tests, migration tests, `cargo test -p gosling`,
  and applicable clippy checks pass under the repository's normal toolchain.

No real Tagteam run is required or permitted by this phase.

### Implementation status, 2026-07-12

- Added the disabled `tagteam-workflow` Cargo feature, architecture ownership,
  versioned bounded domain contracts, a test-only client fixture, deterministic
  reducer/reporting, a restrictive steward policy, and additive SQLite
  persistence with generation and sequence compare-and-set behavior.
- Existing sessions default to `standard`. Import and copy deliberately reset
  sessions to `standard` so a Tagteam workflow cannot be detached from its
  run ownership. Live bindings cascade with session deletion, while durable
  launch and producer identities remain as replay-prevention tombstones.
- No Desktop, text UI, CLI, slash command, production mock, external process,
  or live MCP adapter was added.
- CI now compiles, tests, and lints the dormant feature explicitly in addition
  to the default workspace checks. This section records the intended
  gate; current validation results belong in the pull request rather than a
  durable design report.
- Phase 2 must authenticate producer observations before calling the
  crate-private persistence boundary and must use no-follow/openat-style file
  access to close the remaining filesystem race between approval and execution.

## 7. Producer gate for Phase 2

Live integration begins only after Tagteam supplies or stabilizes:

- an MCP protocol/capability version and machine-readable capability response;
- strict input and output schemas for every enabled operation;
- immediate run-handle semantics for start and resume;
- bounded status, plan, findings, and diagnostics responses with explicit
  completeness/truncation markers;
- stable run identity, event ordering or snapshot freshness rules, terminal
  precedence, exit/reason mapping, and recovery eligibility;
- action-bound approval inputs for start, resume, and cancel;
- cancellation ownership behavior, including the limit after server restart;
- sanitized golden fixtures for success and every material failure class; and
- a compatibility/deprecation policy for schema evolution.

Profile discovery and resolved role provenance may remain absent, but Gosling
must then require explicit role targets and must not invent a profile catalog.

## 8. Deferred phases

### Phase 2 - live MCP adapter and headless playtest

- Implement the production MCP adapter behind `TagteamClient`.
- Pin and verify the producer capability version at connection time.
- Run headless scratch-repository tests without Desktop or text UI activation.
- Exercise start, monitor, findings, deterministic terminal reporting,
  approval-bound resume/cancel, disconnect, and reconnect.
- Gate: contract parity, no duplicate launch, no orphan child owned by the
  current server, and no silent fallback to the legacy provider.

#### Implementation status, 2026-07-14

- Added the feature-gated `McpTagteamClient` Unix-socket adapter implementing
  the producer-conformant `TagteamControlClient`. It initializes only against
  MCP protocol `2025-11-25`, verifies the Tagteam server identity and required
  v1 capabilities, then records the daemon's canonical repository identity.
- The adapter exposes the published read, preparation, and lifecycle tools,
  consumes structured content only, validates response bounds and exact
  mode-role shapes, and forwards the producer-prepared approval record
  unchanged. It does not start a subprocess, select a model, or fall back to
  the legacy provider.
- Socket fixtures cover validation, prepared approval forwarding, status,
  structured terminal errors, malformed producer errors, invalid normalized
  role shapes, and reconnecting a fresh client to the same daemon fixture.
- An opt-in `live_tagteam_socket_smoke_test` connects the adapter to a real
  local daemon named by `TAGTEAM_MCP_SOCKET`. It passed on 2026-07-14 against a
  source-built Tagteam binary for initialization, capability verification, and
  diagnostics only; it deliberately does not launch a model-backed run.
- Still required before this phase can pass its gate: a real Tagteam daemon
  scratch-repository playtest, workflow-service lifecycle/persistence wiring,
  duplicate-launch and cancellation ownership checks, real restart recovery,
  and deterministic reporting through the existing reducer. There is no
  Desktop, text UI, CLI, slash command, or Run Steward activation in this
  implementation.

### Phase 3 - first-class Gosling workflow and Run Steward

- Add Workflow, Run Steward, Tagteam Team, and Execution setup surfaces.
- Reuse normal provider/model/thinking-effort selection for the steward.
- Add live Desktop and text UI projections over the shared workflow service.
- Trigger the initial run from the user's bound Run action, not model tool
  selection.
- Invoke the steward only on material events and fall back deterministically.
- Validate local Ollama, mid-tier, and frontier steward tiers against the same
  evidence fixtures.
- Gate: end-to-end role clarity, accessibility, bounded rendering, restart
  behavior, approval integrity, and real runs in every supported Tagteam mode.

### Phase 4 - durable daemon and fleet view

- Connect to durable Tagteam process ownership with leases, reconnectable event
  streams, safe cross-restart cancellation, and one observer lease per run.
- Add local-first steward escalation and fleet status without centralizing
  repository contents, secrets, prompts, or private reasoning.
- Consider other external orchestrators only after Tagteam demonstrates a
  stable boundary; never generalize into an arbitrary process launcher.

## 9. Risks and tradeoffs

| Risk | Mitigation | Tradeoff |
| --- | --- | --- |
| Tagteam contract changes during its debug loop | Phase 1 depends only on a Gosling port and draft fixtures; live adapter is gated | Some fixture adjustment is expected before Phase 2 |
| Internal contract diverges from producer | One conformance suite applies to fixtures and the future adapter; Tagteam fixtures become the authority | Gosling keeps an anti-corruption mapping layer |
| Persistence added before activation | Additive schema, standard default, feature off, migration tests | Small dormant storage surface exists temporarily |
| Weak steward invents status or actions | Deterministic reducer is authoritative; restricted tools and schema-validated advisory output | Steward may provide less narrative detail |
| Existing provider is removed too soon | Preserve and label it as compatibility until Phase 3 parity | Two paths coexist temporarily |
| Generic orchestration abstraction expands authority | Keep every Phase 1 type and policy Tagteam-specific | A later orchestrator needs a deliberate new design |

## 10. Verification handoff

After Phase 1 implementation, an independent architecture/security review
should verify:

- dependency direction and single lifecycle ownership;
- no provider-layer or UI-layer execution logic;
- no arbitrary shell, path, MCP, or approval authority in the steward profile;
- session migration and stale-update/CAS behavior;
- hostile producer content is always data, never instructions;
- deterministic incomplete/degraded/error reporting; and
- feature-gate truth: no fake or discoverable workflow exists before the live
  adapter is runtime-verified.
