# ADR-0020: Host-enforced plan lifecycle

Date: 2026-09-13
Status: Accepted/staged; implementation and general availability remain gated
Related: ADR-0011, ADR-0017, ADR-0019

## Context

gosling currently has two planning-adjacent behaviors that are not a durable
planning authority boundary. `GoslingMode::Chat` suppresses gosling-hosted tools
but some providers implement their own tools outside gosling. The CLI also owns
an in-memory plan mode that calls a separate model without tools, classifies its
prose with another model, clears conversation history before acting, and
temporarily changes global authorization to `Auto`.

Those behaviors cannot support restart-safe review, exact revision approval,
multi-client conflict detection, or a host-enforced read-only workspace
capability. Prompt instructions, tool names, annotations, provider claims, and
renderer state are not authorization sources.

## Decision

gosling core owns a session-scoped plan lifecycle separate from `GoslingMode`.
Stable states are `drafting`, `awaiting_review`, `approved`, `abandoned`, and
`stale`. Only one generation may be open per session. Plan revisions are
immutable, ordered, content-hashed, and bound to deterministic transcript and
workspace-scope hashes. Feedback and decisions use compare-and-swap expectations
for the exact generation and revision.

Hash contracts are versioned and domain-separated. Plan content is SHA-256 over
the newline-normalized UTF-8 bytes stored for the immutable revision. Transcript
source and workspace scope hashes use SHA-256 over a version tag followed by a
length-prefixed canonical encoding of every field; concatenation without lengths
or canonical field order is forbidden. For a message row containing both plan
lifecycle blocks and other planner-visible content, source hashing filters at the
content-block level: it removes only `plan_update`/`plan_request_review` request
and result bookkeeping, retains prose and other permitted evidence from that
same row, and hashes the retained canonical blocks. The implementation must ship
byte-exact fixtures for mixed rows, empty filtered rows, field ordering, Unicode,
and hash-version changes before approval is exposed.

Every active, parent, feedback, consumed-by, and event revision reference must
resolve within the same plan. Enforce that relationship with SQLite constraints
where practical and with `PlanService` validation in the same transaction in all
cases. Unknown, cross-plan, or malformed references are recoverable errors and
cannot be interpreted as normal or approvable state.

Plan readiness is a structured transition. A model must successfully call a
host-owned revision update and then request review for that exact revision and
hash. A committed review request ends the planner turn. A turn that starts under
`Planning(plan_id, generation)` remains governed by that immutable policy until
the turn is terminal; approval, abandonment, staling, or any other concurrent
transition can only narrow it and can never make the in-flight turn fall back to
`Normal`. A late request from that turn is denied even if the durable plan has
become terminal. Normal permission behavior resumes only for a separately
started ordinary turn.

Approval is a direct host action from an admitted ACP client rather than a model
tool. An authenticated HTTP/WebSocket client presents the existing transport
credential, while a local stdio client relies on its process boundary. If the
operator explicitly starts `gosling serve --dangerously-unauthenticated`, every
client admitted by that transport is treated as user authority for plan decisions,
just as it is for the server's other agent capabilities. That mode remains
explicitly dangerous and does not create an authenticated personal identity.
Approval records a decision; it neither starts a model turn, clears history, nor
changes global or session authorization. An explicit **Approve and implement**
client action first commits approval and then submits a separate ordinary user
turn. If submission fails, approval remains committed and the client reports
partial success.

Open planning uses a typed, server-authored capability registry as a hard upper
bound. The initial capabilities are bounded workspace tree/read/search,
redacted session-history search/read, plan revision update, and review request.
Only in-process gosling implementations can hold those identities. MCP,
frontend, direct-app, code-mode, provider-owned, extension-management,
computer-control, network, shell, mutation, and delegation paths cannot acquire
planning capability through names or annotations. The planning extension is a
non-configurable internal capability: it is injected only for an eligible
planning turn, is not stored in the session's enabled-extension list, cannot be
disabled or enabled by a client, and is absent from ordinary turns.

