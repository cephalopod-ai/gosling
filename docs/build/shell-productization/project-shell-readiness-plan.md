# Project-shell readiness implementation plan v2

Date: 2026-08-13
Baseline: `main` at `6fe6a3bfcab3a48846850fd321fb8a056223d355`
Status: proposed replacement for forward Gates 5–8; Gates 0–4 remain historical evidence
Companion assessment: [`readiness-reassessment.md`](readiness-reassessment.md)
Focused R1–R4 execution companion:
[`pre-gui-backend-implementation-plan.md`](pre-gui-backend-implementation-plan.md)

## 1. Mission

Complete and prove the shared Gosling foundation so independently developed project shells—such as DAWES, a math shell, Project ABC, or later focused applications—can be created without:

- copying or branching full Gosling Desktop main/preload/runtime code;
- adding domain behavior to Gosling core;
- inventing a second settings, credential, provider, permission, or session authority;
- receiving raw ACP credentials, arbitrary IPC, filesystem, process, updater, or release authority;
- editing shared host source for each new shell;
- relying on package/profile checks that never exercise a real application workflow.

The campaign ends at **project-shell consumer ready** and **fixture distribution ready**. It does not implement any named project shell, production brand, real release destination, signing/notarization, publication, or updater promotion.

## 2. Rebaselined status

### Accepted foundation assets

| Area                                                        | Status                | Evidence boundary                                     |
| ----------------------------------------------------------- | --------------------- | ----------------------------------------------------- |
| Rust provisioning, validation, policy, runtime namespace    | retain                | merged shell foundation and spawned ACP tests         |
| strict product profile and identity collision checks        | retain                | 41 current Node tests                                 |
| dedicated main/preload/renderer package entries             | retain                | source and macOS arm64 package readback               |
| main-owned child generation and authenticated ACP preflight | retain                | focused tests plus live create/restart/resume harness |
| compatibility before durable session creation               | retain                | live incompatible-core/no-session proof               |
| diagnostics and handoff primitives                          | retain                | focused unit tests and full-Gosling receiver tests    |
| macOS arm64 host package integrity                          | retain, bounded claim | exact prior artifact only                             |

### Reopened or missing work

| Area                                              | Current state        | Plan disposition                           |
| ------------------------------------------------- | -------------------- | ------------------------------------------ |
| Linux V8 CI                                       | R0 baseline healthy  | verified twice; retain R8 freshness checks |
| project renderer composition                      | absent               | R1 architecture blocker, R2 implementation |
| usable shell session/prompt/update path           | absent               | R1 contract, R3 implementation             |
| permission/elicitation handling                   | auto-denied/dropped  | R3 implementation                          |
| domain adapter registration                       | dead production seam | R1 decision, R4 conformance implementation |
| truthful lifecycle mapping                        | partial              | R3                                         |
| verified safe renderer snapshot                   | absent               | R3                                         |
| shared recovery/session/handoff UI                | absent               | R5                                         |
| least-privilege platform metadata/resources       | incomplete           | R2/R6                                      |
| packaged application workflow/restart/coexistence | absent               | R6                                         |
| reusable cross-platform workflows                 | absent               | R7                                         |
| final project-shell onboarding proof              | absent               | R8                                         |

## 3. Binding boundaries

1. **No named project implementation.** Names may appear only in scope guards, historical records, or neutral examples.
2. **Main-owned ACP stays.** Renderer never receives ACP URL, token, server secret, process handle, profile path, provisioning path, or raw child logs.
3. **Server-owned authority stays.** Policy, credentials, provider/model resolution, session persistence, domain-operation authorization, and reference access remain in Rust/backend services.
4. **No arbitrary plugin execution.** A project manifest cannot name an unrestricted main/preload script, native library, shell command, URL, or package-install hook.
5. **No fake domain adapter.** A descriptor without a callable, validated adapter is invalid for a project-shell-ready artifact.
6. **No hidden host edits per shell.** A new neutral consumer must build after adding only consumer-owned files plus declarative workspace/registry membership where architecture requires it. Host source, Forge logic, preload, and workflow policy remain unchanged.
7. **No second full chat client by copy.** Reuse pure ACP message/permission reducers where safe; do not import the broad full-Desktop preload/API or duplicate its global singleton architecture.
8. **No release activation.** Fixture workflows may build and attest ephemeral artifacts but may not sign, notarize, publish, update, or upload to a public release.
9. **Historical evidence is immutable.** New evidence may narrow or supersede a status but does not rewrite point-in-time observations.
10. **Every gate is revision-bound.** Static tests cannot satisfy runtime, package, platform, or CI acceptance.

