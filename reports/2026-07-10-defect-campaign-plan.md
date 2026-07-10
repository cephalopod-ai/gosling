# Defect-repair campaign plan — 2026-07-10

**Status:** plan only — no product, workflow, dependency, or repository-settings
changes are authorized by this document.

**Skill:** `020_repair/repair-defect-campaign`.
**Findings source:**
[`reports/2026-07-10-audit-repair-followups.md`](2026-07-10-audit-repair-followups.md),
current source, and GitHub Actions for `main` at
`6ab24704ea2b0698700ac1af88bd2cf0a74a0850`.
**Campaign scope:** all 10 confirmed, non-deferred defects in the ledger. The source
TODO/maintenance list is not feature work in this campaign, and the deliberately
deferred security/architecture backlog is protected.

## Gate 0 — orientation, safety, and git posture

| Item | Recorded state |
| --- | --- |
| Repository and baseline | `repo-makeover/gosling`, local `main` at `6ab24704`; `origin/main` is identical. |
| Worktree | Modified `reports/2026-07-10-audit-repair-followups.md` only. It is related ledger work from the preceding audit and must be preserved. |
| Git policy | Plan-only: no commit and no push. Before execution, create a campaign branch from current `main`; stage commits are local-only unless the maintainer separately authorizes a push. |
| Repository rules | Follow root `AGENTS.md`: activate Hermit for Rust commands, format edits, use targeted tests, and run the requested build/test/clippy sequence only when change verification is authorized. Use `reports/` for campaign artifacts. |
| Baseline state | Red. Current `main` fails Rust tests, ACP schema generation, desktop i18n validation, live-provider tests, Docker publishing, and scheduled Pages deployment as recorded below. |
| Validation inventory | Rust: `cargo build`, targeted `cargo test`, `cargo clippy`; UI: `pnpm run typecheck`, `pnpm test`, i18n scripts; workflows: rerun affected GitHub Actions jobs; documentation: `documentation/scripts/verify-build.sh`. |
| Deferred work | Sections A–G of the audit-repair ledger, the 2026-07-09 deferred findings, and `quick-xml` cargo-deny advisories are deliberately deferred and out of scope. |
| Session log | This plan is the Gates 0–2 record. At execution, create `reports/2026-07-10-defect-campaign-session-log.md` and record Gates 3–9 per stage. |

## Gate 1 — full defect inventory

| ID | Domain | Priority / complexity | Touch set | Evidence and regression surface | Disposition |
| --- | --- | --- | --- | --- | --- |
| CI-001 | Correctness, build/CI | P0 / medium | `providers/chatgpt_codex.rs::create_codex_request`; `providers/codex.rs::metadata`; `providers/utils.rs::get_google_final_status` and their unit tests | CI run `29119632291`: three provider tests fail on reasoning effort, model context limits, and Google error-status mapping. | In scope |
| CI-002 | Build/CI | P0 / low | `crates/gosling/src/posthog.rs::sanitize_value`; ACP schema generator path in `Justfile` | ACP-schema job fails under `-D warnings` because `sanitize_value` is unused. | In scope |
| CI-003 | Build/CI | P1 / low | `ui/desktop/src/i18n/messages/en.json`; UI message source discovered by `pnpm i18n:extract` | Desktop `lint:check` reports that the generated English catalog is out of date. | In scope |
| CI-004 | Build/CI | P1 / medium | `.github/workflows/pr-smoke-test.yml::compaction-tests`; `scripts/test_compaction.sh`; repository Actions secret configuration | Compaction job receives an empty Anthropic key and all live compaction checks return HTTP 401. | In scope; external secret authority required |
| CI-005 | Build/CI, reliability | P1 / medium | `.github/workflows/pr-smoke-test.yml::smoke-tests`; `ui/desktop/tests/integration/test_providers*`; Claude Code CLI provisioning/configuration | The normal provider smoke test enables `claude-code`, whose process exits before ACP initialization. | In scope; CI environment decision required |
| CI-006 | Build/CI, release | P1 / medium | `.github/workflows/publish-docker.yml`; `Dockerfile`; Buildx cache and runner capacity | The arm64 release build exhausts disk (`os error 28`) before image publishing. | In scope |
| CI-007 | Build/CI, docs deployment | P1 / medium | `.github/workflows/rebuild-skills-marketplace.yml`; `.github/workflows/deploy-docs-and-extensions.yml`; GitHub Pages repository setting | Scheduled marketplace rebuild builds documentation, then `configure-pages` fails because Pages is disabled. | In scope; repository-settings authority required |
| DEF-001 | Frontend/UX bug | P2 / low | `scripts/diagnostics-viewer.py::action_search`; `SearchOverlay` and viewer-content widgets | Ctrl/Cmd+F opens a UI overlay but performs no query. | In scope |
| DEF-002 | Data integrity, backend | P1 / high | `crates/gosling/src/acp/server.rs::GoslingAcpAgent::new`; global `Paths::in_state_dir` consumers; ACP fixture data roots | `data_dir` is accepted but global state paths, including request logs, bypass it, defeating ACP data-root isolation. | In scope |
| DEF-003 | Correctness, ACP | P2 / high | ACP provider session lifecycle in `crates/gosling/src/acp/provider.rs`; `tests/acp_fixtures/provider.rs`; `tests/acp_provider_test.rs`; shared ACP load-session tests | Four ACP-provider load-session tests are ignored because the provider connection always reports “not implemented.” | In scope |

