# Gosling Comprehensive Audit Report - 2026-07-06

## Executive Verdict

Verdict: high-risk posture remains for untrusted tool execution, untrusted project content, and desktop renderer-to-main boundaries. No critical issue was confirmed statically in this pass, but several high-severity paths collapse user approval, plugin trust, or local filesystem isolation.

The repository has fixed several issues from earlier point-in-time audits, including fail-closed inspector error handling, atomic secrets writes, plugin clone option termination, legacy settings migration, and `gosling://` documentation consistency. The remaining risk is concentrated in five areas:

- Tool approval defaults and shortcuts: `GoslingMode::Auto` is still the default and allows every tool call.
- Untrusted extension/plugin execution: project plugins are discovered and enabled by default, hooks run shell commands, and local MCP subprocesses inherit the parent environment.
- Security inspection is advisory or disabled by default: prompt-injection scanning defaults off, command name coverage is narrow, and egress detection logs but always allows.
- Desktop IPC and CSP expose broad impact if renderer content is compromised: arbitrary filesystem IPC, inline scripts, broad loopback access, secret-key IPC, all-permission approval, and denylist external URL opening.
- Architecture and reliability debt make these controls hard to reason about: large agent/config/desktop entry points, global configuration access, inconsistent SDK sourcing, and retry defaults that retry generic client failures.

This was an audit-only static pass. I did not modify product code, run `cargo build`, run `cargo test`, or run `cargo clippy` because the request was to audit and report rather than build or change source.

## Scope

Repository: `/Users/eric/Work/vscode/forked/gosling`

Branch and commit: `main` at `7c08048a6`

Primary scope:

- Rust agent core, permission inspection, tool execution, extensions, plugins, hooks, config, ACP server, provider retry behavior.
- Electron desktop main/preload/renderer security boundaries, CSP, URL opening, file IPC, MCP app proxy.
- Text UI and docs were included for architectural and product-contract checks.
- Existing audit reports under `docs/cloud/` and `reports/` were used only as leads; every carried-forward finding below was rechecked against current source.

Commands used:

- Static search and source inspection with `rg`, `find`, `wc`, `git status`, `git rev-parse`, `sed`, and `nl`.
- No build, test, formatter, or linter commands were run.

## Skill Coverage

The user requested the local audit skill set under `~/Work/vscode/agent-skills/010_audit/`, all applicable skills, and at least 30 skills. I exercised 35 applicable audit lenses: 33 full and 2 partial. Three local skills were not applicable to this repository.

Full lenses applied:

- `audit-architecture-nodejs`
- `audit-architecture-seam`
- `audit-compliance-posture`
- `audit-contract-internalapi`
- `audit-dataflow-cascade`
- `audit-dataflow-concurrency`
- `audit-dataflow-input-output`
- `audit-dataflow-integrity`
- `audit-dataflow-pipeline-graph`
- `audit-dataflow-state-transition`
- `audit-dataflow-temporal`
- `audit-deadcode-cleanup`
- `audit-dependency-criticality`
- `audit-design-webapp`
- `audit-failsafe-readiness`
- `audit-invariant-sync`
- `audit-memory-lifecycle`
- `audit-multiagent-consensus`
- `audit-negative-space`
- `audit-operator-signal`
- `audit-performance-profile`
- `audit-pipeline-externalapi`
- `audit-recovery-idempotency`
- `audit-reliability`
- `audit-resource-lifecycle`
- `audit-security`
- `audit-security-code`
- `audit-security-llm`
- `audit-security-nodejs`
- `audit-security-repo-posture`
- `audit-security-repo-triage`
- `audit-security-vuln-harness`
- `audit-workflow-gui`

Partial lenses applied:

- `audit-contract-crossrepo`: limited to in-repo SDK, ACP, and UI/client contract evidence. No separate upstream/downstream repository was checked.
- `audit-playtest-app`: limited to static workflow and UI risk review. No desktop app was launched.

Not applicable:

- `audit-flutter-ios`: no Flutter/iOS app surface in scope.
- `audit-security-supabase`: no Supabase integration found in the scoped repository paths.
- `audit-equation-sourcebase`: no mathematical or financial equation sourcebase surface found.

## Draft Prompt Assessment

The prompt had enough authority to perform an audit and write a report, but it did not define a specific threat model, target platform, or acceptable runtime validation budget. I treated it as a comprehensive static audit request with the local audit skill bundle as the review rubric.

Because the repository is large and includes generated/vendor-heavy areas, this pass used a risk-prioritized strategy rather than line-by-line review of every file. Findings below are included only when current source evidence supports them.

## Surface Inventory

