# Dataflow and Security Audit - 2026-07-09

Scope: repository `repo-makeover/gosling` at `f7b5c03c95678ca5fc0161199b8efbed88d548fd` on branch `main`.

Ground rules: this was an `audit_only` pass using local source, workflows, and repo instructions. It did not claim runtime reproduction where none was performed, and it did not run full build/test/clippy flows because the current task was audit logging rather than code repair.

## Applied skills

- `audit-dataflow-pipeline-graph`
- `audit-security-repo-posture`

## Summary table

| Field | Value |
|---|---|
| Repository | `repo-makeover/gosling` |
| Platform | `github` |
| Audit mode | `audit_only` |
| Variant | `baseline` |
| Overall posture | `partial` |
| Critical | `0` |
| High | `0` |
| Medium | `3` |
| Low | `0` |
| Not observable | `0` |
| Not applicable | `0` |

## Posture matrix

| Category | Status | Severity | Confidence |
|---|---|---|---|
| Secrets controlled and rotated | `partial` | `Low` | `Likely` |
| Dependencies inventoried and updated | `pass` | `Info` | `Confirmed` |
| Workflows least-privileged and trust-boundary aware | `partial` | `Low` | `Confirmed` |
| Third-party automation pinned and governed | `pass` | `Info` | `Confirmed` |
| Branches protected and owned | `partial` | `Info` | `Confirmed` |
| Runners isolated and observable | `pass` | `Info` | `Confirmed` |
| Artifacts signed and provenance-backed | `partial` | `Medium` | `Confirmed` |
| Alerts routed to owners with enforced remediation | `partial` | `Info` | `Confirmed` |

## Pipeline graph

Entry points:
- `crates/gosling-server/src/routes/session_events.rs::session_reply`
- `crates/gosling/src/agents/agent.rs::handle_tool_result`

Terminal products:
- SSE `MessageEvent` stream back to the caller
- persisted session conversation state
- assistant message augmented with tool responses

Major stages:
- request validation and branch classification
- active-request registration
- session lookup and optional history replacement
- agent reply stream start
- frontend tool rendezvous
- event publication and persistence

Decision nodes:
- elicitation-response short-circuit
- active request registration allow/reject
- override conversation present/absent
- reply stream start success/failure
- frontend tool path vs non-frontend tool path

Persistence surfaces:
- `SessionManager::replace_conversation`
- session message persistence via the agent/session manager path

Trace surfaces:
- `SessionEventBus`
- SSE `MessageEvent::{Message,Error}`
- workflow logs and SARIF uploads under `.github/workflows/`

### Node table

| Node ID | File | Function/Class | Stage Type | Inputs | Outputs | Side Effects | Branches | Terminal? |
|---|---|---|---|---|---|---|---|---|
| N01 | `crates/gosling-server/src/routes/session_events.rs` | `session_reply` | `entrypoint` | HTTP request, `session_id`, `request_id`, `user_message`, `override_conversation` | validated task context | opens request lifecycle | elicitation vs normal reply | No |
| N02 | `crates/gosling-server/src/routes/session_events.rs` | elicitation-response detection | `validation` | message content | boolean branch | none | elicitation short-circuit | No |
| N03 | `crates/gosling-server/src/routes/session_events.rs` | `SessionEventBus::try_register_request` call site | `policy` | `request_id`, session bus | cancellation token or rejection | marks one request active | allow/reject | No |
| N04 | `crates/gosling-server/src/routes/session_events.rs` | session load and config assembly | `routing` | session id, state | `SessionConfig`, stored conversation | reads persisted session | session found/error | No |
| N05 | `crates/gosling-server/src/routes/session_events.rs` | override conversation branch | `persistence` | `override_conversation` | replacement conversation | durable conversation rewrite | override present/absent | No |
| N06 | `crates/gosling-server/src/routes/session_events.rs` | `agent.reply(...)` start | `capability` | user message, session config, cancel token | reply stream or error | starts model/tool pipeline | start success/failure | No |
| N07 | `crates/gosling/src/agents/tool_execution.rs` | `handle_frontend_tool_request` | `adapter` | `ToolRequest` | frontend tool request message, tool response append | waits on shared result queue | frontend tool yes/no | No |
| N08 | `crates/gosling/src/agents/agent.rs` | `handle_tool_result` | `routing` | tool result callback from UI/frontend | `(id, result)` tuple in queue | enqueues into shared channel | none | No |
| N09 | `crates/gosling-server/src/routes/session_events.rs` | publish loop | `trace` | `AgentEvent` stream | SSE events | emits `MessageEvent`, tracks token state | message/error/cancel | Yes |

