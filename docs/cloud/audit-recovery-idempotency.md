# Gosling Audit — Recovery & Idempotency Lens

Lens: `audit-recovery-idempotency` (FSR domain, `REC-GSL-NNN` IDs).
Authority: **audit-only / read-only** (per `00-orientation.md`). No source modified.
Method: static walks from `recovery_trace_playbook.md`; no kill/double-run drills were
executed (see Validation Limits). Confidence obeys `evidence_discipline.md`:
missing-guard facts are `Confirmed` from quoted source; crash-timing *manifestations*
are capped at `Likely` / `requires-authorized-drill` unless reproduced.

Focus per tasking: interrupted/partial writes, temp-file handling, transaction
boundaries, external-side-effect idempotency, session resume after crash, config
migration re-run safety, OAuth refresh idempotency, provider retry idempotency.

---

## 1. Recovery & Idempotency Map

| Operation | Side-effect class | Worst interruption point | On-rerun behavior | Idempotency class | Compensation | Safe state |
|---|---|---|---|---|---|---|
| Agent turn: model call → tool exec → persist | process/file/network/external | after a tool side effect, before turn persist (`agent.rs:2680`) | **duplicate** (tool re-issued on resume) | `non-idempotent-unprotected` | none | should be `fail_idempotent` |
| Batch persist of a multi-message turn | DB | between two `add_message` calls (`agent.rs:2680`) | **corrupt→repaired-lossy** (orphaned tool_use stripped by `fix_conversation`) | n/a | `fix_conversation` on read | `fail_resumable` (partial) |
| `config.yaml` write (`save_values`) | file | any point mid-write | clean (atomic temp+lock+fsync+rename) | `naturally-idempotent` | — | `fail_idempotent` ✅ |
| `secrets.yaml` write (`write_secrets_file`) | file (secrets) | between truncate and `write_all` | **corrupt** (truncated/empty secrets file) | n/a | none | should be `fail_idempotent` |
| Session DB write (`add_message`/`replace_conversation`) | DB | any point mid-tx | clean (SQLite WAL + `BEGIN IMMEDIATE`, all-or-nothing) | `naturally-idempotent` per tx | WAL recovery | `fail_rollback` ✅ |
| Session DB schema migration | DB (DDL) | mid-migration | clean (single tx, version-gated, `IF NOT EXISTS`) | `naturally-idempotent` | tx rollback | `fail_rollback` ✅ |
| Config YAML migration (`run_migrations`) | file | mid-save | clean (idempotent transform + atomic save) | `naturally-idempotent` | — | `fail_idempotent` ✅ |
| Provider completion retry (`with_retry_config`) | network/external (tokens) | timeout after server received request | duplicate completion (token/billing cost only; no local effect) | `non-idempotent-unprotected` (benign) | none | at-least-once |
| Provider `refresh_credentials` (databricks v2) | none (cache clear) | any | clean (idempotent cache invalidation) | `naturally-idempotent` | — | ✅ |

---

## 2. Write-Path Atomicity (four answers per material write)

### 2a. `config.yaml` — `Config::save_values` (`crates/gosling/src/config/base.rs:647-688`) — SAFE
1. **Atomic?** Yes. Temp file `config.tmp`, `write_all`, `sync_all()` (fsync),
   then `std::fs::rename(&temp_path, &target_path)` (`base.rs:669-683`) — the
   POSIX temp+fsync+rename idiom.
2. **What does half look like?** A partial `config.tmp`; `config.yaml` itself is
   never truncated (rename is atomic on the same filesystem).
3. **Who detects half?** Readers read `config.yaml`, never `config.tmp`; a torn
   temp is invisible to them.
4. **What repairs half?** Next write opens `config.tmp` with `truncate(true)`,
   overwriting residue. Bounded residue only.
   → **Non-finding.** Idiom = temp+rename, class `naturally-idempotent`.
   (Adjacent concurrency nit — fixed temp name — escalated in §6.)

### 2b. `secrets.yaml` — `write_secrets_file` (`crates/gosling/src/config/base.rs:42-60`) — FINDING (REC-GSL-002)
1. **Atomic?** No. `OpenOptions::new().write(true).create(true).truncate(true)`
   then `file.write_all(...)` **in place** (`base.rs:46-53`). Non-`unix` path uses
   `std::fs::write` — also truncate-in-place.
