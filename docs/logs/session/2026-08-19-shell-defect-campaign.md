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
| A — CI truth and record reconciliation | complete | focused 12/12; Desktop 990/990; typecheck and lint pass | `a317f77d4` |
| B — external consumer boundary | complete | external archive 2/2; complete shell profile suite 59/59; all profiles resolve | pending |
| C — reusable package/lifecycle acceptance | planned | pending | pending |

## Decisions and deviations

- The operator authorized the external Gosling checkout but did not reopen deferred defects 004 or
  006 by ID. They are excluded, not silently closed.
- `SHP-DEF-061` is added as a campaign finding because the failing current CI run directly
  reproduces it. The implementation appears correct; the test oracle is stale.

## Exact next action

Freeze and commit the Group B diff, then begin Group C: reusable guarded package/readback and
lifecycle/coexistence workflows for SHP-DEF-028, SHP-DEF-030, and SHP-DEF-053.

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

## Group B — external consumer boundary

### Repair

- Extracted the canonical profile and consumer resolvers into the packable, dependency-free
  `@repo-makeover/gosling-shell-kit` package. Thin Desktop wrappers retain the established in-tree
  interface; there is one resolver implementation, not an external fork.
- Added an external registration path that derives its trust root from the nearest non-symlinked
  consumer `package.json`, requires an exact dependency matching the installed kit version, and
  requires `requiredShellKit` to match. The manifest still has no approved-root field.
- Added `gosling-shell init`, `check`, and `resolve`. Init is non-overwriting and emits only neutral,
  non-publishable, unsigned, updater-disabled files. Resolve emits canonical profile/build manifests
  under the consumer's own `build/` tree.
- Documented the archive-install path in `docs/SHELL_PRODUCTS.md` and reconciled SHP-DEF-029 plus
  SHP-REQ-033/SHP-REQ-040 status without claiming registry publication or packaged-app acceptance.

### Validation

- Packed-archive external consumer: 2/2 tests pass. The archive is installed with `npm` into a
  temporary package outside Gosling; init/check/resolve succeed without a Gosling checkout.
- Negative-space proof: an unpinned dependency, manifest/package version drift, and a caller-defined
  `approvedRoot` all fail closed.
- Full shell profile/consumer/package resolver suite: 59/59 tests pass, including two unchanged
  in-tree renderer bundles and package-verifier tamper cases.
- `shell:check-profiles`: all three committed product profiles resolve.
- New CLI syntax check and repository diff whitespace check pass.

### Adversarial and diff review

The external root is not an API/manifest input: it is the real package directory containing the
exact dependency declaration. All referenced files are still non-symlinked, containment checked,
schema checked, secret scanned, canonically hashed, and bound to the installed kit's version/core
revision. No host main/preload source, signing, updater, publisher, credential, or runtime authority
was added. Full Electron packaging is intentionally left to Group C rather than being mislabeled as
part of this build-input proof.
