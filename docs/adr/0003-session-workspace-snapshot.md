# ADR-0003: Nullable session workspace snapshot in schema v22

Date: 2026-07-18
Status: accepted
Requirements affected: REQ-008–REQ-012, REQ-017–REQ-021, REQ-029

## Context

Resume must not depend on mutable active-workspace state, and deleted workspaces/profiles must
not invalidate historical session display. Existing schema v21 stores working directory,
provider, and model but no workspace identity or output context.

## Decision

Schema v22 adds nullable `workspace_id`, `workspace_name`, `credential_profile_id`,
`credential_profile_name`, `credential_binding_id`, and `workspace_context_json` columns.
The JSON snapshot contains only non-secret workspace name, primary folder, additional folders
with kind/access, and product outputs/types. New Desktop sessions set all applicable fields in
the same session update before activation. Copy/fork copies the snapshot. Legacy rows remain
null and keep existing resume/fallback behavior. Pinned rows preserve saved working directory
and reject missing credentials rather than adopting global defaults.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Foreign key to a workspace SQLite table | Considered, rejected: workspace metadata is separately owned and deletion must not cascade. |
| Store only `workspace_id` | Considered, rejected: deleted/changed workspaces would destroy reproducibility and display context. |
| Copy the entire workspace/profile record | Considered, rejected: unnecessary mutable metadata and higher secret-leak risk. |

## Consequences

Historical rows are readable after deletion and migration is additive. Snapshot evolution must
remain backward compatible; unknown future JSON fields are ignored by readers, while the
stored session snapshot serializer emits the current canonical subset.

## Dependency record

No new dependency; existing SQLite/sqlx and JSON facilities are reused.

