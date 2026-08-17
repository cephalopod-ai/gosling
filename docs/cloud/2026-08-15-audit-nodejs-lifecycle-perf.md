# Combined Audit — Node.js Security, Architecture, Memory, Resource, Performance

**Date:** 2026-08-15  
**Target:** `/Users/eric/Work/vscode/forked/gosling`  
**Branch:** `main`  
**HEAD:** `073d19428509ea6eb317924b1856a1fe7e9002c8`  
**Authority:** `read_only` — source not modified except this report.  
**Lenses:** `audit-security-nodejs`, `audit-architecture-nodejs` (limited to `ui/desktop` + `ui/text`), `audit-memory-lifecycle`, `audit-resource-lifecycle`, `audit-performance-profile`.

The supplied prompt is treated as a draft. The intended mission is preserved (Electron fuses/contextIsolation, subprocess/MCP handle leaks, session/context unbounded growth, README performance claims). Review was expanded to adjacent seams implied by those surfaces: MCP-app iframe + proxy-secret, CSP lease-key mismatch, renderer file-grant roots, ACP WebSocket buffer bounds, and Ink TUI spawn/teardown.

## Executive Verdict

No Critical finding is Confirmed at this HEAD. The Electron desktop shell is materially hardened: `contextIsolation: true`, `nodeIntegration: false`, Electron Fuses are set on the packaged path, renderer filesystem IPC is root-confined, `gosling serve` is spawned `shell: false` with parent-PID supervision and a process registry, and several memory/resource bounds that README claims (token LRU, event-bus replay, subprocess stderr cap) exist in current source.

**Re-assessed 2026-08-16: SECN-GSL-001 is downgraded to Warning /
not-actionable — see its detail section. The paragraph below is the original
2026-08-15 assessment and its taint-path premise does not hold, because the
untrusted app HTML runs in a nested guest iframe on a different origin, not in
the secret-bearing frame.**

The highest-severity live defect is **SECN-GSL-001** (High, Confirmed code property): MCP Apps load untrusted HTML in an iframe whose default sandbox is `allow-scripts allow-same-origin allow-forms` and whose `src` is the loopback proxy URL with the backend secret in the fragment. That is a taint path from untrusted MCP content to the local control-plane secret. Do not treat it as an exploited break-in; the secret-in-URL and sandbox tokens are source-evidenced. XSS-to-API use was not reproduced.

Do not pause merge for the remaining items. They are real missing bounds (session/digest caches, FileMemory slurp, chat-store retention, `check-ollama` child lifetime) plus a documentation/measurement honesty gap: README footprint numbers are explicitly historical (2026-07-04 / gosling v0.0.5) and were not remeasured at this HEAD. No race, OOM, lock storm, or timeout was marked Confirmed.

Patching is recommended for the session/digest unbounded maps (SECN-GSL-001 was re-assessed 2026-08-16 as not actionable); do not optimize README numbers until a same-harness remasurement exists.

## Scope

- Repository/project / branch / commit: gosling, `main`, `073d19428509ea6eb317924b1856a1fe7e9002c8`
- Prompt or session log reviewed: `docs/cloud/2026-08-15-orientation.md`; prior `docs/cloud/audit-*.md` used as seeds only, not as current evidence
- Skills (lenses) invoked: `audit-security-nodejs`, `audit-architecture-nodejs` (limited), `audit-memory-lifecycle`, `audit-resource-lifecycle`, `audit-performance-profile`
- Files/directories inspected: `ui/desktop/src/{main,preload,goslingServe,backendProcessRegistry,goslingServeLeaseRegistry,shellHost}.ts`, `ui/desktop/src/acp/*`, `ui/desktop/src/components/McpApps/*`, `ui/desktop/src/utils/{csp,rendererFileAccess,rendererDirectoryGrants,sessionImport,urlSecurity}.ts`, `ui/desktop/forge.config.ts`, `ui/desktop/package.json`, `ui/text/src/{tui,slashCommands}.tsx`, `crates/gosling/src/{subprocess,token_counter,execution/manager,context_mgmt/*,agents/{agent,extension_manager}}.rs`, `crates/gosling-server/src/{commands/agent,session_event_bus}.rs`, `README.md`
- Commands/tests run: none (read-only static). No heap snapshots, no `ps`/`lsof` census, no benchmark rerun.
- Effort budget (per-lens) and what it bought:
  - SECN: ~35 files — Electron window prefs, fuses, preload IPC, file/spawn sinks, MCP iframe, CSP, allowlist fetch
  - ARCN: ~25 files — Electron/Ink layer map, env reads, cycles, async ownership, caches
  - MEM: ~20 files — session store, digest cache, memory file slurp, WS buffers, TUI turns
  - RES: ~15 files — serve spawn/cleanup, MCP `configure_subprocess`, TUI kill, ollama probe, parent-PID wait
  - PERF: README + `tests/e2e/performance.spec.ts` + claimed Rust hot paths (ConfigSnapshot, token LRU, event bus)
- Constraints: no source edits; no installs; no runtime measurement; architecture lens excluded Rust backend structure except where memory/resource/perf required it.

### Node applicability

`ui/desktop` is a material Electron 41 / Node 24 app (`ui/desktop/package.json`). `ui/text` is an Ink CLI that spawns `gosling acp`. There is **no Node HTTP backend**. `express`/`cors` appear in desktop `package.json` but have **no `src/` importer**. Architecture review is therefore Electron main/preload/renderer + Ink CLI, not Express/Nest.

### Heap / RSS limits

No `--max-old-space-size`, cgroup `memory.max`, or Rust allocator cap was found on the desktop launch path. Growth is bounded only by host RAM. RSS was not decomposed (no process was run).

## Draft Prompt Assessment

- Intended mission: Electron hardening, MCP/subprocess leaks, session/context growth, README claim hygiene.
- Under-specified: whether Rust session/MCP lifetime is in scope. Included because the stated focus names those objects and they are not Node-only.
- Overly narrow if limited to `sandbox:` string presence: Electron 41 defaults sandbox on; the live defect is MCP iframe + secret, not a missing main-window flag.
- Added angles: producer/consumer of ACP lease IDs (window vs webContents), sibling shell vs main `webPreferences`, README claim vs current source existence (not current numbers).
- Assumption challenged: “missing `sandbox: true` is a High fuse/isolation failure.” It is not, given Electron ≥20 default and no `sandbox: false` / `no-sandbox` switch.

## Surface Inventory

| Surface | Actor | Input/Trigger | State/Output | Boundary | Reviewed |
|---|---|---|---|---|---|
| Electron main window | local user | menu/IPC/deep link | BrowserWindow + ACP lease | contextIsolation, grants, CSP | Yes |
| Launcher window | local user | global shortcut | small BrowserWindow | same preload, no explicit sandbox | Yes |
| Shell host window | shell profile | shell bootstrap | sandboxed window + serve | explicit `sandbox: true` | Yes |
| Preload `window.electron` | renderer | IPC invoke/send | fs/notify/settings/ACP URL | contextBridge allowlist | Yes |
| `gosling serve` child | main | window create | detached process group | registry + PARENT_PID + cleanup | Yes |
| MCP stdio children | Rust agent | extension load | Tokio child | `kill_on_drop` + Linux PDEATHSIG | Yes |
| MCP App iframe | untrusted HTML | tool resource | sandboxed iframe + proxy | iframe sandbox + secret hash | Yes |
| ACP WebSocket | renderer | `getAcpUrl` | session snapshots | 8 MiB / 1024-msg buffer | Yes |
| Chat session store | renderer | load/switch/delete | `sessionsById` Map | delete only on session delete | Yes |
| Ink TUI | CLI user | `gosling-tui` | turns[] + acp child | `process.kill` on exit | Yes |
| README claims | operators | docs | footprint table | historical 2026-07-04 numbers | Yes |

## Boundary Map

| Surface | Intended Boundary | Enforced At | Status |
|---|---|---|---|
| Renderer Node access | no Node in renderer | `nodeIntegration: false` + `contextIsolation: true` + contextBridge | Held |
| Packaged Electron hardening | fuses disable RunAsNode / NODE_OPTIONS / inspect | `forge.config.ts` FusesPlugin | Held on packaged path |
| Renderer filesystem | only granted directory roots | `assertPathWithinRoots` + grant registry | Held |
| Backend spawn | argv array, no shell | `goslingServe.ts` `shell: false` | Held |
| Backend orphan | parent death kills serve | `GOSLING_SERVER__PARENT_PID` + poll | Held (Potential on poll gap) |
| MCP child lifetime | drop/kill on eviction | `configure_subprocess` `kill_on_drop` | Held; macOS APD residual |
| MCP App isolation | untrusted UI cannot take host secret | iframe sandbox + proxy | **Held** — SECN-GSL-001 re-assessed 2026-08-16; guest runs on a separate origin |
| CSP connect-src for local ACP | only this window’s loopback origin | `buildCSP` + lease lookup | **Failed** (ARCN-GSL-001) |
| Session heap | evict unused snapshots | `deleteSnapshot` | **Failed** (MEM-GSL-003) |
| Digest cache | bound or TTL | `DIGEST_CACHE` HashMap | **Failed** (MEM-GSL-001) |
| README perf numbers | current measured HEAD | README table + disclaimer | **Failed honesty** (PERF-GSL-001) |

## Findings Table

