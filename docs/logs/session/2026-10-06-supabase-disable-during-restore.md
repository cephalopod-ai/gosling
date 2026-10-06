# 2026-10-06 — Disable extensions during session restoration

## Scope and baseline

Target: `/Users/eric/Work/vscode/forked/gosling`, `main` at `98b3d5d2d`, initially clean.
The operator authorizes the fix following the screenshot investigation. Source findings
are `REL-GSL-001` and `WFG-GSL-002` in the October 6 Supabase investigation; these
report-local IDs are distinct from older findings with the same IDs in the backlog.
Use `SUP-DISABLE-001/002` for this repair's repo ledger entries.

Catalog workflow: `repair-defect-patchset`; relevant required shared contracts and
playtest intake read. Existing AGENTS/GEMINI/README/docs-index, architecture, advisory
metadata and recent auth/desktop evidence were inspected. Independent reads and checks
run concurrently; mutations and checks of changed state remain sequential. No delegation.
No live accounts, sessions or credentials will be changed. No installation or publishing.

Baseline validation:
- `cargo check --locked --offline -p gosling --lib --tests`, activated Hermit,
  `RUSTUP_AUTO_INSTALL=0`, CommandLineTools: passed (36.18 seconds).
- Desktop Vitest extension agent API and menu: 2 files / 3 tests passed.
- Rust build/test/Clippy remain outside AGENTS.md's explicit-request condition.
  Rust regression tests will be compiled; Desktop tests and structural scans executed.

## One locality stage

- SUP-DISABLE-001: reliability/backend, P1, high complexity. Remove a saved extension
  while runtime restoration waits for OAuth; persist its desired state, cancel startup,
  and ensure a completed initializer cannot re-enable it.
- SUP-DISABLE-002: frontend/error correctness, P3, low complexity. Preserve active-turn
  fencing but return extension-specific busy guidance.

Shared surfaces: ACP extension removal, initializing AgentManager agent custody,
Agent extension loading/cancellation, enabled-extension persistence, OAuth callback
task lifetime, and regression tests. One stage avoids changing the same load/remove
boundary twice. Public ACP request/response shapes and persisted formats remain unchanged.

Baseline paths: idle loaded removal succeeds and persists; absent sessions fail;
internal planning tools cannot be removed; active prompts reject mutation; extension
restore loads configured/saved choices; successful OAuth flow releases its callback
server after receiving the callback. Tests must preserve these paths.

Architecture sources: accepted `docs/architecture.md` core-owned session persistence,
typed ACP requests in `gosling-sdk-types`, the existing `enabled_extensions.v0` format,
and `documentation/docs/getting-started/using-extensions.md` current-chat versus
future-chat default distinction. EX-03/04 in `docs/test_scenarios/05-extensions-and-mcp.md`
require recoverable broken startup and durable removal. Loading-time missing-session
feedback is pre-existing drift. No new schema, provider, credential, or authority policy.

Implementation plan: write desired removal atomically without constructing an agent;
find any already initializing/cached agent and cancel/remove that extension; retain
cancelled per-agent startup tokens through the readiness handoff; close OAuth callback
tasks when their future is dropped. Keep other saved extensions and host metadata.

## Implementation and stage review

- ACP removal keeps the ordinary ready-agent path and active-turn fence. An unregistered
  saved chat removes only the target from durable enabled-extension state in an existing
  `BEGIN IMMEDIATE` transaction, preserving other pending extensions, config keys and host
  metadata. Missing/closed sessions and internal planning tools retain their refusal paths.
- AgentManager exposes weak references to initializing agents without making them usable
  for prompts; a drop guard removes those entries on success, failure or cancellation.
  Lookup examines initializing custody before the LRU cache to avoid a readiness-handoff gap.
- Per-extension cancellation and completion locks stop the targeted initializer before
  live removal. The stage review found that the shared MCP lifecycle lock spans login;
  fenced restoration cleanup therefore removes only this extension without waiting for
  another initializer. Ordinary ready-agent removal retains the existing lifecycle lock.
- Both OAuth callback-server paths now abort their listener task when the flow is dropped.
  Normal callback receipt, timeout and error paths retain their original results.
- Existing Desktop presentation waits for confirmed success and displays backend causes.
  Added regressions verify these behaviors with the new busy-chat message.
  Extension-specific mapping also handles the active-run fence's error payload, including
  the interval before a competing prompt acquires its local turn claim; provider-switch
  checkpoint guidance remains specific to actual provider switches.

Three ACP regression cases cover two delayed extension startups and late server responses,
saved-chat removal without activation, metadata/other-extension preservation, reopen,
missing-session and planning refusals, and active-turn refusal followed by idle removal.
A callback-listener regression checks port release on cancellation. These tests use
disposable stores, local fake HTTP/MCP/model endpoints, and a masked test API key. They
have been compiled, not executed. Fixture review corrected a local-spawn call to match
the fixture's ordinary Tokio runtime; the fake MCP server has a valid handshake/tool list.
An additional unit regression takes the real operation gate's active-turn refusal and
checks extension guidance while the gate still holds the active run.

The first compile attempt rejected `tokio_util::task` because the existing dependency lacks
its `rt` feature. Reused the repository's join-handle drop-guard pattern instead, with no
dependency change. A later private-interface warning was resolved by keeping the new
Agent field private. These intermediate results are not counted as final validation.

## Final re-audit and completeness pass

The scoped host-orchestration review walked Desktop toggle -> typed ACP -> local-turn
claim -> saved-state transaction -> initializing/cached agent -> cancellation gate ->
MCP cleanup -> registration/reopen. The distinct completeness pass checked both finding
IDs, protected planning behavior, config-key retention, failure-before-success ordering,
callback task lifetime, concurrent prompt exclusion and dependency/config changes.
No public ACP schema, persisted format, credential scope or global-default change was added.
Architecture comparison: no new drift against the sources above; the observed loading
control mismatch is repaired in source. Runtime concurrency behavior remains unverified.

## Validation and record closure

- Core/test-target compilation: final `cargo check --locked --offline -p gosling --lib --tests`
  under the baseline environment: **passed without warnings** (final run: 25.57 seconds).
- Desktop: four focused Vitest files, **13 tests passed**. Includes two new removal
  success/refusal checks alongside existing menu, error-message and load-failure regressions.
- Desktop typecheck and targeted ESLint passed. Prettier passed for the changed test.
- Rust formatting and `git diff --check` passed. Three added local documentation links,
  both governance markers, and unchanged ACP schema/stored-format/default/dependency sources
  passed the targeted structural scan.
- No Rust test execution, build, Clippy, real Supabase/OAuth call, native app replay,
  package/install, commit or publication. AGENTS.md requires an explicit build/test request.

`docs/TODO.md` and its active mirror record both IDs as source-fixed with verification
pending; the test ledger and docs index link this evidence. The extension manual describes
pending-login removal and busy-chat guidance. The original investigation remains historical;
its source IDs are mapped above rather than colliding with older backlog IDs.
The original temporary investigation report received a dated repair addendum pointing
to this record; its historical observations were retained.

Status: **completed_with_partial_verification** — source repair and records complete;
Rust runtime and installed-app verification remain pending.
Remaining: execute the compiled Rust regressions, rebuild the Desktop bundle, and replay
the original pending-Supabase-login interaction. The upstream token-refresh parsing failure
is separate and is not claimed resolved. Resume by checking this log and the scoped diff;
do not repeat implementation solely because runtime/deployment verification is pending.
