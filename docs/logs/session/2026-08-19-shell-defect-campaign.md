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
| B — external consumer boundary | complete | external archive 2/2; complete shell profile suite 59/59; all profiles resolve | `afc8451c9` |
| C — reusable package/lifecycle acceptance | locally complete; remote gate pending | shell 67/67; integration 15/15; Desktop 990/990; lint/type/i18n; Clippy; package/readback/lifecycle/coexistence pass | pending |

## Decisions and deviations

- The operator authorized the external Gosling checkout but did not reopen deferred defects 004 or
  006 by ID. They are excluded, not silently closed.
- `SHP-DEF-061` is added as a campaign finding because the failing current CI run directly
  reproduces it. The implementation appears correct; the test oracle is stale.

## Exact next action

Commit the reviewed Group C diff, rerun profile resolution with a clean source tree, then obtain
explicit push authorization. Do not close SHP-DEF-053 until required CI and the reusable four-target
acceptance workflow are green on that exact commit.

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

## Group C — reusable package/lifecycle acceptance

### Repair

- Added a reusable, read-only, unsigned acceptance workflow and a PR/main/nightly/manual caller for
  pinned macOS arm64, macOS x64, Linux x64, and Windows x64 runners. It runs shell contracts, the
  authenticated child integration, package construction/readback, lifecycle recovery, stop/relaunch,
  and two-product coexistence. Only bounded JSON reports are retained for five days.
- Added a packaged lifecycle CLI that launches the package through a loopback-only CDP endpoint,
  gives each launch isolated Playwright-gated Electron paths, and accepts only a registry record
  whose parent PID and binary path match the package it launched before injecting backend loss.
- The first cycle proves ready → unexpected backend exit → offline → accepted retry → ready → clean
  close. The second proves a fresh ready launch → explicit stop → stopped → clean close. Both require
  an empty product-local process registry. Coexistence requires distinct product identities, closes
  one ready shell, verifies the survivor remains ready, and then verifies both registries are empty.
- Closed the local implementation portions of SHP-DEF-028 and SHP-DEF-030. Updated SHP-DEF-053 to
  retain only exact-commit remote execution as its residual gate; no remote success is claimed.

### Failures and deviations preserved

- Initial isolation changed `HOME`, but macOS packaged Electron did not reliably expose CDP under
  that environment. The harness now uses a Playwright-gated path root applied before shell identity
  bootstrap; normal production path derivation is unchanged.
- The first lifecycle draft attempted retry from `stopped`. The frozen lifecycle contract correctly
  exposes no such action, so the packaged oracle was corrected to inject an owned backend exit,
  observe `offline`, and retry from the state that actually permits it.
- Fixture B reached `incompatible` with `ADAPTER_MISMATCH`, as its declared domain adapter was not
  provisioned in the package run. That fail-closed result was preserved. The neutral coexistence
  proof uses Default Shell and fixture A, matching the reusable workflow.
- The first full Desktop run overlapped the cold full-workspace Clippy build; one unrelated
  workspace-editor test exceeded its five-second timeout (989/990). It then passed 11/11 focused,
  and the uncontended full rerun passed 990/990. No production or test timeout was changed.
- An already-running installed Default Shell process was never terminated or modified. Test
  instances used isolated product roots and all processes launched by the harness were reaped.

### Validation

- Shell profile/consumer/package/workflow suite: 67/67 passed.
- Authenticated `gosling serve` integration: 15/15 passed.
- Desktop: focused timeout rerun 11/11; clean full rerun 990/990 across 118 files.
- Desktop typecheck, ESLint, i18n structure/recovery tests, and all profile resolution: passed.
- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings`: passed.
- macOS arm64 package/readback: Default Shell, fixture A, and fixture B passed structural readback;
  profile hashes were `830f6143...`, `c99e5901...`, and `7ad3922b...`, with embedded backend hash
  `9ad45c06...`.
- Real packaged fixture-A lifecycle passed both cycles with one registry per isolated launch and no
  registered process after close.
- Real Default-Shell/fixture-A coexistence passed with both at generation 1, independent shutdown,
  and one empty registry per product.
- Workflow YAML parsing, security/static contract tests, and `git diff --check`: passed.

### Residual gate

Push, PR creation, signing, publication, deployment, updater activation, and production credentials
were not authorized. SHP-DEF-053 therefore remains `partially validated` until the final local
commit is pushed and required CI plus all four package-matrix jobs are green at that same revision.
