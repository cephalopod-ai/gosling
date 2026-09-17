# Compacted tool history ordering and failover (2026-09-16)

## Scope, authority, and diagnosis

- Agent: Codex; `main` at `87e295606`, with the earlier heartbeat and read-only
  shell permission repairs uncommitted and preserved.
- Input: the operator's screenshot of session `20260916_4` failing with a 400
  `No tool call found for function call output`, and the follow-up asking why
  automatic failover did not activate. This authorizes bounded source diagnosis
  and repair, not a new provider route or a live-session rewrite/install.
- Workflow: reuse catalog `repair-defect-priority` and its binding contracts;
  one selected P1 backend/data-integrity item **CMP-ORDER-001**, recurring failures
  after compaction. Low involvement reflects the operator's approval-friction
  report. No unrelated patch batch or independent reviewer is required.
- Read-only session/log inspection found the exact failed request
  `call_oSAmGqalwFNeJ6YZ6piBz2vJ`. Its tool request and result are both stored.
- Compaction revision `01a0ad30-a7ba-7022-b22f-ff1e0126ffba` was committed at
  `2026-09-17T02:27:22.170630Z`. It covers 28 earlier message IDs, excluding
  both sides of the failed tool exchange. The compaction did not split that pair.
- The summary occupies ledger row 106395, followed by the retained request
  (106396) and result (106397). The summary's creation time is 02:27:22 UTC;
  the retained tool exchange's creation time is 00:41:57 UTC.
- Both full conversation loaders sort by `created_timestamp, id`. That moves
  the summary behind older retained exchanges. `prepare_reply_context` runs
  `fix_conversation`, whose final lead/trail step strips the leading assistant
  request after its tool-pair validation has already run. Its result remains.
- The CLI log at 02:42:53.611909 UTC records MOIM detecting the same orphan.
  MOIM returns `None` on unexpected issues, retaining that incoming conversation;
  provider preparation filters visibility but does not repair again. OpenAI
  rejects the remaining orphan result at 02:42:54.481081 UTC. No shell commands,
  tool side effects, provider calls, or secret values were replayed.
