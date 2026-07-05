# Audit — Node.js / TypeScript Architecture Lens (`ui/`)

Lens: `audit-architecture-nodejs` (v0.2) over `audit-base` v3.1.
Authority: **audit-only / read-only**. Only this file was written.
Scope: the Node/TypeScript surface under `ui/` — the Electron desktop app
(`ui/desktop`), the Ink/React TUI (`ui/text`), and the shared ACP SDK
(`ui/sdk`). Builds on `docs/cloud/00-orientation.md`.

> The supplied prompt is treated as a draft. I preserved the intended mission
> (layering/boundary, state, IPC contract, UI↔Rust/ACP coupling, module
> boundaries, error-handling architecture, the `src/api` generated-client rule,
> type ownership) and expanded to the adjacent seams these imply: the
> workspace-level linkage of the generated SDK, and the asymmetry between the
> typed IPC `invoke` contract and the untyped event-push contract.

---

## 1. Intake summary

- **Stack**: Electron 41 + React 19 + Vite (`ui/desktop`); Ink/React TUI
  (`ui/text`); a generated TypeScript SDK `@repo-makeover/gosling-sdk`
  (`ui/sdk`). TypeScript throughout; ESM (`"module": "ESNext"`,
  `moduleResolution: bundler`).
- **Module system**: ESM. `tsconfig.json` is `strict: true` +
  `noImplicitAny` + `strictNullChecks`.
- **Package manager / workspace**: pnpm workspace (`ui/pnpm-workspace.yaml`):
  members `sdk`, `text`, `desktop`, `gosling-binary/*`. `node-linker=hoisted`.
- **Entry points**:
  - Electron **main process**: `ui/desktop/src/main.ts` (2981 lines, 44
    `ipcMain` handlers), spawns the Rust binary via `goslingServe.ts`.
  - Electron **preload**: `ui/desktop/src/preload.ts` (IPC contract).
  - Electron **renderer**: `ui/desktop/src/renderer.tsx` → `App.tsx`.
  - TUI: `ui/text/src/tui.tsx`.
- **UI↔domain boundary**: There is essentially **no business/domain logic in
  the UI**. The domain lives in Rust (`crates/gosling*`) and is reached over
  **ACP** (Agent Client Protocol): the main process spawns `gosling serve`
  (`goslingServe.ts`), the renderer opens a WebSocket to `/acp`
  (`acp/acpConnection.ts`), and the TUI connects over stdio ndjson
  (`text/src/tui.tsx`). Both front ends consume the generated SDK.
- **Per-lens budget**: ~40 tool calls, prioritized to main-process IPC surface,
  the ACP/type-ownership seam, and the workspace contract linkage. `ui/text`
  and the 257 `components/**` files were sampled, not fully read (Validation
  Limits §7).

Because the UI is a **thin client over a Rust core**, several ARCN checks that
target server-side layering (route/controller/service/repository, transactions,
queues, DI containers) are structurally **N/A** here and are recorded as such in
§4. The load-bearing seams in this codebase are: (a) the **IPC contract**
between main/preload/renderer, (b) the **generated-SDK contract** between Rust
and the two front ends, and (c) the **ACP-SDK-type ↔ local-type** translation.

---

## 2. Component inventory

| Module | Layer | Role |
|---|---|---|
| `desktop/src/main.ts` | Electron main (composition root) | window mgmt, 44 IPC handlers, settings persistence, updates, file IO, tray/dock, binary lifecycle wiring |
| `desktop/src/preload.ts` | preload bridge | `contextBridge` API surface (`window.electron`, `window.appConfig`) |
| `desktop/src/goslingServe.ts` | main / process lifecycle | spawns + health-checks the Rust `gosling serve` binary |
| `desktop/src/goslingServeLeaseRegistry.ts` | main | per-window secret/URL lease |
| `desktop/src/renderer.tsx`, `App.tsx` | renderer bootstrap | React root, top-level event wiring, context providers |
| `desktop/src/acp/acpConnection.ts` | renderer / transport | ACP WebSocket client singleton |
| `desktop/src/acp/adapter/*`, `sessionNotificationAdapter.ts` | renderer / anti-corruption | translate ACP SDK notifications → local `Message`/state |
| `desktop/src/acp/chatSessionStore.ts` | renderer / state | external snapshot store (`Map<sessionId, entry>`) |
| `desktop/src/types/*` | renderer / domain types | local `Message`, `Session`, `ChatState`, etc. |
| `desktop/src/components/**` (257 files) | renderer / view | React UI |
| `sdk/src/generated/*.gen.ts` | shared contract | types + zod validators generated from Rust ACP schema |
| `sdk/generate-schema.ts` | build tool | codegen from `crates/gosling/acp-schema.json` via `@hey-api/openapi-ts` |
| `text/src/*` | TUI | Ink front end, its own ACP-over-stdio bootstrap |