| Surface | Key files | Risk focus |
| --- | --- | --- |
| Agent permission mode and tool execution | `crates/gosling-providers/src/gosling_mode.rs`, `crates/gosling/src/permission/permission_inspector.rs`, `crates/gosling/src/agents/tool_execution.rs`, `crates/gosling/src/agents/agent.rs` | Whether tool calls require user approval and whether inspectors are enforced. |
| Security inspectors | `crates/gosling/src/security/mod.rs`, `crates/gosling/src/security/scanner.rs`, `crates/gosling/src/security/egress_inspector.rs`, `crates/gosling/src/tool_inspection.rs` | Prompt injection, egress, and fail-closed behavior. |
| Extensions, plugins, hooks | `crates/gosling/src/plugins/discovery.rs`, `crates/gosling/src/hooks/mod.rs`, `crates/gosling/src/agents/extension_manager.rs`, `crates/gosling/src/agents/extension_malware_check.rs` | Untrusted project code and MCP process execution. |
| Config and secrets | `crates/gosling/src/config/base.rs` | Durable config writes, parse-error behavior, and secret handling. |
| ACP server and app tools | `crates/gosling/src/acp/server/tools.rs`, `crates/gosling/src/acp/mcp_app_proxy.rs`, `crates/gosling-server/src/routes/mcp_app_proxy.rs` | App-initiated tool calls and MCP app auth consistency. |
| Desktop main/preload/renderer | `ui/desktop/src/main.ts`, `ui/desktop/src/preload.ts`, `ui/desktop/src/utils/csp.ts`, `ui/desktop/src/utils/urlSecurity.ts`, `ui/desktop/src/components/McpApps/McpAppRenderer.tsx` | Renderer compromise blast radius and local file/URL/secret exposure. |
| Provider retry | `crates/gosling-providers/src/retry.rs`, `crates/gosling-providers/src/base.rs`, `crates/gosling-providers/src/ollama.rs` | Permanent-vs-transient retry behavior. |
| Product contracts and docs | `README.md`, `documentation/INDEX.md`, `ui/*/package.json`, `documentation/GOOSE_COMPATIBILITY.md` | Rebrand compatibility and cross-package drift. |

## Boundary Map

| Boundary | Evidence | Assessment |
| --- | --- | --- |
| User intent to tool execution | `GoslingMode::Auto` default, permission inspector allows Auto, tool executor dispatches on allow | Weak by default. User approval is bypassed unless configured away from Auto. |
| Model output to local tools | Prompt-injection scanner defaults off; shell-name matching only recognizes `shell`; egress inspector logs and allows | Weak default prevention. Detection exists but is not a reliable gate. |
| Project workspace to agent process | Project `.agents/plugins` are discovered; unknown plugins are inserted enabled; hooks execute commands | Weak. Opening a repository can activate project-defined execution surfaces. |
| MCP server to host environment | Non-container stdio MCP command inherits parent environment | Weak. Environment minimization is not enforced for local MCP subprocesses. |
| Renderer to Electron main | Preload exposes read/write/delete/list/secret IPC; main implements arbitrary path operations | High-impact boundary if renderer content is compromised. |
| ACP app client to tools | App tool route calls extension manager directly | Missing parity with normal agent permission and hook path. |
| Stored config to durable state | Corrupt YAML is treated as empty for write operations | Fail-open durability behavior can erase previous config on the next write. |

## Findings Table

| ID | Severity | Confidence | Domains | Finding |
| --- | --- | --- | --- | --- |
| GSL-SEC-001 | High | Confirmed | Security, LLM, workflow | Default `GoslingMode::Auto` automatically approves every tool call. |
| GSL-SEC-002 | High | Confirmed | Security, LLM, failsafe | Prompt-injection scanning defaults off and shell detection is narrow. |
| GSL-SEC-003 | Medium | Confirmed | Security, egress, operator signal | Egress inspector detects destinations but always returns allow. |
| GSL-SEC-004 | High | Confirmed | Security, resource lifecycle | Local stdio MCP subprocesses inherit the parent environment. |
| GSL-SEC-005 | High | Confirmed | Security, plugins, workflow | Project plugins auto-enable and can execute hooks or MCP servers from the repository. |
| GSL-SEC-006 | Medium | Confirmed | Security, dependency criticality | OSV/malware guard is narrow and fails open on unknown commands and service errors. |
| GSL-CON-007 | High | Confirmed | Internal API, workflow, security | ACP app tool calls bypass normal permission inspection and `PreToolUse` hooks. |
| GSL-DAT-008 | Medium | Confirmed | Data integrity, recovery | Corrupt config files are treated as empty during read-modify-write updates. |
| GSL-NODE-009 | High | Confirmed | Node/Electron security | Desktop preload exposes arbitrary filesystem IPC to renderer code. |
| GSL-NODE-010 | High | Confirmed | Node/Electron security, CSP | Desktop CSP permits inline scripts and broad loopback, while renderer IPC exposes the ACP secret. |
| GSL-NODE-011 | Medium | Confirmed | Node/Electron security | Electron permission handler approves every permission request. |
| GSL-NODE-012 | Medium | Confirmed | Node/Electron security | External URL opening uses a denylist despite a safer protocol allowlist existing. |
| GSL-CON-013 | Medium | Confirmed | Internal API, cross-contract | MCP app proxy authenticates by query-string secret and core/server guest auth differs. |
| GSL-REL-014 | Medium | Confirmed | Reliability, performance | Provider retry defaults retry generic `RequestFailed` client errors. |
| GSL-ARC-015 | Medium | Confirmed | Architecture, maintainability | Large central files and 258 global config call sites make policy reasoning brittle. |
| GSL-ARC-016 | Low | Confirmed | Cross-contract, dependency | Desktop and text UI depend on different sources for `@repo-makeover/gosling-sdk`. |