## 4. Target architecture

### 4.1 Layers

```text
project-shell consumer
  product profile + provisioning refs + renderer/domain UI + adapter/extension + assets
                    │ versioned consumer manifest
                    ▼
shared shell application kit
  typed renderer runtime API + session/domain reducers + recovery/handoff/diagnostics UI
                    │ narrow typed preload
                    ▼
dedicated Electron shell host
  app identity + child lifecycle + authenticated ACP + compatibility + package resources
                    │ loopback authenticated ACP
                    ▼
Gosling Rust authority
  provisioning + policy + credentials + sessions + tools + domain-operation authorization
```

### 4.2 Required contracts

#### Consumer manifest

A new versioned, strict, source-controlled manifest identifies:

- consumer schema version and required Gosling shell-kit/core versions;
- product profile path;
- renderer entry selected from an approved consumer root;
- optional domain adapter/extension descriptor by stable ID and protocol version;
- declared shell-kit capabilities used by the renderer;
- asset and test-fixture roots;
- no secrets, commands, arbitrary URLs, native-library paths, main/preload replacement, or release credentials.

The manifest is resolved and hashed with the product profile. Unknown fields and versions fail closed. The exact schema is frozen in R1, not improvised during R2.

#### Safe runtime snapshot

The renderer receives a bounded immutable snapshot including only:

- generation and lifecycle state/reason/actions;
- verified product identity and runtime namespace;
- safe compatibility status;
- provisioning issue codes/paths and safe resolution labels/IDs where explicitly approved;
- session status/ID and prompt phase;
- domain adapter descriptor and capability availability;
- pending permission/elicitation summaries with opaque action IDs;
- no secrets, ACP endpoint, full config/provisioning, private paths, prompt history, or raw errors.

#### Application runtime operations

The reviewed preload/API must support only the minimum application workflow:

- runtime read/subscribe/retry/stop;
- session create or resume under main/server-owned working-directory policy;
- bounded text prompt submit and cancel;
- bounded session update events reduced in main or a trusted shared service;
- action-bound permission/elicitation response for currently pending server requests;
- domain snapshot and declared action through the selected adapter contract;
- diagnostics save;
- handoff prepare/confirm;
- allowlisted external support/relink open.

Every request includes generation and, where applicable, session ID plus an action/request nonce. Main validates sender, exact fields, byte/count bounds, current state, capability declaration, and staleness. Responses and events are bounded and schema-validated.

#### Domain integration

R1 must choose and document one topology. Preferred:

- project domain logic runs out-of-process as a versioned MCP/ACP-style adapter or extension;
- Gosling launches or connects only through a source-controlled typed descriptor and bounded protocol;
- adapter identity/capabilities are validated before `ready`;
- snapshot is read-only;
- every action is allowlisted by descriptor and server policy;
- mutation actions require an action-bound confirmation token or explicit permission interaction;
- crash, timeout, malformed output, and version mismatch map to typed lifecycle/domain states;
- domain payloads remain opaque to the shared host and are validated by the project consumer’s generated schema;
- no Rust `cdylib` loading and no domain crate linked into the generic Gosling CLI by default.

If a different topology is selected, ADR review must show equivalent copy-free composition, isolation, lifecycle, versioning, least privilege, packaging, and testability.

### 4.3 Composition topology decision

