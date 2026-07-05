# Gosling Audit — Dataflow / Concurrency Lens (`CON`)

Lens: `audit-dataflow-concurrency` v3.1. Authority: **audit-only / read-only**.
Builds on `docs/cloud/00-orientation.md`. Only this file was written; no source
was modified.

Focus: concurrent / duplicate execution in async Rust (tokio) — races, lost
updates, double-processing, replay/retry collisions, stale reads/writes,
ordering deps, shared-state (`Arc<Mutex>`/`RwLock`) misuse, TOCTOU,
Mutex-held-across-`.await`, double-checked patterns, non-atomic file writes.

> The supplied prompt is treated as a draft. I preserved the intended mission
> (the named priority surfaces) but expanded review to the secrets-file write
> path and the cross-process deployment posture (CLI + desktop + server share
> `~/.config/gosling/`), which are the material seams the named files sit on.

Calibration note (mandatory addendum applied): race/lost-update *manifestation*
is runtime-dependent. Per `confidence_calibration.md` I mark the missing
guard / non-atomic pattern from source, and cap the *observed collision* at
Likely/Plausible unless reproduced. No concurrency was executed in this run.

---

## 1. Surface inventory & boundary map

| State / Artifact | Writers | Cross-process? | Guard in place | Hazard |
|---|---|---|---|---|
| `sessions.db` (SQLite) | session_manager | yes (WAL) | `BEGIN IMMEDIATE` + `busy_timeout=30s` + guarded conditional UPDATE | held (see non-findings) |
| session `extension_data` column | todo / enabled-extensions / memory tools via `apply_update` | in-process | none (whole-blob overwrite) | **CON-GSL-004 lost update** |
| `config.yaml` | `set_param`/`update_param`/`delete` | yes | in-proc `guard: Mutex<()>`; atomic temp+rename | **CON-GSL-002 cross-proc RMW** |
| `config.tmp` (fixed temp path) | `save_values` | yes | `flock` on temp | **CON-GSL-003 fixed-artifact race** |
| `secrets.yaml` (file backend + keyring fallback) | `set_secret`/`delete_secret`/OAuth refresh | yes | in-proc `guard`; **direct truncate write, NO rename** | **CON-GSL-001 non-atomic + secret loss** |
| OS keyring blob | `write_all_secrets` | keyring-serialized | keyring | held (atomic single-key write) |
| `KEYRING_RUNTIME_DISABLED` flag | keyring fallback path | in-proc | `AtomicBool` | held (see non-findings) |
| `param_cache` / `secrets_cache` | `load`/`all_secrets` | in-proc | `Mutex`, double-check | held (benign) |
| `tool_permissions.json` (`ToolPermissionStore`) | `record_permission` | — | temp+rename write; whole-struct RMW | **CON-GSL-005 (latent / unwired)** |

Actors: operator (CLI/TUI/desktop), LLM provider (untrusted, emits parallel
tool calls), MCP/ACP subprocesses. Trust boundary per orientation §4.

---

## 2. Findings

### CON-GSL-001: `secrets.yaml` written non-atomically — crash or concurrent read can lose ALL API keys / OAuth tokens

Severity: High
Confidence: Confirmed (non-atomic write pattern) / Likely (data-loss manifestation)
Evidence basis: source-evidenced
Domain: Concurrency (secondary: Data-Integrity, Security)

Evidence:
- `crates/gosling/src/config/base.rs:42-60` `write_secrets_file` opens the
  **final** path with `.truncate(true)` then `file.write_all(...)` — it streams
  directly to the destination; there is no temp-file + `rename`.
- Live callers: `write_all_secrets` File backend `base.rs:981-984`; keyring
  fallback `write_secrets_to_file` `base.rs:1101-1106` (invoked at
  `base.rs:1148,1170`).
- Contrast: the non-secret `config.yaml` path `save_values` `base.rs:647-688`
  *does* write `config.tmp` then `std::fs::rename` (atomic). Secrets get the
  weaker guarantee than plain config.

Observed behavior:
- A secret write truncates `secrets.yaml` to zero, then writes the full YAML.
  Between truncate and completed write the file is empty/partial on disk.

