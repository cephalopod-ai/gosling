# Clean-Slate Architecture & Dataflow Audit: Data Paths, Chat Execution, and Research Systems

**Document ID**: `20260913_Gemini_Audit_Data_gosling`  
**Date**: September 13, 2026  
**Auditor**: Antigravity / Gemini  
**Audit Scope**: Clean-slate repository scan of `gosling` core data paths, session persistence, chat execution loop, research deliverable pipelines, handoff continuity, memory retrieval, and workspace isolation.  
**Operating Constraint**: Clean start (no reliance on prior audit documents or historical playtest logs); strictly zero runtime code modifications.

---

## Executive Summary

This independent, clean-slate audit covers the primary runtime execution and data-flow systems of the `gosling` agent framework:
1. **Session Persistence & SQLite Schema Architecture** ([`crates/gosling/src/session/`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/))
2. **Chat Execution, Agent Loop & Turn Leases** ([`crates/gosling/src/agents/agent/`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/agents/agent/))
3. **Research & Deep Research Deliverable Pipeline** ([`crates/gosling/src/session/research.rs`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/research.rs) & [`crates/gosling/src/acp/server/research_completion.rs`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/acp/server/research_completion.rs))
4. **Session Handoff & Continuity** ([`crates/gosling/src/session/handoff.rs`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/handoff.rs) & [`crates/gosling/src/session/session_manager/handoff_storage.rs`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager/handoff_storage.rs))
5. **Context Management, Memory Retrieval & Cross-Workspace Isolation** ([`crates/gosling/src/context_mgmt/`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/context_mgmt/) & [`crates/gosling/src/workspace/`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/workspace/))

### High-Level Summary of Findings

| Finding ID | Domain | Category | Severity | Summary |
| :--- | :--- | :--- | :--- | :--- |
| **AUD-DAT-001** | Context / Memory | Security / Privacy | **High** | Cross-workspace memory leak: Global `memories.jsonl` injects private facts across unrelated workspaces without scoping. |
| **AUD-DAT-002** | Context / Memory | Reliability / Logic | **Medium** | Inverted read window: `FileMemorySource` reads the oldest 8MB instead of the newest entries from append-only memory file. |
| **AUD-DAT-003** | Research Pipeline | Silent Failure | **High** | Artifact limit threshold (>2000) causes deliverable verification to silently return `true`, completely bypassing deliverable checks. |
| **AUD-DAT-004** | Research Pipeline | Conflicting Logic | **Medium** | Path verification divergence: Deliverable closeout tolerates non-existent paths, but verification bails on them with an unrecoverable error. |
| **AUD-DAT-005** | Chat / Compaction | Reliability / UX | **Medium** | Conversation truncation leaves stored token counts stale; `stored.max(estimated)` causes spurious auto-compaction thrashing. |
| **AUD-DAT-006** | Agent Loop | Performance / CPU | **Medium** | Busy-wait 100ms sleep loop in `reply_stream.rs` tool streaming select loop causes continuous wakeups during long tool runs. |
| **AUD-DAT-007** | Session Persistence | Concurrency / Crash | **Medium** | Process-local `FIRST_INIT_LOCK` does not protect multi-process startup races (CLI + Desktop) during SQLite WAL and migration init. |
| **AUD-DAT-008** | Output Tracking | Archival / Storage Leak | **Medium** | `output_revisions` table lacks session/workspace scoping and foreign keys; file content blobs are never purged on session deletion. |
| **AUD-DAT-009** | Session Storage | Performance / Bloat | **Low** | `import_session` deserializes the entire SQLite `sessions` table in Rust memory on every import to check SHA256 deduplication. |
| **AUD-DAT-010** | Session Schema | Dead Code / Bloat | **Low** | Dead Goose columns (`schedule_id`, `recipe_json`, `user_recipe_values_json`) remain in schema definitions and base migrations. |
| **AUD-DAT-011** | Session Handoff | Performance / I/O | **Low** | Handoff provider transition updates message invisibility via N individual sequential SQL queries in a transaction loop. |
| **AUD-DAT-012** | Workspace Store | Concurrency / Blocking | **Low** | Synchronous file locking (`fs2::FileExt`) inside async workspace service methods risks stalling Tokio worker threads. |

---

## 1. Data Paths & Session Persistence Architecture

