# Documentation Index

Index of repo-local documentation for the `gosling` fork. See `UPSTREAM.md`
for the relationship to cephalopod-ai/gosling.

## Standard sections

- [architecture.md](architecture.md) — system architecture
- [architecture/shell-foundation.md](architecture/shell-foundation.md) — focused shell identity, provisioning, runtime, adapter, handoff, and host foundation
- [architecture/shell-productization-contracts.md](architecture/shell-productization-contracts.md) — accepted product profile, process/preload, compatibility, lifecycle, diagnostics, release, and threat-model contracts
- [build/shell-productization/README.md](build/shell-productization/README.md) — index for the shell productization plan, traceability, risks, evidence, audits, and handoff state
- [build/shell-productization/readiness-reassessment.md](build/shell-productization/readiness-reassessment.md) — post-Gate-4 source/CI reassessment and project-shell readiness blockers
- [build/shell-productization/project-shell-readiness-plan.md](build/shell-productization/project-shell-readiness-plan.md) — superseding R0–R8 plan for consumer, application-runtime, domain-adapter, package, and onboarding readiness
- [build/shell-productization/execution-plan.md](build/shell-productization/execution-plan.md) — historical original plan; forward Gates 5–8 are superseded
- [build/shell-productization/build-state.md](build/shell-productization/build-state.md) — current resumable status and verify-before-execution handoff
- [build/shell-productization/evidence/r0.md](build/shell-productization/evidence/r0.md) — R0 Linux CI repair, two clean Rust executions, and Gate 4 acceptance reconciliation
- [SHELL_PRODUCTS.md](SHELL_PRODUCTS.md) — strict product-profile roots, local package/readback commands, fixtures, and extension recipe
- [INTENT.md](INTENT.md) — fork intent and scope
- [TODO.md](TODO.md) — outstanding work
- [adr/](adr/) — architecture decision records
- [adr/0013-session-artifact-inventory.md](adr/0013-session-artifact-inventory.md) — durable session-scoped Outputs inventory and preview authorization boundary
- [build/](build/) — build documentation
- [build/context-compaction-failsafe-plan.md](build/context-compaction-failsafe-plan.md) — recurring oversized-session compaction repair plan and acceptance criteria
- [cloud/](cloud/) — cloud deployment notes
- [test_scenarios/](test_scenarios/) — test scenario definitions

## Repo entry points

- [../README.md](../README.md) — project overview
- [../AGENTS.md](../AGENTS.md) — canonical agent operating contract
- [../BUILDING_LINUX.md](../BUILDING_LINUX.md) — Linux build instructions
- [../BUILDING_DOCKER.md](../BUILDING_DOCKER.md) — Docker build instructions
- [../CONTRIBUTING.md](../CONTRIBUTING.md) — contribution guide
- [../RELEASE.md](../RELEASE.md) — release process
