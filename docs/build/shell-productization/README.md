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

## Current status

Planning-only, pre-Gate 0. The package is structurally validated and reviewed for implementation handoff. No runtime, package, CI repair, signing, publication, or updater work is claimed complete.

When implementation is authorized, begin with `build-state.md`; verify the current repository instead of trusting the 2026-08-12 baseline blindly.
