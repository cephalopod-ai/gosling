# 2026-08-27 Remove Tagteam support

## Task

Remove Tagteam runtime, provider, workflow, persistence, API, CI, test, and
current-documentation support from Gosling while preserving the database
migration ladder required to open older installations.

## Files changed

- Removed the built-in Tagteam provider and the feature-gated Tagteam workflow,
  MCP client, contracts, policy, reducer, store, and tests.
- Removed the `tagteam-workflow` Cargo feature and its CI-only build, test, and
  lint invocations.
- Removed `SessionWorkflow` and `workflow_kind` from the runtime session model,
  SQL queries, fresh database schema, and generated OpenAPI schema.
- Added sessions schema migration v30. It drops the four retired Tagteam tables
  and the `workflow_kind` column. Legacy sessions remain ordinary sessions;
  obsolete Tagteam run-control records are discarded.
- Removed Tagteam setup and backlog guidance from current README and TODO
  documentation. Historical release, audit, report, and session-log evidence
  remains unchanged.

## Validation

- `source bin/activate-hermit && cargo fmt --all -- --check` — passed.
- `source bin/activate-hermit && cargo test -p gosling test_removed_tagteam_schema_is_cleaned_up`
  — passed.
- `source bin/activate-hermit && cargo test -p gosling` — passed: 1,747 library
  tests plus integration and documentation tests; repository-designated ignored
  tests remained ignored.
- `source bin/activate-hermit && just generate-openapi` — passed and refreshed
  `ui/desktop/openapi.json`.
- `source bin/activate-hermit && cargo clippy --all-targets -- -D warnings` —
  passed.
- Residual source/current-doc scan — only migration v19/v20 compatibility,
  migration v30 cleanup, and its regression test retain Tagteam identifiers.

## Risks and follow-ups

- Upgrading an older database preserves the sessions but intentionally removes
  persisted Tagteam launch ownership, identities, counters, and snapshots.
- Migration v19/v20 must remain in the sequential upgrade ladder even though
  fresh databases no longer create Tagteam state.
