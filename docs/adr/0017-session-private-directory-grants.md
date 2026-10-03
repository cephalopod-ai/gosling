# ADR-0017: Session-private directory grants

Date: 2026-08-26
Status: implemented and locally validated
Related: ADR-0003, ADR-0011

## Context

Workspace sessions pin their primary folder, reference folders, output folders, and effective
folder policy when the session starts. The launcher could add session-only directories to that
snapshot, but an active workspace session rejected the same additive operation and told the user to
edit the workspace. Editing the workspace would grant the directory to future workspace sessions,
which is broader than the requested access.

The generic Electron directory chooser also grants the selected root to every renderer operation in
the current window. That capability is useful for general file workflows but is broader than a
folder chosen only to extend one agent session.

## Decision

The original session-only decision below is amended for explicit ACP directory additions by the
2026-10-02 decision at the end of this record. Tool-approval grants and launcher overrides retain
their session-only behavior.

An active workspace session may add an existing absolute directory to its own pinned folder policy.
The server canonicalizes the path, records it as read/write in only that session's
`workspace_context_json`, updates the same row's `additional_working_dirs_json`, and refreshes only
that loaded session's extension clients. The two persisted fields are written together. If an
extension refresh fails, both fields and the extension state are rolled back to the prior session
snapshot.

The operation is additive. It cannot replace the primary workspace directory, remove a pinned
workspace root, or alter an existing root's read-only/read-write classification. The workspace
record and unrelated session rows are never updated. Ordinary new sessions created later from the
same workspace therefore do not inherit the grant. Explicit session copies retain their snapshot,
consistent with ADR-0003's existing copy semantics.

Desktop uses a purpose-specific native session-directory chooser for launcher and active-session
additional folders. Unlike the generic chooser, it does not add the selected root to the renderer's
general file-access registry. The selected path is passed to the existing session working-directory
ACP operation, whose server-owned session ID is the persistence and enforcement scope.

## Consequences

- A user can add private working material to the current workspace chat without widening the
  workspace definition or sibling chats.
- The current session's Gosling-hosted tools and extension clients can use the directory; a sibling
  session receives an out-of-scope approval result when it would modify the same path.
- Amended 2026-09-05: a workspace session only prompts for out-of-scope *mutations* unless the
  operator turns on "restrict tools to working directories", which also prompts for out-of-scope
  reads. Read-only shell segments (`cat`, `ls`, `grep`, ...) are judged separately from the
  segments that follow them in a pipeline. Read-only workspace roots are still denied outright.
- Amended 2026-09-08 at the operator's request: unrestricted workspace sessions also allow
  temporary scratch paths under the runtime's OS temp directory and Unix `/tmp` and `/var/tmp`.
  Canonicalization handles aliases such as macOS `/private/tmp`; targets escaping through
  symlinks or parent traversal are checked against their resolved destinations. The temp roots
  themselves are not included in this exception. Every mutation destination is checked, so a
  scratch write cannot conceal another out-of-scope write. Explicit directory restriction and
  read-only workspace policy still take precedence. This allowance does not persist a folder
  grant, change workspace definitions, or authorize renderer file access.
- Existing workspace folder permissions remain pinned. Removing or replacing workspace roots still
  requires starting a new session from an updated workspace.
- This is an application capability boundary, not an operating-system ACL. Separate local programs
  and providers that manage their own external tool process remain subject to their OS permissions;
  Gosling does not claim to revoke filesystem access outside its hosted tool boundary.

## Subsequent change (2026-09-19)

This records a later change to the renderer-side grant scope described in Context. The decision
above is unchanged: session directory additions remain additive, session-scoped, and subject to
workspace folder policy.

- Directory grants recorded through a file chooser now apply to every window and survive a
  restart. They were previously consulted only for `webContentsId === 0`, which is never a real
  window, so the persisted file had no effect on the UI and each launch re-requested the same
  folders.
- A grant covering the user's home directory or a filesystem root is never persisted, and one
  written by an earlier version is dropped when the store loads. Such a root would subsume every
  other approval and make the boundary meaningless.
- A session's own working directories are granted to its window for renderer reads, including a
  directory chosen from the recent list rather than the chooser. These grants are transient: they
  are re-established when the session loads rather than written to the store. The paths come from
  the renderer, which is the same trust the per-file artifact capability in ADR-0013 already
  carries; the main process still refuses the home directory, filesystem roots, symlinks,
  non-directories, and batches over 64.

Implemented in `e79427faf` (persistence and breadth pruning) and `88470e9c0` (session directory
grants). Session evidence is retained locally under `docs/logs/session/`, which `.gitignore`
excludes from the repository.

## Amendment: remembered workspace folders (2026-10-02)

The operator explicitly requested that folders added from an active chat automatically carry into
future chats in the same workspace. The ACP Add working directory operation used by Desktop now
updates the current session as before, then appends the canonical folder to the latest workspace
document under the existing store locks. It is saved as a read/write working folder. An existing
primary, additional, or output root is not duplicated or reclassified; read-only roots stay read-only.

This changes the original decision's prohibition on mutating the workspace for this explicit action.
The session's primary folder remains pinned. Other existing chats retain their saved snapshots;
new chats inherit the amended workspace. Tool-approval grants and launcher-specific overrides do not
modify workspace defaults. If the workspace was deleted, its historical chat can still add a
session-only folder without recreating the workspace.

Extension refresh precedes workspace persistence. An extension or workspace save failure triggers
rollback of the session fields and extension state through the existing transition handler.
SQLite and the workspace JSON file are separate stores: this is not a cross-store transaction.
Interruption between the writes can leave an unacknowledged session-only grant; a file-sync failure
after the workspace file's atomic rename can leave that folder in workspace defaults despite the
reported save error. Retry is additive and canonical-path idempotent. The operation does not restore
an old workspace document over intervening edits.

The saved primary and additional paths are supplied to the model independently of hint files and
optional per-turn context. Paths are escaped data and do not themselves grant permission or bypass
the host's folder policy. Desktop refreshes workspace state locally and across other windows after
a successful addition. Source and validation evidence: `docs/logs/session/2026-10-02-working-folder-memory.md`.
