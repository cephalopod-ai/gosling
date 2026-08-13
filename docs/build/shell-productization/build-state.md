# Build state — Gosling shared shell productization

Updated: 2026-08-12 Gate 4 compatibility-metadata slice
Mode: patch-authorized; local commits only; no push, signing, publication, updater promotion, production identifiers, release destination, or domain-shell work authorized
Current gate: Gate 4 — shared Electron bootstrap, preload, ACP, compatibility, and diagnostics
Current step: checkpoint canonical ACP custom-method capability metadata, then implement pure app-identity/lifecycle/compatibility modules before the shell entrypoint

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
| 3 | `269f04b94` | GO | `evidence/gate-3.md`, `audits/gate-3-profile-release.md` |
| 4a | pending local checkpoint | canonical custom-method metadata unit/runtime proof green | source tests; Gate 4 evidence pending |

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

## Gate 4 orientation and current proof

Fresh inspection covered `shellHost.ts`, `goslingServe.ts` and its spawned-process tests,
`backendProcessRegistry.ts`, `startupDiagnostics.ts`, Desktop ACP connection/transport, full preload
and IPC channels, Vite/Forge entries, Rust shell validation/runtime, generated SDK custom methods,
and full Desktop app identity ordering. Findings:

- `createMinimalShellHost` still points at the broad `preload.js` and has no production call site;
- `startGoslingServe` already owns loopback binding, generated-secret injection, TLS fingerprint,
  readiness, process registration, graceful/forced cleanup, and parent-death signaling;
- full Desktop app name/protocol/partition/single-instance behavior is hard-coded and cannot be the
  shell entrypoint;
- generated SDK already exposes provisioning read/validate and handoff prepare;
- initialization shell metadata lacked the authoritative custom-method list required for pre-session
  compatibility.

The metadata gap is now patched additively: `availableMethods` is sorted directly from
`GoslingAcpAgent::custom_method_schemas`. The focused Rust unit test passed; the CLI runtime E2E
compiled and its actual spawned-server provisioning/session/isolation test passed.

## Next actions in strict order

1. Create a local compatibility-metadata checkpoint commit; do not push.
2. Implement and exhaustively test pure shell app-identity, lifecycle, and compatibility modules.
3. Implement shell-only IPC channel/types and separate preload with a frozen operation snapshot.
4. Implement ACP adapter/preflight from generated SDK types; reject identity/core/schema/method and
   invalid provisioning before any session create/resume.
5. Implement the dedicated shell bootstrap/main entry with one generation owner and isolated paths.
6. Add shell renderer entry only after the main/preload contracts are green.
7. Extend bounded/redacted diagnostics and explicit handoff, then run process failure-path tests.

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
