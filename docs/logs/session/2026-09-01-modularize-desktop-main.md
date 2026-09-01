# Modularize Desktop `main.ts` — gated run log

Skill: `repair-source-modularization` (private agent-skills catalog).
Branch: `codex/modularize-desktop-main-2026-09-01`.
Routed by: `docs/TODO.md` / `MOD-GSL-001`.

## Gate 0 — Orientation

- Target repository: `/Users/eric/Work/vscode/forked/gosling`.
- Starting branch/state: `main` tracking `origin/main`, clean; work moved to the
  dedicated branch above before source edits.
- Involvement: L2 standard, inferred from a technically specific workflow request
  with no request for step-by-step approval.
- Effective authority: governed, behavior-preserving source repair only.
- Required repo sources read in order: `AGENTS.md`; root `GEMINI.md` is absent;
  `README.md`; `docs/INDEX.md`; relevant architecture/ADR sources; advisory
  `.giles/*.yaml`; prior modularization logs.
- Validation inventory: Desktop `format:check`, `lint:check`, and `test:run` are
  available through `ui/desktop/package.json`. The Vite main entry is declared in
  `ui/desktop/forge.config.ts`.
- Architecture/contract baseline:
  - `docs/adr/0008-shell-host-process-boundary.md` is accepted and requires focused
    shells to remain separate from full Desktop `main.ts`.
  - `docs/adr/0011-shell-application-runtime-boundary.md` governs focused-shell ACP
    ownership and does not authorize widening the full Desktop renderer boundary.
  - `docs/architecture.md` keeps Electron as an adapter over backend-owned contracts.
  - `docs/build/shell-productization/execution-plan.md` explicitly permits full
    Desktop main-process modularization while forbidding shell lifecycle/domain
    rules from being restored into `main.ts`.
  - Pre-repair disposition: conformant for the touched full-Desktop boundary; no
    public contract or shell/full-Desktop boundary change is planned.
- Giles files are advisory and stale (generated 2026-07-07); they were read but are
  not promoted to repository authority.
- Process/port baseline: no repo Cargo, Rust compiler, pnpm, Vitest, Vite, or test
  process was running. A pre-existing installed `/Applications/Gosling.app` owns a
  `gosling serve` child on loopback ports 53273/53275 and must remain untouched.
  A separate pre-existing Node service listens on loopback port 8888.
- Execution shape: source changes are sequential because every seam edits the same
  facade/import graph. Independent static reads and searches are batched. Each seam
  is a bounded checkpoint with its own validation and git commit.

## Gate 1 — Candidate inventory and target lock

The canonical backlog names four remaining original files. Line count is the
skill's primary discovery ordering; later factors break ties.

| Candidate | Lines | Surface and validation | Disposition this run |
|---|---:|---|---|
| `ui/desktop/src/main.ts` | 3,614 | Electron entrypoint; strong type/lint/unit checks; ordering-sensitive | **selected** as shortest routed file |
| `crates/gosling/src/agents/extension_manager.rs` | 4,531 | extension lifecycle and MCP wiring | rejected: longer; later one-file run |
| `crates/gosling/src/acp/server.rs` | 5,136 | ACP protocol/server surface | rejected: longer; later one-file run |
| `crates/gosling/src/agents/agent.rs` | 5,521 | central agent loop | rejected: longest and highest fan-in; later one-file run |

Additional files above 2,000 lines exist, including provider-format, CLI, MCP,
configuration, and SDK-type sources. `docs/TODO.md` and the active polish ledger
define the four-file routed slice as the current campaign; widening to unrelated
large files would violate the one-task rule.

The entire selected file was read in overlapping chunks (1–760, 741–1,520,
1,501–2,280, 2,261–3,040, and 3,021–3,614) before this record was written.
TypeScript AST enumeration then accounted for every top-level declaration and
executable registration.

### Symbol inventory

Imports (all retained or reassigned to one extracted owner): Electron types/runtime,
Node URL/buffer/fs/path/os/child-process/crypto, dotenv, YAML, window state,
devtools, and the existing backend/status/lease/process-registry/request-origin/ACP,
settings/security/artifact/updater/protocol modules at lines 1–86.

