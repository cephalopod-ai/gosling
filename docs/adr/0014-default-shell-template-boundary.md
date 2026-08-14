# ADR-0014: Default Shell template boundary

Date: 2026-08-14
Status: accepted by operator direction; implementation and revision-bound acceptance remain open
Requirements affected: SHP-REQ-044–053

## Context

The next product milestone is not DAWES, math, Physics/CST, or another named shell. It is a generic
Default Shell template: smaller than the full Gosling desktop application, larger than a workspace
chat window, and reusable by future independently owned shell products. Starting with a named
renderer would freeze domain assumptions before the common ownership and security boundaries have
been exercised.

The existing shell foundation already separates product identity, consumer composition, the
main-owned application runtime, credential-profile references, and supervised domain adapters. It
did not yet guarantee that a shell has its own instructions, that an empty shell tool policy stays
empty, or that reduced settings are persisted only inside that product's data root.

## Decision

The Default Shell is a composition of four independently versioned inputs:

1. A product profile owns the launcher identity, display name, executable/bundle/protocol IDs, and
   replaceable target icons.
2. A consumer manifest owns renderer composition and its declared capabilities. It does not own
   backend authority.
3. Shell provisioning owns the shell instruction profile, credential-profile reference,
   provider/model selection, extension/tool allowlist, skill references, protocol restrictions, and
   optional domain-adapter descriptor.
4. A product-scoped local settings document owns only the small allowlisted presentation and
   selection preferences defined by the Default Shell contract.

The shell runtime may resolve and use a credential through Gosling's protected credential service,
but neither the shell renderer nor the local settings document receives or stores secret values.
The renderer may receive bounded safe credential-profile metadata and relink status only through a
reviewed main-owned operation.

When provisioning supplies `instructions.systemPrompt`, the agent uses that prompt instead of the
generic Gosling prompt. Workspace instruction files may still be loaded from the selected project,
but Gosling's global hints are excluded. A shell process with no requested builtins starts with no
builtin extensions; `developer` remains the default only for non-shell `gosling serve`.

The local settings schema is fixed rather than an arbitrary key-value store. Version 1 contains
theme, text scale, the last absolute working directory, and an opaque preferred credential-profile
ID. It is stored under the product-specific Electron `userData` root with private permissions and
does not mutate Gosling global settings.

The first UI may be implemented only after the nonvisual gates in
`docs/architecture/default-shell-template.md` are satisfied. Named shell profiles, branding,
instructions, renderers, adapters, and domain behavior remain deferred until the generic template
passes its acceptance gate.

## Consequences

- Future shells can replace identity, icon, instructions, capabilities, and optional adapter without
  editing common host orchestration.
- A shell-local preference can remember a credential-profile ID, but deleting the profile in
  Gosling invalidates that reference and requires relinking; it never leaves a copied secret behind.
- Working-directory selection and settings editing require new narrow main/preload operations before
  a GUI can expose them. A generic filesystem or settings bridge remains forbidden.
- Prompt and settings schema changes require migration and compatibility tests.
- Optional backends remain supervised adapters or declared Gosling modules; the renderer does not
  become a generic RPC router.

## Rejected alternatives

| Alternative                                                  | Reason rejected                                                                                   |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------- |
| Begin with a DAWES, math, or physics shell                   | Couples common contracts to one domain before the reusable template is proven.                    |
| Reuse Gosling's generic prompt and global hints              | Makes shell behavior dependent on mutable full-Gosling instructions and violates shell ownership. |
| Copy credentials into shell storage                          | Creates a second secret owner and complicates revocation, migration, and incident response.       |
| Expose arbitrary settings or filesystem APIs to the renderer | Expands renderer authority beyond the reviewed least-privilege boundary.                          |
| Enable the developer builtin by default                      | Makes the reduced shell silently inherit developer tooling outside its declared capabilities.     |
| Let a renderer call arbitrary backends directly              | Bypasses lifecycle supervision, compatibility checks, action confirmation, and diagnostics.       |

## Dependency record

No new dependency is approved. This decision extends the existing provisioning DTO, prompt manager,
product-scoped path derivation, and strict local file-writing patterns.
