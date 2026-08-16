# Combined Security Audit — gosling

- **Date:** 2026-08-15
- **Target:** `/Users/eric/Work/vscode/forked/gosling`
- **Branch:** `main` (clean)
- **HEAD:** `073d19428509ea6eb317924b1856a1fe7e9002c8`
- **Authority:** read_only / repo-only static review
- **Lenses:** `audit-security` (SEC-001..015), `audit-security-code` (CSEC-001..074), `audit-security-owasp` (OWASP Top 10 2021 + API Top 10 2023 + ASVS L2 source pass)
- **Scan mode:** repo-only
- **ASVS target level:** L2 (source-verified) for chapters actually walked; not L3

The supplied prompt is treated as a draft. The intended mission is preserved: a combined classic-appsec + CSEC + OWASP scan of gosling at this HEAD. Review was expanded to adjacent seams implied by the high-value surfaces (permission/default posture, MCP host/server spawn, session import, Electron main/preload, serve/ACP auth, MCP-app guest CSP, Nostr relay fetch) rather than only ranked inventory labels.

## Executive Verdict

This is a **local-first agent control plane** with a real HTTP/WebSocket attack surface (`gosling serve`, `goslingd`) and an Electron desktop that launches that plane on loopback. There is **no Critical confirmed unauthenticated remote RCE** in the default bind (`127.0.0.1`) posture. There **are High confirmed defects** that turn a stolen or leaked loopback secret into full agent/control-plane authority, and that weaken the MCP-app sandbox on the live `gosling serve` path.

Highest-risk confirmed themes:

1. The ACP bearer is accepted in the URL query string and is constructed that way by desktop (`?token=`).
2. The live ACP MCP-app guest path trusts a client-supplied CSP string; the sibling `goslingd` path does not.
3. Auto mode (used unconditionally for subagents) approves every tool that lacks an explicit user `NeverAllow`/`AskBefore`.
4. Working-directory filesystem containment is opt-in for ordinary sessions.
5. Nostr session import connects to relay URLs taken from the attacker-influenced deeplink with no destination allowlist.

Default desktop/`gosling serve` bind is loopback. Severity below is scored from the mechanism; where posture changes the blast radius, both bands are stated. Patching is recommended now for SEC-GOS-001, SEC-GOS-002, and the Auto/working-dir default-posture pair. Do not treat prior `docs/cloud/` reports as current; every finding below was re-read at this HEAD.

## Scope

- **Repository / branch / commit:** gosling `main` `073d19428509ea6eb317924b1856a1fe7e9002c8`
- **Prompt / orientation reviewed:** `docs/cloud/2026-08-15-orientation.md`; `AGENTS.md`; assigned skill files and `000_common/audit-base/*`
- **Skills invoked:** `audit-security`, `audit-security-code`, `audit-security-owasp`
- **Files/directories inspected (deep):** `crates/gosling/src/permission/`, `crates/gosling/src/security/`, `crates/gosling/src/acp/transport/`, `crates/gosling/src/acp/mcp_app_proxy.rs`, `crates/gosling/src/acp/server/manage_sessions.rs`, `crates/gosling/src/acp/server/extensions.rs`, `crates/gosling/src/session/import_formats/`, `crates/gosling/src/session/nostr_share.rs`, `crates/gosling/src/session/session_manager.rs` (import + untrusted replay), `crates/gosling/src/agents/extension_manager.rs` (spawn/env), `crates/gosling/src/agents/mcp_client.rs`, `crates/gosling/src/agents/extension_malware_check.rs`, `crates/gosling/src/agents/platform_extensions/developer/shell.rs`, `crates/gosling/src/subprocess.rs`, `crates/gosling/src/oauth/`, `crates/gosling/src/workspace/credentials.rs`, `crates/gosling/src/config/permission.rs`, `crates/gosling/src/plugins/discovery.rs`, `crates/gosling-server/src/auth.rs`, `crates/gosling-server/src/commands/agent.rs`, `crates/gosling-server/src/configuration.rs`, `crates/gosling-server/src/routes/{mod,session,status,config_management,mcp_app_proxy}.rs`, `crates/gosling-cli/src/cli.rs` (serve), `crates/gosling-mcp/src/computercontroller/mod.rs` (SSRF helper), `ui/desktop/src/{main.ts,preload.ts,goslingServe.ts,utils/rendererFileAccess.ts,utils/rendererDirectoryGrants.ts,utils/urlSecurity.ts,handoffProtocol.ts,App.tsx,shellHost.ts}`
- **Commands/tests run:** none (read_only; no compile, no live probe, no credential test)
- **Effort budget:** ~140 targeted reads/greps across the named high-value surfaces. Bought: full SEC-001..015, CSEC-001..074, and OWASP A01–A10 + API1–API10 coverage as finding or explicit non-finding. Not bought: full provider-adapter body review, full Ink TUI, Docusaurus, or every Electron renderer component.
- **Constraints:** no source/test/doc edits except this report; no live exploitation; LLM-in-the-trust-path owned by `audit-security-llm` except where it is also a classic privilege boundary.

## Draft Prompt Assessment

- **Intended mission:** one combined security report at HEAD, inventory-complete, source-quoted.
- **Under-specified:** multi-user vs local-first scoring; whether `goslingd` is still a live desktop path.
- **Overly narrow if followed literally:** only `permission/` and `security/` modules.
- **Added angles:** sibling-implementation (ACP vs goslingd MCP-app proxy), producer/consumer (desktop URL builder vs ACP auth), deployment-posture split, import-untrusted replay, Electron sandbox sibling (`shellHost` vs main window).
- **Assumptions challenged:** “loopback is a security boundary”; “goslingd and `gosling serve` share the same guest-CSP control”; “Auto mode is only a UX default”.

## Surface Inventory

| Surface | Actor | Input/Trigger | State/Output | Boundary | Reviewed |
|---|---|---|---|---|---|
| `gosling serve` ACP HTTP/WS `/acp` | Local renderer, CLI ACP client, any TCP client that can reach bind | JSON-RPC / WS upgrade; `X-Secret-Key` or `?token=` | Session create/load, tools, extensions, import | Token (optional), CORS/origin, loopback default | Yes |
| `gosling serve` `/health`,`/status` | Anyone who can hit bind | GET | `"ok"` | None (intentional liveness) | Yes |
| `gosling serve` `/mcp-app-proxy` GET | Loopback client | Query CSP domain lists | Sandbox HTML | ConnectInfo loopback only | Yes |
| `gosling serve` `/mcp-app-guest` POST | Loopback client with secret | JSON `{secret,html,csp}` | Nonce + guest URL | Secret compare (non-ct) + loopback | Yes |
| `gosling serve` guest HTTP on ephemeral `127.0.0.1:0` | Anyone on loopback with nonce | GET `?nonce=` | Attacker/MCP HTML + client CSP | UUID nonce, one-time | Yes |
| `goslingd` REST (`/sessions`, `/config/*`, `/diagnostics/*`, `/reply`, …) | Desktop (legacy) / any client with secret | JSON | Sessions, secrets (masked), config, diagnostics | `X-Secret-Key`; exemptions for `/status`,`/mcp-app-proxy`,`/mcp-app-guest` | Yes |
| `goslingd` bind/TLS | Operator env `GOSLING_HOST`/`GOSLING_PORT`/`GOSLING_TLS` | Process start | Listening socket | Default `127.0.0.1:3000` + TLS default true | Yes |
| Permission inspector / judge / store | Agent loop | Tool requests | Allow / ask / deny | `permission.yaml`, mode, LLM judge | Yes |
| Working-dir scope inspector | Agent loop | Tool path/command args | RequireApproval / Deny | Opt-in flag or workspace policy | Yes |
| Security scanner / adversary / egress inspectors | Agent loop | Tool JSON + messages | Review/block | Pattern/ML; fail-open if disabled | Sampled |
| MCP / extension stdio spawn | User config, ACP add-extension, plugins | cmd/args/env/cwd | Child process | `env_clear` + allowlist env; OSV for npx/uvx only | Yes |
| Developer shell tool | Model tool call (after permission) | Command string | Shell `-c` / `cmd /C` | Permission + optional path inspector | Yes |
| Session import (file/JSON/Nostr) | User / ACP / deeplink | JSON / jsonl / nostr URI | New session | Size cap, cwd canonicalize, untrusted flags | Yes |
| Electron main/preload IPC | Renderer | Paths, URLs, settings | FS read/write, ACP URL, proxy URL | Grants + `assertPathWithinRoots`; `openExternal` protocol filter | Yes |
| OAuth callback `127.0.0.1:<port>/oauth_callback` | Browser redirect | `code`,`state` | Token persist (keyring) | Loopback bind; CSRF via `handle_callback` | Yes |
| Workspace credentials | Authenticated ACP/REST | Profile + secret fields | Keyring keys | Name uniqueness; source lock | Sampled |
| Computer-controller HTTP fetch | MCP tool user | URL | Outbound GET | `ensure_public_http_url` + resolved-IP deny | Yes |
| Nostr share/import | User / deeplink | Relay URLs in nevent | Outbound WS; imported session | Relay parse only; no IP allowlist | Yes |
| SQLite session store | Local process | Session IDs, FTS query | Rows | Bound parameters; single-user DB | Sampled |
| Plugin discovery | Disk + `config.yaml` | Plugin dirs | Enabled plugin list | Local FS only | Yes |

## Boundary Map

| Surface | Intended Boundary | Enforced At | Status |
|---|---|---|---|
| ACP HTTP/WS | Shared secret + loopback/CORS | `check_acp_token`; `AcpOriginPolicy`; default host `127.0.0.1` | **Partial** — token also in query; Origin optional on WS; `--dangerously-unauthenticated` skips token; host can be `0.0.0.0` |
| goslingd REST | Shared secret | `check_token` | **Partial** — three path exemptions; one secret = all sessions |
| MCP-app sandbox | Server-derived CSP + secret + loopback | goslingd: server-built CSP; ACP serve: **client CSP** | **Split** — sibling implementations disagree |
| Tool execution | User permission + mode + inspectors | `PermissionInspector`, `WorkingDirScopeInspector`, security scanner | **Partial** — Auto allow-all; working-dir opt-in; LLM judge in SmartApprove |
| Renderer FS | User-selected grant roots | `RendererDirectoryGrantRegistry` + realpath containment | **Held** |
| External URL open | http/https (chrome) or SAFE_PROTOCOLS | `normalizeWebUrl` / `isProtocolSafe` | **Held** |
| MCP spawn env | Minimal env, no parent secret inheritance | `env_clear` + `minimal_child_environment` | **Held** |
| Session import | Untrusted history; operator-chosen cwd | `history_trusted: false`; `with_imported_untrusted`; cwd canonicalize | **Held** |
| Registry package spawn | OSV MAL-* deny | `deny_if_malicious_cmd_args` | **Partial** — fail-open for local binaries |
| Computer-controller fetch | Public HTTP(S) only | Resolve + deny private/metadata | **Held** |
| Nostr relay connect | Public wss relays | `RelayUrl::parse` only | **Missing** destination allowlist |
| Secrets at rest | OS keyring / 0o600 files | `Config::set_secret`; several 0o600 writers | **Held** (sampled) |
| SQL | Bound parameters | sqlx `.bind` | **Held** (sampled) |

