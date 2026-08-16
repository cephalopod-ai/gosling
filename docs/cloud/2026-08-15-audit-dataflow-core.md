# Gosling Audit — Dataflow Core (Cascade + Concurrency + Integrity)

**Date:** 2026-08-15  
**Target:** `/Users/eric/Work/vscode/forked/gosling`  
**HEAD:** `073d19428509ea6eb317924b1856a1fe7e9002c8`  
**Authority:** `read_only` — source was not modified; no tests were executed.  
**Lenses:** `audit-dataflow-cascade` (CAS-001..015), `audit-dataflow-concurrency` (CON-001..018), `audit-dataflow-integrity` (DAT-001..015)  
**IDs:** `CAS-GSL-NNN`, `CON-GSL-NNN`, `DAT-GSL-NNN`

> The supplied prompt is treated as a draft. The intended mission is preserved (session SQLite WAL, secrets/config writes, session import transactions, retry amplification, grind/nudge loops, concurrent CLI+desktop on `~/.config/gosling`, mid-turn crash replay) and expanded to adjacent handoffs: compacted-resume summaries, cross-process tool-ledger recovery, ACP whole-blob `extension_data` replace, and optional MCP tool-listing fail-closed.

Historical `docs/cloud/audit-dataflow-{cascade,concurrency,integrity}.md` reports are **seeds only**. Several 2026-07 findings are repaired at this HEAD (atomic secrets write, fail-closed corrupt config, single-transaction import/copy, grind nudge cap, default `transient_only`, `merge_extension_state`). Those are recorded as non-findings below; they are not copied forward.

## Executive Verdict

No Critical durable-corruption path remains on the previously High secrets-truncate writer. The highest current defects are (1) compacted resume injecting a summary without a freshness/status gate, which can drop the middle of a long conversation into the provider context, and (2) `recover_tool_operations` treating another process's live tool as crashed because `owner_id` is per-process UUID. Concurrent CLI+desktop on the default `~/.config/gosling` / data-dir pair is a first-class deployment posture, not a hypothetical.

Historical High/Medium items that were the previous campaign's headline (non-atomic `secrets.yaml`, silent corrupt-config wipe, multi-transaction import, unbounded grind, default 4xx retry) are **held** at this HEAD, except that `RetryConfig::new()` still retries deterministic 4xx for Bedrock/Databricks/GCP Vertex.

Patching is recommended before treating CLI+desktop concurrent use, compacted resume, or session-import of foreign `extension_data` as safe. No merge/deploy pause is required for a single-process local user who never opens the same session from two UIs.

## Scope

- Repository/project / branch / commit: gosling `main` @ `073d19428509ea6eb317924b1856a1fe7e9002c8`
- Prompt or session log reviewed: `docs/cloud/2026-08-15-orientation.md`; historical cloud reports used as seeds only
- Skills (lenses) invoked: cascade, concurrency, integrity (combined core report)
- Files/directories inspected: `crates/gosling/src/session/session_manager.rs`, `import_formats/mod.rs`, `config/base.rs`, `config/migrations.rs`, `config/paths.rs`, `oauth/persist.rs`, `permission/permission_store.rs`, `agents/agent.rs`, `agents/tool_execution.rs`, `agents/extension_manager.rs`, `agents/execute_commands.rs`, `acp/server.rs`, `acp/server/load_session.rs`, `acp/server/new_session.rs`, `context_mgmt/summarizer/mod.rs`, `providers/{bedrock,databricks,databricks_v2,gcpvertexai,utils}.rs`, `crates/gosling-providers/src/{retry,http_status,ollama}.rs`, `crates/gosling-server/src/routes/agent.rs`
- Commands/tests run: none (static re-verify only; `git`/runtime not executed)
- Effort budget: ~90 tool calls across the three lenses, concentrated on the named focus surfaces
- Constraints: read_only; no live kill/WAL-corruption drill; race *manifestation* capped per `confidence_calibration.md`

## Draft Prompt Assessment

Intended mission: re-verify the dataflow core at current HEAD, focusing on WAL, secrets/config, import transactions, retries, grind/nudge, CLI+desktop sharing, and mid-turn crash replay.

Under-specified: whether desktop and CLI are assumed to share one process (they do not) and whether compacted resume is in scope (it is a first-class ACP/agent path).

Overly narrow if limited to the 2026-07 finding list: several of those are repaired; the live residual is recovery-vs-liveness, summary freshness, and remaining `RetryConfig::new()` callers.

Added angles: producer/consumer of `owner_id`, sibling `merge_extension_state` vs ACP whole-blob replace, compacted-summary status field vs resume consumer.

## Surface Inventory

| Surface | Actor | Input/Trigger | State/Output | Boundary | Reviewed |
|---|---|---|---|---|---|
| `sessions.db` (+ WAL/SHM) | CLI, desktop ACP, `gosling-server` | create/update/import/copy/add_message/upsert/tool ledger | session + messages + tool_operations + summaries | WAL + `BEGIN IMMEDIATE` + `busy_timeout=30s` + `foreign_keys=true` | Yes |
| Stream checkpoint | agent reply loop | 250ms / new message id | partial assistant `upsert_message` | none for incomplete-stream quarantine | Yes |
| Tool operations ledger | `begin/complete/recover` | tool dispatch, process drop, session load/resume/`reply` | `started`/`completed`/`in_doubt` | unique `(session_id, tool_request_id)`; liveness is in-process only | Yes |
| Session import/copy | CLI/ACP/Nostr/file | JSON / foreign transcript | new session row + messages + provenance | single `BEGIN IMMEDIATE` (held); file SHA is check-then-act | Yes |
| `config.yaml` | `set_param` / `update_param` / `delete` | settings write | whole mapping rewrite | in-proc mutex; `save.lock` only around rename | Yes |
| `secrets.yaml` / keyring | `mutate_secrets`, OAuth persist | API keys, OAuth blobs | whole secret map | flock across RMW + temp+fsync+rename | Yes |
| Grind / goal / stop-hook | `/grind`, `/goal`, stop hooks | no-tool turn | re-injected user nudge | grind cap 50; goal one-shot; stop-hook cap 8 | Yes |
| Provider retry | `with_retry` / `RetryConfig` | 4xx/5xx/429/network | repeated HTTP | default `transient_only=true`; `RetryConfig::new()` is not | Yes |
| Compacted resume | ACP `loadSession`, `Agent::reply` | `compacted_context` / `historyLoad.mode=compacted` | summary prefix + tail as conversation | status/freshness **not** checked | Yes |
| MCP tool listing | `fetch_all_tools` | any extension `list_tools` error | entire tool set fails | fail-visible, no per-extension isolate | Yes |
| `tool_permissions.json` | `ToolPermissionStore` | unused at this HEAD | latent whole-file RMW | unwired | Yes (dead) |

## Boundary Map

| Surface | Intended Boundary | Enforced At | Status |
|---|---|---|---|
| Session logical write (import/copy/message+updated_at) | one `BEGIN IMMEDIATE` | `import_session`/`copy_session`/`add_message`/`replace_conversation_inner` | Held |
| HTTP/ACP new-session extensions | same | create commits, then `update().extension_data().apply()` | **Missing** |
| Secrets durability | temp+fsync+rename + 0600 | `write_secrets_file` | Held |
| Secrets RMW across processes | flock across read+mutate+write | `lock_secret_transaction` | Held |
| Config RMW across processes | flock across read+mutate+write | only in-proc `guard`; `save.lock` is write-only | **Missing** |
| Config corrupt-on-read | fail closed | `load_write_config` returns parse error | Held |
| Tool dispatch idempotency | ledger + no auto-retry of `in_doubt` | `begin_tool_operation` unique + `InDoubt` | Held in-process |
| Tool recovery vs live peer | do not recover another process's live op | `owner_id == self.owner_id && active` only | **Missing** |
| Grind feedback | independent nudge cap | `DEFAULT_MAX_GRIND_NUDGES=50` | Held |
| Retry amplification | no deterministic 4xx retry | `RetryConfig::default().transient_only=true`; `RetryConfig::new()` false | **Partial** |
| Compacted summary | only `Current` summaries become provider context | `get_session_for_compacted_resume` ignores `status` | **Missing** |
| Import provenance | untrusted marker + no artifact promotion | `imported_untrusted` + `history_trusted: false` | Held for artifacts; **not** for `extension_data` keys |
| Optional MCP | degrade-and-continue | `fetch_all_tools` fails the whole list | **Inverted** (fail-global) |
| WAL power-loss | at most last commit lost | `synchronous=NORMAL` (documented) | Accepted residual |

