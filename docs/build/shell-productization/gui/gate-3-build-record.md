# Default Shell GUI — Gate 3: build record

Status: implemented and verified in the operator checkout; **not packaged and not CI-bound**
Date: 2026-08-18
Gate: `plan-webapp-design` Gate 3
Implements: [`gate-1-product-workflow-design.md`](gate-1-product-workflow-design.md),
[`gate-2-frontend-handoff.md`](gate-2-frontend-handoff.md)
Requirements: SHP-REQ-058–061

## 1. What exists now

A reusable, generic Default Shell application under `ui/desktop/src/shell-ui/` (31 files), composed
by the neutral consumer through one stable specifier.

| Area | Files | Notes |
| ---- | ----- | ----- |
| State | `state/{types,reducer,transcript,route,store}.ts` | R-1…R-8 from Gate 2 §4 |
| Bridge | `api.ts` | the only module that touches `window.goslingShell` |
| Copy | `copy.ts` | every user-facing string, product name injected |
| Components | `components/*.tsx`, `ShellApp.tsx` | C-01…C-25 |
| Styling | `shell.css` | hand-authored, no framework |
| Entry | `mount.tsx`, `index.ts` | `mountDefaultShell()` |
| Tests | 6 files, 162 tests | see §4 |

Component coverage against the Gate 2 inventory: C-01 `ShellApp`, C-02 `StatusPill`,
C-03 `IdentityBadge`, C-04/C-05/C-06 `ContextBar`, C-07 `SessionPicker`,
C-08/C-09/C-10/C-11 `Transcript`, C-12 `Composer`, C-13/C-14/C-15/C-16 `InteractionDock`,
C-17/C-18 `ModulesStrip`, C-19/C-20 `SettingsPanel`, C-21/C-22/C-23 `LifecycleFailureScreen`,
C-24 `HandoffDialog`, C-25 `FailureBanner`. **C-26 `OutputsPanel` is not built** — see §5.

## 2. Host changes

Four host changes were required. All are generic; none is consumer-specific.

**H-3 — `declaredCapabilities` on the runtime snapshot (implemented).** `ShellRuntimeSnapshot` now
carries the consumer's declared capability list, sorted, projected from the manifest the runtime
controller already held. Without it the renderer could only infer its own capabilities from failed
operations, so it could not distinguish "nothing pending" from "this consumer cannot answer
interactions". It leaks nothing: the renderer *is* the consumer, and the value is its own
declaration.

**H-2 — minimum window size (already satisfied; no change).** `createMinimalShellWindowOptions` in
`ui/desktop/src/shellHost.ts` already sets `minWidth: 760` and `minHeight: 520`. The Gate 2 gap was
wrong on this point; the stylesheet targets those bounds instead.

**`settingsSchema.ts` extraction (new, no behaviour change).** `SettingsPanel` needs the theme list
and the 0.8–2.0 text-scale bounds. Importing them from `localSettings.ts` pulled `node:fs` and
`node:path` into the renderer bundle — a real boundary breach, caught by running the renderer build
(§4). The schema constants and validators now live in `ui/desktop/src/shell/settingsSchema.ts`,
which has no Node or Electron import, and `localSettings.ts` re-exports every symbol so no existing
importer changed. There is still exactly one source of truth.

**Composition seam.** `vite.shell.renderer.config.mts` aliases `@gosling-shell-ui` to
`src/shell-ui`, and `shell.html` links `./src/shell-ui/shell.css`. The neutral consumer's
`renderer.ts` calls `mountDefaultShell(...)`. A consumer may ignore the alias entirely and render
its own tree; the host gained no consumer-specific branch.

### Two CSP-driven decisions

`shell.html` declares `style-src 'self'; script-src 'self'`.

- **No `@vitejs/plugin-react`.** Its dev preamble injects an inline script, which `script-src 'self'`
  forbids. Vite's esbuild transform handles `.tsx` through `jsx: react-jsx` in `tsconfig.json`, so
  the shell builds and serves without it. The cost is no fast-refresh in shell dev mode.
- **Stylesheet linked from HTML, not imported from JS.** A JS `import './shell.css'` becomes an
  inline `<style>` element in dev, which `style-src 'self'` forbids. An HTML `<link>` stays a link in
  both dev and build. The only runtime style write is the text-scale custom property, set through the
  CSSOM, which CSP does not restrict.

## 3. Defects found by auditing this build

Four defects were found and fixed before this record was written. Each has a regression test.

