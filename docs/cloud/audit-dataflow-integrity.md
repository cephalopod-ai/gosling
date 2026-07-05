# Audit Lens — Data-Flow / Persisted-Correctness Integrity (`audit-dataflow-integrity`)

Domain: Data-Integrity (`DAT`) · Authority: **audit-only / read-only** · Model: claude-opus-4-8

Builds on `docs/cloud/00-orientation.md` (shared surface inventory). This lens owns
**persisted correctness**: scope leakage, duplicate/orphaned entities, lost
provenance, corrupt merges, bad normalization, **partial persistence** (interrupted
session/config/secret writes leaving corrupt state), migration meaning-loss, and
round-trip loss. The draft prompt was treated as a draft: I preserved its priority
list (session persistence & manager, config writes/migrations, permission_store,
oauth/persist, extension_data, nostr_share) and added the adjacent secrets write
path (`write_secrets_file`) and the config corrupt-recovery path, which turned out to
carry the highest-severity findings.

## Effort budget & coverage

~18 tool calls, static source review only (no build, no tests run, no race
reproduced — per `confidence_calibration.md`, concurrency *manifestations* are held
at Likely). Deep-read: `config/base.rs` write/secrets/migration paths,
`config/migrations.rs`, `session/session_manager.rs` (schema, pool options,
create/import/migrate/replace_conversation), `permission/permission_store.rs`,
`oauth/persist.rs`, `session/nostr_share.rs`, `session/extension_data.rs`.
See Validation Limits for what was not read.

## Data-integrity inventory (writers → boundary)

| Entity | Store | Writer(s) | Scope key | Provenance | Atomic write? | Constraint |
|---|---|---|---|---|---|---|
| Config params | `config.yaml` (YAML) | `save_values` via set_param/update_param/delete | file path (per Config instance) | none | **yes** temp+rename+fsync | none in-file |
| Secrets (API keys, bearer, OAuth creds) | secrets file / keyring | `write_secrets_file` via `write_all_secrets`→set_secret/delete_secret; `oauth/persist.rs` | key string | none | **NO** in-place truncate | 0600 perms only |
| Sessions | SQLite `sessions` | create/update/import/copy/legacy-import | `id` PK | `session_type` | per-statement tx | PK; FK from messages |
| Messages | SQLite `messages` | `replace_conversation_inner` | `session_id` FK | role/metadata | **yes** single tx delete+insert | FK enforced (pragma on) |
| Tool permissions | `tool_permissions.json` | `ToolPermissionStore::save` | `tool_name:context_hash` | timestamp | temp+rename, **no fsync** | none |
| Extension data | `sessions.extension_data` blob | `update().extension_data` | session id | version key | in session UPDATE | none |

## Findings table

| ID | Title | Severity | Confidence |
|---|---|---|---|
| DAT-GSL-001 | Secrets file written non-atomically; interrupted write corrupts entire secret store | High | Confirmed |
| DAT-GSL-002 | Corrupt config silently discarded then fully overwritten on next write | Medium | Confirmed |
| DAT-GSL-003 | Session import/copy spans multiple transactions → partial-persistence on interrupt | Medium | Likely |
| DAT-GSL-004 | Fixed `.tmp` filename + in-process-only guard races across processes | Medium | Likely |
| DAT-GSL-005 | Imported/shared (Nostr) sessions lose origin provenance | Low (Medium if reachable) | Likely |
| DAT-GSL-006 | Permission store & config omit fsync-before-rename (last write lost on crash) | Low | Confirmed |
| DAT-GSL-007 | Permission records accumulate unbounded duplicates | Info | Confirmed |

---