2. **What does half look like?** The file is truncated to zero first, then written;
   a crash between truncate and `write_all` completing leaves an empty or
   partial-YAML `secrets.yaml`.
3. **Who detects half?** `read_secrets_from_file` (`base.rs:1072-1084`) parses the
   whole file as YAML; a truncated map silently drops keys, or a mangled document
   fails to parse and every `get_secret` becomes `NotFound`. No checksum/footer.
4. **What repairs half?** Nothing. No temp+rename, no backup.
   → **Finding.** Contrast with 2a on the very same struct.

### 2c. Session DB — `add_message` / `replace_conversation_inner` — SAFE per statement
`create_pool` uses WAL journal + `synchronous=Normal` (`session_manager.rs:751-756`)
and every writer runs `pool.begin_with("BEGIN IMMEDIATE")` … `tx.commit()`
(`add_message` `:1605-1634`; `replace_conversation_inner` `:1643-1675`). A crash
mid-transaction rolls back via WAL; `replace_conversation` DELETE+re-INSERT is one
transaction, so it never leaves a partially-replaced conversation.
→ **Non-finding at the single-write level.** The gap is *across* writes (§REC-GSL-001/003).

---

## 3. Crash-Point Enumeration — Agent Turn (the headline)

Steps of one tool-using turn (`crates/gosling/src/agents/agent.rs`, `reply_internal`
stream, lines ~2078-2683):

1. Receive model response with tool requests; `messages_to_add.push(response/request_msg)`
   — **in memory only** (`:2145`, `:2435`).
2. Run permission inspection + execute tools (`handle_approved_and_denied_tools`,
   tool futures at `:2221-2299`) → **external side effects fire** (shell, file edits,
   MCP calls, network).
3. Collect tool responses into `request_to_response_map` → pushed to `messages_to_add`.
4. `for msg in &messages_to_add { session_manager.add_message(...).await? }`
   (`:2680-2682`) — **first durable persistence of the whole turn**, one DB
   transaction *per message*.
5. `conversation.extend(messages_to_add)` in memory (`:2683`).

| Crash point | State left in DB | Rerun/resume behavior | Class |
|---|---|---|---|
| C0 before model call | last user msg only | clean re-ask | clean |
| C1 after model response, before tool exec | nothing new | re-ask; model may re-plan | clean |
| **C2 after a tool side effect, before `:2680`** | **nothing new** (assistant+response still in RAM) | resume loads conv **without** the tool_use/tool_result; model re-issues the same call | **duplicate** |
| **C3 mid-`:2680` batch** (assistant persisted, some tool_results missing) | assistant tool_use with no matching tool_result | on read, `fix_conversation` (`agent.rs:660`) **strips the orphaned tool_use**; history silently loses the executed call | **corrupt→lossy** |
| C4 after full batch persisted | complete turn | clean | clean |

The done-marker (durable turn record) is written **last** (C4) and is not atomic with
the side effects (step 2). Every crash in C2/C3 either duplicates an external effect or
silently erases the record that it happened. There is **no idempotency key** on tool
execution and **no in-progress-turn checkpoint**.

---

## 4. Findings

| ID | Title | Severity | Confidence | Evidence basis |
|---|---|---|---|---|
| REC-GSL-001 | Mid-turn crash replays tool side effects; no tool idempotency / atomic turn boundary | High | Confirmed (gap); duplication Likely | source-evidenced |
| REC-GSL-002 | `secrets.yaml` written non-atomically (truncate-in-place) — crash loses all secrets | Medium | Confirmed | source-evidenced |
| REC-GSL-003 | Multi-message turn persisted as N separate transactions — torn conversation on crash | Low | Confirmed (gap) | source-evidenced |

---

### REC-GSL-001: Mid-turn crash replays tool side effects (no idempotency key, no atomic turn boundary)

Severity: High
Confidence: Confirmed for the missing guard; duplicate-effect manifestation Likely (crash-timing dependent)
Evidence basis: source-evidenced
Domain: Failsafe (REC-004 / REC-007 / REC-010)