### Trust-boundary inventory (SEC taxonomy)

| Surface | Actor | Authority | Object/Data Reached | Boundary | Enforcement Location | Bypass Path |
|---|---|---|---|---|---|---|
| `/acp` | Any client with token or unauth flag | Full ACP (sessions, tools, add-extension) | All local sessions + spawn | Token / flag | `create_acp_router_with_policy` | Query-string leak; `--dangerously-unauthenticated`; missing Origin |
| goslingd `/config/upsert` | Secret holder | Write any config/secret | Keyring + `config.yaml` | Token | `check_token` | Stolen `X-Secret-Key` |
| goslingd `/sessions/{id}` | Secret holder | Read any session by id | Conversation + metadata | Token, not owner | `get_session` | Single-secret model = no object scope |
| `/mcp-app-guest` GET | Anyone with nonce | Read stored HTML | Guest HTML | Nonce | In-memory map | Unauthenticated by design |
| Tool shell | Model after permission | Host shell | FS/network of user | Mode + inspectors | `permission_inspector` | Auto allow; path inspector off |
| Nostr import | Deeplink + user dir pick | Outbound WS to named relays | Internal/metadata if relay is private | None | `LiveNostrClient::fetch` | Crafted `nevent` relays |
| Electron `write-file` | Renderer | Write under grants | Granted trees | Grant + realpath | `assertRendererFileAccess` | XSS in unsandboxed renderer + existing grant |

## Findings Table

| ID | Severity | Confidence | Evidence Basis | Domain | Title | Patch Priority | Blast Radius | Complexity | Cost | Nominal Agent |
|---|---|---|---|---|---|---|---|---|---|---|
| SEC-GOS-001 | High | Confirmed | source-evidenced | Security | ACP secret accepted and constructed as `?token=` | 1 | Service | local_guardrail | S | codex |
| SEC-GOS-002 | High | Confirmed | source-evidenced | Security | Live ACP MCP-app guest trusts client CSP | 2 | Workflow | local_guardrail | S | codex |
| SEC-GOS-003 | High | Confirmed | source-evidenced | Security | Auto mode allows all tools without a user permission | 3 | Service | workflow_protocol | M | claude |
| SEC-GOS-004 | Medium (High with Auto) | Confirmed | source-evidenced | Security | Working-dir filesystem boundary is opt-in | 4 | Workflow | workflow_protocol | M | claude |
| SEC-GOS-005 | Medium (High if reachable beyond loopback intent) | Confirmed | source-evidenced | Security | Nostr import fetches attacker-supplied relay URLs | 5 | Cross-system | local_guardrail | S | codex |
| SEC-GOS-006 | Medium | Confirmed | source-evidenced | Security | Desktop main/launcher windows omit `sandbox: true` | 6 | Local | local_guardrail | S | codex |
| SEC-GOS-007 | Medium (High if `GOSLING_HOST` is public) | Confirmed | source-evidenced | Security | goslingd leaves MCP-app routes unauthenticated | 7 | Service | local_guardrail | S | codex |
| SEC-GOS-008 | Low | Confirmed | source-evidenced | Security | ACP guest store uses `!=` on the secret | 8 | Local | local_guardrail | XS | codex |
| SEC-GOS-009 | Medium | Confirmed | source-evidenced | Security | Prompt-injection scanner logs full tool-call JSON | 9 | Repo | local_guardrail | S | codex |
| SEC-GOS-010 | Medium | Confirmed | source-evidenced | Security | `/config/read` mask reveals first 8 secret characters | 10 | Service | local_guardrail | XS | codex |
| SEC-GOS-011 | Medium | Confirmed | source-evidenced | Security | WebSocket origin check skipped when `Origin` is absent | 11 | Service | local_guardrail | S | codex |
| SEC-GOS-012 | High if reachable | Confirmed | source-evidenced | Security | Unauthenticated serve does not force loopback bind | 12 | Service | local_guardrail | S | codex |
| SEC-GOS-013 | High | Confirmed | source-evidenced | Security | SmartApprove treats LLM “read-only” as Allow | 13 | Workflow | workflow_protocol | M | multi-agent |

## Detailed Findings

### SEC-GOS-001: ACP secret accepted and constructed as `?token=`

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A02 · API: API2 · ASVS: V2/V3/V8 · CWE: CWE-598 · CSEC: CSEC-044 · SEC: SEC-015 / SEC-007

Evidence:
- `crates/gosling/src/acp/transport/auth.rs:25-31` — query parser:
  ```
  let query_token = request.uri().query().and_then(|query| {
      url::form_urlencoded::parse(query.as_bytes())
          .find(|(key, _)| key == "token")
          .map(|(_, value)| value.into_owned())
  });
  ```
- `crates/gosling/src/acp/transport/auth.rs:31` — `token_matches(header_token, &state) || token_matches(query_token.as_deref(), &state)`
- `ui/desktop/src/goslingServe.ts:334-336` — desktop builds the live URL:
  ```
  const acpUrl = new URL(`${httpBaseUrl}/acp`);
  acpUrl.protocol = websocketProtocol;
  acpUrl.searchParams.set('token', token);
  ```
- `ui/desktop/src/main.ts:2091-2097` — renderer can fetch that URL via `get-acp-url`.

Observed behavior:
- The same high-privilege ACP secret is a header **or** a query parameter. The desktop always places it in the query string of the WebSocket URL handed to the renderer.

Expected boundary:
- Secrets used as session/control-plane authenticators must not travel in URLs (logs, Referer, crash reports, `ps`, reverse-proxy access logs, browser history). Header-only (`X-Secret-Key`) or WS subprotocol/first-message auth.

Failure mechanism:
- CSEC-044 / OWASP A02 “secret in URL”. Any component that records request URIs (Electron logs, `goslingServe` startup traces if redaction regresses, corporate proxies, `window.location` if ever used as HTTP, crash dumps) captures the bearer that authorizes every ACP method.

Break-it angle:
- Copy `ws://127.0.0.1:<port>/acp?token=...` from a log or renderer DevTools; replay without `X-Secret-Key`. Auth succeeds today by construction.

Impact:
- Token disclosure is full local-agent takeover: session read/import, extension add/spawn, config, tool execution.
- Blast radius: Service
- Side-effect class: network
- Reversibility: compensatable (rotate secret / restart serve)
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- SEC-GOS-007 (unauthenticated sibling routes), SEC-GOS-011 (Origin optional), SEC-GOS-012 (unauth serve).

Recommended mitigation:
- Remediation patterns: header-only authenticator; URL redaction; secret-not-in-query.
- Minimal repair: stop accepting `?token=`; pass the secret only as `X-Secret-Key` (WS handshake header is already CORS-allowed).
- Local guardrail: reject requests whose URI query contains `token=`; assert desktop `buildLocalServeUrls` no longer appends it.
- Behavior test: request `/acp?token=<valid>` with no header returns 401; header-only still works.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: two call sites (auth middleware + URL builder) plus existing `acp_transport_auth_test` / `goslingServe.test.ts`.

Validation:
- Negative: query-only token is 401.
- Positive: `X-Secret-Key` still authenticates HTTP and WS.
- Redaction: any remaining log of `acpUrl` stays `token=REDACTED` (`goslingServe.ts:338-340` already redacts the diagnostic copy).

Non-goals:
- Do not redesign ACP into per-user OAuth in this slice.
- Do not change goslingd `X-Secret-Key` header path.

### SEC-GOS-002: Live ACP MCP-app guest trusts client-supplied CSP

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A08 · API: API8 · ASVS: V5/V14 · CWE: CWE-829 / CWE-1021 · CSEC: CSEC-073 · SEC: SEC-005
Bypass row: “sandbox policy taken from the guest/client instead of derived from an allowlist.”

Evidence:
- `crates/gosling/src/acp/mcp_app_proxy.rs:46-50` — store body includes `csp: Option<String>`.
- `crates/gosling/src/acp/mcp_app_proxy.rs:272-284`:
  ```
  if body.secret != state.secret_key { ... }
  ...
  let csp = body.csp.unwrap_or_default();
  ```
- `crates/gosling/src/acp/mcp_app_proxy.rs:338-341` — that string is installed as `Content-Security-Policy` on the guest response.
- Sibling that **held**: `crates/gosling-server/src/routes/mcp_app_proxy.rs:282-295` derives CSP from domain lists and documents “rather than trusting a pre-built CSP string from the client.”
- Desktop launches `gosling serve` (`ui/desktop/src/goslingServe.ts:447-448`), which mounts the ACP proxy (`crates/gosling/src/acp/transport/mod.rs:239-242`), not the goslingd sibling.

Observed behavior:
- A caller who knows the serve secret (the renderer does — `get-mcp-app-proxy-url` puts it in the URL hash) can store arbitrary HTML and an arbitrary CSP. An empty CSP means the guest HTML runs without the outer policy ceiling.

Expected boundary:
- The host is the policy authority. Guest CSP must be derived from parsed, allowlisted domain lists (as goslingd already does).

Failure mechanism:
- Sibling-implementation drift. The live desktop path is the weaker one. MCP-app HTML is untrusted by design; client-chosen CSP is a sandbox escape.

Break-it angle:
- POST `/mcp-app-guest` with valid secret, hostile HTML, `csp: ""` or a policy that includes `script-src *`. Load the returned `guest_url`. The response carries the attacker CSP.

Impact:
- MCP-app sandbox escape on loopback: fetch to unexpected origins, script load, or framing beyond the declared domain lists.
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- SEC-GOS-008 (non-ct compare on the same handler), SEC-GOS-007 (goslingd GET is unauthenticated).

Recommended mitigation:
- Remediation patterns: server-derived policy; delete client CSP field.
- Minimal repair: copy goslingd `build_outer_csp` + domain-list body into the ACP store handler; ignore `body.csp`.
- Local guardrail: reject non-empty `csp` field or overwrite it.
- Behavior test: store with `csp: "script-src *"`; served header must not contain `script-src *` and must match the derived policy.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: the safe implementation already exists 1:1 in `gosling-server` routes.

Validation:
- ACP store with malicious CSP serves the derived policy only.
- Domain-list injection tokens (`*`, `javascript:`, `;`) remain rejected (`normalize_csp_source`).

Non-goals:
- Do not change MCP Apps UX or the separate guest port.

### SEC-GOS-003: Auto mode allows all tools without a user permission

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A01 / A04 · API: API5 / API6 · ASVS: V4/V11 · CWE: CWE-269 / CWE-840 · CSEC: CSEC-002 / CSEC-073 · SEC: SEC-011 / SEC-004

