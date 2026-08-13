# Gosling shared shell productization planning package

This package is the durable, resumable plan for turning the merged Gosling shell foundation into reusable packaged/release infrastructure. It deliberately excludes every actual domain shell.

## Start here

1. [`execution-plan.md`](execution-plan.md) — mission, scope, architecture, requirements, nine gates, tests, release/security/diagnostic contracts, and definition of complete.
2. [`build-state.md`](build-state.md) — current truth, baseline evidence, blockers, strict next actions, and verify-don't-trust resume commands.
3. [`traceability-matrix.md`](traceability-matrix.md) — all SHP-REQ IDs and planned design/test/evidence mapping.
4. [`risk-register.md`](risk-register.md) and [`assumption-ledger.md`](assumption-ledger.md) — open risks, triggers, mitigations, and inferred defaults.
5. [`plan-changes.md`](plan-changes.md) — append-only change control before deviations.
6. [`defects.md`](defects.md) — known baseline gaps and future audit/implementation findings.
7. [`evidence/planning.md`](evidence/planning.md) and [`audits/plan-review.md`](audits/plan-review.md) — what was observed and how the plan was challenged.
8. [`evidence/gate-4.md`](evidence/gate-4.md) and [`audits/gate-4-host-package.md`](audits/gate-4-host-package.md) — latest dedicated host/package-integrity checkpoint and residual Gate 4 work.

## Current status

Implementation is in Gate 4. Gates 0, 2, and 3 have local GO checkpoints; Gate 1 is built locally but remote Linux evidence is blocked because push/PR is unauthorized. The dedicated shell host and host-target package-integrity path are built and locally verified, while session create/resume and full-Gosling handoff receiving still block Gate 4 exit. No production signing, notarization, publication, updater promotion, production identifier, release destination, or domain shell is authorized or implemented.

Resume from `build-state.md` and verify the current repository instead of trusting an earlier checkpoint blindly.
