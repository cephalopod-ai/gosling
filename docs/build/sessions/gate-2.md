# Gate 2 session log

## Intent

Turn Workspaces requirements into explicit module, persistence, credential, session, ACP,
renderer, error, and test contracts before product code.

## Actions

- Inspected actual SDK generation, custom dispatch, provider field metadata/config save,
  AgentConfig/AgentManager restore, session list filters, and Desktop ACP adapters.
- Defined backend store, strict credential scope, schema v22 snapshot, canonical generated
  contract, path/output policy, and UI ownership decisions.
- Planned every new/modified file and mapped requirement groups to test levels/commands.
- Loaded and applied the internal API contract audit; repaired product-output cardinality.

## Decisions

- New DTOs live in an extracted SDK module, not the already-large custom request file.
- The workspace store remains separate from `sessions.db`; only snapshots enter v22.
- Strict profile scope intercepts provider-declared config keys and leaves unrelated
  Gosling configuration global.
- Full workspace replacement avoids PATCH null/absence ambiguity.
- Session archives and application updates are not routed into product output folders.

## Next action

Gate 3: add canonical DTOs, workspace module/service skeleton, handler registration and
real-contract test harnesses, then run the focused foundation audit.

