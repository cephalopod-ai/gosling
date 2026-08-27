# 2026-08-27 All remaining TODOs repair campaign

## Gate 0 — orientation and baseline

- Task: apply the repository-wide `repair-defect-campaign` workflow to every
  unfinished defect and TODO record, repair every item that does not require
  operator input, and log exact deferrals.
- Target: `/Users/eric/Work/vscode/forked/gosling`, baseline
  `48946f41585c1d34456614bf0ccba0f948852e6e`.
- Initial worktree: clean; `main` matched `origin/main`. A local campaign branch
  was created at the baseline, but the completed repair commit was recorded on
  local `main` as `a9de163b5`; the campaign branch remains at the baseline. No
  push, merge, release publication, tag, or other remote mutation is authorized.
- Involvement: L1 low. The operator asked the campaign to proceed and to defer
  only items that require input.
- Authority: governed repository repair. Product/security/architecture choices,
  release publication, credentials, external services, and protected deferred
  programs are decision gates rather than implicit authority.
- Canonical instructions: `AGENTS.md`. `GEMINI.md` is absent. `.giles/*.yaml`
  is advisory and cannot be promoted without a fresh Giles scan.
- Baseline validation:
  - `source bin/activate-hermit && cargo test -p gosling --lib` passed:
    1,753/1,753 tests.
  - `cd documentation && pnpm run typecheck` failed with 18 TypeScript errors.
  - Desktop typecheck did not start under ambient pnpm 10.6.4 because the
    package requires pnpm 10.30.0 or newer; the repository Hermit environment
    will be used for the stage validation.
  - `cargo-deny`, `giles`, `gitleaks`, `trufflehog`, and `cargo-audit` are not
    installed in the current environment.
- Architecture baseline: `docs/architecture.md` governs core/interface
  ownership; accepted ADRs govern workspace/library/research behavior; release
  identity and publication are governed by `RELEASE.md` and
  `RELEASE_CHECKLIST.md`. The documentation TypeScript repair does not amend a
  runtime architecture or persisted contract.

## Gate 1 — full unfinished-record inventory

The canonical `docs/TODO.md` contains 39 meaningful unfinished entries after
excluding one struck-through resolved item and one warning explicitly recorded
as closed. The active polish ledger and documentation index add status mirrors
and three documentation-governance follow-ups. Every class is dispositioned
below.

| Class | Entries | Evidence | Disposition |
|---|---:|---|---|
| Repairable documentation defect | `TODO-20260817-004` | Documentation typecheck has 18 deterministic errors in config, React return types, prompt types, and theme augmentations. | in scope |
| Stale or duplicate records | version alignment; repository identity; `PATH-GSL-001`; Grok/xAI struck row; chat auto-follow; CLI usage reporting | Current manifests are 1.1.0; release identity log names the selected candidate and remote; README documents shared AAIF discovery; current source/tests implement chat following and CLI usage; adjacent TODO text records the Grok repair. | in scope as record reconciliation, with focused validation |
| Documentation governance follow-ups | durable test ledger; scoped documentation TODO; `.dory/` disposition | A test ledger already exists; a documentation-scoped ledger is requested by `documentation/INDEX.md`; `.dory/` ownership is an operator policy choice. | first two in scope; `.dory/` deferred |
| Product/security/data decisions | `DAT-GSL-002`, `NEG-GSL-001`, `INV-GSL-001`, `ACP-GSL-003`, `NEG-GSL-005`, `CON-GSL-001`, `SEC-GSL-003`/`SEC-GOS-007` | Each open record states a competing accepted behavior or missing authority contract. | deferred — operator/architecture decision required |
| Architecture migrations | `ARC-GSL-002`, `ARC-GSL-003`, `ARC-GSL-004`, `ARC-GSL-005` | Cross-crate ownership, provider ports, MCP dependency direction, and process-global state need governed architecture amendments. | routed to architecture planning; not a defect patch |
| External-tool/security prerequisites | `RSP-GSL-001`, `RSP-GSL-002`, `RSP-GSL-003`, `CMP-GSL-004`, Giles uniqueness debt | Required validators are absent; Giles artifacts are advisory and the prior scan crashed. | deferred with exact prerequisite |
| Profile-gated performance residual | `PERF-GSL-003` | The source ledger requires a break-it profile before changing an unmeasured path; the current source already removed the two cited unconditional conversation clones. | verify current record and defer unless measurement proves material cost |
| Release execution | two canonical release TODOs plus active release gate | Local validation can provide evidence, but signing, clean-install, GitHub readiness, tagging, publishing, verification, and announcement contain maintainer and outward-action gates. | local evidence in scope; publication deferred |
| Tagteam program | 11 deferred-live and 5 future-vision entries | Explicitly a feature program gated on workflow contracts, a live daemon, product UI decisions, and staged acceptance. | excluded-feature; retain as deferred program |
| Source modularization | four files at 3,614–5,435 lines | Every file exceeds the repair skill's 2,000-line in-band split ceiling. | routed to `repair-source-modularization` |
| Other feature backlog | Session Handoff / expanded Tagteam | Explicitly recorded as feature backlog, not an open defect. | excluded-feature; retain |

