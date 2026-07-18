# Defect-repair campaign session log — 2026-07-17

Agent/model: Codex / GPT-5 family. Repository: `cephalopod-ai/gosling`
(local `origin` still uses the redirecting `repo-makeover/gosling` URL).

## Gate 0 — orientation, safety, and git posture

- Starting repair baseline: branch
  `claude/gosling-defect-repair-followup-6093` at `cdf2634ae`.
- Audit checkpoint: committed and remote-verified at
  `cdf2634ae97979c3f8fda438b89004a038e924f8` before repair skill load.
- Worktree: clean; no unrelated changes.
- Git/remote: available; audit push explicitly authorized and complete. Repair
  stages use local commits; no further push is assumed.
- Repo instructions: `AGENTS.md` read. Rust changes require `cargo fmt`. Cargo
  build, test, and Clippy commands are reserved for an explicit user build/test
  request. UI typecheck/test commands exist but are not baseline evidence.
- Baseline validation: unknown/not run by policy. Static source audit and
  `git diff --check` are available substitutes; regression tests will be added
  but recorded as unexecuted unless authorization changes.
- Documentation convention: dated reports and campaign logs under `reports/`,
  durable backlog/status summary in `docs/TODO.md`.
- Protected deferred work: Tagteam workflow activation, earlier audit ledger
  deferrals not present in the frozen inventory, ORCH-002, quick-xml advisories.
- Repo file-size rule: none beyond the loaded repair skill. The skill's
  <=1000 / 1001-1999 / >=2000 modularization rule governs this campaign.

## Gate 1 — inventory

- Finding source: exhaustive static audit checkpoint at
  `reports/2026-07-17-exhaustive-defect-audit-checkpoint.md` using multi-agent
  consensus plus architecture, dataflow, reliability, security, LLM, MCP,
  contract, lifecycle, Node/Electron, GUI, and external-API lenses.
- Counts: 34 in-scope defects; 16 security/P0-or-equivalent high-impact entries,
  17 P1 correctness/reliability/data-integrity entries, 1 P2 frontend defect.
  Complexity: 12 high/medium-high, 12 medium, 10 low.
- Candidate disposition and complete touch sets are recorded in
  `reports/2026-07-17-defect-campaign-plan.md`.
- AUD-031 is reopened by the user's all-findings instruction but retains a
  mandatory stop checkpoint if it requires a broad architectural rewrite.

## Gate 2 — locality grouping

- Thirteen ordered groups cover every in-scope finding exactly once.
- Same-file affinity joins SmartApprove, import, provider outcome, MCP
  pagination, and state-transition defects; same-data-path affinity joins
  lifecycle and desktop-boundary defects.
- Planned in-band modularizations: developer shell process-group seam,
  Codex event-result interpretation seam, and review worker-launch seam.
  Claude/Codex/route files will trigger additional extraction only if their
  planned local edits expand into multiple substantial functions.
- Files >=2000 lines routed for dedicated modularization: `session_manager.rs`,
  `acp/server.rs`, `agent.rs`, `extension_manager.rs`, `summon.rs`, `main.ts`.
- Commit boundary: one local commit per verified group, with hashes appended
  below. Remote synchronization is not repeated without authorization.

## Stage results

### Stage 1 — credential persistence and OAuth input

- Defects: AUD-011 and AUD-033 fixed.
- Changes: ChatGPT Codex, GitHub Copilot, and Kimi token caches now use the
  repository's owner-only atomic secret writer. Databricks workspace/OIDC URL
  parsing now returns contextual errors instead of panicking.
- Regression guardrails added: Unix mode assertions for each cache and an
  invalid Databricks host test.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: `git diff --check` passed. Regression tests were not
  executed under the repository's build/test authorization rule.
- Adversarial review: verified parent-directory creation remains supplied by
  the shared helper, existing-file replacement receives 0600 through atomic
  rename, invalid input performs no request, and cache clear/load contracts are
  unchanged. No additional finding.
- Change review: scoped to four provider files plus this log; no unrelated
  formatting or protected deferred work.
- Commit: `ae9264458`.

### Stage 2 — SmartApprove authority

- Defects: AUD-003 and AUD-004 fixed.
- Changes: MCP `readOnlyHint=true` is advisory and can no longer populate
  `AlwaysAllow`; `readOnlyHint=false` still tightens policy. LLM read-only
  results authorize only the concrete call and are no longer persisted by
  tool name. Negative decisions remain safely cached as `AskBefore`, and old
  positive cache entries are reclassified instead of trusted.
