# Shell defect-repair campaign plan — 2026-08-19

Baseline: `main` at `386a7442a38165fd524c4910a8b493afaae5d3eb`
Working branch: `codex/repair-shell-defect-campaign`
Authority: local governed repair; no push, publication, signing, release, or updater activation

## Scope and acceptance

Repair every eligible unresolved entry in `defects.md`, plus defects reproduced while validating
those paths. `SHP-DEF-004` and `SHP-DEF-006` remain protected because their ledger records say the
work was deliberately deferred and the operator did not reopen either item by ID.

The campaign is complete only when each in-scope record is either fixed with behavioral evidence,
verified not to remain a defect, or honestly retained as partial/open with the missing external
gate named. Documentation status must agree with current source and observed checks.

## Inventory

| ID | Domain / priority / complexity | Current evidence | Disposition entering campaign |
| --- | --- | --- | --- |
| SHP-DEF-028 | workflow/reliability, P1, high | R3 source tests cover the formerly omitted paths, but the committed packaged failure matrix is absent | in scope; group C |
| SHP-DEF-029 | architecture/build, P1, high | `shell-consumer.js` requires a manifest inside a Gosling repo and under `fixtures/shell-consumers`, contrary to accepted ADR-0010's separate-repository topology | in scope; group B |
| SHP-DEF-030 | build/CI, P1, high | no reusable shell build/smoke workflow exists; `ci.yml` runs only profile tests | in scope; group C |
| SHP-DEF-038 | frontend/UX, P2, low | code, regression test, TODO, and a dated repair log prove the OAuth detail projection is fixed; only the defect ledger and dependent prose are stale | record reconciliation; group A |
| SHP-DEF-053 | build/CI/process, P1, high | CI run 32306201277 is revision-bound but red; packaged lifecycle/coexistence is not committed as a repeatable harness | in scope; group C |
| SHP-DEF-061 | build/CI, P1, low | CI run 32306201277 fails two `bootstrap.test.ts` assertions because expected settings snapshots omit the now-required `modelSelection` field | newly confirmed; group A |

Excluded/protected:

- `SHP-DEF-004`: explicitly deferred distribution assets/release-profile work.
- `SHP-DEF-006`: explicitly deferred shared application expansion; the later Default Shell GUI is
  preserved as evidence, but this campaign does not reinterpret the historical scope silently.
- Signing, notarization, publication, updater activation, production identities/destinations, and
  named shells remain out of scope.

## Architecture and contract baseline

| Source | Status | Touched contract |
| --- | --- | --- |
| `docs/adr/0010-project-shell-consumer-composition.md` | accepted | external consumers use a versioned build interface and never nominate their own trust root |
| `docs/architecture/shell-productization-r1-contracts.md` | accepted | strict consumer manifest v1, containment, capability, and hash rules |
| `docs/build/shell-productization/project-shell-readiness-plan.md` | active | R6 packaged acceptance, R7 reusable workflows, R8 external onboarding proof |
| `AGENTS.md` | canonical repo policy | Hermit toolchain, no fake success, validation before completion |

Pre-repair disposition: the current in-tree resolver and package scripts conform for in-tree
fixtures, but external-consumer and repeatable packaged-workflow obligations are pre-existing drift
from the accepted contracts. Repairs must introduce no wider renderer, filesystem, signing, secret,
or publication authority.

## Ordered locality groups

### Group A — CI truth and record reconciliation

- Defects: SHP-DEF-038, SHP-DEF-061.
- Files: `ui/desktop/src/shell/bootstrap.test.ts`, defect/status documents, this session record.
- Data path: settings snapshot -> IPC result -> test oracle; ACP error payload -> rendered OAuth
  failure detail -> source ledger.
- Modularization: none; all edits are localized and files are not heavily changed in the
  1001–1999-line band.
- Validation: focused Bootstrap and provider-modal tests, full Desktop suite, lint/typecheck.
- Commit boundary: one local commit after adversarial/diff review.

### Group B — external consumer boundary

- Defect: SHP-DEF-029.
- Files/functions: consumer resolver, scaffold/conformance command, versioned shell-kit/build
  interface package and external-consumer fixtures/tests, onboarding docs.
- Data path: trusted package registration -> manifest/profile/renderer containment -> canonical
  hashes -> Vite/Forge projection.
- Modularization: decide after the exact implementation touch set is frozen; no currently identified
  file exceeds 1000 lines.
- Validation: a consumer in a temporary directory outside the Gosling repository installs/uses the
  versioned local package, passes conformance, and builds without editing Gosling host/core source;
  traversal, self-declared-root, secret, and unpinned-version negatives fail closed.
- Commit boundary: one local commit after contract and negative-space review.

### Group C — reusable package/lifecycle acceptance

- Defects: SHP-DEF-028, SHP-DEF-030, SHP-DEF-053.
- Files/functions: package/readback runner, new bounded lifecycle/coexistence harness, reusable
  GitHub workflow plus caller, package scripts, evidence/status records.
- Data path: canonical profile + consumer + exact revision -> target package -> structural readback
  -> lifecycle/cleanup/coexistence results -> short-lived CI artifact/evidence.
- Modularization: none anticipated; new single-purpose files are preferred over growing existing
  scripts.
- Validation: local supported-host package/readback and lifecycle replay, workflow syntax/security
  inspection, targeted tests, full relevant suites. Remote CI on the final commit requires an
  explicit push and remains a hard external gate until authorized.
- Commit boundary: one local commit for harness/workflows, followed by a documentation/ledger
  closeout commit if necessary.

## Risks and rollback

- External-root support must never accept a trust root from the manifest. Roll back the group if
  containment depends on caller-controlled JSON rather than package registration/resolution.
- Fixture workflows must never read signing/provider credentials, publish, attach to a public
  release, or accept arbitrary commands/paths from pull-request content.
- Lifecycle checks must report skipped/unsupported observations as partial, never success.
- Every group is independently revertible by its local commit. No migration or persistent user data
  change is planned.

## Validation matrix

| Acceptance | Evidence |
| --- | --- |
| Current CI regression is repaired | focused Bootstrap tests and full Desktop suite pass locally; final CI job passes when push is authorized |
| OAuth defect record is truthful | source test + dated repair log + corrected `defects.md`/status prose |
| External consumer no longer needs host edits | temp external consumer proof plus fail-closed negative tests |
| Shell workflows are reusable and guarded | workflow schema/static review, package/readback/lifecycle outputs, no signing/publish credentials |
| Gate-4 acceptance debt is not hidden | every reopened path maps to executable evidence or remains explicitly partial |
| SHP-DEF-053 is closed honestly | one exact commit has green required CI and supported-host lifecycle/coexistence evidence |