### Edge table

| Edge ID | From Node | To Node | Condition | Data Passed | Side Effect |
|---|---|---|---|---|---|
| E01 | N01 | N02 | unconditional | request message | classify elicitation response |
| E02 | N02 | N06 | elicitation branch | user message, session config | skips active-request registration |
| E03 | N02 | N03 | normal reply branch | request/session ids | reserves active request slot |
| E04 | N03 | N04 | registration success | cancellation token, state | opens tracked request lifecycle |
| E05 | N04 | N05 | override present | imported history | durable conversation replacement |
| E06 | N04 | N06 | override absent | stored conversation, config | none |
| E07 | N05 | N06 | after replacement | user message, config | start reply against already-mutated session |
| E08 | N06 | N09 | reply stream started | agent events | publishes SSE events |
| E09 | N08 | N07 | frontend tool callback arrives | `(id, result)` | first waiter consumes next queued tuple |

## Path inventory

- Candidate path count: `4`
- Equivalent path clusters:
  - `C1 canonical reply`: normal reply without override conversation and without frontend tool branching.
  - `C2 override replay`: normal reply with imported conversation replacement before reply start.
  - `C3 frontend rendezvous`: reply emits frontend tool request and waits for asynchronous tool callback.
  - `C4 elicitation short-circuit`: elicitation response re-enters `agent.reply` without opening a second SSE request lane.
- High-risk paths:
  - `P1` override conversation + reply-start failure: canonical durable state can be replaced before any assistant output exists.
  - `P2` concurrent or stale frontend tool callback: first queued callback may bind to the wrong request.
  - `P3` ask-ai-bot publication path: artifact pushed to GHCR without provenance.

## Selected paths

Three deliberate paths selected for this audit:
1. `Path A`: canonical success, normal `session_reply`, no override conversation, reply stream starts, SSE messages publish.
2. `Path B`: controlled degraded branch, override conversation present and reply start fails before any assistant output.
3. `Path C`: failure/race branch, frontend tool request waits while a stale or out-of-order result is queued first.

Five randomized base paths:
1. Deferred: runtime randomized replay was not executed in this audit-only pass.
2. Deferred: runtime randomized replay was not executed in this audit-only pass.
3. Deferred: runtime randomized replay was not executed in this audit-only pass.
4. Deferred: runtime randomized replay was not executed in this audit-only pass.
5. Deferred: runtime randomized replay was not executed in this audit-only pass.

## Randomization

- Seed: not generated
- Selection method: static risk-prioritized path selection
- Path signatures: `P1 override-before-start`, `P2 shared-tool-result-queue`, `P3 GHCR-without-provenance`
- Input generation strategy: static source reasoning only
- Input hashes: not generated
- Replay method: targeted regression tests should be added during repair
- Collision-space estimate: not applicable to this static-only pass

## Branch coverage

- Decision nodes inspected: elicitation short-circuit, active request registration, override conversation branch, reply start success/failure branch, frontend tool result queue branch, publish workflow provenance branch
- Branches covered: source-level branch expansion only
- Branches deferred:
  - runtime provider-start failure branch
  - concurrent frontend tool result reordering branch
  - workflow execution proof for ask-ai-bot image publication
- Reason for deferral: this audit produced a static evidence log and did not run build/test or live workflow drills.

## Invariants tested

