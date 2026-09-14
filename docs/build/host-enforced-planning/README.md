# Host-enforced planning implementation record

Status: Implemented and automatically validated; ready for code review, with no
release or shipped claim

This record tracks the staged implementation of ADR-0020 and the approved
host-enforced planning plan pinned to `main` commit
`bd064a5168fd4b26016b51ffd6f8d0f10e82dd6e`.

## Scope

Committed release scope:

- persisted plan generations, revisions, feedback, events, and decisions;
- bounded gosling-owned planning tools and non-widenable execution policy;
- typed ACP, CLI, and first-party Desktop review workflows;
- permission conformance and hermetic hostile-provider/tool tests;
- restart, mutation, copy, fork, import/export, handoff, archive, and deletion
  semantics.

Explicit non-goals are a Grok integration, a generic workflow engine,
delegated planning, arbitrary shell or web research during planning, automatic
execution on approval, and independent `/goal` verification.

## Baseline

| Fact | Evidence |
| --- | --- |
| Branch and revision | `main` at `bd064a5168fd4b26016b51ffd6f8d0f10e82dd6e` |
| Initial worktree | clean |
| Session schema | v34 in `session_manager.rs` |
| Plan source | 2026-09-13 architectural plan, 1,823 lines, read in full |
| Toolchain | repository pin `1.92`; activated `rustc 1.92.0`, `cargo 1.92.0` |
| Declared validation | focused Rust/ACP/CLI/Desktop gates, full Rust tests, formatting, clippy |

The pinned baseline itself had no material path or interface drift. Runtime
paths now exist for the core plan lifecycle, execution boundary, ACP, CLI, and
Desktop review. The current-state architecture registry includes the core,
security-boundary, and Desktop plan-review paths. Focused and broad automated
validation is recorded below. Release remains an operator decision, and no
published or shipped claim is made.

## Binding decision register

This record resolves review ambiguities in the originating plan. Where its
sequence or an open design alternative differs from this section, these accepted
decisions control the staged implementation:

| Decision | Accepted contract |
| --- | --- |
| Mixed transcript rows | Source hashing filters plan-lifecycle request/result blocks individually and retains other planner-visible blocks from the same row. |
| Hash encoding | Content, source, and scope contracts use SHA-256; source/scope use a version tag and canonical length-prefixed fields, never ambiguous concatenation. |
| Turn policy | A turn that starts as `Planning(plan_id, generation)` remains planning-restricted until terminal and can never fall back to `Normal` in flight. |
| Ledger fence | Planning authorization revalidation and durable tool-operation begin/replay are one atomic fenced action before tracing, hooks, prompts, frontend emission, or side effects. |
| Planning extension | Planning capabilities are internal and non-configurable, injected only for eligible planning turns, and excluded from persisted enabled-extension state. |
| Provider transition | Provider/model transition is rejected while a generation is `drafting` or `awaiting_review`. |
| Native transfer | Native export uses the exact optional top-level `plan_history_v1` section; import remaps all identities and references and stores every imported generation as stale history. |
| ACP authority | Token-authenticated HTTP/WebSocket and local stdio clients use their existing admission boundary. In explicit `--dangerously-unauthenticated` mode, every admitted client is treated as user authority and the mode remains explicitly dangerous. |
| Architecture registry | Current paths are registered as `core.session_planning`, `security.planning_execution_boundary`, and `desktop.plan_review`; ARC-011 and ARC-012 are active review gates, not conformance or shipped claims. |
| Native wire-key drift | Resolved and tested: native transfer uses the accepted exact `plan_history_v1` key and its round-trip test references that constant. |

The canonical source/scope encoding is a domain tag followed by each field as
`u64 little-endian byte length || field bytes`, in fixed documented field order.
Collections encode their item count followed by items in canonical order. Every
hash fixture records the exact input bytes and expected lowercase SHA-256. A
mixed row that becomes empty after lifecycle filtering contributes no evidence
block; retained prose or another permitted tool exchange remains part of that
row's canonical evidence.

`plan_history_v1` contains `schema_version: 1` and bounded `plans`, `revisions`,
`feedback`, and `events` collections. Import assigns new local identities,
remaps parent/active/consumed/event references, verifies every reference belongs
to the same imported plan, records import provenance, and forces plan status to
`stale` regardless of exported status. The new session, transcript, and plan
history commit atomically. Missing sections preserve current flat-session import;
malformed or over-limit sections fail visibly rather than being discarded.

