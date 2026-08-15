# Build state — Gosling project-shell readiness

Updated: 2026-08-14 after operator selection of a generic Default Shell pre-GUI milestone
R0 implementation revision: `3feffca7c86c7f429b65ee749b8596e5ff4b3d9d` on merged `main`
R1 planning baseline: branch `claude/pre-gui-backend-plan-0wnijv` from `main` at `33a5e73`
Evidence branch: `codex/r0-evidence-reconciliation`
Mode: architecture readiness; no named shell, production release, signing, publication, or updater activation
Completed gate: **R0 — baseline CI restored and evidence reconciled**
Completed gate: **R1 — project-shell topology and application contracts accepted**
Completed gate: **R2 — copy-free consumer composition and package readback**
Completed gate: **R3/R4 — main-owned application runtime and live neutral domain adapter, revision-bound**
Current gate: **DS-1–DS-7 — Default Shell nonvisual foundation and fresh revision-bound acceptance. No renderer or named shell is authorized in the current worktree.**

## Intent

Finish a copy-free, least-privilege shared foundation before DAWES, math, Project ABC, Physics/CST, or another named project shell is implemented. Gosling must own host/runtime contracts; each project must be able to own its renderer, domain UI, adapter/extension, assets, and tests without copying or editing shared orchestration.

## Current decision

**Historical host substrate: accepted on its recorded revision. Current Default Shell additions: isolated on a clean candidate; exact-SHA CI and the operator decision are external gate evidence.**

2026-08-15 corrective status: merged `main` at
`0140e8169c231539c61a4dce98d4e713eccd07ce` is green, and supported-host package/readback,
packaged renderer-to-backend startup, and full-Gosling-plus-two-shell coexistence now pass. The
packaged startup replay found and locally fixed an unbounded credential/Keychain stall; the
follow-up audit also fixed unknown selected-profile acceptance and strengthened live session
pinning. The corrected candidate is isolated as one clean local revision and its profiles report
`sourceClean:true`. The final handoff must bind a successful mandatory CI run to that exact SHA and
record the operator's explicit decision; this source document does not infer those post-commit
facts, and the gate is **NO-GO** without both. See
[`audits/ds7-acceptance.md`](audits/ds7-acceptance.md) for the superseding reassessment.

The operator has narrowed the next MVP to a generic Default Shell template and explicitly deferred
all renderer and named-shell work until its pre-GUI boundaries are complete. ADR-0014 and
[`../../architecture/default-shell-template.md`](../../architecture/default-shell-template.md)
define the active DS-1–DS-7 sequence. The Default Shell must use its own instructions, keep settings
inside its product data root, use credential references without owning secrets, start without
developer tools, and remain a front end to declared supervised backends.

The implementation-ready DS-3–DS-7 breakdown is
[`default-shell-ds3-ds7-implementation-plan.md`](default-shell-ds3-ds7-implementation-plan.md).
It makes the remaining DS-1/DS-2 proof an entry gate, then orders directory/session authority, safe
credential projection and pinning, the v1 module registry, the neutral scaffold, and exact-revision
acceptance. Its v1 boundary permits multiple extensions/skills and one supervised domain adapter;
multiple adapter processes remain a later schema/lifecycle decision.

PG-50 remains valid historical evidence only for exact revision
`b921e6ee1299dba2207ab27ab6fd9452cc57aa26`. The current dirty worktree contains later foundation
changes and therefore does not inherit that acceptance. The local audit is recorded in
[`../../logs/session/2026-08-14-shell-pg50-local-audit.md`](../../logs/session/2026-08-14-shell-pg50-local-audit.md).

This supersedes the "not ready" decision recorded here before 2026-08-14. The former Gate 4
process boundary is useful and merged; the former instruction to begin Gate 5 UI was superseded by:

- [`readiness-reassessment.md`](readiness-reassessment.md);
- [`project-shell-readiness-plan.md`](project-shell-readiness-plan.md);
- plan change `SHP-PC-003`;

and that R2–R4/PG-50 gate is itself now closed — see
[`audits/pg-50-pre-gui-acceptance.md`](audits/pg-50-pre-gui-acceptance.md).

`ShellRuntimeProvider` and all renderer work are paused until DS-7. Do not start any named project
shell (DAWES, math, physics/CST, or other) before M5. R4's conformance is revision-bound and local: the desktop
CI job runs only the default `vitest.config.ts` (`src/**`), not `vitest.integration.config.ts`, so
the non-visual consumer/runtime/adapter conformance suite's evidence is local-only, not CI-backed
(see `audits/pg-50-pre-gui-acceptance.md`). R6/R8 must still reproduce it in packaged and
cross-platform workflows.

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

