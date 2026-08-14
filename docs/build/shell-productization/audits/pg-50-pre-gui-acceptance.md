# PG-50 pre-GUI backend acceptance audit

Date: 2026-08-14 (revision-bound rerun)
Decision: **GO — PG-50 is accepted; R5 shared GUI shell kit work is authorized**

Supersedes the same-day earlier entry in this file, which recorded a NO-GO because the R1–R4
evidence was collected against a dirty local worktree instead of a committed revision. That
finding is preserved in git history (`c232d04`'s parent, `5933637`) rather than deleted, per this
campaign's evidence rules (`pre-gui-backend-implementation-plan.md#10-testing-and-evidence-strategy`).

## Exact acceptance revision

- Branch: `claude/pg-50-shared-gui-shell-skdwwh`
- Commit: `b921e6ee1299dba2207ab27ab6fd9452cc57aa26`
- Pull request: [cephalopod-ai/gosling#50](https://github.com/cephalopod-ai/gosling/pull/50)
- `pnpm run shell:check-profiles` on this commit reports `"sourceClean":true` for both neutral
  fixtures, with `resolvedGoslingRevision` equal to the commit above. This is the condition-12
  proof the prior audit could not produce.

## What changed to reach a clean, exact revision

1. The R1–R4 shell/backend commit (`5933637`) had incidentally carried an unrelated 15-line
   `ui/desktop/src/components/settings/chat/SummarizerSection.tsx` change (auto-fill and
   auto-persist a default summarizer endpoint) that has nothing to do with project-shell
   productization. It was removed by a new commit (`c232d04`) that restores the file to its
   pre-`5933637` state, isolating the R1–R4 work.
2. Re-running `cargo clippy --all-targets -- -D warnings` on the isolated revision surfaced one
   real lint violation introduced by the R1–R4 work: a manual `if let Err(..) { return Err(..) }`
   block in `crates/gosling/src/acp/server.rs` (prompt-failure path) that clippy's
   `question_mark` lint flags. Fixed with commit `b921e6e` (`persisted?;`). This is
   `SHP-DEF-034` below.
3. No other defect was found. The R1–R4 implementation itself was not changed beyond this one
   lint fix.

## Local evidence collected on `b921e6e`

- `cargo fmt --all -- --check` — clean.
- `cargo clippy --all-targets -- -D warnings` — clean (after `SHP-DEF-034` fix).
- `cargo test -p gosling` — all suites pass, including the seven `acp::domain_adapter` unit tests
  (negotiate, confirmed mutation, crash mapping, malformed output, timeout/hang, multi-block
  rejection).
- `cargo test -p gosling-cli --test shell_runtime_e2e_test` and
  `--test shell_provisioning_validation_test` — 3/3 and 3/3, each run twice independently.
- ACP schema generation (`cargo run --bin generate-acp-schema`) run twice: no diff against the
  checked-in `acp-schema.json`/`acp-meta.json`. The TypeScript SDK generator
  (`ui/sdk generate-schema.ts`, exercised via the `desktop` package's `postinstall`) also produced
  no diff against `ui/sdk/src/generated/**`.
- `pnpm run typecheck` and `pnpm run lint:check` (ESLint at zero warnings plus i18n catalog
  checks) — clean.
- Full desktop `vitest` unit suite — 106 files, 729/729 passed.
- `pnpm run shell:test-profile` — 47/47 passed.
- `pnpm run shell:check-profiles` — `sourceClean:true` on this exact commit (see above).
- The non-visual consumer/runtime/adapter conformance suite,
  `vitest run --config vitest.integration.config.ts tests/integration/shell_session_runtime.test.ts`,
  which spawns a real `gosling serve` child and drives it through the main-owned ACP runtime: run
  twice independently, 15/15 both times. It covers deterministic streaming, in-flight cancel,
  terminal child-exit failure, explicit permission allow-once/deny, live MCP form elicitation,
  compatibility failure without a durable session, adapter version mismatch with reaping,
  fenced prompt-attempt recovery from a real child exit, confirmed neutral-adapter mutation,
  idle and in-flight adapter crash projection, non-cooperative adapter reaping during cleanup,
  and **backend/adapter restart** (`creates after compatibility and resumes after a backend
  restart`, `restarts the adapter with the backend and restores its declared capability`).
  A first attempt at this suite showed one timeout (`streams a deterministic provider response...`,
  60s budget exceeded at 325s) while several unrelated `cargo` compilations were running
  concurrently on this 4-core sandbox. Per this plan's evidence rule that a retry-only pass is not
  closure for a flaky process test, the contention hypothesis was reproduced rather than assumed:
  a fresh `cargo build --workspace` was started deliberately against a scratch target directory to
  saturate all 4 cores, and the suite was rerun against that live contention. It failed again, but
  on a *different* test this time (`does not create durable session state when compatibility
  fails`, `no such table: sessions` — the test's own SQLite readback raced the real `gosling serve`
  child's session-store initialization). With the contention build killed, three subsequent
  back-to-back runs were clean: 15/15, 15/15, 15/15 (~35s each). The failure moving to a different
  test under a different contention source, while every run is 15/15 with no competing CPU load,
  establishes the mechanism: this suite spawns and depends on a real `gosling serve` subprocess and
  its SQLite session store, and severe CPU starvation of this 4-core sandbox (this task ran many
  concurrent `cargo` builds and `pnpm` installs throughout the session) pushes real subprocess/DB
  startup past the suite's fixed timeouts. It is not a defect in the R1-R4 shell/backend code, and
  it is not present in an uncontended run. It is, however, a real finding for anyone rerunning this
  suite: **do not run it alongside other heavy CPU consumers**, and consider raising its
  per-test timeout as future hardening against loaded machines (not done here, to avoid changing
  test behavior inside an acceptance-evidence PR). PG-50 condition 11 is satisfied by three
  consecutive clean, uncontended 15/15 runs, not by treating the earlier contended failures as
  transient noise without explanation.
- **Correction to avoid overstating scope:** the desktop CI job (`Test and Lint Electron Desktop
  App`, `pnpm run test:run`) runs the default `vitest.config.ts`, which only includes
  `src/**/*.test.ts` — it does not execute `vitest.integration.config.ts` or this conformance
  suite. Current CI (below) corroborates the unit-test/typecheck/lint/profile evidence above; the
  non-visual consumer/runtime/adapter conformance suite's evidence is local-only, exactly as it was
  in the prior (NO-GO) audit. This matches the plan's own test-layer table, which lists "spawned
  neutral adapter integration" and "non-visual conformance workflow" as local layers distinct from
  "current CI."
- Package build and readback for the `linux-x64` target (`gosling-shell-fixture-a` profile), run
  from `ui/desktop` after `cargo build --release --target x86_64-unknown-linux-gnu`:

  ```sh
  node scripts/package-shell.js \
    /home/user/gosling/fixtures/shell-products/fixture-a/product-profile.json \
    --consumer /home/user/gosling/fixtures/shell-consumers/consumer-a/shell-consumer.json \
    --platform linux --arch x64

  node scripts/verify-shell-package.js \
    /home/user/gosling/fixtures/shell-products/fixture-a/product-profile.json \
    --consumer /home/user/gosling/fixtures/shell-consumers/consumer-a/shell-consumer.json \
    --platform linux --arch x64 \
    --package "/home/user/gosling/ui/desktop/out/Gosling Shell Fixture A-linux-x64" \
    --binary /home/user/gosling/target/x86_64-unknown-linux-gnu/release/gosling
  ```

  Both succeed; the verifier's JSON output confirms exact profile/manifest identity
  (`profileHash: c99e5901a56664291976a7e1b5d49583aabf9ebfab6e185b4cc5e5ea9bdb6375`), the
  target-specific staged binary hash
  (`4f2af733dc688ed2f6b22b6207a1c0630c5dbeb8f0c23d61fba106673ae46ab0`), and no unexpected package
  content.
- **Residual gap, disclosed rather than hidden:** this sandbox has no macOS host, so the
  `darwin-arm64` package readback the prior (non-revision-bound) local audit performed could not
  be repeated on this exact commit. The `linux-x64` readback above proves the same
  resource/metadata/manifest machinery on a different supported target. Full cross-platform
  package-readback parity (macOS, Windows, and Linux together, on one revision) remains R6/R7
  scope per the plan, not a PG-50 condition-10 blocker — condition 10 requires exact,
  consumer-derived resources on a supported host target, which is met here.

## Current CI

GitHub Actions run triggered by PR #50 against commit `b921e6e`: all 22 required/attached checks
completed with `success` or an expected `skipped` (path-filtered jobs such as the Windows Rust
build and `machete`, which only run when their trigger conditions are met). No check failed.
`Build and Test Rust Project`, `Lint Rust Code`, `Check Rust Code Format`, `Check MSRV`,
`Check Generated Schemas are Up-to-Date`, `Test and Lint Electron Desktop App`, and `Build Binary`
all passed.

## Acceptance-condition disposition

| PG-50 condition | Result | Disposition |
| --- | --- | --- |
| 1. Accepted topology and ownership | ADR-0010–0012 remain accepted; no new P0 decision found. | Satisfied |
| 2–3. Strict, copy-free neutral consumer composition | Two structurally distinct neutral consumers resolve/build through the fixed host; `shell:test-profile` 47/47. | Satisfied |
| 4. Main-owned bounded ACP application service | Preload/IPC review plus the live integration suite show typed create/resume/prompt/cancel/update with no raw ACP/process/configuration authority. | Satisfied |
| 5. Explicit stale-safe interaction decisions | Live suite exercises fenced permission allow-once/deny and MCP elicitation. | Satisfied |
| 6. Lifecycle producers and recovery | Live suite exercises child-exit recovery, compatibility failure, and forced/non-cooperative cleanup. | Satisfied |
| 7. Safe verified snapshot | Unit and integration coverage confirm identity/compatibility/provisioning agreement before snapshot exposure; source review found no raw-authority leak. | Satisfied |
| 8–9. Live bounded/supervised adapter | Live suite covers negotiation, confirmed mutation, mismatch-before-ready with reaping, idle/in-flight crash, and **backend/adapter restart**. | Satisfied |
| 10. Exact package resources and metadata | `linux-x64` fixture-A package readback passes with exact binary hash/manifest identity on this revision; macOS readback not reproducible in this sandbox (see residual gap above). | Satisfied for the available host target |
| 11. Non-visual conformance | Three consecutive clean, uncontended 15/15 runs; the one earlier contended failure was deliberately reproduced under artificial CPU saturation (a different test failed each time) to establish, not assume, that the mechanism is subprocess/DB timing under severe local resource starvation rather than a product defect. | Satisfied |
| 12. Local checks/current CI on one exact revision | `sourceClean:true` on `b921e6e`; all local checks above pass on that commit; current CI green for that commit (PR #50). | **Satisfied** |

## Negative-space and domain-neutrality review

Re-ran the source search for DAWES, physics, math, CST, Project ABC, raw ACP authority, direct
renderer Node/Electron authority, and named domain behavior across `crates/gosling/src`,
`ui/desktop/src`, and `fixtures/`. The only match is the pre-existing `dawes`/`math` pair in
`crates/gosling/src/config/paths.rs`'s namespace-isolation unit test, used purely as arbitrary
distinct namespace strings to prove path isolation — no scientific, mathematical, or project
behavior is implemented. `ui/desktop/src/shell/preload.ts`, `preloadApi.ts`, and `ipc.ts` contain
no ACP URL/token/process-handle exposure. No `nodeIntegration: true` or `contextIsolation: false`
appears anywhere in the shell Vite/Forge/main configuration. No `TODO`/`FIXME`/debug `console.log`
was found in the R1–R4 adapter or shell core modules.

## Defect ledger updates

- `SHP-DEF-034` (new): `cargo clippy -D warnings` failure (`clippy::question_mark`) in the R1–R4
  prompt-failure path, `crates/gosling/src/acp/server.rs`. Severity: low (lint only, no behavior
  change). Fixed in commit `b921e6e`; verified by a clean clippy rerun. See `defects.md`.
- `SHP-DEF-035` (new): the R1–R4 commit incidentally carried an unrelated `SummarizerSection.tsx`
  UX change. Severity: low (scope hygiene, no functional defect in either change). Fixed by
  isolating it out in commit `c232d04`. See `defects.md`.
- No critical or high pre-GUI (R1–R4/PG-50-scoped) finding remains open in `defects.md`. The
  open high-severity entries (`SHP-DEF-002`, `SHP-DEF-007`, `SHP-DEF-029`, `SHP-DEF-030`) are all
  explicitly scoped to R6–R8 (packaged coexistence, external consumer SDK conformance, reusable
  release workflows), which the plan's non-goals section excludes from PG-50. `SHP-DEF-028` is
  preserved as historical acceptance debt from the original Gate 4 process and is superseded by
  the R3/R4 recovery evidence recorded here and in `build-state.md`.

## Decision

All twelve pre-GUI completion-definition conditions in
[`pre-gui-backend-implementation-plan.md#5-pre-gui-completion-definition`](../pre-gui-backend-implementation-plan.md#5-pre-gui-completion-definition)
are satisfied on exact revision `b921e6ee1299dba2207ab27ab6fd9452cc57aa26`. **PG-50 is accepted.**
R5 (shared GUI shell kit implementation) may begin. This acceptance does not authorize R6–R8
(packaged restart/coexistence, cross-platform workflow coverage, signing/release/publication) or
any named-shell (DAWES, math, physics/CST, or other) implementation, which remain blocked until
M5 per the plan's named-shell start policy.
