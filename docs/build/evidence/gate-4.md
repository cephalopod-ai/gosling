# Gate 4 evidence — backend Workspaces vertical slice

Date: 2026-07-18

## Implemented

- Versioned, backend-owned workspace/profile store under the Gosling data directory with
  owner-only permissions, file locking, truncate-safe temporary writes, fsync + rename,
  corruption backup/recovery, and semantic identity validation.
- Default-workspace migration plus non-secret custom-distribution template materialization.
- Secure credential-profile metadata with namespaced secret references, derived readiness,
  task-local provider scopes, metadata-first fail-closed updates, and restart-drained secure
  deletion intents containing no raw values.
- Workspace validation for path traversal, native-platform availability, missing folders,
  folder identity/default invariants, product outputs, and credential readiness.
- Session schema v22 and migration with nullable legacy workspace fields, immutable workspace /
  profile / context snapshots, workspace-aware session filtering, and copy preservation.
- ACP workspace/profile handlers, new-session workspace resolution, pinned resume behavior,
  generated SDK schema/client updates, and non-secret structured agent context.
- Distribution bootstrap through `GOSLING_WORKSPACE_TEMPLATES` with schema versioning,
  UUID identities, allowlisted `${HOME}`, `${CONFIG_DIR}`, `${DATA_DIR}`, `${CWD}` and `~`
  path expansion, and secret-shaped-field rejection.

## Audit and repairs

The required `audit-dataflow-integrity` v3.1 pass found five defects. All five are fixed and
recorded in `audits/gate-4-data-integrity-audit.md` plus the schema-shaped JSON findings file:

- stale temp files were not truncated;
- credential metadata and secure values lacked a recovery protocol;
- incomplete profiles were reported configured;
- foreign-platform paths could reach native I/O;
- store reads did not reject duplicate canonical identities.

## Commands and results

| Command                                                                     | Result                                |
| --------------------------------------------------------------------------- | ------------------------------------- |
| `source bin/activate-hermit && cargo check -p gosling`                      | pass                                  |
| `source bin/activate-hermit && cargo test -p gosling workspace --lib`       | pass: 23 passed, 0 failed             |
| `source bin/activate-hermit && cargo test -p gosling scoped_declared --lib` | pass: 1 passed, 0 failed              |
| `source bin/activate-hermit && cargo test -p gosling-sdk-types workspace`   | pass: 3 passed, 0 failed              |
| `source bin/activate-hermit && cargo fmt --check`                           | pass                                  |
| `jq empty docs/build/audits/gate-4-data-integrity-findings.json`            | pass                                  |
| `git diff --check`                                                          | pass                                  |
| `cd ui/sdk && pnpm run build:ts`                                            | pass (after generated schema refresh) |

The repository `just generate-acp-types` recipe did not resolve its crate-relative path from
this checkout. The same approved generator entrypoints were run manually from
`crates/gosling` and `ui/sdk`, and the generated SDK TypeScript build passed.

## Structure check

All new workspace backend source files remain below the selected build skill's 800-line hard
limit. The largest is `service.rs` at 687 formatted lines; credential and bootstrap concerns are
separate modules.

## Gate decision

Gate 4 passes. The backend can create, persist, validate, switch, snapshot, filter, resume, and
delete workspaces/profile references without storing raw secrets or mutating historical
sessions.
