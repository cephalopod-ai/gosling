# ADR-0008: Dedicated shell host process and preload boundary

Date: 2026-08-12
Status: accepted for implementation
Requirements affected: SHP-REQ-003–SHP-REQ-010, SHP-REQ-015–SHP-REQ-018, SHP-REQ-024–SHP-REQ-026, SHP-REQ-028

## Context

Full Desktop `main.ts` initializes Gosling settings paths, protocol registration, single-instance
locking, broad file/settings IPC, updater handlers, menus, tray, launcher, and backend leases. Its
preload exposes those capabilities to the renderer. Focused shells need authenticated ACP and the
existing managed child lifecycle, but inheriting the full entry or preload would defeat the
minimal-host security boundary and add more responsibility to an oversized orchestrator.

## Decision

Focused shells use separate Vite main, preload, and renderer entries. There is no runtime shell
branch inside full Desktop `main.ts`, and no shell window loads `src/preload.ts`. Shared code is
limited to focused modules: resolved app identity, `createMinimalShellHost`, `goslingServe`, ACP
adapters/generated types, diagnostics, lifecycle, and common presentation primitives.

The shell main process resolves the immutable product profile before Electron readiness. It sets
application paths and name before protocol registration or `requestSingleInstanceLock`, creates
one generated server secret, starts one child through `createMinimalShellHost`, and owns every
cleanup path. Full Gosling protected configuration remains authoritative; shell application and
runtime data, cache, logs, process registry, browser partition, protocol, and lock are isolated by
profile identity.

The preload exposes only the allowlist frozen in
[`shell-productization-contracts.md`](../architecture/shell-productization-contracts.md): read a
bounded runtime snapshot, retry/stop, save a redacted diagnostic bundle, prepare/confirm explicit
handoff, and open an allowlisted external destination. Identity, namespace, provisioning path,
filesystem path, updater feed, release destination, raw ACP URL, and server secret are never
renderer inputs. Main validates every IPC payload, sender, size, state transition, and destination.

Compatibility preflight order is fixed: resolved profile/provisioning identity and schema → child
readiness/authenticated ACP initialization → server-fixed identity/method capability → provisioning
read/validation → session create/resume. Unknown newer profile/provisioning/handoff schemas or
missing required methods fail before durable session creation. Initial policy bundles the exact
Gosling binary tested with the shell; external-core discovery is out of scope.

The lifecycle reducer is deterministic and generation-fenced. Retry first cleans the prior child,
secret, trust, registry, and listeners, then creates a fresh generation. Expected stop and abnormal
exit are distinct. Renderer or transport failure never transfers child ownership out of main.

## Alternatives considered

| Alternative | Why rejected |
| --- | --- |
| Add shell branches to full `main.ts` and preload | It exposes broad authority, couples shell behavior to Desktop features, and violates the router-only growth constraint. |
| Renderer starts or reconnects the backend directly | It would expose process/secret/identity authority and create split lifecycle ownership. |
| Reimplement `goslingServe` per product | It duplicates readiness, TLS pinning, diagnostics, registry, and cleanup behavior. |
| Permit compatible external Gosling installations | Discovery, signature trust, version skew, migration, and independent lifecycle exceed the first release. |
| Create a session then report incompatibility | It can leave durable state created under an unsupported contract. |

## Consequences

Shell startup has one auditable trust boundary and can be packaged independently while preserving
Rust provisioning/policy authority. Common components must depend on narrow interfaces instead of
full `window.electron`. Some existing Desktop components will not be reusable without extraction;
that is intentional rather than a reason to widen preload authority.

## Dependency record

No new dependency. Electron, existing Gosling Desktop lifecycle modules, generated ACP SDK/types,
and React/Vitest/Playwright infrastructure are reused.