## Detailed Findings

### GSL-SEC-001: Default `GoslingMode::Auto` automatically approves every tool call

Severity: High

Confidence: Confirmed

Evidence:

- `crates/gosling-providers/src/gosling_mode.rs:24-27` defines `GoslingMode::Auto` as `#[default]` with message `Automatically approve tool calls`.
- `crates/gosling/src/agents/agent.rs:317-322` builds the agent with `config.get_gosling_mode().unwrap_or_default()`.
- `crates/gosling/src/permission/permission_inspector.rs:151-154` maps `GoslingMode::Auto` to `InspectionAction::Allow`.
- `crates/gosling/src/permission/permission_inspector.rs:191-195` records the reason as `Auto mode - all tools approved`.
- `crates/gosling/src/agents/tool_execution.rs:134-135` dispatches the tool call when confirmation is `AllowOnce` or `AlwaysAllow`.
- `crates/gosling/src/agents/prompt_manager.rs:131-143` marks the prompt context as autonomous when the mode is Auto.

Impact:

A default install or session with no explicit mode setting runs with full automatic tool approval. In an AI agent that can receive untrusted repository, web, MCP, or app content, this makes prompt injection and malicious tool-call generation materially more dangerous.

Recommended fix:

Make the default mode approval-gated, preferably `SmartApprove` only if its allow decisions are scoped by tool provenance and arguments. Otherwise default to `Approve`. Preserve explicit Auto as an opt-in setting with visible operator signal.

Regression tests:

- Unit test that absent `gosling_mode` resolves to the new safe default.
- Permission inspector test that Auto remains opt-in and that unset config never yields unconditional tool approval.
- CLI and desktop startup tests that display the selected mode consistently.

### GSL-SEC-002: Prompt-injection scanning defaults off and shell detection is narrow

Severity: High

Confidence: Confirmed

Evidence:

- `crates/gosling/src/security/mod.rs:46-55` reads `SECURITY_PROMPT_ENABLED` and defaults to `false`.
- `crates/gosling/src/security/mod.rs:82-88` returns `Ok(vec![])` when prompt scanning is disabled.
- `crates/gosling/src/security/scanner.rs:373-390` extracts the `command` argument if present, otherwise scans a string containing the tool name and JSON arguments.
- `crates/gosling/src/security/scanner.rs:394-396` treats only the literal name `shell` as a shell tool.

Impact:

The default path performs no prompt-injection scan. Even when enabled, the shell-tool classifier is too narrow for renamed, namespaced, plugin, MCP, or app-exposed shell-like tools. This limits the scanner's usefulness as a control in the paths where untrusted tool descriptions and content matter most.

Recommended fix:

Enable a baseline prompt-injection scanner by default for tool requests that can affect local state, network, or secrets. Replace literal shell-name matching with capability metadata or a central registry of command-execution tools.

Regression tests:

- Scanner enabled-by-default test for an unset config.
- Namespaced shell-like tool test.
- Test that non-shell destructive tools still produce scanner input.

### GSL-SEC-003: Egress inspector detects destinations but always allows

Severity: Medium

Confidence: Confirmed

Evidence:

- `crates/gosling/src/security/egress_inspector.rs:333-337` only inspects shell and web tools.
- `crates/gosling/src/security/egress_inspector.rs:356-366` logs detected egress destinations.
- `crates/gosling/src/security/egress_inspector.rs:369-383` pushes an `InspectionResult` with `InspectionAction::Allow`, `confidence: 0.0`, and no finding ID.

Impact:

Egress detection is operator telemetry, not prevention. In Auto mode this produces no gate. In approval modes it does not require approval or deny, despite having detected outbound destinations.

Recommended fix:

