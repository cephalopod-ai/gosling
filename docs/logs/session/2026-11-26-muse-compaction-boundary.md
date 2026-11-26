# 2026-11-26 — Muse Code compaction boundary

## Task

Close the compaction path identified in the [provider and mail audit](2026-11-25-provider-mail-audit.md)
from clean `main` at `e21729706`. Muse Code owns its tools and its configured hooks.
The installed CLI's separate shell, write, and web-tool controls do not establish
isolation from all configured Muse behavior.

## Changes

- `context_mgmt` rejects providers with external tool runtimes at the manual and
  automatic compaction, standalone handoff summary, and tool-pair summary entry points.
- The agent continues a fitting chat request after a single soft-limit notice.
  It closes an oversized turn before an external runner is called, even with automatic
  compaction disabled. The Muse adapter also checks the serialized prompt before it
  probes or starts the executable. Overflow reported after dispatch ends the turn
  and preserves partial output and undelivered steering for the user.
- Desktop keeps the context estimate visible for Muse and omits the compaction
  controls. The guide documents recovery by switching to a provider with gosling-managed
  tools or starting another session.

## Validation

- The fixture recorded zero Muse inference launches for `/compact` and hard-limit
  rejection and one chat launch across the soft threshold. It also verified direct
  compaction and handoff rejection, serialized prompt size rejection, and the absence
  of a new compaction revision.
- The compaction suite passed 21 tests, including cancellation and temporary-tail
  resume. The Muse provider suite passed 14 tests. The external-runtime overflow
  and steering regression passed, as did five turn-lease tests. The installed
  Muse executable passed a separate isolated echo round trip.
- Replays through the rebuilt CLI and ACP stdio both passed manual compaction,
  soft-threshold chat, and hard-limit refusal. The CLI replay verified hard-limit
  refusal with automatic compaction disabled.
- Desktop typecheck and 34 focused chat-input, context-indicator, and alert tests passed.
- Strict scoped Clippy, Cargo formatting, Desktop lint and formatting, and the
  Muse guide's MDX compilation passed.

## Follow-up

An explicitly selected local compaction model remains an independent enhancement.
It needs its own context budget and history provenance. Authenticated Meta inference
and native packaged and cross-platform behavior remain outside this validation.
