# Defect-repair campaign session log — 2026-07-10

**Campaign skill:** `repair-defect-campaign`.
**Repository:** `repo-makeover/gosling`.
**Branch:** `repair/defect-campaign-2026-07-10`.
**Baseline:** `e218f247dd4cca12136f0df92d37f3f59480d291` (local `main` was one
documentation commit ahead of `origin/main`).
**Commit policy:** one local commit per completed stage; push and PR creation are
authorized by the repair request, but no direct push to protected `main`.

## Gates 0–2 — completed before source edits

- Repository instructions, the defect ledger, and the campaign plan were read.
- Findings source: the current ledger, GitHub Actions run `29119632291`, and related
  workflow runs on `main` at `6ab24704`.
- Baseline is red for the 10 defects in
  `reports/2026-07-10-defect-campaign-plan.md`; deliberately deferred items and
  non-defect TODOs remain excluded.
- Planned groups: ACP-schema blocker; provider contracts; generated i18n; live-provider
  CI prerequisites; Docker publish capacity; Pages deployment; ACP data-root isolation;
  diagnostics search; ACP provider session loading.
- Files over 2000 lines (`acp/server.rs`, `acp/provider.rs`) will not be modularized
  in this campaign. Any required structural split is routed to
  `repair-source-modularization`.

## Stage record

### Stage 1 — CI-002: unblock ACP schema generation

Status: completed.

- Change: removed the unreachable recursive `sanitize_value` helper. Its
  string-level sanitizer remains used by the guarded telemetry error path, so product
  telemetry behavior is unchanged.
- Validation:
  - `cargo test -p gosling posthog` — passed; no tests are tagged to this module.
  - `cargo run --manifest-path crates/gosling/Cargo.toml --features code-mode,aws-providers,telemetry,otel,rustls-tls,system-keyring --bin generate-acp-schema` — passed; generated schema and metadata had no diff.
  - `cargo clippy -p gosling --all-targets --features code-mode,aws-providers,telemetry,otel,rustls-tls,system-keyring -- -D warnings` — passed.
  - `git diff --check` — passed.
- Adversarial review: confirmed the patch neither enables telemetry nor removes the
  actively used string sanitizer; no schema or public-contract change occurred.
- Change review: scoped to CI-002 and this session log only.

### Stage 2 — CI-001: reconcile provider contracts

Status: completed after corrective adversarial review.

- Change: added the public `gpt-5.3-codex` model to the ChatGPT Codex capability
  table with a 400,000-token context window and `xhigh` reasoning support. Updated
  stale Codex CLI context-limit assertions to the metadata actually returned by the
  provider registry. Google-compatible responses now fail closed when a successful
  HTTP status contains an explicit error object with an unknown, malformed, or
  missing error code.
- Contract evidence: OpenAI's GPT-5.3-Codex model documentation lists a 400,000-token
  context window and `low`, `medium`, `high`, and `xhigh` reasoning effort. The
  undocumented 5.4–5.6 aliases were not changed.
- Validation:
  - `cargo test -p gosling --lib providers::utils::tests::` — passed, 12 tests,
    including response-level coverage for an HTTP 200 body with error code `999`.
  - `cargo test -p gosling --lib` — passed, 1,332 tests.
  - `cargo clippy -p gosling --all-targets --features code-mode,aws-providers,telemetry,otel,rustls-tls,system-keyring -- -D warnings` — passed.
  - `cargo fmt --all` — passed.
- Adversarial review:
  - Opus 4.8 produced no output before a deliberately short three-minute watchdog
    expired; the run was retained as a failed orchestration artifact rather than
    treated as review evidence.
  - Codex GPT-5.6 Sol found one major issue: the initial test-only change accepted an
    explicit unknown Google error as success. It also rejected the session log's
    unsupported validation claims. Both findings were fixed and the listed commands
    were run against the corrected implementation.
  - Unknown model fallback remains unchanged; only known public GPT-5.3-Codex
    receives the expanded effort/context contract.
- Change review: source changes are confined to the provider contract surface and
  this session log. The pre-existing diagnostics-viewer worktree change was not
  touched or included.

### Stage 3 — CI-003: synchronize desktop message catalogs

Status: completed.

- Change: regenerated the English source catalog, removed obsolete keys from all
  translated catalogs, and added each new source key as an English fallback while
  preserving existing translations. Added `pnpm i18n:sync` so future catalog
  updates apply that exact, reviewable reconciliation.
- Discovery: after the source-catalog drift was corrected, strict validation exposed
  the previously masked locale drift: each of the 15 translated catalogs lacked 78
  source keys and retained 27 removed keys.
