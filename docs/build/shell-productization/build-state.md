# Build state — Gosling shared shell productization

Updated: 2026-08-12 23:03 implementation session
Mode: patch-authorized; local commits only; no push, signing, publication, updater promotion, production identifiers, or domain-shell work authorized
Current gate: Gate 2 — freeze product, compatibility, security, and module contracts
Current step: Gate 1 built/locally validated and remote-Linux-blocked; inspect existing contracts and author ADRs before feature code

## Intent echo

Build reusable shared infrastructure that turns the merged shell foundation into a packaged, tested, diagnosable, isolated, and releasable Electron host. Do not implement any named/domain shell, domain adapter semantics, domain UI/workflow, final branding, real publication, or updater promotion.

## Baseline verified during planning

| Fact | Evidence source | Freshness |
| --- | --- | --- |
| Target repository `/Users/eric/Work/vscode/forked/gosling` was on clean `main` | `git status --short --branch` returned `## main...origin/main` | planning session |
| Current baseline was `8627dc31a`, merge of PR #46 | `git log -1 --oneline --decorate` | planning session |
| Shell foundation architecture and implementation files exist | static inspection of `docs/architecture/shell-foundation.md`, Rust shell files, Desktop `shellHost.ts` | baseline SHA |
| `createMinimalShellHost` has no production call site | repository search for symbol found definition only | baseline SHA |
| `ShellFrame`/`ShellStatus` are primitives, not a domain/runtime app | static inspection/search | baseline SHA |
| Forge currently parameterizes product name, protocol scheme, and package ID through environment values | static inspection of `ui/desktop/forge.config.ts` | baseline SHA |
| Actual shell icons/updater feeds/package artifacts/Electron packaged E2E were deferred by PR #46 | shell architecture/session record | baseline SHA |
| Historical Linux CI Rust jobs could fail on missing native `rusty_v8` | GitHub run logs show the native-link failure on runs `31642034573` and `31659795858` | current Gate 0 readback |
| Current merged-main Rust CI reached tests; its red cause was an unrelated missing Anthropic weather replay | GitHub run `31660173759` log | current Gate 0 readback |
| A verified V8 archive/cache helper exists and passed isolated cold/warm preparation on macOS | `scripts/with-rusty-v8-cache.sh`; `evidence/gate-0.md` | current Gate 0 readback |
| Repository validation must run through Hermit Rust 1.92, not the direct Homebrew Rust 1.97.1 found in the shell | `rust-toolchain.toml`; observed `rustc --version` before/after activation | current Gate 0 readback |
| Existing Desktop `main.ts` is 3,399 lines; plan must avoid adding shell rules there | `wc -l`; repo TODO modularization item | baseline SHA |
| No `docs/testing/test-ledger.yaml` exists in Gosling at planning baseline | filesystem inspection | planning session |

These are evidence of current structure, not proof that the proposed implementation works.

## Planning artifacts created

- `docs/build/shell-productization/execution-plan.md`
- `docs/build/shell-productization/traceability-matrix.md`
- `docs/build/shell-productization/risk-register.md`
- `docs/build/shell-productization/assumption-ledger.md`
- `docs/build/shell-productization/build-state.md`
- `docs/build/shell-productization/plan-changes.md`
- `docs/build/shell-productization/defects.md`
- `docs/build/shell-productization/audits/plan-review.md`
- `docs/build/shell-productization/evidence/planning.md`
- local ignored `docs/logs/session/2026-08-12-shell-productization-plan.md`
- links in `docs/INDEX.md` and `docs/TODO.md`

## Next actions in strict order

1. Commit the accepted Gate 1 helper/workflow/tests/evidence slice locally; do not push.
2. Read existing ADR convention/index, shell DTO/validation, Desktop shell host/preload/main/Forge/updater/deep-link/diagnostic contracts, and release workflow inputs.
3. Author ADRs for product-profile authority, Electron process/preload ownership, and package/updater/release isolation without implementing behavior.
4. Freeze the versioned profile schema, compatibility matrix, lifecycle/error taxonomy, IPC allowlist, path matrix, threat model, module contracts, and test fixture identities.
5. Run docs/diagram/link/contract checks and adversarial architecture/appsec review; patch findings before Gate 2 GO.
6. Seek remote Gate 1 CI evidence only after a future authorized push/PR; until then SHP-REQ-014 stays built, not verified.
7. Begin Gate 3 only after Gate 2 contracts are accepted; continue checkpointing every coherent slice.

## Open blockers / decisions

- Whether production shell releases remain in the Gosling GitHub release destination or use separate repositories/feeds. This does not block fixture implementation; it blocks production profile/release activation.
- Availability of signing/notarization credentials and a compatible predecessor artifact. These remain human Gate 7/8 checks.
- Which platform will run the first full installed packaged E2E. Default preference: local macOS host; CI structural/readback on all targets.
- Whether existing ACP initialization/provisioning metadata is sufficient for exact core/method compatibility. Gate 2 must inspect before adding canonical DTO fields.
- Fresh Linux cold-download/helper evidence requires CI or a disposable Linux environment; local macOS evidence cannot close that target.
- Current merged-main Rust CI has an unrelated missing Anthropic weather replay (`31660173759`); the V8 slice must not silently absorb that separate defect.

## Verify-don't-trust resume commands

Run from the target repository, not the current Muninn workspace:

```bash
cd /Users/eric/Work/vscode/forked/gosling
git status --short --branch
git rev-parse --abbrev-ref HEAD
git rev-parse HEAD origin/main
git log -3 --oneline --decorate
sed -n '1,220p' AGENTS.md
sed -n '1,220p' docs/build/shell-productization/execution-plan.md
cat docs/build/shell-productization/traceability-matrix.md
cat docs/build/shell-productization/risk-register.md
cat docs/build/shell-productization/assumption-ledger.md
cat docs/build/shell-productization/plan-changes.md
cat docs/build/shell-productization/defects.md
git diff --check
git diff -- docs/build/shell-productization docs/INDEX.md docs/TODO.md docs/logs/session
```

Then re-run symbol/path searches before modifying files. If current `main` differs from `8627dc31a`, record a plan change for any material impact.

## Validation status

- Gate 0: GO; orientation and live-state correction are recorded in `evidence/gate-0.md`.
- Gate 1: helper/workflow/tests are built and locally validated; remote Linux CI is blocked pending push/PR authority. Evidence/audit: `evidence/gate-1.md`, `audits/gate-1-supply-chain.md`.
- Focused Rust shell tests: 4 passed, 0 failed after final Gate 1 changes.
- SHP-REQ-014 is `built; remote verification blocked`; every other P0/P1 remains `planned`.
- Known CI baseline: current merged-main Rust CI has an unrelated replay-data failure; it is not waived and remains a Gate 8 acceptance blocker if unresolved.