Evidence:
- `crates/gosling/src/agents/agent.rs:2078` — `let mut messages_to_add = Conversation::default();` (turn buffer held in memory).
- `crates/gosling/src/agents/agent.rs:2221-2299` — tools are executed (`handle_approved_and_denied_tools`, tool-future stream) and their side effects fire here.
- `crates/gosling/src/agents/agent.rs:2680-2683` — `for msg in &messages_to_add { session_manager.add_message(...).await? }` then `conversation.extend(...)`: the assistant tool-request message and the tool-response messages reach the DB only *after* the tools have already run.
- `crates/gosling/src/agents/agent.rs:660` — on the resume/reply path `fix_conversation` is applied, which (per `crates/gosling-providers/src/conversation.rs:388-468`, "Removed orphaned tool request/response") strips a tool_use that has no persisted tool_result.
- `crates/gosling/src/session/session_manager.rs:1603-1636` — `add_message`; there is no idempotency/dedupe key derived from tool-call id at the execution boundary.

Observed behavior:
- A turn's externally-visible tool effects (shell commands, file edits, MCP tool
  calls, network requests) execute in step 2, but the conversation record of the
  request+response is first persisted in step 4. If the process is killed
  (SIGKILL/OOM/crash/deploy restart) between the two, the DB has no record the call
  happened.

Break-it angle:
- Resume the session after a C2 crash: the loaded conversation ends at the prior user
  message (the in-memory assistant/tool messages were lost). The agent re-sends to the
  provider, which re-proposes the same tool call, and it executes a second time. For a
  non-idempotent tool (`rm`, `git push`, an MCP "send"/"create" tool, a POST) this is a
  duplicate irreversible side effect. `fix_conversation` at `:660` actively *hides* the
  first execution by dropping the orphaned tool_use, so nothing signals the replay.

Impact:
- Duplicate execution of arbitrary tool side effects after any mid-turn interruption;
  silent history loss of an executed action. For a coding/agent framework whose blast
  radius is the user's workstation (per `SECURITY.md`), a double `rm`/push/deploy is the
  material risk.

Operational impact:
- Blast radius: Cross-system (tools reach shell, MCP servers, network).
- Side-effect class: process / file / network / external API.
- Reversibility: irreversible (tool-dependent).
- Operator visibility: silent (orphan stripped; no warning surfaced to the user).
- Rerun safety: unsafe.

Adjacent failure modes: REC-GSL-003 (torn-conversation variant of the same root).

Recommended mitigation:
- Remediation patterns: `checkpoint_resume`, `idempotency_key`, `transaction_boundary`.
- Minimal repair: persist the assistant tool-request message **before** dispatching
  tools (step 1 → durable), so resume sees the tool_use and can reason about it rather
  than silently replaying; and/or record a per-`tool_call_id` execution marker consulted
  before re-dispatch so a completed call is not re-run on resume.
- Behavior test: inject a crash at C2 (env-gated fault after the first tool future
  resolves), resume the session against scratch state, and assert the tool executes
  **exactly once** and the tool_use is present in the reloaded conversation.

Resilience mapping:
- Phase: recover. Objective(s): reconstitute, constrain. Safe state: fail_idempotent.

Failure analysis (FMECA row):
- Failure mode: tool re-executed on resume after mid-turn crash. Likely cause: side
  effects fire before the turn is persisted; no execution dedupe key. Operational phase:
  recovery/cancellation. Local effect: second tool invocation. Workflow effect: duplicate
  external mutation. System-or-operator effect: irreversible double action, hidden.
  Detection method: none. Detection latency: none/never. Operator visible: false.
  Compensating provision: none.

Criticality:
- Likelihood: plausible (any kill during a tool loop). Detectability: silent.

Implementation assessment:
- Complexity: persistence_recovery. Cost: M. Cost drivers: modules (agent loop +
  session store), tests, runtime_verification.
- Nominal agent: claude (touches the core reply stream and session schema; broad blast
  radius warrants context-heavy review).
- Rationale: crosses the agent turn loop and the session persistence layer; needs a
  deliberate turn/checkpoint protocol, not a local guard.

Validation:
- Crash-at-C2 resume test asserting exactly-once tool execution and tool_use presence.

Non-goals:
- Do not change retry counts, provider semantics, or the WAL settings in this slice.

---

### REC-GSL-002: `secrets.yaml` written non-atomically (truncate-in-place)

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe (REC-001 / REC-002)

Evidence:
- `crates/gosling/src/config/base.rs:42-60` — `write_secrets_file`: `OpenOptions`
  with `.create(true).truncate(true)` then `file.write_all(...)`; no temp file, no
  fsync, no rename. Non-unix branch is `std::fs::write` (also truncate-in-place).
