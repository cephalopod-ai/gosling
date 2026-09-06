# 2026-09-06 — Optimization discovery and chat-list snippet repair

## Scope and checkpoint

- Target: `/Users/eric/Work/vscode/forked/gosling`, branch `main`, initially clean at `a109432d6b5854834c9560c71707976b567a7da2`.
- Request: discover performance opportunities, patch, then audit only the patch and repair follow-up findings.
- Skills: `audit-optimization-opportunities`, `repair-performance-bottleneck`, and `audit-performance-profile` for measurement review.
- Involvement: low, inferred from the explicit request to proceed with a patch and repair loop. Local edits, benchmarks, and checks are authorized; no publication is requested.
- Discovery budget: sample approximately 20 implementation/configuration files plus their immediate contracts. This is not an exhaustive repository audit.
- Execution: independent source reads run concurrently. Baseline, patch, post-patch measurement, and review are sequential because each depends on the preceding state. Cargo builds use the existing artifact cache; interrupted benchmark sampling must restart.
- Completed: repository orientation, prior-performance-record reconciliation, sampled work map, snippet producer/consumer and audience-contract trace, manual benchmark harness, baseline measurement, bounded production repair and regression cases.
- Current checkpoint: production patch, same-harness measurement, correctness checks, Clippy, and patch-only review complete. REL-OPT-001 is closed in `docs/TODO.md`. Final documentation/format checks are recorded below.

## Objectives and invariants

No single metric was supplied. Discovery considered user latency, temporary allocation/copying, and developer feedback cost. The selected measurement is snippet projection time and the neighboring metric is a 20-session list operation including SQLite retrieval and deserialization.

| Beneficiary / metric | Workload / baseline | Constraint / success test |
| --- | --- | --- |
| User: chat-list latency | Short text, 64 KiB whitespace-rich text, 1 MiB word, and 1 MiB image with short text; manual benchmark pending | Exact snippet equality; repeat the same benchmark before/after |
| Process: temporary data copying | Source currently clones audience-visible content and materializes entire normalized text for 128 characters | Borrow only visible text; do not copy images or tool results; preserve visibility and Unicode semantics |
| Developer: feedback time | CI and Cargo profile sampled; runtime timings unavailable | Keep full CI gates and existing disk-budget choices |

## Architecture and contract baseline

| Source | Status / touched surface | Baseline |
| --- | --- | --- |
| `last_message_snippet.rs::message_snippet` documentation and tests | Active snippet contract: user-visible text, collapsed whitespace, character limit, trailing ellipsis | Conformant; implementation performs avoidable work |
| `gosling-providers/src/conversation/message.rs::filter_for_audience` and `as_text` | Active content contract: absent audience allows text, explicit audience must contain User; only Text projects to a snippet | Conformant; borrowed selection must preserve these rules |
| `session_manager/session_listing.rs` and `acp/server/list_sessions.rs` | Active opt-in hydration, eight recent rows per session, database and pagination ownership | Conformant; unchanged by proposed repair |
| `ui/desktop/src/acp/sessions.ts`, `components/sessions/SessionListPane.tsx` | Active adapter and consumer use optional snippet verbatim with empty fallback | Conformant; output contract unchanged |
| `docs/architecture.md` | Accepted architecture; older workspace format explicitly described as design intent with code authoritative | No schema/interface change proposed |

The historical performance reports describe earlier snapshots. Their already-repaired paths are not new findings. Giles YAML is advisory and contains historical open governance work; this task does not reconcile or promote it.

## Recurring-work map and opportunity register

| Work unit / consumer | Evidence and mechanism | Disposition / validation / risk |
| --- | --- | --- |
| Snippet projection / chat-list cards | `last_message_snippet.rs`, baseline lines 133–150: audience filtering clones even non-text payloads, followed by full text copies and normalization before taking 128 characters | Measured, Ready; repair selected. Borrow text and stop at known truncation. Same-harness list and microbenchmark, exact Unicode/visibility oracle; low local regression risk, reversible source-only change |
| Session listing / ACP and Desktop | `session_manager/session_listing.rs:68–70, 115–140`: joins messages and computes COUNT/MAX before page LIMIT | Structural, Measure; profile list queries over many sessions/messages. Metadata denormalization would touch every message writer, import, truncation, and migration; deferred |
| Token counting / context assembly | `token_counter.rs:35–40, 54–72`: blake3 hashes text before checking the 1,024-entry LRU | Structural, Measure; existing PERF-GSL-003 remains partial. Capture a real turn profile before considering per-message estimates. Invalidation and context-budget correctness are material risks |

