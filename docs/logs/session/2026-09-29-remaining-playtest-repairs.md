# 2026-09-29 — Remaining playtest repairs

## Task

Continue the 2026-09-27 playtest closeout, repair findings that do not require product direction,
and preserve unresolved decisions for the operator.

## Repairs

- G127: a Desktop process launched with `GOSLING_PATH_ROOT` now creates and grants its default
  Research Library inside that isolated root. An explicitly selected library remains authoritative.
- G220: permission dropdowns now use Radix's `asChild` composition, so their existing buttons are
  the triggers instead of being nested inside generated trigger buttons.

## Validation

- `cargo fmt --check` passed.
- `git diff --check` passed.
- Desktop formatting passed for every touched TypeScript/TSX file.
- The targeted Desktop test could not start because this checkout has no installed `vitest`
  executable.
- Workspace Clippy could not download the missing `buffer-redux` crate because the environment's
  network proxy returned HTTP 403.

## Pending product decisions

- E01/S19: decide whether `.goslingignore` should become an access-control boundary. Recommended:
  add enforcement behind an explicit setting that defaults **on**, while retaining a visible
  compatibility escape hatch for workflows that intentionally need ignored files.
- G108: decide whether relinking a moved workspace should re-home pinned historical chats.
  Recommended: offer a confirmation toggle in the relink flow, default **on**, and preserve the old
  path in recovery metadata.
- H02: decide whether failed delegate launch shapes should suppress equivalent retries.
  Recommended: deduplicate normalized launch attempts within a turn by default, with a per-session
  opt-out for diagnostic work.
- D24: decide whether importing a grown transcript should merge new turns into the existing import
  or create a new session. Recommended: merge only an append-only continuation and otherwise create
  a separate session.
- D21: decide whether terminal integration should forward typed shell commands to the model.
  Recommended: expose an explicit opt-in setting that defaults **off**.
- H08/H09: decide whether Gosling should wrap extension contract errors and reserve the normalized
  `dummy` delegate sentinel. Recommended: provide Gosling-owned contract detail and replace the
  sentinel with an unambiguous internal representation.

Other queued Desktop diagnostics remain open in the authoritative playtest register and require a
separate runtime-focused pass; this patch does not represent them as complete.

## Resumed Desktop backend repairs

The sections above retain the earlier September 29 checkpoint. The hoisted workspace Vitest is
available via `pnpm exec`; the earlier executable-discovery limitation is not a current blocker.

Baseline: `/Users/eric/Work/vscode/forked/gosling`, `main` at `1563c008b`, clean tree.
Scope: GSL-PT-20260927-G213 and G215 (Low; reliability/correctness, medium complexity).
The request to resume repairs authorizes these local code changes. Product decisions above remain
protected. Independent reads run concurrently; edits and validation proceed in dependency order.

1. G213: distinguish startup authentication, TLS, HTTP and reachability failures in
   `backendStatus.ts` and the existing main-process dialog; expose ACP connection closure through
   `acpConnection.ts` and a reconnect notice in the Desktop layout, including when no prompt runs.
2. G215: bind persisted renderer session state to the window's backend identity, stable across
   embedded-backend port changes and distinct for external backend base URLs. Preserve old records
   without assigning records of unknown ownership to a new backend.

Contract baseline: DT-09 requires bounded, distinguished faults and backend-owned sessions;
DT-07 and accepted ADR-0013 require session-scoped preview state, metadata-only inventory and
unchanged file authorization. `docs/architecture.md` owns the existing main/preload/renderer
boundary. The observed silent disconnect and shared storage keys are pre-existing contract drift.
No backend protocol, permission, TLS-trust or backend session-persistence contract change is planned.

Baseline validation: `pnpm exec vitest run src/backendStatus.test.ts
src/acp/createWebSocketStream.test.ts src/contexts/ArtifactWorkbenchContext.test.tsx
src/contexts/WorkspaceContext.test.tsx` in `ui/desktop`: **4 files / 26 tests passed**.
Live replay uses disposable fixtures only; existing cards remain unchanged.

### Backend stage checkpoint

G213 now classifies startup authentication, TLS, HTTP/protocol and reachability failures, and
observes idle ACP socket closure. The layout offers a reconnect action that stays visible and
disabled during retry. G215 pins the window's backend namespace from its external URL or effective
local store root, scopes preview state, workspace filters, composer history and archives, and
records backend ownership on crash-recovery markers. Unknown legacy ownership is preserved without
automatic adoption. Recovery checks the actual backend again after startup fallback.

