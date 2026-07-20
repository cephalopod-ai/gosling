# PLAY-GSL defect-repair campaign plan — 2026-07-20

**Status:** `completed_verified`; one confirmed defect repaired, one baseline
repair re-verified, and one speculative finding closed as not a current defect.
**Skill:** `repair-defect-campaign`.
**Findings source:** `docs/cloud/audit-playtest-app.md`, attributed by the user to
the Antigravity/Gemini playtest audit.
**Repository state:** `repair/play-gsl-campaign-20260720` at
`9ad58caf2c72518d4b6df99e8a3428bed8a7a9ee`, branched from clean `main`.
**Git policy:** local stage commits enabled; no push or remote mutation authorized.

## Gate 0 — orientation and baseline

- Repository rules: root `AGENTS.md` and `CONTRIBUTING.md`; Rust changes require
  `cargo fmt`, targeted tests, and clippy before merge.
- Native campaign records live under `reports/`; source finding closure remains
  in the originating audit report.
- Protected deferred work in `docs/TODO.md` is unrelated and remains untouched.
- Baseline checks:
  - `cargo fmt --all -- --check` — passed.
  - `cargo test -p gosling-cli --lib` — passed, 226 tests.
  - Live `PLAY-GSL-002` reproduction with invalid UTF-8 stdin — exit 1 with a
    panic at `cli.rs:1550`, confirming the defect.

## Gate 1 — frozen inventory

| ID | Domain | Priority | Complexity | Touch set | Disposition |
| --- | --- | --- | --- | --- | --- |
| PLAY-GSL-001 | frontend/UX-bug | P2 | low | `session/input.rs`; inline command parsing; input unit tests | Already repaired at baseline commit `9ad58caf`; baseline CLI tests pass. Source record already carries repair status. |
| PLAY-GSL-002 | reliability | P3 | low | `cli.rs`; `parse_run_input`; stdin bytes → UTF-8 `InputConfig`; CLI unit/live process tests | Confirmed and in scope. |
| PLAY-GSL-003 | reliability | P3 | medium | `session/builder.rs`; `load_extensions`; builder-owned `Arc<Agent>` lifecycle; `CliSession` construction | Verified not a current defect. All builder clones are owned by tasks in a fully drained `JoinSet`; `Agent::add_extension` receives `&self`, so extension implementations cannot retain the builder's outer `Arc<Agent>`. |

## Gate 2 — locality groups

### Group 1 — fallible run-input decoding

- Defect: `PLAY-GSL-002`.
- Files/functions: `crates/gosling-cli/src/cli.rs`;
  `parse_run_input` and a reader-injected stdin helper; colocated tests.
- Data path: stdin bytes → UTF-8 string → `InputConfig` → one-shot run.
- Repair: replace the `.expect` boundary with an `anyhow::Result` carrying stdin
  context, while preserving valid input and all file/text branches. Inject a
  `Read` implementation for deterministic invalid-byte regression coverage.
- Regression surface: invalid UTF-8 returns `Err` without panic; valid stdin is
  preserved exactly; live CLI exits non-zero without panic text.
- Modularization decision: `cli.rs` is 2,083 lines, but this is a narrow local
  guard and test change, not a heavy edit. Apply the smallest patch; no in-stage
  split.
- Documentation: close `PLAY-GSL-002` and `PLAY-GSL-003` accurately in the
  source audit; update the campaign session log.
- Commit boundary: one local conventional commit after targeted verification,
  adversarial review, and full stage diff review.

## Excluded and protected work

- No feature work is included.
- Deferred Tagteam/chat-persistence/usage items in `docs/TODO.md` remain
  protected and unchanged.
- `PLAY-GSL-003` is closed as verified not a defect, not represented as repaired.

## Cross-stage risks

- The `run` command must reject malformed stdin before provider/session creation.
- Error text must identify stdin without exposing a panic/backtrace.
- Reader injection must remain private and must not alter the public CLI contract.

## Completion update

- Inventory completed: three findings, all in CLI reliability/session handling.
- Repair group completed: `PLAY-GSL-002`, isolated to `parse_run_input` stdin handling.
- Baseline closure re-verified: `PLAY-GSL-001` remains covered by its unknown-command regression.
- Evidence-only closure completed: `PLAY-GSL-003` is not reachable under the current `JoinSet` ownership and extension-registration API.
- Modularization decision: no extraction was justified for the localized parser seam and tests.
- Final status: `completed_verified`.