| ID | Severity | Confidence | Evidence Basis | Domain | Title | Patch Priority | Blast Radius | Complexity | Cost | Nominal Agent |
|---|---|---|---|---|---|---|---|---|---|---|
| SECN-GSL-001 | ~~High~~ Warning (re-assessed 2026-08-16, not actionable) | Confirmed (secret+sandbox tokens); runtime exfil Likely | source-evidenced | Security | MCP App iframe gets backend secret in URL hash under `allow-scripts allow-same-origin` | 1 | Local | workflow_protocol | M | claude |
| ARCN-GSL-001 | Medium | Confirmed (wrong key); WS-block Likely | source-evidenced | Architecture | CSP rebuild looks up ACP lease by `webContentsId`, leases are keyed by `BrowserWindow.id` | 2 | Workflow | local_guardrail | S | codex |
| MEM-GSL-001 | Medium | Likely (Potential; not measured) | source-evidenced | Memory Lifecycle | Process-lifetime `DIGEST_CACHE` HashMap has no cap/TTL/eviction | 3 | Service | local_guardrail | S | codex |
| MEM-GSL-003 | Medium | Likely (Potential; not measured) | source-evidenced | Memory Lifecycle | Renderer `sessionsById` retains full message lists until explicit delete | 3 | Workflow | local_guardrail | S | codex |
| MEM-GSL-002 | Medium | Confirmed (slurp property); OOM Likely | source-evidenced | Memory Lifecycle | `FileMemorySource` reads entire `memories.jsonl` every retrieve | 4 | Service | local_guardrail | S | codex |
| PERF-GSL-001 | Medium | Confirmed | source-evidenced | Performance | README footprint table is a 2026-07-04 / v0.0.5 historical run, not this HEAD | 5 | Repo | operator_ux | XS | human-owner |
| SECN-GSL-002 | Medium | Confirmed (unbounded fetch); SSRF reachability operator-only | source-evidenced | Security | `GOSLING_ALLOWLIST` `fetch` + `response.text()` has no size/timeout/host allowlist | 6 | Local | local_guardrail | S | codex |
| RES-GSL-001 | Low | Likely (Potential) | source-evidenced | Resource Lifecycle | `check-ollama` `ps \| grep` has no timeout and no `finally` kill | 7 | Local | local_guardrail | XS | codex |
| MEM-GSL-005 | Low | Likely (Potential) | source-evidenced | Memory Lifecycle | `toolsCache` has `clearToolsCache` but no production caller | 7 | Workflow | local_guardrail | XS | codex |
| ARCN-GSL-002 | Low | Confirmed | source-evidenced | Architecture | `process.env` reads are scattered (main, serve, updater, winShims) | 8 | Repo | workflow_protocol | M | claude |
| MEM-GSL-004 | Low | Likely (Potential) | source-evidenced | Memory Lifecycle | Ink TUI `turns` array grows for the whole process | 8 | Local | local_guardrail | S | codex |
| MEM-GSL-006 | Low | Confirmed (clone exists); cost unmeasured | source-evidenced | Memory Lifecycle | Per-turn `conversation.messages().clone()` duplicates the live conversation | 8 | Service | local_guardrail | S | gpt |
| MEM-GSL-008 | Low | Confirmed (no size cap) | source-evidenced | Memory Lifecycle | `read-file` IPC `readFile`s the whole granted path | 8 | Local | local_guardrail | XS | codex |
| MEM-GSL-009 | Low | Likely (Potential) | source-evidenced | Memory Lifecycle | Artifact list paginates but concatenates every page in memory | 9 | Workflow | local_guardrail | XS | codex |
| RES-GSL-002 | Low | Likely (Potential) | source-evidenced | Resource Lifecycle | TUI `cleanup()` `kill()`s the ACP child without `wait`/`SIGKILL` | 9 | Local | local_guardrail | XS | codex |
| SECN-GSL-003 | Info | Confirmed (omission); runtime sandbox still default-on | source-evidenced | Security | Main/launcher windows omit explicit `sandbox: true` (shell pins it) | 10 | Local | local_guardrail | XS | codex |
| PERF-GSL-002 | Info | Confirmed | source-evidenced | Performance | Desktop `performance.spec.ts` is a single-run mark script, not a valid harness | 10 | Repo | operator_ux | S | gpt |

## Detailed Findings

### SECN-GSL-001: MCP App iframe receives the backend secret under a scriptable same-origin sandbox

> **RE-ASSESSED 2026-08-16 — downgraded to Warning / not-actionable. The
> original finding is preserved below; the stated taint path does not hold.**
>
> The finding assumes untrusted MCP app HTML runs in the frame whose URL
> carries the secret. It does not. That frame is the *proxy* page; the app HTML
> runs in a **nested guest iframe served from a different origin**, so
> `allow-same-origin` grants the guest its own origin and same-origin policy
> blocks `parent.location` reads. The guest only ever receives an unguessable
> single-use nonce.
>
> Verified per variant at `ed7cd5d17`:
>
> | variant | guest origin | guest sandbox | verdict |
> |---|---|---|---|
> | `crates/gosling/src/acp/` | separate loopback listener, own ephemeral port (`spawn_guest_server`) | `allow-scripts allow-same-origin allow-forms` | safe — different origin |
> | `crates/gosling-server/` | same router | `allow-scripts allow-forms` (**no** same-origin) | safe — opaque origin, and its `srcdoc` fallback is safe for the same reason |
>
> Upstream comparison: `aaif-goose/goose` puts the same secret in the **query
> string**, which is strictly worse (it reaches the server, access logs, and
> `Referer`). Upstream had the genuine same-origin form of this bug in its
> `goose-server` crate and **deleted that crate** (PR #10224) rather than fixing
> it. There is no upstream mechanism to port.
>
> **What is load-bearing, and what a future agent must not undo:** the two
> variants are safe for *different* reasons, and each is one edit away from
> being unsafe. Merging the ACP guest route into the main router while it keeps
> `allow-same-origin`, or adding `allow-same-origin` to the gosling-server
> guest, re-creates the real vulnerability. Both sites now carry comments
> saying so, and
> `acp::mcp_app_proxy::tests::the_guest_is_served_from_its_own_loopback_origin`
> fails if the ACP guest stops owning its origin.
>
> Residual hardening, **not** a demonstrated path: the outer page's `script-src`
> is widened by app-declared `resource_domains`/`script_domains`. No injection
> sink was found in that page's own code (its only DOM writes are
> `document.body.textContent` and `createElement('iframe')`). If the secret is
> to leave the URL entirely, the `proxy_token` added by the SEC-GOS-002 fix
> (`5ea594f4b`) is the natural basis.

#### Original finding (2026-08-15, unmodified)


Severity: High  
Confidence: Confirmed (secret placement + sandbox tokens); runtime secret use / sandbox escape is Likely  
Evidence basis: source-evidenced  
Domain: Security (SECN-018, SECN-003-adjacent iframe; not a Node `eval` sink)

Evidence:
- `ui/desktop/src/main.ts:2099-2119` — `get-mcp-app-proxy-url` builds `${httpBase}/mcp-app-proxy` and sets `proxyUrl.hash = new URLSearchParams({ secret: secretKey }).toString()`
- `ui/desktop/src/components/McpApps/McpAppRenderer.tsx:182-188` — renderer asks main for that URL
- `ui/desktop/src/components/McpApps/McpAppRenderer.tsx:754-766` — that URL becomes `sandboxUrl`
- `ui/desktop/src/components/McpApps/McpAppRenderer.tsx:404` — `iframe.src = sandbox.url.href`
- `ui/desktop/src/components/McpApps/McpAppRenderer.tsx:133` — `DEFAULT_SANDBOX_PERMISSIONS = 'allow-scripts allow-same-origin allow-forms'`
- `ui/desktop/src/components/McpApps/McpAppRenderer.tsx:701-705` — MCP-declared `permissions` are forced to `null` (comment: SDK does not yet accept them)

Observed behavior:
- Untrusted MCP-app HTML is loaded inside an iframe whose document URL carries the per-window `gosling serve` secret in the fragment. The iframe is allowed scripts and same-origin. The host never applies the extension’s own permission tokens.

Expected boundary:
- The guest must not observe the backend secret. Prefer a one-time, origin-scoped cookie or a main-process-only proxy that injects `x-secret-key`. Iframe sandbox must not combine `allow-scripts` with `allow-same-origin` unless the iframe origin is uniquely generated and secret-free.

Failure mechanism:
- Fragment is part of `iframe.src`. A script in the guest can read `location.hash`. Combined `allow-scripts` + `allow-same-origin` is the documented iframe-sandbox escape pair if the iframe document is same-origin with a parent-controlled origin.

Break-it angle:
- Enable any MCP App that serves HTML; in the guest console (or injected script) read `location.hash`. Static evidence confirms the secret is on the URL. Whether a given extension’s HTML actually executes that read was not reproduced.

Impact:
- Theft of the loopback ACP/HTTP secret lets the guest call local goslingd routes as the desktop app (session/tools/config), scoped to that machine and that backend instance.

Operational impact:
- Blast radius: Local. Side-effect class: process / network / user-visible. Reversibility: compensatable (rotate secret / restart). Operator visibility: silent. Rerun safety: unsafe.

Adjacent failure modes:
- ARCN-GSL-001 (CSP may fail to pin the same origin). SECN-GSL-003 (main window sandbox is implicit).

Recommended mitigation:
- Remediation patterns: secret-out-of-URL; distinct iframe origin; drop `allow-same-origin` or drop `allow-scripts`.
- Minimal repair: stop putting `secret` in the hash; have the proxy authenticate from the Electron session, not the guest URL.
- Local guardrail: test that `iframe.src` has empty search/hash; lint ban `allow-scripts` + `allow-same-origin` together.
- Behavior test: load a fixture MCP app that reads `location.hash` / `location.href`; assert the secret is absent.

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Cost drivers: modules, tests, runtime_verification. Nominal implementation agent: claude. Rationale: touches main IPC, proxy, and iframe host; needs a guest-side negative test.

Validation:
- Guest fixture cannot observe the secret; host-to-proxy auth still works for the real renderer.

Non-goals:
- Do not redesign the MCP Apps protocol in the same slice.

---

### ARCN-GSL-001: CSP lease lookup uses `webContentsId`; leases are keyed by `BrowserWindow.id`

Severity: Medium  
Confidence: Confirmed (key mismatch); connect-src miss / blocked WS is Likely  
Evidence basis: source-evidenced  
Domain: Architecture (ARCN-005-adjacent dual identity; producer/consumer pair)

