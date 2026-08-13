# Pre-GUI backend implementation plan

Mode: P3 — architectural planning
Date: 2026-08-13
Baseline: `main` at `e5af0f640007f78f4782617dcae1553936dc627e`
Status: execution companion for R1–R4 of the
[project-shell readiness plan](project-shell-readiness-plan.md)
Authority: documentation plan only; no runtime, package, release, or named-shell change

## 1. Intent and authority

The maintainer-supplied intent is to complete and rigorously prove the shell backend before
creating shared or project-specific GUI. This plan turns that intent into bounded implementation
work for R1–R4. It does not replace the parent R0–R8 plan, change its milestone meanings, or claim
that any planned backend capability already works.

The feature belongs to Gosling's declared project-shell consumer capability:

```text
lighter shared Gosling runtime
  -> independently owned project-shell consumers
     -> copy-free consumer composition
     -> safe main-owned application runtime
     -> live versioned domain integration
     -> shared shell GUI only after backend proof
```

Intent basis:

- **Maintainer supplied:** backend architecture and behavior must be proven before GUI work begins.
- **Declared:** the shell foundation, readiness reassessment, R0–R8 plan, traceability matrix,
  risk register, and accepted ADR-0007–0009.
- **Observed:** the renderer is lifecycle-only, the preload has no prompt/update/domain API,
  focused ACP callbacks discard interactive behavior, and production CLI construction registers no
  domain adapter.
- **Unresolved, not inferred as decided:** separate-repository versus isolated-workspace consumer
  composition and the exact out-of-process adapter protocol remain R1 decisions.

## 2. Relationship to the parent milestones

| Parent gate | This plan owns | GUI rule |
| --- | --- | --- |
| R0 | Retain the green baseline and reverify it at each gate | No GUI work |
| R1 | Freeze consumer, application-runtime, and adapter contracts | No GUI work |
| R2 | Implement copy-free composition and least-privilege package inputs | Neutral conformance renderer only |
| R3 | Implement and prove the main-owned agent application runtime | Non-visual test harness only |
| R4 | Implement and prove a live neutral domain adapter | Non-visual payload fixture only |
| R5 | Build the shared shell application kit | May start only after this plan's exit gate |
| M5 and later | Begin named project-shell implementation | DAWES, math, Physics/CST, and other named GUI remain blocked until M5 |

Completing this plan means **ready to begin shared GUI implementation**, not project-shell consumer
ready, packaged-distribution ready, or release ready. Those claims still require R5–R8.

## 3. Current baseline and retained assets

Retain rather than replace:

- server-fixed shell identity, provisioning, policy, and runtime namespace;
- strict secret-free product profile and collision/path validation;
- dedicated Electron main, preload, and renderer entries;
- main-owned child generation, authenticated ACP preflight, and compatibility ordering;
- session create/resume after successful preflight;
- lifecycle generation ownership, diagnostics, cleanup, and explicit handoff;
- generated Rust/SDK contracts and existing full-Desktop behavior;
- neutral, permanently non-publishable fixture profiles;
- current CI baseline, including Linux Rust, Desktop, Clippy, format, MSRV, schema/SDK, and
  Windows build jobs.

Known blocking facts:

1. `shell.html` and the Vite renderer config select one hard-coded renderer.
2. `GoslingShellAPI` exposes lifecycle, diagnostics, handoff, and external-open only.
3. `ShellAcpConnection` creates/resumes sessions but cannot prompt, cancel, or deliver updates.
4. Permission requests are cancelled, elicitations declined, and session updates discarded.
5. The lifecycle vocabulary contains states with no production producer or recovery proof.
6. `runtime.read` returns lifecycle only, not a verified application snapshot.
7. `DomainAdapter` exists as a Rust trait/DTO seam, but production constructs
   `ShellRuntime::new(..., None)`.
8. Shell packages still inherit broad Gosling resources and platform metadata.
9. No named DAWES, math, Physics/CST, or other project implementation exists in this repository.

## 4. Scope and non-goals

### In scope

- ADR-0010–0012 and the strict consumer, runtime, event, permission, elicitation, adapter, and
  capability-negotiation contracts;