| Finding | Severity | Disposition |
| ------- | -------- | ----------- |
| `SettingsPanel` value-imported `localSettings`, pulling `node:fs`/`node:path` into the renderer bundle. Vite externalised them, so a call would have thrown at runtime in the renderer. | high | fixed by the `settingsSchema` extraction; the renderer build now reports zero externalised modules, and the negative-space suite no longer whitelists that module |
| `repairTranscript` read the session id from the runtime snapshot, which main publishes asynchronously. Resuming a session therefore skipped its own transcript replay. | high | fixed: resume passes the id from the operation result |
| The single-use interaction guard used React state, so two clicks inside one batch could both observe the stale value and send two responses for one `actionId`. | medium | fixed: the guard is a ref |
| The usage meter set a `data-percent` attribute that no CSS rule consumed, so the bar never filled. | low | fixed: width written through the CSSOM |

A fifth finding is a **host** defect, not a GUI one, and is recorded as **SHP-DEF-055**: the host
lists `handoff` in `allowedActions` for `relink_required` and `incompatible`, but `handoff.prepare`
requires a non-empty `sessionId` (`assertString` rejects `''`) **and** a live ACP connection
(`bootstrap.ts` throws when `controller.getAcp()` is null). In both of those states neither exists.
Gate 1 §7 named "Open in Gosling" as the *only* honest action for `relink_required`, so this is a
gap in the recovery story, not a cosmetic issue. The GUI now refuses to render a control that cannot
work and explains what to do instead; the underlying contradiction is left for the operator to
decide, because fixing it properly means letting a handoff envelope be prepared without a server,
which ADR-0012 assigns to the server.

## 4. Verification performed

Verified in the operator checkout with the repository's Hermit-managed toolchain. The earlier
reconstructed-environment limits and the remaining packaging/CI gaps are preserved in §6.

| Check | Result |
| ----- | ------ |
| `tsc --noEmit` over `src/**` | pass, 0 errors |
| `vitest run src/shell-ui/` | pass, 162/162 |
| `vitest run src/shell/` (existing foundation suite) | pass, 188/188 |
| `vitest run` (whole Desktop suite) | pass, 974/974 across 118 files |
| `shell:test-profile` | pass, 57/57 |
| Default consumer conformance | pass, no findings |
| Real shell renderer build via `vite.shell.renderer.config.mts` | pass; `shell.html` 0.73 kB, CSS 11.29 kB as a linked asset, JS 239.72 kB |
| Renderer bundle free of externalised Node modules | pass, 0 warnings (was 2 before the `settingsSchema` fix) |
| Consumer/profile resolution through the strict resolver | pass — the build only runs once `resolveConsumerManifest` accepts the manifest, profile, icons, and Git metadata |
| `cargo fmt --all -- --check` | pass |
| `cargo clippy --all-targets -- -D warnings` | pass |

Test coverage of the Gate 1 and Gate 2 obligations:

- every one of the 11 `ShellLifecycleStateName` values renders;
- all 4 directory states, 3 credential catalog statuses, 4 credential selection statuses,
  5 settings-recovery statuses, 3 module statuses, and 4 adapter statuses render;
- route derivation asserted for every lifecycle state and every precedence rule;
- transcript ordering proved over 5 arrival permutations, with duplicate and gap cases;
- `preservesDraft` restores the exact submitted text; the draft survives a generation bump;
- a streamed tool update does not dismiss a pending interaction (SHP-DEF-052 regression);
- a double click sends exactly one interaction response;
- no retry control in `relink_required`, `incompatible`, or `fatal`;
- no handoff control where handoff cannot succeed (SHP-DEF-055);
- capability gating: an undeclared capability is never invoked and its control is absent;
- accessibility A-2, A-3, A-4, A-5, A-8, A-9 asserted in jsdom;
- negative space: no `require`, Electron import, `ipcRenderer`, Node builtin, `fetch`,
  `XMLHttpRequest`, `WebSocket`, `localStorage`, `sessionStorage`, `eval`,
  `dangerouslySetInnerHTML`, `innerHTML` write, or `process` reference anywhere in the kit; no
  `resolvedPath`, `baseWorkingDir`, `serverSecret`, `acpUrl`, or secret-shaped field; exactly one
  module reaches `window.goslingShell`; host shell modules are imported for types only.

## 5. What is deliberately absent

- **Outputs (C-26 / S-29).** Still blocked by **SHP-DEF-054**. Closing it needs a *Rust-owned* safe
  projection, not an Electron-side filter: `SessionArtifactDto` carries `resolvedPath` and
  `baseWorkingDir`, and `display_path` can itself be absolute
  (`DiscoveredArtifact::from_path` normalises an absolute input rather than rejecting it). DS-4 set
  the precedent that a catalog projection is built field by field in Rust so a future field cannot
  reach a shell by merely existing. Doing it in Electron would create a second authority. The SDK is
  generated from Rust, so a new method cannot be added correctly without running the Rust schema
  generation. That work is outside Gate 3's renderer scope.
