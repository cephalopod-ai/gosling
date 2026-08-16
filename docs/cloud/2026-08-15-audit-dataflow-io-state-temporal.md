# Gosling Audit — Input/Output, State Transition, Temporal

**Date:** 2026-08-15  
**Target:** `/Users/eric/Work/vscode/forked/gosling`  
**HEAD:** `073d19428509ea6eb317924b1856a1fe7e9002c8`  
**Authority:** `read_only` — source was not modified; no tests executed.  
**Lenses:** `audit-dataflow-input-output` (IOP-001..015), `audit-dataflow-state-transition` (STT-001..012), `audit-dataflow-temporal` (TMP-001..015)

The supplied prompt is treated as a draft. The intended mission is preserved: re-read current source at this HEAD for session import/export, plugin git clone args, path policy, permission-mode transitions, OAuth token expiry, workspace snapshot, `GOSLING_PATH_ROOT` isolation, and config migration. Review was expanded to adjacent seams those surfaces imply (archive reactivation, Chat-mode frontend tools, CLI global mode write, plugin auto-update stamp, JSON/Nostr replay). Historical reports under `docs/cloud/audit-dataflow-*.md` were used only as seeds and were not copied; several 2026-07/08 findings are stale against this tree.

---

## Executive Verdict

No Critical defect was confirmed on these three lenses. The session import quarantine (strip executable extensions, force `GoslingMode::Approve`, pin working dir, mark history untrusted, 16 MiB bound, transactional write) and plugin clone `--` option terminator hold at the source I read. Path policy canonicalizes and prefix-checks; workspace import rejects traversal and secret-shaped keys.

Three Confirmed defects are worth patching now:

1. **STT-GOS-001 (Medium)** — Chat mode still dispatches frontend tools before the Chat skip.
2. **STT-GOS-002 (Medium)** — Archive is not a terminal state: `get_session_agent` reloads and prompts an archived session without clearing `archived_at`.
3. **STT-GOS-003 (Medium)** — CLI `/mode` writes the process-global default, so one session's mode change becomes the next session's default.

Isolation and freshness gaps around `GOSLING_PATH_ROOT` plugin settings, `RUNTIME_PATHS` vs `plugins_dir`, permission persist fail-open, and plugin auto-update stamping `last_update_check` *before* the update are real but narrower. Deployment/merge need not pause; patch the Chat/archive/global-mode trio first.

Oracle integrity: I did not run a fresh-process import/export or the in-process suite. Non-findings below are **source-evidenced holds**, not test-oracle holds.

---

## Scope

- Repository / branch / commit: `gosling` `main` `073d19428509ea6eb317924b1856a1fe7e9002c8`
- Prompt reviewed: 2026-08-15 exhaustive audit orientation; this lens trio assigned session import/export, plugin git clone args, path policy, permission mode, OAuth expiry, workspace snapshot, `GOSLING_PATH_ROOT`, config migration
- Skills invoked: `audit-dataflow-input-output` v3.2, `audit-dataflow-state-transition` v3.2, `audit-dataflow-temporal` v3.3 plus `000_common/audit-base/{audit_method,evidence_discipline,finding_format,severity_matrix,confidence_calibration,report_template,project_overlays,cross_lens_escalation}`
- Referenced but not present/readable here: `routing_guide.md`, `model_adaptations.md`, `static_vs_observed_examples.md`, `finding_schema.json` — proceeded with the files that loaded
- Files/directories inspected (primary):
  - `crates/gosling/src/session/{session_manager.rs,import_formats/*,nostr_share.rs}`
  - `crates/gosling-cli/src/commands/session.rs`, `crates/gosling-cli/src/session/mod.rs`
  - `crates/gosling/src/acp/server.rs`, `crates/gosling/src/acp/server/manage_sessions.rs`
  - `crates/gosling/src/plugins/{mod.rs,discovery.rs,formats/open_plugins.rs,mcp_servers.rs}`
  - `crates/gosling/src/permission/{permission_inspector.rs,permission_judge.rs,working_dir_scope_inspector.rs}`
  - `crates/gosling/src/config/{paths.rs,permission.rs,migrations.rs,base.rs}`
  - `crates/gosling/src/oauth/{mod.rs,persist.rs}`
  - `crates/gosling/src/providers/{provider_secrets.rs,xai_oauth.rs,gemini_oauth.rs,chatgpt_codex.rs}`
  - `crates/gosling/src/workspace/{service.rs,validation.rs,store.rs}`
  - `crates/gosling/src/agents/{agent.rs,tool_execution.rs}`
  - `crates/gosling-providers/src/gosling_mode.rs`
  - `ui/desktop/src/{main.ts,utils/sessionImport.ts,components/sessions/SessionListView.tsx}`
- Commands/tests run: none (read-only static review)
- Effort budget: ~deep-read of the eight named surfaces + adjacent mutation/consume sites (~80 files sampled, ~40 deep-read). Stop condition: every IOP/STT/TMP inventory item is a finding or explicit non-finding for the in-scope surfaces.
- Constraints: no source mutation; no live OAuth or hostile-archive execution.

---

## Draft Prompt Assessment

- Intended mission: combined IOP/STT/TMP audit of the eight named surfaces at current HEAD, written to this path, covering every inventory code with quoted `file:line`.
- Under-specified: which prior `docs/cloud/audit-dataflow-*.md` findings to re-close; whether shell-namespace isolation is in scope. I treated both as in scope.
- Overly narrow if taken literally: plugin clone args without plugin install copy / MCP spawn; OAuth expiry without consume-site refresh; path policy without workspace snapshot pinning.
- Added angles: archive reactivation, Chat frontend bypass, CLI global mode write, plugin auto-update stamp-before-update, JSON/Nostr replay vs file idempotency, `RUNTIME_PATHS` vs `plugins_dir`.
- Assumptions challenged: historical “silent JSONL drop” and “read_only_hint grants Auto” findings are **stale** at this HEAD.

---

## Surface Inventory

| Surface | Actor | Input/Trigger | State/Output | Boundary | Reviewed |
|---|---|---|---|---|---|
| CLI session import file | operator | `.json`/`.jsonl` path | new session row + messages | `import_session_file` + 16 MiB + provenance sha256 | yes |
| CLI/ACP/Desktop JSON import | operator | session JSON string | new session | `import_session` + `convert_to_gosling_session_json` | yes |
| Nostr session import | operator | `gosling://sessions/nostr?...` | decrypted JSON → `import_session` | size cap + kind check + same quarantine | yes |
| Session export (ACP/CLI JSON) | operator | session id | pretty `Session` JSON | `export_session` / `serde_json::to_string_pretty` | yes |
| CLI markdown/yaml/nostr export | operator | session id + format | file/stdout/relay | CLI writer; Nostr encrypts | yes |
| Plugin git clone/install | operator | git URL or local path | `$plugins_dir/<name>` | `clone_git_repo` + `--` + name/path validators | yes |
| Plugin auto-update | process | 24h timer on skill list | replace install dir | `mark_last_update_check` then `update_plugin_at_root` | yes |
| Working-dir / workspace path policy | agent tools | tool args / shell tokens | Allow / RequireApproval / Deny | `WorkingDirScopeInspector` | yes |
| Workspace import/export | operator | workspace JSON | new workspace (new UUID) | `normalize_workspace_path` + secret-key reject | yes |
| Workspace snapshot on session | session create/copy | `WorkspaceSessionContext` | pinned columns + folder policy | `workspace_snapshot` builder | yes |
| GoslingMode | CLI/ACP/UI | mode id | session column + in-memory | `update_gosling_mode` | yes |
| Tool permission.yaml | inspector / user | AlwaysAllow / AskBefore / NeverAllow | in-memory map + disk | `PermissionManager` | yes |
| Archive / unarchive | ACP | session id | `archived_at` | `on_archive_session` / `on_unarchive_session` | yes |
| Provider OAuth tokens | providers | token files | access token | `expires_at` + refresh + 60–120s skew | yes |
| MCP OAuth | extension auth | stored creds / callback | `AuthorizationManager` | refresh-or-clear then browser flow | yes |
| Config.yaml migrations | Config load/write | legacy flat keys / platform ext | rewritten mapping | `run_migrations` / `run_read_migrations` | yes |
| Session DB schema | SessionStorage init | `schema_version` | v1..v26 in one tx | `run_migrations` | yes |
| `GOSLING_PATH_ROOT` | env / desktop | root path | config/data/state/plugins | `Paths::get_dir` + desktop `resolveGoslingPathRoot` | yes |
| Shell `RUNTIME_PATHS` | ACP/shell | namespace | scoped data/state, shared config | `RuntimePaths::for_namespace` | yes |