Introduce policy-driven egress actions: allow known-safe destinations, require approval for unknown destinations, and deny configured blocked destinations. Include destination, direction, and tool provenance in the approval UI.

Regression tests:

- Unknown destination requires approval in approval-gated modes.
- Allowed destination remains allow.
- Denied destination returns deny and blocks dispatch.

### GSL-SEC-004: Local stdio MCP subprocesses inherit the parent environment

Severity: High

Confidence: Confirmed

Evidence:

- `crates/gosling/src/agents/extension_manager.rs:1101-1115` builds `all_envs` from configured extension env and session ID.
- `crates/gosling/src/agents/extension_manager.rs:1123-1131` passes explicit env to Docker with `docker exec -e`.
- `crates/gosling/src/agents/extension_manager.rs:1133-1136` launches non-container commands with `.envs(all_envs)` but does not call `env_clear()`.

Impact:

Rust child processes inherit the parent environment by default. A local stdio MCP server can receive ambient tokens, cloud credentials, shell state, proxy settings, and other process-level secrets even when not listed in the extension's configured `env` or `env_keys`.

Recommended fix:

Call `env_clear()` for non-container MCP subprocesses, then set a minimal allowlist: configured env, explicitly requested `env_keys`, `AGENT_SESSION_ID`, and required platform variables. If compatibility requires inheritance, make it explicit per extension.

Regression tests:

- Spawn a test stdio extension with an ambient parent env var and assert it is not visible.
- Assert explicitly configured env and `env_keys` still propagate.
- Platform tests for PATH/HOME behavior where required.

### GSL-SEC-005: Project plugins auto-enable and can execute hooks or MCP servers from the repository

Severity: High

Confidence: Confirmed

Evidence:

- `crates/gosling/src/plugins/discovery.rs:52-70` discovers project-scope plugins under the project root.
- `crates/gosling/src/plugins/discovery.rs:88-109` inserts newly discovered plugins into config with `enabled: true`.
- `crates/gosling/src/plugins/discovery.rs:122-147` returns `true` when no setting disables a plugin.
- `crates/gosling/src/hooks/mod.rs:236-258` loads enabled plugins and reads `hooks/hooks.json`.
- `crates/gosling/src/hooks/mod.rs:488-499` accepts command actions from hook config.
- `crates/gosling/src/hooks/mod.rs:546-566` spawns the configured hook command.
- `crates/gosling/src/hooks/mod.rs:579-603` executes hooks through `sh -c`.
- `crates/gosling/src/agents/agent.rs:1468-1474` emits `SessionStart` on the first turn.
- `crates/gosling-cli/src/session/builder.rs:433-439` extends new sessions with enabled plugin MCP servers from the current project root.

Impact:

An untrusted repository can carry project plugins that become enabled by default and can define shell hooks or MCP servers. Combined with Auto mode and inherited subprocess environment, this creates a high-impact path from opening or using a project to local command execution and secret exposure.

Recommended fix:

Do not auto-enable project plugins. Require explicit trust for project-scoped plugins, persist that trust with path and content hash, and show the hook/MCP server execution plan before activation. Disable shell hooks until the project has been trusted.

Regression tests:

- Project plugin present but untrusted does not load hooks or MCP servers.
- Trusted plugin loads only after explicit approval.
- Changing plugin content invalidates trust.

### GSL-SEC-006: OSV/malware guard is narrow and fails open

Severity: Medium

Confidence: Confirmed

Evidence:

- `crates/gosling/src/agents/extension_malware_check.rs:44-56` infers ecosystems only for commands ending in `uvx` or `npx`; unknown commands skip the check and return `Ok(())`.
- `crates/gosling/src/agents/extension_malware_check.rs:82-87` checks only the first non-flag package argument.
- `crates/gosling/src/agents/extension_malware_check.rs:211-231` returns `Ok(())` on OSV request, HTTP, and JSON parse errors after logging a fail-open message.

Impact:

The malware check is useful telemetry for a narrow class of package launchers, but it is not a dependable gate for project plugin or MCP execution. Network outages or alternate launchers bypass the check.

Recommended fix:

Make the policy explicit: either fail closed for untrusted project-scope plugin execution when OSV is unavailable, or label the check as advisory and require trust approval before execution. Expand package parsing for common invocation forms.

Regression tests:

- Unknown launcher in untrusted project scope requires approval.
- OSV outage blocks or requires approval according to configured policy.
- Multiple package forms are parsed and checked.

### GSL-CON-007: ACP app tool calls bypass normal permission inspection and `PreToolUse` hooks

Severity: High

Confidence: Confirmed

Evidence:

- `crates/gosling/src/acp/server/tools.rs:60-99` validates a visible tool and arguments, then directly calls `agent.extension_manager.dispatch_tool_call(...)`.
- Normal agent execution runs `inspect_tools` first at `crates/gosling/src/agents/agent.rs:2209-2217`.
- Normal agent execution then processes permission inspector decisions at `crates/gosling/src/agents/agent.rs:2219-2228`.
- Normal direct dispatch runs `PreToolUse` hooks before extension dispatch at `crates/gosling/src/agents/agent.rs:987-1004`.
- Extension dispatch happens only after that hook path at `crates/gosling/src/agents/agent.rs:1050-1057`.

Impact:

ACP app clients can call visible tools through a path that skips the approval and hook controls used by the main agent loop. Tool visibility is not equivalent to execution authorization, especially for tools with side effects or app-controlled arguments.

Recommended fix:

Route ACP app tool calls through the same tool inspection, permission, and `PreToolUse` pipeline as model-generated tool calls. If app clients need a different policy, define it explicitly and include it in the permission manager.

Regression tests:

- ACP app tool call to side-effecting visible tool requires approval in approval mode.
- `PreToolUse` deny hook blocks ACP app tool call.
- Read-only visible tool path still works when policy allows it.

### GSL-DAT-008: Corrupt config files are treated as empty during read-modify-write updates

Severity: Medium

Confidence: Confirmed

Evidence:

- `crates/gosling/src/config/base.rs:553-567` reads the writable config and returns an empty `Mapping` when YAML parsing fails, logging `Starting fresh`.
- `crates/gosling/src/config/base.rs:862-878` uses `load_write_config()` in `update_param`.
- `crates/gosling/src/config/base.rs:893-898` uses `load_write_config()` in `set_param`.
- `crates/gosling/src/config/base.rs:900-912` uses `load_write_config()` in `set_param_values`.
- `crates/gosling/src/config/base.rs:675-711` atomically writes the resulting values back to the config file.

Impact:

If the writable config becomes corrupt, the next successful setting update can durably replace the user's previous config with a new file containing only the latest updates. Atomic write protects against torn writes but does not protect against parse-error data loss.

Recommended fix:

Fail closed on parse errors for read-modify-write operations. Preserve the corrupt file, write a backup, and require repair or explicit reset before saving new config.

Regression tests:

- Corrupt config plus `set_param` returns an error and leaves the file unchanged.
- Explicit reset path creates a backup and writes fresh config.
- Valid config still updates atomically.

### GSL-NODE-009: Desktop preload exposes arbitrary filesystem IPC to renderer code

Severity: High

Confidence: Confirmed

Evidence:

- `ui/desktop/src/preload.ts:118-122` exposes `readFile`, `writeFile`, `deleteFile`, `ensureDirectory`, and `listFiles` on the Electron API.
- `ui/desktop/src/preload.ts:209-215` directly invokes the corresponding IPC handlers.
- `ui/desktop/src/main.ts:2203-2241` expands a renderer-supplied path and reads it.
- `ui/desktop/src/main.ts:2243-2253` writes a renderer-supplied path.
- `ui/desktop/src/main.ts:2255-2264` deletes a renderer-supplied path.
- `ui/desktop/src/main.ts:2267-2294` creates directories and lists files for renderer-supplied paths.
- The main windows do have `nodeIntegration: false` and `contextIsolation: true` at `ui/desktop/src/main.ts:1213-1218` and `ui/desktop/src/main.ts:1485-1488`; this reduces but does not remove the IPC blast radius.

Impact:

Any renderer compromise, XSS, malicious local UI content, or dependency compromise that reaches the preload API can read, write, delete, and list arbitrary user files under the app's privileges.

Recommended fix:

Replace generic path-based file IPC with capability-specific APIs. Require user-picked file handles or app-owned roots, validate paths against allowed directories, and remove delete/write primitives from general renderer access.

Regression tests:

- Renderer cannot read outside an approved app/session directory.
- Renderer cannot write or delete arbitrary home-directory files.
- User-selected import/export paths continue to work.

### GSL-NODE-010: Desktop CSP permits inline scripts and broad loopback while renderer IPC exposes the ACP secret

Severity: High

Confidence: Confirmed

Evidence:

- `ui/desktop/src/utils/csp.ts:3-16` allows `http`, `https`, `ws`, and `wss` loopback sources on any port.
- `ui/desktop/src/utils/csp.ts:65-69` sets `script-src 'self' 'unsafe-inline'`.
- `ui/desktop/src/main.ts:1894-1900` returns the ACP secret key to the renderer through IPC.
- `ui/desktop/src/preload.ts:281-282` exposes `getSecretKey` and `getAcpUrl`.

Impact:

The desktop app has good Electron defaults for node isolation, but the renderer is still a sensitive trust boundary. Inline script allowance, broad loopback connectivity, and renderer-visible ACP secrets increase the impact of any renderer injection.