Named top-level declarations, grouped by responsibility:

- updater/menu: `shouldSetupUpdater`, `MENU_TRANSLATIONS_ZH_CN`,
  `detectMenuLocale`, `menuT`, `translateMenuLabels`;
- settings/path grants: `SETTINGS_FILE`, `RENDERER_DIRECTORY_GRANTS_FILE`,
  `STARTUP_LOGS_DIR`, `BACKEND_PROCESS_REGISTRY_PATH`, `validLanguageSettings`,
  `isValidLanguageSetting`, `settingsCache`, `externalBackendSecret`,
  `legacySecretRemovalNoticePending`, `externalSecretPersistenceNoticePending`,
  `settingsRecoveryNoticePending`, `isLegacySettings`, `getSettings`,
  `resolveRendererPath`, `rendererDirectoryGrants`, `rendererFileRoots`,
  `firstGrantedRecentDirectory`, `assertRendererFileAccess`;
- artifacts/proxy/external URL: `rendererArtifactFileGrants`,
  `artifactRoutingRegistry`, `ARTIFACT_PRODUCT_TYPES`,
  `assertRendererArtifactFileAccess`, `assertArtifactOutputRootAccess`,
  `validateArtifactRoutingConfig`, `openExternalIfSafe`,
  `loopbackHttpBaseFromAcpUrl`, `McpAppProxyCsp`, `appendDomainParams`;
- settings mutation/locale: `updateSettings`, `getConfiguredGoslingLocale`;
- Git/proxy: `listGitWorktreeDirs`, `gitArgs`, `runGit`, `getGitBranchInfo`,
  `isValidGitBranch`, `configureProxy`;
- backend certificate trust: `BackendCertificateTrust`,
  `BackendCertificateTrustRegistration`, `trustedBackendCertificates`,
  `backendCertificateVerifierSessions`, `MAIN_WINDOW_SESSION_PARTITION`,
  `normalizeHostname`, `normalizeFingerprint`, `trustBackendCertificate`,
  `getBackendCertificateTrusts`, `verifyBackendCertificate`, `isTrustedHost`,
  `installBackendCertificateVerifier`;
- protocol/open handling: `openUrlHandledLaunch`, `shouldQuitForSingleInstance`,
  `focusExistingWindow`, `handleSecondInstanceCommandLine`, `pendingDeepLinks`,
  `queuePendingDeepLink`, `reactReadyWindows`, `DEEPLINK_BURST_DEDUP_MS`,
  `recentSessionDeepLinkSends`, `pruneExpiredSessionDeepLinkSends`,
  `isBurstDuplicateSessionDeepLink`, `recordSessionDeepLinkSend`,
  `sendOpenSharedSession`, `createResumeChatWindow`,
  `deliverRendererProtocolUrl`, `handleProtocolUrl`, `processProtocolUrl`,
  `handleFileOpen`;
- build/runtime config: `MAIN_WINDOW_VITE_DEV_SERVER_URL`,
  `MAIN_WINDOW_VITE_NAME`, `getAppUrl`, `parseArgs`, `BundledConfig`,
  `getBundledConfig`, destructured bundled defaults, `resolveGoslingPathRoot`,
  `GENERATED_SECRET`, `ExternalBackend`, `getExternalBackendUrlFromEnv`,
  `getExternalBackendFromEnv`, `getActiveExternalBackend`,
  `getExternalBackendForCsp`, `appConfig`;
- window/backend/wakelock state: `windowMap`, `goslingServeLeases`,
  `windowPowerSaveBlockers`, `activeWakelockSessionsByWindow`,
  `syncWindowPowerSaveBlocker`, `clearWindowWakelock`, `clearAllWakelocks`,
  `pendingInitialMessages`, `pendingInitialMessageNoAutoSubmit`,
  `CreateChatOptions`, `createChat`;