R1 compares:

1. **Separate repository consumer package (preferred):** project repo consumes a versioned/pinned shell kit and host build interface.
2. **Isolated workspace consumer package:** project package lives under an approved consumer root in this monorepo but cannot edit host modules.

The decision must explain source distribution, package manager/workspace wiring, renderer bundling, generated types, compatibility, update cadence, private project support, and how Gosling security fixes flow to consumers. “Copy the fixture and edit Gosling” is not an option.

## 5. Milestones

- **M0 — baseline healthy:** R0 complete; current main CI green.
- **M1 — architecture accepted:** R1 complete; consumer/runtime/domain contracts frozen.
- **M2 — host consumer seam ready:** R2 complete; independent neutral renderer composes without host edits.
- **M3 — common application runtime ready:** R3 complete; real prompt/update/permission workflow works through the narrow boundary.
- **M4 — domain integration ready:** R4 complete; neutral adapter snapshot/action works end to end.
- **M5 — project-shell consumer ready:** R5 complete; an independently authored neutral shell is usable and accessible in development.
- **M6 — fixture distribution ready:** R6/R7 complete; packaged and cross-platform workflow evidence is green.
- **M7 — campaign accepted:** R8 complete; onboarding/conformance proof and final audits pass.

**Named project shells may begin implementation only after M5.** Production packaging/release should wait for M6/M7 unless the operator explicitly accepts a narrower development-only start.

## 6. Gate plan

### R0 — Reconcile truth and restore baseline CI

**Status:** complete on 2026-08-13. PR run `31731952749` and merged-main run `31732990062` both completed the Linux helper, verified archive preparation, and full Rust test step. See [`evidence/r0.md`](evidence/r0.md).

**Purpose:** remove stale status and repair the current red prerequisite before new architecture work.

**Implementation**

1. Repair `archive_size` so BSD and GNU `stat` output is selected by platform/capability, never by a successful but semantically different command.
2. Add self-test coverage that runs the GNU path under `set -u` and rejects nonnumeric output.
3. Re-run helper cold/warm/corrupt/network/checksum/concurrency/target tests locally where applicable.
4. Push through normal review and obtain two clean Linux `rust-build-and-test` runs on the repaired lineage.
5. Update build state, README, risk/defect/assumption ledgers, and traceability to distinguish merged, local, remote, and packaged evidence.
6. Record Gate 4 as historical process-boundary acceptance and list its reopened failure-path criteria under R3/R6.

**Exit criteria**

- current branch and subsequent main CI run complete Linux Rust tests, not merely helper setup;
- no red required check is described as blocked or green;
- source and status documents agree.

### R1 — Freeze project-shell topology and application contracts

**Purpose:** decide how a project shell plugs in before UI or IPC implementation creates accidental architecture.

**Deliverables**

1. ADR-0010: project-shell consumer/composition topology.
2. ADR-0011: main-owned application runtime and renderer capability boundary.
3. ADR-0012: domain adapter lifecycle, transport, registration, and authority.
4. Strict consumer-manifest v1 schema and threat model.
5. Safe runtime snapshot, operation/event, permission/elicitation, session, domain, and compatibility schemas.
6. Dependency diagram and ownership table for Gosling versus project repository.
7. Migration decision for the current in-process `DomainAdapter` trait: internal-only, adapted behind the selected transport, or deprecated. No dual ambiguous production paths.
8. Exact required method/capability negotiation model. Product profiles must not carry a hand-maintained global method list that disagrees with consumer capabilities.

**Required design proofs**

- a project renderer cannot replace main/preload or import Node/Electron authority;
- main can operate ACP without exposing credentials;
- permission and elicitation are user-mediated, action-bound, and stale-safe;
- domain adapter registration is real and versioned;
- consumer can be private/separate without vendoring Gosling source;
- full Gosling remains backward compatible.

**Exit criteria**

- architecture review accepts one topology and rejects alternatives explicitly;
- all future R2–R5 modules map to frozen contracts and tests;
- no open P0 decision remains about where renderer, ACP session control, or domain logic lives.

