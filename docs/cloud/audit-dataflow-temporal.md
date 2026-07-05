# Gosling Audit — Temporal & Lifecycle Lens (audit-dataflow-temporal)

Lens domain: **Temporal** (prefix `TMP`). Authority: **audit-only / read-only**.
Builds on `docs/cloud/00-orientation.md`. Focus per tasking: token/credential
expiry & refresh, migration sequencing, stale-artifact reuse, replay/ordering,
cache TTLs, clock assumptions, session freshness, stale model/provider catalog.

## 1. Scope actually reviewed

Files read in full or in the load-bearing region:

- `crates/gosling/src/oauth/mod.rs`, `crates/gosling/src/oauth/persist.rs` (MCP OAuth)
- `crates/gosling/src/providers/oauth.rs` (Databricks OIDC token cache)
- `crates/gosling/src/providers/gemini_oauth.rs`
- `crates/gosling/src/providers/xai_oauth.rs`
- `crates/gosling/src/providers/chatgpt_codex.rs`
- `crates/gosling/src/providers/gcpauth.rs`
- `crates/gosling/src/providers/azureauth.rs`
- `crates/gosling/src/providers/githubcopilot.rs` (token region)
- `crates/gosling/src/providers/databricks_v2.rs`, `databricks_auth.rs`
- `crates/gosling/src/providers/catalog_util.rs`, `inventory/resolver.rs`
- `crates/gosling/src/config/migrations.rs`
- `crates/gosling/src/session/session_manager.rs` (migration runner), `session/legacy.rs`

Grep sweeps: `expires|expiry|expires_at|expires_in|expired|refresh` across
`providers/`; `version|schema|migrat|updated_at|mtime` across `session/`.

## 2. Temporal inventory (artifact → consume → freshness → cleanup)

| Artifact / authority | Created | Consumed at | Freshness / expiry check at consume | Verdict |
|---|---|---|---|---|
| Gemini OAuth token cache | `gemini_oauth/tokens.json` | `get_valid_setup` | `expires_at > now + 60s`, else refresh | safe |
| xAI OAuth token cache | `xai_oauth/tokens.json` | `get_valid_token` | `expires_at > now + 120s`, single-flight refresh | safe |
| ChatGPT Codex token cache | `chatgpt_codex/tokens.json` | `get_valid_token` | `expires_at > now + 60s`, else refresh | safe |
| Databricks OIDC token | `databricks/oauth/*.json` | `get_oauth_token_async` (per request) | `expires_at > Utc::now()`, else refresh | safe |
| GCP token | in-mem `CachedToken` | `get_token` | `expires_at(Instant) > Instant::now()`, 30s buffer, DCL | safe |
| Azure AD (DefaultCredential) | in-mem `CachedToken` | `get_default_credential_token` | `expires_at(Instant)`, 30s buffer, DCL | safe |
| Azure pre-acquired AD token | env `AZURE_OPENAI_AD_TOKEN` | `get_token` | **none** — returned verbatim | TMP-GSL-003 |
| Copilot API token | `githubcopilot/info.json` | `get_api_info` | `expires_at(= now+refresh_in) > Utc::now()` | safe |
| ChatGPT Codex JWKS | in-mem `jwks_cache` | `parse_jwt_claims` | **never invalidated**; falls back to unverified | TMP-GSL-001 |
| Databricks OIDC token w/o `expires_in` | disk cache | `get_oauth_token_async` | **skipped** ("using without expiration check") | TMP-GSL-002 |
| Provider config layout | `config.yaml` | `run_migrations` | convergent/idempotent, detection-based | safe |
| Session DB schema | SQLite `schema_version` | `run_migrations` | transactional, sequential, version-tracked | safe |
| Model known-lists | compile-time `const` arrays | catalog / setup UI | rebuilt only on binary rebuild | TMP-GSL-004 |

## 3. Findings

### TMP-GSL-001: Cached JWKS never refreshed; JWT verification silently downgrades to unverified parse on key rotation

Severity: Low
Confidence: Likely
Evidence basis: source-evidenced
Domain: Temporal

Evidence:
- `crates/gosling/src/providers/chatgpt_codex.rs:410-418` — `get_jwks` caches the
  first `JwkSet` in `state.jwks_cache` and returns it forever; there is no TTL,
  `kid`-miss invalidation, or re-fetch.
- `crates/gosling/src/providers/chatgpt_codex.rs:448-455` — `parse_jwt_claims`
  tries the cached JWKS and, on any failure, falls through to
  `parse_jwt_claims_unverified` (line 437-446), which base64-decodes the JWT
  payload with **no signature check**.