- Regression guardrails: annotation tests now query the actual tool name and
  cover an innocuously named hostile tool; legacy positive, negative, and deny
  cache behavior is explicit. Permission-judge coverage continues to assert
  that concrete arguments enter classification.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: `git diff --check` passed. Tests were not executed under
  the repository authorization rule.
- Adversarial review: verified explicit user `AlwaysAllow` remains authoritative,
  extension management remains approval-gated, legacy smart-positive entries
  cannot bypass classification, and classifier failure tightens to approval.
- Change review: three permission files plus this log; obsolete name heuristics
  and misleading cache comments removed.
- Commit: `dac101bcf`.

### Stage 3 — session import trust boundary

- Defects: AUD-001 and AUD-002 fixed.
- Changes: native import removes the versioned enabled-extension state before
  persistence, preventing imported Stdio/InlinePython/builtin/platform configs
  from becoming automatic resume-time authority. The working-directory
  restriction flag is now applied explicitly during import.
- Regression guardrail: the export/import round-trip fixture includes a Stdio
  payload and safe Todo state; it asserts executable state is quarantined,
  safe state survives, and the restriction remains enabled.
- Modularization: `session_manager.rs` is >=2000 lines, so the patch is local
  and the file remains routed for dedicated modularization.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: `git diff --check` passed; tests not executed by policy.
- Adversarial review: verified all executable-capable imported variants share
  the removed versioned key, non-executable extension state is preserved, and
  trusted local config remains the fallback after quarantine.
- Change review: two session files plus this log, no schema or unrelated import
  behavior changed.
- Commit: `a52862231`.

### Stage 4 — extension secret and process-environment boundaries

- Defects: AUD-009 and AUD-010 fixed.
- Changes: configured environment values are rehydrated only when a client
  echoes the exact stored Stdio command/arguments or HTTP URI/headers/socket,
  preventing same-name endpoint substitution. Inline Python now receives the
  same allowlisted child environment as Stdio extensions instead of inheriting
  the server's full process environment.
- Regression guardrails: Stdio and HTTP endpoint-redirection cases cover each
  identity field, and a Unix subprocess test verifies an inherited secret is
  removed by the shared minimal-environment policy.
- Modularization: `acp/server.rs` and `extension_manager.rs` are >=2000 lines,
  so both patches remain local and both files stay routed for dedicated
  modularization.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: `git diff --check` passed; tests not executed by policy.
- Adversarial review: verified client-supplied values still win on an exact
  destination, mismatched transport variants cannot cross-match, configured
  HTTP headers are part of destination identity, and the Inline Python launch
  adds no environment after applying the allowlist.
- Change review: two implementation files plus this log; extension transport
  and lifecycle behavior are otherwise unchanged.
- Commit: `f69a00d87`.

### Stage 5 — desktop content, filesystem, and transport boundaries

- Defects: AUD-012, AUD-013, and AUD-032 fixed.
- Changes: untrusted Markdown images render as inert alt-text placeholders and
  the renderer CSP no longer permits arbitrary HTTPS image loads. Renderer IPC
  file operations canonicalize existing paths and the nearest existing parent
  for prospective paths, reject symlink escapes and dangling symlink ancestors,
  and store artifact-picker grants canonically. The ACP WebSocket receive path
  now has per-message, aggregate-character, and message-count bounds, respects
  ReadableStream backpressure, and closes oversized peers with code 1009.
- Regression guardrails: Markdown rendering and CSP tests cover remote-image
  suppression; path tests cover legitimate existing/missing paths, `..`-prefixed
  names, existing/missing symlink escapes, and dangling links; WebSocket tests
  cover queue and single-message overflow.
- Modularization: the focused canonical-path helper is isolated for testing;
  `main.ts` remains >=2000 lines and routed for dedicated modularization rather
  than undergoing a broad split in this campaign.
- Formatting: `source bin/activate-hermit && cargo fmt` and targeted Prettier
  formatting passed.
- Static verification: `pnpm exec tsc --noEmit` and `git diff --check` passed.
  The package-script wrapper could not run because the environment has pnpm
  10.6.4 while the manifest requires >=10.30.0; the same local compiler was
  invoked directly through `pnpm exec`.
