# Plan-change ledger — Gosling shared shell productization

Material changes to scope, requirements, architecture, sequencing, platform support, or acceptance must be recorded here **before** implementation departs from the accepted plan. Requirement IDs are never renumbered or reused.

## Change levels

- **L0 — clarification:** no requirement/architecture/acceptance impact; update plan text and record a row.
- **L1 — implementation adjustment:** file/module/test approach changes while acceptance remains intact.
- **L2 — material design or sequence change:** ADR/module boundary/critical path/platform evidence changes; requires explicit review.
- **L3 — scope/authority/acceptance change:** P0/P1 cut/defer, real product/release/updater activation, secret/config authority change, external core support, or security-boundary change; requires operator approval.

## History

| ID | Date | Level | Gate | Summary | Reason/evidence | Requirement/risk impact | Approval |
| --- | --- | --- | --- | --- | --- | --- | --- |
| SHP-PC-000 | 2026-08-12 | L0 | planning | Initial plan package established against `main` at `8627dc31a` | Operator agreed to all identified remaining shared shell work and requested a detailed plan using the prototype skill | Seeds SHP-REQ-001–032 and SHP-RSK-001–030 | operator request |
| SHP-PC-001 | 2026-08-12 | L1 | 0 | Correct current Linux CI diagnosis while retaining deterministic V8 provisioning work | Historical runs `31642034573`/`31659795858` show missing `rusty_v8`; current merged-main run `31660173759` reached tests and failed unrelated replay data | SHP-REQ-014 acceptance is explicit helper provisioning plus Linux evidence, not an inaccurate claim that every current run stops at V8; unrelated replay failure remains separate | autonomous live-state correction within acceptance |

## Record template

### SHP-PC-NNN — title

- Date / gate / level:
- Proposed before deviation:
- Change:
- Trigger and observed evidence:
- Alternatives considered:
- Requirements affected:
- Risks/assumptions affected:
- Test/evidence impact:
- Rollback/compatibility impact:
- Approval required/received:
- Files updated:
