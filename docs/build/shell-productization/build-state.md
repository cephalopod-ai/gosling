# Build state — Gosling project-shell readiness

Updated: 2026-08-13 after R1 ADR drafting
R0 implementation revision: `3feffca7c86c7f429b65ee749b8596e5ff4b3d9d` on merged `main`
R1 draft revision: branch `claude/pre-gui-backend-plan-0wnijv` from `main` at `33a5e73`
Evidence branch: `codex/r0-evidence-reconciliation`
Mode: architecture readiness; no named shell, production release, signing, publication, or updater activation
Completed gate: **R0 — baseline CI restored and evidence reconciled**
Current gate: **R1 — freeze project-shell topology and application contracts (ADR-0010–0012 and the R1 contracts addendum drafted as `proposed`; NOT yet accepted — R1 is not complete)**

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

| Area                                               | Revision/evidence                     | Current interpretation                                                |
| -------------------------------------------------- | ------------------------------------- | --------------------------------------------------------------------- |
| original orientation                               | `ee0d79ee0`                           | historical baseline                                                   |
| V8 helper implementation                           | `72c22f4cc`, `9d8ef5db9`, `ee3db7893` | numeric GNU/BSD probes pass locally and on Linux CI                   |
| product/host contracts                             | `e68c5791a`                           | retain, subject to new ADRs for missing consumer/runtime/domain seams |
| product profile                                    | `269f04b94`                           | retain; consumer manifest still absent                                |
| host lifecycle/preload/ACP/package/session/handoff | `842356a53` through `098f45cef`       | merged host substrate; not a usable project application               |
| Gate 4 status record                               | `bd293a50e`                           | historical local process-boundary GO only                             |
| R0 repair PR                                       | `436c846f0`, PR #47                   | one clean PR Linux Rust execution                                     |
| merged main                                        | `3feffca7c`                           | second clean Linux Rust execution                                     |

## Reassessment findings that block project shells

1. Hard-coded `shell.html` / `src/shell/renderer.ts`; no independently supplied renderer composition.
2. Preload has lifecycle/diagnostics/handoff only; no safe session create/resume/prompt/cancel/update API.
3. Focused ACP callbacks auto-cancel permissions, decline elicitations, and discard session updates.
4. Rust `DomainAdapter` exists, but CLI always constructs `ShellRuntime::new(..., None)`.
5. `busy`, `relink_required`, and `fatal` have no production transition source; runtime snapshot lacks verified identity/session/adapter facts.
6. Shell packaging inherits Gosling-specific metadata/permissions and bundles broad `src/bin` resources.
7. No packaged application workflow/restart/coexistence harness and no shell build/release workflows.
8. Linux CI is restored; project-shell readiness remains blocked by findings 1–7, not baseline CI.

Full details and source references are in the reassessment.

## Current observed validation

Local at `436c846f0`:

```text
source bin/activate-hermit
scripts/test-with-rusty-v8-cache.sh                         passed repeatedly on macOS
cargo fmt --all -- --check                                 passed
cargo clippy --all-targets -- -D warnings                  passed
cargo test -p gosling --test agent                         17/17 passed
focused concurrent-tool regression                         10 consecutive passes
focused Anthropic weather replay                            passed
pnpm --dir ui/desktop run shell:test-profile               41/41 passed
pnpm --dir ui/desktop run shell:check-profiles              passed; sourceClean=true
```

Remote on the repaired lineage:

```text
PR run 31731952749 / job 94554362098                        passed
merged-main run 31732990062 / job 94557761229               passed
V8 helper self-test and verified archive preparation        passed in both jobs
full Linux Build and Test step                              passed in both jobs
PR Desktop, format, Clippy, MSRV, and schema/SDK jobs       passed
both complete CI workflows                                  passed overall
merged-main Windows Rust build                              passed
```

The two Linux jobs are independent executions on the repaired branch and merged `main`, not retries of one attempt. Detailed evidence is in [`evidence/r0.md`](evidence/r0.md).

## Reopened Gate 4 acceptance

Gate 4 remains a historical local process-boundary acceptance. R0 does not relabel it as full application-runtime acceptance. The following originally declared paths remain reopened:

- renderer crash and cleanup;
- ACP loss/reconnect and compacted resume;
- forced-cleanup escalation;
- the complete typed startup/runtime failure matrix.

R3 must implement truthful event/state/recovery behavior. R6 must exercise these paths in the packaged workflow and coexistence matrix.

## R1 drafting status (2026-08-13)

ADR-0010 (project-shell consumer composition), ADR-0011 (main-owned application runtime and
renderer capability boundary), and ADR-0012 (domain adapter lifecycle, transport, and authority) are
drafted at `docs/adr/0010-project-shell-consumer-composition.md`,
`docs/adr/0011-shell-application-runtime-boundary.md`, and
`docs/adr/0012-shell-domain-adapter-topology.md`. Each is grounded in the exact current source
(`ui/desktop/shell.html`, `vite.shell.renderer.config.mts`, `forge.config.ts`,
`GoslingShellAPI`/`ipc.ts`, `ShellAcpConnection`/`acpRuntime.ts`, `DomainAdapter`/`ShellRuntime` in
`crates/gosling/src/acp/shell.rs`, and `build_shell_runtime` in `crates/gosling-cli/src/cli.rs`) and
each selects the plan's preferred direction: separate-repository consumer topology (ADR-0010),
main-owned bounded session/prompt/permission/elicitation operations (ADR-0011), and a versioned
out-of-process domain adapter (ADR-0012). The companion schema freeze is
`docs/architecture/shell-productization-r1-contracts.md` (consumer manifest v1, safe runtime
snapshot v2, application-runtime operations/events, interaction records, adapter descriptor and
confirmation token, error taxonomy additions, negative-space rules).

**Status is `proposed`, not `accepted for implementation`.** Per the parent plan
("architecture review accepts one topology and rejects alternatives explicitly... Draft ADRs are not
R1 completion"), R1 does not exit until an independent architecture/operator review accepts these
three ADRs and confirms no open P0 topology/authority decision remains — in particular the two
items `assumption-ledger.md` still lists as `unresolved` (SHP-ASM-029 separate-repo vs
isolated-workspace, SHP-ASM-031 out-of-process adapter). This session did not implement any R2–R4
code (Vite/Forge, IPC/preload, Rust adapter registration): the plan explicitly forbids that before
R1 schemas are frozen and accepted, and the topology/transport choices are named as human decisions
the plan requires the operator to accept, not an agent to self-certify.

## Strict next actions

Execute the dependency-aware work packages in
[`pre-gui-backend-implementation-plan.md`](pre-gui-backend-implementation-plan.md). In summary:

1. **Obtain operator/architecture review and acceptance of ADR-0010–0012** and the R1 contracts
   addendum drafted above. This is the remaining R1 exit criterion.
2. Only after acceptance, implement R2 (consumer resolver/Vite/Forge) and pure R3 reducers per the
   plan's safe-concurrency rule.
3. Only then implement the rest of R3/R4. Shared UI is R5, not the next task.

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
cat docs/build/shell-productization/evidence/r0.md
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