- Tests: four targeted Vitest files passed, 51 tests total.
- Adversarial review: verified canonical paths, not attacker-controlled lexical
  aliases, reach filesystem APIs; dangling links fail closed; stale invalid
  approved roots cannot deny unrelated valid roots; the receive buffer is
  bounded by both item count and encoded size; and only one parsed message can
  enter the ReadableStream queue per pull.
- Change review: desktop boundary files, focused test helpers, and this log;
  generated API code and unrelated renderer behavior are untouched.
- Commit: `1f9867b66`.

### Stage 6 — provider execution and working-directory policy

- Defects: AUD-005, AUD-006, AUD-007, AUD-008, and AUD-022 fixed.
- Changes: working-directory findings remain approval-required in Auto mode;
  path checks canonicalize existing targets and the nearest existing ancestor
  for prospective targets before applying component-aware containment. The
  provider contract now identifies backends that execute tools outside Gosling,
  and restricted sessions reject those backends before compaction or inference.
  Gemini maps supported modes to explicit approval modes and rejects Chat;
  Cursor and Tagteam accept only their safely representable Auto mode and reject
  every unsupported mode. Gemini, Cursor, Claude Code, Codex, and Tagteam now
  retain the session working directory and apply it to every task command.
- Regression guardrails: traversal, existing/missing symlink, dangling-link,
  command-cwd, provider-mode matrix, and unsupported-mode tests were added.
- Modularization: `agent.rs` remains >=2000 lines and routed. Claude Code and
  Codex remain in the 1001-1999 band; edits stayed local, with Codex and Cursor
  command-construction seams extracted to keep policy testable.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: contract/override/factory/current-dir coverage was
  enumerated with `rg`; `git diff --check` passed. Rust tests/build/Clippy were
  not executed under the repository's explicit authorization rule.
- Adversarial review: verified mode changes are applied per resumed Gemini
  invocation; unsupported headless approval modes fail before provider use;
  provider configuration cannot bypass the restricted-session preflight;
  additional working directories are canonicalized independently; stale invalid
  roots cannot suppress a valid root; and absolute shell tokens use the same
  canonical boundary as path arguments.
- Change review: the provider capability trait, five command providers, the
  working-directory inspector, the localized reply preflight, and this log;
  ACP-backed provider mode negotiation and generated UI API code are untouched.
- Commit: `ea69344aa`.

### Stage 7 — cancellation ownership and process cleanup

- Defects: AUD-014, AUD-015, and AUD-016 fixed.
- Changes: canceling a server request no longer releases the per-session slot
  until its task guard actually unwinds. ACP and subagent stream consumers now
  select directly on cancellation instead of waiting indefinitely for another
  event. Developer shell calls propagate the tool cancellation token, and a
  dedicated process-tree guard terminates the isolated process group on normal
  return, timeout, explicit cancellation, or dropped-future cleanup.
- Regression guardrails: request registration covers the canceled-but-live
  interval; optional cancellation waiting has a wakeup test; the extracted Unix
  process-tree guard test covers a background descendant.
- Modularization: process-tree ownership moved out of the 1001-1999-line shell
  implementation into a focused guard module. The localized ACP loop remains in
  `acp/server.rs`, which is still routed for dedicated modularization.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: `git diff --check` passed. Rust tests/build/Clippy were
  not executed under the repository's explicit authorization rule.
- Adversarial review: verified a canceled request cannot overlap its successor;
  cancellation wins a ready-stream race; a missing optional subagent token does
  not busy-loop; the process guard is created synchronously after spawn and
  therefore runs if the tool future is dropped; explicit timeout/cancel cleanup
  waits for the root child after terminating descendants.
- Change review: request ownership, two stream loops, the optional-token helper,
  developer shell cancellation plumbing, the extracted guard, and this log;
  provider and session persistence behavior are untouched.
- Commit: `1dc1880e1`.

### Stage 8 — server, extension-loader, and desktop lifecycle supervision

- Defects: AUD-017, AUD-018, and AUD-019 fixed.
- Changes: one application shutdown token now fans out to the HTTP/TLS server,
  SSE producers, and extension-loader registry; TLS graceful shutdown has a
  ten-second deadline, and blocked SSE sends wake on shutdown. Extension-loader
  registration is atomic, concurrent waiters share one result, and replacement,
  removal, or application shutdown aborts the owned task. Desktop cleanup no
  longer treats `ChildProcess.killed` as an exit; it escalates after the grace
  period, waits for `close` up to a hard deadline, and retains the registry
  record unless actual closure was observed.
