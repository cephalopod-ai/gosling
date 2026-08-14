# Default Shell DS-3 through DS-7 implementation plan

Status: implementation-ready plan; no DS-3–DS-7 runtime or GUI implementation claimed
Date: 2026-08-14
Authority: ADR-0007–0012, ADR-0014, `architecture/default-shell-template.md`, and
SHP-REQ-044–053

## 1. Objective and completion boundary

Complete the nonvisual Default Shell foundation so a generic renderer can be implemented without
inventing backend, credential, filesystem, module, packaging, or recovery policy while building the
UI. The sequence ends at DS-7 with a clean-revision, current-CI decision authorizing or rejecting
Default Shell GUI work.

This plan does not implement:

- a Default Shell renderer or shared shell component library;
- DAWES, math, Physics/CST, or any other named shell;
- production identifiers, signing, notarization, updater activation, publishing, or release feeds;
- a generic filesystem/settings/RPC bridge, renderer-owned backend endpoint, credential editor, or
  plugin installer;
- multiple simultaneous sessions or multiple supervised domain-adapter processes in v1.

## 2. Current source-grounded baseline

| Area               | Current behavior                                                                                                                                          | Consequence for this plan                                                                                                             |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Working directory  | `shell/main.ts` supplies `process.cwd()`; `runtimeController` and `connectShellAcp` capture it; `session.create` carries no renderer path                 | DS-3 must replace the startup constant with main-owned validated selection state without giving the renderer arbitrary path authority |
| Server validation  | `validate_absolute_cwd` requires absolute, existing directories; workspace sessions additionally canonicalize and pin folder policy                       | DS-3 adds a shell-specific, side-effect-free canonical validation contract and revalidates at session creation                        |
| Session ownership  | `sessionController` permits one active session and already has an internal non-destructive `close()`                                                      | Directory changes use explicit idle-session detach; they never delete persisted sessions or silently move an active session           |
| Local settings     | schema-v1 product-local reader/writer exists; no IPC, migration, or recovery service exists                                                               | Foundation closure must finish DS-2 before DS-7 and DS-3/DS-4 may write only their existing allowlisted references                    |
| Credential catalog | `_gosling/unstable/credential-profiles/list` returns full `CredentialProfile`, including configured secret-field names and non-secret provider parameters | DS-4 must add a shell-safe server projection; main must not fetch the broad response and filter it after receipt                      |
| Credential use     | provisioned profile IDs are backend-resolved, status-checked, provider-scoped, and pinned as safe ID/name session snapshots                               | DS-4 extends session launch selection without copying secrets or weakening backend revalidation                                       |
| Modules            | provisioning selects extensions/skills and one optional supervised adapter; runtime snapshot exposes only the adapter summary                             | DS-5 defines one safe registry view over these existing authorities; it does not add generic invocation                               |
| Consumer/build     | strict consumer and product profiles, two neutral fixtures, manifest hashing, asset verification, and local packaging exist                               | DS-6 extends these established resolvers and approved fixture roots; it does not add per-product host branches                        |
| Acceptance         | historical PG-50 is exact-revision evidence; current Default Shell worktree is dirty and later                                                            | DS-7 requires a new isolated revision and current CI; historical acceptance cannot be inherited                                       |

## 3. Binding planning defaults

These defaults resolve implementation ambiguity. Deviating from one requires an L2 plan-change
entry and, where the preload or provisioning authority changes, an ADR amendment before code.

1. **Directory selection is not workspace creation.** A user may choose an arbitrary local directory
   through the native picker. Validation canonicalizes and pins it for the session without creating,
   updating, or activating a global Gosling workspace.
2. **The backend revalidates.** Main may perform early checks for responsiveness, but Rust is the
   final authority before selection is accepted and again before a session is created.
3. **Switching is explicit and idle-only.** Selecting a different directory while a session exists
   requires explicit confirmation, no streaming prompt, and no pending permission, elicitation, or
   mutation confirmation. The current session is detached locally, remains resumable, and is not
   deleted.
4. **Credential catalog access is explicitly provisioned.** Existing fixed-profile behavior remains
   the default. A new optional shell credential policy may opt a product into safe catalog selection;
   absence never silently widens access.
