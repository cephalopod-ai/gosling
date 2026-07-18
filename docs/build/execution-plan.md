# Gated Execution Plan — Gosling Desktop Workspaces

Date: 2026-07-18
Target: Rust backend + Electron/React Desktop
Profile: existing-repository/Giles, governed repair
Commit policy: local gate checkpoints; no remote push without explicit authorization

## Sources of truth

- Intent: `docs/INTENT.md`
- Architecture and decisions: `docs/architecture.md`, `docs/adr/0001`–`0005`
- I/O: `docs/build/io-contract.md`
- Status: `docs/build/traceability-matrix.md`, `docs/build/build-state.md`

## File plan

Expected sizes apply to new/extracted files. Existing files above the soft limit receive only
bounded edits; new UI/persistence concerns are extracted rather than appended there.

| File | Purpose | Expected lines | Gate |
|---|---|---:|---:|
| `crates/gosling-sdk-types/src/workspace.rs` | canonical DTOs and custom request/response types | 600 | 3/4 |
| `crates/gosling-sdk-types/src/lib.rs` | export workspace module | +2 | 3 |
| `crates/gosling-sdk-types/src/custom_requests.rs` | re-export workspace request types | +2 | 3 |
| `crates/gosling/src/workspace/mod.rs` | public service/module surface | 250 | 3/4 |
| `crates/gosling/src/workspace/model.rs` | store envelope/conversions | 250 | 3/4 |
| `crates/gosling/src/workspace/store.rs` | private atomic store and migration | 550 | 4 |
| `crates/gosling/src/workspace/validation.rs` | path/import/template validation | 450 | 4 |
| `crates/gosling/src/workspace/credentials.rs` | metadata lifecycle and scope resolution | 500 | 4 |
| `crates/gosling/src/workspace/context.rs` | session snapshot/prompt rendering | 250 | 4 |
| `crates/gosling/src/lib.rs` | expose workspace module | +2 | 3 |
| `crates/gosling/src/config/base.rs` | strict task-local config resolution scope | +180 | 4 |
| `crates/gosling/src/acp/server/workspaces.rs` | typed ACP handlers/error mapping | 650 | 4 |
| `crates/gosling/src/acp/server/custom_dispatch.rs` | register workspace/profile methods | +130 | 4 |
| `crates/gosling/src/acp/server.rs` | own service; activation/pinned cwd/context | +100 | 4 |
| `crates/gosling/src/acp/server/new_session.rs` | authoritative workspace preparation/pinning | +120 | 4 |
| `crates/gosling/src/acp/response_builder.rs` | workspace session metadata | +45 | 4 |
| `crates/gosling/src/acp/server/list_sessions.rs` | workspace/list filter metadata | +90 | 4 |
| `crates/gosling/src/acp/server_factory.rs` | initialize workspace service through agent options | +10 | 4 |
| `crates/gosling/src/session/session_manager.rs` | v22 columns/model/update/filter/copy | +260 | 4 |
| `crates/gosling/src/agents/agent.rs` | scoped create/recreate/restore and context | +130 | 4 |
| `crates/gosling/src/execution/manager.rs` | fail closed for pinned restore errors | +35 | 4 |
| `crates/gosling/tests/workspaces.rs` | store/domain/migration/path tests | 700 | 4 |
| `crates/gosling/tests/workspace_sessions.rs` | session v22/pin/resume/delete tests | 600 | 4 |
| `crates/gosling/tests/workspace_security.rs` | sentinel/import/scope/concurrency tests | 550 | 4/6 |
| `crates/gosling/acp-schema.json` | generated schema | generated | 4 |
| `crates/gosling/acp-meta.json` | generated method metadata | generated | 4 |
| `ui/sdk/src/generated/types.gen.ts` | generated DTOs | generated | 4 |
| `ui/sdk/src/generated/zod.gen.ts` | generated validators | generated | 4 |
| `ui/sdk/src/generated/client.gen.ts` | generated client methods | generated | 4 |
| `ui/desktop/src/acp/workspaces.ts` | thin typed ACP adapter | 300 | 5 |
| `ui/desktop/src/contexts/WorkspaceContext.tsx` | focused state/mutations/session filter | 500 | 5 |
| `ui/desktop/src/components/workspaces/WorkspaceSection.tsx` | sidebar section and row actions | 400 | 5 |
| `ui/desktop/src/components/workspaces/WorkspaceEditorDialog.tsx` | editor orchestration | 350 | 5 |
| `ui/desktop/src/components/workspaces/WorkspaceGeneralFields.tsx` | general fields | 180 | 5 |
| `ui/desktop/src/components/workspaces/WorkspaceFoldersEditor.tsx` | primary/additional folder controls | 400 | 5 |
| `ui/desktop/src/components/workspaces/WorkspaceOutputsEditor.tsx` | output destinations/types/default | 400 | 5 |
| `ui/desktop/src/components/workspaces/WorkspaceCredentialsEditor.tsx` | bindings/profile selection/status | 300 | 5 |
| `ui/desktop/src/components/workspaces/CredentialProfileDialog.tsx` | short-lived write-only credential form | 400 | 5 |
| `ui/desktop/src/components/workspaces/workspaceForm.ts` | draft/default/validation helpers | 250 | 5 |
| `ui/desktop/src/components/Layout/NavigationPanel.tsx` | mount section and filtered existing list | +60 | 5 |
| `ui/desktop/src/hooks/useNavigationSessions.ts` | backend workspace filter, same list state | +45 | 5 |
| `ui/desktop/src/App.tsx` | provider and active workspace new-session defaults | +35 | 5 |
| `ui/desktop/src/components/Hub.tsx` | active workspace working folder | +25 | 5 |
| `ui/desktop/src/sessions.ts` | explicit workspace ID in shared create seam | +25 | 5 |
| `ui/desktop/src/acp/chatSessionController.ts` | carry workspace ID | +15 | 5 |
| `ui/desktop/src/acp/sessions.ts` | ACP meta/session mapping/filter | +75 | 5 |
| `ui/desktop/src/types/session.ts` | workspace session metadata fields | +15 | 5 |
| `ui/desktop/src/components/SessionActionsHeader.tsx` | pinned/active workspace badge | +70 | 5 |
| `ui/desktop/src/components/BaseChat.tsx` | pass session workspace to header | +10 | 5 |
| `ui/desktop/src/ipc/channels.ts` | typed workspace invalidation event | +10 | 5 |
| `ui/desktop/src/preload.ts` | broadcast API and folder chooser default | +15 | 5 |
| `ui/desktop/src/main.ts` | folder default and multi-window broadcast | +30 | 5 |
| `ui/desktop/src/test/setup.ts` | Workspace/Electron mocks | +35 | 5 |
| `ui/desktop/src/contexts/WorkspaceContext.test.tsx` | context CRUD/switch/broadcast/clearing | 550 | 5 |
| `ui/desktop/src/components/workspaces/WorkspaceSection.test.tsx` | sidebar/actions/a11y/keyboard | 600 | 5 |
| `ui/desktop/src/components/workspaces/WorkspaceEditorDialog.test.tsx` | editor/chooser/warnings/secrets | 700 | 5 |
| `ui/desktop/src/components/SessionActionsHeader.test.tsx` | pinned-vs-active header | +120 | 5 |
| `ui/desktop/src/hooks/useNavigationSessions.test.tsx` | filters/legacy/all/resume | +200 | 5 |
| `docs/INTENT.md` and `docs/architecture.md` | authoritative intent/design | existing | 1/2/7 |
| `docs/adr/0001`–`0005` | decision records | existing | 2/7 |
| `docs/WORKSPACES.md` | user and operator manual | 500 | 7 |
| `docs/build/*` | evidence, traceability, QA/handoff | ≤1200 each | all |