- Regression guardrails: extension-loader tests cover atomic registration and
  replacement/removal aborts; an SSE backpressure test covers shutdown wakeup;
  the desktop integration test uses a child that ignores `SIGTERM` and asserts
  forced exit precedes registry removal.
- Modularization: route coordination remains one atomic state call in the
  1001-1999-line `routes/agent.rs`; ownership and sharing live in `state.rs`.
- Formatting: `source bin/activate-hermit && cargo fmt` and targeted Prettier
  formatting passed.
- Static verification: `pnpm exec tsc --noEmit` and `git diff --check` passed.
  Rust tests/build/Clippy were not executed under the repository's explicit
  authorization rule.
- Tests: `pnpm exec vitest run src/goslingServe.test.ts` passed, 14 tests.
- Adversarial review: verified signal observation is centralized; pre-cancelled
  tokens still stop newly starting servers/streams; channel backpressure cannot
  hide shutdown; loader replacement/removal can abort without waiting behind a
  task-held mutex; two resume calls cannot both spawn; cleanup is idempotent;
  kill-request state is never used as liveness; and a deadline failure preserves
  the backend registry record for later recovery.
- Change review: server command/state/SSE lifecycle, the single resume call
  site, desktop child cleanup and its focused test, plus this log; routing and
  session persistence are otherwise untouched.
- Commit: `e741c3527`.

### Stage 9 — bounded, complete MCP discovery

- Defects: AUD-020 and AUD-021 fixed.
- Changes: tool, resource, UI-resource, and prompt discovery now consumes every
  MCP page. A shared pagination guard rejects repeated cursors, more than 100
  pages, more than 10,000 returned items, and arithmetic overflow before
  aggregate vectors can grow without bound.
- Regression guardrails: a two-page mock covers complete tool/resource/prompt
  collection; a repeating server cursor is rejected; page and item ceilings
  have direct boundary tests.
- Modularization: `extension_manager.rs` remains >=2000 lines and routed for
  dedicated modularization; this change adds one small shared guard and three
  category-specific collection seams without broad file movement.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: every extension-manager discovery call site was
  enumerated against the paginated collectors; `git diff --check` passed. Rust
  tests/build/Clippy were not executed under the repository's explicit
  authorization rule.
- Adversarial review: verified unavailable tools still count toward the remote
  item budget; empty cursor pages cannot loop beyond the page cap; a cursor is
  recorded before another request can use it; cancellation is propagated on
  every page; pagination failures preserve the existing per-extension error
  behavior; and UI resources from later pages are filtered only after complete
  bounded collection.
- Change review: one extension-manager implementation/test file plus this log;
  MCP wire request construction already forwarded cursors and is unchanged.
- Commit: `4a24b7f8f`.

### Stage 10 — CLI provider terminal outcomes

- Defects: AUD-023, AUD-024, and AUD-025 fixed.
- Changes: Cursor now drains stdout and a bounded stderr tail concurrently,
  preventing pipe saturation while retaining failure diagnostics. Its parser
  rejects malformed JSON, explicit error events/results, empty or missing
  results, and duplicate terminal results. Codex now fails on every nonzero
  process exit even when stdout contains partial text, and an extracted outcome
  module makes `error` and `*.failed` events authoritative over accumulated
  agent messages.
- Regression guardrails: Cursor parser cases cover explicit, malformed, empty,
  and non-terminal output; a Unix fake CLI floods stderr beyond pipe capacity;
  Codex covers partial text followed by `turn.failed` and partial stdout followed
  by exit 7 with stderr.
- Modularization: Cursor remains <=1000 lines. Codex remains in the 1001-1999
  band, with terminal event/process interpretation extracted to
  `providers/codex/output.rs` before the runner and parser were changed.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: obsolete partial-success conditions and Cursor fallback
  paths were searched and are absent; `git diff --check` passed. Rust
  tests/build/Clippy were not executed under the repository's explicit
  authorization rule.
- Adversarial review: verified both Cursor pipes begin draining before waiting;
  stderr retention is tail-bounded; late Cursor errors override an earlier
  result; a successful exit cannot legitimize malformed/empty output; Codex
  checks process status before returning lines; terminal event classification
  runs before response accumulation; missing error messages still yield a
  terminal failure; and context/rate-limit error mapping is preserved.
