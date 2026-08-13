# Build state — Gosling project-shell readiness

Updated: 2026-08-13 post-Gate-4 reassessment
Assessed revision: `6fe6a3bfcab3a48846850fd321fb8a056223d355` on merged `main`
Planning branch: `codex/project-shell-readiness-replan`
Mode: assessment and planning; no named shell, production release, signing, publication, or updater activation
Current gate: **R0 — reconcile truth and restore baseline CI**
Next architecture gate: **R1 — freeze project-shell topology and application contracts**

## Intent

Finish a copy-free, least-privilege shared foundation before DAWES, math, Project ABC, Physics/CST, or another named project shell is implemented. Gosling must own host/runtime contracts; each project must be able to own its renderer, domain UI, adapter/extension, assets, and tests without copying or editing shared orchestration.

## Current decision

**Host substrate: partially ready. Project-shell consumer foundation: not ready.**

The former Gate 4 process boundary is useful and merged, but the former instruction to begin Gate 5 UI is superseded by:

- [`readiness-reassessment.md`](readiness-reassessment.md);
- [`project-shell-readiness-plan.md`](project-shell-readiness-plan.md);
- plan change `SHP-PC-003`.

Do not start `ShellRuntimeProvider`, widen preload, or create a named project shell until R1 freezes the consumer, runtime, permission, and domain-adapter contracts.

## Retained checkpoints

| Area                                               | Revision/evidence               | Current interpretation                                                |
| -------------------------------------------------- | ------------------------------- | --------------------------------------------------------------------- |
| original orientation                               | `ee0d79ee0`                     | historical baseline                                                   |
| V8 helper implementation                           | `72c22f4cc`                     | locally passes; remote Linux path is currently defective              |
| product/host contracts                             | `e68c5791a`                     | retain, subject to new ADRs for missing consumer/runtime/domain seams |
| product profile                                    | `269f04b94`                     | retain; consumer manifest still absent                                |
| host lifecycle/preload/ACP/package/session/handoff | `842356a53` through `098f45cef` | merged host substrate; not a usable project application               |
| Gate 4 status record                               | `bd293a50e`                     | historical local process-boundary GO only                             |
| merged main                                        | `6fe6a3bfc`                     | assessed source and remote CI revision                                |

## Reassessment findings that block project shells

1. Hard-coded `shell.html` / `src/shell/renderer.ts`; no independently supplied renderer composition.
2. Preload has lifecycle/diagnostics/handoff only; no safe session create/resume/prompt/cancel/update API.
3. Focused ACP callbacks auto-cancel permissions, decline elicitations, and discard session updates.
4. Rust `DomainAdapter` exists, but CLI always constructs `ShellRuntime::new(..., None)`.
5. `busy`, `relink_required`, and `fatal` have no production transition source; runtime snapshot lacks verified identity/session/adapter facts.
6. Shell packaging inherits Gosling-specific metadata/permissions and bundles broad `src/bin` resources.
7. No packaged application workflow/restart/coexistence harness and no shell build/release workflows.
8. Current main CI run `31695906352` failed at helper line 44 before Rust tests.

Full details and source references are in the reassessment.

## Current observed validation

Local at `6fe6a3bfc`:

```text
source bin/activate-hermit
scripts/test-with-rusty-v8-cache.sh                         passed on macOS
cd ui/desktop
pnpm run shell:test-profile                                41/41 passed
focused shell/host/serve/handoff Vitest                    120/120 passed, 14 files
pnpm run typecheck                                          passed
```

Remote at the same revision:

```text
Desktop lint/tests                                          passed
Rust format, Clippy, MSRV, schema/SDK, Windows build        passed
Canary and live-provider workflows                          passed
Linux Build and Test Rust Project                           failed
failure                                                     with-rusty-v8-cache.sh line 44: File: unbound variable
```

The macOS helper pass does not override the Linux failure.

## Strict next actions

1. **R0 only:** repair the portable V8 archive-size probe and add Linux/GNU regression coverage.
2. Obtain two clean Linux CI runs on the repaired lineage and confirm Rust tests actually execute.
3. Reconcile evidence/ledgers after observed remote success; do not predeclare it.
4. **R1 architecture:** write and review ADR-0010–0012 for consumer composition, main-owned application runtime, and domain adapter lifecycle/transport.
5. Freeze consumer manifest, safe runtime snapshot, session/prompt/update/permission, and domain operation contracts.
6. Only then implement R2/R3. Shared UI is R5, not the next task.

## Named-shell start policy

Requirements discovery may proceed in project repositories, but no named profile, renderer, adapter, prompt, workflow, branding, or domain special case enters this campaign. Named implementation begins only after milestone M5 (project-shell consumer ready), unless the operator explicitly accepts a narrower development-only exception.

## Open human decisions

- separate repository consumer package versus isolated monorepo workspace consumer (R1; separate repository is preferred but not yet accepted);
- exact out-of-process domain adapter protocol/ownership (R1);
- production identifiers, release destination, signing/notarization, predecessor artifact, and updater policy (later human release gates);
- which platforms require full launch E2E versus structural/readback evidence (R7).

## Verify-don't-trust resume commands

```bash
cd /Users/eric/Work/vscode/forked/gosling
git status --short --branch
git log -10 --oneline --decorate
sed -n '1,220p' AGENTS.md
cat docs/build/shell-productization/readiness-reassessment.md
cat docs/build/shell-productization/project-shell-readiness-plan.md
cat docs/build/shell-productization/build-state.md
cat docs/build/shell-productization/traceability-matrix.md
cat docs/build/shell-productization/risk-register.md
cat docs/build/shell-productization/assumption-ledger.md
cat docs/build/shell-productization/defects.md
gh run list --branch main --workflow CI --limit 5
source bin/activate-hermit
scripts/test-with-rusty-v8-cache.sh
cd ui/desktop
pnpm run shell:test-profile
pnpm exec vitest run src/shell/*.test.ts src/handoffProtocol.test.ts src/shellHost.test.ts src/goslingServe.test.ts
pnpm run typecheck
```

Inspect live source and CI before every status update. Current source, current GitHub checks, and gate-specific runtime/package evidence outrank old summaries.
