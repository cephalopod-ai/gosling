# Audit — Memory Lifecycle Lens (gosling)

Lens: `audit-memory-lifecycle` (adapted for a Rust core with **no GC** + an
Electron/Node UI that **does** have GC). Authority: **audit-only / read-only**.
Builds on `docs/cloud/00-orientation.md`. IDs: `MEM-GSL-NNN`.

Skill note on Rust adaptation: the skill's "managed heap / forced-GC snapshot"
protocol does not map to the Rust crates — there is no GC sawtooth, no
`--max-old-space-size`, and RSS is driven by explicit `Vec`/`HashMap`/`String`
allocation + `Arc` refcount lifetime. The lens question therefore becomes:
*which long-lived roots grow monotonically, and which per-call materializations
have no size cap?* The Node/Electron side does have a real managed heap and the
classic listener/closure retention shapes apply; those were checked directly.

---

## 1. Intake summary

| Item | Value |
|---|---|
| Runtimes | Rust (tokio async) core; Node/V8 (Electron main + renderer) UI |
| Deployment shapes | (a) **CLI** = process-per-session, short-lived; (b) **gosling-server** = long-lived, multi-session, `AgentManager` LRU up to 100 agents; (c) **Electron desktop** = long-lived GUI process |
| Heap limits | None found configured. No cgroup `memory.max`, no `--max-old-space-size` for the Electron main/renderer, no Rust allocator cap. Growth is bounded only by host RAM → OOM-kill by the OS. (Source: no limit flag in `ui/desktop/src/*.ts` launch path or server startup that I read.) |
| Symptoms supplied | None (proactive audit). All findings are `source-evidenced` / `simulation-reasoned`; **no heap was measured** (read-only, no build/run). |

Deployment weighting: the **server** shape is where retention matters most
(one process, many sessions, process lifetime = days). The CLI shape self-heals
on exit. The findings below are scored primarily against the server shape and
note the CLI posture separately.

## 2. RSS decomposition statement

Not measured. Reasoned statically: on the Rust side, RSS ≈ explicit heap
allocations (conversation `Vec<Message>`, caches, per-call buffers) + tokio task
stacks + native tokenizer (`tiktoken`/`CoreBPE`) tables. No GC, so "flat heap +
rising RSS" does not apply; a growing Rust container grows RSS directly. The two
findings below are pure managed-allocation growth (a static `HashMap` and a
per-call full-file read), not native/off-heap or FD-backed — they stay in this
lens and are **not** routed to `audit-resource-lifecycle`. On-disk growth of
`memories.jsonl` itself (the *file*) is a resource-lifecycle concern and is
routed there; this lens owns only the in-memory read of it.

## 3. Memory surface map

Long-lived roots (process lifetime):
- `context_mgmt/summarizer/mod.rs:147` `DIGEST_CACHE: OnceLock<Mutex<HashMap<u64, CachedDigest>>>` — **static, unbounded** → **MEM-GSL-001**.
- `execution/manager.rs:33` `sessions: LruCache<String, Arc<Agent>>` (cap 100) — bounded.
- `gosling-server/src/state.rs:27` `session_buses: LruCache<String, Arc<SessionEventBus>>` (cap = `DEFAULT_MAX_SESSION` 100) — bounded.
- `token_counter.rs:25` `token_cache: LruCache<TokenCacheKey, usize>` (cap 1024) — bounded.
- `providers/.../catalog.rs:20` `PROVIDER_METADATA` static; `canonical/name_builder.rs`, `security/patterns.rs:310` static regex maps — closed sets, bounded.

Per-call materialization (transient, freed after the call):
- `agents/agent.rs:1772` `FileMemorySource::from_config().retrieve(...)` → `context_mgmt/memory.rs:116` `std::fs::read_to_string(&self.path)` of whole `memories.jsonl`, **every turn**, unbounded file → **MEM-GSL-002**.
- `agents/agent.rs:1777` `conversation.messages().clone()` + `conversation/…fix_conversation` clones (agent.rs:659-660) — full-conversation copies per turn → **MEM-GSL-003**.
- `agents/large_response_handler.rs` — offloads tool text > 200 KB to a file — mitigation, non-finding (see §7).

