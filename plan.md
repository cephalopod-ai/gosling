# Gosling Continuity and Security Remediation Master Plan

Status: Active; v1.0.0 documentation prepared; release version bump, validation, tag, and publication remain maintainer-owned; Session Handoff proposed; Security Phases 1–3 implemented and targeted checks complete
Scope: Gosling core, session persistence, providers, ACP server, Desktop, build and test automation, dependency integrity, and repository governance  
Primary outcomes: Switching providers or models preserves enough verified session state to continue safely without asking the replacement model to rediscover prior work, and Gosling's delivery paths do not execute mutable remote installers or unpinned provider packages.

## 0. v1.0.0 documentation and release boundary

- Release-facing documentation targets `v1.0.0`: the README, release notes, release process, release checklist, installation/update manuals, documentation inventory, and TODO ledger are aligned.
- At the 2026-07-20 stewardship pass, the Rust workspace and Desktop manifests still reported `0.1.0`. The release owner must update every version surface to `1.0.0`, keep lockfiles/generated metadata aligned, and review the resulting diff before tagging.
- The historical v0.0.6 note, audit reports, scenario ledgers, and command evidence remain immutable point-in-time records. Current documents link to them without rewriting their results.
- Documentation preparation does not claim a published release, a green release workflow, a complete post-repair 110-card replay, or closure of the proposed Session Handoff and later security phases.
- Release execution is intentionally out of this documentation task: no branch, tag, GitHub release, package publication, updater promotion, build, test, or signing action is performed here.

## 1. Goals

- Make Gosling's persisted session ledger the source of truth for cross-provider continuity.
- Create a versioned, bounded, redacted `SessionHandoffSnapshot` before a provider or model transition.
- Support Gosling-managed, provider-managed, ACP, hybrid, and native-resume providers through an explicit capability contract.
- Replace the current provider/model/thinking-effort sequence with one atomic transition.
- Preserve the old provider and persisted configuration until the target is ready.
- Recover from provider failures by offering a checkpoint-backed continuation on another provider.
- Show users what continuity level a target provider can provide before switching.
- Keep durable project knowledge separate from transient session handoff state.

## 2. Non-goals

- Do not treat `AGENTS.md`, `CLAUDE.md`, or another repository file as the canonical session checkpoint.
- Do not silently write transient session state into the working repository.
- Do not require the outgoing provider to be healthy or available.
- Do not replay an unbounded raw transcript into a new provider.
- Do not resume active tool execution automatically on another provider.
- Do not transfer credential values, authorization headers, secrets, or unredacted sensitive tool output.
- Do not guarantee bit-for-bit reproduction of provider-private reasoning or proprietary provider session state.

## 3. Current seams to replace or generalize

- `Agent::update_provider` currently replaces the in-memory provider and then persists provider/model metadata without a transition record or rollback boundary.
- Desktop currently applies provider, model, and thinking effort as separate ACP configuration operations.
- `Provider::manages_own_context()` is too coarse to describe resume, import, model-switch, and handoff behavior.
- ACP providers have a useful one-time handoff mechanism, but it serializes all visible prior history without a dedicated handoff budget.
- `claude-code` starts a provider-owned context and sends only the latest user message.
- Session summaries and summary facts already provide useful rollup inputs, but generation is optional and a summary may be absent, stale, or incomplete.
- Project memory files contain durable facts and instructions, not a reliable current-task checkpoint.

## 4. Design principles

1. **Ledger first:** a snapshot must be constructible from persisted Gosling state even after the outgoing provider fails.
2. **Evidence over invention:** unknown state remains unknown; the snapshot must not infer completed work or successful commands without ledger evidence.
3. **Bounded by construction:** every section has a byte/token budget and source coverage metadata.
4. **Redacted before persistence:** the canonical stored snapshot is safe to deliver; delivery adapters do not receive an unredacted version.
5. **One transition owner:** one core service coordinates preparation, target activation, commit, and rollback.
6. **Capabilities drive behavior:** provider names and suffixes must not determine continuity behavior.
7. **No duplicated side effects:** prior tool activity is context, never an instruction to repeat a command or approval.
8. **Observable transitions:** users can inspect snapshot coverage, continuity class, activation state, and failures.

## 5. Core data model

### 5.1 `SessionHandoffSnapshot`

Add a versioned Rust type with a stable serialized representation. Version 1 should include:

```text
SessionHandoffSnapshotV1
  snapshot_id
  schema_version
  session_id
  generation
  trigger
  created_at
  source
    provider_id
    requested_model
    resolved_model
  target
    provider_id
    requested_model
    resolved_model
  coverage
    first_row_id
    covered_through_row_id
    covered_message_count
    source_hash
    summary_status
    recent_tail_message_count
    estimated_tokens
    truncations
  current_objective
  latest_user_intent
  completed_work[]
  decisions[]
  files_touched[]
  workspace_state[]
  commands_and_checks[]
  active_or_interrupted_operations[]
  current_errors[]
  attempted_mitigations[]
  pending_approvals[]
  unresolved_questions[]
  next_actions[]
  recent_conversation_tail[]
  redaction_report
  enrichment
    source
    status
    model
```

Every structured item should retain provenance where possible:

- message row or message ID
- tool request/response ID
- timestamp
- confidence or evidence class: `observed`, `summarized`, or `unknown`

Do not store chain-of-thought or provider-private reasoning.

### 5.2 Snapshot triggers

Use an enum rather than free-form strings:

- `UserRequestedSwitch`
- `ProviderFailure`
- `ModelChangeRequiresRecreation`
- `ThinkingEffortChangeRequiresRecreation`
- `SessionResume`
- `SessionFork`
- `ManualCheckpoint`

### 5.3 Persistence

Add a dedicated `session_handoff_snapshots` table rather than overloading the one-row `session_summaries` table. Reuse session summaries and facts as snapshot inputs.

Proposed columns:

```sql
snapshot_id TEXT PRIMARY KEY
session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE
generation INTEGER NOT NULL
schema_version INTEGER NOT NULL
trigger TEXT NOT NULL
status TEXT NOT NULL
from_provider TEXT
from_model TEXT
to_provider TEXT NOT NULL
to_model TEXT NOT NULL
covered_through_row_id INTEGER NOT NULL
source_hash TEXT NOT NULL
estimated_tokens INTEGER NOT NULL
snapshot_json TEXT NOT NULL
failure TEXT
created_at TIMESTAMP NOT NULL
activated_at TIMESTAMP
superseded_at TIMESTAMP
UNIQUE(session_id, generation)
```

Snapshot statuses:

- `prepared`
- `activating`
- `active`
- `failed`
- `rolled_back`
- `superseded`

Add indexes for `session_id`, `status`, and newest generation. Introduce the table through the existing SQLite schema-version migration mechanism and cover fresh-database plus upgrade paths.

### 5.4 Retention

- Retain the active snapshot and a small configurable number of prior transition generations per session.
- Delete snapshots when their session is deleted.
- Preserve failed transition metadata long enough to diagnose switching failures.
- Do not export snapshots by default unless a session export explicitly opts into internal handoff metadata.

## 6. Deterministic snapshot construction

Create a core `SessionHandoffBuilder` that depends on persisted session APIs, not on a provider instance.

### 6.1 Inputs

- persisted session messages and metadata
- current session summary and summary facts when present and current
- bounded recent message tail
- normalized built-in tool request/response records
- Agent runtime state for active turns, tools, approvals, cancellation, and errors
- current provider/model and requested target provider/model
- working directory and additional working-directory metadata

### 6.2 Extraction rules

- Derive the latest user intent from the newest visible textual user message.
- Prefer explicit goal/task state for the current objective; otherwise use the current summary plus recent user turns.
- Record completed work only from successful tool responses, committed ledger events, or a summary item with provenance.
- Record commands and checks with command text, outcome, exit status, and bounded output summary. Never imply that an interrupted command succeeded.
- Derive files touched from structured edit/write tool calls and verified tool responses. Distinguish `read`, `created`, `modified`, and `deleted`.
- Record current errors and attempted mitigations as separate entries.
- Record active tool calls and approvals as interrupted or pending; never mark them resumable without an explicit provider/tool capability.
- Use summary facts for decisions, constraints, and preferences only when their source coverage falls within the session.
- Preserve unresolved fields as empty/unknown rather than calling a model to fill them.

### 6.3 Optional outgoing-model enrichment

- Enrichment may improve objective, decisions, and next-action phrasing.
- It runs only after the deterministic snapshot exists.
- It has a short timeout and cannot delay failure recovery beyond a configurable limit.
- Failure, cancellation, authentication errors, and transport errors retain the deterministic snapshot unchanged.
- Enrichment output must conform to a schema, cite ledger evidence, pass redaction again, and never replace observed fields with unsupported claims.
- Record whether enrichment was skipped, failed, or accepted.

### 6.4 Budget

Start with a configurable total handoff budget:

- maximum: the smaller of 16,000 tokens or 10% of the target context window
- structured checkpoint target: up to 4,000 tokens
- recent tail: remaining budget after structured content and delivery framing
- tool output: summarized and capped per item
- always preserve the latest user intent, unresolved approvals, current errors, and next actions before adding older tail messages

Record all omitted sections and counts in `coverage.truncations`.

### 6.5 Source hash

- Canonically serialize the covered message row IDs, content hashes, relevant summary revision, structured runtime events, and target configuration.
- Compute a BLAKE3 hash.
- Use the hash for staleness detection, idempotent preparation, and auditability.
- Never use a hash match as proof that an external provider imported the snapshot.

## 7. Redaction and data safety

Apply redaction before `snapshot_json` is persisted.

- Replace known credential values with typed references such as credential-profile ID or provider name.
- Remove authorization headers, cookies, bearer tokens, API keys, private keys, and environment secret values.
- Apply bounded pattern-based redaction as a second layer.
- Exclude raw binary/image data; retain safe attachment metadata and references.
- Truncate large file contents, command output, diffs, and tool results.
- Mark every redaction and truncation in `redaction_report` without storing the removed value.
- Treat tool output as untrusted quoted context in delivery prompts.
- Make handoff messages explicitly state that historical tool calls and outputs must not be repeated without current user intent and permission.

Add security tests for secrets in tool arguments, tool responses, environment maps, URLs, HTTP headers, patches, and provider error bodies.

## 8. Provider capability contract

Replace `manages_own_context()` with structured capabilities.

```text
ContextOwnership
  Gosling
  Provider
  Hybrid

ProviderCapabilities
  context_ownership
  native_resume
  history_import
  in_place_model_change
  session_fork
  bootstrap_handoff
  bootstrap_acknowledgement
```

Use explicit enums for support level where behavior is not binary:

- `Unsupported`
- `Supported`
- `Required`

Provider implementations must declare capabilities. The default should describe a Gosling-managed API provider safely, not infer behavior from provider naming.

Compatibility rollout:

- temporarily retain `manages_own_context()` as a derived compatibility method
- migrate call sites to the capability object
- remove the boolean after all built-in and custom provider adapters are covered

Expose capabilities through provider inventory/config APIs so Desktop can predict continuity before switching.

## 9. Delivery strategies

Introduce a core `HandoffDeliveryPlan` selected from target capabilities.

### 9.1 Gosling-managed API provider

- Persist and activate the snapshot before the next provider call.
- Inject the structured snapshot as an internal, agent-visible context block followed by the bounded recent tail and current user request.
- Do not add the handoff as a synthetic user-authored message in the visible conversation.
- Mark delivery acknowledged when Gosling has constructed the target input successfully.

### 9.2 ACP or provider-managed agent

- Replace the ACP provider's unbounded raw-history memo with the core snapshot envelope.
- Send exactly one bootstrap handoff message before the current user request.
- Require the bootstrap response to acknowledge objective, current state, and next action without starting tools or repeating previous work.
- Store the target provider session ID and acknowledgement metadata with the snapshot generation.
- Add the same adapter to `claude-code` while retaining it only for compatibility; recommend `claude-acp` in product copy.

### 9.3 Native resume/import provider

- Initialize or import the provider session through its native API.
- Verify acknowledgement or returned provider session identity.
- Retain the redacted Gosling snapshot as fallback and audit evidence.
- If native import fails and bootstrap handoff is supported, fall back to bootstrap delivery before failing the transition.

