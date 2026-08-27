# 2026-08-27 — Repository defect closeout

## Task

Review the repository's current defect state, repair every actionable finding,
defer work requiring operator input, validate the result, and publish a final
report.

## Files changed

- `reports/2026-08-27-repository-defect-closeout.md`
- `docs/logs/session/2026-08-27-repository-defect-closeout.md`
- `crates/gosling/src/acp/server/new_session.rs`
- `crates/gosling/src/agents/execute_commands.rs`
- `crates/gosling/src/agents/platform_extensions/developer/process_tree.rs`
- `crates/gosling/src/agents/platform_extensions/developer/shell.rs`
- `crates/gosling/src/config/permission.rs`
- `crates/gosling/src/session/import_formats/mod.rs`
- `crates/gosling/src/session/session_manager.rs`
- `crates/gosling/src/session/session_manager/migrations.rs`
- `crates/gosling/src/session/session_manager/schema.rs`
- `crates/gosling/src/subprocess.rs`
- `CUSTOM_DISTROS.md`
- `README.md`
- `docs/TODO.md`

The continuation fixed a build-breaking Deep Research path comparison, aligned
session mode defaults, distinguished Linux zombies from running processes,
made permission-persist failure coverage root-independent, unified slash-command
registration and dispatch, and corrected two documentation contracts. The
canonical ledger now records these closures and the source-confirmed stale
classifier-signal finding.

## Validation run

- `cargo fmt --all -- --check` — passed.
- Three focused `gosling` library regressions for foreign-import defaults,
  schema defaults, and builtin command registration — passed.
- Full `gosling` library suite with loopback excluded from the environment
  proxy — 1,753/1,753 passed.
- `cargo clippy -p gosling --all-targets -- -D warnings` — passed.
- `git diff --check` — passed before the report write and repeated at closeout.
- `cargo clippy --all-targets -- -D warnings` — blocked by an HTTP 403 while
  downloading the prebuilt `rusty_v8` archive.
- `cd ui/desktop && pnpm run typecheck && pnpm run test:run` — blocked before
  execution because pnpm 10.28.1 is below the declared 10.30.0 minimum.
- `source bin/activate-hermit` — bootstrap blocked by an HTTP 403 from GitHub.

## Risks and follow-ups

The operator-input register and exact validation gaps are recorded in
`reports/2026-08-27-repository-defect-closeout.md`. No blocked check is rounded
up to success, and no deferred design decision is presented as repaired.
