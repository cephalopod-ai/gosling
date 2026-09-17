# Documentation Index

Index of repo-local documentation for the `gosling` fork. See `UPSTREAM.md`
for the relationship to cephalopod-ai/gosling.

## Standard sections

- [architecture.md](architecture.md) — system architecture, including plan lifecycle, Context History, and Recall Brief boundaries
- [architecture/shell-foundation.md](architecture/shell-foundation.md) — focused shell identity, provisioning, runtime, adapter, handoff, and host foundation
- [architecture/shell-productization-contracts.md](architecture/shell-productization-contracts.md) — accepted product profile, process/preload, compatibility, lifecycle, diagnostics, release, and threat-model contracts
- [architecture/shell-productization-r1-contracts.md](architecture/shell-productization-r1-contracts.md) — accepted R1 consumer manifest, application-runtime, and domain-adapter contracts (companion to ADR-0010–0012)
- [architecture/default-shell-template.md](architecture/default-shell-template.md) — active pre-GUI contract, ownership boundaries, work packages, and acceptance gate for the generic Default Shell MVP
- [build/shell-productization/README.md](build/shell-productization/README.md) — index for the shell productization plan, traceability, risks, evidence, audits, and handoff state
- [build/shell-productization/readiness-reassessment.md](build/shell-productization/readiness-reassessment.md) — post-Gate-4 source/CI reassessment and project-shell readiness blockers
- [build/shell-productization/project-shell-readiness-plan.md](build/shell-productization/project-shell-readiness-plan.md) — superseding R0–R8 plan for consumer, application-runtime, domain-adapter, package, and onboarding readiness
- [build/shell-productization/pre-gui-backend-implementation-plan.md](build/shell-productization/pre-gui-backend-implementation-plan.md) — dependency-aware R1–R4 execution plan and hard backend acceptance gate before shared or named shell GUI work
- [build/shell-productization/default-shell-ds3-ds7-implementation-plan.md](build/shell-productization/default-shell-ds3-ds7-implementation-plan.md) — implementation-ready work packages, security boundaries, validation matrix, and GO/NO-GO criteria for the remaining Default Shell pre-GUI foundation
- [build/shell-productization/audits/default-shell-pre-gui-corrective-audit.md](build/shell-productization/audits/default-shell-pre-gui-corrective-audit.md) — fresh workflow, data-flow, recovery, and security audit of the corrective pre-GUI patch
- [build/shell-productization/audits/ds-7-operator-acceptance.md](build/shell-productization/audits/ds-7-operator-acceptance.md) — operator acceptance of DS-7, its conditions, and the Gate 3 entry condition
- [build/shell-productization/gui/gate-1-product-workflow-design.md](build/shell-productization/gui/gate-1-product-workflow-design.md) — Default Shell GUI product and workflow design: screen/state inventory, workflow walkthroughs, failure and recovery copy, negative space
- [build/shell-productization/gui/gate-2-frontend-handoff.md](build/shell-productization/gui/gate-2-frontend-handoff.md) — Default Shell GUI front-end handoff: operation/event contract, state ownership, reducer rules, component inventory, bounds, accessibility criteria
- [build/shell-productization/gui/gate-3-build-record.md](build/shell-productization/gui/gate-3-build-record.md) — Default Shell GUI build record: what was implemented, the defects the build found, the verification performed, and the limits of that verification
- [build/shell-productization/execution-plan.md](build/shell-productization/execution-plan.md) — historical original plan; forward Gates 5–8 are superseded
- [build/shell-productization/build-state.md](build/shell-productization/build-state.md) — current resumable status and verify-before-execution handoff
- [build/shell-productization/evidence/r0.md](build/shell-productization/evidence/r0.md) — R0 Linux CI repair, two clean Rust executions, and Gate 4 acceptance reconciliation
- [SHELL_PRODUCTS.md](SHELL_PRODUCTS.md) — strict product-profile roots, local package/readback commands, fixtures, and extension recipe
- [INTENT.md](INTENT.md) — fork intent and scope
- [TODO.md](TODO.md) — outstanding work
- [RECALL_BRIEF.md](RECALL_BRIEF.md) — ACP user guide for opt-in, exact-revision Muninn Recall Brief reporting
- [polish/](polish/) — code-polish, documentation-stewardship, and release-readiness evidence
- [polish/documentation-inventory.md](polish/documentation-inventory.md) — canonical documentation surfaces, ownership, and retention
- [polish/structure-compliance.md](polish/structure-compliance.md) — documentation layout findings and repo-specific dispositions
- [polish/documentation-stewardship-report.md](polish/documentation-stewardship-report.md) — latest stewardship gate results and remaining risks
- [polish/test-ledger.md](polish/test-ledger.md) — current validation commands, results, and evidence limits
- [logs/](logs/) — retained session evidence and logging conventions
- [logs/session/2026-09-16-evidence-instruction-authority.md](logs/session/2026-09-16-evidence-instruction-authority.md) — ADR-0023 implementation, baseline, validation, and the operator-config side effect of `tests/agent.rs`
- [logs/session/2026-09-16-branch-input-inheritance.md](logs/session/2026-09-16-branch-input-inheritance.md) — branch input pointers, independent library entries, branch title labels, and focused validation
- [logs/session/2026-09-13-consolidated-audit-merge-install.md](logs/session/2026-09-13-consolidated-audit-merge-install.md) — local fast-forward merge, documentation refresh, release rebuild, CLI/Desktop reinstall, and installed Keychain-startup limit
- [logs/session/2026-09-13-host-enforced-planning.md](logs/session/2026-09-13-host-enforced-planning.md) — ADR-0020 staged implementation, decision register, and hermetic validation runs
- [logs/session/2026-09-14-context-compaction-history.md](logs/session/2026-09-14-context-compaction-history.md) — ADR-0021 ledger, CLI and Desktop surfaces, changes-only diffs, and validation evidence
- [logs/session/2026-09-14-recall-brief-implementation.md](logs/session/2026-09-14-recall-brief-implementation.md) — source implementation, plan revisions, and focused validation evidence
- [logs/session/2026-09-15-audit-findings-repair.md](logs/session/2026-09-15-audit-findings-repair.md) — disposition and validation of the three findings in the 2026-09-15 private-skills audit
- [logs/session/2026-09-09-v1.2.5-release-documentation.md](logs/session/2026-09-09-v1.2.5-release-documentation.md) — v1.2.5 local version and release-documentation preparation, validation, and open publication gates
- [adr/](adr/) — architecture decision records
- [adr/0010-project-shell-consumer-composition.md](adr/0010-project-shell-consumer-composition.md) — accepted project-shell consumer/composition topology
- [adr/0011-shell-application-runtime-boundary.md](adr/0011-shell-application-runtime-boundary.md) — accepted main-owned application runtime and renderer capability boundary
- [adr/0012-shell-domain-adapter-topology.md](adr/0012-shell-domain-adapter-topology.md) — accepted domain adapter lifecycle, transport, and authority
- [adr/0013-session-artifact-inventory.md](adr/0013-session-artifact-inventory.md) — durable session-scoped Outputs inventory and preview authorization boundary
- [adr/0014-default-shell-template-boundary.md](adr/0014-default-shell-template-boundary.md) — generic Default Shell ownership, instruction, credential, settings, launcher, and module boundary
- [adr/0015-shell-project-session-library.md](adr/0015-shell-project-session-library.md) — least-privilege project/session input library for linked files, pasted text, images, and prompt attachment
- [adr/0016-deep-research-library.md](adr/0016-deep-research-library.md) — durable, user-configurable Deep Research deliverable library and bounded prior-context browser
- [adr/0017-session-private-directory-grants.md](adr/0017-session-private-directory-grants.md) — additive, session-only directory grants for active workspace chats
- [adr/0018-output-contribution-history.md](adr/0018-output-contribution-history.md) — agent/model attribution, saved output revisions, comparison, export, and guarded restore
- [adr/0019-full-session-handoff-continuity.md](adr/0019-full-session-handoff-continuity.md) — bounded, redacted checkpoints and atomic provider/model continuity
- [adr/0020-host-enforced-plan-lifecycle.md](adr/0020-host-enforced-plan-lifecycle.md) — persisted, revision-bound planning with a non-widenable host capability boundary
- [adr/0021-context-compaction-history.md](adr/0021-context-compaction-history.md) — append-only local compaction provenance, retention, and atomic capture contract
- [adr/0022-provenance-safe-recall-brief.md](adr/0022-provenance-safe-recall-brief.md) — explicit ACP report gate, exact Muninn revision citations, and epistemic limits
- [adr/0023-evidence-instruction-authority-boundary.md](adr/0023-evidence-instruction-authority-boundary.md) — host-owned, revision-bound, turn-scoped skill admission; content never changes authority
- [build/](build/) — build documentation
- [build/context-compaction-failsafe-plan.md](build/context-compaction-failsafe-plan.md) — recurring oversized-session compaction repair plan and acceptance criteria
- [build/host-enforced-planning/README.md](build/host-enforced-planning/README.md) — ADR-0020 implementation sequence, release gates, evidence, and resumable status
- [build/evidence-authority-boundary/README.md](build/evidence-authority-boundary/README.md) — ADR-0023 integration handoff: types, coverage matrix, compatibility, shared `EIA-*` fixtures, and residual risk
- [cloud/](cloud/) — audit and playtest reports (not cloud-hosting runbooks)
- [cloud/2026-09-13-consolidated-audit-repair.md](cloud/2026-09-13-consolidated-audit-repair.md) — canonical reconciliation, disposition, repair commits, validation, local install addendum, and remaining decisions for both 2026-09-13 audits
- [cloud/2026-09-13-live-all-scenarios-playtest.md](cloud/2026-09-13-live-all-scenarios-playtest.md) — historical 2026-09-13 live playtest report, with a disposition addendum linking to the repair campaign
- [cloud/20260913_Gemini_Audit_Data_gosling.md](cloud/20260913_Gemini_Audit_Data_gosling.md) — historical Gemini architecture/dataflow audit, with corrected severity and disposition addendum
- [cloud/2026-09-08-recent-work-independent-audit.md](cloud/2026-09-08-recent-work-independent-audit.md) — 2026-09-08 independent multi-lens audit of the last ~20 commits (revisions, compaction, artifacts, permission chrome); no security scan
- [cloud/2026-09-08-system-surface-audit.md](cloud/2026-09-08-system-surface-audit.md) — 2026-09-08 independent multi-lens system, reliability, and dataflow surface audit
- [cloud/2026-09-08-evening-audit-repair.md](cloud/2026-09-08-evening-audit-repair.md) — independent audits and repairs of today's changes, including revision custody, compaction outcomes, workflow state, and contract drift; no security scan
- [cloud/2026-09-08-cross-cutting-lens-audit.md](cloud/2026-09-08-cross-cutting-lens-audit.md) — negative-space, invariant-sync, resource-lifecycle, and recovery-idempotency lenses on the same day's changes; no security scan
- [cloud/2026-08-26-clean-independent-audit.md](cloud/2026-08-26-clean-independent-audit.md) — 2026-08-26 clean independent multi-lens system audit, with a later targeted High-severity repair closure
- [cloud/2026-08-15-master-report.md](cloud/2026-08-15-master-report.md) — 2026-08-15 exhaustive multi-lens audit + 110-card playtest merge
- [cloud/2026-08-15-live-all-scenarios-playtest.md](cloud/2026-08-15-live-all-scenarios-playtest.md) — live playtest ledger for that pass
- [test_scenarios/](test_scenarios/) — test scenario definitions

## Repo entry points

- [../README.md](../README.md) — project overview
- [../AGENTS.md](../AGENTS.md) — canonical agent operating contract
- [../BUILDING_LINUX.md](../BUILDING_LINUX.md) — Linux build instructions
- [../BUILDING_DOCKER.md](../BUILDING_DOCKER.md) — Docker build instructions
- [../CONTRIBUTING.md](../CONTRIBUTING.md) — contribution guide
- [../RELEASE.md](../RELEASE.md) — release process
