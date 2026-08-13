# Gate 4 evidence — shared Electron host checkpoint

Date: 2026-08-13
Decision: **CONTINUE Gate 4 after local checkpoint commit**
Baseline revision: `269f04b94`
Checkpoint revisions: `842356a53`, `ff4567d74`, `d7e4178a5`, `48b4d5c6c`
Final bootstrap/package checkpoint: recorded by the local commit after this evidence file

## Implemented in this checkpoint

- pre-ready product-specific app identity, paths, partition, protocol, lock, and lifecycle contracts;
- exact eight-channel shell IPC allowlist and dedicated frozen `window.goslingShell` preload;
- main-owned authenticated ACP initialization, method/provisioning/identity/core/schema validation,
  and fail-closed compatibility mapping before session use;
- one backend generation owner with fresh secrets, stale-event fencing, expected/unexpected exit
  handling, ordered cleanup, and deterministic retry;
- dedicated shell main, preload, and renderer Vite entries selected mechanically by Forge;
- secure shell window bootstrap, denied navigation/window creation, second-instance focus, and
  cleanup-before-quit;
- compile-time resource selection and packaged profile/manifest identity/hash validation;
- bounded redacted diagnostics with explicit save dialog and atomic owner-private output;
- one-time exact server-prepared handoff codec/store and explicit main-owned protocol opening;
- tracked local package wrapper that requires one approved non-publishable profile, builds the exact
  host-target `gosling-cli` through the verified V8 helper, stages that artifact, rebuilds the SDK,
  packages without release credentials, and verifies the artifact;
- package readback for profile, full manifest, provisioning, binary hash, dedicated entries,
  forbidden broad preload/renderer authority, updater absence, executable, protocol, and macOS
  bundle metadata;
- fixture macOS bundle IDs corrected to Packager-stable hyphenated values, with validation rejecting
  characters Electron Packager would silently strip.

No signing identity, notarization, publication, upload, updater promotion, production identifier,
release destination, domain shell, domain semantics, or domain workflow was added. Electron Forge's
fuse plugin necessarily restored an ad-hoc macOS signature after changing the arm64 Electron fuse
wire; no release signing identity or team identifier was used.

## Package evidence

The tracked command was exercised against neutral fixture A:

```text
cd ui/desktop
pnpm run shell:package-local -- ../../fixtures/shell-products/fixture-a/product-profile.json --platform darwin --arch arm64
```

Observed result:

```text
productId: gosling-shell-fixture-a
target: macos-arm64
profileHash: bbdc328863718e3a94c2a379140bc16568bab9474be72066fb87bf0a7a9bbe75
binaryHash: 16edd16fe9995bc44c28f131fc64dd7789f8d345639884f9ec6d5708bee96cec
CFBundleIdentifier: io.github.repo-makeover.gosling.fixture.a
TeamIdentifier: not set
signature: ad-hoc (Electron fuse rewrite only)
```

The just-built target binary, ignored staging copy, and packaged resource all had the same binary
hash. Readback found dedicated `main.js`, `shell-preload.js`, and `shell.html`; broad Desktop
`preload.js`, main renderer, updater resource, ACP URL, and secret/token authority sentinels were
absent from the inspected preload/renderer payloads.

## Validation observed

Run through the Hermit environment:

```text
pnpm run shell:test-profile
41 tests passed, 0 failed

pnpm run typecheck
passed

pnpm run lint:check
passed, including 21 i18n transaction tests and 15 locale catalogs

pnpm run test:run
100 test files passed; 655 tests passed

focused shell Vitest
10 files passed; 53 tests passed

focused shell/package ESLint
passed with zero warnings

focused touched-file Prettier check
passed

git diff --check
passed

shell package readback after build
passed with exact profile/manifest/binary/macOS identity
```

The repository-wide `pnpm run format:check` still fails on 62 pre-existing unrelated source files.
It initially also reported touched `src/shell/runtimeController.ts`; that file was formatted and the
focused touched-file check then passed. No unrelated formatting was changed.

## Gate 4 status and residual limits

Gate 4 remains open. The following exit criteria are not yet satisfied:

- session create/resume is not implemented or exercised after preflight;
- no deterministic neutral session probe proves no durable session is created on compatibility
  failure;
- full Gosling does not yet receive `gosling://handoff` through a focused routed module;
- broader renderer recovery/accessibility UI remains Gate 5;
- full packaged workflow, restart, coexistence, and failure matrix remains Gate 6;
- Linux, Windows, and macOS x64 package readback remains Gate 7/CI work;
- remote Linux CI evidence remains blocked because push/PR is not authorized.

This checkpoint proves package construction and integrity readback for the host macOS arm64 neutral
fixture only. It does not claim Gate 4 complete or any target release-ready.
