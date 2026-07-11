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

### Post-PR CI correction — desktop runtime default assertion

Status: completed after PR check inspection.

- Evidence: PR #21's `Test and Lint Electron Desktop App` job failed because
  `CodeExecutionRuntimeSection.test.tsx` asserted that a missing
  `GOSLING_CODE_EXECUTION_RUNTIME` value selected `Enabled`, whereas the component
  intentionally maps an absent or invalid value to `Disabled`.
- Change: corrected the test name and selected-button expectation. Product runtime
  behavior remains unchanged and retains the safe disabled default.
- Validation: the hosted CI failure directly identifies the prior assertion. The
  focused local recheck uses the project's required pnpm version when available;
  the currently installed pnpm remains too old (10.6.4 versus >=10.30.0).

### Post-PR CI correction — managed-secret profile persistence tests

Status: completed after PR check inspection.

- Evidence: after the runtime assertion was corrected, PR #21's desktop suite exposed
  two auth-settings tests that expected profile creation to persist immediately. The
  component intentionally keeps profile edits local until its explicit `Save` action.
- Change: the VPS and Supabase profile tests now click `Save` before asserting the
  settings write. This verifies the real user flow without changing persistence
  behavior.

### Post-PR CI correction — compacted ACP session-load assertion

Status: completed after full local desktop-suite validation.

- Evidence: the complete desktop suite showed that `sessions.test.ts` still expected
  a bare ACP `session/load` request. Production has intentionally included
  `gosling.loadMode=compacted` and `tailLimit=50` since the compacted-resume paging
  change.
- Change: the request assertion now covers the compacted-load metadata, preserving
  regression coverage for the bounded resume contract.

## Corrective continuation — 2026-07-11

This section supersedes the earlier Stage 3 and Stage 8 validation/review summaries.
The original summaries were based on incomplete checks and did not record defects found
when the landed commits were re-reviewed. Work continued from `main` and is delivered
through protected-branch PR #22.

### Stage 3 corrective audit — locale synchronization

Status: completed in commits `1149d33f1` and `65a5cd1b9`.

- The landed locale commit retained stale translations for the changed
  `permissionSetting.permissionRulesDescription` source message and its synchronizer
  could overwrite concurrent translator edits or leave partial catalog updates.
- The corrected synchronizer fingerprints source messages, requires explicit
  acknowledgement for changed or removed messages, rejects malformed and duplicate-key
  JSON before mutation, and uses a durable claim/journal/rollback protocol with
  process locking and startup recovery. Recovery checks original and output digests so
  it cannot displace a newer destination edit.
- Validation:
  - 21 synchronization, concurrency, stale-descriptor, rollback, termination, and
    recovery tests passed.
  - `pnpm --dir ui/desktop run lint:check` passed with the pinned Hermit pnpm/Node
    toolchain.
  - All 15 translated catalogs validated against 1,032 source messages.
- Reviews:
  - production core: Codex GPT-5.6 Sol, pass with no findings;
    `/tmp/tagteam-gosling-stage3-core-state-v5/.../2026-07-11T024658.446249000Z`.
  - test harness: Agy Gemini 3.5 Flash (Medium), pass with no findings;
    `/tmp/tagteam-gosling-stage3-tests-state/.../2026-07-11T025223.802186000Z`.

### Stage 8 corrective audit — diagnostics viewer search

Status: completed in commit `2629cdd91`.

- Re-review found incorrect Unicode offsets, stale widget state after closing/reopening
  search, and a depth-limited JSON search. Subsequent reviews also found unbounded eager
  rendering, accumulating reveal nodes, silent search truncation, incorrect JSON `null`
  matching, Rich-markup injection, and loss of full-value inspection for omitted long
  strings.
- The corrected viewer bounds initial rendering, searches parsed JSON iteratively,
  reports capped/partial results, reuses one labeled proxy for omitted matches, renders
  diagnostic content as literal `Text`, and preserves the full-string modal contract.
- Validation: 11 mounted Textual tests, `ruff check`, Python compilation, and
  `git diff --check` passed.
- Final review: Codex GPT-5.6 Sol, pass with no findings;
  `/tmp/tagteam-gosling-stage8-final-state-v7/.../2026-07-11T032243.053773000Z`.

### Externally landed commit audit

Status: completed for the late commits that landed outside this continuation.

- `876526817` (provider-utils helpers): inspected the fail-closed Google error-body
  behavior and retry clamp; all 12 focused provider-utils Rust tests passed.
- `251cebf7e` and `300d11db3` (desktop CI assertions): inspected the runtime default,
  explicit profile-save flow, and compacted ACP load metadata. The three focused Vitest
  files passed with 16 tests.