Evidence:
- `ui/desktop/src/main.ts:1466-1468` — `attachWindow(mainWindow.id, lease)`
- `ui/desktop/src/goslingServeLeaseRegistry.ts:94-107` — `get` / `getAcpUrl` take `windowId`
- `ui/desktop/src/main.ts:2091-2096` — IPC `get-acp-url` correctly uses `BrowserWindow.fromWebContents(event.sender)?.id`
- `ui/desktop/src/main.ts:2676-2683` — CSP rebuild uses `(details as { webContentsId?: number }).webContentsId` as the lease key
- `ui/desktop/src/utils/csp.ts:10-26,32-50` — `connect-src` only adds the local ACP origin when `localAcpUrl` parses as loopback ws/wss

Observed behavior:
- The renderer obtains the ACP URL via the correct window id. The CSP header for the same document looks up a different id space. When the two integers diverge (second window, launcher created first, destroyed windows), `connect-src` omits `ws://127.0.0.1:<port>`.

Expected boundary:
- One identity for a window’s backend lease. CSP and IPC must use the same key.

Failure mechanism:
- Producer writes `BrowserWindow.id`. CSP consumer reads Electron `webContentsId`. Those are not the same generator.

Break-it angle:
- Open a second chat window after a launcher or after closing window 1. If IDs diverge, DevTools will show CSP blocking the ACP WebSocket. Not reproduced in this run.

Impact:
- Silent connect failures or a CSP that is weaker/stronger than intended, depending on accidental id collision (first window is often `1`/`1`).

Operational impact:
- Blast radius: Workflow. Side-effect class: user-visible. Reversibility: reversible (reload/new window). Operator visibility: UI-visible (connection error) or silent. Rerun safety: safe.

Adjacent failure modes:
- SECN-GSL-001 (same origin family). ARCN-GSL-002 (config/id ownership).

Recommended mitigation:
- Remediation patterns: single lease key; resolve `webContentsId` → `BrowserWindow.fromId`/`fromWebContents`.
- Minimal repair: in `onHeadersReceived`, map `webContentsId` to `BrowserWindow.fromWebContents(...)?.id` before `getAcpUrl`.
- Local guardrail: unit test with distinct fake ids asserts CSP includes the lease origin only for the matching window.
- Behavior test: two windows, two ports; each document’s CSP `connect-src` contains only its own ACP origin.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: tests. Nominal implementation agent: codex.

Validation:
- Fixture with `windowId=2`, `webContentsId=7` still emits the port for window 2.

Non-goals:
- Do not change CSP source lists in the same patch.

---

### MEM-GSL-001: `DIGEST_CACHE` is a process-lifetime unbounded HashMap

Severity: Medium  
Confidence: Likely (Potential; not measured). OOM occurrence is not Confirmed.  
Evidence basis: source-evidenced  
Domain: Memory Lifecycle (MEM-001 / MEM-004)

Evidence:
- `crates/gosling/src/context_mgmt/summarizer/mod.rs:153-189` — `static DIGEST_CACHE: OnceLock<Mutex<HashMap<u64, CachedDigest>>>`; `store_digest` only `insert`s
- Test-only `remove_digest_for_test` explicitly refuses a full-map clear because it is process-global (`:199-201`)

Observed behavior:
- Every unique summarized block is retained for the life of `gosling` / `gosling serve`. Keys are blake3-derived from rendered text (user-influenced cardinality).

Expected boundary:
- LRU or byte cap, or per-session map dropped on session eviction.

Failure mechanism:
- Process-lifetime root + insert-only + open key set.

Break-it angle:
- Long server uptime with many distinct compacted prefixes. Heap/RSS floor after idle would rise. Not measured.

Impact:
- Monotonic native heap growth on the long-lived server/desktop-backend process.

Operational impact:
- Blast radius: Service. Side-effect class: process. Reversibility: reversible (restart). Operator visibility: silent. Rerun safety: unsafe (adds more keys).

Growth / time-to-OOM:
- Cannot compute. No measured bytes/entry, no heap limit. Label: extrapolation impossible.

Adjacent failure modes:
- MEM-GSL-002 (same context-management path). AgentManager LRU (Held) does not evict this static.

Recommended mitigation:
- Remediation patterns: LRU (entry + byte cap); session-scoped map.
- Minimal repair: `LruCache` with a documented cap (e.g. 256 / 8 MiB).
- Local guardrail: unit test that 10k unique keys leave `len() <= cap`.
- Behavior test: N unique `store_digest` then assert eviction.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: tests. Nominal implementation agent: codex.

Validation:
- Census of `DIGEST_CACHE` length under unique-key flood, not “insert exists”.

Non-goals:
- Do not change summarizer quality or packet format.

---

### MEM-GSL-003: Renderer chat store retains every loaded session until explicit delete

Severity: Medium  
Confidence: Likely (Potential; not measured). OOM not Confirmed.  
Evidence basis: source-evidenced  
Domain: Memory Lifecycle (MEM-001 / MEM-014)

Evidence:
- `ui/desktop/src/acp/chatSessionStore.ts:151-152,183-185,187-191` — process-lifetime `sessionsById` Map; `deleteSnapshot` is the only removal; `getOrCreateEntry` inserts on first touch
- `ui/desktop/src/hooks/useChatSession.ts:148-153` — every `sessionId` mount calls `loadSession` (creates an entry)
- Production `deleteSnapshot` call sites: `NavigationPanel.tsx:366,402` and `SessionListPane.tsx:615,680` — archive/delete only
- `useChatSession.ts:218-226` + `chatSessionStore.ts:303-319` — “load older” **prepends** pages onto the same array with no renderer-side cap
- `chatSessionStore.ts:721-722` — `notifications` append with no cap

Observed behavior:
- Switching chats accumulates full message/artifact snapshots in the renderer heap. History paging can pull the entire compacted session into that snapshot. Closing the view does not drop the entry.

Expected boundary:
- At most N live snapshots (e.g. current + 1), or explicit drop on unmount / LRU.

Failure mechanism:
- Intended lifetime is the visible chat; retainer lifetime is the renderer process.

Break-it angle:
- Open 50 historical sessions and page each to the start. `sessionsById.size` and retained messages should track “ever opened,” not “visible.” Not measured.

Impact:
- Desktop tab death / main-renderer GC pressure on long operator days.

Operational impact:
- Blast radius: Workflow. Side-effect class: user-visible (jank). Reversibility: reversible (restart app). Operator visibility: silent. Rerun safety: unsafe.

Adjacent failure modes:
- MEM-GSL-005 (`toolsCache` same session-id cardinality). MEM-GSL-009 (artifact slurp into the same entry).

Recommended mitigation:
- Remediation patterns: LRU of session snapshots; unmount `deleteSnapshot` for non-active ids.
- Minimal repair: keep only the active session plus one previous; call `deleteSnapshot` on eviction.
- Local guardrail: store test that switching A→B→C without delete leaves `size <= 2`.
- Behavior test: heap/object count after 50 switches returns to a bound.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: tests. Nominal implementation agent: codex.

Validation:
- Assert `sessionsById` size, not FPS.

Non-goals:
- Do not change ACP history API pagination.

---

### MEM-GSL-002: `FileMemorySource` slurps `memories.jsonl` on every retrieve

Severity: Medium  
Confidence: Confirmed (missing size bound); OOM/latency manifestation Likely  
Evidence basis: source-evidenced  
Domain: Memory Lifecycle (MEM-008 / MEM-012)

Evidence:
- `crates/gosling/src/context_mgmt/memory.rs:97-99` — comment: file is “small enough that a synchronous read per provider call is acceptable at MVP scale”
- `crates/gosling/src/context_mgmt/memory.rs:116-131` — `File::open` + `read_to_string` of the entire file, then parse every line
- `crates/gosling/src/agents/agent.rs:2110-2128` — called every context build; also `conversation.messages().clone()`

Observed behavior:
- Memory recall cost and peak allocation scale with file size, every turn, on the agent thread.

Expected boundary:
- Size cap / refuse / tail-read; or mmap + line budget.

Failure mechanism:
- User-controlled (agent-appended) file with no read cap.

Break-it angle:
- Grow `memories.jsonl` to 100 MiB; one turn allocates at least that string plus parsed copies. Not measured.

Impact:
- Latency and RSS spikes on the serving process (desktop backend or CLI).

Operational impact:
- Blast radius: Service. Side-effect class: process. Reversibility: reversible (truncate file). Operator visibility: silent / latency. Rerun safety: unsafe.

Adjacent failure modes:
- PERF-GSL-003 (same path is an unmeasured hot path). MEM-GSL-006 (clone).

Recommended mitigation:
- Remediation patterns: cap + streaming parse; reuse a mtime-checked snapshot (same idea as `ConfigSnapshot`).
- Minimal repair: refuse or truncate reads above a documented byte cap; take only `MAX_RECALLED_ITEMS`.
- Local guardrail: test a 2×-cap fixture is rejected or truncated before full parse.
- Behavior test: peak allocation delta below a stated bound.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: tests. Nominal implementation agent: codex.

Validation:
- Oversized fixture does not produce a `raw` string of file length.

Non-goals:
- Do not change recall ranking in the same slice.

---

### PERF-GSL-001: README footprint table is a historical v0.0.5 run, not HEAD evidence

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Performance (PERF-001 / PERF-003 / PERF-005)

Evidence:
- `README.md:29-41` — comparison dated **2026-07-04**, goose v1.41.0 vs **gosling v0.0.5** commit `5b7d039`; table lists binary size, build time, `--version` / `doctor` cold start
- `README.md:31` — text already says these are “historical baseline measurements, not v1.0.0 benchmark claims; rerun them before publishing current performance deltas”
- `README.md:45-56` — present-tense “Gosling implements several targeted performance enhancements” for ConfigSnapshot, keyring cache, token LRU 1024, event-bus bounds
- This HEAD is `073d19428` / desktop `package.json` version `0.1.0`. No matching remasurement artifact is in-tree.

Observed behavior:
- Operators reading the comparison table as “current gosling vs goose” get numbers from a different commit than the product they install. The disclaimer is on the table; the section title is still “Footprint & performance vs. goose”.