- launcher/tray/menu helpers: `activeLauncherWindow`, `createLauncher`, `tray`,
  `destroyTray`, `disableTray`, `createTray`, `showWindow`,
  `buildRecentFilesMenu`, `openDirectoryDialog`;
- errors/settings/library: `handleFatalError`, `rendererSettingValue`,
  `configuredResearchLibraryPath`, `ensureResearchLibrary`;
- bounded operations: `CHECK_OLLAMA_TIMEOUT_MS`, `READ_FILE_MAX_BYTES`;
- orchestration: `createNewWindow`, `focusWindow`, `registerGlobalShortcuts`,
  `appMain`;
- allowlist/shutdown: `ALLOWLIST_FETCH_TIMEOUT_MS`, `ALLOWLIST_MAX_BYTES`,
  `getAllowList`, `shutdownCleanupPromise`, `shutdownCleanupComplete`,
  `runShutdownCleanup`, `scheduleShutdownCleanup`.

Module-level executable statements are separately accounted for:

- Playwright user-data path setup (200–202), renderer-grant load (294–298),
  squirrel quit (575), certificate verifier registration (679–697), readiness
  locale update (699–701), Playwright debug switch (704–708), protocol client
  registration (712–731), and single-instance/startup protocol handling (781–823);
- Electron `open-url`, launch/about, `open-file`, and macOS `open-files` handlers
  (922–983);
- process fatal handlers (2,029–2,037) and all top-level Desktop IPC registrations
  from `reactReady` through `get-allowed-extensions` (2,039–2,805), individually
  represented by their channel names in the source/AST inventory;
- `app.whenReady` startup (3,467–3,506), quit handlers (3,606–3,607), and
  `window-all-closed` (3,609–3,614).

Direct connection: `ui/desktop/forge.config.ts` declares this exact entrypoint.
Second-level paths are the Vite main build, Electron preload channel surface,
Desktop renderer workflows, spawned `gosling serve` lifecycle, and package/start
commands. No source module imports `main.ts`; it is an executable entrypoint.

### Bug ledger (MOD-B)

#### BUG-001 — Allowlist size guard appears not to enforce its claimed byte bound

- Code: MOD-B14 (contract mismatch), with a security/resource consequence.
- Location: `ui/desktop/src/main.ts:3510-3561`.
- Observed while: reading the locked target.
- Evidence: the limit is named `ALLOWLIST_MAX_BYTES`, but the response is first
  consumed through `response.text()` and then checked with `rawYaml.length`.
- Why it looks wrong: decoding the whole response precedes the limit, and JavaScript
  string length counts UTF-16 code units rather than received bytes. The adjacent
  comments claim the response cannot exhaust memory and is byte-capped.
- Why it might be intentional: the allowlist is expected to be a small UTF-8 YAML
  document and HTTPS-only; no test or documented contract was found that treats
  character count as the intended unit.
- Severity if real: medium — a configured allowlist endpoint could return a much
  larger body than the stated guard before rejection.
- Confidence: high.
- Estimated fix complexity: small.
- Routed to: `audit-security-code` because the helper gates executable extensions
  and the consequence is security/resource shaped.
- Extraction impact: none; behavior will be retained unchanged, or the helper will
  remain in the facade if its seam is not selected.

## Gate 2 — Baseline and extraction plan

### Pre-edit validation baseline

Commands ran from the Hermit environment before any production source edit:

| Check | Baseline result |
|---|---|
| `pnpm run format:check` | exit 1: 59 pre-existing Desktop files require Prettier; `src/main.ts` is not one of them |
| `pnpm run lint:check` | exit 1 after TypeScript and ESLint passed; `i18n:check` reports `src/i18n/messages/en.json` is already out of date |
| `pnpm run test:run` | exit 0: 135 files, 1,079 tests passed |
| `pnpm exec vite build --config vite.main.config.mts` | exit 0; production bundle completed with existing client-directive/sourcemap/chunk-size warnings |

