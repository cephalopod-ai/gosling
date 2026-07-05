# Audit Lens Report — Code-Level Security Flaws (`audit-security-code`)

Lens domain prefix: `SECC-GSL-NNN`. Authority: **audit-only / read-only** (Phase 0).
Builds on `docs/cloud/00-orientation.md`. Base contracts obeyed: `audit_method.md v3.0`,
`evidence_discipline.md`, `severity_matrix.md`, `finding_format.md`.

## Draft-prompt note

The supplied lens prompt targets Python/Node/SQLite defect classes (CSEC taxonomy).
gosling is Rust; I mapped each class to the Rust reality and recorded where the
assumption does not transfer (memory-safe serde instead of pickle; sqlx bound
parameters instead of f-string SQL; argv-array `Command` spawning instead of
`shell=True`). I preserved the intended mission — find reachable
injection/deserialization/path/SSRF/TLS/secret flaws — and expanded to the local
HTTP server trust boundary, which is the most exposed code-level surface here.

## Effort budget

~30 tool calls, prioritized on the prompt's named surfaces: process spawning
(`execute_commands.rs`, `shell.rs`, `extension_manager.rs`, provider ACP launch),
path traversal (`hints/import_files.rs`, session persistence), deserialization
(`session/import_formats/`), SSRF/remote MCP, `config/tls.rs`, `unsafe` blocks,
secrets-in-logs. Surfaces sampled and not deep-read are listed under Validation
Limits. Stop condition reached: named surfaces are each a finding or explicit
non-finding; budget exhausted.

---

## Findings

### SECC-GSL-001: Server auth secret transmitted in URL query string

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security (CSEC-044)

Evidence:
- `crates/gosling-server/src/routes/mcp_app_proxy.rs:25-26` — `struct ProxyQuery { secret: String, ... }` (secret is a query parameter).
- `crates/gosling-server/src/routes/mcp_app_proxy.rs:161` — `if !token_matches(Some(params.secret.as_str()), &state.secret_key)`; also `:203` (JSON body) and `:259` (`GuestQuery.secret` query param).
- `crates/gosling-server/src/routes/mcp_app_proxy.rs:304-314` — `routes(secret_key)` is fed the **same** `secret_key` used for the whole server (`crates/gosling-server/src/commands/agent.rs:50-51,74-80`), so this is the master server credential, not a scoped token.
- `crates/gosling-server/src/auth.rs:15-20` — `/mcp-app-proxy` and `/mcp-app-guest` are exempted from the header-based `X-Secret-Key` middleware, so the query-string secret is their only auth.

Observed behavior:
- The full server secret_key is passed as `?secret=<key>` on GET `/mcp-app-proxy` and GET `/mcp-app-guest`. Secrets in URLs are persisted in HTTP server access logs, shell/process history, proxy logs, and browser history.

Expected boundary:
- A security token should travel in a header or request body, not the request-target, so it is not captured by URL-logging intermediaries (CSEC-044).

Failure mechanism:
- The MCP-apps proxy predates/bypasses the header middleware and reuses the master secret as a query parameter for iframe `src=` convenience (a URL cannot carry a custom header).

Break-it angle:
- Any component that logs request lines (reverse proxy, `RUST_LOG` access logging, OS process listing when a browser is launched with the URL as an arg) now holds the master credential; one leaked log line authenticates all header-protected REST routes too.

Impact:
- Master credential disclosure through side channels. Mitigated by `referrer-policy: no-referrer` on the proxy page (`:187-190`) and localhost binding, but not by log redaction.

Operational impact:
- Blast radius: Service (grants full REST/ACP access if captured)
- Side-effect class: network / user-visible (URL)
- Reversibility: irreversible (once logged)
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- SECC-GSL-003 (CORS Any lets a browser origin read responses if it learns the secret).

Recommended mitigation:
- Mint a short-lived, path-scoped nonce for the MCP-apps iframe flow instead of reusing the master secret; keep the master secret header-only. Minimal repair: separate proxy token from `secret_key`.
- Behavior test: assert the master `X-Secret-Key` value never appears in any query-string-accepting handler, and that a proxy nonce cannot authenticate a REST route.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: modules (proxy + desktop caller), tests
- Nominal implementation agent: claude
- Rationale: touches the Rust proxy and the Electron caller that builds the iframe URL.

Validation:
- Grep/test proving no handler binds the master secret from `Query<..>`; positive test that iframe flow still renders with a scoped nonce.

Non-goals:
- Do not change the REST header-auth scheme in this slice.

---

### SECC-GSL-002: OSV malware pre-spawn check is fail-open and narrowly scoped

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security (CSEC-030, defense-in-depth)

