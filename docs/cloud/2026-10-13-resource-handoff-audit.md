# Resource lifecycle and handoff audit — 2026-10-13

## Intake and frozen findings

Target: `/Users/eric/Work/vscode/forked/gosling`, branch `main`, baseline
`77da40dda`; initially clean. The operator requested a comprehensive audit and
patch of excessive work, zombie processes, lost handoffs, deadends, redundancy,
logic, seams, and races. This is a broad source screening with deeper lifecycle
review; it is not exhaustive runtime certification of the repository.

Audit findings frozen before entering the separately authorized repair stage:

| ID | Severity | Source | Mechanism | Evidence basis |
| --- | --- | --- | --- | --- |
| F01 | High | developer/shell.rs:838 | Empty lines add zero to the byte budget, allowing unbounded queued line objects. | Source; potential exhaustion |
| F02 | Medium | developer/shell.rs:559 | Dropping run_command detaches its spawned pipe collector; inherited pipes can keep it alive after command cancellation. | Source; potential resource retention |
| F03 | High | summon/task_tracking.rs:249,292 | load removes the background task before waiting; cancellation drops its JoinHandle without cancelling/aborting the child, loses its handoff, and releases its concurrency slot. | Source; potential orphan execution |
| F04 | Medium | acp/outputRevisions.ts:80 | AbortSignal is checked only on admission; abandoned rows retain waiters and still cause backend work. | Source |
| F05 | Medium | acp/outputRevisions.ts:67 | Decrement-before-wake permits a newly arriving batch to take a queued waiter's slot before that waiter resumes. | Source; interleaving candidate |
| F06 | Medium | hooks/useRunStatus.ts:62 | Result checks fence the connection but not the prompt attempt; late checks can be applied to a replacement turn. | Source; interleaving candidate |
| F07 | Medium | acp/elicitationRequests.ts:92 | Session cancellation resolves elicitation promises without removing their pending user-input identifiers. | Source |
| F08 | Medium | acp/domain_adapter.rs:61,177,212 | Exit watcher waits only for future changes; an already-exited child can remain Ready. A late timeout can replace Crashed with Hung. | Source; interleaving candidate |
| F09 | Medium | acp/server/initialization.rs:135,157,227 | Repeated initialize starts duplicate notifiers; idle subscriber tasks retain connection handles without an owner that aborts them on teardown. | Source; potential retention |
| F10 | Medium | gosling-mcp/src/subprocess.rs:74 | Login-shell PATH discovery has no deadline and materializes all stdout; a stuck shell profile blocks all subsequent OnceLock users. | Source; potential stall |

Separate follow-ups: frontend results can be buffered without bounds; failed-turn
steering lacks durable reconciliation; background load removes tasks from peek
visibility while waiting; auxiliary tool summaries are detached model work;
hard process death and native Windows identity checks require platform drills.
No measured latency, memory-growth, or time-to-exhaustion claim is made.

## Live snapshot

`ps -axo pid,ppid,stat,%cpu,%mem,etime,comm` and a zombie-state filter returned
no zombie entries. The highest instantaneous CPU entry was Python PID 93015
(31.4%, parent 1, approximately four days uptime); its ownership was not
established, and no unrelated process was terminated. This snapshot cannot
establish long-term process health or attribute CPU usage to this checkout.

## Executive verdict and current dispositions

Nine of the ten frozen candidates have source repairs. F05 was rejected:
`flushBatch` is admitted through timer callbacks, and a resolved queued waiter
runs as a microtask before a new timer callback can take its slot. The apparent
counter window alone did not prove a reachable race. Its counter logic remains
unchanged. Two High findings concern possible unbounded capture and abandoned
subagent execution; exhaustion and external side effects were not observed.

The source repairs are qualified by regression coverage, not by a claim that
every native platform or installed build is verified. Findings retain their
original source-evidence confidence; passing post-patch tests does not pretend
that all manifestations were reproduced before patching. No performance
percentage, dominant CPU bottleneck, heap-growth rate, or exhaustion deadline
has been measured.

