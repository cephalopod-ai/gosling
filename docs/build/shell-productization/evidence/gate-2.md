# Gate 2 evidence — architecture and contract freeze

Date: 2026-08-12
Decision: **GO** to Gate 3

## Source inspected

- existing ADR-0001–0006 conventions;
- `docs/architecture/shell-foundation.md`;
- Rust `ShellProvisioning`, policy, validation, handoff, and custom dispatch contracts;
- generated SDK shell types and ACP client initialization surface;
- `createMinimalShellHost`, `goslingServe`, startup diagnostics, process registry;
- full Desktop `main.ts`, broad preload IPC, channels, Forge/Vite entries;
- updater configuration and release workflow identity/destination surfaces.

## Frozen outputs

- ADR-0007: strict source-controlled profile authority, schema/canonical hash/path/publish rules.
- ADR-0008: separate shell entries, least-privilege preload, main-owned process lifecycle, exact bundled core, pre-session compatibility.
- ADR-0009: manifest/readback-driven package identity, fixture hard denial, human signing/release/updater gates.
- `shell-productization-contracts.md`: exact schema v1, fixtures A/B, module contracts, path matrix, eight-operation IPC allowlist, compatibility order/codes, lifecycle/error/diagnostic contracts, threat model, and test oracles.

## Live-state conclusions

- No additive Rust provisioning/handoff DTO is needed for product-profile work.
- Current ACP initialization does not enumerate custom methods; Rust already derives the canonical set through `GoslingAcpAgent::custom_method_schemas`, so Gate 4 must expose generated capability metadata.
- The full preload exposes broad filesystem/settings/updater operations and cannot be reused by a focused shell.
- Forge and updater identities currently derive independently and remain Gate 3/7 implementation work.

## Validation

After final edits:

- relative documentation links resolve;
- Markdown fences balance;
- all SHP-REQ IDs remain complete;
- `git diff --check` passes;
- ADR format matches existing repository convention;
- adversarial architecture/security review passed after two corrections recorded in `audits/gate-2-architecture-security.md`.

No runtime behavior is claimed by this design gate.