Expected boundary:
- Either move the table to a dated lab note, or replace numbers with a same-harness remasurement of this tag.

Failure mechanism:
- Stale metric published next to current product claims. The disclaimer is present, so this is honesty/ops, not fraud.

Break-it angle:
- Re-run the 2026-07-04 protocol on this HEAD; if Δ signs flip, the table is actively misleading. Not run.

Impact:
- Wrong capacity planning; false “we are still 27% faster” conversations.

Operational impact:
- Blast radius: Repo. Side-effect class: user-visible. Reversibility: reversible (docs). Operator visibility: UI-visible (README). Rerun safety: safe.

Adjacent failure modes:
- PERF-GSL-002 (the only desktop perf test cannot replace this table).

Recommended mitigation:
- Remediation patterns: date-box the table; add “not measured at HEAD” to the heading; schedule remasurement.
- Minimal repair: retitle to “Historical baseline (2026-07-04)” and link a remasurement issue.
- Local guardrail: release checklist forbids shipping the table without a date + commit pair matching the tag, or a “stale” banner.
- Behavior test: none (docs). Remeasure with the protocol in `scripts/README.md` if numbers stay.

Implementation assessment:
- Complexity: operator_ux. Cost: XS (banner) / L (remeasure). Nominal implementation agent: human-owner.

Validation:
- Published numbers cite a commit that exists on the released tag, with run count and host.

Non-goals:
- Do not invent new numbers in this audit.

Measurement-quality rubric for the README table (as an evidence source):

| Dimension | Result |
|---|---|
| Repeatability | FAIL — “avg” with no run count / CV |
| Warm-up | unclear |
| Workload realism | PASS for `--version`/`doctor` microbench; FAIL as product latency |
| Percentile sample size | FAIL — no p95/p99 |
| Observer effect | unknown |
| Environment control | unknown host |
| Single variable | PASS (matched features, code-mode excluded) |

The table may motivate investigation. It cannot support a current bottleneck conclusion.

---

### SECN-GSL-002: `GOSLING_ALLOWLIST` fetch is unbounded and host-unrestricted

Severity: Medium (current local/single-user: Low–Medium; High if an attacker can set the env)  
Confidence: Confirmed (missing bounds); network/DoS manifestation Likely  
Evidence basis: source-evidenced  
Domain: Security (SECN-010 / SECN-015 posture)

Evidence:
- `ui/desktop/src/main.ts:3291-3306` — `fetch(process.env.GOSLING_ALLOWLIST)` then `response.text()` then `yaml.parse`
- No timeout, no max bytes, no HTTPS/host allowlist

Observed behavior:
- Whatever URL the operator (or a compromised launcher env) set is pulled into main-process memory and parsed as YAML.

Expected boundary:
- Timeout, byte cap, HTTPS + host allowlist, `yaml.parse` already used (no custom tags — eemeli yaml; SECN-005 Held for code-exec).

Failure mechanism:
- Operator-controlled URL is a taint source into an unbounded slurp.

Break-it angle:
- Point the env at a multi-GB endpoint or `file://` / link-local URL. Not reproduced.

Impact:
- Main-process memory DoS; SSRF from the desktop process if env is attacker-influenced.

Operational impact:
- Blast radius: Local. Side-effect class: network. Reversibility: reversible. Operator visibility: log-only. Rerun safety: unsafe.

Adjacent failure modes:
- MEM-GSL-008 (same slurp class on file IPC).

Recommended mitigation:
- Remediation patterns: `AbortSignal.timeout`, byte cap, URL allowlist.
- Minimal repair: 5s timeout + 1 MiB cap + `https:` only.
- Local guardrail: test oversize body is rejected before `yaml.parse`.
- Behavior test: assert fetch abort and no full-body buffer.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal implementation agent: codex.

Validation:
- Oversize/slow URL does not grow RSS by the payload size.

Non-goals:
- Do not change allowlist YAML schema.

---

### RES-GSL-001: `check-ollama` pipes `ps` into `grep` with no timeout and no guaranteed reap

Severity: Low  
Confidence: Likely (Potential). Handle leak not measured.  
Evidence basis: source-evidenced  
Domain: Resource Lifecycle (RES-002 / RES-004 / RES-007)

Evidence:
- `ui/desktop/src/main.ts:2381-2435` — `spawn('ps')` + `spawn('grep')`; `ps.stdout.pipe(grep.stdin)`; resolve on `grep` close; `kill` only on the sibling’s `error` event; no timeout; no `finally`

Observed behavior:
- A hung `ps`/`grep` leaves two children and a pending IPC. A successful path does not explicitly `kill`/`wait` `ps` after `grep` exits (usually EOF reaps it; not guaranteed on every platform).

Expected boundary:
- `execFile`/`which`/`fetch` to the Ollama API, or `spawn` in a `try/finally` with timeout.

Failure mechanism:
- Happy-path implicit reap; error/hang path has no deadline.

Break-it angle:
- Stub `ps` to block; repeat the IPC. Process count should plateau. Not measured.

Impact:
- Rare fd/process leak on a settings/status click.

Operational impact:
- Blast radius: Local. Side-effect class: process. Reversibility: reversible (app quit). Operator visibility: silent. Rerun safety: unsafe.

Adjacent failure modes:
- Same file’s notification `spawn`s (`:2184-2188`) are fire-and-forget but have constant argv.

Recommended mitigation:
- Remediation patterns: timeout + `kill`/`wait` in `finally`; or replace with `fetch('http://127.0.0.1:11434')`.
- Minimal repair: 1s timeout, `ps.kill()`/`grep.kill()` in `finally`.
- Local guardrail: fake hung `ps`; assert no child after timeout.
- Behavior test: N clicks, process count plateau.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal implementation agent: codex.

Validation:
- Census of child processes, not `true`/`false` return.

Non-goals:
- Do not change Ollama UX.

---

### MEM-GSL-005: MCP tools cache is insert-only in production

Severity: Low  
Confidence: Likely (Potential)  
Evidence basis: source-evidenced  
Domain: Memory Lifecycle (MEM-001 / MEM-007); Architecture ARCN-028

Evidence:
- `ui/desktop/src/components/McpApps/toolsCache.ts:16-37` — module `Map` keyed by `sessionId:extensionName`
- `toolsCache.ts:40-49` — `clearToolsCache` exists
- Repo-wide production callers of `clearToolsCache`: **none** (`McpAppRenderer.tsx` only imports `getCachedTools`)

Observed behavior:
- Every session that ever rendered an MCP App permanently retains its tools-list promise.

Expected boundary:
- Clear on session delete / snapshot eviction / ACP reconnect.

Failure mechanism:
- Cache owner is “nobody.”

Break-it angle:
- 100 sessions each opening an MCP App; `cache.size` == 100 after all views close.

Impact:
- Small per session; same cardinality driver as MEM-GSL-003.

Operational impact:
- Blast radius: Workflow. Side-effect class: none. Reversibility: reversible (reload). Operator visibility: silent. Rerun safety: unsafe.

Adjacent failure modes:
- MEM-GSL-003.

Recommended mitigation:
- Call `clearToolsCache(sessionId)` from `deleteSnapshot` and ACP reconnect.
- Test: delete session ⇒ cache key gone.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal implementation agent: codex.

Validation:
- Map size, not UI.

Non-goals:
- Do not add TTL without an owner.

---

### ARCN-GSL-002: `process.env` sprawl outside a single validated config owner

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture (ARCN-013 / ARCN-014)

Evidence:
- `ui/desktop/src/main.ts` — `ENABLE_DEV_UPDATES`, Playwright paths, locale, proxy, `GOSLING_*` (lines 81, 191-192, 456, 498-500, 643-720, 999-1108, 1437-1440, 3292)
- `ui/desktop/src/goslingServe.ts:79,373-400`
- `ui/desktop/src/utils/githubUpdater.ts:30-32`
- `ui/desktop/src/utils/autoUpdater.ts:368-406`
- `ui/desktop/src/utils/winShims.ts:15,43-55`

Observed behavior:
- Defaults for the same conceptual keys (`GOSLING_LOCALE`, updater flags, path root) are read in multiple modules. No startup schema validates the set.

Expected boundary:
- One parsed config object at process start; the rest take it as a parameter.

Failure mechanism:
- Drift: two modules can default the same key differently (already visible: locale from settings vs `GOSLING_LOCALE` vs `app.getSystemLocale()`).

Break-it angle:
- Set conflicting locale/env/settings; menu vs renderer disagree (locale path already has three sources).

Impact:
- Config drift, untestable main, surprise Playwright/userData behavior.

Operational impact:
- Blast radius: Repo. Side-effect class: none. Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- ARCN-GSL-001 (another dual-identity).

Recommended mitigation:
- Remediation patterns: single `loadDesktopConfig()`; lint ban on `process.env` outside it.
- Minimal repair: document the owner file and migrate new keys only there.
- Local guardrail: eslint `no-restricted-properties` on `process.env` except `config.ts`.
- Behavior test: domain module loads with an injected config object.

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Nominal implementation agent: claude.

Validation:
- Grep for `process.env` in `src/` returns only the owner file.

Non-goals:
- Do not invent a DI container.

---

### MEM-GSL-004: Ink TUI retains every turn for process lifetime

Severity: Low  
Confidence: Likely (Potential). The file already documents a prior OOM from 300 ms re-renders (`ui/text/src/tui.tsx:560-563`); that comment is not a measurement of this HEAD.  
Evidence basis: source-evidenced  
Domain: Memory Lifecycle (MEM-001)

Evidence:
- `ui/text/src/tui.tsx:520` — `const [turns, setTurns] = useState<Turn[]>([])`
- `ui/text/src/tui.tsx:865-879` — `addLocalTurn` always appends
- Viewport virtualizes display (`:400-425`) but the array is the retainer

Observed behavior:
- Long TUI sessions keep every tool call and chunk in React state.

Expected boundary:
- Ring buffer of turns, or drop tool payloads after collapse.

Failure mechanism:
- Process-lifetime React state with user-driven cardinality.

Break-it angle:
- 1k turns with large tool outputs. Not measured.

