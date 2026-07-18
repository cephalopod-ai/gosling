# ADR-0005: Canonical path validation and non-secret output context

Date: 2026-07-18
Status: accepted
Requirements affected: REQ-004–REQ-006, REQ-019, REQ-022–REQ-026, REQ-030

## Context

Workspace paths are user input and can disappear, change type, or traverse through symlinks.
They must guide tools and deliverables without implying ownership or allowing a renderer-only
check to become the safety boundary.

## Decision

The backend lexically normalizes absolute paths, rejects traversal/template escapes, and
canonicalizes existing paths immediately before session creation and containment checks.
Primary folders must exist and be directories. Optional folders yield warnings. Missing
outputs are created only after an explicit request/confirmation. Validation never deletes,
moves, or recursively alters a directory.

The session snapshot is rendered as an internal “Workspace context” system section that names
folders, access intentions, and product-output routing but excludes every credential detail.
Application-level product exports use a matching/default output only where an existing
product-export seam exists; session archives and application updates are not product outputs.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Renderer-only path checks | Considered, rejected: alternate clients and TOCTOU bypass. |
| Prompt-only validation/routing | Considered, rejected: prompt text is not a filesystem safety boundary. |
| Automatically create all configured outputs | Considered, rejected: unexpected filesystem mutation. |
| Route session archives into product outputs | Considered, rejected: transcript backup is not a user deliverable product type. |

## Consequences

Cross-platform lexical normalization has pure target-style tests, while canonical/symlink tests
run on the host. Optional unavailable destinations do not block the application, but the agent
is told only about the saved snapshot and the UI shows warnings.

## Dependency record

No new dependency; standard path/filesystem APIs and existing Electron chooser/reveal helpers are reused.

