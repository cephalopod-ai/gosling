# Gosling shell foundation

Gosling shells are focused clients over the shared Gosling runtime. They reduce
cognitive load without becoming independent configuration authorities or
forking the agent engine.

## Current foundation

A shell runtime is fixed by the server process, not asserted by renderer RPC
parameters. `gosling serve` accepts a shell identity, optional provisioning
file, and runtime namespace. The resolved identity is returned in ACP
initialization metadata and through the read-only
`_gosling/unstable/shell/provisioning/read` method.

The default authority mode is `inherit`. It preserves the same protocol and
tool authority as the main Gosling client. A provisioning document may opt
into `restricted` mode and name custom ACP methods to deny. Denial occurs in
the server before custom-method dispatch; hiding a UI control is not the
security boundary.

The provisioning contract contains references and selections, not secret
values:

- workspace and credential-profile IDs;
- optional provider and model defaults;
- optional extension selection and per-extension tool allowlists;
- optional skill IDs enforced by the session skills extension;
- an optional domain-adapter descriptor.

`gosling shell-validate` resolves those references against main Gosling state
without starting an ACP server and emits a structured JSON report. The same
report is returned by `_gosling/unstable/shell/provisioning/read` and
`_gosling/unstable/shell/provisioning/validate`; session creation rejects an
invalid server provisioning document before creating durable session state.
Validation covers schema and identity shape, workspaces, credential profiles,
registered providers, model configuration shape, enabled extensions, extension
tool-selection shape, skills, denied custom methods, and domain-adapter
descriptor shape. Provider model catalogs can be credential-backed and dynamic,
so preflight does not reject a non-empty model merely because it is absent from
static metadata; session startup performs the authoritative provider model
availability check. Extension tool names likewise remain runtime-discovered MCP
metadata, so preflight validates their selection syntax while the extension
verifies actual names when it loads. Session creation performs directory-scoped
validation only after resolving the effective working directory, including a
workspace working-directory override, so project-local extension and skill
selection matches the directory used by the runtime.

Gosling remains the settings authority. Namespaced shell runtimes share the
main Gosling configuration and protected credential catalog while separating
data and state directories, including session databases and caches.

## Domain adapters and handoff

`DomainAdapter` keeps only a common transport envelope: descriptor, snapshot,
action, native payload, and exact resource references. Domain implementations
remain responsible for their own semantics and authority.

Shell-to-Gosling handoff is explicit. The server prepares a versioned envelope
containing the server-fixed origin, source session, question, requested
capability, exact references, mutation intent, and return destination. The
foundation does not silently widen a domain session.

## Desktop composition

`createMinimalShellHost` reuses the authenticated `gosling serve` lifecycle
without copying the general Desktop main process. It returns secure baseline
`BrowserWindow` options and the managed backend lifecycle. `ShellFrame` and
`ShellStatus` are the initial shared presentation primitives. No domain shell
or domain-specific workflow is created by this foundation.

A spawned-process regression test exercises authenticated ACP initialization,
provisioning read/validation, session creation, extension and skill filtering,
server-side policy denial, namespace isolation, and persistence across a server
restart.

Package names, executable names, protocol schemes, and Linux package IDs can be
composed at build time through `GOSLING_SHELL_PRODUCT_NAME`,
`GOSLING_SHELL_PROTOCOL_SCHEME`, and `GOSLING_SHELL_PACKAGE_ID`. Shell-specific
icons and updater feeds remain distribution inputs. Runtime namespaces are
already independent.