- Change review: the two CLI providers, one focused Codex outcome module, their
  local tests, and this log; permission flags and command working directories
  are unchanged.
- Commit: `92dab2abf`.

### Stage 11 — failure-atomic live state transitions

- Defects: AUD-026 and AUD-027 fixed. AUD-031 reached its mandatory campaign
  stop checkpoint and remains residual; see below.
- Changes: provider/mode transitions are serialized. A replacement provider is
  configured while detached, persisted, and only then installed live. Mode
  persistence occurs before live mutation, with provider and session rollback
  on rejection. ACP primary-working-directory changes now compensate provider,
  session, and every extension root when downstream refresh fails; extension
  root fan-out reports aggregate failures instead of silently warning.
- Regression guardrails: injected SQLite update failures assert the previous
  live provider and mode remain installed. Root propagation errors are now
  observable at load, refresh, and primary-directory transition boundaries.
- Modularization: `agent.rs`, `acp/server.rs`, and `extension_manager.rs` remain
  >=2000-line routed targets. The ACP transition is isolated in the existing
  small `manage_sessions.rs` submodule; no broad routed-file split was mixed in.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: every extension-root update call now consumes its
  result; transition locks cover provider and mode writers; `git diff --check`
  passed. Rust tests/build/Clippy were not executed under the repository's
  explicit authorization rule.
- Adversarial review: verified persistence failure cannot swap the provider or
  call the live provider's mode transition; detached-provider setup failure is
  inert; concurrent provider/mode transitions cannot interleave; provider
  rejection attempts both provider and durable rollback; cwd rollback attempts
  all three state surfaces even after one compensation fails; and partial root
  fan-out is followed by an old-root fan-out during rollback.
- AUD-031 stop checkpoint: at-most-once external side effects across process
  death require a durable operation identity/ledger, an in-doubt recovery state,
  request replay semantics, and idempotency-key support where external tools can
  honor it. Persisting a tool-request message before dispatch is insufficient:
  a crash between dispatch and response durability cannot distinguish “not
  executed” from “executed,” and conversation repair currently removes orphaned
  requests, allowing a new model call to issue a fresh ID. Implementing the
  required protocol/schema/recovery contract is a broad architectural rewrite,
  so the campaign's explicit stop rule forbids an approximate reorder patch.
- Change review: transition ordering/serialization, ACP cwd compensation,
  extension root error propagation, focused tests, and this log; tool dispatch
  was deliberately left unchanged at the mandatory stop checkpoint.
- Commit: `0ea3eac71`.

### Stage 12 — review and delegation capacity

- Defects: AUD-028, AUD-029, and AUD-030 fixed.
- Changes: review subprocess launch and permit ownership now live in one
  repository-rooted worker pool shared by the main and check phases. Async
  delegation atomically reserves a semaphore slot before any setup, retains it
  for the task's full running lifetime, and cannot be cancelled between child
  spawn and task registration.
- Regression guardrails: the extracted worker command asserts its child cwd;
  an eight-worker, two-phase-style concurrency test observes one shared peak;
  and barrier-synchronized delegation reservations with a limit of one admit
  exactly one contender.
- Modularization: worker launch and capacity coordination moved out of the
  1001-1999-line review orchestrator into `review/worker.rs` before behavior
  changed. The localized summon ownership change remains in the >=2000-line
  routed file.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: both concurrent review futures receive the same worker
  pool, every subprocess command is rooted at the resolved repository, and
  every constructed background task owns a slot; `git diff --check` passed.
  Rust tests/build/Clippy were not executed under the repository's explicit
  authorization rule.
- Adversarial review: verified the shared permit surrounds process creation,
  stdin delivery, and exit collection; legacy checks-only fan-out uses the same
  root and limit; a failed delegate setup releases its reservation; cleanup,
  cancellation, synchronous load, and timed reinsertion retain or release the
  slot with task ownership; and no await point remains between subagent spawn
  and task-map insertion.
- Change review: review handler/orchestrator/extracted worker, the summon slot
  lifecycle, focused tests, and this log; review parsing and sync delegation
  behavior are unchanged.
- Commit: `03895803e`.

### Stage 13 — fixed-height Ink text budgets

- Defect: AUD-034 fixed.
- Changes: every affected fixed-height screen now flattens and conservatively
  pre-truncates dynamic text to an explicit cell budget and uses Ink truncation
  instead of wrapping. The provider configurator counts all margins and rows,
  bounds visible setup steps to remaining height, and truncates descriptions,
  key labels, help, and setup strings. Error screens use the parent width and
  render title, error, and retry text as fixed one-line rows.