| Invariant | Result | Basis |
|---|---|---|
| Only one active non-elicitation request may own a session stream at a time | `Pass` | `SessionEventBus::try_register_request` branch in `session_events.rs` |
| Elicitation responses avoid registering a second active request | `Pass` | `session_events.rs:320-333` |
| Failed reply startup must not durably rewrite canonical conversation state | `Fail` | `STT-GOS-001` |
| Frontend tool responses must correlate to the originating request id | `Fail` | `CON-GOS-001` |
| Published release artifacts should carry provenance on every public publication path | `Fail` | `RSP-ART-001` |
| Untrusted PR-target workflows must not check out attacker-controlled PR head with write credentials | `Pass` | `quarantine.yml` and `dependabot-auto-merge.yml` are metadata-only |
| Self-hosted runner exposure should be absent or isolated | `Pass` | search of `.github/workflows` found only GitHub-hosted runners |

## Findings

| ID | Severity | Path | Finding | Evidence | Recommendation |
|---|---|---|---|---|---|
| `STT-GOS-001` | `Medium` | `P1` | Override conversation is persisted before reply startup succeeds | `session_events.rs`, `session_manager.rs` | stage replacement until reply startup succeeds or roll back on failure |
| `CON-GOS-001` | `Medium` | `P2` | Frontend tool result queue can misassociate results across requests | `tool_execution.rs`, `agent.rs` | bind results by request id or use per-request channels |
| `RSP-ART-001` | `Medium` | `P3` | Ask AI Bot publication path lacks provenance attestation | `publish-ask-ai-bot.yml`, `publish-docker.yml`, `release.yml` | align workflow with existing attestation pattern |

### STT-GOS-001: Override conversation is persisted before reply startup succeeds

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: State-Transition

Evidence:
- `crates/gosling-server/src/routes/session_events.rs:401-428` replaces the session conversation when `override_conversation` is present, then calls `agent.reply(...)` afterward.
- `crates/gosling-server/src/routes/session_events.rs:430-439` returns an error event if reply startup fails, with no compensating restore.
- `crates/gosling/src/session/session_manager.rs:571-572` shows `replace_conversation` delegates directly to storage.
- search performed: `rg -n "replace_conversation|tool_result_rx|tool_result_tx" crates/gosling/src crates/gosling-server/src`

Observed behavior:
- Imported override history becomes canonical session state before the reply stream proves it can start.

Expected boundary:
- Temporary or imported history should not durably replace canonical session state until the new turn is accepted and startup succeeds, or the operation should be wrapped in rollback semantics.

Failure mechanism:
- The route performs a durable write first, then enters a fallible startup call. The error branch publishes diagnostics but does not restore prior conversation state.

Break-it angle:
- Provider initialization failure, invalid configuration, cancellation, or any early `agent.reply(...)` error after override import leaves the session with replaced history even though no successful turn occurred.

Impact:
- Canonical conversation state can diverge from user-visible turn outcomes. Session history can be rewritten by a failed replay/import attempt, which weakens auditability and can discard prior context.

Operational impact:
- Blast radius: Service
- Side-effect class: DB
- Reversibility: compensatable
- Operator visibility: UI-visible
- Rerun safety: unsafe

Adjacent failure modes:
- A later retry can compound the divergence by appending on top of already-replaced history.

Recommended mitigation:
- Remediation patterns: transactional write barrier, deferred-commit state transition.
- Minimal repair: stage override conversation in memory, start the reply stream, and only persist replacement after startup succeeds.
- Local guardrail: snapshot previous conversation and restore it on any startup error before returning.
- Behavior test: inject a failing `agent.reply(...)` startup path and assert persisted conversation remains unchanged when override history is supplied.

Implementation assessment:
- Complexity: persistence_recovery
- Cost: M
- Cost drivers: modules, tests, runtime_verification
- Nominal implementation agent: codex
- Rationale: the fix is local to the route/session boundary but needs careful persistence sequencing and regression coverage.

Validation:
- Add a route-level test covering `override_conversation + reply startup error`.
- Assert stored conversation bytes/hash are unchanged across the failed request.

