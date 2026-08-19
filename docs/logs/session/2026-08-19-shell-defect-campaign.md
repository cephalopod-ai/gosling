# 2026-08-19 — shell defect-repair campaign

## Task

Run the repository-wide repair campaign against the supplied shell-productization defect ledger,
grouping work by shared files/data paths, validating every group, and updating the authoritative
records without erasing historical evidence.

## Preflight

- Target: `/Users/eric/Work/vscode/forked/gosling`
- Baseline: clean `main` at `386a7442a38165fd524c4910a8b493afaae5d3eb`
- Branch: `codex/repair-shell-defect-campaign`
- Involvement: standard; target modification explicitly authorized.
- Authority: local code/tests/docs/commits. Push, PR, signing, publication, deployment, updater
  activation, and production credentials are not authorized.
- Validation: Hermit-managed Rust/Node commands from `AGENTS.md` and the shell package records.
- Execution: three sequential locality groups because later exact-revision acceptance depends on
  the code produced by the earlier groups. Each group is a durable local commit.
- Restart: resume from this log and the plan at
  `docs/build/shell-productization/2026-08-19-defect-campaign-plan.md`.

## Baseline evidence

- The supplied attachment matches `docs/build/shell-productization/defects.md`.
- `SHP-DEF-004` and `SHP-DEF-006` are deliberately deferred and remain protected.
- `SHP-DEF-038` is already fixed in source and `docs/TODO.md`; its defect-ledger record is stale.
- Exact-main CI run `32306201277` is red only in Desktop tests. The two failing assertions in
  `src/shell/bootstrap.test.ts` omit the required `modelSelection` snapshot now returned by the
  settings IPC path. Rust, Windows, format, Clippy, MSRV, schema/SDK, shell profile, and Desktop
  lint jobs passed.
- `shell-consumer.js` still locates a Gosling repository and allows only
  `fixtures/shell-consumers`, directly reproducing `SHP-DEF-029` against ADR-0010.
- No reusable shell package/lifecycle workflow exists, reproducing `SHP-DEF-030`; profile tests in
  the ordinary Desktop job are not packaged acceptance.

## Campaign state

| Group | Status | Validation | Commit |
| --- | --- | --- | --- |
| A — CI truth and record reconciliation | complete | focused 12/12; Desktop 990/990; typecheck and lint pass | pending |
| B — external consumer boundary | planned | pending | pending |
| C — reusable package/lifecycle acceptance | planned | pending | pending |

## Decisions and deviations

- The operator authorized the external Gosling checkout but did not reopen deferred defects 004 or
  006 by ID. They are excluded, not silently closed.
- `SHP-DEF-061` is added as a campaign finding because the failing current CI run directly
  reproduces it. The implementation appears correct; the test oracle is stale.

## Exact next action

Freeze and review the Group A diff, run its final formatting/record checks, and commit it. Then begin
Group B at the accepted ADR-0010 external-consumer boundary.

## Group A — CI truth and record reconciliation

### Reproduction

The focused pre-patch run reproduced the exact remote failure: 2 failed and 10 passed. Both failures
were exact settings-object assertions, and both diffs contained only the omitted safe
`modelSelection` projection. The provider-modal regression already passed, confirming SHP-DEF-038
was repaired in source and stale only in the shell defect ledger/status prose.

### Repair

- Added the complete unavailable model-selection projection to initial read, appearance update,
  cancelled reset, and confirmed-reset expectations in `bootstrap.test.ts`.
- Closed SHP-DEF-038 in the authoritative defect ledger using its dated repair log and focused test.
- Added SHP-DEF-061 with exact run/job/revision evidence, root cause, repair, regression, and the
  still-open remote-CI residual.
- Reconciled stale readiness prose that still called SHP-DEF-054, SHP-DEF-055, SHP-DEF-057, and the
  Gemini OAuth detail active gates despite their later closure records.

### Validation

- Focused Bootstrap/provider-modal run: 2 files passed, 12 tests passed.
- Desktop typecheck: passed.
- Desktop ESLint: passed.
- Full Desktop Vitest: 118 files passed, 990 tests passed.

### Adversarial and diff review

The production settings projection was not weakened to satisfy the stale tests. Exact equality still
protects the full safe response shape, including model-selection status and empty/null details. No
provider flow, persisted setting, IPC schema, renderer authority, or release behavior changed. The
records distinguish local closure of SHP-DEF-061 from the final revision-bound remote CI required to
close SHP-DEF-053.