Evidence:
- `crates/gosling/src/permission/permission_inspector.rs:162-166`:
  ```
  } else if gosling_mode == GoslingMode::Auto {
      (
          InspectionAction::Allow,
          "Auto mode - all tools approved".to_string(),
      )
  ```
- Same file `133-139` documents that Auto is used unconditionally for subagents and cannot prompt.
- User `NeverAllow` / `AlwaysAllow` still apply first (`141-161`). `AskBefore` in Auto becomes Deny. The gap is the **absence** of a user row.

Observed behavior:
- Any tool the user has not listed in `permission.yaml` is auto-executed in Auto, including shell and extension-management unless other inspectors fire.

Expected boundary:
- Default-deny for mutating/open-world tools. Auto may skip the prompt but must not skip a conservative policy (deny or a static read-only allowlist).

Failure mechanism:
- Overbroad function-level authorization keyed on mode rather than tool class. Combined with SEC-GOS-004 (no FS scope) this is host-equivalent agency.

Break-it angle:
- Session/subagent in Auto; model emits `developer__shell` with `rm -rf ~` or `curl | sh`. No user prompt. Working-dir inspector does not run unless the session opted in.

Impact:
- Unattended destructive or exfiltrating tool execution.
- Blast radius: Service
- Side-effect class: process
- Reversibility: irreversible (depends on the tool)
- Operator visibility: UI-visible after the fact
- Rerun safety: unsafe

Adjacent failure modes:
- SEC-GOS-004, SEC-GOS-013 (LLM judge is a different mode with the same trust inversion), LLM-lens prompt injection.

Recommended mitigation:
- Remediation patterns: default-deny mutating tools; static read-only allowlist for Auto.
- Minimal repair: in Auto, Allow only tools with user `AlwaysAllow` or a built-in read-only allowlist; Deny shell/write/extension-management otherwise.
- Local guardrail: never take the `InspectionAction::Allow` “all tools approved” branch for names matching `shell|command|write|edit|delete`.
- Behavior test: Auto session, no user permission row, `developer__shell` is denied; `developer__text_editor__read` may still allow.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: modules, tests, operator_training
- Nominal implementation agent: claude
- Rationale: product-policy decision (Auto vs. unattended subagents) plus several call sites.

Validation:
- Auto + no yaml row + shell → Deny.
- Auto + user AlwaysAllow shell → Allow.
- Auto + user AskBefore shell → Deny (already implemented).

Non-goals:
- Do not remove Auto mode or rewrite the inspector framework.

### SEC-GOS-004: Working-dir filesystem boundary is opt-in

Severity: Medium (High with Auto)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A01 · API: API5 · ASVS: V4/V12 · CWE: CWE-22 / CWE-862 · CSEC: CSEC-033 (enforcement optional) · SEC: SEC-008 / SEC-011

Evidence:
- `crates/gosling/src/permission/working_dir_scope_inspector.rs:45-47`:
  ```
  if !session.restrict_tools_to_working_dirs && session.workspace_context.is_none() {
      return Ok(Vec::new());
  }
  ```
- The inspector itself is careful when enabled: canonicalize + dangling-symlink fail-closed + shell path extraction (`148-186`, `693-714` tests).
- Import **does** force the flag on (`session_manager.rs:4857` `restrict_tools_to_working_dirs(true)`).

Observed behavior:
- Ordinary (non-workspace, non-imported) sessions do not constrain tool paths. `cat ~/.ssh/id_rsa` is not flagged.

Expected boundary:
- A session that can run a shell should have a pinned filesystem root unless the user explicitly widens it.

Failure mechanism:
- The strong path-containment code is gated behind a default-off flag. Default posture is “whole user account.”

Break-it angle:
- New chat in project dir, leave restrict off, Auto or approve shell, read `~/.ssh` or write `/etc` (if permitted by OS). Inspector returns `[]`.

Impact:
- Path/scope bypass by non-use of the guard.
- Blast radius: Workflow
- Side-effect class: file
- Reversibility: irreversible
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- SEC-GOS-003.

Recommended mitigation:
- Remediation patterns: default-on containment; explicit widen.
- Minimal repair: default `restrict_tools_to_working_dirs` true for new user sessions; keep workspace policy as-is.
- Local guardrail: if Auto and flag false, still run the inspector in require-approval mode.
- Behavior test: new session, `cat ~/.ssh/id_rsa` → RequireApproval or Deny.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: modules, tests, operator_training
- Nominal implementation agent: claude
- Rationale: default change affects every new session; needs UX copy.

Validation:
- New session flags `../` and `$HOME` paths.
- User can still add additional working dirs.

Non-goals:
- Do not weaken the canonicalize/symlink tests already present.

### SEC-GOS-005: Nostr import fetches attacker-supplied relay URLs

Severity: Medium (High if the process has reachability the operator did not intend)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A10 · API: API7 · ASVS: V13 · CWE: CWE-918 · CSEC: CSEC-061 · SEC: SEC-006 (URL sink)
Bypass row: “server/client fetches a user-influenced URL without allowlist + resolved-IP checks.”

Evidence:
- `crates/gosling/src/session/nostr_share.rs:226-236` — relays come from the nevent:
  ```
  let relays = event_ref
      .relays
      .iter()
      .map(ToString::to_string)
      .collect::<Vec<_>>();
  ...
  let event = fetcher.fetch(event_ref.event_id, &relays).await?;
  ```
- `crates/gosling/src/session/nostr_share.rs:288-298` — `normalize_relays` only trims and dedupes.
- `crates/gosling/src/session/nostr_share.rs:89-97` — `add_relay(relay)` with no scheme/host allowlist.
- Desktop auto-handles `gosling://sessions/nostr` (`ui/desktop/src/App.tsx:347-365`) after a directory chooser.
- Contrast: `crates/gosling-mcp/src/computercontroller/mod.rs:1881-1921` **does** resolve and deny private/metadata IPs.

Observed behavior:
- A crafted share link can name `ws://169.254.169.254/`, `ws://127.0.0.1:<internal>`, or an intranet host. After the user picks a working directory, the process connects.

Expected boundary:
- Same class of control as computer-controller: scheme allowlist (`wss:`), public-suffix/host allowlist or resolved-IP deny for loopback/RFC1918/link-local/metadata.

Failure mechanism:
- User-controlled URL reaches an outbound connect sink with no destination sanitizer.

Break-it angle:
- Build a nevent whose relay list is an internal WS endpoint; open `gosling://sessions/nostr?nevent=...&key=...`; choose a directory. Watch the connect.

Impact:
- SSRF from the desktop/CLI process; possible scan of local ports or cloud metadata if a WS stack completes a handshake.
- Runtime manifestation (whether a given relay stack follows HTTP redirects to metadata) is not executed here — the missing-guard code property is Confirmed; exploitation success is Likely.
- Blast radius: Cross-system
- Side-effect class: network
- Reversibility: n/a (connect)
- Operator visibility: UI-visible import error/success
- Rerun safety: unsafe

Adjacent failure modes:
- Import still marks history untrusted (non-finding) — this finding is the fetch, not replay.

Recommended mitigation:
- Remediation patterns: destination allowlist; resolved-IP deny.
- Minimal repair: reuse `is_disallowed_ip` / `ensure_public_http_url` ideas for `ws`/`wss`; reject non-`wss` and non-public IPs; optionally pin to `GOSLING_NOSTR_RELAYS`.
- Local guardrail: ignore nevent relays and use config/default relays only.
- Behavior test: nevent relay `ws://127.0.0.1:9` is rejected; `wss://relay.damus.io` still works.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: helper already exists in computercontroller.

Validation:
- Private/metadata/loopback relays denied before `add_relay`.
- Empty relay list after filter fails closed.

Non-goals:
- Do not change NIP-44 crypto or event-kind checks.

### SEC-GOS-006: Desktop main and launcher windows omit `sandbox: true`

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A05 · API: — · ASVS: V14 · CWE: CWE-1188 · CSEC: CSEC-073 (host privilege) · SEC: SEC-013 / SEC-011

Evidence:
- `ui/desktop/src/main.ts:1424-1444` — main window `webPreferences`: `webSecurity: true`, `nodeIntegration: false`, `contextIsolation: true`, **no `sandbox`**.
- `ui/desktop/src/main.ts:1685-1688` — launcher window: same omission.
- Sibling that held: `ui/desktop/src/shellHost.ts:38-42` sets `sandbox: true`.

Observed behavior:
- The privileged desktop UI (file IPC, ACP URL with token, artifact write) runs unsandboxed. Preload still uses `contextBridge` and does not expose Node.

Expected boundary:
- Renderer process sandbox on every window that loads untrusted markdown/MCP HTML/session content.

Failure mechanism:
- Defense-in-depth hole. A renderer RCE (markdown XSS, prototype pollution, compromised dependency) talks to preload/IPC without Chromium sandbox confinement. IPC itself is grant-scoped (non-finding), so this is not a direct FS break.

Break-it angle:
- XSS in the main renderer → `electron.writeFile` / `getAcpUrl` / `openExternal`. Grants still apply; ACP token in renderer memory is already there by design.

Impact:
- Raises the value of any renderer XSS from “UI glitch” to “same privileges as preload + granted roots + ACP token.”
- Blast radius: Local
- Side-effect class: process
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unknown

Adjacent failure modes:
- SEC-GOS-001 (token already in renderer URL).

Recommended mitigation:
- Remediation patterns: Electron sandbox on all windows.
- Minimal repair: set `sandbox: true` on main and launcher `webPreferences` (preload already compatible with contextIsolation).
- Local guardrail: fail tests if any `BrowserWindow` is created without sandbox.
- Behavior test: existing `shellHost.test.ts` pattern applied to main-window factory.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: sibling window already sandboxed; watch for preload APIs that need Node.

Validation:
- Window options include `sandbox: true`.
- Smoke: IPC read/write under a grant still works.

Non-goals:
- Do not enable `nodeIntegration`.

### SEC-GOS-007: goslingd leaves MCP-app routes unauthenticated

Severity: Medium (High if `GOSLING_HOST` is not loopback)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A01 / A05 · API: API5 / API8 · ASVS: V4/V14 · CWE: CWE-306 · CSEC: CSEC-002 · SEC: SEC-001 / SEC-010

Evidence:
- `crates/gosling-server/src/auth.rs:16-20`:
  ```
  if request.uri().path() == "/status"
      || request.uri().path() == "/mcp-app-proxy"
      || request.uri().path() == "/mcp-app-guest"
  {
      return Ok(next.run(request).await);
  }
  ```
- GET `/mcp-app-proxy` applies query-controlled CSP to a page (`routes/mcp_app_proxy.rs:229-267`) with no secret check.
- GET `/mcp-app-guest` serves stored HTML by nonce with no secret (`329-371`).
- POST store **is** secret-checked (`276-278`) via `token_matches`.
- Default bind is loopback (`configuration.rs:82-84`); host is env-overridable to any `IpAddr` including `0.0.0.0`.

Observed behavior:
- Unauthenticated clients can load the proxy shell and consume a guest nonce. Store still requires the secret.