Impact:
- TUI process RSS; Ink already warns that overflow is unclipped (project `AGENTS.md`).

Operational impact:
- Blast radius: Local. Side-effect class: user-visible. Reversibility: reversible (quit). Operator visibility: silent then crash. Rerun safety: unsafe.

Recommended mitigation:
- Cap retained turns (e.g. 200) with a “N older hidden” marker (already used for scroll).
- Test: after 300 turns, `turns.length <= cap`.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal implementation agent: codex.

Validation:
- State length, not frame time.

Non-goals:
- Do not change ACP streaming.

---

### MEM-GSL-006: Per-turn conversation clone

Severity: Low  
Confidence: Confirmed (clone exists); allocation cost unmeasured  
Evidence basis: source-evidenced  
Domain: Memory Lifecycle (MEM-011 / MEM-012)

Evidence:
- `crates/gosling/src/agents/agent.rs:765-766` — `unfixed_conversation.messages().clone()` plus `fix_conversation(unfixed_conversation.clone())`
- `crates/gosling/src/agents/agent.rs:2128` — another `conversation.messages().clone()` into `ContextBuildRequest`

Observed behavior:
- At least two full conversation copies live during context build, on top of MEM-GSL-002’s file buffer.

Expected boundary:
- Borrow or move into `fix_conversation` / `ContextBuildRequest`.

Failure mechanism:
- Duplicate in-memory forms of one large payload.

Break-it angle:
- Long session: clone cost should show in a CPU/alloc profile. Not taken.

Impact:
- GC-less Rust RSS spikes per turn (churn, not a leak) unless clones escape.

Operational impact:
- Blast radius: Service. Side-effect class: none. Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Move instead of clone where the original is unused; profile before micro-optimizing (Amdahl unknown).

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal implementation agent: gpt.

Validation:
- Alloc profile share, not “clone removed from source.”

Non-goals:
- Do not change fix_conversation semantics.

---

### MEM-GSL-008: `read-file` IPC slurps the whole granted file

Severity: Low  
Confidence: Confirmed (no cap)  
Evidence basis: source-evidenced  
Domain: Memory Lifecycle (MEM-008); Security SECN-010 posture

Evidence:
- `ui/desktop/src/main.ts:2438-2442` — after `assertRendererFileAccess`, `fs.readFile` then `buffer.toString('utf8')`
- Contrast: `read-artifact-file` (`:2466-2476`) caps at 2–20 MiB; `readBoundedSessionImportFile` caps at 16 MiB (`ui/desktop/src/utils/sessionImport.ts:5-14`, `sessionImportConstants.ts:1`)

Observed behavior:
- Sibling APIs are bounded; generic `read-file` is not.

Expected boundary:
- Same family of byte caps.

Failure mechanism:
- Granted-root file of unbounded size, renderer-reachable.

Break-it angle:
- Grant a workspace that contains a multi-GB file; invoke `readFile`. Not reproduced.

Impact:
- Main-process allocation; renderer then holds the string.

Operational impact:
- Blast radius: Local. Side-effect class: process. Reversibility: reversible. Operator visibility: silent. Rerun safety: unsafe.

Recommended mitigation:
- Apply the artifact preview cap to `read-file`; return `truncated`.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal implementation agent: codex.

Validation:
- Oversize file is truncated or rejected; peak buffer ≤ cap.

Non-goals:
- Do not change grant roots.

---

### MEM-GSL-009: Session artifact listing concatenates every page

Severity: Low  
Confidence: Likely (Potential)  
Evidence basis: source-evidenced  
Domain: Memory Lifecycle (MEM-009 / MEM-018)

Evidence:
- `ui/desktop/src/acp/sessions.ts:43-60` — `do { ... artifacts.push(...); cursor = next } while (cursor)` with `ARTIFACT_PAGE_LIMIT = 200` and no page/byte/count cap

Observed behavior:
- A session with 10k artifacts materializes all of them in the renderer before the store dedupes.

Expected boundary:
- Stop after N pages or N bytes; UI already pages chat history.

Failure mechanism:
- Pagination exists for the RPC, not for the client accumulator.

Break-it angle:
- Session with many artifacts. Not measured.

Impact:
- Renderer heap spike on session load.

Operational impact:
- Blast radius: Workflow. Side-effect class: none. Reversibility: reversible. Operator visibility: silent. Rerun safety: unsafe.

Recommended mitigation:
- Hard cap (e.g. 1k artifacts / 8 MiB) and a “more on disk” flag.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal implementation agent: codex.

Validation:
- Fixture of 5k artifacts stops at the cap.

Non-goals:
- Do not change server artifact schema.

---

### RES-GSL-002: TUI ACP child is `kill()`ed without wait or escalation

Severity: Low  
Confidence: Likely (Potential)  
Evidence basis: source-evidenced  
Domain: Resource Lifecycle (RES-002 / RES-004)

Evidence:
- `ui/text/src/tui.tsx:1452-1455` — `spawn(binary, ["acp"], { stdio: ["pipe","pipe","ignore"], detached: false })`
- `ui/text/src/tui.tsx:1487-1490` — `if (serverProcess && !serverProcess.killed) serverProcess.kill()`
- `ui/text/src/tui.tsx:1493-1506` — `exit` / `SIGINT` / `SIGTERM` / `main().catch` all call `cleanup` then `process.exit`

Observed behavior:
- Default `kill()` is SIGTERM. The Node process then exits immediately. The child is in the same process group (`detached: false`), so OS teardown usually reaps it. There is no `wait`, no SIGKILL, no timeout.

Expected boundary:
- Same contract as `goslingServe.ts` cleanup: SIGTERM, grace, SIGKILL, wait.

Failure mechanism:
- Cleanup on the happy path is “signal and abandon.”

Break-it angle:
- Child ignores SIGTERM (the desktop tests already cover a `trap "" TERM` fixture in `goslingServe.test.ts:490`). TUI would leave that child. Not run against TUI.

Impact:
- Orphan `gosling acp` after a stuck child + TUI exit.

Operational impact:
- Blast radius: Local. Side-effect class: process. Reversibility: compensatable (`kill`). Operator visibility: silent. Rerun safety: unsafe.

Adjacent failure modes:
- Desktop path is stronger (Held). Sibling inconsistency.

Recommended mitigation:
- Port the serve cleanup helper; `unref` only after `wait`.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal implementation agent: codex.

Validation:
- After TUI exit, no `gosling acp` child remains under a TERM-ignoring fixture.

Non-goals:
- Do not change ACP protocol.

---

### SECN-GSL-003: Main and launcher windows omit explicit `sandbox: true`

Severity: Info  
Confidence: Confirmed (omission). Runtime sandbox is still Electron’s default-on (Electron ≥20, this app is 41).  
Evidence basis: source-evidenced  
Domain: Security (SECN-022 posture)

Evidence:
- `ui/desktop/src/main.ts:1424-1444` — main window: `webSecurity: true`, `nodeIntegration: false`, `contextIsolation: true`; **no `sandbox` key**
- `ui/desktop/src/main.ts:1685-1696` — launcher: same, also no `webSecurity` key (defaults true)
- `ui/desktop/src/shellHost.ts:38-42` — shell **pins** `sandbox: true`
- Repo grep for `sandbox: false` / `no-sandbox`: **no matches**
- `ui/desktop/forge.config.ts:237-245` — fuses: RunAsNode off, cookie encryption on, NODE_OPTIONS off, Node CLI inspect off, ASAR integrity on, OnlyLoadAppFromAsar on

Observed behavior:
- Isolation depends on Electron default + fuses, not an explicit main-window pin. A future Electron default flip, or a copy-paste of these `webPreferences`, could disable sandbox without a test failing.

Expected boundary:
- Same explicit triple as `shellHost` on every `BrowserWindow`.

Failure mechanism:
- Sibling implementations disagree; the product window is the weaker declaration.

Break-it angle:
- None required for the omission. Do not treat default-on as a vulnerability.

Impact:
- Posture / drift only at this HEAD.

Operational impact:
- Blast radius: Local. Side-effect class: none. Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Add `sandbox: true` to both constructors; assert in the same style as `shellHost.test.ts:58-63`.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal implementation agent: codex.

Validation:
- Unit assertion on `webPreferences.sandbox === true` for main and launcher option builders.

Non-goals:
- Do not change preload privileges.

---

### PERF-GSL-002: Desktop performance e2e is not a trustworthy harness

Severity: Info  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Performance (PERF-003 / PERF-005)

Evidence:
- `ui/desktop/tests/e2e/performance.spec.ts:1-79` — single `test`, `performance.mark`, `waitForTimeout(50)` poll, no run count, no percentiles, traces on, live model prompt (“Write a haiku…”)

Observed behavior:
- This cannot validate README claims or detect a regression except as a flake-prone smoke.

Expected boundary:
- If it stays, label it smoke. Do not cite it as a benchmark.

Failure mechanism:
- Toy, single-run, coordinated-omission-prone, network-bound.

Impact:
- False confidence if someone quotes its console marks.

Operational impact:
- Blast radius: Repo. Side-effect class: none. Reversibility: reversible. Operator visibility: log-only. Rerun safety: safe.

Recommended mitigation:
- Rename/describe as smoke; keep README remasurement in `scripts/run-benchmarks.sh` / a dated lab note.

Implementation assessment:
- Complexity: operator_ux. Cost: S. Nominal implementation agent: gpt.

Validation:
- Docs do not cite this spec’s timings.

Non-goals:
- Do not turn it into a CI gate on wall-clock.

## Non-Findings / Checked But Not Confirmed

### Electron fuses and renderer isolation (Held)

- Packaged fuses at `ui/desktop/forge.config.ts:237-245` disable `RunAsNode`, `EnableNodeOptionsEnvironmentVariable`, `EnableNodeCliInspectArguments`; enable cookie encryption, ASAR integrity, OnlyLoadAppFromAsar.
- Main/launcher/shell all set `contextIsolation: true` and `nodeIntegration: false`.
- Preload exposes a typed `electron` / `appConfig` via `contextBridge.exposeInMainWorld` (`ui/desktop/src/preload.ts:393-394`); renderer event channels are a closed map (`ui/desktop/src/ipc/channels.ts:30-49`).
- No `sandbox: false`, no `no-sandbox` switch.