### R2 — Implement the consumer build/composition seam

**Purpose:** make a separately owned renderer a real build input while keeping host authority immutable.

**Implementation**

1. Implement strict consumer-manifest resolver and canonical hash.
2. Update Vite/Forge adapters to consume only resolver output; host main/preload entries remain fixed.
3. Select a renderer entry only from an approved consumer root/package; reject symlinks, traversal, commands, URLs, and main/preload entries.
4. Embed consumer manifest/hash and compatibility metadata in package resources.
5. Move neutral fixture renderer into a consumer-owned fixture package rather than `src/shell/renderer.ts` hard-coding.
6. Add a second structurally different neutral consumer to prove composition does not depend on one UI shape.
7. Stage the exact Gosling binary and exact required resources instead of all `src/bin`.
8. Remove or parameterize inherited Gosling metadata/permissions from shell mode: document types, TCC strings, Linux desktop fields, homepage/category, Flatpak permissions, and signing entitlements.
9. Extend package verifier to inspect consumer hash, exact resource inventory, platform metadata, permissions/entitlements, CSP, and absence of Gosling-only extras.

**Acceptance tests**

- build each neutral consumer without changing host source;
- hostile consumer manifest/path/entry/import tests;
- host main/preload bundle hashes or source snapshots remain identical across consumers;
- default full Gosling Forge behavior remains unchanged;
- package tamper tests cover renderer/consumer manifest and extra-resource injection.

**Exit criteria**

- adding a third neutral consumer requires only consumer-owned files and declarative registration permitted by ADR-0010;
- no product-specific condition exists in host, Forge, Vite, or preload source;
- exact shell resources and platform metadata are least-privilege and read back.

### R3 — Implement the usable common application runtime

**Purpose:** turn main-owned ACP from preflight-only infrastructure into a safe agent application service.

**Implementation**

1. Extend `ShellAcpConnection` with session create/resume, bounded text prompt, cancel, and generated domain methods selected by capabilities.
2. Replace discarded callbacks with focused notification, permission, and elicitation controllers. Reuse pure adapters from full Desktop where contracts fit; do not reuse broad globals/preload.
3. Add a main-owned session controller with one active session initially, explicit state, prompt-attempt IDs, cancellation ordering, reconnect, and compacted resume.
4. Implement the safe runtime snapshot and bounded event stream.
5. Add reviewed IPC/preload operations from R1, retaining exact field/size/sender/generation/action-nonce validation.
6. Classify real provisioning issues into `relink_required`, configuration `degraded`, compatibility `incompatible`, transport `offline`, cleanup/invariant `fatal`, and active prompt `busy`.
7. Ensure every lifecycle state has a production event source, user action, and diagnostic event; remove unused states rather than presenting fiction.
8. Record illegal transitions and stale events in bounded diagnostics without mutating state.
9. Define permission defaults for focused shells. No automatic approval; cancellation/denial must be visible rather than silently swallowed.

**Acceptance tests**

- main → real child → ACP → create → prompt fixture → streamed updates → idle;
- cancel during prompt; permission allow-once/deny; elicitation cancel/submit where server supports it;
- connection loss during each phase; child restart and compacted resume;
- stale generation/session/request nonce; duplicate submit/cancel/response;
- malformed/oversized prompt and events;
- no ACP token, secret, raw config, or private path reaches renderer snapshots/events;
- renderer cannot invoke undeclared methods or domain action absent negotiated capability.

**Exit criteria**

- neutral consumer can complete one deterministic no-credential or fixture-provider prompt workflow through preload;
- permission/elicitation behavior is explicit and tested;
- every declared lifecycle state is reachable by a real tested path or removed;
- no failed compatibility/provisioning path creates a session.

### R4 — Make domain integration real with a neutral adapter

**Purpose:** prove the exact extension seam future project shells will use without implementing domain semantics.

**Fixture adapter behavior**

