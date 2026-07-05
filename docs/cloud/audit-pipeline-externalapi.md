# External API Pipeline Audit — LLM Providers & Remote Boundaries

Lens: `audit-pipeline-externalapi` · Domain prefix `EXT-GSL`
Authority: **audit-only / read-only**. No source modified; only this report written.
Repo state: branch `claude/gosling-stress-test-audit-jwhooa`, commit `8f39f5d`.
Builds on `docs/cloud/00-orientation.md` (surface #6: provider request/response pipeline).

## 1. Orientation & constraints

The external APIs in scope are the **LLM provider integrations** and the shared
request/response/retry plumbing that fronts them. gosling has two provider
layers:

- `crates/gosling-providers/src/` — the **shared plumbing** and the direct-HTTP
  provider families: `api_client.rs` (reqwest wrapper), `retry.rs`,
  `http_status.rs` (status→error mapping + `Retry-After`), `errors.rs`
  (`ProviderError` taxonomy), `base.rs` (`Provider` trait, `collect_stream`),
  `anthropic.rs`, `openai.rs`, `openai_compatible.rs`, `ollama.rs`, and the
  streaming decoders in `formats/`.
- `crates/gosling/src/providers/` — provider *bindings* that add auth: `bedrock.rs`
  (AWS SDK), `azure.rs`/`azureauth.rs`, `gcpauth.rs`/`gcpvertexai.rs`,
  `databricks*`, `githubcopilot.rs`, the `*_oauth.rs` flows, and the **ACP**
  subprocess providers (`claude_acp.rs`, `codex_acp.rs`, …).

Sampled this pass (per the effort budget): shared plumbing in full
(`api_client`, `retry`, `http_status`, `errors`, `base`, `json`), the OpenAI-
compatible + Anthropic HTTP paths, the OpenAI streaming decoder
(`formats/openai.rs`), Bedrock (`bedrock.rs`), Azure auth (`azureauth.rs`), GCP
auth (`gcpauth.rs`), and one ACP binding (`claude_acp.rs`).

Hard constraints obeyed: no live provider calls; provider-behavior claims cite
in-repo tests/fixtures or documented library defaults, not memory; outage/hang
manifestations are `simulation-reasoned` and capped at `Likely` per
`confidence_calibration.md`.

## 2. Integration inventory

| Provider / boundary | Endpoints | Auth + scope | Call classes | Failure policy (timeout / retry / breaker) | Idempotency | Test strategy | Blast radius on outage |
|---|---|---|---|---|---|---|---|
| Anthropic (direct) `anthropic.rs` | `POST v1/messages` (stream), `GET v1/models` | `x-api-key` header → configured `ANTHROPIC_HOST` | streaming completion (P1), idempotent read | 600s total / default 3 retries incl. 4xx / no breaker | none (LLM POST) | wiremock unit tests (`anthropic.rs` tests) | agent turn stalls/errors; caught by agent loop |
| OpenAI-compatible `openai_compatible.rs` (openai, xai, openrouter, litellm, azure, nanogpt, tetrate, custom) | `POST {prefix}chat/completions` (stream), `GET models` | Bearer/ApiKey → configured base URL | streaming completion (P1) | 600s total / default 3 retries incl. 4xx / no breaker | none | `test_case` status-map + wiremock | same |
| AWS Bedrock `bedrock.rs` | SDK `Converse`/`ConverseStream`; `POST bedrock-mantle.{region}.api.aws/openai/v1/responses` for `openai.gpt-*` | AWS SigV4 (SDK creds) OR `AWS_BEARER_TOKEN_BEDROCK`; region-scoped host | streaming completion (P1) | **SDK retry + provider 6-retry (nested); mantle client has NO timeout** | none | `not_observable` live; unit fmt tests | same + amplified request volume |
| Azure OpenAI `azure*.rs` | via openai-compatible + `azureauth.rs` token | ApiKey / Entra bearer / `az` CLI token scoped to `cognitiveservices.azure.com` | completion (P1) + token fetch (P1) | 600s inference / token via `az` subprocess | n/a | 3 unit tests | inference stalls; token-fetch failure blocks |
| GCP Vertex `gcpvertexai.rs` + `gcpauth.rs` | Vertex generate; OAuth token / **metadata server** | ADC / metadata / SA; **token clients have NO timeout** | completion (P1) + token fetch (P1) | 600s inference / no-timeout auth | n/a | `not_observable` | inference + auth hang |
| OAuth flows `oauth.rs`, `*_oauth.rs` | provider token endpoints, device flow | OAuth device/callback; **token clients `reqwest::Client::new()` no timeout** | non-idempotent token exchange (P1) | no timeout / — | n/a | partial | auth hang blocks provider init |
| ACP subprocess providers `*_acp.rs` | local subprocess speaking ACP JSON-RPC | inherits child CLI's own auth (Claude/Codex/Gemini subscription) | streaming session (P1), subprocess | subprocess lifecycle (out of this lens; see below) | n/a | `not_observable` | child crash = session loss |

Risk levels: all model-inference paths are **P1** (model/tool execution, metered).
Token-fetch paths are **P1** (block the P1 path). Model-listing (`GET models`)
is **P2** (read, with static fallback).

## 3. Call-class map & policy diff

Every inference call is a **long-running / streaming** class. Per the decision
matrix (`resilience_policy.md`): required = bounded connect + **idle/read**
timeout (not total), retries only via provider resume mechanism, no idempotency
key. The code uses one **total** 600s reqwest timeout (`api_client.rs:17`,
`DEFAULT_PROVIDER_TIMEOUT_SECS`) and re-POSTs the whole request inside
`with_retry` on connection/status failure. Divergences below become
EXT-GSL-001..004.

## 4. Findings

### EXT-GSL-001: Default retry policy retries non-retryable 4xx (400/404)

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Reliability (XAPI-006 / XAPI-008)

Evidence:
- `crates/gosling-providers/src/retry.rs:99-108` — `should_retry`:
  `ProviderError::RequestFailed(_) => !config.transient_only`.
- `crates/gosling-providers/src/retry.rs:28-38` — `Default for RetryConfig` sets
  `transient_only: false`.
- `crates/gosling-providers/src/base.rs:440-442` — `Provider::retry_config`
  default returns `RetryConfig::default()`; Anthropic and OpenAI-compatible
  providers do not override it (`anthropic.rs:305-336`,
  `openai_compatible.rs:122-133` call `self.with_retry(...)`).
- `crates/gosling-providers/src/http_status.rs:206-236` — `404` and generic
  `400` map to `ProviderError::RequestFailed`.
- Test pinning the behavior: `retry.rs:266-271`
  `default_config_retries_request_failed` asserts a 400 IS retried.
- Contrast: `ollama.rs:400` calls `.transient_only()` — the codebase knows the
  correct policy but does not apply it by default.

Observed behavior:
- A deterministic `400 Bad request` or `404` is retried up to 3× with backoff
  before surfacing, for every direct-HTTP provider using the default config.

Expected boundary:
- Never retry 400/401/403/404/422 (contract_failure_matrix). Only
  429/5xx/network/timeout are retryable.

Failure mechanism:
- The single `RequestFailed` bucket collapses "provider server hiccup" and
  "client sent a bad request"; the default treats both as retryable.

Break-it angle:
- Fake server returning 400 with a fixed body: adapter issues 4 identical POSTs
  before failing, adding ~7s (1+2+4s backoff) latency to a permanent error.

Impact:
- Wasted latency and redundant load on a deterministically-failing request. LLM
  400s are rejected pre-generation (no token billing), so this is
  cost-neutral but delays honest failure and violates the retry contract; a
  provider that *does* have a side effect on a 4xx path would double it.

Operational impact:
- Blast radius: Workflow. Side-effect class: network. Reversibility: reversible.
  Operator visibility: log-only (`tracing::warn!` per attempt). Rerun safety: safe.

Adjacent failure modes:
- Amplifies EXT-GSL-002 on Bedrock (6 outer retries of a permanent 4xx).

Recommended mitigation:
- Minimal repair: make `transient_only` the default, or special-case
  `RequestFailed` derived from a non-retryable status. Preserve the existing
  `PERMANENT_REQUEST_FAILURE_MARKERS` carve-out.
- Behavior test: fake server returns 400 once; assert exactly one request on its
  log and no retry sleep.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: modules, tests.
  Nominal agent: codex.
- Rationale: one predicate + config default; wiremock harness already present.

Validation:
- Assert request count == 1 for 400/404 on the fake's log; assert 429/500 still
  retried (existing `transient_only_*` tests cover the positive side).

Non-goals: do not change 429/5xx retry counts here.

---

### EXT-GSL-002: Bedrock nests provider retries over AWS SDK retries (request/cost multiplication)

Severity: High
Confidence: Likely
Evidence basis: simulation-reasoned
Domain: Reliability (XAPI-006 / XAPI-015)

Evidence:
- `crates/gosling/src/providers/bedrock.rs:122-123` — client built from
  `aws_config::defaults(BehaviorVersion::latest())`, which enables the SDK's
  **standard** retry mode (documented default max 3 attempts); no
  `.retry_config(RetryConfig::disabled())` / `retry_config(...)` override is
  present anywhere in the file.
- `bedrock.rs:49` — `BEDROCK_DEFAULT_MAX_RETRIES: usize = 6`.
- `bedrock.rs:511` — `.with_retry(|| self.converse(...))`; `:777` /`:806` —
  `.with_retry(|| self.post_mantle_streaming(...))`; `:724-725` — provider
  `retry_config()` returns the 6-retry config.

Observed behavior:
- A transient Bedrock error is retried up to 6× by gosling, and each of those
  attempts is independently retried by the SDK (~3×), for a worst case on the
  order of ~18 `Converse`/`ConverseStream` invocations per logical completion.

Expected boundary:
- One layer owns retries (`resilience_policy.md` "Retry placement"). Wrapping an
  internally-retrying SDK in another retry loop multiplies calls.

Failure mechanism:
- Two independent retry budgets on the same call path compound multiplicatively.

Break-it angle:
- Point the SDK at a fake endpoint returning 503; count invocations — expect
  ~SDK×provider, not 6. Backoff totals compound (provider max_interval 120s ×
  6) so failure surfaces very slowly.

Impact:
- Cost angle: each attempt is a metered model call. `per-request-cost × ~3 (SDK)
  × 6 (provider) × concurrent turns` — an outage or a persistent 4xx
  (compounded with EXT-GSL-001) becomes a spend and rate-limit-ban amplifier.

Operational impact:
- Blast radius: Cross-system (provider account rate limits). Side-effect class:
  external API. Reversibility: reversible. Operator visibility: log-only.
  Rerun safety: unsafe (amplifies load).

Adjacent failure modes:
- EXT-GSL-001 (4xx retried), EXT-GSL-005 (no spend cap).

Recommended mitigation:
- Disable SDK retries (`RetryConfig::disabled()`) and keep gosling's policy as
  the single owner, or set SDK `max_attempts(1)` on the Bedrock config builder.
- Behavior test: fake Bedrock returns 503; assert total attempts == the
  provider budget only.

Implementation assessment:
- Complexity: external_service_semantics. Cost: S. Cost drivers: modules, tests.
  Nominal agent: codex.
- Rationale: one config-builder line; the multiplication mechanism is
  source-evidenced, only the SDK's exact default count is un-quoted (hence Likely).

Validation:
- Count invocations on a fake endpoint under forced 503; assert single retry budget.

Non-goals: do not restructure the mantle vs Converse split.

---

### EXT-GSL-003: Missing request timeout on auth-token and Bedrock-mantle clients

Severity: Medium
Confidence: Confirmed (missing timeout) / Likely (hang manifestation)
Evidence basis: source-evidenced
Domain: Reliability (XAPI-005)

Evidence — `reqwest::Client::new()` (no timeout; reqwest has **no** default
request timeout) on live-path clients:
- `crates/gosling/src/providers/bedrock.rs:173` (also `:925`, `:1097`) —
  `http_client: reqwest::Client::new()`, used at `bedrock.rs:251-268`
  `post_mantle_streaming` (`req.send().await`) — the **inference path** for
  `openai.gpt-*` Bedrock models.
- `crates/gosling/src/providers/gcpauth.rs:244` `load_from_metadata_server` —
  calls the GCP metadata server (`169.254.169.254` link-local) with no timeout;
  plus `:352,:698,:707,:732,:770,:803,:856,:875`.
- `crates/gosling/src/providers/oauth.rs:109,257,286` and
  `xai_oauth.rs:199,227,273,307` — OAuth token exchange, no timeout.
- Contrast (correct pattern): `api_client.rs:17,248-254` sets 600s;
  `gcpvertexai.rs:176-177`, `kimicode.rs:169-170`, `githubcopilot.rs:287-288`,
  `gemini_oauth.rs:41-42`, `toolshim.rs:525-526`, `chatgpt_codex.rs:385` all set
  `DEFAULT_PROVIDER_TIMEOUT_SECS`.

Observed behavior:
- If the metadata server, an OAuth token endpoint, or the Bedrock mantle host
  accepts the connection but never responds, the awaiting task blocks
  indefinitely (bounded only by OS/keepalive), with no `ProviderError` raised.

Expected boundary:
- Every external call has a bounded connect+read timeout (XAPI-005). A hung
  provider must fail at the configured deadline.

Failure mechanism:
- `reqwest::Client::new()` applies no timeout; these call sites add none.

Break-it angle:
- Stall a fake metadata/token/mantle endpoint (accept, never send): the call
  hangs past any reasonable budget instead of erroring.

Impact:
- Agent startup / provider init / a mantle completion can hang silently. The
  metadata-server case is the sharpest: on a non-GCP host where the link-local
  address is routable-but-silent, auth hangs rather than falling through to the
  "not on GCP" branch (which only runs after a *response*).

Operational impact:
- Blast radius: Service (whole agent stalls). Side-effect class: network.
  Reversibility: reversible (kill). Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- Streaming idle timeout: the main inference clients set a **total** 600s timeout
  (`api_client.rs`), not a per-chunk idle timeout, so a stream that stalls
  mid-response consumes the remaining budget before erroring, and a legitimate
  generation exceeding 600s is cut off. Same file, related class.

Recommended mitigation:
- Replace bare `reqwest::Client::new()` on these paths with a builder that sets
  a connect timeout and a short overall timeout (auth calls are fast); give the
  mantle client the same 600s policy as sibling inference clients.
- Behavior test: fake stalled endpoint; assert the call errors at the deadline.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: modules, tests.
  Nominal agent: codex.

Validation:
- `wiremock`/fake TCP that accepts and never responds; assert timeout error
  within budget for each patched client.

Non-goals: introducing per-chunk idle timeouts for streams (separate slice;
route to `audit-resource-lifecycle`).

---

### EXT-GSL-004: Truncated / prematurely-closed stream committed as a complete message

Severity: Medium
Confidence: Likely
Evidence basis: simulation-reasoned
Domain: Reliability (XAPI-013 / XAPI-017)

Evidence:
- `crates/gosling-providers/src/base.rs:323-372` `collect_stream` — accumulates
  yielded messages; returns `Ok((msg, usage))` whenever the stream ends with at
  least one message. The only error path is `result?` (a yielded `Err`) or the
  empty-stream `None` case. There is **no** check that a terminal marker was
  observed.
- `crates/gosling-providers/src/formats/openai.rs:1016-1022` — the decode loop
  breaks on `[DONE]`, but the outer `while let Some(response) = stream.next()`
  also exits normally on a clean byte-stream EOF (no `[DONE]`), after which the
  trailing flush (`:1310-1318`) yields the accumulated partial text as the final
  message with no error.
- Text truncation is not surfaced: `finish_reason` is only read to decide
  whether to attach usage (`openai.rs:1296-1301`); a `finish_reason == "length"`
  (max_tokens) on **text** content yields the partial text as a normal complete
  message.
- Positive contrast (held): **tool-call** argument truncation *is* caught —
  `json.rs:131-238` `looks_truncated`/`parse_tool_arguments` returns `None` and
  `openai.rs:1222-1233` surfaces an `INVALID_PARAMS` tool error rather than
  invoking the tool with partial args.

Observed behavior:
- A response whose connection closes at a message boundary without `[DONE]`
  (proxy/CDN truncation, provider bug), or a text answer cut at the output-token
  limit, is returned as a successful, complete assistant message.

Expected boundary:
- An interrupted stream's prefix is discarded or explicitly marked partial; a
  finish marker / length check gates "complete" (XAPI-013). Truncation
  (`finish_reason == length`) is surfaced distinctly (XAPI-017).

Failure mechanism:
- `collect_stream` equates "stream ended" with "response complete"; the decoder
  has no "saw terminal event" flag to assert on EOF.

Break-it angle:
- Fake SSE server that sends two content chunks then closes the socket without
  `[DONE]`: `complete()` returns `Ok` with the two-chunk prefix. (Whether reqwest
  surfaces this as clean EOF vs a hyper `Err` depends on transfer-encoding — an
  `Err` is caught by `result?`; a clean close is not — hence Likely, not Confirmed.)

Impact:
- The agent acts on a silently-truncated answer as if it were the model's full
  response — misleading reasoning, half-written content in a text field.

Operational impact:
- Blast radius: Workflow. Side-effect class: user-visible. Reversibility:
  compensatable (re-ask). Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- Intersects EXT-GSL-003 (a stalled stream that then closes).

Recommended mitigation:
- Track whether a terminal event (`[DONE]` / Anthropic `message_stop`) was seen;
  if the byte stream ends without it, yield a `NetworkError`/partial marker
  instead of a clean completion. Surface `finish_reason == length` to the caller
  (the truncation-error machinery in `json.rs` already exists for the tool path).
- Behavior test: fake stream closes without `[DONE]`; assert `complete()` errors
  or marks the message partial.

Implementation assessment:
- Complexity: external_service_semantics. Cost: M. Cost drivers: modules, tests
  (per-format decoder + collect_stream). Nominal agent: codex.

Validation:
- Fake SSE truncation test; assert non-success. `finish_reason=length` fixture;
  assert truncation is signaled.

Non-goals: streaming idle-timeout (EXT-GSL-003 adjacent).

---

### EXT-GSL-005: No spend/token budget cap bounding metered model calls (cost exposure)

Severity: Medium (High if reachable in an unattended loop)
Confidence: Plausible
Evidence basis: simulation-reasoned
Domain: Reliability (XAPI-015)

Evidence:
- The provider layer applies no per-run cost/quota check; retries
  (`retry.rs`, EXT-GSL-002) fan out metered calls with no budget gate.
- `grep` for `budget|spend|cost_limit|max_cost|quota` across
  `crates/gosling/src/agents/` returned only `agent.rs` and `moim.rs` (not
  confirmed to be spend caps this pass — not traced).

Observed behavior:
- A runaway agent loop or a retry storm can issue unbounded metered completions;
  spend is observable only after the fact via usage logging.

Expected boundary:
- Per-job / per-period budget check in code; quota exhaustion a distinct,
  alertable error (`cost/quota playbook`).

Impact:
- Worst-case single-action spend = `per-turn tokens × loop iterations × retry
  multiplier (EXT-GSL-002) × concurrency`, unbounded.

Recommended mitigation / Non-goals:
- Route to `audit-reliability` (retry storms) and `audit-resource-lifecycle`
  (loop/poll bounds) for the agent-loop side; this lens flags the provider
  boundary only. Confirm/deny presence of a spend cap in `agent.rs` before
  scoring higher.

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Nominal agent: human-owner (policy).

Validation: not attempted this lens (see Validation Limits).

---

### EXT-GSL-006: API keys / OAuth tokens sent to user-configured host without allowlist

Severity: Low (Medium if a subscription OAuth token is misdirected)
Confidence: Confirmed (mechanism) / Plausible (exploited impact)
Evidence basis: source-evidenced
Domain: Security (XAPI-003)

Evidence:
- `crates/gosling-providers/src/api_client.rs:449-459` — `send_request` attaches
  the configured `AuthMethod` (Bearer/ApiKey/Custom) to whatever `host` the
  client was built with; `build_url` (`:375-394`) does no host allowlisting.
- `crates/gosling-providers/src/anthropic.rs:363-412` `from_declarative_config`
  builds the client from an arbitrary `config.base_url` and attaches the
  `x-api-key`.

Observed behavior:
- A misconfigured or attacker-supplied `ANTHROPIC_HOST` / declarative `base_url`
  causes the provider credential to be transmitted to that host.

Expected boundary:
- Bring-your-own-endpoint is intentional for API keys, but credentials
  (especially OAuth subscription tokens) should be pinned to known provider
  hosts, or the config source treated as trusted-only.

Impact:
- Credential exfiltration if provider config is attacker-influenceable (e.g. via
  an imported/shared session or config injection — see
  `audit-security`/`session` lenses).

Recommended mitigation:
- For OAuth-token auth, pin the host to the provider's known domain(s); for
  API-key BYO-endpoint, document the trust assumption.
- Behavior test: assert an OAuth-token client refuses a non-provider host.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal agent: human-owner (policy on
  which hosts are allowed).

Validation: not attempted (config-provenance is the security lens's domain).

Non-goals: this lens does not audit where `base_url`/`ANTHROPIC_HOST` originates
(route to `audit-security` / `audit-dataflow-integrity`).

## 5. Explicit non-findings (checked and held)

- **Error taxonomy is fine-grained (XAPI-008) — held.**
  `http_status.rs:181-249` maps 401/403→`Authentication`, 402→`CreditsExhausted`,
  413/context-400→`ContextLengthExceeded`, 429→`RateLimitExceeded`,
  5xx→`ServerError`, network vs HTTP separated in `errors.rs:130-169`
  (`is_network_error` distinguishes connect/timeout/request). Verified by
  `openai_compatible.rs:223-291` `test_case` matrix.
- **429 / `Retry-After` honored (XAPI-007) — held.**
  `http_status.rs:43-108` reads body `retry_after_seconds` then the header in
  seconds and all three RFC-7231 date forms, clamps absurd/NaN/infinite values
  (`:37,:64-70`), and `retry.rs:134-140,233-239` sleeps for the provider delay.
  Ten unit tests (`http_status.rs:293-395`).
- **Backoff has jitter and a cap (XAPI-006 positive) — held.**
  `retry.rs:65-81`: exponential, full-ish jitter (0.8–1.2×), capped at 30s
  default. Auth-error one-shot credential refresh is separate from the transient
  budget (`retry.rs:196-222`).
- **Tool-argument validation before execution (XAPI-016) — held.**
  `json.rs:131-238` detects truncation (incl. nested-closer cases) and returns
  `None`; `formats/openai.rs:1201-1234` surfaces `INVALID_PARAMS` for
  truncated/malformed/non-object args instead of invoking the tool. Twelve tests
  in `json.rs`.
- **Malformed / non-JSON response body classified, not crashed (XAPI-009) — held.**
  `http_status.rs:270-276` `handle_response` maps a JSON decode failure to
  `RequestFailed`; `anthropic.rs:159-228` treats an HTML body on `GET v1/models`
  as `EndpointNotFound` and falls back to the static list; a `200`-with-error
  body is caught (`anthropic.rs:192-199` + tests `:582-611`).
- **Secret redaction in request logs & error text (XAPI-004) — held.**
  `request_log.rs:66-134` logs only `model_config` + request/response payloads,
  never headers; `AuthMethod`'s `Debug` hides keys (`api_client.rs:205-218`);
  `ApiClient`'s `Debug` shows `"[auth method]"` (`:465-474`); `sanitize_url`
  (`http_status.rs:20-30`) strips userinfo and query before URLs enter errors.
  (Caveat: logged *message payloads* may contain sensitive user content — a
  data-handling concern, not a credential leak; note for `audit-security`.)
- **Azure token scope (XAPI-002) — held.** `azureauth.rs:138-149` requests a
  token scoped to `https://cognitiveservices.azure.com`, cached with a 30s
  early-expiry (`:165-171`), double-checked locking (`:120-136`).
- **Mid-stream network *errors* propagate (XAPI-013 positive half) — held.** A
  yielded stream `Err` is surfaced by `collect_stream`'s `result?`
  (`base.rs:332`) and `stream_openai_compat` maps decode errors to
  `NetworkError` (`openai_compatible.rs:181-185`). Only clean-EOF-without-marker
  is unguarded (EXT-GSL-004).

## 6. Failure-matrix coverage (per shared plumbing)

| Mode | Handled? | Evidence |
|---|---|---|
| 401/403 | Yes — `Authentication`, not retried | `http_status.rs:201`, `retry.rs:100-107` |
| 404 | Distinguished but **retried** by default | `http_status.rs:206` + EXT-GSL-001 |
| 400 / context | Yes — `ContextLengthExceeded` vs `RequestFailed` | `http_status.rs:215-222` |
| 429 + `Retry-After` | Yes — delay honored, capped | `http_status.rs:223-267` |
| 5xx | Yes — `ServerError`, retried w/ backoff | `http_status.rs:227`, `retry.rs:103` |
| timeout | Bounded (600s total) on main clients; **unbounded** on mantle/auth | `api_client.rs:17` vs EXT-GSL-003 |
| conn reset / DNS | Yes — `NetworkError`, separate from HTTP | `errors.rs:130-157` |
| malformed/HTML body | Yes — classified, not a panic | `http_status.rs:273-276`, `anthropic.rs:186-188` |
| partial/interrupted stream | **No** — clean EOF committed as complete | EXT-GSL-004 |
| refusal / empty model output | Partial — `Refusal` variant exists (`errors.rs:48-52`); empty-completion detection not traced | see Validation Limits |
| truncated tool args | Yes | `json.rs`, held |
| truncated text (`length`) | **No** — not surfaced | EXT-GSL-004 |

## 7. Selected paths (replay metadata)

Deliberate paths (all replayable against fakes, no live creds):
1. **Canonical success** — `POST chat/completions` streams chunks + `[DONE]`;
   `collect_stream` returns coalesced message. Covered by existing fixtures
   (`formats/openai.rs:2648-2714`).
2. **Provider failure/degraded** — 503 then success: assert bounded retries +
   backoff (target for EXT-GSL-002 Bedrock nesting test).
3. **Auth/validation rejection** — 400/404: assert single attempt (target for
   EXT-GSL-001 regression).
Risk-weighted (P1=5) additional paths: stalled-endpoint (EXT-GSL-003),
close-without-`[DONE]` (EXT-GSL-004), `finish_reason=length` text (EXT-GSL-004).
No live-credential drills run; all specified as fake-server tests.

## 8. Smallest hardening slice

1. `retry.rs` — default `transient_only = true` (or gate `RequestFailed` on
   status class). One-line-ish + tests. (EXT-GSL-001)
2. `bedrock.rs` — disable SDK-side retries so gosling owns the single budget.
   (EXT-GSL-002)
3. Give the mantle + auth/token clients a bounded timeout via a shared builder.
   (EXT-GSL-003)
Items 1–3 are all `codex`/`S`. EXT-GSL-004 (stream-completion marker) is the
next slice (`M`).

## 9. Validation Limits (what was NOT reviewed)

- **Build/test not run** (audit-only); all "tests exist" claims are from reading
  test source, not execution.
- **AWS SDK default retry count** not quoted from the pinned `aws-config`
  version — EXT-GSL-002 held at `Likely` for that reason.
- **ACP subprocess providers** (`*_acp.rs`, `crates/gosling/src/acp/`) reviewed
  only at the binding surface (`claude_acp.rs`); subprocess spawn/timeout/crash
  handling, ACP JSON-RPC framing robustness, and the `bypassPermissions` mode
  mapping (`claude_acp.rs:64-73`) were **not** traced — route to
  `audit-pipeline` follow-up + `audit-security` (the `Auto → bypassPermissions`
  mapping is security-relevant).
- **Not sampled**: `databricks*`, `snowflake.rs`, `sagemaker_tgi.rs`,
  `openrouter.rs`, `litellm.rs`, `gcpvertexai.rs` request bodies,
  `gemini_cli.rs`/`cursor_agent.rs`, `oauth_device_flow.rs` callback handling,
  and the Anthropic streaming decoder (`formats/anthropic.rs`) beyond its
  `handle_status`/marker structure.
- **Empty-completion / refusal detection** on the success path (does an empty
  streamed message or a `Refusal` reach the agent distinctly?) not traced — the
  `Refusal` variant exists but its producers were not enumerated.
- **Spend cap** (EXT-GSL-005) left `Plausible`: `agent.rs`/`moim.rs` matched the
  budget grep but were not read this lens.
- **MCP remote servers** (orientation names them as an external API) live in
  `crates/gosling-mcp` / `agents/mcp_client.rs` and were **not** covered here;
  route to a dedicated MCP pass.
- Host-provenance for EXT-GSL-006 (where `base_url` comes from) is out of scope —
  `audit-security` / `audit-dataflow-integrity`.