- a consumer-owned renderer entry selected without replacing host main/preload authority;
- exact-resource and project-neutral package inputs required to make the consumer seam truthful;
- main-owned session create/resume/prompt/cancel/update behavior;
- explicit, stale-safe permission and elicitation mediation behind the typed boundary;
- a safe verified runtime snapshot and bounded event stream;
- truthful lifecycle production, recovery, diagnostics, and reconnect behavior;
- a live versioned neutral adapter with snapshot and explicitly confirmed mutation;
- compatibility, negative-space, failure, cleanup, and headless end-to-end evidence;
- documentation, traceability, risk, defect, and gate evidence updates.

### Not in scope

- `ShellRuntimeProvider`, `ShellHostApp`, conversation components, recovery screens, or other shared
  GUI;
- DAWES, math, Physics/CST, Project ABC, or other named profile, prompt, adapter, domain payload,
  renderer, workflow, brand, or asset;
- production identifiers, release destinations, signing, notarization, publication, updater
  activation, or public artifacts;
- a second settings, credential, provider, permission, session, or domain-authorization authority;
- arbitrary renderer plugins, Electron main/preload replacement, native library loading, shell
  commands, or consumer installation hooks;
- full packaged failure/coexistence and cross-platform distribution acceptance, which remain R6–R8.

## 5. Pre-GUI completion definition

All conditions below must be true on one exact revision before R5 shared GUI work begins:

1. ADR-0010–0012 are accepted and no P0 ownership/topology decision remains open.
2. A strict versioned consumer manifest selects an independently owned neutral renderer without
   host source changes or main/preload replacement.
3. Two structurally different neutral consumers build through the same host contract.
4. Main owns ACP and exposes bounded session create/resume/prompt/cancel/update operations without
   exposing the ACP endpoint, token, child process, raw logs, configuration, or private paths.
5. Permission and elicitation requests are explicit, action-bound, generation/session fenced,
   stale-safe, and never silently approved or discarded.
6. Every lifecycle state has a tested production producer and recovery action or has been removed.
7. The renderer-facing snapshot contains only verified, bounded identity, compatibility,
   provisioning, session, prompt, pending-interaction, and adapter-capability facts.
8. A live neutral adapter is registered, version-negotiated, bounded, supervised, and proves a
   read-only snapshot plus one explicitly confirmed mutation.
9. Adapter absence, mismatch, crash, hang, malformed output, overproduction, replay, and stale action
   cannot produce false `ready` or leave an orphan.
10. Shell resources and platform metadata needed by R2 are exact, consumer-derived, and read back;
    broad Gosling-only resources are absent.
11. A non-visual conformance harness completes the prompt/update/permission and adapter
    snapshot/action workflows, including cancel, reconnect, and negative paths.
12. Required local checks and current CI are green; evidence is revision-bound and partial results
    are not rounded up.

## 6. Requirements coverage

This plan does not renumber requirements. It gives every pre-GUI requirement an execution owner and
keeps post-GUI requirements visible so they cannot be accidentally claimed early.

| Requirement group | IDs | Pre-GUI disposition |
| --- | --- | --- |
| Retained profile/authority/host baseline | SHP-REQ-001–003, 009–010, 012, 014–018, 021–023 | Preserve and regression-test; do not rebuild by copy |
| Renderer trust boundary and main-owned application service | SHP-REQ-004, 006, 018, 034–035, 038, 042 | Freeze in R1; implement and prove in R3 |
| Truthful lifecycle and backend recovery | SHP-REQ-005, 008, 015, 028, 037, 042 | Implement producer/action/diagnostic mapping in R3; retain packaged closure for R6 |
| Backend-enforced selections and denial | SHP-REQ-007, 010, 018 | Preserve server authority; exercise through R3/R4 harness |
| Consumer composition | SHP-REQ-017–019, 022–023, 033, 039, 041–042 | Freeze in R1; implement and prove with neutral consumers in R2 |
| Domain integration | SHP-REQ-002, 006–007, 010, 015, 018, 036, 042 | Freeze in R1; implement and prove with neutral adapter in R4 |
| Backend diagnostics and handoff operations | SHP-REQ-015–016, 024 | Preserve main operations in R3; presentation remains R5 |
| Baseline/final quality | SHP-REQ-014, 029, 043 | Reverify per gate; cross-platform workflow closure remains R7/R8 |
| Explicit post-GUI/package acceptance | SHP-REQ-011, 013, 020, 025–027, 040 | Do not claim in this plan; execute at R5–R8 |
| Optional post-P1 capabilities | SHP-REQ-030–032 | Deferred; no pre-GUI dependency |