- exposes a versioned descriptor and two neutral actions: one read-only snapshot and one explicitly confirmed state mutation in isolated temporary fixture state;
- returns generated-schema payload and exact resource references;
- has no network, credential, repository, scientific, mathematical, coding, or project-specific behavior;
- is permanently non-publishable.

**Implementation**

1. Implement selected R1 registration/transport and lifecycle.
2. Validate adapter identity, version, schema, action allowlist, and capability match before `ready`.
3. Route snapshot/action through Rust/server authority and generated DTOs; renderer payload handling stays consumer-owned.
4. Require an action-bound confirmation token for the fixture mutation; replay, cross-action, stale, and expired tokens fail.
5. Bound input/output/resources/time and redact failures.
6. Handle absent, crashing, hanging, malformed, incompatible, and overproducing adapters without orphaning processes or widening authority.
7. Update compatibility negotiation and package resources for the adapter contract.

**Exit criteria**

- independent neutral consumer reads a snapshot and performs one confirmed fixture action end to end;
- adapter absence/mismatch prevents false `ready` when the consumer declares it required;
- no domain implementation is linked into generic Gosling core;
- a descriptor cannot claim actions the live adapter does not expose.

### R5 — Build the shared shell application kit and conformance consumer

**Purpose:** provide reusable non-domain UI after the runtime contract is real.

**Implementation**

1. `ShellRuntimeProvider` with abort-safe subscriptions and generation/session fencing.
2. `ShellHostApp` with a typed domain-content slot mounted only when required runtime and adapter capabilities are ready.
3. Common booting, validating, ready, busy, relink, degraded, incompatible, offline, fatal, and stopping surfaces mapped to real R3 states.
4. Session create/resume and minimal conversation/status primitives needed by both neutral consumers.
5. Permission/elicitation presentation, prompt cancel, diagnostics export, relink handoff, and explicit handoff confirmation.
6. Product identity comes from verified safe snapshot, not untrusted renderer props.
7. Keyboard order, focus restoration, live regions, non-color status, reduced motion, minimum window, zoom, and screen-reader labels.
8. Consumer-owned payload renderers cannot inject HTML, open arbitrary URLs, or bypass action confirmation.
9. Publish or otherwise version the shell-kit contract according to ADR-0010; prove generated type compatibility.

**Conformance proof**

Build two neutral consumers with different layouts and capability declarations. At least one must be outside the host source subtree selected by the composition model. Both use the same shell kit and preload types without host edits.

**Exit criteria — M5 project-shell consumer ready**

- a user can start, recover, create/resume a session, submit/cancel a deterministic prompt, handle a permission, inspect neutral domain state, confirm an action, export diagnostics, and hand off;
- accessibility tests and a manual assistive-technology smoke pass;
- shared UI contains no project copy, payload semantics, prompts, branding, or release destination;
- a template/conformance command rejects incomplete consumers before packaging.

### R6 — Packaged workflow, restart, failure matrix, and coexistence

**Purpose:** prove the consumer/runtime/domain seams in the actual artifact rather than only source tests.

**Primary packaged scenario**

1. package neutral consumer A with exact host/core/adapter resources;
2. verify profile, consumer, manifest, binary, metadata, permissions, CSP, and resource inventory;
3. launch artifact and observe verified identity plus ready adapter;
4. create session and complete deterministic prompt/update/permission workflow;
5. read snapshot and confirm fixture action;
6. export and inspect redacted diagnostics;
7. close/relaunch and resume session/fixture state as contract requires;
8. prepare explicit non-mutating full-Gosling handoff;
9. quit and prove Electron, backend, adapter, registry, temp files, and listeners are clean.

**Failure matrix**

- missing/corrupt binary, profile, consumer manifest, provisioning, renderer, or adapter;
- backend/adapter crash before and after ready;
- renderer reload/crash;
- ACP and adapter disconnect during prompt/action;
- readiness/cleanup timeout and forced termination;
- incompatible core/consumer/adapter schema;
- relink-required credential profile;
- malformed deep link, event, permission response, action token, and diagnostic path;
- double retry, quit during startup, and stale generation.

