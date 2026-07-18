# Traceability Matrix — Gosling Desktop Workspaces

Statuses: planned → built → verified | deferred | cut. `verified` requires linked evidence.

Last updated: Gate 1

| REQ | Pri | Requirement | Design refs | Files | Tests | Evidence | Status |
|---|---|---|---|---|---|---|---|
| REQ-001 | P0 | Default workspace migration | ADR-0001; store/service | execution plan | Rust migration/default | — | planned |
| REQ-002 | P0 | Versioned atomic persistence | ADR-0001; I/O contract | store/model | Rust atomic/recovery | — | planned |
| REQ-003 | P0 | Canonical workspace metadata | ADR-0004; SDK DTO | sdk-types/model | schema + round-trip | — | planned |
| REQ-004 | P0 | Working/source/reference folders | ADR-0005; validator/UI editors | SDK/validation/Desktop | Rust + Vitest | — | planned |
| REQ-005 | P0 | Product output folders | ADR-0005; output editor/context | SDK/context/Desktop | Rust + Vitest | — | planned |
| REQ-006 | P0 | Backend folder validation | ADR-0005; validation | validation/service | path/symlink tests | — | planned |
| REQ-007 | P0 | Active selection/broadcast | ADR-0001/0004; context/IPC | store/context/main/preload | Rust + hook tests | — | planned |
| REQ-008 | P0 | Explicit workspace new-session ID | ADR-0003/0004; ACP contract | sessions/new_session/Desktop | race/parity tests | — | planned |
| REQ-009 | P0 | Session workspace snapshot | ADR-0003; DB v22 | SessionManager/new_session | migration/pin tests | — | planned |
| REQ-010 | P0 | Pinned resume | ADR-0002/0003; Agent | load/manager/agent | resume/switch tests | — | planned |
| REQ-011 | P0 | Switch affects future chats only | ADR-0003/0004; context | WorkspaceContext/header | hook/UI tests | — | planned |
| REQ-012 | P0 | Secure fail-closed profile resolution | ADR-0002; credentials/scope | Config/workspace/Agent | sentinel/relink tests | — | planned |
| REQ-013 | P0 | Metadata-only profile API | ADR-0002/0004; SDK DTO | SDK/handlers/credentials | exact response tests | — | planned |
| REQ-014 | P0 | Secure profile lifecycle UI | ADR-0002/0004; profile dialog | credentials/Desktop | security + form tests | — | planned |
| REQ-015 | P0 | Workspaces sidebar section | ADR-0004; WorkspaceSection | Desktop workspace components | a11y/keyboard tests | — | planned |
| REQ-016 | P0 | Row actions/editor | ADR-0004; editor modules | Desktop workspace components | workflow tests | — | planned |
| REQ-017 | P0 | Pinned workspace header | ADR-0003/0004; header | response/session/header | header tests | — | planned |
| REQ-018 | P0 | Existing session-list integration | ADR-0003/0004; list filter | list_sessions/hook | filter/legacy tests | — | planned |
| REQ-019 | P0 | Non-secret agent context | ADR-0003/0005; context renderer | workspace context/Agent | prompt sentinel tests | — | planned |
| REQ-020 | P0 | Scoped provider recreation | ADR-0002; AgentConfig | config/agent/manager | two-profile tests | — | planned |
| REQ-021 | P0 | Non-cascading deletion | ADR-0001–0003; service/session | store/credentials/session | delete/copy tests | — | planned |
| REQ-022 | P1 | Safe export/import | ADR-0001/0005; I/O contract | store/handlers/Desktop | golden/sentinel tests | — | planned |
| REQ-023 | P1 | Distribution templates | ADR-0001/0005; I/O contract | model/validation/store | template tests | — | planned |
| REQ-024 | P1 | Safe provider migration alias | ADR-0002; credentials migration | service/credentials | alias/no-copy tests | — | planned |
| REQ-025 | P1 | Application output defaults | ADR-0005 | context + discovered exporters | routing/discovery evidence | — | planned |
| REQ-026 | P1 | Existing Electron folder workflows | ADR-0004/0005 | main/preload/editor | chooser/reveal tests | — | planned |
| REQ-027 | P1 | Focused synchronized context | ADR-0004 | WorkspaceContext/IPC | rerender/broadcast tests | — | planned |
| REQ-028 | P1 | Canonical SDK generation | ADR-0004 | sdk-types/schema/generated | schema generation/typecheck | — | planned |
| REQ-029 | P0 | Gosling naming/backward compatibility | all ADRs | cross-cutting | migration/search/CLI checks | — | planned |
| REQ-030 | P0 | Required validation/checks | execution test plan | tests/tooling | Gate 8 commands | — | planned |

## Deferred / cut log

None.

## Gate 8 sweep record

| Check | Result |
|---|---|
| All P0 verified/re-scoped | pending |
| All P1 verified/re-scoped | pending |
| No built-but-unverified rows | pending |
| Reverse trace: all significant code maps to a REQ or plan change | pending |
