# Build state — Gosling shared shell productization

Updated: 2026-08-12 Gate 3 implementation session
Mode: patch-authorized; local commits only; no push, signing, publication, updater promotion, production identifiers, release destination, or domain-shell work authorized
Current gate: Gate 3 — implementation and validation complete; checkpoint commit in progress
Current step: create the local Gate 3 commit, verify clean checkpoint, then orient Gate 4

## Intent echo

Build reusable shared infrastructure that turns the merged shell foundation into a packaged, tested,
diagnosable, isolated, and releasable Electron host. Do not implement any named/domain shell, domain
adapter semantics, domain UI/workflow, final branding, real publication, or updater promotion.

## Verified baseline and checkpoints

| Gate | Local checkpoint | State | Evidence |
| --- | --- | --- | --- |
| 0 | `ee0d79ee0` | GO | `evidence/gate-0.md` |
| 1 | `72c22f4cc` | built locally; remote Linux verification blocked | `evidence/gate-1.md`, `audits/gate-1-supply-chain.md` |
| 2 | `e68c5791a` | GO | `evidence/gate-2.md`, `audits/gate-2-architecture-security.md` |
| 3 | pending local checkpoint | implementation/targeted acceptance green | `evidence/gate-3.md`, `audits/gate-3-profile-release.md` |

Live Gate 0 corrections remain binding: authoritative commands use `source bin/activate-hermit`;
historical Linux V8 failures and current unrelated Anthropic weather replay failure are distinct.
No remote CI evidence is claimed because no push/PR is authorized.

## Gate 3 delivered surfaces

- `ui/desktop/scripts/shell-profile.js`: strict parser, validators, approved roots, assets,
  canonical hash, collision checks, exact revision, target manifest, ignored atomic output.
- `ui/desktop/scripts/resolve-shell-profile.js`: check/check-all/resolve CLI.
- `ui/desktop/scripts/shell-forge-profile.js`: thin Forge projection and retired identity override
  denial.
- Node test corpus: resolver/hostile paths/assets/collisions, CLI, Forge parity/isolation, and fixture
  negative space.
- `ui/desktop/src/shell/profile.ts`: consumer-only resolved profile/manifest types.
- `fixtures/shell-products/fixture-{a,b}/`: frozen neutral identities, matching provisioning, and
  distinct structural test assets.
- `ui/desktop/forge.config.ts`: default Gosling parity with selected profile identity/resources and
  fixture signing/updater/publisher denial.
- package scripts and Desktop CI profile checks.
- `docs/SHELL_PRODUCTS.md`, architecture correction, traceability/evidence/audit/ledger updates.

No production destination is approved (`APPROVED_RELEASE_DESTINATIONS` is empty), so production
profile activation fails closed. Fixture profile resolution is allowed in a dirty checkout;
publishable manifest generation is not.

## Latest observed validation

```text
source bin/activate-hermit
cd ui/desktop
pnpm run shell:test-profile     # 34 passed, 0 failed
pnpm run shell:check-profiles   # fixture A/B valid, deterministic hashes, no collision
pnpm run typecheck              # passed
```

Additional Gate 3 exit evidence: Desktop `lint:check` passed (including 21 i18n transaction tests
and 15 locale catalogs); Desktop Vitest passed 88 files/583 tests; Prettier, Forge default/fixture
load probe, documentation fences/index links, and `git diff --check` passed. Actual package readback
is deliberately deferred until dedicated shell Vite entries exist in Gate 4.

## Next actions in strict order

1. Run broad Gate 3 validation and repair only Gate 3 regressions.
2. Update evidence with exact results and create one local checkpoint commit; do not push.
3. Start Gate 4 with fresh source inspection: existing host/lifecycle/process-registry, Vite entries,
   ACP initialization/custom-method schema generation, startup diagnostics, deep links, and preload
   channel conventions.
4. Add canonical Rust custom-method capability metadata before any session compatibility use; do
   not duplicate method authority in TypeScript.
5. Implement dedicated shell main/preload/renderer entries, app identity ordering, lifecycle,
   compatibility, narrow IPC, ACP preflight, diagnostics, and handoff in focused modules.

## Open blockers / decisions

- Production release destination and identifiers remain unselected and block production profile
  activation only.
- Signing/notarization credentials and compatible predecessor artifact remain human Gate 7/8 checks.
- Gate 1 remote Linux cold-download/helper evidence remains blocked pending push/PR authority.
- Existing merged-main Anthropic weather replay failure remains unrelated and may block final Gate 8
  acceptance if unresolved.
- Current ACP initialization lacks authoritative custom-method capabilities; Gate 4 must derive the
  additive wire metadata from `GoslingAcpAgent::custom_method_schemas`.
- Actual package metadata readback, tamper detection, installed coexistence, and packaged primary
  workflow remain Gates 6/7.

## Verify-don't-trust resume commands

```bash
cd /Users/eric/Work/vscode/forked/gosling
git status --short --branch
git log -5 --oneline --decorate
sed -n '1,220p' AGENTS.md
cat docs/build/shell-productization/build-state.md
cat docs/build/shell-productization/traceability-matrix.md
cat docs/build/shell-productization/risk-register.md
cat docs/build/shell-productization/assumption-ledger.md
cat docs/build/shell-productization/defects.md
source bin/activate-hermit
cd ui/desktop && pnpm run shell:test-profile && pnpm run shell:check-profiles && pnpm run typecheck
cd ../.. && git diff --check
```

Inspect live files before every Gate 4 edit. If source differs materially from the checkpoint,
record a plan change before widening the implementation.