## DAT-GSL-001: Secrets file written non-atomically; interrupted write corrupts the entire secret store

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/config/base.rs:42-60` — `write_secrets_file` opens the target file directly with `.create(true).truncate(true)` and `write_all`, with **no temporary file, no `sync_all`, and no rename**. It truncates the live secrets file first, then streams the new content.
- `crates/gosling/src/config/base.rs:981-984` — the `SecretStorage::File` branch of `write_all_secrets` serializes *all* secrets into one YAML blob and calls `write_secrets_file(path, &yaml_value)`.
- `crates/gosling/src/config/base.rs:991-998` — `mutate_secrets` (the sole write funnel for `set_secret`/`set_secret_values`/`delete_secret`) calls `write_all_secrets`.
- `crates/gosling/src/oauth/persist.rs:37-53` — OAuth credential `save`/`clear` delegate to `config.set_secret`/`delete_secret`, so OAuth tokens ride the same non-atomic path.
- `crates/gosling/src/config/base.rs:1072-1084` — `read_secrets_from_file` parses with `serde_yaml::from_str(&file_content)?`; a truncated/partial file makes the `?` propagate `DeserializeError`, i.e. **every** `get_secret` fails, not just the one being written.

Observed behavior:
- All secrets (provider API keys, custom-header bearer tokens, per-provider OAuth credentials) are persisted as a single serialized blob via an in-place truncate-then-write. Contrast the non-secret config path (`save_values`, lines 658-683), which *does* use temp file + `sync_all` + atomic rename.

Expected boundary:
- A logical record that fully replaces a credential store must be written atomically (temp + fsync + rename) so a crash/kill/`ENOSPC`/full-disk between truncate and completion cannot leave a half-written file. The higher-value store (secrets) should be at least as durable as the lower-value one (config), which already is.

Failure mechanism:
- `truncate(true)` empties the file at `open()`. If the process dies, the machine loses power, or the disk fills between that truncate and the final byte of `write_all`, the on-disk secrets file is left empty or partial. Because the whole store is one YAML document, a partial write is unparseable.

Break-it angle:
- Kill the process (or hit `ENOSPC`) during `gosling configure`/provider setup or an OAuth token refresh. On next start, `read_secrets_from_file` errors on parse; `all_secrets()` propagates it; and because `mutate_secrets` reads `all_secrets()` *before* mutating (line 996), a subsequent `set_secret` cannot even run to repair it. The corruption is **sticky** — no abort/rollback/recovery path; the user must manually delete the file and re-enter every credential.

Impact:
- Durable, silent-until-next-read corruption of the entire credential store. All providers become unauthenticated at once; stored OAuth refresh tokens are lost. Recovery is manual and lossy.

Operational impact:
- Blast radius: Service (all providers/credentials for the user)
- Side-effect class: file
- Reversibility: irreversible (secrets must be re-entered)
- Operator visibility: silent until next secret read, then hard error
- Rerun safety: unsafe (repair write is blocked by the read that precedes it)

Adjacent failure modes:
- DAT-GSL-004 (concurrent writers on the same file), DAT-GSL-006 (fsync gap). The keyring branch (line 969-979) is not affected; only the `File` fallback and non-`system-keyring` builds are.

Recommended mitigation:
- Minimal repair: reuse the existing atomic pattern — write to `path.with_extension("tmp")` (unique-suffixed, see DAT-GSL-004), `sync_all`, then `rename`, preserving `0o600`. Factor `save_values`' proven sequence into a shared `atomic_write_0600` helper and call it from `write_secrets_file`/`write_secrets_to_file`.
- Local guardrail: on a parse error in `read_secrets_from_file`, rename the corrupt file to `*.corrupt-<ts>` and surface a distinct recoverable error rather than wedging all reads.
- Behavior test: write a valid secrets file; simulate an interrupted write (write partial bytes to the real path); assert reads still succeed from the last good file and that no write path leaves the store unreadable.

Implementation assessment:
- Complexity: persistence_recovery · Cost: S · Cost drivers: modules, tests · Nominal agent: codex
- Rationale: localized to two writer helpers and one reader; the atomic template already exists in the same file.

Validation:
- Test: interrupted secrets write leaves the prior secrets intact and readable.
- Test: `oauth/persist.rs` save/clear round-trip survives a simulated mid-write kill.

Non-goals:
- Do not redesign the keyring backend or the secrets schema.

---

## DAT-GSL-002: Corrupt config file is silently discarded, then the entire config is overwritten on the next write

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/config/base.rs:527-548` — `load_write_config` (the read half of every read-modify-write: `set_param`, `set_param_values`, `update_param`, `delete`) parses the write-target file, and on parse failure runs `unwrap_or_else` that logs `"...is corrupt... Starting fresh."` and returns `Mapping::new()`.
- `crates/gosling/src/config/base.rs:865-870` — `set_param` then inserts the single new key into that empty mapping and calls `save_values`, atomically replacing the file with `{ only_the_new_key }`.

