# Architecture Audit - 2026-07-09

Scope: repository `repo-makeover/gosling` at `24acf148f1f08927c2510ccc11df0941b96f5002` on branch `main`.

Ground rules: this was an `audit_only` architecture pass using local source, repository instructions, and local audit skills. It did not claim runtime reproduction where none was performed, and it did not run build/test/clippy flows because this task was audit logging rather than code repair.

## Applied skills

- `audit-architecture-seam`
- `audit-architecture-drift`
- `audit-architecture-nodejs`

Skill mode notes:
- `audit-architecture-drift` ran in bootstrap mode because the repository does not contain a `.architecture/` invariant registry or an equivalent machine-checkable intent map.
- `audit-architecture-nodejs` was applied only to the Electron main/preload boundary, where Node/Electron privilege seams are load-bearing.

## Summary table

| Field | Value |
|---|---|
| Repository | `repo-makeover/gosling` |
| Audit mode | `audit_only` |
| Domain | `architecture` |
| Overall posture | `partial` |
| High | `0` |
| Medium | `3` |
| Low | `0` |
| Bootstrap drift mode | `yes` |

## Seam inventory

| Seam | Current owner | Assessment |
|---|---|---|
| Product intent and architectural boundaries | `README.md`, `documentation/GOOSE_COMPATIBILITY.md`, docs pages | Narrative intent exists, but no durable invariant registry |
| HTTP reply entrypoints to agent/session orchestration | `crates/gosling-server/src/routes/reply.rs`, `crates/gosling-server/src/routes/session_events.rs` | Boundary violation; adapters own shared orchestration logic |
| Session history override lifecycle | route layer + `history_override.rs` | Partial extraction only; commit/rollback policy still duplicated at the edge |
| Electron renderer to privileged main-process contract | `ui/desktop/src/preload.ts`, `ui/desktop/src/main.ts` | Stringly typed and drift-prone; no single source of truth |
| Desktop process lifecycle, updater, windowing, settings, file broker | `ui/desktop/src/main.ts` | Centralized privileged owner with broad mixed responsibilities |

## Invariants checked

| Invariant | Result | Basis |
|---|---|---|
| Architectural intent should be declared in a durable, reviewable, machine-checkable form during active modularization | `Fail` | no `.architecture/` registry; only narrative docs were found |
| HTTP route adapters should not depend on sibling route internals for shared business flow | `Fail` | `session_events.rs` imports helpers and event types from `reply.rs` |
| Equivalent reply entrypoints should share one orchestration service for override/rollback/streaming/telemetry rules | `Fail` | both route files duplicate the same reply loop and state handling |
| Electron main/preload contract should have one authoritative definition | `Fail` | mirrored string channel lists across `main.ts` and `preload.ts`, plus orphaned channels |
| Renderer-facing preload API should not expose dead privileged methods | `Fail` | `hideWindow` and `getBinaryPath` are exposed without matching main handlers or usages |

## Findings

| ID | Severity | Finding | Evidence | Recommendation |
|---|---|---|---|---|
| `AID-GOS-001` | `Medium` | No declared architecture invariant registry, so current modularization and hardening work has no durable intent anchor | `documentation/INDEX.md`, `README.md`, repo search for `.architecture` | add a minimal invariant registry and ADR-style ownership map |
| `ARC-GOS-001` | `Medium` | Reply orchestration is duplicated across route adapters and one route now depends on sibling route internals | `reply.rs`, `session_events.rs` | extract one reply orchestration service and keep routes as thin transport adapters |
| `NJS-GOS-001` | `Medium` | Electron main/preload IPC contract is stringly typed and already drifted | `main.ts`, `preload.ts`, repo search for orphaned channels | move channel names and payload types into a shared contract module and generate thin bindings |

### AID-GOS-001: No declared architecture invariant registry

Severity: Medium  
Confidence: Confirmed  
Domain: Architecture

Evidence:
- Repository search found no `.architecture/` directory or equivalent invariant registry.
- `documentation/INDEX.md` points to narrative architecture docs under `documentation/docs/gosling-architecture/`, but not to a durable intent ledger.
- `README.md` declares strategic goals such as "lighter goose", side-by-side coexistence, compatibility fallbacks, modularization, and hardening.
- `documentation/GOOSE_COMPATIBILITY.md` declares a compatibility rule set, but it is a focused compatibility note rather than a broader architectural boundary map.

Observed behavior:
- Intent is described in prose, but the repo does not define machine-reviewable invariants such as ownership boundaries, allowed cross-layer dependencies, or required adapters for compatibility paths.

Why this matters:
- This repository is actively rebranded, modularized, and hardened. Without a durable invariant registry, refactors can remain locally reasonable while still drifting from the intended product shape.
- The other findings in this audit already show that implicit intent is not enough to keep boundaries stable.