## Failure Taxonomy

### Cascade

| Origin Failure | Propagation Path | Containment Point | Downstream Consumer | Amplifier | Blast Radius |
|---|---|---|---|---|---|
| Stale/failed session summary | `get_session_summary` → compacted resume prefix | **none** (status ignored) | provider context, ACP replay | omitted middle history | Workflow (wrong context) |
| Live tool in another process | `loadSession`/`resume`/`reply` → `recover_tool_operations` | in-process `owner_id` only | conversation + ledger `in_doubt` | CLI complete fails; model sees in-doubt | Workflow + side-effecting tools |
| One MCP `list_tools` error | `fetch_all_tools` join_all | **none** — any Err fails all | `prepare_tools_and_prompt` / turn | whole tool set gone | Workflow |
| Deterministic 4xx on Bedrock/Databricks/GCP | `RetryConfig::new` → `should_retry` | cap 3 + backoff | provider HTTP | 4× load / ~7s delay | Service |
| Permanent thinking-block 400 | provider `RequestFailed` → generic agent arm | retry layer skips (default) | user-visible "please retry" | user retry resends poison | Workflow (stuck session) |
| Mid-stream crash | 250ms checkpoint upsert | none | next `reply` history | thinking-block mismatch | Workflow |
| Grind never completes | no-tool arm re-injects nudge | **50 nudges** then clear | provider | bounded | Local (held) |

### Concurrency

| State/Artifact | Writers | Readers | Transaction | Idempotency Key | Lock/Constraint | Race Hazard |
|---|---|---|---|---|---|---|
| `sessions.db` rows | all processes | all | `BEGIN IMMEDIATE` | session `id` PK | WAL + busy_timeout | held for row writes |
| `tool_operations` | begin/complete/recover | recover/begin | begin/recover in IMMEDIATE; complete is single UPDATE | `UNIQUE(session_id, tool_request_id)` | liveness = in-proc set | **cross-proc recover** |
| `extension_data` | `merge_extension_state`; ACP `update().extension_data()` | get_session | merge is IMMEDIATE RMW | key-in-blob | whole-blob load path | **ACP load clobber** |
| `config.yaml` | set/update/delete | load | none across RMW | none | in-proc mutex + save.lock | **lost update** |
| `secrets.yaml` | `mutate_secrets` | `all_secrets` | flock RMW | none | `.secrets.lock` / `.lock` | held |
| import file SHA | `import_session_file` | list_all + provenance | import tx after check | SHA in blob only | no unique index | **double import** |
| stream checkpoint message | `upsert_message` | get_conversation | per upsert tx | message_id (no UNIQUE) | IMMEDIATE | partial durable text |

### Integrity

| Entity | Source | Owner | Writers | Readers | Scope Key | Provenance | Integrity Constraint |
|---|---|---|---|---|---|---|---|
| Session | create/import/copy | local user | SessionStorage | CLI/ACP/UI | `id` PK | import_provenance v1 | PK |
| Message | add/upsert/import | session_id FK | add/upsert/replace | conversation/provider | `(session_id, message_id)` hoped | `imported_untrusted` | FK; **no UNIQUE(message_id)** |
| Tool op | begin | session + request | begin/complete/recover | recover/begin | `(session_id, tool_request_id)` | owner_id | UNIQUE + CHECK state |
| Summary | summarizer | session_id PK | upsert + stale mark | compacted resume | session_id | source_hash, status | status **not consumed** |
| Secrets | configure/OAuth | file/keyring | `mutate_secrets` | get_secret | key string | none | flock + atomic file |
| Config | set_param | config.yaml | set/update/delete | get_param | key | none | atomic write, not RMW |
| Import extension_data | foreign JSON | new session | import clone minus enabled_extensions | tools/todo | session | history_trusted=false | **other keys promoted** |

## Inventory Result

Every required code is a finding or an explicit non-finding.

### Cascade (CAS-001..015)

| Code | Verdict | Notes |
|---|---|---|
| CAS-001 Cascading Failure | **Finding** `CAS-GSL-003` | One MCP list failure fails the whole tool set / turn prep |
| CAS-002 Feedback Loop | **Not Confirmed** | Grind capped at 50; goal one-shots; stop-hook cap 8; refusal skips nudges |
| CAS-003 Retry Amplification | **Finding** `CAS-GSL-001` | `RetryConfig::new()` still retries `RequestFailed`; default config does not |
| CAS-004 State Poisoning | **Finding** `CAS-GSL-004`, `DAT-GSL-001` | Partial stream + stale summary become next-turn authority |
| CAS-005 Downstream Misclassification | **Finding** `CAS-GSL-002` | Permanent 400 classified as "retry if transient" |
| CAS-006 Blast Radius Expansion | **Finding** `CON-GSL-001` | Peer recover expands a local tool interrupt into the shared conversation |
| CAS-007 Missing Containment | **Finding** `CAS-GSL-003` | No bulkhead between one extension and tool listing |
| CAS-008 Error Context Lost | **Finding** `CAS-GSL-002` | Cause preserved in text, but class flattened to generic retry advice |
| CAS-009 Bad Data Becomes Authority | **Finding** `DAT-GSL-001`, `DAT-GSL-002` | Stale summary / imported `todo` become live state |
| CAS-010 Partial Failure Becomes Global | **Finding** `CAS-GSL-003`, `DAT-GSL-005` | MCP list; legacy import marks all complete after per-file failures |
| CAS-011 Advisory Becomes Enforcement | **Not Confirmed** | `history_trusted`/`imported_untrusted` are not release gates; no SSDF-style enforcement |
| CAS-012 Optional Integration Failure Multiplier | **Finding** `CAS-GSL-003` | Optional MCP is on the critical path |
| CAS-013 Stale Artifact Pollutes Workflow | **Finding** `CAS-GSL-004`, `DAT-GSL-001` | Checkpoint / stale summary reused as current |
| CAS-014 Recovery Causes Secondary Failure | **Finding** `CON-GSL-001` | Recovery writes `in_doubt` over a live peer |
| CAS-015 Alert/Health Noise Masks Root Cause | **Not Confirmed** | Retries `warn!` once per attempt (capped); no health-flap storm on these paths |

### Concurrency (CON-001..018)

| Code | Verdict | Notes |
|---|---|---|
| CON-001 Race Condition | **Finding** `CON-GSL-001` | Recover vs live dispatch (manifestation Likely) |
| CON-002 Lost Update | **Finding** `CON-GSL-002`, `CON-GSL-003` | Config RMW; ACP whole-blob `extension_data` |
| CON-003 Double Processing | **Finding** `CON-GSL-004` | File import SHA check-then-act |
| CON-004 Replay Hazard | **Finding** `CON-GSL-001`, `CON-GSL-005` | Peer recover; WAL last-commit loss then re-dispatch |
| CON-005 Retry Collision | **Finding** `CON-GSL-001` | `complete_tool_operation` vs recover `in_doubt` |
| CON-006 Stale Read | **Finding** `CON-GSL-002`, `CON-GSL-003` | Snapshot then write-back |
| CON-007 Stale Write | **Finding** `CON-GSL-002`, `CON-GSL-003` | Same window |
| CON-008 Ordering Dependency | **Not Confirmed** | Messages ordered by autoincrement `id`; recover orders by `created_at, operation_id` |
| CON-009 Partial Commit | **Finding** `DAT-GSL-006` | HTTP/ACP create then extension apply; import/copy held |
| CON-010 Missing Transaction Boundary | **Finding** `DAT-GSL-006` | Same; import/copy now share one tx |
| CON-011 Lock Inversion | **Not Confirmed** | `save.lock` vs `.extensions.lock` deliberately split to avoid self-deadlock (`save_values` comment) |
| CON-012 Shared Mutable State | **Not Confirmed** | `active_tool_operations` mutex is process-local; `KEYRING_RUNTIME_DISABLED` is `AtomicBool`; tools cache is versioned |
| CON-013 Non-Atomic File Output | **Not Confirmed** | Secrets/config/edit use temp+`sync_all`+rename |
| CON-014 Duplicate Canonical Creation | **Finding** `CON-GSL-004` | No unique on import SHA; session ids are minted new |
| CON-015 Check-Then-Act Hazard | **Finding** `CON-GSL-004` | `import_session_file` lists then imports; upsert check+insert is inside IMMEDIATE (held) |
| CON-016 Concurrent Bulk Scope Drift | **Not Confirmed** | No bulk candidate-selection writer on these surfaces |
| CON-017 Artifact Reuse Race | **Not Confirmed** | Config temp is UUID-suffixed + `save.lock`; secrets `.tmp` is serialized by flock |
| CON-018 Watcher/Event Reentrancy | **Not Confirmed** | No file-watcher amplification on these paths; recover-on-load is a one-shot |

