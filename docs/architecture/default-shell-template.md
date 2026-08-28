# Default Shell template: pre-GUI contract and implementation plan

Status: DS-1 through DS-7 closed. The generic Default Shell GUI, including its declaration-gated
Outputs panel, is implemented at `ui/desktop/src/shell-ui/` and locally verified in the operator
checkout. A supported-host package/readback also passes. Exact-revision CI and the packaged
startup/close/restart/coexistence replay remain the final evidence gate. No named shell is
implemented.
Date: 2026-08-18
Authority: ADR-0007–0012 and ADR-0014–0015

Temporary UI direction (2026-08-19): the generic shell mirrors the normal Gosling desktop frame
and navigation while continuing to use the isolated shell preload/runtime. It intentionally omits
Settings, Skills, and Extensions navigation. Product-local settings remain an internal runtime and
recovery concern; the renderer exposes no settings editor and receives no global-settings authority.
The committed Default Shell template also uses fixed credential policy with no profile ID: it does
not enumerate or select Gosling credential profiles. Provider/API credentials are configured in the
main Gosling application; a product that needs a fixed profile must provision that opaque profile ID
explicitly.

Detailed execution for DS-3 through DS-7 is frozen in
[`../build/shell-productization/default-shell-ds3-ds7-implementation-plan.md`](../build/shell-productization/default-shell-ds3-ds7-implementation-plan.md).

## Outcome

The next MVP is a reusable, generic Default Shell template. It is a reduced Gosling application and
a superset of workspace chat: it can choose a working directory, discover/create/resume one session,
exchange prompts, repair a bounded transcript event gap, mediate permissions, elicitations, and
domain confirmations, show bounded status and recovery, use a
referenced Gosling credential profile, and optionally present declared backend modules. It does not
load developer tools by default and cannot edit global Gosling settings.

The implemented shell also has a declaration-gated input Library. Operators can link a supported
file through a main-owned native chooser or paste bounded text/image content, choose project or
session scope, and attach up to 16 opaque references to a prompt. Rust owns storage, scope checks,
bounded PDF and Office extraction, image normalization, and content resolution. The renderer never
receives a linked path.

This document is the implementation plan for everything that must be true before its GUI is built.
DAWES, math, Physics/CST, and all other named shells remain outside the milestone.

## Ownership matrix

| Surface            | Owner                                              | Renderer may receive                                                        | Renderer must never receive                                           |
| ------------------ | -------------------------------------------------- | --------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Launcher and icon  | product profile/package                            | verified display identity and safe icon asset                               | bundle/protocol mutation authority                                    |
| Shell instructions | provisioning                                       | no raw prompt is required                                                   | Gosling global prompt control                                         |
| Credentials        | Gosling protected credential service               | profile ID, label, provider/service, status, relink action                  | secret value, token, environment, config scope                        |
| Local settings     | product-specific main process store                | the fixed safe settings document                                            | arbitrary keys, global settings API, credential values                |
| Working directory  | main-owned native selection and backend validation | selected display path and stable workspace/session facts                    | arbitrary filesystem operations                                       |
| Tools and skills   | provisioning plus backend validation               | declared availability and permission requests                               | undeclared builtin, raw MCP/ACP connection                            |
| Optional modules   | Gosling extension or supervised adapter contracts  | declared bounded snapshots/actions                                          | arbitrary process launch or RPC routing                               |
| Conversation       | Gosling session runtime                            | bounded updates/replay, session/provider/model facts, interaction summaries | server secret, transport endpoint, private reasoning, raw tool values |
| Input library      | Gosling session store plus Electron main           | opaque ID, safe metadata, scope/status, selection state                     | linked path, listed payload, generic file operation                   |

“Read credentials” means that the backend may resolve a selected profile and use the credential for
a provider request. The shell UI reads only safe catalog metadata; it never reads secret bytes.

## Default v1 capability envelope

Required capabilities:

- lifecycle and compatibility status;
- native working-directory selection with cancel and inaccessible-path handling;
- one active session, bounded current-directory discovery, create/resume, prompt submit/cancel,
  history/live delivery markers, monotonic update sequence, and bounded transcript repair;
- explicit permission, elicitation, and domain-confirmation responses with sufficient safe context;
- stable operation-failure codes and recovery actions that preserve a prompt draft when retrying is
  safe;
- safe credential-profile listing/selection/status and full-Gosling relink handoff;
- diagnostics export and explicit full-Gosling handoff;
- local theme, text scale, last working directory, and preferred credential-profile reference;
- product-specific launcher identity and replaceable icon assets.

Optional, declaration-gated capabilities:

- project/session reference library for linked Office/PDF/PostScript/text/data/image files and
  pasted text/images,
  resolved only into selected prompt content;
- selected Gosling extensions and skills;
- one or more supervised domain/backend adapters through versioned descriptors;
- consumer-owned, data-only renderers for declared module payloads.

Explicitly absent from v1:

- developer tools unless a product explicitly declares and provisions them;
- global Gosling setting mutation, credential creation/editing, arbitrary file access, arbitrary
  backend URLs, process spawning, generic IPC/RPC, updater controls, plugin installation, multiple
  concurrent sessions, autonomous background execution, or named-domain behavior.

## Pre-GUI work packages

### DS-0 — Freeze and trace the template contract

Deliver ADR-0014, this plan, requirements SHP-REQ-044–053, and a change-control record. Resolve any
conflict by keeping historical revision-bound PG-50 evidence distinct from the current worktree.

Exit: every requested capability has one authority, one safe projection, one negative-space rule,
and one planned acceptance oracle.

### DS-1 — Instruction and tool isolation

Add a bounded optional shell instruction profile to canonical provisioning. Apply it before the
first prompt and load project instructions without Gosling global hints. Preserve full Gosling's
existing prompt behavior. Make empty shell builtins resolve to empty while non-shell serve retains
its existing developer default.

Exit: tests prove shell prompt replacement, project instruction inclusion, global-hint exclusion,
invalid prompt rejection, empty shell tools, and unchanged non-shell defaults.

Current source: implemented with targeted and merged-main CI evidence; final DS-7 revision binding
remains.

### DS-2 — Product-scoped settings service

Keep a strict schema-v1 document under each product's `userData` root. Use private permissions,
bounded reads/writes, atomic replacement, deterministic defaults, strict unknown-field rejection,
and explicit schema migration. Provide narrow typed main operations only after the store tests pass;
do not expose a generic settings bridge.

Exit: two identities cannot observe or overwrite each other's settings; malformed, oversized,
unknown, secret-shaped, relative-path, interrupted-write, old-schema, and permission cases pass.

Current source: schema, isolated path, parser, atomic reader/writer, schema-recovery load, narrow
per-field store, and typed `settings.read`, `settings.appearance.update`, and confirmed
`settings.reset` operations are implemented. Tests cover bounds, identity isolation, recovery-state
refusal/reset, interrupted temporary writes, and permission-denied writes without losing the last
committed document. Cross-platform packaged interruption depth remains R6.

### DS-3 — Working-directory authority

Add a main-owned native directory chooser requiring an explicit user gesture. Canonicalize and
validate the selection through the existing source/workspace policy before session creation. Return
cancel as a normal result and inaccessible, removed, symlink-escape, or disallowed paths as bounded
errors. Persist only the validated last directory in shell-local settings.

Exit: typed IPC/preload negative-space tests and integration tests cover select, cancel, stale
generation, invalid sender, removed/inaccessible directory, relaunch, and workspace/session pinning.

Current source: built. `_gosling/unstable/shell/directory/validate` canonicalizes without side
effects; `directory.select`/`session.detach` are typed main-owned operations that never accept a
renderer path; session creation uses the accepted canonical path and the backend canonicalizes
again. Local Rust, Desktop, and live-child evidence passes; revision-bound acceptance remains.