## Reassessment findings and present status

1. **R2 closed:** strict consumer composition now replaces the hard-coded renderer path; two
   neutral consumers build without host source edits.
2. **R3 locally complete:** preload exposes bounded, main-owned create/resume/prompt/cancel/update
   operations, and the live deterministic harness exercises the recovery and interaction matrix.
3. **R3 locally complete:** ACP permission and elicitation callbacks wait for explicit single-use,
   generation-fenced responses; recovery is tested without giving the renderer ACP authority.
4. **R4 locally complete:** Rust has operator-owned registration, live startup negotiation, pre-decode
   frame-size/resource-count limits, server-owned confirmation, and child-exit supervision. Electron
   relays typed, capability-gated snapshot/action/confirmation only through main with
   generation/session fencing and folds server-pushed adapter status into its safe snapshot. The neutral
   failure matrix is exercised locally; packaged and cross-platform reproduction remains R6/R8.
5. **R3 locally complete:** lifecycle producers and the safe runtime snapshot are exercised;
   identity, compatibility, and runtime namespace are exposed only after profile, initialization,
   and canonical server provisioning agree.
6. **R2 closed:** package metadata, permissions, and staged resources have exact readback proof on
   the macOS host target; cross-platform workflow evidence remains R6/R7.
7. **R7/R8 open:** packaged restart/coexistence harnesses and reusable build/release workflows do
   not yet exist.