### Renderer filesystem confinement (Held)

- `assertRendererFileAccess` → `assertPathWithinRoots` (`main.ts:301-304`, `rendererFileAccess.ts:59-72`).
- Grants refuse symlink roots (`rendererDirectoryGrants.ts:24-31`) and persist at most 256 roots of 4096 chars (`:16-17`).
- Transient grants cleared on `webContents` destroy (`main.ts:1562-1566`).

### Backend process lifecycle (Held, desktop)

- Spawn: argv array, `shell: false`, `detached: true` (`goslingServe.ts:491-500`).
- Cleanup: process-group SIGTERM/SIGKILL + deadline (`:656-712`).
- Registry: `backendProcessRegistry.ts` records pid and SIGTERM/SIGKILL on next launch (`:211-231`).
- Parent death: `GOSLING_SERVER__PARENT_PID` (`goslingServe.ts:397-400`) + `parent_exit_wait` (`crates/gosling-server/src/commands/agent.rs:76-107`).

### MCP / provider child configuration (Held)

- `configure_subprocess` sets `kill_on_drop(true)`, Unix process group, Linux `PR_SET_PDEATHSIG` (`crates/gosling/src/subprocess.rs:52-74`).
- MCP spawn uses it (`extension_manager.rs:652-670`); stderr capture capped at 64 KiB (`:682`).
- Residual: macOS/Windows abnormal parent death can orphan MCP children; serve itself is covered by PARENT_PID. Not a new defect — documented in-source. **Not Confirmed as a leak.**

### Agent / event-bus bounds that README claims (structurally Held)

- `DEFAULT_MAX_SESSION = 5` LRU (`execution/manager.rs:15,50-55`); busy sessions skipped for eviction (`:280-286` region).
- Token counter LRU 1024 (`token_counter.rs:13,46-50`).
- Session event bus: broadcast 256, replay 512 events **and** 8 MiB (`session_event_bus.rs:7-13,47`).
- ACP WS client buffer: 1024 messages / 8e6 chars (`createWebSocketStream.ts:7-9,41-48`).
- Session import: 16 MiB (`sessionImport.ts` + constants).
- These confirm the *existence* of the README mechanisms. They do **not** confirm the README *numbers*.

### SECN inventory (nodejs)

| Code | Verdict |
|---|---|
| SECN-001/002 proto pollution | **Held** — no user JSON merge; `set-setting` keys are typed |
| SECN-003 eval/Function/vm | **Held** — no hits in `ui/desktop/src` / `ui/text/src` except `regex.exec` |
| SECN-004 dynamic import specifier | **Held** — `import(\`./compiled/${locale}.json\`)` is locale-controlled, not user path |
| SECN-005 unsafe deser | **Held** — `yaml.parse` (eemeli) on operator allowlist; else JSON |
| SECN-006 command injection | **Held** for serve/git (`execFile`/`spawn` argv). `shell: true` only on constant `ms-settings:notifications` (`main.ts:2188`) |
| SECN-007 argument injection | **Held** for `git -C dir worktree` (`main.ts:474-477`); `dir` is positional after `-C` |
| SECN-008 path traversal | **Held** for renderer FS (grants + realpath). Zip-slip N/A in Node (import is Rust) |
| SECN-009 ReDoS | Not deep-audited; no user-facing catastrophic regex identified in sampled paths |
| SECN-010 body bounds | **Finding** SECN-GSL-002 (allowlist) + MEM-GSL-008 (`read-file`). No Express server |
| SECN-011 HTTP parser | **N/A** — no Node HTTP server in src |
| SECN-012 headers | CSP applied (`main.ts:2672-2692`, `csp.ts:90-115`). `object-src 'none'`, `script-src 'self'` |
| SECN-013 CORS | **N/A** — `cors` unused in src |
| SECN-014 cookies | Electron session cookies; fuse cookie encryption on. No custom session cookie |
| SECN-015 SSRF | **Posture** SECN-GSL-002 (env URL). `openExternal` gated by `normalizeWebUrl` / `isProtocolSafe` |
| SECN-016 JWT | **N/A** in Node UI |
| SECN-017 timing-safe | No Node token compare found |
| SECN-018 secrets | **Finding** SECN-GSL-001. `GENERATED_SECRET` stays main-side except the proxy hash. `additionalArguments` passes security *override flags*, not the serve secret (`main.ts:1430-1441`) |
| SECN-019 lockfile | pnpm lock present; versions not CVE-scanned this pass |
| SECN-020 lifecycle | desktop `postinstall` builds workspace SDK (`package.json:13`) — first-party, not a supply-chain surprise |
| SECN-021 npm publish | `ui/text` publishes `dist` only (`files: ["dist"]`). desktop is an Electron app (`private` not set — **posture**, not traced leak) |
| SECN-022 hardening | **Finding** SECN-GSL-003 (explicit sandbox). `start-gui-debug` uses `--inspect=9229` (dev script only) |

### ARCN inventory (nodejs, limited)

| Code | Verdict |
|---|---|
| ARCN-001/002 business in handler | Coherent Electron alternative: main.ts *is* the adapter. No second HTTP API. **Non-finding** (decision-rule row 1) |
| ARCN-003/004 data/tx | **N/A** — no Node ORM |
| ARCN-005 duplicate rules | Locale resolution has three sources (ARCN-GSL-002). Lease id is the serious duplicate (ARCN-GSL-001) |
| ARCN-006-008 framework leak | **N/A** as Express; Electron `BrowserWindow` staying in main is correct |
| ARCN-009 cycles | No eval-time cycle traced from `main.ts` / `tui.tsx`. Graph tool not run (no-install) |
| ARCN-010 barrels | Not a load-bearing problem in sampled entry points |
| ARCN-011 upward import | Renderer does not import main. **Held** (grep-level) |
| ARCN-012 workspace | desktop depends on `@repo-makeover/gosling-sdk` (workspace). **Held** |
| ARCN-013/014 env | **Finding** ARCN-GSL-002 |
| ARCN-015 unawaited | `void goslingServeLeases.releaseWindow` on `closed` (`main.ts:1466`) is owned; serve `cleanup` is awaited in registry. TUI `cleanup` is fire-and-forget (RES-GSL-002) |
| ARCN-016 emitters | `AppEvents` CustomEvents stay in renderer. **Held** |
| ARCN-017 ALS/context bleed | No module-level current-user. Electron is single-operator. **Held** |
| ARCN-018 queues | **N/A** — no bull/bullmq/sqs/kafka/pg-boss in desktop/text manifests |
| ARCN-019/025 DI | **N/A** — no container; `app.locals` unused |
| ARCN-020 any-at-seam | `as unknown as` at ACP/MCP seams (`mcp-apps.ts:109`, `prompt.ts:31`). Drift risk, not scored separately |
| ARCN-021 DTO/zod | Settings have typed guards (`isSettingKey`). MCP tool schema is asserted. **Partial** |
| ARCN-022 import-time I/O | `main.ts` loads grants/settings at module scope (`:284-289`). Expected for Electron main. Decision-rule row 2 |
| ARCN-023 ORM leak | **N/A** |
| ARCN-024 dual package | Not inspected at lockfile resolution depth |
| ARCN-026 tsconfig paths | Vite bundles main/preload; packaged path is `.vite/build/main.js`. **Likely Held** (bundle not unpacked this pass) |
| ARCN-027 dynamic import | i18n locale JSON. Closed set. **Held** |
| ARCN-028 module cache | **Finding** MEM-GSL-005 / ARCN owner |

### MEM inventory

| Code | Verdict |
|---|---|
| MEM-001 | Findings MEM-GSL-001/003/004/005 |
| MEM-002 | Renderer `on`/`off` paired in sampled hooks. TUI `stdout.on('resize')` removed on unmount (`tui.tsx:512-515`). **Held** |
| MEM-003 | Elicitation timeouts clear (`elicitationRequests.ts:46-63,81-82`). Permission map replaces per key. **Held** |
| MEM-004/005 | DIGEST_CACHE (finding). Token LRU / agent LRU / event bus **Held** |
| MEM-006 | ACP WS buffer bounded. TUI `queueRef` unbounded but interactive (low). **Partial** |
| MEM-007 | toolsCache keys are session ids (finding MEM-GSL-005) |
| MEM-008 | Findings MEM-GSL-002/008; import/artifact **Held** |
| MEM-009 | Finding MEM-GSL-009 |
| MEM-010 | Session import bounded. **Held** |
| MEM-011 | Finding MEM-GSL-006 |
| MEM-012/013 | FileMemory + clones = Potential churn. Not measured |
| MEM-014 | Session store delete only on delete (MEM-GSL-003) |
| MEM-015 | No pool shrink claim |
| MEM-016 | Snapshot unmount missing (MEM-GSL-003) |
| MEM-017 | Not indicated |
| MEM-018 | memories.jsonl, allowlist, artifacts, read-file |
| MEM-019 | `DEFAULT_MAX_SESSION = 5` bounds concurrent agents. **Held** |

### RES inventory

| Code | Verdict |
|---|---|
| RES-001 zombies | Desktop serve: wait/kill. TUI: no wait (RES-GSL-002). MCP: kill_on_drop |
| RES-002 not killed | check-ollama (RES-GSL-001); TUI TERM-only |
| RES-003 per-request process | Ollama probe yes; serve reused per window lease. **Partial** |
| RES-004 happy-path only | RES-GSL-001 |
| RES-005 threads | ACP provider thread join exists in Rust (prior seed; not re-deep-read). Not a Node finding |
| RES-006/007 handles | Artifact read uses `finally close` (`main.ts:2470-2476`). Session import `finally handle.close`. **Held**. check-ollama pipes: finding |
| RES-008/009 pools | **N/A** Node. Rust reqwest clients reused (README / posthog) |
| RES-010 timers | Elicitation timeout cleared. Hub `setInterval` 30s cleared on unmount (`Hub.tsx:41`). TUI animation interval gated and cleared (`tui.tsx:564-571`) |
| RES-011 cron | **N/A** |
| RES-012 watchers | **N/A** sampled |
| RES-013-016 polling | `useNavigationSessions` 300 ms × 10 s then stop (`:158-181`). **Held**. OAuth device-flow backoff is Rust (out of Node architecture; resource-adjacent Held per seed, not remeasured) |
| RES-017/018 disk | Startup logs dir exists; not size-audited. **Not Reviewed** |
| RES-019 amplification | Artifact pages, session open, MCP apps |