**Excluded from hand review** (generated/vendored): `sdk/src/generated/**`,
`pnpm-lock.yaml`, `desktop/openapi.json` (stale artifact — see §5 CLAUDE-rule
non-finding), `i18n/messages/**`.

---

## 3. Boundary-mapping table (Electron-adapted)

| Layer | Owns | May import | Must not import | Observed violations |
|---|---|---|---|---|
| main process (`main.ts`, `goslingServe.ts`) | window/app lifecycle, IPC handlers, FS/settings persistence, Rust binary spawn, env/config | Node/Electron APIs, local util modules, `types/` | React/renderer components | none material (env correctly confined here) |
| preload (`preload.ts`) | typed `contextBridge` surface, channel routing | `ipcRenderer`, `types/settings` | renderer React, Node FS directly | **untyped `on/off/emit` escape hatch** (ARCN-GSL-001) |
| renderer (`App.tsx`, `components/**`, `acp/**`) | view, view-state, ACP client, ACP→local adaptation | SDK, ACP SDK types, `types/`, `window.electron` | Node FS, `process.env` (must go via `appConfig`) | none found (env goes through `appConfig`, held §6) |
| ACP adapter (`acp/adapter/*`) | anti-corruption: SDK types → local `Message` | ACP SDK types, `types/` | Electron main, Node | none — clean (held §6) |
| shared SDK (`sdk/`) | generated contract with Rust | `@hey-api` codegen, ACP SDK | app packages | consumers link it inconsistently (ARCN-GSL-002) |

Rule source: derived from the repo's own structure + CLAUDE.md ("UI Desktop:
Use ACP SDK types or local `src/types/*` types. Do not import generated OpenAPI
types/client code from `ui/desktop/src/api`").

---

## 4. Dependency-direction analysis & quantified counts

- **Type-erosion at seams** (repo-wide, excluding tests): `as any` = **4**
  sites; `as unknown as` = **14**; `@ts-ignore`/`@ts-expect-error` = **0**;
  `: any` = **4**; `eslint-disable no-explicit-any` = **4**. For ~75K LOC this
  is **low** — `tsconfig` is `strict`, and `lint:check` runs
  `eslint … --max-warnings 0`, which gates the `@typescript-eslint/no-explicit-any`
  `warn` rule. ARCN-020 is therefore a *localized* concern (ARCN-GSL-003/004),
  not systemic erosion.
- **`process.env` read sites** (non-test): **44**, confined to `main.ts`,
  `goslingServe.ts`, `utils/githubUpdater.ts`, `utils/autoUpdater.ts`,
  `utils/winShims.ts`. **Zero** in renderer components — the one apparent
  renderer read (`components/settings/models/predefinedModelsUtils.ts:6`) goes
  through `window.appConfig.get('GOSLING_PREDEFINED_MODELS')`. Env is confined
  to the main/bootstrap layer → **ARCN-013 held** (§6).
- **Cycles**: no cross-module runtime cycle surfaced in the sampled graph; the
  `acp/` layer imports downward into `acp/adapter/*` and `types/*` only. Not
  exhaustively tool-verified (no madge run — Validation Limits).
- **IPC push channels** (main→renderer): **~14** string channels
  (`set-view`, `theme-changed`, `fatal-error`, `new-chat`, `focus-input`,
  `open-shared-session`, `set-initial-message`, `toggle-navigation`,
  `fullscreen-change`, `find-*`, `add-extension`, `mouse-back-button-clicked`),
  all delivered through the untyped `on(channel, (event, ...args: unknown[]))`
  seam (ARCN-GSL-001).