### 9.4 New-context-only provider

- Do not claim continuity.
- Require explicit user confirmation in Desktop.
- Start a new provider context while retaining the old session and snapshot for later recovery.

## 10. Atomic provider transition

Create one `ProviderTransitionCoordinator` in core. All CLI, server, ACP, and Desktop switching paths must use it.

### 10.1 Request

Replace sequential provider/model/thinking-effort updates with one request:

```text
ProviderTransitionRequest
  session_id
  target_provider
  target_model
  target_thinking_effort
  target_context_limit
  request_params
  expected_current_generation
  active_operation_policy
```

### 10.2 State machine

1. Acquire a per-session transition lock and reject stale generations.
2. Inspect active turns, tools, approvals, and pending user steers.
3. By default, refuse to switch during an active tool or approval. Offer cancellation first and record interrupted state after cancellation completes.
4. Build, redact, validate, and persist the snapshot as `prepared`.
5. Create the target provider without replacing the old `Arc<dyn Provider>`.
6. Apply target model, thinking effort, mode, extensions, and working directories to the candidate provider.
7. Perform the target-specific readiness/import/bootstrap step and persist `activating`.
8. In one database transaction, update provider/model/thinking effort and mark the snapshot `active`.
9. Swap the in-memory provider only after the database commit succeeds.
10. Retain the old provider until commit and activation events are published.
11. On pre-commit failure, mark the snapshot `failed`, discard the candidate, and leave the old provider/config untouched.
12. If the in-memory swap or notification fails after commit, run compensating rollback using the recorded previous configuration and mark `rolled_back`.

Never expose a session whose database metadata names one provider while the Agent holds another without an explicit transition/rollback status.

### 10.3 Post-activation first response

- The target's first visible response must briefly state its understood objective, current state, and next action.
- It must ask for clarification if required fields are unknown or contradictory.
- It must not resume an interrupted tool operation automatically.
- Record acknowledgement against the snapshot generation.

## 11. Error-driven recovery

Classify provider errors that can offer checkpoint continuation, including transport, authentication, rate limit, unsupported model, server, and context failures.

On eligible failure:

- persist the failed provider error in the session ledger
- prepare or refresh a deterministic snapshot without calling the failed provider
- publish a recovery capability event to clients
- allow the user to select a target provider/model
- invoke the same atomic transition coordinator
- preserve the failed provider as the rollback target when practical

The action label is:

> Continue with another model using session checkpoint

## 12. Desktop product behavior

### 12.1 Model picker

For every target provider/model, display one continuity class:

- **Seamless resume**: native resume/import with acknowledgement
- **Summarized handoff**: bounded Gosling snapshot delivered through bootstrap or context injection
- **New context only**: no supported delivery path

The classification must come from server capabilities and the current session state, not from a Desktop-maintained provider-name list.

### 12.2 Switch confirmation

Show:

- source and target provider/model
- continuity class
- snapshot coverage through message/row and approximate token size
- whether the session summary is current, stale, or absent
- active tools, pending approvals, or interrupted commands
- redaction/truncation counts
- whether native resume, bootstrap, or context injection will be used

Require explicit confirmation for `New context only` and for cancellation of active work.

### 12.3 Transition UI

- Use a single transition request rather than provider/model/effort calls.
- Show `Preparing checkpoint`, `Initializing target`, `Delivering handoff`, and `Activated` states.
- Disable duplicate switch submissions while a transition lock is held.
- On failure, show that the previous provider remains active or that rollback completed.
- Offer `View checkpoint`, `Retry target`, and `Return to previous provider` where supported.

### 12.4 Error card

For eligible provider errors, add the checkpoint continuation action alongside retry. The action opens a target picker filtered or annotated by continuity capability.

## 13. Project memory separation

- Keep session snapshots exclusively in Gosling state storage by default.
- Treat `AGENTS.md` and `CLAUDE.md` as optional user-controlled projections of curated project knowledge.
- Do not append session objectives, errors, commands, or transient next actions to repository instruction files.
- Gate any automatic durable-file projection behind explicit configuration and show the target path before enabling it.
- Keep `memories.jsonl` retrieval independent from handoff correctness.
- Document that memory recall enriches a session but does not establish checkpoint coverage or delivery acknowledgement.

## 14. Implementation phases

### Phase 1: Snapshot foundation

- Add snapshot types, schema versioning, SQLite migration, storage APIs, retention, and source hashing.
- Build the deterministic snapshot extractor and redaction pipeline.
- Reuse current session summaries/facts as optional inputs.
- Add unit and migration tests.

Exit criteria:

- A snapshot can be produced after a simulated provider transport failure with no provider call.
- Persisted snapshots are bounded, redacted, versioned, and coverage-addressable.

### Phase 2: Provider capabilities

- Add `ContextOwnership` and `ProviderCapabilities`.
- Implement capabilities for all built-in providers and custom/declarative defaults.
- Migrate context-manager, compaction, CLI, and ACP decisions away from direct boolean checks.
- Expose capabilities to clients.

Exit criteria:

- Every registered provider produces a deterministic continuity classification.
- No switch behavior depends on provider-name suffixes.

### Phase 3: Atomic transitions

- Add the transition request, coordinator, per-session lock, candidate-provider lifecycle, database transaction, and rollback record.
- Route server and ACP provider/model/effort updates through the coordinator.
- Preserve the previous provider until activation succeeds.

Exit criteria:

- Failure injection at each transition stage leaves provider metadata and in-memory provider consistent.
- Duplicate or stale switch requests cannot create multiple active generations.

### Phase 4: Delivery adapters

- Move ACP handoff construction into core and enforce the snapshot budget.
- Add bootstrap delivery to `claude-code`.
- Add Gosling-managed injection and native resume/import hooks.
- Add acknowledgement recording and first-response confirmation behavior.

Exit criteria:

- Native-to-native, native-to-ACP, ACP-to-native, and provider-managed transitions preserve the checkpoint without raw-history replay.

### Phase 5: Desktop continuity UX

- Replace sequential provider/model/effort operations.
- Add continuity badges, confirmation details, progress states, checkpoint inspection, and rollback messaging.
- Add the provider-error continuation action.

Exit criteria:

- Users can predict continuity before switching and can see whether activation or rollback succeeded.

### Phase 6: Memory policy and documentation

- Separate durable project-memory projections from session handoff documentation and settings.
- Document retention, privacy, redaction, provider capability semantics, and the immediate ACP workaround.
- Deprecate `claude-code` in switching recommendations while retaining compatibility.

### Phase 7: Rollout

- Introduce the snapshot builder in telemetry/shadow mode first.
- Compare predicted snapshot coverage with current ACP handoff behavior.
- Enable bounded ACP delivery, then atomic transitions, then Desktop recovery UX.
- Keep a feature flag for rollback during the first release cycle.

## 15. Test and evaluation matrix

### 15.1 Storage and migration

- fresh database creates snapshot tables and indexes
- upgrade from the previous schema preserves sessions and summaries
- cascade delete and retention behavior
- unknown future schema versions fail safely
- corrupted snapshot JSON is isolated and reported

### 15.2 Snapshot construction

- no summary, current summary, stale summary, and failed summary
- short and multi-million-token sessions
- errors immediately after a user message and after partial assistant output
- completed, failed, cancelled, and still-running commands
- file edits, deletions, binary attachments, and large tool output
- pending permission requests and queued user steers
- deterministic output and stable source hash

### 15.3 Redaction

- credentials in tool arguments, output, URLs, headers, environment maps, diffs, and errors
- known secret values and token-like unknown values
- no raw image/binary payloads
- delivery and persisted JSON contain identical redacted content

### 15.4 Provider transitions

- API to API
- API to ACP
- ACP to API
- ACP to ACP
- API/ACP to `claude-code`
- same-provider in-place model change
- same-provider recreation when in-place switching is unsupported
- native resume success and native resume fallback to bootstrap
- new-context-only confirmation

### 15.5 Failure injection

- snapshot persistence failure
- target provider creation/authentication failure
- mode/model/thinking-effort configuration failure
- bootstrap send failure and acknowledgement timeout
- database commit failure
- in-memory swap or notification failure
- cancellation during preparation and activation
- target failure on the first post-activation turn

For every failure, assert that the old provider remains usable or rollback is explicit and complete.

### 15.6 Behavioral safety

- prior tool calls are not executed twice
- interrupted commands are not reported as completed
- approvals do not transfer as approvals for a new provider
- the latest user request appears exactly once
- target acknowledgement precedes new tool work
- snapshot content cannot override system policy or current permission mode

### 15.7 Desktop

- continuity classification rendering
- checkpoint preview and truncation/redaction counts
- active-operation warning and confirmation
- transition progress and duplicate-submit prevention
- provider-error continuation flow
- rollback and retry states

## 16. Primary implementation touchpoints

Core and persistence:

- `crates/gosling/src/session/session_manager.rs`
- new `crates/gosling/src/session/handoff.rs`
- `crates/gosling/src/agents/agent.rs`
- new `crates/gosling/src/agents/provider_transition.rs`
- `crates/gosling/src/context_mgmt/`

Provider contract and adapters:

- `crates/gosling-providers/src/base.rs`
- `crates/gosling/src/acp/provider.rs`
- `crates/gosling/src/providers/claude_code.rs`
- other providers currently overriding `manages_own_context()`

Server and ACP surface:

- `crates/gosling/src/acp/server.rs`
- `crates/gosling/src/acp/server/manage_sessions.rs`
- `crates/gosling-server/src/routes/agent.rs`
- `crates/gosling-server/src/routes/errors.rs`

Desktop:

- `ui/desktop/src/acp/providers.ts`
- `ui/desktop/src/components/ModelAndProviderContext.tsx`
- `ui/desktop/src/components/settings/models/subcomponents/SwitchModelModal.tsx`
- `ui/desktop/src/components/BaseChat.tsx`
- session/provider types and ACP SDK definitions

## 17. Acceptance criteria

- Switching from a failed provider does not require a successful outgoing-model call.
- The target receives a bounded, redacted checkpoint with objective, latest intent, observed work, errors, unresolved state, next actions, and recent tail.
- The same current user request is never delivered twice.
- No previous tool operation is silently resumed or repeated.
- Provider, model, and thinking effort change through one transition request and one committed generation.
- A failed activation leaves the old provider and stored configuration active, or performs a recorded complete rollback.
- Desktop accurately labels continuity before the switch and reports checkpoint coverage afterward.
- ACP no longer sends an unbounded prior transcript.
- `claude-code` receives the same core bootstrap envelope as ACP provider-managed agents.
- Repository memory files are not modified by session switching.
- Cross-provider, failure, compaction, cancellation, tool, approval, migration, and redaction tests pass.

## 18. Risks and decisions to confirm during implementation

- Whether bootstrap acknowledgement should consume a separate model call or be combined with the first user turn.
- The target-context percentage and absolute handoff token cap after evaluation on 128K, 200K, and 1M models.
- Whether snapshot storage needs application-level encryption in addition to existing local session protections.
- Which native providers can genuinely import history rather than merely resume their own prior session ID.
- How long rollback providers and provider subprocesses should remain alive after activation.
- Whether active read-only tools may finish during snapshot preparation or all active operations must block switching.
- How custom providers declare capabilities safely without claiming unsupported continuity.

Default choices for the first implementation should favor bounded bootstrap delivery, explicit unknown state, blocked switching during active operations, and rollback safety over maximum automation.

## 19. Immediate operational workaround

Until this design ships:

- Prefer `claude-acp` over deprecated `claude-code` when continuing an existing Gosling session.
- Treat the current ACP raw-history handoff as best-effort; very long sessions may still be costly or exceed useful context.
- Avoid switching while a tool call or approval is active.
- After switching, ask the target to restate the objective, current state, and next action before it performs new work.

# Part II: Security Scan Remediation Program

## 20. Plan authority and planning profile

This section incorporates the prior Session Handoff plan rather than replacing it. The continuity feature remains Part I and the security-remediation program is Part II. The two workstreams share the same evidence, change-control, and continuation practices, but they have independent implementation gates.

The planning structure follows the existing-repository, or "Giles," profile from:

- `/Users/eric/Work/vscode/agent-skills/030_plan/plan-prototype-build/SKILL.md`
- `references/evidence-and-traceability.md`
- `references/continuation-and-handoff.md`
- `references/target-repo-standard.md`
- `references/design-for-change.md`
- `references/testing-standard.md`
- `templates/gated_execution_plan.md`
- `templates/traceability_matrix.md`
- `templates/risk_register.md`
- `templates/plan_change_record.md`

The repository's current structure, commands, and conventions are authoritative. This plan does not create a second project structure, alternate build system, parallel dependency manifest, or separate issue database.

The normal prototype-plan source-file guideline is 800 lines. The user explicitly approved a longer `plan.md`, so this master plan intentionally keeps the related continuity and security work in one durable handoff artifact.

## 21. Security objective

Reduce the repository's highest-risk supply-chain and workflow-integrity exposures first, while preserving Gosling behavior and avoiding disruptive local operations.

The implementation order is fixed unless a plan-change record is added:

1. P0 integrity issues: alerts `#53`, `#58`, `#54`, and `#57`.
2. CI permission reductions: alerts `#29`, `#4`, `#6`, `#26`, `#9`, and `#14`.
3. Container digest pinning.
4. Dependency mapping investigation for alert `#65` before any dependency change.
5. Branch-protection and review findings as a separate GitHub-administration workstream.

## 22. Security scope boundaries

### 22.1 In scope for Phase 1

- Replace the JBang network-to-shell installer with an immutable release download and checksum verification.
- Replace the manylinux rustup network-to-shell installer with an immutable archived `rustup-init` binary and architecture-specific checksum verification.
- Replace unpinned, global provider CLI installation in smoke tests with exact, lockfile-managed workspace development dependencies.
- Replace the docs workflow's mutable remote Gosling installer with a CLI built from the already checked-out source and locked Rust dependency graph.
- Add a static regression check covering all four P0 contracts.
- Run syntax, manifest, lockfile, and static security checks only.

### 22.2 Explicitly out of scope for Phase 1

- Building Gosling locally.
- Running `cargo build`, `cargo test`, `cargo clippy`, Electron packaging, or Desktop tests.
- Starting, stopping, rebuilding, restarting, signaling, or otherwise interacting with a running Gosling process.
- Changing repository, organization, branch-protection, review, ruleset, token, secret, variable, or environment settings on GitHub.
- Closing, dismissing, or modifying GitHub code-scanning alerts.
- Changing workflow permissions; those belong to Phase 2.
- Performing the broader container-digest program; that belongs to Phase 3.
- Changing dependencies in response to alert `#65`; Phase 4 begins with dependency mapping only.
- Fixing adjacent findings that are not required to preserve a Phase 1 invariant.
- Committing, pushing, opening a pull request, or publishing artifacts.

## 23. Security requirements

### 23.1 Phase 1 P0 integrity requirements

#### SEC-REQ-001 — JBang installer integrity (`#53`)

The Desktop JBang shim must not execute a script obtained from `sh.jbang.dev` or any other mutable network endpoint.

Acceptance criteria:

- The JBang release version is exact.
- The release URL contains the exact Git tag and archive name.
- The archive is verified against a repository-pinned SHA-256 digest before extraction.
- Checksum mismatch aborts installation.
- The downloaded archive is not executed as shell code.
- JBang remains available to the wrapper without modifying the user's shell profile.
- Existing registry selection, trust configuration, `--fresh`, `--quiet`, and argument forwarding behavior remain unchanged.

#### SEC-REQ-002 — Provider package integrity (`#58`)

Provider smoke tests must not globally install floating npm packages immediately before running tests with credentials.

Acceptance criteria:

- Claude Code, Claude ACP, and Codex ACP packages are exact-version development dependencies of `ui/desktop`.
- The exact packages resolve through `ui/pnpm-lock.yaml` with registry integrity metadata.
- The workflow uses the existing frozen-lockfile install.
- The workflow contains no `npm install -g` provider step.
- Provider executable names remain `claude`, `claude-agent-acp`, and `codex-acp` on the `pnpm run` PATH.
- Deprecated `@zed-industries` packages are not newly introduced.

#### SEC-REQ-003 — rustup bootstrap integrity (`#54`)

The manylinux build must not pipe the mutable rustup bootstrap script into a shell.

Acceptance criteria:

- The rustup version is exact.
- The archived rustup URL contains the exact version and target triple.
- Each supported manylinux architecture selects an explicit SHA-256 digest.
- Unknown architectures fail closed.
- The downloaded binary is verified before it is made executable or run.
- The repository's `rust-toolchain.toml` remains the Rust toolchain-channel authority.
- The manylinux build behavior after rustup installation remains unchanged.

#### SEC-REQ-004 — docs CLI provenance (`#57`)

The documentation workflow must not install Gosling by piping a mutable release script into a shell.

Acceptance criteria:

- The workflow builds `gosling-cli` from the exact checked-out source.
- Cargo uses the committed lockfile.
- The resulting binary is copied into the existing local binary directory.
- The existing PATH publication and `gosling --version` smoke check remain.
- No release-tag alias, mutable installer, or network-delivered shell script is used.

#### SEC-REQ-005 — P0 regression guard

The repository must contain and pass a narrow static check that fails if any of the four prohibited patterns or required immutable pins regress.

Acceptance criteria:

- The check is deterministic and network-free.
- The check does not build or launch Gosling.
- Each P0 alert has at least one positive invariant and one forbidden-pattern assertion.
- Failure output names the violated contract.

### 23.2 Phase 2 CI-permission requirements

#### SEC-REQ-006 — Inventory permissions (`#29`, `#4`, `#6`, `#26`, `#9`, `#14`)

For each affected workflow, record triggers, jobs, steps, GitHub API mutations, artifact operations, fork behavior, and current workflow/job permissions before editing.

#### SEC-REQ-007 — Least-privilege separation

Read/test jobs should receive read-only permissions. Write permissions should exist only on the smallest job that performs a verified write and should be guarded by the narrowest applicable event and repository conditions.

#### SEC-REQ-008 — Untrusted input isolation

Workflows with elevated permissions or secrets must not execute untrusted pull-request code. Any intentional split between analysis and privileged mutation must pass only bounded, validated artifacts across the trust boundary.

#### SEC-REQ-009 — Independent permission delivery

Permission fixes must be reviewed and validated separately from P0 installer-integrity changes so that permission semantics and supply-chain semantics can be evaluated independently.

### 23.3 Phase 3 container requirements

#### SEC-REQ-010 — Digest-pinned containers

Every in-scope workflow container and Docker base image identified by the scan must use an immutable digest while retaining a human-readable tag where supported.

#### SEC-REQ-011 — Digest provenance and update method

Each digest must be resolved for the correct registry, platform, and manifest type. The plan must document how maintainers refresh digests without reverting to floating tags.

### 23.4 Phase 4 dependency-mapping requirements

#### SEC-REQ-012 — Alert `#65` reachability map

Before changing a dependency, map the scanner advisory to the manifest entry, lockfile package, direct or transitive parent, enabled feature or runtime import, shipping artifact, and reachable code path.

#### SEC-REQ-013 — Minimal dependency response

Only after the mapping is complete may a dependency update, feature change, package override, removal, or documented no-change disposition be proposed. The smallest behavior-preserving response is preferred.

### 23.5 Phase 5 GitHub-governance requirements

#### SEC-REQ-014 — Settings-only recommendations

Branch-protection and review alerts must be handled as an explicit administrator runbook. Code changes must not pretend to satisfy repository-settings findings.

#### SEC-REQ-015 — Approval and readback

Any future GitHub settings change requires explicit user authorization, an exact preview, the least-privilege administrator operation, and a readback confirming the resulting rules.

## 24. Phase 1 threat and invariant model

### 24.1 Protected assets

- CI secrets supplied to provider integration tests.
- Release signing and artifact provenance.
- Developer workstations that invoke the JBang shim.
- GitHub-hosted runner tokens and repository write capability.
- The integrity of generated CLI reference documentation.
- The contents of the repository and built Gosling binaries.

### 24.2 Trust boundaries

- GitHub and npm release infrastructure are distribution systems, not authorities to execute the latest content without verification.
- Repository-reviewed source and committed lockfiles are trusted inputs for the current revision.
- Checksums committed in reviewed source establish the expected immutable artifact identity.
- CI secrets cross into provider processes only after installation has been resolved from the reviewed lockfile.
- The checked-out repository is the provenance boundary for the docs CLI.

### 24.3 Vulnerable path: alert `#53`

Source: mutable `https://sh.jbang.dev` response.  
Transform: `curl -Ls` streams bytes.  
Sink: `bash -s - app setup` executes those bytes on a developer machine.  
Consequence: endpoint, DNS, TLS termination, hosting, or release-promotion compromise can become arbitrary code execution.

Security invariant: network content may be unpacked only after identity verification against the reviewed digest; it may not be interpreted as shell.

Preserved behavior: a compatible JBang command is present for the wrapper, uses Java 17, respects Gosling's config root, and receives the original arguments.

### 24.4 Vulnerable path: alert `#58`

Source: floating npm package metadata for three provider packages.  
Transform: global npm resolution downloads current package versions and lifecycle content.  
Sink: package installation and subsequent provider execution in a job with numerous API credentials.  
Consequence: a compromised or unexpectedly changed latest release can execute with provider secrets.

Security invariant: provider executables used by credential-bearing tests are exact packages selected by the reviewed manifest and verified lockfile.

Preserved behavior: smoke tests can resolve all three provider binary names from the Desktop workspace.

### 24.5 Vulnerable path: alert `#54`

Source: mutable `https://sh.rustup.rs` response.  
Transform: `curl` streams bytes.  
Sink: shell executes the installer inside the release build container.  
Consequence: bootstrap compromise can alter binaries or exfiltrate the job's accessible state.

Security invariant: a target-specific archived rustup executable is run only after its digest matches the reviewed value.

Preserved behavior: rustup installs the channel from `rust-toolchain.toml` and the target selected by the workflow matrix.

### 24.6 Vulnerable path: alert `#57`

Source: mutable `stable/download_cli.sh` release alias.  
Transform: `curl` streams the script.  
Sink: shell runs it in a workflow with write permissions and an Anthropic credential later used by the job.  
Consequence: compromised release content can execute in a privileged automation context.

Security invariant: the CLI used for documentation generation is built from the reviewed checkout and committed dependency lock.

Preserved behavior: a `gosling` executable is placed in `/home/runner/.local/bin`, added to later-step PATH, and version-checked before use.

## 25. Phase 1 implementation contracts

### 25.1 Alert `#53` patch contract

File: `ui/desktop/src/bin/jbang`

Planned change:

1. Declare `JBANG_VERSION=0.139.3`.
2. Declare the SHA-256 digest for the official `jbang-0.139.3.zip` release asset.
3. Install under a versioned Gosling-managed directory inside `mcp-hermit`.
4. Download from the exact `v0.139.3` GitHub release URL with curl failure handling.
5. Verify with `sha256sum` or macOS `shasum -a 256`.
6. Extract with the already-installed Java 17 `jar` command.
7. Validate the expected `jbang-0.139.3/bin/jbang` path before activation.
8. Put the versioned JBang `bin` directory on PATH for the wrapper process.
9. Leave registry, trust, and final invocation behavior unchanged.

Rollback unit: revert only the JBang installation block and its constants. Existing Gosling-managed JBang directories can be ignored or removed after rollback; no user shell file is changed.

### 25.2 Alert `#58` patch contract

Files:

- `.github/workflows/pr-smoke-test.yml`
- `ui/package.json`
- `ui/pnpm-lock.yaml`

Planned change:

1. Add exact development dependencies:
   - `@anthropic-ai/claude-code@2.1.206`
   - `@agentclientprotocol/claude-agent-acp@0.58.1`
   - `@agentclientprotocol/codex-acp@1.1.2`