| Finding | Current disposition | Repair / boundary | Verification |
| --- | --- | --- | --- |
| F01 / REL-GSL-001 | Source repaired | 10 MiB plus 50,000-line capture bounds; account for newlines and decoded bytes; report aggregate and per-line truncation. Continue draining discarded output. | Blank-line flood, large-line/following-line and normal-output regressions |
| F02 / FSR-GSL-002 | Source repaired; native pipe-retention drill pending | Own collector with AbortOnDropHandle instead of detaching it on future drop. | Compile/clippy; existing shell process-group cleanup regression |
| F03 / FSR-GSL-003 | Source repaired; task teardown tested | BackgroundTask drop cancels/aborts the owned child; child also retains the semaphore permit until actual teardown. Aborted cancellation joins the handle. | Dropped-load regression observes child-future destruction, token cancellation and admission release; 50 summon tests |
| F04 / REL-GSL-004 | Verified Desktop repair | Abort removes just that waiter and its listener; empty/abandoned batches and cancelled later chunks make no backend call. Existing in-flight shared RPCs remain owned until settlement. | Before-flush, duplicate-waiter, in-flight and queued-abort regressions |
| F05 / CON-GSL-005 | Not Confirmed / rejected | Scheduling does not admit the proposed interleaving. No counter repair retained. | Six-session test observes at most four concurrent RPCs |
| F06 / TMP-GSL-006 | Verified Desktop repair | Both success and failure status updates fence prompt-attempt identity as well as connection generation and session lifecycle. | Replacement-turn regression, hook/controller suite |
| F07 / WFG-GSL-007 | Verified Desktop repair | Session cancellation removes pending elicitation user-input identifiers while resolving only that session's requests. | Session-scoped cancellation and store/controller regressions |
| F08 / STT-GSL-008 | Source repaired; controlled status paths tested | Inspect current watch value before awaiting changes; own the watcher; atomically prevent a late timeout replacing Crashed with Hung. | Already-exited watch and terminal-state regression; 12 adapter tests including real disposable child exit |
| F09 / REL-GSL-009 | Source repaired; real in-process ACP connection tested | One OnceCell-owned notifier set per connection; dropping the connection aborts idle subscribers. | Three initialize requests start one notifier; teardown observes it finishing |
| F10 / FSR-GSL-010 | Source repaired; controlled Unix probes tested | Both MCP and core login PATH capture use one five-second deadline for stdout and process exit, 64 KiB capture, and kill/reap on failure. | Normal profile, failed exit, oversized output and stdout-closed/hung-child cases |

## Scope, authority and effort limits

Repository source inventory: 1,304 Rust/TypeScript/TSX files under `crates`,
`ui/desktop/src` and `ui/text/src`; 201 contain lifecycle acquisition patterns
(`spawn`, blocking tasks, commands, timers or unbounded channels). These are
search counts, not a claim that 201 files were deeply read. Deep review focused
on shell/MCP/provider children, background delegation, ACP initialization,
active-run gates, result/approval routing, Desktop lifecycle/refresh and output
history. Other files were screened by searches or remain Not Reviewed.

The catalog selected all 13 required lenses and 31 standalone candidates.
They were loaded for applicability; this report distinguishes focused application
from deferred deep audits below. Review budget was mechanism-driven: roughly
40 source/document surfaces deeply inspected, then targeted regression and
transitive review of the changed workflows. Lower-priority full-pack inventories
remain explicitly Not Reviewed. A single agent performed this pass; there was
no independent-review/consensus claim.

Governing sources: AGENTS.md and GEMINI.md; README and docs/INDEX.md;
docs/architecture.md, shell foundation/productization contracts, ADR-0019 and
ADR-0020; CONTRIBUTING.md; advisory .giles YAML; current validation ledger and
the October 12/13 audit/repair session records. The prior records supplied
candidate context, not proof that this checkout or installed application was
already validated. No credentials were read, accounts changed, installed
application restarted, or unrelated process terminated.

The operator explicitly approved targeted Rust tests and clippy. The working
tree advanced externally to `05259746c` during repair, containing the earlier
patch group. That commit was preserved; this run did not perform the commit.
Remaining source/document changes are local.

## Surface inventory, ownership and boundary map

| Surface / actor | Producer → consumer | Ownership and intended lifetime | Failure boundary / review result |
| --- | --- | --- | --- |
| ACP foreground turn | Prompt → operation gate/local lease → agent stream → tools → ledger/client | Run owns cancellation and terminal bookkeeping | Active-run gate registers waiters before checking state; inspected, existing tests retained |
| Shell execution | Command → stdout/stderr collector → bounded lines → rendered/file output | Child/process-group guard and collector live with command future | F01/F02; cancellation now owns collector; group cleanup retained |
| Async delegate | Session + permit → child agent → registry → load → parent result | Registry or active loader owns BackgroundTask; child holds admission through teardown | F03; abandoned loader stops child; waiting task still temporarily absent from peek registry (follow-up) |
| MCP operator adapter | Handshake → descriptor → tool request/result → status/client | OperatorChildGuard supervises process; adapter owns status watcher | F08; configured frame/time bounds retained; crash outranks timeout |
| ACP notifications | Initialize → plan/domain subscription → typed client notification | Exactly one owned set per connection | F09; repeated init and idle teardown exercised |
| Output history row | AbortSignal + path → per-session batch → authorized revision RPC → row promise | Each row owns a removable waiter; batch owns shared RPC | F04; 500 paths per RPC, four active batches, per-path failure semantics retained |
| Run heartbeat | Snapshot → status RPC → hook state | Session + generation + attempt fence; hook owns intervals | F06; late prior-turn updates ignored |
| Elicitation | Backend form → pending promise/input ID → submit/cancel | Session owns request, timer and input ID | F07; submit, expiry and cancellation now clear the same input bookkeeping |
| Login PATH | Shell profile → bounded stdout + terminal status → cached PATH | Probe owns reader/process group; OnceLock/LoginPath owns result | F10; both reader and exit share deadline, overflow fails to inherited-PATH fallback |
| Backend registry | Electron windows → serialized/process-locked registry → startup sweep | Atomic temp file, fsync/rename and cross-process file lock | Inspected current code; historical unguarded-writer claim no longer applies |