Observed behavior:
- If the config file is ever unparseable (transient partial write, concurrent temp-race per DAT-GSL-004, manual edit error, disk glitch), the next config write does not fail or back up — it silently replaces the whole file with just the key currently being set, discarding every other setting (providers block, extensions, active_provider, etc.).

Expected boundary:
- Corrupt-on-read recovery for a read-modify-write store should fail closed (refuse to overwrite) or preserve the damaged file (back it up) before writing, so a transient parse error cannot escalate into total, silent config loss.

Failure mechanism:
- "Start fresh" treats an unreadable file as an empty file. Because the very next `save_values` persists that empty-plus-one mapping, the recovery path is destructive rather than fail-safe.

Break-it angle:
- Corrupt one byte of `config.yaml`, then run any command that writes a config value (e.g. switching model/provider). The provider block and all other keys vanish silently; only the just-set key survives.

Impact:
- Silent, durable loss of all non-secret configuration on the first write after any corruption. Detectable only by the operator noticing missing settings.

Operational impact:
- Blast radius: Workflow (whole config for the user)
- Side-effect class: file · Reversibility: irreversible (no backup taken) · Operator visibility: log-only (`warn!`) · Rerun safety: unsafe (each write compounds loss)

Adjacent failure modes:
- The merged read path `load` (lines 569-580) is non-destructive (skips the bad layer with a warning); only the write path escalates. DAT-GSL-004 supplies a realistic corruption source.

Recommended mitigation:
- Minimal repair: on parse failure in `load_write_config`, return an error (fail closed) instead of `Mapping::new()`, or rename the corrupt file to `*.corrupt-<ts>` first so the data is recoverable.
- Behavior test: given a corrupt config file, assert that `set_param` either errors or preserves the corrupt bytes as a backup, and never emits a file containing only the new key.

Implementation assessment:
- Complexity: persistence_recovery · Cost: XS · Cost drivers: tests · Nominal agent: codex
- Rationale: a few lines at one call site plus a regression test.

Validation:
- Test: corrupt config + `set_param` does not destroy unrelated keys.

Non-goals:
- Do not add schema validation of config contents here.

---

## DAT-GSL-003: Session import/copy spans multiple independent transactions → partial persistence on interrupt