## 7. Architectural impact

### Affected subsystems

| Subsystem | Existing responsibility | Planned impact | Must not absorb |
| --- | --- | --- | --- |
| Rust shell SDK types | Canonical provisioning, identity, handoff, domain envelopes | Versioned capability/adapter contract changes selected by R1 | Renderer state or project payload semantics |
| Rust shell runtime/server | Policy, session authority, custom-method dispatch | Live adapter registration, validation, action authorization, bounded failure mapping | Project domain implementation |
| Gosling CLI serve construction | Fixed runtime identity/provisioning | Construct the selected generic adapter supervisor/transport | Named adapters or dynamic native libraries |
| Electron shell ACP adapter | Authenticated preflight and session create/resume | Prompt/cancel/update plus focused permission/elicitation controllers | UI presentation or raw ACP exposure |
| Electron runtime controller | Child/ACP generation and lifecycle | One active session, prompt attempt, reconnect, pending interaction, truthful state producers | Domain payload interpretation |
| Shell IPC/preload | Frozen capability bridge | R1-approved bounded application operations and events | Generic RPC, Node, filesystem, process, settings, or updater authority |
| Product/consumer resolver | Product profile and package identity | Strict consumer manifest, entry containment, compatibility hash | Runtime provisioning, secrets, commands, arbitrary URLs |
| Vite/Forge/package verifier | Fixed host entries and artifact projection | Consumer-selected renderer only; exact resources and metadata readback | Consumer main/preload replacement or product conditionals |
| Neutral fixtures | Identity/package tests | Two non-domain consumer layouts and one neutral adapter | Scientific, mathematical, coding, or named-project behavior |
| Evidence/docs | Gate state and revision proof | Per-work-package checkpoints, risk/defect/traceability updates | Evergreen claims detached from a revision |

### Required interfaces and owners

Exact field names and file placement are R1 deliverables; implementation must not invent them before
the ADRs are accepted.

| Interface | Owner | Required semantics |
| --- | --- | --- |
| Consumer manifest v1 | Product/consumer resolver | Strict schema, canonical hash, approved renderer root, declared capabilities, compatibility, no executable authority |
| Capability negotiation | Rust/SDK plus main ACP adapter | Runtime reports live methods/adapter capabilities; consumer declares only what it uses; mismatches fail before `ready` |
| Safe runtime snapshot | Electron main runtime | Immutable bounded verified facts; no credentials, ACP authority, raw config, private paths, or unrestricted history |
| Runtime event stream | Electron main runtime | Generation/session/prompt-attempt ordering, bounded payloads, stale rejection, reconnect semantics |
| Session application operations | Electron main ACP adapter | Create/resume, bounded prompt, cancel, and compacted resume under server-owned policy |
| Permission/elicitation mediation | Focused main controllers | Opaque action IDs, explicit responses, expiry/staleness, duplicate rejection, visible cancellation semantics |
| Domain adapter transport | Rust runtime/CLI | Versioned descriptor, lifecycle, bounds, timeout, capability set, crash cleanup, no native library loading |
| Domain snapshot/action | Rust server authority | Read-only snapshot and allowlisted actions; mutation requires action-bound confirmation |
| Consumer payload boundary | Consumer renderer package | Generated project-owned schema; shared host treats native payload as opaque and bounded |

### Data model and migration

- Reuse the existing server-owned session store; do not create a renderer or consumer session store.
- Keep pending prompt, permission, elicitation, and confirmation records bounded and runtime-owned.
  If R1 decides any must survive restart, it must identify the existing backend persistence owner,
  define schema/version/migration/cleanup behavior, and add rollback tests before implementation.
- The neutral adapter uses isolated temporary fixture state only. It cannot establish a production
  project data model.
- Consumer/profile/adapter compatibility versions are contract data embedded in manifests and
  runtime metadata; unknown major versions fail closed.
- No migration may run before compatibility and provisioning validation complete.

### Security implications

New untrusted inputs are the consumer manifest, renderer IPC requests, ACP updates, permission and
elicitation responses, adapter descriptors/messages, native project payloads, confirmation tokens,
and package resources. Every input requires exact schema validation, byte/count/time bounds,
generation/session/action fencing where applicable, redacted error mapping, and negative tests.