Representative pipeline branches were traced for success, early error, timeout,
future drop, cancellation, replayed initialization, already-known exit and late
status delivery. No randomized whole-application replay was performed.

## Detailed finding evidence and operational assessment

Original evidence below refers to baseline `77da40dda` unless identified as a
post-patch test. Local F IDs remain the frozen-set identities; domain IDs above
provide catalog mapping.

### REL-GSL-001 / F01 — Capture accounting did not bind empty output

Severity: High. Confidence: Likely. Evidence basis: source-evidenced.
Domain: Reliability. Evidence: developer/shell.rs, `collect_tagged_lines`:
`collected_bytes += line.len();` before each unbounded-channel send.
Expected boundary: output objects as well as text must have a bound.
Failure mechanism: newline-only input produces zero-byte lines indefinitely.
Break-it angle: blank-line flood and a large UTF-8/unterminated line.
Impact: potential backend memory exhaustion; no OOM was observed.
Operational impact: Service / process / reversible / silent / unknown rerun safety.
Mitigation: byte-and-count admission, continued drain and explicit partial-output
metadata. Assessment: local_guardrail, S, codex; one collector/result path and
behavioral regressions. Validation: blank/normal/large-line tests plus shell suite.
Non-goal: replacing existing output spill files or recovery semantics.

### FSR-GSL-002 / F02 — Collector ownership did not follow command cancellation

Severity: Medium. Confidence: Likely. Evidence basis: source-evidenced.
Domain: Failsafe. Evidence: developer/shell.rs, `run_command`:
`tokio::spawn(collect_tagged_lines(child_stdout, child_stderr, tx))` retained
a plain JoinHandle. Dropping it detached rather than aborted the collector.
Expected boundary: command-future teardown releases its pipe reader.
Break-it angle: cancellation while an inherited pipe remains open.
Impact: potential detached reader retention; ordinary process-group termination
can close those pipes, so persistent leakage was not asserted from source alone.
Operational impact: Workflow / process / reversible / silent / unknown.
Mitigation: abort-on-drop ownership. Assessment: local_guardrail, S, codex.
Validation: compilation/clippy and existing process cleanup; escaped-descendant
pipe retention is not reproduced. Resilience: withstand/constrain/fail_visible.
Failure phase: cancellation; local effect retained reader; detection test/source;
no independent runtime resource census. Non-goal: killing unrelated descendants.

### FSR-GSL-003 / F03 — Abandoned load detached delegated execution

Severity: High. Confidence: Likely. Evidence basis: source-evidenced.
Domain: Failsafe. Evidence: summon/task_tracking.rs, `handle_load_task_result`:
`let mut task = running.remove(task_id).unwrap();` precedes the long wait.
Expected boundary: the loader that temporarily owns a task must stop it on drop.
Failure mechanism: plain JoinHandle drop loses execution ownership and its result;
the permit previously lived only in the removed task object.
Break-it angle: drop the load future while the child is pending.
Impact: potential invisible side effects and overlapping newly admitted tasks.
Operational impact: Workflow / process / compensatable / silent / unsafe.
Mitigation: cancel/abort on task drop; retain permit in child until destruction;
join after forced abort. Assessment: workflow_protocol, S, codex.
Validation: forced pending-child teardown and successful/cancelled load tests.
Resilience: withstand/constrain/fail_visible. Failure phase cancellation;
detection by task/permit regression; external model/tool side effects not exercised.
Non-goal: durable background-task resume or changing timeout reinsertion.

### REL-GSL-004 / F04 — Abandoned rows retained history work

