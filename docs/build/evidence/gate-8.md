# Gate 8 evidence — acceptance and handoff

Date: 2026-07-18

## Acceptance results

| Command | Result |
| --- | --- |
| `source bin/activate-hermit && cargo fmt --check` | pass |
| `source bin/activate-hermit && cargo build` | pass |
| `source bin/activate-hermit && cargo test -p gosling --lib` | pass: 1,481 tests |
| `cargo test -p gosling workspace --lib` | pass: 26 tests |
| scoped cross-instance Config concurrency test | pass: 1 test |
| `cargo test -p gosling-sdk-types workspace --lib` | pass: 3 tests |
| `source bin/activate-hermit && cargo clippy --all-targets -- -D warnings` | pass |
| `(cd ui/desktop && source ../../bin/activate-hermit && pnpm run typecheck)` | pass |
| `(cd ui/desktop && source ../../bin/activate-hermit && pnpm run test:run)` | pass: 63 files, 479 tests |
| `(cd ui/sdk && source ../../bin/activate-hermit && pnpm run build:ts && pnpm test && pnpm run typecheck:test)` | pass: 6 tests plus both typechecks |
| `(cd documentation && source ../bin/activate-hermit && npm test)` | pass: 15 tests |
| `(cd documentation && source ../bin/activate-hermit && npm run build)` | pass: 166 Markdown files exported and static site built |
| `git diff --check` | pass |
| ACP/audit JSON parse with `jq empty` | pass |
| prohibited Desktop API import and Goose runtime-name negative searches | pass |

The first full Desktop run was intentionally concurrent with a cold repository-wide Rust build;
three five-second TLS startup tests timed out under contention. The isolated rerun above passed all
479 tests, so the timeout was recorded as `ACC-GOS-006`, not hidden.

## Acceptance repairs

- Restored repository-wide compilation by importing `anyhow::Context` in the existing CLI review
  orchestrator.
- Resolved all clippy findings: direct session role binding, consolidated template-status logic,
  panic-safe Windows drive normalization, and allocation-free test slice construction.
- Re-ran formatting, the complete gosling library suite, and mandatory clippy after the repairs.

## Known unrelated baseline

`documentation/npm run typecheck` reports existing Docusaurus/React/prompt-model type failures in
configuration and pages unrelated to the Workspaces Markdown. Documentation unit tests and the
production build both pass. The repository-specified Desktop TypeScript check passes.

## Gate decision

Gate 8 passes. Every P0/P1 Workspaces row is verified or explicitly bounded, required Rust/Desktop
checks pass, mandatory clippy is clean, security negative-space checks pass, and remaining product
limitations are documented rather than silently claimed complete.
