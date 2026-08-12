# Six-lens agent-skills audit and defect-repair campaign — 2026-08-12

Repository: `cephalopod-ai/gosling`
Audit baseline: `11b5dc411f862fc69f7f35439c90a172dceeaab5`
Repair branch: `codex/full-audit-repair-20260812`
Method: static repository audit plus targeted and broad observed validation
Selected catalog workflows: `audit-agent-orchestration-code`, `audit-reliability`,
`audit-dataflow-integrity`, `audit-dataflow-concurrency`, `audit-security`,
`audit-negative-space`, followed by `repair-defect-campaign`

## Executive verdict

The six-lens sweep froze five source-evidenced findings: four repairable defects and one
architectural residual. The repair campaign closed all four in-scope defects in separate,
reviewable local commits:

| ID | Severity | Confidence | Disposition | Repair |
|---|---|---|---|---|
| AUD-2026-001 | High | Confirmed source property; runtime reproduction covered by regression | Repaired | `c78b67042`, `b0dd032f7` |
| AUD-2026-002 | High | Confirmed | Repaired | `94ce6be70` |
| AUD-2026-003 | Medium | Confirmed source property; concurrent manifestation reproduced by regression | Repaired | `2bc4dc27a` |
| AUD-2026-004 | Medium | Confirmed source property; lock exclusion reproduced by regressions | Repaired | `378add252` |
| AUD-2026-005 | High | Likely runtime manifestation; unsafe source property is Confirmed | Deferred architecture repair | — |

No Critical finding was confirmed. The principal remaining risk is AUD-2026-005: Bedrock and
SageMaker provider construction writes AWS credentials into the process-global environment even
though the source itself documents that concurrent `getenv`/`setenv` can crash the process.

## Scope and evidence limits

The sweep covered the Rust workspace, CLI, ACP/server boundaries, Electron and text UI source,
session persistence, workspace/project state, provider construction, tool execution and recovery,
memory JSONL, configuration, tests, repository governance, and recent audit/session records.
The workspace contains 2,135 source/configuration files; the primary inventories were 440 Rust
files (216,778 lines), 461 Desktop TypeScript/TSX files (72,877 lines), and 20 text-UI files.

Runtime claims were not promoted beyond their evidence. File-lock behavior, bounded output,
permission modes, and interrupted-request recovery were observed in deterministic local tests.
The AWS libc race was not deliberately triggered because a process crash is nondeterministic and
the source already states the unsafe mechanism. External provider faults, OAuth callbacks,
multi-process network clients, and Desktop interaction were not live-driven in this campaign.

The `.giles` metadata was treated as an advisory, stale snapshot. Its last recorded scan is from
2026-07-07 and includes an internal tool crash; it was not promoted over current `AGENTS.md`, code,
or observed tests.

## Architecture maps

### Role and mode matrix

| Actor/mode | Authority and boundary | Input/output contract | Failure/recovery policy |
|---|---|---|---|
| User agent — Chat | Inspect and explain; tool calls are skipped or require explicit policy | Provider messages and typed tool requests → persisted conversation | Errors remain visible; no implicit side effect |
| User agent — Auto | Approved local tools may execute; security inspectors still fail closed | Provider messages → inspection → durable tool ledger → responses | Dispatched interruption becomes `in_doubt`; never auto-retried |
| User agent — Manual/approval | Each gated tool request waits for confirmation | `ActionRequired::ToolConfirmation` → router → dispatch/denial response | Explicit turn cancellation closes undispatched siblings; restart recovery remains ledger-scoped |
| Subagent | Auto mode with role capability intersection from the parent and role policy | Parent task → child session → bounded result | Unanswerable approval is denied; parent receives failure |
| ACP/Desktop/server client | Authenticated transport/client boundary; backend remains source of truth | Typed ACP/custom requests → session/workspace services | Resume/load recover conversation-bound operations first |
| Provider adapter | Model/provider network or subprocess authority; no direct session authority | Canonical conversation → provider format → canonical messages | Adapter errors surface; fallback is provider-specific |

### Trust-boundary map