---

## I/O Path Inventory

| Surface | Direction | Format | Source Trust | Validation Point | Sink | Size/Resource Bound |
|---|---|---|---|---|---|---|
| Session file import | in | JSON/JSONL | operator file | metadata preflight + UTF-8 + detect/convert + serde `Session` | SQLite session | 16 MiB |
| Session JSON/Nostr import | in | JSON / NIP-44 | operator / relay | `ensure_import_payload_size` + convert + serde | SQLite session | 16 MiB (Nostr ciphertext 2×) |
| Session export | out | JSON/YAML/MD | local session | none (serialize all fields) | file/stdout/Nostr | none |
| Plugin clone | in | git tree | operator URL | `--` terminator; no scheme allowlist | temp checkout → install dir | none |
| Plugin copy | in | dir tree | checkout | skip non-file/dir (no symlink copy); relative path `./` | install dir | none |
| Workspace import | in | JSON | operator | schema ≤ current; path normalize; secret-key reject | workspace store | `MAX_SERIALIZED_WORKSPACE_BYTES` |
| Workspace export | out | JSON | local workspace | `reject_secret_shaped_value` | string | n/a |
| Tool path args | in | strings | model | canonicalize + `starts_with` roots | inspector decision | n/a |
| Desktop import picker | in | file bytes | operator | `readBoundedSessionImportFile` | ACP JSON import | 16 MiB |
| Diagnostics export | out | JSON | local | 0o600 on Unix | file | n/a |

---

## State Model Inventory

| Object | States | Legal Transitions | Gate/Guard | Mutation Layer | Durable? | Idempotent? |
|---|---|---|---|---|---|---|
| `GoslingMode` | Auto / SmartApprove / Approve / Chat | any → any (user/client) | parse enum only | `Agent::update_gosling_mode` + session row | yes, with provider rollback | last-write-wins |
| Tool request | new → approved / needs_approval / denied | mode + user/smart lists + inspectors | `PermissionInspector` + others at inspect | domain | execution recorded as tool op | per request id |
| Session archive | active (`archived_at` null) / archived | archive / unarchive | **none on prompt/load** | ACP manage + `update()` | `archived_at` column | unarchive is set-null |
| Session file import | absent / imported / source-changed | create or return existing | sha256 + source_path | `import_session_file` | yes (tx) | yes for same bytes |
| Session JSON/Nostr import | always create | create | working-dir + quarantine | `import_session` | yes (tx) | **no** |
| Workspace | create / update / delete / import / duplicate | create new UUID on import | path + secret reject + name unique | `WorkspaceService` | store mutate + lock | name collision fails |
| Workspace snapshot | pinned on session | copy copies snapshot | folder policy re-derived on load | session columns | yes | n/a |
| `permission.yaml` | per-tool level | update / annotation tighten / remove_ext | annotations cannot grant | `PermissionManager` | persist may fail open | last-write-wins |
| Config provider layout | flat keys / `providers:` block | heuristic migrate | write path only | `migrate_provider_config` | save-after-migrate | yes if save succeeds |
| Session schema | 0..26 | sequential +1 | version table in same tx | `SessionStorage::run_migrations` | yes | skip if current |

---

## Temporal Inventory

| Artifact/State | Created | Consumed | Freshness Check | Expiry/Version | Ordering | Cleanup |
|---|---|---|---|---|---|---|
| Session import provenance | import time | resume / list | sha256 for file replay | schema_version=1 | n/a | none |
| Workspace snapshot | session prepare/copy | inspector + loader | **none vs live workspace** | pinned policy | n/a | none (session delete) |
| Plugin `last_update_check` | auto-update start | next auto-update | 24h | stamped **before** update | n/a | none |
| Gemini/xAI/Codex tokens | OAuth | `get_valid_*` | `expires_at > now + skew` then refresh | 3600 default if `expires_in` missing | single-flight on xAI | clear on refresh fail |
| Provider-secret UI expiry | token JSON | settings list | `find_expires_at` **skipped if refresh_token present** | display only | n/a | delete API |
| MCP OAuth creds | callback save | `initialize_from_store` + refresh | refresh fail → clear | `issued_at` stored | n/a | clear on fail |
| Codex JWKS | first fetch | `parse_jwt_claims` | kid-miss refresh | process lifetime | n/a | none |
| Config migrations | write load | next load | heuristic, no version pin | none | n/a | leftover key cleanup |
| Session DB migrations | startup | pool init | `schema_version` max | 26 | sequential in one tx | n/a |
| `GOSLING_PATH_ROOT` trees | process env | Paths / desktop / MCP cache | env at call / LazyLock first use | n/a | n/a | n/a |

---

## Boundary Map

| Surface | Intended Boundary | Enforced At | Status |
|---|---|---|---|
| Import executable extensions | must not launch from import | `EnabledExtensionsState` removed before persist | holds |
| Import working dir | operator-supplied absolute existing dir | `validate_import_working_dir` | holds |
| Import mode | untrusted history → Approve | `create_session_in_tx(..., GoslingMode::Approve)` | holds |
| Import size | 16 MiB | file metadata + take + convert + desktop + Nostr | holds |
| Plugin clone flags | source cannot be a git option | `git clone --depth 1 -- <source> <dest>` | holds |
| Plugin path escape | `./` only, no `..` | `validate_relative_plugin_path` / `validate_plugin_name` | holds |
| Path policy | tools stay in roots; RO roots deny mutate | `WorkingDirScopeInspector` | holds (TOCTOU residual) |
| Chat mode | no tool calls | agent remaining_requests skip; **frontend path open** | **fail** (STT-GOS-001) |
| Archive | hide + unload | list filter + memory remove; **prompt reactivates** | **fail** (STT-GOS-002) |
| Mode change | session-scoped | ACP session only; **CLI also writes global** | **fail** (STT-GOS-003) |
| OAuth consume | reject/refresh expired access | provider `get_valid_*` | holds |
| Workspace import | no secrets, no traversal | `reject_secret_shaped_value` + `normalize_workspace_path` | holds |
| PATH_ROOT isolation | all gosling state under root | config/data/state yes; plugin settings path **diverges** | **fail** (TMP-GOS-002) |
| Shell namespace | isolate data/state | `RuntimePaths::for_namespace`; plugins/agents **unscoped** | **fail** (TMP-GOS-003) |
| Config migrate | once, ordered, durable | session DB yes; config.yaml heuristic | holds with caveats |

---

## Inventory Result

Every required code is a finding or an explicit non-finding. Details in Findings / Non-Findings.

