# ADR: Tagteam is a workflow with a deterministic authority boundary

**Status:** Accepted and implemented for the Phase 1 foundation; live
integration remains gated.
**Date:** 2026-07-12.

## Decision

Gosling will integrate the future Tagteam control plane as a session workflow,
not as an LLM provider. A Tagteam-specific workflow service will own normalized
contracts, persistence, state reduction, and capability policy. A future MCP
adapter will implement the workflow's client port.

The selected Gosling provider and model act as the Run Steward. The steward is
advisory: it receives bounded normalized evidence and cannot edit the
repository, construct commands, broaden scope, approve recovery, or recursively
invoke Tagteam. Deterministic state and error rendering remain available when
the steward is absent or invalid.

## Rationale

The existing provider abstraction models one completion stream. Tagteam is a
long-running external workflow with independent role models, progress,
findings, recovery, and process ownership. Treating its team profiles as models
conflates these domains and prevents reliable monitoring and authority
separation.

## Consequences

- Existing sessions default to the Standard workflow.
- The current Tagteam provider remains a compatibility path until a later
  parity and migration decision.
- Phase 1 introduces no visible workflow control and performs no live Tagteam
  calls.
- Live integration requires a versioned producer contract and shared
  conformance fixtures.
