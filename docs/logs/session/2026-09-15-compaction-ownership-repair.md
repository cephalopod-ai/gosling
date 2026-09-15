# 2026-09-15 — Common-issue sweep and CMP-OWN-001 repair

## Task and authority

The user asked for a check of the code for common issues, with patches. The sweep ran the
repository's own gates first, then targeted pattern scans, then closed the one open, well-specified
defect the repository itself documents as patch-ready: **CMP-OWN-001**, the provider-managed
`<compaction>` countdown listed in `docs/TODO.md` and the v1.2.5 Known Issues.

## Sweep results (no defects found)

| Gate / scan | Result |
| --- | --- |
| `cargo clippy --all-targets --locked -- -D warnings` (workspace) | clean before the change; clean after |
| `cargo fmt --all -- --check` | clean |
| Desktop `pnpm run typecheck` | clean |
| Desktop `pnpm run lint:check` (ESLint, i18n, locale catalogs) | clean |
| Desktop `pnpm test -- --run` | 1,367 tests passed |
| `cargo test -p gosling-sdk-types --locked` | passed |
| unwrap/expect scan over runtime code changed since `4c3dbdf32` | only guarded or test-module uses |
| `format!`-built SQL scan | DDL with constant identifiers only; no interpolated values |
| `shell.openExternal` / `dangerouslySetInnerHTML` / `UNIX_EPOCH` unwrap scans | all guarded or absent |
| Manual review: `acp/server/active_runs.rs`, `ContextHistoryDiff.tsx`, context-history CLI | no defects; one duplicated `require_active_run` call in `on_steer_session` noted as harmless |

`cargo-audit`/`cargo-deny` are not installed in this environment, so no fresh dependency-advisory
scan is claimed. The known documentation-site npm advisories remain as recorded in the test ledger.

## CMP-OWN-001 repair

`crates/gosling/src/agents/moim.rs`:

- `inject_moim` now reads the active provider handle once and derives
  `gosling_owns_context` from `capabilities().context_ownership`, the same idiom used by
  `check_if_compaction_needed`, `/compact`, and the summarizer target selection. A missing provider
  handle keeps the previous behavior.
- `compose_moim` takes a `CompactionStatus` struct (total tokens, context limit, threshold,
  ownership) and emits the `<compaction>` countdown only when gosling owns the context. Current
  time, working directory, turn budget, and extension parts are unchanged, so the
  turn-context detector coupling with `gosling-providers` is unaffected.
- Two regression tests pin the projection: a gosling-managed context at 150k/200k tokens keeps the
  countdown; a provider-managed context suppresses it while retaining the rest of the block.

Records updated: `docs/TODO.md` marks CMP-OWN-001 repaired; the v1.2.5 release notes move the
countdown from Known issues to the architecture/maintenance fix list.

## Validation

- `cargo fmt` and `cargo fmt --all -- --check` passed.
- `cargo clippy -p gosling --all-targets --locked -- -D warnings` passed after the change (an
  initial 8-argument signature was rejected by Clippy and refactored into `CompactionStatus`).
- `cargo test -p gosling --lib agents::moim --locked`: 8/8 passed.
- `cargo test -p gosling --lib agents:: --locked`: 365 passed, 3 failed against the operator's live
  configuration root — the three `agents::prompt_manager` snapshot tests already documented as
  environment baseline failures in the 2026-09-15 audit-findings log. With a clean
  `GOSLING_PATH_ROOT`, `agents::prompt_manager` passed 12/12 and the full `agents::` module passed
  368/368.
- Linking on this machine requires `DEVELOPER_DIR=/Library/Developer/CommandLineTools` because the
  Xcode license is unaccepted; the override was per-command and host settings were not changed.
- Not run: the complete workspace test suite, packaged-app runtime verification, and any live
  provider session exercising the injected turn context.

## Risks and follow-ups

- The suppression is derived from the provider handle available to the extension manager at
  injection time. A session mid-provider-transition falls back to the previous behavior until the
  new provider is installed; transitions are already fenced by the operation gate.
- `on_steer_session` in `acp/server/active_runs.rs` validates the active run twice; harmless, left
  in place to avoid changing error ordering without need.
