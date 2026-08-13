# Project-shell R1 contracts (consumer, application-runtime, domain-adapter)

Status: proposed — companion schema freeze for ADR-0010, ADR-0011, ADR-0012; pending operator
acceptance per `pre-gui-backend-implementation-plan.md` PG-14/PG-15
Date: 2026-08-13
Authority: proposed extension of `shell-productization-contracts.md` (accepted Gate 2 contract);
this document freezes only the new R1 inputs/boundaries and does not itself implement or claim any
module exists yet.

This document exists so R2–R4 implementation (PG-20/PG-30/PG-40) has one frozen target instead of
improvising field names during coding, per the parent plan's rule that "the exact schema is frozen
in R1, not improvised during R2." Nothing here supersedes the accepted Gate 2 contract; the eight
existing channels, product-profile schema v1, lifecycle states, diagnostic contract, and threat model
in `shell-productization-contracts.md` remain in force and are extended, not replaced.

## Consumer manifest v1 (ADR-0010)

Strict, source-controlled, sibling to the product profile rather than a field on it:

```text
schemaVersion: 1
consumerId: lower kebab identifier, 3..48 characters
requiredShellKit: exact version or `current` for an in-tree fixture (semver ranges unsupported, per SHP-ASM-012)
productProfilePath: repository-relative or consumer-relative JSON path (existing ResolvedShellProductProfile)
rendererEntry: path under an approved consumer root; rejected if it resolves outside that root, is a symlink, or targets a host main/preload file
domainAdapter (optional)
  descriptorId: stable ID matching a DomainAdapterDescriptor.domain_id
  protocolVersion: exact version string
declaredCapabilities: exact sorted set of application-runtime operations (§ below) and domain actions the renderer will invoke
assetsRoot (optional): repository-relative or consumer-relative non-symlink directory
testFixturesRoot (optional): same containment rule as assetsRoot
```

Rejected content, in addition to everything the product profile already rejects: secrets, shell
commands, arbitrary URLs, native-library paths, main/preload replacement paths, release/signing
credentials, any `approvedRoot`-shaped field (the approved root is trusted resolver configuration,
never manifest content — see ADR-0010), and any `declaredCapabilities` entry that does not name a
frozen operation from this document. The manifest is resolved and canonically hashed the same way as
the product profile (`shell-profile.js` pattern); the resolved hash is embedded in
`ShellBuildManifest` alongside `profileHash`. Unknown fields and unknown `schemaVersion` fail closed
(`CONSUMER_SCHEMA_UNSUPPORTED`).

## Safe runtime snapshot v2 (ADR-0011)

Extends (does not replace) the existing `runtime.read`/`runtime.changed` payload:

```text
generation, lifecycleState, reasonCode, allowedActions          # unchanged from Gate 2 contract
identity: { id, displayName, version }                            # newly exposed, already computed during compatibility check
runtimeNamespace: string | null                                   # null until R3 adds it to ShellIdentity/checkShellCompatibility; only ever non-null once genuinely verified, never trusted-but-unlabeled
compatibility: { expectedVsActual summary, no raw values }
provisioningIssues: [{ code, path }]                              # codes only, matches existing diagnostic redaction
session: { sessionId, status: none|creating|active|resuming|closing, resumeKind: fresh|resumed, promptAttempt: { id, phase: idle|streaming|cancelling } | null }
adapter: { descriptorId, version, capabilities: string[], status: absent|negotiating|ready|incompatible|crashed } | null
pendingInteractions: [{ actionId, kind: permission|elicitation|confirm, summary (allowlisted fields only), expiresAtGeneration }]
```

Total bound remains <=64 KiB as today. No ACP endpoint, token, raw config, private path, prompt
history, or raw error ever appears here — this is a hard carry-over from the accepted threat model,
not a new relaxation.

### Reconnect and generation semantics

