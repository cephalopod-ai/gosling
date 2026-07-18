# Gate 0 targeted Architecture & Seam audit

## Executive verdict

No critical defect blocks a Workspaces implementation, but two confirmed architecture
seams must shape it. `SessionManager` is already an oversized persistence hub, and
provider credentials are ambiently coupled through global `Config` during per-session
provider construction. Adding workspace storage inside the session manager or swapping
global secrets would compound those defects. A renderer secret-storage surface was
also identified and escalated to Security; it must not be reused.

## Scope

- Repository/branch/commit: Gosling, `main`, `9b9571febf06f7fc6dfddea32267b5c0d325b369`
- Lens: `audit-architecture-seam` 3.2, read-only
- Focus: Workspaces-adjacent renderer, IPC, ACP, provider, config, and session seams
- Budget: approximately 25 focused files and 35 static searches/reads
- Stack adaptation: Rust monorepo + Electron/React SPA + code-generated ACP SDK
- Validation limit: no target code was executed; dynamic provider registry wiring was
  inspected at its registry/factory sites but not exhaustively walked for every provider.

## Seam inventory

| Module | Layer | Responsibility/owner | Main seam | Risk |
|---|---|---|---|---|
| `ui/desktop/src/components/Layout/AppLayout.tsx` | UI | persistent shell | navigation/chat/artifact contexts | low |
| `ui/desktop/src/components/Layout/NavigationPanel.tsx` | UI | navigation/session presentation | session list + route state | medium |
| `ui/desktop/src/sessions.ts` + `acp/sessions.ts` | UI service | session requests/mapping | ACP new/load/list metadata | medium |
| `ui/desktop/src/main.ts` | Electron infra | desktop IPC | directory/settings/events | medium |
| `crates/gosling-sdk-types` + `ui/sdk` | contract/generated | custom ACP DTO source/output | Rust-to-TypeScript schema generation | medium |
| `crates/gosling/src/acp/server/*` | application | session orchestration | ACP requests to session/provider services | high |
| `crates/gosling/src/session/session_manager.rs` | persistence | sessions DB schema and all session data | SQLite | high |
| `crates/gosling/src/config/base.rs` | infrastructure | config and secrets | keyring/protected file/environment | high |
| `crates/gosling/src/providers/*` | adapters | provider construction | registry + ambient `Config` | high |

## Boundary map

| Surface | Intended boundary | Current enforcement | Status |
|---|---|---|---|
| Workspace metadata | backend-owned versioned store | absent | design required |
| Credential secret | `Config` secure storage only | keyring/file abstraction | held if reused |
| Session identity/context | `SessionManager` row and response metadata | schema v21 | migration required |
| Provider credential choice | pinned session context | global `Config` reads | finding ARC-GOS-002 |
| Sidebar behavior | focused context/service | monolithic navigation component | design constraint |
| Generated contract | canonical Rust DTO + generator | existing SDK pipeline | held |

## Findings

### ARC-GOS-001: SessionManager is an oversized persistence hub

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `crates/gosling/src/session/session_manager.rs:240-265` — `SessionUpdateBuilder` owns updates across identity, working directories, provider/model, usage, workflow, archive, and project state.
- `crates/gosling/src/session/session_manager.rs:1478-1539` — the same file owns the complete schema migration runner.
- `crates/gosling/src/session/session_manager.rs` — 5,844 hand-written lines at the audited baseline.

Observed behavior:
- One file owns schema creation/migration, session CRUD, conversation persistence,
  history paging/search, summaries, imports, and update construction.

Expected boundary:
- New workspace metadata should have a dedicated store/service with a narrow contract;
  only session snapshots belong in `SessionManager`.

Failure mechanism:
- Adding workspace CRUD, file recovery, imports, templates, credential metadata, and
  validation to this hub would increase unrelated responsibilities and make migration
  and state changes harder to test independently.

Break-it angle:
- A workspace write failure or malformed workspace record should be testable without
  constructing or mutating `sessions.db`; that is not possible if both stores share the hub.

Impact:
- Increased regression surface and harder isolation, rather than an existing runtime outage.

Operational impact:
- Blast radius: Repo
- Side-effect class: DB
- Reversibility: reversible
- Operator visibility: log-only
- Rerun safety: unknown

Adjacent failure modes:
- ARC-004 wrong ownership and ARC-020 untestable core if workspace policy is added here.

Recommended mitigation:
- Remediation patterns: extract store; repository boundary.
- Minimal repair: create a dedicated workspace store/service and keep only nullable
  workspace snapshot columns in the session persistence model.
- Local guardrail: independent store tests with an injected temporary data directory.
- Behavior test: workspace CRUD/default migration tests run without a session database.

Implementation assessment:
- Complexity: persistence_recovery
- Cost: M
- Cost drivers: modules, tests, migrations
- Nominal implementation agent: codex
- Rationale: the new bounded module is moderate work but prevents further expansion of a high-risk file.

Validation:
- Workspace store tests and a session migration test cover their separate contracts.

Non-goals:
- Do not refactor all existing `SessionManager` responsibilities in this feature.

### ARC-GOS-002: Provider credentials are hidden global inputs to session construction

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `crates/gosling/src/config/base.rs:495-502` — `Config::global()` supplies a process-global config instance.
- `crates/gosling/src/agents/agent.rs:3133-3143` — session provider recreation calls the global extension/config path and constructs a provider without an explicit credential context.
- `crates/gosling/src/agents/agent.rs:3172-3179` — resume begins from global `Config` when saved values are absent.
- `crates/gosling/src/agents/agent.rs:3223-3261` — failed saved-provider construction can select and construct a global fallback provider.

Observed behavior:
- Provider construction is per session, but credential lookup is an ambient process
  input. Resume may replace the saved provider with the global default.

