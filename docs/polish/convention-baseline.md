# Code polish convention baseline

Date: 2026-08-27

## Scope

This baseline covers the Rust workspace, the Desktop TypeScript application, and
the repository documentation changed in this run. Generated files, dependency
trees, build outputs, and historical audit records were not reformatted or
rewritten.

## Observed conventions

- Rust code is formatted with `cargo fmt`; crate and module names use
  `snake_case`.
- Desktop TypeScript uses Prettier, strict TypeScript, and Vitest configuration
  rooted in `ui/desktop`.
- Repository documentation uses Markdown with `docs/INDEX.md` as the local
  entry point and session logs under `docs/logs/session/`.
- The repository has no established per-source-file copyright or license header
  convention.

## Baseline checks

- Rust: `cargo fmt --all -- --check`, `cargo build`, `cargo test -p gosling`,
  and all-target Clippy with warnings denied passed.
- Desktop: `pnpm run typecheck` and `pnpm run test:run` passed from
  `ui/desktop` (134 files and 1,071 tests).
- Documentation: typecheck, 16 tests, and the production build passed from
  `documentation`; the build generated 165 Markdown pages.
- `cargo machete` was not installed, so unused-dependency analysis was not
  represented as passing evidence.
