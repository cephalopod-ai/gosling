# 2026-08-13 Gosling shell Gate 4 host/package checkpoint

- Task: continue the accepted shared Gosling shell productization plan through the dedicated
  Electron host and local package-integrity checkpoint, excluding every domain shell and all release
  activation.
- Scope delivered: isolated app identity/lifecycle, narrow IPC/preload, main-owned authenticated ACP
  preflight, one backend generation owner, dedicated main/preload/renderer entries, secure bootstrap,
  packaged resources, bounded diagnostics, exact one-time handoff sender, local host-only package
  wrapper, package readback verifier, and neutral fixture macOS identity correction.
- Critical defect closed: packaging no longer trusts the ignored staged `src/bin/gosling`; the
  wrapper builds and stages one exact target artifact and verifier rejects embedded hash mismatch.
- Package evidence: fixture A macOS arm64 profile hash
  `bbdc328863718e3a94c2a379140bc16568bab9474be72066fb87bf0a7a9bbe75`; built/staged/embedded
  binary hash `16edd16fe9995bc44c28f131fc64dd7789f8d345639884f9ec6d5708bee96cec`;
  bundle ID `io.github.repo-makeover.gosling.fixture.a`; no team identifier.
- Signing boundary: no release signing or notarization was invoked. Forge's existing fuses plugin
  restored an ad-hoc local signature after changing the arm64 Electron fuse wire; no credential,
  team, publication, upload, or updater operation occurred.
- Validation: shell profile/package Node suite 41/41; Desktop typecheck; full lint/i18n; focused
  shell Vitest 53/53; full Desktop Vitest 655/655; focused touched-file Prettier; package readback;
  `git diff --check`.
- Baseline caveat: repository-wide format check still reports 62 pre-existing unrelated files;
  touched files pass the focused check. No test ledger update was needed because this repository has
  no `docs/testing/test-ledger.yaml` or equivalent required ledger.
- Gate status: Gate 4 remains open. Session create/resume, deterministic no-session-on-failed-
  preflight proof, and focused full-Gosling `gosling://handoff` receiving remain next. Recovery/
  accessibility UI, packaged restart/coexistence, and cross-platform release workflows remain later
  gates.
- Authority honored: local commits only; no push, merge, production identifiers, release
  destination, release signing, notarization, publication, upload, updater promotion, or domain-shell
  implementation.