Recommended fix:

Remove `unsafe-inline` from `script-src`, use nonces or hashes for any required inline code, narrow loopback connect-src to the app's own managed ACP endpoint, and avoid exposing raw ACP secrets to renderer code. Prefer main-process mediated requests.

Regression tests:

- CSP snapshot test rejects `unsafe-inline` for scripts.
- Renderer cannot read raw ACP secret.
- MCP app proxy still works through a main-mediated token or one-time capability.

### GSL-NODE-011: Electron permission handler approves every permission request

Severity: Medium

Confidence: Confirmed

Evidence:

- `ui/desktop/src/main.ts:2361-2370` installs a permission request handler that calls `callback(true)` for `media` and also calls `callback(true)` for all other permissions.

Impact:

Any renderer path that triggers permissions receives approval, including permissions beyond microphone/media. This is too broad for a desktop app that renders local and remote-adjacent content.

Recommended fix:

Allow only required permissions and only for expected origins or webContents. Deny by default. Add operator-visible prompts for sensitive permissions if needed.

Regression tests:

- Non-media permissions are denied.
- Media permission is allowed only for the intended webContents/origin.

### GSL-NODE-012: External URL opening uses a denylist despite a safer allowlist existing

Severity: Medium

Confidence: Confirmed

Evidence:

- `ui/desktop/src/utils/urlSecurity.ts:7-16` defines blocked protocols.
- `ui/desktop/src/utils/urlSecurity.ts:18-65` defines safe protocols.
- `ui/desktop/src/main.ts:1325-1336` opens any URL whose protocol is not blocked.
- `ui/desktop/src/main.ts:1342-1352` repeats the denylist behavior for a `new-window` event path.
- `ui/desktop/src/main.ts:1789-1797` exposes `open-external` IPC with the same denylist behavior.

Impact:

New or uncommon URL schemes are opened by default unless they appear in the blocklist. This is a classic denylist maintenance problem and can hand off control to local protocol handlers.

Recommended fix:

Use the existing `SAFE_PROTOCOLS` allowlist for all `shell.openExternal` paths. For non-allowlisted schemes, deny or require explicit user confirmation.

Regression tests:

- Unknown custom protocol does not open automatically.
- Allowed protocols still open.
- The window handler, `new-window` fallback, and IPC path share one validation function.

### GSL-CON-013: MCP app proxy uses query-string auth and core/server guest auth differs

Severity: Medium

Confidence: Confirmed

Evidence:

- `ui/desktop/src/components/McpApps/McpAppRenderer.tsx:183-201` fetches ACP URL and secret key, then appends `secret` to `/mcp-app-proxy`.
- `crates/gosling/src/acp/mcp_app_proxy.rs:30-38` models proxy auth as a query parameter.
- `crates/gosling/src/acp/mcp_app_proxy.rs:223-230` checks the query-string secret for `/mcp-app-proxy`.
- `crates/gosling/src/acp/mcp_app_proxy.rs:40-43` defines `GuestQuery` with only `nonce`.
- `crates/gosling/src/acp/mcp_app_proxy.rs:316-345` serves guest HTML using only the nonce.
- `crates/gosling/src/acp/mcp_app_proxy.rs:349-363` wires the guest route without secret state.
- `crates/gosling-server/src/routes/mcp_app_proxy.rs:157-163`, `:199-205`, and `:255-261` require secret checks for proxy, store, and guest routes in the server implementation.

Impact:

Secrets in URLs can leak through logs, browser history, referrers, screenshots, and diagnostics. The core ACP implementation and server route also disagree on guest-route authentication, which creates a cross-implementation contract risk.

Recommended fix:

Move proxy authentication out of query strings into headers or short-lived one-time capabilities. Make core ACP and `gosling-server` enforce the same guest route contract. Avoid giving renderer code the raw long-lived secret.

Regression tests:

- Proxy URL does not include the secret.
- Guest route requires the same auth contract in both implementations.
- Old query-string auth is rejected or covered by an explicit compatibility window.

### GSL-REL-014: Provider retry defaults retry generic client failures

Severity: Medium

Confidence: Confirmed

Evidence:

- `crates/gosling-providers/src/retry.rs:28-36` defaults `transient_only` to `false`.
- `crates/gosling-providers/src/retry.rs:99-106` retries `ProviderError::RequestFailed(_)` when `transient_only` is false unless a permanent marker is recognized.
- `crates/gosling-providers/src/retry.rs:267-270` tests that default config retries `RequestFailed("Bad request (400): model not found")`.
- `crates/gosling-providers/src/base.rs:600-602` returns `RetryConfig::default()` for providers that do not override it.
- `crates/gosling-providers/src/ollama.rs:393-400` shows one provider opting into `.transient_only()`, proving the safer behavior is available but not default.

