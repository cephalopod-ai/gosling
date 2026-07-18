# Gate 3 Architecture Drift audit — Workspaces foundation

## CI verdict

Verdict: PASS — no new architecture-drift violation in the Gate 3 change set.

Scope: canonical Workspaces DTOs, SDK re-export, and Gosling workspace module boundary.
Intent basis: declared `docs/INTENT.md`, `docs/architecture.md`, and ADR-0001–0005.
Baseline: no architecture graph baseline; trend and AID-013 are unavailable.
Validation: SDK tests and Gosling compile check passed; generated ACP registration is
deliberately scheduled for Gate 4 and is not represented as shipped.

## Trace map

| Implementation | Intent trace | Owner | Test |
|---|---|---|---|
| `gosling-sdk-types/src/workspace.rs` | REQ-003/028; ADR-0004 | canonical DTO contract | three real serialization/projection tests |
| `gosling-sdk-types/src/custom_requests.rs` re-export | ADR-0004 generated-contract seam | SDK custom requests | crate compile/test |
| `gosling/src/workspace/mod.rs` | architecture module contract | workspace application boundary | Gosling compile check |

## AID-001..014 disposition

| Code | Result |
|---|---|
| AID-001 Partial implementation | Not a finding — feature status is building and Gate 3 scope is explicitly foundation-only. |
| AID-002 Duplicate implementation | Held — no TypeScript/local/backend duplicate Workspace model was added. |
| AID-003 Abandoned architecture | Not a finding — later modules have scheduled gates and trace rows, not shipped claims. |
| AID-004 Accidental architecture | Held — every new file traces to REQ-003/028 and ADR-0004. |
| AID-005 Dead interface | Held for scope — request structs are definitions, not registered/exposed endpoints yet. |
| AID-006 Orphan service | N/A — no service implementation exists in this gate. |
| AID-007 Unused abstraction | Held — the SDK DTO module is the declared cross-language contract seam, not a speculative interface. |
| AID-008 Excessive indirection | Held — one re-export joins the existing SDK convention; no pass-through call chain exists. |
| AID-009 Declared-design contradiction | Held — canonical DTO ownership and module direction match ADR-0004. |
| AID-010 Documentation drift | Held — docs describe current DTO foundation as building, not complete. |
| AID-011 Testing gap | Held for Gate 3 — real serialization and secret-metadata shape tests import the real DTO implementation. |
| AID-012 Ownership ambiguity | Held — SDK module owns public shape; Gosling module re-exports it. |
| AID-013 Coupling growth | N/A — no architecture baseline. |
| AID-014 Invariant violation | Held in scope — credential profile output type has no secret value field and its test pins that property. |

## Validation limits

Only the Gate 3 change set was evaluated. Persistence, handler consumption, generated SDK,
session pinning, and Desktop state remain planned and receive their own gate audits.