The two failures are attributable pre-existing repository-wide gates and do not
touch the selected file. Type checking, ESLint, all unit tests, and bundling
provide a credible behavior-preservation baseline. Final verification must
reproduce the same outcomes or improve only through an explained structural
effect; this run will not format unrelated files or refresh unrelated i18n output.

### Extraction plan

`src/main.ts` remains the full Desktop executable **compatibility facade**. It
retains entrypoint-only ordering, Electron application listeners, startup/shutdown
orchestration, and private callback state. Extracted modules receive explicit
dependencies and are registered from the same top-level/app-ready phase as today.

1. `src/main/menuLocalization.ts` — the Simplified-Chinese native-menu dictionary
   and pure locale/label translation. Facade keeps locale resolution and Electron
   menu traversal. Direct unit coverage pins locale fallback and translation.
2. `src/main/gitIpc.ts` — Git process helpers and the four authorized Git IPC
   registrations. Facade delegates registration with its renderer path guard.
   Direct unit coverage pins option ordering and branch validation.
3. `src/main/backendCertificateTrust.ts` — certificate trust records,
   fingerprint normalization, TOFU pinning, and per-session verifier installation.
   Facade keeps Electron's application-level `certificate-error` listener.
4. `src/main/allowlist.ts` — HTTPS allowlist fetch/parsing behavior. BUG-001 moves
   unchanged. The facade's existing IPC handler delegates to the module.
5. `src/main/rendererIpc.ts` — React-ready delivery, external URL, directory/recent
   directory, settings/research-library, ACP URL, and MCP proxy registrations;
   Git registration is delegated to seam 2. One responsibility: renderer-to-main
   application/session adapter registration.
6. `src/main/systemIpc.ts` — menu-bar, dock, notifications settings, wakelock,
   spellcheck, focus, and fullscreen registrations.
7. `src/main/fileIpc.ts` — native pickers, bounded file/artifact operations,
   clipboard, message box, artifact routing, Ollama probe, and allowlist IPC.
8. `src/main/windowChrome.ts` — quick launcher, tray, show-window, recent-directory
   menu, and native directory chooser. It receives the facade's window/settings
   callbacks so backend/window ownership does not move.
9. `src/main/applicationMenu.ts` — menu augmentation and translation after the
   facade has created the first window. It receives callbacks for window creation,
   directory selection, launcher, and focus.
10. `src/main/appIpc.ts` — post-ready app/window IPC listeners (create/close window,
    notifications, renderer logging, theme/workspace broadcasts, reload/restart,
    version/locale, and explorer opening).

Every seam is capped below roughly 400 lines and 15 owned top-level symbols.
Leaf/pure seams run first, registration/orchestration seams last. After each seam:

- run Prettier on touched files only;
- run TypeScript check and focused module tests when present;
- inspect direct facade registration and the second-level preload/renderer channel;
- inspect the actual diff and single-owner searches;
- checkpoint with a git commit only after the seam is verified.

Final validation reruns the exact baseline set, adds focused facade/module tests,
checks the Vite/Forge entrypoint and IPC channel references two levels deep, and
executes the complete MOD-V01..10 sweep. Import-time listener order and module
identity are explicit risks; public renderer channel strings and preload contracts
must not change.

## Gates 3–5 — Seam checkpoints and intermediary audit

### Seam 1 — Native menu localization

- New owner: `ui/desktop/src/main/menuLocalization.ts`.
- Facade surface: private `menuT` and `translateMenuLabels` wrappers stay in
  `main.ts`; the executable entrypoint imports the pure translation functions.
- Direct check: `menuLocalization.test.ts` passes 3/3 for Simplified Chinese,
  POSIX locale normalization, Traditional Chinese exclusion, fallback labels, and
  nested menu traversal.
- Connection check: the only second-level consumer remains application-menu and
  context-menu construction in `main.ts`; searches show one dictionary owner and
  one facade traversal wrapper.
- Validation: touched-file Prettier clean; TypeScript check clean; focused Vitest
  3/3; `git diff --check` clean.
- Intermediary audit: no behavior drift, duplicate dictionary, stale import, public
  contract change, or MOD-B suspect. The facade marker is present. Test assertions
  were added, not weakened.

