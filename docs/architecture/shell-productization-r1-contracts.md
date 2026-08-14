# Project-shell R1 contracts (consumer, application-runtime, domain-adapter)

Status: accepted — companion schema freeze for ADR-0010, ADR-0011, ADR-0012; R1 operator
authorization recorded 2026-08-13 per `pre-gui-backend-implementation-plan.md` PG-14/PG-15
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
  protocolVersion: exact version string identifying the domain adapter *contract* version
                    (descriptor/snapshot/action/confirm shapes) this consumer targets — distinct
                    from MCP's own transport protocol version, which MCP's initialize handshake
                    already negotiates and this field does not duplicate
  actions: non-empty sorted set of domain action names the renderer will invoke — validated by
           live comparison against the adapter's descriptor() result (ADR-0012), never against the
           frozen application-runtime operation table below, since these names are consumer/adapter
           -specific and cannot be known when this document is frozen
declaredCapabilities: exact sorted set of application-runtime operation names from the frozen table
                       below ONLY — e.g. `domain.action` (declaring "this consumer uses domain
                       actions" generically) belongs here; specific action names belong in
                       domainAdapter.actions above, never mixed into this list
assetsRoot (optional): repository-relative or consumer-relative non-symlink directory
testFixturesRoot (optional): same containment rule as assetsRoot
```

Rejected content, in addition to everything the product profile already rejects: secrets, shell
commands, arbitrary URLs, native-library paths, main/preload replacement paths, release/signing
credentials, any `approvedRoot`-shaped field (the approved root is trusted resolver configuration,
never manifest content — see ADR-0010), `domainAdapter.actions` present without `domainAdapter`
itself, and any `declaredCapabilities` entry that does not name a frozen operation from this
document — domain action names are validated separately, by the rule above, precisely so this
rejection rule does not have to (and must not) reject consumer-specific action names it cannot know
in advance. The manifest is resolved and canonically hashed the same way as the product profile
(`shell-profile.js` pattern); the resolved hash is embedded in `ShellBuildManifest` alongside
`profileHash`. Unknown fields and unknown `schemaVersion` fail closed
(`CONSUMER_SCHEMA_UNSUPPORTED`).

## Safe runtime snapshot v2 (ADR-0011)

Extends (does not replace) the existing `runtime.read`/`runtime.changed` payload:

```text
generation, lifecycleState, reasonCode, allowedActions          # unchanged from Gate 2 contract
identity: { id, displayName, version }                            # newly exposed, already computed during compatibility check
directory: { state: unselected|selected|missing|invalid, path: string|null, label: string|null, reasonCode: string|null, remembered: bool }
settingsRecovery: { status: loaded|absent|unsupported_schema|malformed|unreadable, schemaVersion: number|null }
credentials: { catalogStatus: available|denied|unavailable, profiles: [{ id, name, providerOrServiceId, status: configured|relink_required }], selectedProfileId: string|null, selectionStatus: none|configured|relink_required|missing }
modules: [{ id: "core:<name>"|"extension:<name>"|"skill:<id>"|"adapter:<domainId>", kind, status: ready|unavailable|incompatible, version?, capabilities: string[] }]   # resolved against the selected directory, re-read whenever it changes
runtimeNamespace: string | null                                   # null until R3 adds it to ShellIdentity/checkShellCompatibility; only ever non-null once genuinely verified, never trusted-but-unlabeled
compatibility: { expectedVsActual summary, no raw values }
provisioningIssues: [{ code, path }]                              # codes only, matches existing diagnostic redaction
session: { sessionId, status: none|creating|active|resuming|closing, resumeKind: fresh|resumed, resumeIntegrity: clean|uncertain|not_applicable, promptAttempt: { id, phase: idle|streaming|cancelling } | null }
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
  `session.create`), and `promptAttempt` is always `null` immediately after resume.
