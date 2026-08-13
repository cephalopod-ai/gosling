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
identity: { id, displayName, version, runtimeNamespace }         # newly exposed, already computed during compatibility check
compatibility: { expectedVsActual summary, no raw values }
provisioningIssues: [{ code, path }]                              # codes only, matches existing diagnostic redaction
session: { sessionId, status: none|creating|active|resuming|closing, promptAttempt: { id, phase: idle|streaming|cancelling } | null }
adapter: { descriptorId, version, capabilities: string[], status: absent|negotiating|ready|incompatible|crashed } | null
pendingInteractions: [{ actionId, kind: permission|elicitation|confirm, summary (allowlisted fields only), expiresAtGeneration }]
```

Total bound remains <=64 KiB as today. No ACP endpoint, token, raw config, private path, prompt
history, or raw error ever appears here — this is a hard carry-over from the accepted threat model,
not a new relaxation.

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
| `session.updates` | event | main only | bounded update <=64 KiB, attempt-fenced | streamed model/tool output |
| `permission.respond` | invoke | action ID + allow_once\|deny | status <=1 KiB | rejects replay/expired/foreign action ID |
| `elicitation.respond` | invoke | action ID + submit(bounded fields)\|cancel | status <=8 KiB | rejects replay/expired/foreign action ID |
| `permission.requested` | event | main only | action summary <=8 KiB | opaque action ID, allowlisted fields only |
| `elicitation.requested` | event | main only | action summary <=8 KiB | opaque action ID, allowlisted fields only |
| `domain.snapshot` | invoke | none beyond generation | bounded payload <=64 KiB (matches the safe-snapshot ceiling; the R4 neutral fixture's snapshot must fit this bound, and an adapter that would exceed it fails as overproducing) | only when `adapter.status == ready` |
| `domain.action` | invoke | action name + args, <=16 KiB | bounded payload <=64 KiB, or `CONFIRMATION_REQUIRED` + interaction actionId for an unapproved `mutate` action | `read` actions execute directly; `mutate` actions execute only once their `confirm` interaction (below) is approved |
| `confirmation.requested` | event | main only | interaction summary <=8 KiB | opaque action ID, allowlisted action-name/args summary only |
| `confirmation.respond` | invoke | action ID + approve\|deny | status <=1 KiB | rejects replay/expired/foreign action ID; on approve, main executes the pending action server-side using the token it minted internally |

Explicitly still absent, unchanged from the Gate 2 contract: arbitrary file/settings/clipboard/
notification/updater access, raw ACP URL/token, MCP proxy URL, server secret, and arbitrary IPC
channels. `declaredCapabilities` in the consumer manifest must be a subset of this table's operation
names (plus adapter action names once ADR-0012's fixture exists); an undeclared invoke is rejected
before dispatch (PG-INV-004).

## Interaction records (ADR-0011)

A pending permission/elicitation is represented server-side (main-owned, not renderer-owned) as:

```text
actionId: opaque, unguessable, unique per request
kind: permission | elicitation | confirm
generation, sessionId, promptAttemptId: fencing keys
issuedAt: monotonic counter (not wall-clock, to keep event ordering testable)
expiresWith: promptAttempt end | session end | explicit cancel
status: pending | resolved | expired | superseded
```

A `confirm` interaction is how a `domain.action` mutation is authorized (§ below): it is not tied to
a `promptAttemptId` the way permission/elicitation interactions are, since a mutation can be
requested outside an active prompt attempt; it fences on `generation`/`sessionId` only.

A `respond` call is accepted exactly once per `actionId`; every subsequent call for the same ID
returns a stale-action error rather than re-executing the response.

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
AdapterProcessDescriptor       # source-controlled, not renderer-suppliable
  executable: repository-relative or approved-root path, no shell interpretation
  protocolVersion: exact string
  startupTimeoutMs, actionTimeoutMs: bounded positive integers
  maxMessageBytes: bounded positive integer

ActionConfirmationToken        # internal to main/server; never sent over IPC or to the adapter process
  actionName, adapterVersion, sessionId, generation: binds the token to exactly one action/session/generation
  nonce: single-use, server-minted
  expiresAtGeneration: fails closed on generation rollover
```

`domain.action` for any action whose descriptor marks it `mutate` requires a valid, unexpired,
matching-scope `ActionConfirmationToken`. The token is minted by main only when a `confirm`
interaction (§ "Application-runtime operations and events") for that exact action/session/generation
is approved via `confirmation.respond`, and is consumed server-side in the same step that dispatches
the action to the adapter — the renderer requests a mutation and answers a yes/no confirmation
prompt, but never receives, stores, or resubmits the token itself, closing the same "opaque action
ID, not a raw credential" pattern already used for permission/elicitation. `read` actions require no
token. Replay, cross-action, cross-session, cross-generation, or expired tokens fail with a typed
`ADAPTER_ACTION_UNAUTHORIZED` error and no mutation occurs.

## Error taxonomy additions

Extends the existing table in `shell-productization-contracts.md` §"Error taxonomy":

| Category | Representative codes | User actions |
| --- | --- | --- |
| consumer manifest | schema/hash/root-containment/capability-declaration mismatch | fix reviewed manifest; no build |
| application interaction | stale/foreign/duplicate action ID, no outstanding attempt, attempt already active | resubmit through current snapshot state |
| adapter negotiation | descriptor/version/capability mismatch, adapter absent when required | stop, diagnostics; adapter is a build/consumer defect, not a user error |
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

## Status

This document and ADR-0010–0012 collectively answer PG-15 (traceability review) at the schema level.
They do not close R1: R1 exit still requires architecture review acceptance of the topology choices,
and no P0 decision may remain open per `project-shell-readiness-plan.md` §6 exit criteria. Until that
review, no R2–R4 work package may treat these field names as final without re-reading this file for
drift, and no implementation may cite this document as "R1 complete."
