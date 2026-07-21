# Open Defects Repair Campaign Session Log - 2026-07-20

Status: `completed_with_partial_verification`

## Stage 1 - Intake and freeze

The latest audit/report/TODO inventory was normalized in Existing Findings Mode. Feature backlog and external tool failures were separated from repairable defects.

## Stage 2 - Source repair

- Added task-local ACP runtime paths for config, data, and state.
- Keyed global configuration and instance identity by active scoped paths.
- Scoped ACP construction and serving to the server instance.
- Removed Desktop browser-global and workspace-filter dependency debt.
- Added provider-inventory caching, in-flight coalescing, and mutation invalidation.
- Made the ACP schema recipe independent of the caller's working directory.
- Added focused runtime-path isolation coverage.

## Stage 3 - Record closure

Historical records were retained and superseding closure entries were added for AUD-GOS-011, GEM-005, ORCH-002, REC-001, REC-002, RES-002, and RES-003. `docs/TODO.md` and `plan.md` now distinguish closed defects from feature, release, maintenance, manual-validation, and external-tool work.

## Verification boundary

No build, test, lint, format, schema-generation, or Git command was run because the user requested repair and ledger reconciliation, not execution verification or repository publication.

Commands remaining for an authorized verification pass:

```bash
cargo fmt --all -- --check
cargo test -p gosling config::paths
cargo test -p gosling instance_id
cargo test -p gosling --lib
cargo test -p gosling-server
cargo clippy --workspace --all-targets -- -D warnings
cd ui/desktop && pnpm run typecheck
cd ui/desktop && pnpm test
cd ui/desktop && pnpm run lint:check
just check-acp-schema
```

## Residuals

- The 8,000,000-character ACP response ceiling remains intentional fail-closed behavior; streaming/pagination is future API work.
- Session Handoff, Tagteam expansion, CLI usage reporting, release execution, and broad modularization remain non-defect backlog.
- Giles's uniqueness-constraint crash is external tool debt.
- macOS Keychain authorization remains manual integration validation.


## Stage 4 - Provider-cache race closure

The final known source defect was repaired with mutation-depth and generation tracking. Cache invalidation now occurs at both mutation boundaries, an older in-flight read cannot repopulate the cache, and a read superseded by invalidation retries against the current generation.

Campaign source status: `complete`. Overall status remains `completed_with_partial_verification` until the recorded formatter, build, test, lint, and schema commands are authorized and executed.