| Producer | Artifact | Validator/enforcement | Consumer |
|---|---|---|---|
| Model provider | assistant text/tool requests | format parser, tool inspection, mode/permission routing | agent loop and MCP tools |
| Agent loop | tool request checkpoint | SQLite transaction and stable request ID | durable operation ledger |
| MCP/server | terminal result | typed result serialization and request-ID match | conversation/provider history |
| Renderer/ACP/CLI | session and workspace mutations | authenticated/typed handler plus backend service | SQLite or locked workspace store |
| Summarizer | memory JSONL records | serde serialization plus file lock | `FileMemorySource` parser/retriever |
| Local config/log/session state | diagnostics report | byte caps, explicit errors, private output file | operator/support workflow |
| Provider config/secrets | AWS environment variables | prefix/type filtering only; process-global mutation remains | AWS SDK default credential chain |

### Run-state transition map

`user prompt persisted → provider inference → assistant/tool request persisted → inspection →
approval/denial → durable dispatch start → terminal result → response persisted → next inference`.

The important interruption seams are:

- before dispatch: explicit turn cancellation gives every persisted sibling a terminal response;
- after durable dispatch: state is `in_doubt` and must not be retried automatically;
- after terminal result but before conversation response: recovery replays the stored result;
- during local JSON replacement: readers see the prior complete file or the replacement, never a
  partial file;
- during JSONL append: readers wait for the complete writer batch.

## Detailed findings and closure

### AUD-2026-001 — Undispatched requests in a multi-tool turn remained unmatched

Severity: **High**
Confidence: **Confirmed**
Domains: AOC-027, REL-007/008, DAT-007, CON-009, NEG-001/006/015
Status: **Repaired** in `c78b67042` and adversarial tightening `b0dd032f7`

Evidence at the audited baseline:

- `crates/gosling/src/agents/agent.rs:2607-2636` persisted every tool request from one provider
  response before the sequential approval loop ran.
- `crates/gosling/src/agents/tool_execution.rs:96-163` waited for approvals one request at a time.
- `crates/gosling/src/session/session_manager.rs:3672-3795` recovered only requests that already
  had a `tool_operations` row, returning immediately when no row existed.

Observed behavior: if a turn contained multiple tool requests and the run was cancelled or the
process ended after an earlier request but before a later request reached dispatch, that later
persisted request had neither a response nor an operation-ledger row. Strict providers can reject
the next turn's structurally unmatched history.

Expected boundary: every persisted tool request is followed by exactly one terminal response.
Pre-dispatch interruption is a cancellation; post-dispatch interruption remains explicitly in
doubt.

Failure mechanism: request persistence was batch-oriented while recovery was ledger-oriented.
The gap between those two boundaries was treated as impossible even though sequential approval
made it reachable.

Repair: after persisting the request that received the CLI cancellation, Gosling invokes a
transactional sibling-closure operation scoped to the assistant message containing that exact
request. It excludes requests with an existing response or durable operation-ledger row and
synthesizes idempotent cancellation responses only for undispatched siblings
(`session_manager.rs:3808-3869`, `gosling-cli/src/session/mod.rs:1228-1242`). Generic session
load/resume recovery remains ledger-only.

The repair campaign's adversarial review rejected the broader initial recovery implementation:
generic recovery can run in a second client while the first client is still waiting on a valid
approval, so treating every no-ledger request as abandoned could cancel live work. The follow-up
commit moved cancellation to the confirmed CLI boundary and restored the narrower recovery
contract.

Validation: `cancelling_tool_request_cancels_undispatched_siblings_once` asserts one response per
request, terminal cancellation text, and zero changes on a second cancellation pass
(`session_manager.rs:5153-5225`). `generic_recovery_does_not_cancel_a_pending_approval` is the
negative concurrent-client regression (`session_manager.rs:5228-5264`). Existing in-doubt
recovery and the CLI persistence-before-next-turn regression also pass.

Non-goal: infer whether an operation without a durable dispatch record affected an external
system. The boundary deliberately says it did not start within Gosling's dispatch pipeline.

### AUD-2026-002 — Diagnostics caps did not bound reads and sensitive bundles inherited ambient permissions

