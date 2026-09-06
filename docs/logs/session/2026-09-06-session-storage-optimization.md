# 2026-09-06 — Optimization discovery and session-storage repairs

## Scope and checkpoint

- Target: `/Users/eric/Work/vscode/forked/gosling`, branch `main`, clean at
  `1ee1967c244d3a24e1c5086953240933463d71e7` (the previous snippet optimization commit).
- Request: run another optimization pass after the snippet repair, patch, then audit only
  the patch and repair follow-up findings until none remain.
- Skills: `audit-optimization-opportunities` for discovery and
  `repair-performance-bottleneck` for the bounded repairs.
- Involvement: low, inferred from the explicit "identify, then proceed with patch, then
  audit and repair" instruction. Local edits, benchmarks, and checks are authorized; no
  commit, merge, or publication was requested or performed.
- Discovery budget: three concurrent read-only source sweeps (core crate and server,
  providers and MCP, Desktop renderer) plus direct SQLite prototyping of the two storage
  candidates. This is a sampled audit, not an exhaustive repository audit.
- Execution: discovery sweeps ran concurrently. Baseline, patch, post-patch measurement,
  and review were sequential because each depends on the preceding source state.

## Objectives and invariants

No single metric was supplied. Discovery considered per-turn agent work, per-tool-call
work under the SQLite write lock, list-request latency, streaming decode cost, and
Desktop render cost. Two storage metrics were selected because they could be measured
before and after on one local harness with an exact-output oracle.

| Beneficiary / metric | Workload / baseline | Constraint / success test |
| --- | --- | --- |
| User: chat-list latency | First page of 300 sessions × 100 messages; unpaged list of all 300 | Identical ordering, `message_count`, and `last_message_at`; cursor pagination, keyword, working-dir, workspace, archive filters unchanged |
| Agent turn: tool dispatch under the write lock | `begin_tool_operation` for the newest request after 300 tool rounds (about 6 MB of content) | Checkpoint requirement and payload-mismatch rejection unchanged; no schema change |
| Developer: feedback time | Existing debug test profile and CI gates | Keep full CI gates; add only ignored manual benchmarks |

## Architecture and contract baseline

| Source | Status / touched surface | Baseline |
| --- | --- | --- |
| `session_manager/session_listing.rs` module doc and `SessionListFilters`/`SessionListPageQuery`/`SessionListCursor` | Active listing contract: filters, cursor on `(sort_at, session_id)`, page size plus one lookahead, opt-in snippet hydration | Conformant; SQL shape is an implementation detail behind unchanged types |
| `session_manager.rs` `Session` row decoding (`message_count`, `last_message_timestamp`) and `message_timestamp_to_datetime` | Active row contract: count and normalized newest timestamp per session | Conformant; the same column names and integer semantics are produced |
| `session_manager/tool_operations.rs` module doc | Active ledger contract: dispatch-once, replay on redispatch, checkpoint before dispatch | Conformant; the checkpoint lookup only changes which copy of a duplicated id is compared |
| `session_manager/artifacts_storage.rs` request lookup | Sibling implementation already selects the newest checkpoint (`ORDER BY messages.id DESC LIMIT 1`) | Pre-existing asymmetry with the dispatch lookup; resolved by this patch |
| `docs/architecture.md`, `docs/logs/session/2026-08-22-modularize-session-manager.md` | Describe module ownership, not query shape | No schema, migration, interface, or ownership change proposed |

Giles YAML is advisory and untouched. No architecture or contract declaration was edited.

## Recurring-work map and opportunity register

Evidence pointers are line ranges at the checkpoint commit as read by the discovery
sweeps; they were spot-checked but not all independently re-read by the implementing pass.

