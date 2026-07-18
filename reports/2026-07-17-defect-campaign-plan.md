# Defect-repair campaign plan — 2026-07-17

Campaign scope: all 34 frozen findings in
`reports/2026-07-17-exhaustive-defect-audit-checkpoint.md`.

Baseline: `claude/gosling-defect-repair-followup-6093` at `cdf2634ae`, clean
and synchronized with `origin`. Git is available. The audit checkpoint was
pushed at the user's request; repair stages will be committed locally for
bisectability but not pushed without further authorization. Rust build, test,
and Clippy status is unknown because repository instructions permit those
commands only when the user explicitly requests building/testing. Required
formatting is `cargo fmt`; non-executing verification includes focused static
review and `git diff --check`.

## Inventory and touch sets

All entries are `in-scope`. Prior deferrals outside this frozen inventory,
including Tagteam workflow activation and ORCH-002, remain protected. AUD-031
was explicitly reopened by the user's request to patch every newly frozen
finding, but its stage retains the skill's stop condition if static design work
confirms that only a broad architecture rewrite can repair it safely.

| ID | Domain | Priority | Complexity | Group | Touch set / data path | Regression seam |
|---|---|---|---|---|---|---|
| AUD-001 | security | P0 | high | 3 | `SessionStorage::import_session`; native JSON -> `ExtensionData` -> resume loader | import containing Stdio/InlinePython persists no executable extension |
| AUD-002 | security | P0 | low | 3 | `SessionStorage::import_session`; serialized restriction flag -> session row | true/false restriction round trip |
| AUD-003 | security | P0 | medium | 2 | `PermissionManager::apply_tool_annotations`; MCP annotations -> permission store | `readOnlyHint=true` never grants `AlwaysAllow` |
| AUD-004 | security | P0 | medium | 2 | `PermissionInspector::inspect`; argument-aware judge -> name-only cache | benign then destructive arguments require a second decision |
| AUD-005 | security | P0 | low | 6 | `WorkingDirScopeInspector` -> `ToolInspectionManager` Auto-mode merge | out-of-root result remains approval-required in Auto |
| AUD-006 | security | P0 | medium | 6 | `resolve`/`is_within_any`/`out_of_scope_path`; path args and shell tokens | traversal and symlink fixtures are outside scope |
| AUD-007 | security | P0 | high | 6 | Gemini/Cursor/Tagteam command construction; Gosling mode -> CLI flags | unsupported modes fail closed; supported flags match mode |
| AUD-008 | security | P0 | high | 6 | provider capability -> restricted-session reply path | self-managing provider cannot run under unenforced restriction |
| AUD-009 | security | P0 | medium | 4 | `rehydrate_configured_envs`; client DTO -> stored extension secrets | different command/args/URI receives no stored env |
| AUD-010 | security | P0 | low | 4 | InlinePython `Command`; parent env -> MCP child | child command uses cleared minimal environment |
| AUD-011 | security | P0 | low | 1 | ChatGPT/Copilot/Kimi token cache save paths | caches use owner-only atomic secret writer |
| AUD-012 | security | P0 | low | 5 | assistant text -> ReactMarkdown image -> Electron network | remote Markdown image creates no fetchable `img` source |
| AUD-013 | security | P0 | medium | 5 | renderer IPC path -> lexical root -> filesystem sink | symlinked target/ancestor outside root is denied |
| AUD-014 | correctness | P0 | medium | 7 | cancel token -> session active-request registry -> next reply | cancelled request occupies slot until guard cleanup |
| AUD-015 | reliability | P1 | low | 7 | cancellation token vs provider `stream.next()` in ACP/subagent | never-yielding stream wakes on cancel |
| AUD-016 | reliability | P1 | high | 7 | tool cancellation/drop -> shell/process group ownership | descendant is killed on cancellation/drop |
| AUD-017 | reliability | P1 | medium | 8 | process signal -> HTTP/TLS shutdown -> SSE task | SSE exits on shutdown and TLS has a deadline |
| AUD-018 | reliability | P1 | medium | 8 | extension-loader map -> `JoinHandle` ownership; resume registration | replace/remove aborts; concurrent resume spawns once |
| AUD-019 | reliability | P1 | medium | 8 | Electron cleanup -> signal -> actual child exit -> registry | SIGKILL follows grace period and registry remains until exit |
| AUD-020 | reliability | P1 | low | 9 | MCP `list_tools` cursor loop | repeated cursor/page/item excess returns bounded error |
| AUD-021 | correctness | P1 | low | 9 | MCP resource/prompt pagination | two pages are completely exposed |
| AUD-022 | correctness | P0 | high | 6 | registry cwd-aware factory -> five CLI provider commands | fake CLI observes session cwd across construction paths |
| AUD-023 | reliability | P1 | low | 10 | Cursor child stderr pipe -> wait | stderr flood cannot block completion |
| AUD-024 | correctness | P1 | low | 10 | Cursor JSON/event parser -> assistant success/error | explicit/malformed/empty output is an error |
| AUD-025 | correctness | P1 | low | 10 | Codex exit/events -> accumulated partial response | terminal failure wins over partial text |
| AUD-026 | data integrity | P1 | medium | 11 | live provider/mode mutation -> SQLite apply | injected persistence failure preserves old live state |
| AUD-027 | data integrity | P1 | medium | 11 | ACP cwd request -> session row/provider/extensions | downstream failure rolls every root back |
| AUD-028 | correctness | P1 | low | 12 | review root resolution -> worker `Command` | nested invocation child observes repo root |
| AUD-029 | reliability | P1 | medium | 12 | concurrent review phases -> worker permits | peak combined workers never exceeds four |
| AUD-030 | reliability | P1 | medium | 12 | async delegate capacity check -> setup -> task insertion | barrier-start with cap one creates one task |
| AUD-031 | data integrity | P0 | high | 11 | tool dispatch side effect -> conversation persistence -> crash/resume | crash-point drill proves at-most-once side-effect identity |
| AUD-032 | reliability | P1 | low | 5 | WebSocket message -> ACP adapter queue -> SDK reader | queue applies a deterministic finite bound/close policy |
| AUD-033 | correctness | P1 | low | 1 | `DATABRICKS_HOST` -> `Url::parse`/`join` | invalid URL returns `Err`, never panics |
| AUD-034 | frontend/UX-bug | P2 | low | 13 | dynamic Ink text -> fixed-height screen budget | long strings are pre-truncated and use truncate wrapping |

