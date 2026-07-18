# Gate 2 evidence — architecture and contracts

Date: 2026-07-18

## Produced

- One-way module/dependency map and primary-workflow sequence in `docs/architecture.md`
- Module contract and seam catalog covering domain, store, credentials, session, ACP,
  generated SDK, React context/components, and Electron adapters
- ADR-0001–0005 for persistence, credentials, session snapshot, canonical contracts/UI
  state, and path/output policy
- Exact I/O, persistence, v22 session columns, path, malformed-input, export/import, and
  error behavior contracts
- Complete planned file/gate/test/audit map with source-size targets
- P0/P1 traceability rows mapped to concrete modules, ADRs, and test classes
- Internal-contract audit with all IAPI-001–016 items dispositioned

## Design self-review

- Every REQ has a module and a planned validation level.
- Workspace persistence can be replaced without changing UI/session contracts.
- Provider field evolution is absorbed by registry metadata and logical-field mapping.
- UI framework changes do not touch the store or credential boundary.
- The largest design bend is a provider constructor that reads credentials in a separately
  spawned task; ADR-0002 records the limitation and Gate 4 tests must expose it.
- No new third-party dependency is required.

## Gate exit

Gate 3 can begin with canonical types and compiling boundary scaffolds. No product behavior
is represented as built or verified.

