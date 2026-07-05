# Rebrand Audit Report

## Executive Verdict
This audit found one high-severity regression and one medium-severity contract break introduced by the rebrand cleanup, plus two low-signal stale artifacts that should still be removed because the repo now treats harmless shims as debt. The highest-risk issue is the desktop settings rename from `externalGoosed` to `externalGoslingd` without a migration path: upgraded users can silently stop using their configured external backend and fall back to a spawned local backend instead. The Nostr share-link compatibility contract is also internally inconsistent: the README and the low-level parser still claim `goose://sessions/nostr` interop, but the CLI, ACP server, and desktop UI all reject those links because the old scheme was replaced with a duplicated `gosling://` check.

No evidence in the sampled paths suggests that the `goose_mode` -> `gosling_mode` database migration or `.goslinghints` rename were left half-finished; those surfaces look intentionally handled. I did not manually read all 1,445 changed files, so this is a focused rebrand-contract audit rather than a literal full-file review of the entire diff.

## Scope
- Repository / branch / commit: `gosling` / `main` / `694af0335`
- Change set reviewed: `46f4ef174..HEAD`
- Prompt reviewed: comprehensive rebrand audit using `~/Work/vscode/agent-skills/10_audit/`, with stale shims treated as reportable debt
- Skills (lenses) invoked: `audit-workflow-gui`, `audit-contract-internalapi`, `audit-architecture-nodejs`, `audit-negative-space`, `audit-dataflow-integrity` as the primary execution lenses; the broader `10_audit` pack was loaded as a category framework
- Files/directories inspected:
  - `ui/desktop/src/main.ts`
  - `ui/desktop/src/App.tsx`
  - `ui/desktop/src/utils/settings.ts`
  - `ui/desktop/scripts/prepare-platform-binaries.js`
  - `crates/gosling-cli/src/commands/session.rs`
  - `crates/gosling/src/acp/server/manage_sessions.rs`
  - `crates/gosling/src/session/nostr_share.rs`
  - `crates/gosling/src/session/session_manager.rs`
  - `crates/gosling/src/hints/load_hints.rs`
  - `crates/goose/tests/*`
  - `crates/gosling/tests/*`
  - `README.md`
  - selected workflow and packaging files under `.github/workflows/`
- Commands run:
  - `git diff --stat 46f4ef174..HEAD`
  - `git log --oneline -n 10`
  - `rg` scans for `goose`, `goosed`, `goose://`, `externalGoosed`, `.goosehints`
  - targeted `sed` / `nl` reads on the files above
- Effort budget and what it bought:
  - Workflow / contract / stale-artifact rebrand paths only
  - Deep-read of persisted desktop settings, deep links, import paths, session migration, hint loading, and packaging cleanup
  - Broad grep across the repo for remaining old-brand executable/config/deep-link surfaces
- Constraints:
  - Audit-only, no code changes
  - No build/test execution because the task was an audit, not an implementation request
  - Large diff sampled by risk rather than linearly read end-to-end

## Draft Prompt Assessment
The supplied prompt was treated as a draft. I preserved the intended mission, but narrowed execution toward rename-risk boundaries: persisted settings, deep-link protocols, package/binary surfaces, and orphaned pre-rebrand artifacts. I treated "even harmless shims should be removed or renamed" as an explicit severity-floor change for stale compatibility paths.

## Surface Inventory
| Surface | Actor | Input/Trigger | State/Output | Boundary | Reviewed |
|---|---|---|---|---|---|
| Desktop settings load | Upgraded desktop user | existing `settings.json` | backend selection, CSP, external backend secrets | persisted settings schema migration | yes |
| Session-share import | CLI user / desktop user / ACP client | `gosling://` or `goose://` Nostr deep link | imported shared session JSON | deep-link compatibility contract | yes |
| Desktop protocol dispatch | OS deep-link launch | protocol URL on startup / second-instance | new window, session import, extension install | scheme routing | yes |
| Session DB migration | existing local DB | schema upgrade | `gosling_mode` session column | DB migration compatibility | yes |
| Hint-file loading | CLI / desktop session | nested context files | loaded instructions | context-file rename contract | yes |
| Packaging cleanup | desktop build pipeline | pre-bundle bin cleanup | copied desktop binaries | stale binary shim cleanup | yes |
| Orphan baseline harness | contributors / grep / editors | repo browsing, docs, ad hoc test runs | dead source tree | stale artifact cleanup | yes |

