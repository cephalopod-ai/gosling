# ADR-0009: Shell package, updater, and release isolation

Date: 2026-08-12
Status: accepted for implementation
Requirements affected: SHP-REQ-009, SHP-REQ-011–SHP-REQ-013, SHP-REQ-017, SHP-REQ-019–SHP-REQ-023, SHP-REQ-026, SHP-REQ-028–SHP-REQ-029, SHP-REQ-032

## Context

Current Forge and release workflows are Gosling-specific: resources include Gosling images and
`app-update.yml`, publisher defaults target the Gosling repository, updater code reads Gosling
owner/repository/bundle values, and release jobs upload broad artifact globs. Independent shell
environment variables are insufficient to prevent one product from using another product's
artifact name, protocol, state root, updater feed, or release destination.

## Decision

The canonical resolved profile/hash from ADR-0007 controls every package and artifact identity.
Post-package verification reads the built artifact and rejects any mismatch in product/executable
name, protocol, package/bundle/Flatpak ID, embedded profile/hash/core revision, resources, updater
metadata, target/arch, or artifact prefix. Artifact names are generated, not supplied by workflow
text.

Two neutral fixture profiles prove isolation. They use version `0.0.0-test`, fixture-only names,
protocols, namespaces, bundle/package IDs, artifact prefixes, user-data roots, browser partitions,
and disabled update channels. Both are permanently `publishable: false`. Workflow inputs cannot
enable publication, updater, notarization, signing, or promotion for a non-publishable profile.

Unsigned fixture packaging is allowed for tests. Signing, notarization, publication, production
identifiers, release destination, and updater promotion remain explicit human gates. Production
release jobs use least privilege and a reviewed source-controlled publishable profile, re-resolve
it in the privileged job, compare the canonical hash/artifact manifest from the unprivileged build,
and read back destination before credentials or upload. No real shell destination is chosen by
this ADR; that unresolved operator decision blocks production activation, not fixture work.

Updater policy is profile-specific and disabled by default. A shell may enable it only after a
signed compatible predecessor exists and installed update behavior is observed. Full Gosling's
current updater remains unchanged. Rollback selects the prior signed artifact/profile; schema v1
profiles have no destructive runtime migration, and failed startup cannot mutate another product's
state.

## Alternatives considered

| Alternative | Why rejected |
| --- | --- |
| Reuse full Gosling artifact names/feed | It creates cross-product update and release substitution risk. |
| Let workflow inputs choose names/destinations | Dispatch/PR input is not reviewed product authority and can inject or cross-publish. |
| Clone four workflows per shell | Duplicated policy would drift across products and platforms. |
| Treat fixture release as a harmless dry run | A fixture upload/sign/update path is exactly the failure the fixtures are meant to prevent. |
| Enable updater on the first artifact | There is no compatible predecessor or observed migration/rollback evidence. |

## Consequences

Build and release become manifest-driven and auditable, while production operations remain human
controlled. Platform adapters may transform packaging syntax but cannot reinterpret identity.
Structural/readback proof can run without credentials; release-ready and updater-ready claims still
require target-specific installed/signing evidence.

## Dependency record

No new dependency is approved. Existing Forge makers, GitHub Actions, checksum/attestation tools,
and platform package inspection utilities are reused.