| Code | Verdict | Pointer |
|---|---|---|
| IOP-001 Unvalidated Input | **Finding** IOP-GOS-002 (plugin source); import JSON is schema-parsed | plugin clone; session serde |
| IOP-002 Unsafe Output Path | **Non-finding** (CLI path is operator-chosen); diagnostics 0o600 | session.rs export |
| IOP-003 Path Traversal | **Non-finding** for workspace/import/plugin names | validation.rs, import_formats, open_plugins |
| IOP-004 Archive Slip | **Non-finding** (no untrusted extractall; plugin copy skips symlinks) | plugins/mod.rs `copy_dir_all` |
| IOP-005 Extension/Format Confusion | **Non-finding** (detect_format peeks bytes; fallback fail-closed at serde) | import_formats/mod.rs |
| IOP-006 Malformed Payload Accepted | **Non-finding** (`parse_json_lines` fail-closed) | import_formats/mod.rs:120-129 |
| IOP-007 Dangerous Export Formula | **Non-finding** (no CSV/XLSX session export) | CLI formats json/yaml/md |
| IOP-008 Provider/OCR Treated Trusted | **Non-finding** (import marks `imported_untrusted`; strips EnabledExtensions) | session_manager.rs import |
| IOP-009 Log/Report Leakage | **Finding** IOP-GOS-003 (full session JSON export, no redaction) | export_session |
| IOP-010 Generated Artifact Reuse | **Non-finding** (no keyed export cache) | — |
| IOP-011 Output Overwrite | **Finding** IOP-GOS-003 (CLI `fs::write` truncate) | commands/session.rs |
| IOP-012 Partial Output Presented Complete | **Finding** IOP-GOS-001 (unknown JSONL event types dropped) | claude_code/codex/pi converters |
| IOP-013 Unbounded File/Archive | **Non-finding** for session import (16 MiB); plugin clone unbounded (accepted operator) | import_formats |
| IOP-014 Hidden Input Source | **Non-finding** (Nostr funnels to same `import_session`) | manage_sessions.rs / nostr_share.rs |
| IOP-015 CLI/API/UI Parity | **Finding** IOP-GOS-004 (file import idempotent; JSON/ACP/Nostr not) | import_session vs import_session_file |
| STT-001 Illegal Transition | **Finding** STT-GOS-001 (Chat + frontend tools) | agent.rs |
| STT-002 Gate Bypass | **Non-finding** for user NeverAllow in Auto (held); Chat frontend is STT-GOS-001 | permission_inspector.rs |
| STT-003 Partial Transition | **Non-finding** for import/copy (single IMMEDIATE tx) | session_manager.rs |
| STT-004 Missing Status | **Non-finding** (GoslingMode default SmartApprove; archive null = active) | gosling_mode.rs |
| STT-005 Ambiguous Status | **Finding** STT-GOS-002 (archived_at set but session still promptable) | get_session_agent |
| STT-006 Cross-Scope Transition | **Finding** STT-GOS-003 (CLI mode → global config) | session/mod.rs |
| STT-007 Rejected/Archived Mutable | **Finding** STT-GOS-002 | manage_sessions + get_session_agent |
| STT-008 Review Gate Skipped | **Non-finding** for import extensions (stripped); Chat frontend = STT-GOS-001 | import_session |
| STT-009 Mutation In Wrong Layer | **Finding** STT-GOS-003 (CLI writes config default) | session/mod.rs |
| STT-010 Idempotency Missing | **Finding** IOP-GOS-004 / STT-GOS-004 (JSON/Nostr import replay) | import_session |
| STT-011 Transition Not Durable | **Finding** STT-GOS-005 (permission persist fail-open) | permission.rs |
| STT-012 Derived State Authority | **Finding** TMP-GOS-001 (auto-update stamp) + workspace snapshot pin (documented; Info) | plugins/mod.rs |
| TMP-001 Stale State | **Finding** TMP-GOS-005 (pinned workspace snapshot vs live workspace) | workspace_snapshot + reject mutation |
| TMP-002 Stale Artifact Reuse | **Non-finding** for file import (sha256); JSON import is replay not stale artifact | import_session_file |
| TMP-003 Expired Authority Accepted | **Non-finding** at consume (providers refresh); **Finding** TMP-GOS-004 JWKS unverified fallback | chatgpt_codex.rs |
| TMP-004 Replay Error | **Finding** STT-GOS-004 (JSON/Nostr duplicate sessions) | import_session |
| TMP-005 Ordering Failure | **Non-finding** (session DB migrations sequential; no event-log apply) | run_migrations |
| TMP-006 Migration Sequencing | **Non-finding** for session DB; **Finding** TMP-GOS-006 (config.yaml unversioned heuristic) | migrations.rs |
| TMP-007 TOCTOU | **Finding** TMP-GOS-007 (path check then later tool use) | working_dir_scope_inspector.rs |
| TMP-008 Lifecycle Drift | **Finding** STT-GOS-002 + TMP-GOS-001 | archive / plugin stamp |
| TMP-009 Mixed-Era Report | **Non-finding** (workspace import rejects newer schema; session import rewrites provenance v1) | workspace service |
| TMP-010 Cache Invalidation | **Non-finding** for JWKS kid-miss (now refreshes); process-lifetime cache remains | chatgpt_codex.rs |
| TMP-011 Delayed Job Old Assumption | **Finding** TMP-GOS-001 (auto-update uses stamped time even if update failed) | plugins/mod.rs |
| TMP-012 Draft Treated Final | **N/A** (no draft-standard baseline in these surfaces) | — |
| TMP-013 Clock/Timezone | **Non-finding** (RFC3339 + `DateTime<Utc>` on import provenance / tokens) | import_formats, providers |
| TMP-014 Retention/Cleanup | **Non-finding** (OAuth clear-on-refresh-fail; no stated TTL for sessions) | oauth + providers |
| TMP-015 Over-Preserved Legacy | **Finding** TMP-GOS-006 (read path keeps legacy provider keys) | run_read_migrations |

---

## Findings Table

| ID | Severity | Confidence | Evidence Basis | Domain | Title | Patch Priority | Blast Radius | Complexity | Cost | Nominal Agent |
|---|---|---|---|---|---|---|---|---|---|---|
| STT-GOS-001 | Medium | Confirmed | source-evidenced | State-Transition | Chat mode still runs frontend tools | 1 | Workflow | local_guardrail | S | codex |
| STT-GOS-002 | Medium | Confirmed | source-evidenced | State-Transition | Archived session prompt-reactivates without unarchive | 1 | Workflow | local_guardrail | S | codex |
| STT-GOS-003 | Medium | Confirmed | source-evidenced | State-Transition | CLI session `/mode` writes global default | 2 | Workflow | local_guardrail | S | codex |
| STT-GOS-004 | Medium | Confirmed | source-evidenced | State-Transition | JSON/Nostr import is not replay-safe | 3 | Workflow | local_guardrail | S | codex |
| STT-GOS-005 | Medium | Confirmed | source-evidenced | State-Transition | Permission map persist can fail while memory advances | 2 | Service | local_guardrail | S | codex |
| IOP-GOS-001 | Low | Confirmed | source-evidenced | Input-Output-Path | Foreign JSONL skips unknown events with no drop count | 4 | Workflow | local_guardrail | S | codex |
| IOP-GOS-002 | Low | Confirmed | source-evidenced | Input-Output-Path | Plugin clone accepts any git source (local/file/ext) after `--` | 5 | Local | local_guardrail | S | codex |
| IOP-GOS-003 | Low | Confirmed | source-evidenced | Input-Output-Path | Session export is unredacted and overwrites | 5 | Local | local_guardrail | S | codex |
| IOP-GOS-004 | Medium | Confirmed | source-evidenced | Input-Output-Path | CLI/file vs ACP/JSON import idempotency parity | 3 | Workflow | local_guardrail | S | codex |
| TMP-GOS-001 | Medium | Confirmed | source-evidenced | Temporal | Auto-update stamps last check before the update | 2 | Local | local_guardrail | XS | codex |
| TMP-GOS-002 | Medium | Confirmed | source-evidenced | Temporal | `GOSLING_PATH_ROOT` plugin settings path ≠ `Paths::config_dir()` | 2 | Local | workflow_protocol | M | claude |
| TMP-GOS-003 | Medium | Confirmed | source-evidenced | Temporal | `plugins_dir`/`agents_dir` ignore `RUNTIME_PATHS` | 3 | Service | workflow_protocol | M | claude |
| TMP-GOS-004 | Low | Confirmed | source-evidenced | Temporal | Codex JWT falls back to unverified claims | 4 | Local | local_guardrail | S | codex |
| TMP-GOS-005 | Low | Confirmed | source-evidenced | Temporal | Workspace folder policy is pinned and not refreshed | 5 | Workflow | governance_decision | S | human-owner |
| TMP-GOS-006 | Low | Confirmed | source-evidenced | Temporal | Config.yaml migrations are unversioned; read path skips provider migrate | 4 | Local | workflow_protocol | M | gpt |
| TMP-GOS-007 | Medium | Likely | simulation-reasoned | Temporal | Path policy TOCTOU between canonicalize and tool use | 4 | Local | persistence_recovery | M | codex |

---

## Detailed Findings

### STT-GOS-001: Chat mode still dispatches frontend tools

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: State-Transition

Evidence:
- `crates/gosling-providers/src/gosling_mode.rs:31-33` — Chat is documented as “Chat only, no tool calls”.
- `crates/gosling/src/agents/agent.rs:2648-2677` — `frontend_requests` are executed via `handle_frontend_tool_request` **before** the `GoslingMode::Chat` branch that only skips `remaining_requests`.
- `crates/gosling/src/agents/tool_execution.rs:222-276` — frontend handler begins a durable tool operation and waits for a client result.
- `crates/gosling/src/permission/permission_inspector.rs:129-131` — inspector `continue`s on Chat (no Deny), relying on the agent loop.

Observed behavior:
- In Chat mode, MCP/backend tools are skipped with `CHAT_MODE_TOOL_SKIPPED_RESPONSE`, but any tool classified as a frontend tool is still dispatched to the desktop/client.

Expected boundary:
- Chat must refuse every tool request, including frontend/UI tools, at the dispatch boundary.

Failure mechanism:
- Frontend and remaining tool lists are split earlier; the Chat short-circuit only wraps `remaining_requests`.

Break-it angle:
- Register a frontend tool, set mode to Chat, prompt the model to call it — the client still receives `with_frontend_tool_request` and may execute.

Impact:
- Chat is not a no-side-effect mode for UI tools (file pickers, desktop actions, etc.).

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible / process
- Reversibility: irreversible (action already sent to client)
- Operator visibility: UI-visible
- Rerun safety: unsafe