Expected boundary:
- Secret persistence should be atomic (write-temp-then-rename), matching the OS
  keyring's all-or-nothing guarantee and the sibling `config.yaml` path.

Failure mechanism:
- A crash / kill / power loss / disk-full **between truncate and `write_all`**
  leaves a truncated or empty `secrets.yaml`. On next load
  (`read_secrets_from_file` `base.rs:1072-1084`) an empty/partial file yields
  `HashMap::new()` or a parse error → every stored secret (all provider API
  keys, bearer tokens, and OAuth `StoredCredentials` written via
  `oauth/persist.rs`) is gone.
- Concurrent-reader angle: a `get_secret` in another process/thread reading
  `secrets.yaml` mid-write parses truncated YAML → `DeserializeError` or empty
  map → transient "credential not found".

Break-it angle:
- Kill the process during `set_secret` after a provider re-auth; reopen — the
  secrets file is empty and the user must re-enter every key.

Impact:
- Irreversible loss of all locally-stored secrets (keyring-disabled installs and
  keyring-unavailable/headless/SSH sessions that fell back to file). User-visible
  as sudden total de-auth; no recovery path (no backup kept).

Operational impact:
- Blast radius: Service (all providers for that user)
- Side-effect class: file
- Reversibility: irreversible (secrets not recoverable)
- Operator visibility: silent until next provider call fails
- Rerun safety: safe (but does not restore lost secrets)

Adjacent failure modes:
- CON-GSL-002 (cross-process RMW on same file), CON-GSL-003 (fixed temp path).

Recommended mitigation:
- Remediation pattern: atomic-write (temp + `sync_all` + `rename`), reuse the
  existing `save_values` sequence for the secrets path; keep `0o600` mode on the
  temp file.
- Minimal repair: route `write_secrets_file` through a temp-then-rename helper.
- Behavior test: write a large secrets map, inject a panic before rename, assert
  the previous `secrets.yaml` is intact.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: 1 module, 1 test
- Nominal implementation agent: codex
- Rationale: single-function change mirroring an existing atomic path in the same
  file; low risk, easily tested.

Validation:
- Test: crash-between-writes leaves prior file readable; concurrent read never
  observes a partial file (assert file content is always valid YAML).

Non-goals:
- Do not add cross-process file locking here (that is CON-GSL-002).

---

### CON-GSL-002: Config / secrets read-modify-write is atomic only in-process — cross-process concurrent writes lose updates

Severity: Medium (High if reachable on multi-process installs)
Confidence: Likely (missing guard) / Plausible (collision manifestation)
Evidence basis: source-evidenced (guard scope) + simulation-reasoned (collision)
Domain: Concurrency (secondary: Security — OAuth token loss)

Evidence:
- In-process serialization only: `set_param` `base.rs:865-870`, `update_param`
  `base.rs:835-850`, `delete` `base.rs:899-907`, and `mutate_secrets`
  `base.rs:991-999` each take `let _guard = lock_ignoring_poison(&self.guard)`
  — an instance `Mutex<()>` (`base.rs:157`). `GLOBAL_CONFIG` is one `OnceCell`
  per process (`base.rs:200,447-449`), so `guard` serializes threads **within a
  process only**.
- The cycle is read (`load_write_config` / `all_secrets`) → mutate → write
  (`save_values` / `write_all_secrets`). Nothing holds a cross-process lock
  across the read and the write. `save_values`' `flock` (`base.rs:672`) covers
  only the write of `config.tmp`, not the earlier read.
- Orientation §1/§4: CLI, desktop (`ui/desktop`), and server
  (`crates/gosling-server`) are distinct processes that all read/write the same
  `~/.config/gosling/config.yaml` and `secrets.yaml`.

Observed behavior:
- Two processes each load the file, each set a different key, each write the
  whole mapping back. The second writer's mapping was read before the first
  writer's change landed, so the rename overwrites it.

Expected boundary:
- A read-modify-write on a shared config/secret file must be atomic across every
  writer (advisory file lock held across read+write, or an OS-keyring/DB store).