### 1.1 Process-Local Concurrency Guard on Shared Database (`AUD-DAT-007`)
- **File Reference**: [`crates/gosling/src/session/session_manager/pool_lifecycle.rs:24-91`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager/pool_lifecycle.rs#L24-L91)
- **Mechanism**:
  ```rust
  static FIRST_INIT_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
  ...
  pub(crate) async fn pool(&self) -> Result<&Pool<Sqlite>> {
      self.initialized.get_or_try_init(|| async {
          let _init_guard = FIRST_INIT_LOCK.lock().await;
          ...
          if schema_exists {
              Self::run_migrations(&self.pool).await?;
          } else {
              Self::create_schema(&self.pool).await?;
          }
      })
  }
  ```
- **Vulnerability / Flaw**:
  The inline documentation specifically notes that separate stores for the same database race SQLite's first-open WAL switch and schema creation, which fails fast with `"database is locked"` instead of honoring `busy_timeout`.
  However, `FIRST_INIT_LOCK` is an in-memory `tokio::sync::Mutex<()>`. It only synchronizes tasks within the *same operating system process*. If the user opens the Electron desktop application (`gosling-desktop`) and simultaneously invokes a CLI command (`gosling ...`), or if two CLI instances start concurrently against the default data directory, `FIRST_INIT_LOCK` provides zero cross-process exclusion. One process will fail fatally during startup with a `database is locked` error.
- **Recommended Remediation**:
  Use an operating system advisory lock (via `fs4` / `fs2` lockfile on `.session_db.lock`) or execute a SQLite `PRAGMA busy_timeout` loop with exponential backoff on migration transactions.

### 1.2 Unscoped `output_revisions` Table and Archival Leak (`AUD-DAT-008`)
- **File Reference**: [`crates/gosling/src/session/session_manager/output_revisions_storage.rs:41-45`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager/output_revisions_storage.rs#L41-L45), [`crates/gosling/src/session/session_manager/session_crud.rs:453-495`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager/session_crud.rs#L453-L495)
- **Mechanism**:
  The schema for output revision tracking is defined as:
  ```sql
  CREATE TABLE IF NOT EXISTS output_revisions (
      path TEXT NOT NULL, version INTEGER NOT NULL, event_id TEXT NOT NULL,
      metadata_json TEXT NOT NULL, content BLOB NOT NULL,
      PRIMARY KEY(path, version), UNIQUE(path, event_id)
  );
  ```
- **Vulnerability / Flaw**:
  1. **Cross-Session Version Collision**: The primary key is solely `(path, version)`. It contains neither `session_id` nor `workspace_id`. If two concurrent sessions or workspaces write to the same relative or absolute path, their versions and event IDs collide in this table.
  2. **Indefinite Storage Bloat / Archival Leak**: `delete_session` in [`session_crud.rs:453-495`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager/session_crud.rs#L453-L495) explicitly cleans up `session_summary_facts`, `session_summaries`, `messages`, `session_library_items`, and `sessions`. Because `output_revisions` lacks a `session_id` column and foreign key reference to `sessions(id)`, every file revision BLOB ever written by any tool remains permanently stored in SQLite, causing unbounded database bloat.
- **Recommended Remediation**:
  Alter `output_revisions` to include `session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE` and update primary key to `(session_id, path, version)`.

### 1.3 Full-Table Deserialization on Session Import (`AUD-DAT-009`)
- **File Reference**: [`crates/gosling/src/session/session_manager.rs:1098-1109`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager.rs#L1098-L1109), [`crates/gosling/src/session/session_manager.rs:1141-1155`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager.rs#L1141-L1155)
- **Mechanism**:
  ```rust
  for session in self.list_all_sessions().await? {
      let Some(provenance) =
          super::import_formats::SessionImportProvenance::from_extension_data(
              &session.extension_data,
          )
      else {
          continue;
      };
      if provenance.source_sha256.as_deref() == Some(&source_sha256) {
          return Ok(session);
      }
  }
  ```
- **Vulnerability / Flaw**:
  To prevent duplicate imports, `import_session` and `import_session_file` invoke `self.list_all_sessions().await?`, pulling every single session in the database into memory, deserializing each record into a Rust `Session` struct, and parsing `session.extension_data` JSON. This is an $O(N)$ memory and CPU operation that degrades as session counts increase.
- **Recommended Remediation**:
  Extract `source_sha256` into a first-class indexed column or use SQLite's `json_extract(extension_data, '$.provenance.source_sha256')` directly in a `SELECT id FROM sessions WHERE ...` query.

### 1.4 Dead Schema Columns from Upstream Goose (`AUD-DAT-010`)
- **File Reference**: [`crates/gosling/src/session/session_manager/schema.rs:100-102`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager/schema.rs#L100-L102), [`crates/gosling/src/session/session_manager/migrations.rs:99`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager/migrations.rs#L99)
- **Mechanism**:
  The `sessions` table contains:
  ```sql
  schedule_id TEXT,
  recipe_json TEXT,
  user_recipe_values_json TEXT,
  ```
- **Observation**:
  Gosling explicitly removed Goose's recipe and scheduling subsystems. These columns are never read, written, or referenced anywhere in Gosling's codebase. They represent vestigial schema bloat from upstream migrations.

---

## 2. Chat Execution, Agent Loop & Context Management

### 2.1 100ms Busy-Wait Polling in Streaming Tool Dispatch (`AUD-DAT-006`)
- **File Reference**: [`crates/gosling/src/agents/agent/reply_stream.rs:680-757`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/agents/agent/reply_stream.rs#L680-L757)
- **Mechanism**:
  ```rust
  while !is_token_cancelled(&cancel_token) {
      tokio::select! {
          biased;
          tool_item = combined.next() => {
              match tool_item { ... }
          }
          _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {}
      }
  }
  ```
- **Vulnerability / Flaw**:
  When tools are executing asynchronously (e.g. running a long compiler build, large git checkout, or external network command), the select loop repeatedly times out every 100ms solely to re-evaluate `while !is_token_cancelled(&cancel_token)`.
  A 30-second tool execution produces 300 unnecessary thread wakeups on the Tokio runtime.
- **Recommended Remediation**:
  Eliminate `tokio::time::sleep(100ms)` and add a direct cancellation branch into `tokio::select!`:
  ```rust
  _ = cancel_token.cancelled() => { break; }
  ```

### 2.2 Conversation Truncation Leaves Stale Token Usage / Compaction Thrashing (`AUD-DAT-005`)
- **File Reference**: [`crates/gosling/src/session/session_manager/message_storage.rs:644-709`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager/message_storage.rs#L644-L709), [`crates/gosling/src/context_mgmt/mod.rs:705-723`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/context_mgmt/mod.rs#L705-L723)
- **Mechanism**:
  1. `truncate_conversation` and `truncate_conversation_from_message` delete rows from `messages`, `session_summary_facts`, and `session_summaries`. However, they **do not** update or clear `sessions.total_tokens` or `sessions.input_tokens`.
  2. When resolving context usage on the subsequent turn, [`context_mgmt/mod.rs:712-715`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/context_mgmt/mod.rs#L712-L715) calculates:
     ```rust
     let current_tokens = match stored_current_tokens {
         Some(stored) => stored.max(estimated_tokens),
         None => estimated_tokens,
     };
     ```
- **Vulnerability / Flaw**:
  If a session grew to 150,000 tokens and the user truncates/rewinds the conversation back to turn 2 (where estimated tokens is only ~2,000 tokens), `stored_current_tokens` retains the old value of 150,000.
  Because `stored.max(estimated_tokens)` evaluates to 150,000, `auto_compaction_check` believes the conversation is dangerously close to or exceeding the context limit. It immediately triggers auto-compaction on a 2-message conversation, causing spurious summarization requests, unnecessary LLM token spend, and potential compaction failures.
- **Recommended Remediation**:
  In `truncate_conversation` and `truncate_conversation_from_message`, reset `total_tokens` and `input_tokens` in `sessions` to `NULL` (or recalculate them from remaining messages) within the truncation transaction.

---

## 3. Research & Deep Research Deliverable Pipeline

### 3.1 Silent Deliverable Verification Bypass on Artifact Limit (>2000) (`AUD-DAT-003`)
- **File Reference**: [`crates/gosling/src/session/research.rs:145-173`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/research.rs#L145-L173)
- **Mechanism**:
  ```rust
  pub async fn list_all_artifacts(...) -> Result<Vec<SessionArtifact>> {
      ...
      if page.total_count > MAX_RESEARCH_ARTIFACTS { // MAX_RESEARCH_ARTIFACTS = 2000
          anyhow::bail!("the session has too many artifacts to verify safely");
      }
      ...
  }

  pub async fn turn_wrote_output_deliverable(...) -> bool {
      let output_roots = canonical_dirs(&state.output_paths);
      if output_roots.is_empty() {
          return true;
      }
      let inventory = match list_all_artifacts(session_manager, session_id).await {
          Ok(artifacts) => artifacts,
          Err(_) => return true, // <-- SILENT BYPASS
      };
      ...
  }
  ```
- **Vulnerability / Flaw**:
  When a session's artifact count exceeds 2,000, `list_all_artifacts` bails with an error intended to prevent unsafe verification.
  However, `turn_wrote_output_deliverable` catches **any** `Err(_)` and returns `true`.
  This is a critical silent failure: as soon as a research project grows beyond 2,000 artifacts, the deliverable verification guard is completely neutralized. The agent can end a turn without producing any deliverable report, and the system records the turn as having successfully produced an output deliverable without logging a warning or alerting the operator.
- **Recommended Remediation**:
  Differentiate between transient database errors and artifact limit thresholds. If the session has too many artifacts, log a warning and fallback to a targeted file-system check of `state.output_paths` directly rather than unconditionally returning `true`.

### 3.2 Path Verification Divergence Between Closeout and Verification (`AUD-DAT-004`)
- **File Reference**: [`crates/gosling/src/acp/server/research_completion.rs:166`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/acp/server/research_completion.rs#L166), [`crates/gosling/src/acp/server/research_completion.rs:294-302`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/acp/server/research_completion.rs#L294-L302)
- **Mechanism**:
  1. In `close_out_deliverables`:
     ```rust
     let output_roots = research::canonical_dirs(&state.output_paths);
     if output_roots.is_empty() {
         bail!("a workspace output folder is unavailable");
     }
     ```
     `research::canonical_dirs` filters paths using `std::fs::canonicalize(p).ok()`, silently ignoring configured paths that do not exist as long as at least one valid path remains.
  2. In `verify_artifact_pairs` (called immediately after):
     ```rust
     let output_roots = state
         .output_paths
         .iter()
         .map(std::fs::canonicalize)
         .collect::<std::io::Result<Vec<_>>>()
         .context("a workspace output folder is unavailable")?;
     ```
- **Vulnerability / Flaw**:
  `verify_artifact_pairs` does not use `research::canonical_dirs`. It attempts to canonicalize every path in `state.output_paths` and aborts if *any single path* does not exist on disk.
  If a workspace has multiple output paths configured and one of them is missing or was deleted, `close_out_deliverables` successfully writes and mirrors the report to the available directory, but `verify_artifact_pairs` fails on the next step with `"a workspace output folder is unavailable"`, aborting the turn and reporting an error despite the report having been created.
- **Recommended Remediation**:
  Standardize on `research::canonical_dirs` in both `close_out_deliverables` and `verify_artifact_pairs`.

---

## 4. Context Management, Memory Retrieval & Cross-Workspace Isolation

### 4.1 Cross-Workspace Memory Leak in Global `memories.jsonl` (`AUD-DAT-001`)
- **File Reference**: [`crates/gosling/src/context_mgmt/memory.rs:63-68`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/context_mgmt/memory.rs#L63-L68), [`crates/gosling/src/context_mgmt/memory.rs:173-208`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/context_mgmt/memory.rs#L173-L208), [`crates/gosling/src/context_mgmt/summarizer/mod.rs:333`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/context_mgmt/summarizer/mod.rs#L333)
- **Mechanism**:
  1. [`memories_file_path()`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/context_mgmt/memory.rs#L63-L68) resolves the memory file:
     ```rust
     pub fn memories_file_path() -> PathBuf {
         Config::global()
             .get_param::<String>("GOSLING_MEMORY_FILE")
             .map(PathBuf::from)
             .unwrap_or_else(|_| Paths::in_config_dir("memories.jsonl"))
     }
     ```
     This defaults to a single, global user-level file: `~/.config/gosling/memories.jsonl`.
  2. When the background summarizer extracts facts, it appends them to this global file.
  3. When `FileMemorySource::retrieve` is invoked during turn generation, it reads `memories.jsonl`, performs keyword matching against the user's trailing message, and injects matching entries into the `ContextPacket`.
- **Vulnerability / Flaw**:
  `FileMemorySource::retrieve` completely ignores `query.session_id` and does not check workspace ownership.
  If a user works on a confidential proprietary project in Workspace 1 (e.g., storing client names, proprietary URLs, internal architecture details), those facts are appended to `~/.config/gosling/memories.jsonl`.
  Later, when the user opens an open-source or public project in Workspace 2 and types a prompt that shares common keywords, the confidential facts from Workspace 1 are recalled and injected into the prompt sent to the LLM provider for Workspace 2.
  This represents a severe cross-workspace data and privacy leak.
- **Recommended Remediation**:
  Scope `memories.jsonl` to the active workspace directory (or record `workspace_id` in `MemoryRecord` and enforce strict filtering during retrieval).

### 4.2 Inverted Read Window in `FileMemorySource` (`AUD-DAT-002`)
- **File Reference**: [`crates/gosling/src/context_mgmt/memory.rs:81-85`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/context_mgmt/memory.rs#L81-L85), [`crates/gosling/src/context_mgmt/memory.rs:133-140`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/context_mgmt/memory.rs#L133-L140)
- **Mechanism**:
  ```rust
  // Comment at line 81:
  /// Upper bound on how much of the memory file a single context build will
  /// read. Only the most recent entries matter for recall, and the file is
  /// append-only, so a cap bounds the allocation without changing behavior for
  /// any realistic file. (MEM-GSL-002)
  const MAX_MEMORY_FILE_BYTES: u64 = 8 * 1024 * 1024;
  ...
  // Implementation at line 133:
  let mut raw = String::new();
  if std::io::Read::by_ref(&mut file)
      .take(MAX_MEMORY_FILE_BYTES)
      .read_to_string(&mut raw)
      .is_err()
  {
      return Vec::new();
  }
  ```
- **Vulnerability / Flaw**:
  The developer intent stated in the docstring is: *"Only the most recent entries matter for recall, and the file is append-only"*.
  However, `file.take(MAX_MEMORY_FILE_BYTES)` reads starting from byte 0 (the start of the file), meaning it reads the **oldest** 8MB.
  Because the file is append-only, once `memories.jsonl` exceeds 8MB, any newly added memories appended to the end of the file are **never** read by `FileMemorySource`. The reader is permanently locked to the oldest historical entries.
- **Recommended Remediation**:
  Seek backwards from the end of the file by `file_len.min(MAX_MEMORY_FILE_BYTES)` before reading, ensuring the most recent entries are retrieved.

---

## 5. Session Handoff & Continuity

### 5.1 Sequential Row-by-Row Invisibility Serialization inside Transaction (`AUD-DAT-011`)
- **File Reference**: [`crates/gosling/src/session/session_manager/handoff_storage.rs:255-272`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/session/session_manager/handoff_storage.rs#L255-L272)
- **Mechanism**:
  ```rust
  if let Some(covered_through_row_id) = snapshot.coverage.covered_through_row_id {
      let metadata_rows = sqlx::query_as::<_, (i64, String)>(
          "SELECT id, metadata_json FROM messages WHERE session_id = ? AND id <= ?",
      )
      .bind(&snapshot.session_id)
      .bind(covered_through_row_id)
      .fetch_all(&mut *tx)
      .await?;
      for (row_id, metadata_json) in metadata_rows {
          let metadata: crate::conversation::message::MessageMetadata =
              serde_json::from_str(&metadata_json)?;
          sqlx::query("UPDATE messages SET metadata_json = ? WHERE id = ?")
              .bind(serde_json::to_string(&metadata.with_agent_invisible())?)
              .bind(row_id)
              .execute(&mut *tx)
              .await?;
      }
  }
  ```
- **Vulnerability / Flaw**:
  When activating a handoff snapshot, all historical messages up to `covered_through_row_id` are marked `agent_invisible`.
  The current implementation fetches all rows into memory and runs a sequential Rust loop executing an individual SQL `UPDATE` for every single message.
  In large sessions (e.g. 500+ messages), this holds an exclusive transaction lock on SQLite while performing 500 individual round-trips. Furthermore, if any single row fails serde parsing, the entire handoff transaction is rolled back.
- **Recommended Remediation**:
  Use SQLite's native JSON support:
  ```sql
  UPDATE messages 
  SET metadata_json = json_set(metadata_json, '$.agent_invisible', json('true')) 
  WHERE session_id = ? AND id <= ?;
  ```

---

## 6. Workspaces & Isolation

### 6.1 Synchronous File Locking Inside Async Service Methods (`AUD-DAT-012`)
- **File Reference**: [`crates/gosling/src/workspace/store.rs:202-254`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/workspace/store.rs#L202-L254), [`crates/gosling/src/workspace/service.rs:77`](file:///Users/eric/Work/vscode/forked/gosling/crates/gosling/src/workspace/service.rs#L77)
- **Mechanism**:
  `WorkspaceStore::load` and `WorkspaceStore::mutate` use `fs2::FileExt`:
  ```rust
  let lock = self.open_lock()?;
  lock.lock_shared()?;
  let result = self.read_document(&self.path);
  FileExt::unlock(&lock)?;
  ```
- **Vulnerability / Flaw**:
  `lock.lock_shared()` and `lock.lock_exclusive()` are blocking system calls (`flock` / `lockf`).
  These methods are called directly within async execution paths in `WorkspaceService` (e.g., `list_workspaces`, `create_workspace`) without dispatching via `tokio::task::spawn_blocking`. Under concurrent CLI and GUI operations, if a write lock is held, async worker threads can be stalled waiting for OS file locks.
- **Recommended Remediation**:
  Wrap synchronous store accesses in `tokio::task::spawn_blocking` or use an async-compatible file lock.

---

## 7. Comprehensive Defect & Risk Register

| ID | Location | Risk / Failure Mode | Impact |
| :--- | :--- | :--- | :--- |
| **AUD-DAT-001** | `crates/gosling/src/context_mgmt/memory.rs:63` | Global `memories.jsonl` without workspace filter | Confidential facts from one workspace leaked into LLM prompts of other workspaces. |
| **AUD-DAT-002** | `crates/gosling/src/context_mgmt/memory.rs:133` | `.take(8MB)` reads file head instead of tail | Recent memories in append-only file are completely ignored once file exceeds 8MB. |
| **AUD-DAT-003** | `crates/gosling/src/session/research.rs:172` | `Err(_) => return true` on >2000 artifacts | Deliverable verification silently bypassed; incomplete turns reported as successful. |
| **AUD-DAT-004** | `crates/gosling/src/acp/server/research_completion.rs:298` | Strict `.collect::<Result<Vec<_>>>()` on output roots | Turn aborts with error if any secondary output path is missing, despite successful closeout. |
| **AUD-DAT-005** | `crates/gosling/src/context_mgmt/mod.rs:713` | `stored.max(estimated_tokens)` after message truncation | Truncated conversations falsely trigger auto-compaction loop due to stale stored token counts. |
| **AUD-DAT-006** | `crates/gosling/src/agents/agent/reply_stream.rs:755` | `sleep(100ms)` loop during tool execution stream | High idle CPU and thread wakeups during long tool operations. |
| **AUD-DAT-007** | `crates/gosling/src/session/session_manager/pool_lifecycle.rs:24` | In-memory `FIRST_INIT_LOCK` for database setup | CLI and Desktop starting simultaneously crash with SQLite locked error. |
| **AUD-DAT-008** | `crates/gosling/src/session/session_manager/output_revisions_storage.rs:41` | Unscoped `output_revisions` table | Cross-session version collisions and permanent storage leaks of file BLOBs on session deletion. |
| **AUD-DAT-009** | `crates/gosling/src/session/session_manager.rs:1098` | Full `list_all_sessions()` scan on import | $O(N)$ memory and deserialization bottleneck on session import. |
| **AUD-DAT-010** | `crates/gosling/src/session/session_manager/schema.rs:100` | Unused `schedule_id` and recipe columns | Unused dead schema columns retained from upstream Goose. |
| **AUD-DAT-011** | `crates/gosling/src/session/session_manager/handoff_storage.rs:263` | Sequential row-by-row message updates in transaction | Handoff commit holds transaction lock while performing N round-trip queries. |
| **AUD-DAT-012** | `crates/gosling/src/workspace/store.rs:203` | Blocking `lock_exclusive()` on async worker thread | Potential Tokio executor thread starvation during concurrent workspace mutations. |

---

## 8. Verification Evidence

- **Clean Status**: No runtime code files or test files were modified.
- **File Validation**: Audited file paths and symbol definitions verified directly against current repository state.
- **Git State**:
  ```bash
  git status --short
  ```
  Only the documentation record `docs/cloud/20260913_Gemini_Audit_Data_gosling.md` has been added.