Adjacent failure modes:
- `dispatch_app_tool_call` (`agent.rs:1100-1155`) inspects and treats Chat as needs-approval (safer sibling).
- ACP Chat mapping in `acp/provider.rs` rejects ACP tools but does not cover this frontend path.

Recommended mitigation:
- Remediation patterns: fail-closed Chat at the split, before either loop.
- Minimal repair: skip `frontend_requests` the same way as `remaining_requests` when `gosling_mode == Chat`.
- Local guardrail: inspector should `Deny` in Chat instead of `continue`.
- Behavior test: Chat + frontend tool → skipped text, no `begin_tool_operation`.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: tests
- Nominal implementation agent: codex
- Rationale: one branch + a unit test around the existing Chat skip.

Validation:
- Test: Chat mode frontend tool is not dispatched and no durable operation is begun.
- Test: Approve mode frontend tool still dispatches.

Non-goals:
- Do not remove frontend tools from other modes.

---

### STT-GOS-002: Archived sessions remain fully mutable and prompt-reactivate

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: State-Transition

Evidence:
- `crates/gosling/src/acp/server/manage_sessions.rs:657-671` — archive sets `archived_at`, removes the in-memory session, unloads the agent; does **not** add the id to `closed_session_ids`.
- `crates/gosling/src/acp/server.rs:2685-2718` — `get_session_agent` has no `archived_at` check; a missing in-memory entry is reloaded from SQLite and `activate_acp_session` runs.
- `crates/gosling/src/acp/server.rs:2879-2894` — `on_prompt` uses `get_session_agent`.
- `crates/gosling/src/acp/server.rs:3334-3348` — `on_set_mode` same.
- `crates/gosling/src/acp/server/manage_sessions.rs:644-654` — rename has no archive guard.

Observed behavior:
- Archive is a list filter plus an unload. Knowing the session id (or loading it) reactivates a live agent **without clearing `archived_at`**. The session can stay on the archived tab while running.

Expected boundary:
- Either archive is terminal until unarchive (prompt/mode/rename rejected), or first successful load/prompt is an unarchive (clear `archived_at`) so list state matches liveness.

Failure mechanism:
- `archived_at` is not consulted at the mutation/activation boundary.

Break-it angle:
- Archive session S; call `session/prompt` or `session/set_mode` with S’s id — agent returns, tools can run, UI still lists S as archived.

Impact:
- Ambiguous lifecycle (STT-005): archived ≠ inactive. Operators believe archive froze the session.

Operational impact:
- Blast radius: Workflow
- Side-effect class: process / DB
- Reversibility: compensatable (re-archive)
- Operator visibility: silent relative to archive tab
- Rerun safety: unsafe

Adjacent failure modes:
- TMP-008 lifecycle drift (archived_at vs live agent).
- Unarchive is the only explicit return path (`manage_sessions.rs:675-685`).

Recommended mitigation:
- Remediation patterns: gate at `get_session_agent` / `on_prompt`.
- Minimal repair: if `session.archived_at.is_some()`, return invalid_params unless the request is unarchive.
- Alternative: treat prompt as unarchive (clear `archived_at` in the same tx as activate).
- Behavior test: prompt on archived id is rejected or visibly unarchives.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: tests, ACP docs
- Nominal implementation agent: codex
- Rationale: one precondition at the existing lookup.

Validation:
- Test: archive then prompt → rejected or `archived_at` cleared.
- Test: unarchive then prompt still works.

Non-goals:
- Do not change archive UI tabs.

---

### STT-GOS-003: CLI session `/mode` writes the process-global default

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: State-Transition

Evidence:
- `crates/gosling-cli/src/session/mod.rs:788-804` — `handle_gosling_mode` calls `update_gosling_mode` **and** `config.set_gosling_mode(mode)`.
- `crates/gosling/src/agents/agent.rs:3304-3354` — session persist + provider update + in-memory; no global write.
- `crates/gosling/src/acp/server.rs:3334-3348` — ACP `on_set_mode` only updates the session agent (sibling; safer).
- `crates/gosling-cli/src/cli.rs:1711-1712` — new sessions read `Config::global().get_gosling_mode()`.

Observed behavior:
- Changing mode for the current CLI session changes `config.yaml` so the next `gosling` session inherits it.

Expected boundary:
- A session mode transition is session-scoped unless the operator is in a configure/default command.

Failure mechanism:
- CLI mixes session mutation and default-config mutation in one handler.

Break-it angle:
- Session A is Approve; `/mode auto`; start session B — B is Auto without an explicit choice.

Impact:
- Cross-scope transition (STT-006) and wrong-layer mutation (STT-009). Safer Chat in one session can become Auto for the next.

Operational impact:
- Blast radius: Workflow
- Side-effect class: file (config.yaml)
- Reversibility: compensatable
- Operator visibility: log-only (“Gosling mode set to …”)
- Rerun safety: unsafe

Adjacent failure modes:
- Plan-mode block at `session/mod.rs:1080-1082` also writes Auto globally.

Recommended mitigation:
- Remediation patterns: split session vs default.
- Minimal repair: drop `config.set_gosling_mode` from `handle_gosling_mode`; keep it on `configure`.
- Behavior test: `/mode chat` does not change `get_gosling_mode()` for a new session.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: tests, CLI help text
- Nominal implementation agent: codex
- Rationale: delete one call; add a configure-only path if missing.

Validation:
- Test: session mode change does not alter global default.
- Test: `gosling configure` still persists default.

Non-goals:
- Do not change ACP mode handling.

---

### STT-GOS-004 / IOP-GOS-004: JSON and Nostr imports are not replay-safe

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: State-Transition / Input-Output-Path

Evidence:
- `crates/gosling/src/session/session_manager.rs:939-982` — `import_session_file` dedupes on `source_sha256` / `source_path`.
- `crates/gosling/src/session/session_manager.rs:916-932` and `4792-4876` — `import_session` always `create_session_in_tx` (new id).
- `crates/gosling/src/acp/server/manage_sessions.rs:331-360` — ACP JSON/Nostr call `import_session`, not `import_session_file`.
- `crates/gosling-cli/src/commands/session.rs:324-335` — Nostr CLI uses `import_session`.
- `ui/desktop/src/components/sessions/SessionListView.tsx:118` — desktop native picker sends **contents** as JSON (loses file provenance).

Observed behavior:
- Re-importing the same JSON or the same Nostr link creates another session. Re-importing the same file via CLI file path does not.

Expected boundary:
- Same payload + same transport should be idempotent, or the operator must opt in to duplicate.

Failure mechanism:
- Provenance fingerprint is only attached on the file convenience API.

Break-it angle:
- Import a Nostr link twice, or use the desktop picker twice on the same file — two sessions.

Impact:
- Duplicate canonical transcripts; operator confusion; doubled history in search.

Operational impact:
- Blast radius: Workflow
- Side-effect class: DB
- Reversibility: compensatable (delete extra)
- Operator visibility: UI-visible (new row)
- Rerun safety: unsafe

Adjacent failure modes:
- IOP-015 parity: CLI file vs ACP/UI JSON.
- TMP-004 replay.

Recommended mitigation:
- Remediation patterns: hash the normalized payload in `import_session` and reuse `AlreadyImported`.
- Minimal repair: compute sha256 of `normalized` JSON inside `import_session` for all transports.
- Behavior test: two JSON imports of the same bytes → one session.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: tests
- Nominal implementation agent: codex
- Rationale: extend the existing file-import fingerprint.

Validation:
- Test: ACP JSON replay returns the same id.
- Test: Nostr replay returns the same id.
- Test: changed bytes still create a new session (or SourceChanged).

Non-goals:
- Do not make copy_session idempotent.

---

### STT-GOS-005: Permission persist can fail while in-memory policy advances

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: State-Transition

Evidence:
- `crates/gosling/src/config/permission.rs:97-119` — `persist` logs IO errors and keeps the in-memory map.
- `crates/gosling/src/config/permission.rs:228-258` — `update_permission` mutates the map then `persist()`.
- Contrast `remove_extension` at `permission.rs:261-295` — rolls back the map if persist fails.

Observed behavior:
- A user or SmartApprove cache write can be live for this process and gone after restart. `remove_extension` is the only rollback-on-persist-fail path.

Expected boundary:
- A reported permission change is durable, or the in-memory map rolls back and the caller sees an error.

Failure mechanism:
- Persist is best-effort on the hot update path; success is implied.

Break-it angle:
- Fill the filesystem / make `permission.yaml` unwritable; approve AlwaysAllow; restart — policy reverts while this process already auto-ran the tool.

Impact:
- Non-durable transition (STT-011). Auto/SmartApprove decisions diverge across restart.