Trust boundaries remain:

```text
consumer renderer (untrusted presentation)
  -> narrow preload and validated IPC
Electron main (ACP/process owner)
  -> authenticated loopback ACP
Rust server (policy/session/action authority)
  -> versioned supervised adapter transport
project adapter (domain semantics, separately owned)
```

The plan requests no exception to the existing authority boundaries.

## 8. Plan invariants

These are plan-level acceptance rules derived from existing ADRs and requirements. They are not a
new `.architecture/` registry and this plan does not self-certify conformance.

| ID | Invariant | Evidence source | Disposition |
| --- | --- | --- | --- |
| PG-INV-001 | Renderer never receives ACP URL/token, process handles, raw logs, configuration authority, or arbitrary filesystem access | Preload/API surface snapshots and hostile IPC tests | Must conform |
| PG-INV-002 | Rust/backend remains settings, credential, provider, policy, session, and mutation authority | Dependency review and server integration tests | Must conform |
| PG-INV-003 | Consumer may supply renderer/domain content but may not replace main/preload or execute arbitrary build hooks | Manifest resolver and bundle inspection | Must conform |
| PG-INV-004 | Declared capability equals a live negotiated capability; descriptor-only success is forbidden | Compatibility and adapter conformance matrix | Must conform |
| PG-INV-005 | Every runtime state/action has a production source and tested effect or is removed | Reachability table and failure tests | Must conform |
| PG-INV-006 | Permission, elicitation, and mutation decisions are explicit, action-bound, stale-safe, and non-replayable | Controller and integration tests | Must conform |
| PG-INV-007 | Shared host treats project payloads as bounded opaque data and contains no named domain semantics | Dependency and negative-space audit | Must conform |
| PG-INV-008 | Package contains only exact declared resources and project-neutral metadata | Verifier readback and injection tests | Must conform |
| PG-INV-009 | A failed preflight, compatibility check, or required-adapter negotiation creates no session and reports no false `ready` | Live no-session/failure evidence | Must conform |
| PG-INV-010 | Every gate claim is revision-bound and distinguishes planned, built, and verified | Traceability and evidence review | Must conform |

## 9. Dependency-aware implementation sequence

Every work package is independently reviewable and must update `build-state.md`, the traceability
matrix, affected risks/defects, and revision-bound evidence before the next dependent package starts.

### PG-00 — Reorient and freeze the working baseline

Parent gate: R1 prerequisite

What:

1. Re-read current source, ADRs, plan, risks, defects, and recent shell session logs.
2. Record exact branch/revision, current CI, dirty state, and current shell-focused test evidence.
3. Confirm no newer consumer/runtime/adapter seam supersedes the reassessment.
4. Open a plan-change record before deviating from R1–R4 boundaries or requirement mappings.

Why here: prevents implementation against stale file paths or already-changed contracts.

Verification: source/status/document claims agree; current required CI has no unexplained red job;
the worktree scope is explicit.

Rollback: documentation-only checkpoint; discard the checkpoint if it cites the wrong revision.

### PG-10 — Accept R1 architecture and contracts

Parent gate: R1
Requirements: SHP-REQ-004, 006, 018, 033–038, 042

Work packages:

1. **PG-11 — Consumer topology:** ADR-0010 compares separate-repository and isolated-workspace
   consumers, selects one, defines version distribution, private-consumer support, update flow,
   generated types, renderer-root containment, and immutable host ownership.
2. **PG-12 — Application runtime boundary:** ADR-0011 defines the safe snapshot, operations,
   events, one-active-session rule, prompt-attempt ordering, permission/elicitation mediation,
   reconnect, compacted resume, lifecycle reachability, payload bounds, and forbidden authority.
3. **PG-13 — Adapter topology:** ADR-0012 selects transport/registration, process ownership,
   version/capability negotiation, timeout/cleanup, snapshot/action authority, confirmation tokens,
   payload ownership, and disposition of the current in-process `DomainAdapter` trait.
4. **PG-14 — Schemas and threat model:** freeze consumer manifest v1, compatibility inputs, safe
   snapshot, runtime operations/events, interaction records, adapter descriptor, action confirmation,
   error taxonomy, and negative-space rules using canonical/generated types where they exist.
