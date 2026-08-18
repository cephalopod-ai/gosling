# 2026-08-18 — Live playtest repair campaign

## Task

Run the complete 110-card playtest library, assess scenario coverage, consolidate the result with
prior findings and TODOs, and repair the five highest-priority reliability and data-flow defects.

## Gate 0 — orientation and baseline

- Branch: `codex/playtest-repair-20260818`, created from clean `main` at `c203282753b93066f0af88ce196200b785e6454f`.
- Remote: `origin` exists; no fetch, push, PR, or remote mutation is authorized.
- Involvement: L2 standard (inferred). Authority: governed local repair.
- Baseline build: `cargo build -p gosling-cli --bin gosling` passed.
- Playtest state: disposable root and fixtures under `/tmp/gosling-playtest-20260818`; the installed
  user Desktop instance and its state were not modified.
- Validation inventory: targeted Rust and Desktop tests per group, `cargo fmt`, Desktop typecheck,
  workspace build/tests/clippy, and live reruns of the five failed workflows.
- Contract sources: `docs/architecture.md`, ADR-0013, CLI command documentation, and the scenario
  cards governing session, ACP, and artifact behavior. Prior cloud reports are evidence records, not
  higher-authority contracts.

## Gate 1 — defect inventory

| ID | Domain | Priority | Complexity | Touch set and proof |
|---|---|---:|---|---|
| GSL-PLAY-2026-007 | reliability / data integrity | P1 | medium | `CliSession::process_agent_response` and interruption persistence; live Ctrl-C returned exit 0 and invented an assistant reply |
| GSL-PLAY-2026-005A | reliability / CLI correctness | P1 | low | `SessionCommand::Remove` and `commands/session.rs`; named non-TTY removal announced deletion before failing with `not connected` |
| GSL-PLAY-2026-005B | data integrity / CLI correctness | P1 | low | interactive session fork setup; non-TTY fork copied the session before input failed with `not connected` |
| GSL-PLAY-2026-006 | reliability / protocol data flow | P1 | medium | ACP stdio EOF arbitration; an initialize request followed by EOF exited 0 with no response bytes |
| GSL-PLAY-2026-008 | data flow / Desktop correctness | P1 | low | artifact routing config; a workspace-less CLI session exposed inventory but installed no exact-file capability, so preview was denied |

Security candidates remain open but were deliberately ordered below these reliability/data-flow
failures at the operator's direction. Intentionally deferred architecture and performance items in
`docs/TODO.md` remain protected.

## Gate 2 — locality groups and plan

1. **Session lifecycle:** GSL-PLAY-2026-007, 005A, and 005B. Minimal edits in
   `session/mod.rs`, `cli.rs`, and `commands/session.rs`; targeted CLI tests and live session reruns.
   `session/mod.rs` (2734 lines) and `cli.rs` (2548 lines) exceed the 2000-line in-band split limit,
   so this campaign applies only the smallest safe fixes and routes modularization to a dedicated run.
2. **ACP stdio:** GSL-PLAY-2026-006 in `acp/server.rs`; preserve the ACP v1 contract while allowing
   queued responses to drain after input EOF; targeted unit/live protocol checks.
3. **Desktop artifact capability:** GSL-PLAY-2026-008 in `ArtifactRouterContext`; preserve ADR-0013's
   exact-file capability boundary for sessions without a workspace; targeted Vitest/typecheck/live preview.

The groups are sequential because the live reruns share one rebuilt CLI/package and disposable state.
Each verified group is a local commit boundary; no push is permitted.

## Group 1 — session lifecycle

### Repair rationale

- A headless Ctrl-C is a terminal failure, not a successful assistant turn. The user prompt now
  remains in history, a truthful cancellation notice is persisted, and the process exits non-zero.
  Headless provider errors likewise no longer synthesize the interactive recovery prompt.
- Named/regex removal remains confirm-by-default. Non-TTY callers now fail before output or mutation
  unless they explicitly pass `--yes`.
- Interactive `--fork` has no scriptable continuation input, so a non-TTY invocation now refuses
  before `copy_session` rather than creating a fork and failing afterward.

### Verification and adversarial review

- `cargo test -p gosling-cli --lib` — 245/245 passed before the parser regression was added.
- `cargo test -p gosling-cli --lib session_remove_accepts_non_interactive_confirmation` — passed.
- `cargo test -p gosling-cli --lib session::tests::remove_local_turn_removes_only_the_interrupted_suffix` — passed.
- Live PTY Ctrl-C — exit 1; export retained `FIXTURE-DELAY` and `Run cancelled by user before completion.`;
  the prior invented `Yes — what would you like me to do?` was absent.
- Live non-TTY removal — without `--yes`: exit 1, zero stdout bytes, session retained; with `--yes`:
  exit 0, session removed.
