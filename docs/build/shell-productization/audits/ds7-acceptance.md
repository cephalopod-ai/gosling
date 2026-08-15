# DS-7 nonvisual acceptance audit

Date: 2026-08-15
Decision: **findings recorded below; GO/NO-GO is an explicit operator decision (CA-7), not made by this audit**

This audit closes the CA-0 through CA-6 packages of the Default Shell corrective closure campaign
against [`../../architecture/default-shell-template.md`](../../architecture/default-shell-template.md)'s
DS-7 exit condition: "DS-1–DS-6 are revision-bound and green; no critical/high open finding
applies; the operator accepts GUI implementation." Following this campaign's own evidence rule
(and PG-50's precedent in
[`pg-50-pre-gui-acceptance.md`](pg-50-pre-gui-acceptance.md)), every claim below is checked against
observed output on the exact commits named, not rounded up from local partial evidence.

## Exact revisions audited

Work landed as two stacked pull requests rather than one commit, so this section names both
exactly rather than claiming a single "clean main revision" that does not yet exist:

- **CA-0/CA-1** — branch `claude/ca0-ca1-restore-ci-baseline`, commit
  `7c66c590615499ee5ab0d5e6b0a25f80a4e4ec4e`,
  [PR #53](https://github.com/cephalopod-ai/gosling/pull/53).
- **CA-2–CA-5** — branch `claude/ca2-ca5-default-shell-gaps`, commit
  `e5d407f37e3f22cccd666725aab1d8dfbac1bccf` (built on top of the CA-0/1 commit above),
  [PR #54](https://github.com/cephalopod-ai/gosling/pull/54).
- **Neither PR is merged to `main` at the time of this audit.** `main`'s current tip
  (`32a3696ed`) does not yet contain either change. This audit's evidence is real and
  revision-bound to the two commits above; it is not evidence about `main`.
- `pnpm run shell:check-profiles` on `e5d407f37e` (which contains the full CA-0–CA-5 changeset)
  reports `sourceClean:true` and `resolvedGoslingRevision: e5d407f37e3f22cccd666725aab1d8dfbac1bccf`
  for all three fixture profiles (`gosling-default-shell-template`, `gosling-shell-fixture-a`,
  `gosling-shell-fixture-b`) — the DS-7/condition-12-equivalent clean-revision proof.

## What each package actually changed

- **CA-0** (docs only): reconciled `traceability-matrix.md`'s SHP-REQ-046/048/050/051 status from
  stale `planned`/`foundations exist` language to `built in current worktree` text matching
  `default-shell-template.md`'s DS-3/DS-4/DS-6/DS-5 status; created
  `docs/logs/session/2026-08-14-default-shell-ds3-ds6.md`, which `default-shell-template.md` had
  referenced since 2026-08-14 but which never existed.
- **CA-1**: removed the unused `bands_debug` test helper
  (`crates/gosling/src/context_mgmt/mod.rs`) — confirmed via `gh run view` on the prior red `main`
  CI runs to be the sole cause of the `Lint Rust Code`/`Build and Test Rust Project`/`Check MSRV`
  failures. Unrelated to Default Shell.
- **CA-2**: added `settings.read`/`settings.appearance.update`/`settings.reset` shell IPC
  channels (`ui/desktop/src/shell/ipc.ts`, `ipcMain.ts`, `preload.ts`, `preloadApi.ts`,
  `bootstrap.ts`, `main.ts`), wired to the pre-existing `ShellSettingsStore`. Sender/generation
  fenced, exact-key-allowlisted, capability-independent (settings are core, not an optional
  declared capability), reset gated by a new main-owned native confirmation dialog.
- **CA-3**: `fixtures/shell-products/default-shell-template/provisioning.json`'s
  `credentialPolicy` changed `fixed` → `selectable_catalog`; `credential.select` added to
  `fixtures/shell-consumers/default-shell-template/shell-consumer.json`'s
  `declaredCapabilities`. New Rust acceptance coverage for the policy's mutual-exclusion rule and
  a live-child end-to-end proof that the catalog and caller selection open under
  `selectable_catalog` where `fixed` denies them.
- **CA-4**: four new tests in `crates/gosling/src/acp/domain_adapter.rs` closing startup-crash,
  idle-crash (no call in flight), restart-without-stale-status, and forced-shutdown-orphan gaps in
  the recovery matrix.
- **CA-5**: new `two_shell_identities_coexist_as_live_concurrent_processes` test
  (`crates/gosling-cli/tests/shell_runtime_e2e_test.rs`) addressing `SHP-DEF-007`; a fresh package
  build and independent verify of the committed `default-shell-template` fixture on this host
  (not a reused artifact).

## Local evidence collected

All commands run with `source bin/activate-hermit` from repository root unless noted.

- `cargo fmt --all -- --check` — clean, both commits.
- `cargo clippy --workspace --all-targets --exclude v8 -- -D warnings` — clean, both commits
  (re-verified after CA-4/CA-5 landed, not just before).
- `cargo test -p gosling --lib domain_adapter` — 15/15 passed (4 new: startup crash, idle crash,
  reconnect-with-no-stale-status, forced-shutdown-no-orphan).
- `cargo test -p gosling --lib shell_validation` — 6/6 passed.
- `cargo test -p gosling-sdk-types shell` — 10/10 passed.
- `cargo test -p gosling-cli --test shell_provisioning_validation_test` — 5/5 passed (2 new:
  `selectable_catalog` accepted alone, rejected when combined with a fixed
  `credentialProfileId`).
- `cargo test -p gosling-cli --test shell_runtime_e2e_test` — 6/6 passed (2 new: the
  `selectable_catalog` live-child proof, and the two-identity coexistence test).
- `just check-acp-schema` — regenerated Rust schema and TypeScript SDK types; reports
  "ACP schema and generated types are up-to-date" (no diff).
- `pnpm run typecheck` — clean.
- `pnpm exec vitest run src/shell` — 18 files, 164/164 passed.
- `pnpm run shell:test-profile` — 57/57 passed, including
  `the committed neutral Default Shell sample stays conformant` (updated for the new
  `credential.select` capability and its resolved `_gosling/unstable/shell/credentials/list`
  required method).
- `pnpm run shell:check-profiles` — `sourceClean:true` for all three fixtures on `e5d407f37e` (see
  above).
- Fresh package + independent verify of `default-shell-template`, macOS arm64, unsigned, current
  host:
  ```
  node scripts/package-shell.js fixtures/shell-products/default-shell-template/product-profile.json \
    --consumer fixtures/shell-consumers/default-shell-template/shell-consumer.json \
    --platform darwin --arch arm64
  # -> profileHash 830f6143a45ea309c42f03cb440410b3eb6484009c86cda4aa98f0a7e1282950
  #    binaryHash  b1457546544e8cdcd83608f0d419f4ad075eae3aa3aafe0f719a11d964e15e0c

  node scripts/verify-shell-package.js fixtures/shell-products/default-shell-template/product-profile.json \
    --consumer fixtures/shell-consumers/default-shell-template/shell-consumer.json \
    --platform darwin --arch arm64 \
    --package "out/Default Shell Template-darwin-arm64" \
    --binary build/shell-packages/gosling-default-shell-template/macos-arm64/bin/gosling
  # -> independently re-derives the identical profileHash/binaryHash; exit 0
  ```
  This is real packaging of the committed template, not a reused stale artifact, satisfying CA-5's
  "package the committed Default Shell template" requirement for the current host target.
  Signing, notarization, and other-platform readback remain R6/R7, as scoped.

## Current CI

- **PR #53** (`7c66c590`): GitHub Actions run
  [31872865632](https://github.com/cephalopod-ai/gosling/actions/runs/31872865632) (`pull_request`
  event) — `success`. `Build and Test Rust Project`, `Lint Rust Code`, `Check Rust Code Format`,
  `Check MSRV`, `Check Generated Schemas are Up-to-Date`, `Test and Lint Electron Desktop App` all
  passed.
- **PR #54** (`e5d407f3`): the PR was opened stacked on `claude/ca0-ca1-restore-ci-baseline`, whose
  CI workflow only triggers `pull_request` events targeting `main` — so no run fired automatically
  until the base was retargeted to `main`. Two `workflow_dispatch` runs were then triggered
  directly against this exact commit to get real signal rather than assume success:
  - [31873994858](https://github.com/cephalopod-ai/gosling/actions/runs/31873994858) — `failure`.
    Every Rust job passed (`Build and Test Rust Project`, `Lint Rust Code`,
    `Check Rust Code Format`, `Check MSRV`, `Check Generated Schemas are Up-to-Date`). Only
    `Test and Lint Electron Desktop App` failed, at its `i18n:check` step, specifically
    `scripts/i18n-sync-locales.test.js`'s `simultaneous recovery attempts are serialized by the
    process lock` test — a test that spawns two processes racing for a lock file and asserts
    exactly one wins. The failure was `ENOENT` on the lock file rather than the expected
    contention error, a timing artifact of the race. Neither this test nor the module it exercises
    (`i18n-sync-locales.js`) appears anywhere in the CA-0–CA-5 diff.
  - [31874626556](https://github.com/cephalopod-ai/gosling/actions/runs/31874626556) — `success`,
    same commit, no code change in between. Every job including
    `Test and Lint Electron Desktop App` passed. This is treated as confirmation of a pre-existing,
    unrelated flake (same mechanism PG-50's audit documented for a different suite under CPU
    contention), not as "retry until green" — the first run's failure is recorded above rather
    than discarded, and the specific failing assertion and its unrelated file are named so a future
    audit can tell a real regression from this same flake.

## DS-1–DS-7 disposition

| Package | Result | Disposition |
| --- | --- | --- |
| DS-0 | Requirements SHP-REQ-044–053 traced; CA-0 reconciled stale status language against observed `built` state. | Satisfied |
| DS-1 | Unchanged by this campaign; already built per `default-shell-template.md`. | Unchanged, previously satisfied |
| DS-2 | Unchanged store/schema; CA-2 adds the narrow IPC operations DS-2's own text said were still pending ("later narrow main operations"). | Advanced by CA-2; full DS-2 IPC/migration proof still tracked under SHP-REQ-047 |
| DS-3 | Unchanged; already built. | Unchanged, previously satisfied |
| DS-4 | CA-3 activates `selectable_catalog` on the committed template (previously left on `fixed`, exercising none of DS-4's built catalog capability); new Rust acceptance coverage. | Advanced by CA-3 |
| DS-5 | Unchanged; already built. CA-4 closes 3 recovery-matrix gaps in the underlying adapter supervision this depends on. | Advanced by CA-4 |
| DS-6 | Unchanged; already built. | Unchanged, previously satisfied |
| DS-7 | This audit. Local evidence and current CI (both PRs) collected on exact, `sourceClean:true` revisions. Not yet revision-bound to a single `main` commit — both PRs are open, unmerged. | **Evidence collected; not yet accepted — see Decision** |

## SHP-REQ-044–053 disposition

| REQ | Status after this campaign |
| --- | --- |
| SHP-REQ-044 | `planned; contract recorded` — unchanged, genuinely not yet done (no-named-domain-implementation scaffold-generation scope). |
| SHP-REQ-045 | `built ... pending` — unchanged, already accurate. |
| SHP-REQ-046 | Updated by CA-0 from `focused projection planned` to `built in current worktree` (matches DS-4); CA-3 additionally activates it on the committed template with new acceptance tests. |
| SHP-REQ-047 | `initial store built ... IPC/migration/full proof pending` — CA-2 lands the IPC portion; full proof (migration, crash-interruption per platform) remains open, text not overstated here. |
| SHP-REQ-048 | Updated by CA-0 from `planned` to `built in current worktree` (matches DS-3; this campaign did not change DS-3 itself). |
| SHP-REQ-049 | `built ... pending` — unchanged, already accurate. |
| SHP-REQ-050 | Updated by CA-0 from `Default Shell scaffold planned` to `built in current worktree` (matches DS-6). |
| SHP-REQ-051 | Updated by CA-0 from `generic registry planned` to `built in current worktree` (matches DS-5); CA-4 closes 3 gaps in the recovery matrix this requirement's acceptance criteria name. |
| SHP-REQ-052 | `planned` — unchanged, genuinely not yet done. |
| SHP-REQ-053 | `planned` — unchanged, genuinely not yet done (recovery/accessibility/uninstall UX is GUI-adjacent work, out of this campaign's nonvisual scope). |

No requirement above is marked `verified` — that word is reserved by the traceability matrix's own
vocabulary for revision-bound acceptance, which has not yet occurred (see Decision).

## Negative-space and security review

- Grepped `crates/gosling/src`, `ui/desktop/src/shell`, and `fixtures/` for DAWES, physics, math,
  CST, Project ABC, and other named-domain strings: no match beyond the pre-existing
  namespace-isolation test fixture already documented in `pg-50-pre-gui-acceptance.md`. Nothing
  added by CA-0–CA-5 introduces named-domain content.
- `ui/desktop/src/shell/ipc.ts`/`ipcMain.ts`/`preloadApi.ts` new `settings.*` channels: reviewed
  for secret/path/arbitrary-key exposure. `settings.read`'s response is `{appearance, recovery}`
  only — never raw settings-file content, never `workspace.preferredCredentialProfileId` beyond
  what `localSettings.ts` already bounds to a 128-character opaque reference. No new channel
  accepts a filesystem path or arbitrary key.
- `credential.select`'s activation (CA-3) does not change the credential data path at all — the
  existing `isSafeSummary` projection (`credentialController.ts`) and opaque-ID-only storage were
  already in place and already tested for sentinel secrets; CA-3 only flips the policy flag and
  declares the capability that gates access to that pre-existing, pre-tested path.
- `bootstrap.ts`'s new `showConfirmDialog` adapter method takes only `{title, message, detail,
  confirmLabel, cancelLabel}` — plain strings the shell itself constructs, never renderer input.
- No `TODO`/`FIXME`/debug `console.log` found in the new code (`ipc.ts`, `ipcMain.ts`,
  `bootstrap.ts`, `main.ts`, `preload.ts`, `preloadApi.ts`, `domain_adapter.rs`,
  `shell_runtime_e2e_test.rs`).
- No critical or high finding was produced by this campaign. `SHP-DEF-007` moves from `open` to
  `partially fixed` (see `defects.md`) — the live two-identity coexistence claim is now backed by
  a real concurrent-process test; the narrower "full Gosling app alongside a shell identity"
  variant is not covered and remains open, explicitly, rather than folded into "fixed."

## Residual R6–R8 limitations (unchanged, explicitly out of this campaign's scope)

- Signing, notarization, and publication.
- Cross-platform (Windows, Linux) package readback for `default-shell-template` — only macOS
  arm64 (this sandbox's host) was exercised in CA-5.
- Reusable release workflows.
- Full-Gosling-app-plus-shell-identity concurrent coexistence specifically (as opposed to
  shell-identity-to-shell-identity, which CA-5 now covers).
- SHP-REQ-052 (second-shell generation/validation) and SHP-REQ-053 (recovery/accessibility/
  privacy/uninstall UX boundaries) remain planned, not built.

## Decision

Per this campaign's own CA-7 gate and this repository's operating rules, I do not declare GO. What
this audit establishes:

- CA-0 through CA-5 are complete, locally evidenced, and green on current CI for the two exact
  commits named above.
- Neither commit is on `main`; DS-7's "one exact clean revision with current CI" condition is
  satisfied *per PR*, not yet as a single accepted revision, because merging is a decision this
  audit does not make.
- No critical/high finding is open. `SHP-DEF-007` is partially closed with real evidence, not
  fully closed, and is reported as such above.

**Operator decision required (CA-7):** review and merge #53 and #54 (in that order, or squashed —
your call), or send back anything that shouldn't ship as-is. Only after that, and only with your
explicit GO, does Default Shell GUI implementation begin, in the order
`default-shell-template.md`'s "GUI implementation order after DS-7" already specifies. Named
shells (DAWES, math, physics/CST, or other) remain blocked regardless, until the generic GUI passes
its own M5 acceptance.
