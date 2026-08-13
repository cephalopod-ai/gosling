# Gate 3 evidence — product profile and build resolution

Date: 2026-08-12
Decision: **GO to Gate 4 after local checkpoint commit**
Baseline revision: `e68c5791a`
Final Gate 3 revision: recorded by the local checkpoint commit after this evidence file

## Implemented

- dependency-free CommonJS authority in `ui/desktop/scripts/shell-profile.js`;
- strict duplicate-key JSON parsing, unknown-field/version rejection, identifier validation,
  secret/domain-content rejection, and field-safe errors;
- approved `shell-products/` and `fixtures/shell-products/` roots with repository-relative,
  traversal/backslash/NUL/type/containment/symlink controls;
- target inventory and format/dimension checks for PNG, ICO, ICNS, and SVG icons;
- exact profile/provisioning identity comparison;
- canonical recursively sorted JSON, sorted semantic sets, SHA-256 profile hash, exact Git revision,
  clean-checkout flag, and deterministic target manifest;
- same-field normalized collision checks for all ten identity-sensitive fields;
- atomic generated output under ignored `build/shell-profiles/**` only;
- check/check-all/resolve CLI and package scripts;
- neutral fixture A/B profiles, matching provisioning, and distinct test-only assets;
- thin Forge projection selected only by `GOSLING_SHELL_PROFILE`, with existing Gosling defaults
  when absent and rejection of the prior independent identity environment variables;
- fixture updater/publisher/macOS signing/notarization/Windows signing denial even when credential
  variables are present;
- CI profile tests and collision check in the Desktop job;
- consumer-only TypeScript types in `ui/desktop/src/shell/profile.ts` and developer documentation in
  `docs/SHELL_PRODUCTS.md`.

No production profile, release destination, signing action, publication, updater promotion, domain
shell, domain adapter, or domain workflow was added.

## Commands observed

Run through the Hermit environment:

```text
cd ui/desktop && pnpm run shell:test-profile
34 tests passed, 0 failed

cd ui/desktop && pnpm run shell:check-profiles
fixture A hash a9b0bfbc...; fixture B hash 6e3808cd...; no collisions

cd ui/desktop && pnpm run typecheck
tsc --noEmit passed

cd ui/desktop && pnpm run lint:check
passed; 21 i18n transaction tests passed and 15 locale catalogs validated

cd ui/desktop && pnpm run test:run
88 test files passed; 583 tests passed

cd ui/desktop && pnpm exec prettier --check forge.config.ts package.json src/shell/profile.ts
passed

Forge default/fixture load probe
passed: default resources/publisher preserved; fixture signing/updater/publisher disabled

Documentation fence and docs/INDEX.md relative-link check
passed

git diff --check
passed
```

The check output reports `sourceClean:false` because Gate 3 files are intentionally uncommitted while
this evidence is written. That is correct: non-publishable fixtures may resolve in a dirty checkout,
while publishable manifest creation fails when dirty. Production destinations are also rejected
because the approved destination set is intentionally empty.

## Acceptance mapping

| Requirement | Gate 3 evidence | Status after Gate 3 |
| --- | --- | --- |
| SHP-REQ-001 | strict resolver, canonical hash, hostile corpus | built; local targeted tests pass |
| SHP-REQ-002 | runtime/domain field rejection, provisioning identity comparison, retired identity env rejection | built for build boundary; runtime preflight remains Gate 4 |
| SHP-REQ-009 | fixture A/B differ across all ten collision keys | built identity inputs; installed coexistence remains Gate 6 |
| SHP-REQ-012 | one projection drives Forge names/IDs/assets/resources; default parity test | built; package readback remains Gate 6/7 |
| SHP-REQ-017 | fixture hard denial plus source negative-space audit | built; workflow denial remains Gate 7 |
| SHP-REQ-019 | target-specific asset existence/type/format/dimension tests | built; platform package readback remains Gate 7 |
| SHP-REQ-021 | fixture updater/publisher disabled and no update config bundled | built in Forge adapter; installed feed readback remains Gate 6/7 |
| SHP-REQ-022 | deterministic canonical profile/manifest/hash/revision/target output | built; embedded package readback/tamper remains Gate 6/7 |
| SHP-REQ-023 | documented commands and new-profile recipe without Forge edits | built for profile check/resolve; dedicated shell package path remains Gate 4/7 |

## Residual limits

- Gate 3 uses the existing full Desktop Vite entries. The dedicated shell main/preload/renderer
  entries are a Gate 4 requirement; fixture packaging now proves identity/resource projection only.
- Actual OS package metadata/resource readback and three-product coexistence are Gate 6/7 work.
- Full Desktop lint/test and Forge configuration-load checks passed. Actual package generation and
  OS metadata/resource readback remain Gate 6/7 because Gate 4 has not yet added dedicated shell
  Vite entries.
- The unrelated missing Anthropic weather replay remains outside this slice.
- Gate 1 still lacks remote Linux CI evidence because no push/PR is authorized.