Failure mechanism:
- Classic lost update / check-then-act across processes: the compare/serialize is
  against a stale in-memory copy; last `rename` wins and silently drops the
  other process's edit.
- OAuth angle: `oauth/persist.rs` `save()` → `config.set_secret` →
  `mutate_secrets`. Two providers refreshing tokens concurrently in two
  processes → one refreshed `StoredCredentials` is lost → that provider silently
  reverts to a stale/expired token.

Break-it angle:
- Run `gosling configure` (CLI) while the desktop app writes a provider key;
  one of the two edits vanishes from `config.yaml`.

Impact:
- Silent loss of a config key or a refreshed OAuth credential; downstream effect
  is a provider that appears configured but uses stale/missing state.

Operational impact:
- Blast radius: Service
- Side-effect class: file
- Reversibility: compensatable (re-enter), token refresh may auto-recover
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- CON-GSL-001, CON-GSL-003.

Recommended mitigation:
- Remediation pattern: cross-process advisory lock held across the full
  read-modify-write (e.g. `flock` a dedicated `config.lock` for the whole
  `update_param`/`mutate_secrets` body), or move secrets to the keyring/DB.
- Behavior test: two processes each set a distinct key concurrently; assert both
  keys survive.

Implementation assessment:
- Complexity: cross_process_coordination
- Cost: M
- Cost drivers: lock lifecycle, 2 modules (config + secrets), multi-process test
- Nominal implementation agent: claude
- Rationale: correct lock scoping across read+write and a real multi-process test
  harness; broader than a one-function fix.

Validation:
- Test: concurrent cross-process writers preserve every key; OAuth double-refresh
  keeps both providers' latest tokens.

Non-goals:
- Do not convert the whole config store to SQLite in this slice.

---

### CON-GSL-003: Fixed `config.tmp` path is a shared artifact across processes

Severity: Low (Medium if reachable)
Confidence: Plausible
Evidence basis: source-evidenced (fixed path) + simulation-reasoned (collision)
Domain: Concurrency (CON-017 artifact reuse)

Evidence:
- `base.rs:659` `let temp_path = target_path.with_extension("tmp");` — a single
  deterministic path (`config.tmp`) with no pid/uuid component. Two processes
  saving config target the same temp inode.
- Partial mitigation: `file.lock_exclusive()` (`base.rs:672`) serializes the
  temp write, and `sync_all` precedes `rename` (`base.rs:677,683`).

Observed behavior:
- Concurrent `save_values` in two processes contend on one `config.tmp`; the
  `flock` serializes the *write*, but the subsequent unguarded `rename` from each
  process can still interleave (each renames its own written temp content over
  `config.yaml`).

Expected boundary:
- Per-run isolated temp names (`config.<pid>.<uuid>.tmp`) so no two writers ever
  share the staging file.

Failure mechanism:
- Artifact reuse race: because the flock is released on drop *before* `rename`,
  process B can acquire the same temp inode / re-create `config.tmp` in the
  window between A's unlock and A's rename, muddying which content lands.

Break-it angle:
- Two rapid concurrent `save_values` calls from separate processes; observe
  which content wins is non-deterministic.

Impact:
- Reinforces CON-GSL-002's lost-update; on its own bounded to config staging.

Operational impact:
- Blast radius: Local/Service
- Side-effect class: file
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes: CON-GSL-002.

Recommended mitigation:
- Remediation pattern: unique temp filename per write; keep the atomic rename.
- Behavior test: two writers never observe each other's temp file (unique names).

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: 1 line + 1 test
- Nominal implementation agent: codex
- Rationale: trivial rename to a unique staging path.

Validation:
- Test: temp path contains a per-process unique component.

Non-goals:
- The cross-process RMW lock is CON-GSL-002, not this slice.

---

### CON-GSL-004: `extension_data` persisted as whole-blob read-modify-write — concurrent tools in a turn can lose each other's state

Severity: Medium
Confidence: Plausible (reachability depends on concurrent tool execution)
Evidence basis: source-evidenced (RMW pattern) + simulation-reasoned (interleave)
Domain: Concurrency (CON-002 lost update / CON-007 stale write)

