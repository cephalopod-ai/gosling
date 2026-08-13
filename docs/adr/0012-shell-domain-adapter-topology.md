# ADR-0012: Domain adapter lifecycle, transport, and authority

Date: 2026-08-13
Status: proposed — pending operator acceptance (R1 architecture review; see
`pre-gui-backend-implementation-plan.md` PG-13 and `project-shell-readiness-plan.md` §4.2)
Requirements affected: SHP-REQ-002, SHP-REQ-006, SHP-REQ-007, SHP-REQ-010, SHP-REQ-018,
SHP-REQ-036, SHP-REQ-042

## Context

`DomainAdapter` (`crates/gosling/src/acp/shell.rs:14-26`) is an in-process Rust trait
(`descriptor`/`snapshot`/`perform_action`) held as `Option<Arc<dyn DomainAdapter>>` on
`ShellRuntime` (line 32). `ShellRuntime::domain_snapshot`/`perform_domain_action` (lines 100-124)
return `method_not_found` whenever the adapter is absent. The only production construction site,
`build_shell_runtime` in `crates/gosling-cli/src/cli.rs:1359`, always calls
`ShellRuntime::new(provisioning, None)` — no concrete adapter is ever registered outside tests.
`ShellProvisioning.domain_adapter: Option<DomainAdapterDescriptor>`
(`crates/gosling-sdk-types/src/shell.rs:87`) can still describe an adapter, and
`shell_validation.rs:379-397` validates that descriptor's shape, so a provisioning document can
claim domain capability the running server can never fulfill. This is PSR-004/SHP-DEF-024 and the
root cause of SHP-RSK-033 ("domain adapter descriptor creates false capability while production
runtime has no adapter"). Because `DomainAdapter` is a Rust trait object, any concrete
implementation must currently be compiled into the `gosling` binary — exactly the "domain crate
linked into generic Gosling core" outcome the parent plan's binding boundaries (§3.4) and PG-INV-007
forbid.

## Decision

Adopt a **versioned, out-of-process adapter** as the only supported production topology, matching
the plan's stated preference (`project-shell-readiness-plan.md` §4.2, `SHP-ASM-031`) and the
existing ACP/MCP architectural pattern already used elsewhere in this codebase (`crates/gosling-mcp`).
Specifically:

- **Transport and wire protocol.** The adapter process is a standard **MCP server speaking JSON-RPC
  over stdio** — the exact `rmcp`-based mechanism this codebase already uses both to serve MCP
  (`crates/gosling-mcp/src/mcp_server_runner.rs:37`, `rmcp::transport::stdio`) and to connect to
  externally supplied MCP servers as a client (`ExtensionConfig::Stdio` in
  `crates/gosling/src/agents/extension.rs:178-200`, with its existing `cmd`/`args`/`envs`/`timeout`/
  `cwd` fields). No new transport, port, loopback URL, or bespoke authentication scheme is invented:
  a locally spawned stdio child inherits its pipes directly from the parent, so there is no network
  surface to authenticate. The adapter exposes exactly **three** MCP tools:
  - `descriptor()` — takes no input, returns the live `{ domain_id, display_name, version, actions }`
    shape of `DomainAdapterDescriptor`. `serverInfo`/tool listing from MCP's own `initialize`
    identifies the server process, not the domain-action set, so this tool is the actual live
    negotiation surface: the Rust server calls it once at startup and compares the result against the
    descriptor `ShellProvisioning.domain_adapter` and the consumer manifest declare, failing closed
    (`incompatible`) on any mismatch before `ready` — a two-tool protocol with no descriptor exchange
    would let a mismatched adapter reach `ready` on trust alone (PG-INV-004, PG-INV-009).
  - `snapshot(input)` — matches `DomainSnapshotRequest`, returns the shape of `DomainSnapshotResponse`
    (`crates/gosling-sdk-types/src/shell.rs:204-217`).
  - `action(action, input)` — an **adapter-facing** request shape, deliberately narrower than the
    canonical `DomainActionRequest`: it has no `confirmation_token` field at all, because no token ever
    reaches this hop. For a `read` action the server calls `action` directly. For a `mutate` action the
    server never calls `action` until its internal confirm/approve step (§ "Authority" below) succeeds
    — the confirmation token, where one exists, is minted and consumed entirely inside that internal
    step, before this tool call happens, and authorizes the *Rust server's* decision to make the call
    at all. The adapter tool schema and the canonical `DomainActionRequest` DTO are intentionally
    different shapes for different hops, not the same wire message end to end.
- **Process ownership.** The Rust server (not Electron main, not the renderer) owns adapter process
  lifecycle — launch, health/readiness, timeout, and cleanup — using the *same* child-process
  supervision code path that already spawns and supervises `ExtensionConfig::Stdio` extensions, not a
  new one. This keeps domain authority server-side, consistent with
  `docs/architecture/shell-foundation.md`'s statement that "Domain implementations remain responsible
  for their own semantics and authority" while Gosling remains the settings/session/policy authority.
- **Registration and negotiation.** Registration mirrors exactly how MCP extensions are already
  registered and referenced — never a `gosling-cli` source/rebuild step, closing the gap the first
  review round's fix reopened. `ShellSessionProvisioning.extensions: Option<Vec<ShellExtensionSelection>>`
  (`crates/gosling-sdk-types/src/shell.rs:53`) already references extensions **by name** against
  entries registered in the user/operator-owned Gosling settings (not compiled into the binary, not
  inline commands in provisioning); a `Stdio`-shaped adapter registration is added to that same
  settings surface, and `ShellProvisioning.domain_adapter` continues to carry only `domain_id` — an ID
  reference, exactly like `ShellExtensionSelection.name` — never an inline executable path. Adding a
  new project's adapter is therefore a **settings-file registration**, the same operation as adding a
  new MCP extension today, requiring no Gosling source edit and no rebuild — satisfying ADR-0010's
  copy-free requirement. `build_shell_runtime` resolves `domain_id` against that registry and spawns
  the adapter via the existing `ExtensionConfig::Stdio` launch path, wrapping the connection in a thin
  `DomainAdapter` implementation (a process-transport adapter, not a domain adapter) rather than
  calling `ShellRuntime::new(provisioning, None)` unconditionally. An unregistered `domain_id` fails
  closed (`ADAPTER_NOT_REGISTERED`) rather than silently falling back to `None`. Before the runtime
  reports `ready`, the live adapter's version, schema, and action set are compared against the
  descriptor and against what the consumer manifest (ADR-0010) declares; any mismatch is
  `incompatible`/`degraded`, never a silently accepted descriptor-only success (PG-INV-004,
  PG-INV-009). This directly answers PSR-004's disposition requirement: "Until an ADR and conformance
  fixture prove this path, `DomainAdapter` is an unfulfilled internal seam, not a supported consumer
  capability."
- **Authority.** `domain_snapshot` stays read-only, unchanged from today's shape. `perform_action` for
  a `mutate` action first returns `CONFIRMATION_REQUIRED` plus a `confirm` interaction ID rather than
  executing; the server retains the pending `action`/`input` alongside that interaction record so
  nothing needs to be resupplied later. Approval is handled entirely **inside the Rust server** through
  a **new, distinct custom ACP method**, `_gosling/unstable/shell/domain/action/confirm`
  (`DomainActionConfirmRequest { action_id, approve: bool } → DomainActionConfirmResponse { status:
  approved|denied, result: Option<DomainActionResponse> }`), not by overloading
  `perform_domain_action`/`DomainActionRequest` — that request shape has no field for an interaction's
  opaque `action_id` or an approve/deny decision, so reusing it would leave `confirmation.respond`
  uncallable. On approval, the server mints and immediately consumes an
  **action-bound, single-use confirmation token internally**, executes the retained pending action,
  and returns the resulting `DomainActionResponse` shape inline as
  `DomainActionConfirmResponse.result` — in the *same* response, with no second
  `perform_domain_action` round trip. The token itself never appears on any wire DTO and never reaches
  Electron main, the renderer, or the adapter process; `DomainActionRequest.confirmation_token`
  (`crates/gosling-sdk-types/src/shell.rs:229-230`) is unused by this flow and remains present only for
  API compatibility with a hypothetical future direct caller. Electron main's role is strictly to
  relay `confirmation.respond` to the server and relay `DomainActionConfirmResponse` back to the
  renderer unmodified — it is not a second authority for anything in this flow. Replayed, expired,
  cross-action, cross-session, or cross-generation confirm interactions fail closed. Electron main and
  the renderer never talk to the adapter process directly — all adapter traffic is proxied through the
  existing `domain_snapshot`/`perform_domain_action` custom ACP methods the server already exposes.
- **Payload ownership.** Native adapter payloads stay opaque to `gosling-cli`/`gosling-server` and to
  the shared Electron host; they are validated only by the project consumer's own generated schema
  (ADR-0010), matching the existing DTO comment "Domain implementations remain responsible for their
  own semantics."
- **Failure/bounds.** Absent, mismatched, crashing (before/after ready), hanging, malformed-output,
  and overproducing adapters map to typed lifecycle/domain error codes (extending the existing error
  taxonomy in `shell-productization-contracts.md`) and always leave no orphaned process — reusing the
  same generation-owned cleanup discipline `ShellRuntime`'s host process already implements.
- **Post-ready health.** A crash or hang detected only by the *next* `domain_snapshot`/
  `perform_domain_action` call is not enough: an idle adapter can die with no domain request in
  flight, and main has no other way to learn about it — main receives adapter metadata exactly once,
  in the ACP `initialize` response. Rather than adding a polled health-check operation (more IPC
  surface, and still only as fresh as the last poll interval), the Rust server pushes a **new custom
  ACP notification**, `_gosling/unstable/shell/domain/status`
  (`DomainStatusNotification { status: ready | crashed | hung | incompatible }`), whenever its own
  process supervision observes an adapter status change outside of a request/response cycle — this
  reuses the existing agent-to-client asynchronous notification pattern ACP already has for session
  updates (`sessionUpdate`), rather than inventing a second async channel. Electron main folds this
  into the safe runtime snapshot's `adapter.status` and pushes it to the renderer via the existing
  `runtime.changed` event — no new renderer-facing operation is added, only a new input to main's
  already-existing snapshot/push machinery.
- **Migration of the current trait.** The current in-process `DomainAdapter` Rust trait
  (`crates/gosling/src/acp/shell.rs:14`) is retained as the **internal boundary between
  `ShellRuntime` and the process-transport implementation**, not exposed as a second production
  extension mechanism. No second, ambiguous "in-process Rust adapter" production path is introduced;
  a future in-process adapter would require its own ADR and explicit rejection of this one's
  out-of-process default (per the parent plan's binding boundary: "No dual ambiguous production
  paths").

## Alternatives considered

| Alternative | Why rejected |
| --- | --- |
| Keep `DomainAdapter` as a Rust trait object implemented by project code compiled into `gosling` | Requires linking project (potentially proprietary) domain code and its transitive dependencies into the generic Gosling binary/ABI; directly contradicts PG-INV-007 and SHP-RSK-037, and couples release cadence of Gosling core to every project's adapter. |
| Load a native `cdylib`/shared library selected by provisioning | Same ABI-coupling and supply-chain risk as above, with weaker isolation (a crashing or malicious library can take down the host process); the plan's binding boundaries explicitly reject "no native library loading." |
| Descriptor-only capability advertisement with no live negotiation ("trust the manifest") | This is precisely PSR-004/SHP-RSK-033's failure mode — a provisioning document can claim capability the runtime cannot fulfill, producing false `ready`. PG-INV-004 requires live equality. |
| Route adapter traffic directly from Electron main or the renderer to the adapter process, bypassing the Rust server | Creates a second mutation/session authority outside Rust (forbidden by the parent plan §3.3) and would require the adapter to independently re-derive session/permission context the server already owns. |
| Support arbitrary transports chosen per adapter (stdio, TCP, Unix socket, HTTP) in the first contract | Multiplies the supervision/timeout/cleanup surface this ADR must prove; one stdio MCP transport, reusing the existing `ExtensionConfig::Stdio`/`rmcp` pattern, is sufficient for the neutral R4 fixture and can be revisited with evidence. |
| A new loopback-TCP-plus-generated-secret transport, mirroring the Electron-main-to-`gosling-serve` ACP link | That pattern exists specifically to authenticate a connection Electron main did not itself spawn as a direct child with inherited pipes; the adapter here *is* a directly spawned child of the Rust server, so stdio needs no network authentication step, and reusing the existing MCP stdio path avoids inventing and securing a second network listener. |
| Compile a `domain_id → executable` registry into `gosling-cli` source (the first review round's fix) | Requires a Gosling source edit and rebuild for every new consumer's adapter, which directly contradicts ADR-0010's copy-free consumer topology; superseded by registering the adapter the same way an MCP extension is registered — in user/operator-owned Gosling settings, referenced by ID. |

## Consequences

`build_shell_runtime` changes from an unconditional `None` to a real, capability-negotiated
registration path — closing PSR-004 and giving `DomainAdapter` an actual production caller for the
first time. This adds a new supervised child-process class to the Rust server (timeout, crash,
cleanup, confirmation-token bookkeeping) that R4 (PG-40) must implement and prove with the neutral
fixture adapter before any named project (DAWES, math, Project ABC) may register a real one. Full
Gosling without a project adapter is required to remain unaffected — `ShellRuntime::main_gosling()`
(`crates/gosling/src/acp/shell.rs:36`) continues to pass `None` and is not touched by this decision.

## Dependency record

No new dependency is approved by this ADR. The selected transport reuses `rmcp`
(`crates/gosling-mcp/src/mcp_server_runner.rs:4`, already a dependency) and the existing stdio
child-process extension pattern (`ExtensionConfig::Stdio`,
`crates/gosling/src/agents/extension.rs:178-200`); R4 implementation must cite the specific reused
module or justify any new crate at that time.