## Corrected dependency ledger

This order supersedes the originating plan's two dependency conflicts: it does
not activate current-state architecture entries before code exists, and it does
not require the Workstream-D harness before the step that originally created it.
Continuity protection also precedes client exposure rather than relying on a
later rollback note.

| Step | State | Evidence / next action |
| --- | --- | --- |
| 1. Characterize baseline and establish D0 harness | complete | baseline behavior was captured and the hermetic hostile-provider/tool fixture corpus is present under `crates/gosling/tests/fixtures/` |
| 2. Accept architecture intent | implemented | ADR-0020 accepted; this traceability record and index entry added; current implemented paths are now registered under Step 13 |
| 3. Domain, schema v35, and canonical hashes | complete | fresh/migrated schemas, byte-exact hash fixtures, mutation invalidation, and cross-plan corruption rejection are covered |
| 4. PlanService transitions and mutation fences | complete | compare-and-swap decisions, immutable turn policy, provider-transition blocking, and stale-state handling are implemented and tested |
| 5. Bounded workspace helpers | complete | planning reads/search/tree walk use cap-std roots with explicit result, byte, depth, and match bounds |
| 6. Internal planning capability/readiness | complete | seven typed planning capabilities are host-injected only for eligible planning turns and excluded from ordinary/public extension state |
| 7. Atomic dispatch enforcement and plan-policy corpus | complete | policy revalidation and durable operation begin/replay share one transaction before hooks, prompts, frontend emission, tracing, or side effects |
| 8. Typed ACP/SDK | complete | get/start/feedback/approve/abandon/export requests and plan notifications are generated, typed, and exercised through ACP |
| 9. Core continuity and native transfer | complete | copy, fork, import/export, handoff, archive, and deletion preserve or invalidate plan authority according to the accepted contract |
| 10. CLI lifecycle | complete | `/plan` lifecycle commands replace the process-local implementation without clearing history or mutating global authorization mode |
| 11. Desktop lifecycle | complete; operator acceptance pending | typed adapter, reconnect recovery, review dialog, line feedback, decisions, export, stale refresh, and separate implementation submission are covered by unit tests and a compact/standard Electron playtest |
| 12. Final conformance/harness | complete | focused policy, ACP, continuity, CLI, provider, Desktop, hostile-command, and broad Rust/UI suites pass; exact commands and qualifications are in the session log |
| 13. Registry, docs, release controls, and legacy removal | complete | guides/defaults now match host-enforced behavior; current paths and ARC-011/ARC-012 are registered; generated ACP artifacts are byte-idempotent |
| 14. Compaction value gate | decision complete; implementation deferred | [Source audit](session-compaction-runs-future-plan.md) confirms the diagnostic gap in criterion 1 and part of criterion 2; the metadata-only follow-up remains outside this release |

Step 7 cannot pass on catalog filtering alone. Its required race case commits a
review transition, concurrently approves or abandons it, then supplies a late
fabricated backend/frontend/app tool request from the original planning turn.
The expected result is a hard denial with no approval prompt, raw-argument log,
operation row, hook, frontend request, or side effect. Normal permission behavior
is tested only in a separately started ordinary turn.

Before Step 9 completes, any copy, fork, native import/export, share, handoff,
provider transition, or scope/history mutation that lacks its final plan
semantics must reject an open plan. It may not silently omit, preserve as
approvable, or transfer plan authority.

## Architecture registration

The current runtime paths are registered without making a shipped or conformance
claim:

- `core.session_planning` owns the existing plan state, storage, continuity,
  ACP, and CLI paths;
- `security.planning_execution_boundary` owns the existing immutable turn
  policy, typed capability identity, provider-readiness, and dispatch fences;
- `desktop.plan_review` owns the typed Desktop plan adapter and first-party
  review controls that bind feedback and decisions to an exact revision;
- ARC-011 scopes the current enforcement and provider-readiness paths and checks
  every dispatch origin plus pre-ledger denial;
- ARC-012 scopes the current core storage/continuity, ACP, and CLI paths and
  checks exact revision/source/scope comparison for mutating references,
  generation/revision/content/status identity for explicit export, stale
  transfer semantics, and separation of approval from implementation
  submission;
- ARC-012 now also scopes the registered Desktop adapter and review-control
  paths. This registration does not replace final presentation or conformance
  validation.

## Release gates