- Earlier campaign commits for schema generation, provider contracts, live-provider
  prerequisites, Docker capacity, and Pages gating were separately reviewed against
  their focused regressions; no additional landed-code defect remained in those stages.

### Stage 7 and Stage 9 routing revalidation

Status: both routing decisions independently confirmed; no source edits made.

- Stage 7 used supervisor-worker and automatically transitioned to relay when both roles
  requested broader context. The Agy worker hit the five-minute watchdog and was
  replaced by `gpt-5.6-terra`; the no-edit result passed `acp_server_test` (46 active
  tests) and supervisor review. A correct fix still requires an immutable per-agent
  runtime context and scoped request logger across global configuration/path consumers.
  Artifact: `/tmp/tagteam-gosling-stage7-revalidation/.../2026-07-11T032715.882260000Z`.
- Stage 9 used relay with local Gemma reconnaissance, `gpt-5.6-terra` coding, and
  `gpt-5.6-sol` supervision. It confirmed that native ACP identity is not persisted and
  activation always creates a new native session; fixture-only loading would be false
  support. The four load-session tests remain explicitly ignored and were not counted
  as validation. Artifact:
  `/tmp/tagteam-gosling-stage9-revalidation/.../2026-07-11T035058.602143000Z`.

### PR #22 CI correction

Status: implemented in commit `760e801c4`; hosted rerun pending.

- GitHub's first PR #22 run exposed two baseline failures: the MCP smoke script invoked
  its default Anthropic model without `ANTHROPIC_API_KEY`, and the ACP code-mode fixture
  relied on the runtime's obsolete enabled-by-default behavior.
- The workflow now visibly skips only the Anthropic-backed MCP step when its credential
  is absent, while the script fails immediately with a clear prerequisite error if run
  directly. Code-mode fixtures now declare the runtime state explicitly and grant the
  successful discovery flow one-time permission.
- Validation:
  - ACP provider code-mode suite: 13 active tests passed, 9 intentional ignores.
  - ACP server code-mode suite: 47 active tests passed, 1 intentional ignore.
  - MCP script syntax and missing-key preflight passed; workflow YAML parsed.
  - Codex GPT-5.6 Sol adversarial review passed with no findings;
    `/tmp/tagteam-gosling-pr22-ci-fix-state/.../2026-07-11T034929.879693000Z`.

## Post-merge hardening - 2026-07-11

Status: local validation complete; protected-branch PR and hosted rerun pending.

### Hosted `main` audit and independently landed changes

- PR #22 merged as `bac79d107`. Its `main` push passed Live Provider Tests,
  Scorecard, Unused Dependencies, and release-PR creation. CI failed six
  `gosling-providers` expectations because the generated Anthropic catalog had stale
  contracts. Canary failed before creating jobs because its reusable Windows workflow
  requested `id-token: write` while the caller supplied only `contents: read`.
- The Canary regression came from independently landed commit `9d6fee12f`. GitHub's
  reusable-workflow permission contract permits permissions to be maintained or reduced
  through a call chain, not elevated by the called workflow. The repair grants OIDC only
  at Windows signing callers and jobs; macOS workflows do not receive OIDC because they
  have no OIDC consumer. The phase-2 verifier now enforces that split.
- Independently landed commits `876526817`, `251cebf7e`, and `300d11db3` were re-read
  and their focused tests passed (12 provider-utils tests and 16 desktop tests).
  `239faeb45` and `41903261a` are report-only. No additional source defect was found in
  that late batch.
- The `bac79d107` Docker publish run remained in progress during this audit; it is an
  external hosted gate rather than local validation evidence.

### Anthropic canonical-model contracts

- Added one curated-contract application point shared by registry loading and catalog
  generation. It corrects Sonnet 4.5 to a 200k context, marks Sonnet 5 adaptive
  thinking, and preserves the official 1M contexts for current Opus, Sonnet, and Fable
  models.
- Retired Claude 3.5, Claude 3.7, and Sonnet 4 identifiers remain resolvable through a
  non-enumerated compatibility map. They cannot enter active listings,
  recommendations, or generated JSON, including when a live provider inventory returns
  only retired models.
- Tagteam found and drove fixes for two defects: compatibility-only live inventory
  leaking into recommendations, and future regenerated retired models remaining active
  beside compatibility entries. Final supervisor review passed with no findings:
  `/private/tmp/tagteam-gosling-postmerge-hardening-state-v3/.../2026-07-11T044311.698784000Z`.

### Provider mode propagation