- **Generated-SDK linkage**: `desktop` → `workspace:*` (`link:../sdk`);
  `text` → exact `0.20.2` resolved **from the registry**, not the workspace
  (ARCN-GSL-002).

**N/A ARCN items** (no such layer in a thin Electron client, recorded per the
inventory rule): ARCN-001/002/003 (no route/controller/middleware/data-access
layer — the "server" is the Rust binary), ARCN-004 (no TS-side DB
transactions), ARCN-005 (business rules live once, in Rust; both front ends
share them via the SDK — no TS re-implementation found), ARCN-006/007/008 (no
Express/Nest/Fastify domain; `express`/`cors` appear only as devitalized deps —
see note in §6), ARCN-016 (no cross-layer EventEmitter control flow; Electron
IPC is the bus), ARCN-017 (renderer is single-user single-process — no
tenant/principal context to bleed), ARCN-019 (no DI container).

---

## 5. Findings

### ARCN-GSL-001: Asymmetric IPC contract — main→renderer event channels are stringly-typed with `unknown[]` payloads

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture (ARCN-020)

Evidence:
- `ui/desktop/src/preload.ts:143-151` — the only channel-generic surface:
  ```ts
  on: (channel: string, callback: (event: Electron.IpcRendererEvent, ...args: unknown[]) => void) => void;
  off: (channel: string, callback: ...) => void;
  emit: (channel: string, ...args: unknown[]) => void;
  ```
- `ui/desktop/src/main.ts` emits ~14 distinct `webContents.send('<channel>', …)`
  payloads (e.g. `set-view`, `set-initial-message`, `open-shared-session`).
- Consumers hand-cast the untyped payload: `ui/desktop/src/App.tsx:469-471`
  `const newView = args[0] as View; const section = args[1] as string | undefined;`

Observed behavior:
- The `invoke`/request direction of the IPC contract is fully typed (the
  `ElectronAPI` interface, `preload.ts:96-174`). The **event-push** direction
  is not: any channel name and any argument tuple is accepted, and each
  renderer consumer independently asserts the payload shape with `as`.

Expected boundary:
- A single typed channel registry (a `Record<ChannelName, PayloadType>` shared
  by `main.ts` emit sites and renderer listeners) so the compiler links sender
  and receiver.

Failure mechanism:
- Rename a channel or reorder/reshape a push payload on the `main.ts` side and
  nothing on the renderer side fails to compile — the `as View`/`as string`
  casts silently reinterpret whatever arrives. Drift is invisible to `tsc`.

Break-it angle:
- Change `webContents.send('set-view', view, section)` to send an options
  object; `App.tsx:470` still compiles, reads `args[0] as View` on an object,
  and routes to `/[object Object]`. No compile error, no test catch.

Impact:
- View-routing / initial-message / shared-session-open misdelivery on refactor;
  a silently mis-cast payload becomes a runtime navigation or state bug.

Operational impact:
- Blast radius: Workflow. Side-effect class: user-visible. Reversibility:
  reversible. Operator visibility: silent (no type error, no log). Rerun
  safety: safe.

Adjacent failure modes:
- ARCN-GSL-003 (the settings-write seam erases value types the same way);
  security-nodejs owns the fact that `emit(channel, …)` exposes arbitrary
  in-renderer channel emission.

Recommended mitigation:
- Remediation pattern: typed IPC channel map. Define
  `type MainToRendererChannels = { 'set-view': [View, string?]; … }`; wrap
  `send`/`on` in helpers generic over the channel key; keep `on/off` per-channel
  typed instead of `channel: string`.
- Behavior test: a type-level test (`expectTypeOf`) that a wrong payload for a
  known channel fails to compile; assert the channel map is the single source
  used by both `main.ts` and the renderer listeners.

Implementation assessment:
- Complexity: operator_ux / local_guardrail. Cost: M. Cost drivers: modules
  (touches every `webContents.send` site + every renderer listener), tests.
  Nominal implementation agent: claude. Rationale: broad but mechanical;
  context spans main + ~10 renderer files.

