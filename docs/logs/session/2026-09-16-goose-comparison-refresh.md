# 2026-09-16 — Goose comparison refresh and stale documentation corrections

## Task

The user asked for the stale documents to be updated, especially the Goose comparison
(`DOC-CMP-001`). The comparison was pinned to Goose v1.49.0 and gosling v1.2.3 as of 2026-09-08.

## Sources checked

- Goose: GitHub's release API returned `v1.50.1` (published 2026-09-14) as the latest
  non-prerelease. It was shallow-cloned at `881f96c00d618ab6fca9b2aaa3a0abf07673cc2c` into the
  session scratchpad. The `v1.50.0` and `v1.50.1` release notes were read to find candidates.
  These were treated as leads, and each claim was checked against the source.
- gosling: `main` at `274955f7b`, manifests at `1.2.5`.
- Every Goose evidence path from the old page still resolves at the new commit. Each existing
  row was rechecked on both sides: inspector error handling, `never_allow` precedence, the
  five-entry recent-model list, the Git branch switch, the OAuth client fields, hook events, and
  local inference.

## Corrections and additions

- **CLI row was wrong, not just stale.** It listed `review` and `term` as gosling-only. Both were
  already registered in Goose v1.49.0 (verified at `71fc4be1`), and Goose's feature-gated `roam`
  was missing. The row now lists the shared commands, Goose's feature gates, and the commands only
  gosling has (`project`/`projects`, `secret`, `shell-validate`, `tui`).
- **New row: fast-model routing.** Goose v1.50.0 removed it (#11469), and no `fast_model` remains
  in the pinned source. gosling keeps it in `crates/gosling/src/model_config.rs`.
- **New row: peer-to-peer agent access.** This covers the optional `goose-roaming` crate. gosling
  has no equivalent; remote clients use `gosling serve`.
- **Local inference row** now notes that v1.50.0 removed the managed model registry.
- **v1.2.5 session features:** handoff, planning, and Context History are mentioned with no claim
  that Goose lacks them, because Goose was not searched for equivalents.
- **`documentation/docs/release-notes/v1.2.5.md` status was false.** It said the candidate had not
  been tagged or published. Tag `v1.2.5` is on origin at `4c3dbdf32` (2026-09-10), and a GitHub
  release page with that name exists but has no assets. Release run `34453362720` failed at
  *Import Apple signing certificate* (both macOS jobs) and *Azure login* (Windows). The status now
  says this, and says that later work is not in the tag.
- Updated the README matrix, `documentation/INDEX.md`, and `documentation/DOCUMENTATION_INVENTORY.md`.
- `docs/TODO.md`: closed `DOC-CMP-001`. Closed `DOC-LOG-001` because its premise was stale: the
  allowlist now runs through 2026-09-16, and `docs/logs/README.md` records the per-file opt-in
  decision.

## Validation

- `node scripts/generate-docs-map.js`: the map regenerated with no diff, because the headings are
  unchanged.
- `pnpm test` in `documentation/`: 16 passed, 0 failed.
- `pnpm exec docusaurus build`: exit 0, with no broken-link or error output.
- No Rust or TypeScript source changed.

## Risks and follow-ups

- Release workflow runs for `v1.2.2`, `v1.2.4`, and `v1.2.5` all ended in `failure`. Signing secrets
  or certificates appear to be missing for this fork. Published release pages therefore have no
  CI-built binaries. Fixing this needs the operator's credentials.
- `DOC-EVID-001` (links to `generated/today-audit/` reports, which git ignores) and `ARC-REG-001`
  still need human decisions.
- The upstream triage watermark is still `v1.49.0`. This refresh compared features and did not
  triage the `v1.49.0..v1.50.1` commits for security ports.