Severity: Medium
Confidence: Likely
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/session/session_manager.rs:1939-1964` — `import_session` performs `create_session(...)` (its own commit), then `update(&session.id)...apply()` (a second write), then `replace_conversation(...)` (a third `BEGIN IMMEDIATE` transaction at 1643-1674) as three separate, independently-committed steps with **no enclosing transaction**.
- `crates/gosling/src/session/session_manager.rs:1969-2006` — `copy_session` has the same create → update → replace_conversation shape.
- Contrast `import_legacy_session` (960-1010) which at least inserts the session row in one tx but still calls `replace_conversation_inner` after `tx.commit()` (line 1006-1009).

Observed behavior:
- Importing a shared/exported session, or copying one, writes the session row, then its metadata, then its messages in three commits. An interruption between them persists a session row with missing or empty conversation, or metadata that never landed.

Expected boundary:
- A logical "session + its conversation" import should be one atomic unit: either the fully-populated session exists, or nothing does.

Failure mechanism:
- The steps use the pool directly and each commits independently; there is no shared `Transaction` threaded through create/update/replace.

Break-it angle:
- Kill the process (or induce a message-insert error mid-loop) after `create_session` commits but before `replace_conversation` commits: a session row is listed in the UI/CLI with no (or partial) messages. Rerun of the import creates a *second* session (new id, DAT-GSL non-dup by design) rather than repairing the first, leaving an orphaned empty session behind.

Impact:
- Recoverable but user-visible: empty/half sessions appear in session lists; import "success" is not conditioned on the conversation actually landing.

Operational impact:
- Blast radius: Workflow · Side-effect class: DB · Reversibility: compensatable (user deletes the stub) · Operator visibility: UI-visible (empty session) · Rerun safety: unsafe (rerun duplicates rather than heals)

Adjacent failure modes:
- Overlaps concurrency (CON) for interleaved imports; this lens owns the persisted-correctness angle. `nostr_share` import feeds this path (DAT-GSL-005).

Recommended mitigation:
- Minimal repair: thread a single `BEGIN IMMEDIATE` transaction through the session insert, metadata update, and message insert for `import_session`/`copy_session` (mirror how `replace_conversation_inner` already batches deletes+inserts).
- Behavior test: force a failure after the session insert and assert no partial session row remains (or that it is unlisted).

Implementation assessment:
- Complexity: persistence_recovery · Cost: M · Cost drivers: modules, tests · Nominal agent: codex
- Rationale: requires refactoring create/update/replace to accept a shared executor.

Validation:
- Test: interrupted import leaves zero session rows.
- Test: successful import is all-or-nothing across session + messages.

Non-goals:
- Do not change the export format or id-generation strategy.

---

## DAT-GSL-004: Fixed `.tmp` filename plus in-process-only guard races across concurrent processes

Severity: Medium
Confidence: Likely
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/config/base.rs:659` — `let temp_path = target_path.with_extension("tmp");` — a **fixed, shared** temp name.
- `crates/gosling/src/config/base.rs:662-669` — the temp file is opened with `.create(true).truncate(true)`; the exclusive `lock_exclusive()` (672) is taken **after** the truncate, and on a temp file that is recreated (new inode) every write, so it provides no cross-process mutual exclusion over the target.
- `crates/gosling/src/config/base.rs:841,866,878,901` — the only cross-write serialization is `lock_ignoring_poison(&self.guard)`, an **in-process** `Mutex` on the `Config` instance; separate processes (CLI + desktop app + server) each have their own.
- `crates/gosling/src/permission/permission_store.rs:66-74` — `save` uses the same fixed `path.with_extension("tmp")` then `std::fs::write(&temp_path, ...)` (truncating overwrite) with no lock at all.

Observed behavior:
- Two gosling processes writing config (or permissions) concurrently open the same `*.tmp` path. In config, process B's `truncate` at `open()` can clobber content process A is mid-writing before B blocks on the flock; in the permission store, both `fs::write` the same temp then both `rename`, so the survivor is whichever races last. Because each write persists a whole read-modify-write snapshot, the loser's committed changes are silently dropped (lost update), and a torn temp can be renamed into place.

Expected boundary:
- Concurrent independent processes must not corrupt or silently drop each other's writes. Atomic-write temp files must be unique per writer (pid/random suffix), and cross-process RMW needs a cross-process lock over the *target*, not an in-process mutex over a per-instance temp file.

Failure mechanism:
- Shared temp name + truncate-before-lock (config) / lockless overwrite (permissions) + in-process-only guard.

Break-it angle:
- Run the desktop app and CLI simultaneously (a realistic pairing per orientation §4/§6), each toggling a setting or approving a tool. Observe last-writer-wins loss of the other's setting/permission, and — under a crash-timed race — a truncated `config.yaml` that then triggers DAT-GSL-002's destructive recovery.

Impact:
- Lost config/permission updates; a corruption source that feeds DAT-GSL-002. Contained (recoverable by re-setting), but silent.

Operational impact:
- Blast radius: Workflow · Side-effect class: file · Reversibility: compensatable · Operator visibility: silent · Rerun safety: unknown (timing-dependent)

Adjacent failure modes:
- Feeds DAT-GSL-002 (corrupt config) and shares the durability gap of DAT-GSL-006. Session DB is *not* affected — it uses SQLite WAL + `busy_timeout` + `BEGIN IMMEDIATE` for cross-process serialization (see Non-Findings).