### Excluded candidates and protected work

| Candidate | Disposition | Reason |
| --- | --- | --- |
| TODO-001 through TODO-010 in the ledger | Excluded feature/maintenance work | They are test coverage, product capabilities, cleanup, or tooling improvements rather than presently broken behavior. Handle them under their own feature or maintenance plans. |
| Sections A–G of the audit-repair ledger and deferred findings in `reports/2026-07-09-defect-audit-and-repair.md` | Excluded intentionally deferred | The user has not explicitly reopened each protected security, architecture, or product-policy item. |
| `quick-xml` cargo-deny advisories | Excluded intentionally deferred | The ledger records the required dependency/security posture decision; do not use its CI failure as an adjacent fix. |

## Gate 2 — locality grouping and ordered campaign plan

### Affinity and ordering

- CI-004 and CI-005 share `pr-smoke-test.yml`, CI secret/provisioning policy, and
  provider-test execution, so they are one stage.
- DEF-002 and DEF-003 are both ACP session concerns but do not share an implementation
  file; combining them would create a large, cross-cutting review surface. They remain
  separate stages.
- The remaining defects have no meaningful shared touch set and stay isolated.
- Within each priority tier, localized files precede broad workflow or data-root work.

### Modularization decisions

| File | Lines | Planned edit | Decision |
| --- | ---: | --- | --- |
| `crates/gosling/src/posthog.rs` | 555 | One unreachable helper | No modularization (`<=1000`). |
| `crates/gosling/src/providers/chatgpt_codex.rs` | 1561 | Local provider-contract behavior/test adjustment | No modularization: not a heavy multi-function edit. |
| `crates/gosling/src/providers/codex.rs` | 1377 | Model metadata and assertions | No modularization: not a heavy multi-function edit. |
| `crates/gosling/src/providers/utils.rs` | 473 | Error-status contract and unit test | No modularization (`<=1000`). |
| Generated i18n catalog and workflows | 67–3122 | Generated catalog or declarative configuration | Do not modularize generated/config files. |
| `scripts/diagnostics-viewer.py` | 901 | One search action and focused tests | No modularization (`<=1000`). |
| `crates/gosling/src/acp/server.rs` | 4005 | Minimal data-root injection/wiring only | Do not split in this campaign (`>=2000`). Route a dedicated `repair-source-modularization` follow-up if structural extraction is needed. |
| `crates/gosling/src/acp/provider.rs` | 2190 | Minimal load-session lifecycle wiring only | Do not split in this campaign (`>=2000`). Route a dedicated `repair-source-modularization` follow-up if the implementation cannot remain localized. |

### Stages

#### Stage 1 — unblock ACP schema generation

- Defects: CI-002 (P0, low).
- Files/functions: `posthog.rs::sanitize_value`; schema-generation invocation only for validation.
- Data path: telemetry property sanitization; do not re-enable product telemetry as part of this fix.
- Plan: decide whether the helper is truly needed by a reachable, privacy-preserving path. Remove it if telemetry remains permanently disabled; otherwise call it at the serialization boundary and test recursive redaction.
- Regression surface: unit tests for the retained behavior, `just generate-acp-schema`, `just check-acp-schema`, and the narrow Rust crate check.
- Adversarial focus: do not accidentally expose telemetry, bypass sanitization, or update generated schema without checking it in.
- Documentation: mark CI-002 resolved in the ledger after verification.
- Commit boundary: one local commit, `fix(ci): unblock ACP schema generation`.

#### Stage 2 — reconcile provider contracts

- Defects: CI-001 (P0, medium).
- Files/functions: `create_codex_request`, Codex model metadata/context limits,
  `get_google_final_status`, and their colocated unit tests.
- Data paths: model identifier → reasoning-effort capability; model identifier → context limit; provider JSON error payload → HTTP status.
- Plan: establish authoritative expected behavior from the supported provider contracts before changing code or assertions. Make each mapping and test agree, preserving fallback behavior for unknown models and malformed payloads.
- Regression surface: the three failing tests plus neighboring reasoning-effort, model-metadata, and Google retry/status tests; then `cargo test -p gosling --lib` once targeted tests pass.
- Adversarial focus: case variations, unsupported effort levels, unknown model fallbacks, malformed/non-numeric error codes, and accidental changes to retry classification.
- Documentation: update model/provider references only if user-visible limits or supported effort levels change.
- Commit boundary: one local commit, `fix(providers): reconcile model and error contracts`.