8. PG-50 is **accepted** on exact revision `b921e6ee1299dba2207ab27ab6fd9452cc57aa26` (PR #50):
   repeated 15/15 live conformance including backend/adapter restart, the full Desktop suite,
   `linux-x64` package readback, source-negative-space review, all local checks, and green current
   CI were all reproduced on that one committed revision with `sourceClean:true`. Shared GUI (R5)
   is authorized to begin; named shells remain blocked until M5. See
   [`audits/pg-50-pre-gui-acceptance.md`](audits/pg-50-pre-gui-acceptance.md).

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

**R1 is accepted for implementation.** The operator's 2026-08-13 request to take Gosling to
GUI-shell readiness accepted ADR-0010–0012 and the companion contracts as the R1 topology and
authority decision. R2 PG-21–PG-25 provide a strict consumer resolver, fixed-host Vite/Forge
composition, two neutral consumer fixtures, target-specific one-binary staging, minimized package
metadata/permissions, and exact package verifier coverage. A real unsigned macOS arm64 consumer
package passed readback on this checkout.

_(Historical, as recorded on 2026-08-13/14 before the PG-50 rerun — preserved for the evidence
trail; the acceptance decision at the top of this document is current.)_

R3 is **locally implemented and exercised; it is not PG-50 acceptance**. The current desktop worktree contains a main-owned
single-session controller with create/resume, bounded prompt submit/cancel, generation and
attempt fencing, monotonic update sequence, typed IPC/preload exposure, and a main-owned
permission/elicitation controller. ACP callbacks now wait for an explicit, single-use renderer
decision and are cancelled on runtime/session teardown; they are no longer blanket-cancelled or
declined. Their owning controller independently verifies the record generation and session before
consuming a response, in addition to the IPC fence. Standard ACP `sessionUpdate` callbacks project only bounded text, tool progress, session
title, and usage fields. `runtime.read` and its change event now return a safe partial snapshot:
verified identity, compatibility, and runtime namespace after ACP preflight, bounded provisioning issue
code/schema paths, the active session, pending interaction summaries, and the live adapter descriptor
after a consumer-manifest comparison. The namespace is checked across the resolved profile/manifest,
ACP initialization metadata, and canonical server provisioning validation before the snapshot can
expose it. Raw ACP content, credentials, endpoints,
tokens, configuration, and private paths are excluded. Prompt activity, credential relink failures,
and cleanup failures now have runtime transition producers, with rejected lifecycle transitions
recorded as redacted diagnostic codes. Focused tests and `pnpm run typecheck` pass locally. At the
time this paragraph was written, this was not yet a shared-GUI readiness claim; PG-50 has since
been accepted on a clean, revision-bound rerun of this evidence (see
[`audits/pg-50-pre-gui-acceptance.md`](audits/pg-50-pre-gui-acceptance.md)). Durable
prompt-outcome reconciliation now makes compacted resume `clean` only after a persisted terminal
outcome and preserves `uncertain` after interruption or for legacy sessions. A Node-environment
integration test now runs a real `gosling serve` child through compatibility, session create, a
deterministic local OpenAI-compatible streamed prompt, in-flight cancellation, and explicit
allow-once/deny permission decisions that respectively resume a held tool turn or prevent its
requested side effect; it also covers a no-credential prompt attempt, a live MCP form elicitation
that resumes after a bounded main-owned submission, unexpected child exit, retry, and explicit
compacted resume without a renderer. The restart harness verifies both a completed prompt's `clean`
resume and an interrupted held prompt's `uncertain` resume.
R4 now has a bounded startup slice: the descriptor carries a protocol version and typed
`read`/`mutate` actions; an operator-owned `domain_adapters` registration is validated before use;
and `gosling serve` resolves a matching registration, starts it through the hardened stdio-MCP path,
and rejects startup unless the live descriptor exactly matches provisioning. Adapter tool inputs,
decoded tool results, and newline-delimited MCP frames are bounded by the registration value before
JSON decoding. Rust now retains a `mutate` action only behind an opaque, session/generation-fenced,
single-use confirmation ID and executes it inline only after the matching confirm request. Adapter
tool calls also observe the configured deadline. Electron parses the live descriptor, compares it
against the consumer declaration before compatibility is accepted, and projects only its bounded
ID/protocol/action names into the runtime snapshot. Its main/preload boundary relays typed domain
snapshot/action/confirmation requests only when the consumer declared the capability; main checks
generation and active session before calling the server, and carries no adapter transport, process,
payload interpretation, or confirmation authority. The supervised MCP child now sends its exit
status through a typed custom ACP notification; Electron advertises that notification capability and
folds `ready`/`crashed`/`hung`/`incompatible` into the safe adapter snapshot. `gosling serve` now
uses graceful signal shutdown, which drops the server-owned adapter guard. A neutral test process
proves descriptor negotiation, snapshot/read action calls, a confirmed mutation through
`ShellRuntime`, malformed output rejection, a post-negotiation crash, a hang timeout, and explicit
resource-reference overproduction rejection over stdio MCP. The real authenticated
`gosling serve` integration harness supplies an independent neutral-consumer manifest, negotiates
the adapter, creates a session, requests and confirms a `toggle` mutation, proves that an externally
terminated idle adapter changes the safe snapshot to `crashed`, and verifies both an empty backend
registry and a dead adapter PID after normal shutdown. A mismatched live adapter version fails before
`ready` and is reaped. A non-cooperative adapter with an in-flight action receives group SIGTERM and
is reaped after its backend exits, so forced desktop cleanup cannot leave it behind. An in-flight
adapter crash fails the action and projects `crashed`; a backend restart reaps the old adapter, starts
a new process, restores `ready`, and completes a snapshot. This completes local R4 conformance.
Subsequent work must follow the accepted contract without adding a second authority or
consumer-specific host branch.

## Strict next actions

PG-50 is accepted on exact revision `b921e6ee1299dba2207ab27ab6fd9452cc57aa26` (PR #50); see
[`audits/pg-50-pre-gui-acceptance.md`](audits/pg-50-pre-gui-acceptance.md) for the full
condition-by-condition disposition, current-CI result, and the one residual gap (macOS
package-readback could not be reproduced in the accepting sandbox; `linux-x64` readback stands in
as the supported-host-target proof).

1. Close the remaining DS-1/DS-2 live proof and settings migration/recovery entry gate recorded in
   `default-shell-ds3-ds7-implementation-plan.md`.
2. Execute DS-3–DS-6 in dependency order: working-directory/session authority, safe credential
   metadata and backend pinning, bounded module inventory, then the neutral scaffold.
3. Run DS-7 on one exact clean revision with current CI before beginning the Default Shell GUI.
4. R6–R8 (packaged restart/coexistence, cross-platform workflow coverage, signing/release/
   publication) remain open and unauthorized by this acceptance.

## Named-shell start policy

Requirements discovery may proceed in project repositories, but no named profile, renderer, adapter, prompt, workflow, branding, or domain special case enters this campaign. Named implementation begins only after milestone M5 (project-shell consumer ready), unless the operator explicitly accepts a narrower development-only exception.

## Open human decisions

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