5. **PG-15 — Traceability review:** map every future R2–R4 module/test/evidence target to a
   requirement and plan invariant; reject any interface with no owner or verification source.

Why here: Vite, IPC, session, and adapter code would otherwise make unreviewed topology decisions
irreversible through usage.

Verification:

- all three ADRs are accepted and indexed;
- schemas use exact versions, owners, bounds, and failure behavior;
- proposed operations have forward and reverse traceability;
- no P0 question remains about renderer ownership, ACP control, permission authority, or adapter
  lifecycle;
- architecture review confirms full Gosling compatibility and no second authority/store.

Rollback: ADRs and schemas land before implementations; if review rejects the topology, revise them
without compatibility shims or partial runtime code.

Gate evidence: create R1 evidence only after review acceptance. Draft ADRs are not R1 completion.

### PG-20 — Implement copy-free consumer composition

Parent gate: R2
Requirements: SHP-REQ-001–002, 009, 012, 017–019, 021–023, 033, 039, 041–042
Dependency: PG-10 complete

Work packages:

1. **PG-21 — Consumer resolver:** implement the strict R1 manifest parser, canonical hash,
   compatibility validation, approved-root containment, secret/command/URL/native-library rejection,
   and deterministic manifest projection.
2. **PG-22 — Build composition:** make Vite/Forge select only the resolved consumer renderer while
   preserving fixed host main/preload entries and unchanged full-Gosling defaults.
3. **PG-23 — Independent neutral consumers:** move the current neutral renderer into a
   consumer-owned fixture and add a structurally different second neutral consumer. Both remain
   minimal, non-domain, non-publishable conformance surfaces.
4. **PG-24 — Exact resources and metadata:** stage only the required Gosling binary and declared
   resources; remove or derive inherited document types, TCC text, Linux fields, Flatpak permissions,
   entitlements, homepage/category, and other Gosling-only package metadata.
5. **PG-25 — Verifier expansion:** read back consumer hash, renderer entry, fixed host entries,
   exact resource inventory, CSP, metadata, permissions, entitlements, updater absence, and tamper
   resistance.

Why here: R3 needs a real consumer boundary for its non-visual harness, but composition must not be
allowed to widen runtime authority.

Verification:

- two neutral consumers resolve and build without host source edits;
- host main/preload sources or bundles are identical across consumers;
- hostile manifest, path, symlink, entry, import, extra-resource, and metadata fixtures fail closed;
- full Gosling Forge behavior remains unchanged;
- exact package resources and platform metadata read back for the supported host target;
- source-content audit finds no named domain nouns or publish controls in neutral fixtures.

Rollback: retain the existing fixed renderer path until one full consumer path passes; remove the
temporary compatibility branch once both fixtures pass rather than maintaining dual composition
authorities.

Gate evidence: R2 is not complete merely because a third profile validates; the renderer must be an
independent resolved input with unchanged host authority.

### PG-30 — Implement the main-owned application runtime

Parent gate: R3
Requirements: SHP-REQ-003–010, 015–016, 018, 024, 028, 034–035, 037–038, 042
Dependencies: PG-10 complete; PG-22 available for end-to-end harness

Work packages:

1. **PG-31 — Focused ACP callbacks:** replace automatic permission cancellation, elicitation
   decline, and discarded updates with pure bounded controllers. Reuse full-Desktop pure adapters
   only after a dependency review proves they do not import broad global/Desktop authority.
2. **PG-32 — Session controller:** own one active session, create/resume ordering, prompt attempt
   IDs, bounded prompt submit, cancel, compacted resume, reconnect, and close behavior in Electron
   main.
3. **PG-33 — Safe snapshot and event reducer:** derive verified identity, compatibility,
   provisioning issue codes, session/prompt phase, pending interaction summaries, and negotiated
   adapter facts; fence every event by generation/session/attempt as applicable.
4. **PG-34 — Narrow IPC/preload operations:** expose only the R1-approved application operations;
   validate sender, exact fields, sizes, lifecycle state, capability, generation, session, and opaque
   action nonce. Preserve the forbidden-surface snapshot.
