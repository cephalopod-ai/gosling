# ADR-0010: Project-shell consumer composition topology

Date: 2026-08-13
Status: proposed — pending operator acceptance (R1 architecture review; see
`pre-gui-backend-implementation-plan.md` PG-11 and `project-shell-readiness-plan.md` §4.3)
Requirements affected: SHP-REQ-033, SHP-REQ-039–SHP-REQ-042

## Context

`ui/desktop/shell.html:14` always loads `src/shell/renderer.ts`; `vite.shell.renderer.config.mts:9`
builds that one fixed HTML entry; `forge.config.ts:8-44` branches `viteEntries` only on the boolean
`product.shell`, selecting a literal `src/shell/*` or `src/main.ts`/`src/renderer.tsx` triple. Neither
`ResolvedShellProductProfile` nor `ShellBuildManifest` (`ui/desktop/src/shell/profile.ts:16-62`)
carries a renderer entry, declared capability set, or consumer identity — the resolver
(`ui/desktop/scripts/shell-profile.js:550`) only ever produces product/compatibility/asset/
update/distribution fields. `repositoryRoot()` and the package scripts require a Gosling checkout;
`docs/SHELL_PRODUCTS.md` instructs adding a profile inside this repository. These are readiness
findings PSR-001/SHP-DEF-021 and PSR-010/SHP-DEF-029: no project can supply a renderer, and no
project can consume the foundation from outside a Gosling checkout, without editing Gosling source.

The parent plan (`project-shell-readiness-plan.md` §4.3) requires R1 to choose between two
composition models and forbids "copy the fixture and edit Gosling" as an option:

1. **Separate repository consumer package** — a project repository depends on a versioned,
   pinned shell-kit/build-interface package published by Gosling, and supplies its own renderer,
   domain adapter reference, assets, and tests entirely outside this repository.
2. **Isolated workspace consumer package** — a project package lives under an approved consumer
   root inside this monorepo (e.g. `consumers/<name>/`) but cannot import or edit host modules
   (`src/shell/*`, `src/main.ts`, `forge.config.ts`, `vite.shell.*.config.mts`).

`SHP-ASM-029` records "separate repo preferred, not accepted" as the standing assumption at
planning time, and `SHP-RSK-031`/`SHP-RSK-039` name the un-decided topology as a high/critical risk
blocking R1 exit. This ADR resolves the comparison; it does not itself constitute the operator
acceptance the parent plan requires before R2 implementation may begin.

## Decision

Adopt the **separate repository consumer package** as the primary supported topology, with an
**isolated workspace consumer package** retained only as the fixture/conformance mechanism used
inside this repository during R2/R5/R8 (per `project-shell-readiness-plan.md` §5 R2/R8 milestones,
which require both an in-tree fixture and a third consumer built with no Gosling source edits).
Concretely:

- Gosling publishes a versioned **shell-kit build interface**: generated ACP/SDK types
  (`@repo-makeover/gosling-sdk`, already consumed by `acpRuntime.ts`), a resolver contract
  compatible with `resolveProfile()`/`resolveForgeProjection()`, and — once R5 exists — the
  `ShellRuntimeProvider`/`ShellHostApp` package. No Gosling host source (`forge.config.ts`,
  `vite.shell.*.config.mts`, `src/shell/*`) changes per consumer.
- A **consumer manifest v1** (frozen in this ADR's companion schema addendum; see
  `docs/architecture/shell-productization-r1-contracts.md`) is the only new build input. It names:
  consumer schema version, required shell-kit/core version range (exact-pin initially, per
  `SHP-ASM-012`), the product profile path (existing `ResolvedShellProductProfile`), a renderer
  entry, an optional domain-adapter descriptor reference (ADR-0012), and the declared shell-kit
  capabilities the renderer uses. The renderer entry is validated by containment against an
  **approved consumer root**, but — mirroring `APPROVED_PROFILE_ROOTS` in
  `ui/desktop/scripts/shell-profile.js:170-174`, which is a source-controlled constant in the
  trusted resolver, never a field the profile itself supplies — the approved root is derived from
  trusted package/workspace registration (a resolver-side allowlist keyed by `consumerId`, or the
  verified installed location of the pinned shell-kit-consuming package) and is never read from the
  manifest under evaluation. A manifest that declared its own containment boundary could simply
  declare a boundary wide enough to contain any path, which is not a security boundary. The manifest
  is resolved and hashed alongside the product profile, exactly as `ShellBuildManifest` already
  embeds `profileHash` today.