Expected boundary:
- Control-plane-adjacent HTML sinks require the same authenticator as `/acp`, or a loopback ConnectInfo check like the ACP serve path (`mcp_app_proxy.rs` in gosling crate:229-235).

Failure mechanism:
- Path allowlist in auth middleware. `/status` is a reasonable liveness exemption; the MCP-app routes are not equivalent.

Break-it angle:
- `curl http://127.0.0.1:3000/mcp-app-proxy?script_domains=evil.example`. If something stored a nonce, GET `/mcp-app-guest?nonce=` retrieves HTML without the secret (nonce is the capability).

Impact:
- Unauthenticated policy-loosened proxy page; nonce-capability guest read. With public bind, this is a world-reachable HTML sink.
- Blast radius: Service
- Side-effect class: network
- Reversibility: compensatable
- Operator visibility: log-only
- Rerun safety: safe (GET)

Adjacent failure modes:
- SEC-GOS-012 (public bind), SEC-GOS-002 (CSP on the other process).

Recommended mitigation:
- Remediation patterns: authenticate or loopback-constrain every non-liveness route.
- Minimal repair: remove the two MCP-app exemptions; keep `/status` only. Rely on the handlers’ own secret/nonce checks plus ConnectInfo loopback.
- Behavior test: GET `/mcp-app-proxy` without secret from a non-loopback peer is 401/400.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: one middleware condition.

Validation:
- Unauthenticated GET `/status` still 200.
- Unauthenticated GET `/mcp-app-proxy` denied unless loopback **and** product accepts that.

Non-goals:
- Do not add a second auth scheme.

### SEC-GOS-008: ACP guest store uses `!=` on the secret

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A07 · API: API2 · ASVS: V2 · CWE: CWE-208 · CSEC: CSEC-049 · SEC: SEC-005

Evidence:
- `crates/gosling/src/acp/mcp_app_proxy.rs:272` — `if body.secret != state.secret_key`
- Sibling that held: `crates/gosling/src/acp/transport/auth.rs:9-13` and `gosling-server` store handler use `subtle::ConstantTimeEq` via `token_matches`.

Observed behavior:
- Byte-string inequality on the high-privilege serve secret.

Expected boundary:
- All compares of the serve secret use `token_matches`.

Failure mechanism:
- Timing side channel on a local HTTP endpoint. Exploitability on loopback is low; the code property is real.

Break-it angle:
- Statistical timing of POST `/mcp-app-guest` bodies. Not executed (runtime manifestation capped conceptually at Likely; the `!=` property is Confirmed).

Impact:
- Theoretical secret recovery given a fast local timer and no jitter.
- Blast radius: Local
- Side-effect class: none
- Reversibility: n/a
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- SEC-GOS-002 (same handler).

Recommended mitigation:
- Remediation patterns: constant-time compare.
- Minimal repair: `token_matches(Some(body.secret.as_str()), &state.secret_key)`.
- Behavior test: wrong secret 401; right secret 200 (existing).

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: one-line plus import.

Validation:
- Store with wrong-length secret still 401.
- Both handlers share `token_matches`.

Non-goals:
- Do not add rate limiting in this slice.

### SEC-GOS-009: Prompt-injection scanner logs full tool-call JSON

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A09 · API: API8 · ASVS: V7/V8 · CWE: CWE-532 · CSEC: CSEC-043 · SEC: SEC-015 / SEC-007

Evidence:
- `crates/gosling/src/security/mod.rs:160-175` (degraded), `191-209` (malicious), `227-241` (passed):
  ```
  let tool_call_json = serde_json::to_string(&tool_call).unwrap_or_else(|_| "{}".to_string());
  ...
  tool.call_json = %tool_call_json,
  ```
- Tool arguments routinely include file contents, paths, and command lines. File logs are written under the state dir (`crates/gosling/src/logging.rs:56-64`).

Observed behavior:
- Every scanned tool call, including those that pass, is serialized into tracing fields and therefore into rolling log files.

Expected boundary:
- Security telemetry logs tool name, id, decision, and a redacted/hashed argument digest — not raw arguments.

Failure mechanism:
- Defense feature becomes a secret/PII sink. `cat` of an env file or an API key in a command is persisted on disk.

Break-it angle:
- Approve/Auto a tool whose args contain a key; grep `logs/` for the key.

Impact:
- Secret persistence in logs; broader than the original tool invocation.
- Blast radius: Repo
- Side-effect class: file
- Reversibility: compensatable (delete logs; rotate leaked keys)
- Operator visibility: log-only
- Rerun safety: unsafe

Adjacent failure modes:
- SEC-GOS-010 (another secret-prefix leak).

Recommended mitigation:
- Remediation patterns: structured redaction; never log raw tool args.
- Minimal repair: log `tool.name` + `tool.request_id` only; drop `tool.call_json`.
- Behavior test: scanner test fixture with sentinel secret does not appear in captured logs.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: delete or redact one field in three branches.

Validation:
- Sentinel `sk-test-not-a-real-key` in tool args is absent from log output.

Non-goals:
- Do not disable the scanner.

### SEC-GOS-010: `/config/read` mask reveals first 8 secret characters

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A02 / A09 · API: API3 · ASVS: V8 · CWE: CWE-200 · CSEC: CSEC-043 · SEC: SEC-015

Evidence:
- `crates/gosling-server/src/routes/config_management.rs:226-239`:
  ```
  const SECRET_MASK_SHOW_LEN: usize = 8;
  let show_len = std::cmp::min(chars.len() / 2, SECRET_MASK_SHOW_LEN);
  let visible: String = chars.iter().take(show_len).collect();
  ```
- Route is token-authenticated (`check_token`), then returns `MaskedSecret` for `is_secret: true` (`308-313`).

Observed behavior:
- Up to the first 8 characters of a keyring secret are sent to any client that holds the server secret.

Expected boundary:
- Existence + last-4 or a boolean `configured` only (the provider-secrets list already has `has_secret` without values).

Failure mechanism:
- Prefix leak shrinks brute-force/search space and identifies key type (`sk-ant-`, `ghp_`, `xai-`).

Break-it angle:
- `POST /config/read` `{"key":"OPENAI_API_KEY","is_secret":true}` with `X-Secret-Key`.

Impact:
- Partial secret disclosure to every token holder (and to XSS that can call goslingd).
- Blast radius: Service
- Side-effect class: network
- Reversibility: compensatable (rotate)
- Operator visibility: UI-visible
- Rerun safety: safe

Adjacent failure modes:
- SEC-GOS-001 (token theft makes this reachable).

Recommended mitigation:
- Remediation patterns: no secret material in API responses.
- Minimal repair: return `{ "configured": true }` or last-4 only.
- Behavior test: response must not contain the first 8 chars of a known fixture secret.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: one function; UI may need to stop displaying prefixes.

Validation:
- Fixture secret `abcdefghijklmnop` → mask does not include `abcdefgh`.

Non-goals:
- Do not change keyring storage.

### SEC-GOS-011: WebSocket origin check skipped when `Origin` is absent

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A05 · API: API8 · ASVS: V14 · CWE: CWE-346 · CSEC: CSEC-073 · SEC: SEC-005

Evidence:
- `crates/gosling/src/acp/transport/mod.rs:122-127`:
  ```
  if is_websocket_upgrade(&request) {
      if let Some(origin) = request.headers().get(header::ORIGIN) {
          if !policy.origin_allowed(origin) {
              return Err(StatusCode::FORBIDDEN);
          }
      }
  ```
- Missing Origin → no reject. Then Origin is overwritten to `http://gosling.local` for the upstream (`129-132`).
- CORS for fetch still uses the predicate (`138-141`).

Observed behavior:
- Non-browser clients (and some browser cases that omit Origin) skip the origin allowlist. Token still applies when `require_token` is true.

Expected boundary:
- Browser-like transports: require Origin and allowlist it. Native clients: use a separate non-WS path or a required custom header.

Failure mechanism:
- CSRF/WS-hijack pattern: a remote page cannot set Origin to empty in a modern browser, so this is mainly “native client + unauthenticated serve” and future policy holes. Combined with SEC-GOS-012 it is material.

Break-it angle:
- `--dangerously-unauthenticated --host 0.0.0.0`, then WS upgrade with no Origin. Connect succeeds.

Impact:
- Origin policy is not a reliable WS boundary.
- Blast radius: Service
- Side-effect class: network
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unknown

Adjacent failure modes:
- SEC-GOS-012.

Recommended mitigation:
- Remediation patterns: fail-closed Origin on WS; require token independently.
- Minimal repair: if upgrade && Origin missing && policy is not “any native,” reject **or** require token (already default).
- Behavior test: WS upgrade without Origin and without token is 401/403 even with `--dangerously-unauthenticated` unless bind is loopback.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: one branch; tests already in `acp_transport_auth_test.rs`.

Validation:
- Missing Origin + valid token: decide allow (native) vs deny (browser-only) and lock it with a test.
- Missing Origin + no token: deny.

Non-goals:
- Do not break CLI ACP clients that omit Origin if they send the header token.

### SEC-GOS-012: Unauthenticated serve does not force loopback bind

Severity: High if reachable
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A05 / A04 · API: API8 · ASVS: V14 · CWE: CWE-1188 · CSEC: CSEC-045 · SEC: SEC-009 / SEC-014

Evidence:
- `crates/gosling-cli/src/cli.rs:757-758` — `--host` default `127.0.0.1`, but any string parseable as `SocketAddr` is accepted (`1543`).
- `crates/gosling-cli/src/cli.rs:1502-1511` — refuse start without secret **unless** `--dangerously-unauthenticated`; warning is printed; **no check that host is loopback**.
- `create_router(..., require_token: false)` omits `check_acp_token` (`crates/gosling/src/acp/transport/mod.rs:193-198`).

Observed behavior:
- `gosling serve --dangerously-unauthenticated --host 0.0.0.0` is a valid invocation and then exposes the full ACP agent on all interfaces with no auth.

Expected boundary:
- A documented “dangerous” flag still cannot bind a non-loopback address without a second explicit flag, or it must refuse.

Failure mechanism:
- Reverse-proxy / “I’ll firewal it” assumption is not enforced at runtime (SEC-014).

Break-it angle:
- Run the command above on a VPS; curl `/acp` from the internet. Static property Confirmed; live bind not executed.

Impact:
- Unauthenticated remote agent control plane.
- Blast radius: Service
- Side-effect class: network
- Reversibility: compensatable (kill process)
- Operator visibility: stderr warning only
- Rerun safety: unsafe

Adjacent failure modes:
- SEC-GOS-011, SEC-GOS-001.

Recommended mitigation:
- Remediation patterns: fail-closed bind; two-person dangerous flags.
- Minimal repair: if `!require_token` && host is not loopback → bail.
- Behavior test: the command above exits non-zero; loopback unauth still starts (if product wants that).

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: one start-up check.

Validation:
- Non-loopback + unauth → error mentioning both flags.
- Loopback + secret → start.