Impact:
- Architectural review becomes person-dependent instead of repository-enforced.
- Future merge campaigns can reintroduce route coupling or Electron boundary drift without tripping a durable contract.

Recommended mitigation:
- Create a minimal `.architecture/` registry with:
  - component owners
  - forbidden dependency edges
  - transport-vs-service boundary rules
  - Electron privilege-boundary rules
  - compatibility-adapter invariants for Goose fallback behavior
- Add a small ADR or invariant record for each major seam touched during the ongoing modularization campaign.

Next action:
- `plan-architecture-invariants`

### ARC-GOS-001: Reply orchestration is duplicated across route adapters

Severity: Medium  
Confidence: Confirmed  
Domain: Architecture

Evidence:
- `crates/gosling-server/src/routes/session_events.rs:5` imports `get_token_state`, `track_tool_telemetry`, and `MessageEvent` from sibling route module `reply.rs`.
- `crates/gosling-server/src/routes/reply.rs:270-421` owns override application, rollback handling, stream startup, stream iteration, telemetry, token-state emission, and finish publication.
- `crates/gosling-server/src/routes/session_events.rs:405-580` repeats the same orchestration flow with a different output transport.
- Both routes also share history-override policy via `history_override.rs`, which means partial extraction has happened, but the higher-order orchestration rule set still lives in two adapters.

Observed behavior:
- The legacy `/reply` SSE endpoint and the session-event bus reply endpoint each implement their own copy of the reply lifecycle.
- The newer route depends on sibling adapter internals instead of a service-layer owner for shared behavior.

Failure mechanism:
- Any change to rollback timing, early-provider-failure detection, token-state publication, telemetry, or finish semantics must be made twice.
- When one route learns a repair first, the other route can silently diverge.

Impact:
- Regression risk is elevated on the highest-traffic session path in the repo.
- Architectural ownership is inverted: transport adapters own domain workflow, and one adapter now acts as a utility module for another.

Recommended mitigation:
- Extract a `reply_orchestrator` or equivalent service that owns:
  - session lookup and `SessionConfig` assembly
  - override staging / rollback / commit rules
  - agent stream lifecycle
  - message/notification/history event normalization
  - token-state and telemetry hooks
- Keep `reply.rs` and `session_events.rs` responsible only for transport concerns:
  - direct SSE framing for `/reply`
  - event-bus publication and replay semantics for `/sessions/{id}/events`

Change safety note:
- Preserve current behavior by extracting the shared loop first and leaving transport-specific finish/error publication in the adapters until tests are in place.

### NJS-GOS-001: Electron IPC contract is stringly typed and already drifted

Severity: Medium  
Confidence: Confirmed  
Domain: Architecture

Evidence:
- `ui/desktop/src/main.ts` is a 3082-line privileged main-process module with 43 `ipcMain.handle(...)` or `ipcMain.on(...)` entrypoints between lines `1823` and `2974`.
- `ui/desktop/src/preload.ts` exposes a large `ElectronAPI` surface and mirrors channel names directly with `ipcRenderer.invoke/send/sendSync`.
- `ui/desktop/src/preload.ts:308-320` exposes generic `on`, `off`, and `emit` channel access in addition to named methods.
- Repo search found no shared IPC contract module under `ui/desktop/src`.
- Repo search found orphaned preload channels:
  - `ui/desktop/src/preload.ts:200` exposes `hideWindow`, but no matching `ipcMain` handler exists in the repo.
  - `ui/desktop/src/preload.ts:215` exposes `getBinaryPath`, but no matching `ipcMain` handler exists in the repo.
- Repo search also found no current renderer usage of those orphaned methods, which means dead contract surface is already accumulating.

Observed behavior:
- The desktop privilege boundary is maintained by manually duplicated string channel names and mirrored method lists across `main.ts` and `preload.ts`.
- Contract drift is already present even before new features are added.

Failure mechanism:
- Main/preload/renderer changes can compile while still shipping a broken or partially broken privileged contract.
- The broad main-process owner makes it hard to reason about whether a change is lifecycle, updater, file-broker, settings, or transport work.

Impact:
- Privileged desktop changes are harder to review and modularize safely.
- Orphaned API surface raises the chance of future renderer code binding to nonexistent main-process capabilities.

Recommended mitigation:
- Introduce a shared IPC contract module that owns:
  - channel names
  - request/response payload types
  - event channel names
  - allowed generic event usage, if any
- Generate or hand-wrap thin main/preload bindings from that contract.
- Split the privileged main-process implementation into capability modules such as:
  - window lifecycle
  - backend lease and ACP connectivity
  - settings and persistence
  - file broker
  - updater

## Validation limits

- This was a static audit only; no runtime Electron session or HTTP reply replay was executed.
- No build, test, or clippy runs were performed in this audit turn because the task was to generate an architecture audit log.
- Intent-relative findings beyond the bootstrap drift finding were kept conservative because the repo lacks a formal invariant registry.