- Blind resubmission is unsound: if the prior attempt's tool call or mutation committed server-side
  before the transport dropped, only the response may be missing and repeating the prompt can repeat
  that side effect. R3 therefore persists a bounded state for the latest ACP prompt before agent work
  begins and replaces it only after a terminal completion, cancellation, or failure. `session.info`
  projects `resumeIntegrity: clean | uncertain` from that durable state before compacted load:
  `clean` means the prior prompt reached a known terminal outcome, while `uncertain` means a prompt
  was in progress (or predates this record) when the connection was lost. The renderer receives that
  fact through the safe snapshot and must not treat `uncertain` as permission to silently resubmit.
  This is an outcome fence, not an automatic replay or an idempotency guarantee for an operator who
  chooses to repeat a completed prompt.
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
| `permission.respond` | invoke | generation + session ID + action ID + allow_once\|deny | status <=1 KiB | rejects replay/expired/foreign action ID |
| `elicitation.respond` | invoke | generation + session ID + action ID + submit(bounded fields)\|cancel | status <=8 KiB | rejects replay/expired/foreign action ID |
| `permission.requested` | event | main only | action summary <=8 KiB | opaque action ID, allowlisted fields only |
| `elicitation.requested` | event | main only | action summary <=8 KiB | opaque action ID, allowlisted fields only |
| `domain.snapshot` | invoke | none beyond generation | bounded payload <=64 KiB (matches the safe-snapshot ceiling; the R4 neutral fixture's snapshot must fit this bound, and an adapter that would exceed it fails as overproducing) | only when `adapter.status == ready` |
| `domain.action` | invoke | session ID + generation + action name + args, <=16 KiB | bounded payload <=64 KiB, or bounded `confirmationActionId` for an unapproved `mutate` action | `read` actions execute directly; `mutate` actions execute only once their `confirm` interaction (below) is approved |
| `confirmation.requested` | event | main only | interaction summary <=8 KiB | opaque action ID, allowlisted action-name/args summary only |
| `directory.select` | invoke | generation + `userGesture: true`; **never a path** | typed result <=8 KiB | main opens Electron's native directory chooser, sends the operator-confirmed path to the authenticated loopback backend for canonicalization, and keeps only an accepted canonical path; cancel is `{status: "cancelled"}`, not an error; rejected paths return a stable reason code and no path; a settings document the store refuses to overwrite yields `remembered: false` rather than blocking the selection |
| `credential.select` | invoke | generation + opaque profile ID from the current safe catalog, or `null` | safe catalog snapshot <=64 KiB | rejects unknown/stale/policy-disallowed IDs; persists only the opaque ID; overlapping selections are sequence-fenced so a slower earlier one never overwrites a later committed one; never returns a secret, auth kind, source, secret-field name, provider parameter, or timestamp |
| `session.detach` | invoke | none beyond generation | typed result <=8 KiB | releases the local one-session slot so a different directory can be chosen; permitted only from `active` — a create or resume still awaiting the backend would otherwise finish afterwards and reinstate a session nobody holds — and refuses while a prompt attempt streams or an interaction is pending; never deletes or mutates the server session, which stays resumable by ID |
| `confirmation.respond` | invoke | action ID + approve\|deny | on approve: bounded `DomainActionResponse` payload <=64 KiB; on deny/reject: status <=1 KiB | main relays to the Rust server, which rejects replay/expired/foreign action ID and, on approve, executes the already-pending action immediately (it retained the action/input from the original `domain.action` call) and returns the result inline — there is no second `perform_domain_action` round trip and no token ever crosses the main/server boundary |

Provisioning read/validate accept an optional `workingDir`; main restores the remembered directory
before the compatibility gate so a product with project-local extensions or skills is judged against
the directory its sessions will run in rather than the backend's startup directory.

`session.create` requires `directory.select` and `session.detach` requires `session.create`: a
consumer that can open a session must be able to choose the directory that session runs in, and a
consumer that can release a session must be able to open one. The resolver rejects a declaration
that omits a prerequisite rather than silently widening it.

Explicitly still absent, unchanged from the Gate 2 contract: arbitrary file/settings/clipboard/
notification/updater access, raw ACP URL/token, MCP proxy URL, server secret, and arbitrary IPC
channels. `declaredCapabilities` in the consumer manifest must be a subset of this table's operation
names (plus adapter action names once ADR-0012's fixture exists); an undeclared invoke is rejected
before dispatch (PG-INV-004).

### Capability negotiation mapping (one canonical source, not two)

The Gate 2 contract already has a hand-authored `compatibility.requiredMethods` field on the product
profile (`shell-productization-contracts.md` §"Product-profile schema v1"), and
`checkShellMethods`/`availableMethods` perform **exact string membership against
`custom_method_names()`** (`crates/gosling/src/acp/server.rs:2506-2530`) — this checks only
`_gosling/unstable/...` custom methods, never standard ACP capabilities. Standard ACP support is a
*different* mechanism: boolean/structured predicates on `AgentCapabilities`
(`load_session`, `session_capabilities`, `prompt_capabilities`, `mcp_capabilities` —
`crates/gosling/src/acp/server.rs:2563-2577`), returned unconditionally on every `initialize` call,
not looked up by method name. Conflating the two (as an earlier draft of this table did) would either
never match (`requiredMethods` cannot contain a standard capability name `custom_method_names()` never
lists) or force inventing fictitious method names. This freezes two separate, correctly-typed
derivations instead of one mixed list:

```text
Custom-method operations -> derive into requiredMethods (checked by exact string membership):
  directory.select                               -> _gosling/unstable/shell/directory/validate
  credential.select                              -> _gosling/unstable/shell/credentials/list
  domain.snapshot                                -> _gosling/unstable/shell/domain/snapshot
  domain.action                                  -> _gosling/unstable/shell/domain/action
  confirmation.respond                           -> _gosling/unstable/shell/domain/action/confirm

Standard-ACP-capability operations -> derive into a capability-predicate check (NOT requiredMethods):
  session.create, session.resume                 -> agentCapabilities.load_session == true
  prompt.submit, prompt.cancel                    -> unconditional core ACP prompt/cancel; no predicate needed
  permission.respond                              -> unconditional core ACP requestPermission path; no predicate needed
  elicitation.respond                             -> unconditional core ACP unstable_createElicitation path; no predicate needed
```

R2's manifest resolver computes two things from `declaredCapabilities`, never a single hand-typed
list: `requiredMethods` as the union of the custom-method entries above (plus the always-required
baseline — `_gosling/unstable/session/info` and shell provisioning read/validate, per the existing
Gate 2 compatibility contract), and a `requiredAgentCapabilities` predicate set checked against the
live `InitializeResponse.agentCapabilities` rather than any method-name list. A profile used without a
consumer manifest (the current fixed neutral renderer, and any product that predates ADR-0010) keeps
hand-authored `requiredMethods` unchanged, since it declares no `declaredCapabilities` to derive from.
`ready` requires the derived method set, the derived capability predicates, and the live adapter's
declared actions (ADR-0012) to all match what was actually negotiated — a manifest cannot declare a
capability this table does not know how to translate into either a method check or a capability
predicate check.

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
  custom method, `_gosling/unstable/shell/domain/action/confirm`, alongside `domain_snapshot`/
  `perform_domain_action`); main holds no pending-record state of its own for this kind and cannot
  resolve it locally. The server-side record additionally retains the pending action's `action`/
  `input` alongside the fencing keys (not just an opaque ID) — this is what lets approval execute the
  action immediately instead of requiring the renderer or main to resupply it. It is not tied to a
  `promptAttemptId` the way permission/elicitation interactions are, since a mutation can be
  requested outside an active prompt attempt; it fences on `generation`/`sessionId` only.

A `respond` call is accepted exactly once per `actionId`; every subsequent call for the same ID
returns a stale-action error rather than re-executing the response. For `confirm`, that exactly-once
check is enforced by the Rust server, not by main.

## Domain adapter descriptor and confirmation token (ADR-0012)

```text
DomainAdapterDescriptor (legacy v1)
  domain_id, display_name, version, actions: Vec<String>   # flat action names only; no protocolVersion field

DomainAdapterDescriptor (v2, implemented as the R4 schema slice)
  domain_id, display_name, version, protocolVersion, actions: [{ name, kind: read|mutate, schemaRef }]
```

The legacy v1 descriptor's `actions` field was a flat `Vec<String>` and carried no `protocolVersion`,
so it could not identify mutations or detect a domain-adapter-contract-version mismatch. The R4 DTO
migration now provides `protocolVersion` and typed action kinds in
`crates/gosling-sdk-types/src/shell.rs`. A first runtime slice also validates an operator-owned
`domain_adapters` registration, starts the process through the existing hardened stdio-MCP path, and
rejects server startup unless its live `descriptor` tool exactly equals provisioning. The configured
size cap is enforced on newline-delimited MCP frames before JSON decoding as well as on tool values.
Electron now compares the live descriptor with the consumer declaration and relays typed,
capability-gated snapshot/action/confirmation calls through Electron main. R4 adds post-ready
supervision/status notification and local neutral-process failure conformance; packaged and
cross-platform reproduction remains R6/R8.

New, R4-scoped:

```text
AdapterRegistration             # lives in user/operator-owned Gosling settings, NOT gosling-cli source, NOT the consumer manifest
  domainId: matches DomainAdapterDescriptor.domain_id exactly — the join key ShellProvisioning.domain_adapter references
  cmd, args, envs, envKeys, timeout, cwd: identical shape to the existing ExtensionConfig::Stdio fields
                                          (crates/gosling/src/agents/extension.rs:178-200) — no new field vocabulary
  maxMessageBytes: positive integer, schema maximum 4 MiB, default 1 MiB if unset (new; MCP itself
                    has no message-size ceiling, so this document sets a finite hard cap rather than
                    leaving registrations free to request effectively unbounded framing/parse
                    allocation — the transport must frame and parse a message before any later 64 KiB
                    response-body check could reject it, so the cap belongs at the registration/schema
                    level, not only at the response-bound level)

# The three MCP tools the adapter process exposes (ADR-0012). "descriptor" is the live negotiation
# surface MCP's own initialize handshake does not provide; "action" deliberately excludes the
# confirmation token, which authorizes the server's dispatch decision, not the adapter call itself.
AdapterTool: descriptor()                      -> DomainAdapterDescriptor { domain_id, display_name, version, actions,
                                                     protocolVersion }  # NEW field: the domain-adapter CONTRACT version
                                                                        # (descriptor/snapshot/action/confirm shapes),
                                                                        # frozen at "1.0.0" by this document — distinct
                                                                        # from `version` (the adapter's own semantic
                                                                        # version) and from MCP's transport protocol
                                                                        # version, which MCP's own initialize handshake
                                                                        # already negotiates independently
AdapterTool: snapshot(input)                   -> matches DomainSnapshotRequest/Response exactly
AdapterTool: action(action, input)             -> AdapterActionRequest { action, input }  # NOT DomainActionRequest — no confirmation field
                                                -> matches DomainActionResponse

# New custom ACP method (main <-> Rust server), distinct from perform_domain_action:
DomainActionConfirmRequest      # method: _gosling/unstable/shell/domain/action/confirm
  session_id, generation: must exactly match the server-retained action
  action_id: the confirm interaction's opaque actionId
  approve: boolean
DomainActionConfirmResponse
  status: approved | denied
  result: Option<DomainActionResponse>   # present only when status == approved; the server executes
                                          # the retained pending action immediately and returns the
                                          # result inline — there is no second perform_domain_action
                                          # call and no token is ever serialized to main or the renderer

PendingDomainAction            # purely internal Rust-server bookkeeping, keyed by the opaque actionId
                                # and atomically consumed by the matching confirm request; no confirmation
                                # token reaches Electron main, the renderer, or the adapter process
  actionName, input, sessionId, generation: binds exactly one retained action/session/generation
  actionId: single-use server-minted nonce; removed on approval or denial
```

Before reporting `adapter.status == ready`, the Rust server calls the adapter's `descriptor()` tool
once and compares the live `{ domain_id, version, actions, protocolVersion }` against
`ShellProvisioning.domain_adapter` — both are already available to Rust today, and the v2 DTO
migration (§ above) adds `protocolVersion` alongside the structured `actions` shape in the same
schema bump. An exact `protocolVersion` mismatch is `incompatible` even if `domain_id`/`actions`
otherwise match, since it means the two sides implement different descriptor/snapshot/action/confirm
wire shapes. The consumer manifest's `domainAdapter` declaration is a *separate*, Electron-side check:
the server's authenticated `initialize` response already carries the live descriptor back to main
today via `_meta.goslingShell.domainAdapter` (`shell_capabilities_meta()`,
`crates/gosling/src/acp/server.rs:2506-2530` — this metadata exists now, this contract adds no new
field to *that* method, only to the `DomainAdapterDescriptor` payload it already carries), so
`checkShellCompatibility` in Electron main compares *that* live descriptor's `protocolVersion` against
the manifest's `domainAdapter.protocolVersion`, and its `actions` against `domainAdapter.actions` — no
new trusted channel is needed to move manifest data into Rust, because Rust never needs to see the
manifest at all; it only needs to compare against provisioning, which it already has. `ready` requires
both
independent comparisons (Rust vs. provisioning, main vs. manifest) to pass; either failing is
`incompatible`, never a silently accepted `ready` (PG-INV-004/PG-INV-009).

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

`domain.action` for any action whose descriptor marks it `mutate` first returns a bounded
`DomainActionResponse.confirmationActionId` rather than executing (the server retains the pending
`action`/`input` alongside that interaction record). Consistent with ADR-0012's
server-owned-authorization decision, approval is handled entirely **inside the Rust server** through
the dedicated `_gosling/unstable/shell/domain/action/confirm` method (new;
`DomainActionConfirmRequest`/`DomainActionConfirmResponse`, defined above) — `confirmation.respond`
maps to *this* method, not to `perform_domain_action`, since `DomainActionRequest` has no field for an
interaction's `action_id` or an approve/deny decision. On approval the server atomically consumes the
matching retained pending action, executes it, and returns
the resulting `DomainActionResponse` shape as `DomainActionConfirmResponse.result` in the *same*
response — there is no second `perform_domain_action` call, no token relay through Electron main, and
no wire field anywhere carries the token value; main's role is strictly to relay the
`confirmation.respond` invoke to the server and relay `DomainActionConfirmResponse` back to the
renderer unmodified. The canonical `DomainActionRequest` contains its session and generation fences
but no confirmation token or approval field. The renderer, in turn, only answers a yes/no confirmation
prompt and never sees a token at all, closing the same
"opaque action ID, not a raw credential" pattern already used for permission/elicitation. `read`
actions require no confirmation step. Replay, cross-action, cross-session, cross-generation, or
expired confirm interactions fail with a typed `ADAPTER_ACTION_UNAUTHORIZED` error and no mutation
occurs.

**Post-ready health.** Between requests, an idle adapter crash or hang is detected by the Rust
server's own process supervision, not by the next renderer-initiated call — pushed to Electron main as
`_gosling/unstable/shell/domain/status` (`DomainStatusNotification`, ADR-0012 "Post-ready health"),
which main folds into `adapter.status` on the existing safe runtime snapshot and delivers through the
existing `runtime.changed` event. No new renderer-facing IPC operation exists for this; it is new input
to the same push path `runtime.changed` already uses for every other lifecycle change.

## Error taxonomy additions

Extends the existing table in `shell-productization-contracts.md` §"Error taxonomy":

| Category | Representative codes | User actions |
| --- | --- | --- |
| consumer manifest | schema/hash/root-containment/capability-declaration mismatch | fix reviewed manifest; no build |
| application interaction | stale/foreign/duplicate action ID, no outstanding attempt, attempt already active | resubmit through current snapshot state |
| adapter negotiation | descriptor/version/`protocolVersion`/capability mismatch, adapter absent when required, `ADAPTER_NOT_REGISTERED` (`domain_id` has no `AdapterRegistration` settings entry), post-ready `crashed`/`hung` pushed via `DomainStatusNotification` | stop, diagnostics; adapter is a build/consumer/operator-settings defect, not a user error |
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
- A confirmation token value must never be observed on any wire DTO, IPC payload, or log —
  including in transit through Electron main. It is minted, consumed, and discarded entirely inside
  the Rust server's handling of one `_gosling/unstable/shell/domain/action/confirm` call; a token
  reaching main, the renderer, or the adapter process at all would be a regression of ADR-0012's
  server-owned-authorization decision.
- `prompt.submit` after `session.resume` with `resumeIntegrity: uncertain` must surface that
  uncertainty to the renderer via the snapshot before submission, not silently behave as if resuming
  were always safe.
- A `declaredCapabilities` entry that names a domain action rather than a frozen operation must be
  rejected at manifest resolution; conversely a `domainAdapter.actions` entry must never be validated
  against the frozen operation table — the two lists have different, non-overlapping validation rules
  and mixing them is a manifest-resolver defect.
- An `AdapterRegistration.maxMessageBytes` above the 4 MiB schema maximum must fail registration
  before the adapter is ever spawned, not be silently clamped or accepted and enforced only later.
- A `protocolVersion` mismatch between the live adapter descriptor and `ShellProvisioning.domain_adapter`
  (Rust-side) or the consumer manifest's `domainAdapter.protocolVersion` (main-side) must produce
  `incompatible`, even when `domain_id`, `version`, and `actions` all otherwise match.
- An adapter that exits or stops responding after `ready` with no domain request in flight must not
  leave `adapter.status`/`runtime.read` reporting stale `ready` indefinitely; R4 must wire adapter
  health into the existing `runtime.changed` push path (§ ADR-0012 "Post-ready health") rather than
  leaving post-ready liveness undetectable between requests.

## Status

This document and ADR-0010–0012 collectively close R1's schema-level architecture review. The
operator accepted the topology choices on 2026-08-13 and no P0 ownership or authority decision remains
open for R2–R4. R2–R4 implementation must still re-read this document for drift and cannot claim its
own gate is complete without its specified live conformance evidence.
