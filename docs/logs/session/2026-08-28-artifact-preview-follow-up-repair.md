# Artifact preview and Deep Research follow-up repair

Date: 2026-08-28

## Task

Repair two regressions observed in Deep Research session `20260828_51`:

- a session output could not be previewed without granting it again through a native file chooser;
- asking for the path of an older output failed completion because Gosling demanded a new Research
  Library copy.

## Selected patch batch

1. **P1 reliability — exact-file preview capability rejected.** The active session published the
   backend-owned artifact path, but Electron accepted it only when it was also under the app's
   launch-time directory roots. This contradicted ADR-0006's transient exact-file capability.
2. **P1 reliability — re-mentioned output treated as current.** Assistant artifact discovery updates
   `last_seen_at` and `source_id` when a later answer mentions an existing path. Completion scoping
   treated that refresh as a new deliverable and required a new archive copy.

## Changes

- Canonicalize and admit existing exact-file capabilities only for the established document,
  spreadsheet, presentation, text, and JSON deliverable extensions. Directory access, source code,
  configuration files, and arbitrary extensions remain denied.
- Scope assistant-message deliverables to the run in which their artifact was first observed.
  Built-in tool writes and modifications continue to use current-run update provenance.
- Added regressions for an output outside launch roots and for a current path-only answer that
  re-mentions an older unpaired output while the session already has a verified report pair.

## Architecture and contract check

- `docs/architecture.md` and ADR-0006 are active and require backend-owned inventory plus transient
  exact-file preview capabilities; the preview repair restores that declared behavior.
- ADR-0016 remains unchanged: genuinely new Deep Research deliverables still require separately
  reported, byte-identical Output and Research Library copies. Gosling does not create or overwrite
  archive files after the fact.
- Pre-repair disposition: evidenced drift from ADR-0006 and incorrect current-run classification
  within ADR-0016's completion gate.
- Post-repair disposition: no new drift; the existing exact-file and dual-copy contracts remain in
  force.

## Validation

- `cargo test -p gosling research_completion::tests -- --nocapture`: pass, 7/7.
- `cargo clippy --all-targets -- -D warnings`: pass.
- Focused desktop artifact and preview suites: pass, 24/24.
- `pnpm test:run`: pass, 1,074/1,074.
- `pnpm run typecheck`: pass.
- `cargo fmt --all` and `git diff --check`: pass.
- `just package-ui`: pass.
- Packaged and installed sidecar SHA-256:
  `087d8abeea788e45f85d5f9bed4251b7ace5b6bfb41be8ef0a10bfcaca527739`.
- Installed `/Applications/Gosling.app` code-signature verification: pass.

## Installation and recovery

- Installed application: `/Applications/Gosling.app`, version 1.1.0.
- Previous application backup:
  `/Users/eric/.local/share/gosling/install-backups/Gosling-before-artifact-preview-fix-20260828-085854.app`.
- The macOS Keychain authorization dialog caused by the rebuilt ad-hoc signature was accepted and
  the installed backend reached its loopback listening state. ScreenCaptureKit returned `-3811`, so
  the preview click-through and a path-only follow-up remain partial live checks.

Final status: `completed_with_partial_verification`
