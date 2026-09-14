# Future plan: bounded in-session compaction provenance

Status: Value gate passed; implementation deferred until after the
host-enforced planning Workstreams A-D release

Decision date: 2026-09-13

Baseline: `main` at `bd064a5168fd4b26016b51ffd6f8d0f10e82dd6e`, with the
current staged host-enforced-planning worktree inspected for path accuracy

## Decision and release boundary

The diagnostic value gate in the originating plan's section 12.3 passes.
Criterion 1 is satisfied: a user or support engineer cannot determine which
source rows a successful in-session compaction consumed. Criterion 2 is partly
satisfied: the algorithm and its fixed bounds are visible in source, but the
filter level, byte budget, chunks, reduction rounds, and retry count selected by
a particular run are not persisted or exposed. The audit did not establish
criterion 3, and it is not needed to pass the gate.

This is a documentation-only decision. It does not add a migration, runtime
type, diagnostics field, telemetry, or release gate now. Workstreams A-D remain
the complete scope of the host-enforced-planning release. Workstream E may start
only after A-D are released or release-candidate stable, using the next schema
version available at that time.

ADR-0020 already classifies compaction provenance as a separate value-gated
follow-up, so this focused plan records the passed gate without widening the
ADR's accepted release scope.

## Current-state evidence

| Observation | Source evidence |
| --- | --- |
| In-session compaction determines a protected boundary and eligible message prefix, summarizes it, and returns only a replacement `Conversation` plus aggregate `ProviderUsage`. | `crates/gosling/src/context_mgmt/mod.rs:310-339`, `:394-488`, `:582-585` |
| Chunk packing, default retry behavior, byte budgets, transient payload fingerprints, reduction rounds, and tool-pair filter levels are in-memory implementation details. No per-run result carries them out of the compactor. | `crates/gosling/src/context_mgmt/mod.rs:924-983`, `:994-1085`, `:1106-1198` |
| Automatic and hard-limit recovery compaction receive only the replacement conversation; their visible events do not carry durable source or strategy provenance. | `crates/gosling/src/agents/agent/reply_entry.rs:584-631`, `crates/gosling/src/agents/agent/reply_stream.rs:181-280`, `:923-968` |
| Manual `/compact` follows the same return contract and reports only completion or a visible failure. | `crates/gosling/src/agents/execute_commands.rs:183-247` |
| A successful full-session compaction atomically replaces conversation rows and records current/accumulated usage and cost. The transaction has no compaction-run insert or source-run identity. | `crates/gosling/src/agents/reply_parts.rs:756-803`, `crates/gosling/src/session/session_manager/message_storage.rs:724-771`, `crates/gosling/src/session/session_manager.rs:1023-1042` |
| `session_summaries` has coverage and source-hash metadata, but it belongs to the separate rolling summarizer used for compacted resume rather than to each in-session compaction run. | `crates/gosling/src/session/session_manager.rs:227-239`, `crates/gosling/src/session/session_manager/summary_storage.rs:1-12`, `:50-87`, `:132-163`, `crates/gosling/src/context_mgmt/summarizer/mod.rs:422-509`, `:573-587` |
| Full diagnostics exports session state and bounded plan metadata, but `DiagnosticsReport` has no rolling-summary or in-session-compaction provenance field. | `crates/gosling/src/session/diagnostics.rs:81-97`, `:314-403`, `:485-499` |

The source-level reproduction is therefore any successful manual, automatic, or
hard-limit in-session compaction followed by a full diagnostics report: the
report can show the post-compaction session and usage totals, but cannot identify
the committed run's source range/hash or the strategy that produced it. No
runtime reproduction was performed for this documentation decision.

## Future v1 contract

Add one bounded, metadata-only `session_compaction_runs` row for each successful
compaction commit. The first implementation should use these contracts:

- Identity: run id, session id with delete cascade, trigger (`manual`, `auto`, or
  `hard_limit_recovery`), and an algorithm-version constant.
- Source binding: inclusive source start/end row ids, source row count, a
  domain-separated/versioned source hash, prior rolling-summary source hash and
  covered-through row when one contributed, and the protected-tail start row.
  These historical row ids cannot be foreign keys because conversation
  replacement removes their message rows.