### DS-4 — Credential metadata and use-without-ownership

Expose a bounded safe catalog projection from the existing backend credential service: opaque ID,
display label, provider/service ID, configured/relink-required status, and no other fields. Selection
stores only the opaque ID; session creation re-resolves it and pins the provider scope in the
backend. Missing/revoked/mismatched profiles fail closed and offer explicit full-Gosling relink.

Exit: sentinel secrets never cross ACP, main snapshot, preload, diagnostics, settings, or logs;
revoke, relink, provider mismatch, stale selection, and shell coexistence tests pass.

Current source: built. `session.credentialPolicy` gates a four-field Rust-owned safe projection
behind `_gosling/unstable/shell/credentials/list`; selection persists only an opaque ID and the
backend re-resolves it immediately before session creation. Catalog access runs outside the async
worker with a bounded fail-closed result, and provisioning does not touch credential storage when
no fixed profile is declared. Sentinel-secret, unknown/revoked/relink/mismatch, configured-profile
pinning, and packaged unavailable-state evidence pass locally; revision-bound acceptance remains.

### DS-5 — Module and backend composition

Define a single registry view that merges provisioned Gosling extensions/skills and supervised
adapter descriptors into declared shell capabilities. Keep lifecycle, compatibility, size/time
bounds, permission mediation, mutation confirmation, and cleanup in existing backend/main
authorities. The consumer may render declared data but cannot introduce a backend endpoint.

Exit: no undeclared module becomes reachable; absent, incompatible, crashing, hanging, malformed,
overproducing, permission-requiring, and mutating modules have deterministic recovery behavior.

Current source: built. `_gosling/unstable/shell/modules/list` reports the intersection of
provisioned selection and live backend resolution as one bounded inventory; v1's one-adapter limit
is explicit rather than assumed. Local unit and live-child evidence passes; the full DS-5.4 crash,
hang, and malformed-adapter matrix remains for R6.

### DS-6 — Launcher, icon, identity, and scaffold

Create a neutral Default Shell consumer/profile/provisioning sample through the existing strict
resolver. Its launcher names and icons are profile-owned and replaceable without host source edits.
The scaffold includes its own instruction profile, empty extension list, local-settings schema
version, capability declaration, asset checklist, and conformance command. It must remain visibly a
development template, not a production or named-domain product.

Exit: generating a second neutral shell changes only consumer/profile/provisioning/assets; host
source hashes stay fixed and the strict resolver rejects incomplete or colliding identities/assets.

Current source: built. `pnpm run shell:scaffold` generates a non-destructive neutral template into
an approved root through a staged temporary directory, and `pnpm run shell:conformance` refuses to
certify an incomplete one. A second fresh neutral identity is generated and validated without host
source edits. `fixtures/shell-{products,consumers}/default-shell-template` is the committed neutral
sample, and macOS arm64 package/readback has been reproduced during DS-7.

### DS-7 — Nonvisual acceptance gate

Run targeted Rust, SDK, Desktop, profile, consumer, package-readback, security negative-space, and
documentation checks on one exact clean revision with current CI. Record failures rather than
rounding local partial evidence up to acceptance.

Exit: DS-1–DS-6 are revision-bound and green; no critical/high open finding applies; the operator
accepts GUI implementation. Only then begin the Default Shell renderer. Named shells remain blocked
until the generic GUI passes M5.

Current status: **accepted 2026-08-18.** The operator recorded explicit DS-7 acceptance in
[`../build/shell-productization/audits/ds-7-operator-acceptance.md`](../build/shell-productization/audits/ds-7-operator-acceptance.md),
authorizing `plan-webapp-design` Gate 1 (product/workflow design) and Gate 2 (front-end handoff) for
the generic Default Shell. Renderer implementation stays closed until Gate 3, whose entry condition
is SHP-DEF-053 — reproducing the DS-7 check battery on a current clean revision, because `main` has
advanced 76 commits from the accepted revision and one of them touched shell runtime source. Named
shells remain blocked until M5. The accepted technical evidence follows.