| Work unit / consumer | Evidence and mechanism | Disposition |
| --- | --- | --- |
| Session listing / ACP `session/list`, CLI, orchestrator | `session_listing.rs:110-146`: `LEFT JOIN messages` + `GROUP BY s.id` with `COUNT` and non-sargable `MAX(CASE …)` before `LIMIT`; sqlite3 prototype on 2,000 sessions × 200 messages: 30 ms warm, sessions-only baseline 0.3 ms | Measured, Ready; repaired as REL-OPT-002 |
| Tool dispatch checkpoint lookup / every conversation-bound tool call | `tool_operations.rs:82-96`: `json_each` over every session message with `LIMIT 1` and no ordering, inside `BEGIN IMMEDIATE`; sqlite3 prototype on 600 messages / 6 MB: 3 ms per lookup versus 20 µs newest-first | Measured, Ready; repaired as REL-OPT-003 |
| Artifact discovery on persisted tool responses | `artifacts_storage.rs:42-56`: same scan but already `ORDER BY messages.id DESC LIMIT 1`; prototype 20 µs | Held; early termination already effective |
| `get_session(id, false)` per reply, SSE message, and loop iteration | `session_crud.rs:118-127`: `COUNT(*)` plus `MAX(CASE …)` per call; 25 agent and 40 server/ACP call sites | Candidate, Measure; the count needs the index scan regardless, so the seek trick alone does not help |
| Whole-history token counting per loop iteration | `reply_stream.rs:183-189`, `context_mgmt/mod.rs:412-446`, `token_counter.rs:52-73` | Candidate; existing PERF-GSL-003 stays partial pending a turn profile |
| System prompt rebuilt per turn with filesystem reads and template parse under a mutex held across an await | `reply_parts.rs:290-299`, `prompt_manager.rs:92-125`, `prompt_template.rs:60-98`, `load_hints.rs:234-300` | Candidate, Measure; caching needs hint-file invalidation semantics |
| Full conversation clones per turn | `reply_entry.rs:313-322` (unconditional `conversation_to_compact` clone), `reply_entry.rs:449-454` default fallback clone per iteration | Candidate, Measure; allocation reduction with lifetime review |
| Repetition inspector rebuilds full-history failed-call index per tool batch | `tool_monitor.rs:95-152` | Candidate; O(history × arguments) per batch |
| Request builders deep-copy formatted messages and tool schemas via `json!(expr)` | `formats/openai.rs:1691-1701`, `formats/anthropic.rs:693-710`, `formats/openai_responses.rs:644-678`; `serde_json::to_value` of an owned `Vec<Value>` | Candidate, Ready; `Value::Array(...)` move is exact. Provider crate, separate slice |
| Responses-API SSE line deserialized twice | `formats/openai_responses.rs:314-336` | Candidate; same class as resolved PERF-GSL-005 |
| Image paths re-detected and files re-read/re-encoded for the whole history each turn | `formats/openai.rs:255-268`, `images.rs:167-234` | Candidate, Measure; cache keyed by path and mtime changes freshness semantics |
| Signed-thinking dedupe clones all assistant content and compares O(k²) | `conversation.rs:509-541` | Candidate |
| Autovisualiser rebuilds multi-MB templates with chained `replace` per resource read | `gosling-mcp/src/autovisualiser/mod.rs:768-864` | Candidate; pure function of URI, one-time `OnceLock` |
| Spreadsheet/document tools re-parse the whole file per call | `computercontroller/mod.rs:1342-1465`, `xlsx_tool.rs:41-44`, `docx_tool.rs:106-115` | Candidate, Measure |
| Ollama stream rescans the accumulated buffer per chunk; regexes compiled per response | `formats/ollama.rs:38-39`, `:180-190` | Candidate; bounded incremental search |
| Desktop: store snapshot copies `artifacts`/`notifications` per notification, forcing per-chunk IPC and whole-list re-render | `chatSessionStore.ts:863-883`, `BaseChat.tsx:172-190`, `ArtifactWorkbenchContext.tsx:268-326`, `ArtifactRouterContext.tsx:107-133` | Candidate, likely highest user-visible leverage; needs renderer profiling, not a Rust harness |
| Desktop: O(n²) tail scan inside the per-message render map; `notificationsMap`, `commandHistory`, `ThreadNavigator` recomputed per chunk; `indexOf`/`find` per adapter event | `ProgressiveMessageList.tsx:501-512`, `useChatSession.ts:400-409`, `BaseChat.tsx:255-267`, `ThreadNavigator.tsx:53-142`, `acp/adapter/shared.ts:45-55` | Candidate, Measure |
| Desktop: session-list refetch storms | `useNavigationSessions.ts:148-196` (33 polls per created session), `SessionListPane.tsx:422-557` (full re-pagination on rename) | Candidate; amplifies REL-OPT-002's request count |