Queues / channels:
- `gosling-server/.../reply.rs:215` `mpsc::channel(100)` — bounded.
- `session_event_bus.rs:47` `broadcast::channel(256)` + replay buffer bounded by count(512) **and** bytes(8 MiB) — bounded (exemplary).
- `developer/shell.rs:538` `unbounded_channel` for shell output — producer capped at 10 MiB (exemplary; §7).
- `acp/server.rs:256`, `platform_extensions/summon.rs:1000/1434` `unbounded_channel` — low-volume, keep-up consumers (§7).

Node/Electron:
- `ui/desktop/src/main.ts` — 44 `ipcMain.on/handle`, all registered once at module/startup scope; 0 removals (idiomatic for app-lifetime singletons).
- Renderer `window.electron.on(...)` — paired with `.off(...)` in `useEffect` cleanup (§7).

## 4. Retention matrix (material object classes)

| Object class | Alloc site | Retaining root | Intended lifetime | Clearing event | Verdict |
|---|---|---|---|---|---|
| `CachedDigest` (summary strings) | `summarizer/mod.rs:178` `store_digest` | `DIGEST_CACHE` static | ~session block | **NONE** (only `clear_cache_for_test`) | MEM-GSL-001 |
| `memories.jsonl` contents (raw+parsed+keyword sets) | `memory.rs:116-123`,`161-177` | none (dropped after `retrieve`) | single turn | function return | MEM-GSL-002 (churn/materialization, not retention) |
| Conversation copies | `agent.rs:659-660,1777` | none (turn-scoped locals) | single turn | function return | MEM-GSL-003 (transient dup) |
| `Arc<Agent>` (holds session state) | `execution/manager.rs:54` | `sessions` LRU | session | LRU eviction / removal | bounded — non-finding |
| Server conversation | reloaded per reply from disk (`reply.rs`), not held on `Agent` | request | request end | bounded — non-finding |

No `Arc<Mutex<...>>` reference **cycle** was found in the roots I traced
(`AgentManager`, `SessionEventBus`, agent loop). `creation_locks`
(`execution/manager.rs:45`) is explicitly documented as pruned on removal/evict;
its `Arc<Mutex<()>>` outliving the map entry is by-design and bounded by
concurrent callers. Cycle search was **not exhaustive** (see Validation Limits).

## 5. Bounds assessment table

| Container | Key cardinality | Bound | Eviction/TTL | Worst-case | Backpressure | Verdict | MEM |
|---|---|---|---|---|---|---|---|
| `DIGEST_CACHE` | content hash of conversation blocks = **user/session-mintable, unbounded** | **NONE** | **NONE** | grows w/ distinct summarized blocks over process life | n/a | **UNBOUNDED** | MEM-004 |
| `token_cache` | blake3 of text, LRU | 1024 entries | LRU | ~1024×(40B+8B)×2 ≈ 0.1 MB | n/a | bounded | — |
| `sessions` (agents) | session id | 100 | LRU | 100 × agent | n/a | bounded | — |
| `session_buses` | session id | 100 | LRU | 100 × 8 MiB = 800 MB | n/a | bounded, **large** | note |
| event-bus replay | seq | 512 **or** 8 MiB | drop-oldest | 8 MiB/session | broadcast lag-drop | bounded (exemplary) | — |
| shell output chan | n/a | 10 MiB collected + 1 MiB/line | drop-after-cap, keep draining | 10 MiB/exec | producer stops | bounded (exemplary) | — |
| `reply` mpsc | n/a | 100 | await (block) | small | block | bounded | — |
| memories.jsonl read | n/a (full file) | **NONE (file size)** | n/a | = file size ×~3 (raw+parsed+keyword sets), per turn | n/a | **UNBOUNDED input** | MEM-008/013 |

