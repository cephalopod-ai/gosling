# Gosling shared shell productization planning package

This package is the durable, resumable plan for turning the merged Gosling shell foundation into reusable project-shell and packaged/release infrastructure. It deliberately excludes every actual domain shell. The 2026-08-13 reassessment found that the host substrate is real but the project-shell consumer, application-runtime, and domain-adapter seams are not yet ready.

## Start here

1. [`readiness-reassessment.md`](readiness-reassessment.md) — current source/CI assessment, retained strengths, and blocking drift findings.
2. [`project-shell-readiness-plan.md`](project-shell-readiness-plan.md) — superseding R0–R8 plan and project-shell consumer-ready milestones.
3. [`build-state.md`](build-state.md) — current truth, blockers, strict next actions, and verify-don't-trust resume commands.
4. [`execution-plan.md`](execution-plan.md) — historical original plan; Gates 0–4 evidence remains useful, but its forward Gates 5–8 are superseded.
5. [`traceability-matrix.md`](traceability-matrix.md) — all SHP-REQ IDs and planned design/test/evidence mapping.
6. [`risk-register.md`](risk-register.md) and [`assumption-ledger.md`](assumption-ledger.md) — open risks, triggers, mitigations, and inferred defaults.
7. [`plan-changes.md`](plan-changes.md) — append-only change control before deviations.
8. [`defects.md`](defects.md) — known baseline gaps and future audit/implementation findings.
9. [`evidence/planning.md`](evidence/planning.md) and [`audits/plan-review.md`](audits/plan-review.md) — what was observed and how the original plan was challenged.
10. [`evidence/gate-4.md`](evidence/gate-4.md) and [`audits/gate-4-host-package.md`](audits/gate-4-host-package.md) — historical Gate 4 process-boundary evidence, audit, and residual limits.

## Current status

Gates 0–4 are retained as historical foundation evidence, and their implementation is merged on `main` at `6fe6a3bfc`. The reassessment reopens omitted failure-path acceptance and replaces forward Gates 5–8 with R0–R8. Current Linux CI is red in the V8 helper self-test; the renderer is lifecycle-only; main-owned ACP has no usable renderer session/prompt/update API; the Rust domain adapter has no production registration path; and no external project renderer composition seam or reusable shell workflow exists. The next action is R0 CI repair, followed by R1 architecture—not shared UI implementation.

No production signing, notarization, publication, updater promotion, production identifier, release destination, or named project shell is authorized or implemented. Resume from `build-state.md` and verify the current repository instead of trusting an earlier checkpoint blindly.
