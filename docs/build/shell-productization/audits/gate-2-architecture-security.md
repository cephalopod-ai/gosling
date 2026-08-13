# Gate 2 audit — shell productization architecture and security contracts

Date: 2026-08-12
Scope: ADR-0007–0009 and `docs/architecture/shell-productization-contracts.md`
Verdict: **GO after two design corrections**

## Reviewed boundaries

- product profile versus Rust provisioning/config/credential authority;
- Electron app identity ordering and main/preload/renderer ownership;
- lifecycle generation, retry, stop, crash, and process cleanup;
- ACP identity, method, provisioning, and handoff compatibility order;
- shared protected configuration versus isolated application/runtime state;
- diagnostic collection/export and renderer filesystem denial;
- package, fixture, updater, signing, release, and rollback authority;
- domain-neutral fixture and module negative space.

## Findings and dispositions

| ID | Severity | Finding | Disposition |
| --- | --- | --- | --- |
| SHP-G2-AUD-001 | high | A source profile requiring one exact Git SHA would become stale on every implementation commit and encourage bypass/update churn | Fixed: `goslingRevision: current` resolves to clean-checkout HEAD in the manifest; explicit SHA pins must match HEAD; dirty checkout cannot publish |
| SHP-G2-AUD-002 | high | Current ACP initialization does not expose the authoritative custom-method set needed for pre-session compatibility | Design fix: Gate 4 must add canonical capability metadata derived from Rust `GoslingAcpAgent::custom_method_schemas`; probing by session creation and TypeScript duplicate lists are forbidden |

## Checked invariants that held

- Profile contains build/distribution identity and a provisioning reference only; runtime selections and secrets are rejected.
- Renderer cannot provide identity, namespace, provisioning path, filesystem path, ACP URL/secret, updater feed, or release destination.
- Separate shell Vite main/preload/renderer entries are mandatory; full Desktop `main.ts` and preload are excluded.
- Main owns one child generation and cleanup; retry requires full cleanup and a fresh secret.
- Compatibility and provisioning validation precede session create/resume.
- Fixtures are permanently non-publishable through reviewed profile state, not workflow input.
- Production identity, signing, destination, publication, and updater promotion remain human gates.
- Diagnostics use allowlisted bounded fields and main-owned native save; no generic shell filesystem bridge is introduced.
- Every module contract names a must-not-own boundary and a failable test oracle.

## Residual decisions and limits

- Production release destination remains unresolved and blocks only production activation.
- Exact platform app-path syntax is deferred to tested Electron/OS adapters; sharing policy is frozen.
- The capability metadata wire shape remains a Gate 4 canonical Rust/generated-SDK design detail, not permission to hand-author a UI DTO.
- This audit evaluates design consistency, not implementation conformance.