`session_buses` worst case (100 × 8 MiB ≈ 800 MB of replay buffers alone, per
server process) is a bound that binds high; called out as a sizing note, not a
finding (each individual bound is correct and intentional).

## 6. Findings

### MEM-GSL-001: Process-lifetime summarizer digest cache has no bound, eviction, or TTL

Severity: Medium (Low under default config; Medium when `GOSLING_SUMMARIZER=on` on the long-lived server)
Confidence: Confirmed (unbounded-container *property*; OOM *manifestation* Likely)
Evidence basis: source-evidenced
Domain: Reliability / Memory Lifecycle (MEM-004)

Evidence:
- `crates/gosling/src/context_mgmt/summarizer/mod.rs:147`
  `static DIGEST_CACHE: OnceLock<Mutex<HashMap<u64, CachedDigest>>>`
- `:178-183` `store_digest` only ever `insert`s; the sole other mutation is
  `:186-191 clear_cache_for_test` (`#[cfg(test)]`). No `remove`, no cap, no TTL,
  no LRU in production.
- Key is `cache_key_for` (`:157-164`) = blake3 of **rendered conversation
  block text** → cardinality is driven by session content (user-mintable,
  effectively unbounded over a process lifetime).
- Populated from `run_pending` `:275-280` only in `SummarizerMode::On` +
  `SummarizerTarget::ContextPacket`. Default is **Off** (`:329-332`
  `defaults_to_off`), which is why this is Low under stock config.

Observed behavior:
- With the summarizer enabled, every compaction that produces a fresh digest
  for a not-yet-cached block inserts a permanent `(u64, String)` entry into a
  static map that is never pruned for the life of the process.

Expected boundary:
- A bounded cache: LRU/LFU entry cap (an `lru::LruCache`, already a workspace
  dep and used in `token_counter.rs`/`execution/manager.rs`), or a TTL, or
  clearing on session close.

Failure mechanism:
- CLI: process-per-session, so the map dies with the process — negligible.
- Server: one process serves many sessions for days; distinct conversation
  blocks keep minting new keys, so retention scales with total historical
  churn, not concurrent sessions. Slow monotonic growth.

Break-it angle:
- Run the server with `GOSLING_SUMMARIZER=on`, drive N sessions each long enough
  to trigger several compactions; `DIGEST_CACHE.len()` and RSS grow monotonically
  and never return to baseline after sessions end.

Impact:
- Distant OOM on a long-uptime server. Per-entry is small (a summary `String`,
  bounded by summarizer output ~1-2 KB, + key 8 B + HashMap overhead ~2×).
  Time-to-OOM arithmetic (extrapolated, no heap limit configured): assume ~5
  new digests/session × 2 KB × 2 ≈ 20 KB/session; 10 000 sessions ≈ 200 MB.
  Slow — Medium, not High — but genuinely unbounded.

Operational impact:
- Blast radius: Service (whole server process). Side-effect class: none (pure
  memory). Reversibility: reversible (restart). Operator visibility: silent
  (no metric on cache size). Rerun safety: safe.

Adjacent failure modes:
- Same feature appends to `memories.jsonl` (`:287`) with no file-size cap — the
  on-disk twin of this (route to `audit-resource-lifecycle`) and the read-side
  amplifier MEM-GSL-002.

Recommended mitigation:
- Remediation pattern: bounded cache. Replace the raw `HashMap` with
  `lru::LruCache` (cap sized against the largest realistic single session's
  block count, e.g. 512) or key the cache per-session and drop it on session
  close. Add a gauge on cache length.
- Behavior test: enable summarizer, insert > cap distinct digests, assert
  `len()` stops at the cap and evictions occur.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: 1 module, 1 test.
  Nominal agent: codex. Rationale: single-container swap to an already-used dep.

