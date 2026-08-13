# Build state — Gosling shared shell productization

Updated: 2026-08-13 Gate 4 host/package-integrity checkpoint
Mode: patch-authorized; local commits only; no push, signing identity, notarization, publication,
updater promotion, production identifiers, release destination, or domain-shell work authorized
Current gate: Gate 4 — shared Electron bootstrap, preload, ACP, compatibility, and diagnostics
Current step: checkpoint the dedicated host and package-integrity slice, then add session
create/resume and focused full-Gosling handoff receiving

## Intent echo

Build reusable shared infrastructure that turns the merged shell foundation into a packaged, tested,
diagnosable, isolated, and releasable Electron host. Do not implement any named/domain shell, domain
adapter semantics, domain UI/workflow, final branding, real publication, or updater promotion.

## Verified baseline and checkpoints

| Gate | Local checkpoint         | State                                                       | Evidence                                                       |
| ---- | ------------------------ | ----------------------------------------------------------- | -------------------------------------------------------------- |
| 0    | `ee0d79ee0`              | GO                                                          | `evidence/gate-0.md`                                           |
| 1    | `72c22f4cc`              | built locally; remote Linux verification blocked            | `evidence/gate-1.md`, `audits/gate-1-supply-chain.md`          |
| 2    | `e68c5791a`              | GO                                                          | `evidence/gate-2.md`, `audits/gate-2-architecture-security.md` |
| 3    | `269f04b94`              | GO                                                          | `evidence/gate-3.md`, `audits/gate-3-profile-release.md`       |
| 4a   | `ce7586aa8`              | canonical ACP method capabilities committed                 | focused Rust/runtime evidence                                  |
| 4b   | `842356a53`              | app identity/lifecycle/compatibility committed              | source tests                                                   |
| 4c   | `ff4567d74`              | constrained IPC/preload committed                           | source tests                                                   |
| 4d   | `d7e4178a5`              | main-owned ACP preflight committed                          | source tests                                                   |
| 4e   | `48b4d5c6c`              | backend generation/cleanup owner committed                  | source tests                                                   |
| 4f   | pending local checkpoint | dedicated host/package integrity built and locally verified | `evidence/gate-4.md`, `audits/gate-4-host-package.md`          |

Live Gate 0 corrections remain binding: authoritative commands use `source bin/activate-hermit`;
historical Linux V8 failures and any current unrelated baseline failures are distinct. No remote CI
evidence is claimed because no push/PR is authorized.

## Gate 4 delivered surfaces

- isolated pre-ready app identity, paths, persistent partition, protocol, and single-instance lock;
- deterministic lifecycle state machine and one fenced backend generation owner;
- exact shell IPC allowlist and dedicated frozen preload with no broad Desktop authority;
- main-owned authenticated ACP initialization and profile/core/schema/method/provisioning preflight;
- dedicated shell main/preload/renderer Vite and Forge entries;
- secure bootstrap navigation/window restrictions, second-instance focus, lifecycle forwarding, and
  cleanup-before-quit;
- compile-time packaged resources plus runtime identity/profile-hash validation;
- bounded redacted diagnostics and one-time explicit handoff preparation/confirmation;
- tracked host-only non-publishable package wrapper and deterministic package verifier;
- exact macOS arm64 readback of profile, manifest, provisioning, binary hash, dedicated entries,
  updater absence, protocol, executable, and stable bundle identifier.

The package wrapper builds `gosling-cli` through `scripts/with-rusty-v8-cache.sh`, stages that exact
artifact, rebuilds the SDK, invokes Forge package with release credentials cleared, and rejects
publishable/update-enabled/signing-required profiles. Electron Forge's fuses plugin applies only the
mandatory local ad-hoc signature after changing the arm64 Electron fuse wire; no team identity,
notarization, release signature, upload, or publication occurs.

## Latest observed validation

```text
source bin/activate-hermit
cd ui/desktop
pnpm run shell:test-profile     # 41 passed, 0 failed
pnpm run typecheck              # passed
pnpm run lint:check             # passed; i18n checks included
pnpm run test:run               # 100 files, 655 tests passed
focused shell Vitest            # 10 files, 53 tests passed
focused shell/package ESLint    # zero warnings
focused touched-file Prettier   # passed
shell package build/readback    # passed on macOS arm64 fixture A
git diff --check                # passed
```

Repository-wide `pnpm run format:check` still reports 62 pre-existing unrelated source files. The
only touched file it initially reported was formatted, and the focused touched-file check is green.
No unrelated formatting was changed.

Package evidence:

```text
profileHash  bbdc328863718e3a94c2a379140bc16568bab9474be72066fb87bf0a7a9bbe75
binaryHash   16edd16fe9995bc44c28f131fc64dd7789f8d345639884f9ec6d5708bee96cec
bundle ID    io.github.repo-makeover.gosling.fixture.a
team ID      absent
```

## Next actions in strict order

1. Create the local Gate 4 host/package checkpoint commit; do not push.
2. Add a main-owned session create/resume seam only after compatibility succeeds.
3. Add deterministic neutral fixture probes proving success and no durable session on failed
   preflight; no domain semantics or live provider credential.
4. Add full Desktop `gosling://handoff` receiving through a focused parser/router module, keeping
   lifecycle/domain rules out of oversized `main.ts`.
5. Re-run focused/full Desktop validation and process cleanup tests, then decide Gate 4 exit.
6. Proceed to Gate 5 recovery/diagnostic/handoff UI only after the Gate 4 primary workflow is real.

## Open blockers / decisions

- Gate 4 is not complete: session create/resume and full-Gosling handoff receiving remain absent.
- Production release destination and identifiers remain unselected and block production profile
  activation only.
- Signing/notarization credentials and compatible predecessor artifact remain human Gate 7/8 checks.
- Gate 1 remote Linux cold-download/helper evidence remains blocked pending push/PR authority.
- macOS x64, Linux, and Windows package readback require matching runners and remain Gate 7 work.
- Installed restart/coexistence and full packaged primary workflow remain Gate 6.
- A deterministic no-live-credential session probe still needs design/validation.

## Verify-don't-trust resume commands

```bash
cd /Users/eric/Work/vscode/forked/gosling
git status --short --branch
git log -8 --oneline --decorate
sed -n '1,220p' AGENTS.md
cat docs/build/shell-productization/build-state.md
cat docs/build/shell-productization/traceability-matrix.md
cat docs/build/shell-productization/risk-register.md
cat docs/build/shell-productization/assumption-ledger.md
cat docs/build/shell-productization/defects.md
source bin/activate-hermit
cd ui/desktop && pnpm run shell:test-profile && pnpm run shell:check-profiles
pnpm run typecheck && pnpm run lint:check && pnpm run test:run
pnpm run shell:verify-package -- ../../fixtures/shell-products/fixture-a/product-profile.json --platform darwin --arch arm64 --package 'out/Gosling Shell Fixture A-darwin-arm64' --binary ../../target/aarch64-apple-darwin/release/gosling
cd ../.. && git diff --check
```

Inspect live files before every Gate 4 edit. If source differs materially from the checkpoint,
record a plan change before widening the implementation.