Observed behavior:
- The OpenAI/Codex signing keys are fetched once per process and cached
  indefinitely. When OpenAI rotates keys (new `kid`), `jwks.find(&kid)` returns
  `None`, verification fails, and the code silently parses the token unverified
  to extract `chatgpt_account_id`.

Expected boundary:
- A signing-key set is a time-bounded artifact; on a `kid` miss the consumer
  should re-fetch the JWKS before trusting (or rejecting) the token, not fall
  back to an unauthenticated parse.

Failure mechanism:
- Cache has no invalidation keyed on `kid`; the unverified fallback exists for
  robustness but masks the staleness instead of triggering a refresh.

Break-it angle:
- Rotate the id_token signing key mid-process: every subsequent verify misses
  the cache and the account id is taken from an unverified token.

Impact:
- The value extracted unverified is only `chatgpt_account_id`, sent as the
  `chatgpt-account-id` header; the access token itself is still validated by the
  upstream API, so this is not an auth bypass. Blast radius is a possibly-wrong
  account header, not privilege escalation. Hence Low.

Operational impact:
- Blast radius: Workflow
- Side-effect class: network (outbound header)
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- TMP-GSL-004 (stale compile-time catalogs) — same "cached-forever" shape.

Recommended mitigation:
- On `kid`-miss, re-fetch JWKS once and retry verification; only then consider a
  fallback. Add a bounded TTL to `jwks_cache`.
- Behavior test: seed cache with an old key set, present a token signed by a new
  `kid`, assert a re-fetch occurs and unverified parse is not used when a
  verified path is available.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: single function, well-isolated, one new test.

Validation:
- Test asserts JWKS re-fetch on `kid` miss and that verified parse wins.

Non-goals:
- Do not remove the unverified fallback entirely (needed when JWKS endpoint is
  unreachable); gate it behind a refresh attempt.

### TMP-GSL-002: Databricks OIDC token with no recorded expiry is consumed without any freshness check

Severity: Low
Confidence: Plausible
Evidence basis: source-evidenced
Domain: Temporal

Evidence:
- `crates/gosling/src/providers/oauth.rs:410-416` — in `get_oauth_token_async`,
  when `token.expires_at` is `None` the cached access token is returned directly:
  `"Token has no expiration time, using it without expiration check" ... return Ok(token.access_token)`.
- `crates/gosling/src/providers/oauth.rs:201-212` — `expires_at` is only set when
  the server returns `expires_in`; otherwise it is `None` and persisted that way.

Observed behavior:
- If a token response ever omits `expires_in`, gosling caches the access token
  with `expires_at = None` and thereafter serves it on every request with no
  time check, relying entirely on the eventual server-side 401 + refresh.

Expected boundary:
- A credential with unknown lifetime should be treated as short-lived (proactive
  refresh) rather than assumed valid until the server rejects it.

Failure mechanism:
- The no-`expires_in` branch trades a freshness guard for a reactive 401 path;
  whether that 401 actually triggers a refresh depends on the caller's retry
  wiring, which was not traced here.

Break-it angle:
- A provider that returns `refresh_token` but omits `expires_in` yields a token
  that is used past its real server expiry until a request fails.

Impact:
- One failed request per expiry cycle at worst; refresh-token path still exists.
  Databricks in practice returns `expires_in`, so this is an edge branch — Low /
  Plausible.

Operational impact:
- Blast radius: Workflow
- Side-effect class: network
- Reversibility: reversible
- Operator visibility: log-only (`tracing::debug`)
- Rerun safety: safe

Adjacent failure modes:
- TMP-GSL-003 (pre-acquired token, no expiry check).

Recommended mitigation:
- When `expires_in` is absent, set a conservative default lifetime (e.g. treat as
  already-stale so the refresh path runs), matching the other providers'
  `unwrap_or(3600)` convention used at write time elsewhere.
- Test: cache a token with `expires_at = None` + a refresh token; assert the
  refresh path is taken rather than the raw token being returned.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: one branch, one test.

Validation:
- Test asserts no-expiry cached token is refreshed, not blindly served.

Non-goals:
- Do not change the Databricks OIDC discovery flow.

### TMP-GSL-003: Pre-acquired Azure Entra token accepted verbatim with no expiry validation

Severity: Low
Confidence: Likely
Evidence basis: source-evidenced
Domain: Temporal

Evidence:
- `crates/gosling/src/providers/azureauth.rs:112-115` — `AzureCredentials::BearerToken`
  is returned directly as `AuthToken` with no decode of `exp` and no expiry check.
