# 2026-08-12 six-lens audit and defect repair

- Task: run a full private-catalog audit sweep and repair confirmed findings with
  `repair-defect-campaign`.
- Audit lenses: agent orchestration, reliability, data integrity, data concurrency,
  security, and negative space.
- Baseline: `11b5dc411f862fc69f7f35439c90a172dceeaab5`.
- Branch: `codex/full-audit-repair-20260812`.
- Inventory: five findings frozen; four repaired, one architectural AWS provider residual
  deferred with explicit reasoning.
- Repair commits:
  - `c78b67042` — close undispatched tool requests after an interrupted turn.
  - `b0dd032f7` — scope that closure to explicit CLI cancellation after adversarial review.
  - `94ce6be70` — bound diagnostics I/O and protect report files.
  - `2bc4dc27a` — serialize and atomically replace project tracker state.
  - `378add252` — coordinate memory JSONL readers and writers.
- Files changed: session recovery and diagnostics, CLI diagnostics and project tracking,
  shared-memory reader/writer, `fs2` dependency metadata, audit report, TODO, and this log.
- Targeted validation: scoped sibling cancellation and idempotency; generic-recovery live-approval
  negative regression; CLI persistence ordering; 7 diagnostics tests; diagnostics Unix permission
  regression; 8-writer project tracker regression and permission test; forced shared/exclusive
  memory-lock regressions. All passed.
- Baseline validation: Desktop Hermit typecheck and 555 tests passed. The initial workspace
  `cargo test` stopped at two CLI integration cases whose test binary could not locate the
  Gosling executable; this pre-patch observation was retained as baseline evidence.
- Final validation: `cargo build`, `cargo fmt --all -- --check`, full-workspace `cargo test`, and
  `cargo clippy --all-targets -- -D warnings` passed. Desktop Hermit typecheck and 82 files / 555
  tests passed. Documentation markers and campaign diff checks passed; `GEMINI.md` is absent.
- Residual: `providers/aws_env.rs` documents and performs process-global AWS environment
  mutation on a multithreaded runtime. Correct repair requires provider-instance AWS SDK
  configuration across Bedrock and SageMaker and was not narrowed into this campaign.
- Operator work preserved: an unrelated root `Cargo.toml` profile edit appeared during the
  run and was excluded from every campaign commit and campaign diff.
- Report: [`reports/2026-08-12-six-lens-agent-skills-audit-repair.md`](../../../reports/2026-08-12-six-lens-agent-skills-audit-repair.md).

## Gosling shell foundation follow-up

- Task: implement the shared shell foundation without creating a DAWES, Project ABC,
  math_mcp, or Physics/CST shell.
- Branch/worktree: `codex/shell-foundation` in a dedicated clean worktree, preserving
  unrelated Desktop work in the original worktree.
- Authority decision: shell policy defaults to permissive `inherit`; optional restricted
  policy is server-enforced before custom ACP method dispatch.
- Implemented: server-fixed shell identity, read-only provisioning, shared-config and
  namespaced data/state paths, workspace/credential/provider/model/extension/tool/skill
  profile application, domain adapter contract, explicit handoff envelope, minimal
  Electron host composition, shell UI primitives, package identity composition, and
  generated ACP schema and TypeScript SDK.
- Validation: Rust formatting, targeted checks/tests, and scoped clippy with warnings
  denied passed; generated schema and SDK completed. `goslingServe.test.ts` passed 15/15
  after its subprocess polling bound was stabilized for loaded hosts. Full SDK/Desktop
  typecheck remained unavailable in the isolated worktree because the shared install was
  stale/incomplete (`@agentclientprotocol/sdk` and MCP Apps entrypoints missing); the prior
  install attempt was blocked by pnpm's exotic-subdependency policy.
- Follow-up steps 1-4: rebased onto `origin/main` after the foundation merges landed;
  restored a deterministic pnpm 11 install by moving pnpm 11 settings into
  `pnpm-workspace.yaml` and declaring direct Desktop dependencies already imported by the
  source; added structured shell provisioning validation through CLI and ACP; and added a
  spawned-process authenticated ACP test covering runtime identity, provisioning,
  extension/tool/skill selection, policy denial, namespace isolation, and restart
  persistence.
- Final follow-up validation: `cargo fmt --all -- --check`, full-workspace
  `cargo test --workspace`, and full-workspace `cargo clippy --workspace --all-targets --
  -D warnings` passed. After the final review corrections, the two provisioning CLI tests
  plus the dynamic-model regression (3/3), spawned-runtime test (1/1), targeted compile,
  scoped Clippy, and `git diff --check` passed again. The pnpm 11 frozen install,
  generated SDK build/typechecks and 6 tests, Desktop typecheck, and 88 files / 582
  Desktop tests passed. ACP schema and TypeScript SDK regeneration also passed.
- Final review corrections: dynamic provider models are no longer rejected from static
  metadata during preflight (session startup remains authoritative); extension resolution
  now matches runtime replacement and plugin discovery; validation uses the server's
  captured default working folder; credential-catalog read failures remain visible; the
  ACP HTTP client feature is scoped to CLI tests; and namespace isolation is asserted by
  querying empty session stores rather than relying on absent database files.
- Deferred: actual domain adapters and shells, shell-specific icons/updater feeds, built
  package artifacts, and an end-to-end Electron renderer-to-backend shell smoke test.
