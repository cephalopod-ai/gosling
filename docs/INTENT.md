# Intent Charter — Gosling Desktop Workspaces

Status: authoritative. Changes only through `docs/build/plan-changes.md`.

Date created: 2026-07-18
Last amended: initial charter

## Mission

Give Gosling Desktop users repeatable, secure working environments that keep project
folders, deliverable destinations, provider/model defaults, and credential-profile
references together. A workspace changes defaults for future chats while every session
retains the environment and credential identity with which it was created.

## Users and context

This feature serves a local Gosling Desktop user working across multiple projects or
accounts. The renderer presents and edits workspaces, but the Gosling backend owns
workspace metadata, validation, secure credential references, session association, and
resume behavior.

## Primary workflow (P0)

On first launch, the user has a usable Default workspace. They create or edit a
workspace from the sidebar, choose a primary folder, optional reference/source folders,
named product-output folders, and a secure credential profile. They activate it and
start a chat. Gosling validates the workspace, starts the session in the primary folder,
resolves the selected profile through secure storage, pins a non-secret workspace
snapshot to the session, and shows the pinned workspace in the chat header. Later
workspace switches affect only new chats; resuming this chat restores its pinned
workspace and fails visibly if its credential profile must be relinked.

## Invariants

| ID      | Invariant                                                                                                                   | Why                                                        |
| ------- | --------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| INV-001 | Raw secrets exist only in Gosling's existing secure secret-storage system and short-lived credential-entry state.           | Prevent credential disclosure and cross-account use.       |
| INV-002 | A session's workspace and credential-profile references never change merely because the active workspace changes.           | Preserve reproducibility and account identity.             |
| INV-003 | Workspace metadata, exports, renderer responses, session rows, prompts, logs, and settings contain no raw secrets.          | Keep every non-secret boundary safe by construction.       |
| INV-004 | Deleting a workspace or profile never deletes sessions or user files.                                                       | Prevent destructive cascades.                              |
| INV-005 | A missing primary folder or credential profile fails visibly before provider/session activation; no silent fallback occurs. | Avoid running in the wrong project or account.             |
| INV-006 | Workspace and active-selection writes are atomic, versioned, and owner-permission hardened.                                 | Survive interruption without exposing or corrupting state. |
| INV-007 | The backend is the source of truth; renderer storage contains only harmless UI preferences.                                 | Avoid split-brain state and insecure persistence.          |
| INV-008 | Path normalization and canonical checks happen in the backend immediately before use.                                       | Prevent traversal, symlink, and time-of-check bypasses.    |
| INV-009 | Existing sessions and users remain usable without manual migration.                                                         | Preserve backward compatibility.                           |
| INV-010 | Workspace changes never move, delete, or recursively rewrite selected folders.                                              | Treat filesystem references as user-owned data.            |

## Requirements