The independent reviewer found and verified repairs for archive basename growth, relative-root
identity, missing recovery ownership and external-to-local recovery fallback. Its final review
found no blocking defect. A separate completeness pass checked settings parsing, same-backend
multiwindow behavior, legacy preservation, IPC ownership and connection-generation invalidation.

- Full Desktop checkpoint: `pnpm exec vitest run`: **211 files / 1,608 tests passed**.
- `pnpm run lint:check`: passed typecheck, ESLint, i18n extraction, 21 locale-sync tests and
  validation of 15 locales. A new test initially used unsupported `Array.at`; it was corrected
  before the passing typecheck and full rerun.
- `source bin/activate-hermit; cargo fmt --check`: passed. No Rust runtime code changed or Rust
  build/test/Clippy run in this continuation.
- Focused Chrome replay passed using real React components and ACP WebSocket transport: idle
  disconnect notice, reconnect, same-session-ID backend isolation, same-backend reload and
  cross-backend tab closure. The fixture received exactly two `initialize` calls and no prompts.
  Python Playwright was unavailable, so the existing Node Playwright package drove Chrome.
  Disposable evidence: `/var/folders/by/x9xn788x3fg1p54wlgwmp0500000gn/T/gosling-backend-repair-02gmk1jx/`
  (`replay.cjs`, saved harness, `results.json` and screenshots). The temporary repository harness
  was removed by the replay's cleanup.

This is component/transport replay, not a complete DT-07/DT-09 or native Electron replay. Native
startup fault dialogs, crash/relaunch and disable-external-backend fallback remain unplayed.
Closed sockets are covered; silent half-open sockets without a close event are not proven.
Relative `GOSLING_PATH_ROOT` plus a session directory different from the backend launch directory
can prevent automatic recovery; ownership checks safely refuse the ambiguous replay. Capturing
the original backend launch directory is a follow-up.

### Remaining queue stage plan

Continue the same eight-item Desktop repair set through its remaining six findings; product gates
above remain protected. All six still have source evidence. Baseline for the next groups:
**9 files / 141 tests passed** covering Git IPC, directory grants, renderer IPC/artifact access,
composer, artifact previews, chat hook and ACP prompt lifecycle.

3. G210 + G134 (Note/P3, security-boundary diagnostics, medium): automatic launch grants must not
   include home/ancestor roots; an optional branch probe outside granted roots must return no branch
   without spawning Git or producing an IPC rejection. Touch `main.ts`, directory-grant registry
   and Git IPC. Preserve native-picker grants and all existing subprocess authorization.
4. G206 + G121 (Note/P3, frontend correctness, medium): preview content must match the active tab
   before media URLs are rendered; mounted hidden composers need unique DOM IDs and only the active
   composer should expose the shared test selector. Touch artifact preview state and ChatInput's
   existing active-session caller. Preserve preview file bytes, draft/queue lifetime and tab state.
5. G216 + G115 (Note/P3, outcome/storage wording, low-to-medium): propagate ACP stop reason to the
   completion hook, exclude failed/cancelled/refused/limited turns, and describe a completed response
   without promising task success. Replace unconditional secure/keychain storage claims with the
   documented keyring/plaintext behavior. Touch prompt completion, notification hook, credential
   profile copy and shared storage notice; synchronize existing localization catalogs.

Interaction: removing automatic broad grants makes the branch probe's normal denial path more
common, so those changes share one gate. Preview loading is independent of the repaired backend
namespace but must retain that isolation. Composer identity must retain inactive queues. Prompt
completion must preserve cancellation, recovery and plan-review behavior. No new permission grant,
backend protocol or product decision is introduced.

### Remaining stage results