This freezes the reconnect gap PG-12/PG-32 name, reusing the existing Gate 2 rule rather than
inventing a parallel mechanism: `offline → stopping → booting` already assigns a **fresh
generation** on every retry, and "events from older generations are ignored." R1 makes the
consequence for session/prompt/interaction state explicit, since the Gate 2 contract predates
`session`/`promptAttempt`/`pendingInteractions` existing at all:

- A generation change (any retry/reconnect) invalidates every prompt attempt and pending
  interaction from the prior generation — they are not carried over, replayed, or silently
  duplicated. `session.updates`, `permission.requested`/`elicitation.requested`/
  `confirmation.requested` events already stop firing once their generation is stale, per the
  existing rule; this ADR adds no new delivery guarantee beyond that.
- A live conversation survives a reconnect only via the existing `session.resume` operation: after
  `booting` completes on the new generation, the renderer calls `session.resume` with the known
  `sessionId`. The resulting snapshot's `session.resumeKind` is `resumed` (as opposed to `fresh` for
  `session.create`), and `promptAttempt` is always `null` immediately after resume — any prompt that
  was mid-flight when the disconnect happened is not resubmitted automatically; the renderer must
  call `prompt.submit` again if it wants to continue.
- `session.updates` carries a monotonic per-session `updateSeq` (added to the event payload) that
  resets to 1 on every `session.create`/`session.resume`. This gives the renderer a way to detect a
  gap (a jump in `updateSeq`) without this contract committing to a replay/backfill buffer, which is
  not required for the R4 neutral-fixture conformance workflow and would be scope creep to freeze
  now without an implementation to validate it against.
- "Compacted resume" (an ACP-level session resume that may drop or summarize prior history, already
  reachable through the existing `resumeSession`/`loadSession` call in `acpRuntime.ts`) is exposed to
  the renderer through the same `resumeKind: resumed` signal; this contract does not distinguish
  compacted from non-compacted resume at the snapshot level, since the renderer has no action that
  differs between them — only R6's packaged failure matrix needs that distinction, for diagnostics.

## Application-runtime operations and events (ADR-0011)

Added to the existing eight-channel allowlist; each invoke carries generation and, where applicable,
session ID and a prompt-attempt or action nonce, validated exactly as `runtime.retry`/`runtime.stop`
already are:

| Operation | Direction | Input bound | Output/event bound | Main behavior |
| --- | --- | --- | --- | --- |
| `session.create` | invoke | none beyond generation | typed result <=8 KiB | main/server-owned working directory only |
| `session.resume` | invoke | session ID | typed result <=8 KiB | rejects unknown/foreign session ID |
| `prompt.submit` | invoke | bounded text, <=64 KiB | prompt-attempt ID <=1 KiB | rejects if an attempt is already outstanding |
| `prompt.cancel` | invoke | prompt-attempt ID | status <=1 KiB | idempotent no-op success if the named ID is the current attempt (whether or not it was already cancelled); rejects with a stale-ID error if the ID was never issued or belongs to a different session/generation |
| `session.updates` | event | main only | bounded update <=64 KiB, attempt-fenced, carries monotonic per-session `updateSeq` (resets on create/resume) | streamed model/tool output |
| `permission.respond` | invoke | action ID + allow_once\|deny | status <=1 KiB | rejects replay/expired/foreign action ID |
| `elicitation.respond` | invoke | action ID + submit(bounded fields)\|cancel | status <=8 KiB | rejects replay/expired/foreign action ID |
| `permission.requested` | event | main only | action summary <=8 KiB | opaque action ID, allowlisted fields only |
| `elicitation.requested` | event | main only | action summary <=8 KiB | opaque action ID, allowlisted fields only |
| `domain.snapshot` | invoke | none beyond generation | bounded payload <=64 KiB (matches the safe-snapshot ceiling; the R4 neutral fixture's snapshot must fit this bound, and an adapter that would exceed it fails as overproducing) | only when `adapter.status == ready` |
| `domain.action` | invoke | action name + args, <=16 KiB | bounded payload <=64 KiB, or `CONFIRMATION_REQUIRED` + interaction actionId for an unapproved `mutate` action | `read` actions execute directly; `mutate` actions execute only once their `confirm` interaction (below) is approved |
| `confirmation.requested` | event | main only | interaction summary <=8 KiB | opaque action ID, allowlisted action-name/args summary only |
| `confirmation.respond` | invoke | action ID + approve\|deny | status <=1 KiB | main relays to the Rust server, which rejects replay/expired/foreign action ID and, on approve, mints a single-use token and returns it; main relays that opaque value straight into the follow-up `perform_domain_action` call without inspecting, storing, or being able to mint one itself |

Explicitly still absent, unchanged from the Gate 2 contract: arbitrary file/settings/clipboard/
notification/updater access, raw ACP URL/token, MCP proxy URL, server secret, and arbitrary IPC
channels. `declaredCapabilities` in the consumer manifest must be a subset of this table's operation
names (plus adapter action names once ADR-0012's fixture exists); an undeclared invoke is rejected
before dispatch (PG-INV-004).

### Capability negotiation mapping (one canonical source, not two)

The Gate 2 contract already has a hand-authored `compatibility.requiredMethods` field on the product
profile (`shell-productization-contracts.md` §"Product-profile schema v1"). Introducing
`declaredCapabilities` on the consumer manifest without relating the two would leave two independent,
possibly-disagreeing lists — exactly what PG-INV-004 forbids. This freezes one direction of
derivation instead: **`declaredCapabilities` is canonical; `requiredMethods` is generated from it**,
never independently hand-authored once a consumer manifest exists.

```text
session.create, session.resume        -> _gosling/unstable/session/info, plus standard ACP session methods
prompt.submit, prompt.cancel          -> standard ACP prompt/cancel methods (already required unconditionally)
permission.respond                    -> standard ACP requestPermission response path (already required unconditionally)
elicitation.respond                   -> standard ACP unstable_createElicitation response path (already required unconditionally)
domain.snapshot                       -> _gosling/unstable/shell/domain/snapshot
domain.action, confirmation.respond   -> _gosling/unstable/shell/domain/action
```

This table is exhaustive for the operations frozen in this document; R2's manifest resolver computes
`requiredMethods` as the union of the right-hand entries for every operation the manifest declares in
`declaredCapabilities`, plus the always-required baseline (`_gosling/unstable/session/info` and shell
provisioning read/validate, per the existing Gate 2 compatibility contract) — it does not accept a
hand-typed `requiredMethods` list that could drift from `declaredCapabilities`. A profile used without
a consumer manifest (the current fixed neutral renderer, and any product that predates ADR-0010)
keeps hand-authored `requiredMethods` unchanged, since it declares no `declaredCapabilities` to derive
from. `ready` requires both the derived method set and the live adapter's declared actions (ADR-0012)
to match what was actually negotiated — a manifest cannot declare a capability the derivation table
does not know how to translate into a method check.

## Interaction records (ADR-0011)

```text
actionId: opaque, unguessable, unique per request
kind: permission | elicitation | confirm
generation, sessionId, promptAttemptId: fencing keys
issuedAt: monotonic counter (not wall-clock, to keep event ordering testable)
expiresWith: promptAttempt end | session end | explicit cancel
status: pending | resolved | expired | superseded
```

Ownership follows the authority boundary each kind already belongs to, not one shared store:

- `permission` and `elicitation` interactions are **main-owned**: Electron main is the ACP client
  that receives these ACP-protocol callbacks (`clientCallbacks()` in `acpRuntime.ts`), so main holds
  the pending-record table and mediates the response directly.
- `confirm` interactions are **Rust-server-owned**, per ADR-0012's server-owned-authorization rule
  and the parent plan's binding boundary that domain-operation authorization stays in Rust/backend
  services (`project-shell-readiness-plan.md` §3.3). Electron main relays `confirmation.requested`/
  `confirmation.respond` to and from the server over the existing authenticated ACP channel (a new
  custom method alongside `domain_snapshot`/`perform_domain_action`); main holds no pending-record
  state of its own for this kind and cannot resolve it locally. It is not tied to a
  `promptAttemptId` the way permission/elicitation interactions are, since a mutation can be
  requested outside an active prompt attempt; it fences on `generation`/`sessionId` only.

A `respond` call is accepted exactly once per `actionId`; every subsequent call for the same ID
returns a stale-action error rather than re-executing the response. For `confirm`, that exactly-once
check is enforced by the Rust server, not by main.

## Domain adapter descriptor and confirmation token (ADR-0012)

```text
DomainAdapterDescriptor (existing, v1)   # crates/gosling-sdk-types/src/shell.rs:60-66
  domain_id, display_name, version, actions: Vec<String>   # flat action names only

DomainAdapterDescriptor (proposed v2)    # NOT yet implemented; requires an explicit R4 DTO migration
  domain_id, display_name, version, actions: [{ name, kind: read|mutate, schemaRef }]
```

The existing v1 descriptor's `actions` field is a flat `Vec<String>` — it carries no `kind`, so a
runtime cannot yet decide which actions require a confirmation token from the descriptor alone. R4
must version this DTO (schema bump on `DomainAdapterDescriptor`, with the client/server compatibility
check in `compatibility.ts` extended accordingly) before the confirmation-token authorization rule
below can be enforced; until that migration lands, the authorization rule is a frozen target, not an
implementable one, and no R4 work may claim it works against the current v1 shape.

New, R4-scoped:

```text
AdapterRegistration             # lives in user/operator-owned Gosling settings, NOT gosling-cli source, NOT the consumer manifest
  domainId: matches DomainAdapterDescriptor.domain_id exactly — the join key ShellProvisioning.domain_adapter references
  cmd, args, envs, envKeys, timeout, cwd: identical shape to the existing ExtensionConfig::Stdio fields
                                          (crates/gosling/src/agents/extension.rs:178-200) — no new field vocabulary
  maxMessageBytes: bounded positive integer (new; MCP itself has no message-size ceiling)

ActionConfirmationToken        # minted and validated by the Rust server only; the value itself is an opaque
                                # single-use bearer string that main relays (never mints/forges/validates) — see below
  actionName, adapterVersion, sessionId, generation: binds the token to exactly one action/session/generation
  nonce: single-use, server-minted
  expiresAtGeneration: fails closed on generation rollover
```

**Resolution mapping.** `ShellProvisioning.domain_adapter: Option<DomainAdapterDescriptor>` and the
consumer manifest's `domainAdapter.descriptorId` both identify an adapter by `domain_id` — an ID
reference only, exactly like `ShellExtensionSelection.name` already is for extensions
(`crates/gosling-sdk-types/src/shell.rs:35-39`); neither carries an executable path, and none may
(provisioning already forbids arbitrary commands/paths, and the manifest's threat model forbids
native-library/executable paths for the same reason a shell command would be rejected).
`build_shell_runtime` resolves `domain_id` against an `AdapterRegistration` entry in the **same
Gosling settings surface that already registers MCP extensions** — a per-installation, operator-owned
config file Gosling reads at runtime, not compiled into the binary and not supplied by the untrusted
consumer manifest or provisioning. Registering a new project's adapter is therefore the same kind of
operation as registering a new MCP extension today: a settings-file entry, requiring no Gosling
source edit or rebuild, which is what keeps ADR-0010's copy-free consumer model intact (the first
review round's "registry compiled into `gosling-cli`" fix is superseded by this). `build_shell_runtime`
spawns the registered adapter through the existing `ExtensionConfig::Stdio` child-process path and
fails closed (`ADAPTER_NOT_REGISTERED`) if `domain_id` has no registry entry, rather than falling
back to `None` silently.

`domain.action` for any action whose descriptor marks it `mutate` requires a valid, unexpired,
matching-scope `ActionConfirmationToken`, populated into the **already-existing**
`DomainActionRequest.confirmation_token: Option<String>` field
(`crates/gosling-sdk-types/src/shell.rs:229-230`) — this contract does not add a new wire field, it
specifies how that field gets filled. Consistent with ADR-0012's server-owned-authorization decision,
the token is minted and validated **by the Rust server only**, never by Electron main, when a
`confirm` interaction (§ "Application-runtime operations and events") for that exact
action/session/generation is approved via `confirmation.respond`; the server returns the opaque token
as the result of that approval call. Main relays this single-use, short-lived, scope-bound value from
the approval response directly into the immediately following `perform_domain_action` call — main
holds the value only in transit between two server round trips and cannot mint, forge, extend, or
independently validate it, so this is not a second authority, the same way main already relays the
ACP loopback secret without being able to generate one. The renderer, in turn, only answers a yes/no
confirmation prompt and never sees the token at all, closing the same "opaque action ID, not a raw
credential" pattern already used for permission/elicitation. `read` actions require no token. Replay,
cross-action, cross-session, cross-generation, or expired tokens fail with a typed
`ADAPTER_ACTION_UNAUTHORIZED` error and no mutation occurs.

## Error taxonomy additions

Extends the existing table in `shell-productization-contracts.md` §"Error taxonomy":

| Category | Representative codes | User actions |
| --- | --- | --- |
| consumer manifest | schema/hash/root-containment/capability-declaration mismatch | fix reviewed manifest; no build |
| application interaction | stale/foreign/duplicate action ID, no outstanding attempt, attempt already active | resubmit through current snapshot state |
| adapter negotiation | descriptor/version/capability mismatch, adapter absent when required, `ADAPTER_NOT_REGISTERED` (`domain_id` has no `AdapterRegistration` settings entry) | stop, diagnostics; adapter is a build/consumer/operator-settings defect, not a user error |
| adapter action authority | invalid/expired/replayed confirmation token | retry from a fresh snapshot; never silently retried by the client |

## Negative-space rules (carried into R2–R4 test design)

- A consumer manifest with a `rendererEntry` outside its approved root, or naming a host
  main/preload file, must fail resolution before any build step runs.
- An operation name absent from the frozen table above must be rejected by main even if a future
  ACP method exists with a similar name — declared capability must equal live capability (PG-INV-004).
- A `permission.respond`/`elicitation.respond`/`domain.action` call with a valid-looking but
  unissued `actionId`/token must fail, not fall through to a default-allow branch.
- Adapter absence when the consumer manifest's `declaredCapabilities` requires one must prevent
  `ready`, not silently report an adapter-less "ready" state.
- A `domain_id` present in provisioning/manifest but absent from the Gosling-settings
  `AdapterRegistration` entries must fail closed (`ADAPTER_NOT_REGISTERED`), not silently fall back
  to `None` the way `build_shell_runtime` does today.
- Adding a new project's adapter must never require editing or rebuilding `gosling-cli` source — only
  a settings-file registration, the same operation already required to add an MCP extension.
- A generation change (reconnect/retry) must not silently replay, duplicate, or auto-resume a prior
  generation's prompt attempt or pending interaction; the renderer must observe the new generation,
  call `session.resume` explicitly, and resubmit any prompt it wants continued.
- Electron main must never be observed minting, caching, or independently validating an
  `ActionConfirmationToken` — that authority is Rust-server-only; a main-side token would be a
  regression of ADR-0012's server-owned-authorization decision.

## Status

This document and ADR-0010–0012 collectively answer PG-15 (traceability review) at the schema level.
They do not close R1: R1 exit still requires architecture review acceptance of the topology choices,
and no P0 decision may remain open per `project-shell-readiness-plan.md` §6 exit criteria. Until that
review, no R2–R4 work package may treat these field names as final without re-reading this file for
drift, and no implementation may cite this document as "R1 complete."