## Boundary Map
| Surface | Intended Boundary | Enforced At | Status |
|---|---|---|---|
| Desktop persisted settings | renamed settings keys must migrate or fail visibly | `ui/desktop/src/main.ts` settings loader | broken |
| Nostr session-share interop | all entrypoints should either accept or reject `goose://sessions/nostr` consistently | parser, CLI import, ACP import, desktop UI routing | broken |
| Session DB rename | old session schema should upgrade in place | `crates/gosling/src/session/session_manager.rs` | holds |
| Hint-file rename | only new `.goslinghints` contract should load unless compatibility is intentionally retained | `crates/gosling/src/hints/load_hints.rs` | holds |
| Packaging cleanup | no stale brand-specific executable shims unless intentionally required | `ui/desktop/scripts/prepare-platform-binaries.js` | stale shim remains |

## Skill Escalation
| Trigger | Lens |
|---|---|
| renamed persisted desktop key controlling active backend selection | Workflow-GUI, Data-Integrity |
| shared parser accepts one contract while callers reject it | Contract / Workflow-GUI |
| leftover source tree and packaging special-cases with old brand names | Architecture, Negative-Space |

## Findings Table
| ID | Severity | Confidence | Evidence Basis | Domain | Title | Patch Priority | Blast Radius | Complexity | Cost | Nominal Agent |
|---|---|---|---|---|---|---|---|---|---|---|
| `WFG-GOS-001` | High | Confirmed | source-evidenced | Workflow-GUI | Desktop settings rename drops persisted external backend configuration | P1 | Workflow | local_guardrail | S | codex |
| `WFG-GOS-002` | Medium | Confirmed | source-evidenced | Workflow-GUI | `goose://sessions/nostr` interop is advertised and parsed, but rejected by every top-level entrypoint | P1 | Workflow | workflow_protocol | S | codex |
| `ARC-GOS-003` | Low | Confirmed | source-evidenced | Architecture | Orphan `crates/goose/tests` tree still ships dead pre-rebrand harnesses and commands | P3 | Repo | local_guardrail | XS | codex |
| `ARC-GOS-004` | Info | Confirmed | source-evidenced | Architecture | Desktop packaging cleanup still special-cases the legacy `goosed` binary | P4 | Repo | local_guardrail | XS | codex |

## Detailed Findings

