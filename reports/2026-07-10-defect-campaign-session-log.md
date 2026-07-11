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

Status: completed.

- Change: added the public `gpt-5.3-codex` model to the ChatGPT Codex capability
  table with a 400,000-token context window and `xhigh` reasoning support. Updated
  two stale Codex CLI context-limit assertions to the metadata actually returned by
  the provider registry, and updated the Google-status test to preserve a successful
  transport status for an unmapped payload error code.
- Contract evidence: OpenAI's GPT-5.3-Codex model documentation lists a 400,000-token
  context window and `low`, `medium`, `high`, and `xhigh` reasoning effort. The
  undocumented 5.4–5.6 aliases were not changed.
- Validation:
  - `cargo test -p gosling --lib test_create_codex_request_reasoning_effort_from_unified_thinking` — passed.
  - `cargo test -p gosling --lib test_known_model_context_limits` — passed.
  - `cargo test -p gosling --lib test_get_google_final_status_with_error_code` — passed.
  - `cargo test -p gosling --lib test_model_transport_and_context_limits` — passed.
  - `cargo clippy -p gosling --all-targets --features code-mode,aws-providers,telemetry,otel,rustls-tls,system-keyring -- -D warnings` — passed.
  - `git diff --check` — passed.
- Adversarial review: unknown model fallback remains unchanged; only known public
  GPT-5.3-Codex receives the expanded effort/context contract; unmapped Google
  payload codes no longer fabricate a 500 response.
- Change review: scoped to CI-001 and this session log only.

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
