# PG-50 pre-GUI backend acceptance audit

Date: 2026-08-14  
Decision: **NO-GO — formal PG-50 acceptance is not yet valid**

## Scope and decision basis

This audit re-evaluates the shared Gosling backend for the pre-GUI completion definition in
[`pre-gui-backend-implementation-plan.md`](../pre-gui-backend-implementation-plan.md#5-pre-gui-completion-definition).
It covers the generic project-shell substrate only. It does not authorize shared GUI work or any
DAWES, math, physics/CST, Project ABC, or other named shell.

The local worktree contains the R1–R4 implementation and its evidence, but it is not a clean,
committed acceptance revision. The closest committed baseline is
`34920cc037d741999a0aa48540ad4d63ee296c3c`; CI run
[`31756273340`](https://github.com/cephalopod-ai/gosling/actions/runs/31756273340) is green for
that commit. It cannot validate the current uncommitted worktree. The profile checker also records
`sourceClean: false` for the local package. This fails PG-50 condition 12 and the plan requirement
that every condition be proven on one exact revision.

No attempt was made to commit, reset, stash, or otherwise alter the mixed worktree. That preserves
in-progress operator work, including unrelated Desktop settings changes.

## Local evidence collected

- The same real `gosling serve` non-visual integration suite passed twice independently: 15/15
  tests on each run. It covers deterministic streaming, cancel, terminal failure, explicit
  permission allow-once/deny, MCP elicitation, durable clean/uncertain resume, compatibility
  failure without a durable session, adapter mismatch, confirmation, crash/hang, forced reaping,
  and backend/adapter restart.
- The focused Desktop shell suite passed: 17 files, 152 tests. Desktop typecheck and focused ESLint
  also passed.
- `cargo fmt --all -- --check`, SDK-type unit tests (12), focused CLI provisioning/runtime tests
  (3 + 3), and targeted Clippy completed without diagnostics.
- ACP schema generation was run twice with no generated-file diff; the TypeScript SDK distribution
  build passed after the schema change.
- Profile tests and the consumer/profile checker passed. The checker truthfully reports the dirty
  source state rather than treating it as release-ready.
- An unsigned macOS arm64 Fixture A package was built and independently read back with the package
  verifier. It validated exact profile/manifest/provisioning resources, the staged binary hash,
  app-ASAR content/negative-space checks, and macOS metadata. The verified binary SHA-256 was
  `0029cb6cbc804c54725ad18102fda032225fb8998e2a31847b939915920cf52a`.
- Current CI for the committed baseline completed Rust format, Rust lint, Rust build/test, MSRV,
  Windows build, Desktop lint/test and profile validation, and schema/SDK jobs successfully.

## Acceptance-condition disposition

| PG-50 condition | Local audit result | Formal disposition |
| --- | --- | --- |
| 1. Accepted topology and ownership | ADR-0010–0012 remain accepted; no new P0 decision was found. | Locally evidenced |
| 2–3. Strict, copy-free neutral consumer composition | Consumer manifests select two structurally distinct neutral renderers through the fixed host. | Locally evidenced |
| 4. Main-owned bounded ACP application service | Preload/IPC and source review show main-owned, typed create/resume/prompt/cancel/update operations with no raw ACP/process/configuration authority. | Locally evidenced |
| 5. Explicit stale-safe interaction decisions | The double integration run exercises fenced permission and elicitation decisions, cancellation, and negative paths. | Locally evidenced |
| 6. Lifecycle producers and recovery | Runtime/restart/reaping cases exercise the required failure and recovery states. | Locally evidenced |
| 7. Safe verified snapshot | Identity, compatibility, canonical server provisioning, and runtime namespace must agree before safe snapshot projection. | Locally evidenced |
| 8–9. Live bounded/supervised adapter | Live neutral read action, confirmed mutation, mismatch, malformed output, crash, hang, overproduction, replay/staleness, and orphan cleanup have local evidence. | Locally evidenced |
| 10. Exact package resources and metadata | Fixture A macOS arm64 package and readback verifier passed. | Locally evidenced |
| 11. Non-visual conformance | The two complete 15-test live runs provide this evidence. | Locally evidenced |
| 12. Local checks/current CI on one exact revision | Local checks pass, but CI run 31756273340 covers committed `34920cc…`, not the dirty worktree. | **Not satisfied** |

## Negative-space and domain-neutrality review

The shared runtime, shell fixture, and consumer-fixture source was searched for DAWES, physics,
math, CST, Project ABC, raw ACP authority, direct renderer Node/Electron authority, and named
domain behavior. The named terms found in generic Rust source are fixture/test labels used to prove
per-consumer path isolation; they do not implement scientific, mathematical, or project behavior.
No shell source match introduced `nodeIntegration: true`, `contextIsolation: false`, broad remote
module use, or a renderer-owned ACP endpoint/token. The main/preload boundary remains typed and
capability-gated; the generic adapter surface projects bounded opaque descriptors/actions rather
than interpreting a domain payload.

This is a source review, not a substitute for the clean-revision acceptance required below.

## Remaining work and required next decision

PG-50 is deliberately not accepted. Before R5 shared GUI work can begin, the owner must isolate
the intended R1–R4 changes into a reviewable exact revision, run the required CI on that revision,
and rerun or retain reproducible revision-bound PG-50 evidence. The review must separately verify
the mixed worktree's unrelated files are not accidentally included.

Even after PG-50 acceptance, R6–R8 remain open: packaged restart/coexistence evidence,
cross-platform workflow coverage, and release/signing/publication decisions. Named project GUI
work remains blocked until M5.