| Stage | Disposition and changed surface | Verification and preserved behavior |
|---|---|---|
| G210 + G134 | Repaired automatic startup grants in `main.ts` / `rendererDirectoryGrants.ts`, with explicit native selection carried by `windowChrome.ts`. Git branch IPC returns `null` when authorization denies its optional probe. | Registry, renderer/artifact access and Git tests: 44 passed; final native-menu/registry/Git subset: 15 passed. No denied request spawns Git; canonical authorized paths retain subprocess hardening. Native picker and OS file-open selection reach the new window before navigation; ordinary startup does not grant home. |
| G206 + G121 | `ArtifactPane.tsx` pairs loaded content with its tab object/revision and respects UTF-8 SVG encoding. `ChatInput.tsx` uses unique IDs and active-only test selectors/automatic focus, with `BaseChat.tsx` supplying visibility. | Preview/composer/workbench subset: 74 passed; final preview/composer subset: 58 passed. Regression observes every image `src` assignment, so a transient invalid URL cannot pass by disappearing after the effect. Composer activation preserves the edited draft and stable IDs. Three Chrome rounds decoded PNG, file SVG and inline Unicode SVG: nine images, no console errors or failed requests. |
| G216 + G115 | `chatSessionController.ts` forwards ACP stop reason and credit failures; `useChatSession.ts` only notifies on `end_turn`, using “Gosling finished responding.” Profile and provider notices describe keyring/plaintext storage conditionally. | Controller/lifecycle/hook/profile/provider subset: 67 passed. All five ACP stop reasons, credit exhaustion, notification setting, foreground suppression and plan-review suppression covered; cleanup and `onStreamFinish` preserved. Existing incorrect locale IDs were replaced; all 15 other locales receive explicit English fallback for the two corrected messages. |

The independent stage review caught a native selection handoff regression and automatic-focus
paths that bypassed inactive state. Both were corrected and regression-tested. The final reviewer
reported **no blocking findings** across all eight repairs. Its source review did not stand in for
the executed tests. Follow-up completeness inspection traced the trusted native option through
main-process callers and confirmed renderer IPC cannot supply it; verified artifact cancel/reload
paths, inactive draft persistence, completion/cancellation ordering and all source dispositions.

Browser evidence for G206 is in the same disposable directory as the backend replay:
`preview-replay.cjs`, `preview-harness.tsx`, `preview-results.json`, `preview-browser-errors.json`
and `preview-switching.png`. It renders the real ArtifactPane and workbench, with fixture Electron
reads and a stubbed save-routing dependency; it is not the installed Electron application. An
initial run decoded all nine images but failed its strict console check on the harness's missing
favicon; the harness now supplies a data favicon, and the clean rerun passed. Repository harness
directories were removed and no provider prompt or operator document was used.

### Final validation and closure

- `cd ui/desktop && pnpm exec vitest run`: **211 files / 1,628 tests passed** after the stage and
  focus repairs. A final explicit `return undefined` in the focus effect satisfies the compiler's
  return-path check without changing runtime behavior; the composer subset passed **3 tests** afterward.
- Desktop typecheck, ESLint and i18n extraction/sync/locale checks **passed** for the final source.
  In addition to the earlier `Array.at` correction, an unqualified browser constructor in a new
  test was changed to `window.HTMLImageElement`, and the focus effect's explicit no-cleanup return
  resolved `TS7030`.
- `pnpm run i18n:compile` passed for the synchronized message catalogs. Existing translations for
  unrelated keys were retained; the catalog tool kept its recovery copy.
- Touched TypeScript/TSX formatting, Rust formatting, `git diff --check`, six added/updated Markdown
  links/anchors and the unchanged governance markers **passed** separately from runtime verification.
  No Rust source, schema or provider dependency
  changed; no Rust build/test/Clippy, full 127-card replay, package or install was performed.

Source records refreshed: the original playtest register retains each observation and now links
its repair evidence; `docs/TODO.md` records all eight dispositions; the active TODO ledger retains
G210/G213/G215/G216 native-verification exits; the test ledger, architecture description and docs
index reflect the source behavior. Existing product decisions remain unchanged. The historical
September 27 checkpoint and earlier September 29 test limitations remain clearly dated evidence.

The final architecture/contract pass used the repair skill's binding drift contract against
DT-03/06/07/09/10 and ADR-0013. The fixes restore existing access, session ownership, preview and
honest-notification expectations; no new architectural decision is claimed. Unverified native
behavior remains explicit rather than being promoted to passing scenario-card results.

Residual work: native startup authentication/TLS/reachability dialogs, crash/relaunch including
external-disable fallback, default launch/native picker handoff and macOS notification delivery;
the relative-root recovery limitation above; translated versions of the two English fallback
messages; and the previously protected product decisions. Silent half-open socket detection is
not established by the closed-socket replay. No further actionable source item remains queued in
this eight-finding Desktop set.

Final status: **completed_with_partial_verification** — all eight queued source repairs landed
locally with regression coverage; native end-to-end verification remains pending. No commit or
push was requested or performed.