2. Keep CI-only provider tools at the existing `ui` workspace root rather than merging their dependency graph into the Desktop application's direct dependencies.
3. Add exact `@modelcontextprotocol/sdk@1.29.0` and `zod@4.4.3` workspace-root development dependencies to satisfy the current Claude Agent SDK peer contract.
4. Scope the pnpm override `@agentclientprotocol/claude-agent-acp>zod` to `4.4.3` so pnpm does not select Desktop's compatible-but-too-old Zod 3 instance for the provider tool.
5. Generate lockfile entries with pnpm so integrity metadata is retained.
6. Remove the global, floating npm installation step.
7. Continue using `pnpm install --frozen-lockfile` and the existing `pnpm run` test command. Pnpm exposes workspace-root binaries to workspace scripts.

Compatibility note: the replacement ACP packages retain the executable names expected by Gosling. The prior `@zed-industries` packages are deprecated in favor of the `@agentclientprotocol` namespace.

Rollback unit: restore the removed workflow step and remove the five exact workspace-root dev dependencies, the scoped Zod override, and their lockfile entries. This rollback is mechanically possible but would reintroduce the finding and should be used only to diagnose compatibility in an isolated, credential-free job.

### 25.3 Alert `#54` patch contract

File: `.github/workflows/build-cli.yml`

Planned change:

1. Pin rustup to `1.29.0`.
2. Select one of these official archive digests:
   - `x86_64-unknown-linux-gnu`: `4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10`
   - `aarch64-unknown-linux-gnu`: `9732d6c5e2a098d3521fca8145d826ae0aaa067ef2385ead08e6feac88fa5792`
3. Fail on any unsupported architecture.
4. Download the exact archive binary to a temporary path.
5. Verify it with `sha256sum -c` before `chmod` and execution.
6. Preserve `--default-toolchain none`, `--profile minimal`, and `--no-modify-path`.
7. Continue installing Rust channel `1.92` from `rust-toolchain.toml`.

Rollback unit: revert the manylinux bootstrap block only. Container digest pins and host-runner Hermit behavior are independent.

### 25.4 Alert `#57` patch contract

File: `.github/workflows/docs-update-cli-ref.yml`

Planned change:

1. Use the already-configured Rust toolchain.
2. Replace the mutable `stable` compiler selector with repository channel `1.92` while retaining the commit-pinned setup action.
3. Run `cargo build --locked -p gosling-cli --bin gosling` from the repository checkout.
4. Install `target/debug/gosling` into `/home/runner/.local/bin/gosling` with executable mode.
5. Publish the same directory through `GITHUB_PATH`.
6. Retain `gosling --version` as a local smoke check.

Tradeoff: the docs workflow spends additional runner time compiling, but its CLI now corresponds exactly to the checked-out revision and does not depend on a mutable or missing `stable` release alias.

Rollback unit: revert the single workflow step. No release assets or repository settings are changed.

### 25.5 Regression-check contract

File: `.github/scripts/verify-phase1-integrity.sh`

The script will:

- reject `sh.jbang.dev` in the JBang shim;
- require the exact JBang version, digest, and tagged URL;
- reject provider `npm install -g` in the smoke workflow;
- require exact current provider dependencies in `package.json`;
- require lockfile resolution of all three packages;
- reject `sh.rustup.rs` in the CLI build workflow;
- require the rustup version, both architecture digests, and archived URL form;
- reject the remote `stable/download_cli.sh` installer in the docs workflow;
- require locked local CLI compilation and local installation.

## 26. Gated execution plan

### Gate 0 — Preserve and constrain

Status: complete.

Evidence required:

- Read repository `AGENTS.md`.
- Record the user's non-build, non-restart, and non-settings constraints.
- Inspect `git status` and preserve unrelated changes.
- Confirm no commit or push authority was requested.

Exit criteria:

- Only the existing untracked `plan.md` was present before Phase 1 edits.
- No running process is queried, signaled, or changed.

### Gate 1 — Revalidate findings and sources

Status: complete.

Evidence required:

- Inspect the four exact source-to-sink paths.
- Resolve immutable release identities from primary upstream sources.
- Verify replacement package names and executable compatibility.
- Confirm the docs workflow already has a Rust setup and repository checkout.

Exit criteria:

- Each alert has a documented vulnerable path, protected invariant, preserved behavior, and narrow strategy.

### Gate 2 — Plan and review patch contracts

Status: complete.

Evidence required:

- Stable requirement IDs.
- File-level patch contracts.
- Traceability and risk entries.
- Explicit rollback units.
- Check commands selected before edits.

Exit criteria:

- Phase 1 changes can be evaluated without inferring intent from the patch.

### Gate 3 — Implement Phase 1

Status: complete.

Ordered work:

1. Add the security regression guard so contracts are executable.
2. Patch JBang installation.
3. Patch manylinux rustup bootstrap.
4. Patch docs CLI installation.
5. Add provider dependencies through pnpm and remove global installation.
6. Inspect the generated lockfile diff for unrelated churn.

Exit criteria:

- Only planned files changed.
- No mutable network response is piped to a shell in the four alert paths.
- Dependency changes are exact and lockfile-backed.

### Gate 4 — Targeted verification

Status: complete within the user-authorized non-build boundary.

Required checks:

1. `bash -n ui/desktop/src/bin/jbang`
2. `bash -n .github/scripts/verify-phase1-integrity.sh`
3. `.github/scripts/verify-phase1-integrity.sh`
4. Parse both changed workflow YAML files and the smoke workflow with a local YAML parser.
5. Parse `ui/package.json` as JSON.
6. Validate the pnpm lockfile under frozen-lockfile, lockfile-only mode if supported.
7. Run `shellcheck` on changed shell scripts if available.
8. Run `git diff --check`.
9. Search the four fixed paths for prohibited remote-to-shell and global-install patterns.
10. Inspect `git diff --stat`, `git diff --name-only`, and the complete diff.

Prohibited checks:

- No Gosling build.
- No Rust, JavaScript, integration, or end-to-end test suite.
- No Desktop launch.
- No workflow dispatch.

Exit criteria:

- Every required available check passes.
- Any unavailable check is recorded with a reason and compensating evidence.
- Each fixed alert has a regression assertion.

### Gate 5 — Hostile review and handoff

Status: complete.

Review questions:

- Can mutable content still reach an interpreter before verification?
- Can a checksum branch be bypassed by an unsupported platform?
- Did package pinning accidentally retain global resolution elsewhere in the same job?
- Does the workspace PATH still expose the expected provider binary names?
- Does the docs workflow build the same checked-out revision it later analyzes?
- Did any edit expand permissions, expose secrets, or modify runtime behavior beyond the finding?
- Did generated lockfile churn exceed the three requested packages and required peer resolutions?

Exit criteria:

- Findings have an honest `fixed`, `no_change`, or `blocked` disposition.
- Residual adjacent risks are named without being silently expanded into Phase 1.
- `plan.md` records actual commands, evidence, remaining work, and the next safe action.

## 27. Phase 2 detailed plan: CI permission reductions

Phase 2 is intentionally a separate implementation unit.

### 27.1 Inventory each alert

For alerts `#29`, `#4`, `#6`, `#26`, `#9`, and `#14`:

1. Re-open the alert and record the exact workflow and line.
2. Enumerate all triggers, including `pull_request`, `pull_request_target`, `workflow_run`, `workflow_call`, schedules, manual dispatch, pushes, and releases.
3. Record top-level and job-level permissions.
4. Record every GitHub API, `gh`, release, issue, pull-request, contents, checks, actions, package, deployment, OIDC, or security-event operation.
5. Classify every checkout as trusted or untrusted.
6. Record whether secrets are available at the point untrusted code could run.
7. Propose the smallest permission set per job.
8. Separate mutation into a guarded job when a read/test job does not need write access.

### 27.2 Permission design rules

- Default to `contents: read` or `permissions: {}` at workflow scope.
- Add job permissions individually.
- Do not grant `pull-requests: write` to code-execution jobs.
- Do not grant `contents: write` to jobs that only upload workflow artifacts.
- Treat `id-token: write` as a separate high-risk capability.
- Ensure forked pull requests cannot route attacker-controlled content into a privileged interpreter.
- Pin any new action by full commit SHA.
- Do not change GitHub settings during code delivery.

### 27.3 Phase 2 validation

- YAML parse every changed workflow.
- Run a static permission-contract script or actionlint if available.
- Map every requested permission to a specific step.
- Confirm untrusted checkouts occur only after privileges have been reduced.
- Review event expressions for fork and actor confusion.
- Use dry-run or read-only GitHub inspection only unless separately authorized.

### 27.4 Phase 2 revalidation evidence

Read-only GitHub code-scanning API inspection on 2026-07-10 confirmed all six findings remain open on `main` at commit `9f661a661a2b3451dfca87a5953890f3da6677c1`:

| Alert | Workflow | Scanner evidence | Current exposure |
|---:|---|---|---|
| #29 | `release.yml` | top-level `contents: write` | build and packaging jobs inherit release, OIDC, attestation, and PR write capabilities they do not need |
| #4 | `build-cli.yml` | no top-level permission defined | reusable build permissions depend on caller/repository defaults |
| #6 | `bundle-desktop-linux.yml` | no top-level permission defined | checkout/build/package job permissions depend on caller/repository defaults |
| #26 | `publish-npm.yml` | top-level `contents: write` | build jobs inherit contents/PR/OIDC writes needed by neither the build nor package assembly |
| #9 | `canary.yml` | top-level `contents: write` | prepare and build jobs inherit release and attestation capabilities |
| #14 | `close-release-pr-on-tag.yaml` | top-level `contents: write` | PR lookup/closure and workflow dispatch share contents, PR, and Actions writes for the whole job |

GitHub's current reusable-workflow contract allows the caller to pass `jobs.<job_id>.permissions`, and the called workflow may only retain or downgrade those permissions. Therefore, hardening `build-cli.yml` also requires a `contents: read` permission on the `build-cli` call in `pr-comment-build-cli.yml`; otherwise the existing IssueOps caller may be unable to check out the selected PR commit.

### 27.5 Phase 2 patch contract

#### Alert #29 — release orchestration

Vulnerable path: release-wide write token → build/repository checkout and packaging jobs → arbitrary action or repository build step can access release-class capabilities before the release job.

Security invariant: only the final release job may write repository contents or attestations; only Windows signing and final attestation may request OIDC; build and artifact jobs receive read-only repository access.

Preserved behavior:

- release branches and `v1.*` tags still trigger the workflow;
- every reusable build/bundle job still receives the permissions it needs;
- tagged releases still create/update versioned and stable releases;
- provenance attestation and optional macOS update-manifest upload remain enabled.

Patch:

- set workflow permission to `contents: read`;
- set `contents: read` explicitly on CLI build, install-script, and Linux bundle call jobs;
- retain OIDC only on the Windows signing call; macOS signing uses environment-protected Apple credentials and does not access GitHub OIDC;
- remove unused `actions: read` from the Windows caller because same-run artifact operations use the Actions runtime and no GitHub Actions API call appears in the workflow;
- change `bundle-desktop-windows.yml` from workflow-wide OIDC to `contents: read`, then grant `id-token: write` only to `sign-desktop-windows` so caller-provided OIDC cannot reach repository build steps;
- retain `contents: write`, `id-token: write`, and `attestations: write` only on the release job;
- remove unused workflow-wide `pull-requests: write` and `actions: read` grants.

Rollback boundary: restore only `release.yml` permissions. No trigger, secret, action version, artifact name, or release command changes.

#### Alert #4 — reusable CLI build

Vulnerable path: absent workflow permission contract → caller/repository defaults → every matrix build receives permissions unrelated to checkout and artifact creation.

Security invariant: the reusable CLI build receives only `contents: read`.

Preserved behavior:

- all matrix targets still check out, build, package, cache, and upload artifacts;
- caller-selected `ref` and version inputs remain unchanged;
- IssueOps PR builds retain checkout access to the selected commit.

Patch:

- set `contents: read` at the reusable workflow level;
- set `contents: read` on each in-scope call job;
- add `contents: read` to the IssueOps `build-cli` call as the required compatibility adjustment.

Rollback boundary: remove the reusable-workflow permission block and the caller permission entries together.

#### Alert #6 — reusable Linux Desktop bundle

Vulnerable path: absent workflow permission contract → caller/repository defaults → checkout and build steps may receive caller release permissions.

