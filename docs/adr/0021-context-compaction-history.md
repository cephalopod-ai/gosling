# ADR-0021: Append-only context compaction history

Date: 2026-09-14
Status: implemented and locally validated
Related: ADR-0019, ADR-0020

## Context

Gosling preserves the transcript rows hidden by compaction, but its current
summary projection is overwritten. That makes it difficult to inspect what a
successful compaction generated, which source range it covered, and which
provider and prompt produced it. The transcript is already durable, so copying
the raw conversation into a second archive would waste space and expand the
sensitive-data surface.

LLM summaries also commonly rewrite most of their text. Physical delta chains
would make expiration, pinning, recovery, and corruption handling depend on
ancestors while providing uncertain savings for summaries bounded to roughly
12,000 characters.

## Decision

`sessions.db` owns an append-only `session_compaction_revisions` ledger. Every
successful manual or automatic compaction stores one independent, versioned
JSON payload containing the exact summary and the stable IDs that were covered.
Indexed columns record a never-reused per-session generation, parent revision,
trigger, durable or temporary effect, source coverage, provider and requested
and resolved models, provider usage, token estimates, lifecycle timestamps,
payload size, and BLAKE3 integrity hashes.

Source hashes use a domain-separated, length-prefixed stream of each complete
serialized message, including tool content and metadata. Summary and rendered
prompt hashes use separate domains. Hierarchical compaction records a hash over
the ordered hashes of every rendered prompt variant used by its bands.

The sibling `session_compaction_state` row retains the next generation and an
aggregate purge count, so deleted gaps remain visible without an unbounded
deletion log. Existing sessions are not backfilled because prior summaries and
their provenance cannot be reconstructed reliably.

Durable compaction commits the conversation rewrite, usage update, and ledger
append in one immediate transaction. A compacted-tail Desktop resume records a
`temporary` revision and usage in one transaction without replacing unloaded
history. Failed or cancelled provider work reaches neither commit path.

The version-one policy is one validated object:

```yaml
GOSLING_COMPACTION_HISTORY_POLICY:
  version: 1
  capture_enabled: true
  retention_days: 90
  purge_grace_days: 7
  max_revisions_per_session: 100
  max_total_bytes: 268435456
```

`retention_days: null` disables time expiry but not count or capacity limits.
Cleanup removes unpinned rows past grace, then applies the per-session count,
then the global payload-byte budget. Ordinary history replacement such as
`/clear` removes that session's ledger. Session deletion cascades through both
ledger tables. Archiving does not delete history.

## Safety boundary

This is local walkthrough and recovery history, not a compliance-grade or
tamper-proof audit log. Hashes detect corruption only while a trusted digest is
available; they are not signatures and no head is anchored outside the local
database. SQLite deletion is logical and backups, WAL files, and filesystem
snapshots are outside this feature's deletion guarantee.

Payloads contain model-generated summaries and are sensitive. Normal transcript
exports and sharing continue to exclude this table. A future explicit export
must warn the user and opt in. Telemetry must not include summary text.

## Consequences and rollout

Independent snapshots make any revision readable, expirable, or pinnable
without retaining an ancestor chain. The byte cost is bounded and measurable;
a versioned compression codec may be added if measurement justifies it.

The core migration, atomic capture, typed ACP operations, policy impact preview,
pin/delete/purge controls, CLI export/prune, and Desktop timeline and settings
ship together. List operations return metadata only; a client retrieves the
full summary and provenance for the selected generation, keeping routine
browsing bounded. Policy changes recalculate expiration for existing unpinned
rows and reject application when the reviewed database state has changed.

Desktop comparison is derived only after the user enables it. The client loads
the previous independent snapshot, computes a bounded word- or line-level diff,
and hides unchanged content; no derived diff is persisted. **View source
messages** resolves the snapshot's recorded message IDs through the existing
paged transcript interface only when requested. Missing rows are reported as
unavailable, and no raw message copy is added to the compaction ledger.

Normal transcript exports and sharing still exclude Context History. Explicit
CLI export requires a sensitive-data acknowledgement and writes owner-only files.
Secure physical page reclamation remains outside this decision and requires a
separate storage-level design.