Severity: Medium. Confidence: Confirmed (static omission). Evidence basis:
source-evidenced. Domain: Reliability. Evidence: acp/outputRevisions.ts,
`getLatestOutputRevision`: `signal.throwIfAborted();` was the only signal use.
Expected boundary: cancellation after admission removes the row's waiter.
Failure mechanism: a promise and its result consumer survived the row's lifetime.
Break-it angle: abort before flush, while queued, in flight, or beside a live
duplicate. Impact: redundant backend requests and retained row closures.
Operational impact: Workflow / network / reversible / silent / safe.
Mitigation: per-waiter abort listener/removal; skip empty chunks/batches;
settlement removes listeners. Assessment: local_guardrail, S, codex.
Validation: cancellation and duplicate/scope regressions; full Desktop suite.
Non-goal: retrying or externally cancelling a shared in-flight ACP RPC.

### TMP-GSL-006 / F06 — Late heartbeat crossed prompt attempts

Severity: Medium. Confidence: Likely. Evidence basis: source-evidenced.
Domain: Temporal. Evidence: hooks/useRunStatus.ts, `checkNow`: updates originally
checked `snapshotRef.current?.connectionGeneration === generation` without
checking captured `promptAttemptId`. Expected boundary: result belongs to the
same active turn. Break-it angle: a replacement attempt on the same connection
before the old response arrives. Impact: misleading error/health display.
Operational impact: Workflow / user-visible / reversible / UI-visible / safe.
Mitigation: attempt fence in success/error paths. Assessment: local_guardrail,
XS, codex. Validation: late-result replacement-turn regression and hook/controller
tests. Non-goal: replaying timed-out checks or changing the reconciliation period.

### WFG-GSL-007 / F07 — Elicitation cancellation left an input deadend

Severity: Medium. Confidence: Confirmed (static mismatch). Evidence basis:
source-evidenced. Domain: Workflow-GUI. Evidence: elicitationRequests.ts,
`cancelAcpElicitationRequestsForSession` called
`pending.resolve(cancelledElicitationResponse());` without the
`resolveUserInputRequest` used by submit and expiry. Expected boundary:
resolution clears the paired waiting-input identifier.
Break-it angle: cancel one of two sessions, then finish another input request.
Impact: stale pending bookkeeping can keep active UI input state waiting.
Operational impact: Workflow / user-visible / reversible / UI-visible / safe.
Mitigation: use the same input-ID resolution on cancellation. Assessment:
local_guardrail, XS, codex. Validation: scoped cancellation and chat store/controller
tests. Non-goal: broad approval behavior changes.

### STT-GSL-008 / F08 — Adapter status depended on future notifications

Severity: Medium. Confidence: Likely. Evidence basis: source-evidenced.
Domain: State-Transition. Evidence: acp/domain_adapter.rs:
`while process_exit.changed().await.is_ok()` preceded the first value read;
timeouts used `status.send_replace(DomainAdapterStatus::Hung)` unconditionally.
Expected boundary: already-known exit is consumed and crash is terminal for this
adapter instance. Break-it angle: subscribe after exit; timeout after crash.
Impact: Ready/Hung can conceal a dead adapter.
Operational impact: Workflow / user-visible / reversible / UI-visible / unknown.
Mitigation: current-value read, atomic terminal-state guard and owned watcher.
Assessment: local_guardrail, S, codex. Validation: forced watch/state ordering and
adapter process regressions. Non-goal: a new reconnect policy.

### REL-GSL-009 / F09 — Notification workers lacked connection lifetime ownership

Severity: Medium. Confidence: Likely. Evidence basis: source-evidenced.
Domain: Reliability. Evidence: initialization.rs called
`self.spawn_plan_update_notifier();` on every successful initialize and discarded
the spawned handle. Expected boundary: one subscription set per connection,
released even if no subsequent update occurs.
Break-it angle: repeated initialization, then disconnect while subscribers idle.
Impact: redundant notifications and possible retained connection resources.
Operational impact: Workflow / network / reversible / silent / safe.
Mitigation: OnceCell-owned abort-on-drop task set. Assessment: local_guardrail,
S, codex. Validation: actual in-process ACP handshake and teardown regression.
Non-goal: notification payload or capability changes.

### FSR-GSL-010 / F10 — Login PATH probes could stall after stdout handling

Severity: Medium. Confidence: Likely. Evidence basis: source-evidenced.
Domain: Failsafe. Evidence: gosling-mcp/src/subprocess.rs used
`child.wait_with_output().ok()?`; core shell.rs used a five-second receive timeout
followed by `child.wait()` with no remaining deadline. Both readers had no total
output bound. Expected boundary: profile output and exit share a bounded probe.
Break-it angle: close stdout, then sleep; flood stdout; nonzero exit after a PATH.
Impact: initialization/first shell use can stall and consume memory.
Operational impact: Workflow / process / reversible / silent / unknown.
Mitigation: bounded reader, one deadline, kill/reap and inherited-PATH fallback.
Assessment: local_guardrail, S, codex. Validation: controlled Unix child probes.
Resilience: withstand/constrain/fail_degraded; startup failure detection is bounded
timeout; no live profile edits performed. Non-goal: unify differing shell-selection
policies across crates or change supported shell argument syntax.

