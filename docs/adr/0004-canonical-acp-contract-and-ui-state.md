# ADR-0004: Canonical ACP contract and focused renderer state

Date: 2026-07-18
Status: accepted
Requirements affected: REQ-003–REQ-007, REQ-013–REQ-018, REQ-022, REQ-026–REQ-028

## Context

Workspaces cross Rust persistence, ACP JSON-RPC, generated TypeScript, React forms, Electron
folder operations, and session list metadata. Hand-maintained parallel models would drift,
while putting persistence calls in sidebar components would duplicate state.

## Decision

`gosling-sdk-types::workspace` owns all public workspace/profile DTOs and typed custom
requests. The approved schema generator produces UI SDK types/clients. Desktop adds a thin ACP
adapter and one memoized `WorkspaceContext`; components never persist directly. Existing
`useNavigationSessions` remains the only sidebar session list state and receives a workspace
filter. Electron owns only directory/reveal operations and a typed “workspaces changed”
invalidation broadcast.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Local TypeScript workspace interfaces | Considered, rejected: duplicate schema with no enforced agreement. |
| Generated OpenAPI Desktop imports | Considered, rejected: explicitly prohibited by repository rules. |
| Sidebar-owned fetch/state | Considered, rejected: presentation would own persistence and cause whole-app/duplicate-list drift. |
| Persist full workspace state in Electron settings | Considered, rejected: violates backend authority and secret boundary. |

## Consequences

Wire changes require Rust DTO update and regeneration. The Desktop can keep harmless collapsed
and session-filter preferences locally, but workspaces and active selection always refresh from
the backend.

## Dependency record

No new dependency; existing ACP macros, schema generator, React context, and typed IPC are reused.