- Live non-TTY fork — exit 1, zero stdout bytes, session count unchanged (46 before and after).
- Adversarial checks: interactive confirmation/recovery behavior remains unchanged; refusal occurs
  before session copying; `--yes` is explicit and scoped only to the already-matched sessions.
- Contract drift: CLI documentation now states both non-TTY contracts. Post-repair delta: no new drift.

Commit: `e7ff63031` (`fix(cli): make interrupted session flows honest`).

## Group 2 — ACP stdio EOF

### Repair rationale

The EOF watcher won its `select` as soon as stdin closed and immediately dropped the ACP connection,
including responses already accepted but not yet serialized. After EOF, the connection now receives a
bounded one-second drain opportunity. A completed connection error remains an error; only expiry of the
bounded drain is treated as clean EOF.

### Verification and adversarial review

- `cargo test -p gosling input_eof_ --lib` — 3/3 passed: prompt termination of a permanently pending
  connection, completion of an in-flight response, and propagation of an in-flight connection error.
- Live initialize followed immediately by EOF — exit 0, one valid JSON-RPC line, response id 1, 5,237
  stdout bytes; stderr remained protocol-separated.
- Adversarial checks: the drain is bounded below the scenario's shutdown deadline, does not turn a
  completed transport error into success, and does not change request/response schema or framing.
- Contract drift: HS-03/AP-05 and ACP v1 framing remain unchanged. Post-repair delta: no new drift.

Commit: `e5436dfe6` (`fix(acp): drain queued responses on stdin EOF`).

## Group 3 — workspace-less Desktop artifact capability

### Repair rationale

`visibleSessionWorkspaceId` uses three states: `undefined` means no visible-session override and may
fall back to the active workspace, while `null` means the visible session explicitly has no workspace.
Nullish coalescing collapsed those states. The router now preserves them and can publish an
artifact-only routing config, with no workspace/output identity, for exact user-facing files from the
visible session. Electron still canonicalizes each file, requires it to exist, limits the list, and
grants no directory capability.

### Verification and adversarial review

- Focused Vitest: six files, 25/25 tests passed, including a regression where an active workspace
  exists but the visible CLI session has `workspaceId: null`.
- `pnpm run typecheck` — passed.
- `just package-ui` — passed, including release CLI build, schema generation, production Vite build,
  Electron packaging, and ad-hoc signing.
- Live pre-repair proof: the existing `/tmp/.../playtest-artifact.md` inventory item was visible but
  preview was denied outside approved roots.
- Live post-repair package startup was blocked before renderer creation by macOS Keychain
  `SecItemCopyMatching`; CDP accepted TCP but could not serve a page. No credential prompt was
  accepted or bypassed. Therefore this group is code-, type-, test-, and package-verified, but its
  post-fix GUI click is partially verified rather than rounded up to live-pass.
- Adversarial checks: workspace-backed download routing remains unchanged; workspace identity fields
  must be both present or both absent; exact-file validation remains canonical, existing-file-only,
  and per-window.
- Contract drift: this restores ADR-0013's accepted workspace-less exact-file capability without
  expanding it to code/config/tool-metadata files or directories. Post-repair delta: no new drift.

Commit: `0cb1ec2d4` (`fix(desktop): authorize workspace-less session artifacts`).

## Campaign closeout

### Final regression

- `cargo fmt` — passed.
- `cargo build` — passed.
- `cargo clippy --all-targets -- -D warnings` — passed.
- Desktop `pnpm test` — 112 files, 810/810 passed.
- Desktop `pnpm run typecheck` — passed.
- `just package-ui` — passed.
- `cargo test` — partially passed: the `gosling` unit suite passed 1,823/1,823 and subsequent ACP
  suites progressed until `acp_transport_auth_test`; 23/25 there passed. Two stale integration tests
  expect query-string token authentication to succeed, while current `auth.rs` deliberately rejects
  it and pins that security behavior in `query_string_token_is_no_longer_accepted`. A serial isolated
  rerun reproduced the same 401-vs-406 mismatch. It is unrelated to this campaign and remains open.

### Architecture and contract drift

Authoritative sources were `docs/architecture.md`, accepted ADR-0013, current CLI documentation, ACP
v1 framing, and cards CH-03/SE-01/HS-03/AC-02/DT-06/DT-07/AP-05. The pre-repair disposition was
evidenced drift from those declared behaviors. Repeated tests/diffs show no new drift: CLI automation
contracts are now documented; ACP schema/framing is unchanged; and Desktop file authority remains
canonical, existing-file-only, exact, bounded, and per-window. Drift delta: `no new drift`.

### Record closure