| REQ     | Priority | Requirement                                                                          | Acceptance criteria                                                                                                                                                                                                                                                                                                                            |
| ------- | -------- | ------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| REQ-001 | P0       | Create and persist a Default workspace for clean and migrated installs.              | First backend initialization returns exactly one usable Default workspace when none exist; its primary folder derives from the current/default working directory; restart preserves the same stable UUID.                                                                                                                                      |
| REQ-002 | P0       | Provide backend-owned versioned workspace persistence and active selection.          | Workspace CRUD and active ID survive restart; writes use temp+fsync+rename and owner-only permissions; malformed records yield a recoverable error instead of crashing.                                                                                                                                                                        |
| REQ-003 | P0       | Model workspace general metadata canonically.                                        | Stable UUID, schema version, name, optional description/icon, provider/model defaults, and created/updated/last-opened timestamps round-trip through one shared contract.                                                                                                                                                                      |
| REQ-004 | P0       | Support a primary working folder and additional source/reference folders.            | Users can add, edit, relink, reveal, and remove references; kinds and read/read-write access round-trip; removal never touches the physical directory.                                                                                                                                                                                         |
| REQ-005 | P0       | Support named product-output folders and product-type assignments.                   | At least one output, exactly one default, create-if-missing, and one-or-more supported product types per output round-trip; explicit confirmation is required before creating a missing folder.                                                                                                                                                |
| REQ-006 | P0       | Validate and normalize workspace folders in the backend.                             | Missing/non-directory/inaccessible paths return actionable per-folder warnings; a missing primary folder blocks new sessions; optional missing folders do not crash the app.                                                                                                                                                                   |
| REQ-007 | P0       | Persist and switch the active workspace.                                             | Selecting a workspace updates backend state, list/header indicators, and other open Desktop windows; restart restores it; switching does not restart the backend.                                                                                                                                                                              |
| REQ-008 | P0       | Pass an explicit workspace ID through every new-session entry path.                  | Hub, Pair route, launcher/shortcut, and shared create helper use the same request seam; the backend pins the requested valid workspace even if active selection changes while the request is in flight.                                                                                                                                        |
| REQ-009 | P0       | Pin workspace context to new sessions.                                               | A created session stores workspace ID/name snapshot, effective working folder, profile/binding metadata reference, and non-secret folder/output context in `sessions.db`; no secret sentinel is present.                                                                                                                                       |
| REQ-010 | P0       | Resume from the session's pinned workspace snapshot.                                 | Resume uses saved working folder, workspace context, provider/model, and profile reference rather than the current active workspace; legacy nullable rows still resume.                                                                                                                                                                        |
| REQ-011 | P0       | Keep workspace switching isolated from visible/running sessions.                     | Switching updates only future-chat defaults, does not interrupt generation or mutate the visible session, and shows a non-blocking “new chats” indication.                                                                                                                                                                                     |
| REQ-012 | P0       | Resolve credential profiles securely and fail closed.                                | Provider construction resolves logical fields to profile-specific secure keys without rewriting global canonical keys; a missing/deleted profile returns a relink-required error and never falls back silently.                                                                                                                                |
| REQ-013 | P0       | Expose credential-profile metadata without secret values.                            | Renderer list/create/rename/update/delete responses include only ID, name, provider/service, auth kind, configured fields/status, and timestamps; stored values are never returned.                                                                                                                                                            |
| REQ-014 | P0       | Provide secure credential-profile lifecycle UI.                                      | Password fields are blank on edit, show configured status, clear on submit/cancel, use the secure setter, sanitize errors, warn on references, and require explicit confirmation before referenced-profile deletion.                                                                                                                           |
| REQ-015 | P0       | Add a Workspaces sidebar section.                                                    | Heading, collapse control, add button, active indicator, loading/empty/error states, keyboard navigation, visible focus, and accessible icon-control names are covered by UI tests.                                                                                                                                                            |
| REQ-016 | P0       | Provide workspace row actions and management UI.                                     | Open/switch, edit, duplicate, reveal, export metadata, and delete work; editor covers General, Credentials, Folders, and Product outputs with current Gosling visual conventions.                                                                                                                                                              |
| REQ-017 | P0       | Show the session's pinned workspace in the chat header.                              | Header displays saved workspace name and visibly distinguishes it when different from the active workspace; deleted metadata displays the snapshot or “Deleted workspace.”                                                                                                                                                                     |
| REQ-018 | P0       | Integrate workspace association into the existing session list.                      | Active workspace filters/groups the existing list, All workspaces is available, legacy sessions appear as Default/Unassigned, cross-workspace search works in All, and resume ignores the current filter.                                                                                                                                      |
| REQ-019 | P0       | Provide non-secret workspace context to the agent.                                   | New/resumed sessions receive workspace name, primary folder, additional folders/access, and named product outputs/types plus safe routing guidance; no credential identifiers or secret fields appear.                                                                                                                                         |
| REQ-020 | P0       | Preserve credential isolation during provider/model recreation.                      | Model/thinking/provider recreation for a pinned session uses its profile scope; two profiles for one provider resolve independently in tests.                                                                                                                                                                                                  |
| REQ-021 | P0       | Handle workspace/profile deletion without destructive cascades.                      | Workspace deletion leaves sessions/files intact and retains snapshots; only-workspace deletion creates/reassigns Default; profile deletion preserves session metadata and produces relink-required resume state.                                                                                                                               |
| REQ-022 | P1       | Export and import safe workspace metadata.                                           | Export omits raw secrets, secure key identifiers, and account identifiers; import rejects secret-shaped fields and unsafe traversal/template paths, normalizes IDs, and reports malformed records.                                                                                                                                             |
| REQ-023 | P1       | Support non-secret custom-distribution templates.                                    | First launch materializes valid templates, safely resolves documented path placeholders, and marks unprovisioned profiles as needing setup; packaged templates cannot contain secrets.                                                                                                                                                         |
| REQ-024 | P1       | Map an existing provider configuration during migration when safe.                   | Migration may create a metadata alias to existing secure keys without reading/copying secret values; unsupported mappings remain unconfigured and actionable.                                                                                                                                                                                  |
| REQ-025 | P1       | Route every application-owned artifact save/export/download through product outputs. | One shared router classifies documents, spreadsheets, presentations, images, video, code, data, exports, and other artifacts; it selects the session-pinned or active workspace's matching/default output, drives all Gosling-owned save/export entry points, and configures native downloads without moving an already-generated source file. |
| REQ-026 | P1       | Use existing Electron folder workflows.                                              | Directory chooser, reveal, recent-dir permission/grant behavior, and optional ensure-directory are reused; tests verify chooser integration and no recursive filesystem mutation.                                                                                                                                                              |
| REQ-027 | P1       | Keep workspace state efficient and synchronized.                                     | A focused `WorkspaceContext` exposes the required CRUD/validation methods with memoized values/callbacks; broadcasts refresh other supported windows without duplicate persistence state.                                                                                                                                                      |
| REQ-028 | P1       | Use the canonical SDK/type generation path.                                          | Rust custom DTOs are the source for workspace wire types, generated SDK artifacts are regenerated through the approved command, and Desktop imports no prohibited OpenAPI client/type.                                                                                                                                                         |
| REQ-029 | P0       | Preserve Gosling naming and compatibility.                                           | New environment/config/keyring/deep-link/UI identifiers use Gosling names; existing provider configuration, sessions, CLI, folder chooser, and custom distributions continue working.                                                                                                                                                          |
| REQ-030 | P0       | Verify security and behavior with required tests/checks.                             | Sentinel-secret, migration, persistence, path, session, credential, frontend workflow, typecheck, Desktop unit, relevant Rust, fmt, and clippy checks execute successfully or failures are reported exactly.                                                                                                                                   |

