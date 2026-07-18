# Exhaustive defect audit checkpoint — 2026-07-17

Status: audit frozen; no repairs applied. Baseline: branch
`claude/gosling-defect-repair-followup-6093` at `2e7e02023` with a clean
worktree before this report was created.

## Method and limits

The audit used the private `agent-skills` catalog's multi-agent consensus
method plus the applicable architecture, dataflow, reliability, negative-space,
security, LLM security, MCP, internal-contract, invariant, resource-lifecycle,
Node/Electron, workflow-GUI, and external-API lenses. Two blind static passes
and a root adjudication pass inspected the same immutable revision. A dedicated
UI pass stalled while returning its transcript and was recorded as interrupted;
none of its unreturned work is counted as corroboration. All workers use the
same model family, so agreement is weak corroboration rather than
family-diverse consensus.

The tracked-file inventory contained 1,092 eligible code/configuration files
(540,590 lines), including 825 product-source files (260,706 lines), 373 Rust
product files, and 375 desktop/text TypeScript product files. Generated API
code, vendored dependencies, build outputs, assets, and earlier audit reports
were excluded from discovery. The audit was static: repository instructions
reserve build, test, and Clippy commands for an explicit build/test request, so
no runtime exploit drill, build, test, lint, or formatter was run. "Exhaustive"
therefore means exhaustive static coverage of the resolved product-source
scope, not proof that no undiscovered defect exists.

## Frozen inventory

Thirty-four unique defects survived source-to-sink re-reading and
deduplication. Related manifestations are grouped under one root-cause ID.