### WFG-GOS-001: Desktop settings rename drops persisted external backend configuration

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/utils/settings.ts:49-60`
- `ui/desktop/src/utils/settings.ts:89-104`
- `ui/desktop/src/main.ts:199-218`
- `ui/desktop/src/components/settings/app/ExternalBackendSection.tsx:86-93`
- prior rebrand baseline: the pre-rebrand desktop settings key was `externalGoosed` (`git show 46f4ef174:ui/desktop/src/utils/settings.ts`)

Observed behavior:
- The current desktop settings schema only defines and hydrates `externalGoslingd`.
- `getSettings()` spreads the raw stored JSON into memory, but only reads `stored.externalGoslingd` into the active config object.

Expected boundary:
- A renamed persisted key that controls backend target selection must either migrate from the old key or fail visibly before the app changes which backend it talks to.

Failure mechanism:
- Users upgrading from a build that persisted `externalGoosed` will keep that old field in `settings.json`, but the active code path ignores it and falls back to the default `externalGoslingd` object (`enabled: false`, empty URL/secret).
- Because `...stored` is spread into the cached settings object before the typed override, the obsolete field can linger as inert residue while the live backend selection silently switches to defaults.

Break-it angle:
- Start with an existing desktop profile configured against an external backend under the old key, upgrade to the rebrand build, and launch the app without re-saving settings. The UI/backend path now behaves as if external backend mode is disabled.

Impact:
- Upgraded users can silently stop using their configured external backend and fall back to a local backend instead.
- This changes target selection, secret usage, TLS fingerprint expectations, and operator understanding of which backend is active.

Operational impact:
- Blast radius: Workflow
- Side-effect class: process
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- stale `externalGoosed` residue persists in `settings.json`
- CSP and external backend trust settings no longer match operator intent
- future settings migrations can repeat the same pattern if raw spreads hide unknown keys

Recommended mitigation:
- Remediation patterns: persisted-key migration, startup guardrail
- Minimal repair: detect `stored.externalGoosed` during settings load and map it into `externalGoslingd` before the settings object is cached
- Local guardrail: if the legacy key is present and the new key is absent, log a one-time migration notice and persist the migrated shape
- Behavior test: load a legacy `settings.json` fixture containing only `externalGoosed`; assert that `getActiveExternalBackend()` still uses the migrated URL/secret/fingerprint

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: the failure is localized to desktop settings hydration and can be fixed with a narrow migration plus regression tests

Validation:
- Desktop unit test with legacy settings fixture
- End-to-end desktop settings load test asserting external backend remains enabled after upgrade

Non-goals:
- Do not redesign external backend settings UX
- Do not add broad settings-versioning unless separately requested

### WFG-GOS-002: `goose://sessions/nostr` interop is advertised and parsed, but rejected by every top-level entrypoint

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Workflow-GUI

Evidence:
- `README.md:51-52`
- `README.md:76-77`
- `crates/gosling/src/session/nostr_share.rs:247-254`
- `ui/desktop/src/main.ts:471-505`
- `ui/desktop/src/main.ts:519-525`
- `ui/desktop/src/App.tsx:358-363`
- `crates/gosling-cli/src/commands/session.rs:297-304`
- `crates/gosling/src/acp/server/manage_sessions.rs:281-284`

Observed behavior:
- The README says gosling still accepts `goose://` session-share links for interop.
- The low-level Nostr share parser still accepts both `gosling` and `goose` schemes.
- The desktop main process, desktop React client, CLI import path, and ACP server helper all check `gosling://sessions/nostr` twice instead of checking `goose://sessions/nostr` for the compatibility branch.

Expected boundary:
- Deep-link compatibility must be consistent across parser, desktop UI, CLI, and ACP import surfaces.

Failure mechanism:
- The compatibility shim survived in the shared parser and in the README, but every top-level caller rewrote the old scheme check into a duplicated `gosling://...` comparison.
- The result is a split contract: the parser claims interop, but the surrounding product surfaces reject the link before it reaches that parser.

Break-it angle:
- Open an old upstream share link such as `goose://sessions/nostr?...` via the desktop, `gosling session import`, or ACP session import. The top-level gate rejects the link as unsupported even though the parser would accept it.

Impact:
- Existing upstream share links stop importing through the product’s documented entrypoints.
- Operators get inconsistent behavior depending on which internal layer they read: docs and parser imply support, the actual UX rejects it.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: compensatable
- Operator visibility: UI-visible
- Rerun safety: safe

Adjacent failure modes:
- future deep-link changes can drift again because the scheme check is duplicated across multiple entrypoints
- tests can miss contract breaks when the shared parser and entrypoints are not exercised together

Recommended mitigation:
- Remediation patterns: single-source deep-link predicate, compatibility-contract test
- Minimal repair: either restore `goose://sessions/nostr` handling consistently in every entrypoint or remove the compatibility claim from `README.md` and `nostr_share.rs`
- Local guardrail: centralize the session-share scheme predicate so desktop, CLI, and ACP all call the same helper
- Behavior test: add one regression test per entrypoint asserting the chosen compatibility policy for both `gosling://` and `goose://`