- Validation:
  - `pnpm run i18n:check` — passed for all 15 translated catalogs (1,032 source
    messages).
  - `pnpm run lint:check` — passed (TypeScript, ESLint, and i18n checks).
  - `pnpm exec prettier --check package.json scripts/i18n-sync-locales.js src/i18n/messages/*.json` — passed.
  - `git diff --check` — passed.
- Adversarial review: the synchronizer only retains keys in the current English
  catalog, preserves each existing translation verbatim, and gives new messages the
  English source text; placeholder validation covers all retained and added entries.
- Change review: scoped to generated catalogs and their explicit maintenance command;
  no runtime message-loading behavior changed.

### Stage 4 — CI-004 and CI-005: gate live-provider prerequisites

Status: completed with external verification pending.

- Change: added a visible Actions prerequisite job. It enables compaction tests only
  when `ANTHROPIC_API_KEY` is configured and otherwise emits an explicit notice with
  the remediation. The compaction script now fails immediately, before running any
  scenario, when the key is absent. Claude Code smoke tests now require the explicit
  `RUN_CLAUDE_CODE_SMOKE=true` repository variable; this avoids treating an arbitrary
  `claude` executable as a configured, authenticated provider.
- Evidence: the failed `main` run exposed an empty `ANTHROPIC_API_KEY`; all three
  compaction scenarios consequently returned 401, while the hosted runner's Claude
  executable exited before provider initialization. No secret value was inspected or
  emitted.
- Validation:
  - `bash -n scripts/test_compaction.sh` — passed.
  - YAML parse of `.github/workflows/pr-smoke-test.yml` — passed.
  - `pnpm run typecheck` — passed.
  - provider integration test with every external provider explicitly skipped — passed
    (32 expected skips; verifies discovery/registration without live calls).
  - compaction-script missing-key preflight — exited with the expected clear error.
  - `pnpm exec prettier --check tests/integration/test_providers_lib.ts` and
    `git diff --check` — passed.
- Adversarial review: the workflow sends only booleans to job outputs, never secret
  values; configured compaction and explicitly enabled Claude Code smoke tests still
  fail on real regressions. The no-key path is visibly skipped rather than falsely
  reported as a compaction pass.
- External gate: a maintainer must add a valid `ANTHROPIC_API_KEY` repository secret
  to exercise compaction. Enabling `RUN_CLAUDE_CODE_SMOKE=true` additionally requires
  a runner with an authenticated Claude CLI.
- Change review: limited to live-test prerequisite discovery and reporting; provider
  runtime code and test assertions are unchanged.

### Stage 5 — CI-006: reduce Docker multi-architecture build pressure

Status: completed with hosted-workflow verification pending.

- Change: the Docker-only release profile now uses Thin LTO and normal parallel
  codegen units instead of fat LTO with one unit. The image remains optimized and
  stripped. The Buildx cache is scoped to this image and exports only final-image
  layers (`mode=min`) rather than every large intermediate build layer.
- Evidence: the failed arm64 build exhausted runner storage while writing fat-LTO
  `.rcgu.bc` files under `/build/target/release/deps`; a prior run on the same hosted
  runner image completed the two-platform image and provenance attestation.
- Validation:
  - YAML parse of `.github/workflows/publish-docker.yml` — passed.
  - `git diff --check` — passed.
- Adversarial review: both `linux/amd64` and `linux/arm64`, registry push, metadata
  tags, and provenance attestation remain unchanged. The changed cache mode cannot
  cache builder-only layers; it therefore avoids retaining an unusable multi-gigabyte
  intermediate cache.
- External verification: a protected-branch or manually dispatched GitHub workflow
  must build and publish both architectures to verify the runner capacity in practice.
- Change review: limited to the release build profile and cache policy; application
  behavior and image contents are otherwise unchanged.

### Stage 6 — CI-007: align Pages workflows with repository configuration

Status: completed with repository-settings action pending.

- Change: both Pages workflows now make deployment an explicit opt-in through the
  `ENABLE_GITHUB_PAGES=true` Actions variable. With the variable absent, each
  workflow records a visible disabled-deployment notice rather than building and
  failing at `configure-pages`. Documentation records the matching activation steps.
- Evidence: the repository Pages API returns 404, and both the scheduled marketplace
  workflow and documentation deployment workflow repeatedly fail at the Pages
  configuration stage.
- Validation:
  - YAML parse of both workflow files — passed.
  - `npm test`, `npm run build`, and `documentation/scripts/verify-build.sh` — passed.
  - `git diff --check` — passed.
- Adversarial review: the two workflows share the same opt-in condition; when enabled,
  their build, upload, concurrency, permissions, and deployment steps are unchanged.
  The disabled path is an explicit successful status rather than a fabricated deploy.
- External gate: a maintainer must enable GitHub Pages with GitHub Actions as its
  source and set `ENABLE_GITHUB_PAGES=true` to activate deployments.