Non-goals:
- Do not redesign the typed `invoke` contract; it is already sound.

---

### ARCN-GSL-002: The two front ends link the Rust-generated SDK inconsistently — `text` pins a registry snapshot while `desktop` links the workspace copy

Severity: Medium (High if the Rust ACP schema has advanced past SDK 0.20.2)
Confidence: Confirmed (linkage divergence); manifestation Likely
Evidence basis: source-evidenced
Domain: Architecture (ARCN-012; ARCN-018-flavored contract drift)

Evidence:
- `ui/sdk/package.json:3` — the workspace SDK is `"version": "0.20.2"`; it is
  **generated from the current Rust schema**: `sdk/generate-schema.ts:19`
  reads `crates/gosling/acp-schema.json`, and `desktop`'s `postinstall`/`start`
  scripts run `build-gosling-sdk` to regenerate it (`desktop/package.json:12,14`).
- `ui/desktop/package.json:52` — `"@repo-makeover/gosling-sdk": "workspace:*"`.
  Lockfile `ui/pnpm-lock.yaml:21-23` resolves it to `version: link:../sdk`
  (the local, regenerated copy).
- `ui/text/package.json:30` — `"@repo-makeover/gosling-sdk": "0.20.2"` (exact
  registry specifier, **not** `workspace:*`). Lockfile
  `ui/pnpm-lock.yaml:406-408` resolves it to
  `version: 0.20.2(@agentclientprotocol/sdk@0.19.0(zod@4.3.6))…` — i.e. pulled
  from the **npm registry**, not `link:../sdk`.

Observed behavior:
- `desktop` always consumes the SDK **regenerated from this repo's Rust ACP
  schema**. `text` consumes a **frozen published tarball (0.20.2)** whose
  generated types + zod validators reflect whatever Rust schema existed when
  0.20.2 was cut — decoupled from the current `crates/gosling/acp-schema.json`.
- The root has `@changesets/cli` wired for publishing (`ui/package.json`), so
  SDK version bumps are an expected, routine event.

Expected boundary:
- A single, consistent linkage of the internal generated contract package:
  both consumers should track the same source of truth (both `workspace:*`, or
  both the published version). One front end regenerating-from-source while the
  other pins a registry snapshot means they can talk to the **same** running
  Rust binary with **different** contract versions.

Failure mechanism:
- On the next SDK bump (0.20.2 → 0.20.3), the local `ui/sdk` advances with the
  Rust schema and `desktop` follows via `link:../sdk`; `text`'s exact `0.20.2`
  no longer matches the workspace version, so pnpm keeps resolving it from the
  registry — `text` silently runs **stale generated types and stale zod
  runtime validators** against the current binary. Payload fields added/renamed
  in the Rust schema are invisible to `text` at compile time and can be
  **rejected or mis-parsed by its stale zod validators** at runtime.
- Secondary skew: the registry SDK drags `zod@4.3.6` into `text`
  (`pnpm-lock.yaml:408,411`) while `desktop` pins `zod@^3.25.76`
  (`desktop/package.json:107`). Two zod majors validate the same ACP contract
  across the two UIs.

Break-it angle:
- Add a required field to an ACP notification in the Rust schema and bump the
  local SDK. `desktop` regenerates and compiles against it; `text` still
  imports 0.20.2, whose zod validator either strips or rejects the new field —
  a divergence no build step in `text` surfaces.

Impact:
- Silent contract drift between the Rust core and the TUI; the desktop and TUI
  front ends of the same binary can disagree on the wire contract. Runtime
  parse/validation failures or dropped fields in `text` after any SDK bump.

Operational impact:
- Blast radius: Cross-system (Rust↔TUI contract). Side-effect class: network /
  process (ACP). Reversibility: reversible (dependency change). Operator
  visibility: silent until a runtime validation error. Rerun safety: safe.

Adjacent failure modes:
- `audit-contract-crossrepo` / `audit-dependency-criticality` own the zod-major
  skew and the "registry snapshot vs source" supply question; noted here as the
  structural cause is the inconsistent workspace linkage.

