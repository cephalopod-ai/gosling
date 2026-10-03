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

## 2026-10-02 amendment: explicit authentication bindings

The operator authorized provider and MCP authentication controls for an existing chat and for
workspace defaults. Workspace edits affect new chats only. A chat can explicitly disconnect its
provider without adopting global credentials; `authentication.v1` records that policy in host-owned
session extension data. A compatible replacement provider is constructed before one atomic SQL
update changes its profile reference and authentication policy, then the live provider is swapped.
ACP mutations reserve the session operation gate and reject an active run.

MCP bindings hold an opaque account UUID, destination fingerprint, declared secret-field names,
and disconnection state. Values remain in Config secure storage under
`scoped-authentication::<account UUID>::<field>`. OAuth uses an account-and-URI-derived store
through initialization, browser sign-in, refresh, and clearing failed tokens. The caller's selected
store is retained throughout the flow. Editing credentials allocates a fresh account reference so
a workspace or sibling chat's existing account is not overwritten. Disconnect retains credentials,
stops the selected client, and blocks restoration; a failed replacement also stops the old client.
Changed extension destinations and missing scoped secrets fail closed. Unconfigured legacy
bindings continue to use their existing app defaults.

Scoped MCP sign-ins require gosling-managed tools. Active session MCP mutations are rejected for
providers that manage connections outside gosling; those providers can inherit workspace
disconnection defaults when a new chat starts. A shell's provisioning remains authoritative.
Import strips host authentication references, and workspace exports omit scoped account references.
Retained, unreferenced account secrets are not automatically garbage-collected. These source changes
have focused Desktop and typecheck coverage; live OAuth/native-provider replay remains a separate
validation step.