Non-goals:
- Do not remove the dangerous flag if CLI tests depend on it.

### SEC-GOS-013: SmartApprove treats LLM “read-only” as Allow

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security
OWASP: A01 / A04 · API: API5 · ASVS: V4/V11 · CWE: CWE-807 · CSEC: CSEC-006 / CSEC-073 · SEC: SEC-005
Escalate: `audit-security-llm` owns prompt-injection flavor; this finding is the **classic** “untrusted classifier is the authorization oracle.”

Evidence:
- `crates/gosling/src/permission/permission_judge.rs:149-190` — LLM tool `platform__tool_by_tool_permission` returns `read_only_tools` names.
- `crates/gosling/src/permission/permission_inspector.rs:206-243` — if the name is in that set, `InspectionAction::Allow` with reason `"LLM detected as read-only"`.
- Judge prompt explicitly says a generic shell can be read-only depending on args (`permission_judge.rs:91-92`, `116-117`), so the model is asked to bless `developer__shell`.

Observed behavior:
- SmartApprove will auto-run a tool the model (or a poisoned transcript) classifies as read-only, including a shell whose args the judge misread.

Expected boundary:
- Authorization oracles are deterministic. An LLM may only **tighten** (force Ask/Deny), never grant Allow. Tool annotations already follow that rule (`config/permission.rs:142-144`).

Failure mechanism:
- Trust inversion: the same model family that proposed the tool is asked to authorize it. Prompt injection in prior tool output can target the judge context.

Break-it angle:
- SmartApprove; arrange judge output `read_only_tools: ["developer__shell"]` for `rm`/`curl`. Code path Allows without a user prompt.

Impact:
- Authorization bypass via classifier error or injection.
- Blast radius: Workflow
- Side-effect class: process
- Reversibility: irreversible
- Operator visibility: UI-visible after execution
- Rerun safety: unsafe

Adjacent failure modes:
- SEC-GOS-003 (Auto is the blunt version of the same default-allow). LLM lens: indirect prompt injection.

Recommended mitigation:
- Remediation patterns: classifier can only deny; static allowlist for auto-approve.
- Minimal repair: LLM result may set RequireApproval; never Allow. Keep Allow for user AlwaysAllow and a hardcoded read-only name list.
- Behavior test: judge returns `developer__shell` as read-only; inspector still RequireApproval.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: modules, tests, operator_training
- Nominal implementation agent: multi-agent
- Rationale: crosses permission product policy and LLM-judge prompts.

Validation:
- SmartApprove + shell → not Allow solely from judge.
- SmartApprove + `developer__text_editor__read` can still auto-allow via static list.

Non-goals:
- Do not delete the judge; use it as a denylist signal only.

## Inventory coverage

### SEC-001..015

| Code | Verdict | Note |
|---|---|---|
| SEC-001 Missing Authentication | **Finding** SEC-GOS-007, SEC-GOS-012 | Default serve requires secret; goslingd MCP-app GETs and `/status` do not |
| SEC-002 Missing Authorization | **Finding** SEC-GOS-003, SEC-GOS-013 | One shared secret; no role model; Auto/SmartApprove skip function authz |
| SEC-003 Object Scope / IDOR | **Non-finding (local-first)** / **design** | `get_session` is id-only (`session.rs:57-68`). Single secret ⇒ every session is in-scope. Not multi-tenant. |
| SEC-004 Privilege Escalation | **Finding** SEC-GOS-003 | Auto/subagent Allow-all |
| SEC-005 Trust Boundary Confusion | **Finding** SEC-GOS-002, SEC-GOS-011, SEC-GOS-013 | Client CSP; missing Origin; LLM judge |
| SEC-006 Injection | **Non-finding** (SQL/cmd argv); **Finding** SEC-GOS-005 (URL) | sqlx binds; MCP spawn is argv; shell `-c` is intentional after permission |
| SEC-007 Secret Exposure | **Finding** SEC-GOS-001, SEC-GOS-009, SEC-GOS-010 | |
| SEC-008 Unsafe Path/File | **Finding** SEC-GOS-004 (opt-in); **Non-finding** when inspector on; Electron grants held |
| SEC-009 Unsafe Deployment Default | **Finding** SEC-GOS-012; goslingd default host/TLS are safe (`127.0.0.1`, TLS true) |
| SEC-010 Sensitive Route Exposure | **Finding** SEC-GOS-007 | `/mcp-app-proxy`, `/mcp-app-guest`; `/health`/`/status` are liveness |
| SEC-011 Overbroad Permission | **Finding** SEC-GOS-003, SEC-GOS-004, SEC-GOS-006 | |
| SEC-012 Unsafe External Tool Invocation | **Non-finding** for spawn env (`env_clear` + allowlist); **Finding** SEC-GOS-003 for *when* the tool runs |
| SEC-013 Security Boundary Only In UI | **Finding** SEC-GOS-006 (sandbox sibling-only); permission is server-side (held) |
| SEC-014 Reverse Proxy Assumption | **Finding** SEC-GOS-012 | Unauth + non-loopback bind allowed |
| SEC-015 Provider/Env Secret Leakage | **Finding** SEC-GOS-001, SEC-GOS-009, SEC-GOS-010; MCP child env held |

### CSEC-001..074

| Code | Verdict |
|---|---|
| CSEC-001 Object-scope | Non-finding / N/A multi-tenant. Session lookup by id under one secret. |
| CSEC-002 Function-level | **Finding** SEC-GOS-003 / SEC-GOS-013 |
| CSEC-003 Mass assignment | Non-finding on import: new session created; `EnabledExtensionsState` stripped; mode forced `Approve`; restrict-tools forced on (`session_manager.rs:4809-4857`). |
| CSEC-004 Tenant predicate on lists | N/A — no tenants. |
| CSEC-005 Service-role / BYPASSRLS | N/A — SQLite, no RLS. |
| CSEC-006 Client-supplied identity | **Finding** SEC-GOS-013 (LLM as identity/oracle). Token itself is server-held. |
| CSEC-011 SQL string-build | Non-finding. Dynamic SQL only adds `?` placeholders (`chat_history_search.rs:122-137`; `last_message_snippet.rs:77-86`). |
| CSEC-012 ORM raw interpolates | Non-finding. sqlx `.bind`. |
| CSEC-013 Dynamic identifier | Non-finding. `session_type IN (?,?,…)` bound; no user ORDER BY column. |
| CSEC-014 Multi-statement from input | Non-finding. No `executescript` on request data. |
| CSEC-015 Command injection | Non-finding at spawn (argv). Shell tool uses `-c` **after** permission (product). Auto makes that High (SEC-GOS-003). |
| CSEC-021..025 Privileged DB / search_path | N/A — no PostgreSQL / SECURITY DEFINER. |
| CSEC-026 pickle | N/A — Rust. |
| CSEC-027 Unsafe YAML | Non-finding. `serde_yaml` is data-only (permission.yaml, config, plugins). |
| CSEC-028 eval/exec | Non-finding in Rust/TS product code. Inline Python extensions write a temp `.py` and run via `uvx` after OSV on deps (`extension_manager.rs:1762-1786`) — operator-installed code, not request `eval`. |
| CSEC-029 XXE | N/A — no untrusted XML parse found. |
| CSEC-030 Dynamic import / network config | Partial / design: `GOSLING_ALLOWLIST` URL is documented (`gosling-server/ALLOWLIST.md`) as desktop launch-time; not re-audited as a live fetch here. Plugin discovery is local FS only. |
| CSEC-033 Path traversal | **Finding** SEC-GOS-004 (guard off by default). When on: held (`canonicalize_potential_path`, symlink tests). Electron: held (`assertPathWithinRoots`). |
| CSEC-034 Zip-slip | Non-finding — no archive extract of untrusted input found in crates. |
| CSEC-035 Upload filename trusted | Non-finding on guest HTML (not stored as a filename). Artifact save uses dialog + grant. |
| CSEC-036 SQLite URI/flags from input | Non-finding — pool path from `Paths`, not request. No `enable_load_extension` found. |
| CSEC-037 Insecure temp | Non-finding sampled: docker env file via `tempdir`; inline python via `tempdir`. |
| CSEC-041 Hardcoded secret | Non-finding. Matches were test fixtures / docs examples (`gcpauth.rs` tests, prompt-library sample). |
| CSEC-042 Service secret in client bundle | Non-finding. Desktop generates per-process secret and passes via env, not a baked key. |
| CSEC-043 Auth/token/DSN in logs | **Finding** SEC-GOS-009, SEC-GOS-010. ACP diagnostic URL is redacted (`goslingServe.ts:338-340`). |
| CSEC-044 Secret in URL | **Finding** SEC-GOS-001 |
| CSEC-045 Debug/verbose errors | Partial: panic hook prints backtrace to stderr (`gosling-server/src/main.rs:59-60`). Not a client response. `/status` is `"ok"` / degraded string only. |
| CSEC-047 Weak token generation | Non-finding. goslingd: `hex::encode(rand::random::<[u8; 32]>())` (`agent.rs:121-122`). Serve fallback: 32 alphanumeric via `rand::rng()` (`cli.rs:69-75`). |
| CSEC-048 Missing expiry | Design: serve secret is process-lifetime. Acceptable for local process; no idle timeout. |
| CSEC-049 Timing-unsafe compare | **Finding** SEC-GOS-008; `token_matches` held elsewhere |
| CSEC-050 Ad-hoc JWT | Non-finding — no custom JWT parse found on these surfaces. |
| CSEC-051 Open redirect | Non-finding. `open-in-chrome` uses `normalizeWebUrl` (http/https only). `openExternal` uses `isProtocolSafe`. |
| CSEC-052 Fast password hash | N/A — no local password DB. |
| CSEC-055 TLS verify disabled | Non-finding in product. Only `danger_accept_invalid_certs(true)` is in `gosling-server/tests/tls_test.rs`. |
| CSEC-056 Weak/ECB crypto | Non-finding on these surfaces. Nostr uses NIP-44 v2. |
| CSEC-057 Static key/IV | Non-finding. |
| CSEC-061 User URL fetch | **Finding** SEC-GOS-005. Computer-controller held. |
| CSEC-062 Redirect-follow to metadata | Computer-controller denies resolved private IPs (pre-connect). Nostr path not protected. |
| CSEC-063 Unbounded fetch | Nostr: 8s connect / 10s fetch. Guest HTML: 16 MiB ACP / TTL 300s / 64 entries. Import: 16 MiB. Held where present. |
| CSEC-066 RMW without lock | Out of scope for this lens (state/concurrency). Permission persist uses a persist mutex (`permission.rs:97-107`). |
| CSEC-067 Replayable transition | Session import nonce/sha recorded; Nostr guest nonce one-time. ACP token has no nonce (process secret). |
| CSEC-068 Missing idempotency | Not a security defect on read routes. Import creates a new session id. |
| CSEC-069 Bulk mutate without owner | N/A tenants. |
| CSEC-073 Logic/authz semantic | **Finding** SEC-GOS-002, SEC-GOS-003, SEC-GOS-011, SEC-GOS-013 |
| CSEC-074 Business-flow abuse | Partial: Auto/subagent is unlimited agency (SEC-GOS-003). Quota/rate not designed (API4). |

