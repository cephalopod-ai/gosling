# Gosling shared shell productization planning package

This package is the durable, resumable plan for turning the merged Gosling shell foundation into reusable project-shell and packaged/release infrastructure. It deliberately excludes every actual domain shell. The 2026-08-13 reassessment found that the host substrate is real but the project-shell consumer, application-runtime, and domain-adapter seams are not yet ready.

## Start here

1. [`readiness-reassessment.md`](readiness-reassessment.md) — current source/CI assessment, retained strengths, and blocking drift findings.
2. [`project-shell-readiness-plan.md`](project-shell-readiness-plan.md) — superseding R0–R8 plan and project-shell consumer-ready milestones.
3. [`pre-gui-backend-implementation-plan.md`](pre-gui-backend-implementation-plan.md) — focused, dependency-aware R1–R4 work packages and the hard backend-complete gate before R5 shared GUI.
4. [`build-state.md`](build-state.md) — current truth, blockers, strict next actions, and verify-don't-trust resume commands.
5. [`execution-plan.md`](execution-plan.md) — historical original plan; Gates 0–4 evidence remains useful, but its forward Gates 5–8 are superseded.
6. [`traceability-matrix.md`](traceability-matrix.md) — all SHP-REQ IDs and planned design/test/evidence mapping.
7. [`risk-register.md`](risk-register.md) and [`assumption-ledger.md`](assumption-ledger.md) — open risks, triggers, mitigations, and inferred defaults.
8. [`plan-changes.md`](plan-changes.md) — append-only change control before deviations.
9. [`defects.md`](defects.md) — known baseline gaps and future audit/implementation findings.
10. [`evidence/planning.md`](evidence/planning.md) and [`audits/plan-review.md`](audits/plan-review.md) — what was observed and how the original plan was challenged.
11. [`evidence/r0.md`](evidence/r0.md) — merged Linux baseline repair, two clean Rust CI executions, and reopened Gate 4 acceptance.
12. [`evidence/gate-4.md`](evidence/gate-4.md) and [`audits/gate-4-host-package.md`](audits/gate-4-host-package.md) — historical Gate 4 process-boundary evidence, audit, and residual limits.
13. [`audits/pg-50-pre-gui-acceptance.md`](audits/pg-50-pre-gui-acceptance.md) — current local
    PG-50 evidence and the formal clean-revision NO-GO.

## Current status

Gates 0–4 remain historical foundation evidence. R0 is merged on `main` at `3feffca7c` and closed by two clean Linux Rust executions: PR run `31731952749` and merged-main run `31732990062`; later `main` CI run `31744291492` also passed at `e5af0f640`. The reassessment still reopens omitted Gate 4 failure-path acceptance and replaces forward Gates 5–8 with R0–R8. R1 is accepted and R2 is verified: strict neutral consumer composition and exact host-package readback now exist. R3 is locally exercised for bounded main-owned session/prompt/interaction/recovery behavior, including server-verified namespace compatibility. R4 is locally complete: the adapter has fail-closed operator registration, live negotiation, frame limits, request deadlines, server-owned confirmation, status supervision, and live neutral-process coverage for mismatch, crash, timeout, in-flight disconnect, cleanup, and restart. R6/R8 must reproduce that evidence in packaged and cross-platform workflows.

As of 2026-08-13, ADR-0010–0012 and a companion R1 contracts addendum
([`../../adr/0010-project-shell-consumer-composition.md`](../../adr/0010-project-shell-consumer-composition.md),
[`../../adr/0011-shell-application-runtime-boundary.md`](../../adr/0011-shell-application-runtime-boundary.md),
[`../../adr/0012-shell-domain-adapter-topology.md`](../../adr/0012-shell-domain-adapter-topology.md),
[`../../architecture/shell-productization-r1-contracts.md`](../../architecture/shell-productization-r1-contracts.md))
are accepted for implementation — see `build-state.md` for the current disposition. R2 and local R4
conformance are complete. R3's main-owned interaction/recovery contract is now locally exercised,
including a server-verified runtime namespace; the current PG-50 audit records strong local
evidence but remains a formal NO-GO until it is rerun on a clean exact revision with current CI.
Shared GUI, named project shells, and production release work remain blocked until that gate passes.

No production signing, notarization, publication, updater promotion, production identifier, release destination, or named project shell is authorized or implemented. Resume from `build-state.md` and verify the current repository instead of trusting an earlier checkpoint blindly.