- Callers: `write_all_secrets` File branch (`base.rs:981-984`) and the keyring-
  unavailable fallback `write_secrets_to_file` (`base.rs:1101-1106`).
- Reader with no torn-state defense: `read_secrets_from_file` (`base.rs:1072-1084`).
- Contrast (the correct idiom on the same struct): `save_values` (`base.rs:658-683`).

Observed behavior:
- Every secrets write truncates `secrets.yaml` to zero, then rewrites the full JSON/YAML
  map of all secrets. A crash/kill/full-disk between truncate and completion leaves the
  file empty or with a partial map.

Break-it angle:
- SIGINT/OOM mid-write (or ENOSPC on `write_all`) on the file-storage path (keyring
  disabled via `GOSLING_DISABLE_KEYRING`, or keyring unavailable in a headless/SSH
  session — `is_keyring_availability_error`, `base.rs:1116-1128`) yields a truncated
  file. On next read either all keys silently vanish (empty map) or parse fails and every
  `get_secret` returns `NotFound`.

Impact:
- Loss of all locally-stored secrets (provider API keys, OAuth/MCP tokens) when the file
  is the only copy. Recoverable only by re-authenticating every provider. Every secret
  is rewritten on *each* `set_secret`/`delete_secret` (`mutate_secrets`, `base.rs:991-999`
  reads-all then writes-all), so the whole set is exposed to the torn write on any single
  change.

Operational impact:
- Blast radius: Workflow (all providers on the host). Side-effect class: file (secrets).
- Reversibility: irreversible (data loss) but recoverable via re-auth.
- Operator visibility: log-only at read time ("NotFound") — cause not surfaced.
- Rerun safety: safe (re-writing fixes it once you still have the values in memory).

Recommended mitigation:
- Remediation pattern: `transactional_write`.
- Minimal repair: route `write_secrets_file` through the same temp+fsync+rename idiom
  already implemented in `save_values` (preserve the `0o600` mode on the temp file).
- Behavior test: truncate a copy of `secrets.yaml` at N bytes, feed to
  `read_secrets_from_file`, assert it does not silently return a smaller map than
  written; and assert an interrupted write never leaves a zero-length live file.

Resilience mapping:
- Phase: recover. Objective(s): prevent_avoid, reconstitute. Safe state: fail_idempotent.

Failure analysis (FMECA row):
- Failure mode: torn secrets file. Likely cause: truncate-in-place write. Operational
  phase: normal_run. Local effect: empty/partial file. Workflow effect: all secrets lost.
  System-or-operator effect: providers stop authenticating. Detection method: downstream
  `NotFound`. Detection latency: delayed. Operator visible: false (cause hidden).
  Compensating provision: none.

Criticality:
- Likelihood: unlikely (narrow crash window, file path only). Detectability: inferred.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: tests, runtime_verification.
- Nominal agent: codex (single-function change with a fixture test).
- Rationale: reuse the existing atomic idiom from the same file; bounded.

Validation:
- Truncated-fixture read test + interrupted-write test asserting no zero-length live file.

Non-goals:
- Do not change keyring behavior or the secrets format.

---

### REC-GSL-003: Multi-message turn persisted as N independent transactions

Severity: Low
Confidence: Confirmed (gap); torn-state manifestation Likely
Evidence basis: source-evidenced
Domain: Failsafe (REC-003 / REC-007)

Evidence:
- `crates/gosling/src/agents/agent.rs:2680-2682` — the turn's messages are persisted in
  a `for` loop, each `add_message` opening its **own** `BEGIN IMMEDIATE` transaction
  (`session_manager.rs:1605`). There is no enclosing transaction spanning the batch.
- Repair-on-read: `fix_conversation` (`agent.rs:660`; `conversation.rs:388-468`).

Observed behavior:
- A crash after persisting the assistant tool_use message but before its tool_result
  message leaves the DB with a tool_use that has no matching tool_result (crash-point C3).

Break-it angle:
- On reload the torn pair is not sent raw to the provider (which would 400 on Anthropic);
  `fix_conversation` strips the orphaned tool_use. The provider call is thus protected,
  but the executed action is silently deleted from history — feeding REC-GSL-001's replay.

Impact:
- Silent, lossy history repair; no corruption surfaced. Its real weight is as the
  DB-side manifestation of REC-GSL-001; on its own it is Low.