5. **Safe credential metadata is minimal.** It contains only opaque profile ID, display name,
   provider/service ID, and configured/relink-required status. It excludes auth kind, source,
   configured secret-field names, non-secret parameter names/values, timestamps, usage, and secrets.
6. **A selected credential is a session input, not a copied credential.** Local settings may remember
   only the opaque profile ID. Rust resolves it at session creation, checks policy/provider/status,
   constructs the scoped provider, and pins only safe ID/name metadata.
7. **Module v1 is an inventory, not a generic bus.** It combines core session capability, selected
   extensions/skills, and at most one supervised adapter. Extension tools remain agent-invoked;
   adapter actions keep their existing typed snapshot/action/confirmation routes.
8. **Scaffolding is non-destructive and non-production.** It refuses existing destinations, emits
   only non-publishable/update-disabled/signing-none templates, and never invents credentials,
   release destinations, production identifiers, or finished icons.
9. **Corruption is visible.** Invalid settings, stale directories, revoked credentials, incompatible
   modules, and missing assets produce stable recovery state. No component silently resets or
   substitutes a broader default.

## 4. Dependency graph and execution units

```text
Foundation closure (remaining DS-1/DS-2 proof)
        |
        +--> DS-3 directory/session authority ----+
        |                                          |
        +--> DS-4 safe credential authority -------+--> DS-5 module registry
                                                           |
                                                           v
                                                    DS-6 scaffold
                                                           |
                                                           v
                                                    DS-7 acceptance
```

DS-3 and the read-only half of DS-4 may proceed independently after foundation closure. Their
session-launch changes must be integrated together before DS-5 starts. One implementation unit is
completed, reviewed, and validated before the next begins; do not carry partially wired IPC or ACP
surfaces across units.

### Foundation closure — entry gate for DS-3/DS-4

This plan begins only after the existing DS-1/DS-2 worktree is isolated enough to review. Before
DS-3 or DS-4 is marked complete:

- exercise the shell-owned prompt through a live session and prove generic Gosling prompt/global
  hints are absent;
- prove an empty shell extension selection yields no developer client or developer tools in the live
  session inventory;
- define settings schema migration behavior for missing, v1, unsupported, malformed, interrupted,
  and permission-denied files;
- add narrow main-owned settings read/update operations for the fixed allowlisted document, with no
  arbitrary key or path input;
- prove atomic replacement on supported platforms and a visible recovery result on failure;
- record DS-1/DS-2 as locally complete without claiming revision-bound acceptance.

Failure to close these items blocks DS-7 even if later packages pass.

## 5. DS-3 — Working-directory and session authority

### DS-3.0 — Freeze the operation and state contract

Amend ADR-0011/ADR-0014 and the R1 contract before IPC implementation. Freeze these behaviors:

- renderer requests selection using its current generation; it never sends a filesystem path or a
  caller-asserted authorization flag;
- main permits only one outstanding request, opens Electron's native directory picker, and sends a
  user-confirmed absolute path only to the authenticated loopback backend for canonical validation;
- cancel is a successful typed result, not an error;
- accepted selection returns a bounded canonical path and a display label; raw validation errors are
  mapped to stable reason codes;
- main owns `selected`, `missing`, `invalid`, and `unselected` directory states and publishes the
  safe state in `ShellRuntimeSnapshot`;
- session creation uses the selected canonical path. Session resume always uses the persisted
  session path returned by the server and does not overwrite the current preference implicitly;
- an explicit idle-session detach operation is required before switching directories with an active
  session. It does not delete or mutate that server session.

Planned contract surfaces follow existing modules: canonical DTOs in
`crates/gosling-sdk-types/src/shell.rs`; shell handler/dispatch in `crates/gosling/src/acp/`; typed
main/preload definitions in `ui/desktop/src/shell/ipc.ts` and `preloadApi.ts`. Exact ACP method and
main/preload channel names are frozen together in this contract step. Generated ACP/SDK artifacts
must derive from the Rust contract; the TypeScript IPC allowlist remains one reviewed shell-local
surface rather than independently drifting copies.

**Exit:** operation table, request/response bounds, lifecycle legality, capability name, error codes,
and negative-space tests are reviewed before implementation.

### DS-3.1 — Add side-effect-free backend validation

- Add a shell-specific request/response that accepts one bounded absolute directory and returns one
  canonical directory or a stable invalid/unavailable reason.