### Integrity (DAT-001..015)

| Code | Verdict | Notes |
|---|---|---|
| DAT-001 Scope Leakage | **Not Confirmed** | Local-first; session `id` is the scope. `get_session` is unscoped by workspace by design, not a tenant IDOR |
| DAT-002 Duplicate Entity | **Finding** `CON-GSL-004` | File import can mint two sessions for one SHA |
| DAT-003 Orphaned Record | **Not Confirmed** | `delete_session` deletes children then parent; artifacts/tool_ops/summaries CASCADE |
| DAT-004 Lost Provenance | **Not Confirmed** | `SessionImportProvenance` written; messages get `imported_untrusted` |
| DAT-005 Corrupt Merge | **Finding** `CON-GSL-003` | Whole-blob `extension_data` replace can drop sibling keys |
| DAT-006 Incorrect Normalization | **Not Confirmed** | Mixed ms/s timestamps normalized in SQL (`normalized_message_timestamp_sql`) |
| DAT-007 Partial Persistence | **Finding** `DAT-GSL-005`, `DAT-GSL-006` | Legacy import; two-step session create. Import/copy held |
| DAT-008 Migration Meaning Loss | **Finding** `DAT-GSL-004` | Migration 24 clears workspace `restrict_tools_to_working_dirs` |
| DAT-009 Round-Trip Loss | **Not Confirmed / design** | Export→import mints new id, forces `Approve`, strips enabled extensions, marks untrusted — tested and intentional |
| DAT-010 Stale Derived Data | **Finding** `DAT-GSL-001` | Summary `status=stale` still injected |
| DAT-011 Evidence Misclassification | **Not Confirmed** | Imported assistant text is excluded from artifact inference |
| DAT-012 Advisory Output Misrepresented | **Not Confirmed** | `importedUntrusted` is surfaced on ACP replay meta |
| DAT-013 Silent Constraint Violation | **Finding** `DAT-GSL-003` | Hoped uniqueness of `message_id` is not in the schema |
| DAT-014 Cross-Batch Contamination | **Not Confirmed** | Import always `create_session_in_tx` (new id); cannot overwrite a named local id |
| DAT-015 Weak Data Promoted To Authority | **Finding** `DAT-GSL-002`, `DAT-GSL-001` | Imported `todo` etc.; stale summary prefix |

## Findings Table

| ID | Severity | Confidence | Evidence Basis | Domain | Title | Patch Priority | Blast Radius | Complexity | Cost | Nominal Agent |
|---|---|---|---|---|---|---|---|---|---|---|
| DAT-GSL-001 | High | Confirmed | source-evidenced | Data-Integrity | Compacted resume injects summary without status/freshness gate | 1 | Workflow | local_guardrail | S | codex |
| CON-GSL-001 | High | Likely | source-evidenced | Concurrency | Cross-process recover marks a live peer tool `in_doubt` | 2 | Workflow | persistence_recovery | M | claude |
| CON-GSL-002 | Medium (High if CLI+desktop write settings together) | Confirmed (missing lock) / Likely (collision) | source-evidenced | Concurrency | Config RMW is atomic only in-process | 3 | Service | cross_process_coordination | M | claude |
| CON-GSL-003 | Medium | Likely | source-evidenced | Concurrency | ACP/HTTP load still whole-blob replaces `extension_data` | 3 | Workflow | local_guardrail | S | codex |
| CAS-GSL-001 | Medium | Confirmed | source-evidenced | Cascade | `RetryConfig::new()` retries deterministic 4xx | 4 | Service | local_guardrail | S | codex |
| CAS-GSL-002 | Medium | Confirmed | source-evidenced | Cascade | Permanent provider 400 advised as user-retryable | 4 | Workflow | local_guardrail | S | codex |
| CAS-GSL-003 | Medium | Confirmed | source-evidenced | Cascade | One MCP `list_tools` failure fails all tools | 5 | Workflow | local_guardrail | S | codex |
| CAS-GSL-004 | Medium | Likely | source-evidenced | Cascade | Mid-turn stream checkpoint becomes durable incomplete history | 5 | Workflow | persistence_recovery | M | claude |
| DAT-GSL-002 | Medium | Confirmed | source-evidenced | Data-Integrity | Import promotes foreign `extension_data` keys | 5 | Workflow | local_guardrail | S | codex |
| DAT-GSL-004 | Medium | Confirmed | source-evidenced | Data-Integrity | Migration 24 silently unrestricts workspace tool dirs | 6 | Workflow | governance_decision | S | human-owner |
| CON-GSL-004 | Medium | Likely | source-evidenced | Concurrency | File-import SHA dedupe is check-then-act | 6 | Workflow | local_guardrail | S | codex |
| CON-GSL-005 | Medium (High if power-loss) | Plausible | requires-authorized-drill | Concurrency | WAL NORMAL last-commit loss can re-dispatch a tool | 7 | Workflow | persistence_recovery | M | claude |
| DAT-GSL-003 | Low | Confirmed | source-evidenced | Data-Integrity | `messages.message_id` has no UNIQUE constraint | 8 | Local | local_guardrail | S | codex |
| DAT-GSL-005 | Low | Confirmed | source-evidenced | Data-Integrity | Legacy import marks complete after partial failures | 8 | Local | persistence_recovery | S | codex |
| DAT-GSL-006 | Low | Confirmed | source-evidenced | Data-Integrity | New-session create + extension apply are two commits | 8 | Local | persistence_recovery | S | codex |

## Detailed Findings