Implementation assessment:
- Complexity: workflow_protocol
- Cost: S
- Cost drivers: modules, tests, docs
- Nominal implementation agent: codex
- Rationale: the fix is mechanically small but spans several entrypoints and needs docs/tests to stay aligned

Validation:
- CLI import test for `goose://sessions/nostr`
- ACP/server helper test for old and new schemes
- desktop deep-link handler test covering startup and already-running flows

Non-goals:
- Do not redesign the Nostr share format itself
- Do not change unrelated extension deep-link behavior

### ARC-GOS-003: Orphan `crates/goose/tests` tree still ships dead pre-rebrand harnesses and commands

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture

Evidence:
- `Cargo.toml:1-4`
- `crates/goose/tests/MEMORY_MANAGER_EVAL.md:1-20`
- `crates/goose/tests/memory_manager_eval.rs:35-44`
- `crates/gosling/tests/MEMORY_MANAGER_EVAL.md:10-16`

Observed behavior:
- The repo still contains `crates/goose/tests/*` even though the active workspace crates are `gosling*`.
- The orphaned files still instruct contributors to run `cargo test -p goose --test memory_manager_eval` and still import `use goose::...`.

Expected boundary:
- After a repo-wide rename, dead source trees that no longer correspond to workspace crates should be removed or renamed so grep, editors, and contributors only see live surfaces.

Failure mechanism:
- The old test tree survived the rename as a detached artifact rather than being deleted or clearly archived.
- A contributor following those instructions lands on a non-buildable path that no longer matches the workspace contract.

Break-it angle:
- Search the repo for memory-manager evaluation instructions or old crate names; the stale tree appears alongside the live `crates/gosling/tests/*` harness and can be mistaken for the current baseline path.

Impact:
- Contributor time is wasted on dead paths.
- Grep-based audits and editor diagnostics get noisier because stale old-brand code is still present under `crates/`.

Operational impact:
- Blast radius: Repo
- Side-effect class: none
- Reversibility: reversible
- Operator visibility: log-only
- Rerun safety: safe

Adjacent failure modes:
- more pre-rebrand analysis harnesses may still exist outside the live workspace
- dead artifacts can cause future automated rename sweeps to over-report false positives

Recommended mitigation:
- Remediation patterns: stale-tree deletion, archive-or-remove cleanup
- Minimal repair: delete `crates/goose/tests/*` or move it to a clearly non-source archival location outside `crates/`
- Local guardrail: add a repo check preventing old-brand source trees under active code roots
- Behavior test: a repository hygiene check that fails if `crates/goose/` exists without a manifest

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: modules
- Nominal implementation agent: codex
- Rationale: this is a narrow cleanup with little validation surface

Validation:
- repo hygiene script or CI grep check for `^crates/goose/`
- confirm live memory-eval docs only point at `crates/gosling/tests/*`

Non-goals:
- Do not remove historical references in docs that intentionally describe upstream goose

### ARC-GOS-004: Desktop packaging cleanup still special-cases the legacy `goosed` binary

Severity: Info  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture

Evidence:
- `ui/desktop/scripts/prepare-platform-binaries.js:164-168`
- `README.md:76-77`

Observed behavior:
- The desktop bin cleanup script still treats `goosed` as a named "legacy backend binary" and deletes it specially during non-Windows packaging.

Expected boundary:
- If harmless legacy shims are now reportable debt, packaging scripts should not carry old-brand executable special-cases unless they are still required by an active compatibility promise.

Failure mechanism:
- A stale cleanup branch preserves knowledge of the old executable name even though the repo’s current compatibility notes only call out the narrow DB/share-link cases that intentionally keep reading old names.

Break-it angle:
- Future packaging audits still have to reason about whether `goosed` is a supported artifact because the build pipeline carries a bespoke branch for it.

Impact:
- This does not break shipping builds, but it keeps stale brand-specific behavior alive in packaging code and makes later audits noisier.

Operational impact:
- Blast radius: Repo
- Side-effect class: file
- Reversibility: reversible
- Operator visibility: log-only
- Rerun safety: safe

