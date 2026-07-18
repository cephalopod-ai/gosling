# Gate 1 evidence — intent and requirements

Date: 2026-07-18

## Produced

- Authoritative intent charter: `docs/INTENT.md`
- Thirty stable requirements with priorities and observable acceptance criteria
- Ten invariants, explicit non-goals/out-of-scope, glossary, and scope-pressure cut list
- Seeded traceability matrix, defect ledger, and plan-change log
- Negative-space audit of hidden actors, alternate paths, timing, deletion, and recovery

## Exit checks

- Every primary-workflow step maps to at least one P0 requirement.
- No requirement permits raw secrets outside secure storage.
- Active workspace and session-pinned workspace are distinct terms and contracts.
- Legacy/null, deleted workspace, deleted profile, missing folder, and interrupted-write
  states each have observable behavior.
- All requirements remain `planned`; none are represented as implemented or verified.

