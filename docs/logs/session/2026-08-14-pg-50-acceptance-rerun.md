# 2026-08-14 PG-50 pre-GUI backend acceptance rerun

- Task: close the PG-50 pre-GUI backend acceptance gate per the no-go finding in
  `docs/build/shell-productization/audits/pg-50-pre-gui-acceptance.md` — isolate the R1-R4
  shell/backend work onto a clean, exact revision (excluding the unrelated summarizer-settings
  edit), run the required checks against that exact commit, complete the traceability/negative-space
  review, and record a revision-bound GO/NO-GO decision. This is the gate before R5 (shared GUI
  shell kit implementation); it does not build R5 itself.
- Read order followed, per AGENTS.md's required read order for code/documentation changes:
  `AGENTS.md`; root `README.md` (project overview — no conflict with shell-productization work);
  `docs/INDEX.md` (confirms the shell-productization doc set below is the authoritative,
  cross-referenced index entry for this campaign); `docs/build/shell-productization/pre-gui-backend-implementation-plan.md`
  (line 111, pre-GUI completion definition); `docs/build/shell-productization/audits/pg-50-pre-gui-acceptance.md`
  (the prior no-go); `build-state.md`; `traceability-matrix.md`; `defects.md`; `assumption-ledger.md`;
  `.giles/repo.yaml`, `.giles/compliance_status.yaml`, `.giles/patch_todo.yaml`,
  `.giles/audit_report.yaml` (advisory metadata, `canonical: false`); recent
  `docs/logs/session/` entries (`2026-08-13-shell-r1-adr-drafting.md` and neighbors) for continuity.
  Conflict found and preserved rather than resolved: the `.giles/*.yaml` artifacts record a
  `blocked_by_giles_limitation` compliance scan from 2026-07-07 against a different `repo_path`
  (`/Users/eric/Work/vscode/forked/gosling`, this sandbox's checkout is elsewhere) with generic
  fleet-governance findings (AGENTS.md contract drift, missing `governance/repo_config.yaml`,
  a crashed Giles structure audit) unrelated to PG-50/shell productization. Per AGENTS.md's
  authority rules, this is left as an open, unresolved advisory item — not something this task
  was asked to fix, and not promoted to canonical status.
- Source grounding before acting: `git log` showed the working branch (`claude/pg-50-shared-gui-shell-skdwwh`)
  already had the R1-R4 work committed as `5933637` on top of baseline `34920cc`, with a clean
  working tree — the prior audit's "dirty worktree" condition no longer applied verbatim, but
  `git show --stat 5933637` confirmed it still carried an unrelated 15-line
  `ui/desktop/src/components/settings/chat/SummarizerSection.tsx` hunk (auto-fill/auto-persist a
  default summarizer endpoint) alongside the shell work, matching the audit's warning about
  "unrelated Desktop settings changes."
- Implementation:
  - Isolated the R1-R4 revision: `git checkout 34920cc -- ui/desktop/src/components/settings/chat/SummarizerSection.tsx`
    then a new commit (`c232d04`) restoring that file to its pre-`5933637` state, appended (not
    rewriting already-pushed history) so the unrelated edit is excluded from the accepted revision.
  - Reran the required checks on the isolated commit; `cargo clippy --all-targets -- -D warnings`
    failed on a real `question_mark` lint violation in `crates/gosling/src/acp/server.rs`
    introduced by the R1-R4 work. Fixed with `persisted?;` (commit `b921e6e`), reran clippy clean.
  - This exact commit, `b921e6ee1299dba2207ab27ab6fd9452cc57aa26`, is the PG-50 acceptance
    revision. `pnpm run shell:check-profiles` on it reports `sourceClean:true` for both neutral
    fixtures — the condition-12 proof the prior audit could not produce.
  - Ran the full local evidence set on that commit: `cargo fmt`/`clippy`/`test -p gosling`;
    `cargo test -p gosling-cli --test shell_runtime_e2e_test`/`shell_provisioning_validation_test`
    twice each; ACP schema generation twice (no diff); `pnpm typecheck`/`lint:check`; full desktop
    `vitest` (729/729); `shell:test-profile` (47/47); a `linux-x64` package build + verifier
    readback (exact binary hash/manifest match).
  - The non-visual consumer/runtime/adapter conformance suite
    (`tests/integration/shell_session_runtime.test.ts`, `vitest.integration.config.ts`) is not
    exercised by CI (CI's Desktop job runs only the default `vitest.config.ts`, `src/**`); its
    evidence is local-only. First attempt: 14/15, a 60s timeout on the first test, while several
    unrelated `cargo` compilations ran concurrently on this 4-core sandbox. Per the plan's rule
    that a retry-only pass does not close a flaky-test finding, the contention hypothesis was
    reproduced rather than assumed: a fresh `cargo build --workspace` was started against a scratch
    target directory to saturate all 4 cores, and the suite reran under that live load. It failed
    again but on a *different* test (`does not create durable session state when compatibility
    fails`, a SQLite readback race against the real `gosling serve` child's session-store init),
    which established the mechanism (subprocess/DB timing under severe local CPU starvation, not a
    single flaky test or a product defect) rather than merely asserting it. With the contention
    build killed, three further consecutive runs were clean: 15/15, 15/15, 15/15. PG-50 condition
    11 rests on those three uncontended runs, not on the earlier contended failures.
  - Opened PR #50, which triggered current CI (`ci.yml` only fires on `pull_request`/push-to-`main`,
    so no CI had run against this branch before); all required/attached checks completed
    green (22/22, with expected path-filtered skips such as the Windows Rust build).
  - Codex review on the PR surfaced, and this session fixed, four further issues: a missing
    session log (this file), stale post-acceptance "NO-GO"/"not ready" language left in
    `build-state.md` and `README.md` after the audit flipped to GO, an incomplete AGENTS.md read
    order (this entry), and a real regression the summarizer-edit revert had reintroduced
    (`SummarizerConfig::from_config_with` returns `None` on an unset endpoint, so enabling
    summarization without retyping the suggested endpoint silently no-ops) — fixed as its own
    dedicated commit (`a3b0dd9`), kept out of the PG-50 acceptance revision since it is unrelated
    to shell productization.
  - Negative-space re-review: no DAWES/physics/CST/Project ABC domain implementation (only the
    pre-existing `dawes`/`math` namespace-isolation unit-test labels in `paths.rs`), no
    `nodeIntegration`/`contextIsolation` violations, no raw ACP token/endpoint in the preload
    surface, no debug/TODO leftovers in the R1-R4 adapter or shell core modules.
  - Rewrote `docs/build/shell-productization/audits/pg-50-pre-gui-acceptance.md` with a GO decision
    and the condition-by-condition disposition; updated `build-state.md` (current gate ->
    PG-50 accepted, R5 authorized), `defects.md` (new `SHP-DEF-034`/`SHP-DEF-035` for the two
    defects found and fixed during this rerun — both already disposed as fixed), and
    `traceability-matrix.md` (`SHP-REQ-029` row).
- Explicitly not done, and why: R5 (the shared GUI shell kit) was not started — that is the next
  gate this acceptance authorizes, not part of this task. macOS `darwin-arm64` package readback
  could not be reproduced because this sandbox has no macOS host; the `linux-x64` readback is
  recorded as the supported-host-target proof instead, and the gap is disclosed rather than hidden
  in the audit. Full cross-platform package/workflow parity remains explicit R6/R7 scope.
- Validation run: see the local evidence set and CI result above; all commands and their results
  are recorded in `audits/pg-50-pre-gui-acceptance.md`. No destructive git operations were used —
  the unrelated edit was excluded via a new commit, not a history rewrite, since `5933637` was
  already pushed.
- Residual risks / follow-ups: macOS and Windows package-readback parity for this revision is
  still open (R6/R7). The historical `SHP-DEF-028` acceptance-debt entry was left as-is per the
  ledger's no-erase rule even though R3/R4 evidence now covers the paths it flagged.
- Next action: begin R5 (shared shell GUI application kit) per `project-shell-readiness-plan.md`,
  consuming only the frozen R1-R4 contracts; do not start any named shell (DAWES, math,
  physics/CST) before M5.