Security invariant: Linux Desktop build/package steps receive only `contents: read`.

Preserved behavior: manual and reusable invocations still check out the selected branch/ref, build all Linux formats, use caches, and upload workflow artifacts.

Patch: set workflow-level `contents: read` and pass `contents: read` from release/canary call jobs.

Rollback boundary: remove the called-workflow permission block and the two caller entries together.

#### Alert #26 — npm publishing

Vulnerable path: workflow-wide contents/PR/OIDC writes → source checkout, dependency installation, package build, and artifact assembly → build-time code can access publish-class permissions.

Security invariant: build jobs have only repository read access; only the environment-gated publish job can mint an OIDC token.

Preserved behavior:

- CLI artifacts are still built and assembled into platform packages;
- SDK compatibility checks and package artifact upload remain;
- npm trusted publishing and provenance remain enabled;
- the production publishing environment remains the approval/secret boundary.

Patch:

- set workflow-level `contents: read`;
- pass `contents: read` to the reusable CLI build;
- retain read-only access for the build job;
- grant the publish job only `contents: read` and `id-token: write`;
- remove unused `contents: write` and `pull-requests: write`.

Rollback boundary: restore only `publish-npm.yml` permissions. Package content, commands, and environment remain unchanged.

#### Alert #9 — canary orchestration

Vulnerable path: workflow-wide release/OIDC/attestation writes → version preparation and build/bundle jobs → build-time code can access canary release capabilities.

Security invariant: preparation and unsigned build jobs have only repository read access; only the final release job can write releases, attestations, or OIDC tokens.

Preserved behavior:

- pushes to `main` still compute the canary version;
- all platform artifacts are still built and uploaded;
- unsigned canary bundles remain unsigned;
- the final job still attests artifacts and updates the canary release.

Patch:

- set workflow-level `contents: read`;
- pass `contents: read` to CLI, Linux, macOS, Intel macOS, and Windows build calls;
- remove OIDC from the always-unsigned macOS call jobs;
- retain `contents: write`, `id-token: write`, and `attestations: write` only on the release job.

Rollback boundary: restore only `canary.yml` permissions. Version, build, signing input, artifact, and release semantics remain unchanged.

#### Alert #14 — close release PR and dispatch patch workflow

Vulnerable path: one job has Actions, contents, and PR write permissions → checkout plus shell and GitHub CLI steps → either mutation capability is available to every step.

Security invariant: PR closure receives only `pull-requests: write`; workflow dispatch receives only `actions: write`; repository contents remain read-only at workflow scope.

Preserved behavior:

- semantic version tags still identify the matching `release/<version>` branch;
- an open matching PR is closed with the same comment when found;
- the patch release workflow is dispatched after successful PR handling, including when no PR is open;
- a failure in PR handling still prevents dispatch, matching current step ordering.

Patch:

- set workflow-level `contents: read`;
- remove the unnecessary checkout;
- give the PR job only `pull-requests: write` and export the branch as a job output;
- create a dependent dispatch job with only `actions: write`;
- set `GH_REPO` explicitly because the jobs no longer rely on checkout for repository discovery.

Rollback boundary: recombine the two jobs, restore checkout, and restore the original permission block as one unit.

### 27.6 Phase 2 regression contract

Add `.github/scripts/verify-phase2-permissions.rb` to parse the workflow YAML and enforce:

- every alert workflow has an explicit read-only top-level permission contract;
- no workflow-wide write permission remains in those six workflows;
- every job-level write grant is on the allowlist established above;
- build/reusable-call jobs explicitly receive `contents: read` where caller compatibility requires it;
- `publish-npm` grants OIDC only to `publish`;
- release/canary grant contents and attestation writes only to `release`;
- close-release PR and Actions writes exist on separate jobs;
- the IssueOps CLI caller passes `contents: read` and no write permission to the reusable build.

The script must fail if a removed top-level write is restored, if a job gains an unapproved write, or if a required reusable-workflow read permission is removed.

### 27.7 Phase 2 execution status

Status: complete in source; remote alert closure awaits push and GitHub rescanning.

Files changed:

- `.github/workflows/release.yml`
- `.github/workflows/build-cli.yml`
- `.github/workflows/bundle-desktop-linux.yml`
- `.github/workflows/bundle-desktop-windows.yml`
- `.github/workflows/publish-npm.yml`
- `.github/workflows/canary.yml`
- `.github/workflows/close-release-pr-on-tag.yaml`
- `.github/workflows/pr-comment-build-cli.yml`
- `.github/scripts/verify-phase2-permissions.rb`
- `plan.md`

Permission result:

- All six alert workflows now have explicit workflow-level `contents: read` and no workflow-level write permission.
- Release and canary content/attestation writes exist only on their final release jobs.
- npm OIDC exists only on the environment-gated publish job.
- Windows OIDC exists only on `sign-desktop-windows`; repository build code receives `contents: read` only.
- macOS release/canary calls receive no unused OIDC permission.
- PR closure and patch-workflow dispatch have separate job tokens.
- The CLI IssueOps caller explicitly passes `contents: read` to the hardened reusable build.

### 27.8 Phase 2 validation evidence

Applicability and buildability:

- Read-only `gh api` calls confirmed alerts `#29`, `#4`, `#6`, `#26`, `#9`, and `#14` were open and mapped to the expected files before editing.
- Ruby parsed all eight changed workflow/caller YAML files successfully.
- `ruby -c .github/scripts/verify-phase2-permissions.rb`: passed.
- `actionlint v1.7.12` on all eight changed workflow/caller files: passed with no findings.
- The three run blocks in `close-release-pr-on-tag.yaml`, extracted from parsed YAML and passed to `bash -n`: passed.

Security closure:

- `.github/scripts/verify-phase2-permissions.rb`: passed the complete workflow/job allowlist.
- The checker confirmed no workflow-wide write in any alert workflow.
- The checker confirmed every job-level write is one of the exact approved release, attestation, OIDC, PR, or Actions mutations.
- A temporary negative mutation changed `release.yml` back to workflow-wide `contents: write`; the checker failed with the expected contract error.
- Caller search confirmed every current `build-cli.yml` caller passes `contents: read`.
- Caller/called-workflow tracing confirmed caller-provided Windows OIDC is downgraded away from build/package jobs and retained only by the signing job.

Preserved behavior:

- Workflow triggers, conditions, inputs, artifacts, actions, secrets, environments, build commands, and release commands did not change, except for the intentional two-job split in `close-release-pr-on-tag.yaml`.
- The close-release branch value is now an explicit job output and the dispatch job has the same success dependency as the original later step.
- From a directory without a checkout, `GH_REPO=repo-makeover/gosling gh pr list ...` succeeded, proving PR repository discovery no longer depends on Git state.
- From a directory without a checkout, `GH_REPO=repo-makeover/gosling gh workflow view release.yml --yaml` succeeded, proving registered workflow lookup honors `GH_REPO`.
- `git diff --check`: passed.

Skipped by explicit scope:

- No Gosling build or test suite.
- No Desktop launch, rebuild, or restart.
- No workflow dispatch or hosted release/publish execution.
- No GitHub settings or branch-protection mutation.
- No code-scanning alert dismissal or state mutation.

### 27.9 Phase 2 finding dispositions

- Alert `#29`: `fixed` in source. `release.yml` is read-only by default; final release and Windows signing retain narrowly scoped writes/OIDC.
- Alert `#4`: `fixed` in source. `build-cli.yml` declares `contents: read`, and all current callers preserve checkout access without broader permissions.
- Alert `#6`: `fixed` in source. `bundle-desktop-linux.yml` and its callers are read-only.
- Alert `#26`: `fixed` in source. npm build jobs are read-only; only the production publish job receives OIDC.
- Alert `#9`: `fixed` in source. canary preparation and builds are read-only; only the release job can write or attest.
- Alert `#14`: `fixed` in source. contents are no longer writable, and PR/Actions writes are isolated into separate jobs.

The alerts will remain open in GitHub until these source changes are pushed and the Scorecard code-scanning workflow evaluates them.

### 27.10 Residual operational finding

Read-only GitHub Actions API inspection returned `404` for `patch-release.yaml`, and that workflow is absent from the repository's registered Actions workflow list even though the file exists on `main` and passes `actionlint`. The original `gh workflow run patch-release.yaml` command therefore appears unable to dispatch in the current remote state.

This predates Phase 2 and is not caused by permission scoping. Phase 2 preserved the target and command at the time. On 2026-07-10, the repository owner explicitly retired `patch-release.yaml` together with its only dispatcher, `close-release-pr-on-tag.yaml`; the unregistered target is therefore no longer an operational dependency.

## 28. Phase 3 detailed plan: container digest pinning

### 28.1 Inventory

1. Search workflow `container`, service containers, Dockerfiles, Compose files, release scripts, and documentation examples used by automation.
2. Separate runtime images from test-only and documentation images.
3. Record tag, registry, platform set, current digest, upstream release cadence, and owning workflow.
4. Exclude values already pinned by digest after verifying digest format.

### 28.2 Resolution

1. Resolve the registry manifest for each image.
2. For multi-platform images, pin the manifest-list digest where the consumer selects platforms.
3. For architecture-specific jobs, verify the selected digest includes the expected architecture.
4. Retain the readable tag as `image:tag@sha256:digest` where syntax permits.
5. Record the source command and resolution date in plan evidence, not noisy source comments unless maintainers need the update instruction there.

### 28.3 Maintenance

- Prefer Dependabot or Renovate digest updates if already supported by repository policy.
- Never replace one floating tag with another floating tag.
- Review upstream changelogs before accepting digest refreshes.
- Validate container entrypoint and available tools after each update in CI, not by restarting local Gosling.

### 28.4 Phase 3 revalidation and scope

Read-only GitHub code-scanning API inspection on 2026-07-10 mapped the live container-image findings to:

| Alert | File | Current reference | Role |
|---:|---|---|---|
| #40 | `.devcontainer/Dockerfile` | `mcr.microsoft.com/devcontainers/rust:1` | developer environment |
| #41 | `Dockerfile` | `rust:1.82-bookworm` | Gosling CLI/container builder |
| #42 | `documentation/docs/docker/Dockerfile` | `rust:bullseye` | documented full-environment builder |
| #43 | `documentation/docs/docker/Dockerfile` | `ubuntu:22.04` | documented full-environment runtime |
| #44 | `services/ask-ai-bot/Dockerfile` | `oven/bun:1` | Ask AI Bot common base |
| #45 | `services/ask-ai-bot/Dockerfile` | internal `base` stage | dependency stage derived from mutable base |
| #46 | `services/ask-ai-bot/Dockerfile` | internal `base` stage | build stage derived from mutable base |
| #47 | `services/ask-ai-bot/Dockerfile` | internal `base` stage | production stage derived from mutable base |

The same inventory found two executable same-boundary references not reported by the scanner:

- `.github/workflows/test-finder.yml` executes `ghcr.io/repo-makeover/gosling:latest` as the entire credential-bearing job container.
- `ui/scripts/publish.sh` generates a temporary Dockerfile with `rust:1.92-bookworm` to build published Linux npm binaries.

These are included because leaving mutable images in executable automation would preserve the same source-to-execution path that Phase 3 is intended to close.

Already pinned and unchanged:

- root runtime `debian:bookworm-slim@sha256:b1a741...`;
- x86_64 and aarch64 manylinux workflow containers in `build-cli.yml`.

Inventory-only and out of implementation scope:

- illustrative image references in Markdown/tutorial content that are not consumed by automation;
- generated documentation build output;
- third-party source/cache contents under `.hermit`, `node_modules`, or `target`;
- `.devcontainer/devcontainer.json` feature reference, which is an OCI Dev Container Feature rather than a Docker base/job image and requires a separate compatibility/update contract;
- the mutable Dockerfile frontend syntax directive, which is not one of the current container-image findings.

### 28.5 Phase 3 immutable source evidence

Digests were resolved from the registries with `crane v0.21.7`. Each public digest was read back by digest and confirmed to be an OCI multi-platform index containing both `linux/amd64` and `linux/arm64`:

| Image tag | Manifest-list digest |
|---|---|
| `mcr.microsoft.com/devcontainers/rust:1` | `sha256:1707e2a8007968925f110c0961811200e9bb10e0ec055e2734857c59189a8b13` |
| `rust:1.82-bookworm` | `sha256:d9c3c6f1264a547d84560e06ffd79ed7a799ce0bff0980b26cf10d29af888377` |
| `rust:bullseye` | `sha256:9a11136145d74a2c7b2a74a36163fe9a58f392ef7eba15c2cb5b10e3ef13f361` |
| `ubuntu:22.04` | `sha256:0e0a0fc6d18feda9db1590da249ac93e8d5abfea8f4c3c0c849ce512b5ef8982` |
| `oven/bun:1` | `sha256:e10577f0db68676a7024391c6e5cb4b879ebd17188ab750cf10024a6d700e5c4` |
| `rust:1.92-bookworm` | `sha256:e90e846de4124376164ddfbaab4b0774c7bdeef5e738866295e5a90a34a307a2` |

The GHCR package is not anonymously readable and the current CLI credential lacks `read:packages`. Its immutable index digest was therefore taken from the latest completed successful `publish-docker.yml` build log:

- source commit/tag: `9f661a661a2b3451dfca87a5953890f3da6677c1` / `sha-9f661a6`;
- published `linux/amd64,linux/arm64` manifest list: `sha256:45c178cd40aceac2d3ea70bb99e0bcfaab584cdd758f7844dac9b0057f8e158c`;
- the build log records the same digest for `main`, `latest`, and `sha-9f661a6`.

Newer Docker publication runs for the Phase 1 and Phase 2 commits were still in progress during resolution. The test-finder pin intentionally selects the last successfully published immutable image rather than racing a mutable `latest` tag.

### 28.6 Phase 3 patch contract

Vulnerable path: mutable tag → registry-selected manifest at pull/build time → image entrypoint or build-stage commands execute with developer, CI, publishing, or provider credentials.

Security invariant: every in-scope executable container image resolves to a reviewed SHA-256 manifest-list digest before any image layer or command is used.

Preserved behavior:

- readable image tags remain beside digests for maintainability;
- all selected manifests retain amd64 and arm64 support;
- Rust, Debian/Ubuntu, Bun, and Dev Container tag families remain unchanged;
- root and documentation Dockerfile stage names and copy paths remain unchanged;
- Ask AI Bot dependency, build, and production stages retain `/app` as `WORKDIR` and the same copy/build/runtime commands;
- test-finder retains the same Gosling container family, root user, environment, secrets, and job steps;
- npm publishing retains Rust 1.92 Bookworm and the same target-platform build behavior.

Patch strategy:

1. Add `@sha256:<manifest-list>` after each external tag.
2. Replace Ask AI Bot's shared internal `FROM base` chain with three direct uses of the same pinned Bun image, repeating `WORKDIR /app` in each stage. This is required because the live scanner reports every `FROM base` occurrence and internal stage aliases cannot carry a registry digest.
3. Pin test-finder to the successful `sha-9f661a6` image tag and its published index digest.
4. Pin the generated npm-builder Dockerfile to the Rust 1.92 digest.
5. Add one weekly Dependabot Docker entry covering the four real Dockerfile directories so digest refreshes arrive as reviewable source changes rather than silent tag movement.
6. Do not change package installation, build commands, users, entrypoints, triggers, permissions, or credentials.

Rollback units:

- Each Dockerfile's digest changes can be reverted independently.
- Ask AI Bot's three direct stages must be reverted together with restoration of the shared base stage.
- The test-finder image can be rolled back independently to another known-good tag/digest pair; do not restore an unpinned `latest` reference.
- The generated npm builder can be rolled back independently to another verified Rust 1.92 manifest digest.

### 28.7 Phase 3 regression contract

Add `.github/scripts/verify-phase3-container-digests.rb` to enforce:

- exact expected tag/digest pairs for every in-scope Dockerfile `FROM`;
- no in-scope `FROM` uses an internal alias or lacks `@sha256:`;
- exact stage counts and names for the multi-stage Dockerfiles;
- the Ask AI Bot's three stages all use the same Bun digest and each declares `/app` as its work directory;
- the test-finder job container uses the exact immutable GHCR reference;
- the generated npm Dockerfile uses the exact Rust 1.92 reference;
- every digest contains exactly 64 lowercase hexadecimal characters;
- previously pinned root Debian and manylinux references remain present.
- Dependabot monitors `/`, `/.devcontainer`, `/documentation/docs/docker`, and `/services/ask-ai-bot` weekly for Docker updates.

The regression checker must reject a temporary mutation that removes any digest while accepting the legitimate multi-stage and multi-platform references.

### 28.8 Phase 3 execution status

Status: complete in source; remote alert closure awaits push and GitHub rescanning.

Files changed:

- `.devcontainer/Dockerfile`
- `Dockerfile`
- `documentation/docs/docker/Dockerfile`
- `services/ask-ai-bot/Dockerfile`
- `.github/workflows/test-finder.yml`
- `ui/scripts/publish.sh`
- `.github/dependabot.yml`
- `.github/scripts/verify-phase3-container-digests.rb`
- `plan.md`

Implementation result:

- All Dockerfile base images reported by alerts `#40–#47` now use exact manifest-list digests.
- Ask AI Bot has three direct pinned Bun stages with unchanged stage names, work directories, copy graph, commands, and runtime entrypoint.
- The test-finder job no longer executes mutable `latest`; it uses the last successfully published commit-tagged Gosling image and digest.
- The generated npm builder no longer resolves a mutable Rust 1.92 image.
- Existing root Debian and manylinux pins remain unchanged.
- Weekly Dependabot Docker monitoring covers every real Dockerfile directory.

### 28.9 Phase 3 validation evidence

Applicability and source resolution:

- Read-only code-scanning API inspection confirmed alerts `#40–#47` and their exact source lines.
- `crane v0.21.7 digest` readback matched all six public tag/digest pairs after editing.
- `crane manifest <tag>@<digest>` confirmed all six public references are OCI image indexes containing both `linux/amd64` and `linux/arm64`.
- Read-only log inspection of successful run `29090938028` confirmed GHCR commit tag `sha-9f661a6` was pushed as manifest list `sha256:45c178...`.

Syntax and contract checks:

- `ruby -c .github/scripts/verify-phase3-container-digests.rb`: passed.
- `.github/scripts/verify-phase3-container-digests.rb`: passed.
- Ruby YAML parsing for `.github/dependabot.yml` and `.github/workflows/test-finder.yml`: passed.
- `actionlint v1.7.12 .github/workflows/test-finder.yml`: passed.
- `bash -n ui/scripts/publish.sh`: passed.
- `git diff --check`: passed.

Security closure and bypass review:

- The regression guard found no in-scope unpinned `FROM`, internal stage alias, malformed digest, or mutable test-finder image.
- A temporary negative mutation removed the root Rust builder digest; the checker rejected it with the expected `FROM references` error.
- The Ask AI Bot stage graph was compared before and after: only base selection and explicit inherited `WORKDIR` declarations changed.
- Public tag digests were resolved a second time after editing and still matched the committed values.
- GHCR was not guessed from a mutable tag: the committed digest is tied to a successful multi-platform publication log and commit tag.

Skipped by explicit scope:

- No Gosling or container image build.
- No image layer pull or container execution.
- No Desktop launch, rebuild, or restart.
- No workflow dispatch, GitHub settings change, alert mutation, commit, or push.
- Hosted multi-platform Docker builds remain the final runtime compatibility evidence after delivery.

### 28.10 Phase 3 finding dispositions

- Alert `#40`: `fixed` in source; the Dev Container Rust base is digest-pinned.
- Alert `#41`: `fixed` in source; the root Rust builder is digest-pinned while the existing Debian runtime pin is preserved.
- Alerts `#42` and `#43`: `fixed` in source; both documentation Dockerfile stages are digest-pinned.
- Alerts `#44`, `#45`, `#46`, and `#47`: `fixed` in source; every Ask AI Bot `FROM` is now a direct pinned Bun reference.
- Unalerted `test-finder.yml` mutable job image: `fixed` in source.
- Unalerted generated npm-builder base: `fixed` in source.
- SEC-REQ-010 and SEC-REQ-011: complete in source with deterministic enforcement and a reviewable weekly update path.

GitHub alerts remain open until the changes are pushed and the Scorecard code-scanning workflow evaluates them.

### 28.11 Phase 3 residual findings

The following remain intentionally outside this container-image phase:

- PinnedDependencies `downloadThenRun` alerts `#48–#52`.
- PinnedDependencies npm-command alerts `#55`, `#56`, and `#59`.
- The Dev Container Feature tag in `.devcontainer/devcontainer.json`.
- Docker image references in copy/paste documentation examples.
- The Dockerfile frontend syntax tag.
- Alert `#65` dependency mapping, which is the next planned phase and must precede dependency changes.

## 29. Phase 4 detailed plan: alert `#65` dependency mapping

No dependency changes are authorized until these questions are answered:

1. Which ecosystem and package does the scanner identify?
2. Which manifest declares it, if any?
3. Which lockfile entry is affected?
4. Is it direct, transitive, optional, development-only, build-only, or runtime?
5. Which parent dependency introduces it?
6. Which features, imports, binaries, bundles, or target platforms make it reachable?
7. Is vulnerable behavior invoked with attacker-controlled input?
8. Does the affected package ship in CLI, Desktop, MCP, SDK, container, or CI-only artifacts?
9. Is a fixed version compatible with current Rust, Node, Electron, ACP, and platform constraints?
10. Would an override produce duplicate versions or violate the upstream package's tested range?

Required output before implementation:

- an advisory-to-lockfile mapping;
- a dependency path for every affected resolved version;
- reachability and artifact exposure classification;
- fixed-version options and compatibility risks;
- a recommended `update`, `override`, `feature-disable`, `remove`, or `no_change` disposition;
- targeted regression checks for the affected behavior.

## 30. Phase 5 detailed plan: branch protection and review settings

This workstream produces a runbook before any settings mutation.

The runbook must include:

- repository and default branch;
- current rulesets and legacy branch-protection state;
- required pull-request approvals;
- stale-review dismissal behavior;
- code-owner review requirement;
- required conversation resolution;
- administrator bypass policy;
- force-push and deletion policy;
- required status checks and strictness;
- expected impact on release automation, Dependabot, bots, and emergency maintenance;
- exact proposed settings changes;
- rollback settings;
- explicit owner approval;
- post-change API or UI readback.

No source-code patch can close these alerts. Their disposition remains separate until GitHub reports the desired administrative state.

### 30.1 Stage 1 solo-maintainer bridge execution

Status: complete on 2026-07-10.

- Repository ruleset `18782969`, `Protect default branch`, is active for `~DEFAULT_BRANCH` with no bypass actors.
- `main` is protected against deletion and non-fast-forward updates and requires changes through pull requests.
- Pull requests require resolved review threads but zero approving reviews, preserving solo-maintainer operation.
- Strict required checks are `Check Rust Code Format`, `Lint Rust Code`, and `Check MSRV`, each bound to GitHub Actions integration `15368`.
- Merge, squash, and rebase remain enabled; Actions workflow permissions and unrelated repository settings were unchanged.
- The disabled-first creation, active update, and effective-rule readback all passed. A rejected `PATCH` attempt was rolled back to zero rulesets before the documented `PUT` update was used.

Stage 2 remains optional and requires an independent reviewer before increasing the approval count. Scorecard alerts `#1` and `#61` remain scanner/history follow-up rather than reasons to weaken the solo-maintainer bridge.

## 31. Traceability matrix