#### Stage 3 — regenerate desktop i18n output

- Defects: CI-003 (P1, low).
- Files/functions: `ui/desktop/src/i18n/messages/en.json` and precisely the source messages discovered by the extractor.
- Data path: FormatJS source messages → checked-in English catalog → locale validation.
- Plan: run the repository extractor, review every generated delta, and retain only source-derived catalog changes.
- Regression surface: `pnpm i18n:check`, `pnpm run typecheck`, and desktop lint; run focused UI tests only if message-bearing components change.
- Adversarial focus: accidental message-ID churn, source-language text loss, and generated-file edits unrelated to extraction.
- Documentation: no public docs expected; mark CI-003 resolved in the ledger.
- Commit boundary: one local commit, `chore(i18n): refresh generated English messages`.

#### Stage 4 — make live provider CI prerequisites explicit

- Defects: CI-004 and CI-005 (P1, medium).
- Files/functions: `pr-smoke-test.yml` normal-provider and compaction jobs;
  `scripts/test_compaction.sh`; desktop provider integration test configuration;
  CI secret and Claude Code CLI setup.
- Data paths: Actions secrets and installed CLI → provider process initialization → live provider/compaction assertions.
- Plan:
  1. Add explicit preflight checks that distinguish absent credentials/CLI from product failures.
  2. With maintainer authorization, restore the required Anthropic secret and provision or pin a compatible Claude Code CLI.
  3. If live credentials or the CLI are intentionally unavailable, make the jobs skip with a visible, non-green-by-accident status and preserve a hermetic substitute test.
  4. Keep the live checks mandatory only when their prerequisites are actually available.
- Regression surface: rerun normal provider smoke, compaction tests, and hermetic provider tests on the same commit.
- Adversarial focus: no secret echoing, no accidental broad skip that masks provider regressions, CLI version drift, and no false “compaction succeeded” result after an authentication error.
- Documentation: document the required secrets/CLI version in the workflow or contributor CI documentation.
- Commit boundary: one local commit, `fix(ci): gate live provider test prerequisites`.
- External gate: repository secret administration and any licensed CLI installation require maintainer authorization.

#### Stage 5 — restore Docker release publishing capacity

- Defects: CI-006 (P1, medium).
- Files/functions: `publish-docker.yml`, `Dockerfile`, Buildx cache policy, and possibly the build matrix.
- Data path: multi-platform Buildx cache and release artifacts → GitHub runner disk → pushed image and provenance attestation.
- Plan: measure image/build-cache pressure, choose the smallest durable mitigation (cache-mode reduction, cache cleanup, separate architecture builds, or a larger runner), and preserve multi-architecture manifest plus provenance behavior.
- Regression surface: a manual workflow run that builds both `linux/amd64` and `linux/arm64`, pushes the manifest, and completes attestation; inspect builder disk use before/after.
- Adversarial focus: cache eviction regressions, dropped architecture support, tag/manifest mismatch, provenance subject mismatch, and release-only failures.
- Documentation: update release/build instructions if the runner, matrix, or cache behavior changes.
- Commit boundary: one local commit, `fix(ci): prevent Docker multi-arch disk exhaustion`.

#### Stage 6 — reconcile Pages deployment configuration

- Defects: CI-007 (P1, medium).
- Files/functions: `rebuild-skills-marketplace.yml`, `deploy-docs-and-extensions.yml`, and GitHub Pages repository settings.
- Data path: documentation build → Pages configuration → uploaded artifact → deployment URL.
- Plan: obtain maintainer choice: enable Pages with GitHub Actions as the source, or make both deployment workflows deliberately no-op/disabled. Apply the same choice to both workflows so scheduled and push behavior agree.
- Regression surface: manual dispatch of the marketplace rebuild and a documentation-only push/test branch; verify artifact upload and deployment, or an explicit successful skip if Pages is intentionally disabled.
- Adversarial focus: duplicated deployments, concurrency cancellation, unnecessary documentation builds, token permission scope, and a workflow that reports success without serving the intended docs.
- Documentation: align `documentation/README.md` and deployment guidance with the selected hosting model.
- Commit boundary: one local commit, `fix(ci): align documentation deployment with Pages configuration`.
- External gate: enabling/disabling Pages is a repository-settings change and requires maintainer authorization.

#### Stage 7 — restore ACP data-root isolation