### DAT-GSL-001: Compacted resume injects a summary without a status/freshness gate

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/session/session_manager.rs:3506-3536` — `get_session_for_compacted_resume` loads any non-empty `summary.summary` and prepends it as `[Compacted summary of N earlier message(s)]`. It never reads `summary.status`, `source_hash`, or `covered_through_row_id` against the tail gap.
- `crates/gosling/src/session/session_manager.rs:4081-4084` and `4157-4160` — every `add_message` / `upsert_message_in_tx` sets `session_summaries.status = 'stale'`.
- `crates/gosling/src/session/session_manager.rs:183-188` — status enum is `current` / `stale` / `failed`.
- `crates/gosling/src/agents/agent.rs:1932-1940` — `reply` uses this builder whenever `session_config.compacted_context` is set.
- `crates/gosling/src/acp/server/load_session.rs:249-255,272-276` — ACP `loadSession` with `compacted` uses the same builder for replay **and** the agent conversation.

Observed behavior:
- After any new message the stored summary is marked stale, but compacted load/resume still treats its text as the authoritative prefix and then appends only the tail page. Messages after `covered_through_row_id` that are not in the tail never appear.

Expected boundary:
- Only a `Current` summary whose coverage meets the tail's oldest row may be injected. A stale/failed/gapped summary must force a full load, a recompute, or an explicit "gap" marker — not silent omission.

Failure mechanism:
- Derived rollup is persisted and later read as conversation authority with no freshness check (DAT-010). The stale text becomes the provider's view of "everything before the tail" (CAS-009).

Break-it angle:
- Summarize a long session; send more than `tail_limit` additional turns; open the session with compacted load (or `reply` with `compacted_context`). The middle turns are absent from the model context while the UI `historyLoad.totalCount` still reports them.

Impact:
- Provider acts on an incomplete history: forgotten decisions, re-issued tools, contradictory plans. User-visible as "the agent forgot the middle of the chat."

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: compensatable (full reload)
- Operator visibility: silent in the model path; ACP meta reports counts only
- Rerun safety: unsafe (another compacted turn compounds the gap)

Adjacent failure modes:
- CAS-GSL-004 (another incomplete-history authority)
- TMP (freshness) — escalate

Recommended mitigation:
- Remediation patterns: freshness gate; fail-visible gap.
- Minimal repair: skip injection unless `status == Current` **and** `covered_through_row_id + 1` meets `page.oldest_row_id`.
- Local guardrail: if stale/gapped, inject the existing "No durable summary" notice or refuse compacted mode.
- Behavior test: stale summary + tail smaller than the uncovered span must not send the stale text and must not drop uncovered rows silently.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: one loader + tests
- Nominal implementation agent: codex
- Rationale: status column already exists; the consumer simply ignores it.

Validation:
- Test: `status=stale` summary is not prepended.
- Test: `covered_through_row_id` below `oldest_row_id - 1` refuses compacted resume or fills the gap.
- Test: `status=current` and contiguous coverage still prepends.

Non-goals:
- Do not redesign the summarizer in this slice.

### CON-GSL-001: Cross-process recover marks a live peer tool `in_doubt`

Severity: High  
Confidence: Likely  
Evidence basis: source-evidenced (missing liveness) + simulation-reasoned (collision)  
Domain: Concurrency

Evidence:
- `crates/gosling/src/session/session_manager.rs:1385-1393` — each `SessionStorage` mints `owner_id: uuid::Uuid::new_v4()`.
- `crates/gosling/src/session/session_manager.rs:3909-3912` — recover skips only when `owner_id == self.owner_id && active_operations.contains(&operation_id)`. A different process never matches.
- `crates/gosling/src/session/session_manager.rs:3943-3975` — a `started` row then becomes an `in_doubt` tool response persisted into `messages`.
- Callers that run recover on an ordinary open/turn, not only after a crash: `agents/agent.rs:1748-1751` (`reply`), `acp/server/load_session.rs:227-230` (`loadSession`), `crates/gosling-server/src/routes/agent.rs:249-252` (`/agent/resume`).
- `crates/gosling/src/session/session_manager.rs:5594-5631` — unit test *proves* a second `SessionManager` (new `owner_id`) recovers the first manager's in-flight op; that is the CLI+desktop topology.
- `crates/gosling/src/config/paths.rs:19-31` — default config/data dirs are the shared `gosling` app dirs (CLI and desktop).

Observed behavior:
- Opening or continuing a session in process B while process A is dispatched on a conversation-bound tool synthesizes "execution status is in doubt" into the shared conversation and flips the ledger.

Expected boundary:
- Recovery may run only for operations whose owner is dead (or after an explicit crash/restart of that owner). A live peer must be left alone.

Failure mechanism:
- Liveness is an in-process `HashSet` plus a per-process UUID. WAL lets two processes share the ledger; the skip condition does not.

Break-it angle:
- Start a turn with a long-running tool in the CLI. In desktop, open the same session (ACP `loadSession`). Desktop persist an `in_doubt` response. CLI `complete_tool_operation` then hits `cannot complete tool operation in state in_doubt` (`3779-3801`).

Impact:
- Live side-effecting tools are declared in-doubt while still running; the model is told not to retry; the operator sees a crash that did not happen. Completing the real tool cannot land on the ledger.

Operational impact:
- Blast radius: Workflow
- Side-effect class: DB + process
- Reversibility: compensatable (manual)
- Operator visibility: UI-visible (`in_doubt` message)
- Rerun safety: unsafe (second recover is a no-op; the live tool is already poisoned)

Adjacent failure modes:
- CAS-014 recovery secondary failure; CON-005 complete-vs-recover; CAS-006 blast into conversation.
- CON-GSL-005 (true crash + lost last commit is the inverse).

Recommended mitigation:
- Remediation patterns: lease/heartbeat; recover-only-if-owner-dead.
- Minimal repair: persist a heartbeat/`pid`/`started_at` and skip recover when the owner lease is fresh; or recover only from a dedicated crash-recovery entrypoint, not `loadSession`/`reply`.
- Local guardrail: if `owner_id != self.owner_id` and `state=started` and `updated_at` is recent, skip.
- Behavior test: two `SessionManager`s on one DB; A begins a tool; B `recover_tool_operations` returns 0 and leaves `started`.

Implementation assessment:
- Complexity: persistence_recovery
- Cost: M
- Cost drivers: lease protocol, two processes in tests
- Nominal implementation agent: claude
- Rationale: need a real definition of "owner dead" across CLI and desktop.

Validation:
- Test: peer recover does not flip `started` while A's lease is live.
- Test: after A is dropped (new process, stale lease), recover still synthesizes `in_doubt`.

Non-goals:
- Do not remove the in-doubt path for same-process drop (`ToolOperationGuard`).

### CON-GSL-002: Config read-modify-write is atomic only in-process

Severity: Medium (High if CLI and desktop write settings/OAuth-adjacent keys together)  
Confidence: Confirmed (missing cross-process RMW lock) / Likely (lost-update manifestation)  
Evidence basis: source-evidenced  
Domain: Concurrency

Evidence:
- `crates/gosling/src/config/base.rs:1024-1028` (`set_param`), `994-1008` (`update_param`), `1058-1065` (`delete`) — only `lock_ignoring_poison(&self.guard)` spans the read (`load_write_config`) and `save_values`. That mutex is per `Config` instance / process.
- `crates/gosling/src/config/base.rs:787-828` — `save.lock` is taken only inside `save_values` (write-temp-rename), after the stale mapping is already in memory.
- Contrast secrets: `1205-1215` holds `lock_secret_transaction()` across `all_secrets` + `write_all_secrets`, and `2179-2214` tests two `Config` instances.
- No sibling test exists for `set_param` across instances.

Observed behavior:
- Two processes each load `config.yaml`, each set a different key, each rename a full mapping. The later rename drops the earlier key.

Expected boundary:
- The same exclusive flock used for secrets must cover config read-modify-write.

Failure mechanism:
- Classic lost update: compare/serialize against a stale snapshot; last rename wins.

Break-it angle:
- Run `gosling configure` (or any `set_param`) while desktop writes another setting. One key disappears.

Impact:
- Silent loss of a provider/model/extension setting. Contained and re-enterable, but silent.

Operational impact:
- Blast radius: Service
- Side-effect class: file
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- CON-GSL-003 (same snapshot-then-replace shape on `extension_data`).

Recommended mitigation:
- Remediation patterns: cross-process lock across RMW.
- Minimal repair: take `save.lock` (or a dedicated `config.lock`) before `load_write_config` in `set_param`/`update_param`/`delete`/`set_param_values`.
- Behavior test: two processes each `set_param` a distinct key; both keys survive.

Implementation assessment:
- Complexity: cross_process_coordination
- Cost: M
- Cost drivers: lock lifecycle + multi-process test
- Nominal implementation agent: claude

Validation:
- Test: concurrent cross-process `set_param` preserves both keys (mirror `secret_mutations_across_config_instances_do_not_drop_updates`).

Non-goals:
- Do not change YAML schema.

### CON-GSL-003: ACP/HTTP session activation still whole-blob replaces `extension_data`

Severity: Medium  
Confidence: Likely  
Evidence basis: source-evidenced  
Domain: Concurrency

Evidence:
- `crates/gosling/src/session/session_manager.rs:1120-1128` documents that `update(...).extension_data(...)` replaces the whole column and that `merge_extension_state` exists specifically to stop CON-001 clobber.
- `crates/gosling/src/acp/server.rs:1481-1495` — `prepare_session_for_activation` builds a snapshot via `build_enabled_extensions_data` (`1513-1542` clones `session.extension_data` then writes enabled-extensions / skills) and `apply()`s the whole blob.
- `crates/gosling-server/src/routes/agent.rs:166-175` — HTTP create: `create_session` then `update().extension_data(extension_data.clone()).apply()`.
- `crates/gosling/src/acp/server/new_session.rs:394-404` — `apply_initial_session_config` also `extension_data(config.extension_data)`.
- Live tool writers (`todo.rs`, `persist_extension_state` at `agent.rs:1398-1420`) correctly use `merge_extension_state`.

Observed behavior:
- Session load/create still does get-snapshot → mutate one key → replace column. A concurrent `merge_extension_state` of `todo.v0` (or memory) in the other process is dropped.

Expected boundary:
- Every live writer of a single extension key must go through `merge_extension_state` (or an equivalent in-transaction merge).

Failure mechanism:
- Sibling implementations: merge vs replace. The load path was not converted when the merge helper was added.

Break-it angle:
- CLI turn updates todos while desktop `loadSession` rebuilds enabled extensions from a stale snapshot and `apply()`s.

Impact:
- Lost todo/memory/skill-adjacent session state after a routine session open.

Operational impact:
- Blast radius: Workflow
- Side-effect class: DB
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unsafe (another load can clobber again)

Adjacent failure modes:
- DAT-005 corrupt merge; CON-002/007.

Recommended mitigation:
- Remediation patterns: in-transaction merge.
- Minimal repair: `prepare_session_for_activation` / HTTP create call `merge_extension_state` for `enabled_extensions.v0` (and skill key) only.
- Behavior test: concurrent `merge_extension_state(todo.v0)` vs load-time enabled-extensions write; both keys survive.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: a few call sites + the existing concurrent merge test pattern (`6145-6189`)
- Nominal implementation agent: codex

Validation:
- Test: load-path write does not drop a different key written in another IMMEDIATE tx.

Non-goals:
- Do not change the `EnabledExtensionsState` schema.

### CAS-GSL-001: `RetryConfig::new()` retries deterministic 4xx

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Cascade

Evidence:
- `crates/gosling-providers/src/retry.rs:28-36` — `Default` now sets `transient_only: true`.
- `crates/gosling-providers/src/retry.rs:40-53` — `RetryConfig::new(...)` still sets `transient_only: false`.
- `crates/gosling-providers/src/retry.rs:99-107` — `RequestFailed` is retried when `!transient_only` (unless the two thinking-block markers match).
- Callers: `crates/gosling/src/providers/bedrock.rs:219-224,732-733`; `databricks.rs:180-185,538-539`; `databricks_v2.rs:159`; `gcpvertexai.rs:224`. Ollama is the sibling that chains `.transient_only()` (`ollama.rs:384-390`).
- `retry.rs:266-271` asserts the *default* config does **not** retry a 400.

Observed behavior:
- Bedrock/Databricks/GCP Vertex still issue up to `max_retries` extra HTTP calls for a 400/404/422 whose payload cannot succeed.

Expected boundary:
- Deterministic client errors are not retried unless a provider opts in.

Failure mechanism:
- The safe default was flipped; the named constructor used by config-driven providers was not.

Break-it angle:
- Point Bedrock/Databricks/GCP at an unknown model name: four attempts + backoff before the error surfaces.

Impact:
- 4× provider load and delayed error on a request that cannot succeed. Bounded (not a storm).

Operational impact:
- Blast radius: Service
- Side-effect class: network
- Reversibility: reversible
- Operator visibility: log-only
- Rerun safety: safe

Adjacent failure modes:
- CAS-GSL-002 (user then retries the same poison).

Recommended mitigation:
- Remediation patterns: correct the constructor default.
- Minimal repair: `RetryConfig::new` sets `transient_only: true` (or those providers chain `.transient_only()` like Ollama).
- Behavior test: Bedrock/Databricks/GCP loaded config does not retry `RequestFailed("400")`.

Implementation assessment:
- Complexity: local_guardrail · Cost: S · Nominal agent: codex

Validation:
- Unit test `RetryConfig::new(...).transient_only == true` or provider `retry_config()` skips 400.

Non-goals:
- Do not change backoff math or the Retry-After clamp (`http_status.rs` `MAX_RETRY_AFTER_SECS=3600`).

### CAS-GSL-002: Permanent provider 400 is advised as user-retryable

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Cascade

Evidence:
- `crates/gosling-providers/src/retry.rs:84-97,104` — thinking-block signatures are treated as permanent at the retry layer only.
- `crates/gosling/src/agents/agent.rs:3005-3013` — every other `ProviderError` (including `RequestFailed` thinking-block 400) yields: `"Please retry if you think this is a transient or recoverable error."`
- Contrast the refusal arm at `2978-2992`, which correctly says start a new session and sets `exit_chat = true`.

Observed behavior:
- A mid-session model/thinking-config change produces a permanent 400. The retry layer correctly stops. The agent still tells the operator to retry. Retry rebuilds the same signed thinking blocks (`2565-2614` re-attaches them).

Expected boundary:
- Permanent request failures must use the refusal-style terminal path, not the transient retry hint.

Failure mechanism:
- Downstream misclassification: retry predicate knows it is permanent; the user-facing arm does not consume that predicate.

Break-it angle:
- Switch thinking effort / model mid-session on Anthropic; click retry as advised; same 400 forever.

Impact:
- Session appears "flaky" and stays stuck; extra paid retries.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: compensatable (new session / strip thinking)
- Operator visibility: UI-visible (wrong class)
- Rerun safety: unsafe (rerun is the amplifier)

Adjacent failure modes:
- CAS-GSL-004 (partial thinking checkpoint makes this easier to hit).

Recommended mitigation:
- Remediation patterns: classify-then-advise.
- Minimal repair: export `is_permanent_request_failure` and branch in the generic arm like refusal.
- Behavior test: thinking-block 400 message does not contain "retry" / "transient".

Implementation assessment:
- Complexity: local_guardrail · Cost: S · Nominal agent: codex

Validation:
- Test: permanent marker → terminal guidance; 500/network still suggest retry.

Non-goals:
- Do not broaden the marker list in this slice (status-based retry is CAS-GSL-001).

### CAS-GSL-003: One MCP `list_tools` failure fails all tools

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Cascade

Evidence:
- `crates/gosling/src/agents/extension_manager.rs:2107-2158` — per-extension `list_tools` `Err` is collected; if `!errors.is_empty()` the function returns `SetupError("Failed to enumerate extension tools: ...")` and **discards** every successful sibling's tools.
- `crates/gosling/src/agents/extension_manager.rs:3436-3456` — test `test_get_prefixed_tools_fails_visible_when_extension_tool_listing_fails` asserts this is intentional fail-visible.

Observed behavior:
- A single optional/broken MCP server fails `prepare_tools_and_prompt` for the turn. Healthy extensions are not offered.

Expected boundary:
- Optional integrations degrade: surface the broken name, continue with the rest. Fail-visible ≠ fail-global.

Failure mechanism:
- Containment missing between one client future and the aggregated tool list (CAS-007/010/012). Historical degrade-to-empty-tools was inverted to fail-closed-all.

Break-it angle:
- Add a dead stdio MCP next to a working `developer` extension; the next turn cannot see `developer` tools either.

Impact:
- One optional plugin takes down the agent tool surface until it is removed.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: compensatable (disable the extension)
- Operator visibility: UI-visible (good) but over-scoped
- Rerun safety: unsafe until the dead extension is gone

Adjacent failure modes:
- Operator-signal (the error is visible — keep that).

Recommended mitigation:
- Remediation patterns: per-item isolation + visible warning.
- Minimal repair: log/return extension errors alongside the successful tool list; only fail the turn if a *required* inspection extension failed.
- Behavior test: healthy+broken → healthy tools present and error names the broken extension.

Implementation assessment:
- Complexity: local_guardrail · Cost: S · Nominal agent: codex

Validation:
- Flip the existing test to assert isolation + visible error, not global `Err`.

Non-goals:
- Do not silently swallow required security-scanner listing failures.

### CAS-GSL-004: Mid-turn stream checkpoint becomes durable incomplete history

Severity: Medium  
Confidence: Likely  
Evidence basis: source-evidenced  
Domain: Cascade

Evidence:
- `crates/gosling/src/agents/agent.rs:86,2438-2546` — text-only chunks `upsert_message` every 250ms (`STREAM_CHECKPOINT_INTERVAL`) under a stable `stream_message_id`.
- `crates/gosling/src/agents/agent.rs:4984-5030` — test asserts the first chunk `"streamed "` is already in SQLite before the stream finishes.
- `crates/gosling/src/session/session_manager.rs:4092-4098,4112-4150` — upsert commits immediately. There is no "in_progress" flag and no crash-cleanup of a half-streamed assistant message.
- Next `reply` loads `get_session(..., true)` (`agent.rs:1942-1944`) and sends that conversation to the provider.

Observed behavior:
- Kill mid-stream: the last checkpointed prefix is a completed-looking assistant message. The next turn treats it as finished history.

Expected boundary:
- In-progress stream rows must be quarantined or completed/aborted on recover, not replayed as a final assistant turn.

Failure mechanism:
- Crash-recovery artifact (the checkpoint) pollutes the next workflow (CAS-013/004). Combined with thinking blocks this is a reliable way to hit CAS-GSL-002.

Break-it angle:
- Kill during a long assistant narration; resume; provider sees a truncated assistant message then a new user message. On Anthropic thinking models this can 400.

Impact:
- Truncated visible history; possible permanent 400; user thinks the model "stopped mid-sentence" and that this is canonical.

Operational impact:
- Blast radius: Workflow
- Side-effect class: DB
- Reversibility: compensatable (truncate/edit)
- Operator visibility: UI-visible
- Rerun safety: unsafe (resume sends the fragment)

Adjacent failure modes:
- CAS-GSL-002; CON-GSL-005.

Recommended mitigation:
- Remediation patterns: in-progress marker; recover abort.
- Minimal repair: persist `metadata.partial_stream=true` on checkpoints; on `reply`/`recover`, either drop or close the partial with an explicit interruption marker.
- Behavior test: kill after first checkpoint; next `get_session` must not present the fragment as a final assistant turn without an interruption tag.

Implementation assessment:
- Complexity: persistence_recovery · Cost: M · Nominal agent: claude

Validation:
- Test: partial checkpoint is marked/removed before the next provider payload is built.

Non-goals:
- Do not remove streaming checkpoints (they exist to survive crash *visibility*).

### DAT-GSL-002: Import promotes foreign `extension_data` keys to live session state

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/session/session_manager.rs:4808-4826` — import clones `import.extension_data`, removes only `enabled_extensions`, then writes provenance (`history_trusted: false`).
- `crates/gosling/src/session/session_manager.rs:7115-7126` — test asserts `todo.v0` from the export is present on the imported session.
- Messages are correctly marked `imported_untrusted` (`4828-4834`) and skipped for artifact inference (`2732`, `4226`).