Enforcement has two layers. Reply preparation publishes only the planning
catalog and instructions. Every execution path then revalidates durable plan
state and typed identity before argument tracing, hooks, approval routing,
frontend emission, or execution. Revalidation and insertion/replay of the
durable tool-operation ledger row are one atomic fenced action; plan state cannot
change between authorization and ledger begin. Backend, frontend, direct-app,
and code-mode outer dispatch use the same operation. A disallowed planning
request is denied without an approval prompt, ledger row, raw-argument log, or
side effect; ordinary authorization modes and saved grants cannot widen the
boundary.

Providers that execute tools outside gosling are incompatible with
host-enforced planning in this first version. Plan entry or a planner prompt
fails before the initiating user message is persisted. Provider-native plan or
read-only modes remain separate compatibility features and are not represented
as host-enforced planning. Provider or model transition is blocked while any
plan generation is open (`drafting` or `awaiting_review`); the user must first
approve, abandon, or otherwise close the generation. This removes a second
planner identity from the source/scope and operation-fencing contract.

The lifecycle is exposed through typed ACP methods and compact invalidation
updates. CLI and first-party Desktop are adapters over the core service; they do
not duplicate transitions. Imported, copied, forked, shared, or handed-off plan
records are historical context only and become stale. Archive preserves plans;
session deletion removes them atomically.

Native session export retains the current flat session object and adds one
exact optional top-level section named `plan_history_v1`. Its value contains
`schema_version: 1` plus bounded `plans`, `revisions`, `feedback`, and `events`
collections. Absence preserves current-file compatibility. Import allocates new
local plan, revision, feedback, and event identities; remaps every internal
reference; validates that all references stay within the imported plan; and
commits the session, transcript, and plan history in one transaction. Every
imported generation is stored as `stale`, with an import provenance reason, and
approval events remain non-authoritative history. Invalid or over-limit plan
sections fail the import rather than being silently dropped. Copy, fork, share,
and new-session handoff apply the same no-authority-transfer rule.

## Safety boundary

The session database is the only authorization source for plan state. Rendered
snapshots, notifications, exported Markdown, chat prose, and provider output are
caches or evidence and cannot authorize execution. Unknown or malformed open
plan state fails closed. A client-supplied tool name, annotation, provenance
field, actor label, or cached plan snapshot cannot establish a planning
capability or decision identity.

Workspace inspection is confined to canonical session roots, does not launch a
subprocess or use the network, rejects binary, oversized, and credential-like
files, and returns bounded redacted output. Project and prior-session content is
untrusted evidence. Users are told that inspected text is sent to the selected
planner provider.

No Grok provider, authentication, runtime dependency, TUI, workflow DSL, or
generic orchestration system is introduced. The existing permission engine,
context manager, durable summaries, handoff service, session lease, and durable
tool-operation ledger remain in place.

## Rollout

Implementation follows the dependency and evidence gates in
`docs/build/host-enforced-planning/README.md`. General availability remains
gated on hostile conformance evidence for both catalog minimization and
execution-time enforcement. Architecture registration was initially deferred because its
components are a current-state map and its active invariants claim implemented
enforcement. The registry now records the real core, security-boundary, and
Desktop plan-review paths under ARC-011/ARC-012 without claiming candidate-wide
conformance or a published release.

The baseline permission/harness fixture skeleton lands before the execution
gate; plan-policy cases co-land with that gate; final cross-layer corpus coverage
follows client and continuity work. Until continuity semantics are complete,
every copy, fork, import/export, share, handoff, provider transition, and scope or
history mutation that cannot preserve the plan contract must fail closed while a
plan is open. General availability additionally requires CLI and Desktop parity,
continuity semantics, legacy CLI removal, documentation, and broad validation.

Compaction provenance is a separate value-gated follow-up. Independent goal
verification is out of scope and requires its own future ADR.

## Consequences

Planning becomes restart-safe, reviewable, and consistent across clients, but
the release adds schema, transport, UI, and compatibility surface. The first
version deliberately provides fewer tools than ordinary execution and rejects
providers whose tool runtime gosling cannot constrain. Separate planner-provider
selection may remain unavailable until it can use the normal provider-transition
and session-operation guarantees.