- Defects: DEF-002 (P1, high).
- Files/functions: `GoslingAcpAgent::new`, every state/log path currently read through `Paths::in_state_dir`, ACP server fixtures, and any request-log construction path.
- Data path: ACP `data_dir` option → session/log/state storage → per-agent test root.
- Plan: enumerate all global path consumers reached during ACP startup, inject an explicit data-root dependency or scoped path provider, and ensure session, request-log, and auxiliary state all resolve under the configured root. Keep default CLI/desktop paths behavior-compatible.
- Regression surface: isolated ACP server tests with two temporary data roots, request-log assertions, existing ACP server session/load tests, and targeted concurrent-agent isolation tests.
- Adversarial focus: cross-root state leakage, races from global path mutation, migration/legacy-state lookup, relative-path traversal, and changed desktop defaults.
- Documentation: update ACP configuration docs if the data-root contract becomes explicit.
- Modularization: `acp/server.rs` is 4005 lines. Make only the smallest safe change in this campaign; if the required seam is not already isolated, create a routed follow-up for `repair-source-modularization` rather than splitting it in-stage.
- Commit boundary: one local commit, `fix(acp): honor configured data roots`.

#### Stage 8 — implement diagnostics viewer search

- Defects: DEF-001 (P2, low).
- Files/functions: `scripts/diagnostics-viewer.py::action_search`, `SearchOverlay`, text/JSON viewer widgets, and focused Python tests if the existing test harness supports Textual widgets.
- Data path: query input → visible content/tree nodes → deterministic match navigation and empty-state feedback.
- Plan: define search scope for both plain text and JSON-rendered diagnostics, implement match filtering/navigation with explicit clearing behavior, and ensure focus/keyboard shortcuts remain accessible.
- Regression surface: focused widget tests or a scripted Textual smoke test covering match, no-match, next/previous, close, and malformed JSON rendering.
- Adversarial focus: large diagnostic files, case sensitivity, nested JSON, Unicode, stale content after file switching, and no-match accessibility feedback.
- Documentation: update diagnostics-viewer usage only if search syntax or bindings change.
- Commit boundary: one local commit, `fix(diagnostics): implement viewer search`.

#### Stage 9 — implement ACP provider session loading

- Defects: DEF-003 (P2, high).
- Files/functions: ACP provider session lifecycle in `acp/provider.rs`; `tests/acp_fixtures/provider.rs::load_session`; ignored provider tests; shared ACP load-session assertions.
- Data path: ACP `session/load` request → provider-backed session state/model state/MCP configuration → client replay and subsequent prompt.
- Plan: mirror the already-working ACP server contract only where provider ownership permits it. Define provider-session persistence and lookup semantics, then implement loading, model/mode state, MCP restoration, unknown-session errors, and cleanup. Unignore tests only after behavior is implemented.
- Regression surface: all four formerly ignored provider tests, shared load-session tests, provider session lifecycle tests, and targeted reconnect/close-session cases.
- Adversarial focus: stale provider handles, mismatched session IDs, duplicate replay, MCP extension restoration, model state leakage, cancellation, and concurrent loads.
- Documentation: update ACP client/provider documentation if load-session support or limits change.
- Modularization: `acp/provider.rs` is 2190 lines. Use the smallest safe local implementation; if the session seam needs broad movement, route `repair-source-modularization` rather than splitting it in this stage.
- Commit boundary: one local commit, `fix(acp): implement provider session loading`.

## Cross-stage risks and execution constraints

- Stage 2 cannot safely choose new model limits or reasoning levels without an
  authoritative provider-contract decision.
- Stages 4 and 6 need repository-level secret or settings authority; do not replace
  those requirements with hard-coded secrets or silent workflow skips.
- Stage 5 must retain multi-architecture publishing and provenance attestation.
- Stage 7 must not solve ACP isolation by mutating global paths across concurrent
  agents; it needs explicit ownership/scoping.
- Stages 7 and 9 touch files over 2000 lines. Structural refactoring is routed to
  `repair-source-modularization`, never bundled into the defect stage.
- A stage that reveals a deliberately deferred architecture issue pauses and updates
  the ledger; it does not reopen that work implicitly.

## Gate 9 closeout requirements for the future execution

1. Run all targeted validations again after the final stage and rerun the affected
   GitHub Actions workflows on the campaign branch.
2. Perform an end-to-end adversarial review of provider configuration, ACP session
   lifecycle/data roots, and release/documentation workflows.
3. Update the open-work ledger with every resolved, blocked, and routed item; retain
   the deferred-work boundary.
4. Complete `reports/2026-07-10-defect-campaign-session-log.md` with commands,
   results, reviews, commit hashes, and external gates.
5. Report `completed_verified`, `completed_with_partial_verification`, or
   `partially_completed_groups_remaining` based on actual evidence. Do not push unless
   separately authorized.

## Plan-only verification

- The defect inventory was reconciled against current source and GitHub Actions runs.
- File-size decisions were measured before grouping.
- No source, workflow, dependency, repository-setting, test, commit, or push action
  was performed while creating this plan.