Other sampled work: the Desktop memoized render index, streaming payload decoding, security scanning, Cargo compilation, and CI validation. No measured runtime or operator-time claim is inferred from source shape.

## OPT coverage

| Code | State | Evidence / disposition |
| --- | --- | --- |
| OPT-001 | Candidate | Token cache still hashes inputs on hits; PERF-GSL-003, Measure |
| OPT-002 | Held | Snippet retrieval uses one bounded UNION ALL query in `recent_message_rows`, not per-session async round trips |
| OPT-003 | Candidate | Session list aggregation precedes LIMIT; measure query cost before changing durable metadata |
| OPT-004 | Not Reviewed | No independent-operation critical-path profile; adding concurrency is not justified |
| OPT-005 | Finding | REL-OPT-001: snippet content cloning and normalization intermediates |
| OPT-006 | Finding | REL-OPT-001: full text projection for a 128-character consumer; database row decoding remains outside the repair |
| OPT-007 | Held | `SessionManager` retains storage/pool; this patch introduces no per-message client or process |
| OPT-008 | Held | Tokenizer/shared counter use OnceCell; security patterns use LazyLock/RegexSet; avoid adding duplicate caches |
| OPT-009 | Not Reviewed | Background timers/watchers not traced under the discovery budget |
| OPT-010 | Held | Cargo dev/test profiles intentionally disable incremental archives to bound prior target-directory growth; no retention reduction proposed |
| OPT-011 | Not Reviewed | No provider request/billing telemetry; paid-call amplification not inferred |
| OPT-012 | Not Reviewed | No utilization, saturation, or cost history |
| OPT-013 | Held | `.github/workflows/ci.yml` separates formatting and cached Rust builds; Windows/MSRV gates cover distinct contracts; no job timings to justify removal |
| OPT-014 | Held | Existing doc/code path filter and focused local tests provide selective feedback while full CI gates remain |
| OPT-015 | Not Reviewed | No operator frequency/time evidence |
| OPT-016 | Not Reviewed | No complete source-of-truth/consumer inventory for generated or manually repeated fields |
| OPT-017 | Candidate | `session_naming.rs:74–110` has a similar audience/text projection chain, but a different complete-context contract; deferred measurement, not the bounded snippet repair |
| OPT-018 | Held | Provider conversation re-export and ACP/session adapters have real ownership/compatibility consumers; no seam deletion proposed |
| OPT-019 | Not Reviewed | Manifests sampled; no dependency-level build or bundle profile |
| OPT-020 | Not Reviewed | No complete dynamic/reachability inventory; Goose compatibility is protected |

## REL-OPT-001: Full-message work for bounded chat-list snippets

Severity: Low  
Confidence: Confirmed  
Evidence basis: runtime-observed  
Domain: Reliability

Evidence:
- Baseline `crates/gosling/src/session/last_message_snippet.rs:133–150` at `a109432d6b5854834c9560c71707976b567a7da2`: `filter_for_audience(Role::User)`, `.collect::<Vec<_>>().join("\n")`, and `text.split_whitespace().collect::<Vec<_>>().join(" ")` before `.take(max_chars)`.
- `gosling-providers/src/conversation/message.rs:344–391` clones allowed Text/Image/ToolResponse content; `as_text` at lines 600–605 only accepts Text.
- Baseline benchmark: 1 MiB word projection median 14,827.70 µs; 20-session list median 149,925.67 µs, each with 30 samples on the local debug build.

Observed behavior: CPU/copy work grows with complete payload size even when the consumer displays only a short prefix. Expected boundary: only visible text and the normalized prefix are needed. Failure mechanism: owned audience filtering precedes text selection, then whole-text normalization precedes truncation. Break-it angle: large single-word reply, image before text, hidden blocks, and Unicode whitespace at the exact truncation boundary.