Checked and held: HTTP clients are constructed once per `ApiClient`; SSE framing is
incremental; the think-filter drains its buffer; all other provider regexes are lazily
initialized; tokenizer and security patterns already use `OnceCell`/`LazyLock`.

## OPT coverage

| Code | State | Evidence / disposition |
| --- | --- | --- |
| OPT-001 | Candidate | Token hashing per iteration and system prompt rebuild per turn; Measure |
| OPT-002 | Candidate | Desktop session-created polling and per-message metadata writes at turn end; not repaired here |
| OPT-003 | Finding | REL-OPT-002: whole-table aggregation for a paged consumer |
| OPT-004 | Not Reviewed | No critical-path profile of independent work |
| OPT-005 | Candidate | `json!(expr)` request-body deep copies; signed-thinking clone; Desktop message clones |
| OPT-006 | Finding | REL-OPT-003: every message parsed to find one request; REL-OPT-002 counts for the page only |
| OPT-007 | Held | Clients, pools, and tokenizer are long-lived |
| OPT-008 | Candidate | Autovisualiser templates, xlsx/docx parses, image re-encoding; each needs an invalidation owner |
| OPT-009 | Candidate | Desktop 300 ms session polling for 10 s after creation |
| OPT-010 | Held | No artifact retention change proposed |
| OPT-011 | Not Reviewed | No provider billing telemetry |
| OPT-012 | Not Reviewed | No utilization or cost history |
| OPT-013 | Held | CI job shape unchanged; no timings to justify removal |
| OPT-014 | Held | Focused local tests plus retained full gates |
| OPT-015 | Not Reviewed | No operator frequency evidence |
| OPT-016 | Not Reviewed | No source-of-truth inventory |
| OPT-017 | Finding | REL-OPT-003: the dispatch lookup and the artifact-discovery lookup implemented the same request lookup with different ordering |
| OPT-018 | Held | No seam removal proposed |
| OPT-019 | Not Reviewed | No dependency-level build profile |
| OPT-020 | Not Reviewed | No reachability inventory |

## REL-OPT-002: Session listing aggregates every message row before paging

Severity: Low  
Confidence: Confirmed  
Evidence basis: runtime-observed  
Domain: Reliability

Evidence:
- Baseline `crates/gosling/src/session/session_manager/session_listing.rs:110-146`: `LEFT JOIN messages m`, `GROUP BY s.id`, `COUNT(m.id)`, `MAX(CASE WHEN m.created_timestamp > … END)`, `HAVING` on the aggregate for the cursor, then `LIMIT`.
- `EXPLAIN QUERY PLAN` on the prototype fixture: `SEARCH m USING COVERING INDEX idx_messages_session_time_asc` for every session, then `USE TEMP B-TREE FOR ORDER BY`.
- Rust baseline: first page median 36,063 µs; unpaged 41,169 µs (30 samples each).

Observed behavior: list cost grows with the total number of messages across all matching sessions, independent of page size. Expected boundary: a page of fifty sessions needs fifty counts and one newest-timestamp per candidate session. Failure mechanism: the count and the non-sargable normalized maximum require visiting every index entry per session. Break-it angle: sessions with no messages, millisecond-only timestamps, mixed millisecond and second timestamps where the older message is in milliseconds, ties broken by id, and `updated_at` newer than the newest message.

Impact: avoidable work on every chat-list request from Desktop, ACP clients, CLI listing, and the orchestrator; amplified by the Desktop refetch storms noted above.

Operational impact: Local blast radius; user-visible latency; reversible; UI-visible; rerun safe.