**Coexistence**

Run full Gosling plus both neutral consumers concurrently. Verify locks, protocols, application paths, runtime namespaces, browser partitions, sessions, adapter state, diagnostics, registries, cache/temp, restart, and quit signals do not cross. Protected config/credential catalog is the only intentional shared authority, and raw credentials never copy into consumer roots.

**Exit criteria**

- primary scenario passes twice clean and once across restart;
- every reopened Gate 4 failure path is covered or explicitly removed from the contract;
- no orphan or cross-product state remains;
- artifact hash/revision and redacted evidence are recorded.

### R7 — Cross-platform fixture CI and guarded distribution workflows

**Purpose:** make the shared foundation reproducibly buildable without activating a real product release.

**Implementation**

1. Reusable consumer/profile resolution action with pinned dependencies and immutable outputs.
2. PR/manual/nightly fixture workflows for macOS arm64/x64, Windows x64, and Linux x64.
3. Structural/readback on every target; launch smoke where runner support is reliable; full packaged scenario on at least one target per change and scheduled coverage for others.
4. Generated artifact names/checksums/attestations from canonical profile + consumer + core manifest.
5. Fixture artifacts receive short retention and never attach to public releases.
6. Guarded release workflow skeleton may validate a publishable profile but cannot run until an approved destination, production identity, signing policy, compatible predecessor, and human environment approval exist.
7. Least privilege, no untrusted script interpolation, no credential access from fork PRs, no `eval`, no broad artifact globs, and re-resolution in privileged jobs.
8. Update/rollback design test with synthetic signed metadata only; no updater activation.

**Exit criteria — M6 fixture distribution ready**

- all committed target jobs are green on exact revision;
- package/readback proves target metadata and resource isolation;
- workflow security review passes;
- no fixture can sign, publish, notarize, update, or select a destination through inputs.

### R8 — Final readiness audit and onboarding proof

**Purpose:** prove the foundation can be handed to project-shell teams without hidden Gosling surgery.

**Implementation and validation**

1. Start from a clean checkout and the documented template.
2. Create a third neutral conformance consumer without editing host/core/Forge/preload/workflow policy source.
3. Run check, development launch, type/lint/unit, adapter conformance, package, readback, smoke, cleanup, and coexistence commands exactly as documented.
4. Run Rust format, workspace tests, Clippy with warnings denied, schema/SDK regeneration checks, Desktop lint/typecheck/tests/format scope, shell fixtures, packaged tests, and workflow validation.
5. Run architecture, reliability, data isolation, security, accessibility, release/supply-chain, negative-space, and fake-success audits.
6. Search for domain names/semantics, TODO/FIXME/stub/mock/canned success, raw secrets/URLs, broad IPC, direct renderer Node access, duplicated schemas, dead adapter paths, and unreferenced capabilities.
7. Update architecture, ADR index, shell-product manual, troubleshooting, compatibility/migration, test/evidence index, traceability, risks, assumptions, defects, and build state.
8. Produce a project-shell onboarding checklist and compatibility matrix that states exactly what a DAWES/math team owns and what Gosling owns.

**Exit criteria — M7 campaign accepted**

- third-consumer exercise succeeds from docs with no host edits;
- all required automated checks are green on exact revision;
- residual platform/signing/publication limitations are explicit;
- no critical/high unresolved finding remains in the consumer/runtime/domain/package boundary;
- operator accepts the foundation for named project-shell implementation.

## 7. Critical path and concurrency

```text
R0 CI health
  ↓
R1 architecture/contracts
  ├── R2 consumer composition ──┐
  └── R3 application runtime ───┼── R4 domain integration ── R5 application kit
                                └──────────────────────────────┘
                                                              ↓
                                                     R6 packaged acceptance
                                                              ↓
                                                     R7 platform workflows
                                                              ↓
                                                     R8 final onboarding
```