Evidence:
- `crates/gosling/src/agents/extension_malware_check.rs:48-56` — only `uvx`→PyPI and `npx`→npm are checked; every other `cmd` returns `Ok(())` (`"skipping OSV check (fail open)"`).
- `:82-88` — only the **first** non-flag arg is parsed as the package; extra packages / `--with <pkg>` chains are unchecked.
- `:211-233` — any network error, non-2xx, or JSON parse error `return Ok(())` ("failing open").
- Call site: `crates/gosling/src/agents/extension_manager.rs:1084` — `deny_if_malicious_cmd_args(cmd, args)` guards the Stdio-extension spawn at `:1102-1107`.

Observed behavior:
- The advertised malicious-package block only fires for a single pinned npm/PyPI package name reachable over the network at spawn time; anything else (a raw binary, a git-URL install, a second `--with` dependency, or an OSV outage) is allowed.

Expected boundary:
- A control gating process spawn should fail closed or clearly scope its guarantee; a fail-open network check must not be relied on as the boundary.

Failure mechanism:
- Best-effort reputation check treated as a gate. `InlinePython` deps (`extension_manager.rs:1133-1138`) pass `--with <dep>` to `uvx` and are **not** routed through `deny_if_malicious_cmd_args` at all.

Break-it angle:
- Block OSV (offline, DNS, 500) → check passes; or name the malicious package as the second `--with` dep → unchecked.

Impact:
- Overstated protection; malicious extension packages can still be spawned. Real severity depends on whether extension configs are operator-authored (they are — see non-finding N4), so this is defense-in-depth erosion, not a direct RCE path.

Operational impact:
- Blast radius: Local (workstation)
- Side-effect class: process
- Reversibility: irreversible (code executes)
- Operator visibility: log-only (`debug!`)
- Rerun safety: unsafe

Recommended mitigation:
- Document the check as advisory-only, route `InlinePython`/`--with` deps through it, and add a config to fail-closed on OSV unavailability for untrusted sources.
- Behavior test: a config with a MAL-flagged dep in a second `--with` is blocked; an OSV timeout under a fail-closed flag denies the spawn.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex

Non-goals:
- Do not add a full SBOM scanner in this slice.

---

### SECC-GSL-003: Local server uses permissive CORS (`allow_origin(Any)` + `allow_headers(Any)`)

Severity: Low
Confidence: Likely
Evidence basis: source-evidenced
Domain: Security (CSEC-051-adjacent / browser trust boundary)

Evidence:
- `crates/gosling-server/src/commands/agent.rs:56-59` — `CorsLayer::new().allow_origin(Any).allow_methods(Any).allow_headers(Any)`.
- `crates/gosling-server/src/auth.rs:15-19` — `/status`, `/mcp-app-proxy`, `/mcp-app-guest` bypass the header auth entirely.
- Bind default is loopback: `crates/gosling-server/src/configuration.rs:75,93` (`"127.0.0.1"`), which bounds but does not eliminate the browser-origin reach.

Observed behavior:
- Any web origin the user visits can issue cross-origin reads against the loopback goslingd. Authenticated REST routes still require the secret (not credentialed via cookies), so the practical exposure is: unauthenticated `/status` is world-readable to any page, and DNS-rebinding could reach localhost.

Expected boundary:
- A localhost control server should restrict `Origin` to its known desktop/UI origin(s) rather than `Any`, defending against DNS-rebind and malicious-tab probing.

Failure mechanism:
- Blanket `Any` CORS chosen for desktop/dev convenience.

Break-it angle:
- A malicious page fetches `http://127.0.0.1:<port>/status`; with rebinding, probes other routes (blocked without the secret, but the attack surface is enumerable).

Impact:
- Low: information exposure of `/status`; no state mutation without the secret.

Operational impact:
- Blast radius: Local
- Side-effect class: network
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Recommended mitigation:
- Restrict `allow_origin` to the desktop app origin(s); add a `Host`/`Origin` allowlist guard for rebind defense.
- Behavior test: a cross-origin request from an unlisted origin is rejected by CORS; `/status` from an unknown origin is not readable.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Nominal implementation agent: codex

Non-goals:
- Do not add auth to `/status` in this slice if it must stay a liveness probe.

---

### SECC-GSL-004: CSP domain lists interpolated into policy without separator validation

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security (CSEC-013-adjacent / header-value injection)