Severity: **High**
Confidence: **Confirmed**
Domains: AOC-003, REL-005/013/015, SEC-007, NEG-003/010
Status: **Repaired** in `94ce6be70`

Evidence at the audited baseline:

- `crates/gosling/src/session/diagnostics.rs:217-256` called `read_to_string` before truncating,
  so both the server-log tail and “capped” LLM/config reads allocated the entire file.
- `crates/gosling-cli/src/commands/session.rs:385-402` serialized a full report and created the
  output with `File::create`, without owner-only permissions or a sensitive-content warning.

Observed behavior: the advertised cap bounded the returned string but not disk I/O or memory.
A sparse 64 MiB file demonstrated that only the final patch actually reads the bounded head and
tail. The report may contain session prompts, raw config, templates, and LLM logs, while a normal
Unix umask could create it as group/world-readable.

Expected boundary: diagnostics collection performs bounded I/O, reports truncation truthfully,
creates output owner-only, flushes it durably, and warns before operators share it.

Repair: seek-based head/tail reads now cap actual bytes (`diagnostics.rs:219-267`), server-log
truncation is derived from bytes/lines (`diagnostics.rs:352-378`), and CLI output is forced to
`0600` on Unix even when replacing a permissive file, synced, and accompanied by a review warning
(`commands/session.rs:395-447`).

Validation: seven diagnostics tests pass, including a 64 MiB sparse-file bound and truthful tail
flags (`diagnostics.rs:427-488`); the Unix overwrite-permission regression passes.

Non-goal: generic redaction of arbitrary prompt content. Full diagnostics are intentionally
complete support artifacts and must still be reviewed before sharing.

### AUD-2026-003 — `projects.json` updates were a tear-prone, unlocked read-modify-write

Severity: **Medium**
Confidence: **Confirmed**
Domains: DAT-007/010, CON-001/002/006/007/009/010/013/015, SEC-007, NEG-002/006/013
Status: **Repaired** in `2bc4dc27a`

Evidence at the audited baseline: `crates/gosling-cli/src/project_tracker.rs:55-75` read and
overwrote `projects.json` directly; lines 85-114 mutated a stale in-memory copy and saved it;
lines 138-141 composed those operations for every update. No lock or atomic replacement covered
the transaction.

Observed behavior: two CLI processes could read the same snapshot and lose one update; a crash
during `fs::write` could leave malformed JSON. Paths and last instructions also inherited ambient
file permissions.

Expected boundary: one lock covers read, mutation, serialization, file sync, atomic replacement,
permission repair, and directory sync.

Repair: a private sidecar lock now serializes the whole update, reads take a shared lock, and a
same-directory temporary file is synced then atomically persisted with owner-only permissions
(`project_tracker.rs:95-204`). `fs2` was added with `cargo add` using the workspace dependency.

Validation: eight barrier-synchronized writers preserve all eight projects and valid JSON; Unix
tests assert `0600` for both tracker and lock (`project_tracker.rs:214-282`).

Non-goal: silently discard or rebuild a tracker that was already malformed before this repair.
Existing corruption remains an actionable parse error.

### AUD-2026-004 — Shared memory JSONL readers and writers did not coordinate

Severity: **Medium**
Confidence: **Confirmed**
Domains: DAT-007/013, CON-001/006/008/009/010/012, NEG-002/006/013
Status: **Repaired** in `378add252`

Evidence at the audited baseline:

- `crates/gosling/src/context_mgmt/summarizer/writer.rs:54-74` appended a multi-record batch with
  no lock or durable flush.
- `crates/gosling/src/context_mgmt/memory.rs:115-122` read the same file without a shared lock and
  silently skipped malformed lines, including an in-flight partial final line.
- The adjacent human-facing durable-memory writer already used `fs2` locking, proving the
  repository's intended coordination mechanism.

Expected boundary: JSONL batches are indivisible to readers and other writers; readers never
interpret an in-progress record as malformed durable data.

Repair: writers hold one exclusive lock across serialization, append, flush, and `sync_data`;
readers hold a shared lock while loading (`writer.rs:54-82`, `memory.rs:114-132`).