Expected boundary:
- A workspace-pinned session supplies an immutable credential-profile reference to
  provider construction; a missing profile fails visibly and never adopts another profile.

Failure mechanism:
- Implementing workspace switching through global secret mutation would make the
  provider constructor observe whichever workspace wrote last, creating cross-session
  credential races and silent resume drift.

Break-it angle:
- Start session A with profile A, switch to workspace/profile B, then recreate A's
  provider. The current constructor contract has no parameter that distinguishes A.

Impact:
- A naive Workspaces implementation could authenticate a session as the wrong account
  or silently change its provider on resume.

Operational impact:
- Blast radius: Workflow
- Side-effect class: network
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- Security credential crossover; concurrency race; workflow UI falsely implying a pinned workspace.

Recommended mitigation:
- Remediation patterns: explicit context; scoped dependency injection; fail-closed resume.
- Minimal repair: pass a profile-resolution scope through provider creation, map
  logical provider fields to namespaced secure keys, and disable fallback for pinned sessions.
- Local guardrail: never write a profile secret into canonical global keys.
- Behavior test: concurrent/scoped provider construction resolves distinct sentinel values;
  switching active workspace leaves the first session unchanged.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: L
- Cost drivers: modules, tests, migrations, runtime_verification
- Nominal implementation agent: codex
- Rationale: the repair crosses config, provider factory, ACP session orchestration, and resume behavior.

Validation:
- Credential-scope unit tests, session pin/resume tests, and missing-profile recovery tests.

Non-goals:
- Do not redesign providers unrelated to secret lookup or change legacy-session fallback behavior.

## Cross-lens escalation

| Evidence | Primary lens | Secondary lens | Why |
|---|---|---|---|
| `ui/desktop/src/utils/settings.ts:8-24,49-60` and `ManagedSecretProfilesSection.tsx:260-401` | Architecture stub | Security | Existing renderer settings profiles contain and persist raw values. Workspaces must not reuse this surface. |
| ARC-GOS-002 | Architecture | Security / Concurrency / Workflow-GUI | Credential crossover and silent fallback affect trust, races, and operator belief. |

## ARC-001..025 inventory disposition

| Code | Disposition in focused surface |
|---|---|
| ARC-001 | Finding ARC-GOS-001. |
| ARC-002 | Not Confirmed — Desktop consumes ACP/local types as directed; no workspace code exists yet. |
| ARC-003 | Finding ARC-GOS-002 (hidden ambient credential input). |
| ARC-004 | Not Confirmed — workspace ownership absent; proposed owner recorded. |
| ARC-005 | Not Confirmed — no focused import cycle found; transitive graph not exhaustively resolved. |
| ARC-006 | Not Confirmed — callers use the provider trait/factory; credential context is the separate hidden-coupling defect. |
| ARC-007 | Not Confirmed — provider diversity is registered explicitly; not fully audited per provider. |
| ARC-008 | Not Confirmed — provider capabilities are outside this focused change; credential selection is not a flattened provider capability. |
| ARC-009 | Not Confirmed — workspace policy will live in a service, not the persistence mechanism. |
| ARC-010 | Not Confirmed — workspace rules do not exist; renderer secret persistence escalated to Security. |
| ARC-011 | N/A — no passive collector is part of the Workspaces surface. |
| ARC-012 | Not Confirmed — keyring has an explicit protected fallback. |
| ARC-013 | Not Confirmed — ACP custom DTOs have an existing schema-generation contract. |
| ARC-014 | Not Confirmed — current Desktop working-directory changes delegate through ACP. |
| ARC-015 | Not Confirmed — nullable legacy-session behavior is required compatibility, not dead-weight. |
| ARC-016 | Not Confirmed — `SessionManager` is the single writer/owner of `sessions.db`; workspace storage will have one owner. |
| ARC-017 | Not Confirmed — focused ACP messages use declared request/metadata contracts; renderer events are typed channel constants. |
| ARC-018 | Covered by ARC-GOS-002; no separate duplicate count. |
| ARC-019 | Not Confirmed — explicit server/agent construction owns initialization order in the focused path. |
| ARC-020 | Adjacent to ARC-GOS-001; dedicated injected store is the guardrail. |
| ARC-021 | Not Confirmed — owned SDK/local DTOs isolate Desktop from generated OpenAPI types. |
| ARC-022 | Not Confirmed — new credential profiles must not become a second raw-secret abstraction; metadata/security ownership is distinct. |
| ARC-023 | N/A — no production monkey patching in the focused surface. |
| ARC-024 | Not Confirmed — generated SDK has an approved generator; generated files will not be hand-edited. |
| ARC-025 | Not Confirmed — path and secret validation will be centralized in workspace/config services. |

## Break-it review and guardrails

- Provider swap: explicit profile scope and a strict missing-profile error are required.
- Optional keyring absence: preserve the existing protected file fallback.
- Shared store: workspace writes have one backend owner; sessions store snapshots only.
- Generated surface: regenerate from canonical custom DTOs and verify the diff.
- UI bypass: backend validates folders, workspace identity, and credentials independently.
- Deleted metadata: session snapshots preserve history; deletes never touch session rows or files.

## Validation limits

- Static inspection only under the audit skill's read-only authority.
- Import edges were found through repository search; no complete transitive cycle tool was run.
- Dynamic provider registry wiring means clean non-findings for every provider are capped;
  targeted provider/profile tests are required during implementation.
- Security, path, concurrency, and workflow behavior will receive dedicated Gate 6 audits.

## Final confidence

High for the two structural findings because their source seams are directly evidenced;
medium for absence claims due to dynamic registry wiring and focused rather than full-repo scope.
