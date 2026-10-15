# Trilium bounded ACP integration — 2026-10-15

## Scope and authority

The user requested implementation, focused tests, builds and an isolated UI run of a generic Trilium → Gosling → selected provider/model integration. Starting revisions: Trilium `0758ef0009`, Gosling `0dc605f3d`. Changes live on `feat/gosling-acp-integration` and `feat/trilium-bounded-acp`; no installation replacement, merge or publication was requested. Existing Trilium changes were preserved. No Muninn code was changed.

Private catalog workflows `plan-cross-stack-contracts` and `plan-polyglot-staged-delivery` grounded the contract and gated implementation. The durable local checkpoint and raw logs are in `/tmp/trilium-gosling-work`. The user explicitly authorized build/test and synthetic live inference; no real notes were used.

## Result and protocol

Trilium now provides a named Gosling provider over local stdio `gosling acp`, generic inventory/model/reasoning selection, chat-local host session mappings, ordered streaming and cancellation, transient approvals and revocable MCP note access. Gosling retains authentication, inference and the sole agent loop. No REST/OpenAPI service or separate vendor adapters were introduced.

A narrow opt-in `clientToolPolicy` v1 was required because ordinary ACP extension selection and client-side approval denial do not constrain provider-owned tool runtimes or remembered/global approval settings:

- `initialize` advertises `_meta.goslingClientToolPolicyVersion: 1`.
- New-session metadata accepts `_meta.clientToolPolicy: {version: 1}` and requires explicit `enabledExtensions`, including `[]`. The policy persists under `client_tool_policy.v1`.
- New/load receipts report policy version and exact provider/model IDs. Load retains the saved extension allowlist and may refresh only previously selected client MCP endpoints.
- Actual dispatch checks the concrete host extension identity. Unknown/unselected identities fail closed. The mandatory inspector requests approval even in Auto mode, independent of remembered allows. Missing durable sessions cannot dispatch.
- Bounded sessions refuse workspace/builtin tools and providers that execute tools outside Gosling, and disable automatic route fallback and ancillary configured context channels. Ordinary clients retain prior defaults.
- Provider inventory adds `executesToolsOutsideGosling` and non-secret endpoint locality. Optional `includeHiddenConfigured` exposes configured hidden/custom routes; its default remains false. Generated ACP schema and SDK types were regenerated.

The existing explicit-extension initialization path dropped ACP client MCP servers; it now preserves them. This is necessary for bounded note tools and is covered by native read/write exercises. No provider credentials are returned to Trilium, and no authentication fallback is added.

## Focused validation

All Rust commands below used `source bin/activate-hermit` first. Relevant final results:

| Command | Result |
| --- | --- |
| `cargo test -p gosling --lib --no-default-features session::client_tool_policy` | 1 passed |
| `cargo test -p gosling --lib --no-default-features agents::interaction_policy` | 1 passed |
| `cargo test -p gosling --test client_tool_policy_test --no-default-features` | 1 passed; persisted reload, Auto-mode mandatory approval, unknown dispatch denial |
| `cargo test -p gosling --lib --no-default-features acp::server` | 88 passed |
| `cargo test -p gosling --lib --no-default-features agents::agent` | 73 passed |
| `cargo clippy -p gosling --all-targets --no-default-features -- -D warnings` | passed |
| `cargo fmt --all -- --check` | passed |
| `cargo build -p gosling-cli` | full default-feature debug CLI build passed |
| `cd ui/sdk && pnpm run lint` | generated SDK typecheck passed |

`dispatch_app_tool_call_runs_inspectors_in_auto_mode` initially used a nonexistent literal session. The new durable dispatch guard rejected that fixture. It now creates a durable session and still proves the inspector denies execution. The final 73-case agent run passes. An initial no-default-feature CLI build lacked the required TLS backend; the corrected default-feature CLI build passed. No full workspace/desktop/native-Windows certification is claimed.

Trilium's focused server/client/standalone tests, repository typecheck, production server/client build and generated-help checks passed. The exact Trilium boundary counts and commands are in its integration validation report.

## Runtime evidence and limits

An isolated Trilium profile, copied from its synthetic fixture database, ran on `127.0.0.1:39160`, with synchronization unconfigured. A separate synthetic Gosling profile used a loopback model fixture on `39161`, a synthetic key and Auto mode. Native browser checks demonstrated:

- Add Gosling without an API key or endpoint; explicitly select a discovered model.
- Stream and persist a reply through the actual native Gosling process.
- Require a once-only approval for note reads despite Auto mode.
- Reject a write without changing the note; approve a write and verify the actual synthetic note content/tool result.
- Cancel during active inference with note capabilities present, persist an interrupted error, revoke access, reload and confirm no note tool is available on the next explicit handoff.
- Restart the Trilium application after a completed turn, load the saved native Gosling session and continue with exactly four saved messages across two turns, without replay or duplication.
- Reject unauthenticated inventory access (401) and an authenticated approval POST without CSRF (403).

Real configured inventory initially returned 41 provider entries, 16 configured: 10 compatible with bounded execution and 6 unsafe provider-owned runtimes disabled. Three local configured routes reported no models; local model inference was not demonstrated. Other model listings do not establish access or entitlement.

A direct ACP synthetic call using the existing `chatgpt_codex / gpt-6.1-sol` subscription route streamed `TRILIUM_GOSLING_SYNTHETIC_OK`, ended with `end_turn` and reported 728 total tokens (713 input, 15 output). No API fallback or tools were used. This is one model/route live result, not all-provider certification.

Subsequent real-profile inventory requests initially stalled in macOS Keychain. A process sample showed `Config::all_secrets` → `SecKeychainFindGenericPassword` → SecurityServer waiting. Native UI inspection was unavailable (`Sky Computer Use native pipe startup failed`); the user completed the existing-credential OS approval themselves. The retry through the actual Trilium UI passed: inventory exposed 41 providers (16 configured), explicit `chatgpt_codex / gpt-6.1-sol` selection and low reasoning streamed the expected synthetic reply and saved exactly two messages. After a Trilium application restart, the same native Gosling session resumed, retained low reasoning and note access off, and saved the expected follow-up with exactly four messages and no replay or duplication. Screenshots and raw logs are in `/tmp/trilium-gosling-work`. No credential was copied, exposed, replaced, disabled or bypassed. Named Muninn, cephalopod-ai, Alexandria and math_mcp integrations are selectable through the generic extension contract but were not invoked live.

Existing installed CLI/Desktop and the user's Trilium knowledge base remain unchanged. To use the patch, point Trilium's `TRILIUM_GOSLING_BINARY` at this checkout's `target/debug/gosling`; stopping the isolated Trilium preview and removing its environment overrides returns to the existing installation.

## Main integration follow-up

The user subsequently requested merging with main and synchronizing the remotes. Trilium's integration was already committed as `c312b307bd`; Gosling's companion is committed with this record. Both remote main tips matched their original bases, allowing history-preserving fast-forward integration. Publication order is Gosling first, then Trilium. No feature branch deletion, force push or protection bypass is in scope. Final combined-tree checks and published-ref readback are recorded in `/tmp/trilium-gosling-work/merge-report.md`; the earlier live-provider validation limits remain unchanged.