- `crates/gosling/src/providers/azureauth.rs:75-80` — the token originates from the
  `AZURE_OPENAI_AD_TOKEN` env/config value.

Observed behavior:
- A user-supplied pre-acquired AD token is used as-is on every request; if it has
  expired, gosling still sends it and only learns of expiry from the API 401.

Expected boundary:
- Even pre-acquired bearer tokens carry an `exp`; a temporal-safe consumer would
  reject/warn on an already-expired token before use.

Failure mechanism:
- The `BearerToken` variant is explicitly a "trust the operator" path with no
  self-managed lifecycle, unlike `DefaultCredential` which caches with a 30s buffer.

Break-it angle:
- Configure an expired `AZURE_OPENAI_AD_TOKEN`; the first request fails at the API
  rather than at a local guard.

Impact:
- This is a documented "bring-your-own-token" design; the operator owns the
  lifetime. Failure is a clean 401, not corruption. Low; arguably accepted risk.

Operational impact:
- Blast radius: Workflow
- Side-effect class: network
- Reversibility: reversible
- Operator visibility: log-only (upstream error surfaces)
- Rerun safety: safe

Recommended mitigation:
- Optionally decode `exp` and emit a warning (or fail closed) when the supplied
  token is already expired. Report-only; may be intentionally left as-is.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: modules
- Nominal implementation agent: human-owner
- Rationale: product decision whether to validate an operator-supplied token.

Validation:
- Test: expired `BearerToken` produces a warning/error before the HTTP call.

Non-goals:
- Do not add refresh logic for a pre-acquired token (no refresh material exists).

### TMP-GSL-004: Model/provider "known model" catalogs are compile-time constants (stale until rebuild)

Severity: Low
Confidence: Likely
Evidence basis: source-evidenced
Domain: Temporal

Evidence:
- `crates/gosling/src/providers/gemini_oauth.rs:95-103` — `GEMINI_OAUTH_KNOWN_MODELS`
  hardcoded; `fetch_supported_models` (970-975) returns the same constants.
- `crates/gosling/src/providers/chatgpt_codex.rs:57-70` — `CHATGPT_CODEX_KNOWN_MODELS`
  hardcoded; `fetch_supported_models` (1059-1061) returns the constants.

Observed behavior:
- For OAuth/subscription providers, the advertised model list and per-model
  reasoning levels are fixed at build time; upstream additions/removals/renames
  are not reflected until a new gosling binary ships.

Expected boundary:
- A catalog presented to the operator as "supported models" should either be
  fetched live or clearly stamped as a starter/awareness list, not treated as the
  current authoritative catalog.

Failure mechanism:
- No freshness source; the constant is the catalog. This is a deliberate
  trade-off (these providers have no clean model-list endpoint), but it is a
  mixed-era hazard: a renamed/retired model stays selectable and fails only at
  request time.

Break-it angle:
- Upstream retires a model in the list; the UI still offers it; requests 4xx.

Impact:
- Selection of a dead model → clean request failure, not corruption. Low.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible (model picker)
- Reversibility: reversible
- Operator visibility: UI-visible (eventual error)
- Rerun safety: safe

Recommended mitigation:
- Where an upstream model endpoint exists, prefer live fetch with the constant as
  fallback; otherwise document the list as a starter set. Report-only.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: modules, tests, runtime_verification
- Nominal implementation agent: claude
- Rationale: touches multiple providers and UI expectations.

Validation:
- Test: catalog surface labels hardcoded lists as non-authoritative, or live
  fetch supersedes the constant when available.

Non-goals:
- Do not attempt live fetch for providers with no list endpoint.

## 4. Non-findings (seams checked and held)

- **Gemini OAuth expiry** — `gemini_oauth.rs:742` checks `expires_at > now + 60s`
  before returning the cached token; refresh on miss, clear-and-reauth on refresh
  failure. Held.
- **xAI OAuth expiry + rotating-refresh replay** — `xai_oauth.rs:611-651`:
  120s skew check, then a `refresh_mutex` single-flight with a re-load
  double-check (622-629) so concurrent callers do not replay a rotating
  `refresh_token`. TOCTOU/replay window closed. Held.
- **ChatGPT Codex expiry** — `chatgpt_codex.rs:842-877` checks `expires_at > now + 60s`,
  refresh then reauth. Held (JWKS is the separate TMP-GSL-001).
- **GCP token** — `gcpauth.rs:377-422` monotonic `Instant` expiry with 30s buffer
  and double-checked locking; `refresh_credentials` clears the cache. Held.