- `docs/cloud/2026-08-15-live-all-scenarios-playtest.md`: GSL-PLAY-2026-005 and 006 open → closed,
  with live evidence and commit pointers.
- `docs/cloud/2026-08-16-live-all-scenarios-playtest.md`: non-TTY removal open → closed.
- `docs/cloud/2026-08-15-master-report.md`: historical cluster retained; closure note added.
- `docs/TODO.md`: no selected finding had a native row, so none was deleted or fabricated.
- No in-code TODO/FIXME/HACK/XXX marker described any of the repaired defects.

### Outputs and residual risk

- Consolidated report: `docs/cloud/2026-08-18-live-all-scenarios-playtest.md`.
- Final scenario ledger: 31 Pass · 0 Fail · 79 Blocked after repair.
- DT-06/DT-07 remain Blocked pending one Keychain-unblocked packaged preview click.
- The two stale ACP query-token integration expectations remain open test hygiene.
- `session/mod.rs` and `cli.rs` exceed 2,000 lines and are routed to a dedicated modularization run;
  they were not split inside this repair.

Final status: `completed_with_partial_verification`.

## Follow-up — previewable-only Outputs inventory

### Task and finding

The operator supplied a live screenshot showing email-like `compatibility_inference` records in the
Outputs list, each leading only to “This file type does not have an in-app preview yet.” This was
recorded as `GSL-PLAY-2026-009` in the consolidated report. The confirmed Gate 1 failure was a list
state that promoted non-actionable metadata as a usable output; the backend metadata itself remained
valid provenance and was not deleted.

### Repair and changed files

- `artifactUtils.ts` now centralizes the artifact kind derived from path/MIME metadata and exposes the
  matching previewability predicate.
- `ArtifactPane.tsx` filters only the presented inventory and its count through that predicate.
- `ArtifactWorkbenchContext.tsx` uses the same classifier when opening a listed artifact, refuses new
  unsupported file tabs, and prunes persisted tabs that can no longer produce a preview.
- Focused component/unit tests cover four supported entries plus the observed email-like `.mil`
  inference, asserting `Outputs 4`, no unsupported row/message, and no preview read. Adjacent tests
  preserve PDF, generic-MIME extension fallback, parameterized JSON/PDF MIME, and supported persisted
  tabs while rejecting MIME-only binary/image records the file reader cannot decode.
- ADR-0013, `docs/architecture.md`, the workspace guide, and DT-06 now state the previewable-only
  presentation contract. Durable metadata, missing supported files, preview authorization, and file
  contents are unchanged.

### Validation and Gate 1 recheck

- Baseline Desktop suite: 112 files / 810 tests passed before the repair.
- Focused post-repair tests: 3 files / 17 tests passed.
- Full post-repair Desktop suite: 112 files / 812 tests passed.
- Desktop `pnpm run typecheck`: passed.
- ESLint on the six changed TypeScript/TSX files: passed with zero warnings.
- One full-suite attempt run alongside other validation produced three unrelated `userEvent`
  timeout/input-order failures in the workspace and extension dialogs (809/812). Both files passed
  isolated (18/18), and the subsequent standalone full suite passed 812/812.
- `git diff --check`: passed before final record updates and is repeated in closeout.
- Gate 1 changed-surface result: Pass. The list count and rows now represent actionable previews;
  empty/missing behavior for supported types is preserved. The Gate 6 persistence seam received a
  focused upgrade-state check: unsupported tabs are removed while supported selection is restored.
  Gates 2–5 and the remainder of Gate 6 were not re-audited because this follow-up changed no handoff,
  markup semantics, accessibility mechanics, responsive layout, backend permission, or deployment
  surface.

### Architecture and contract drift

Authoritative sources are accepted ADR-0013, `docs/architecture.md`, the workspace guide, and DT-06.
The pre-repair implementation conformed to the old declaration that unknown types remain visible, but
that declaration conflicted with the operator's explicit product decision. This run completed the
authorized amendment consistently across implementation, consumer, tests, guide, architecture text,
scenario, and consolidated report. Backend inventory schema/ACP contracts did not change. Post-repair
disposition: intentional authorized amendment complete; drift delta: `no new drift`.

### Record closure and limits

- `docs/cloud/2026-08-18-live-all-scenarios-playtest.md`: new operator finding → Closed, with focused
  and full-suite evidence.
- `docs/test_scenarios/14-desktop-ux-and-integration.md`: DT-06 stale expectation → amended regression
  requirement.
- No existing TODO/FIXME/HACK/XXX marker or `docs/TODO.md` row named this defect.
- The supplied screenshot is the before evidence. Automated DOM/state assertions are the after
  evidence; a newly packaged visual screenshot was not captured in this follow-up.

Follow-up status: `completed_with_automated_ui_verification`.
