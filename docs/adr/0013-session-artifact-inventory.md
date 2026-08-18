# ADR-0013: Durable session artifact inventory

Date: 2026-08-13
Status: accepted
Related: ADR-0005, ADR-0006

## Context

The Outputs workbench treated user-opened preview tabs as the artifact list. Files mentioned by a
successful tool or completed assistant response therefore remained invisible until the user clicked a
message chip, and global tab persistence could mix preview state across sessions. Scanning output
directories would be broader than conversation provenance and would make filesystem contents an
implicit authority source.

## Decision

Gosling owns a metadata-only, session-scoped inventory in SQLite schema v26. Discovery accepts exact
successful built-in write/edit targets first, local MCP resource links and embedded resources, explicit
tool artifact metadata, conservative output arguments from successful mutating tools, and completed
assistant Markdown file references. A unique session/resolved-path key makes updates idempotent and
retains the strongest provenance. Legacy sessions receive a one-time persisted-message backfill; no
directory scan occurs.

ACP exposes paginated `_gosling/unstable/session/artifacts/list` and an `artifact_update` session
notification sent only after storage succeeds. Older backends may be reconstructed conservatively from
already-loaded trusted assistant messages. The Desktop session store owns inventory state. The Outputs
workbench keeps that inventory separate from session-scoped preview tabs and selection; pane width and
visibility remain window preferences.

Inventory registration grants no filesystem capability. Relative paths retain their discovery working
directory, but selection still passes through the Electron artifact guard. Existing renderer roots,
validated workspace output roots, explicit file-picker grants, and exact session-generated user-facing
deliverables authorize a read/open/reveal/copy. The last category is limited to document-like files
created or modified by a built-in tool or referenced by the assistant; it never grants a directory or
authorizes code, configuration, or MCP/tool-metadata paths.

## Consequences

Outputs populate without click-driven side effects and survive restart, resume, and fork. Missing or
unsupported files stay named. Unknown extensions remain visible, while common code/config extensions
receive a code preview kind. Files are never created, copied, moved, opened, or read merely because a
record exists. Files created by arbitrary shell commands and never referenced remain undiscovered; a
future bounded observer would require a separate decision.
