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

## Status

Audit checkpoint recorded. Repair and validation are pending.
