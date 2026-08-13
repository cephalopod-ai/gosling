# ADR-0007: Source-controlled shell product profile

Date: 2026-08-12
Status: accepted for implementation
Requirements affected: SHP-REQ-001–SHP-REQ-002, SHP-REQ-009–SHP-REQ-012, SHP-REQ-017, SHP-REQ-019–SHP-REQ-023, SHP-REQ-028

## Context

The merged shell foundation accepts independent build-time environment values for product name,
protocol scheme, and Linux package ID. Forge, Vite, updater code, package scripts, and platform
workflows can therefore disagree. Runtime provisioning is already a versioned Rust-owned document
and must not become packaging configuration or a secret carrier.

## Decision

Every focused shell is built from one source-controlled JSON product profile with
`schemaVersion: 1`. The exact contract is frozen in
[`shell-productization-contracts.md`](../architecture/shell-productization-contracts.md).
The build resolver rejects unknown fields/versions, secret-shaped fields, unsafe paths,
platform-invalid identifiers, identity collisions, and incomplete publishable profiles. It
canonicalizes the accepted object, emits deterministic JSON plus SHA-256, and is the only adapter
that may translate profile fields into Forge/workflow environment values.

The profile owns product/build/distribution identity only. It references one repository-relative
provisioning document but cannot contain workspace, credential, provider, model, extension, skill,
policy, domain payload, or secret values. The referenced provisioning identity must exactly match
the product profile identity before Electron starts.

Profile paths resolve relative to the profile file, must remain under approved repository roots,
and may not traverse or resolve through a symlink outside those roots. Unknown fields are rejected
rather than silently ignored because a build profile is reviewed input, not forward-compatible
user data. `publishable: false` cannot be changed by environment/workflow input. A publishable
profile requires complete target assets, non-fixture identifiers, signing/update policy, and an
operator-approved release destination.

Full Gosling packaging remains backward compatible when no shell profile is selected. Shell
profile selection is explicit through one resolver input; independent `GOSLING_SHELL_*` identity
overrides are retired from shell mode after the resolver adapter lands.

## Alternatives considered

| Alternative | Why rejected |
| --- | --- |
| Independent environment variables | They drift across Forge, Vite, updater, scripts, and workflows and are mutable at invocation time. |
| Put packaging data in Rust provisioning | It would mix build identity with server runtime authority and invite secret/policy leakage. |
| Runtime-editable product settings | Product identity must be established before Electron app paths, protocol, and locking; runtime editing requires a different migration/security product. |
| YAML plus a new parser | JSON works in the existing Node/Forge stack and needs no dependency. |
| Permit unknown fields | Build/release inputs must fail closed until a new schema version explicitly defines semantics. |

## Consequences

A new shell can be checked and packaged without editing Forge source, while package metadata is
traceable to a canonical profile hash. Profile schema evolution requires a new version and
migration/readback tests. Private external branding pipelines can generate a profile and assets,
but release workflows still validate the resulting source-controlled input and cannot promote a
fixture.

## Dependency record

No new dependency. Node standard-library JSON, path, filesystem, and crypto APIs are sufficient.