## Locality groups and order

### Group 1 — credential persistence and OAuth input

- Defects: AUD-011 (P0/low), AUD-033 (P1/low).
- Files/functions: token cache `save` methods in `chatgpt_codex.rs`,
  `githubcopilot.rs`, `kimicode.rs`; `oauth.rs::get_workspace_endpoints`.
- Data paths: OAuth credentials to disk; configured host to endpoint URL.
- Modularization: none. Files over 1000 lines receive localized edits only;
  no multi-function structural change is needed.
- Regression surface: secret-writer permissions and invalid-host error tests.
- Grouping rationale: one bounded provider-authentication surface.
- Commit: `repair credential persistence and OAuth validation (AUD-011,AUD-033)`.

### Group 2 — SmartApprove authority

- Defects: AUD-003 (P0/medium), AUD-004 (P0/medium).
- Files/functions: `PermissionManager::apply_tool_annotations`,
  `PermissionInspector::inspect`, related tests.
- Data paths: untrusted MCP metadata and per-call LLM classification into durable
  authorization.
- Modularization: none; all files are at or below 1000 lines.
- Regression surface: hostile read-only hints and argument changes.
- Commit: `close SmartApprove authority bypasses (AUD-003,AUD-004)`.

### Group 3 — session import trust boundary

- Defects: AUD-001 (P0/high), AUD-002 (P0/low).
- Files/functions: `session_manager.rs::import_session`, extension-data import
  helpers and import tests.
- Data paths: native JSON to session metadata and automatic extension resume.
- Modularization: `session_manager.rs` is 5692 lines (>=2000); apply the
  smallest safe fix and route a dedicated modularization follow-up.
- Regression surface: executable-extension quarantine and policy round trip.
- Commit: `quarantine imported execution state (AUD-001,AUD-002)`.

### Group 4 — extension secret and environment boundaries