| Requirement | Alert(s) | Source files or settings | Planned implementation | Verification evidence | Status |
|---|---:|---|---|---|---|
| SEC-REQ-001 | #53 | `ui/desktop/src/bin/jbang` | Exact JBang archive plus SHA-256 verification | shell syntax, live digest/layout check, regression guard | Complete; hosted wrapper exercise pending |
| SEC-REQ-002 | #58 | smoke workflow, UI workspace manifest, pnpm lock | Exact isolated workspace tool dependencies; remove global install | JSON parse, peer-resolution check, frozen lock validation, lock diff, regression guard | Complete; hosted provider smoke pending |
| SEC-REQ-003 | #54 | build CLI workflow | Archived rustup-init plus architecture digests | YAML/embedded-shell parse, official digest readback, regression guard | Complete; hosted manylinux build pending |
| SEC-REQ-004 | #57 | docs workflow | Locked local CLI build and install | YAML/embedded-shell parse, command assertions, regression guard | Complete; hosted docs workflow pending |
| SEC-REQ-005 | #53/#58/#54/#57 | `.github/scripts/verify-phase1-integrity.sh` | Network-free invariant assertions | executable mode, script syntax, successful execution | Complete |
| SEC-REQ-006 | #29/#4/#6/#26/#9/#14 | affected workflows | Permission and event inventory | source-to-permission table and read-only alert API evidence | Complete |
| SEC-REQ-007 | same | affected workflows | least-privilege job permissions | executable permission allowlist and actionlint | Complete in source |
| SEC-REQ-008 | same | affected workflows | untrusted code isolation | caller/called workflow and OIDC boundary review | Complete in source |
| SEC-REQ-009 | same | delivery history | separate Phase 2 patch | isolated diff, negative mutation, and validation log | Complete |
| SEC-REQ-010 | #40–#47 plus same-boundary executable references | workflows and Dockerfiles | tag plus immutable manifest-list digest | registry/log evidence, static guard, negative mutation | Complete in source |
| SEC-REQ-011 | container alerts | Dependabot and plan | weekly reviewed digest update path | parsed Dependabot Docker-directory contract | Complete in source |
| SEC-REQ-012 | #65 | manifest/lock/import graph | complete dependency map | advisory-to-artifact report | Planned Phase 4 |
| SEC-REQ-013 | #65 | determined after mapping | minimal dependency response | targeted compatibility/security checks | Blocked on mapping by design |
| SEC-REQ-014 | branch/review alerts | GitHub settings | administrator runbook | settings preview | Stage 1 complete |
| SEC-REQ-015 | branch/review alerts | GitHub settings | approval-gated mutation/readback | API/UI readback | Stage 1 complete; Stage 2 optional |

## 32. Risk register

| Risk | Likelihood | Impact | Detection | Mitigation | Residual disposition |
|---|---|---|---|---|---|
| JBang archive layout differs from documented layout | Low | Medium | expected launcher path check | fail before activation; preserve prior install | Digest and expected path verified live; wrapper exercise remains |
| JBang checksum is copied incorrectly | Low | High | static pin review; actual download check | use official release asset digest; fail closed | Live archive verification passed |
| macOS lacks GNU `sha256sum` | High | Medium | platform review | support `shasum -a 256` fallback | Low |
| manylinux architecture expression selects unexpected target | Low | High | explicit case branch | fail unsupported architecture | Low |
| official rustup archive is unavailable transiently | Low | Medium | curl failure | fail build; do not fall back to mutable installer | Accept availability failure over integrity bypass |
| exact provider releases have behavior incompatibility | Medium | Medium | provider smoke workflow | retain expected binaries; isolate exact package change | CI runtime verification remains required |
| package manager generates unrelated lockfile churn | Medium | Medium | hostile lock diff review | use repo pnpm; revert unrelated churn only | Low after review |
| package lifecycle scripts execute during install | Medium | High | lock/package review and CI behavior | exact lockfile narrows identity; future hardening may restrict scripts | Residual; not fully eliminated by #58 fix |
| local docs CLI build increases CI duration | High | Low | workflow duration | rely on Rust cache; optimize later only with measured evidence | Accept |
| docs workflow source build differs from historical release comparison intent | Low | Medium | inspect downstream use | current workflow comment permits HEAD and builds are expected | Review in hosted CI |
| static guard overfits formatting | Medium | Low | deliberate guard mutation review | assert security semantics and exact pins, not line numbers | Low |
| adjacent Hermit stable download remains mutable | High | High | scan/source review | track as separate finding; do not hide it | Open residual risk |
| workflow permission issues remain after Phase 1 | High | High | existing alerts | Phase 2 source remediation and regression guard | Fixed in source; remote rescan pending |
| `patch-release.yaml` is absent from the registered Actions workflow API | Existing | High | read-only `gh workflow view` returns 404 | retire the unused target and its dispatcher | Resolved by workflow deletion |
| pinned images stop receiving silent upstream security refreshes | Certain | High if unmanaged | weekly digest drift PRs | Dependabot monitors all four Dockerfile directories | Review and merge digest updates promptly |
| private GHCR test image digest becomes unavailable or is deleted | Low | Medium | test-finder pull failure | commit tag plus digest; fail closed and update from successful publish log | Hosted job validation pending |
| Dockerfile runtime compatibility is not locally built | Certain | Medium | hosted multi-platform build | registry platform validation and syntax/contract checks; no local build by scope | Accepted for this turn |
| non-container PinnedDependencies alerts remain | Certain | Medium–High | live code-scanning alerts | keep separate from base-image patch; schedule after required #65 mapping/order decision | Open and explicit |
| GitHub branch settings remain weak | Low after Stage 1 | High | effective-rules API readback | active no-bypass default-branch ruleset | Stage 1 complete; independent review remains optional |
| no local Gosling build leaves runtime compatibility unproven | Certain | Medium | stated validation boundary | run targeted CI later; user explicitly prohibited local build/restart | Accepted for this turn |

## 33. Phase 1 validation evidence log

This section is updated with observed results, not intentions.

### 33.1 Pre-change evidence

- `git status --short`: only `?? plan.md` before implementation.
- Alert `#53`: confirmed `curl -Ls https://sh.jbang.dev | bash -s - app setup` in the JBang shim.
- Alert `#58`: confirmed global unpinned install of three provider packages in the credential-bearing smoke-test job.
- Alert `#54`: confirmed `curl ... https://sh.rustup.rs | sh` in the manylinux release-build path.
- Alert `#57`: confirmed `stable/download_cli.sh` streamed into bash in the docs workflow.
- `rust-toolchain.toml`: channel `1.92`.
- Local ShellCheck availability: unavailable at Gate 1; use `bash -n` plus the security contract script.
- Local Ruby availability: `/opt/homebrew/bin/ruby`; usable for YAML syntax parsing.

### 33.2 Immutable source evidence

- JBang release: `v0.139.3`, generic archive `jbang-0.139.3.zip`, SHA-256 `6ff8d2f387583a8b1b1eb7839826a5e0a227c7cf1550e3bd85e0beb4838ca3ef`.
- rustup stable archive version: `1.29.0`.
- rustup x86_64 GNU archive SHA-256: `4acc9acc76d5079515b46346a485974457b5a79893cfb01112423c89aeb5aa10`.
- rustup aarch64 GNU archive SHA-256: `9732d6c5e2a098d3521fca8145d826ae0aaa067ef2385ead08e6feac88fa5792`.
- Claude Code package: `2.1.206`, executable `claude`.
- Claude ACP package: `@agentclientprotocol/claude-agent-acp@0.58.1`, executable `claude-agent-acp`.
- Codex ACP package: `@agentclientprotocol/codex-acp@1.1.2`, executable `codex-acp`.

### 33.3 Post-change command log

- `bash -n ui/desktop/src/bin/jbang`: passed.
- `bash -n .github/scripts/verify-phase1-integrity.sh`: passed.
- `.github/scripts/verify-phase1-integrity.sh`: passed all four P0 contracts.
- Ruby YAML parsing for `build-cli.yml`, `docs-update-cli-ref.yml`, and `pr-smoke-test.yml`: passed.
- Extracted `Build CLI (manylinux container)` run block with GitHub expressions replaced for parsing, then `bash -n`: passed.
- Extracted `Install gosling CLI` run block, then `bash -n`: passed.
- Node JSON parse of `ui/package.json`: passed.
- `pnpm install --lockfile-only --ignore-scripts` after peer isolation: passed without peer warnings; only expected cross-platform workspace notices and existing deprecated-subdependency notices remained.
- `pnpm --dir ui/desktop install --frozen-lockfile --lockfile-only --ignore-scripts`: passed with pnpm `10.30.3`.
- Desktop pnpm execution PATH inspection: confirmed `/Users/eric/Work/vscode/forked/gosling/ui/node_modules/.bin` is present.
- Official npm metadata readback: exact package versions, executable names, and the three lockfile integrity hashes matched.
- Live JBang archive download and `shasum -a 256 -c -`: passed for `jbang-0.139.3.zip`.
- Live `unzip -Z1` archive inspection: found exact path `jbang-0.139.3/bin/jbang`.
- The first optional JBang layout attempt used `jar tf` and could not run because the repository Hermit environment did not have a Java runtime active. This did not exercise the wrapper, which installs OpenJDK 17 before extraction. The network artifact was redownloaded, reverified, and inspected successfully with `unzip` as a non-executing compensating check.
- Official rustup `.sha256` readback: both x86_64 and aarch64 digests matched the workflow constants.
- `git diff --check`: passed.
- Prohibited-pattern search across all four patched alert paths: no `sh.jbang.dev`, `sh.rustup.rs`, provider `npm install -g`, or stable `download_cli.sh` installer remained.
- Workflow permission addition check: no permission changes were introduced.
- Hostile lockfile review: changes consist of the five exact UI workspace-root tool/peer dependencies, their platform/transitive packages, one scoped Zod override, and three lines removed as pnpm normalized obsolete peer snapshots.
- ShellCheck was not available locally. Bash syntax checks, embedded-workflow shell parsing, the regression script, live checksum checks, and hostile diff review are the compensating evidence.

No Gosling build, Rust test, JavaScript test, provider integration test, Desktop launch, process restart, workflow dispatch, GitHub alert mutation, or GitHub settings change was performed.

### 33.4 Finding dispositions

- Alert `#53`: `fixed` in source. The vulnerable network-to-shell path is removed; a pinned JBang archive is verified before extraction. Live digest and layout checks passed. Full wrapper execution remains for hosted or separately authorized runtime validation.
- Alert `#58`: `fixed` in source. Floating global installs are removed; exact current packages and necessary peer providers are isolated at the existing UI workspace root and integrity-locked. Hosted credential-bearing smoke tests remain pending.
- Alert `#54`: `fixed` in source. The rustup shell bootstrap is replaced by architecture-specific, verified `rustup-init` archives with an unsupported-target failure branch. Hosted manylinux execution remains pending.
- Alert `#57`: `fixed` in source. The remote mutable CLI installer is replaced by a locked source build from the checked-out revision using Rust `1.92`. Hosted docs-workflow execution remains pending.
- SEC-REQ-005: `fixed`; the executable network-free regression guard passes.

### 33.5 Hostile review outcome

- No downloaded bytes reach bash or another interpreter in the four remediated paths.
- JBang and rustup identity checks occur before extraction or execution.
- Rustup target selection is explicit and fails closed for unknown targets.
- Provider tools do not use global or floating resolution and do not alter Desktop's Zod 3/MCP SDK 1.27 application graph.
- The docs CLI is built from the checkout and committed Cargo lockfile, with a compiler channel matching `rust-toolchain.toml`.
- No permission, secret, trigger, GitHub setting, or running-process behavior changed.
- The remaining risks in Section 32 are not silently represented as closed.

## 34. Plan-change record

### PCR-001 — Merge security remediation into continuity plan

Date: 2026-07-10  
Trigger: user requested that the security plan incorporate and refactor the last session's plan.  
Decision: retain the Session Handoff plan as Part I and add the security program as Part II in the same `plan.md`.  
Rationale: one durable handoff file preserves intent across model switches while keeping the workstreams explicitly gated.  
Requirements affected: all; no acceptance criterion removed.  
Validation impact: adds independent security gates without changing continuity validation.  
Rollback: split Part II into a later document only if repository maintainers request it.

### PCR-002 — Permit master plan beyond 800 lines

Date: 2026-07-10  
Trigger: user explicitly stated that the plan may exceed 800 lines.  
Decision: prioritize a complete, self-contained plan over the prototype skill's normal source-size guideline.  
Rationale: the plan is a handoff and governance artifact, not production source; splitting it would weaken discoverability for this task.  
Requirements affected: none.  
Validation impact: confirm headings and traceability remain navigable.  
Rollback: split by workstream with a top-level index if the file becomes operationally difficult to review.