### PERF inventory

| Code | Verdict |
|---|---|
| PERF-001 | Finding PERF-GSL-001 (unmeasured current claims). Mechanisms exist |
| PERF-002 | README uses `--version` as a proxy for “faster product” |
| PERF-003 | Finding PERF-GSL-002; README table fails rubric |
| PERF-004 | `--version`/`doctor` are cold-start micros; not product startup |
| PERF-005 | No p95 anywhere in cited evidence |
| PERF-006 | toolsCache exists specifically to avoid N+1 (`toolsCache.ts:1-9`). **Held** for that path |
| PERF-007 | N/A Node. SQLite plans not reviewed |
| PERF-008 | Not shown |
| PERF-009 | Artifact page loop is chatty+slurp (MEM-GSL-009) |
| PERF-010 | `FileMemorySource` sync `read_to_string` on the agent path (Rust, not Node event loop) |
| PERF-011 | ConfigSnapshot / token LRU exist |
| PERF-012 | read-file / memories.jsonl |
| PERF-013 | Not measured |
| PERF-014 | Serve process reused via lease registry. **Held** |
| PERF-015 | Not measured |
| PERF-016 | Desktop Vite + Electron; not profiled |

## Architecture extras (required)

### Intake (ARCN)

- Stack: Electron 41 + Vite + React 19 (desktop); Ink 6 + React 19 (text). Module system: TS compiled by Vite (desktop) / `tsc` ESM (text). Package manager: pnpm (`package.json` engines `pnpm >=10.30.0`). Runtime: Electron (desktop), Node (text). Deployment: packaged Electron app / CLI.
- Detection recipe: `electron` dep + `main`/`preload`/`renderer` (`runtime-variants.md` Electron row).

### Boundary-mapping table (desktop)

| Layer | Owns | May import | Must not import | Observed violations |
|---|---|---|---|---|
| Electron main (adapter) | windows, IPC, spawn, grants, CSP | Node, Electron, utils | renderer React | none (main does not import React UI) |
| preload | contextBridge allowlist | electron, channels, types | fs/child_process | **Held** |
| renderer | React UI, ACP client | sdk, preload API | `electron` module, `child_process` | **Held** (uses `window.electron`) |
| acp/ | ACP protocol mapping | sdk | main.ts | **Held** |
| config | *intended* env+settings | nothing app-level | — | **violated** (ARCN-GSL-002) |
| Ink CLI adapter | argv, spawn acp | child_process, sdk | desktop main | **Held** |

### Dependency-direction

- Method: `grep` (no madge; install unauthorized).
- Violating edges: env reads outside a config owner (count: 40+ `process.env` sites in `ui/desktop/src`).
- Cycles: 0 eval-time cycles proven. Blind spots: Vite aliases, dynamic `import()`, pnpm store.
- `any`-at-seam: several `as unknown as` at ACP (see ARCN-020).
- Dynamic-import sites: i18n compiled JSON, lazy `App`, toasts.

### Decision-rule log

| # | Structure observed | Dominant pattern? | Recognized alternative? | Mechanisms checked | Verdict |
|---|---|---|---|---|---|
| 1 | Electron main.ts as god adapter (~3.3k lines) | yes for this app | Electron main-as-shell | no second HTTP API; grants/CSP live in main; renderer cannot spawn | coherent alternative — non-finding |
| 2 | Module-scope settings/grants load | yes | Electron main init | no request-scoped bleed (single operator) | coherent alternative — non-finding |
| 3 | toolsCache module Map | yes (module cache) | request memo | no invalidation owner | finding MEM-GSL-005 / ARCN-028 |
| 4 | Ink App owns session+render | yes for TUI | CLI transaction script | spawn/cleanup in `main()`; turns unbounded | cleanup Held-enough; memory finding MEM-GSL-004 |
| 5 | Feature folders under `components/` | yes | vertical slices | no renderer→main imports | coherent alternative — non-finding |

### Required ARCN questions

1. Stack/entry: Electron main `ui/desktop/src/main.ts`, preload, renderer `renderer.tsx`; Ink `ui/text/src/tui.tsx`. Detected from package.json + file tree.
2. Adapter: Electron IPC + `BrowserWindow`, not `router.get`.
3. Money/authz: no payments. Local FS/authz owned by grant registry (one implementation). Secret handling split (finding SECN-GSL-001).
4. Import direction: renderer → preload API → main. No reverse. Env reads violate config ownership.
5. Runtime cycles: none proven.
6. Config: many owners (finding).
7. Rejected promises: serve cleanup logged; TUI cleanup not awaited.
8. Request-scoped bleed: N/A (single-user). Lease id mismatch is the analogous bug.
9. Domain without Electron: ACP adapter modules can load; main cannot.
10. Second entry (shell vs desktop): shell already exists and pins sandbox; main does not (SECN-GSL-003).
11. Blind spots: declared above.
12. N/A items have absence evidence in the inventory tables.

## Memory extras (required)

### RSS decomposition

Not measured. Reasoned: desktop RSS = Electron/V8 heap (session store, messages) + spawned `gosling` RSS (conversation, digest cache, memories slurp) + child MCP processes (resource lens). Flat V8 + rising RSS would point at serve/MCP, not the renderer Map.

### Memory surface map

| Root | Class | Bound? |
|---|---|---|
| `sessionsById` | renderer Map | no (MEM-GSL-003) |
| `DIGEST_CACHE` | Rust static HashMap | no (MEM-GSL-001) |
| `toolsCache` | renderer Map | no (MEM-GSL-005) |
| TUI `turns` | React state | no (MEM-GSL-004) |
| AgentManager sessions | LRU | yes, 5 |
| token_cache | LRU | yes, 1024 |
| event-bus replay | count+bytes | yes |
| ACP WS incoming | count+chars | yes |
| FileMemory raw | per-call | no (MEM-GSL-002) |

### Retention matrix (material)

| Object | Alloc | Root | Lifetime | Clearing | Paths |
|---|---|---|---|---|---|
| StoreEntry | `getOrCreateEntry` | `sessionsById` | process | `deleteSnapshot` on delete only | success: no; unmount: no; error: no |
| CachedDigest | `store_digest` | `DIGEST_CACHE` | process | NONE (test-only remove) | all paths retain |
| tools list promise | `getCachedTools` | `cache` Map | process | `clearToolsCache` unused | retain |
| Turn | `setTurns` | React state | process | NONE | retain |
| memories.jsonl String | `load` | call stack | turn | drop on return | slurp |

### Bounds assessment

| Container | Key cardinality | Bound | Eviction | Sizing vs heap | Backpressure | Verdict |
|---|---|---|---|---|---|---|
| DIGEST_CACHE | user text | none | none | unknown | none | fail |
| sessionsById | opened sessions | none | delete only | unbounded messages | none | fail |
| toolsCache | session×ext | none | unused clear | small/entry | none | fail |
| turns[] | user turns | none | none | unbounded | none | fail |
| Agent LRU | sessions | 5 | LRU | intended | evict+shutdown | pass |
| token LRU | hashed text | 1024 | LRU | ~small ints | evict | pass |
| event bus | events | 256/512 + 8 MiB | drop old | 8 MiB | drop | pass |
| ACP WS | messages | 1024 / 8e6 | close | 8 MiB | close 4009 | pass |
| FileMemory | file bytes | none | n/a | file-sized | none | fail |
| import | file bytes | 16 MiB | reject | 16 MiB | reject | pass |

### Measured findings

None. No snapshot series.

### Potential findings — measurement that would settle

| ID | Measurement |
|---|---|
| MEM-GSL-001 | 3-snapshot around 10k unique digest stores; post-GC/idle map len + RSS |
| MEM-GSL-003 | 50 session switches; `sessionsById.size`; V8 heap snapshot |
| MEM-GSL-002 | peak RSS delta reading a 50 MiB memories.jsonl |
| MEM-GSL-004 | 1k-turn TUI; heap of `turns` |

## Resource extras (required)

### Ownership matrix (material)

| Resource | Acquire | Owner | Release | Lifetime | success | error | timeout | cancel | crash |
|---|---|---|---|---|---|---|---|---|---|
| gosling serve | `spawn` serve | lease registry | `cleanup` + PARENT_PID | window(s) | kill tree | cleanup on ready fail | deadline 10s | window close | registry on next start |
| MCP stdio | `TokioChildProcess` | `McpClient` | drop/`kill_on_drop` | agent LRU | drop | drop | request-level only | child lives | Linux PDEATHSIG; macOS orphan |
| TUI acp | `spawn` acp | `serverProcess` | `kill()` | process | signal | signal | none | SIGINT kill | OS group |
| check-ollama | ps+grep | handler | implicit/error kill | request | implicit | sibling kill | **none** | none | leak |

### Polling / update-frequency

| Loop | Rate | Change rate | Waste | Stop | Backoff | Verdict |
|---|---|---|---|---|---|---|
| recent-session poll | 300 ms × ≤10 s | session list | ~33 req then stop | yes (`maxPolls`) | none (short window) | Held |
| parent PID wait | `PARENT_LIVENESS_POLL_INTERVAL` | parent death | low | yes (process gone) | n/a | Held |
| TUI animation | 300 ms | splash/load only | gated off when idle | yes | n/a | Held |
| Hub clock | 30 s | wall clock | low | unmount | n/a | Held |

No webhook alternative applies to local session lists.

### Time-to-exhaustion