Evidence:
- `apply_update` binds the **entire** `extension_data` column
  (`session_manager.rs:1492-1494`, `serde_json::to_string(&ed)`) — no per-key
  merge at the DB layer.
- Callers do read-modify-write on the whole blob:
  - todo tool: `platform_extensions/todo.rs:85-96` — `get_session(false)` →
    `todo_state.to_extension_data(&mut session.extension_data)` →
    `.extension_data(session.extension_data).apply()`.
  - enabled-extensions: `acp/server.rs:1181-1185` clones `session.extension_data`,
    mutates, writes back.
- Tool futures are combined and polled concurrently:
  `agent.rs:2243-2261` collects `tool_futures` into `stream::select_all` and
  drives them with `tokio::select!` / `combined.next()`. The LLM (untrusted per
  orientation §4) can emit multiple `extension_data`-mutating tool calls in one
  assistant turn.

Observed behavior:
- Two tools each read the same base `extension_data`, each set their own
  sub-state, each write the full blob. The later write, built from the pre-first
  base, drops the first tool's sub-state.

Expected boundary:
- Concurrent updates to distinct sub-keys of one session's `extension_data` must
  both survive (merge-on-write under a row lock, or a `json_patch`/`json_set`
  UPDATE, or serialized per-session writes).

Failure mechanism:
- Whole-column overwrite + non-atomic read-then-write; the DB `BEGIN IMMEDIATE`
  serializes each *write* but not the read→write pair, and the app supplies a
  stale full snapshot.

Break-it angle:
- Prompt the model to update the todo list and toggle an extension in the same
  turn; if their futures interleave, one change is missing after reload.

Impact:
- Lost todo/enabled-extension/memory state within a session; user-visible as a
  reverted todo or an extension that re-disables itself. Bounded to one session.
- Reachability caveat: if `dispatch_tool_call` (`agent.rs:960`) fully completes
  each platform tool's DB write *before* returning (i.e. the pushed future is
  already resolved), execution is effectively sequential and the race does not
  fire. I did not trace every platform-tool dispatch to closure, so this stays
  **Plausible**, not Confirmed.

Operational impact:
- Blast radius: Workflow (single session)
- Side-effect class: DB
- Reversibility: compensatable (re-issue the tool)
- Operator visibility: silent / UI-visible (missing todo)
- Rerun safety: safe

Adjacent failure modes: none outside session state.

Recommended mitigation:
- Remediation pattern: merge `extension_data` at write time inside the
  `BEGIN IMMEDIATE` tx (read current column in-tx, deep-merge the delta, write),
  or expose a targeted `set_extension_state` DB update instead of whole-blob.
- Behavior test: two concurrent sub-key updates on one session both persist.

Implementation assessment:
- Complexity: persistence_recovery
- Cost: M
- Cost drivers: session_manager write path, in-tx merge, concurrency test
- Nominal implementation agent: codex
- Rationale: contained to the session write path but needs an in-transaction
  merge and a real concurrent test.

Validation:
- Test: spawn two `apply_update`s setting different `extension_data` sub-keys on
  the same session; assert both keys present after reload.

Non-goals:
- Do not restructure `ExtensionData`'s schema.

---

### CON-GSL-005: `ToolPermissionStore` whole-file RMW would lose permission grants if wired — currently unreached

Severity: Low (Medium-if-wired)
Confidence: Speculative (no live caller found)
Evidence basis: source-evidenced (pattern) + simulation-reasoned (reachability)
Domain: Concurrency (CON-002 / negative-space)

Evidence:
- `permission/permission_store.rs:92-117` `record_permission` mutates `&mut self`
  (in-memory `HashMap`) then `save()` writes the **whole struct** to
  `tool_permissions.json` (temp+rename, `:63-77` — write itself is atomic).
- No runtime caller: grepping `crates/**` shows `load`, `record_permission`,
  `check_permission` referenced only inside the permission module and its
  re-export (`permission/mod.rs:10`). The live permission path instead uses
  `tool_inspection_manager.update_permission_manager(...)`
  (`agents/tool_execution.rs:151-153,165-167`) writing `PermissionLevel` through
  the (in-proc-guarded) config store.