Adjacent failure modes:
- other one-off cleanup exceptions may still exist in build scripts without an active contract

Recommended mitigation:
- Remediation patterns: stale-shim removal
- Minimal repair: remove the `goosed` special-case if no supported build path can still place that binary in `src/bin`
- Local guardrail: add a packaging grep check for retired binary names
- Behavior test: packaging script fixture that only expects current binary names

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: modules
- Nominal implementation agent: codex
- Rationale: trivial cleanup with contained impact

Validation:
- packaging script unit or smoke test for current binary set

Non-goals:
- Do not broaden this into a general packaging refactor

## Non-Findings / Checked But Not Confirmed
- `crates/gosling/src/session/session_manager.rs:1314-1321` still carries an intentional `goose_mode` -> `gosling_mode` DB migration. This is a bounded compatibility path with a concrete migration boundary, not a stray stale string.
- `crates/gosling/src/hints/load_hints.rs:10-20` only loads `.goslinghints` and `AGENTS.md` by default. That matches the repo’s published rename policy; I found no half-implemented `.goosehints` fallback in the current loader.
- `ui/desktop/src/goslingServe.ts:81` and `crates/gosling-cli/src/commands/update.rs:19-55` consistently use `gosling` binary names in the desktop launcher and updater paths.

## Break-It Review
- Persisted-settings angle: I checked whether the renamed desktop settings key had a migration path. It does not; the active loader only hydrates the new key and silently falls back to defaults.
- Cross-entrypoint contract angle: I traced the old Nostr share-link promise through README, parser, desktop UI, CLI import, and ACP import. The contract is split: shared parser says yes, every entrypoint says no.
- Stale-artifact angle: I searched for old brand paths and binaries under active code roots. The dead `crates/goose/tests` tree and `goosed` packaging special-case remained as low-severity residue.
- Adjacent compatibility angle: I also checked the session DB column rename and hint-file rename. Those surfaces were internally consistent in the sampled code.

## Evidence
- Files inspected: see Scope
- Commands run:
  - `git diff --stat 46f4ef174..HEAD`
  - `rg -n --hidden '(?i)\bgoose\b|goose-|goose_|Goose|GOOSE' ...`
  - `rg -n 'goose://sessions/nostr|goose://|externalGoosed|goosed\b|crates/goose/tests|use goose::' ...`
  - targeted `sed -n` / `nl -ba` reads on `main.ts`, `App.tsx`, `settings.ts`, `session.rs`, `manage_sessions.rs`, `nostr_share.rs`, `session_manager.rs`, `load_hints.rs`, and the orphan `crates/goose/tests/*` tree

## Cross-Lens Escalations
- `WFG-GOS-001` also touches data integrity because the persisted settings contract is part of desktop state migration.
- `WFG-GOS-002` also touches internal API contract integrity because the parser and entrypoints diverge on the same scheme contract.
- `ARC-GOS-003` and `ARC-GOS-004` are stale-artifact cleanup items, not end-user runtime failures.

## Residual Risk
- I did not manually review every renamed doc, image, or generated file touched by the 1,445-file rebrand range.
- I did not execute desktop or CLI tests, so runtime-only regressions outside the inspected rename-risk surfaces may remain.
- The repo likely contains more low-severity branding residue in documentation or archived material; this report focuses on live code and build surfaces.

## Recommended Patch Order
1. Fix `WFG-GOS-001` by migrating `externalGoosed` -> `externalGoslingd` during desktop settings load and add an upgrade regression test.
2. Decide the policy for old `goose://sessions/nostr` links, then make parser, CLI, ACP, desktop UI, and README all agree; add one shared test matrix for old/new schemes.
3. Delete or archive `crates/goose/tests/*` outside active source roots.
4. Remove the `goosed` packaging special-case if no supported artifact path still needs it.

## Next Action
Patch `WFG-GOS-001` first. It is the only confirmed finding here that silently changes a user’s active backend target after upgrade.