Cannot compute for process/fd growth: no snapshots. `ulimit -n` / pid_max not read. Label: not computed.

## Performance extras (required)

### Target-metric statement

| Field | Value |
|---|---|
| Metric class | (assumed) product latency + binary/startup footprint, because README claims those |
| Percentile | not defined by repo (gap = PERF-005) |
| Measurement point | README: process start of `--version`/`doctor`; not desktop first-paint |
| Workload | microbench; `code-mode` excluded |
| Baseline | 2026-07-04 table only |
| Target/budget | none |

Primary PERF finding is the missing current target, not a profiled hotspot.

### Bottleneck classification

No discriminating runtime measurement. Static candidates: FileMemory slurp + conversation clone (CPU/alloc on the agent turn), renderer session store (V8 heap), MCP iframe (not a perf issue). **Ambiguity stated:** cannot name a dominant class without a profile.

### Prioritization (Amdahl)

No `p` from a profile. Order is therefore **risk/honesty**, not speedup:

1. SECN-GSL-001 (not a perf fix)
2. Session/digest bounds (prevent unbounded growth; p unknown)
3. FileMemory cap (likely material on huge files; p unknown)
4. README remasurement (does not change runtime)

Do not micro-optimize clones while MEM-GSL-001/003 remain.

### Claimed optimizations vs current source

| README claim | Source at HEAD | Numbers |
|---|---|---|
| ConfigSnapshot mtime cache | `crates/gosling/src/config/base.rs` `ConfigSnapshot` / `param_cache` | not remasured |
| KEYRING_RUNTIME_DISABLED | `config/base.rs:28,1380,1404` | not remasured |
| shared_token_counter LRU 1024 | `token_counter.rs` | cap exists; hit rate unknown |
| spawn_blocking tokenizer OnceCell | `token_counter.rs` `TOKENIZER` OnceCell | not remasured |
| would_exceed_limit no clone | `tool_monitor.rs:88,122` | not remasured |
| static reqwest / 10s | not re-read this pass | unknown |
| event bus LRU/byte cap | `session_event_bus.rs` | exists |
| subprocess stdio cap | `extension_manager.rs:682` 64 KiB | exists |

## Break-It Review

| Attack | Target | Result |
|---|---|---|
| `__proto__` via settings key | `set-setting` | Held (typed keys) |
| `../` via `read-file` | grants + realpath | Held |
| `; id` via serve spawn | argv + `shell:false` | Held |
| MCP guest reads `location.hash` | iframe src | **secret is on the URL** (not executed) |
| Second window CSP | lease id | **wrong key** (not executed) |
| Unique digest flood | DIGEST_CACHE | **no cap** (not measured) |
| 50 session switches | sessionsById | **no evict** (not measured) |
| Huge memories.jsonl | FileMemory | **full slurp** (not measured) |
| Hung `ps` | check-ollama | **no timeout** (not measured) |
| TERM-ignoring TUI child | cleanup | **no SIGKILL** (not measured) |
| README numbers at HEAD | remasure | **not run** |

## Recommended Patch Order

1. SECN-GSL-001 — secret out of iframe URL; fix sandbox tokens.
2. ARCN-GSL-001 — CSP uses `BrowserWindow.id`.
3. MEM-GSL-001 + MEM-GSL-003 + MEM-GSL-005 — bound digest + evict snapshots + clear tools cache.
4. MEM-GSL-002 + MEM-GSL-008 — read caps.
5. RES-GSL-001 / RES-GSL-002 — ollama timeout; TUI wait/kill.
6. PERF-GSL-001 — banner or remasure; do not invent numbers.
7. SECN-GSL-003 / ARCN-GSL-002 — explicit sandbox; config owner (hardening).

## Regression Test Strategy

| Test | Purpose | Finding |
|---|---|---|
| Guest fixture cannot read serve secret from `location` | secret isolation | SECN-GSL-001 |
| Distinct window vs webContents ids still emit correct CSP origin | lease identity | ARCN-GSL-001 |
| 10k digest inserts ⇒ `len <= cap` | cache bound | MEM-GSL-001 |
| Switch 10 sessions without delete ⇒ store size bounded | snapshot LRU | MEM-GSL-003 |
| `deleteSnapshot` clears toolsCache key | cache owner | MEM-GSL-005 |
| 2×-cap memories.jsonl refused/truncated | slurp bound | MEM-GSL-002 |
| `read-file` of huge granted file truncated | IPC bound | MEM-GSL-008 |
| Hung `ps` + timeout ⇒ no child | reap | RES-GSL-001 |
| TUI exit vs TERM-ignoring child ⇒ no leftover pid | reap | RES-GSL-002 |
| `webPreferences.sandbox === true` for main/launcher | pin | SECN-GSL-003 |
| Same-harness remasure of `--version`/`doctor` vs goose | claim hygiene | PERF-GSL-001 |

## Deferred Risks

- macOS MCP orphan after Electron SIGKILL (documented; serve covered by PARENT_PID; MCP children not).
- Dual-package / lockfile CVEs not scanned.
- Electron `EnableLoadBrowserProcessSpecificAtLaunch` fuse not set (not in current FusesPlugin block) — posture only.
- `ui/desktop` package `private` unset — publish exposure not packed.
- SECN-009 ReDoS not systematically hunted.
- Startup log dir growth (RES-017) not size-audited.
- Conversation clone cost (MEM-GSL-006) until a profile exists.

## Validation Limits

- No process launched; no heap/`ps`/`lsof`; no README remasurement.
- Architecture graph: grep only. Aliases/dynamic imports/PnP are blind spots; “no cycles” is not stronger than that.
- Rust MCP/provider spawn matrix sampled at `subprocess.rs` + `extension_manager.rs` + serve PARENT_PID; not every provider CLI was re-walked line-by-line.
- `ui/desktop/src/components/settings/**` (120 files) not fully read; sampled via IPC/settings path.
- Generated `ui/desktop/src/i18n/compiled/**` excluded.
- Historical `docs/cloud/audit-*.md` line numbers are stale (e.g. old `DEFAULT_MAX_SESSION = 100` is now **5**); not reused as evidence.
- Oracle integrity: no test suite was used as a Held oracle. No fixture/pragma restore issue applies.

## Final Confidence

**Medium** for security/architecture code properties (quoted). **Low–Medium** for leak/OOM/latency magnitude (uncalibrated, unmeasured). Isolation/fuse **Held** claims are high-confidence as *declarations*. MCP-secret and lease-id bugs are high-confidence as *code*. Runtime exploitation and exhaustion stay Likely.

## Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| SECN-GSL-001 | Security | Architecture / Workflow-GUI | Secret ownership across main↔iframe; operator-visible MCP apps |
| ARCN-GSL-001 | Architecture | Security | CSP is a trust boundary using the wrong identity |
| MEM-GSL-001 | Memory | Reliability | Unbounded static on the long-lived server |
| MEM-GSL-003 | Memory | Workflow-GUI | Session switch UX vs retainer lifetime |
| MEM-GSL-002 | Memory | Performance | Sync slurp on the turn hot path |
| PERF-GSL-001 | Performance | Compliance-Posture | Published metrics vs HEAD |
| SECN-GSL-002 | Security | Memory / Input-Output | Unbounded fetch of operator URL |
| RES-GSL-001 | Resource | Reliability | Hung child / pending IPC |
| MEM-GSL-005 | Memory | Architecture | Cache without owner (ARCN-028) |
| ARCN-GSL-002 | Architecture | Reliability | Config drift |
| RES-GSL-002 | Resource | Reliability | Sibling cleanup weaker than desktop |
| SECN-GSL-003 | Security | Architecture | Sibling webPreferences |

## Next-step route

1. `repair-defect-nodejs` for SECN-GSL-001 + ARCN-GSL-001 + SECN-GSL-003.  
2. `repair-failsafe-guardrails` for MEM-GSL-001/002/003/005/008 and RES-GSL-001/002 (caps, eviction, kill/wait).  
3. Human-owner docs pass for PERF-GSL-001; only then `audit-performance-profile` with a real remasurement if product latency is the question.

## Finding ID index

| ID | Severity | Path |
|---|---|---|
| SECN-GSL-001 | High | `ui/desktop/src/main.ts`, `ui/desktop/src/components/McpApps/McpAppRenderer.tsx` |
| ARCN-GSL-001 | Medium | `ui/desktop/src/main.ts`, `ui/desktop/src/goslingServeLeaseRegistry.ts`, `ui/desktop/src/utils/csp.ts` |
| MEM-GSL-001 | Medium | `crates/gosling/src/context_mgmt/summarizer/mod.rs` |
| MEM-GSL-003 | Medium | `ui/desktop/src/acp/chatSessionStore.ts`, `ui/desktop/src/hooks/useChatSession.ts` |
| MEM-GSL-002 | Medium | `crates/gosling/src/context_mgmt/memory.rs`, `crates/gosling/src/agents/agent.rs` |
| PERF-GSL-001 | Medium | `README.md` |
| SECN-GSL-002 | Medium | `ui/desktop/src/main.ts` |
| RES-GSL-001 | Low | `ui/desktop/src/main.ts` |
| MEM-GSL-005 | Low | `ui/desktop/src/components/McpApps/toolsCache.ts` |
| ARCN-GSL-002 | Low | `ui/desktop/src/main.ts`, `goslingServe.ts`, `utils/{autoUpdater,githubUpdater,winShims}.ts` |
| MEM-GSL-004 | Low | `ui/text/src/tui.tsx` |
| MEM-GSL-006 | Low | `crates/gosling/src/agents/agent.rs` |
| MEM-GSL-008 | Low | `ui/desktop/src/main.ts` |
| MEM-GSL-009 | Low | `ui/desktop/src/acp/sessions.ts` |
| RES-GSL-002 | Low | `ui/text/src/tui.tsx` |
| SECN-GSL-003 | Info | `ui/desktop/src/main.ts` vs `ui/desktop/src/shellHost.ts` |
| PERF-GSL-002 | Info | `ui/desktop/tests/e2e/performance.spec.ts` |