Operational impact:
- Blast radius: Service
- Side-effect class: file
- Reversibility: compensatable
- Operator visibility: log-only
- Rerun safety: unknown

Adjacent failure modes:
- SmartApprove writes AskBefore for non-readonly tools (`permission_inspector.rs:230-234`) through the same persist.

Recommended mitigation:
- Remediation patterns: same rollback as `remove_extension`.
- Minimal repair: return `Result` from `update_permission` and revert on persist fail.
- Behavior test: persist failure leaves get_* unchanged.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: tests, caller Result plumbing
- Nominal implementation agent: codex
- Rationale: clone of existing rollback.

Validation:
- Test: unwritable config path + update → memory unchanged + error.

Non-goals:
- Do not change annotation tighten semantics.

---

### IOP-GOS-001: Foreign JSONL importers drop unknown events without a completeness flag

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Input-Output-Path

Evidence:
- `crates/gosling/src/session/import_formats/mod.rs:120-129` — malformed JSON lines now **fail** the import (stale 2026-07 finding is closed).
- `crates/gosling/src/session/import_formats/claude_code.rs:56-80` — only `user` / `assistant` (and title harvest) become messages; other valid types are skipped.
- `crates/gosling/src/session/import_formats/pi.rs:73-78` — non-`message` entries `continue`.
- `crates/gosling/src/session/import_formats/codex.rs:75+` — `event_msg` used only for usage; other types skipped.
- Import then prints success (`commands/session.rs:336-337`, desktop toast `importSuccess`).

Observed behavior:
- A well-formed transcript that is mostly hooks/queue/compaction lines imports as a short conversation with no “N events ignored” signal.

Expected boundary:
- Partial semantic conversion is visible (count, or refuse if zero messages when events existed).

Failure mechanism:
- Parse success is treated as a complete conversation.

Break-it angle:
- Claude Code file with 100 hook lines and 1 user line → “Session imported” with one message.

Impact:
- Operator resumes a hollow history (IOP-012).

Operational impact:
- Blast radius: Workflow
- Side-effect class: DB
- Reversibility: reversible (re-import after fix)
- Operator visibility: silent
- Rerun safety: safe (file path) / unsafe (JSON path, STT-GOS-004)

Adjacent failure modes:
- STT-GOS-004 duplicate if they retry via JSON.

Recommended mitigation:
- Remediation patterns: return skipped/kept counts on convert.
- Minimal repair: fail if `messages.is_empty()` after non-empty input (Pi/Codex already have empty-file errors; Claude Code similar). Surface skipped-type count in provenance.
- Behavior test: hook-only JSONL is not reported as a full conversation.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: tests, CLI copy
- Nominal implementation agent: codex
- Rationale: counters in existing converters.

Validation:
- Test: unknown-type-only file errors or flagged incomplete.
- Test: mixed file still imports user/assistant lines.

Non-goals:
- Do not import hook payloads as messages.

---

### IOP-GOS-002: Plugin `git clone` accepts any source after `--`

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Input-Output-Path

Evidence:
- `crates/gosling/src/plugins/mod.rs:134-140` — empty check only, then `clone_git_repo`.
- `crates/gosling/src/plugins/mod.rs:292-304` — `git clone --depth 1 -- <source> <dest>` (flag injection via `--upload-pack=` is **held**).
- Tests clone a **local path** (`plugins/mod.rs:479-486`).
- `enabled_plugin_mcp_servers` turns plugin `.mcp.json` `command` into `ExtensionConfig::Stdio` (`mcp_servers.rs:134-164`). User-scope plugins default enabled (`discovery.rs:126-132`).

Observed behavior:
- `source` may be `https://`, `file://`, a filesystem path, or any git transport git will accept after `--`. Install is an operator action; user-scope plugins then auto-enable.

Expected boundary:
- At least reject `ext::` / `-` / unexpected schemes if the product intent is “HTTPS git URL”. Local path can stay as an explicit `--path` flag.

Failure mechanism:
- Quoting `--` fixed option injection; it did not add a source allowlist.

Break-it angle:
- `gosling plugin install /tmp/evil-plugin` or `file:///tmp/evil` clones and, if it has a valid manifest + `.mcp.json`, user-scope enable launches `command`.

Impact:
- Operator-gated supply chain. Severity stays Low because install is explicit; still IOP-001 on the clone argument.

Operational impact:
- Blast radius: Local
- Side-effect class: process / file
- Reversibility: uninstall
- Operator visibility: CLI error/success
- Rerun safety: unsafe

Adjacent failure modes:
- TMP-GOS-001 auto-update reclones `metadata.source` with the same function.

Recommended mitigation:
- Remediation patterns: allowlist `https`/`ssh`/`git@` and a separate local-path API.
- Minimal repair: reject sources starting with `-` or `ext::`.
- Behavior test: `ext::` and `--upload-pack=` fail; `https://` still works.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: tests
- Nominal implementation agent: codex
- Rationale: validate before `Command`.

Validation:
- Test: `--upload-pack=...` is not a flag (already implied by `--`).
- Test: `ext::sh -c` rejected if you add the allowlist.

Non-goals:
- Do not disable local-path installs used in tests without a replacement flag.

---

### IOP-GOS-003: Session export is unredacted and CLI overwrite is unconditional

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Input-Output-Path

Evidence:
- `crates/gosling/src/session/session_manager.rs:4787-4789` — `serde_json::to_string_pretty(&session)` of the full `Session` (conversation, extension_data, workspace/credential ids).
- `crates/gosling-cli/src/commands/session.rs:247-294` — JSON/YAML/markdown; `fs::write` truncate; Nostr publishes that JSON (encrypted).
- Contrast diagnostics: `commands/session.rs:413-421` uses 0o600 and warns about contents.
- Workspace export **does** run `reject_secret_shaped_value` (`workspace/service.rs:235-247`).

Observed behavior:
- Session export includes whatever the conversation captured (tokens pasted into chat, tool results). CLI `--output` overwrites without `O_EXCL`.

Expected boundary:
- Either redact secret-shaped keys (as workspace export does) or require `--include-secrets` and exclusive-create / confirm overwrite.

Failure mechanism:
- Export is a raw serialize; diagnostics learned the warning, session export did not.

Break-it angle:
- Export a session that contains an API key in a user message to a shared path; or `gosling session export --output ~/important.json` clobbers the file.

Impact:
- IOP-009 leakage + IOP-011 overwrite. Nostr is encrypted but the local file is not.

Operational impact:
- Blast radius: Local
- Side-effect class: file / network (Nostr ciphertext of full session)
- Reversibility: overwrite irreversible
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- ACP `on_export_session` (`manage_sessions.rs:319-328`) same payload.

Recommended mitigation:
- Remediation patterns: workspace-style secret walk + exclusive create or confirm.
- Minimal repair: document + refuse overwrite unless `--force`; 0o600 like diagnostics.
- Behavior test: existing output file without `--force` is unchanged.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: tests, CLI flag
- Nominal implementation agent: codex
- Rationale: match diagnostics writer.

Validation:
- Test: `--output` existing file fails without `--force`.
- Test: exported JSON still imports.

Non-goals:
- Do not encrypt local JSON exports.

---

### TMP-GOS-001: Plugin auto-update stamps `last_update_check` before the update runs

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Temporal

Evidence:
- `crates/gosling/src/plugins/mod.rs:190-191`:
  ```
  let result = mark_last_update_check(&plugin_dir, now)
      .and_then(|_| update_plugin_at_root(...));
  ```
- `crates/gosling/src/plugins/mod.rs:201-204` — next run skipped for 24 hours after any stamp.
- `installed_plugin_skill_dirs` (`plugins/mod.rs:85-94`) swallows update errors and uses the old tree.

Observed behavior:
- A failed clone/network/name-mismatch still counts as “checked”. The plugin stays stale for 24h.

Expected boundary:
- Stamp only after a successful update, or on a dedicated “checked and failed, retry sooner” field.

Failure mechanism:
- Check time is used as a success/freshness token (STT-012 / TMP-011).

Break-it angle:
- Enable auto-update, fail DNS on the first skill-list, wait — no retry until `AUTO_UPDATE_INTERVAL_HOURS`.

Impact:
- Operators run yesterday’s plugin after a transient failure.

Operational impact:
- Blast radius: Local
- Side-effect class: file
- Reversibility: manual `plugin update`
- Operator visibility: log-only warning
- Rerun safety: safe (manual update)

Adjacent failure modes:
- Same `clone_git_repo` as IOP-GOS-002.