Validation: forced-interleave tests hold an opposing lock, assert that the operation blocks, then
release it and assert the complete record is observed (`writer.rs:306-347`,
`memory.rs:317-357`).

Non-goal: replace forgiving JSONL parsing or add an unbounded memory-file index in this repair.

### AUD-2026-005 — AWS provider construction mutates the process environment concurrently

Severity: **High**
Confidence: **Likely** for runtime crash/cross-session manifestation; **Confirmed** unsafe source property
Domains: AOC-019, REL-007, CON-001/012, SEC-015, NEG-002/003/007
Status: **Deferred — architectural repair required**

Evidence: `crates/gosling/src/providers/aws_env.rs:9-18` explicitly documents that
`std::env::set_var` races concurrent `getenv` on the multithreaded runtime and can segfault, while
providers are constructed per session and subagent. Lines 19-48 serialize Gosling writers but
cannot serialize reads performed by libc, the AWS SDK, dependencies, or other threads. Bedrock
and SageMaker both call this exporter (`providers/bedrock.rs:95` and
`providers/sagemaker_tgi.rs:53`).

Expected boundary: credentials and AWS settings are passed through provider-specific AWS SDK
configuration/credential providers, scoped to that provider instance, never through mutable
process-global environment.

Failure mechanism: a mutex guards only calls through `export_aws_env`; it does not make libc's
process environment safe against readers outside the mutex. Different session configurations can
also become a last-writer-wins credential source.

Impact: rare process crashes and credential/configuration bleed between concurrent Bedrock or
SageMaker sessions/subagents.

Recommended repair: construct an AWS `SdkConfig` with explicit credentials, region, profile, and
endpoint inputs and pass it to both provider clients. Delete runtime environment export after
adapter parity tests cover env precedence, config reload, concurrent different credentials, and
subagent construction.

Deferral reason: this crosses two provider adapters and changes credential precedence and reload
semantics. The campaign contract forbids disguising that architecture change as a narrow repair.

## Complete lens inventory

Every required code was dispositioned. “Not confirmed” means the reviewed current paths had an
enforced control or no source-evidenced material defect; it is not a claim about hypothetical
future integrations.

### Agent orchestration (`AOC-001`–`AOC-030`)

| Codes | Disposition |
|---|---|
| AOC-003 | AUD-2026-002 repaired |
| AOC-019 | AUD-2026-005 deferred |
| AOC-023 | AUD-2026-003 and AUD-2026-004 repaired |
| AOC-027 | AUD-2026-001 repaired |
| AOC-001/002/004/005/006/007/008/009/010/011/012/013/014/015/016/017/018/020/021/022/024/025/026/028/029/030 | Not confirmed; current role policy, parsing, checkpoint, cancellation, provider/test, usage, and operator-signal controls held in the reviewed paths |

### Reliability (`REL-001`–`REL-015`)

| Codes | Disposition |
|---|---|
| REL-005/013/015 | AUD-2026-002 repaired |
| REL-007/008 | AUD-2026-001 repaired; AUD-2026-005 deferred for its process-crash mechanism |
| REL-001/002/003/004/006/009/010/011/012/014 | Not confirmed in the reviewed startup, health, retry, error, partial-output, cleanup, and configuration paths |

### Data integrity (`DAT-001`–`DAT-015`)

| Codes | Disposition |
|---|---|
| DAT-007 | AUD-2026-001, AUD-2026-003, and AUD-2026-004 repaired |
| DAT-010 | AUD-2026-003 repaired |
| DAT-013 | AUD-2026-004 repaired |
| DAT-001/002/003/004/005/006/008/009/011/012/014/015 | Not confirmed; scoped session/workspace IDs, imports, provenance, migrations, and authority separation held in the reviewed paths |

### Concurrency (`CON-001`–`CON-018`)

| Codes | Disposition |
|---|---|
| CON-001/012 | AUD-2026-003 and AUD-2026-004 repaired; AUD-2026-005 deferred |
| CON-002/006/007/013/015 | AUD-2026-003 repaired |
| CON-008 | AUD-2026-004 repaired |
| CON-009/010 | AUD-2026-001, AUD-2026-003, and AUD-2026-004 repaired |
| CON-003/004/005/011/014/016/017/018 | Not confirmed; durable operation identities, transactions, lock ordering, canonical creation, scoped bulk behavior, artifact ownership, and reentrancy controls held in reviewed paths |