5. **PG-35 — Truthful lifecycle:** map startup, prompt, permission, domain, disconnect, retry,
   cleanup, and invariant failures to real states. Remove states/actions with no producer. Record
   stale and illegal events in bounded diagnostics.
6. **PG-36 — Reconnect and cleanup:** define transport-loss behavior during create, prompt,
   interaction, and idle; prevent duplicate prompt/action dispatch; prove bounded graceful and
   forced cleanup without orphaned child or registry state.
7. **PG-37 — Non-visual application conformance:** run a deterministic fixture-provider workflow
   through main and preload: create/resume, prompt, streamed updates, permission allow-once/deny,
   elicitation submit/cancel where supported, cancel, reconnect, compacted resume, and idle.

Why here: application behavior must be correct and observable before presentation components encode
assumptions about state or recovery.

Verification:

- deterministic main → child → ACP → session → prompt → updates → idle workflow passes;
- cancel, duplicate, stale, malformed, oversized, disconnect, restart, and compacted-resume paths
  have observed terminal outcomes;
- permission and elicitation requests are never silently approved or dropped;
- no failed preflight creates a durable session;
- no renderer snapshot/event contains ACP credentials, secrets, raw configuration, private paths,
  unrestricted history, or raw errors;
- every declared state/action is reachable in a production-backed test or removed;
- policy denial, extension/tool/skill selection, diagnostics, and handoff operations remain
  backend-enforced.

Rollback: keep new operations capability-negotiated and unavailable until their controller tests
pass; a failed slice can be disabled by capability removal without restoring automatic approval or
fake update handling.

Gate evidence: component tests cannot close R3. At least one live deterministic runtime path must
cross the narrow boundary without model credentials.

### PG-40 — Implement live neutral domain integration

Parent gate: R4
Requirements: SHP-REQ-002, 006–007, 010, 015, 018, 036, 042
Dependencies: PG-13 accepted; PG-32–PG-34 domain operations available

Work packages:

1. **PG-41 — Adapter supervisor/transport:** implement the selected generic out-of-process
   lifecycle with source-controlled descriptor, loopback/local transport as selected by ADR-0012,
   version handshake, bounded messages, deadlines, process ownership, cleanup, and redacted errors.
2. **PG-42 — Live registration and negotiation:** replace production's unconditional `None` path
   with generic validated registration; compare provisioning, consumer declaration, runtime
   descriptor, live actions, and schema versions before `ready`.
3. **PG-43 — Server-owned authorization:** route snapshot and actions through Rust/server policy;
   keep native payloads opaque to the shared host; ensure the descriptor cannot claim unavailable
   actions.
4. **PG-44 — Action-bound confirmation:** issue bounded single-use confirmation for the neutral
   mutation; reject replay, expiry, cross-action, cross-session, cross-generation, and stale tokens.
5. **PG-45 — Neutral adapter fixture:** provide one read-only snapshot and one confirmed mutation in
   isolated temporary state, with no network, credential, repository, coding, scientific,
   mathematical, or project-specific behavior.
6. **PG-46 — Failure matrix:** exercise required/optional absence, version/capability mismatch,
   crash before/after ready, hang, timeout, malformed output, excess bytes/resources, disconnect
   during action, cleanup escalation, and restart.
7. **PG-47 — Non-visual domain conformance:** an independent neutral consumer reads a live snapshot
   and completes one confirmed mutation through the negotiated runtime boundary.

Why here: the shared GUI's domain slot must bind to a real capability, not a descriptor, mock, or
dead Rust trait.

Verification:

- required adapter absence/mismatch prevents `ready` without creating false capability;
- live descriptor/action set and declared capability match exactly;
- snapshot/action/confirmation pass end to end through server authority;
- replay/stale/cross-action confirmation attempts fail;
- crash/hang/malformed/overproduction paths are bounded, diagnosable, and orphan-free;
- generic Gosling core contains no named domain implementation or arbitrary native plugin loader;
- full Gosling without a project adapter remains backward compatible.

Rollback: adapter capability is negotiated and fail-closed; rollback removes the generic adapter
registration/capability as one slice without weakening server policy or accepting descriptor-only
success.

Gate evidence: DTO serialization and test-only `DomainAdapter` implementations are insufficient;
the production construction path and supervised neutral process must be exercised.

### PG-50 — Pre-GUI backend acceptance