## Non-findings and important held boundaries

- Backend registry writes now use `withProcessFileLock`, unique temporary paths,
  file sync and rename. The prior single-process-only writer record is historical.
- Active prompt/provider-transition gates register `Notify` waiters before reading
  state, preserving wakeups; provider switches reserve the operation boundary.
- The agent limits turns, repeated-tool denials, grind nudges and midstream retries.
  Retry after executed tools is excluded; no blanket retry was added here.
- Summon admission uses semaphore `try_acquire_owned`; child permits now remain
  live until actual task teardown. Completed results retain the existing TTL.
- Output history already coalesced per-session requests and used 500-path slices;
  no one-RPC-per-row replacement was needed. Per-path omitted authorization
  results remain errors, not fabricated empty history.
- Hub's clock updates at minute boundaries and pauses when hidden. Navigation
  session refreshes are event-driven/coalesced. Timer cleanup exists in reviewed
  hooks; this is not a claim that every UI timer was profiled.
- `unimplemented!` search hits were test providers or unsupported-OS dispatch,
  not a proven supported-platform production deadend. No code was deleted based
  on an unreferenced-symbol search; dynamic extension/provider discovery matters.
- `.giles` status files are advisory historical evidence. They do not certify
  current fleet compliance or override AGENTS.md.

## Polling, retention and optimization assessment

| Work | Bound / frequency | Assessment |
| --- | --- | --- |
| Shell retained lines | 50,000 entries + one marker; decoded text/newlines within 10 MiB | F01 binding count/byte limits; total RSS includes additional rendered copies and allocator overhead |
| Login PATH capture | 64 KiB + one overflow byte, five-second deadline | F10; once/cached per process, failure falls back to inherited PATH |
| Summon concurrency | Configured semaphore, default five | F03 guards admission through child teardown; no external model cost measured |
| Notification buffer | Latest 256 messages | Count bound exists; message-size/fan-out amplification not exhaustively measured |
| Completed tasks | Default 600-second TTL, opportunistic cleanup | TTL alone does not bound a burst; retained-result size/soak measurement remains a candidate |
| Frontend result router | pending_results HashMap, no observed cap | Residual risk: orphan/unknown IDs can retain results; ownership contract needed before evicting valid handoffs |
| Output history | Four shared RPC batches, 500 paths per chunk | Cancellation regression prevents queued abandoned rows from requesting data; in-flight shared RPCs are not duplicated |
| Status heartbeat | 900-second backend check, 10-second local clock while monitoring | At most 96 reconciliation checks/day per continuously active session, plus manual/wake checks. Actual change rate unknown; no invented waste ratio |
| Hub clock | One visible minute-boundary update | At most 1,440 updates/day if visible continuously; calendar data is not refetched |
| Streaming checkpoints | At most every 250 ms while streaming, whole growing message serialized | Potential cost candidate; preserve durability until a representative large-session profile establishes avoidable share |

The concrete work reduction tested here is removal of abandoned output-history
RPCs before admission (zero backend calls in the cancellation case) and duplicate
notifier admission (one task after three initializations). Those counts do not
prove an application-wide latency/CPU improvement. Amdahl share and speedup are
unknown, so no projected percentage is published. Heap/RSS decomposition,
post-GC retention slope and time-to-OOM cannot be calculated without a defined
workload/heap limit and a three-point census.

## Skill escalation

| Findings | Primary | Secondary | Reason |
| --- | --- | --- | --- |
| F01 | Reliability | Memory, input/output, negative space | Blank-line cardinality and truthful partial output |
| F02/F03/F09/F10 | Resource/failsafe | Concurrency, state, recovery | Ownership must survive cancellation and terminal transitions |
| F04 | Reliability | Memory, optimization, concurrency | Abandoned consumers and shared request ownership |
| F06/F08 | Temporal/state | Workflow truth, concurrency | Old or terminal status must not become current health |
| F07 | Workflow truth | State, invariant synchronization | Every resolution path clears the same pending-input fact |

## Validation and reviewed snapshot

Final command results are recorded in the linked session log; intermediate
failures are retained there. Commands use the repo's Hermit Rust toolchain and
Desktop's installed pnpm dependencies. Cargo tests/clippy use
`--no-default-features`, so optional AWS/telemetry/TLS feature combinations are
not certified. Desktop validation runs against source, not the installed bundle.