- Everything in the v1 negative-space list (Gate 1 §13). No developer tools, no global settings, no
  file tree, no arbitrary backend URL, no "always allow", no telemetry.
- Any named shell. No DAWES, math, Project ABC, or Physics/CST concept appears in the kit, the
  fixture, or the copy.

## 6. Initial reconstruction limits and remaining gaps

Before integration, the implementation was verified through a reconstructed Linux copy because the
original execution bridge could not load the operator checkout's native Node modules. The following
limitations describe that initial pass and are retained as provenance:

1. **Dependency versions are not the operator's.** The container installed with
   `--no-frozen-lockfile`, so resolved versions may differ from `ui/pnpm-lock.yaml` — `vitest`
   resolved to 4.1.0, `vite` to 7.3.1. The suites must be re-run against the committed lockfile.
2. **Electron packaging dependencies were removed** from the container's copy of
   `ui/desktop/package.json` so the install could complete (`@electron/rebuild` pulls a GitHub
   tarball this network blocks). That edit was **not** transferred; the repository's `package.json`
   is untouched. It also means no packaging, Forge, or `shell:package-local` path was exercised.
3. **The container ran as root**, so `src/shell/localSettings.test.ts`'s permission-denial case
   cannot fail closed: it `chmod`s a directory to `0o500` and expects the write to throw, which root
   bypasses. That was the one failing test in the initial reconstructed pass; it passed later in
   the operator checkout.
4. **Three suite load errors** (`App.test.tsx`, `OnboardingGuard.test.tsx`,
   `ProviderConfigurationModal.test.tsx`) come from PNG assets excluded from the file transfer, not
   from any change here. They resolve on a complete checkout.
5. **No Rust was compiled.** No crate was edited, but no `cargo build`, `cargo test`, `cargo fmt`, or
   `cargo clippy` ran.
6. **No packaged run, no cross-platform run, no CI.** The packaged renderer-to-backend startup
   replay, the close/restart cleanup replay, the coexistence matrix, and every CI workflow remain
   unexecuted for this revision.

The operator checkout was subsequently revalidated with the committed lockfile and
Hermit-managed pnpm 10.30.3. TypeScript, the targeted and full Desktop suites, shell profile tests,
consumer conformance, the real renderer build, Rust formatting, and full Clippy all passed as
reported in §4. That closes initial limitations 1, 3, 4, and 5. Packaging, real-window walkthroughs,
cross-platform replay, and CI remain unverified.

## 7. Operator commands to reproduce §4 locally

```bash
cd /Users/eric/Work/vscode/forked/gosling
source bin/activate-hermit
pnpm --dir ui install --frozen-lockfile
pnpm --dir ui/desktop run typecheck
pnpm --dir ui/desktop exec vitest run src/shell-ui/ src/shell/
pnpm --dir ui/desktop exec vitest run
pnpm --dir ui/desktop run shell:test-profile
pnpm --dir ui/desktop run shell:check-profiles
pnpm --dir ui/desktop run shell:conformance \
  ../../fixtures/shell-consumers/default-shell-template/shell-consumer.json
GOSLING_SHELL_CONSUMER_MANIFEST=$PWD/fixtures/shell-consumers/default-shell-template/shell-consumer.json \
  pnpm --dir ui/desktop exec vite build --config vite.shell.renderer.config.mts
scripts/test-with-rusty-v8-cache.sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
```

Then run the shell for real (`just run-ui` is full Gosling; the shell needs its own packaged or dev
invocation with the consumer manifest set) and walk the Gate 1 §4 screen list.

## 8. Exit criteria for Gate 3

- [x] Every Gate 1 state has an implemented surface, except the one blocked by SHP-DEF-054.
- [x] Every Gate 2 reducer rule implemented and tested.
- [x] Accessibility criteria A-2…A-5, A-8, A-9 asserted; A-1, A-6, A-7, A-10 remain manual (§9).
- [x] Negative space enforced by executable tests.
- [x] Renderer bundle free of Node and Electron.
- [x] Defects found during the build fixed with regression tests, or recorded as host defects.
- [x] Suites re-run on the operator's checkout against the committed lockfile.
- [ ] SHP-DEF-053 closed (the standing Gate 3 entry condition, still open).
- [ ] SHP-DEF-054 closed and the Outputs surface built.
- [ ] SHP-DEF-055 dispositioned by the operator.
- [ ] Packaged startup, close/restart, and coexistence replays for this revision.
- [ ] Green CI for this revision.

## 9. Still manual

A-1 (full keyboard walkthrough), A-6 (reduced motion), A-7 (usable at 760×520), and A-10 (contrast in
all three themes) are asserted in the stylesheet and structure but not machine-verified. jsdom has no
layout engine, so A-7 and A-10 in particular need a real window. These belong to Gate 5 validation.
