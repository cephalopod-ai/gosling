# Code polish report

Date: 2026-08-27

## Summary and scope

This behavior-preserving pass covered owned Rust source and tests, Desktop and
documentation validation, repository Markdown, and governance evidence.
Generated, vendored, build, cache, lock, historical audit, and advisory Giles
surfaces were excluded from direct cleanup.

The patch removes two obsolete commented-code/debug blocks, removes three
restating comments, clarifies two private test symbols, assigns stable IDs to
nine retained source TODO groups, and repairs stale code/documentation evidence.
No runtime branch, public API, dependency, file path, schema, or migration was
changed.

## Naming, headers, comments, and dead code

- Private test renames are recorded in `rename-manifest.md`; exact old-name
  searches and the Rust suite verify their references.
- No file/directory or public symbol was renamed.
- The repository has no per-file header convention, so `source-header-policy.md`
  records the repo-authority exception and no generic headers were added.
- Removed a nonexistent OpenAI context-probe call left as commented code and a
  test-only debug-print recipe. Git history retains both.
- Reworded the todo extension's misleading type comment and removed comments
  that merely narrated construction and retrieval.

## TODO disposition

Every actionable Rust `TODO` now uses an ID listed in `todo-ledger.md`. The SQL
rollback helper's generated `-- TODO:` output and built-in todo-extension test
strings are product behavior/data, not floating source debt.

## Architecture and layout observations

Oversized files are recorded in `structure-review.md` and routed to
`repair-source-modularization`. Cross-crate conversation ownership and remaining
process-global path state remain architecture work in the active backlog. No
module move, duplicate-cluster consolidation, or boundary redesign was mixed
into this pass.

## POL coverage table

| Code | Disposition | Evidence / action |
|---|---|---|
| POL-001 | finding repaired | Two private test symbols now describe the behavior they exercise; references were updated and scanned. |
| POL-002 | clean | File and directory naming sampled across crates, UI, docs, tests, and scripts; no low-risk violation warranted a rename. |
| POL-003 | n/a by repo convention | No established per-source-file header convention; no misleading ownership blocks added. |
| POL-004 | clean in changed scope | No public API was added or changed; touched public items already describe their non-obvious contract. |
| POL-005 | finding repaired | Reworded the misleading todo-extension implementation comment and corrected stale v29/1,888 documentation. |
| POL-006 | finding repaired | Removed the commented-out OpenAI probe and debug-print recipe. |
| POL-007 | finding repaired | Removed test-only instructions for printing a debug report; intentional CLI output remains. |
| POL-008 | finding repaired | Removed the obsolete nonexistent probe reference; formatter and Clippy report no new unused code/imports. |
| POL-009 | finding repaired | Nine actionable TODO groups have stable IDs and ledger dispositions; product-string exceptions are documented. |
| POL-010 | clean in changed scope | Literal review found no extraction that would add semantic information without widening scope. |
| POL-011 | clean in changed scope | Debug/log scan found intentional user and scenario output only; no secret/noise cleanup was warranted. |
| POL-012 | clean in changed scope | No broad-catch or error-text change was safe or necessary; semantics were left untouched. |
| POL-013 | finding repaired | Renamed two vague private tests/helpers; a snapshot-key rename was detected by validation and reverted. |
| POL-014 | finding repaired | Updated the architecture schema label, test count, indexes, README MCP link/example, and stewardship evidence. |
| POL-015 | clean | `cargo fmt --all -- --check` passes after configured formatting. |
| POL-016 | routed | Files of at least 2,000 lines are listed in `structure-review.md` for `repair-source-modularization`. |
| POL-017 | routed | Provider-domain ownership and global path state remain in the architecture backlog; no boundary change was made. |
| POL-018 | clean/routed | No small identical helper was safe to consolidate; broader clusters belong in `audit-deadcode-cleanup`. |

## Baseline comparison

| Lane | Pre-polish | Post-polish | Delta |
|---|---|---|---|
| Rust | Format, build, `cargo test -p gosling`, and all-target Clippy passed | Same exact lane passed; 1,747 core tests plus integration/doc tests | None after reverting the snapshot-identity rename discovered by the first repetition. |
| Desktop | Typecheck and 1,071 tests passed | Typecheck and 1,071 tests passed | None. |
| Documentation | Typecheck, 16 tests, and 165-page production build passed | Same commands and counts passed | None. |
| Unused dependencies | `cargo machete` unavailable | Still unavailable; not rerun as success | None; tooling gap remains explicit. |

## Diff audit

The complete tracked diff and all new Markdown files were read after
validation. Every hunk maps to POL-001, POL-005–009, POL-013–015, or to a
required polish/stewardship manifest. No behavior-changing or unrelated hunk
remains. `git diff --check` passes.

## Validation commands

- `source bin/activate-hermit && cargo fmt --all -- --check && cargo build && cargo test -p gosling && cargo clippy --all-targets -- -D warnings`
- From `ui/desktop`: `source ../../bin/activate-hermit && pnpm run typecheck && pnpm run test:run`
- From `documentation`: `source ../bin/activate-hermit && npm run typecheck && npm test && npm run build`
- Exact-name, TODO-ID, documentation-link, and Tagteam residual reference scans.

## Remaining risks and compatibility

Public APIs and serialized identities are unchanged. Remaining work is limited
to the active ledgers: dedicated modularization/architecture efforts, missing
dependency tooling, upstream documentation advisories, remote platform
confirmation, Giles repair/rescan, and maintainer-controlled release actions.