176 targeted Rust tests and 1,675 Desktop tests passed. Scoped all-target Clippy,
Desktop typecheck/ESLint/Prettier and Rust formatting passed. Two pre-existing
validation fixture defects were corrected: the permission-log cloned slice and
the ACP LRU test's nonexistent session IDs (already listed as a historical
failure in the October 10 ledger). The latter was reproduced in isolation.
No deployed or installed repair claim is made.

## Residual risk and unreviewed frontier

1. Failed-turn steering durability is a protected prior follow-up. Define
   accepted/delivered/rejected persistence and replay without duplicating effects.
2. Background tasks removed for a waiting load are temporarily invisible to
   concurrent peek/cancel. The new drop guard stops abandoned work, but durable
   result recovery and cancellation/retry handoff require a separate protocol.
3. FrontendToolResultRouter buffers early/late/unknown results without a byte/count
   bound. Trace trusted request admission before choosing an eviction policy that
   cannot drop valid early-delivered results.
4. Auxiliary tool summaries/session naming remain detached model work in reviewed
   paths. Measure per-run request counts and assign cancellation ownership without
   silently changing persisted summary policy.
5. Large-session clones and whole-message checkpoint serialization require a
   repeatable profile. Removing checkpoints to save work would weaken recovery.
6. Native Windows PID identity, parent SIGKILL, escaped process groups, Flatpak host
   cleanup, long-running heap/fd/disk soak, and crash-resume drills are not verified.
7. Full provider wire conformance, dependency/supply-chain scans, CI economics,
   database migration/backup/export round trips and native GUI playtesting were
   outside the deep-review budget. No claim that those surfaces are clean.

Live follow-up identified PID 93015 as `python -m muninn_phase1.cli`. Its later
snapshot showed 0% CPU, approximately 3.2% memory, and four days uptime; Gosling's
installed backend/UI showed 0% CPU at that sample. This disproves a claim of
continuous runaway CPU from the first snapshot. Parent PID 1 alone is not proof
of an unwanted orphan. No zombie was observed in either process census.
One installed-backend lsof census found four IPv4, six Unix sockets and three
kqueue descriptors, among other handles; without a workload/idle series this
does not establish a leak or plateau.

## Final confidence

High for the tested local guardrails; medium for the lifecycle screening as a
whole. Overall status remains `completed_with_partial_verification`: broad
source screening and focused repairs, with the frontier above explicitly open.


## Lens taxonomy coverage and standalone triage

Every code not named as checked below is explicitly listed as Not Reviewed under
this pass's lifecycle-focused deep-read budget. Loading a skill is not evidence
that all of its checks were completed. Security was consulted to constrain the
patch (no authority widening); its independent security taxonomy was deferred.