- Official OpenAI function-calling guidance requires returning tool outputs
  with their matching calls (see https://developers.openai.com/api/docs/guides/function-calling).
  The local ordering and repair trace establishes the application defect.

## Failover evidence

The installed `config.yaml` contains no `GOSLING_FAILOVER_PROVIDER` or
`GOSLING_FAILOVER_MODEL`; neither running packaged backend exposes those variables.
The fallback target resolver returns no configured target in that state. Default
`should_failover` admits transient outages/rate limits and unavailable-model
failures, while this malformed-history 400 is a permanent request failure.
Both conditions prevent automatic failover here. `GOSLING_FAST_MODEL` is an
auxiliary-model route, not the backup route. No fallback is configured in this run.

The existing failover contract is opt-in, once per turn, before tool execution,
turn-local, and excludes planning authority and credential-pinned/provider-owned
tool loops. Those boundaries remain unchanged. Governing implementation and
provenance: `gosling-providers/src/retry.rs`, agent `reply_stream.rs` and
`provider_transitions.rs`, the configuration/environment-variable guides, and
the September 2 provider-failover MVP log.

## Selected patch plan and contract baseline

Repair full conversation reads to preserve insertion/ledger order, consistent
with paged history, atomic compaction rewrites, and handoff source boundaries.
Update both message-ID rollback variants to use that same order, including their
plan-staleness checks. Keep the explicitly timestamp-based truncate API's time
semantics. Use the existing `(session_id, id DESC)` index; no migration/dependency
or public API change is needed.

Add regressions in the existing compaction integration suite for round-trip
summary/tool-pair ordering and inclusive/exclusive rollback with nonmonotonic
creation times. Do not change compaction timestamps, tool execution, provider
selection, permissions, failover eligibility, or historical records. Broader
conversation-normalizer hardening is not selected.

Active declarations: canonical AGENTS; architecture (core owns session storage and
provider preparation); ADR-0021 (atomic compaction conversation rewrite and source
IDs); ADR-0019 (context root and persisted ledger continuity); and failover docs.
Pre-repair disposition: pre-existing read/rollback ordering drift from the stored
conversation and row-boundary contracts. The failover policy is conformant, but
disabled/unqualified for this failure. No higher-priority item is deferred within
this bounded scope. Earlier session logs and index migrations remain provenance.

## Validation plan and checkpoint

Independent source, record, database, and process-environment inspections were
batched. Source edits/checks depend on diagnosis and are sequential. This log is
the durable checkpoint; preserve earlier dirty files on resume.

AGENTS reserves Cargo tests/builds/Clippy for an explicit request. Run Hermit
formatting, diff/source scans, read-only SQL ordering checks against the incident,
and disposable SQLite probes using the changed production queries. Add Rust
regressions but do not represent them as executed. Intended Cargo validation when
authorized: `cargo test -p gosling --test compaction` and targeted session-manager,
planning invalidation, retry/failover suites, then relevant Clippy/build checks.

## Patch and validation results

Both full loaders now read `ORDER BY id`, preserving the conversation committed by
compaction and the same order used by transcript pages and handoff coverage. Both
message-ID rollback methods resolve their anchor by row ID and use inclusive or
exclusive row comparisons for deletion and plan-staleness detection. Existing
immediate transactions, session scoping, plan notifications, summary invalidation,
current-usage reset, and explicitly timestamp-based truncation remain intact.

Three compaction integration regressions use a fixture whose summary is newer
than its retained request/result. Reload must preserve the entire conversation and
leave its agent-visible tool exchange valid without repair. Inclusive rollback
must retain the summary before the request; exclusive rollback must retain the
summary and completed exchange. Original test source remains unchanged except for
imports needed by these additions.

| Check | Result and scope |
| --- | --- |
| `source bin/activate-hermit` then `cargo fmt` | Passed |
| Inline Python production-function comparison with `git show HEAD:<source>` | Passed: only the two loaders and two message-ID rollback methods changed; all other production function bodies are byte-identical |
| Disposable SQLite probes using SQL extracted from the changed production methods | Passed: load order, inclusive/exclusive deletion, unaffected/affected plan boundaries, missing anchors and sibling isolation; old production queries fail the order/rollback assertions |
| SQLite `EXPLAIN QUERY PLAN` for the changed loader query | Passed: existing `idx_messages_session_row_desc` index used; no temporary ordering tree |
| Read-only SQLite incident probe, frozen through row 106530 | Passed: the changed production query puts summary 106395 before failed request 106396; the old production query puts the failed request first after excluding the recorded compacted prefix |
| Inline Python regression inventory comparison | Passed: one fixture and exactly three new Rust tests; original test bodies unchanged |
| Config/environment inspection | Both packaged backends have no fallback pair or config-path overrides; user config has no fallback pair, and system config is absent |
| Record/link/ignore/governance checks and `git diff --check` | Passed |
| Cargo regressions, build and Clippy | Not run; AGENTS requires an explicit test/build request |
| Installed-app/provider replay | Not run; installed app and live database unchanged |

The SQL probes exercise the production query strings rather than compiling the
Rust control flow. They therefore prove the ordering/deletion predicates and
query-plan properties, not the complete adapter/normalizer or transactional Rust
paths. Rust execution and installed-app acceptance remain pending.

The implementing agent reviewed source behavior and then performed a separate
completeness pass. No independent-review or repository-wide-clean claim is made.
The diff preserves transaction/notification/usage handling and makes the rollback
plan guards agree with the changed deletion boundary. Same-second rows still use
their insertion order; nonmonotonic imported/retained creation times no longer
override the explicitly stored conversation sequence. Timestamps themselves and
session last-activity calculations are unchanged. No migration or index removal is
needed; the historical creation-time index migration remains valid provenance.

Post-repair declaration comparison: **no new drift by source and SQL inspection**.
The changed reads/rollback align with ADR-0021's committed conversation sequence
and ADR-0019's row-bounded context continuity; failover remains unchanged and
conformant. Runtime contract verification is incomplete. This patch does not
claim to repair every possible invalid conversation; normalizer/MOIM behavior is
unchanged, and no prior tool is executed to reconstruct its history.

Files changed for this repair: `session_manager/message_storage.rs`,
`tests/compaction.rs`, `.gitignore`, `docs/TODO.md`,
`docs/polish/active-todo-ledger.md`, and this log. The new screenshot finding had no
external issue record; TODO and its existing active mirror now track
**CMP-ORDER-001: source-patched, needs verification**. No old snapshot, release
result, historical ledger, or unrelated marker was rewritten.

Final status: **completed_with_partial_verification** for the source repair.
During this run the checkout advanced externally to `eac096239`, which commits
the earlier read-only navigation repair. Comparison against initial `87e295606`
confirms that both files covered by this run's source/SQL checks have unchanged
committed baselines. This run created no commit and preserves that concurrent work.
Next action when authorized: run the targeted Cargo regressions, session-storage
and planning-boundary suites, relevant lint/build checks, and an installed-app
same-model continuation after compaction. Automatic fallback configuration is a
separate operator choice; this run neither selects nor enables a backup provider.