Recommended mitigation:
- Minimal repair: use a unique temp suffix (`format!("tmp.{}", pid_or_uuid)`) for both writers; add a cross-process advisory lock over a stable lockfile beside the target for the RMW window.
- Behavior test: spawn two writers in parallel and assert both updates survive (no lost update, no torn file).

Implementation assessment:
- Complexity: cross_process_coordination · Cost: M · Cost drivers: modules, tests, runtime_verification · Nominal agent: codex
- Rationale: correct cross-process locking needs care and a concurrency test harness.

Validation:
- Test: two concurrent `set_param` writers preserve both keys.
- Test: two concurrent permission saves preserve both grants.

Non-goals:
- Do not migrate config to SQLite in this slice.

---

## DAT-GSL-005: Imported/shared (Nostr) sessions lose origin provenance

Severity: Low (Medium if reachable as a prompt-injection vector)
Confidence: Likely
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/session/nostr_share.rs:200-237` — `import_session_json_from_deeplink` fetches an encrypted event from a public relay (`DEFAULT_RELAYS`, 15-20) and returns the decrypted session JSON. The relay/event content is attacker-influenceable (anyone can publish kind-30278 with a matching deeplink).
- `crates/gosling-cli/src/commands/session.rs:301,321` — the CLI decrypts the deeplink then calls `import_session(&json, Some(SessionType::User))`, stamping the imported session as an ordinary user session.
- `crates/gosling/src/session/session_manager.rs:1939-1946` — `import_session` sets `session_type` from the override (`User`) and persists no field recording that the content originated from an external relay.

Observed behavior:
- A session pulled from an untrusted external relay is persisted indistinguishably from the operator's own locally-created sessions. On resume, its conversation becomes agent context (untrusted-content boundary per orientation §4) with no provenance marker to downstream consumers (UI, resume, subagents).

Expected boundary:
- Per the data-integrity provenance angle: records that entered from an external/attacker-influenceable source should carry that origin so downstream readers can treat them as untrusted (e.g. force review before resume, or badge the session).

Failure mechanism:
- The import collapses provenance into `SessionType::User`; there is no `imported_from`/`origin` column or flag.

Break-it angle:
- Publish a crafted session whose conversation embeds tool-eliciting instructions; share the deeplink. After import it looks like a first-party session and resumes into the agent loop with no "external origin" signal to gate it.

Impact:
- Lost provenance; enables the shared-session prompt-injection surface flagged in orientation §5.5. Data-integrity impact is the missing origin field; the exploit consequence is a security-lens concern (cross-reference).

Operational impact:
- Blast radius: Workflow (per imported session) · Side-effect class: DB · Reversibility: reversible · Operator visibility: silent · Rerun safety: safe

Adjacent failure modes:
- Feeds the security lens (untrusted content on resume). Legacy/other `import_formats` (codex/claude_code/pi) share the same "no origin recorded" property (not deep-read — see Validation Limits).

Recommended mitigation:
- Minimal repair: add an `origin`/`imported_from` provenance column (or a reserved `extension_data` key) set on import; surface it in UI/CLI and consider gating resume of externally-sourced sessions.
- Behavior test: import a Nostr session and assert the persisted row records external origin.

Implementation assessment:
- Complexity: persistence_recovery · Cost: S · Cost drivers: migrations, modules, tests · Nominal agent: codex
- Rationale: one nullable column + set-on-import + a read surface.

Validation:
- Test: imported session carries origin provenance; native session does not.

Non-goals:
- Do not implement resume-gating policy in this slice (route to security lens).

---

## DAT-GSL-006: Permission store and config omit fsync-before-rename (last write can be lost / zero-length on crash)

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/permission/permission_store.rs:70-74` — `std::fs::write(&temp_path, &content)?` then `std::fs::rename(temp_path, path)?` with **no `File::sync_all`** between write and rename, and no parent-directory fsync after.
- `crates/gosling/src/config/base.rs:677-683` — `save_values` *does* `sync_all` the temp file (good) but does **not** fsync the parent directory after `rename`, so the rename itself may not be durable on some filesystems after a power loss.