| ID | Severity | Confidence | Defect | Primary evidence |
|---|---|---|---|---|
| AUD-001 | High | High | Native session import persists executable Stdio/InlinePython extension configuration, which resume then launches without an import-specific approval boundary | `session/session_manager.rs`, `session/extension_data.rs`, `routes/agent.rs`, `agents/extension_manager.rs` |
| AUD-002 | High | High | Session import silently drops `restrict_tools_to_working_dirs`, weakening the imported session's policy | `session/session_manager.rs`, `permission/working_dir_scope_inspector.rs` |
| AUD-003 | High | High | SmartApprove trusts an MCP server's `readOnlyHint=true` and persists `AlwaysAllow`; an innocuous name can self-authorize a mutating tool | `config/permission.rs`, `permission/permission_inspector.rs` |
| AUD-004 | High | High | An argument-specific LLM read-only decision is cached by tool name, so benign arguments authorize later destructive arguments | `permission/permission_inspector.rs`, `permission/permission_judge.rs` |
| AUD-005 | High | High | Auto mode downgrades the working-directory inspector's `RequireApproval` result to `Allow` | `permission/working_dir_scope_inspector.rs`, `tool_inspection.rs` |
| AUD-006 | High | High | Working-directory scope uses lexical `starts_with`, allowing `..` traversal and symlink escapes | `permission/working_dir_scope_inspector.rs` |
| AUD-007 | High | High | Gemini, Cursor, and Tagteam self-managing CLI providers ignore Gosling approval modes; Gemini and Cursor always enable autonomous/force execution | `providers/gemini_cli.rs`, `providers/cursor_agent.rs`, `providers/tagteam.rs` |
| AUD-008 | High | High | Working-directory restriction only inspects Gosling tool calls and cannot constrain tools executed internally by self-managing CLI providers | `permission/working_dir_scope_inspector.rs`, `agents/agent.rs`, CLI provider modules |
| AUD-009 | High | High | ACP extension secret rehydration matches only name and variant, so a client can redirect stored env secrets to a different command or HTTP endpoint with the same name | `acp/server.rs` |
| AUD-010 | High | High | InlinePython extension processes inherit the entire Gosling environment, unlike ordinary Stdio extensions' minimized environment | `agents/extension_manager.rs` |
| AUD-011 | High | High | ChatGPT Codex, GitHub Copilot, and Kimi OAuth caches use ordinary/non-atomic writes; token files can be created too broadly before later chmod or never restricted | `providers/chatgpt_codex.rs`, `providers/githubcopilot.rs`, `providers/kimicode.rs` |
| AUD-012 | High | High | Assistant Markdown can silently fetch arbitrary HTTPS images, providing a prompt-injected data-egress channel | `components/MarkdownContent.tsx`, `utils/csp.ts` |
| AUD-013 | High | High | Electron renderer file confinement is lexical and follows symlinks for read, write, delete, mkdir, list, open, and reveal operations | `ui/desktop/src/main.ts` |
| AUD-014 | High | Likely | Cancelling a reply removes its token from the session occupancy check before the task stops, admitting an overlapping turn | `gosling-server/src/session_event_bus.rs`, `routes/session_events.rs`, `routes/reply_service.rs` |
| AUD-015 | Medium | High | ACP prompts and direct subagents await the next provider stream item before checking cancellation; a stalled stream cannot be woken by cancellation | `acp/server.rs`, `agents/subagent_handler.rs` |
| AUD-016 | Medium | Medium-high | Cancelling/dropping a developer shell future can kill the direct shell but skip process-group descendant cleanup | `platform_extensions/developer/mod.rs`, `developer/shell.rs` |
| AUD-017 | Medium | High | Server graceful shutdown has no finite TLS deadline and SSE heartbeat tasks have no server-shutdown signal, so shutdown can wait forever | `gosling-server/src/commands/agent.rs`, `routes/session_events.rs` |
| AUD-018 | Medium | High | Extension-loading handles are replaced or removed without aborting; resume uses a non-atomic check-then-set and can launch duplicate detached loaders | `gosling-server/src/state.rs`, `routes/agent.rs` |
| AUD-019 | Medium | High | Desktop backend cleanup treats `ChildProcess.killed` as proof of exit, suppresses escalation, and unregisters a process that may still be alive | `ui/desktop/src/goslingServe.ts` |
| AUD-020 | Medium | High | MCP tool pagination has no cursor-cycle or page/item bound, permitting an infinite loop and unbounded growth | `agents/extension_manager.rs` |
| AUD-021 | Medium | High | MCP resource and prompt discovery reads only the first page and silently hides later entries | `agents/extension_manager.rs`, `agents/mcp_client.rs` |
| AUD-022 | High | High | Five command providers discard session cwd because they use the default cwd-unaware factory and never set `Command::current_dir` | `providers/base.rs`, `gemini_cli.rs`, `cursor_agent.rs`, `claude_code.rs`, `codex.rs`, `tagteam.rs` |
| AUD-023 | Medium | High | Cursor pipes stderr without draining it, allowing pipe saturation to deadlock the provider | `providers/cursor_agent.rs` |
| AUD-024 | Medium | High | Cursor converts explicit error, malformed, and empty output into successful assistant replies | `providers/cursor_agent.rs` |
| AUD-025 | Medium | High | Codex returns accumulated partial text as success after `turn.failed`, another terminal error, or a nonzero exit | `providers/codex.rs` |
| AUD-026 | Medium | High | Provider and Gosling-mode updates mutate live state before durable session persistence, leaving split state when persistence fails | `agents/agent.rs` |
| AUD-027 | Medium | High | ACP working-directory update persists the new root before provider recreation and extension-root refresh, with no rollback on downstream failure | `acp/server/manage_sessions.rs` |
| AUD-028 | Medium | High | Review worker subprocesses run in the caller's subdirectory instead of the resolved repository root | `gosling-cli/src/commands/review/orchestrator.rs`, `handler.rs` |
| AUD-029 | Medium | High | Concurrent review phases use separate four-worker semaphores, allowing eight workers despite the global cap | `gosling-cli/src/commands/review/orchestrator.rs`, `handler.rs` |
| AUD-030 | Medium | High | Concurrent async delegates check the task count and insert later under separate locks, racing past the configured cap | `platform_extensions/summon.rs`, `agents/agent.rs` |
| AUD-031 | High | High | Tool side effects occur before the request/response turn is durably persisted; a crash can cause duplicate execution after resume | `agents/agent.rs` |
| AUD-032 | Medium | High | The desktop ACP WebSocket adapter buffers incoming parsed messages in an unbounded array without backpressure or a maximum | `ui/desktop/src/acp/createWebSocketStream.ts` |
| AUD-033 | Medium | High | Invalid `DATABRICKS_HOST` input panics in OAuth endpoint discovery through `expect` instead of returning a configuration error | `providers/oauth.rs` |
| AUD-034 | Medium | High | Ink uses `wrap="wrap"` for dynamic text inside fixed-height screens, violating the layout budget and bleeding into adjacent cells | `ui/text/src/configure.tsx`, `onboarding.tsx`, `components/ErrorScreen.tsx` |