- Reuse the existing absolute/existing/directory checks and canonicalization conventions; extract a
  common helper rather than creating conflicting workspace validation.
- Do not create a workspace, change active/default workspace, create output folders, or persist the
  path in Rust global config.
- Revalidate and canonicalize on every session creation. Require the session path to match the
  main-selected canonical path used for that request.
- Build the session's source/folder policy from the canonical directory so later tool path checks do
  not operate on the pre-canonical alias.
- Bound paths and errors; never echo an inaccessible private path in renderer-visible diagnostics.

**Primary tests:** relative, missing, file-not-directory, unreadable where testable, symlink alias,
canonical path change, selection/session TOCTOU, oversized/NUL input, and no-workspace-mutation.

### DS-3.2 — Add main-owned directory state and native selection

- Extend `ShellBootstrapAdapter` using the same dependency-injected dialog pattern as diagnostics.
- Load the remembered directory from `identity.paths.localSettings`, validate it after ACP is ready,
  and publish `missing`/`invalid` rather than silently clearing or auto-starting.
- Keep the selected path in main memory; persist it only after backend acceptance.
- Reject selection when the request generation is stale, the runtime is not ready, or a prompt or
  interaction is active.
- On confirmed idle switch, close the local one-session controller state, preserve the session ID for
  later resume, validate/persist the new directory, and return to a no-session ready state.
- Clear selection state on generation teardown without deleting the remembered setting.

**Primary files:** `bootstrap.ts`, `runtimeController.ts`, `runtimeSnapshot.ts`, `sessionController.ts`,
`appIdentity.ts`, and `localSettings.ts`, with their existing focused tests.

### DS-3.3 — Extend narrow IPC/preload capability

- Add only typed read/select and idle-detach operations; do not add file read/list/stat/write or a
  caller-supplied path.
- Apply existing sender-frame checks, exact-key parsing, size bounds, response bounds, generation
  fencing, declared-capability checks, frozen preload objects, and listener disposal.
- Update the consumer resolver's operation allowlist and required-method projection mechanically from
  the accepted contract.
- Update preload surface snapshot and reverse-registration tests so any extra channel fails.

### DS-3.4 — Bind session creation and resume

- Change the trusted `ShellAcpConnection.createSession` input from its captured startup constant to a
  main-supplied accepted directory record.
- Keep the renderer's `session.create` request path-free; main injects the current accepted path.
- Return the server-confirmed canonical working directory in the main session record/snapshot only
  where the accepted safe contract calls for it.
- Resume uses `session.info`'s stored directory, performs existing integrity checks, and fails visibly
  if that directory is now missing; it does not fall back to the newly selected directory.
- Prove retries/new generations cannot reuse stale directory state.

### DS-3.5 — Acceptance package

DS-3 is locally complete only when:

- select, cancel, remembered path, stale remembered path, explicit switch, create, resume, retry, and
  restart pass through main and a live child;
- active prompt/permission/elicitation/confirmation blocks switching;
- no renderer request carries a path and no generic filesystem operation exists;
- two shell identities retain disjoint remembered directories;
- no global workspace record or Gosling setting changes;
- diagnostics contain only stable state/reason and redact the directory.

Rollback: remove the new capability declaration/handlers together and revert session creation to the
startup directory. Never leave a preload method registered without a main handler or vice versa.

## 6. DS-4 — Credential use without ownership

### DS-4.0 — Freeze credential policy and compatibility

Extend shell provisioning with an explicit credential policy that preserves current fixed behavior:

- `fixed`: the existing provisioned `credentialProfileId` is the only permitted profile;
- `selectable_catalog`: the shell may list and select safe summaries from Gosling's catalog;
- absence defaults to `fixed`/no selection and never implies catalog access;
- a fixed profile and selectable-catalog mode cannot be declared together;
- provider/model constraints in provisioning still apply to every selected profile.

Decide and record whether the optional compatible addition remains provisioning schema v1 or
requires v2. Update the exact profile/manifest compatibility version, generated schema, fixtures,
and migration matrix consistently; do not change only one surface.

**Exit:** ADR-0014/R1 contract amendment accepted; invalid combinations and compatibility behavior
are frozen.

### DS-4.1 — Add a Rust-owned safe catalog projection

- Define a shell-specific summary DTO containing only ID, name, provider/service ID, and normalized
  `configured`/`relink_required` status.