Observed behavior:
- On crash/power-loss, the permission store can be left with a renamed-but-unflushed (potentially zero-length or stale) file; the config rename may not survive. At most the last write is lost — but for the permission store the file content itself is not guaranteed flushed before the rename.

Expected boundary:
- Atomic-write durability requires fsync of the file before rename and (for strict durability) fsync of the directory after.

Failure mechanism:
- Missing `sync_all` (permission store) and missing directory fsync (both).

Break-it angle:
- Power-loss immediately after `record_permission` → the just-granted (or just-revoked) permission may not persist, re-prompting or, worse, losing a revocation.

Impact:
- Narrow: last permission/config write lost. Permission loss re-prompts (safe direction); a lost *revocation* is the sharper edge.

Operational impact:
- Blast radius: Local · Side-effect class: file · Reversibility: compensatable · Operator visibility: silent · Rerun safety: safe

Adjacent failure modes:
- Same writer family as DAT-GSL-001/004.

Recommended mitigation:
- Minimal repair: open the temp file, `write_all`, `sync_all`, `rename`, then fsync the parent dir; share the helper proposed in DAT-GSL-001.
- Behavior test: assert the writer calls fsync before rename (or assert durability via a crash-injection harness if available).

Implementation assessment:
- Complexity: local_guardrail · Cost: XS · Cost drivers: modules · Nominal agent: codex
- Rationale: a couple of lines per writer.

Validation:
- Test: permission save flushes content before rename.

Non-goals:
- Do not add directory-fsync to the SQLite path (SQLite manages its own durability).

---

## DAT-GSL-007: Permission records accumulate unbounded duplicates