## Root-cause notes and repair acceptance criteria

- AUD-001/AUD-002 share the session import trust boundary. Imported executable
  extensions must be quarantined, and all intentionally preserved or reset
  security fields must be explicit and round-trip tested.
- AUD-003/AUD-004 share an authorization-cache flaw. Server-authored metadata
  must never grant authority, and an LLM classification of one argument set
  must not become persistent authorization for a tool name.
- AUD-005/AUD-006/AUD-008 share a misleading working-directory control. Its
  decisions must survive Auto mode, compare canonical/normalized paths, and
  fail closed or be declared unsupported for providers that execute tools
  outside Gosling.
- AUD-007/AUD-008/AUD-022 require a single provider capability contract for
  approval-mode support, tool confinement, and session cwd. Silent degradation
  is not an acceptable compatibility behavior.
- AUD-010/AUD-011 share secret/process hygiene. Child environments must be
  allowlisted, and all credential caches must use the repository's owner-only,
  atomic secret writer.
- AUD-014/AUD-015/AUD-016/AUD-018 are cancellation/lifecycle defects. A cancel
  acknowledgement is not task completion; capacity and ownership must remain
  reserved until cleanup finishes.
- AUD-020/AUD-021/AUD-032 require explicit resource bounds and complete,
  cycle-detecting pagination.
- AUD-026/AUD-027 require rollback or prepare/commit ordering so durable and
  live state cannot diverge.

Every repair should add a focused regression test at the lowest stable seam.
Security repairs must include a negative case proving the old bypass is closed;
concurrency repairs must include an interleaving or stalled-resource case;
process repairs must confirm actual exit rather than signal delivery.

## Adjudicated non-findings and exclusions

- Ordinary Stdio MCP children already clear the inherited environment, add a
  minimal child environment, use configured cwd, drain bounded stderr, and
  participate in process-group cleanup. AUD-010 is InlinePython-specific.
- MCP privileged metadata is stripped before trusted host metadata is attached,
  and MCP request handling selects among response, timeout, and cancellation.
- Session copy/import database operations use transactions or compensating
  deletion for partial session rows; the remaining import findings concern
  executable trust and a dropped policy field.
- Direct subagent approval requests fail closed or are redirected rather than
  being implicitly approved. The confirmed subagent issue is stalled-stream
  cancellation.
- The main Electron media permission handler is broad, but the audited
  untrusted iframes do not receive the required microphone permission-policy
  delegation, so no reachable microphone bypass was established.
- The Windows `open-in-chrome` handler uses `cmd.exe`, but no product call site
  for the exposed preload method was found. It remains defense-in-depth debt,
  not a confirmed reachable finding in this checkpoint.
- InlinePython dependency token parsing is permissive for complex requirement
  syntax, but the malware/reputation check is not represented as a complete
  execution sandbox; no separate security guarantee was established.
- The current `Agent::list_extension_prompts` `expect` is fed by a function that
  aggregates per-extension failures and currently always returns `Ok`; no
  reachable panic was established at that call site.

## Checkpoint disposition

No source repair is included in this checkpoint. Per user instruction, this
report must be committed and synchronized with the remote before the
`repair-defect-campaign` skill is loaded or any finding is patched.