- **Azure DefaultCredential** — `azureauth.rs:120-179` monotonic `Instant` expiry,
  30s buffer, DCL. Held (BearerToken variant is TMP-GSL-003).
- **Copilot token** — `githubcopilot.rs:334,343,358` refreshes at
  `now + refresh_in` (always earlier than the server `expires_at`), checked before
  every consume. Held.
- **Databricks OAuth per-request freshness** — `databricks_auth.rs:61-66` calls
  `get_oauth_token_async` on every `get_auth_header`, which re-checks disk-cache
  expiry (`oauth.rs:404-408`). No indefinite in-memory reuse for the OAuth path.
  Held (edge branch is TMP-GSL-002).
- **Config migrations idempotent/convergent** — `config/migrations.rs:11-16`,
  `315-321` (idempotent test), `438-454`: migrations detect the target state and
  no-op if already applied; no version-pin needed because they are convergent, not
  incremental. Re-running is safe. Held (TMP-006).
- **Session DB migrations sequential & transactional** — `session_manager.rs:1014-1037`:
  `BEGIN IMMEDIATE`, loop `current+1..=CURRENT_SCHEMA_VERSION`, per-step
  `update_schema_version`, single commit. `get_schema_version` (1039-1060) reads
  `MAX(version)`; the "schema exists but no version row" case is handled at
  776-787 to avoid stamping an old DB as current and skipping migrations. Ordered,
  once-only, replay-safe. Held (TMP-006).
- **OAuth CSRF state / replay** — `chatgpt_codex.rs:718-724`, `xai_oauth.rs:467-473`,
  `gemini_oauth.rs:612-618` all reject callback `state` mismatches before accepting
  the code. Held.
- **OAuth callback timeout** — `oauth/mod.rs:60-83`, `chatgpt_codex.rs:775-781`:
  bounded waits; a stale/never-arriving callback fails closed. Held.

## 5. Validation limits (NOT reviewed / not provable here)

- **MCP OAuth per-request refresh** — `oauth/mod.rs:93-104` refreshes once at
  `AuthorizationManager` setup and then hands the manager to the caller. Whether
  `rmcp`'s `AuthorizationManager` re-validates/re-refreshes token expiry before
  each MCP call is **inside the `rmcp` crate and was not traced**. If it does not,
  a long-lived MCP session could present an expired token (TMP-003) — cannot
  confirm from this repo. Requires reading `rmcp::transport::auth`.
- **ACP subprocess providers** (`claude_code.rs`, `codex.rs`, `gemini_cli.rs`,
  `copilot_acp.rs`, etc.) that may read *external* CLI credential files
  (`~/.codex/auth.json`, etc.) were **not reviewed**; those are a prime
  stale-artifact/expired-authority surface (reading another tool's token cache
  without checking its expiry). Recommend a follow-up pass.
- **Session import / provenance** — `session/import_formats/`, `nostr_share.rs`,
  `session/legacy.rs` (mixed-era / imported-session provenance, TMP-009/012) were
  only glanced at; not audited for era stamping.
- **context_mgmt** truncation/summarization era-mixing (TMP-009) not reviewed.
- **Clock model** — every provider expiry check uses wall-clock `Utc::now()`
  (except GCP/Azure DefaultCredential, which correctly use monotonic `Instant`).
  A backward wall-clock jump could extend perceived token validity for the
  wall-clock providers. Not exploited/observed; noted as a clock assumption.
- No runtime execution: all evidence is `source-evidenced` / `simulation-reasoned`.

## 6. Summary for the audit lead

The credential-lifecycle core is in good shape: every self-managed provider token
cache checks expiry (with a refresh skew) against a clock before use, refresh is
single-flighted where the refresh token rotates (xAI), and both migration systems
(config YAML convergent; session SQLite sequential+transactional+version-tracked)
are replay-safe. No Confirmed high/critical temporal defect was found in the
reviewed surface.

Four Low findings, all hardening: (1) a never-invalidated JWKS cache in
`chatgpt_codex` that silently downgrades to *unverified* JWT parsing on key
rotation; (2) a Databricks edge branch that serves a no-recorded-expiry token
without any freshness check; (3) pre-acquired Azure AD tokens accepted verbatim;
(4) compile-time model catalogs that go stale until rebuild.

Biggest open risk is a **validation limit, not a finding**: whether the `rmcp`
`AuthorizationManager` re-refreshes MCP OAuth tokens per request, and the
**unreviewed ACP subprocess providers** that may consume external CLI credential
files without an expiry check — both warrant a targeted follow-up.