Recommended mitigation:
- Remediation patterns: stamp after `Ok`.
- Minimal repair: swap order; on `Err`, do not write `last_update_check` (or write a shorter backoff).
- Behavior test: failed update leaves `last_update_check` unset/old so the next list retries.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: tests
- Nominal implementation agent: codex
- Rationale: one call order change.

Validation:
- Test: forced update error → `should_auto_update` still true immediately.

Non-goals:
- Do not change the 24h success interval.

---

### TMP-GOS-002: `GOSLING_PATH_ROOT` does not isolate plugin settings with the rest of config

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Temporal / Input-Output-Path

Evidence:
- `crates/gosling/src/config/paths.rs:8-16` — with `GOSLING_PATH_ROOT`, config is `$ROOT/config`.
- `crates/gosling/src/plugins/discovery.rs:282-289` — plugin `settings.json` is `$ROOT/.config/gosling/settings.json`.
- `crates/gosling-cli/tests/mcp_command_test.rs:482-489` — documents the invariant that `GOSLING_PATH_ROOT` “must fully control where `config.yaml` lands” (`$ROOT/config/config.yaml`).
- Desktop `ui/desktop/src/main.ts:1008-1100` — expands and forwards `GOSLING_PATH_ROOT` to the child.

Observed behavior:
- Under PATH_ROOT, `config.yaml` / `permission.yaml` live in `$ROOT/config`, but Open Plugins user settings live in `$ROOT/.config/gosling/settings.json`. Without PATH_ROOT both happen to sit under `~/.config/gosling/` (except plugins themselves, which are `~/.agents/plugins`).

Expected boundary:
- One root means one tree. Plugin settings should be `Paths::in_config_dir("settings.json")` or equivalent.

Failure mechanism:
- Two independent PATH_ROOT layouts; tests only assert `config/config.yaml`.

Break-it angle:
- Isolate a test or portable install with PATH_ROOT; drop plugin settings in the “obvious” `$ROOT/config` — discovery never reads them. Or leak settings into `$ROOT/.config` while believing the install is contained in `config/`.

Impact:
- Isolation lie; plugin enable/disable/trust can be read from or written outside the documented config dir.

Operational impact:
- Blast radius: Local
- Side-effect class: file
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: unknown

Adjacent failure modes:
- TMP-GOS-003 plugins_dir layout.
- `gosling-mcp` cache uses `$ROOT/cache` (`crates/gosling-mcp/src/lib.rs:16-18`) — a third layout.

Recommended mitigation:
- Remediation patterns: route plugin settings through `Paths::config_dir()`.
- Minimal repair: `user_settings_path()` → `Paths::in_config_dir("settings.json")`.
- Behavior test: with PATH_ROOT, discovery reads/writes only under `$ROOT/config`.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: tests, migration of existing `$ROOT/.config` files
- Nominal implementation agent: claude
- Rationale: path-contract change plus compatibility read.

Validation:
- Test: `GOSLING_PATH_ROOT` + plugin settings only honored from `config/`.
- Test: no write under `$ROOT/.config/gosling` in new installs.

Non-goals:
- Do not change Goose catalog URLs.

---

### TMP-GOS-003: `plugins_dir` and `agents_dir` ignore `RUNTIME_PATHS` shell isolation

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Temporal

Evidence:
- `crates/gosling/src/config/paths.rs:40-56` — `config_dir`/`data_dir`/`state_dir` prefer `RUNTIME_PATHS`.
- `crates/gosling/src/config/paths.rs:58-67` — `plugins_dir`/`agents_dir` always call `get_dir`.
- `crates/gosling/src/config/paths.rs:112-118` — shell namespaces isolate **data/state only**; config is shared.
- ACP constructs a **dedicated** `PermissionManager::new(options.config_dir)` (`acp/server.rs:1158-1164`) but `PermissionManager::instance()` is a process-wide `LazyLock` on first `Paths::config_dir()` (`permission.rs:13-14`, `78-80`).
- `Agent::new()` (`agent.rs:400-404`) uses that singleton.

Observed behavior:
- Shell/ACP namespaces do not get their own plugin or agent trees. CLI agents share one `permission.yaml` bound at first `instance()` call, which may predate `Paths::scope`.

Expected boundary:
- Anything that `GOSLING_PATH_ROOT` / `Paths::scope` claims to isolate is either scoped or documented as shared. Plugins and the permission singleton should follow the same config_dir as `Config::global()` (which **is** keyed by `Paths::config_dir()`, `base.rs:537-549`).

Failure mechanism:
- Incomplete scope: config/data/state were scoped later; plugins/agents/permission singleton were not.

Break-it angle:
- Two shell namespaces, install a plugin in one — both see `plugin_install_dir()`. First CLI `PermissionManager::instance()` outside a scope pins the manager to the unscope path.

Impact:
- Cross-namespace plugin/MCP bleed; permission file may not be the scoped config dir.

Operational impact:
- Blast radius: Service
- Side-effect class: file / process
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unknown

Adjacent failure modes:
- TMP-GOS-002 settings path.
- Comment at `acp/server.rs:1131` already notes `Paths::in_state_dir` ignoring scoped data_dir (RequestLog).

Recommended mitigation:
- Remediation patterns: make `plugins_dir`/`agents_dir` task-local or document shared-by-design.
- Minimal repair: if shared-by-design, document; if not, add them to `RuntimePaths`.
- Key `PermissionManager` like `Config::global()` by config_dir.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: runtime_paths, tests
- Nominal implementation agent: claude
- Rationale: touches path contract + singleton.

Validation:
- Test: two `Paths::scope` tasks do not share plugin writes if isolation is required.
- Test: `PermissionManager` used by ACP is the one constructed with `options.config_dir`.

Non-goals:
- Do not invent a second plugin marketplace.

---

### TMP-GOS-004: Codex JWT verification can fall through to an unverified parse

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Temporal

Evidence:
- `crates/gosling/src/providers/chatgpt_codex.rs:559-570` — after JWKS fetch/refresh, any verify failure calls `parse_jwt_claims_unverified`.
- `crates/gosling/src/providers/chatgpt_codex.rs:548-556` — unverified path base64-decodes the payload with no signature check.
- Kid-miss **does** refresh (`507-522`, `561-562`) — stale 2026-07 “never refresh JWKS” finding is closed.

Observed behavior:
- `chatgpt_account_id` can be taken from an unauthenticated JWT if verify fails for any reason (wrong alg, bad sig, refresh JWKS still missing kid).

Expected boundary:
- Fail closed for account binding if signature verification fails.

Failure mechanism:
- Robustness fallback treats parse success as authority.

Break-it angle:
- Feed an unsigned/forged id_token with a `chatgpt_account_id` claim after forcing verify to fail.

Impact:
- Account id used for routing/headers may not be issuer-authenticated. Access token path still uses OAuth refresh (`958-961`).

Operational impact:
- Blast radius: Local
- Side-effect class: network (wrong account header)
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unknown

Adjacent failure modes:
- Escalate to Security if that claim gates authorization.

Recommended mitigation:
- Remediation patterns: remove unverified fallback or limit it to logging.
- Minimal repair: return `None` when `parse_jwt_claims_with_jwks` fails.
- Behavior test: tampered JWT yields no account id.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: tests
- Nominal implementation agent: codex
- Rationale: delete fallback.

Validation:
- Test: valid signed JWT still extracts account id.
- Test: mutated payload does not.

Non-goals:
- Do not change access-token refresh.

---

### TMP-GOS-005: Workspace folder policy snapshot is pinned and not refreshed

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Temporal

Evidence:
- `crates/gosling/src/session/session_manager.rs:479-508` — `workspace_snapshot` stores context and `effective_folder_policy()`; comment says restriction flag stays opt-in.
- `crates/gosling/src/acp/server/manage_sessions.rs:702-710` — mutating folder policy on a workspace session is rejected: “edit the workspace and start a new session”.
- `crates/gosling/src/session/session_manager.rs:1268-1282` — load re-derives policy from the **stored** context, not from `WorkspaceService`.
- `crates/gosling/src/workspace/service.rs:934-976` — deleting a workspace **keeps** pinned session `workspace_id`.

Observed behavior:
- Tightening a workspace’s read-only roots does not affect existing sessions. Deleted workspaces leave live pinned sessions.

Expected boundary:
- Either refresh-from-workspace at consume, or stamp snapshot generation and refuse tools if the workspace is gone/newer.

Failure mechanism:
- Snapshot is the authority (STT-012 / TMP-001). Product comment treats this as intentional.

Break-it angle:
- Start session; change workspace to add a read-only root; existing session still mutates that path (inspector uses old policy).

Impact:
- Stale path policy. Severity Low because the code **documents** the pin; still a freshness gap.