- The full workspace run exposed a repeatable Codex MCP image cancellation. The Codex
  rollout showed that `get_code` succeeded but `get_image` was rejected before fixture
  execution because the newly installed provider had never received the agent's `Auto`
  mode and fell back to `SmartApprove`.
- `Agent::update_provider` now propagates the active mode before installing a provider;
  persisted-session restoration installs with the persisted mode directly. This
  centralizes the invariant for CLI model changes, ACP/session restoration,
  orchestrator-created agents, subagents, doctor/configure flows, and default-provider
  fallback. A focused regression records and verifies the mode delivered at install.
- The focused live Codex provider test now passes model listing, basic response, text
  MCP tool use, and image MCP tool use with Codex CLI 0.144.1.
- Final adversarial review with Codex GPT-5.6 Terra passed with no findings:
  `/private/tmp/tagteam-gosling-postmerge-mode-audit/.../2026-07-11T050331.860385000Z`.

### Final validation

- `GOSLING_DISABLE_KEYRING=1 cargo test -- --skip scenario_tests::scenarios::tests`
  passed the complete workspace suite, including 1,459 Gosling core tests, 414
  provider-library tests, all integration/doc tests, and live `codex` and
  `claude-code` provider tests. Other live providers skipped because their credentials
  were absent.
- `GOSLING_DISABLE_KEYRING=1 cargo test --jobs 1
  scenario_tests::scenarios::tests` passed all three scenarios.
- `GOSLING_DISABLE_KEYRING=1 cargo clippy --workspace --all-targets -- -D warnings`,
  `cargo fmt --all -- --check`, `git diff --check`, the phase-2 permissions verifier,
  and YAML parsing for all changed workflows passed.
- Local full-feature tests use `GOSLING_DISABLE_KEYRING=1` because an unattended macOS
  Keychain read can wait for UI indefinitely. Hosted Linux CI starts and unlocks its
  keyring daemon, so the hosted path remains covered rather than disabled.
- Stage 7 per-agent global-state isolation and Stage 9 native ACP session loading remain
  routed to source modularization. This post-merge work does not change those decisions.

## Canary bootstrap follow-up - 2026-07-11

Status: repair committed as `633e7dfb4`; protected-branch PR and hosted rerun pending.

- PR #23 merged as `d8282d687` after every PR check passed. Local `main` was
  fast-forwarded to the merge, the remote repair branch was deleted, and only `main`
  remained locally and remotely with no auxiliary worktrees.
- The merge-triggered Canary proved the reusable-workflow permission repair: unlike the
  prior `startup_failure`, its Windows, macOS, Linux, and CLI jobs all started. It then
  exposed a separate manylinux bootstrap defect introduced by independently landed
  commit `2db5ae1d47`.
- Both GNU CLI jobs downloaded and checksum-verified rustup 1.29.0, then failed with
  `unknown proxy name: 'tmp'`. The workflow had saved `rustup-init` to an arbitrary
  `mktemp` filename, but rustup dispatches by its executable basename. The repair uses
  a temporary directory with a `rustup-init` child path and retains cleanup on success
  and failure.
- The changed workflow parses as YAML, the phase-2 permission verifier passes, and
  `git diff --check` passes. Codex GPT-5.6 Terra adversarial review found no issues and
  specifically confirmed both cleanup paths:
  `/private/tmp/tagteam-gosling-rustup-bootstrap-review/.../2026-07-11T054955.300435000Z`.
- The widened independent-commit audit also found that the phase-3 container verifier
  from `223e5d0302` used Ruby 2.7's `Array#filter_map` despite the repository declaring
  no newer Ruby runtime. It crashed under the macOS system Ruby 2.6 before checking any
  contract. Replacing it with behavior-equivalent `each_with_object` made all phase 1,
  phase 2, and phase 3 verifiers pass locally. GPT-5.6 Sol supervisor review passed with
  no findings:
  `/private/tmp/tagteam-gosling-ruby26-integrity-review/.../2026-07-11T055656.210901000Z`.
- The same Canary run exposed a latent feature-boundary defect in both musl CLI jobs:
  `portable-default` intentionally excludes Nostr, but session import referenced the
  feature-gated `nostr_share` module before entering its `#[cfg]` branch. Deeplink
  classification now lives on the always-built session surface, while the Nostr module
  keeps its existing public helper as a delegate. The exact portable CLI check and
  clippy configuration pass, as do focused no-Nostr and Nostr-enabled tests and
  Nostr-enabled all-target clippy. Relay-mode GPT-5.6 Sol review passed with no findings:
  `/private/tmp/tagteam-gosling-portable-nostr-review/.../2026-07-11T060832.941655000Z`.