Recommended mitigation: compute the newest message time as the null-safe maximum of two index seeks (`MAX(created_timestamp)` over the millisecond range and over the second range), derive `sort_timestamp`, apply cursor, order, and limit, and only then count messages for the returned rows. `only_sessions_with_messages` becomes an `EXISTS` predicate. Guardrails: exact comparison of ordering, `message_count`, and `last_message_at` against per-session `get_session` aggregates across seven fixture shapes, three page sizes, the messages-only filter, and the unpaged path; the existing paged, keyword, cursor, tiebreak, archive, and type tests.

Implementation assessment: `local_guardrail`, cost S; drivers tests and runtime verification; nominal agent Codex, because this is one private SQL builder with a local harness. No schema, migration, public API, or configuration change.

Non-goals: keyword search cost (unchanged `json_each` EXISTS), `get_session` aggregates, Desktop refetch behavior, snippet hydration.

## REL-OPT-003: Tool dispatch parses every session message to find its checkpoint

Severity: Low  
Confidence: Confirmed  
Evidence basis: runtime-observed  
Domain: Reliability

Evidence:
- Baseline `crates/gosling/src/session/session_manager/tool_operations.rs:82-96`: `FROM messages, json_each(messages.content_json)` filtered by session and request id with `LIMIT 1` and no `ORDER BY`, executed inside `BEGIN IMMEDIATE` after `acquire_write_guard`.
- Caller `crates/gosling/src/agents/agent/tool_dispatch.rs:118-124` on every conversation-bound tool call.
- Sibling `artifacts_storage.rs:42-56` performs the same lookup with `ORDER BY messages.id DESC LIMIT 1`.
- Rust baseline: 7,893 µs median per dispatch after 300 tool rounds.

Observed behavior: the planner walks `idx_messages_session_time_asc` from the oldest message and parses each `content_json` until the id matches; the request being dispatched is almost always the newest message, so nearly the whole session is parsed while other writers wait on the write gate. Expected boundary: one parse of the most recent checkpoint. Failure mechanism: unordered `LIMIT 1`. Break-it angle: an id checkpointed twice with different payloads, a request that was never checkpointed, and an old request id re-dispatched.

Impact: per-tool-call CPU and write-lock hold time growing linearly with session size; contributes to the DB-lock contention observed when several sessions share the store.

Operational impact: Workflow blast radius; DB side-effect class (lock hold time only, no data change); reversible; log-only; rerun safe.

Recommended mitigation: add `ORDER BY messages.id DESC` so the scan stops at the newest checkpoint. Explicit behavior change: when one request id is checkpointed more than once, the newest payload is now the one compared, matching artifact discovery. Guardrail: a focused test asserts newest-wins, old-request dispatch, and the unchanged missing-checkpoint error; the existing ledger tests cover replay, in-doubt, and payload collision.

Implementation assessment: `local_guardrail`, cost XS; drivers tests and runtime verification; nominal agent Codex. No schema or interface change.

Non-goals: passing the in-memory request from the caller (signature change), the artifact-discovery scan, `get_session` aggregates.

## Benchmark method and evidence quality

Commands (identical before and after; the only source difference is the production patch):

```sh
source bin/activate-hermit
cargo test -p gosling --lib -- --ignored --nocapture benchmark_session_listing
cargo test -p gosling --lib -- --ignored --nocapture benchmark_begin_tool_operation
```

Host: Apple M5, Darwin arm64, macOS 26.6.2; rustc/cargo 1.92.0; unoptimized test profile. One benchmark ran at a time for the retained numbers (an earlier concurrent run of the two after-benchmarks was discarded). Fixtures use fresh temporary `SessionManager` databases. The listing fixture inserts 300 sessions × 100 messages directly through the pool; the dispatch fixture persists 300 tool request/response rounds through `add_message` with 20,000-character results, then times `begin_tool_operation` for the newest id, deleting the ledger row between samples so every sample takes the checkpoint-lookup path. Each case discards warmups and records 30 samples; correctness assertions compare repeated results to the first result inside the timed loop.

