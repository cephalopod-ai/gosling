# Code polish convention baseline

Date: 2026-08-17

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

- `source bin/activate-hermit && cargo fmt --check` passed.
- `cd ui && node_modules/.bin/tsc --noEmit --project desktop/tsconfig.json`
  passed.
- The initial workspace-root Vitest invocation did not load the Desktop JSDOM
  configuration; final validation uses `ui/desktop` as the working directory.