- Regression guardrails: focused utility tests cover ASCII limits, multi-line
  flattening, non-ASCII conservative budgeting, and zero/one-cell edges. A
  source scan confirms no executable `wrap="wrap"` remains under `ui/text/src`.
- Modularization: all touched files remain <=1000 lines; the shared truncation
  seam lives in the existing small `utils.tsx` module.
- Formatting: targeted Prettier formatting passed.
- Tests: `tsx --test text/src/utils.test.ts` passed, 3 tests.
- Static verification: `git diff --check` passed. Text-UI TypeScript checking
  reached two pre-existing `extensions.tsx` request-shape errors where
  `{config}` is supplied to an SDK request requiring `{extension}`; this newly
  surfaced post-freeze defect is isolated for the next local repair stage.
- Adversarial review: verified newlines/tabs cannot create extra rows; the
  conservative non-ASCII policy cannot underestimate cell use; every former
  wrapped value has a matching content-width budget; configurator height math
  includes header margins, per-key margins, input/help spacing, setup header,
  and per-step margins; and omitted setup rows are represented by a bounded
  summary when space permits.
- Change review: the three audited screens, one shared helper and focused test,
  plus the single ErrorScreen call-site width propagation and this log; input
  handling and provider persistence are unchanged.
- Commit: `cb0d45640`.

### Stage 14 — post-freeze extension request schema

- Defect: POST-001 fixed. This was discovered by Stage 13 verification after
  the synchronized audit inventory was frozen.
- Changes: both text-UI session-extension add paths now use the generated
  request's `extension` field and convert local entries to the SDK's
  `GoslingExtension` wire type. The obsolete `{config: value as any}` shape is
  gone.
- Regression guardrail: the text-UI TypeScript compiler checks both request
  literals directly against the generated SDK client signature.
- Formatting: Stage 13's targeted Prettier output remains valid; the two-line
  schema repair required no additional mechanical reformatting.
- Static verification: `tsc --noEmit -p text/tsconfig.json` passed;
  `tsx --test text/src/utils.test.ts` passed, 3 tests; and `git diff --check`
  passed.
- Adversarial review: verified enable-existing and create-then-enable both use
  the same typed conversion; no `as any` remains at either request boundary;
  config persistence still precedes session activation for a new extension;
  and the server request type and generated TypeScript schema both name the
  field `extension`.
- Change review: two request literals and this log only; extension conversion,
  persistence ordering, and UI state transitions are unchanged.
- Commit: `7f6203b57`.

## Campaign closeout

- Final inventory: 33 of 34 frozen findings fixed. AUD-031 is the sole frozen
  residual at its mandatory architectural stop checkpoint. POST-001, discovered
  after freeze by verification, is also fixed.
- Final regression walkthrough: `source bin/activate-hermit && cargo fmt
  --check` and `git diff --check cdf2634ae..HEAD` passed. Desktop
  `tsc --noEmit` passed; five targeted Vitest files passed, 65 tests. Text UI
  `tsc --noEmit -p text/tsconfig.json` passed and its focused Node/TSX suite
  passed, 3 tests. Rust build/test/Clippy were not run under the repository's
  explicit authorization rule.
- Final adversarial walkthrough: all 34 frozen IDs map exactly once to a group;
  old review double-semaphore and summon check-then-insert patterns are absent;
  no executable text-UI `wrap="wrap"` remains; no post-freeze request-boundary
  `as any` remains; no repair-stage TODO/FIXME/XXX/HACK marker was introduced;
  required formatting and whitespace checks pass; and the worktree contains
  only these closeout documents.
- Documentation refresh: the execution ledger was appended to the plan, this
  session log carries per-stage evidence and hashes, and `docs/TODO.md` records
  the architectural residual, routed modularization work, and withheld Rust
  verification.
- Residual risk: AUD-031 cannot honestly be closed without a durable
  operation/recovery protocol and cooperation from side-effecting tools. An
  approximate persistence reorder would still allow duplicate or ambiguous
  external effects after process death.
- Git posture: the audit checkpoint remains synchronized at `cdf2634ae`.
  Repair and closeout commits remain local and are not pushed without further
  authorization.
- Final status: `completed_with_one_architectural_residual`.