Impact: avoidable local processing on snippet-enabled chat lists. User frequency and release-build impact are unmeasured; no general agent-turn speedup is claimed.

Operational impact: Local blast radius; user-visible latency; reversible; UI-visible; rerun safe. Adjacent cost: full SQLite JSON decoding remains necessary in the existing hydration path.

Recommended mitigation: bounded borrowed projection; preserve explicit/default audiences and message visibility; normalize character-by-character with one pending separator; stop when the next non-whitespace character proves truncation. Guardrails: 5,103 small-input equivalence cases, audience combinations, ignored image/tool/thinking content, trailing whitespace, existing storage mutation tests, and a manual performance benchmark.

Implementation assessment: `local_guardrail`, cost S; drivers tests and runtime verification; nominal agent Codex, because this is one private Rust function with an accessible local test harness. No public API, database, configuration, dependency, or security-policy change.

Validation: exact output assertions and same-harness before/after comparison; final status recorded below. Non-goals: session-name generation, message decoding, session-list aggregate SQL, tokenizer cache, desktop rendering, provider latency, or architecture cleanup.

## Specialist routing and measurement backlog

| Opportunity | Owner / secondary lens | Next discriminating evidence |
| --- | --- | --- |
| REL-OPT-001 | `repair-performance-bottleneck`; Input/Output and Security angles for Unicode and audience equivalence | Same-harness comparison plus focused contract tests |
| Session aggregate listing | `audit-performance-profile`; Data Integrity if proposing cached metadata | Query profile at 10× session count and 10× messages/session, including filtered pagination |
| PERF-GSL-003 | `audit-performance-profile`; memory lifecycle if proposing a new cache | Per-turn CPU/profile share of token hashing and session reloads |
| Session naming projection | `audit-performance-profile` | Measure initial-message payload copying against provider naming time; full context is still required |

Highest-leverage action within the measured slice is the snippet repair. Unmeasured neighboring ideas remain candidates, not unresolved findings from the patch audit.

## Benchmark method and evidence quality

Command (same before and after):

```sh
source bin/activate-hermit
cargo test -p gosling --lib benchmark_last_message_snippets -- --ignored --nocapture
```

Host: Apple M5, Darwin arm64, macOS 26.6.2; rustc/cargo 1.92.0; existing unoptimized test profile with line tables. One benchmark test runs at a time. Message construction and database population are outside timing; the microbenchmark black-boxes the input and output, discards 10 warmups, and measures 30 batches of 100 projections per case. The neighboring list measurement uses 20 sessions (seven 64 KiB texts, seven 1 MiB words, six 1 MiB images plus short text), one initial warm list, and 30 timed lists. Each list includes normal SQLite reads, deserialization, snippet hydration, and result mapping. Every fixture uses a new temporary SessionManager/database; no shared-state reset or modified pragma is used. Correctness assertions compare repeated list results, and separate tests assert the projection contract independently.

| Quality dimension | Disposition / limit |
| --- | --- |
| Repeatability | 30 samples per case with median and full spread; large-input conclusions must exceed both runs' spread |
| Warm-up | Explicit warm steady state; not cold startup or first database-open latency |
| Workload realism | Real message/storage code, synthetic large-payload fixtures; actual user payload distribution and concurrency unknown |
| Tail sample size | 30 is insufficient for a stable population p95. Printed p95 is descriptive only; no production-tail claim uses it |
| Observer effect | No profiler or per-iteration logging; debug build changes absolute times and ratios, so no release-build timing claim |
| Environment control | Same workstation and toolchain; CPU scheduling, power, thermals, and unrelated workload are unpinned. Small/noisy differences are inconclusive |
| Single variable | Identical benchmark operations and inputs; production difference is only snippet projection. Added correctness fixtures do not run in the ignored benchmark |

Baseline artifact: `/tmp/gosling-snippets-baseline.log`; retained measurement output:

```text
short: median=1.30us p95=1.35us min=1.27us max=1.70us n=30
64KiB text: median=1819.32us p95=2099.73us min=1727.61us max=2176.68us n=30
1MiB word: median=14827.70us p95=16749.37us min=14233.88us max=16970.36us n=30
1MiB image + short text: median=14.42us p95=17.66us min=14.03us max=18.19us n=30
20-session list with snippets: median=149925.67us p95=182315.88us min=145660.71us max=214280.96us n=30
```

Local attribution: the isolated function performs no I/O or locking. Timed cost is local traversal/allocation/copying; no CPU hardware counters were captured to separate compute from memory bandwidth. As a rough Amdahl estimate, seven long words plus seven whitespace-rich texts and six images cost `7*14.82770 + 7*1.81932 + 6*0.01442 = 116.61566 ms`, about 78% of the 149.92567 ms list median. This sums separate warm microbenchmarks, so it is an estimate, not a measured in-request profile share. Perfect projection would imply at most about 4.5× for this fixture; no such upper bound is inferred for real Desktop latency. Tokenizer and aggregate-SQL candidates have no measured share, so no projected speedup is assigned.

After artifact: `/tmp/gosling-snippets-after.log`; retained measurement output:

```text
short: median=0.62us p95=0.71us min=0.59us max=0.72us n=30
64KiB text: median=4.00us p95=6.04us min=3.79us max=7.37us n=30
1MiB word: median=2.31us p95=3.16us min=2.16us max=6.16us n=30
1MiB image + short text: median=0.60us p95=0.81us min=0.58us max=0.99us n=30
20-session list with snippets: median=31257.17us p95=39711.12us min=30135.38us max=47759.04us n=30
```

The neighboring list median falls from 149.93 ms to 31.26 ms (79.2% less elapsed time, 4.8× in this fixture). Its before/after ranges do not overlap: 145.66–214.28 ms versus 30.14–47.76 ms. All four microbenchmark ranges are also disjoint. The observed 4.8× exceeds the rough 4.5× estimate above because that estimate combines separate medians rather than measuring a component share in one request; it is not a hard empirical bound. No timing budget gates CI. The ignored benchmark is the retained regression-measurement procedure.

## Focused performance inventory

This lens applies only to the changed snippet path and its measurement harness.

| Codes | Disposition |
| --- | --- |
| PERF-001, PERF-002 | Held: baseline measured before the production change; real chat-list operation measured alongside the isolated helper |
| PERF-003, PERF-004, PERF-005 | Measurement limitations above are explicit: synthetic inputs, warm debug build, unpinned host, and descriptive-only p95. No user-level or tail SLO claim |
| PERF-006, PERF-007, PERF-009 | Held for patch delta: SQL shape, parameters, existing index, recent-row cap and round-trip count are unchanged |
| PERF-008, PERF-011, PERF-012 | REL-OPT-001 removes redundant full-payload projection/copying; original cost is linear per message, not claimed quadratic |
| PERF-010 | Synchronous projection remains within async hydration; its work is reduced. No executor-lag or starvation claim |
| PERF-013, PERF-014 | Held for patch delta: no lock, pool, thread, client, or lifecycle changes |
| PERF-015, PERF-016 | Outside this patch: queueing/concurrent load and startup not exercised; no finding or improvement asserted |

## Follow-up review record

- First correctness check: 12 passed, 1 failed, 1 manual benchmark ignored. The new Unicode fixture used `Message::with_text`, which calls `sanitize_unicode_tags` and composes `a` plus combining acute into `á`. Its expectation was based on the raw input, so the oracle was wrong. The fixture now uses `MessageContent::text` with `with_content` to feed exactly the intended Unicode sequence to this private projection; production sanitization is unchanged. This was a test-fixture repair, not an ignored failure.
- The corrected focused suite passed: 13 tests, including all 5,103 Unicode/boundary/limit comparisons. The broader session suite then passed 147 tests. First Clippy pass found an unused production-scope `AnnotateAble` import; it is needed only for test content construction and was moved into that test. The follow-up session/Clippy run validates this final source state.
- Final source validation: `cargo test -p gosling --lib session::` passed (147 passed, 0 failed, 1 manual benchmark ignored); `cargo clippy -p gosling --all-targets -- -D warnings` passed. The same benchmark also ran explicitly and passed. No remaining findings in the final patch-only review.