Recommended mitigation:
- Remediation pattern: uniform workspace linkage. Make `text` depend on
  `"@repo-makeover/gosling-sdk": "workspace:*"` (matching `desktop`) so it
  always tracks the regenerated local SDK, and add its build to the
  `build-gosling-sdk` chain.
- Validation: a CI check asserting every in-repo consumer of an in-repo package
  uses `workspace:*` (or all use the published version) — fail the build on a
  registry-resolved internal package. A round-trip test that a schema field
  added in Rust appears in both front ends' generated types.

Implementation assessment:
- Complexity: cross_process_coordination. Cost: S. Cost drivers: modules
  (manifest + lockfile), runtime_verification. Nominal implementation agent:
  human-owner (release/versioning policy: decide whether `text` is meant to
  ship against the published SDK or the source SDK) then codex for the manifest
  change.

Non-goals:
- Do not change the SDK codegen pipeline; the generation itself is sound.

---

### ARCN-GSL-003: Settings-write IPC erases the per-key value type at the persistence seam

Severity: Low (Medium if a malformed persisted setting can wedge startup)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture (ARCN-020 / ARCN-021 flavor)

Evidence:
- `ui/desktop/src/preload.ts:131` — typed contract:
  `setSetting: <K extends SettingKey>(key: K, value: Settings[K]) => Promise<void>;`
- `ui/desktop/src/main.ts:1863-1878` — handler validates the **key**
  (`validSettingKeys.has(key)`, and a special case for `language`) but writes
  the **value** untyped: `// eslint-disable-next-line …no-explicit-any` /
  `(settings as any)[key] = value;` then `writeFileSync(SETTINGS_FILE, …)`.

Observed behavior:
- The renderer-side contract promises `value: Settings[K]`, but the main-process
  handler receives `value: unknown` and persists it verbatim after only a
  key-membership check. No per-key value-shape validation (e.g. `theme` must be
  `'light'|'dark'`, `keyboardShortcuts` must be an object).

Expected boundary:
- Runtime value validation keyed by setting (a zod schema per key, or a
  discriminated validator), mirroring the compile-time `Settings[K]` type, at
  the same seam that persists to disk.

Failure mechanism:
- The renderer is explicitly **not** a trust/integrity boundary (orientation
  §4). A buggy or manipulated renderer call (`setSetting('theme', 42)` or a
  non-object `keyboardShortcuts`) is persisted to `SETTINGS_FILE`, then read
  back and consumed as its declared type — `registerGlobalShortcuts()`
  (`main.ts:1886`) and theme code operate on a mistyped value.

Break-it angle:
- Persist `keyboardShortcuts: "oops"`; on next launch
  `registerGlobalShortcuts()` iterates a string as if it were the shortcut
  object.

Impact:
- Corrupt persisted settings; a mistyped value can throw during
  startup-time consumption (shortcuts/theme/locale) with no schema to reject it
  at write time.

Operational impact:
- Blast radius: Local (per user profile). Side-effect class: file. Reversibility:
  compensatable (delete/repair settings file). Operator visibility: silent at
  write, possibly a crash/log at next consume. Rerun safety: unsafe (bad value
  persists across restarts).

Adjacent failure modes:
- ARCN-GSL-001 (same "typed in preload, erased in main" pattern for the event
  seam).

Recommended mitigation:
- Remediation pattern: validate-at-seam. A per-key zod validator invoked in the
  `set-setting` handler before persistence; reject and log on mismatch (the key
  check at `main.ts:1865` is the precedent to extend to the value).
- Validation: a test that `set-setting` with a wrong-typed value for each key
  is rejected and the file is unchanged (assert file state, not a source
  string).

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: modules, tests. Nominal
  implementation agent: codex. Rationale: single handler + a validator map.

Non-goals:
- Do not redesign the settings storage format.

---

### ARCN-GSL-004: `as unknown as` casts bridge local content into ACP SDK request types at the prompt/tool seam

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture (ARCN-020)