Operational impact:
- Blast radius: Workflow. Side-effect class: DB. Reversibility: irreversible (record lost).
- Operator visibility: log-only (`debug!` at `agent.rs:662`). Rerun safety: unsafe (via 001).

Recommended mitigation:
- Remediation pattern: `transaction_boundary`.
- Minimal repair: persist a turn's messages within a single transaction (add a
  `replace_conversation`-style batched writer, or wrap the `:2680` loop in one tx) so a
  turn is all-or-nothing in the DB.
- Behavior test: crash between the assistant-message insert and the tool-result insert;
  assert the reloaded conversation contains either both or neither.

Resilience mapping:
- Phase: recover. Objective(s): reconstitute. Safe state: fail_resumable.

Failure analysis (FMECA row):
- Failure mode: half-persisted turn. Likely cause: per-message transactions. Operational
  phase: recovery. Local effect: dangling tool_use. Workflow effect: lossy repair on read.
  System-or-operator effect: feeds duplicate-execution (001). Detection method: debug log.
  Detection latency: delayed. Operator visible: false. Compensating provision:
  `fix_conversation` (partial).

Criticality:
- Likelihood: unlikely (narrow window). Detectability: logged.

Implementation assessment:
- Complexity: persistence_recovery. Cost: S. Cost drivers: modules, tests.
- Nominal agent: codex. Rationale: localized to the session store + one call site.

Validation:
- Interrupted-batch test asserting turn atomicity in the DB.

Non-goals:
- Fold into the REC-GSL-001 fix if that repair already persists the turn atomically.

---

## 5. Non-Findings (checked and held)

- **Config file atomic write** — `save_values` uses temp file + `flock` exclusive +
  `sync_all` + `std::fs::rename` (`base.rs:658-683`). Idiom = temp+rename;
  class `naturally-idempotent`. Symlink target is resolved first (`base.rs:612-645`) so
  the write follows the link rather than clobbering it. Held.
- **Session DB durability & migration re-run safety** — WAL + `synchronous=Normal`
  (`session_manager.rs:751-756`); schema creation and migrations run inside
  `BEGIN IMMEDIATE` transactions with `IF NOT EXISTS` DDL and `INSERT OR IGNORE` for the
  version row (`create_schema` `:804-914`, `run_migrations` `:1014-1037`). Migrations are
  version-gated (`current_version < CURRENT_SCHEMA_VERSION`, per-step
  `update_schema_version` inside the same tx) and each individual `apply_migration` step
  guards additive DDL with `pragma_table_info` existence checks (`:1197-1325`). A
  mid-migration crash rolls back the whole transaction; a re-run is safe and idempotent.
  The `pool()` probe deliberately propagates `SQLITE_BUSY` rather than treating an error
  as "no schema" (`:771-796`), avoiding a version-stamp-that-skips-migrations bug. Held —
  strong.
- **Config YAML migrations** (`config/migrations.rs`) — `run_migrations` transforms are
  idempotent (tested: `test_migrate_platform_extensions_idempotent`,
  `test_migrate_provider_config_idempotent`), and their persistence goes through the
  atomic `save_values` (`base.rs:541-545`). `run_read_migrations` on the read path mutates
  only the in-memory merged map and never writes. A half-applied migration re-runs
  cleanly. Held.
- **Provider `refresh_credentials`** (databricks v2, `databricks_v2.rs:371-375`) — clears
  the token cache and invalidates the secrets cache; naturally idempotent, no external
  side effect, safe to call repeatedly (retry.rs invokes it once per auth error). Held.
- **Provider retry auth path** (`retry.rs:196-259`) — auth refresh is bounded to a single
  attempt (`auth_retried`), separate from the transient-error counter; permanent 4xx
  markers are never retried (`is_permanent_request_failure`, `:88-97`). Held.

---

## 6. Cross-Lens Escalations

- **Concurrency (`audit-dataflow-concurrency`)** — `save_values` writes to a **fixed**
  temp path `target_path.with_extension("tmp")` (`base.rs:659`). The in-process `guard`
  mutex (`base.rs:157`) does not span processes; two gosling processes (CLI + desktop +
  server can share `~/.config/gosling`) writing config concurrently both open the same
  `config.tmp` with `truncate(true)` *before* acquiring `flock` (`base.rs:662-673`),
  so one can truncate the other's in-flight temp. Final `config.yaml` is still one whole
  write or the other (rename), but a lost update is possible. This is the two-writers axis,
  not interrupt-and-rerun — routed out of this lens.