Observed behavior (if wired):
- Two `ToolPermissionStore` instances loaded from disk, each records a grant,
  each serializes its whole in-memory map back → last writer erases the other's
  grant/deny.

Expected boundary:
- If reintroduced, permission records must be appended atomically (per-record
  write or in-tx merge), not whole-file overwrite from a stale snapshot.

Break-it angle:
- Wire two concurrent `record_permission` calls; one grant silently disappears.

Impact:
- If wired: a persisted "always allow/deny" decision could be lost, re-prompting
  or (worse) dropping a deny. Not reachable today.

Operational impact:
- Blast radius: Local
- Side-effect class: file
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Recommended mitigation:
- If reactivated: hold a lock across load→record→save, or append per-record.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Nominal implementation agent: codex
- Rationale: guardrail only relevant if the module is re-wired; otherwise a
  dead-code cleanup candidate (see Skill Escalation).

Validation:
- Test (guard-if-wired): two concurrent records both persist.

Non-goals:
- Do not wire the store; confirm dead-code status first.

---

## 3. Non-findings (checked and held)

- **Session DB writes are transaction-guarded.** Every mutating path opens
  `pool.begin_with("BEGIN IMMEDIATE")` — `create_session`
  (`session_manager.rs:1342`), `add_message` (`:1605`),
  `replace_conversation_inner` (`:1643`), `apply_update` (`:1535`),
  `delete_session` (`:1866`), migrations (`:1015`), schema create (`:817`) —
  with `busy_timeout=30s` and WAL (`:750-756`). SQLite serializes writers
  across processes. Held.
- **Session id generation** (`create_session` `:1345-1371`) computes
  `MAX(...)+1` **inside** the `BEGIN IMMEDIATE` tx, so concurrent creates cannot
  collide on the same id. Held.
- **Auto-name vs user-rename race is explicitly guarded.** The background
  auto-namer uses a conditional UPDATE `... AND user_set_name = 0`
  (`apply_update` `:1471-1476`) and treats 0-rows-affected as a benign lost race
  when the row still exists (`:1539-1559`). This is the correct atomic
  check-then-act collapse; a concurrent user rename is not clobbered. Held.
- **`KEYRING_RUNTIME_DISABLED`** is an `AtomicBool` (`base.rs:28,1144,1168`),
  deliberately replacing a prior `env::set_var` data race (documented at
  `:22-28`). Correct. Held.
- **Config `param_cache` double-check** (`load` `:552-591`): two threads may both
  miss and both parse, but each returns a valid `Arc<Mapping>` and the cache
  ends consistent (last write wins, same data). Benign redundant work, not a
  correctness race. Held.
- **In-process config/secret serialization** via `guard: Mutex<()>` is correct
  for single-process concurrency; the residual risk is strictly cross-process
  (CON-GSL-002). Held for the single-process case.
- **`permission_store.save`** itself uses temp+rename (`:63-77`) — the file write
  is atomic; only the (unwired) whole-struct RMW is the concern (CON-GSL-005).
- **Mutex-across-await scan:** `config` uses `std::sync::Mutex` only around
  synchronous cache/flag access (no `.await` under the guard). `tool_result_rx`
  uses `tokio::sync::Mutex` and is `.await`ed while held
  (`tool_execution.rs:188`) — legitimate async mutex use, single-consumer; no
  deadlock path traced. Held.

---

## 4. Break-it review summary

| Attack | Result |
|---|---|
| Two concurrent session creates → duplicate id | Prevented (MAX+1 in `BEGIN IMMEDIATE`). |
| Auto-name overwrites user rename | Prevented (guarded conditional UPDATE). |
| Kill mid `secrets.yaml` write | **Breaks** — truncated/empty secrets (CON-GSL-001). |
| Two processes each set a config key | **Lost update** likely (CON-GSL-002). |
| Two providers refresh OAuth token cross-process | **Lost update** plausible (CON-GSL-002). |
| Concurrent `config.tmp` writers | Mostly serialized by flock; residual rename race (CON-GSL-003). |
| Two `extension_data` tools in one turn | **Lost update** plausible if futures interleave (CON-GSL-004). |
| Replay same tool request twice | Not a persistence duplicate (SQLite path idempotent per message uuid); no finding. |