Evidence:
- `ui/desktop/src/acp/prompt.ts:31` —
  `prompt: messageToAcpPromptContent(message) as unknown as SteerSessionRequest_unstable['prompt']`
- `ui/desktop/src/acp/mcp-apps.ts:109` —
  `content: (response?.content || []) as unknown as CallToolResult['content']`

Observed behavior:
- The two places where locally-shaped content is handed **into** the ACP SDK
  request contract use a double cast, erasing the compiler's ability to see
  drift between the local `Message`/content shape and the SDK's request/tool
  types.

Expected boundary:
- A typed mapper whose return type *is* the SDK type (so a shape change on
  either side fails to compile), matching the quality of the inbound adapter
  (`sessionNotificationAdapter.ts`, which is cleanly typed — held §6).

Failure mechanism:
- If the SDK's `SteerSessionRequest_unstable['prompt']` or
  `CallToolResult['content']` shape changes (these are `_unstable` /
  regenerated types — see ARCN-GSL-002), the cast keeps compiling and ships a
  mis-shaped payload to the Rust binary.

Break-it angle:
- Regenerate the SDK with a changed prompt-content union; `prompt.ts:31` still
  compiles and sends the old shape.

Impact:
- Malformed prompt/tool payloads to the agent after an SDK bump; caught only at
  runtime.

Operational impact:
- Blast radius: Workflow. Side-effect class: network (ACP). Reversibility:
  reversible. Operator visibility: silent → runtime error. Rerun safety: safe.

Adjacent failure modes:
- ARCN-GSL-002 (these casts are exactly the seams that a stale/advanced SDK
  would break silently).

Recommended mitigation:
- Remediation pattern: typed boundary mapper. Give `messageToAcpPromptContent`
  a return type of the SDK prompt type and drop the cast; same for the MCP
  content mapper.
- Validation: remove the `as unknown as`; `tsc --noEmit` failing on a
  reintroduced mismatch is the guardrail.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: modules. Nominal
  implementation agent: codex.

Non-goals:
- Do not touch the (sound) inbound adapter.

---

## 6. Explicit non-findings (seams checked and held)