Safe parallelism:

- R2 build resolver work and R3 pure reducers may proceed after R1 if they do not edit the same profile/IPC/Forge files.
- R7 workflow design may be reviewed after R2 metadata contracts, but no acceptance claim precedes R6.
- Documentation/evidence updates may proceed alongside a gate but must reference observed results only.

Unsafe parallelism:

- do not implement IPC before R1 schemas;
- do not build shared UI before R3 state/action producers;
- do not implement adapter transport before R1 authority/lifecycle decision;
- do not let multiple workers edit Forge/Vite/profile resolver or IPC/preload concurrently;
- do not start a named project shell as an integration test.

## 8. Requirement deltas

Existing SHP-REQ IDs remain for historical traceability. Add these readiness requirements without renumbering old ones:

| ID          | Priority | Requirement                                                                                                                         | Primary gate |
| ----------- | -------- | ----------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| SHP-REQ-033 | P0       | A project-shell consumer supplies its renderer through a versioned strict composition contract without host source edits            | R1–R2        |
| SHP-REQ-034 | P0       | Main-owned ACP exposes a bounded typed session/prompt/update/cancel service without revealing connection authority                  | R1–R3        |
| SHP-REQ-035 | P0       | Permission and elicitation requests are explicitly presented, action-bound, stale-safe, and never silently auto-approved            | R3/R5        |
| SHP-REQ-036 | P0       | Declared domain adapter capabilities resolve to a live versioned adapter with bounded lifecycle and server-enforced actions         | R1/R4        |
| SHP-REQ-037 | P0       | Every lifecycle state/action has a production producer and tested recovery path or is removed                                       | R3–R6        |
| SHP-REQ-038 | P0       | A safe verified runtime snapshot supplies identity, compatibility, provisioning status, session, and adapter capability to renderer | R3           |
| SHP-REQ-039 | P0       | Shell packages contain only exact required binaries/resources and project-neutral least-privilege platform metadata                 | R2/R6/R7     |
| SHP-REQ-040 | P0       | A third neutral consumer completes documented development conformance without Gosling host/core edits                               | R8           |
| SHP-REQ-041 | P1       | Two structurally different consumers prove the shell kit and domain-content slot are not fixture-specific                           | R2/R5        |
| SHP-REQ-042 | P1       | Consumer/core/adapter compatibility and migration failures are fail-closed and diagnosable                                          | R1/R3/R4/R6  |
| SHP-REQ-043 | P1       | Current main and fixture workflow evidence are green on committed Linux/macOS/Windows gates                                         | R0/R7/R8     |

## 9. Definition of project-shell ready

The foundation is ready for DAWES, math, or another named project shell only when all are true:

- M5 is accepted at minimum; M6/M7 status and any development-only limitation are explicit;
- a project repo/package can supply renderer, profile, provisioning, adapter/extension, assets, and tests without changing shared host source;
- real session prompt/update/cancel and permission behavior works through the narrow API;
- a live neutral adapter snapshot/action path proves domain integration rather than only DTO existence;
- runtime identity, policy, credentials, sessions, and mutation authority remain server-owned;
- renderer receives no ACP/process/filesystem/release authority;
- package metadata/resources are least-privilege and consumer-specific rather than inherited Gosling values;
- development and packaged failure/restart/cleanup behavior is observed;
- current required CI is green;
- documentation tells a project team exactly which contract versions it targets and how to validate compatibility.

A strict product profile, a neutral lifecycle label, generated domain DTOs, or a package that launches without a usable workflow does **not** satisfy project-shell readiness.

## 10. Immediate next actions

1. Conduct R1 as an architecture task before adding shared UI or widening preload.
2. Obtain operator review of ADR-0010–0012 and the consumer/runtime/domain contracts.
3. Implement R2/R3 in bounded, separately reviewable slices with tests and evidence.
4. Do not create DAWES/math source roots, profiles, prompts, adapters, or UI during this campaign.