The source candidate is GO for code review: automated evidence covers domain
correctness, all dispatch paths, CLI and Desktop parity, continuity operations,
hermetic hostile fixtures, legacy removal, and broad validation. Publishing a
release remains NO-GO in this record because an operator has not performed and
accepted the final manual Desktop walkthrough. The Electron playtest supplies
repeatable presentation evidence but does not substitute for that acceptance.

Completed evidence includes:

- byte-exact SHA-256 fixtures for source/scope/content contracts, including
  mixed lifecycle/prose rows and version changes;
- fresh-v35 versus migrated-v34 schema equivalence and same-plan referential
  corruption cases;
- old flat import, valid `plan_history_v1` import with complete identity remap,
  and malformed/over-limit rollback;
- old sessions with explicit enabled-extension state proving planning tools are
  available only through internal policy and never ordinary configuration;
- planning-turn terminal-state races proving no mid-turn fallback to `Normal`;
- authenticated HTTP/WebSocket, local stdio, and explicit dangerous
  unauthenticated ACP admission behavior;
- provider/model transition rejection for both `drafting` and
  `awaiting_review`, with no partial mutation;
- continuity operations proving authority is never silently omitted or
  transferred.

The validation ledger is intentionally qualified:

- the full `gosling` test suite passed in a hermetic path root; its aggregate
  run excluded one environment-mutating prompt-template case, which passed as
  an exact isolated test;
- `cargo clippy --locked --all-targets -- -D warnings`, `cargo build --locked`,
  the full CLI suite, the full Desktop Vitest suite, focused Electron plan
  review, documentation build, and targeted ACP/schema checks passed;
- `just check-acp-schema` correctly reported that generated ACP files differ
  from `HEAD` because this candidate intentionally changes the schema; two
  consecutive generations produced identical bytes;
- the repository-wide Desktop Prettier command still finds a pre-existing
  52-file backlog outside this feature's changed surface; targeted changed
  TypeScript/TSX formatting and `git diff --check` pass.

## Decisions and deviations

- ADR-0020 is accepted before runtime work.
- Architecture components and ARC-011/ARC-012 now register only existing paths.
  Their active status makes the rules binding for review; component notes state
  that implementation remains in progress and make no conformance or release
  claim. The Desktop adapter and review-control paths are registered; final
  adapter/presentation validation remains pending.
- The original Workstream-D sequencing is split: D0 fixture infrastructure is
  a Step-1 prerequisite, plan-policy cases co-land with Step 7, and final
  cross-layer coverage remains Step 12.
- Core continuity precedes CLI/Desktop exposure. Temporary fail-closed blockers
  are acceptable during development; silent omission or authority transfer is
  not.
- The Step-14 value gate passes because committed in-session compaction has no
  durable source-row provenance, and its selected retry/filter/chunk strategy is
  not recoverable from full diagnostics. The bounded metadata-only
  [`session_compaction_runs` future plan](session-compaction-runs-future-plan.md)
  is a post-release Workstream-E follow-up. Workstreams A-D remain the complete
  scope of this release; this decision adds no runtime, schema, API, or telemetry
  change.
- The catalog's generic feature-removal repair workflow was rejected as
  structurally inapplicable to this additive feature; the Rust application
  implementation workflow governs code changes.

## Adjacent ChatGPT Codex fallback repair

Final validation exposed a route-specific model withdrawal: a ChatGPT-account
Codex request could reject the selected model with HTTP 400, while Gosling's
turn-local failover predicate admitted only transient retryable errors. The
repair keeps the primary request non-retryable but permits the still-unexecuted
turn to use an explicitly configured `GOSLING_FAILOVER_PROVIDER` and
`GOSLING_FAILOVER_MODEL`. Unrelated bad requests remain ineligible, the saved
session route is unchanged, and mid-stream/tool-bearing turns are never replayed.

The static offline ChatGPT Codex catalog no longer advertises `gpt-5.4` or
`gpt-5.4-mini`. This is deliberately scoped to that ChatGPT-account route: it
does not claim the model names disappeared from every OpenAI API surface, and it
does not silently migrate saved sessions or enable fallback without explicit
configuration. Claude Code CLI remains a manual checkpoint-recovery target,
not an automatic Gosling-managed API fallback.

## Handoff

Review the source candidate and the exact validation ledger, then perform the
final operator Desktop walkthrough before authorizing a release. Workstream E
(`session_compaction_runs`) and independent `/goal` verification remain bounded
future work and must not be represented as part of this implementation.
