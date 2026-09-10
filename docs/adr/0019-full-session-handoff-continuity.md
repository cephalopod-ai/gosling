# ADR-0019: Full session handoff continuity

Date: 2026-09-09
Status: accepted

## Decision

gosling uses its persisted session ledger as the source of truth when a live session changes
provider or model. Before a transition, core builds a versioned, deterministic
`SessionHandoffSnapshotV1`, redacts it, bounds it to the smaller of 16,000 tokens or 10% of the
target context window, and stores it in the schema-v33 `session_handoff_snapshots` table. Snapshot
coverage includes the last covered row, source hash, summary state, estimated size, redactions, and
truncations so the user can inspect what will cross the boundary.

One capability contract selects delivery without provider-name checks. A target can declare
gosling-, provider-, or hybrid-owned context plus native resume, history import, in-place model
change, session fork, bootstrap, and bootstrap-acknowledgement support. Delivery is classified as
seamless resume, summarized handoff, or new context only. No built-in provider currently claims a
native resume or history-import implementation; those hooks exist for adapters that can return a
provider session identity or acknowledgement.

Provider and model changes use one core transition operation. It prepares a generation, retains the
old live provider, initializes and validates the candidate, performs any supported native or
bootstrap delivery, and then commits provider configuration, snapshot activation, and the context
boundary in one SQLite transaction. The in-memory provider changes only after that commit. A stale
generation, changed message ledger, target failure, or commit failure marks the generation failed
and leaves the prior provider and stored configuration in place.

At commit, covered messages remain user-visible but become agent-invisible. A single hidden,
agent-visible checkpoint becomes the new context root, except for explicitly confirmed new-context
delivery. This prevents later turns and application restarts from replaying the old raw transcript.
Restoring a provider-owned session creates a fresh `SessionResume` checkpoint; ordinary
gosling-managed API sessions can continue from their ledger directly. The ACP compatibility path is
also redacted and capped, but normal switches and resumes use the core checkpoint.

## Safety boundary

The checkpoint never stores chain-of-thought, raw image bytes, raw tool arguments in its recent
tail, authorization headers, cookies, tokens, API keys, passwords, private keys, or secret URL query
values. Historical tool output is quoted as untrusted context. Completed work requires successful
ledger evidence. Active and interrupted operations are recorded as non-retryable, and pending
approvals never transfer as approvals. A new-context-only target requires explicit confirmation;
automatic session restoration fails closed instead of silently discarding continuity.

The target's first response acknowledges the objective, current state, and next safe action. For a
provider-managed bootstrap, gosling makes a bounded acknowledgement request before committing a
live switch. Native delivery must return a provider session identity or acknowledgement; when the
adapter also supports bootstrap, native failure falls back to that bounded path.

## User and API surface

Desktop obtains capability data from the server, previews the exact non-persisted checkpoint, and
shows continuity class, delivery strategy, row/message coverage, summary state, operation and
approval counts, and redaction/truncation counts before confirmation. It submits one atomic request
and reports that the previous provider remains active on failure. Stored checkpoints are available
from the session actions menu, and eligible provider errors offer **Continue with another model
using session checkpoint**.

The typed ACP methods are:

- `_gosling/unstable/session/handoff/checkpoint/preview`
- `_gosling/unstable/session/provider/transition`
- `_gosling/unstable/session/handoff/checkpoint/read`
- `_gosling/unstable/session/handoff` for a new checkpoint-backed session

## Consequences

Cross-provider continuity is deliberately a bounded reconstruction, not a promise to reproduce
provider-private state. Redaction and truncation can omit detail, so the preview makes those losses
visible. Provider-managed bootstrap can consume an extra model request and cannot prove that a
third-party agent followed its acknowledgement instruction. Keeping old messages user-visible also
retains their local storage cost. Snapshot retention therefore keeps the active generation and a
small configurable history (five generations by default) while pruning older terminal generations.

Transient handoff state remains in gosling's private session database. It is not written to project
instructions or exported with normal session export. Crash recovery policy is a separate Desktop
setting: it controls whether an interrupted task starts another turn, while this ADR controls what
context a replacement provider receives.