---

## 5. Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| CON-GSL-001 | Concurrency | Security / Data-Integrity | Non-atomic write of the secret store; total credential loss is a security + integrity event. |
| CON-GSL-002 | Concurrency | Security | Cross-process RMW can drop a refreshed OAuth token / provider key. |
| CON-GSL-003 | Concurrency | Reliability | Shared staging artifact; hardening of the atomic-write path. |
| CON-GSL-004 | Concurrency | State-Transition | Session `extension_data` lifecycle state lost across concurrent tool writes. |
| CON-GSL-005 | Concurrency | Negative-Space / Dead-code | Latent hazard in an apparently unwired module; confirm dead-code vs re-wire. |

---

## 6. Patch order (highest value first)

1. **CON-GSL-001** (S, codex) — atomic secrets write; prevents irreversible
   secret loss. Reuse the existing `save_values` temp+rename sequence.
2. **CON-GSL-002** (M, claude) — cross-process lock across config/secret RMW;
   also mitigates OAuth token loss.
3. **CON-GSL-004** (M, codex) — in-transaction `extension_data` merge.
4. **CON-GSL-003** (XS, codex) — unique temp filename.
5. **CON-GSL-005** — decide dead-code removal vs guard-if-wired.

---

## 7. Regression / guardrail tests to add

- Crash-between-writes leaves `secrets.yaml` a valid prior file (CON-GSL-001).
- Two-process concurrent distinct-key config writes preserve both keys
  (CON-GSL-002).
- OAuth double-refresh across processes keeps both providers' latest tokens
  (CON-GSL-002).
- Concurrent `apply_update` on two `extension_data` sub-keys → both persist
  (CON-GSL-004).
- `config` temp path contains a per-process unique component (CON-GSL-003).

Note: the repo already has `test_concurrent_writes` (`base.rs:1556+`) but it
exercises an external `Mutex`-guarded map, not the cross-process RMW — it does
**not** cover CON-GSL-002. Session tests exercise sequential reload only.

---

## 8. Validation Limits (what was NOT reviewed / not proven)

- **No concurrency was executed.** All race/lost-update *manifestations* are
  Likely/Plausible per calibration, not runtime-observed. None reproduced with a
  failing test.
- `dispatch_tool_call` (`agent.rs:960-1060`) was not fully traced to determine
  whether platform-tool DB writes complete eagerly (sequential) or lazily
  (concurrent) — this gates CON-GSL-004's reachability; kept Plausible.
- `crates/gosling-server` HTTP handlers, `gosling-cli`, and `ui/desktop` (main
  process) were **not** read; the multi-process claim rests on orientation §1/§4
  (three binaries, shared config dir) rather than traced concurrent call sites.
- `context_mgmt/` (summarizer, truncation) and provider **streaming**
  accumulation were not deep-read for ordering/partial-append races — deferred;
  recommend a follow-up pass if streaming buffers share mutable state.
- `nostr_share.rs`, `chat_history_search`, `import_formats/`, subagent /
  `summon.rs` spawn lifecycle (`tokio::spawn` at `summon.rs:369,1441,2170+`) were
  inventoried but not audited for reentrancy/double-spawn.
- `extension_manager.rs` `FuturesUnordered` fan-out (`:1638,1917`) and
  `join_all` extension init (`agent.rs:1178,1267`) were noted but not traced for
  shared-state mutation during parallel init.
- Keyring backend concurrency is assumed atomic per platform contract; not
  independently verified.

Stop condition: named priority surfaces (session persistence, permission_store,
config/base, oauth/persist, tool_execution) all resolved to a finding or an
explicit non-finding; per-lens budget (~30 tool calls) reached. Highest-value
next lens: **Security-LLM / State-Transition** on the concurrent tool-execution
path (CON-GSL-004 boundary), and **Reliability** on streaming accumulation.
