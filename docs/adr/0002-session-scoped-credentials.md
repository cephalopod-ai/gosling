# ADR-0002: Session-scoped credential resolution

Date: 2026-07-18
Status: accepted
Requirements affected: REQ-012–REQ-014, REQ-020–REQ-021, REQ-024, REQ-030

## Context

Gosling providers are instantiated per session, but constructors read logical config keys
from process-global `Config`. Mutating canonical keys during workspace switching would race
and would change credentials beneath resumable sessions. Rewriting every provider trait and
constructor would be high-risk and would expose credential plumbing across the provider layer.

## Decision

Credential profiles persist metadata and non-secret provider configuration only. Secret
fields are stored through `Config` under derived keys
`workspace-credential::<profile UUID>::<logical field>`. `AgentConfig` receives the
`WorkspaceService`; every saved-session provider create/recreate runs inside a strict Tokio
task-local `ConfigResolutionScope`. For provider-declared keys, the scope supplies profile
non-secret values and maps secret reads to derived secure keys before environment/global
lookup. Unmapped required profile keys fail closed. Unrelated Gosling config continues to
resolve globally.

Legacy/global-alias profiles reference the existing canonical secure keys without reading or
copying values. Workspace switching changes only the selected profile reference for future
sessions. Missing profile/secure fields abort pinned-session activation with a relink-required
error; the legacy-session fallback path remains unchanged.

## Alternatives considered

| Alternative | Why rejected |
|---|---|
| Copy selected secret into global canonical key | Considered, rejected: cross-session race and silent account mutation. |
| Store secret values in workspace JSON or session DB | Considered, rejected: violates the security boundary. |
| Call platform keyring directly from workspace code | Considered, rejected: bypasses Gosling atomic fallback/cache abstraction. |
| Add a credential parameter to every provider constructor immediately | Considered, rejected: large provider-wide churn when a strict scoped adapter preserves existing provider contracts. |

## Consequences

Provider implementations remain unchanged, but workspace-sensitive construction must always
use the scoped helper. Tokio task-local context does not flow into independently spawned
tasks; provider constructors that move credential reads into spawned tasks will require an
explicit follow-up adapter and a guard test.

## Dependency record

No new dependency; Tokio task-local support and existing Config storage are reused.

