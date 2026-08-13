# Build state — Gosling shared shell productization

Updated: 2026-08-12 22:58 implementation session
Mode: patch-authorized; local commits only; no push, signing, publication, updater promotion, production identifiers, or domain-shell work authorized
Current gate: Gate 1 — repair and deterministically provision the Linux V8 prerequisite
Current step: Gate 0 GO recorded; implement helper regression tests and CI wiring from current evidence

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

1. Add deterministic, network-independent helper regression tests for target parsing, seed/cold/warm/corrupt cache, checksum sidecar, invalid seed, unsupported target, and cache-under-target rejection.
2. Wire the Linux Rust CI job through the tested helper with a cache location outside Cargo `target/`.
3. Run shell tests plus static workflow/YAML checks and a focused Rust test command under Hermit using the prepared archive.
4. Audit the Gate 1 diff for supply-chain bypass, cache poisoning, unsafe shell interpolation, and unrelated CI changes; patch and revalidate.
5. Commit Gate 0/planning checkpoint, then commit Gate 1 independently when its local checks pass. Do not push.
6. Seek remote CI evidence only after a future authorized push/PR; until then SHP-REQ-014 cannot be marked verified.
7. Freeze Gate 2 ADR/profile/preload/compatibility/release contracts before implementing renderer or workflows.
8. Continue gate by gate, updating state after every coherent slice and before risky operations.

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
- Documentation structural/diff validation: passed; exact commands and limitations are recorded in `evidence/planning.md`.
- Runtime/code changes: none yet; Gate 1 is the first code/workflow slice.
- Plan requirements: all P0/P1 remain `planned`; none are claimed built or verified.
- Known CI baseline: V8 native-link failures are historical and nondeterministic; current merged-main Rust CI instead fails on unrelated replay data. Neither red state is waived.