Observed behavior:
- A shared/Nostr/file session's todo (and any other extension key) becomes the new session's canonical extension state. History is labeled untrusted; extension blobs are not.

Expected boundary:
- Untrusted import may carry provenance and transcript, not live extension authority, unless the operator opts in.

Failure mechanism:
- Weak data promoted: only one key is stripped; the rest skip the review gate (DAT-015).

Break-it angle:
- Export a session whose `todo.v0` / memory key contains attacker-chosen text; import; the agent treats that todo list as this session's plan.

Impact:
- Prompt-adjacent state from an untrusted transcript becomes local authority.

Operational impact:
- Blast radius: Workflow
- Side-effect class: DB
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unsafe (re-import mints another session with the same blob)

Adjacent failure modes:
- Security-LLM (prompt injection via todo); DAT-004 is *not* lost — provenance exists — but consumers of `todo` do not consult it.

Recommended mitigation:
- Remediation patterns: allowlist imported extension keys.
- Minimal repair: import only `import_provenance`; drop other keys (or keep them behind an `imported_*` namespace).
- Behavior test: exported `todo.v0` is absent (or quarantined) after import.

Implementation assessment:
- Complexity: local_guardrail · Cost: S · Nominal agent: codex

Validation:
- Test: only provenance (and explicitly allowlisted keys) survive import.