- `forge.config.ts`/`vite.shell.renderer.config.mts` are extended, not replaced: the branch on
  `product.shell` becomes a branch that additionally resolves the renderer entry from the consumer
  manifest when one is declared, falling back to the current fixed `src/shell/renderer.ts` fixture
  when none is (preserving `SHP-ASM-023` default-Gosling byte-equivalence). Implementation of this
  branch is R2 (PG-21/PG-22), not this ADR.
- The in-tree isolated-workspace fixtures (current `src/shell/renderer.ts`, and the second
  structurally different consumer required by PG-23/SHP-REQ-041) become consumer-owned packages
  under an approved root rather than direct host source, proving the same manifest contract the
  external topology uses before an external repository is exercised.
- A private/unpublished consumer must be supportable without forking Gosling: the shell-kit
  interface is versioned and consumed by reference (package registry or pinned git dependency), not
  copied.

## Alternatives considered

| Alternative | Why rejected |
| --- | --- |
| Isolated workspace only (no separate-repository support) | Contradicts the stated mission that DAWES/math/Project ABC teams own their own repository; `SHP-ASM-039`/PSR-010 require proof a consumer works with no Gosling checkout at all. |
| Keep the current model: add a profile field and hard-code a per-product renderer switch in `forge.config.ts` | This is exactly the rejected "copy the fixture and edit Gosling" pattern; every new project would require a host source edit, which SHP-RSK-031 identifies as the blocking risk. |
| Dynamic renderer loading from an arbitrary URL or unpinned package name at build time | Violates PG-INV-003 (no arbitrary build hooks) and the threat model's traversal/injection controls already established for profile paths in ADR-0007; a manifest must resolve to a source-controlled, hashed, approved-root path. |
| Let the product profile (`ResolvedShellProductProfile`) carry the renderer entry directly | Conflates build/release identity (profile's existing role, ADR-0007) with consumer/composition identity; a profile has no schema slot for capability declarations, adapter references, or asset/test roots, and extending it would relitigate ADR-0007's "no domain semantics in the profile" boundary. |
| Let the manifest declare its own approved consumer root | Containment against a caller-controlled boundary is not a security boundary — a hostile manifest could simply declare a root wide enough to contain any path reachable by the build. The approved root must come from the trusted resolver's own registration data, the same way `APPROVED_PROFILE_ROOTS` already works for product profiles. |

## Consequences

Vite/Forge become manifest-driven for renderer selection while host main/preload entries stay fixed
and identical across every consumer (PG-INV-003). A project team can develop entirely outside this
repository once the shell-kit package and manifest resolver exist (R2), closing PSR-001 and PSR-010.
The in-tree neutral fixture must be migrated into a consumer-owned package (PG-23) rather than
special-cased, so the same containment/hash/compatibility checks apply to first-party and
third-party consumers alike. Publishing and versioning the shell-kit interface is new maintenance
surface not present today; SHP-REQ-042 (fail-closed compatibility) and the exact-pin default in
`SHP-ASM-012` bound that cost for the first release.

## Dependency record

No new runtime dependency is approved by this ADR. `@repo-makeover/gosling-sdk` is already a
dependency (`ui/desktop/src/shell/acpRuntime.ts:1-9`). Publishing it as a consumable shell-kit
package (npm registry, private registry, or pinned git dependency) is an R2 implementation decision
requiring its own dependency-rationale note if a new publishing mechanism is introduced.
