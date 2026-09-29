# 2026-09-27/28 — Live playtest and repair campaign

## Task

Run the full 127-card scenario library (`docs/test_scenarios/`) against a clean build of `main`,
then repair the resulting findings in locality-grouped stages with regression tests and live
replay of each original reproduction. Authoritative report:
`docs/cloud/2026-09-27-live-all-scenarios-playtest.md`.

## Run identity

- Playtest baseline: `main` @ `186c96a01` (gosling 1.3.0); local `main` was independently
  fast-forwarded to `eb2561903` mid-pass by another actor (docs + one unrelated `developer/edit.rs`
  fix) — repairs are based on `eb2561903`.
- Run ID `GSL-PT-20260927`. Ten playtest groups (A–H, G1, G2), one per surface area, each with its
  own disposable `GOSLING_PATH_ROOT`, deterministic loopback provider/MCP fixtures, and (for
  Desktop) a dev Electron build driven over CDP. 127 cards: 65 Pass, 8 Partial, 47 Fail, 3 Blocked,
  4 Not executed (Deep Research Desktop cards, not reached before the campaign moved to repair).
- 211 findings recorded (`GSL-PT-20260927-<group><nn>`): 12 High/Critical, 62 Medium, 113 Low,
  24 Notes.

## Repair process

- Findings were grouped by locality (R1a interrupted-turn closure, R1b cancel/lease/shutdown, R2
  compaction, R3 repetition/loop governance, R4 session store, R5 secrets/config, R6 permissions,
  R7 context/isolation, R8a CLI output, R9 configure/providers, R13a Desktop security) and repaired
  in separate git worktrees on short-lived branches (`fix/pt20260927-<group>`), each based on the
  integration branch's current tip so later groups built on earlier fixes.
- Each finished, tested commit was cherry-picked onto `claude/playtest-repair-20260927`
  (this branch) as soon as it was ready, resolving merge conflicts by hand where two groups touched
  the same function (mostly `crates/gosling-cli/src/session/builder.rs`, edited by five groups).
- Per finding: reproduced against current code, a regression test was written that fails before the
  fix and passes after (plus an adjacent-path assertion that behavior elsewhere is unchanged), the
  original playtest reproduction was replayed live against the repaired worktree binary, and
  `cargo fmt` / `cargo clippy --all-targets -- -D warnings` were run on touched crates.
- As of this commit: **89 of 211 findings fixed** (2 more are docs-only with enforcement deferred).
  Every Critical/High finding is fixed except `.goslingignore` enforcement (E01/S19), which the docs
  now correctly describe as unimplemented rather than fixing in-place (it never existed in this
  fork; reimplementing it is a product decision). Repair work continues on this branch.

## Confirmed High/Critical findings and their fixes (this batch)

- **F10 / A12** (Critical in Auto mode) — a cancelled, abandoned, or killed turn's prompt was merged
  into the next turn and re-executed. Fixed by closing an unterminated turn at cancel, at the start
  of the next reply, and on session load/resume (`crates/gosling/src/session/session_manager/turn_closure.rs`).
- **B08** (High) — mid-turn auto-compaction re-appended the in-flight prompt after the summary,
  replaying already-completed tool calls. Fixed in `crates/gosling/src/context_mgmt/mod.rs`.
- **C20** (High) — extension `--secret`/`env_keys` shared one un-namespaced store with the provider
  key. Fixed with per-extension secret storage (`extension-secret::<ext>::<ENV>`).
- **D01** (High) — session IDs were reissued after deletion, letting a stale ID attach to an
  unrelated session. Fixed in `crates/gosling/src/session/session_manager/session_crud.rs`.
- **E01/S19** (High) — `.goslingignore` was documented as a Developer-tool access control but never
  implemented (removed upstream before this fork). Docs corrected; enforcement deferred.
- **E02** (High) — an identical failed/declined tool call was denied for the rest of the session.
  Fixed to remember failures only within the current turn.
- **E03** (High) — one `delegate` approval in Manual mode let the subagent run tools unprompted.
  Fixed: delegation is now refused outside Autonomous sessions.
- **E04** (High) — the working-directory restriction was escaped via `cd ..`. Fixed in
  `crates/gosling/src/permission/working_dir_scope_inspector.rs`.
- **G101** (High) — Desktop's "Create workspace" defaulted to an unconfigured provider and started a
  real OAuth flow. Fixed; model listing for OAuth-backed providers no longer starts interactive auth.
- **G130** (High) — an invalid `GOSLING_MODE` failed open to Autonomous with no warning. Fixed to
  fall back to `approve` with a visible warning.

Full per-finding detail (reproduction, root cause, regression test, before/after live replay
evidence, disposition) is preserved in this session's working notes and will be folded into future
session logs as remaining stages land on this branch.

## Files changed

89 findings' worth of fixes across `crates/gosling`, `crates/gosling-cli`, `crates/gosling-providers`,
and `ui/desktop` (Desktop security fixes only so far), plus:
- `docs/cloud/2026-09-27-live-all-scenarios-playtest.md` (new) — the authoritative playtest report.
- `docs/INTENT.md` — REQ-031 amended to match the corrected workspace-provider default (pending
  operator confirmation).
- Documentation corrections for `.goslingignore`, context-file default order, `environment-variables.md`,
  `workspaces.md`, and several guides touched by the ACP/serve/compaction fixes.

## Validation run

- Per-commit: targeted regression tests (throwaway `HOME` + `GOSLING_PATH_ROOT` +
  `GOSLING_DISABLE_KEYRING=1`), `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check` on
  touched crates.
- Cumulative on this branch: `cargo test -p gosling -p gosling-cli -p gosling-providers --no-fail-fast`
  (throwaway HOME/root) — 2979+ passed, 1 failed. The failure is
  `tests/agent.rs::tool_pair_summarization_tests::test_batch_summarization_preserves_all_summaries`,
  confirmed pre-existing: it fails identically on `eb2561903` with every change on this branch
  reverted, and it is the sole failure in `main`'s own CI.
- `cargo fmt --check` and `cargo clippy --all-targets -- -D warnings` are clean workspace-wide as of
  this commit.
- Desktop (`ui/desktop`): `pnpm run lint:check` and the full `vitest run` suite (185 files / 1465
  tests) pass for the R13a Desktop security fixes.

## Risks / follow-ups

- Full core/CLI/provider suites and Desktop suites should be re-run once the remaining repair stages
  land, before this branch is considered done.
- Product decisions outstanding: `.goslingignore` reimplementation (E01/S19); re-homing a
  workspace-pinned chat after its folder moves (G108, partially fixed); Desktop extension-form
  secret storage still using bare env names; whether `GOSLING_DISABLE_KEYRING` should also gate
  keychain reads for declared `secret_sources` (currently it does not).
- Remaining queued work (not yet on this branch): a handful of configure/provider edge cases, ACP
  protocol findings, remaining extension/subagent findings, several Desktop UI findings, and the 4
  Deep Research Desktop cards the playtest did not reach.
- Two operator actions flagged by the playtest itself (not code): a real ChatGPT/Codex OAuth grant
  was minted in the operator's browser during testing of the now-fixed G101 and may need revoking;
  a small number of empty log files were left in the operator's real state directory by one
  mis-isolated CLI invocation during testing (safe to delete).