Impact:

Permanent 4xx failures can be retried by default, wasting latency and tokens and hiding configuration problems behind retry delays. This is reliability and operator-signal debt rather than a direct security flaw.

Recommended fix:

Make transient-only retry the default. Providers that genuinely need retry on some `RequestFailed` cases should classify those cases explicitly.

Regression tests:

- Default retry config does not retry generic 4xx request failures.
- Rate limit, server, and network errors still retry.
- Provider-specific overrides are covered.

### GSL-ARC-015: Large central files and global config use make policy reasoning brittle

Severity: Medium

Confidence: Confirmed

Evidence:

- `crates/gosling/src/agents/agent.rs` is 4106 lines.
- `crates/gosling/src/config/base.rs` is 2811 lines.
- `ui/desktop/src/main.ts` is 2981 lines.
- `ui/text/src/tui.tsx` is 1424 lines.
- `rg "Config::global()" crates/gosling crates/gosling-cli crates/gosling-server` finds 258 call sites.

Impact:

Security policy, lifecycle behavior, UI integration, config resolution, and tool execution are spread across large central files and global access patterns. This increases the chance that new call paths bypass controls, as seen in the ACP app tool dispatch finding.

Recommended fix:

Prioritize boundary-specific extraction around permission/tool dispatch, plugin trust, desktop file capabilities, and config writes. Replace new `Config::global()` usage in security-sensitive paths with explicit injected dependencies.

Regression tests:

- Architectural tests or lint checks for new direct extension-manager dispatch paths.
- Unit tests for each extracted policy boundary.

### GSL-ARC-016: Desktop and text UI use different SDK dependency sources

Severity: Low

Confidence: Confirmed

Evidence:

- `ui/desktop/package.json:52` uses `@repo-makeover/gosling-sdk` as `workspace:*`.
- `ui/text/package.json:30` pins `@repo-makeover/gosling-sdk` to `0.20.2`.

Impact:

Desktop and text UI may compile against different ACP SDK contract assumptions. This is a cross-package drift risk, especially while ACP and app-tool surfaces are actively changing.

Recommended fix:

Make SDK sourcing intentional and documented. Prefer one workspace SDK source for in-repo packages, or add compatibility tests proving that the pinned text UI SDK remains valid.

Regression tests:

- Package-level contract test that both UIs agree on the ACP types they use.
- Dependency policy check for unexpected SDK source drift.

## Non-Findings And Fixed Items

These older or plausible issues were checked and are not carried forward as current findings:

- Inspector failure now fails closed. `crates/gosling/src/tool_inspection.rs:95-136` synthesizes approval requirements when an inspector errors.
- Secret file writes are now atomic. `crates/gosling/src/config/base.rs:42-63` writes secrets via temp file, `sync_all`, and rename.
- Config writes use atomic replacement on normal valid input. `crates/gosling/src/config/base.rs:675-711` writes temp and renames. The remaining finding is specifically the corrupt-parse read-modify-write behavior.
- Plugin `git clone` option injection appears fixed. `crates/gosling/src/plugins/mod.rs:292-301` uses `git clone --depth 1 -- source destination`.
- Legacy desktop `externalGoosed` settings migration exists. `ui/desktop/src/utils/settings.ts:120-135` maps `externalGoosed` into `externalGoslingd`, and `ui/desktop/src/utils/settings.test.ts:6-47` covers migration and precedence.
- `gosling://` is the documented session-share scheme. `README.md:91` names `gosling://`, `documentation/INDEX.md:36` says legacy `goose://` share-link compatibility is not part of the current docs contract, and `crates/gosling/src/session/nostr_share.rs:375-390` rejects legacy `goose://`.
- CLI non-interactive approval no longer appears to silently allow approval-mode tool calls. The current code path cancels in non-interactive approval modes and only allows Auto by explicit warning path.
- Google retry-delay clamping and Bedrock/GCP metadata timeouts were spot-checked as fixed from prior reports.
- The desktop main windows use `nodeIntegration: false` and `contextIsolation: true` at `ui/desktop/src/main.ts:1213-1218` and `ui/desktop/src/main.ts:1485-1488`.
- `ui/desktop/src/api` does not exist in this checkout, so I found no current violation of the repo rule forbidding generated OpenAPI desktop client imports.

## Break-It Review

The highest-value adversarial paths are:

- Malicious repository path: place a project plugin under `.agents/plugins`, rely on default-enabled discovery, execute a `SessionStart` hook through `sh -c`, or start a plugin MCP server.
- Prompt-injection path: get untrusted content into the model context, rely on Auto mode and disabled prompt scanning, then request a side-effecting tool call.
- App-client path: call visible ACP app tools directly and bypass normal permission inspection and `PreToolUse` hooks.
- Renderer compromise path: execute renderer JavaScript, call preload filesystem APIs and ACP secret IPC, then read/write/delete files or reach broad loopback services.
- Secret exposure path: start a local stdio MCP server and read inherited parent environment variables.
- Recovery path: corrupt writable YAML config, then trigger a normal settings update that writes a fresh mapping over the previous config.

## Skill Escalation

| Finding | Escalated lenses |
| --- | --- |
| GSL-SEC-001 | `audit-security`, `audit-security-llm`, `audit-workflow-gui`, `audit-dataflow-state-transition`, `audit-failsafe-readiness` |
| GSL-SEC-004, GSL-SEC-005, GSL-SEC-006 | `audit-security-code`, `audit-resource-lifecycle`, `audit-dependency-criticality`, `audit-recovery-idempotency`, `audit-pipeline-externalapi` |
| GSL-CON-007, GSL-CON-013 | `audit-contract-internalapi`, `audit-contract-crossrepo`, `audit-dataflow-input-output`, `audit-invariant-sync` |
| GSL-NODE-009 through GSL-NODE-012 | `audit-security-nodejs`, `audit-architecture-nodejs`, `audit-design-webapp`, `audit-workflow-gui` |
| GSL-REL-014 | `audit-reliability`, `audit-performance-profile`, `audit-operator-signal` |
| GSL-ARC-015, GSL-ARC-016 | `audit-architecture-seam`, `audit-deadcode-cleanup`, `audit-negative-space`, `audit-contract-crossrepo` |

## Recommended Patch Order

1. Change the default tool mode away from Auto and make Auto visibly opt-in.
2. Route ACP app tool calls through the same permission and hook pipeline as normal tool execution.
3. Disable default project-plugin trust; require explicit trust before project hooks or plugin MCP servers run.
4. Add `env_clear()` and an explicit allowlist for local stdio MCP subprocess environments.
5. Replace desktop generic filesystem IPC with capability-scoped file APIs.
6. Remove renderer access to raw ACP secrets and replace query-string MCP app proxy auth.
7. Tighten desktop CSP and external URL opening.
8. Make egress policy enforceable and enable a baseline prompt-injection scan for side-effecting tools.
9. Make corrupt config read-modify-write fail closed with backup/repair flow.
10. Flip provider retry defaults to transient-only.
11. Extract policy boundaries from large central files and add guard tests for new direct dispatch paths.
12. Align UI SDK dependency policy.

## Regression Test Plan

| Area | Tests to add |
| --- | --- |
| Permission defaults | Unset config resolves to safe approval mode; Auto requires explicit config; approval UI receives risky tool call. |
| ACP app tools | App-visible side-effecting tool requires approval; `PreToolUse` deny hook blocks app tool call; read-only allowed path remains functional. |
| Project plugin trust | Untrusted project plugin hooks/MCP servers do not load; explicit trust enables; content change invalidates trust. |
| MCP environment | Ambient parent env var is absent in local stdio MCP server; configured env and env_keys still propagate. |
| Security inspectors | Prompt scanning enabled for side-effecting tool requests; namespaced shell-like tools are classified; egress policy can require approval or deny. |
| Desktop IPC | Renderer cannot read/write/delete outside approved roots; secret key is unavailable to renderer; unknown URL schemes are denied. |
| MCP app proxy | No secret in proxy URL; guest auth contract matches core and server implementations. |
| Config durability | Corrupt config update fails without overwrite; explicit reset backs up corrupt file. |
| Retry | Generic 4xx request failures do not retry by default; rate limits/server/network errors still retry. |
| Architecture guardrails | Static or unit guard preventing new direct `extension_manager.dispatch_tool_call` paths without permission wrapper. |

## Deferred Risks

- Runtime exploitability was not demonstrated. Several findings are source-confirmed control weaknesses, but exploitability varies by user settings, enabled plugins, renderer content, and session topology.
- The audit did not launch the Electron desktop, run ACP app clients, run MCP servers, or exercise real provider calls.
- Dependency vulnerability scanning was limited to source-level dependency-criticality review. No `cargo audit`, `npm audit`, OSV scan, or SBOM generation was run.
- The full local audit skill library was used as a rubric, but this report does not include every checklist item from every skill. It focuses on current, source-supported findings.
- Cross-repository compatibility was not checked against upstream Goose or external consumers.

## Final Confidence

Confidence is high for the confirmed source-level findings in this report. Confidence is medium for severity ranking because no runtime validation or exploit harness was executed. The highest-risk areas should be treated as actionable before adding more plugin, app, or desktop capabilities.