- Add a read-only shell request guarded by credential policy. Do not reuse the broad workspace
  `CredentialProfileListResponse` across the shell boundary.
- Sort deterministically, deduplicate by ID, cap entries/string sizes/total response bytes, and map
  catalog/storage failures to stable non-secret codes.
- Apply any provisioned provider constraint before returning summaries.
- Add the exact method to capability metadata and compatibility requirements only for consumers that
  declare credential selection.

**Primary tests:** secret sentinel values in every excluded field, oversized catalogs, duplicate
IDs, unsupported status, storage failure, fixed-mode denial, provider filtering, deterministic order,
and response bounds.

### DS-4.2 — Add main-owned selection state and local preference

- Main fetches only the safe shell DTO after compatibility/provisioning succeeds.
- Renderer receives an immutable bounded list plus selected ID/status; it receives no edit/create/
  delete/test/usage operations.
- Selection requests carry generation and an ID from the current safe list. Main rejects unknown,
  stale, duplicate, or policy-disallowed IDs.
- Persist only `preferredCredentialProfileId` in shell-local settings after acceptance.
- Re-read the backend catalog at startup, retry, selection, and immediately before session create.
  A missing or relink-required profile remains selected-but-invalid for honest recovery; no alternate
  profile is silently chosen.

### DS-4.3 — Pin selected credentials at session creation

- Extend the trusted main-to-ACP session launch metadata with the selected opaque profile ID; keep it
  out of renderer-controlled arbitrary metadata.
- Rust re-resolves the ID through `WorkspaceService`, checks policy, status, provider compatibility,
  and model validity, then builds the existing `ConfigResolutionScope`.
- Pin only safe ID/name/provider facts to the session; never persist resolved secret keys, values, or
  scoped configuration in shell state.
- Resume uses the session's existing pinned profile. If it was deleted/revoked, enter
  `relink_required`; do not substitute the current preference.
- Fixed provisioning continues to work byte-for-byte when selectable catalog is absent.

### DS-4.4 — Recovery and handoff

- Map missing, needs-authentication, provider mismatch, catalog unavailable, and scope-resolution
  failures to stable safe statuses and the existing `relink_required` lifecycle.
- Reuse the explicit server-prepared full-Gosling handoff for credential management. The shell itself
  never gains create/update/delete/test operations.
- After external relink, a fresh generation/retry reloads the safe catalog; no polling loop or global
  settings mutation is added.

### DS-4.5 — Acceptance package

DS-4 is locally complete only when:

- fixed and selectable modes, empty catalog, selection, persistence, session pin, revoke, relink,
  provider mismatch, restart, and resume are exercised through a live child;
- sentinel secrets and excluded metadata never appear in ACP shell responses, main snapshot, preload,
  settings, diagnostics, logs, errors, or fixture output;
- renderer cannot call broad credential/workspace configuration methods;
- two shells may reference the same Gosling profile while retaining separate preferences and without
  copying the credential;
- deletion/revocation in Gosling invalidates both references on revalidation.

Rollback: disable selectable-catalog policy and remove its method/capability as one compatibility
change. Existing fixed provisioning remains the safe fallback; never migrate a selection into copied
secret storage.

## 7. DS-5 — Declared module/backend composition

### DS-5.0 — Freeze the v1 module model

Define a safe `ShellModuleSummary` concept with namespaced identity, kind, status, version where
applicable, and declared capabilities. V1 sources are:

- `core:session` from verified ACP session capabilities;
- `extension:<name>` from the intersection of provisioning selection and live enabled extensions;
- `skill:<id>` from provisioned skill selection resolved through the skills extension;
- `adapter:<domainId>` from the one existing supervised domain-adapter descriptor and live status.

V1 explicitly supports many selected extensions/skills and at most one supervised domain adapter.
Adding an adapter collection is a later schema/lifecycle decision, not an implicit interpretation of
the singular current field.

Each consumer may declare modules it presents, but declaration grants no authority. Effective
availability is the intersection of product provisioning, live backend resolution, consumer
capability declaration, and protocol policy.

### DS-5.1 — Build the backend resolution report

- Extend shell provisioning validation/resolution or a dedicated safe read response so Rust reports
  only resolved module identity/capability/status, not extension configuration, commands,
  environment, tool arguments, skill bodies, adapter transport, or credentials.
