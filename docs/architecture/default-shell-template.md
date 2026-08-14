# Default Shell template: pre-GUI contract and implementation plan

Status: active pre-GUI foundation; no Default Shell GUI or named shell is implemented
Date: 2026-08-14
Authority: ADR-0007–0012 and ADR-0014

Detailed execution for DS-3 through DS-7 is frozen in
[`../build/shell-productization/default-shell-ds3-ds7-implementation-plan.md`](../build/shell-productization/default-shell-ds3-ds7-implementation-plan.md).

## Outcome

The next MVP is a reusable, generic Default Shell template. It is a reduced Gosling application and
a superset of workspace chat: it can choose a working directory, create or resume one session,
exchange prompts, mediate permissions and elicitations, show bounded status and recovery, use a
referenced Gosling credential profile, and optionally present declared backend modules. It does not
load developer tools by default and cannot edit global Gosling settings.

This document is the implementation plan for everything that must be true before its GUI is built.
DAWES, math, Physics/CST, and all other named shells remain outside the milestone.

## Ownership matrix

| Surface            | Owner                                              | Renderer may receive                                       | Renderer must never receive                            |
| ------------------ | -------------------------------------------------- | ---------------------------------------------------------- | ------------------------------------------------------ |
| Launcher and icon  | product profile/package                            | verified display identity and safe icon asset              | bundle/protocol mutation authority                     |
| Shell instructions | provisioning                                       | no raw prompt is required                                  | Gosling global prompt control                          |
| Credentials        | Gosling protected credential service               | profile ID, label, provider/service, status, relink action | secret value, token, environment, config scope         |
| Local settings     | product-specific main process store                | the fixed safe settings document                           | arbitrary keys, global settings API, credential values |
| Working directory  | main-owned native selection and backend validation | selected display path and stable workspace/session facts   | arbitrary filesystem operations                        |
| Tools and skills   | provisioning plus backend validation               | declared availability and permission requests              | undeclared builtin, raw MCP/ACP connection             |
| Optional modules   | Gosling extension or supervised adapter contracts  | declared bounded snapshots/actions                         | arbitrary process launch or RPC routing                |
| Conversation       | Gosling session runtime                            | bounded updates, prompt state, interaction summaries       | server secret or transport endpoint                    |

“Read credentials” means that the backend may resolve a selected profile and use the credential for
a provider request. The shell UI reads only safe catalog metadata; it never reads secret bytes.

## Default v1 capability envelope

Required capabilities:

- lifecycle and compatibility status;
- native working-directory selection with cancel and inaccessible-path handling;
- one active session, create/resume, prompt submit/cancel, and bounded conversation updates;
- explicit permission and elicitation responses;
- safe credential-profile listing/selection/status and full-Gosling relink handoff;
- diagnostics export and explicit full-Gosling handoff;
- local theme, text scale, last working directory, and preferred credential-profile reference;
- product-specific launcher identity and replaceable icon assets.

Optional, declaration-gated capabilities:

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

Current worktree: implemented with targeted local tests passing; full matrix and revision-bound
acceptance remain.

### DS-2 — Product-scoped settings service

Keep a strict schema-v1 document under each product's `userData` root. Use private permissions,
bounded reads/writes, atomic replacement, deterministic defaults, strict unknown-field rejection,
and explicit schema migration. Provide narrow typed main operations only after the store tests pass;
do not expose a generic settings bridge.

Exit: two identities cannot observe or overwrite each other's settings; malformed, oversized,
unknown, secret-shaped, relative-path, interrupted-write, old-schema, and permission cases pass.

Current worktree: schema, isolated path, parser, reader/writer, and initial bounds/isolation tests
implemented and passing locally; IPC, migration, and crash-interruption proof remain.

### DS-3 — Working-directory authority

Add a main-owned native directory chooser requiring an explicit user gesture. Canonicalize and
validate the selection through the existing source/workspace policy before session creation. Return
cancel as a normal result and inaccessible, removed, symlink-escape, or disallowed paths as bounded
errors. Persist only the validated last directory in shell-local settings.

Exit: typed IPC/preload negative-space tests and integration tests cover select, cancel, stale
generation, invalid sender, removed/inaccessible directory, relaunch, and workspace/session pinning.

Current worktree: not implemented.

### DS-4 — Credential metadata and use-without-ownership

Expose a bounded safe catalog projection from the existing backend credential service: opaque ID,
display label, provider/service ID, configured/relink-required status, and no other fields. Selection
stores only the opaque ID; session creation re-resolves it and pins the provider scope in the
backend. Missing/revoked/mismatched profiles fail closed and offer explicit full-Gosling relink.

Exit: sentinel secrets never cross ACP, main snapshot, preload, diagnostics, settings, or logs;
revoke, relink, provider mismatch, stale selection, and shell coexistence tests pass.

Current worktree: backend reference resolution and session pinning exist; safe catalog projection
and focused shell operation are not implemented.

### DS-5 — Module and backend composition

Define a single registry view that merges provisioned Gosling extensions/skills and supervised
adapter descriptors into declared shell capabilities. Keep lifecycle, compatibility, size/time
bounds, permission mediation, mutation confirmation, and cleanup in existing backend/main
authorities. The consumer may render declared data but cannot introduce a backend endpoint.

Exit: no undeclared module becomes reachable; absent, incompatible, crashing, hanging, malformed,
overproducing, permission-requiring, and mutating modules have deterministic recovery behavior.

Current worktree: extension selection and one supervised domain-adapter contract exist; the generic
registry projection and multiple-module policy remain to be specified and tested.

### DS-6 — Launcher, icon, identity, and scaffold

Create a neutral Default Shell consumer/profile/provisioning sample through the existing strict
resolver. Its launcher names and icons are profile-owned and replaceable without host source edits.
The scaffold includes its own instruction profile, empty extension list, local-settings schema
version, capability declaration, asset checklist, and conformance command. It must remain visibly a
development template, not a production or named-domain product.

Exit: generating a second neutral shell changes only consumer/profile/provisioning/assets; host
source hashes stay fixed and the strict resolver rejects incomplete or colliding identities/assets.

Current worktree: existing neutral consumer/product fixtures prove parts of composition and identity;
the Default Shell scaffold command and sample are not implemented.

### DS-7 — Nonvisual acceptance gate

Run targeted Rust, SDK, Desktop, profile, consumer, package-readback, security negative-space, and
documentation checks on one exact clean revision with current CI. Record failures rather than
rounding local partial evidence up to acceptance.

Exit: DS-1–DS-6 are revision-bound and green; no critical/high open finding applies; the operator
accepts GUI implementation. Only then begin the Default Shell renderer. Named shells remain blocked
until the generic GUI passes M5.

Current worktree: not accepted. Historical PG-50 evidence applies only to its recorded revision;
the current local additions require fresh validation and CI binding.

## GUI implementation order after DS-7

1. lifecycle/recovery frame and verified product identity;
2. working-directory and credential-profile selection;
3. single-session conversation and prompt cancellation;
4. permission and elicitation surfaces;
5. reduced local settings;
6. optional declared module slots;
7. diagnostics and full-Gosling handoff;
8. accessibility, restart, coexistence, and packaged failure matrix.

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