### PCR-003 — Build docs CLI from source

Date: 2026-07-10  
Trigger: alert `#57` and discovery that the referenced mutable `stable` release is not a reliable immutable CLI identity.  
Decision: use the checked-out revision and committed Cargo lockfile instead of introducing another binary-download checksum table.  
Rationale: the workflow generates docs for repository versions and already configures Rust; source provenance is clearer and avoids a mutable release alias.  
Requirements affected: SEC-REQ-004.  
Validation impact: hosted CI must later confirm build duration and downstream command compatibility.  
Rollback: revert the workflow step; do not restore the mutable installer except for isolated diagnosis.

### PCR-004 — Migrate deprecated ACP npm namespaces

Date: 2026-07-10  
Trigger: alert `#58` research showed the existing `@zed-industries` ACP packages are deprecated and point to `@agentclientprotocol` replacements.  
Decision: lock current replacement packages rather than pinning already-deprecated package identities.  
Rationale: replacement packages preserve executable names and align smoke tests with current ACP distribution.  
Requirements affected: SEC-REQ-002.  
Validation impact: provider smoke CI must confirm protocol compatibility.  
Rollback: exact older package pins can be tested in an isolated credential-free job if compatibility fails; do not restore floating globals.

### PCR-005 — Isolate provider tools at the UI workspace root

Date: 2026-07-10  
Trigger: the first lockfile-only resolution placed Claude ACP beside Desktop's Zod 3 and MCP SDK 1.27 dependencies, producing peer-contract warnings for Claude Agent SDK.  
Decision: declare the provider executables in the existing `ui` workspace root, add exact MCP SDK 1.29 and Zod 4 peer providers there, and scope a Zod 4 override only to Claude ACP.  
Rationale: smoke-test tools remain available to workspace scripts without forcing a Zod or MCP SDK upgrade into the Desktop application dependency graph.  
Requirements affected: SEC-REQ-002 and SEC-REQ-005.  
Validation impact: require a warning-free provider peer resolution, frozen-lockfile validation, and hosted smoke-test confirmation that workspace-root binaries are on the script PATH.  
Rollback: remove the five root tool dependencies and scoped override, regenerate the lockfile, and test any alternative only in a credential-free job before restoring provider credentials.

### PCR-006 — Scope reusable-workflow permissions through callers and called jobs

Date: 2026-07-10
Trigger: Phase 2 caller tracing showed that a reusable workflow can only downgrade the token passed by its caller, and Windows OIDC was requested at called-workflow scope.
Decision: harden both sides of each boundary: callers pass only required permissions, called build workflows declare read-only defaults, and Windows OIDC is granted only to the signing job.
Rationale: changing only the six scanner anchors would leave compatibility gaps or allow a broad caller token to remain available to repository build steps.
Requirements affected: SEC-REQ-006, SEC-REQ-007, SEC-REQ-008, and SEC-REQ-009.
Validation impact: include `pr-comment-build-cli.yml` and `bundle-desktop-windows.yml` in the permission contract, caller graph review, YAML parsing, and actionlint.
Rollback: revert caller and called-workflow permission changes together so reusable-workflow token negotiation remains internally consistent.

### PCR-007 — Record unregistered patch-release workflow without expanding scope

Date: 2026-07-10
Trigger: the read-only no-checkout behavior check found that GitHub returns `404` for `patch-release.yaml` and does not list it as a registered Actions workflow.
Decision: preserve the existing dispatch target and record the pre-existing operational problem instead of changing release behavior during a permission remediation.
Rationale: alert `#14` concerns token scope; workflow registration is a separate release-automation defect with different validation and change risk.
Requirements affected: SEC-REQ-007 and SEC-REQ-009 behavior-preservation evidence.
Validation impact: mark actual patch dispatch as unavailable, retain actionlint and structural dependency evidence, and require separate investigation before release automation relies on it.
Rollback: none; this record changes no behavior.

### PCR-008 — Include executable container references missed by Scorecard

Date: 2026-07-10
Trigger: the Phase 3 inventory found a mutable credential-bearing workflow image and a generated release-builder Dockerfile outside alerts `#40–#47`.
Decision: pin `test-finder.yml` and the npm publish script's generated Rust builder alongside the reported Dockerfiles, while leaving documentation-only examples and non-image OCI features out of scope.
Rationale: both included references cross the same mutable-tag-to-code-execution boundary; omitting them would leave the Phase 3 security invariant false in active automation.
Requirements affected: SEC-REQ-010 and SEC-REQ-011.
Validation impact: add both references to the deterministic guard, use the successful GHCR publication log as private-image evidence, and retain weekly Dependabot coverage for real Dockerfiles.
Rollback: replace either with a different verified tag/digest pair; never restore an unpinned tag.

### PCR-009 — Make Auto authoritative for permission prompts

Date: 2026-07-10
Trigger: repeated provider-managed `Write` permission cards despite the session being configured as Auto.
Decision: Auto approves normal and app-dispatched tool requests without running permission/security inspectors, auto-confirms provider-managed tool permission requests before they reach clients, and supplies the Claude Code subprocess permission flags from the session mode rather than the global default.
Rationale: the public Auto contract is automatic approval. A session-level Auto choice must not be downgraded by a global SmartApprove default or by provider-specific permission routing.
Preserved boundary: explicit pre-tool policy hooks still run and may deny a call; Approve, SmartApprove, and Chat retain their existing behavior.
Validation impact: require focused tests for approve-all classification, provider permission-message filtering, and Claude Code mode flags.
Rollback: revert the three Auto decision points together to avoid inconsistent behavior between native tools, app calls, and provider-managed tools.

### PCR-010 — Retire unused write-capable workflows

Date: 2026-07-10
Trigger: repository-owner direction to remove the unregistered patch-release path, Ask AI Bot publication, and tag-triggered patch dispatcher instead of retaining their write permissions.
Decision: delete `.github/workflows/patch-release.yaml`, `.github/workflows/publish-ask-ai-bot.yml`, and `.github/workflows/close-release-pr-on-tag.yaml`, then remove the deleted close-release workflow from the Phase 2 regression contract.
Rationale: deletion is the narrowest complete boundary when the automation is outside the owner's normal workflow; it removes the write-capable execution paths instead of disguising them with scanner suppression.
Behavior change: automatic patch-release creation, tag-triggered release-PR closure/patch dispatch, and Ask AI Bot image publication are intentionally retired.
Validation impact: require repository-wide reference checks, workflow YAML parsing for survivors, and the adjusted Phase 2 permission contract.
Rollback: restore a workflow only with an explicit current use case and a new least-privilege design; do not restore the three files merely to close a historical compatibility gap.

### PCR-011 — Exclude inherited CI/review skills

Date: 2026-07-10
Trigger: repository-owner direction to remove Goose-inherited CI skills outside the normal Gosling workflow.
Decision: exclude `code-review` and `testing-strategy` from generated and live Goose fallback manifests while preserving the rest of the compatibility catalog and provenance normalization.
Rationale: a shared deterministic exclusion at generation and runtime prevents the live fallback from silently restoring removed entries.
Validation impact: extend converter tests, regenerate the ignored local manifest, and verify both excluded IDs are absent while unrelated skills remain.
Rollback: remove an ID from the shared exclusion set if it becomes part of the supported workflow.

### PCR-012 — Activate the Stage 1 solo-maintainer bridge

Date: 2026-07-10
Trigger: explicit repository-owner approval after the Phase 5 runbook and settings preview.
Decision: activate repository ruleset `18782969` with no bypass actors, zero required approvals, resolved-thread enforcement, deletion and force-push protection, and three stable strict checks.
Rationale: this closes direct-push integrity gaps without locking a single maintainer out of the repository.
Validation impact: disabled-first readback, active readback, effective-rule inspection, protected-branch status, green check identities, and preserved repository behavior.
Rollback: disable and delete the ruleset through the repository rulesets API if activation becomes unusable.

## 35. Auto/workflow/skills cleanup validation evidence

Affected paths:

- Auto permission routing: `crates/gosling/src/agents/agent.rs`, `crates/gosling/src/permission/permission_judge.rs`, and `crates/gosling/src/providers/claude_code.rs`.
- Retired automation: `.github/workflows/patch-release.yaml`, `.github/workflows/publish-ask-ai-bot.yml`, and `.github/workflows/close-release-pr-on-tag.yaml`.
- Goose skill compatibility: `documentation/scripts/goose-compat.js`, `documentation/scripts/generate-skills-manifest.js`, `documentation/src/utils/goose-compat.ts`, and `documentation/src/utils/skills.ts`.

Observed verification:

- `cargo fmt` and `cargo fmt --check`: passed; no Gosling build or runtime restart was performed.
- `git diff --check`: passed.
- `ruby -c .github/scripts/verify-phase2-permissions.rb`: passed.
- `.github/scripts/verify-phase2-permissions.rb`: passed after deleting the retired workflow contract.
- `.github/scripts/verify-phase1-integrity.sh`: passed.
- `.github/scripts/verify-phase3-container-digests.rb`: passed.
- `node --check` for the three changed documentation scripts: passed.
- `node --test documentation/scripts/goose-compat.test.js`: six tests passed, including the new exclusion test.
- `node documentation/scripts/goose-compat.js --self-test`: passed.
- `actionlint` over all surviving workflow YAML: passed with no findings.
- The regenerated ignored local skill manifest contains five entries, excludes `code-review` and `testing-strategy`, and retains `frontend-design` as a legitimate control.
- Repository-wide active `.github` references contain no patch-release dispatch or Ask AI Bot publish workflow self-reference.
- Focused Rust regression tests were added for approve-all classification, permission-message filtering, and session-mode Claude flags. They were not executed because the standing repository instruction requires explicit authorization before Cargo tests/builds.

Security closure:

- Auto no longer routes normal, app-dispatched, or provider-managed tool calls through an interactive permission request.
- Non-Auto permission behavior remains unchanged, and explicit pre-tool policy hooks remain enforceable in Auto.
- The three write-capable workflow execution paths no longer exist in source; future scanner runs may retain their historical alerts until SARIF is refreshed.
- Excluded Goose skills cannot reappear through either build-time generation or live fallback normalization.

## 36. Continuation handoff for the next session

If work resumes after the Stage 1 governance and Auto/workflow cleanup, the next model should:

1. Read repository `AGENTS.md` and this entire `plan.md`.
2. Inspect `git status --short` and preserve all changes.
3. Treat security Phases 1–3 and governance Stage 1 as complete; preserve the current Auto/workflow/skill cleanup diff until it is reviewed and delivered.
4. Do not run a Gosling build, test suite, Desktop process, or restart unless the user expands authorization.
5. Do not modify further GitHub settings or alert states without a new explicit authorization.
6. Re-run the Phase 1, Phase 2, and Phase 3 regression guards after any overlapping edit.
7. Update the evidence log with actual exit status and relevant output.
8. Record any strategy change in Section 34 before expanding scope.

The next planned security phase is Phase 4: investigate alert `#65` and produce the dependency-to-lockfile-to-artifact mapping before changing any dependency. The three retired workflows are no longer follow-up items. Keep remaining non-container PinnedDependencies alerts and optional Stage 2 independent-review governance explicit and separate.


## 37. Open-defect repair campaign closure (2026-07-20)

Status: source repair complete; execution verification deferred because this campaign did not authorize builds or tests.

- ACP agents now bind configuration, data, state, instance identity, and request execution to task-local runtime paths.
- Global configuration and instance identity caches are keyed by their active scoped paths.
- Desktop browser globals are explicit and workspace session filters are referentially stable.
- Provider inventory reads are cached/coalesced and mutation paths invalidate stale inventory.
- `check-acp-schema` resolves from `justfile_directory()` instead of the caller's working directory.
- Previously deferred records ORCH-002, REC-001, REC-002, RES-002, and RES-003 are reconciled as already satisfied by current source.
- The bounded ACP response policy remains intentional fail-closed behavior; streaming/pagination is future API work.
- Session Handoff, Tagteam expansion, CLI usage reporting, release execution, and broad modularization are feature/maintenance backlog rather than defects.
- Giles's uniqueness-constraint crash and macOS Keychain authorization are external/manual validation constraints.