Parent transition: R4 complete → R5 eligible to start
Dependencies: PG-10, PG-20, PG-30, and PG-40 complete

What:

1. Run the combined non-visual consumer/runtime/adapter workflow twice clean and once across
   backend/adapter restart.
2. Run requirement forward/reverse traceability and the PG-INV-001–010 verification handoff.
3. Search shared source and fixtures for named domain semantics, raw ACP authority, broad IPC,
   arbitrary renderer Node access, duplicate schemas, dead adapter paths, fake/canned success,
   and unreferenced capabilities.
4. Reconcile traceability, risk, assumption, defect, evidence, architecture, and build-state docs.
5. Reverify required Rust, SDK/schema, Desktop, profile, package-readback, and current CI gates on
   the exact acceptance revision.
6. Record all residual R6–R8 package/platform/release limitations without converting them into R4
   blockers or hiding them.

Verification: every item in section 5 has revision-bound evidence, no critical/high pre-GUI
consumer/runtime/domain finding remains, and the operator accepts R5 shared GUI work.

Rollback: if acceptance fails, keep R5 blocked and reopen the owning R1–R4 work package. Do not
paper over a backend failure in presentation code.

## 10. Testing and evidence strategy

### Test layers

| Layer | Proves | Does not prove |
| --- | --- | --- |
| Pure contract/reducer tests | Exact schemas, bounds, transitions, stale/replay behavior | Real process/transport behavior |
| Main/ACP integration | Authentication, compatibility, session/prompt/update/interaction ordering | Packaged resource correctness |
| Spawned neutral adapter integration | Live negotiation, snapshot/action, timeout/crash/cleanup | Named domain correctness |
| Consumer build and package readback | Copy-free renderer input, fixed host, exact resources/metadata | Full installed workflow |
| Non-visual conformance workflow | Common backend works without GUI assumptions | Accessibility or user experience |
| Current CI | Repository-wide regression baseline | R6–R8 release/distribution acceptance |

### Planned validation commands

Use repository-documented commands and gate-specific focused tests. Exact new test selectors are
recorded when their files exist; this plan does not invent successful commands for absent tests.

```bash
source bin/activate-hermit
cargo fmt --all -- --check
cargo test -p gosling
cargo clippy --all-targets -- -D warnings
pnpm --dir ui/desktop run shell:test-profile
pnpm --dir ui/desktop run shell:check-profiles
pnpm --dir ui/desktop run typecheck
pnpm --dir ui/desktop test
```

Package/readback and spawned-process commands must use the existing scripts or commands introduced
and documented by the owning work package. A gate report records exact commands, revision, platform,
artifact hash where applicable, pass/fail counts, skipped coverage, and residual risks.

### Evidence rules

- `planned` means a contract or work item exists; it is not implemented.
- `built` requires merged source plus focused automated evidence.
- `verified` requires revision-specific live/process/package evidence appropriate to the requirement.
- Failed and blocked runs remain in the ledger; a later pass does not erase them.
- A retry-only pass is not closure for a flaky process test; root cause and deterministic repair are
  required.
- Each gate evidence file lists source revision, environment/platform, commands, artifacts,
  negative paths, failures, and residuals.

## 11. Risk-minimizing delivery and checkpoints

### Safe concurrency

After R1 acceptance:

- consumer manifest/resolver work may proceed alongside pure ACP notification/reducer extraction;
- generated contract work may proceed alongside neutral fixture design when schemas are frozen;
- documentation/traceability updates may proceed with a gate but may report only observed results.

Keep sequential:

- ADR/schema acceptance before Vite, IPC, session, or adapter implementation;
- consumer resolver before Vite/Forge selection;
- session/update controllers before preload application operations;
- adapter authority/lifecycle decision before transport or registration;
- application domain operations before adapter end-to-end conformance;
- all R1–R4 evidence before PG-50 acceptance.

Do not allow concurrent edits to the same Forge/Vite/profile-resolver or IPC/preload surfaces. The
shared mutable configuration and security boundaries make that merge risk larger than the latency
benefit.

### Durable checkpoints

Each PG work package is a resumable unit. At its boundary:

1. implementation and focused tests are complete or explicitly partial;
2. traceability rows and risks/defects are updated;
3. evidence records exact revision and commands;
4. `build-state.md` names the next uncompleted package;
5. rerunning the package begins by validating the checkpoint rather than repeating earlier work.