Severity: Info
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/permission/permission_store.rs:113` — `record_permission` does `self.permissions.entry(key).or_default().push(record)`, appending a new `ToolPermissionRecord` to the `Vec` for `tool_name:context_hash` on every grant with no dedup.
- `crates/gosling/src/permission/permission_store.rs:84-89` — `check_permission` uses `.rfind` (last matching record wins), so older duplicates are dead weight.
- `crates/gosling/src/permission/permission_store.rs:133-146` — `cleanup_expired` only removes *expired* records; non-expiring grants (`expiry: None`) accumulate forever.

Observed behavior:
- Repeated approvals of the same tool+context append duplicate non-expiring records that are never pruned, growing the JSON file unboundedly.

Expected boundary:
- A permission decision for a given `(tool_name, context_hash)` is idempotent; storage should hold at most one live record per key (dedup / replace on write).

Failure mechanism:
- Append-only Vec with `.rfind` read and expiry-only cleanup.

Impact:
- Slow file growth and redundant records; no correctness error (last-wins read is correct). Auditability/size hygiene only.

Operational impact:
- Blast radius: Local · Side-effect class: file · Reversibility: reversible · Operator visibility: silent · Rerun safety: safe

Recommended mitigation:
- Minimal repair: replace the last non-expired record for a key instead of pushing; or dedup in `cleanup_expired`.
- Behavior test: repeated `record_permission` for the same key yields a single stored record.

Implementation assessment:
- Complexity: local_guardrail · Cost: XS · Cost drivers: tests · Nominal agent: codex

Validation:
- Test: N identical grants → 1 stored record.

Non-goals:
- None.

---

## Non-findings (checked and held)

- **Session DB orphan protection is real.** `crates/gosling/src/session/session_manager.rs:746-756` sets `.foreign_keys(true)` (plus WAL, `busy_timeout(30s)`, `synchronous=Normal`) on the production pool, so the `messages.session_id REFERENCES sessions(id)` FK (877) is actually enforced — no orphaned messages on the write path. (Test-only pools at 3595/3665 omit the pragma; not a production concern.)
- **Conversation replacement is atomic.** `replace_conversation_inner` (1643-1674) runs the `DELETE` + all message `INSERT`s inside one `BEGIN IMMEDIATE` transaction; a mid-loop failure rolls back — no message-level partial persistence.
- **Schema migrations are atomic and incremental.** `run_migrations` (1014-1037) wraps the whole `current+1..=CURRENT` loop and the `schema_version` bump in a single `BEGIN IMMEDIATE` transaction; a failed migration rolls back the version stamp too.
- **First-run schema creation is concurrency-safe.** `create_schema` (804-833) uses `BEGIN IMMEDIATE` + `CREATE TABLE IF NOT EXISTS` + `INSERT OR IGNORE` on the version row, closing the documented two-process create race.
- **Nostr/exported import does not overwrite local sessions.** `import_session` (1939-1946) calls `create_session`, which mints a fresh id; the embedded JSON `id` is not reused, so a hostile shared session cannot collide with or clobber a local session by id. (Provenance is still lost — DAT-GSL-005.)
- **Config (non-secret) write is atomic.** `save_values` (658-683) writes to a temp file, `sync_all`s, and `rename`s (dir-fsync gap noted in DAT-GSL-006). This is the template the secrets path should adopt.
- **Config migration is idempotent and unit-tested** (`config/migrations.rs` tests 269-540): platform-extension and provider migrations re-run to no-ops and preserve `enabled` state.

## Break-it review summary

| Attack | Result |
|---|---|
| Kill during secrets write | **Corrupts entire secret store, sticky** → DAT-GSL-001 |
| Corrupt config then write a key | **Silent total config loss** → DAT-GSL-002 |
| Interrupt session import mid-way | **Partial/empty session persisted** → DAT-GSL-003 |
| Two processes write config/permissions concurrently | **Lost update / torn temp** → DAT-GSL-004 |
| Import hostile shared session by id | Held — fresh id minted (non-finding) |
| Delete session, look for orphan messages | Held — FK enforced (non-finding) |
| Re-run conversation save | Held — atomic delete+insert (non-finding) |
| Import externally-sourced session | **No origin provenance recorded** → DAT-GSL-005 |

## Patch order

1. DAT-GSL-001 (High, XS-S, atomic secrets write — reuse existing template).
2. DAT-GSL-002 (Medium, XS, fail-closed corrupt-config recovery).
3. DAT-GSL-004 (Medium, M, cross-process temp/lock — also cures DAT-GSL-002's corruption source).
4. DAT-GSL-003 (Medium, M, single-transaction import).
5. DAT-GSL-005 (Low/Medium, S, origin provenance) → coordinate with security lens.
6. DAT-GSL-006 / DAT-GSL-007 (Low/Info, XS, durability + dedup hygiene).

## Validation Limits (not reviewed / not proven)

- **No build, no tests run, no race reproduced.** All findings are static (`source-evidenced` / `simulation-reasoned`). Concurrency manifestations (DAT-GSL-004) held at Likely per `confidence_calibration.md`; a durability drill would be `requires-authorized-drill`.
- **`system-keyring` backend not deep-traced.** Findings DAT-GSL-001/006 target the `File` secrets backend and non-keyring builds; the keyring `set_password` path (base.rs:969-979) was read but its atomicity is the OS keyring's responsibility.
- **Other import formats not deep-read:** `session/import_formats/{codex,claude_code,pi}.rs`, `import_formats/mod.rs` (`convert_to_gosling_session_json`) — normalization/round-trip and provenance for these were not audited (DAT-006/DAT-009 for these paths is `Not Reviewed`).
- **Not read:** `session/diagnostics.rs`, `session/last_message_snippet.rs`, `session/chat_history_search.rs`, `session/session_naming.rs`, `session/legacy.rs` (load side), `config/extensions.rs` / `config/permission.rs` / `config/declarative_providers.rs` writers, `providers/inventory` `create_tables`, and the server routes (`gosling-server/src/routes/*`) that call `replace_conversation`. Their write paths may share the multi-transaction and temp-file patterns flagged here.
- **`create_schema` vs cumulative-migration divergence** (whether the fresh-DB DDL at 835-866 exactly equals applying migrations 1..N) was not diffed column-by-column; that shape-vs-migration parity check belongs to `audit-invariant-sync`.
