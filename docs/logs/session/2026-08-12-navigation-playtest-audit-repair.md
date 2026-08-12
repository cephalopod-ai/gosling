# 2026-08-12 navigation playtest, audit, and repair

- Task: run the private-catalog playtest and six-gate design audit specifically
  against Desktop thread navigation, then patch confirmed findings with the defect
  repair campaign.
- Baseline: `53444c8fc` on `codex/full-audit-repair-20260812`; the concurrent Giles-
  approved navigation implementation and focused playtest harness were accepted and
  preserved.
- Scenario source: existing `docs/test_scenarios/` library; navigation portions of
  CH-03/04, ST-02, SX-04/09, and DT-03/05/07.
- Inventory: five confirmed findings — responsive disappearance, missing single-long-
  turn controls, hundred-turn target/tab/per-frame scaling, reduced-motion mismatch,
  and false-start scrolling after a history load failure.
- Grouping: one cohesive Desktop navigation/history stage. All heavily edited source
  files were under 1,000 lines, so no modularization was triggered.
- Repair checkpoint: `92b800e18` (`fix(desktop): harden thread navigation edge
  cases`).
- Files changed: `ThreadNavigator`, `BaseChat`, `useChatSession` and its type contract,
  motion utility, focused hook/component/unit tests, and the existing focused Chromium
  harness.
- Rendered evidence: Vite-rendered production component in headless Google Chrome at
  1280px, 900px, and 390px; 1/5/100 turns, keyboard, reduced motion, and rapid repeat.
  Final evidence: `/tmp/gosling-navigation-playtest-final`.
- Before/after: 100-turn target height ~6.03px -> 24px; 20-scroll geometry reads
  1,919 -> 149; tablet/mobile hidden -> visible without horizontal overflow; reduced-
  motion behavior `smooth` -> `auto`; one long turn 0 -> 1 navigator; normal pointer
  and rapid-latest paths remained green.
- Validation: Hermit typecheck; Desktop full Vitest (88 files / 581 tests); ESLint and
  i18n checks; targeted Prettier; Rust `cargo fmt --all -- --check` and
  `cargo clippy --all-targets -- -D warnings`; final rendered Chromium assertions.
  The broad `pnpm run format:check` remains red on 61 pre-existing unrelated files;
  every changed TS/TSX file passed targeted Prettier.
- Architecture/contract drift: authoritative repo instructions, desktop architecture,
  scenario cards, theme tokens/breakpoint, and component contracts were checked before
  and after; result `no new drift`.
- Tracking closure: findings were discovered in this run, so the report and this
  existing repo-native session-log convention are the source records. No prior TODO or
  defect ledger entry named them; no stale in-code marker remained.
- Report: `reports/2026-08-12-navigation-playtest-audit-repair.md`.
- Residual: full Electron/native accessibility and Firefox/Safari/Edge/touch coverage
  were not executed; see the report's validation limits.
