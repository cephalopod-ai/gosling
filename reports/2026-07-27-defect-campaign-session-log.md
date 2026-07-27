# Repair Defect Campaign Session Log - 2026-07-27 (Round 1)

Status: `completed_with_partial_verification`

Mode: Existing Findings Mode. Source: six fresh audit reports (security-code,
reliability, architecture-seam, dataflow-concurrency, mcp-server,
agent-orchestration-code) produced in a prior round on this branch. Findings
were re-verified against current source before grouping, not taken on faith.

This is round 1 of a two-round audit -> repair cycle. Priority was the 2
Critical + 6 High findings; Medium/Low findings were folded in only where
they shared a file/function already being edited for a P0/P1 fix
(locality/cost-amortization rule). See the closeout report for the full
per-finding disposition table:
`/tmp/claude-0/-home-user/cad3bd32-8dd9-557c-a8e1-57b87c8df4c3/scratchpad/repair-round1-report.md`
(local to the auditing session; not part of this repo).

## Stage 1 - SEC-GSL-101 (Critical): untrusted-repo plugin/hook auto-exec

`crates/gosling/src/plugins/discovery.rs`: a project-scope plugin's own
`settings.json` could self-mark `enabled: true` and `trusted: true` on first
discovery, letting a malicious repo's `SessionStart` hook (and any declared
MCP servers) execute on the first prompt sent in that project, with no
out-of-repo user confirmation. `filter_by_config` no longer derives `trusted`
from repo content for project-scope plugins; trust is set only by the new
`trust_project()` / `gosling plugin trust [path]` CLI action
(`crates/gosling-cli/src/commands/plugin.rs`, `crates/gosling-cli/src/cli.rs`).
Both attack paths (`HookManager::load`, `enabled_plugin_mcp_servers`) route
through `discover_enabled_plugins`, so this one choke point closes both.

## Stage 2 - ARC-GOS-001 (Critical): ui/text auto-approves every permission request

`ui/text/src/tui.tsx`'s ACP client answered `session/request_permission`
with `params.options[0]`, which the server always orders as
`[AllowAlways, AllowOnce, RejectOnce, RejectAlways]` - every gated tool call
was silently granted "allow always." Interactive mode now relays the request
to a new `PermissionPrompt` component
(`ui/text/src/components/PermissionPrompt.tsx`) and blocks on a real
keypress; `--text` mode fails closed (declines) unless the new `--yes`/`-y`
flag opts in, and even then never auto-selects "allow always."

## Stage 3 - Agent loop reliability and permission (REL-GOS-001/002/003, AOC-ORCH-001/002)

`crates/gosling/src/agents/{agent.rs,mcp_client.rs,extension_manager.rs}`,
`crates/gosling/src/permission/permission_inspector.rs`:

- MCP `initialize` handshake and the frontend-tool-result wait are now
  bounded (REL-GOS-001, REL-GOS-002) instead of hanging forever on a
  hung/malicious peer or a crashed frontend.
- `list_prompts` failures are now logged at `warn` instead of `debug`
  (REL-GOS-003).
- `PermissionInspector::inspect` now consults the user's explicit per-tool
  permission before the `GoslingMode::Auto` shortcut, closing the bypass
  that let a delegated subagent (always run in Auto) ignore a `NeverAllow`
  the user had set (AOC-ORCH-001).
- Cancelling mid-tool-batch no longer persists an empty response paired
  with an already-durable tool request (AOC-ORCH-002).

## Stage 4 - Server reply single-flight guard (CON-GOS-100 / REL-GOS-012)

`crates/gosling-server/src/routes/reply.rs`: the legacy `POST /reply`
endpoint now registers with the same per-session `SessionEventBus` the newer
`/sessions/{id}/reply` uses, closing the two-concurrent-turns race the audit
flagged from both the concurrency and reliability lenses.

## Stage 5 - gosling-mcp SSRF and path containment (MCP-031, SEC-GSL-102/103)