| Lens | Checked items / evidence | Not Reviewed codes |
| --- | --- | --- |
| audit-architecture-seam | ARC-019: F09 initialization ownership; ARC-017: F09 typed ACP notifications; payload preserved; ARC-022: Held: core/MCP shell policies differ; no consolidation justified | ARC-001, ARC-002, ARC-003, ARC-004, ARC-005, ARC-006, ARC-007, ARC-008, ARC-009, ARC-010, ARC-011, ARC-012, ARC-013, ARC-014, ARC-015, ARC-016, ARC-018, ARC-020, ARC-021, ARC-023, ARC-024, ARC-025 |
| audit-dataflow-cascade | CAS-007: F01 containment; CAS-005: F08 status classification; CAS-013: F06 stale status; CAS-010: Held: output-batch failures keep existing per-path/whole-request semantics | CAS-001, CAS-002, CAS-003, CAS-004, CAS-006, CAS-008, CAS-009, CAS-011, CAS-012, CAS-014, CAS-015 |
| audit-compliance-posture | CMP-005: Held: .giles metadata advisory under AGENTS.md; CMP-015: Historical July .giles records; not treated as current validation; CMP-003: Source versus installed-build evidence kept separate | CMP-001, CMP-002, CMP-004, CMP-006, CMP-007, CMP-008, CMP-009, CMP-010, CMP-011, CMP-012, CMP-013, CMP-014 |
| audit-dataflow-concurrency | CON-001: F06/F08; source interleavings and post-patch guard regressions; CON-007: F06 attempt fence; CON-008: F08 subscribe/current-value ordering; CON-015: Held: SessionOperationGate registers Notify waiter before state check; CON-013: Held: backend registry file lock + fsync/rename; CON-005: F05 rejected; event-loop scheduling prevents proposed slot interleave | CON-002, CON-003, CON-004, CON-006, CON-009, CON-010, CON-011, CON-012, CON-014, CON-016, CON-017, CON-018 |
| audit-dataflow-integrity | DAT-010: F06 current-status integrity; DAT-003: Residual: unknown/late frontend results retain buffered objects | DAT-001, DAT-002, DAT-004, DAT-005, DAT-006, DAT-007, DAT-008, DAT-009, DAT-011, DAT-012, DAT-013, DAT-014, DAT-015 |
| audit-dataflow-input-output | IOP-012: F01 partial-output signal; IOP-013: F01/F10 input/capture bounds | IOP-001, IOP-002, IOP-003, IOP-004, IOP-005, IOP-006, IOP-007, IOP-008, IOP-009, IOP-010, IOP-011, IOP-014, IOP-015 |
| audit-invariant-sync | INV-016: F01 truncation flag; F07 paired input identifier; INV-008: F07 cancellation omitted paired resolution | INV-001, INV-002, INV-003, INV-004, INV-005, INV-006, INV-007, INV-009, INV-010, INV-011, INV-012, INV-013, INV-014, INV-015 |
| audit-negative-space | NEG-005: F01 empty-line accounting; F10 stdout EOF does not imply exit; NEG-006: F08 already-known exit; NEG-002: F02/F03/F09 hidden workers; NEG-008: New blank-line, cancellation, stale-turn and notifier regressions | NEG-001, NEG-003, NEG-004, NEG-007, NEG-009, NEG-010, NEG-011, NEG-012, NEG-013, NEG-014, NEG-015 |
| audit-reliability | REL-005: F01 potential exhaustion; no OOM observed; REL-006: F10 missing exit deadline; REL-012: F02/F03/F09 cleanup ownership; REL-013: F01/F10 capture bounds; REL-011: F01 truthful partial-output metadata; REL-002: F08 incorrect Ready state | REL-001, REL-003, REL-004, REL-007, REL-008, REL-009, REL-010, REL-014, REL-015 |
| audit-security | Constraint review only; no independent security verdict | SEC-001, SEC-002, SEC-003, SEC-004, SEC-005, SEC-006, SEC-007, SEC-008, SEC-009, SEC-010, SEC-011, SEC-012, SEC-013, SEC-014, SEC-015 |
| audit-dataflow-state-transition | STT-001: F08 crash must not regress to Hung; STT-005: F08 readiness ambiguity; STT-010: F09 idempotent initialization; STT-004: F07 residual pending-input state | STT-002, STT-003, STT-006, STT-007, STT-008, STT-009, STT-011, STT-012 |
| audit-dataflow-temporal | TMP-005: F08 watch/current-value ordering; TMP-010: F06 attempt invalidation; TMP-011: F06 old result versus replacement turn; TMP-014: F02/F09 teardown | TMP-001, TMP-002, TMP-003, TMP-004, TMP-006, TMP-007, TMP-008, TMP-009, TMP-012, TMP-013, TMP-015 |
| audit-workflow-gui | WFG-004: F06/F08 stale health; WFG-005: F07 pending-input deadend; WFG-008: F08 incorrect adapter state; WFG-009: F01 truncation metadata | WFG-001, WFG-002, WFG-003, WFG-006, WFG-007, WFG-010, WFG-011, WFG-012, WFG-013, WFG-014, WFG-015 |