The final corrective source is committed at
`259935f01b1fbf0bcffcb17f21a01a7f9c2548fc` and merged to `main` as
`240ab751585afc03c68a710f8be10ea891ab168f`; the merge has the identical source tree. Exact-source
Rust and Desktop validation passes, all fixture profiles resolve the corrective commit with
`sourceClean:true`, and a fresh macOS arm64 package/readback independently matches profile hash
`830f6143a45ea309c42f03cb440410b3eb6484009c86cda4aa98f0a7e1282950` and embedded backend hash
`76b812b5677520b5c8a564b4251cda7113f47277dcd4f0ff0eb34cd53f3d6574`. Mandatory CI is green for
both the exact PR head and merged-main tree; the one merged-main Desktop failure was an unrelated
i18n lock-test race and passed on the failed-job rerun. Main's Live Provider Tests also pass. No
critical or high Default Shell finding remains open. The technical evidence therefore supports a
GO recommendation for generic GUI planning, and the operator's acceptance above closed the gate.

## Design gate status

| Gate                                  | State                                                                                                    | Deliverable                                                                                                                                           |
| ------------------------------------- | -------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- |
| Gate 1 — product and workflow design  | complete 2026-08-18                                                                                      | [`../build/shell-productization/gui/gate-1-product-workflow-design.md`](../build/shell-productization/gui/gate-1-product-workflow-design.md)          |
| Gate 2 — front-end handoff            | complete 2026-08-18                                                                                      | [`../build/shell-productization/gui/gate-2-frontend-handoff.md`](../build/shell-productization/gui/gate-2-frontend-handoff.md)                        |
| Gate 3 — build                        | locally complete 2026-08-18                                                                              | [`../build/shell-productization/gui/gate-3-build-record.md`](../build/shell-productization/gui/gate-3-build-record.md) and `ui/desktop/src/shell-ui/` |
| Gates 4-6 — integrate, validate, ship | local integration and package/readback complete; exact-revision CI and packaged lifecycle replay pending | [`../build/shell-productization/gui/gate-3-build-record.md`](../build/shell-productization/gui/gate-3-build-record.md)                                |

## GUI implementation order after DS-7

1. lifecycle/recovery frame, verified product identity, and draft-preserving error recovery;
2. working-directory and credential-profile selection;
3. bounded session picker plus create/resume and explicit provider/model context;
4. single-session conversation, history/live reconciliation, transcript-gap repair, and prompt
   cancellation;
5. permission, elicitation, and domain-confirmation surfaces;
6. durable session Outputs inventory through the existing artifact guard, without directory scans;
7. reduced local settings and optional declared module slots;
8. diagnostics and full-Gosling handoff;
9. accessibility, restart, coexistence, and packaged failure matrix.

No named shell begins merely because the Default Shell scaffold exists. The reusable GUI and its
conformance workflow must pass M5 first.

## Additional design requirements

- Schema migration and reset must be explicit; corrupt local settings fall back only through a
  user-visible recovery path, not silent data loss.
- First-run, offline, missing-credential, missing-directory, incompatible-core, and adapter-failure
  states need honest copy and recovery actions before visual polish.
- Telemetry is off unless separately designed and accepted; diagnostics remain user-initiated and
  redacted.
- Deep links and launcher invocations cannot select credentials, tools, modules, or mutation intent.
- Accessibility, reduced motion, zoom, keyboard order, focus restoration, and minimum window size
  are acceptance requirements, not post-MVP cleanup.
- Product uninstall/reset must remove only that shell's state and never Gosling's credential catalog
  or another shell's settings.
- Gemini configuration uses the `Google Gemini (API Key)` provider. Provider setup/relink workflow
  testing must cover the API-key path before claiming a polished credential-selection experience.