Evidence:
- `crates/gosling-server/src/routes/mcp_app_proxy.rs:69-121` — `build_outer_csp` string-joins caller-supplied domains directly into the CSP (`connect-src 'self'{connections}; ...`).
- `:124-133` — `parse_domains` splits on `,` and trims but does not reject `;`, spaces inside a token, or CSP keywords.
- Ceiling: for the header path, `:281-292` rejects a CSP with invalid `HeaderValue` characters (blocks CR/LF); the inline-template path `:182` performs raw `{{OUTER_CSP}}` replacement with no such check.

Observed behavior:
- A domain token containing `;` (e.g. `evil.com; script-src *`) is spliced into the CSP, injecting/loosening directives. Guarded by the master secret, and the served guest HTML is already fully attacker-controlled, so this weakens a ceiling over content the caller already controls.

Expected boundary:
- Domain inputs to a security policy must be validated as host tokens (allowlist charset) before interpolation.

Failure mechanism:
- Trusted-caller assumption on the desktop→proxy channel; no host-token validation.

Impact:
- Low: CSP hardening bypass on an authenticated, already-attacker-influenced surface.

Operational impact:
- Blast radius: Local; Side-effect class: user-visible; Reversibility: reversible; Operator visibility: silent; Rerun safety: safe.

Recommended mitigation:
- Validate each domain against a host/scheme allowlist charset; reject tokens containing `;`, whitespace, or CSP keywords.
- Behavior test: a `;`-bearing domain is rejected before reaching `build_outer_csp`.

Implementation assessment:
- Complexity: local_guardrail; Cost: XS; Nominal agent: codex.

Non-goals:
- Do not redesign the MCP-apps CSP model here.

---

## Cross-lens escalation (Skill Escalation table)

| Observation | Evidence | Route to lens |
|---|---|---|
| `--dangerously-skip-permissions` passed to Claude Code CLI in `GoslingMode::Auto`; `--yolo`/`--full-auto` to Codex | `crates/gosling/src/providers/claude_code.rs:396-399`, `crates/gosling/src/providers/codex.rs:105-117` | permission / multiagent-consensus lens: the spawned sub-agent CLI runs with its own confirmations disabled when gosling is in Auto; verify gosling's own permission gate fully compensates. |
| `manage_extensions` tool lets the model enable preconfigured extensions by name | `crates/gosling/src/agents/platform_extensions/ext_manager.rs:145-192` | security-llm lens: model can activate an operator-configured (possibly powerful) extension mid-session; excessive-agency review. |
| OSV fail-open + `InlinePython --with` deps unchecked | SECC-GSL-002 | dependency-criticality lens. |
| Master secret in URL (SECC-GSL-001) reused as sole proxy auth | SECC-GSL-001 | security-repo-posture / operator-signal (log redaction). |

---

## Non-findings (checked and held)

- **N1 — Shell tool is arbitrary-execution by design, not an injection defect.**
  `crates/gosling/src/agents/platform_extensions/developer/shell.rs:635-662` runs
  `sh/bash -c "<command>"` (Unix) / `cmd /C` (Windows). The command string is
  meant to be arbitrary; the trust boundary is permission gating (other lens), so
  CSEC-015 "command injection" does not apply — there is no *separate* trusted
  command being subverted. Recorded honestly as design surface, not a flaw.

- **N2 — Extension / ACP-provider subprocesses use argv arrays, no shell.**
  `extension_manager.rs:1093-1107` (`Command::new(cmd).args(args)`),
  `providers/claude_code.rs:376-425` and `providers/codex.rs:151-197` all pass
  args as discrete `.arg(...)` values. No string-built command line, so argument
  injection cannot add flags/operators. (Windows `cmd /C` uses `raw_arg`, expected.)

- **N3 — Process-spawn sink is not reachable with attacker-controlled argv from
  model output.** The only model-facing entry, `manage_extensions`
  (`ext_manager.rs:176-191`), resolves the config via `get_extension_by_name`
  from the operator-configured set; the model cannot supply `cmd`/`args`. So
  `cmd`/`args` provenance is operator config, not untrusted LLM/tool content.

- **N4 — No SQL injection.** Session storage uses sqlx with bound parameters
  throughout (`session_manager.rs:1573,1621,1630,...`). Dynamically-built SQL
  (`session_manager.rs:1699-1738`) interpolates only fixed column names and
  `?`-placeholder counts; every user value is `.bind()`-ed. `ALTER TABLE ... {column}`
  at `:1307,3681` interpolates from a hardcoded array literal (`:1293-1298`).

- **N5 — No path traversal via imported/foreign `session_id`.** Sessions are keyed
  in SQLite by `session_id` as a bound parameter, not used to build a filesystem
  path; imports (`session/import_formats/mod.rs`) convert to native JSON and hand
  off to the SQLite pipeline. `session_id` never reaches a `join()`/filename.