## OWASP Top 10 (2021) coverage matrix

| Category | Verdict | Evidence (file:line) | ASVS | CSEC | Notes |
|---|---|---|---|---|---|
| A01 Broken Access Control | finding | `permission_inspector.rs:162-166`; `working_dir_scope_inspector.rs:45-47`; `session.rs:57-68` | V4 | CSEC-002, 033, 073 | Auto allow-all; FS scope opt-in; no object owner |
| A02 Cryptographic Failures | finding | `auth.rs:25-31`; `goslingServe.ts:334-336`; `config_management.rs:226-239` | V6/8/9 | CSEC-044, 043 | Token in URL; mask prefix |
| A03 Injection | covered | sqlx binds; MCP `args` argv; `normalizeWebUrl` | V5 | CSEC-011..015 | Shell `-c` is authorized execution, not injection |
| A04 Insecure Design | finding | Auto + SmartApprove + LLM judge | V1/11 | CSEC-073/074 | Default agency |
| A05 Security Misconfiguration | finding | `main.ts:1424-1444`; `transport/mod.rs:122-127`; `cli.rs:1507-1511` | V14 | CSEC-045 | No sandbox; Origin optional; unauth bind |
| A06 Vulnerable & Outdated Components | n/a here | — | V14 | — | Routed to `audit-security-repo-posture` |
| A07 Identification & Authentication Failures | finding | query token; `!=` compare; process-lifetime secret | V2/3 | CSEC-044, 048, 049 | |
| A08 Software & Data Integrity Failures | finding | ACP `body.csp` | V5/12 | CSEC-073 | Import integrity held |
| A09 Security Logging & Monitoring Failures | finding | `security/mod.rs` `tool.call_json` | V7 | CSEC-043 | Auth failures return 401 without body leak |
| A10 Server-Side Request Forgery | finding | `nostr_share.rs:226-236` vs computercontroller held | V13 | CSEC-061 | |

## API Security Top 10 (2023) coverage matrix

API surface is in scope (`gosling serve` ACP + `goslingd` REST).

| API category | Verdict | Evidence | Maps to | Notes |
|---|---|---|---|---|
| API1 BOLA | design / n/a multi-tenant | session by id | A01 | One secret sees all sessions |
| API2 Broken Authentication | finding | query token; unauth flag | A07 | SEC-GOS-001, 012 |
| API3 BOPLA | covered on import | extension state stripped | A01 | Masked secret still leaks prefix (API3-ish) |
| API4 Unrestricted Resource Consumption | partial | guest 16MiB/64; import 16MiB; no request rate limit | A04 | |
| API5 BFLA | finding | Auto / add-extension / config upsert | A01 | Token = admin |
| API6 Sensitive Business Flows | finding | tool exec, import, extension spawn | A04 | |
| API7 SSRF | finding | Nostr relays | A10 | |
| API8 Security Misconfiguration | finding | CORS loopback predicate; client CSP | A05 | `*` origin rejected at CLI |
| API9 Improper Inventory Management | covered | OpenAPI + utoipa on goslingd; ACP schema generated | A04/A05 | Shadow guest port on ACP path |
| API10 Unsafe Consumption of APIs | finding | Nostr; OSV fail-open for local cmds | A08/A10 | |

## Authorization matrix (sampled)

| Actor | Resource | Operation | Expected guard | Observed guard | File:line |
|---|---|---|---|---|---|
| Unauth TCP client | `/acp` | any | deny unless dangerous flag | deny if secret set | `cli.rs:1502-1505`, `transport/mod.rs:193-198` |
| Token holder | any session | GET/fork/import | same principal | any id | `session.rs:57-68` |
| Token holder | `/config/upsert` | write secret | admin | any token | `config_management.rs:167-191` |
| Unauth | `/status` | GET | allow liveness | allow | `auth.rs:16`; serve `/health` |
| Unauth | `/mcp-app-proxy` (goslingd) | GET | deny or loopback | allow | `auth.rs:17-19` |
| Loopback + secret | ACP guest store | POST | server CSP | **client CSP** | `acp/mcp_app_proxy.rs:284` |
| Auto agent | `developer__shell` | execute | deny or ask | **Allow** if no user row | `permission_inspector.rs:162-166` |
| SmartApprove agent | tool LLM calls read-only | execute | ask | **Allow** | `permission_inspector.rs:237-243` |
| Renderer | `write-file` | FS write | grant root | grant + realpath | `main.ts:2519-2522`, `rendererFileAccess.ts:59-72` |
| Imported message | artifact register | write | skip | skip if `imported_untrusted` | `session_manager.rs:4226-4228` |

## Taint-path table

| Source | Sanitizer | Sink | Result |
|---|---|---|---|
| Request `?token=` | `token_matches` (ct) | ACP auth | **Finding** — secret in URL |
| Tool args | none | `tool.call_json` log | **Finding** |
| Config `is_secret` value | `mask_secret` first 8 | HTTP JSON | **Finding** |
| FTS user query | bound `MATCH ?` | SQLite FTS | Held |
| MCP `cmd`/`args` | argv + `env_clear` | `Command::new` | Held |
| Shell `command` | permission inspectors | `sh -c` | Held only if permission holds |
| Electron `filePath` | `canonicalizePotentialPath` + roots | `fs.readFile/writeFile` | Held |
| `open-in-chrome` url | `normalizeWebUrl` | `shell.openExternal` | Held |
| Nostr nevent relays | trim/dedupe only | `add_relay` | **Finding** |
| Computer-controller URL | resolve + `is_disallowed_ip` | HTTP fetch | Held |
| MCP-app `csp` (goslingd) | `normalize_csp_source` + build | CSP header | Held |
| MCP-app `csp` (ACP serve) | **none** | CSP header | **Finding** |
| Import JSON | size cap + `with_imported_untrusted` | session DB | Held |
| permission.yaml | `serde_yaml` / panic on corrupt | PermissionManager | Held (fail-closed parse) |

## Non-Findings / Checked But Not Confirmed

- **SQL parameterization held.** `chat_history_search.rs:118-139` and `last_message_snippet.rs:77-86` bind every value; dynamic fragments are `?` placeholders only.
- **MCP spawn env held.** `extension_manager.rs:1745-1747` `env_clear().args(args).envs(all_envs)` plus `minimal_child_environment()` (`390-431`). Test name `minimal_child_environment_drops_inherited_secrets` exists. Docker secrets go through `--env-file`, not argv (`1726-1736`).
- **OSV malware gate held for npx/uvx.** `extension_malware_check.rs:73-90` fail-closed without a package argument. Fail-open for local binaries is documented and intentional.
- **Tool annotations cannot self-grant.** `config/permission.rs:142-144` — annotations only force `AskBefore`.
- **Working-dir inspector quality held when enabled.** Canonicalize, dangling symlink fail-closed, shell option/redir/`$HOME`/`file://` coverage with tests in `working_dir_scope_inspector.rs`.
- **Session import untrusted held.** `history_trusted: false`; every message `with_imported_untrusted()`; artifact registration skipped (`session_manager.rs:4226-4228`, `2732`). Size cap 16 MiB. CWD must be absolute existing directory.
- **Electron path grants held.** Symlink grant roots rejected (`rendererDirectoryGrants.ts:24-31`); access realpath-contained (`rendererFileAccess.ts:59-72`).
- **External URL protocols held.** `urlSecurity.ts` + `open-in-chrome` http(s) only (`main.ts:3209-3217`).
- **CORS wildcard rejected.** CLI `--allowed-origin *` bails (`cli.rs:1517-1518`). Desktop CORS limited to `null` + loopback (`agent.rs:41-55`).
- **ACP serve default host loopback.** `cli.rs:757`. Desktop forces `--host 127.0.0.1` (`goslingServe.ts:451-452`).
- **goslingd TLS default true / host 127.0.0.1.** `configuration.rs:82-91`.
- **Constant-time token compare held** on ACP HTTP middleware and goslingd REST (`auth.rs` both crates).
- **Computer-controller SSRF held.** `ensure_public_http_url` + metadata/private deny with tests.
- **OAuth callback loopback-only.** `oauth/mod.rs:135-137`; CSRF `state` passed to `handle_callback` (`178`).
- **No product `danger_accept_invalid_certs`.** Test-only.
- **No hardcoded live credentials** in the inspected product paths.
- **Guest CSP domain parser held** against `*`, `javascript:`, `;`, userinfo (`normalize_csp_source` tests in both proxies).
- **Permission.yaml corrupt → panic/refuse start** (`permission.rs:51-60`), not fail-open empty allow.

## Break-It Review

| Attack | Result |
|---|---|
| Unauth GET `/acp` with default serve | Denied unless secret set or dangerous flag (held / SEC-GOS-012 if misbound) |
| Valid token only in `?token=` | **Succeeds** (SEC-GOS-001) |
| Unauth GET `/status` | Succeeds; body `"ok"` only (held as liveness) |
| Unauth GET goslingd `/mcp-app-proxy` | **Succeeds** (SEC-GOS-007) |
| POST guest HTML with client `csp: "script-src *"` on `gosling serve` | **Succeeds** (SEC-GOS-002) |
| Same on goslingd store | CSP derived; `*` dropped (held) |
| Cross-session id with one server secret | Succeeds by design (single-user) |
| FTS `'; DROP` | Bound as FTS query text (held) |
| MCP cmd `; id` | Single argv token (held) |
| Electron `read-file` `../../.ssh/id_rsa` | Denied unless that tree is a grant (held) |
| `open-in-chrome` `file:///etc/passwd` | Rejected (held) |
| Auto + unlisted `developer__shell` | **Allowed** (SEC-GOS-003) |
| Path `~/.ssh` with restrict off | **Not flagged** (SEC-GOS-004) |
| Path `~/.ssh` with restrict on | Flagged (held) |
| Import hostile session JSON | New id, Approve, restrict on, untrusted history (held) |
| Nostr deeplink with `ws://127.0.0.1` relay | **Connect attempted** (SEC-GOS-005) |
| Computer-controller `http://169.254.169.254/` | Rejected (held) |
| WS upgrade, no Origin, with token | Allowed (SEC-GOS-011 code property) |
| `--dangerously-unauthenticated --host 0.0.0.0` | **Start allowed** (SEC-GOS-012) |
| Scanner-pass tool with secret in args | **Logged in full** (SEC-GOS-009) |

Oracle integrity: no test suite was used as evidence of a non-finding. Non-findings are source-quoted guards, not green CI.

## Recommended Patch Order