## Gate 2 — locality groups and campaign plan

### Group 1 — documentation build truth

- Work: repair all current documentation TypeScript errors; reconcile prompt
  content types, Docusaurus config/plugin types, React return types, and theme
  type augmentation without weakening the typecheck.
- Files: `documentation/docusaurus.config.ts`, documentation prompt types and
  loaders, and the components/theme files named by the baseline diagnostics.
- Data path: committed Markdown/JSON prompt content and Docusaurus config into
  the static documentation build.
- Modularization: every touched file is below 1,000 lines; patch in place.
- Regression: documentation typecheck, documentation tests/build where
  available, and changed-file formatting.
- Commit boundary: one local stage commit after verification and review.

### Group 2 — record and documentation-governance reconciliation

- Work: close only stale records proven by current source/tests; refresh the
  active polish ledger and stale release-posture report; make the existing test
  ledger discoverable; add the requested scoped documentation TODO ledger; log
  every protected deferral without converting it to success.
- Files: `docs/TODO.md`, `docs/polish/*`, `documentation/INDEX.md`, and the
  campaign report/log.
- Data path: source/test evidence into canonical and derived status records.
- Modularization: documentation only.
- Regression: exact unchecked-item inventory, governance-marker checks,
  Markdown link/path checks where available, and scoped diff review.
- Commit boundary: one local stage commit after Group 1 evidence is available.

## Cross-stage risks

- A type declaration may reflect real content variation rather than an error;
  Group 1 must inspect representative JSON/front matter before widening it.
- `RELEASE_CHECKLIST.md` explicitly forbids inferring or automatically checking
  gates. The campaign may record evidence but will not mark release gates
  complete without the prescribed observation.
- Deferred architecture and feature programs remain protected. Record cleanup
  must not erase or silently downgrade them.

## Gate 3 — repairs and focused evidence

### Group 1 result — documentation build truth

- Aligned the Docusaurus runtime and type packages at 3.10.2 and refreshed the
  canonical npm lockfile.
- Repaired all 18 TypeScript failures without weakening the typecheck: prompt
  JSON shape/loader typing, Docusaurus reading-time configuration, React 19
  return types, custom blog front matter, and react-markdown code handling now
  match their actual producers and library contracts.
- The first production build found a separate broken link from the v1.1.0
  release note to the root release checklist. The source now uses the canonical
  repository URL and the build passes.
- Evidence:
  - `npm run typecheck` — passed.
  - `npm test` — passed, 16/16 tests.
  - `npm run build` — passed; client/server compiled and 165 Markdown pages
    exported.
- Dependency audit: updating the lockfile and running non-forcing `npm audit
  fix` reduced the report from 48 advisories (including two critical) to 25
  transitive advisories (19 high, 6 moderate). The remaining roots are
  `image-size`, `serialize-javascript`, and `uuid` through `sockjs`; npm reports
  no compatible complete automated fix. Recorded as `RSP-GSL-004`, not closed.

### Group 2 result — record and governance reconciliation