Validation:
- Assert entry count plateaus at the cap under a unique-key flood; assert RSS
  returns toward baseline after sessions end (would upgrade to Confirmed/
  runtime-observed).

Non-goals:
- Do not change summarizer semantics or the memories.jsonl write path here.

---

### MEM-GSL-002: Whole `memories.jsonl` is read + fully parsed on every provider turn, with no file-size bound

Severity: Medium
Confidence: Confirmed (full-read code property); latency/peak-memory manifestation Likely
Evidence basis: source-evidenced
Domain: Memory Lifecycle (MEM-008 materialization; MEM-013 per-turn re-parse churn)

Evidence:
- `crates/gosling/src/agents/agent.rs:1772`
  `let retrieved_memory = FileMemorySource::from_config().retrieve(&memory_query);`
  — called **unconditionally** in the per-turn context-build path (not gated
  behind `NoopMemorySource`).
- `crates/gosling/src/context_mgmt/memory.rs:115-123` `load()` does
  `std::fs::read_to_string(&self.path)` of the **entire** file, then
  `serde_json::from_str` per line into a `Vec<StoredMemory>`.
- `:161-177` `retrieve` then builds a keyword `HashSet` for **every** entry
  (`keywords(&stored.content)`, `:126-132`) and intersects — full parse + set
  construction per entry, per turn.
- `:96-99` doc comment concedes the bound is *assumed*: "the file is small
  enough that a synchronous read per provider call is acceptable **at MVP
  scale**." No size cap, no line cap enforced anywhere.
- Writer side has no cap: `summarizer/mod.rs:287` `append_memories` appends
  facts across sessions → the file grows monotonically on disk.

Observed behavior:
- Each provider call re-reads and re-parses the whole memories file. Peak
  transient memory ≈ file size × ~3 (raw string + parsed structs + keyword
  sets), plus O(entries) CPU, on the hot path in front of every model call.

Expected boundary:
- Streaming/line-capped read, or an in-memory index loaded once and refreshed
  on mtime change, or a hard file-size/entry cap with truncation. A cursor or
  bounded top-K scan instead of full materialization.

Failure mechanism:
- The file is unbounded (append-only, no rotation), the read is a full slurp,
  and it runs every turn — so both peak memory and per-turn latency scale with
  total accumulated memories, not with what a single recall needs.

Break-it angle:
- Point `GOSLING_MEMORY_FILE` at (or let the summarizer grow) a multi-hundred-MB
  jsonl; every turn then slurps + parses it. Peak RSS spikes per turn and
  turn latency degrades linearly; a large enough file spikes peak memory toward
  OOM on a memory-constrained host.

Impact:
- Transient (freed after the call) so it is churn/materialization, not a leak —
  but under concurrency (server: up to N in-flight turns) the peaks stack
  (MEM-019): `N × filesize × 3`. With no admission bound on concurrent turns
  and no heap limit, this is the more realistic OOM path than MEM-GSL-001.

Operational impact:
- Blast radius: Service. Side-effect class: file (read). Reversibility:
  reversible. Operator visibility: silent (shows only as latency/RSS spikes).
  Rerun safety: safe.

Adjacent failure modes:
- Co-routes to `audit-performance-profile` (per-turn latency) and
  `audit-resource-lifecycle` (unbounded on-disk file growth). MEM-GSL-001 is the
  producer that inflates this reader.

Recommended mitigation:
- Remediation patterns: streaming-vs-slurping + bounded input. Cap file size /
  entry count (rotate/trim on append); load once into an mtime-cached index
  instead of per-call read; or stream lines with an early top-K cutoff.
- Behavior test: recall against a 100 MB fixture; assert peak memory delta and
  per-call time stay under a stated bound (not merely that recall returns).

Implementation assessment:
- Complexity: local_guardrail → workflow_protocol (if caching/rotation added).
  Cost: S. Cost drivers: 2 modules (reader + writer cap), tests. Nominal agent:
  codex. Rationale: contained to `memory.rs` + the append path; measurable.

