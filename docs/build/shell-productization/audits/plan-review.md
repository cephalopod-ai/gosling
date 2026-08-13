# Planning audit — Gosling shared shell productization

Date: 2026-08-12
Scope: pre-implementation review of `execution-plan.md`, traceability, risks, assumptions, build state, and known gaps
Mode: planning-only; no code or workflow conformance certification

## Evidence basis

- Maintainer-supplied intent: all previously identified shared non-domain shell work is accepted and should receive a detailed robust plan.
- Declared repository intent: `AGENTS.md`, `docs/INTENT.md`, `docs/architecture.md`, `docs/architecture/shell-foundation.md`, ADRs, release docs, and recent shell session record.
- Observed baseline: clean Gosling `main` at `8627dc31a`; current Rust shell contracts; `createMinimalShellHost`; `goslingServe`; Forge config; platform bundle/release workflows; Linux CI job; existing `with-rusty-v8-cache.sh`.
- Inferences remain labeled in `assumption-ledger.md` and are Gate 0/2 validation work, not accepted implementation facts.

## Review lenses

1. requirement completeness and non-goal discipline;
2. dependency and reversibility ordering;
3. Electron main/preload/renderer trust boundaries;
4. server/runtime authority and compatibility;
5. process cleanup, restart, and isolation;
6. profile/path/secret/config authority;
7. packaged and cross-platform test realism;
8. CI/V8 supply-chain integrity;
9. signing/release/updater permissions and cross-publication risk;
10. evidence, change control, and cold-start handoff.

## Findings and dispositions

| ID | Finding | Severity | Disposition |
| --- | --- | --- | --- |
| SHP-PLAN-001 | The first draft stated invariants in prose but did not give each a stable checkable ID and named evidence source | medium | Fixed: added SHP-INV-001–012 registry in Section 3.3; final Gate 8 architecture audit must evaluate it |
| SHP-PLAN-002 | The first draft was dependency ordered but did not state safe parallel execution boundaries or same-file conflict rules | medium | Fixed: added limited parallelism rules in Section 6.1; integration still waits on frozen interfaces and package smoke |
| SHP-PLAN-003 | Relative gate cost and highest implementation uncertainty were implicit | low | Fixed: added S/M/L/XL complexity table with cost drivers; estimates are explicitly non-schedule commitments |
| SHP-PLAN-004 | Exit criteria existed but gate state decisions were not normalized | medium | Fixed: added GO/PATCH/REPLAN/BLOCKED/STOP protocol and barred static-only GO where observed evidence is required |
| SHP-PLAN-005 | Risk of self-certifying the architecture plan needed an explicit boundary | medium | Fixed: this audit records plan quality only; execution plan requires an independent final architecture/security/release audit against landed code |
| SHP-PLAN-006 | V8 helper version selection could be mistaken for the `v8-goose` dependency version (`145.0.2`) instead of vendored wrapper version (`145.0.0`) | high | Already covered but clarified by observed `vendor/v8/Cargo.toml`; Gate 1 must verify upstream asset/checksum/build-script contract and never weaken verification |
| SHP-PLAN-007 | Existing updater code is full-Gosling-specific and broad; merely changing Forge feed metadata would not prove shell updater isolation | high | Covered by SHP-REQ-021 and Gate 7 readback; implementation must either add a narrow profile-aware updater adapter or keep shell updater fully disabled—no inheritance by accident |
| SHP-PLAN-008 | Real domain-shell requirements could pressure the neutral fixture into speculative shared abstractions | high | Covered by SHP-INV-008, SHP-RSK-026, P2 two-consumer rule, and explicit scope-change trigger |
| SHP-PLAN-009 | Full packaged E2E on every OS may be blocked by CI/GUI/signing environment | medium | Covered by SHP-ASM-009/010 and SHP-REQ-020: one full packaged primary path is mandatory; each release-ready target still needs target-specific structural/readback and explicit human installed/signing evidence |
| SHP-PLAN-010 | Production release destination remains undecided | medium | Correctly retained as unresolved SHP-ASM-017; fixture/runtime work may proceed, but production profile/release activation is blocked pending operator decision |

## Dependency-order verdict

The sequence is coherent:

- Gate 1 restores trustworthy Rust CI independently and before final acceptance.
- Gate 2 freezes profile, process, compatibility, and release authority before consumers.
- Gate 3 creates deterministic product/build identity before Electron runtime or workflows rely on it.
- Gates 4–5 implement process and interface layers before package claims.
- Gate 6 proves the actual package before Gate 7 release productization is accepted.
- Gate 8 re-runs final-revision evidence and performs reverse traceability.

No destructive migration or publication appears before reversible fixture proof. The plan explicitly retains human authority for production identifiers, signing, publication, updater promotion, and P0/P1 scope changes.

## Residual planning risks

- Current main may change before implementation; Gate 0 must reorient and record plan changes.
- External release topology is unresolved and blocks only production release activation.
- Signing/notarization/updater predecessor evidence remains human/tooling-dependent and cannot be pre-certified.
- The packaged automation platform is unconfirmed; Gate 0/6 may require harness adjustment without reducing acceptance.
- No independent audit tool was available through the active catalog for this exact plan surface; the review used the prototype skill's adversarial fallback plus the architectural-invariant planning checklist. Landed code still requires independent audits at each gate.

## Plan acceptance verdict

**GO for implementation planning handoff, not GO for implementation completion.** The package is sufficiently detailed to start Gate 0 without re-deriving intent. All product requirements remain `planned`; no runtime, package, release, or CI closure is claimed by this audit.

## Verification handoff to implementation/audit agents

At minimum, future auditors must evaluate:

- SHP-INV-001–012 against landed files and runtime/package/workflow evidence;
- every critical/high open risk and every baseline defect;
- profile/provisioning/renderer negative space;
- process cleanup and three-product coexistence;
- package manifest/readback and updater/release destination isolation;
- V8 archive trust/cold-cache behavior;
- final-revision evidence freshness and reverse requirement traceability.