Non-goals:
- Do not change transcript import.

### DAT-GSL-004: Migration 24 silently unrestricts workspace tool directories

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/session/session_manager.rs:2506-2516` — migration 24 `UPDATE sessions SET restrict_tools_to_working_dirs = FALSE WHERE workspace_id IS NOT NULL`.
- Comment states older builds force-seeded the flag on for every workspace session; the migration clears it as a policy flip.

Observed behavior:
- Upgrading a DB that has workspace sessions with `restrict_tools_to_working_dirs = TRUE` (whether force-seeded or operator-chosen on a workspace session) rewrites them to unrestricted.

Expected boundary:
- A security-posture column must not change meaning/value on upgrade without an operator-visible migration note or a preserved "explicitly set" bit.

Failure mechanism:
- Shape-preserving meaning change (DAT-008): boolean still exists, its value is rewritten in bulk.

Break-it angle:
- Workspace session with restriction on; run migration 24; tools may leave the working dir.

Impact:
- Silent relaxation of a session isolation control on upgrade.

Operational impact:
- Blast radius: Workflow
- Side-effect class: DB
- Reversibility: compensatable (re-toggle)
- Operator visibility: silent
- Rerun safety: safe (idempotent false)

Adjacent failure modes:
- Security (tool path escape); CMP (posture change).

Recommended mitigation:
- Remediation patterns: preserve explicit operator values; document the flip.
- Minimal repair: only clear rows that can be proven force-seeded, or ship a one-time notice.
- This needs a product decision (human-owner) before a mechanical patch.

Implementation assessment:
- Complexity: governance_decision · Cost: S · Nominal agent: human-owner

Validation:
- Data test: an explicitly restricted workspace session remains restricted if that is the chosen policy.

Non-goals:
- Do not revert the opt-in default for *new* workspace sessions in this slice.

### CON-GSL-004: File-import SHA dedupe is check-then-act

Severity: Medium  
Confidence: Likely  
Evidence basis: source-evidenced  
Domain: Concurrency

Evidence:
- `crates/gosling/src/session/session_manager.rs:939-981` — `import_session_file` hashes the payload, scans `list_all_sessions()` for matching `source_sha256` / `source_path`, then calls `import_session` which always mints a new id (`2783-2789`, `4845-4852`).
- Provenance stores the SHA (`4813-4824`) but there is no UNIQUE index on it.
- JSON/Nostr imports do not even attempt SHA dedupe (by design).

Observed behavior:
- Two concurrent file imports of the same path both pass the scan and create two sessions.

Expected boundary:
- Replay-safe file import: one SHA → one session (or a documented conflict error), enforced by a uniqueness constraint.

Failure mechanism:
- CON-015 check-then-act without a unique key (DAT-002).

Break-it angle:
- Double-click import / CLI+desktop import the same file.

Impact:
- Duplicate canonical sessions; `AlreadyImported` only helps the sequential case.

Operational impact:
- Blast radius: Workflow
- Side-effect class: DB
- Reversibility: compensatable (delete the extra)
- Operator visibility: UI-visible
- Rerun safety: unsafe (another race creates a third)

Recommended mitigation:
- Unique index on provenance SHA (or a side table) + treat UNIQUE violation as `AlreadyImported`.
- Behavior test: two concurrent `import_session_file` calls yield one session.

Implementation assessment:
- Complexity: local_guardrail · Cost: S · Nominal agent: codex

Validation:
- Concurrent import count == 1; sequential replay returns `AlreadyImported`.

Non-goals:
- Do not change JSON/Nostr "always new id" behavior unless product asks.

### CON-GSL-005: WAL NORMAL last-commit loss can re-dispatch a conversation-bound tool

Severity: Medium (High if a power-loss drill is in scope)  
Confidence: Plausible  
Evidence basis: requires-authorized-drill  
Domain: Concurrency

Evidence:
- `crates/gosling/src/session/session_manager.rs:1370-1380` — `journal_mode=WAL`, `synchronous=NORMAL`; comment: "at most the last commit is lost on power failure."
- `crates/gosling/src/agents/agent.rs:1216-1252` — `begin_tool_operation` **commits `started` before** the tool runs.
- `crates/gosling/src/session/session_manager.rs:3659-3717` — if recover does not see the `started` row but the checkpointed `toolRequest` remains, a later `begin` INSERTs a new `started` and returns `Execute`.

Observed behavior (inferred, not drilled):
- Power loss after dispatch begins but before the `started` commit is durable: on reboot recover finds no ledger row; the next `reply`/`begin` re-dispatches the same conversation-bound tool.

Expected boundary:
- Conversation-bound tools must not re-execute when durability of the ledger commit is uncertain. NORMAL already admits last-commit loss; the dispatch order must not turn that into a second side effect.

Failure mechanism:
- Replay after acknowledged-but-not-durable begin (CON-004). Calibration forbids Confirmed without a kill/power drill.

Break-it angle:
- Authorized power-loss / `kill -9` immediately after tool start, before WAL checkpoint.

Impact:
- Duplicate side effects (mail, writes, payments) if the tool already ran in the lost-commit window.

Operational impact:
- Blast radius: Workflow
- Side-effect class: process + external
- Reversibility: irreversible (depends on the tool)
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- CON-GSL-001 (the inverse: recover too eagerly).

Recommended mitigation:
- Remediation patterns: commit-then-execute is already the shape; pair with FULL sync for `tool_operations` or a two-phase "prepare" that recover treats as in-doubt if `started` is missing but the checkpoint exists.
- Behavior test: requires an authorized crash drill.

Implementation assessment:
- Complexity: persistence_recovery · Cost: M · Nominal agent: claude

Validation:
- Drill: crash between begin-commit and tool start / after start; assert no second Execute.

Non-goals:
- Do not switch the whole DB to `FULL` without measuring macOS `F_FULLFSYNC` cost (the comment exists for a reason).

### DAT-GSL-003: `messages.message_id` uniqueness is not a schema constraint

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/session/session_manager.rs:1536-1546` — `messages.message_id TEXT` with only a non-unique index (`1558`).
- `4112-4150` — upsert UPDATEs by `(session_id, message_id)` then INSERTs if `rows_affected == 0`. Inside `BEGIN IMMEDIATE` this is serialized; the DB still accepts two rows with the same `message_id` from any path that INSERTs directly (`add_message` `4061-4074`, import replace loop `4488-4501`).