- Defects: AUD-009 (P0/medium), AUD-010 (P0/low).
- Files/functions: `rehydrate_configured_envs`, InlinePython command creation.
- Data paths: configured secrets to a client-selected endpoint; parent process
  env to extension child.
- Modularization: `acp/server.rs` and `extension_manager.rs` are >=2000 lines;
  use localized edits and route both for dedicated modularization.
- Regression surface: exact endpoint identity and child env allowlist.
- Commit: `bind extension secrets to trusted identities (AUD-009,AUD-010)`.

### Group 5 — desktop renderer trust and transport bounds

- Defects: AUD-012 (P0/low), AUD-013 (P0/medium), AUD-032 (P1/low).
- Files/functions: Markdown renderer/CSP, renderer filesystem guards, ACP
  WebSocket adapter.
- Data paths: model output to network; renderer path to main-process filesystem;
  socket input to memory.
- Modularization: `main.ts` is 3223 lines (>=2000); extract no mid-campaign
  structure and route it. Other files are <=1000.
- Regression surface: no remote image load, canonical path denial, bounded queue.
- Commit: `harden desktop renderer boundaries (AUD-012,AUD-013,AUD-032)`.

### Group 6 — provider execution contract and working-directory policy

- Defects: AUD-005, AUD-006, AUD-007, AUD-008, AUD-022 (all P0; mixed
  low/high complexity).
- Files/functions: working-dir inspector, provider capability/factory contract,
  Gemini/Cursor/Claude/Codex/Tagteam construction and mode handling, restricted
  reply preflight.
- Data paths: session cwd/mode/restriction through provider construction to
  external CLI execution.
- Modularization: `agent.rs` is >=2000 and routed. Claude (1646) and Codex
  (1429) receive small field/factory/command edits; if implementation expands
  beyond those local edits, extract their command-construction seams first.
- Regression surface: canonical path fixtures, mode capability matrix, fake-CLI
  cwd, fail-closed unsupported restriction.
- Commit: `enforce provider execution policy (AUD-005..AUD-008,AUD-022)`.

### Group 7 — request and shell cancellation ownership

- Defects: AUD-014 (P0/medium), AUD-015 (P1/low), AUD-016 (P1/high).
- Files/functions: session request registry/guard, ACP and subagent stream loops,
  developer shell process-group guard.
- Data paths: cancellation signal to task and descendant completion.
- Modularization: `developer/shell.rs` is 1227 lines and needs lifecycle logic;
  extract the process-group ownership seam while retaining the shell facade.
  Other touched files are <=1000 or >=2000 (routed).
- Regression surface: occupied cancelling slot, stalled streams, descendants.
- Commit: `make cancellation retain task ownership (AUD-014..AUD-016)`.

### Group 8 — server and desktop lifecycle supervision

- Defects: AUD-017, AUD-018, AUD-019 (P1/medium).
- Files/functions: shutdown signal/SSE loop, extension-loader handle registry and
  route registration, desktop backend cleanup.
- Data paths: shutdown/removal to actual async task or process termination.
- Modularization: `routes/agent.rs` is 1068 lines, but route edits will be kept
  as thin calls into `state.rs`; if more than those call sites change, extract
  extension-loading route coordination first. Other files are <=1000.
- Regression surface: shutdown token, atomic loader registration, actual exit.
- Commit: `supervise server and desktop lifecycles (AUD-017..AUD-019)`.

### Group 9 — bounded complete MCP discovery

- Defects: AUD-020, AUD-021 (P1/low).
- Files/functions: extension manager tool/resource/prompt list loops.
- Data paths: remote cursor pages to in-memory extension inventory.
- Modularization: `extension_manager.rs` is >=2000 and routed; introduce one
  small shared pagination guard without splitting the file.
- Regression surface: repeated cursor, page/item limit, multi-page completeness.
- Commit: `bound and complete MCP pagination (AUD-020,AUD-021)`.

### Group 10 — CLI provider terminal outcomes

- Defects: AUD-023, AUD-024, AUD-025 (P1/low).
- Files/functions: Cursor subprocess collection/parser; Codex event/exit parser.
- Data paths: child stderr/stdout/exit into provider success/failure.
- Modularization: Cursor is <=1000. Codex is 1429 lines; extract its event-result
  interpretation seam before changing multiple parser/runner functions.