| Standalone | Disposition | Basis / limits |
| --- | --- | --- |
| audit-architecture-drift | Deferred | No full invariant registry evaluation; change-set contract gate only |
| audit-security-code | Deferred | Independent authorization/injection/database security deep audit not executed |
| audit-security-owasp | Deferred | No complete OWASP/ASVS category verification |
| audit-contract-crossrepo | Not applicable to selected target | Only this repository was authorized/inspected; no external producer checkout/version pair selected |
| audit-deadcode-cleanup | Screened; deep audit deferred | Entry-point/source-stub searches, no complete reachability proof and no deletion |
| audit-pipeline-externalapi | Deferred | No authenticated external-provider workload or current wire-conformance sweep |
| audit-dependency-criticality | Screened; deep audit deferred | F10 PATH failure fallback traced; complete dependency/SPF map not performed |
| audit-failsafe-readiness | Applied to selected workflows | F02/F03/F08/F09/F10 cancellation, shutdown, timeouts and safe fallback |
| audit-contract-internalapi | Screened; deep audit deferred | Typed ACP result/status seams retained; complete internal API inventory not performed |
| audit-security-llm | Deferred | No independent injection/agency/exfiltration campaign; existing approval gates preserved |
| audit-architecture-nodejs | Screened; deep audit deferred | Batch/hook lifecycle and main/renderer ownership traced; complete module/workspace graph deferred |
| audit-security-nodejs | Deferred | No complete Node attack-class/dependency scan |
| audit-optimization-opportunities | Applied to recurring work map | Cancellation, duplicate notifier work, batching and checkpoint candidates |
| audit-testsuite-economics | Deferred | Full Desktop wall time recorded, but no cost/value or per-test-tier campaign |
| audit-llm-config-drift | Deferred | No live vendor catalog refresh in this pass |
| audit-performance-profile | Deferred | No representative workload, latency percentile baseline or hotspot profile |
| audit-memory-lifecycle | Applied to selected containers | Capture/queue/closure ownership; source risks, no heap census |
| audit-resource-lifecycle | Applied to selected resources | Owned children, collectors, notifiers, timers and two host process snapshots |
| audit-operator-signal | Screened; deep audit deferred | F01/F06/F07/F08 truthful partial output/health; no complete signal scorecard |
| audit-dataflow-pipeline-graph | Applied statically; replay deferred | Representative prompt, delegate, history and initialize branches; no randomized full graph execution |
| audit-recovery-idempotency | Applied to selected boundaries | Idempotent notifier startup, terminal status and cancellation/admission; no storage-crash drill |
| audit-security-repo-posture | Deferred | No fresh CI/supply-chain/remote branch-policy audit |
| audit-security-repo-triage | Deferred | No independent repository exploit-path triage |
| audit-security-vuln-harness | Deferred | No exploit-focused multi-reviewer harness or independent corroboration requested/executed |
| audit-design-webapp | Deferred | Source status-flow review, no rendered design/accessibility audit |
| audit-design-handheld | Not applicable | Selected interfaces are Electron desktop and terminal; no handheld/touch game target |
| audit-playtest-app | Deferred | Installed app was not restarted or exercised through native UI workflows |
| audit-mcp-server | Applied to lifecycle slice | Child supervision, bounded frames/PATH and adapter status; full MCP protocol/security taxonomy deferred |
| audit-agent-orchestration-code | Applied to lifecycle/handoff slice | Turn gates, delegated-task ownership, status and output handoffs; provider cost/authority/replay campaign deferred |
| audit-repo-state-reconciliation | Applied to local change/evidence reconciliation | Initial clean tree, external commit 05259746c, historical audit records and current finding dispositions |
| audit-repo-path-consistency | Screened; deep audit deferred | Hermit toolchain resolves current checkout; complete relocation/symlink provenance audit not performed |

Catalog repository exclusions honored: audit-equation-sourcebase, audit-security-supabase,
audit-flutter-ios, audit-graphdb-design and audit-go-repo-hardening. No independent
multi-agent consensus overlay was requested or used.

## Optimization taxonomy coverage

| Code | Disposition | Evidence / measurement needed |
| --- | --- | --- |
| OPT-001 | Held | Default Unix shell PATH scan is cached (shell.rs DEFAULT_SHELL); no measured recomputation hotspot |
| OPT-002 | Held | Output history already batches 500 paths/RPC; regression preserves it |
| OPT-003 | Not Reviewed | No full render/invalidation dependency profile |
| OPT-004 | Held | Four output batches and independent tasks already concurrent; no new parallelism justified |
| OPT-005 | Candidate | Whole-message checkpoint clone/serialization; requires large-session profile |
| OPT-006 | Finding | F01/F10 oversized capture; bounded retention added |
| OPT-007 | Held | Login PATH cached; per-session MCP ownership isolation deliberately retained |
| OPT-008 | Held | LoginPath/OnceLock retain stable PATH; no speculative new cache added |
| OPT-009 | Candidate | Active-monitor 10-second local clock remains while hidden; backend reconciliation is only every 900 seconds |
| OPT-010 | Candidate | Whole-message streaming checkpoint churn; recovery consumer forbids deleting unmeasured work |
| OPT-011 | Finding | F04 abandoned history calls and F09 duplicate notifiers; auxiliary LLM summaries remain a measurement follow-up |
| OPT-012 | Not Reviewed | No utilization/SLO/seasonality workload |
| OPT-013 | Not Reviewed | No complete CI DAG/timing analysis |
| OPT-014 | Held | Targeted Rust checks plus retained full Desktop gate; no authoritative gate removed |
| OPT-015 | Not Reviewed | No operator frequency/time/rework evidence |
| OPT-016 | Finding | F07 submit/expiry/cancel copied pending-input resolution diverged |
| OPT-017 | Candidate | Core/MCP PATH capture mechanisms share bounds, but differing shell selection/dependency ownership prevents casual cross-crate consolidation |
| OPT-018 | Held | ACP/domain/provider seams carry lifecycle and authorization contracts; no removal justified |
| OPT-019 | Not Reviewed | No transitive install/bundle-cost measurement |
| OPT-020 | Held | Stub search hits were test doubles/unsupported OS; no supported-path dead code proven |