Observed behavior:
- The application hopes `message_id` is unique per session; the schema does not enforce it. A buggy or future writer can persist duplicates; later UPDATE-by-id becomes ambiguous.

Expected boundary:
- Hoped uniqueness lives in the DB (`UNIQUE(session_id, message_id)`).

Failure mechanism:
- DAT-013 silent constraint: only code enforces the invariant.

Break-it angle:
- Two INSERTs with the same `message_id` (bypass upsert). Both rows remain.

Impact:
- Ambiguous upsert/truncate (`4995-5001` uses `ORDER BY ... LIMIT 1`).

Operational impact:
- Blast radius: Local · Side-effect class: DB · Reversibility: compensatable · Operator visibility: silent · Rerun safety: unknown

Recommended mitigation:
- Add `UNIQUE(session_id, message_id)` in a migration after deduping.

Implementation assessment:
- Complexity: local_guardrail · Cost: S · Nominal agent: codex

Validation:
- DB rejects a second row with the same `(session_id, message_id)`.

Non-goals:
- Do not change upsert semantics.

### DAT-GSL-005: Legacy import marks complete after partial failures

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/session/session_manager.rs:1424-1428` — on first init, `import_legacy` errors are warned, then `mark_legacy_import_complete` always runs.
- `1727-1768` — `import_legacy` returns `Ok(())` even when `failed_count > 0`; failed files are only logged.

Observed behavior:
- A mixed legacy directory: successful files commit (per-session tx, `1817-1828`); failed files never retry because the marker is written.

Expected boundary:
- Mark complete only when every discovered legacy session imported or was skipped for a recorded, idempotent reason.

Failure mechanism:
- Partial failure becomes "done" (CAS-010 / DAT-007). Narrow: only brand-new DBs with leftover `.jsonl` (migration 21 backfills existing DBs as complete, `2464-2475`).

Break-it angle:
- New data dir with one valid and one corrupt legacy file; restart; the corrupt one is gone forever.

Impact:
- Silent loss of some legacy sessions on first run.

Operational impact:
- Blast radius: Local · Side-effect class: DB · Reversibility: irreversible without the original files · Operator visibility: log-only · Rerun safety: unsafe

Recommended mitigation:
- Do not mark complete while `failed_count > 0`; or persist per-file status.

Implementation assessment:
- Complexity: persistence_recovery · Cost: S · Nominal agent: codex

Validation:
- Mixed directory: marker absent; next start retries only the failed name.

Non-goals:
- Do not re-import already-committed ids (PK already prevents that).

### DAT-GSL-006: New-session create and extension apply are two commits

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Data-Integrity

Evidence:
- `crates/gosling-server/src/routes/agent.rs:140-175` — `create_session` (own commit, `2751-2763`) then `update().extension_data(...).apply()` (second `BEGIN IMMEDIATE`, `2861-2865`).
- Sibling `import_session` already threads one tx (`4837-4872`) with an explicit comment that the old three-step form left empty sessions.

Observed behavior:
- Crash between the two commits leaves a listed session with empty `extension_data`.

Expected boundary:
- Create + initial extension state is one transaction (the import/copy pattern).

Failure mechanism:
- CON-009/010 / DAT-007 on the HTTP/ACP create path only.

Break-it angle:
- Kill after create returns and before apply; UI shows a session with default extensions missing.

Impact:
- Recoverable empty session; rerun creates another.

Operational impact:
- Blast radius: Local · Side-effect class: DB · Reversibility: compensatable · Operator visibility: UI-visible · Rerun safety: unsafe

Recommended mitigation:
- Use `create_session_in_tx` + `apply_update_in_tx` like import.

Implementation assessment:
- Complexity: persistence_recovery · Cost: S · Nominal agent: codex

Validation:
- Forced failure after insert → zero listed sessions (or unlisted).

Non-goals:
- Do not change extension resolution.

## Non-Findings / Checked But Not Confirmed

**Secrets durability (historical DAT-GSL-001 / CON-GSL-001) — Held.**  
`write_secrets_file` writes temp, `sync_all`, rename (`config/base.rs:45-69`). `mutate_secrets` holds `lock_secret_transaction` across read+write (`1205-1215`). OAuth `save`/`clear` go through `set_secret` (`oauth/persist.rs:37-53`). Two-instance test at `2179-2214`.

**Corrupt config wipe (historical DAT-GSL-002) — Held.**  
`load_write_config` returns the parse error (`651-663`). `set_param_refuses_to_overwrite_corrupt_config` (`2539-2554`) asserts the file is unchanged.

**Import/copy multi-transaction (historical DAT-GSL-003) — Held.**  
`import_session` / `copy_session` / `import_legacy_session` share one `BEGIN IMMEDIATE` (`1771-1828`, `4837-4872`, `4887-4940`).

**Config torn write / fixed `config.tmp` (historical CON-GSL-003) — Held.**  
UUID temp + `save.lock` across write+rename (`787-828`). In-process high-concurrency test `2109-2175`. Residual is RMW lost-update (CON-GSL-002), not a torn file.

**Grind unbounded loop (historical CAS-GSL-002) — Held.**  
`DEFAULT_MAX_GRIND_NUDGES = 50` (`agent.rs:78-84`, `3073-3097`). Goal path still one-shots via `goal_check_pending`. Tests in `crates/gosling/tests/agent.rs` (`test_grind_nudge_cap_terminates_before_max_turns`).

**Default retry of deterministic 4xx (historical CAS-GSL-001) — Held for `RetryConfig::default()`.**  
`transient_only: true` (`retry.rs:28-36`, `266-271`). Residual is `RetryConfig::new` (CAS-GSL-001 here).

**Retry-After / Google retryDelay abuse — Held.**  
`MAX_RETRY_AFTER_SECS = 3600` (`http_status.rs:37,68`); Google clamp `utils.rs:99-105`.

**Nested AWS + ProviderRetry — Held.**  
Bedrock disables the SDK retry loop (`bedrock.rs:122-128`) so only `ProviderRetry` runs.

**Agent loop rethrow / subagent containment — Held (sampled).**  
Provider errors `break` the reply loop (`2907-3014`). Compaction recovery caps at 2 (`2905-2916`). Stop-hook cap `DEFAULT_STOP_HOOK_BLOCK_CAP = 8` (`77`).

**extension_data merge for tool writers — Held.**  
`merge_extension_state` (`5163-5198`) + concurrent test (`6145-6189`). Residual is the ACP/HTTP whole-blob path (CON-GSL-003).

**Session WAL cross-process row writes — Held.**  
`foreign_keys(true)`, `busy_timeout=30s`, `BEGIN IMMEDIATE` (`1370-1375`). Inventory store uses the same to avoid `SQLITE_BUSY_SNAPSHOT` (`providers/inventory/mod.rs:462-465`).

**Import cannot clobber a local id — Held.**  
`create_session_in_tx` mints `YYYYMMDD_N`. Provenance is recorded (`4813-4826`). Artifact inference skips `imported_untrusted` (`2732`, `4226`). ACP replay surfaces `importedUntrusted` (`acp/server.rs:2519-2520`).

**Round-trip identity — Held as intentional non-equality.**  
Export→import new id, `GoslingMode::Approve`, strips enabled extensions (`7083-7134`).

**Delete orphans — Held.**  
`delete_session` deletes summary facts, summaries, messages, then session (`4716-4751`). Artifacts/tool_ops CASCADE (`2633`, `2663`).

**ToolPermissionStore (historical CON-GSL-005 / DAT-GSL-007) — Not Confirmed / unwired.**  
Only referenced from `permission/mod.rs` and its own file. `save` is temp+rename without fsync (`63-76`) and `record_permission` appends unbounded (`113-115`). Latent if re-wired.

**Lock inversion — Held.**  
`save.lock` is a different file from `.extensions.lock` specifically to avoid self-deadlock (`787-793`).

**DAT-001 workspace IDOR — Held for this local-first product.**  
`get_session` is by bare id; there is no tenant. Workspace is a label, not an authz scope.

**CAS-011 advisory enforcement / CAS-015 alert storms — Held** on the reviewed paths.

**CON-012 / CON-016 / CON-018 — Held** on the reviewed paths.

## Break-It Review

| Attack | Result |
|---|---|
| Kill mid `secrets.yaml` write | **Survives** — destination untouched until rename (`45-69`) |
| Corrupt `config.yaml` then `set_param` | **Survives** — write refused (`651-663`, `2539-2554`) |
| Interrupt session import mid-way | **Survives** — single tx (`4837-4872`) |
| Two processes `set_secret` | **Survives** — flock + test (`1205-1215`, `2179-2214`) |
| Two processes `set_param` | **Breaks** — CON-GSL-002 |
| Desktop `loadSession` during CLI tool | **Breaks** — CON-GSL-001 (Likely) |
| Two `merge_extension_state` writers | **Survives** — IMMEDIATE + test |
| Desktop load vs CLI todo merge | **Breaks** — CON-GSL-003 |
| `/grind` never completes | **Survives** — 50-nudge cap |
| Default provider 400 | **Survives**; Bedrock/Databricks/GCP **Breaks** — CAS-GSL-001 |
| Thinking-block 400 + user retry | **Breaks** at the advice layer — CAS-GSL-002 |
| One dead MCP | **Breaks** whole tool list — CAS-GSL-003 |
| Compacted resume after many new turns | **Breaks** — DAT-GSL-001 |
| Import hostile `todo.v0` | **Breaks** — DAT-GSL-002 |
| Double file import | **Breaks** under concurrency — CON-GSL-004 |
| Power-loss after tool start | **Plausible** — CON-GSL-005 (not drilled) |
| Kill mid-stream | **Leaves** durable prefix — CAS-GSL-004 |
| Migration 24 workspace sessions | **Clears** restriction — DAT-GSL-004 |

## Recommended Patch Order

1. **DAT-GSL-001** (S, codex) — stop injecting stale summaries; highest silent-wrong-context risk.
2. **CON-GSL-001** (M, claude) — lease or recover-only-on-dead-owner; unblocks safe CLI+desktop.
3. **CON-GSL-003** (S, codex) — load/create use `merge_extension_state`.
4. **CON-GSL-002** (M, claude) — flock across config RMW.
5. **CAS-GSL-001 / CAS-GSL-002** (S, codex) — constructor default + terminal advice.
6. **CAS-GSL-003** (S, codex) — isolate optional MCP list failures.
7. **DAT-GSL-002 / CON-GSL-004** (S, codex) — import allowlist + SHA unique.
8. **CAS-GSL-004** (M, claude) — mark/abort partial stream checkpoints.
9. **DAT-GSL-004** (human-owner) — decide migration-24 posture.
10. **DAT-GSL-003 / 005 / 006** (S) — constraint, legacy marker, create tx.
11. **CON-GSL-005** — only with an authorized durability drill.

## Regression Test Strategy

| Test | Purpose | Finding |
|---|---|---|
| Compacted resume with `status=stale` and a coverage gap does not prepend and does not drop middle rows | Freshness gate | DAT-GSL-001 |
| Two SessionManagers: A begins tool, B recover returns 0 | Peer liveness | CON-GSL-001 |
| Two processes `set_param` distinct keys; both survive | Config RMW | CON-GSL-002 |
| Concurrent `merge_extension_state(todo)` vs ACP enabled-extensions write | Whole-blob | CON-GSL-003 |
| `RetryConfig::new` / Bedrock config does not retry 400 | Retry predicate | CAS-GSL-001 |
| Thinking-block 400 user text has no "retry/transient" | Classification | CAS-GSL-002 |
| Healthy+broken MCP → healthy tools + named error | Isolation | CAS-GSL-003 |
| First stream checkpoint is `partial`; recover does not send it as final | Checkpoint | CAS-GSL-004 |
| Imported `todo.v0` absent or quarantined | Promotion | DAT-GSL-002 |
| Concurrent `import_session_file` count == 1 | Dedupe | CON-GSL-004 |
| UNIQUE `(session_id, message_id)` rejects a second insert | Constraint | DAT-GSL-003 |

## Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| DAT-GSL-001 | Data-Integrity | Cascade, Temporal | Stale derived data becomes provider authority |
| CON-GSL-001 | Concurrency | Cascade, Reliability | Recovery poisons a live peer |
| CON-GSL-002 | Concurrency | Reliability | Cross-process settings loss |
| CON-GSL-003 | Concurrency | State-Transition | Session extension lifecycle clobber |
| CAS-GSL-001 | Cascade | Reliability | Retry amplifier on 4xx |
| CAS-GSL-002 | Cascade | Workflow-GUI | Operator told to retry a permanent error |
| CAS-GSL-003 | Cascade | Reliability, Operator-Signal | Optional MCP on critical path |
| CAS-GSL-004 | Cascade | Concurrency, Temporal | Partial stream reused as current |
| DAT-GSL-002 | Data-Integrity | Security-LLM, Cascade | Untrusted todo/memory becomes plan |
| DAT-GSL-004 | Data-Integrity | Security, Compliance-Posture | Isolation flag rewritten on upgrade |
| CON-GSL-004 | Concurrency | Data-Integrity | Duplicate imported sessions |
| CON-GSL-005 | Concurrency | Reliability | Power-loss replay |
| DAT-GSL-003 | Data-Integrity | Concurrency | Hoped uniqueness |
| DAT-GSL-005 | Data-Integrity | Cascade | Partial legacy import becomes "done" |
| DAT-GSL-006 | Data-Integrity | Concurrency | Two-commit create |

## Deferred Risks

- CON-GSL-005 power-loss re-dispatch (needs authorized drill).
- ToolPermissionStore if re-wired (unbounded records, no fsync, fixed `.tmp`).
- `write_secrets_file` still uses a fixed `.tmp` name; flock currently serializes writers — do not drop the lock.
- `refresh_stale_summary_status` (`summarizer/mod.rs:552-561`) can mark a summary `Current` without recomputing when the whole session fits in the tail; harmless for resume today, lying for any consumer that trusts `status` after DAT-GSL-001 is fixed.
- Multi-workspace `get_session(id)` remains globally addressable; only a problem if workspace is later treated as a tenant.

## Residual Risk

- WAL `NORMAL` last-commit loss remains an accepted durability tradeoff (`1376-1380`) even after CON-GSL-005 is designed.
- Compacted resume will still omit history that was never summarized if the gate fails closed — that is visible, not silent-wrong.
- Single-process local use of CLI *or* desktop (not both on one session) does not hit CON-GSL-001.

## Validation Limits

- No `cargo test`, no multi-process harness, no WAL kill drill, no live CLI+desktop run. Oracle-integrity check: in-process tests that were *read* (secrets two-instance, recover second `SessionManager`, grind cap, compacted-not-tested-for-stale) were **not** executed and were not used as `test-reproduced` evidence. `recover_tool_operations`'s second-manager test is source evidence that the topology is intended, not a green-suite claim.
- Not reviewed in depth: Ink UI, Electron renderer stores, every provider beyond retry constructors, tagteam event log, FTS `message_search` rebuild, Nostr crypto, keyring OS atomicity.
- Stop condition: all 48 inventory codes are findings or non-findings; remaining surfaces are below this audit's named focus.

## Final Confidence

Medium-high on repaired historical paths (quoted current code + sibling tests). Medium on CLI+desktop races (guards missing, manifestation not reproduced). Low on power-loss WAL replay (CON-GSL-005).

## Next Action

Patch DAT-GSL-001 (compacted-resume freshness gate) and CON-GSL-001 (recover liveness / lease) before treating concurrent CLI+desktop or compacted load as supported.

## Finding IDs + severities + path

| ID | Severity | Path |
|---|---|---|
| DAT-GSL-001 | High | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| CON-GSL-001 | High | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| CON-GSL-002 | Medium (High if CLI+desktop both write settings) | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| CON-GSL-003 | Medium | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| CAS-GSL-001 | Medium | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| CAS-GSL-002 | Medium | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| CAS-GSL-003 | Medium | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| CAS-GSL-004 | Medium | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| DAT-GSL-002 | Medium | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| DAT-GSL-004 | Medium | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| CON-GSL-004 | Medium | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| CON-GSL-005 | Medium (High if power-loss) | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| DAT-GSL-003 | Low | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| DAT-GSL-005 | Low | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
| DAT-GSL-006 | Low | `docs/cloud/2026-08-15-audit-dataflow-core.md` |
