# ADR-0022: Provenance-safe Recall Brief reporting

Date: 2026-09-14
Status: Implemented in source; release qualification pending
Related: ADR-0020, ARC-011, ARC-013

## Context

Muninn returns memory records as untrusted retrieval evidence. A remembered
belief says what a source reports believing; it does not establish that the
belief's proposition is true. gosling's ordinary agent reply streams text as
the provider generates it, so a validator applied after the reply cannot
retract a false claim that was already sent to a client.

## Decision

gosling adds an explicit, read-only Recall Brief action at
`_gosling/unstable/session/recall/brief`. The caller names an enrolled Muninn
MCP extension in the current session, supplies a query, and may supply facets,
a continuation cursor, and explicit selectors. Omitted selectors remain
omitted. The ACP adapter rejects an open plan before catalog lookup or model
access, reserves the session operation gate, and dispatches the advertised
`muninn_recall` through the existing app-direct permission and inspection path.
The host does not infer permission from the server's read-only annotation.

The core decodes Muninn's structured result, with a strictly validated JSON
text fallback. Each source is bound to its pinned `muninn://memory/` store,
memory ID, and revision, cross-checked against the hit and returned content
window. Repeated appearances of one exact revision collapse for display;
different records or revisions remain separate even if Muninn gives them the
same `effort_group`. Version one quotes only the returned excerpt and marks
truncation. It never replaces a missing pin with a mutable head.

The selected gosling-managed session provider receives a bounded packet of
untrusted excerpts through a one-shot completion with no tools. The model
proposes claim categories, exact quotes, and unresolved questions. gosling
assigns source keys and citation URIs, checks every proposed key and quote
against the authorized returned windows, requires every source to be covered,
and renders fixed sections. Claim kinds include reported beliefs, reported
changes, reported source assertions, historical referent reports, and labeled
inference; there is no verified-world-fact category. Muninn's receipt carries
minimum/target/budget, continuation and generation signals, lane states, and
facet empty/failed coverage when supplied. Missing fields remain unknown.

An invalid or unavailable model proposal produces an evidence-only report
with exact excerpts and `Inference: None`. Unsafe or missing source identity
produces a visible partial result or an unavailable result. Empty successful
retrieval remains distinct from tool failure. A partial Muninn recall stays
partial even if a valid proposal describes the returned sources.

## Safety and limits

Memory excerpts and source metadata are untrusted data, not instructions or
external truth. The report sends selected private memory excerpts to the
selected provider and names that provider and model in its response. Providers
whose tools execute outside gosling are excluded because the no-tools boundary
cannot be established for them here. This action writes no memory, summary,
fact ledger, or Muninn promotion record, and adds no datastore or migration.

Structural checks prove citation and quote membership, not arbitrary
paraphrase fidelity, autobiographical accuracy, or the truth of a world claim.
The action does not validate ordinary streamed chat answers. CLI and Desktop
entrypoints may use the typed ACP contract later; this source slice exposes the
ACP action only.

## Consequences

The opt-in action incurs one bounded model call after retrieval. Its typed
response is the gated report itself; it does not hand control to an unvalidated
free-form final reply. Exact source revision identity and degraded states are
observable to ACP clients, while Muninn remains the memory authority and
gosling remains the interpretation/presentation owner.
