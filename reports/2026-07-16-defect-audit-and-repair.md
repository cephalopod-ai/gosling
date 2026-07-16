# Defect audit and repair campaign — 2026-07-16

Skill source: `agent-skills` repo, `010_audit/*` (audit) and `020_repair/repair-defect-campaign` (repair). Scope: full repository, discovery mode — no supplied findings list. Branch: `claude/gosling-audit-repair-khn5pk`, based on current `main` (no prior uncommitted work on this branch).

## Skill applicability disposition (010_audit, 44 total)

Applied in depth (12 lenses, one parallel audit agent per lens, each reading the skill's own `SKILL.md`/`CHECKLIST.md` before auditing):

`audit-agent-orchestration-code`, `audit-mcp-server`, `audit-resource-lifecycle`, `audit-dataflow-concurrency`, `audit-recovery-idempotency`, `audit-security-llm`, `audit-security` + `audit-security-nodejs` (combined pass), `audit-dependency-criticality`, `audit-invariant-sync`, `audit-operator-signal`, `audit-workflow-gui`, `audit-security-repo-triage` + `audit-deadcode-cleanup` (combined pass).

Considered and dispositioned not-applicable or lower-priority for this repository (a Rust agent framework + Electron/Ink UI, no direct database/SQL beyond local SQLite session storage already covered under recovery-idempotency, no GraphQL/Flutter/Go/equation-sourcebase/acquisition surfaces):

`audit-architecture-drift` (needs a maintained invariant registry this repo doesn't have — route to a future `plan-architecture-invariants` pass), `audit-architecture-nodejs` (backend is Rust, not Node — the only Node surface is Electron main process, covered by `audit-security-nodejs`), `audit-architecture-seam` (general seam review, partially covered incidentally by the orchestration/invariant-sync passes), `audit-compliance-posture` (no SSDF/NIST mapping requested), `audit-contract-crossrepo` (no sibling repo shares a contract with gosling today — the Goose-compatibility catalog fallback is one-directional and documented, not a bidirectional contract), `audit-contract-internalapi` (partially covered by invariant-sync; full pass deferred), `audit-dataflow-cascade` (no multi-stage pipeline/report-to-patch chain in scope), `audit-dataflow-input-output` (no file upload/OCR/export-injection surface), `audit-dataflow-integrity` (covered by recovery-idempotency for the one DB — SQLite session store), `audit-dataflow-pipeline-graph` (no orchestrator/pipeline graph of the shape this skill targets), `audit-dataflow-state-transition` (session lifecycle spot-checked under recovery-idempotency), `audit-dataflow-temporal` (session/cache staleness spot-checked under recovery-idempotency and dependency-criticality), `audit-design-webapp` (gosling ships a desktop app, not a webapp; `audit-workflow-gui` covers the equivalent ground), `audit-equation-sourcebase` (not applicable — no equation/sourcebase stack), `audit-failsafe-readiness` (overlaps `audit-reliability`/`audit-operator-signal`, which were run), `audit-flutter-ios` (not applicable — no Flutter/iOS surface), `audit-go-repo-hardening` (not applicable — no Go code), `audit-graphdb-design` (not applicable — no graph database), `audit-memory-lifecycle` (spot-checked incidentally; no evidence of unbounded heap growth found in the audited crates, not run as its own full pass), `audit-multiagent-consensus` (meta-methodology for running multiple *models*; this campaign already runs multiple *lenses* per repository, achieving the same evidence-normalization goal without a second full model pass), `audit-negative-space` (partially exercised — several findings above were surfaced by "what if this control silently degrades" reasoning; not run as an independent full pass given time budget), `audit-performance-profile` (no reported latency/throughput complaint; out of scope for a defect-repair campaign per that skill's own routing note), `audit-pipeline-externalapi` (provider/external-API contract concerns covered under dependency-criticality and security-llm), `audit-playtest-app` (no live Electron/CLI harness available in this sandbox to drive interactively), `audit-security-owasp` (OWASP-framed re-statement of ground already covered by `audit-security`/`audit-security-code`-equivalent passes; redundant for this repo's non-webapp shape), `audit-security-code` (Python/Node/Postgres/Supabase-focused; gosling's backend is Rust — the Node-relevant slice is covered by `audit-security-nodejs`), `audit-security-repo-posture` (broader supply-chain engagement; the faster `audit-security-repo-triage` pass was run instead per that skill's own routing note), `audit-security-supabase` (not applicable — no Supabase), `audit-security-vuln-harness` (heavy multi-agent pentest harness; out of scope for a defect-repair campaign, no exploit-proof deliverable requested).

44 skills reviewed for applicability; 12 run as full audits, 32 dispositioned not-applicable/out-of-scope/covered-incidentally with reasons above (total considered: 44).

## Defect inventory

35 unique, source-verified defects survived the audit (2 raw findings merged as duplicates of the same root cause). Table below; full per-lens evidence lives in the individual audit transcripts referenced by lens name. Priorities: P0 correctness/security/crash/broken-build, P1 major backend-risk/broken-workflow, P2 moderate, P3 low/maintainability.

| ID | Title | Domain | Priority | Files (primary) |
|---|---|---|---|---|
| ORCH-001 | `delegate()` forces every subagent into Auto mode, silently bypassing Security/Egress inspectors and the operator's configured approval gate | security | **P0** | `agents/platform_extensions/summon.rs`, `tool_inspection.rs`, `security/security_inspector.rs`, `security/egress_inspector.rs` |
| ORCH-002 | Delegated agents inherit full tool/extension surface by default; agent-file "role" claims are prose, not an enforced control | security | P1 | `agents/platform_extensions/summon.rs` |
| ORCH-003 | Orchestrator `send_message` hangs forever when the managed agent's tool call needs approval | reliability | P1 | `agents/platform_extensions/orchestrator.rs`, `agents/tool_execution.rs` |
| ORCH-004 | Cancelling a synchronous `delegate()` reports success with silently truncated output | reliability | P2 | `agents/subagent_handler.rs`, `agents/platform_extensions/summon.rs` |
| ORCH-005 | Subagent extension-load failures are silently swallowed (`debug!` only) | reliability | P2 | `agents/subagent_handler.rs` |
| LLM-001 | SmartApprove's "read-only" auto-allow LLM classification never sees call arguments, then caches `AlwaysAllow` persistently per tool name | security | P1 | `permission/permission_judge.rs`, `permission/permission_inspector.rs`, `config/permission.rs` |
| LLM-002 | SmartApprove trusts an MCP server's own self-declared `read_only_hint` for silent auto-execution (confused deputy) | security | P1 | `permission/permission_inspector.rs` |
| LLM-003 | Tool/MCP results enter conversation context unscanned; injection scanner only inspects the model's own outgoing tool-call arguments | security | P1 | `agents/large_response_handler.rs`, `security/scanner.rs` |
| MCP-001 | `println!` on stdout during ComputerController startup corrupts the JSON-RPC stream | reliability | P1 | `gosling-mcp/src/computercontroller/mod.rs` |
| MCP-002 | Unbounded XLSX range request can exhaust memory / hang the MCP server | reliability | P1 | `gosling-mcp/src/computercontroller/xlsx_tool.rs` |
| MCP-003 | `remove_specific_memory` always reports success even when nothing matched | correctness | P2 | `gosling-mcp/src/memory/mod.rs` |
| MCP-004 | `remove_specific_memory` substring-matches whole memory blocks, over-deleting unrelated entries | data-integrity | P2 | `gosling-mcp/src/memory/mod.rs` |
| MCP-005 | Linux `computer_control` multi-line scripts interpolated unescaped into a Python string literal (injection) | security | P2 | `gosling-mcp/src/computercontroller/platform/linux.rs` |
| MCP-006 | Cache filenames collide at one-second granularity, silent overwrite | reliability | P3 | `gosling-mcp/src/computercontroller/mod.rs` |
| RES-001 | Backgrounded/detached shell child processes are never killed — **orphaned process**, confirmed by the repo's own test | reliability | **P1** | `agents/platform_extensions/developer/shell.rs` |
| RES-002 | Provider CLI subprocess can be orphaned on app quit while a session is active (macOS/Windows) | reliability | P1 | `gosling-server/src/commands/agent.rs`, `ui/desktop/src/goslingServe.ts`, `subprocess.rs` |
| RES-003 | Docker-container MCP extension processes may survive local `docker exec` client force-kill | reliability | P2 | `agents/extension_manager.rs`, `agents/container.rs` |
| REC-001 | Tool side effects execute before the turn's messages are durably persisted — crash risks duplicate re-execution on resume | reliability | P1 | `agents/agent.rs` |
| REC-002 | Legacy-session import marks migration complete before the import work finishes — permanent silent data-visibility loss on interrupt | reliability | P2 | `session/session_manager.rs`, `session/legacy.rs` |
| REC-003 | `import_session`/`copy_session` not transactional — interruption leaves an orphaned partial session | reliability | P3 | `session/session_manager.rs` |
| DEP-001 | Provider-fallback logic never engages for its actual target failure (missing/invalid credentials) | reliability | P1 | `agents/agent.rs` |
| DEP-002 | Provider returning HTTP 200 with empty content is accepted as a normal completed turn — no signal | reliability | P1 | `gosling-providers/src/formats/openai.rs`, `gosling-providers/src/base.rs`, `agents/agent.rs` |
| DEP-003 | One malformed custom-provider config file silently deregisters all custom providers | reliability | P2 | `config/declarative_providers.rs`, `providers/init.rs` |
| INV-001 | `ProviderType` enum bypassed by `Debug`-formatted string at the DTO boundary, consumed via unchecked TS cast | correctness | P1 | `acp/server/providers.rs`, `gosling-sdk-types/src/custom_requests.rs`, `ui/desktop/src/acp/providers.ts` |
| INV-002 | No drift-guard test for hand-maintained TS enum mirrors of Rust enums | maintainability | P3 | `ui/desktop/src/types/*.ts` |
| OPS-001 | `/status` is a static-200 health lie with no dependency probe | reliability | P1 | `gosling-server/src/routes/status.rs` |
| OPS-002 | Diagnostics report's `errors` field exists end-to-end but is never populated | correctness | P2 | `session/diagnostics.rs`, `acp/server/diagnostics.rs`, `ui/desktop/src/types/diagnostics.ts` |
| OPS-003 | `Finish.reason`/`exit_type` telemetry hardcoded regardless of actual exit cause (error/cancel look identical to normal) | correctness | P2 | `gosling-server/src/routes/reply_service.rs` |
| OPS-004 | Session-mutation 500s discard the root cause entirely — no client message, no server log | correctness | P2 | `gosling-server/src/routes/session.rs`, `routes/status.rs` |
| OPS-005 | Add/remove-extension endpoints return 200 even when the extension retry-load silently failed | correctness | P1 | `gosling-server/src/routes/agent.rs` |
| GUI-001 | Tool-call status badge shows **"success"** for tool calls that never received a backend response | frontend/UX-bug | **P1** | `ui/desktop/src/components/ToolCallWithResponse.tsx` |
| GUI-002 | "Always Allow all {extension} tools" mutates backend permissions before validating the approval request is still live | frontend/UX-bug | P1 | `ui/desktop/src/components/ToolApprovalButtons.tsx` |
| GUI-003 | User-prompt row uses `wrap="wrap"` inside a fixed `height={1}` Box — the exact anti-pattern `AGENTS.md` documents — overflows for long prompts | frontend/UX-bug | **P2** | `ui/text/src/components/ContentRenderers.tsx`, `ui/text/src/tui.tsx` |
| GUI-004 | External-backend settings appear saved even when persistence actually fails | frontend/UX-bug | P2 | `ui/desktop/src/components/settings/app/ExternalBackendSection.tsx` |
| GUI-005 | "Check for Updates" success confirmation can silently be skipped (stale closure) | frontend/UX-bug | P3 | `ui/desktop/src/components/settings/app/UpdateSection.tsx` |
| SEC-001 | `gosling tui` falls back to executing an untrusted script from the current working directory | security | P1 | `gosling-cli/src/commands/tui.rs` |
| SEC-002 | LLM request/response logs written without owner-only (0600/0700) permissions, unlike every other secret-bearing file in the codebase | security | P1 | `providers/utils.rs` |
| SEC-003 | `open-directory-in-explorer` IPC handler skips the renderer-file-access confinement its siblings enforce | security | P3 | `ui/desktop/src/main.ts` |
| CON-001 | Lost update on `sessions.extension_data` when an LRU-evicted agent is re-created while its turn is still running | reliability | P1 | `execution/manager.rs`, `agents/agent.rs`, `agents/platform_extensions/todo.rs`, `session/session_manager.rs` |
| CON-002 | `Config::save_values` releases its exclusive lock before the atomic rename, and truncates a fixed shared temp path unprotected by that lock | reliability | P2 | `config/base.rs` |
| CON-003 | Concurrent summarizer runs on the same project can duplicate the extracted-memory heading in `CLAUDE.md`/`AGENTS.md` | correctness | P3 | `context_mgmt/summarizer/writer.rs` |
| CI-001 | `rebuild-skills-marketplace.yml` duplicates `deploy-docs-and-extensions.yml`'s deploy pipeline but silently omits `npm test` | build/CI | P2 | `.github/workflows/rebuild-skills-marketplace.yml`, `.github/workflows/deploy-docs-and-extensions.yml` |

Note: `ORCH-001` and the LLM-audit's independently-surfaced "Auto mode neuters the security/egress inspectors" finding are the same root cause (the `auto_downgrades_require_approval` default in `tool_inspection.rs` plus `summon.rs` hard-coding `Auto`) and are repaired together as one group.

Deferred (pre-existing, explicitly out of scope, not touched by this campaign): `quick-xml` cargo-deny advisories (per `deny.toml`); Sections A-G of the 2026-07-09 audit-repair ledger and its deferred findings; the Tagteam Phase-2/3 integration horizon tracked in `docs/TODO.md`; `TagteamRunStore` production wiring (sound in isolation, not yet reachable from live code — noted for a future audit once wired).

## Repair status

See "Repairs" sections below, added as each stage lands.