Operational impact:
- Blast radius: Workflow
- Side-effect class: file
- Reversibility: start new session
- Operator visibility: error only if they try to edit folders on the session
- Rerun safety: unsafe (old policy persists)

Adjacent failure modes:
- TMP-007 TOCTOU on the pinned roots themselves.

Recommended mitigation:
- Remediation patterns: compare workspace `updated_at` at inspect time, or attach `snapshot_generation`.
- Minimal repair: if workspace was deleted, fail inspect closed.
- Behavior test: deleted workspace → inspector Deny.

Implementation assessment:
- Complexity: governance_decision
- Cost: S
- Cost drivers: product choice
- Nominal implementation agent: human-owner
- Rationale: pin vs live is a product invariant.

Validation:
- Test: workspace delete + existing session tool path denied or session marked detached.

Non-goals:
- Do not silently rewrite historical sessions’ folders.

---

### TMP-GOS-006: Config.yaml migrations are unversioned; read path skips provider migration

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Temporal

Evidence:
- `crates/gosling/src/config/migrations.rs:11-23` — `run_migrations` = platform extensions + providers; `run_read_migrations` = platform extensions **only**.
- `crates/gosling/src/config/base.rs:665-668` — write-load migrates and **warns** if save fails, then returns the in-memory mapping anyway.
- `crates/gosling/src/config/base.rs:712` — ordinary `load()` never persists and never runs provider migration.
- Session DB contrast: `session_manager.rs:33` `CURRENT_SCHEMA_VERSION = 26`; `1832-1854` sequential versions in one `BEGIN IMMEDIATE`.

Observed behavior:
- Flat `GOSLING_PROVIDER` keys remain on disk until a write path runs. In-memory write mapping may be migrated when disk is not. No `config_schema_version` pin.

Expected boundary:
- Schema-like rewrites are versioned, persisted, and identical on read and write, or leftover keys stay readable **and** documented as dual-read forever.

Failure mechanism:
- Heuristic migrate + fail-open save + read/write split.

Break-it angle:
- Read-only config dir: process sees migrated providers in a write, save fails, next process still has flat keys; `get_param("GOSLING_PROVIDER")` still works only because the read path left them.

Impact:
- Mixed-era config (TMP-009-adjacent). Low because dual-read is intentional for `get_param`.

Operational impact:
- Blast radius: Local
- Side-effect class: file
- Reversibility: compensatable
- Operator visibility: log-only
- Rerun safety: safe (idempotent when save works)

Adjacent failure modes:
- STT-GOS-005 same fail-open persist pattern.

Recommended mitigation:
- Remediation patterns: add `config_version`; fail the write if migrate-save fails.
- Minimal repair: if `save_values` after migrate fails, return the error from `load_write_config`.
- Behavior test: unwritable config → write API errors, disk unchanged.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: tests, dual-read window
- Nominal implementation agent: gpt
- Rationale: schema-ish config.

Validation:
- Test: migrate + save fail does not apply the caller’s subsequent write on a half-migrated mental model.

Non-goals:
- Do not drop `get_param` dual-read in the same slice.

---

### TMP-GOS-007: Working-dir path policy is check-then-use

Severity: Medium  
Confidence: Likely  
Evidence basis: simulation-reasoned  
Domain: Temporal

Evidence:
- `crates/gosling/src/permission/working_dir_scope_inspector.rs:148-186` — `canonicalize_potential_path` then later the tool executes in another component.
- `crates/gosling/src/permission/working_dir_scope_inspector.rs:194-202` — containment is `canonical_path.starts_with(dir)` (Rust `Path::starts_with` is component-aware).
- `working_dir_scope_inspector.rs:162-167` — dangling symlink through the path is refused.
- No lock/re-validate at `dispatch_tool_call`.

Observed behavior:
- A path that is inside a root at inspect time can be replaced (symlink retarget, mount) before the tool runs.

Expected boundary:
- Re-validate at use, or open with `O_NOFOLLOW` / refuse symlink components.

Failure mechanism:
- Classic TOCTOU between inspector and extension process.

Break-it angle:
- Race a symlink under the working dir to `$HOME` between inspect Allow and write.

Impact:
- Path policy bypass under a concurrent writer. Runtime manifestation is **Likely**, not Confirmed (no drill).

Operational impact:
- Blast radius: Local
- Side-effect class: file
- Reversibility: irreversible (write)
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- Concurrency lens; workspace snapshot pin (TMP-GOS-005).

Recommended mitigation:
- Remediation patterns: refuse symlink components at inspect; re-canonicalize immediately before dispatch.
- Minimal repair: treat any symlink in the resolved chain as RequireApproval/Deny.
- Behavior test: symlink-to-outside is denied (may already hold for dangling; add live retarget if authorized).

Implementation assessment:
- Complexity: persistence_recovery
- Cost: M
- Cost drivers: tests, OS symlink cases
- Nominal implementation agent: codex
- Rationale: TOCTOU needs a use-site guard.

Validation:
- Test: symlink escape denied.
- Authorized drill: replace symlink between inspect and use.

Non-goals:
- Do not disable all symlinks inside a repo without product OK.

---

## Non-Findings / Checked But Not Confirmed

| Check | Why it held | Line |
|---|---|---|
| Session import size cap | 16 MiB before materialize (metadata + `take`) and again in convert | `import_formats/mod.rs:70-117`, `265` |
| Malformed JSONL | `parse_json_lines` errors on the exact line | `import_formats/mod.rs:120-129` |
| Import working dir | absolute + canonicalize + is_dir | `import_formats/mod.rs:80-94` |
| Import executable extensions | `EnabledExtensionsState` removed; test expects none | `session_manager.rs:4809-4812`, `7117-7122` |
| Import mode quarantine | always `GoslingMode::Approve` regardless of JSON `auto` | `session_manager.rs:4845-4851`, `7105` |
| Import history trust | `with_imported_untrusted`; `history_trusted: false` | `session_manager.rs:4813-4833` |
| Import atomicity | create + metadata + conversation in one `BEGIN IMMEDIATE` | `session_manager.rs:4837-4872` |
| File import idempotency | sha256 / source_path | `session_manager.rs:953-968`, test `7530-7598` |
| Foreign `cwd` not used as live root | operator `working_dir` wins; original stored in provenance | `session_manager.rs:4804-4818` |
| Plugin git flag injection | `--` before source | `plugins/mod.rs:297-300` |
| Plugin name traversal | charset + no `..` | `open_plugins.rs:203-237` |
| Plugin relative paths | must start `./`, no `ParentDir` | `open_plugins.rs:388-406` |
| Plugin copy symlink slip | `copy_dir_all` only copies `is_dir` / `is_file` from `file_type()` (no follow) | `plugins/mod.rs:375-388` |
| Workspace path traversal | parent components rejected; must be absolute | `validation.rs:8-18`, tests `370-372` |
| Workspace secret export/import | `reject_secret_shaped_value` | `service.rs:235-247`, `679-708` |
| Workspace import newer schema | rejected | `service.rs:249-251` |
| Workspace import new identity | `create` assigns `Uuid::now_v7` | `service.rs:114-131` |
| Path policy `starts_with` | Rust path component prefix | `working_dir_scope_inspector.rs:200-202` |
| Path policy dangling symlink | bail | `working_dir_scope_inspector.rs:162-167` |
| User NeverAllow in Auto | still Deny; AskBefore in Auto Denies | `permission_inspector.rs:133-156` |
| MCP annotations cannot grant | only `read_only_hint == false` tightens to AskBefore | `permission.rs:142-160`, tests `452-467` |
| `update_gosling_mode` rollback | provider reject rolls session + provider back | `agent.rs:3324-3348` |
| Runtime namespace validation | rejects `../escape` | `paths.rs:122-132`, test `162` |
| Desktop import bound | picker uses `readBoundedSessionImportFile` | `main.ts:2358-2375`, `sessionImport.ts:5-29` |
| Desktop renderer fallback size | `file.size > MAX_SESSION_IMPORT_BYTES` | `SessionListView.tsx:140-142` |
| Nostr size | ciphertext + plaintext caps | `nostr_share.rs:164`, `243-248` |
| Gemini/xAI/Codex consume expiry | refresh if `expires_at` within skew | `gemini_oauth.rs:742-744`, `xai_oauth.rs:730-744`, `chatgpt_codex.rs:960-961` |
| xAI refresh single-flight | mutex + reload | `xai_oauth.rs:736-746` |
| Secret UI expiry vs refresh | `find_expires_at` returns None if `refresh_token` present (display Unknown, not consume-accept) | `provider_secrets.rs:78-87` |
| Session DB migrations | sequential, version table, one tx | `session_manager.rs:1832-1854` |
| Config provider migrate idempotent | second run no-op | `migrations.rs:443-458` |
| Platform extension migrate preserves enabled | | `migrations.rs:284-316` |
| Chat `dispatch_app_tool_call` | needs_approval default blocks app clients | `agent.rs:1145-1155` + inspector `continue` |
| Diagnostics 0o600 | even when overwriting | `commands/session.rs:413-440` |
| Import does not copy workspace snapshot / extra dirs / provider | quarantine | `import_session` vs `copy_session` `4908-4935` |
| `GOSLING_PATH_ROOT` forwarded to MCP children | | `extension_manager.rs:399-403` |
| Shell config isolation test | `mcp install` writes `$ROOT/config/config.yaml` | `mcp_command_test.rs:482-489` |