1. SEC-GOS-001 — drop query-string token (auth + desktop URL).
2. SEC-GOS-002 + SEC-GOS-008 — derive guest CSP; `token_matches`.
3. SEC-GOS-012 — refuse unauth non-loopback bind.
4. SEC-GOS-007 — authenticate or loopback-constrain goslingd MCP-app routes.
5. SEC-GOS-003 + SEC-GOS-013 — Auto/SmartApprove cannot Allow mutating tools.
6. SEC-GOS-004 — default-on working-dir restrict.
7. SEC-GOS-005 — Nostr relay allowlist / resolved-IP deny.
8. SEC-GOS-009 + SEC-GOS-010 — stop logging/returning secret material.
9. SEC-GOS-006 + SEC-GOS-011 — sandbox windows; fail-closed Origin policy.

## Regression Test Strategy

| Test | Purpose | Finding |
|---|---|---|
| `/acp?token=valid` without header → 401 | Query token dead | SEC-GOS-001 |
| Desktop `buildLocalServeUrls` has no `token` query | Producer side | SEC-GOS-001 |
| ACP store `csp: "script-src *"` → derived header | Sandbox authority | SEC-GOS-002 |
| ACP store uses `token_matches` (wrong-length 401) | Timing | SEC-GOS-008 |
| Auto + no yaml + `developer__shell` → Deny | Default deny | SEC-GOS-003 |
| SmartApprove + judge says shell read-only → not Allow | Oracle | SEC-GOS-013 |
| New session `cat ~/.ssh/id_rsa` flagged | Default scope | SEC-GOS-004 |
| Nostr relay `ws://127.0.0.1:9` rejected | SSRF | SEC-GOS-005 |
| goslingd GET `/mcp-app-proxy` unauth non-loopback denied | Route auth | SEC-GOS-007 |
| `serve --dangerously-unauthenticated --host 0.0.0.0` exits | Bind | SEC-GOS-012 |
| Sentinel secret absent from scanner logs | Redaction | SEC-GOS-009 |
| `/config/read` secret has no 8-char prefix | Mask | SEC-GOS-010 |
| Main window `webPreferences.sandbox === true` | Electron | SEC-GOS-006 |

## Deferred Risks

- Process-lifetime ACP secret without idle expiry (CSEC-048) — acceptable for a supervised local process; revisit if serve is hosted.
- OSV fail-open for local/non-registry launchers — documented; operator-trust model.
- `GOSLING_ALLOWLIST` remote YAML — not shown as enforced in gosling-server; desktop-only note in ALLOWLIST.md. Route to repo-posture / LLM-adjacent if the fetch is live.
- Single-secret multi-session model — fine for local-first; do not market as multi-user without object authz.
- Prompt injection → tool execution — `audit-security-llm`.
- Supply chain / lockfile CVEs — `audit-security-repo-posture`.
- Electron renderer XSS in markdown/MCP UI — `audit-security-nodejs` / `audit-design-webapp`.

## Scanner blind spots (needs human semantic review)

- **A01 / CSEC-073 business logic:** Auto vs SmartApprove vs Chat vs workspace policy interactions; subagent mode inheritance (`permission_inspector.rs:133-139`).
- **A04 insecure design:** local-first “the user is the agent” vs a bindable HTTP server.
- **A09 alerting:** logs exist; no evidence of operator alerting on `prompt_injection_finding` or unauth serve.
- **RLS/DB privilege:** N/A (SQLite file as the user).
- **State integrity / concurrency:** permission persist lock reviewed; session races belong to concurrency/integrity lenses.

## ASVS Coverage Note

```
Target level: L2 (justification: local agent with provider API keys, filesystem tools, and a bindable HTTP/WS control plane)
Scan type: repo-only — 2026-08-15

Exercised chapters:
- V1 Architecture / threat model — L2 design review of serve/desktop/MCP/import boundaries
- V2 Authentication — L2 source-verified (token, query, unauth flag)
- V3 Session Management — L1–L2 (process-lifetime secret; no idle timeout)
- V4 Access Control — L2 source-verified (Auto, SmartApprove, session-by-id)
- V5 Validation / sanitization — L2 (SQL binds, CSP normalize, import size)
- V6 Stored Cryptography — L1–L2 (keyring; no password KDF surface)
- V7 Error Handling & Logging — L2 (tool.call_json; 401 bodies)
- V8 Data Protection — L1–L2 (URL secret; mask prefix)
- V9 Communication (TLS) — L1–L2 (goslingd default TLS; serve TLS opt-in; no verify-disable in product)
- V11 Business Logic — L1 semantic (Auto/judge) — human-review-gated
- V12 Files & Resources — L2 (working-dir inspector; Electron grants)
- V13 API & Web Service / SSRF — L2 (Nostr vs computercontroller)
- V14 Configuration — L1–L2 (bind defaults, CORS, sandbox)

Not exercised / out of scope:
- V10 Malicious Code / build provenance → audit-security-repo-posture
- Live TLS cipher/header observation (no runtime)
- ASVS L3 (needs testing + independent architecture review)

Depth caveats:
- V11 verdicts are semantic.
- Runtime-only checks (live headers, actual WS from a browser) were not done.
```

## Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| SEC-GOS-001 | Security | Workflow/GUI, Input/Output | Renderer constructs and stores the tokenized URL |
| SEC-GOS-002 | Security | Architecture/Seam, Input/Output | Sibling ACP vs goslingd implementations drifted |
| SEC-GOS-003 | Security | Workflow/GUI, LLM | Mode is a product control; injection targets it |
| SEC-GOS-004 | Security | Input/Output, Negative Space | Default-off path guard |
| SEC-GOS-005 | Security | Input/Output, Cascade | Deeplink → outbound connect |
| SEC-GOS-006 | Security | Architecture (nodejs), Workflow/GUI | Electron window factory split |
| SEC-GOS-007 | Security | Architecture/Seam | goslingd vs serve route auth |
| SEC-GOS-009 | Security | Compliance/Posture, Reliability | Logs as a secret store |
| SEC-GOS-011 | Security | Architecture/Seam | WS vs CORS policy split |
| SEC-GOS-012 | Security | Reliability, Negative Space | Dangerous flag + bind |
| SEC-GOS-013 | Security | LLM, State Transition | Classifier as authorization |

Also escalate the whole LLM/tool/MCP confused-deputy surface to `audit-security-llm` (assigned separately). Electron preload/IPC details to `audit-security-nodejs`. Lockfiles/CI to `audit-security-repo-posture`.

## Validation Limits

- No process was started; no HTTP/WS was sent; no Electron window was launched.
- No `cargo test` / Playwright. Oracle-integrity clause: test suites were **not** used to close non-findings.
- Provider adapter bodies, Ink TUI, Docusaurus, `documentation/node_modules`, and most renderer React views were not deep-read.
- `GOSLING_ALLOWLIST` live fetch path not traced through `main.ts` in this pass.
- Egress/adversary inspectors sampled via `security/mod.rs` only.
- Runtime races, OOM, and actual metadata-service reachability from Nostr are not Confirmed as manifestations.
- Historical `docs/cloud/audit-security*.md` were not copied.

Stop condition: every SEC-001..015 and CSEC-001..074 item is a finding or explicit non-finding; OWASP A01–A10 and API1–API10 have a verdict.

## Final Confidence

**High** on the quoted code properties (query token, client CSP, Auto allow, opt-in FS scope, Nostr relays, missing sandbox, goslingd exemptions, `!=`, log JSON, mask prefix, optional Origin, unauth bind). **Medium** on exploitability of SSRF and timing compare, because those were not executed.

## Machine-readable findings