Checkpoint: `1fac8bd33` (source/tests), with the initial gated log tracked in
`735b65355` because the repo ignores newly created session logs by default.

### Seam 2 — Authorized Git IPC

- New owner: `ui/desktop/src/main/gitIpc.ts`.
- Facade surface: one `registerGitIpcHandlers(ipcMain,
  assertRendererFileAccess)` call remains at the original registration altitude.
- Direct check: `gitIpc.test.ts` passes 3/3 for hardening-option order, branch
  validation, and the exact four renderer channel names.
- Connection check: the facade supplies the same renderer path authorization;
  preload/renderer channel strings remain unchanged; subprocess calls remain
  option-separated and repository-scoped.
- Validation: touched-file Prettier clean; TypeScript check clean; focused module
  suites 6/6; one owner for each Git helper and channel registration.
- Intermediary audit: no command argument, timeout, error mapping, or branch-switch
  behavior changed; no duplicate handlers, new process leak, or MOD-B suspect.

Checkpoint: `011e45a90`.

### Seam 3 — Backend certificate trust

- New owner: `ui/desktop/src/main/backendCertificateTrust.ts`.
- Facade surface: the application-level `certificate-error` listener remains in
  `main.ts`; it imports the same trust/verification functions. Window/backend
  startup still installs the per-session verifier through the facade.
- Direct check: `backendCertificateTrust.test.ts` passes 3/3 for hexadecimal and
  sha256 normalization, case-insensitive host matching, exact pin rejection,
  release, and first-use pinning.
- Connection check: certificate-error, external backend setup, local TLS startup,
  and partition verifier installation retain the same calls and result codes.
- Validation: touched-file Prettier clean; TypeScript check clean; focused module
  suites 9/9; trust state has one owner.
- Intermediary audit: no event registration order, certificate result code, TOFU
  semantics, or cleanup behavior changed; no module-identity hazard or MOD-B suspect.

Checkpoint: `d3c4a5676`.

### Seam 4 — Extension allowlist retrieval

- New owner: `ui/desktop/src/main/allowlist.ts`.
- Facade surface: the existing `get-allowed-extensions` handler imports
  `getAllowList`; registration order and channel are unchanged.
- Direct check: `allowlist.test.ts` passes 3/3 for absent configuration, HTTPS
  YAML command extraction, and rejection before fetch for non-HTTPS sources.
- Connection check: renderer IPC still reaches the same helper and environment
  variable; YAML parsing and error/empty-list behavior moved verbatim.
- Validation: touched-file Prettier clean; TypeScript check clean; focused module
  suites 12/12; one allowlist helper owner.
- Intermediary audit: BUG-001 remains intentionally unchanged and routed; existing
  comments moved with the code. No accidental security fix, timeout drift, or stale
  YAML import was introduced.

Checkpoint: `0bbf3f2df`.

### Seam 5 — File and artifact IPC

- New owner: `ui/desktop/src/main/fileIpc.ts` (under 400 lines).
- Facade surface: one `registerFileIpcHandlers` call supplies the existing path
  guards, grant registries, artifact-routing validator, and allowlist helper.
- Direct check: `fileIpc.test.ts` proves the exact 18 original channel names are
  registered once and in the same order. Existing comments moved with their code.
- Connection check: direct renderer/preload strings, filesystem authorization,
  native dialogs, bounded reads, artifact routing, shell open/reveal, clipboard,
  Ollama child processes, and allowlist delegation were traced two levels deep.
- Validation: touched-file Prettier clean; TypeScript check clean; focused module
  suites 13/13; facade reduced from 3,289 to 2,979 lines.
- Intermediary audit: no path guard, byte limit, timeout, process cleanup, return
  shape, channel, or error mapping changed. One registration owner remains; no new
  MOD-B suspect surfaced.

Checkpoint: `dfa9595ae`.

### Seam 6 — System IPC