Priorities: P0 primary workflow / P1 committed / P2 stretch. IDs are stable and are
never renumbered or reused.

## Explicitly out of scope

| Item                                           | Why out                                                                          | Revisit trigger                                          |
| ---------------------------------------------- | -------------------------------------------------------------------------------- | -------------------------------------------------------- |
| Cloud synchronization of workspace definitions | Explicitly excluded and requires account/conflict architecture.                  | A separate sync proposal.                                |
| Credential sharing or upload                   | Violates local secure-storage boundary.                                          | Never without a new security design and authorization.   |
| Team/RBAC behavior                             | Desktop feature is local single-user.                                            | Multi-user product requirements.                         |
| Repository cloning or Git worktree creation    | Workspaces reference folders; they do not provision source trees.                | Separate project bootstrap feature.                      |
| Moving/deleting user project or output files   | Violates INV-004/INV-010.                                                        | Not part of Workspaces.                                  |
| Drag-and-drop workspace ordering               | Explicitly out of scope.                                                         | User demand after core usability ships.                  |
| Plugin marketplace redesign                    | Unrelated architecture.                                                          | Separate initiative.                                     |
| CLI workspace commands                         | Backward compatibility says CLI remains unchanged.                               | Explicit CLI scope.                                      |
| Workspace extension defaults                   | Current extension configuration does not expose a clean scoped default contract. | A scoped extension-default ADR and explicit requirement. |

## Non-goals

This build does not create a general-purpose secrets manager, synchronize workspace
state across machines, add user accounts, or reinterpret a workspace as ownership of
the files it references.

## Glossary

| Term                   | Meaning                                                                          | Not to be confused with                                    |
| ---------------------- | -------------------------------------------------------------------------------- | ---------------------------------------------------------- |
| Workspace              | Backend-owned repeatable Desktop environment definition.                         | A Git worktree or OS virtual desktop.                      |
| Active workspace       | Default used for future sessions.                                                | The workspace pinned to the visible session.               |
| Pinned workspace       | Session snapshot/reference selected at creation.                                 | The currently active workspace.                            |
| Credential profile     | Non-secret metadata whose fields resolve through secure storage.                 | Raw API keys or existing renderer managed-secret profiles. |
| Credential binding     | Workspace reference connecting a profile to a provider/extension/service target. | The secret value itself.                                   |
| Primary working folder | Default project root and required session working directory.                     | An optional source/reference folder.                       |
| Product output folder  | Named deliverable destination with product-type assignments.                     | Session transcript/archive storage.                        |
| Workspace context      | Non-secret snapshot supplied to session/tool/model behavior.                     | ACP prompt credentials or keyring identifiers.             |
| Legacy session         | A pre-v22 session with nullable workspace fields.                                | A deleted-workspace session with a saved snapshot.         |

## Scope-pressure policy

Ordered cut list under pressure (first eligible cut first):

1. Third-party extensions or agent tools that write directly to an absolute path outside Gosling's save/download UI remain responsible for that explicitly chosen path; all Gosling-owned save, export, artifact-copy, and native-download surfaces use REQ-025's router.
2. REQ-024 automatic existing-provider aliasing for providers without a safe mapping.
3. REQ-023 custom-distribution templates beyond a documented static template format.
4. REQ-022 workspace import (export remains committed for the sidebar action).

P0 is never cut without user contact.