| Quality dimension | Disposition / limit |
| --- | --- |
| Repeatability | 30 samples with median and full spread; before/after ranges are disjoint for all three measurements |
| Warm-up | Warm steady state; not cold page cache. The sqlite3 prototype showed a cold first run an order of magnitude slower for both shapes |
| Workload realism | Real storage code and real SQLite; synthetic uniform fixtures. Real session-size distribution unknown |
| Tail sample size | p95 over 30 samples is descriptive only |
| Observer effect | No profiler; debug build, so no release-build timing claim |
| Environment control | Same workstation and toolchain; scheduling, power, and thermals unpinned |
| Single variable | Same harness code in both runs; production SQL is the only difference |

Baseline (`scratchpad/listing-baseline.log`, `scratchpad/toolop-baseline.log`):

```text
first page of 300 sessions x 100 messages: median=36063.29us p95=39506.83us min=34544.96us max=58900.92us n=30
unpaged list of all sessions: median=41168.58us p95=43701.54us min=38712.92us max=44087.58us n=30
begin_tool_operation, newest request after 300 tool rounds: median=7893.42us p95=10723.50us min=7214.33us max=10829.00us n=30
```

After (`scratchpad/listing-after.log`, `scratchpad/toolop-after.log`):

```text
first page of 300 sessions x 100 messages: median=2140.29us p95=2229.75us min=2087.12us max=2250.08us n=30
unpaged list of all sessions: median=6921.71us p95=7405.08us min=6776.50us max=7848.62us n=30
begin_tool_operation, newest request after 300 tool rounds: median=187.33us p95=657.92us min=138.92us max=909.17us n=30
```

| Measurement | Before median | After median | Ratio |
| --- | --- | --- | --- |
| First page, 300 × 100 | 36.06 ms | 2.14 ms | 16.8× |
| Unpaged list, 300 × 100 | 41.17 ms | 6.92 ms | 5.9× |
| Tool dispatch after 300 rounds | 7.89 ms | 0.19 ms | 42× |

The unpaged path still counts messages for every session, so its improvement comes only from the seek-based newest timestamp. The sqlite3 prototype on 2,000 sessions × 400,000 messages (30 ms → 4 ms warm) is consistent with the Rust fixture but is not a retained regression procedure.

## Follow-up review record

Patch-only review was performed by the implementing agent in a separate pass; this is not independent corroboration.

- First patched test run failed with `no such column: s.last_message_timestamp`: the derived layer computed `sort_timestamp` but did not project `last_message_timestamp`. The column is now projected alongside `sort_timestamp`; the exact-activity test then passed. Test-fixture repair on the initial prototype: an unlimited derived table does not preserve inner `ORDER BY`, so the production query orders the outer select as well.
- The initial benchmark used a closure returning an async block that borrowed its argument, which fails lifetime inference; it is a nested `async fn`.
- The new newest-wins test fails on the baseline source (returns `Execute` for the superseded payload) and passes on the patched source, which confirms the guard exercises the changed ordering.
- Independent patch-only review (a separate reviewer agent that also rebuilt both SQL
  shapes in a Python/SQLite harness and compared 43,200 filter, cursor, page-size, and
  timestamp-boundary cases with zero mismatches) returned three Low findings, all repaired:
  the `LAST_MESSAGE_TIMESTAMP_SQL` comment claimed "one index seek per side" although the
  flattened plan evaluates each range maximum several times per session, so it now describes
  an index-range maximum instead; `docs/TODO.md` said the listing counts only the returned
  page, which is true only for the paged path, so the entry now says so; and the equivalence
  walk did not place a cursor anchor on the first member of a tie, so a page size of 1 was
  added (the existing tiebreak test already covered the id comparison). The reviewer's Info
  note about the duplicated `report` helper in the two benchmark modules is accepted as
  test-only duplication and left in place.
- The full `cargo test -p gosling --lib` run reported one failure unrelated to this patch: `merge_environments_keeps_the_original_error_when_nothing_is_declared` expects no `MUNINN_MCP_BEARER_TOKEN` credential, but this host session exports that variable. Re-running with `env -u MUNINN_MCP_BEARER_TOKEN` passes. This is a host-environment leak into a pre-existing test, recorded as a follow-up, not silenced.