- New owner: `ui/desktop/src/main/systemIpc.ts`.
- Facade surface: one `registerSystemIpcHandlers` call supplies settings, tray,
  focus, and wakelock ownership without moving those lifecycle states.
- Direct check: `systemIpc.test.ts` proves all 12 original channel names register
  once and in the same order.
- Connection check: renderer controls still reach native menu/dock visibility,
  platform notification settings, wakelock tracking, spellcheck, and window state
  through the same channels and facade-owned state.
- Validation: the first type check caught dependency evaluation of the private
  `focusWindow` const before initialization. Converting that equivalent private
  helper to a hoisted function preserved registration order; the rerun is clean.
  Touched-file Prettier clean; focused module suites 14/14.
- Intermediary audit: no settings default, platform command, delay, wakelock map,
  return value, or channel changed. Existing platform comments moved with the code;
  no MOD-B suspect surfaced.

Checkpoint: `19c23e4a2`.

### Seam 7 — Renderer/session IPC

- New owner: `ui/desktop/src/main/rendererIpc.ts`.
- Facade surface: one registration call supplies pending renderer/deep-link state,
  directory grants, path authorization, backend leases, and the facade's external
  URL/session dispatch callbacks. Git IPC registration is delegated inside this
  module at the same relative point.
- Direct check: `rendererIpc.test.ts` pins loopback-only ACP URL conversion and the
  readiness listener plus 11 original handler names (including the four Git names).
- Connection check: renderer readiness → pending message/deep link, directory/recent
  paths, Git, ACP token subprotocol, and MCP proxy CSP parameters were traced through
  preload/renderer callers and backend leases.
- Validation: TypeScript clean. The first focused test run exposed an incomplete
  Electron mock (`recentDirs` reads `app.getPath` at import); the test harness was
  corrected without source change. Focused module suites then passed 16/16.
- Intermediary audit: no listener/handler order, channel, token location, loopback
  restriction, pending-state deletion, or proxy parameter changed; no MOD-B suspect.

Checkpoint: `b13a338b1`.

### Seam 8 — Settings and research-library IPC

- New owner: `ui/desktop/src/main/settingsIpc.ts`.
- Facade surface: one `registerSettingsIpcHandlers` call supplies settings state,
  locale/shortcut/update callbacks, the external backend secret bridge, and the
  renderer directory-grant registry.
- Direct check: `settingsIpc.test.ts` pins the exact six original settings and
  research-library handler names in registration order.
- Connection check: renderer/preload calls still reach the same typed settings
  validation, secret redaction/persistence, native folder chooser, directory grant,
  and research-library listing behavior.
- Validation: touched-file Prettier clean; TypeScript check clean; focused module
  suites 17/17. The private shortcut-registration const became an equivalent
  hoisted function so the facade can pass it at the unchanged registration point.
- Intermediary audit: no setting validation, secret handling, native dialog option,
  directory resolution, registration order, channel, or return value changed; no
  new MOD-B suspect surfaced.

Checkpoint: `2c0438295`.

### Seam 9 — Window chrome

- New owner: `ui/desktop/src/main/windowChrome.ts` (under 400 lines).
- Facade surface: `createWindowChrome` returns launcher, tray, recent-directory,
  native chooser, and tray-presence callbacks while keeping app/window creation and
  renderer authorization dependencies explicit.
- Direct check: `windowChrome.test.ts` pins the complete facade callback surface and
  the initial no-tray lifecycle state.
- Connection check: app startup, settings IPC, application menus, global shortcuts,
  and `window-all-closed` still reach the same launcher/tray/dialog behavior. The
  Win32 tray click callback remains private to the extracted owner.
- Validation: touched-file Prettier clean; TypeScript check clean; focused module
  suites 18/18. An initially exposed but facade-unused `showWindow` callback was
  kept module-private after TypeScript identified the unnecessary facade binding.
- Intermediary audit: no BrowserWindow option, tray path, menu shape, filesystem
  check, directory grant, recent-directory write, window placement, or quit rule
  changed; all original explanatory/security comments moved with their code and no
  new MOD-B suspect surfaced.