## Gate plan

| Gate | Scope | Slices | Validation | Audit |
|---|---|---|---|---|
| 3 Foundation | REQ-003, 028 | canonical DTO module, skeleton service/handlers/tests compile | targeted Rust/SDK compile/schema shape | architecture drift |
| 4 Backend | REQ-001–014, 019–024, 028–030 | store → profiles → session v22 → scoped provider → prompt/list | Rust unit/integration, schema regen, fmt | data integrity + security seam |
| 5 Desktop | REQ-004–018, 022, 025–030 | context → create flow → sidebar/editor/header/filter/broadcast | typecheck + focused Vitest + scripted UI | workflow/GUI |
| 6 Hardening | all P0/P1 | negative paths, concurrency, sentinel, symlink, interruption, repair | relevant full Rust/Desktop checks | security + recovery/concurrency |
| 7 Documentation | REQ-022–030 | user/developer/custom distro docs and traceability | doc-vs-code search/check | compliance/posture |
| 8 Acceptance | all | full acceptance walkthrough and evidence closure | required commands + clippy/fmt | final negative-space sweep |

## Test and validation plan

| REQs | Level | Core proof | Evidence target |
|---|---|---|---|
| 001–003, 021–024 | Rust unit/integration/golden | default/migration/CRUD/atomic/recovery/export/import/template | Gate 4 evidence |
| 004–006, 019, 025–026 | Rust unit + Desktop component | path normalization/symlink/missing/output routing/chooser | Gate 4/5/6 evidence |
| 007–011, 017–018, 020–021 | Rust integration + React hooks | pin/resume/switch/filter/header/multi-window/delete | Gate 4/5 evidence |
| 012–014, 020, 024 | Rust security + React component | scoped distinct sentinels, relink failure, metadata-only, form clearing | Gate 4/6 evidence |
| 015–018, 027 | Vitest/testing-library | accessible sidebar/editor/list/context without duplicate store | Gate 5 evidence |
| 028 | schema generation + TypeScript | generated client exact methods/types and no forbidden imports | Gate 4/8 evidence |
| 029–030 | migration/compat + command sweep | naming, CLI unchanged, old sessions, fmt/typecheck/tests/clippy | Gate 8 QA report |

Planned commands include `source bin/activate-hermit`, targeted `cargo test -p gosling
workspace`, `just generate-acp-types`, Desktop `pnpm run typecheck` and focused `pnpm test
-- --run`, then `cargo fmt --check`, relevant Rust tests, Desktop unit suite, and
`cargo clippy --all-targets -- -D warnings` before final acceptance.

## Audit plan

| Gate | Primary lens | Secondary |
|---|---|---|
| 3 | architecture drift | invariant sync |
| 4 | dataflow integrity | security, temporal migration |
| 5 | workflow/GUI | accessibility, state transition |
| 6 | security | recovery/idempotency, concurrency, path IO |
| 7 | compliance/posture | documentation drift |
| 8 | negative space | contract/invariant sync |

## Residual risks before build

RSK-001–RSK-012 remain open. The highest-risk implementation points are task-local scope
propagation, fail-closed agent restore, session query parity, import secret rejection, and
backend/renderer new-session race closure.

## Plan-change history

None after the Gate 1 baseline.

