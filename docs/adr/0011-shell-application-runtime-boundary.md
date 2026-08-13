# ADR-0011: Main-owned application runtime and renderer capability boundary

Date: 2026-08-13
Status: proposed — pending operator acceptance (R1 architecture review; see
`pre-gui-backend-implementation-plan.md` PG-12 and `project-shell-readiness-plan.md` §4.2)
Requirements affected: SHP-REQ-004, SHP-REQ-006, SHP-REQ-018, SHP-REQ-034, SHP-REQ-035,
SHP-REQ-037, SHP-REQ-038, SHP-REQ-042

## Context

`GoslingShellAPI` (`ui/desktop/src/shell/preloadApi.ts:13-30`, wired in `preload.ts:23-52`, channels
in `ipc.ts:7-16`) exposes exactly eight operations: `runtime.read`, `runtime.retry`, `runtime.stop`,
`diagnostics.save`, `handoff.prepare`, `handoff.confirm`, `external.open`, and the `runtime.changed`
event. None of them create/resume a session, submit or cancel a prompt, or answer a permission or
elicitation request. `ShellAcpConnection` (`ui/desktop/src/shell/acpRuntime.ts:53`) exposes
`createSession`, `resumeSession`, `prepareHandoff`, and `close`, and `connectShellAcp()` (line 214)
builds it after authenticated `initialize` and compatibility checks — but the callbacks it installs
(`clientCallbacks()`, lines 114-120) unconditionally cancel every permission request, decline every
elicitation, and discard every session update. `runtime.read` returns only `ShellLifecycleState` (no
identity, session, or adapter facts). These are readiness findings PSR-002/SHP-DEF-022,
PSR-003/SHP-DEF-023, and PSR-006/SHP-DEF-026. `shell-productization-contracts.md` already froze the
current eight-channel allowlist as a Gate 2 contract and states "later additions require ADR/change-
control review and a typed negative-space test" — this ADR is that review for the operations needed
to make the shell a usable agent client.

## Decision

Main (`ui/desktop/src/shell/`) remains the sole ACP owner. The renderer never receives the ACP URL,
loopback token, child process handle, raw provisioning document, or unredacted errors — this is
unchanged from ADR-0008 and is reaffirmed, not loosened, by this ADR. Three things are added inside
that boundary, specified in full in the companion schema addendum
(`docs/architecture/shell-productization-r1-contracts.md`):

### 1. Bounded session/prompt operations

Extend `ShellAcpConnection` and the IPC allowlist with:

- `session.create` / `session.resume` — thin wrappers around the existing `createSession`/
  `resumeSession` methods, now reachable from the renderer under main/server-owned working-directory
  policy (no renderer-supplied path).
- `prompt.submit` — bounded text prompt, tagged with a main-generated **prompt-attempt ID**; only
  one attempt may be outstanding per session (per `SHP-ASM-032`, one active session initially).
- `prompt.cancel` — cancels the current attempt by ID; stale/duplicate attempt IDs are rejected, not
  silently accepted.
- `session.updates` (event) — replaces the discarded `sessionUpdate` callback with a bounded,
  schema-validated, attempt-fenced stream to the renderer.

### 2. Explicit permission/elicitation mediation

Replace the `requestPermission`/`unstable_createElicitation` callbacks that always answer
`cancelled`/`decline` with focused controllers that:

- surface each pending request to the renderer as an **opaque action ID** plus the allowlisted,
  non-secret fields needed to render it (tool name, elicitation prompt/schema — never raw arguments
  containing paths/secrets beyond what the server already redacts);
- accept exactly one renderer response per action ID (`permission.respond` /
  `elicitation.respond`), reject replays and mismatched generation/session/action IDs, and expire
  a pending action if its owning prompt attempt or session ends;
- never auto-approve. If the renderer is not yet capable of answering a request class (e.g. a
  consumer that declared no elicitation capability), the request is declined visibly and recorded in
  diagnostics — it is not silently discarded the way it is today.

This directly closes SHP-RSK-032 and SHP-RSK-034 ("focused shell silently cancels permissions...
making agent workflows fake or unusable").

### 3. Safe runtime snapshot v2

Extend `runtime.read`/`runtime.changed` beyond `ShellLifecycleState` to include: verified product
identity and runtime namespace (already computed during `connectShellAcp`'s compatibility check,
`compatibility.ts:75-150`, but currently not returned to the renderer), safe compatibility result,
provisioning issue codes/paths (not values — reusing the existing diagnostic redaction contract),
session status/ID and prompt-attempt phase, negotiated domain-adapter descriptor and capability
availability (ADR-0012), and pending permission/elicitation summaries by opaque action ID. The
snapshot excludes everything `shell-productization-contracts.md`'s existing error-taxonomy/threat-
model tables already forbid (ACP endpoint, full config, private paths, raw errors, secrets).

Every new operation carries the existing generation-fencing convention (`ShellGenerationRequest`)
plus, where applicable, session ID and a prompt-attempt/action nonce. Main validates sender, exact
field shapes, byte/count bounds, current lifecycle state, declared consumer capability (ADR-0010),
and staleness before acting — the same validation posture `runtime.retry`/`runtime.stop` already use.

Lifecycle states gain real producers per PSR-005/SHP-DEF-025: `busy` is entered while a prompt
attempt is outstanding; `relink_required` is entered when a compatibility/provisioning check yields
a credential-relink issue code rather than folding into generic `degraded`; illegal transitions are
recorded as the internal-bug diagnostic the reducer already promises but the controller does not yet
emit. A state with no event producer after this mapping is removed rather than left as dead UI
vocabulary (SHP-REQ-037).

## Alternatives considered

| Alternative | Why rejected |
| --- | --- |
| Expose ACP directly to the renderer (drop main-owned proxying) | Reverses ADR-0008's trust boundary and SHP-ASM-030; renderer code is the least-trusted consumer-supplied surface (ADR-0010) and must not hold the loopback token. |
| Generic RPC/passthrough channel forwarding arbitrary ACP method names | PG-INV-001/PG-INV-004 forbid undeclared method invocation and require capability equality; a passthrough channel cannot be sender/field/state validated per-operation. |
| Auto-approve permissions/elicitations for "trusted" first-party consumers to unblock UX | SHP-RSK-034 and this repo's no-fake-success rule forbid silent approval; there is no mechanism to distinguish a "trusted" consumer at the IPC layer once ADR-0010's external consumers exist. |
| Support N concurrent sessions in the first contract | `SHP-ASM-032` records one session as the conservative initial default; concurrency multiplies the cancel/reconnect/permission state space this ADR must prove before any UI exists. Deferred pending two-consumer evidence. |
| Reuse full-Desktop's broad preload/global ACP singleton for callbacks | AGENTS.md and PSR-003's disposition explicitly forbid importing broad full-Desktop preload/global architecture; only pure, contract-fitting reducers may be reused (per `SHP-ASM-034`, still to validate at R3). |

## Consequences

The shell becomes a usable agent client instead of a preflight-only connection: a consumer can
create/resume a session, prompt, observe streamed output, and answer interactive requests through a
narrow typed boundary. This adds real IPC/controller surface area (more channels, more negative-
space tests) but keeps the trust boundary unchanged — every new capability is additive to, not a
replacement of, ADR-0008's frozen allowlist discipline. R3 (PG-30) is where this ADR's contracts get
implemented and proven with a deterministic fixture-provider workflow; this ADR does not itself
claim the workflow works.

## Dependency record

No new dependency is approved. Implementation reuses the existing generated
`@repo-makeover/gosling-sdk` types and the existing `ShellGenerationRequest`/compatibility/
diagnostics machinery.