### Security (`SEC-001`–`SEC-015`)

| Codes | Disposition |
|---|---|
| SEC-007 | AUD-2026-002 and AUD-2026-003 repaired |
| SEC-015 | AUD-2026-005 deferred |
| SEC-001/002/003/004/005/006/008/009/010/011/012/013/014 | Not confirmed; transport auth, backend authorization, object scoping, path canonicalization, deployment defaults, tool inspection, and UI-bypass controls held in reviewed paths |

### Negative space (`NEG-001`–`NEG-015`)

| Codes | Disposition |
|---|---|
| NEG-001/015 | AUD-2026-001 repaired |
| NEG-002/006/013 | AUD-2026-003 and AUD-2026-004 repaired; NEG-002 also contributes to AUD-2026-005 |
| NEG-003 | AUD-2026-002 repaired; AUD-2026-005 deferred |
| NEG-007 | AUD-2026-005 deferred |
| NEG-008 | Closed by the direct, negative, forced-interleave, idempotency, permission, and adjacent-failure regressions added with AUD-2026-001 through 004 |
| NEG-010 | AUD-2026-002 repaired with private output and an explicit sharing warning |
| NEG-004/005/009/011/012/014 | Not confirmed after export/import composition, alternate entry point, provider-output, future integration, and Giles advisory-authority checks |

## Repair grouping and architecture compliance

| Group | Pre-patch contract | Post-patch result | Architecture judgment |
|---|---|---|---|
| Session interruption | request checkpoint before dispatch; dispatched uncertainty is never retried | explicit CLI cancellation closes only undispatched siblings in the abandoned assistant batch; generic recovery remains ledger-only | Strengthens the existing boundary without stealing another client's live approval |
| Diagnostics | best-effort support snapshot with visible errors | real byte bounds, truthful truncation, private durable output, sharing warning | Aligns with existing private state-file conventions |
| Project tracker | CLI-local recent-project metadata | locked read/mutate/write and atomic private replacement | Reuses `WorkspaceStore` locking/atomicity pattern |
| Memory JSONL | forgiving file-backed retrieval plus summarizer producer | shared reader/exclusive batch writer lock | Extends the existing durable-memory lock convention |
| AWS providers | AWS default chain populated through environment export | unchanged | Deferred because correct repair changes provider construction/credential precedence |

The only touched file above 2,000 lines was `session_manager.rs`; its patch is confined to the
session interruption boundary and colocated regressions. No modularization was mixed into the
defect repair.

## Validation record

Baseline:

- Desktop, inside Hermit: typecheck passed; 82 files / 555 tests passed.
- Initial workspace `cargo test`: reached broad passing suites, then failed two
  `config_validation_command_test` cases because the compiled test could not find the Gosling
  binary (`Os { code: 2, kind: NotFound }`). This was recorded as baseline, not attributed to the
  repair.

Targeted repair tests passed:

- session: scoped sibling cancellation, idempotency, live-approval negative regression, CLI
  persistence ordering, and existing in-doubt recovery;
- diagnostics: 7 core tests and Unix owner-only overwrite test;
- project tracker: 8-writer forced concurrency and private permissions;
- memory: reader/writer forced lock interleaves.

Final validation passed: `cargo build`, `cargo fmt --all -- --check`, full-workspace `cargo test`,
and `cargo clippy --all-targets -- -D warnings`. Desktop validation inside Hermit also passed:
typecheck plus 82 files / 555 tests. Documentation markers and diff checks are recorded in
`docs/logs/session/2026-08-12-six-lens-audit-repair.md`. The unrelated operator-owned
`Cargo.toml` profile edit was excluded from campaign commits and campaign diff review.

## Residual risk and next action

AUD-2026-005 remains open. It should be repaired as a dedicated AWS credential-construction
change, with Bedrock and SageMaker parity tests and a deliberate review of precedence/reload
semantics. No campaign commit was pushed or merged.