- **External-API pipeline (`audit-pipeline-externalapi`)** — provider completions are
  retried on `NetworkError`/`ServerError`/`RateLimitExceeded` and even `RequestFailed`
  (`retry.rs:99-108`) without confirming the prior attempt failed; a timeout after the
  server received the request produces a duplicate completion (at-least-once). Benign for
  local state (no side effect beyond token cost), but with `OPENAI_STORE`
  (`base.rs:1268-1270`) a retried request can store a duplicate server-side record. Route
  the billing/store-duplication question there.
- **Operator-signal (`audit-operator-signal`)** — the silent history repair
  (`fix_conversation` at `agent.rs:660` logs only at `debug!`) and the silent secrets-loss
  cause (REC-GSL-002 surfaces only as downstream `NotFound`) are detection-quality gaps.

---

## 7. Residual Risk Register

| Risk | Retained because | Mitigation if accepted |
|---|---|---|
| Duplicate tool side effect on mid-turn crash (REC-GSL-001) | No turn checkpoint / execution dedupe key today | Persist tool_use pre-dispatch + per-tool-call execution marker |
| Secrets loss on torn write (REC-GSL-002) | File path uses truncate-in-place | Reuse the atomic temp+rename idiom already in `save_values` |
| Provider at-least-once completions | Retries do not confirm prior-attempt failure | Acceptable for LLM calls; revisit if a store/side-effecting endpoint is added |

---

## 8. Break-It Review (angles exercised, statically)

- **Interruption angle:** kill at agent-turn C2/C3 → duplicate tool exec / torn
  conversation (REC-GSL-001/003). Kill mid `write_secrets_file` → empty secrets
  (REC-GSL-002).
- **Rerun angle:** config/session/YAML-migration reruns converge (atomic + idempotent) —
  held. Agent turn rerun after crash does **not** converge — replays tools.
- **Resume angle:** session resume loads via `get_conversation` →
  `Conversation::new_unvalidated` (`session_manager.rs:1600`), repaired by
  `fix_conversation` on the reply path; repair is lossy (silently drops orphaned
  executed calls) rather than quarantine/flag.
- **Retry angle:** provider retry distinguishes permanent vs transient and bounds auth
  refresh to one; retry layers do not stack destructively (single application-level loop).
- **Rollback angle:** DB writes roll back per transaction (WAL); tool side effects have
  no rollback/compensation.
- **Marker angle:** the durable turn record is written last and is not atomic with the
  side effects it should gate — the core REC-GSL-001 defect.

---

## 9. Validation Limits (what was NOT proven / reviewed)

- **No live drills executed.** All findings are static (`source-evidenced` /
  `simulation-reasoned`). No injected-crash, double-run, truncated-fixture, or timeout
  reproduction was run — this session is read-only. Upgrading REC-GSL-001's *duplication*
  and REC-GSL-002's *torn-file* manifestations to `Confirmed`/`test-reproduced` requires
  the crash-at-C2 and truncated-`secrets.yaml` tests named above (`requires-authorized-drill`
  for the kill-timing outcomes).
- **rmcp `AuthorizationManager` internals not audited** — MCP OAuth token refresh
  (`oauth/mod.rs:94` `auth_manager.refresh_token()`), including rotating-refresh-token
  handling and concurrent-refresh safety across multiple connections, lives in the
  external `rmcp` crate and was not traced. Each `oauth_flow` builds its own
  `AuthorizationManager` with no cross-instance lock (`oauth/mod.rs:89-91`); a concurrent
  double-refresh against a rotating provider is a plausible-but-unverified risk.
- **Not all provider `refresh_credentials` impls traced** — only databricks v2 read;
  `gcpauth.rs`, `databricks.rs` (v1) not examined.
- **`session_manager.rs` read to ~line 1682 of 3728** — `copy_session`, `import_session`,
  `export_session`, `truncate_conversation*`, and the legacy-import write paths were not
  fully audited for atomicity/idempotency.
- **ACP subprocess bridges** (Claude/Codex/Gemini/Cursor CLI resume semantics) and the
  `gosling-server` request layer were out of scope for this lens pass.
- **Fork provenance:** several patterns are inherited from goose v1.38; findings are
  scored on present-code mechanism regardless of origin (per `00-orientation.md §6`).