Stale historical items **re-checked and rejected**:
- Silent `filter_map(...ok())` JSONL drop — replaced by `parse_json_lines`.
- `read_only_hint: true` auto-approves — annotations cannot grant.
- Session import no size cap — 16 MiB everywhere I traced.
- JWKS never refreshed — kid-miss now refreshes.

---

## Break-It Review

| Attack | Result |
|---|---|
| JSONL truncated mid-line | **Held** — parse error on that line |
| JSONL unknown event types | **Fails** IOP-GOS-001 — success with fewer messages |
| Import stdio extension | **Held** — stripped; test `test_export_import_roundtrip` |
| Import `gosling_mode: auto` | **Held** — created as Approve |
| Import relative working dir | **Held** |
| Import 16 MiB + 1 | **Held** (source) |
| Replay same file | **Held** (CLI file) |
| Replay same JSON/Nostr | **Fails** STT-GOS-004 |
| `git clone --upload-pack=` as source | **Held** (`--`) |
| Plugin path `../` | **Held** |
| Plugin symlink copy | **Held** (skipped) |
| Workspace `..` path | **Held** |
| Workspace `apiKey` field | **Held** |
| Chat + backend tool | **Held** (skipped) |
| Chat + frontend tool | **Fails** STT-GOS-001 |
| Archive then prompt | **Fails** STT-GOS-002 |
| CLI `/mode` then new session | **Fails** STT-GOS-003 |
| Expired Gemini/xAI/Codex access | **Held** at consume (refresh) |
| Forged Codex JWT account claim | **Fails** TMP-GOS-004 if verify errors |
| PATH_ROOT plugin settings in `config/` | **Fails** TMP-GOS-002 |
| Shell namespace plugin isolation | **Fails** TMP-GOS-003 |
| Auto-update network fail | **Fails** TMP-GOS-001 (24h silence) |
| Symlink race after inspect | **Likely** TMP-GOS-007 — not drilled |
| Config migrate + read-only disk | **Fails** TMP-GOS-006 / STT-GOS-005 pattern |

---

## Recommended Patch Order

1. STT-GOS-001 Chat frontend skip + inspector Deny in Chat.
2. STT-GOS-002 archive gate or auto-unarchive at activate.
3. STT-GOS-003 stop CLI `/mode` from writing global default.
4. TMP-GOS-001 stamp auto-update only on success.
5. STT-GOS-005 permission persist rollback.
6. STT-GOS-004 / IOP-GOS-004 payload-hash idempotency for all import transports.
7. TMP-GOS-002 / TMP-GOS-003 path-root + RUNTIME_PATHS contract.
8. IOP-GOS-001 skipped-event counts; IOP-GOS-003 export overwrite; TMP-GOS-004 drop unverified JWT; TMP-GOS-006 fail write if migrate save fails.
9. Product decision on TMP-GOS-005 snapshot freshness; TMP-GOS-007 symlink policy.

---

## Regression Test Strategy

| Test | Purpose | Finding |
|---|---|---|
| Chat + frontend tool not dispatched | Chat contract | STT-GOS-001 |
| Prompt archived session rejected or unarchives | Archive terminal/consistent | STT-GOS-002 |
| CLI mode change does not alter `get_gosling_mode()` | Session vs default | STT-GOS-003 |
| JSON import twice → one session | Replay | STT-GOS-004 |
| Permission persist fail rolls back | Durable policy | STT-GOS-005 |
| Failed plugin auto-update retries immediately | Freshness | TMP-GOS-001 |
| PATH_ROOT plugin settings only under `config/` | Isolation | TMP-GOS-002 |
| Tampered Codex JWT yields no account id | Expired/forged authority | TMP-GOS-004 |
| Hook-only Claude JSONL not silent-complete | Completeness | IOP-GOS-001 |
| Existing export `--output` not clobbered | Overwrite | IOP-GOS-003 |

Oracle note: those tests should include at least one **fresh-process** CLI `session import` / `session export` round-trip. I did not run it. Do not treat existing `#[tokio::test]` green as proof until the fixture’s PATH_ROOT / pragma isolation is checked (`evidence_discipline.md` Oracle Integrity).

---

## Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| STT-GOS-001 | State-Transition | Workflow-GUI | Chat UI still shows frontend tool cards |
| STT-GOS-002 | State-Transition | Workflow-GUI / Temporal | Archive tab vs live agent |
| STT-GOS-003 | State-Transition | Negative-Space | Next CLI session inherits mode |
| STT-GOS-004 / IOP-GOS-004 | State-Transition | Input-Output-Path / Temporal | Replay + CLI/UI parity |
| STT-GOS-005 | State-Transition | Reliability | Persist fail-open |
| IOP-GOS-001 | Input-Output-Path | Workflow-GUI | Success toast on partial history |
| IOP-GOS-002 | Input-Output-Path | Security | Plugin MCP spawn |
| IOP-GOS-003 | Input-Output-Path | Security | Conversation secrets |
| TMP-GOS-001 | Temporal | Reliability | Silent stale plugin |
| TMP-GOS-002 / 003 | Temporal | Architecture-Seam / Path-consistency | PATH_ROOT / shells |
| TMP-GOS-004 | Temporal | Security | Unverified JWT claims |
| TMP-GOS-005 | Temporal | State-Transition | Snapshot as authority |
| TMP-GOS-006 | Temporal | Reliability | Unversioned config rewrite |
| TMP-GOS-007 | Temporal | Concurrency / Security | Symlink TOCTOU |

---

## Deferred Risks

- Plugin `ext::` / local-path clone without an HTTPS-only product rule (IOP-GOS-002 stays Low).
- TMP-GOS-005 pin-vs-live is a product invariant until owners say otherwise.
- TMP-GOS-007 needs an authorized symlink drill to raise above Likely.
- MCP OAuth library `initialize_from_store` internals were not read (external `rmcp`); consume path here is refresh-or-clear (`oauth/mod.rs:93-108`).
- `expires_in.unwrap_or(3600)` on several providers: if a token is shorter than 3600s and the field is omitted, consume could use a near-expired token for up to the skew window — Speculative without a provider that omits `expires_in`.
- Nostr deeplink embeds the decryption key (`nostr_share.rs:252-257`) — by design for share-by-link; Security lens owns link leakage.

---

## Validation Limits

- No `cargo test`, no CLI fresh-process import/export, no hostile zip, no live OAuth, no symlink race drill.
- Not fully read: every provider beyond gemini/xAI/Codex/provider_secrets; HTTP session routes vs ACP (CLI + ACP + desktop covered); Ink TUI import.
- `RequestLog` / `Paths::in_state_dir` global vs scope (commented at `acp/server.rs:1131`) not deep-traced.
- `finding_schema.json` machine emit skipped (file not loaded).
- Absence of review on those items is a fact, not a clean bill.

---

## Residual Risk

Pinned workspace sessions, shared plugin trees across shells, and unredacted session JSON remain after the small Chat/archive/mode patches. Isolation of `GOSLING_PATH_ROOT` is real for `config/data/state` and incomplete for plugin settings and `plugins_dir`.

---

## Final Confidence

**Medium-High** for the named surfaces: mutation and consume sites were read at this HEAD with quoted lines. **Low** for runtime races (TMP-GOS-007) and unexecuted import/export oracles.

---

## v3.1 Calibration Addendum

Static review Confirmed missing Chat skip, missing archive precondition, global config write, persist fail-open, stamp-before-update, path-root mismatch, and unverified JWT fallback — these are code properties. TOCTOU and “expired token used in production” are not Confirmed as runtime events.

---

## Next Action

Patch STT-GOS-001 and STT-GOS-002 in one slice (Chat deny + archive gate), then STT-GOS-003 and TMP-GOS-001 (one-line / one-call fixes), and add a fresh-process `gosling session export | import` fixture before closing IOP import non-findings as test-reproduced.