- Treat undeclared, duplicate, missing, incompatible, and denied modules deterministically.
- Required module failure prevents `ready`; optional module failure projects `unavailable` only if
  the consumer contract explicitly supports optionality.
- Keep mutation confirmation, permission mediation, frame/response bounds, deadlines, supervision,
  and cleanup in their existing authorities.

### DS-5.2 — Project one safe main snapshot

- Replace the adapter-only presentation assumption with an immutable bounded module inventory while
  retaining the existing typed adapter operations.
- Module status notifications are generation-fenced and must match the verified module identity.
- Do not add `module.call`, arbitrary method names, backend URLs, process descriptors, or generic
  payload passthrough.
- Extension/skill entries are informational for v1; the agent invokes their tools under the existing
  prompt/permission flow.

### DS-5.3 — Extend consumer composition mechanically

- Add strict, sorted, duplicate-free required/optional module declarations only if needed by the
  module model; reject unknown fields and secret-shaped content as the resolver does today.
- Derive required ACP methods and renderer capabilities from declared typed operations rather than a
  second hand-maintained list.
- Bind domain renderer slots only when the live adapter descriptor exactly equals provisioning and
  consumer declarations.
- Preserve fixed host main/preload/Vite/Forge source hashes across consumers.

### DS-5.4 — Failure and recovery matrix

Exercise:

- no optional modules; one extension; multiple extensions; skill without skills extension;
- undeclared installed extension; declared but unprovisioned extension; duplicate/colliding IDs;
- adapter absent, wrong version/action set, pre-ready crash, idle crash, in-flight crash, hang,
  malformed/oversized response, restart, and forced cleanup;
- read-only adapter action and confirmed mutation with stale/replayed/cross-session confirmation;
- permission- or elicitation-requiring extension tool through the existing interaction controller;
- backend retry with no stale module status or orphaned adapter process.

### DS-5.5 — Acceptance package

DS-5 is locally complete only when every module shown to the renderer traces to a live verified
authority and every invokable operation traces to an existing typed route. No installed-but-
undeclared module is visible or callable; no generic backend router exists; and v1's one-adapter limit
is validated rather than assumed.

Rollback: a failing optional inventory projection may be removed while retaining current typed
adapter operations. Do not fall back to renderer-selected endpoints or unverified installed-module
discovery.

## 8. DS-6 — Neutral Default Shell scaffold and launcher identity

### DS-6.0 — Freeze scaffold inputs and outputs

The scaffold accepts only explicit safe identity inputs and source paths under the established
approved fixture roots. It emits:

- strict consumer manifest;
- non-publishable product profile;
- provisioning document with shell-owned instructions, explicit credential policy, and empty
  extension/skill/adapter selection by default;
- consumer-owned neutral conformance renderer entry;
- settings schema/version declaration or reference required by the accepted contract;
- target icon checklist and operator-supplied asset locations;
- conformance metadata/tests.

It does not emit secrets, production/release values, a named-domain prompt, a finished production
icon, updater configuration, signing policy, or an enabled developer extension.

### DS-6.1 — Implement a non-destructive scaffold command

- Follow existing dependency-free Node script/resolver patterns under `ui/desktop/scripts/`.
- Require an explicit destination under an approved root; canonicalize containment and reject
  symlink/traversal/absolute/NUL/backslash escapes.
- Refuse any existing destination or file. Never merge, overwrite, or delete user work.
- Validate all identity collisions and secret/domain-shaped values before writing anything.
- Stage output in a new sibling temporary directory, run the existing resolvers against it, and
  rename only after complete validation; clean up only that exact temporary directory on failure.
- Print a machine-readable created-file manifest and the remaining icon/input checklist.
- Add package scripts beside the existing `shell:check-*`, `shell:resolve-profile`, and
  `shell:package-local` commands.

### DS-6.2 — Add the neutral Default Shell conformance sample

- Keep it under current `fixtures/shell-consumers/` and `fixtures/shell-products/` roots and visibly
  test-only/non-publishable.
- Use unique neutral IDs, launcher names, protocol, bundle/app/package IDs, runtime namespace, and
  replaceable target icons.
- Supply a Default Shell-specific instruction profile; do not reuse Gosling's generic prompt.
- Start with no developer builtin, no selected extension/skill/adapter, and no named-domain copy.
- Declare only capabilities completed by DS-3–DS-5.
- The renderer entry remains a conformance surface, not the final GUI or shell kit.