- Closed stale records for version alignment, repository identity,
  `PATH-GSL-001`, the duplicate Grok row, chat auto-follow, and CLI usage only
  after checking their current source, tests, manifests, origin, and maintained
  documentation.
- Updated current GitHub repository links and repository-gated workflows from
  the retired placeholder to the origin designated by `UPSTREAM.md`, README,
  and the 2026-08-23 maintainer record. Historical audit/report evidence, the
  separately governed npm package scope, the compatibility source recognizer,
  and the pinned legacy GHCR digest were preserved.
- Added `documentation/TODO.md`, linked the existing test ledger, corrected the
  documentation index's false claim that `docs/` did not exist, and routed the
  `.dory/` question to an explicit governance decision.
- Reconciled PERF-GSL-003 by removing stale clone call-site claims while
  retaining its profile-gated residual.

### Group 3 result — failures discovered in current remote CI

- Read-only GitHub checks found baseline CI run `33120050894`, shell-package
  run `33120050956`, and BuildNotify run `33121228380` failing on 2026-08-27.
- Repaired the cross-platform shell contract failures:
  - the consumer validator and scaffold now recognize the already exposed
    session-extension read/write capabilities and enforce their prerequisites;
  - path, ASAR-entry, workflow-line-ending assertions are platform-neutral;
  - cfg-specific Rust imports, arguments, and helpers no longer become Windows
    `-D warnings` errors.
- `pnpm run shell:test-profile` passed 67/67 tests and `pnpm run
  shell:check-profiles` passed all three profiles locally.
- A macOS cross-check installed the Windows Rust standard library but stopped
  in native `libsqlite3-sys`/`zstd-sys` C compilation because the host lacks a
  Windows SDK/header toolchain. The patched Rust cfg warnings therefore remain
  partially validated until a real Windows runner executes the next revision.
- BuildNotify's absent Discord webhook, disabled branch-protection ruleset, and
  a remote rerun all require repository-owner configuration or a published
  revision; they remain explicit release prerequisites.

## Gate 4 — remaining-record disposition

The canonical TODO now has 36 unchecked rows plus four partial rows. One partial
row (`SECN-GSL-001`) is explicitly a closed warning and is excluded from the
actionable count; the others are profile-gated `PERF-GSL-003` and two repairs
awaiting an authoritative remote cross-platform rerun. The unchecked rows are
retained because they are one of:

- product/security/data decisions: eight rows;
- architecture/external-tool/dependency/profile prerequisites: eight rows,
  including `PERF-GSL-003`;
- remote cross-platform confirmation: two partial rows;
- release execution and external repository configuration: three rows;
- Tagteam feature program: 16 rows;
- routed source modularization: one row;
- Session Handoff feature backlog: one row;
- `.dory/` governance decision: one documentation-ledger row.

The documentation ledger mirrors `RSP-GSL-004`; that mirror is not an
additional defect.

## Gate 5 — campaign validation

- `cargo fmt --check` — passed.
- `cargo build` — passed.
- `cargo test -p gosling -p gosling-mcp -p gosling-cli` — passed: 1,886
  gosling library tests plus integration/doc tests, 252 CLI tests, and 88 MCP
  tests; expected ignored tests remained visible.
- `cargo clippy --all-targets -- -D warnings` — passed.
- Desktop `pnpm run typecheck` and `pnpm test -- --run` — passed: 133 files,
  1,069 tests.
- Shell profile/conformance and profile checks — passed: 67 tests, three
  profiles.
- Documentation typecheck, 16 tests, and production build — passed; 165
  Markdown pages exported.
- OIDC proxy tests — passed: 11 tests. The local Cloudflare harness warned that
  its runtime supports an older compatibility date than configured.
- Windows Rust cross-check — partial: target standard library installed, then
  native dependency compilation stopped because macOS lacks Windows C
  headers/SDK. No patched-crate warning was reached.

No runtime architecture, persistence schema, or accepted ADR contract changed.
The shell fix restores the existing declared-capability boundary; the identity
work changes repository routing/metadata only. Deferred architecture records
remain open and no new drift was introduced.
