# ADR-0006: Universal Desktop artifact routing

Date: 2026-07-18
Status: accepted
Requirements affected: REQ-005, REQ-017, REQ-022, REQ-025, REQ-026, REQ-030
Supersedes: ADR-0005's conditional application-export wording

## Context

Workspace product folders guided the agent and the workspace-metadata export, but session exports,
generated content in the Outputs pane, and Electron downloads did not share one destination seam.
That made output behavior depend on which control produced the artifact.

## Decision

One pure Desktop resolver classifies a suggested name and MIME type into the canonical workspace
`ProductType`, selects the first matching output or the explicit default, sanitizes the portable
leaf name, and builds the proposed path. A focused React context owns workspace selection,
missing-output confirmation, and the single renderer save call. Artifacts and exports pass their
pinned workspace ID; otherwise the active workspace supplies the default.

Electron owns the actual save dialog, content write, full-file copy, source-path authorization,
and native `will-download` placement. Renderer-provided native routing configurations are accepted
only after every output directory passes Gosling's canonical approved-root check. Simultaneous
downloads reserve collision-safe names. Async configuration validation publishes through a
per-window revision guard, so an older workspace update cannot overwrite the newest route. A
routing failure is sent back to the renderer and shown to the operator.

The router never moves an existing artifact. Save dialogs remain an intentional user override.
Missing output folders are created only after explicit confirmation through the existing backend
operation. App updates, automatic session archives, settings, and configuration are not user
product artifacts and retain their dedicated destinations.

## Alternatives considered

| Alternative                                   | Why rejected                                                                                                    |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| Copy every generated file automatically       | It would duplicate or relocate tool output without a user action and violate the no-move invariant.             |
| Rewrite the active global output setting      | Active sessions could change underneath an in-progress task.                                                    |
| Keep per-component save dialogs               | Classification, session pinning, path handling, and failure behavior would drift again.                         |
| Force every save inside the configured folder | The workspace supplies a default; the native save dialog remains an explicit user authorization to override it. |

## Consequences

All Gosling-owned artifact saves, exports, Outputs copies, and native downloads share one routing
policy and test surface. Independent third-party tools that write directly to explicit paths cannot
be transparently intercepted; their existing permission boundaries and the agent's structured
workspace context remain authoritative.

## Dependency record

No new dependency. The implementation uses existing React context, Electron IPC/session APIs,
Node path/filesystem APIs, the generated workspace SDK type, and the existing renderer file-access
guard.