### DS-6.3 — Prove launcher/icon/profile replacement

- Generate a second neutral scaffold with different identity and assets.
- Resolve/build both without editing host main/preload/Vite/Forge source.
- Prove path, browser partition, runtime namespace, logs, diagnostics, process registry, local
  settings, launcher/protocol, and package identity are disjoint.
- Prove missing/wrong-format/dimension/symlink/colliding icons fail through the existing asset
  validator.
- Verify the embedded manifest and OS metadata contain the selected profile values and no fallback
  Gosling/fixture-A identity where a product-specific value is required.

### DS-6.4 — Conformance command

One command must reject a scaffold before packaging if any of these are incomplete:

- profile, provisioning, consumer, renderer entry, required methods/capabilities, instruction
  profile, credential policy, settings schema version, target assets, or test fixture;
- secret-shaped or named-domain content in neutral shared surfaces;
- developer tools or undeclared modules;
- host source edits required to resolve/build the consumer;
- publish/update/sign/release fields enabled for the template.

The command runs strict profile/consumer resolution, schema/type generation checks, focused Desktop
contract tests, source-negative-space checks, and a dry package-resource projection. It produces a
bounded JSON report suitable for DS-7 evidence.

### DS-6.5 — Acceptance package

DS-6 is locally complete only when a fresh second neutral template is generated into a temporary
approved fixture location, passes conformance and a host-target package/readback, and is removed by
the test harness without touching tracked/user files. Re-running into the same destination must fail
without changing its hash.

Rollback: remove the scaffold entry command and neutral sample together while preserving the strict
profile/consumer resolvers. A failed generation never leaves a partially valid product directory.

## 9. DS-7 — Clean-revision nonvisual acceptance

### DS-7.0 — Resolve the exact review scope

- Inventory the dirty worktree and separate intended Default Shell changes from unrelated user work.
- Do not stage, commit, stash, reset, or discard unrelated changes without explicit operator
  authorization.
- Produce one reviewable revision containing the intended DS-1–DS-6 changes and documentation.
- Record exact full SHA, branch/PR, base revision, toolchain versions, target, and `sourceClean:true`.
- Regenerate ACP schema/SDK types on that revision and prove a second generation is byte-identical.

If an exact clean revision cannot be established, DS-7 is automatically NO-GO regardless of local
test results.

### DS-7.1 — Automated validation matrix

Run from the Hermit environment on the exact revision:

```bash
source bin/activate-hermit
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test -p gosling-sdk-types shell
cargo test -p gosling --lib prompt_manager
cargo test -p gosling --lib shell_validation
cargo test -p gosling-cli --test shell_provisioning_validation_test
cargo test -p gosling-cli --test shell_runtime_e2e_test
just check-acp-schema
pnpm --dir ui/sdk run build:ts
pnpm --dir ui/desktop run typecheck
pnpm --dir ui/desktop run shell:test-profile
pnpm --dir ui/desktop run shell:check-profiles
pnpm --dir ui/desktop exec vitest run src/shell
git diff --check
```

Add the exact DS-3–DS-6 integration/conformance test commands when their files exist; do not use a
name in evidence until it is present in the revision. Run the repository's broader required Rust and
Desktop suites if the change touches their shared surfaces or CI requires them.

### DS-7.2 — Security and negative-space review

On the same revision, verify:

- preload surface equals the accepted allowlist and reverse handler trace;
- renderer receives no Node/Electron, raw ACP/MCP address/token, generic path/settings/RPC, broad
  credential DTO, extension configuration, adapter transport, process authority, or secret;
- sentinel credential values/field names/provider parameters do not cross Rust shell response,
  generated SDK, main snapshot, preload, settings, logs, diagnostics, package resources, or errors;
- directory selection requires confirmation in the native chooser, is main-owned, canonicalized
  twice, generation-fenced, and does not mutate global workspace/config;
- module authority is an intersection rather than a union and mutation remains confirmed;
- scaffold refuses overwrite, traversal, symlinks, secrets, production release fields, named-domain
  content, and developer defaults;
- source contains no DAWES/math/Physics/CST implementation outside planning/negative-space text.

Any critical/high finding is a NO-GO and returns work to its owning DS package.