- **Generated-client rule (CLAUDE.md) — ADHERED.** `ui/desktop/src/api` does
  **not exist**; no source imports `./api`/`@/api`; `@hey-api/openapi-ts` is
  **not** a `ui/desktop` dependency (verified `desktop/package.json:51-162`) —
  it appears only in `ui/sdk` (`sdk/package.json:62`), which is the sanctioned
  codegen package generating from the Rust ACP schema. `eslint.config.js`
  ignores `src/api/**` defensively. The 137 KB `ui/desktop/openapi.json` is a
  **stale, unreferenced artifact** (no `src` import; the "openapi.json" strings
  in `sdk/src/generated/*` are that codegen's own input filename) — harmless but
  worth deleting; belongs to `audit-deadcode-cleanup`.
- **ACP-SDK-type ↔ local-type ownership — held.** The inbound seam is a clean
  anti-corruption layer: `acp/sessionNotificationAdapter.ts` +
  `acp/adapter/{messages,tools,permissions,elicitations,…}.ts` translate
  `SessionNotification`/`RequestPermissionRequest` (ACP SDK) into local
  `types/message.ts` `Message` via pure `apply*` functions returning a
  `AcpChatStateChange[]` contract (`sessionNotificationAdapter.ts:23-99`).
  Local types are owned in `desktop/src/types/*` and `text/src/types.tsx`; SDK
  types are consumed, not redefined. This is the pattern CLAUDE.md prescribes.
- **Contract single-source (Rust↔TS) — held (except the linkage in
  ARCN-GSL-002).** `sdk/generate-schema.ts` generates types **and** zod
  runtime validators (`zod.gen.ts`) from one source
  (`crates/gosling/acp-schema.json`), so DTO/validator parity (ARCN-021) is
  handled by codegen rather than hand-maintained — no drift within a single SDK
  version.
- **`process.env` sprawl (ARCN-013) — held.** 44 reads, all in
  main/bootstrap/utils; renderer reaches config through the `window.appConfig`
  bridge (`predefinedModelsUtils.ts:6`), never `process.env` directly.
- **Floating promises in main (ARCN-015) — held for the main process.**
  `main.ts:1741` `process.on('uncaughtException', …)` and `main.ts:1746`
  `process.on('unhandledRejection', …)` provide the rejection sink. (Renderer
  floating promises were not exhaustively traced — Validation Limits.)
- **Rust-binary lifecycle owner (`goslingServe.ts`) — held / exemplary
  (ARCN-022).** Dependency-injected (`logger`, `readinessFetch`,
  `diagnosticsDir`), pure helpers (`buildLocalServeUrls`,
  `findGoslingBinaryPath`), factory-returned handle with `cleanup()`; no
  import-time side effects — imports open no handles. Its `.test.ts` companions
  confirm isolated testability.
- **ACP client singleton (`acpConnection.ts`) — held.** Module-level mutable
  `clientPromise`/`resolvedClient` is a coherent renderer-scoped connection
  cache in a single-process window, with monitored teardown
  (`monitorConnection`, lines 35-45) resetting both on close. Not cross-request
  context (ARCN-017 N/A in a single-user renderer).
- **`chatSessionStore` module singleton — held.** A coherent external
  snapshot-store pattern (`Map<sessionId, StoreEntry>` + subscribe), local
  types in, ACP types adapted at the edge; single-process, no concurrency
  bleed.
- **ARCN-005 (rule re-implementation across front ends) — held.** Business
  rules live in Rust; `desktop` and `text` both reach them via the shared SDK.
  No duplicated TS domain rule found. (Bootstrap/transport differs by design —
  WebSocket vs stdio — which is transport, not a duplicated business rule.)
- **`express`/`cors` in `desktop/package.json` — noted, not a domain-framework
  leak.** They are Electron-main utilities (local loopback/update plumbing),
  not a web framework leaking into domain code; no domain module imports them.

---

## 7. Validation limits

- **Not executed**: no `tsc --noEmit`, `eslint`, `knip`, `madge`, or
  `dependency-cruiser` was run (read-only audit; no install/build authorized).
  Cycle absence is from targeted reading, not tool proof.
- **Not diffed**: the published registry SDK `0.20.2` was **not** diffed
  against the locally regenerated `ui/sdk` build, so whether ARCN-GSL-002 is
  *currently* a live contract mismatch (vs a latent one that triggers on the
  next bump) is **Likely, not Confirmed** — confirm by diffing
  `sdk/src/generated/types.gen.ts` (freshly built) against the 0.20.2 tarball,
  or by advancing the Rust schema and observing `text` fail.
- **Sampled, not fully read**: the 257 `desktop/src/components/**` files were
  sampled (grep-level) for env/`any`/IPC usage, not individually reviewed; the
  full `ui/text/src/**` was read only at the import/bootstrap level (`tui.tsx`,
  `types.tsx`, `onboarding.tsx`, `extensions.tsx`).
- **No runtime**: no app was launched; all findings are `source-evidenced`.
- **Renderer floating-promise sweep** was not exhaustive; only the main-process
  rejection sink was confirmed.

---

## 8. Follow-up routing

- **Plan** (`plan-nodejs-architecture`): ARCN-GSL-001 (typed IPC channel map) —
  it touches many files and benefits from a design pass.
- **Human-owner decision then patch**: ARCN-GSL-002 — decide the intended SDK
  distribution model for `text` (source vs published) before changing manifests.
- **Bounded patch** (`repair-defect-nodejs` / codex): ARCN-GSL-003 (settings
  value validation) and ARCN-GSL-004 (typed prompt/tool mappers) are local.
- **Re-audit / sibling lenses**: the untyped `on/off/emit` channel surface and
  the unscoped `read-file`/`write-file`/`delete-file` FS-authority IPC handlers
  (`main.ts:2203-2294`, arbitrary renderer-supplied paths, only `expandTilde`,
  no scoping) → **`audit-security-nodejs`**. The zod-major skew and the
  registry-vs-source SDK supply → **`audit-dependency-criticality`** /
  **`audit-contract-crossrepo`**. Stale `desktop/openapi.json` →
  **`audit-deadcode-cleanup`**.
</content>
</invoke>