Validation:
- Peak-memory-delta + latency assertion against an oversized fixture.

Non-goals:
- Do not redesign the recall ranking algorithm in this slice.

---

### MEM-GSL-003: Full-conversation clones per turn (transient duplicate materialization)

Severity: Low
Confidence: Confirmed (clone sites); impact Plausible
Evidence basis: source-evidenced
Domain: Memory Lifecycle (MEM-011)

Evidence:
- `crates/gosling/src/agents/agent.rs:659-660` `fix_conversation` path clones
  the conversation twice (`unfixed_conversation.clone()` and
  `unfixed_conversation.messages().clone()`).
- `:1777` `conversation_messages: conversation.messages().clone()` builds
  another full copy for the `ContextBuildRequest` each turn.

Observed behavior:
- Two-to-three transient full copies of the conversation coexist while a turn is
  assembled; peak ≈ 2-3× the conversation size for the duration of the build.

Expected boundary:
- Pass by reference / `Arc<[Message]>` sharing, or build the packet from a
  borrowed slice, so the whole history is not deep-cloned per turn.

Failure mechanism / Impact:
- Transient, freed after the turn — not retention. Matters only for very long
  single sessions (a multi-MB conversation × 3 per turn), and only as a peak
  spike, not monotonic growth. Low.

Operational impact:
- Blast radius: Workflow (one session's turn). Side-effect class: none.
  Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Remediation pattern: share instead of copy (`Arc`/borrow). Minimal repair:
  thread a `&[Message]` / `Arc<[Message]>` through the packet builder.
- Behavior test: assert no full clone on the hot path (allocation-count probe)
  — hard to assert cheaply; report-only unless a long-session peak is measured.

Implementation assessment:
- Complexity: local_guardrail. Cost: S-M (touches packet-build signatures).
  Nominal agent: claude. Rationale: signature threading across context_mgmt.

Non-goals:
- Do not change conversation-fix correctness semantics.

## 7. Non-findings (checked and held)

- **`large_response_handler.rs`** — tool text > 200 KB (`DEFAULT_LARGE_TEXT_THRESHOLD`, `:7`) is written to a temp file and replaced in-context by a short pointer (`:28-48`). Keeps huge tool output out of conversation history. (Caveat noted, not a defect: the MCP result is already fully materialized in RAM before this runs — inherent to stdio JSON-RPC — so this bounds *retention*, not the initial *peak*.)
- **Shell output collection** — `developer/shell.rs:776` `MAX_COLLECTED_BYTES = 10 MiB` hard cap with continued pipe draining, plus `:709` 1 MiB per-line cap in `bounded_line_stream`. Exemplary: the `unbounded_channel` at `:538` is safe because the producer stops sending at the cap. The code comment (`:768-775`) explicitly reasons about the OOM this prevents.
- **Session event-bus replay buffer** — `session_event_bus.rs:8-13` bounds by count (512) **and** bytes (8 MiB), keeping ≥1 newest event; broadcast channel cap 256 with lag-drop. Byte-bound comment (`:9-12`) explicitly reasons about image/whole-conversation payloads. Exemplary.
- **`token_cache`** (`token_counter.rs:13,50`) LRU 1024; **`sessions`** (`execution/manager.rs:54`) and **`session_buses`** (`state.rs:43`) LRU 100. All bounded.
- **`active_requests`** (`session_event_bus.rs:42`) — `try_register_request` (`:158`) `retain`s only non-cancelled and rejects when non-empty (≤1/session); `cleanup_request` (`:181`) removes on completion. Bounded.
- **Static registries** — `PROVIDER_METADATA` (`catalog.rs:20`), `security/patterns.rs:310` regex map, `platform_extensions/mod.rs:27`, `builtin_extension.rs:7` — closed sets built once. Bounded.
- **Electron main IPC** (`ui/desktop/src/main.ts`) — 44 `ipcMain.on/handle` all at startup scope (app-lifetime singletons), not per-window; no accumulation across window cycles. `webContents.on`/`window.on('closed')` listeners die with their window (Electron auto-cleanup).
- **Renderer IPC listeners** — `window.electron.on(...)` consistently paired with `window.electron.off(...)` in `useEffect` cleanup returns (verified `SearchView.tsx:363-374`, `App.tsx:480-504`; `preload.ts:299-310` exposes symmetric `on`/`off`). `onUpdaterEvent`/`onMouseBackButtonClicked` return an unsubscribe closure. Balanced.
- **Server conversation** — reloaded from disk per `reply` (`reply.rs`), not retained on the long-lived `Agent`, so conversation growth is request-scoped, not process-retained.
- **`SessionNameUpdate` / summon `ServerNotification` unbounded channels** (`acp/server.rs:256`, `summon.rs:1000/1434`) — low-volume producers with keep-up spawned consumers; no producer-outpaces-consumer path found.

## 8. Dominant risk & next action

Dominant risk class: **MEM-004 (unbounded cache) + MEM-008/013 (per-turn full
read of an unbounded file)**, both on the server/long-uptime shape and both
tied to the summarizer/memory feature. MEM-GSL-002 is the higher-leverage of
the two because it runs **unconditionally every turn** (not gated) and its peak
stacks under turn concurrency (MEM-019) with no configured heap limit.

Highest-leverage next action: cap + index `memories.jsonl` reads (MEM-GSL-002),
then bound `DIGEST_CACHE` (MEM-GSL-001) with the `lru` crate already in the
workspace. Both are XS-S local guardrails.

## 9. Validation Limits (what was NOT done / reviewed)

- **No heap or RSS was measured.** Read-only, no build/run/soak. All growth
  claims are `source-evidenced`/`simulation-reasoned`; per `confidence_
  calibration.md`, OOM/exhaustion manifestations are capped at Likely. The
  time-to-OOM figures in MEM-GSL-001 are labeled extrapolations, not measurements.
- **No heap limit is configured** in any deployment path I read, so time-to-OOM
  is against host RAM, not a named cap — arithmetic is illustrative only.
- **Arc-cycle search was not exhaustive.** I traced the primary roots
  (`AgentManager`, `SessionEventBus`, agent loop) and found no cycle, but did
  not statically prove the whole `Arc<Mutex<...>>` graph acyclic.
- **Renderer growing-arrays** (chat message state, `ProgressiveMessageList`,
  session lists) were **not** deeply reviewed for virtualization/capping; chat
  message arrays grow with session length by nature (bounded by session, not a
  leak) but I did not confirm the renderer trims or virtualizes long histories.
- Not reviewed: `gosling-mcp/autovisualiser` assets, ACP subprocess buffering
  in `acp/provider.rs` beyond channel sizing, provider streaming accumulation in
  `gosling-providers/formats/*`, and per-subagent memory multiplication under
  deep subagent nesting (`summon.rs`) — flagged for a follow-up pass.
- `fix_conversation` internal allocation was inferred from the two clone call
  sites, not from reading the full function body.

## 10. Skill Escalation

| Observation | Route to | Why |
|---|---|---|
| `memories.jsonl` grows on disk with no rotation/cap | `audit-resource-lifecycle` | on-disk growth, not managed-heap |
| Per-turn full-file read degrades turn latency | `audit-performance-profile` | latency is the co-symptom of MEM-GSL-002 |
| No admission bound on concurrent turns (N × per-turn peak) | `audit-reliability` | unbounded concurrency multiplier (MEM-019) |
| `session_buses` 100 × 8 MiB replay = ~800 MB ceiling per server | `repair-failsafe-guardrails` | sizing decision if server RAM is tight |
| `GOSLING_MEMORY_FILE` is operator-pointable at arbitrary path/size | `audit-security` | user-controlled amplification surface (note only) |
