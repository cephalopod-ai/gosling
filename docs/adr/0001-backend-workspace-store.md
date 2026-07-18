# ADR-0001: Backend-owned workspace store

Date: 2026-07-18
Status: accepted
Requirements affected: REQ-001–REQ-007, REQ-021–REQ-024, REQ-027, REQ-029

## Context

Workspace definitions contain sensitive filesystem topology and affect session creation.
Renderer settings/localStorage cannot provide atomic writes, migration, multi-window
consistency, or an authoritative validation boundary. `SessionManager` is already a large
SQLite owner and should not acquire unrelated workspace CRUD and template responsibilities.

## Decision

Create a dedicated `WorkspaceStore` under the injected Gosling data directory at
`workspaces/workspaces.json`. A versioned envelope owns the active/default IDs, canonical
workspace records, credential-profile metadata, and migration marker. Every mutation takes
an inter-process file lock, reloads current state, validates, writes a private temp file,
fsyncs, and atomically renames. A stale valid temp file is recovered only when the main file
is absent. The service creates a Default workspace when no store exists.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Desktop settings JSON | Considered, rejected: renderer-facing settings are not the backend source of truth and currently include unsafe raw-profile data. |
| localStorage | Considered, rejected: UI preference store with no migration/atomic/multi-window contract. |
| New workspace tables in `sessions.db` | Considered, rejected: couples independent lifecycle/storage and expands the existing persistence hub. |
| Reuse global config YAML directly | Considered, rejected: weak fit for nested CRUD/versioned imports and unknown-field recovery. |

## Consequences

Workspace and session stores remain separate and independently testable. Session rows carry
snapshots rather than foreign keys. Cross-store operations are not one SQLite transaction,
so session creation first prepares a validated immutable snapshot and then writes that
snapshot to the session row; workspace deletion never rewrites sessions.

## Dependency record

No new dependency. Existing `serde`, `serde_json`, `uuid`, `chrono`, and `fs2` are reused.

