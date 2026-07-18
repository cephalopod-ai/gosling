# Gate 2 Internal API Contract audit — Workspaces design

## Verdict

The planned Workspaces boundaries are typed, single-owner, versioned where durable, and
have real producer/consumer pinning tests in the execution plan. One design ambiguity was
found and repaired during the audit: product outputs now explicitly require a non-empty
list with exactly one default in REQ-005 and the I/O malformed-input table. No unresolved
contract finding remains before the foundation gate.

## Scope

- Lens: `audit-contract-internalapi` 0.2, read-only
- Inputs: `docs/INTENT.md`, `docs/architecture.md`, ADR-0001–0005,
  `docs/build/io-contract.md`, execution and traceability plans
- Existing seam evidence: SDK custom requests/generator, ACP dispatch, Config secret
  storage, session new/load/list/restore paths, Desktop ACP/session adapters
- Sampling: all planned P0/P1 Workspaces boundaries; unrelated existing Gosling APIs excluded
- Runtime paths: none executed because this gate audits design contracts, not implementation

## Boundary inventory and cards

| Boundary | Risk | Input/output contract | Error contract | Lifecycle owner | Pinning test |
|---|---|---|---|---|---|
| React form/context → Desktop ACP adapter | P1 | generated `Workspace*` DTO/request types | typed rejection mapped to inline/toast state | WorkspaceContext | real adapter/context tests |
| generated client → ACP handler | P0 | canonical `gosling-sdk-types::workspace` | stable validation/not-found/conflict/credential/storage classes | ACP transport only | schema/client generation + handler tests |
| ACP handler → WorkspaceService | P0 | owned mutation/operation types | `WorkspaceError` variants, safely mapped | WorkspaceService | handler/service integration tests |
| WorkspaceService → WorkspaceStore | P0 | versioned store state/mutation closure | parse/version/lock/write/recovery variants | WorkspaceStore | real temp-dir atomic/migration tests |
| credentials → Config secure storage | P0 | derived profile/logical-field map; write-only values | missing/unsupported/storage without secret echo | credentials service + Config | sentinel and two-profile scope tests |
| new session → SessionManager v22 | P0 | `WorkspaceSessionContext` + snapshot fields | update failure cleans new row | SessionManager | v21/fresh/pin/copy tests |
| Agent → scoped Config → provider constructor | P0 | pinned profile ID to strict resolution scope | relink-required, no global fallback | Agent/workspace credentials | real scoped Config/provider-construction tests |
| SessionManager → response metadata → Desktop Session | P1 | nullable IDs/names/context status | legacy null preserved | response builder/mappers | exact metadata and legacy tests |
| workspace mutation → Electron broadcast → peer context | P2 | typed invalidation event, no payload state | refresh failure visible | WorkspaceContext | two-listener broadcast test |

Authn/authz is not a Workspaces boundary in the local Desktop scope. Trace correlation is
not a modeled product contract; operations propagate stable workspace/profile/session UUIDs
without logging secret-bearing inputs.

## Selected paths

| Path | Branches inspected | Replay target |
|---|---|---|
| canonical create/activate/new chat | valid folders/profile; persisted active; pinned session | workspace service + ACP integration test |
| controlled path rejection | missing primary; traversal/symlink; unknown profile field | validation table tests |
| degraded resume | workspace deleted; profile deleted; legacy null | session resume tests |
| interrupted persistence | old main + stale temp; absent main + valid temp; malformed main | store recovery tests |
| active-switch race | request captures workspace A; active changes to B before backend create | delayed create integration test |
| secret-bearing import | token/password/key fields at nested levels | import sentinel negative test |

No randomized runtime path was used at the design gate; Gate 6 will record seeds/hashes for
any fuzz/property cases.

## IAPI-001..016 disposition

| Code | Result |
|---|---|
| IAPI-001 Untyped boundary | Held by design — all material requests/responses use canonical SDK DTOs; arbitrary JSON is confined to a versioned import parser. |
| IAPI-002 Persistence DTO leakage | Held — store envelope is internal; public responses return canonical DTOs. |
| IAPI-003 Provider DTO leakage | Held — provider registry metadata is mapped into owned profile DTOs. |
| IAPI-004 Duplicate schema definitions | Held — Rust SDK DTOs generate TypeScript types; Desktop creates no local Workspace model. |
| IAPI-005 Error/result inconsistency | Held by I/O taxonomy; handler mapping tests are planned. |
| IAPI-006 Error collapsed | Held — validation, not-found, conflict, unavailable, credential, and storage remain distinguishable. Raw provider detail is intentionally sanitized. |
| IAPI-007 Service bypass | Held — sidebar/context use ACP; handlers call WorkspaceService; no Desktop/store reach-in. |
| IAPI-008 Hidden global coupling | Addressed by ADR-0002 — active/profile identity is explicit; scoped Config is bounded to provider construction. |
| IAPI-009 Rules in multiple layers | Held — UI mirrors warnings; WorkspaceService/validator owns enforcement. |
| IAPI-010 Versionless evolution | Held — store v1, session v22, and canonical schema generation are explicit. |
| IAPI-011 Nullable drift | Held — full-replacement workspace mutations make null/absence semantics explicit; session fields are nullable only for legacy/deleted snapshots and mappers guard them. |
| IAPI-012 Construction/lifecycle violation | Held — service generates IDs/timestamps/defaults and enforces only-workspace/default-output rules. |
| IAPI-013 Contract test absent/mock-on-mock | Open verification obligation, not a finding — execution plan requires tests importing real store/service/Config implementations and exact DTO shapes. |
| IAPI-014 Trace/provenance loss | N/A for request trace IDs; stable entity/session IDs cross every material seam. |
| IAPI-015 Policy decision not propagated | Held — all Desktop mutations and session creation funnel through service/ACP boundaries. CLI stays legacy and cannot mutate workspaces. |
| IAPI-016 Validation drift | Held — editor validation is advisory; backend validator is authoritative for create/update/import/session preparation. |

## Smallest hardening slice

Gate 3 must introduce the canonical DTOs, service/store traits or concrete boundaries, typed
handler registrations, and compiling real-contract test scaffolds before persistence logic.
Gate 4 may not add a renderer-only model or direct Config/store access from handlers.

## Validation limits

- This is a design audit; “held” means the written contract closes the gap, not that code
  or tests are verified.
- Provider constructor task spawning remains a specific ADR-0002 risk until scope tests run.
- Existing non-Workspace internal APIs were not audited.