- Strategy: selected tool-pair filter level, byte and token budgets, initial and
  final chunk counts, reduction-round count, and provider-attempt/retry count.
- Size and usage: bounded input/output byte counts and estimated token counts,
  plus the aggregate provider usage already returned by the compactor.
- Runtime identity: bounded provider and model labels.
- Result: a bounded status enum (`conversation_replaced` or
  `resume_context_only`), nullable bounded error-code enum, resulting summary
  SHA-256, resulting covered-through source row, and created/completed
  timestamps. Version 1 writes only successful rows, so its error code is null.
  Durable failed-attempt recording requires a separate privacy/value decision.

The source hash must use a new `session-compaction-source-v1` domain and the
repository's versioned length-prefixed SHA-256 convention over the exact included
row ids and canonical agent-visible blocks before lossy tool-pair filtering.
Store the rolling summary's existing source hash as an algorithm-tagged opaque
value rather than silently reinterpreting it as SHA-256. Hash the exact committed
summary bytes with SHA-256. `result_covered_through_row_id` names the last old
source row represented by that summary, not the newly inserted summary-message
row. Hashes are correlation identifiers, not proof that content is safe to
disclose.

Retain at most the latest 32 committed rows per session and prune older rows in
the same transaction as insertion. Use checked integer conversions and database
constraints for enums and non-negative counts. The limit is an internal constant,
not user configuration.

## Implementation order

1. Add an internal provenance carrier from session loading through compaction.
   Do not infer database row ids from `Conversation` vector positions. It must
   represent synthetic prior-summary context separately from persisted rows.
2. Refactor compaction to return a bounded metadata outcome alongside the
   replacement conversation and usage. Instrument actual filter, budget, chunk,
   reduction, and retry choices without retaining chunk bodies.
3. At compaction start, capture the source and prior-summary binding. Immediately
   before commit, atomically revalidate that binding; discard the generated
   replacement with a visible retryable error if the source changed.
4. In the existing `BEGIN IMMEDIATE` save path, atomically apply the conversation
   replacement or resume-only metrics, record usage, insert the run row, and prune
   retention. A rollback must leave all four effects absent. Planning correctness
   must not depend on the manifest or any summarizer/telemetry path.
5. Allocate the next available session-schema migration only after A-D stabilize.
   Compare fresh and migrated schemas; do not backfill invented history.
6. Add a read-only, full-diagnostics-only view capped to the newest 10 rows. Add
   no product telemetry until that view has shipped and demonstrated value.

The table is local diagnostic state in v1. Exclude it from normal session export,
import, copy, fork, share, and handoff; those operations must not transfer a
source-correlation ledger or any authority. A later transfer format would require
its own versioned privacy and identity-remapping contract.

## Privacy and acceptance gates

Never persist transcript or summary bodies, chain-of-thought, prompts, raw
credentials, full tool arguments/results, workspace paths, exception strings, or
duplicate raw content. Provider/model labels, row ids, timestamps, and hashes are
still potentially linkable metadata: expose them only in an explicit full
diagnostics request, apply the existing diagnostics redaction policy, and never
emit them to telemetry in v1.

Implementation is accepted only with focused tests for:

- manual, automatic, hard-limit-recovery, and resume-only successful rows;
- exact source/prior-summary/result hash fixtures and row-boundary accuracy;
- each filter/budget/chunk/reduction/retry path without content persistence;
- source mutation between summarization and commit, cancellation, provider
  failure, transaction failure, and crash rollback;
- atomic conversation/metrics/usage/run insertion and same-transaction pruning;
- fresh-versus-migrated schema equivalence, session-delete cascade, and no
  fabricated backfill;
- the 32-row storage bound, 10-row diagnostics bound, metadata-only serialization,
  redaction, and exclusion from telemetry and transfer formats.

Complexity/risk: medium. The primary risk is transaction or lineage drift that
associates a successful summary with the wrong source. The change is reversible
by hiding the diagnostics field and stopping new inserts while leaving the
additive table readable by newer binaries; do not drop user databases on
rollback.
