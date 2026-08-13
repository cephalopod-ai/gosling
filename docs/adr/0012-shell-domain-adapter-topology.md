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
  surface to authenticate. The adapter exposes exactly two MCP tools whose JSON schemas are the
  already-generated canonical DTOs — `snapshot(input)` returning the shape of `DomainSnapshotResponse`
  and `action(action, input, confirmationToken)` returning the shape of `DomainActionResponse`
  (`crates/gosling-sdk-types/src/shell.rs:204-242`); `DomainActionRequest.confirmation_token` already
  exists on that DTO today, so this reuses rather than adds a wire field. MCP's own `initialize`
  handshake supplies protocol/version negotiation; the adapter's declared `DomainAdapterDescriptor`
  (`domain_id`, `version`, `actions`) is compared against the live `serverInfo`/tool list before
  `ready`.
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
- **Authority.** `domain_snapshot` stays read-only, unchanged from today's shape. `perform_action`
  additionally requires an **action-bound, single-use confirmation token** minted and validated by
  the server only, matching the existing `DomainActionRequest.confirmation_token: Option<String>`
  field (`crates/gosling-sdk-types/src/shell.rs:229-230`) — this ADR does not add a wire field, it
  specifies how that already-existing field gets populated. The server mints the token only after the
  renderer's explicit approval of a `confirm` interaction naming that exact action (mechanics frozen
  in `shell-productization-r1-contracts.md`) and returns the opaque value as the result of that
  approval call; Electron main relays this single-use, short-lived, scope-bound value from the
  approval response into the immediately following `perform_domain_action` call without being able to
  mint, forge, extend, or independently validate it — main is a relay for an opaque bearer value here,
  the same relationship it already has to the ACP loopback secret, not a second authority. Replayed,
  expired, cross-action, cross-session, or cross-generation tokens fail closed. The token is never
  sent to the adapter process — it authorizes the *server's* dispatch decision, not the adapter call.
  Electron main and the renderer never talk to the adapter process directly — all adapter traffic is
  proxied through the existing `domain_snapshot`/`perform_domain_action` custom ACP methods the server
  already exposes.
- **Payload ownership.** Native adapter payloads stay opaque to `gosling-cli`/`gosling-server` and to
  the shared Electron host; they are validated only by the project consumer's own generated schema
  (ADR-0010), matching the existing DTO comment "Domain implementations remain responsible for their
  own semantics."
- **Failure/bounds.** Absent, mismatched, crashing (before/after ready), hanging, malformed-output,
  and overproducing adapters map to typed lifecycle/domain error codes (extending the existing error
  taxonomy in `shell-productization-contracts.md`) and always leave no orphaned process — reusing the
  same generation-owned cleanup discipline `ShellRuntime`'s host process already implements.
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