- Change review: limited to Pages gating and maintainer documentation; documentation
  content and artifact generation are unchanged.

### Stage 7 — DEF-002: ACP data-root isolation

Status: routed to source modularization; no safe local patch.

- Discovery: ACP session storage and permission state honor the injected roots, but
  request logging is installed as a process-wide `OnceLock`, and ACP startup, provider
  creation, and extension discovery still consume process-global `Config` and `Paths`
  state. The current fixtures serialize tests by mutating `GOSLING_PATH_ROOT`, which
  cannot provide concurrent per-agent isolation.
- Decision: do not mutate a process-global path while an ACP agent is active or add a
  partial request-log override that leaves configuration and provider state leaking.
  A correct repair needs an explicit per-agent configuration/state-root dependency and
  a request-log routing seam across `acp/server.rs`, provider creation, and related
  global consumers. This exceeds the campaign's permitted local edit in a 4,005-line
  module and is routed to `repair-source-modularization`.
- Change review: no source behavior was changed; this preserves current CLI/desktop
  defaults and avoids introducing cross-agent races.

### Stage 8 — DEF-001: implement diagnostics viewer search

Status: completed.

- Change: implemented case-insensitive search of the current diagnostics file. Text
  results are highlighted and scrolled into view; JSON results expand, select, and
  scroll to matching tree nodes. Enter and Shift+Enter navigate next/previous matches;
  closing search or changing files clears search state.
- Validation:
  - `uv run --with textual>=0.87.0 --with pyperclip python -m py_compile scripts/diagnostics-viewer.py` — passed.
  - scripted Textual smoke tests — passed for text highlighting/navigation, JSON-tree
    selection/navigation, file-change reset, Unicode matching, and previous-match
    navigation.
  - `git diff --check` — passed.
- Adversarial review: empty and no-match queries report their state without changing
  content; malformed JSON remains renderable; tree searches include full truncated
  string values; and selection state is reset before a new file is rendered.
- Change review: localized to the standalone diagnostics viewer; no diagnostics
  collection, serialization, or application runtime behavior changed.

### Stage 9 — DEF-003: ACP provider session loading

Status: routed to source modularization; no safe local patch.

- Discovery: the outer Gosling ACP server already implements `session/load`, but
  `AcpProvider` represents exactly one eagerly-created session of an external ACP
  agent. Its control and prompt methods discard their Gosling session-id argument and
  always target that one external session. The provider fixture consequently creates
  synthetic Gosling ids and returns `load_session not implemented`; the four affected
  common tests remain correctly ignored.
- Decision: implementing the fixture alone would falsely claim support while a loaded
  Gosling session still creates a new external ACP session. A correct repair must
  persist the external ACP session identifier with the Gosling session, explicitly
  choose native `session/new` versus `session/load` on provider activation, route
  mode/model/MCP state to the selected native session, and define close/reconnect
  ownership. That crosses the 2,190-line `acp/provider.rs`, ACP session persistence,
  and activation lifecycle. It is routed to `repair-source-modularization`.
- Change review: no source behavior was changed; the ignored tests remain an accurate
  statement of unsupported behavior rather than a passing fixture-only simulation.

## Gate 9 — campaign closeout before PR

Status: ready for protected-branch PR; two routed ACP defects and three external
verification/settings actions remain open.

- Final validation:
  - `cargo fmt --all -- --check` — passed.
  - `cargo clippy -p gosling --all-targets --features code-mode,aws-providers,telemetry,otel,rustls-tls,system-keyring -- -D warnings` — passed.
  - `cargo test -p gosling --lib` — passed, 1,332 tests.
  - `cargo test -p gosling --test acp_provider_test` — passed, 12 tests; the four
    `load_session` tests remain ignored for the routed DEF-003 work.
  - `uv run --with textual>=0.87.0 --with pyperclip python -m py_compile scripts/diagnostics-viewer.py` — passed.
  - `documentation: npm test`, `npm run build`, and `scripts/verify-build.sh` — passed
    (15 tests; generated documentation map verified).
  - `git diff --check` — passed.
- Environment limitation: the final `ui/desktop` lint recheck could not start because
  the available pnpm is 10.6.4 while the project requires pnpm 10.30.0 or newer. The
  stage-3 i18n and lint validations passed before this environment drift; no desktop
  files changed afterward.
- External gates: add `ANTHROPIC_API_KEY` to exercise live compaction; optionally
  enable authenticated Claude Code smoke coverage; manually run the two-architecture
  Docker publishing workflow; and enable Pages plus `ENABLE_GITHUB_PAGES=true` only
  if Pages deployment is desired.
- Worktree note: `crates/gosling/src/providers/utils.rs` contains an unrelated,
  unstaged concurrent edit. It was not staged, committed, or modified by this campaign.