```json
[
  {"id":"SEC-GOS-001","title":"ACP secret accepted and constructed as query token","domain":"Security","severity":"High","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling/src/acp/transport/auth.rs","lines":"25-31","quote":"let query_token = request.uri().query().and_then(|query| {"},{"file":"ui/desktop/src/goslingServe.ts","lines":"334-336","quote":"acpUrl.searchParams.set('token', token);"}],"observed":"ACP bearer is valid in ?token= and desktop always puts it there.","expected_boundary":"Header-only or non-URL authenticator.","failure_mechanism":"CSEC-044 secret in URL.","break_it_angle":"Replay ws/http URL from logs without X-Secret-Key.","impact":"Token leak is full ACP takeover.","operational_impact":{"blast_radius":"Service","side_effect_class":"network","reversibility":"compensatable","operator_visibility":"silent","rerun_safety":"unsafe"},"adjacent":["SEC-GOS-007","SEC-GOS-011","SEC-GOS-012"],"mitigation":{"patterns":["header-only authenticator","secret-not-in-query"],"minimal_repair":"Reject query tokens; stop setting searchParams token.","local_guardrail":"401 if URI contains token=","behavior_test":"query-only token is 401"},"implementation_assessment":{"complexity":"local_guardrail","cost":"S","nominal_agent":"codex","rationale":"Two call sites plus existing auth tests.","cost_drivers":["modules","tests"]},"validation":["query-only 401","header-only still works"],"non_goals":["Do not add OAuth in this slice"],"patch_priority":1},
  {"id":"SEC-GOS-002","title":"Live ACP MCP-app guest trusts client CSP","domain":"Security","severity":"High","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling/src/acp/mcp_app_proxy.rs","lines":"284","quote":"let csp = body.csp.unwrap_or_default();"}],"observed":"Guest CSP is the client string; goslingd sibling derives CSP.","expected_boundary":"Server-derived CSP from domain allowlists.","failure_mechanism":"Sibling drift; sandbox policy inversion.","break_it_angle":"POST empty/wildcard CSP with secret; load guest_url.","impact":"MCP-app sandbox escape on loopback.","operational_impact":{"blast_radius":"Workflow","side_effect_class":"user-visible","reversibility":"compensatable","operator_visibility":"silent","rerun_safety":"unsafe"},"mitigation":{"patterns":["server-derived policy"],"minimal_repair":"Copy goslingd build_outer_csp; ignore body.csp."},"implementation_assessment":{"complexity":"local_guardrail","cost":"S","nominal_agent":"codex","rationale":"Safe implementation already exists in gosling-server.","cost_drivers":["modules","tests"]},"validation":["malicious csp not reflected in header"],"patch_priority":2},
  {"id":"SEC-GOS-003","title":"Auto mode allows all tools without a user permission","domain":"Security","severity":"High","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling/src/permission/permission_inspector.rs","lines":"162-166","quote":"\"Auto mode - all tools approved\""}],"observed":"Unlisted tools are Allow in Auto, including shell.","expected_boundary":"Default-deny mutating tools.","failure_mechanism":"Mode used as function-level authorization.","break_it_angle":"Auto session, developer__shell, no yaml row.","impact":"Unattended host-equivalent agency.","operational_impact":{"blast_radius":"Service","side_effect_class":"process","reversibility":"irreversible","operator_visibility":"UI-visible","rerun_safety":"unsafe"},"mitigation":{"patterns":["default-deny mutating tools"],"minimal_repair":"Auto Allow only AlwaysAllow or static read-only list."},"implementation_assessment":{"complexity":"workflow_protocol","cost":"M","nominal_agent":"claude","rationale":"Product-policy plus inspector change.","cost_drivers":["modules","tests","operator_training"]},"validation":["Auto+shell+no row is Deny"],"patch_priority":3},
  {"id":"SEC-GOS-004","title":"Working-dir filesystem boundary is opt-in","domain":"Security","severity":"Medium","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling/src/permission/working_dir_scope_inspector.rs","lines":"45-47","quote":"if !session.restrict_tools_to_working_dirs && session.workspace_context.is_none()"}],"observed":"Ordinary sessions skip path containment.","expected_boundary":"Default-on pinned root.","failure_mechanism":"Strong inspector gated default-off.","break_it_angle":"cat ~/.ssh with restrict off.","impact":"Whole-account FS agency.","operational_impact":{"blast_radius":"Workflow","side_effect_class":"file","reversibility":"irreversible","operator_visibility":"silent","rerun_safety":"unsafe"},"mitigation":{"patterns":["default-on containment"],"minimal_repair":"Default restrict_tools_to_working_dirs true."},"implementation_assessment":{"complexity":"workflow_protocol","cost":"M","nominal_agent":"claude","rationale":"Default change for every new session.","cost_drivers":["modules","tests","operator_training"]},"validation":["new session flags $HOME paths"],"patch_priority":4},
  {"id":"SEC-GOS-005","title":"Nostr import fetches attacker-supplied relay URLs","domain":"Security","severity":"Medium","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling/src/session/nostr_share.rs","lines":"226-236","quote":"let event = fetcher.fetch(event_ref.event_id, &relays).await?;"}],"observed":"Nevent relays used verbatim; no IP allowlist.","expected_boundary":"wss + public resolved IP only.","failure_mechanism":"User URL to connect sink.","break_it_angle":"Deeplink whose relay is loopback/metadata.","impact":"SSRF from desktop/CLI.","operational_impact":{"blast_radius":"Cross-system","side_effect_class":"network","reversibility":"compensatable","operator_visibility":"UI-visible","rerun_safety":"unsafe"},"mitigation":{"patterns":["destination allowlist","resolved-IP deny"],"minimal_repair":"Ignore nevent relays or reuse is_disallowed_ip."},"implementation_assessment":{"complexity":"local_guardrail","cost":"S","nominal_agent":"codex","rationale":"Helper exists in computercontroller.","cost_drivers":["modules","tests"]},"validation":["ws://127.0.0.1 rejected"],"patch_priority":5},
  {"id":"SEC-GOS-006","title":"Desktop main and launcher windows omit sandbox","domain":"Security","severity":"Medium","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"ui/desktop/src/main.ts","lines":"1424-1444","quote":"nodeIntegration: false,","note":"sandbox key absent"},{"file":"ui/desktop/src/shellHost.ts","lines":"38-42","quote":"sandbox: true"}],"observed":"Main/launcher unsandboxed; shell host sandboxed.","expected_boundary":"sandbox true on every window.","failure_mechanism":"Missing defense in depth.","break_it_angle":"Renderer XSS uses preload/IPC.","impact":"XSS inherits grant+token privileges.","operational_impact":{"blast_radius":"Local","side_effect_class":"process","reversibility":"compensatable","operator_visibility":"silent","rerun_safety":"unknown"},"mitigation":{"patterns":["Electron sandbox"],"minimal_repair":"Set sandbox true on main and launcher."},"implementation_assessment":{"complexity":"local_guardrail","cost":"S","nominal_agent":"codex","rationale":"Sibling window already sandboxed.","cost_drivers":["modules","tests"]},"validation":["window options include sandbox true"],"patch_priority":6},
  {"id":"SEC-GOS-007","title":"goslingd leaves MCP-app routes unauthenticated","domain":"Security","severity":"Medium","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling-server/src/auth.rs","lines":"16-20","quote":"|| request.uri().path() == \"/mcp-app-proxy\""}],"observed":"GET proxy and guest skip check_token.","expected_boundary":"Same auth or loopback ConnectInfo.","failure_mechanism":"Auth exemption list too broad.","break_it_angle":"Unauth GET /mcp-app-proxy.","impact":"Unauth HTML sink; worse if host public.","operational_impact":{"blast_radius":"Service","side_effect_class":"network","reversibility":"compensatable","operator_visibility":"log-only","rerun_safety":"safe"},"mitigation":{"patterns":["authenticate non-liveness routes"],"minimal_repair":"Remove MCP-app exemptions."},"implementation_assessment":{"complexity":"local_guardrail","cost":"S","nominal_agent":"codex","rationale":"One middleware condition.","cost_drivers":["modules","tests"]},"validation":["unauth proxy denied"],"patch_priority":7},
  {"id":"SEC-GOS-008","title":"ACP guest store uses non-constant-time secret compare","domain":"Security","severity":"Low","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling/src/acp/mcp_app_proxy.rs","lines":"272","quote":"if body.secret != state.secret_key {"}],"observed":"!= on serve secret.","expected_boundary":"token_matches / ct_eq.","failure_mechanism":"CSEC-049.","break_it_angle":"Timing POST bodies.","impact":"Theoretical local secret recovery.","operational_impact":{"blast_radius":"Local","side_effect_class":"none","reversibility":"compensatable","operator_visibility":"silent","rerun_safety":"safe"},"mitigation":{"patterns":["constant-time compare"],"minimal_repair":"Use token_matches."},"implementation_assessment":{"complexity":"local_guardrail","cost":"XS","nominal_agent":"codex","rationale":"One line.","cost_drivers":["modules","tests"]},"validation":["wrong secret 401"],"patch_priority":8},
  {"id":"SEC-GOS-009","title":"Prompt-injection scanner logs full tool-call JSON","domain":"Security","severity":"Medium","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling/src/security/mod.rs","lines":"160-175","quote":"tool.call_json = %tool_call_json,"}],"observed":"Raw tool args including secrets go to tracing/logs.","expected_boundary":"Name/id/decision only.","failure_mechanism":"CSEC-043.","break_it_angle":"Tool args contain a key; grep logs.","impact":"Secrets persist on disk.","operational_impact":{"blast_radius":"Repo","side_effect_class":"file","reversibility":"compensatable","operator_visibility":"log-only","rerun_safety":"unsafe"},"mitigation":{"patterns":["structured redaction"],"minimal_repair":"Drop tool.call_json field."},"implementation_assessment":{"complexity":"local_guardrail","cost":"S","nominal_agent":"codex","rationale":"Three log branches.","cost_drivers":["modules","tests"]},"validation":["sentinel absent from logs"],"patch_priority":9},
  {"id":"SEC-GOS-010","title":"Config read mask reveals first 8 secret characters","domain":"Security","severity":"Medium","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling-server/src/routes/config_management.rs","lines":"226-239","quote":"const SECRET_MASK_SHOW_LEN: usize = 8;"}],"observed":"Up to 8 leading secret chars returned.","expected_boundary":"configured boolean or last-4.","failure_mechanism":"Prefix leak.","break_it_angle":"POST /config/read is_secret true.","impact":"Key-type identification and reduced search space.","operational_impact":{"blast_radius":"Service","side_effect_class":"network","reversibility":"compensatable","operator_visibility":"UI-visible","rerun_safety":"safe"},"mitigation":{"patterns":["no secret material in API"],"minimal_repair":"Return configured flag only."},"implementation_assessment":{"complexity":"local_guardrail","cost":"XS","nominal_agent":"codex","rationale":"One function.","cost_drivers":["modules","tests"]},"validation":["response lacks first 8 of fixture"],"patch_priority":10},
  {"id":"SEC-GOS-011","title":"WebSocket origin check skipped when Origin is absent","domain":"Security","severity":"Medium","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling/src/acp/transport/mod.rs","lines":"122-127","quote":"if let Some(origin) = request.headers().get(header::ORIGIN) {"}],"observed":"Missing Origin skips allowlist.","expected_boundary":"Fail-closed Origin or required token.","failure_mechanism":"Optional origin check.","break_it_angle":"WS upgrade no Origin on unauth serve.","impact":"Origin policy not a WS boundary.","operational_impact":{"blast_radius":"Service","side_effect_class":"network","reversibility":"compensatable","operator_visibility":"silent","rerun_safety":"unknown"},"mitigation":{"patterns":["fail-closed Origin"],"minimal_repair":"Reject missing Origin unless token present and bind loopback."},"implementation_assessment":{"complexity":"local_guardrail","cost":"S","nominal_agent":"codex","rationale":"One branch; tests exist.","cost_drivers":["modules","tests"]},"validation":["no Origin + no token is 401/403"],"patch_priority":11},
  {"id":"SEC-GOS-012","title":"Unauthenticated serve does not force loopback bind","domain":"Security","severity":"High","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling-cli/src/cli.rs","lines":"1507-1511","quote":"the ACP endpoint will accept unauthenticated connections"}],"observed":"Dangerous flag plus --host 0.0.0.0 is allowed.","expected_boundary":"Unauth implies loopback or refuse.","failure_mechanism":"Deployment assumption not enforced.","break_it_angle":"serve --dangerously-unauthenticated --host 0.0.0.0.","impact":"Remote unauthenticated ACP.","operational_impact":{"blast_radius":"Service","side_effect_class":"network","reversibility":"compensatable","operator_visibility":"log-only","rerun_safety":"unsafe"},"mitigation":{"patterns":["fail-closed bind"],"minimal_repair":"Bail if !require_token && !loopback."},"implementation_assessment":{"complexity":"local_guardrail","cost":"S","nominal_agent":"codex","rationale":"Startup check.","cost_drivers":["modules","tests"]},"validation":["non-loopback unauth exits non-zero"],"patch_priority":12},
  {"id":"SEC-GOS-013","title":"SmartApprove treats LLM read-only as Allow","domain":"Security","severity":"High","confidence":"Confirmed","evidence_basis":"source-evidenced","evidence":[{"file":"crates/gosling/src/permission/permission_inspector.rs","lines":"237-243","quote":"InspectionAction::Allow"},{"file":"crates/gosling/src/permission/permission_inspector.rs","lines":"244-246","quote":"\"LLM detected as read-only\""}],"observed":"Judge-listed tools auto-run.","expected_boundary":"LLM may only tighten policy.","failure_mechanism":"Untrusted classifier is authorization oracle.","break_it_angle":"Judge returns developer__shell as read-only.","impact":"Authz bypass via injection or misclassify.","operational_impact":{"blast_radius":"Workflow","side_effect_class":"process","reversibility":"irreversible","operator_visibility":"UI-visible","rerun_safety":"unsafe"},"mitigation":{"patterns":["classifier can only deny"],"minimal_repair":"Never Allow from judge; static read-only list only."},"implementation_assessment":{"complexity":"workflow_protocol","cost":"M","nominal_agent":"multi-agent","rationale":"Crosses product policy and LLM prompts.","cost_drivers":["modules","tests","operator_training"]},"validation":["judge-blessed shell is not Allow"],"patch_priority":13}
]
```
