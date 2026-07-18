# Traceability Matrix — Gosling Desktop Workspaces

Statuses: planned → built → verified | deferred | cut. `verified` requires linked evidence.

Last updated: Gate 8

| REQ | Pri | Requirement | Implementation | Verification | Evidence | Status |
| --- | --- | --- | --- | --- | --- | --- |
| REQ-001 | P0 | Default workspace migration | workspace store/service/bootstrap | default initialization and v21 session migration tests | Gate 4/8 | verified |
| REQ-002 | P0 | Versioned atomic persistence | `workspace/store.rs` | atomic truncation, concurrency, corruption recovery, unknown-field tests | Gate 4/6/8 | verified |
| REQ-003 | P0 | Canonical workspace metadata | SDK `workspace.rs`; generated ACP client | Rust round-trip plus SDK/Desktop typechecks | Gate 3/4/8 | verified |
| REQ-004 | P0 | Working/source/reference folders | validator, editor, chooser | Rust path tests; editor/chooser Vitest | Gate 4/5/8 | verified |
| REQ-005 | P0 | Product output folders | workspace DTO/service/editor/context | output creation/routing and editor tests | Gate 4/5/7/8 | verified |
| REQ-006 | P0 | Backend folder validation | `workspace/validation.rs` | missing/foreign/traversal/symlink/canonical tests | Gate 4/6/8 | verified |
| REQ-007 | P0 | Active selection/broadcast | store, WorkspaceContext, main/preload broadcast | store/context/multi-window tests | Gate 4/5/8 | verified |
| REQ-008 | P0 | Explicit workspace new-session ID | ACP new-session metadata and Desktop create seam | ACP and `createSession` Vitest | Gate 4/5/8 | verified |
| REQ-009 | P0 | Session workspace snapshot | sessions DB schema v22 and update builder | migration/snapshot/copy tests | Gate 4/8 | verified |
| REQ-010 | P0 | Pinned resume | ACP load, Agent restore, session snapshot | Rust snapshot/copy plus Desktop pinned-session tests | Gate 4/5/8 | verified |
| REQ-011 | P0 | Switch affects future chats only | WorkspaceContext/sidebar/header | context, sidebar, header tests | Gate 5/8 | verified |
| REQ-012 | P0 | Secure fail-closed profile resolution | credentials service and scoped Config resolution | required-secret/relink/sentinel tests | Gate 4/6/8 | verified |
| REQ-013 | P0 | Metadata-only profile API | SDK DTOs and ACP handlers | serialization, response, sentinel, debug-redaction tests | Gate 4/6/8 | verified |
| REQ-014 | P0 | Secure profile lifecycle UI | credential manager and backend lifecycle | create/update/delete/test/form-clearing Vitest | Gate 5/7/8 | verified |
| REQ-015 | P0 | Workspaces sidebar section | `WorkspaceSidebarSection` | render, active state, warning, keyboard/a11y tests | Gate 5/8 | verified |
| REQ-016 | P0 | Row actions/editor | sidebar/editor components | CRUD, chooser, validate, output, export tests | Gate 5/7/8 | verified |
| REQ-017 | P0 | Pinned workspace header | `SessionActionsHeader` | pinned-versus-active test | Gate 5/8 | verified |
| REQ-018 | P0 | Existing session-list integration | existing navigation-session hook plus filter | filter/all/legacy/search projection tests | Gate 5/8 | verified |
| REQ-019 | P0 | Non-secret agent context | structured workspace context renderer | credential-absence and bounded-context tests | Gate 4/6/8 | verified |
| REQ-020 | P0 | Scoped provider recreation | Agent/Config session credential scope | distinct profile and fail-closed resolution tests/audit trace | Gate 4/6/8 | verified |
| REQ-021 | P0 | Non-cascading deletion | workspace/profile services; session snapshots | workspace deletion preserves sessions/files test | Gate 4/8 | verified |
| REQ-022 | P1 | Safe export/import | service handlers and sidebar export | sentinel/traversal import and export-destination tests | Gate 4/7/8 | verified |
| REQ-023 | P1 | Distribution templates | `workspace/bootstrap.rs`; custom distro docs | placeholder/schema/profile status tests and docs build | Gate 4/7/8 | verified |
| REQ-024 | P1 | Safe provider migration alias | metadata-only global profile alias | no-copy/status resolution tests and security audit | Gate 4/6/8 | verified |
| REQ-025 | P1 | Application output defaults | agent context plus workspace metadata export destination | context/output/export tests | Gate 4/7/8 | verified (bounded) |
| REQ-026 | P1 | Existing Electron folder workflows | main/preload chooser/reveal; workspace editor | chooser/reveal/create feedback tests | Gate 5/8 | verified |
| REQ-027 | P1 | Focused synchronized context | `WorkspaceContext` and invalidation channel | context callback/filter/broadcast tests | Gate 5/8 | verified |
| REQ-028 | P1 | Canonical SDK generation | Rust schema/meta and generated UI SDK | JSON validation, SDK build/tests/typecheck, Desktop typecheck | Gate 4/6/8 | verified |
| REQ-029 | P0 | Gosling naming/backward compatibility | Gosling paths/namespaces; nullable session columns | negative naming search, migration and full regression suites | Gate 4/7/8 | verified |
| REQ-030 | P0 | Required validation/checks | Gate 8 acceptance matrix | build, 1,481 Rust tests, 479 Desktop tests, SDK/docs, fmt, clippy | Gate 8 | verified |

## Deferred / cut log

- Per-workspace extension defaults were deliberately cut: the current extension configuration is
  global and lacks a clean session-scoped hook.
- A universal application artifact-export router was re-scoped. Workspace metadata exports use an
  `export`-capable/default output, and agent tools receive structured output routing; independent
  extension/save-dialog destinations remain owned by those flows.
- Live provider network validation remains capability-gated. The credential Test control reports
  secure presence/status; current providers do not expose a uniform safe validation hook.

## Gate 8 sweep record

| Check | Result |
| --- | --- |
| All P0 verified/re-scoped | pass |
| All P1 verified/re-scoped | pass |
| No built-but-unverified rows | pass |
| Reverse trace: all significant code maps to a REQ or documented acceptance repair | pass |