### DS-7.3 — Package/readback and coexistence

- Package the neutral Default Shell consumer on the current supported host target through
  `shell:package-local` with its explicit profile and consumer manifest.
- Run the independent verifier against binary hash, profile, consumer hash, renderer hash,
  provisioning, instruction/credential/module contract versions, exact resources, icons, metadata,
  permissions, CSP, updater absence, and process cleanup.
- Launch full Gosling plus two neutral shell identities where supported; prove locks, protocols,
  local settings, selected directories, credential preferences, sessions, logs, diagnostics, process
  registries, cache/temp, and adapter processes remain isolated as specified.
- Restart and resume; remove/revoke remembered directory/credential fixtures and confirm visible
  recovery without state substitution.

Unsigned host-target evidence is sufficient for DS-7. It is not signing, notarization, release, or
cross-platform readiness.

### DS-7.4 — Current CI binding

- Push the exact isolated revision only after operator authorization for repository mutation.
- Require current mandatory CI checks for that SHA; do not cite a prior green run.
- Confirm workflow jobs execute the intended generated-schema, Rust, Desktop, profile/consumer, and
  package/conformance surfaces rather than merely existing.
- Record run/job links and any skipped platform with its explicit later R6/R7 owner.

### DS-7.5 — Decision record

DS-7 is **GO for Default Shell GUI only** when all of the following are true:

1. foundation closure and DS-3–DS-6 exit criteria are satisfied;
2. every P0 SHP-REQ-044–053 item in this scope is revision-bound and green;
3. exact source and generated artifacts are clean and reproducible;
4. targeted security/negative-space review has no critical/high open finding;
5. host-target package/readback and coexistence/recovery evidence pass;
6. current mandatory CI is green for the exact SHA;
7. documentation, change control, risks, defects, traceability, and session evidence match code;
8. the operator explicitly accepts beginning the generic Default Shell renderer.

The decision is **NO-GO** if any condition is missing. A GO does not authorize a named shell,
production release, signing, publishing, updater activation, R6–R8 completion, or broader renderer
authority.

## 10. Work-package acceptance matrix

| Package            | Requirements           | Depends on                                                  | Primary proof                                                                      | Blocks                  |
| ------------------ | ---------------------- | ----------------------------------------------------------- | ---------------------------------------------------------------------------------- | ----------------------- |
| Foundation closure | 045, 047, 049, 053     | current DS-1/DS-2 work                                      | live prompt/tool isolation; settings migration/recovery/IPC                        | DS-7; DS-3/4 completion |
| DS-3               | 047, 048, 053          | foundation store/runtime                                    | native selection + Rust revalidation + live create/resume/switch                   | DS-5/6/7                |
| DS-4               | 046, 047, 053          | credential policy contract; DS-3 session-launch integration | source-safe catalog + live scope pin/revoke/relink                                 | DS-5/6/7                |
| DS-5               | 049, 051, 053          | DS-3/4 integrated session state                             | verified bounded module inventory and failure matrix                               | DS-6/7                  |
| DS-6               | 044, 045, 047, 049–053 | DS-3–DS-5                                                   | non-destructive scaffold, second neutral identity, conformance/package proof       | DS-7                    |
| DS-7               | 044–053                | all prior packages                                          | exact clean revision, security review, host package, current CI, operator decision | all GUI work            |

## 11. Implementation checkpoints and evidence discipline

Each implementation unit ends with:

1. intended-file diff and unrelated-worktree check;
2. format/lint/typecheck for touched languages;
3. focused unit and integration tests, including at least one negative-space test;
4. generated-schema/type reconciliation when canonical DTOs change;
5. traceability/risk/defect/status update using `planned` → `built` → `verified` accurately;
6. session log listing commands, results, partial status, and residual risk.

`built` means implementation exists with local evidence. `verified` is reserved for DS-7 exact-
revision evidence. A skipped, unavailable, or host-inapplicable check is recorded explicitly and is
never rounded up.

## 12. Open later gates

After DS-7 GO, implement only the generic Default Shell GUI in the order already recorded in
`architecture/default-shell-template.md`. M5 still owns renderer usability/accessibility acceptance.
R6 owns packaged restart/failure/coexistence depth, R7 cross-platform workflows, and R8 external
third-consumer/final readiness. Named shell implementation remains blocked until M5.