`crates/gosling-mcp/src/computercontroller/{mod.rs,docx_tool.rs,pdf_tool.rs}`:
`web_scrape`'s HTTP client now re-validates every redirect hop against the
same private/loopback/metadata-IP check used pre-flight, plus a timeout and
response size cap (MCP-031). `xlsx_tool`/`docx_tool`/`pdf_tool` (including
`docx_tool`'s `image_path`) now confine model-supplied paths to the gosling
process's working directory via a new `resolve_document_path` helper
(SEC-GSL-102). `pdf_tool`'s `extract_images` sanitizes the PDF-internal
XObject name before joining it into an output path (SEC-GSL-103).

## Record closure

These six findings (plus co-located REL-GOS-003/AOC-ORCH-002/SEC-GSL-103)
are closed as of the commits on branch `claude/merge-and-repair-defects-njrq1r`
listed in the closeout report referenced above. CON-GOS-101 (usage/cost
lost-update) is not closed - its primary reachability path (CON-GOS-100) is
now closed, but the underlying read-modify-write in
`crates/gosling/src/agents/reply_parts.rs` / `session_manager.rs::apply_update`
is unchanged; left open with a note for round 2. All other Medium/Low
findings from the six source reports are explicitly deferred (not
attempted this round) - see the closeout report's disposition table.

## Verification

```bash
cargo fmt --check                                          # clean
cargo test -p gosling --lib -- permission_inspector plugins::discovery   # 23 passed
cargo test -p gosling --test agent streaming_persistence    # 1 passed
cargo test -p gosling-server --lib routes::reply::          # 3 passed
cargo test -p gosling-mcp --lib                              # 97 passed
cargo check -p gosling -p gosling-cli -p gosling-server -p gosling-mcp   # clean
cargo clippy -p gosling -p gosling-cli -p gosling-server -p gosling-mcp \
  --all-targets -- -D warnings                               # clean
```

`ui/text`'s TypeScript change (Stage 2) could not be typechecked/built in
this environment: the outbound proxy blocked `pnpm install`'s fetch of
`electron/node-gyp` (403), and disk space was constrained. Every SDK field
used was cross-checked against the already-typechecked sibling
implementation in `ui/desktop/src/acp/permissionRequests.ts` rather than
guessed. Flagged as a residual verification gap for round 2 or CI.

## Residuals for round 2

- CON-GOS-101 (Medium): atomic-increment fix for `accumulated_usage`/
  `accumulated_cost` not applied; reachability narrowed by Stage 4.
- REL-GOS-011 (Medium): reply-task setup phase still not cancellation-guarded
  before the streaming loop begins (cost M, deferred).
- `ui/text` TypeScript build/typecheck for Stage 2's change is unverified in
  this environment - run `pnpm install && pnpm run lint` (or `tsc --noEmit`)
  from `ui/` before treating it as fully verified.
- A dedicated interleaved-cancellation regression test for AOC-ORCH-002 was
  not added (test-harness complexity); the fix was code-reviewed and the
  existing `test_streaming_text_not_persisted_per_token` covers the changed
  branch's non-cancelled path.
- Full live-redirect-following coverage for MCP-031 (mock HTTP server) was
  not added; the redirect-policy unit test (`redirect_target_is_private`)
  covers the core logic without a live server.
- All Medium/Low findings from the six source reports not named above
  (provider timeout/retry consistency, TLS boot-marker ordering, mutex
  poisoning, `session_manager.rs` god-object split, `GOSLING_PATH_ROOT`
  divergence, `databricks_v2` documentation, CORS `Any`, secret-mask
  reveal length, verbose error text, memory-tool concurrent-write race,
  gosling-mcp subprocess timeouts/exit-status handling, tool annotations,
  unbounded tool output) are unaddressed - see the closeout report for the
  full inventory and why each was deferred rather than attempted.
