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