- **N6 — `.goslinghints` file-reference expansion contains path traversal.**
  `crates/gosling/src/hints/import_files.rs:15-49`: absolute paths rejected
  (`:20-25`), resolved path `canonicalize()`d and required to
  `starts_with(boundary_canonical)` (`:34-44`); non-existent paths fall through
  but are then rejected by `is_file()` (`:93`). Integration test at `:433-486`
  proves `@../etc/passwd` and absolute refs are not expanded.

- **N7 — Server secret comparison is constant-time.**
  `crates/gosling/src/acp/transport/auth.rs:9-13` uses
  `subtle::ConstantTimeEq::ct_eq`. Not CSEC-049. Default secret is a random
  32 bytes when unset (`commands/agent.rs:50-51`).

- **N8 — TLS verification is not disabled in production.** `config/tls.rs` only
  *adds* client cert / CA (`with_client_cert_and_key`, `with_ca_cert`); there is
  no `danger_accept_invalid_certs` path in shipped code — the only occurrence is a
  test (`crates/gosling-server/tests/tls_test.rs:94`). Not CSEC-055.

- **N9 — Deserialization is memory-safe.** All untrusted parsing (sessions,
  imports, extension config, MCP payloads) is serde_json/serde — no
  pickle/marshal/`yaml.load`/`eval` equivalent (CSEC-026/027/028 do not map to
  this Rust code). The residual risk from importing hostile sessions is
  prompt-injection content, which belongs to the security-llm lens.

- **N10 — `unsafe` blocks are benign.** `crates/gosling/src/subprocess.rs:6-23`
  (`libc::prctl(PR_SET_PDEATHSIG)` + `pre_exec`, standard parent-death handling)
  and `crates/gosling/src/plugins/discovery.rs:349-353` (test-only
  `std::env::set_var`). No raw-pointer / transmute / FFI-buffer memory-safety
  concern found in the sampled set.

- **N11 — No secret values written to logs in the sampled set.** The
  `tracing::*` sites touching `token`/`secret`/`api_key`
  (providers/oauth/*, `config/base.rs`) log status/lifecycle strings
  ("token refreshed", "secrets cache miss"), not the values themselves.

- **N12 — Remote MCP connect is not an open SSRF sink.** `StreamableHttp` URIs
  (`extension_manager.rs:954-984`) come from operator extension config with
  `substitute_env_vars` (`:569-591`, brace/`$VAR` substitution from the env map,
  no shell); the model cannot inject an arbitrary URL (see N3). `mcp_app_proxy`
  does not fetch arbitrary URLs — it stores/serves caller-provided HTML only.

---

## Validation Limits (not reviewed / sampled only)

- `crates/gosling-mcp/src/computercontroller/platform/{macos,windows,linux}.rs` —
  process spawning for the computer-controller extension: **not deep-read**.
- `crates/gosling/src/oauth/` (device/callback flow, `oauth_callback.html`) —
  open-redirect / state-parameter handling **not reviewed**.
- `crates/gosling/src/session/nostr_share.rs` — network session sharing
  (provenance, key handling) **not reviewed**.
- `crates/gosling/src/providers/{cursor_agent,gemini_cli,githubcopilot}.rs` and
  their subprocess/OAuth details — **sampled, not deep-read**.
- `crates/gosling-server/src/routes/{config_management,dictation,telemetry,
  session_events}.rs` — write paths behind header auth **not deep-read**; the
  header-auth middleware was confirmed but per-route input validation was not.
- `crates/gosling-mcp/src/subprocess.rs` and `peekaboo` — **not reviewed**.
- Runtime behavior (actual SSRF reach, DNS-rebind exploitability, log capture of
  the URL secret) is **static-only**; per `confidence_calibration.md`, runtime
  manifestation of SECC-GSL-003 is capped at Likely.

## CSEC coverage ledger

Injection (011-015): SQLi none (N4); command-injection none — argv arrays (N2/N3),
shell-tool by-design (N1). Deserialization (026-030): N/A in Rust (N9); OSV
network-config trust weak → SECC-GSL-002. Path/file (033-037): traversal held
(N5/N6). Secrets/logs (041-045): secret-in-URL → SECC-GSL-001; no log leakage
(N11). Auth/token (047-052): constant-time compare (N7). Crypto/TLS (055-057): no
verification-disable (N8). SSRF (061-063): not reachable (N12). Header/policy
injection: SECC-GSL-004. CORS/browser boundary: SECC-GSL-003. `unsafe`: benign (N10).