Non-goals:
- Do not redesign general conversation import/export semantics in the same repair slice.

### CON-GOS-001: Frontend tool result queue can misassociate results across requests

Severity: Medium
Confidence: Likely
Evidence basis: simulation-reasoned
Domain: Concurrency

Evidence:
- `crates/gosling/src/agents/tool_execution.rs:175-194` waits for `self.tool_result_rx.lock().await.recv().await` and attaches whichever `(id, result)` tuple arrives first.
- `crates/gosling/src/agents/agent.rs:236-249` defines one shared `tool_result_tx`/`tool_result_rx` pair on the `Agent`.
- `crates/gosling/src/agents/agent.rs:331-377` initializes that queue once per agent with `mpsc::channel(32)`.
- `crates/gosling/src/agents/agent.rs:3186-3189` forwards arbitrary `(id, result)` tuples into the shared queue without checking the waiting request.
- search performed: `rg -n "replace_conversation|tool_result_rx|tool_result_tx" crates/gosling/src crates/gosling-server/src`

Observed behavior:
- A frontend tool waiter consumes the next available tool result from a shared queue, regardless of whether the queued `id` matches the originating `ToolRequest`.

Expected boundary:
- Frontend tool results should be delivered only to the matching request id, or mismatched/stale results should be discarded or re-routed.

Failure mechanism:
- The design uses one shared queue for all frontend tool callbacks and does not correlate the dequeued tuple with the current waiter before attaching the result.

Break-it angle:
- Parallel frontend tool requests, retries, stale UI responses, or delayed results from a prior turn can satisfy the wrong waiter and poison the assistant message for a different request.

Impact:
- Tool outputs can be attached to the wrong logical request, which can mislead the model, the UI, or the operator. In the worst case, one request can be satisfied with another request's privileged result.

Operational impact:
- Blast radius: Service
- Side-effect class: user-visible
- Reversibility: compensatable
- Operator visibility: UI-visible
- Rerun safety: unsafe

Adjacent failure modes:
- `STT-GOS-001` becomes harder to debug if a misbound tool result produces misleading post-failure state.

Recommended mitigation:
- Remediation patterns: request-scoped rendezvous, keyed async mailbox.
- Minimal repair: replace the shared first-in queue wait with a map from request id to oneshot sender/receiver, or loop until a matching id is received and safely requeue mismatches.
- Local guardrail: reject or log mismatched ids instead of attaching them silently.
- Behavior test: issue two concurrent frontend tool requests and deliver results out of order; assert each response binds only to its originating request id.

Implementation assessment:
- Complexity: cross_process_coordination
- Cost: M
- Cost drivers: modules, tests, runtime_verification
- Nominal implementation agent: codex
- Rationale: the queue semantics are local, but the fix spans agent/frontend coordination and needs concurrency-focused regression tests.

Validation:
- Add an async test with concurrent frontend tool requests and out-of-order callbacks.
- Assert stale results are ignored or correctly rerouted.

Non-goals:
- Do not redesign all tool execution plumbing or unrelated MCP notification routing in the same slice.

### RSP-ART-001: Ask AI Bot publication path lacks provenance attestation

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- `.github/workflows/publish-ask-ai-bot.yml:13-16` grants `contents: read` and `packages: write`, but no `id-token: write` or `attestations: write`.
- `.github/workflows/publish-ask-ai-bot.yml:44-54` builds and pushes the image to GHCR with no attestation step.
- `.github/workflows/publish-docker.yml:15-19` grants `id-token: write` and `attestations: write` for the main Docker publication path.
- `.github/workflows/publish-docker.yml:67-72` emits build provenance for the main image.
- `.github/workflows/release.yml:103-127` attests release artifacts on the tagged release path.
- search performed: `rg -n "attest-build-provenance|permissions:|packages: write|id-token: write" .github/workflows -g '*.yml' -g '*.yaml'`

Observed behavior:
- The repository publishes the `ask-ai-bot` container image to GHCR without the provenance guarantees already used by the main Docker and release workflows.