| Reviewed boundary / attempted failure | Clearing evidence |
| --- | --- |
| Ordering drift when `updated_at` is newer than the newest message, or ties on time | Fixture rows "stale messages, newer updated_at", "empty, recently updated", and "tie with seconds only" match the `session_sort_at` oracle; existing tiebreak test passes |
| Millisecond/second mix picks the wrong side | Both mixed fixtures (older message in each unit) match `get_session`, which uses the original `CASE` aggregate |
| SQLite multi-argument `MAX` returning NULL when one side is NULL | Explicit `CASE` in `LAST_MESSAGE_TIMESTAMP_SQL`; "seconds only" and "milliseconds only" fixtures assert a non-null `last_message_at` |
| `only_sessions_with_messages` no longer excludes empty sessions | `EXISTS` predicate; the six-session filtered walk and the existing "filters empty and cwd before pagination" test pass |
| Cursor evaluated against a different expression than the one used to build it | Cursor `WHERE` targets the projected `sort_timestamp` column that also drives ordering; multi-page walks at page sizes 1, 2, 3, and 100 return identical sequences, and the existing duplicate-activity tiebreak test covers the id comparison |
| Bind-order mismatch | Placeholder order is unchanged: types, working dir, workspace, keywords, cursor triple, limit; keyword and workspace tests pass |
| Count computed for rows outside the page | `EXPLAIN QUERY PLAN` on the prototype shows the count as a correlated subquery over the co-routine that already applied `LIMIT` |
| Dispatch accepts a non-checkpointed request | Unchanged `must be durably checkpointed` error asserted |
| Newest-wins changes which duplicate is compared | Explicit, documented, matches artifact discovery; asserted by the new test |
| Extra allocations or new caches | None added; both changes are SQL text only |

Residual costs: keyword search and the unpaged count still visit every message index entry of matching sessions; the flattened query evaluates the two seek subqueries several times per session, which the fixture shows is cheap. Cold-cache behavior is unmeasured in Rust.

## Closure and validation

| Check | Result |
| --- | --- |
| `cargo test -p gosling --lib -- session_listing::tests tool_operations::tests` | 2 passed; 2 manual benchmarks ignored |
| `cargo test -p gosling --lib` on patched source | 1847 passed; 1 failed (host-environment leak in an unrelated secret-source test, see above); 3 ignored |
| `cargo test -p gosling --test acp_fork_session_test --test acp_custom_provider_methods_test --test acp_custom_requests_test` | 20 passed (2 + 1 + 17); 0 failed |
| Both ignored benchmarks | Passed before and after; outputs retained above |
| `cargo clippy -p gosling --all-targets -- -D warnings` | Passed |
| `cargo fmt --all -- --check`, `git diff --check`, governance-marker scan | All passed; required AGENTS marker present |
| Architecture/contract comparison | Conformant before and after; **no new drift** |
| Patch-only follow-up audit | Three Low findings from the independent review repaired; **0 remaining findings** on the re-run focused tests, Clippy, and format checks |

Files changed: `crates/gosling/src/session/session_manager/session_listing.rs`,
`crates/gosling/src/session/session_manager/tool_operations.rs`, `.gitignore`,
`docs/TODO.md`, and this session log.

Record reconciliation: REL-OPT-002 and REL-OPT-003 are fresh findings recorded as closed in
`docs/TODO.md`. The prior log's "Session listing / ACP and Desktop" measurement backlog item
is now resolved by REL-OPT-002. No in-code marker named either defect. PERF-GSL-003 and all
other findings retain their prior status. Follow-ups: the host-environment-sensitive
secret-source test, and the candidate register above for the next optimization pass.

## Validation limits

Live provider throughput, packaged Electron responsiveness, cold-cache database opens,
production session-size distributions, concurrent multi-session lock contention, and
cross-platform behavior are not measured. `gosling-cli` end-to-end tests that list sessions
were not run. Candidate register entries outside the two repairs are structural evidence
with prototype or sweep pointers only; no measured benefit is claimed for them.