## 12. Recommendations, tradeoffs, complexity, and risk

Complexity estimates are relative engineering estimates based on the current source and existing
tests; they are not schedule commitments.

| Recommendation | Why | Expected benefit | Tradeoff | Incremental migration | Order | Complexity | Architectural risk |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Delay shared GUI until PG-50 | Prevent presentation from freezing accidental backend contracts | Less rework; every UI action maps to proven behavior | Delays visible product progress | Use non-visual fixtures through R1–R4, then add R5 UI | First/binding | M | Low if followed; high if bypassed |
| Prefer separate consumer ownership, subject to ADR-0010 | Matches independent project ownership and security-update flow | No Gosling source forks; clearer responsibility | Packaging/version-distribution overhead | Prove one isolated neutral consumer before external distribution | R1 then R2 | M | High until topology is accepted |
| Keep ACP in Electron main with bounded capability IPC | Preserves authenticated connection authority outside renderer | Smaller renderer attack surface and centralized ordering | More typed controller/IPC code | Add one capability slice at a time behind negotiation | R1 then R3 | L | Medium; IPC sprawl is the main risk |
| Prefer a versioned out-of-process adapter, subject to ADR-0012 | Avoids linking project code/ABI into generic Gosling | Isolation, independent updates, bounded lifecycle | Process/serialization latency and supervision cost | Prove neutral snapshot first, then confirmed mutation | R1 then R4 | L | High until crash/timeout/authority rules are proven |
| Start with one active session | Matches current focused-shell scope and reduces ordering ambiguity | Smaller state space for cancel/reconnect/permission proof | DAWES/math may later need comparisons or concurrency | Add concurrency only after two-consumer evidence | R3 | M | Medium if prematurely generalized |
| Use neutral non-domain fixtures | Prevents shared host overfitting and fake named-product progress | Reusable conformance and negative-space evidence | Less domain realism before M5 | Pair with read-only domain-team contract review, not domain code | R2–R4 | M | Low; late missing generic requirements remain possible |
| Require live capability equality | Prevents descriptor-only or canned success | Accurate readiness and fail-closed compatibility | More handshake/version cases | Add descriptor, handshake, then action-set comparison | R1/R4 | M | Low if exact; critical if weakened |

## 13. Open decisions and stop conditions

R1 requires operator/architecture acceptance of:

- separate-repository consumer package versus isolated monorepo workspace package;
- exact adapter transport, process ownership, installation/resource model, and trust boundary;
- whether any pending interaction/session-controller state must persist across restart;
- exact platform scope for R2 package metadata/readback before R5;
- single-session limitations accepted for the first shared runtime contract.

Stop and amend the plan if any proposed implementation:

- requires editing/copying Gosling host source for each consumer;
- exposes raw ACP, generic RPC, Node, process, filesystem, settings, or updater authority to the
  renderer;
- links a named domain crate or arbitrary native library into generic Gosling;
- makes a descriptor, mock, or canned response count as live adapter success;
- hides permission/elicitation cancellation or drops session updates;
- adds GUI behavior to compensate for missing backend state/action;
- introduces a second credential, policy, provider, permission, session, or mutation authority;
- turns fixture packaging into signing, publication, updater, or production identity work;
- cannot name a deterministic verification source for a new interface or state.

## 14. Verification handoff

After implementation, an independent architecture/security/reliability review should verify:

1. consumer-to-host dependency direction and absence of host edits per consumer;
2. preload/API negative space and absence of raw connection/process authority;
3. session/prompt/update/permission/elicitation event ordering and stale/replay defenses;
4. lifecycle state/action reachability and real recovery effects;
5. server ownership of policy, credentials, sessions, adapter authorization, and mutation;
6. live adapter equality, supervision, bounds, cleanup, and confirmation semantics;
7. exact package resources/metadata and absence of Gosling-only extras;
8. neutral fixture negative space and absence of named domain semantics;
9. full-Gosling backward compatibility and no-session-on-failed-preflight behavior;
10. revision-bound requirement/evidence traceability with no self-graded completeness.

The reviewer, not this plan, decides whether the implementation conforms. Until that review and
PG-50 acceptance, shared GUI remains blocked and named project GUI remains prohibited by the M5
policy.