- Regression surface: stderr flood, malformed/empty/error Cursor, partial Codex.
- Commit: `preserve CLI provider failures (AUD-023..AUD-025)`.

### Group 11 — durable state transitions and side effects

- Defects: AUD-026 (P1/medium), AUD-027 (P1/medium), AUD-031 (P0/high).
- Files/functions: provider/mode update ordering, ACP cwd transition, tool-call
  persistence boundary.
- Data paths: live state and external side effects versus SQLite durability.
- Modularization: `agent.rs` and `acp/server.rs` are >=2000 and routed; use
  smallest safe changes only.
- Regression surface: persistence fault injection, cwd rollback, crash-point
  idempotency. AUD-031 has a mandatory re-design/stop checkpoint before edit.
- Commit: `make agent transitions failure-atomic (AUD-026,AUD-027,AUD-031)`.

### Group 12 — review and delegation capacity

- Defects: AUD-028 (P1/low), AUD-029 (P1/medium), AUD-030 (P1/medium).
- Files/functions: review worker spawning/shared permits; async delegate slot
  reservation.
- Data paths: repository root and capacity through subprocess/task creation.
- Modularization: `review/orchestrator.rs` is 1182 lines; extract worker launch
  and shared permit coordination before patching. `summon.rs` is >=2000 and
  routed.
- Regression surface: child cwd, global peak workers, atomic task reservation.
- Commit: `enforce orchestration roots and capacity (AUD-028..AUD-030)`.

### Group 13 — fixed-height Ink text budgets

- Defect: AUD-034 (P2/low).
- Files/functions: configure/onboarding/error-screen dynamic text rendering.
- Data paths: provider/error strings to fixed terminal cell budgets.
- Modularization: none; all files are <=1000.
- Regression surface: long-string render assertions and explicit truncation.
- Commit: `contain fixed-height Ink text (AUD-034)`.

## Cross-stage risks and ordering constraints

- Group 6 establishes provider capability/cwd contracts consumed by Groups 10
  and 11; run it first.
- Groups 4, 8, and 9 all touch `extension_manager.rs` or extension lifecycle.
  Re-read each later group's touch set after the prior commit.
- Group 7 cancellation ownership precedes Group 8 shutdown ownership.
- Group 11's AUD-031 cannot be approximated by merely reordering messages. If
  an at-most-once operation identity cannot be added without a broad protocol
  rewrite, stop and report that single residual after completing safe groups.
- Routed modularization targets: `session_manager.rs`, `acp/server.rs`,
  `agent.rs`, `extension_manager.rs`, `summon.rs`, and `ui/desktop/src/main.ts`.

## Execution summary

The audit checkpoint was committed and remote-verified at `cdf2634ae` before
this repair skill was loaded. Repair commits remain local by user instruction.

| Group | Disposition | Commit |
|---|---|---|
| 1 | AUD-011, AUD-033 fixed | `ae9264458` |
| 2 | AUD-003, AUD-004 fixed | `dac101bcf` |
| 3 | AUD-001, AUD-002 fixed | `a52862231` |
| 4 | AUD-009, AUD-010 fixed | `f69a00d87` |
| 5 | AUD-012, AUD-013, AUD-032 fixed | `1f9867b66` |
| 6 | AUD-005–AUD-008, AUD-022 fixed | `ea69344aa` |
| 7 | AUD-014–AUD-016 fixed | `1dc1880e1` |
| 8 | AUD-017–AUD-019 fixed | `e741c3527` |
| 9 | AUD-020, AUD-021 fixed | `4a24b7f8f` |
| 10 | AUD-023–AUD-025 fixed | `92dab2abf` |
| 11 | AUD-026, AUD-027 fixed; AUD-031 mandatory architectural residual | `0ea3eac71` |
| 12 | AUD-028–AUD-030 fixed | `03895803e` |
| 13 | AUD-034 fixed | `cb0d45640` |
| Post-freeze | POST-001 extension request schema fixed | `7f6203b57` |

Final frozen-inventory disposition: 33 fixed, 1 architectural residual
(AUD-031). The additional POST-001 defect discovered by TypeScript verification
was also fixed. Detailed change, regression, and adversarial notes are in
`reports/2026-07-17-defect-campaign-session-log.md`.