Patch-only review was performed by the implementing agent in a separate review pass; this is not independent multi-agent corroboration. Scope: the changed helper, tests/benchmark, ignore exception, and evidence/backlog records, with adjacent code read only to establish contracts.

| Reviewed boundary / attempted failure | Clearing evidence |
| --- | --- |
| Hidden message or assistant-only/empty audience leaks into a snippet | The `user_visible` early return is unchanged. Text audiences use the same absent/default and contains-User rules as `filter_for_audience`; explicit audience cases and existing hidden-message tests pass |
| Image/tool/thinking payload accidentally becomes visible text | Match admits only `MessageContent::Text`; mixed-content regression and existing tool-only cases pass |
| Unicode byte truncation or combined-character drift | Only `char` values are appended; 5,103 raw-input comparisons preserve scalar-value count and original normalization/truncation semantics |
| Missing separator between blocks; spurious ellipsis for trailing whitespace | Per-text newline preserves the previous join boundary; delayed space insertion excludes trailing whitespace; exact-fit and next-word cases pass |
| Empty or zero-budget behavior | Empty normalized input stays None; nonempty input at zero limit stays an ellipsis, as in the original helper |
| Stale cached previews after replacement/truncation | No cache added; existing live add/replace/truncate tests pass |
| Excessive allocation or work moved elsewhere | Projection borrows only text and allocates the result; no image/tool copies. Both isolated projection and the real list operation improved in the recorded fixture |
| New schema/API/ordering/freshness drift | Function signature, Session shape, ACP opt-in, SQL, recent-row cap, ordering, and mutation paths have no diff |
| Benchmark changes conceal result differences | Before/after harness is identical; exact-output tests are separate from timed code. Debug/synthetic/warm/unpinned/small-tail-sample limitations remain explicit |

Residual costs: source JSON is still read and decoded; leading or trailing whitespace may still require scanning to distinguish empty/exact-fit from truncated output. No constant-time guarantee is made for arbitrary whitespace-only input. The output allocation is bounded by the character limit, while input/database memory is unchanged. Rollback is the source diff; no database migration or cleanup is required.

## Closure and validation

| Check | Result |
| --- | --- |
| `cargo test -p gosling --lib session::last_message_snippet` | 13 passed; 1 manual benchmark ignored |
| `cargo test -p gosling --lib session::` on final source | 147 passed; 0 failed; 1 manual benchmark ignored |
| `cargo test -p gosling --lib list_sessions` | 2 passed (also covered in the session suite) |
| `cargo test -p gosling --lib benchmark_last_message_snippets -- --ignored --nocapture` | Passed before and after; same-workload outputs retained above |
| `cargo clippy -p gosling --all-targets -- -D warnings` | Passed after repairing the test-only import |
| `cargo fmt --all` and `cargo fmt --all -- --check` | Applied; final check passed |
| `git diff --check` and AGENTS governance-marker scan | Passed; required marker remains present |
| Architecture/contract comparison against the pre-repair source map | Conformant before and after; **no new drift**; no follow-up required for this patch |
| Patch-only follow-up audit | **0 remaining findings** after the Unicode fixture and import repairs |

Files changed: `crates/gosling/src/session/last_message_snippet.rs`, `.gitignore`, `docs/TODO.md`, and this session log. The ignore exception makes the required evidence trackable; it does not change runtime behavior.

Record reconciliation: this was a fresh finding, so no historical source report was rewritten. The canonical `docs/TODO.md` now carries REL-OPT-001 as closed with its evidence link. The active-only mirror gets no completed item. No stale TODO/FIXME/HACK/XXX marker names this defect in the changed module. Existing PERF-GSL-003 and all unrelated findings retain their prior status. No commit, merge, publication, or external tracker update was requested or performed.

## Validation limits

Live provider throughput, packaged Electron responsiveness, production workload frequencies, cold startup, and cross-platform behavior are not measured. Large repository files and Giles reports were sampled, not comprehensively audited. Local benchmark fixtures contain synthetic data in temporary directories; no user database is used.
