# 2026-10-02 — Session and workspace authentication

## Scope and checkpoint

Target: `/Users/eric/Work/vscode/forked/gosling`, `main` at `c036ce53d`, initially clean.
The operator requests adding/removing both provider and MCP authentication at session/workspace
scope. Workspace changes apply only to new chats; existing chats retain their bindings.
Assumption: removal disconnects the target and retains stored accounts elsewhere. No live
credentials are read, printed, revoked, or deleted during this implementation.

Catalog discovery used `repair-defect-priority` for diagnosis, then `plan-rust-app` execute mode
because active binding controls expand the existing read-only chat selector. Shared contracts and
Cargo validation guidance were read. Local implementation is authorized by the current request;
no install, publication, or external account action is included. Independent reads/checks are
batched; dependent storage/runtime changes remain sequential. Resume from this log and the diff.

## Findings and implementation plan

- AUTH-001: active-chat credential menu displays profiles but cannot attach/detach them. Add a
  typed ACP operation and UI controls for compatible provider profiles. Validate and construct
  the replacement before committing profile metadata and replacing the runtime provider.
- AUTH-002: null profile means global credentials, so detaching alone would silently reuse global
  auth. Persist an explicit disconnected state, enforce it at activation/transitions, and preserve
  chat history and reconnect controls.
- AUTH-003: MCP OAuth receives a credential store but recreates a global name-keyed store during
  the flow. Use the same selected store throughout refresh/sign-in. Pin opaque account references
  and scoped static secret fields; disconnect blocks the selected extension without deleting
  shared credentials. Workspace defaults are snapshotted into new chats only.

ADR-0001 owns atomic locked workspace JSON, ADR-0002 owns strict scoped provider resolution,
ADR-0003 owns immutable launch snapshots, and ADR-0004 owns typed ACP/backend authority. These are
conformant for existing bindings, but lack explicit attach/detach policy. Record an authorized
amendment without altering global account deletion or folder/permission policy. Persist only
metadata in workspace/session stores, reuse Config secure storage for secret values, retain
existing global bindings for backward compatibility, and strip new auth metadata from imports.

No dependencies/toolchain changes. Existing Rust edition 2021/MSRV 1.91.1/toolchain 1.92, empty
default features, macOS arm64 host, build scripts/proc macros and Cargo configuration inspected.

## Baseline

- `cargo check --locked --offline -p gosling --lib --tests` under activated Hermit,
  `RUSTUP_AUTO_INSTALL=0`, CommandLineTools: passed (1.83 seconds).
- Four focused Desktop test files (credential selector, workspace editor, auth settings,
  extension agent API): 33 tests passed.
- Rust runtime tests/build/Clippy remain gated by AGENTS.md's explicit-request rule. Use Rust
  typecheck, Desktop tests, formatting/lint, schema generation, and isolated regression targets.

## Implementation and disposition

- AUTH-001: source-fixed, partially validated. Typed ACP authentication read/provider-set/extension-set
  requests and Desktop dialogs are available through the active-chat credential control and workspace
  actions. Only configured, compatible provider profiles are selectable; replacement construction
  precedes persisted/live replacement. Normal workspace edits retain authentication settings.
- AUTH-002: source-fixed, partially validated. Host-owned `authentication.v1` records explicit
  disconnection. An atomic source SQL update commits profile metadata and policy together while
  preserving other extension state. Restoration, manager fallback and direct provider updates cannot
  use a disconnected provider. Session list metadata exposes its disconnected status. An explicit MCP
  auth policy also blocks unscoped restoration fallback. Account deletion semantics remain separate.
- AUTH-003: source-fixed, partially validated. Caller-selected OAuth stores now survive sign-in and
  refresh. Scoped UUID/URI keys and strict static-field lookup isolate account data; changing a target's
  credentials uses a fresh namespace. Destination fingerprints reject redirecting a saved binding.
  Disconnect suspends the selected client and retains reconnect metadata; failed replacement suspends
  the old client. Workspace auth snapshots are copied only at new-chat creation. Imported transcripts
  strip host auth references; workspace import/export strips scoped account references. Empty field
  metadata is omitted so exports continue to obey the existing secret-shaped-field rejection.

Canonical DTOs, core persistence/lifecycle/provider/OAuth helpers, typed ACP handlers, generated SDK
contracts, thin Desktop adapters, chat/workspace controls and focused regression fixtures changed.
ADR-0002/0003/0004, workspace usage, docs index, test ledger and this retained record explain the
authorized contract amendment. No dependency, toolchain, permission-policy or saved-account change.
The exact `.gitignore` negation retains this evidence. Concurrent Muninn provider/schema work and its
docs/ignore changes were preserved; its historical blocked-check row remains point-in-time evidence.

## Validation

- Core and SDK types: `cargo check --locked --offline -p gosling --lib --tests -p gosling-sdk-types`
  with activated Hermit, `RUSTUP_AUTO_INSTALL=0` and CommandLineTools; final rerun passed
  (1 minute 2 seconds). Regression fixtures cover provider rollback/disconnect,
  reload/copy/import isolation, future workspace snapshots, scoped secret fallback rejection, UUID/URI
  OAuth stores and request Debug redaction. These Rust tests were compiled, not executed.
- Consumers: `cargo check --locked --offline -p gosling-cli -p gosling-sdk`; passed (59.34 seconds).
- Desktop: seven focused files (AuthenticationDialog, CredentialProfileSelector, SessionInfoSummary,
  WorkspaceSidebarSection, WorkspaceEditorDialog, AuthSettingsSection and extension agent API);
  **52 tests passed**. Typecheck and targeted ESLint/Prettier passed. `pnpm run i18n:extract` retained
  recovery catalogs and added three source messages; compilation/check passed, including **21 locale
  synchronization tests** and validation of all 15 non-English catalogs (1,427 messages).
- SDK: regenerated ACP schema/meta and TypeScript clients/validators; TypeScript build, source/test
  typechecks and **7 SDK tests passed**, including scoped MCP request log redaction.
- Source-derived SQLite check executed the actual Rust update SQL in an isolated database: atomic
  disconnect/reconnect, preservation of another host key, other-chat isolation and missing target passed.
- Rust formatting, whitespace check, **85 local documentation links** and both governance markers
  passed, including final fixture/code changes. Final Desktop typecheck, ESLint and Prettier passed.

Final review also prohibits carrying saved static secrets across a changed extension destination
without fresh values for the still-declared fields. Removed declarations are not copied. Final
validation completed after the task crossed into 2026-10-03; this record retains its start date.

## Residual limits and follow-up

Live OAuth, real provider/native-context replay, cross-platform replay and installed Desktop behavior
are unverified. Rust build/test/Clippy, packaging, installation, commits and publication were not
requested and were not run. Providers owning MCP connections outside gosling reject active-chat MCP
auth mutations and scoped MCP sign-ins; workspace disconnect defaults still filter those connections
at new-session construction. A future scoped adapter is needed to enable that provider-owned surface.
Unreferenced secrets retained after credential replacement or a failed/cancelled save are not
garbage-collected automatically. Old schema-v1 readers cannot enforce new auth policy on downgrade.

Status: source-fixed, partially validated, deployment-unverified. No claim of full runtime closure,
external account revocation or fleet compliance. Rollback is the scoped source/docs/generated diff;
new auth metadata is additive, and disabling/downgrading enforcement requires checking account scope.