Expected boundary:
- Public artifact publication paths should emit provenance consistently so downstream consumers can verify origin across all released deliverables.

Failure mechanism:
- The dedicated Ask AI Bot workflow omits OIDC/attestation permissions and never runs the existing attestation action pattern.

Break-it angle:
- If the build path or dependencies are compromised, consumers of this image have a weaker verification story than consumers of the main release artifacts.

Impact:
- Supply-chain integrity for one published artifact line is weaker than the rest of the repository's hardened release posture.

Operational impact:
- Blast radius: Cross-system
- Side-effect class: external API
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unknown

Adjacent failure modes:
- Mutable base-image tags in container build paths remain a related hardening concern even after provenance is added.

Recommended mitigation:
- Remediation patterns: release-path parity, provenance normalization.
- Minimal repair: mirror the `publish-docker.yml` attestation pattern in `publish-ask-ai-bot.yml`.
- Local guardrail: fail PR review or release review when a publish workflow lacks provenance permissions and attestation steps.
- Behavior test: add a workflow policy check or review checklist that asserts every publish workflow uses provenance.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: S
- Cost drivers: modules, tests, docs
- Nominal implementation agent: codex
- Rationale: the change is localized to workflow policy and can reuse an existing repository pattern.

Validation:
- Dry-run workflow review confirming `id-token: write`, `attestations: write`, and `actions/attest-build-provenance` are present on the Ask AI Bot publish path.

Non-goals:
- Do not expand this slice into a full container base-image pinning campaign unless explicitly requested.

## Not observable

- Branch protection rules and merge requirements exist in GitHub settings, not repository files. No `.github/settings.yml` or committed ruleset file was present in the scanned tree.
- Environment protection rules for release or package publication are platform-side controls and were not observable from local files.
- Whether code-scanning, scorecard, or other alerts block merges is not observable from repository files alone.
- Secret rotation cadence, break-glass handling, and incident paging beyond `SECURITY.md` were not observable from the repository.

## Not applicable

- None. All eight baseline posture categories had at least partial local evidence.

## Recommended remediation order

1. Fix `STT-GOS-001` first because it is a canonical state-transition defect on persisted session history.
2. Fix `CON-GOS-001` next because concurrent or stale tool callbacks can silently corrupt the logical reply path.
3. Normalize `RSP-ART-001` so all public publication paths carry provenance.
4. Open a platform-governance follow-up for branch protection, merge enforcement, and environment protection settings that are not observable from source control.

## Files changed

- `reports/2026-07-09-dataflow-security-audit.md`

## Validation

- Baseline state captured with `git status --short`, `git branch --show-current`, and `git rev-parse HEAD`.
- Static evidence gathered with `rg`, `nl -ba`, and targeted workflow/code-path inspection.
- No runtime reproduction, build, or full test suite execution was performed in this audit-only pass.

## Dory/checkpoint

- No separate Dory/checkpoint file was updated.
- The repository's canonical audit artifact location appears to be `reports/`, and this report was written there.

## Risks

- `CON-GOS-001` is a high-quality static concurrency finding, but it remains `Likely` rather than `Confirmed` until reproduced with an out-of-order callback test.
- Platform-side controls may be stronger than this report can observe locally; they should not be assumed absent, only unverified.
- Container base-image mutability in some Dockerfiles is a related hardening concern, but it was not elevated to a material finding in this pass.

## Deferred work

- Add targeted regression tests for reply-start rollback and concurrent frontend tool result ordering.
- Decide whether `override_conversation` should be a transactional operation or an explicitly ephemeral import path.
- Review mutable container base-image tags in deployment Dockerfiles as a separate hardening slice.
- Verify GitHub-side branch protection, required checks, and environment protection outside the repository.

## Next recommended slice

- Implement a narrow repair slice for `STT-GOS-001` and `CON-GOS-001`, backed by targeted async regression tests.
- Follow with a workflow-only hardening slice for `RSP-ART-001`.
- Re-run this audit after those repairs and re-check the platform-only controls against live GitHub settings before treating the repo as fully hardened.
