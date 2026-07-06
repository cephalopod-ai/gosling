# Gosling Delta Audit & Repair — 2026-07-05 (commits `713f1eef2..9d9df730f`)

Follow-up engagement on top of the merged stress-test audit + repair campaign
(`99-master-report.md`, `repair-campaign-log.md`). Scope: **only** the 8 commits
that landed today, `713f1eef2^..9d9df730f` (83 files, +4115/-205). Authority:
audit + patch-authorized, working-tree only — nothing committed. Orchestrated by
`dataflow-lead`; executed by `dataflow-architect`, `concurrency-engineer`,
`pipeline-analyst` in parallel, with one cross-department security review
(`senior-security-officer`).

## Scope decision (honest)

The user asked to "audit it and repair any findings." Same eligibility bar as
the prior campaign: real, localized, compile/test-verifiable bugs get fixed
directly; feature decisions, default-posture/policy calls, and anything needing
a live app build or a network-gated compile target get **dispositioned** with a
reason and a recommended owner instead of blind-patched. Two items additionally
carried a live security implication and were routed cross-department to
`senior-security-officer` rather than silently dispositioned — see below.

Three specialists were run in parallel, each bounded to a slice with an explicit
do-not-collide boundary; two shared files with the concurrent slice next door
were reviewed **by hunk**, not whole-file, to avoid stepping on each other:

| Specialist | Slice |
|---|---|
| `dataflow-architect` | Session-resume/compaction data contract + provider-config rework (`session_manager.rs`, ACP schema/custom-request types, `providers/*`) |
| `concurrency-engineer` | New code-execution-runtime (embedded v8/Deno) feature — gating, resource lifecycle, blast-radius |
| `pipeline-analyst` | Documentation tooling (`goose-compat.*`), frontend session-resume consumer (`useChatSession.ts` and siblings) |

## Gate 0 — posture & baseline

- Working tree was clean at start; branch `main`.
- Verification harness (matches the existing campaign's harness): `cargo check
  -p gosling --features nostr` / `cargo test -p gosling --features nostr --lib`.
  `-p gosling-cli` remains known-broken here (network-gated static-lib
  dependency) — pre-existing environment limit, not this delta's doing.
- The new v8/code-execution-runtime feature itself cannot be compiled or
  exercised in this sandbox (same network gate on its build dependency) — its
  findings are **source-confirmed**, not runtime-reproduced. Flagged explicitly
  wherever relevant below.
- `documentation/` and `ui/desktop/` both have pre-existing `node_modules`
  (no network install needed): `node --test`, `npx tsc` / `npx tsc --noEmit`,
  `vitest run` all ran directly.

## Findings — Fixed (8 defects, all working-tree, all verified)

| ID | Severity | Owner | File(s) | What |
|---|---|---|---|---|
| CTR-GSL-010 | Low | dataflow-architect | `acp/server/load_session.rs` | `_meta.summary` emitted `coverageThroughRowId`/`coverageThroughTimestamp`; canonical name everywhere else (schema, SDK, Rust struct) is `coveredThrough*`. Latent (no consumer yet) but a silent break the moment one lands. Same item independently flagged by pipeline-analyst as cross-cutting `DWF-D1` — folded in, not double-fixed. |
| CTR-GSL-011 | Low (test-only) | dataflow-architect (found by dataflow-lead) | `context_mgmt/summarizer/mod.rs`, `context_mgmt/packet.rs` | Process-global `DIGEST_CACHE` cleared via an **unscoped** `clear_cache_for_test()` full-map `.clear()`, called from 4 sites across 2 files with no mutual serialization (`env_lock` doesn't cover 3 of the 4 sites). Under `cargo test`'s default concurrency, any clear could wipe another in-flight test's freshly-stored entry — the intermittent `on_mode_populates_the_cache_and_writes_memories` panic caught during final verification. Fixed with a key-scoped `remove_digest_for_test(key)` helper, removing the 3 unnecessary defensive clears and scoping the one genuine cleanup to its own key. Pre-existing code (not introduced by this delta) — disclosed as such rather than misattributed. |
| DWF-001 | Medium | pipeline-analyst | `ui/desktop/src/hooks/useChatSession.ts` | Paginated history-load error in `loadOlderMessages` failed silently — no user-facing signal. Added a `toastError` call mirroring the existing `onMessageUpdate` error pattern. |
| DWF-002 | Medium | pipeline-analyst | `ui/desktop/src/hooks/useChatSession.ts` | Re-entrancy guard read a stale `useRef` snapshot instead of the live store (`acpChatSessionStore.getSnapshot(sessionId)`), allowing duplicate concurrent page fetches on fast scroll. Fixed to read the store directly. |
| DWF-003 | Low | pipeline-analyst | `documentation/scripts/goose-compat.js`, `documentation/src/utils/goose-compat.ts` | Case-sensitive Goose→gosling regex left mixed-case text unconverted, violating `AGENTS.md`'s lowercase-gosling mandate. Fixed with a `gi` flag; removed the now-redundant `\.goose\b` rule. |
| DWF-004 | Low | pipeline-analyst | same two files | Catalog entries missing `id` were silently dropped via a bare `continue`. Added a `console.warn` before the drop in both `.js`/`.ts`. |
| DWF-005 | Low | pipeline-analyst | `documentation/src/utils/mcp-servers.ts`, `documentation/src/utils/skills.ts` | Fallback-to-Goose-catalog logic checked raw-fetch emptiness instead of post-dedupe/normalization emptiness, breaking the documented fallback contract when dedupe/missing-id filtering collapsed a non-empty fetch to zero. Fixed to check emptiness after normalization. Self-caused regression (loss of generic-type inference on `dedupeAndSortById<T>`, TS2322 in `skills.ts`) caught and fixed with an explicit `: Skill[]` annotation in the same pass — confirmed via `git stash`/`npx tsc`/`git stash pop` that the error did not exist on the original baseline. |
| DWF-006 | — | pipeline-analyst | `documentation/scripts/goose-compat.test.js` | 2 new regression tests added (12→14) covering DWF-003/004. |
| CER-GSL-002 | Medium | dataflow-architect (security-approved implementation) | `config/base.rs`, `agents/agent.rs`, `CodeExecutionRuntimeSection.tsx` | Code-execution-runtime default flipped `Enabled → Disabled` at all 4 sites, per operator decision (see Escalation Resolution below). Fail-closed-on-parse-error behavior preserved; a proving test confirms an unset config registers no `code_execution` extension and omits `execute_typescript` from the tool list/prompt. |

**Regression coverage:** `cargo test -p gosling --features nostr --lib` = **1289
passed / 0 failed**, confirmed across 4 back-to-back parallel runs (post
CTR-GSL-011 fix). `node --test scripts/*.test.js` = 14/14. `ui/desktop`: `npx tsc
--noEmit` clean; `vitest run` 379/382 (3 pre-existing failures, confirmed
unrelated via git-stash diff — 2 in `AuthSettingsSection.test.tsx`, 1 in
`sessions.test.ts` predating this delta). `documentation`: `npx tsc` output
byte-for-byte identical to the stashed baseline outside touched files, zero new
errors in touched files. Post CER-GSL-002 fix: `cargo test -p gosling --features
nostr` = 1289/0; `cargo test -p gosling --features nostr,code-mode` = **1298/0**
(the wider feature set is required to exercise the code-mode proving test —
`dataflow-architect` caught that the standard `--features nostr` run wouldn't
have pulled it in, and ran the broader set to actually confirm it).

## Escalation resolution — CER-GSL-002 (closed this engagement)

Escalated to `senior-security-officer` (see below), who confirmed Medium
severity and recommended flipping the default to `Disabled`/opt-in as the
low-cost, reversible mitigation. **The operator (this repo's de facto
maintainer) approved the flip directly.** The security officer delegated
implementation to `dataflow-architect` with a fixed, testable spec (flip the 4
default sites; grep for any other silent `::Enabled` reliance; add a proving
test; keep CER-GSL-001 explicitly out of scope) and independently verified the
resulting diff and test run before signing off:

- Enum default (`base.rs:93-95`), unset-resolve arm (`base.rs:1237`),
  `AgentConfig::new` default (`agent.rs:193`), and the UI default
  (`CodeExecutionRuntimeSection.tsx:42-49`) all now resolve to `Disabled`.
  Fail-closed-on-parse-error path untouched (still resolves `Disabled` + logs a
  warning, so it was already conservative on that arm).
  All 5 other hardcoded `::Enabled` occurrences in the crate are inside
  `#[cfg(test)]`/`mod tests` blocks (explicit fixtures), not default-reliant —
  confirmed via grep, correctly left untouched.
- New proving test (`agent.rs`, `#[cfg(feature = "code-mode")]`): with the
  runtime unset, `AgentConfig::new` resolves `Disabled`, `add_extension` for
  `code_execution` errors with `GOSLING_CODE_EXECUTION_RUNTIME=disabled`, and
  `prepare_tools_and_prompt` returns no tools and a prompt without
  `execute_typescript`.
- **CER-GSL-001 (the callback-bypass) is unaffected and still open** —
  `code_execution.rs`/`extension_manager.rs`/`reply_parts.rs` were explicitly
  fenced out of this task and remain unchanged. The practical effect of this
  fix: the bypass is **no longer default-reachable** — a fresh install now
  ships `execute_typescript` off, so an operator must explicitly opt in before
  that code path exists at all. The "enabled + hardened mode" combination
  remains unsafe until CER-GSL-001 itself is fixed; anyone who re-enables the
  runtime should be aware the bypass is live the moment they harden to
  `Approve`/`SmartApprove`.

`dataflow-lead` independently re-verified the working-tree diff against this
description before folding it into this report — matches exactly, no scope
creep. Changes remain **uncommitted**, left for the operator alongside the
rest of this engagement's fixes.

## Escalated findings — new code-execution-runtime feature (not silently patched)

The concurrency-engineer's slice traced a brand-new embedded v8/Deno
code-execution feature added in this delta, whose own last commit
(`9d9df730f`) claims to fix "gating gaps found in ultrareview." That claim was
independently re-traced rather than taken at face value.

### CER-GSL-001 — Code-mode callbacks bypass the permission gate — **High, DISPOSITIONED → ESCALATE-SECURITY**
`code_execution.rs:353` → `ExtensionManager::dispatch_tool_call`
(`extension_manager.rs:1822`) — the **raw** dispatch path. The permission
inspector and PreToolUse hooks live only in the Agent-level wrapper
(`agent.rs:989-1052, 2211-2220`); code-mode's `execute_typescript` calls into
`ExtensionManager` directly, skipping both. Any tool a script invokes runs
ungated, even in the app's hardened `SmartApprove`/`Approve` modes. Same class
as the prior audit's Cluster A finding `CTR-GSL-001`, but now **default-reachable**
because of CER-GSL-002 below. Confirmed against source; not runtime-reproduced
(feature is network-gated to build here).

**Security review (`senior-security-officer`), verbatim:**
> **CER-GSL-001 confirmed.** The code-mode callback closure holds an
> `Arc<ExtensionManager>` (`code_execution.rs:337,343`) and calls
> `manager.dispatch_tool_call(...)` (`:353`)... The code-mode path enters the
> ExtensionManager directly, so it skips both [the permission inspector and
> PreToolUse hooks].
>
> **Confirm High** (considered Critical, held at High). The concrete risk: in
> `Approve`/`SmartApprove` modes — the modes an operator deliberately opts into
> to get per-call confirmation on shell/write — a script run from inside
> `execute_typescript` issues those same calls with no confirmation and no
> PreToolUse enforcement. The operator approves one opaque TypeScript blob and
> unknowingly authorizes an unbounded, ungated tool sequence. This is precisely
> the prompt-injection threat model the gate exists for... PreToolUse hooks are
> also an audit/telemetry surface, so bypassed nested calls may be invisible to
> logging, not just to confirmation. Held at High rather than Critical only
> because it's a defense-in-depth bypass requiring the model to be driven to
> emit the script, and it does not by itself cross a network/external trust
> boundary. In hardened modes it is High edging toward Critical.

Disposition reason: routing code-mode callbacks through the existing gate is an
architecture change to the execution path, not a bounded bug-fix, and the
feature can't be compiled/exercised here to verify a fix live.

**Hold-lift bar (security officer's fixed, testable requirement):** code-mode
callbacks must get the same permission-inspector verdict as Agent-level calls
(min. `Shell`/`Write`) **and** fire PreToolUse (confirmation *and* audit
surface). Runtime proof: `developer.shell("id")` from inside a script produces
a confirmation/denial in `SmartApprove`, and an unset-config runtime registers
no `code_execution` tool. **Owner: senior-security-officer + dataflow-architect**,
once picked up.

### CER-GSL-002 — Runtime defaults to `Enabled` — **Medium, ESCALATE-SECURITY → FIXED ✅ (see Escalation Resolution above)**
`config/base.rs:92-97,1237`; `agent.rs:193`; TS default at
`CodeExecutionRuntimeSection.tsx:45` — all default `Enabled` when unset. Fails
closed only on a parse error, not on unset. This is the same "default=Auto is
non-enforcing" pattern the original 35-lens audit flagged as ship-gating
(Cluster A) elsewhere in the codebase, now recurring in a brand-new feature.

**Security review, verbatim:** *"confirm Medium, but note it is the force
multiplier. Alone it's an unsafe default; combined with 001 and the default
`GoslingMode = Auto`, a fresh install with zero operator action can execute
model-authored TypeScript and any tool it calls, ungated. 002 is what turns 001
from latent to default-reachable."*

**Rollout recommendation, verbatim:** *"Hold the default-Enabled posture; the
feature can ship if it ships opt-in. Flip the default to `Disabled` (or
first-use explicit opt-in) now. This is the minimum safer alternative..."*

**Standing flag for the operator:** default `GoslingMode = Auto` makes the
CER-GSL-001 bypass moot today — but it becomes live the instant the operator
switches to `Approve`/`SmartApprove`. "Enabled + hardened mode" is the specific
combination that is unsafe until CER-GSL-001 is fixed.

Disposition reason: flipping a shipped default is a maintainer/policy call, not
a silent patch. **This repo's human maintainer is the user/operator directly** —
there is no separate maintainer team — so this decision is surfaced to the
operator in this report rather than routed further. **Awaiting operator
decision** on: (a) flip the default now, and (b) whether to hold further
rollout of the feature pending a CER-GSL-001 fix.

### CER-GSL-003 — `DEFAULT_MAX_SESSION` cut 100→5 amplifies subprocess churn — **Low, DISPOSITIONED**
`execution/manager.rs:15`. Lower cap means more frequent eviction/recreation of
MCP subprocesses under the new runtime's usage pattern. Tuning decision, not a
bug. → **owner: maintainer/dataflow-lead.**

### CER-GSL-004 — Restart-required notice not `aria-live` announced — **Low/a11y, DISPOSITIONED**
`CodeExecutionRuntimeSection.tsx:133-139`. Plausible (needs a live app to
verify screen-reader behavior safely). → routed to `repair-design-webapp` /
UI-owning team, consistent with how the prior campaign routed all UI/a11y
items it couldn't verify without a running app.

## Non-findings (checked and held)

- **Resource lifecycle of the v8 runtime**: in-process Deno isolate under a
  process-wide mutex — no OS process to leak, no orphaned subprocess on error
  or cancel paths. Held.
- **Config parse failure fails closed**; persistence does not resurrect a
  disabled runtime across restarts. Held.
- **No new/suspicious dependency** introduced for the v8 runtime in
  `Cargo.lock`/`Cargo.toml`. Held.
- **Pagination cursor math** in `get_session_message_page`: sentinel/`has_more`/
  truncate/reverse/next-cursor chain has no off-by-one, no cross-page gap or
  overlap. Held.
- **Schema/meta ↔ Rust parity** for all 3 new ACP methods: field names, required
  sets, and types match exactly across `acp-schema.json`, `acp-meta.json`,
  `custom_requests.rs`, and the generated TS SDK. Held.
- **Migration v16**: dual-pathed, idempotent, correct cascade/FK behavior. Held.
- **Transaction discipline** (`BEGIN IMMEDIATE` on summary/truncate paths,
  cascade-clear on delete). Held.
- **`is_first_turn = message_count == 0`** optimization: provably safe (`COUNT`
  is 0 iff empty). Held.
- **Provider-config rework** (`base.rs` +236 lines): heuristics reasonable and
  unit-tested; legacy-model removal only affects the picker list, limits still
  resolve via fallback. No regression. Held.
- **3dddf31bd commit message overstates its own change** (claims new capability
  marking; the diff actually centralizes an existing hidden-provider ID list and
  adds 2 IDs to it). Functional change is coherent and correct — message
  imprecision only, not a code defect.

## Disposition ledger — NOT fixed this engagement (with reasons)

### Escalated — security/policy decision (operator = maintainer here)
- **CER-GSL-001** (permission-gate bypass in code-mode) — still open.
  Architecture-level fix, needs a live v8 build to verify. → owned jointly by
  `senior-security-officer` + `dataflow-architect`, once the operator
  green-lights work on it. **Note:** CER-GSL-002's fix (below) makes this
  no longer default-reachable, but the underlying bypass mechanism is
  unchanged and becomes live again the moment an operator opts the runtime
  back on under `Approve`/`SmartApprove`.
- ~~**CER-GSL-002** (unsafe `Enabled` default)~~ — **resolved this engagement.**
  Operator approved flipping to `Disabled`/opt-in; implemented by
  `dataflow-architect`, security-signed-off by `senior-security-officer`. See
  "Escalation resolution — CER-GSL-002" above. Moved out of this ledger into
  the Fixed table.

### Deferred — needs a live app / integration environment to verify safely
- **SRP-GSL-001** (Medium, Likely) — `get_session_for_compacted_resume`
  (`session_manager.rs:2170-2205`) prepends the durable summary whenever it's
  non-empty, regardless of `status` or whether `covered_through_row_id` reaches
  the tail's `oldest_row_id`. When the async rollup is stale/failed or races a
  resume, messages in the gap are in neither the summary nor the tail — dropped
  from reconstructed context, with the "older history exists" notice suppressed
  because a summary row exists. The staleness *is* surfaced via
  `_meta.summary.status`, but a real fix changes context content/token
  accounting. → `dataflow-lead` + maintainers.
- **SRP-GSL-003** (Low, Likely, edge) — `get_session_tail_page` can still
  return an orphaned `ToolResponse` if the orphan-clearing window hits the
  200-message page cap first; a resumed conversation could front a dangling
  tool response some strict providers reject. Needs >200 tool-heavy tail
  messages to trigger; correct remedy (prune vs. keep growing) is a behavior
  decision. → recommend pruning; owner: maintainer.
- **CER-GSL-003** (subprocess churn from lowered session cap) — tuning
  decision. → maintainer/dataflow-lead.
- **CER-GSL-004** (a11y notice not announced) — needs a running app to verify
  safely. → `repair-design-webapp`.
- **DWF-D3** (Low, Plausible) — unchecked `response.messages as Message[]`
  cast in `ui/desktop/src/acp/sessions.ts`. Needs a live app to verify safely.
  → repair-design-webapp / frontend owner.
- **DWF-D4** (Low, Plausible) — possible `messageId` collision risk in
  `useChatSession.ts`'s steer-message reconciliation; not confirmed, needs
  further investigation of the server-side ID generator. → frontend owner,
  follow-up investigation.

### Routed — API/contract semantics decision
- **SRP-GSL-002** (Low, Confirmed) — `search_session_messages.total_matches`
  reports the capped returned-count (`matches.len()`), not the true total, when
  hits exceed `limit`. Correct semantics (true `COUNT(*)` vs. returned-count) is
  an API-owner decision — recommend either a second `COUNT` query or renaming
  the field to `returned_matches`. → API owner.

### Not a bug (feature gap / half-shipped, informational)
- **DWF-D2** — `_meta.summary` blob has no frontend consumer yet. Not a defect;
  noted so CTR-GSL-010's fix doesn't get mistaken for closing out a feature.

## Final status

**Completed and verified** for the in-scope eligible-defect set: **8 defects
fixed** across Rust core, docs tooling, the frontend session-resume consumer,
and the code-execution-runtime default posture, all compile/test verified
(`cargo test --lib` 1289/0 across 4 runs; `cargo test -p gosling --features
nostr,code-mode` 1298/0 for the CER-GSL-002 proving test specifically; `node
--test` 14/14; `vitest` 379/382 with all 3 failures confirmed pre-existing;
`tsc` clean in touched files both repos). 6 further findings dispositioned with
concrete owners and reasons (session-resume edge cases, an API-semantics call,
a subprocess-tuning call, an a11y item, two plausible-but-unconfirmed frontend
items). Of the 2 findings escalated cross-department to
`senior-security-officer`:

- **CER-GSL-002** (unsafe `Enabled` default) is **resolved this engagement** —
  confirmed Medium, operator approved flipping to `Disabled`/opt-in directly in
  the security officer's session, implemented by `dataflow-architect` under a
  security-issued spec, independently verified by both the security officer and
  `dataflow-lead`. See "Escalation resolution — CER-GSL-002" above.
- **CER-GSL-001** (permission-gate bypass in code-mode script callbacks)
  **remains open** — confirmed High by two independent specialists
  (`concurrency-engineer` and `senior-security-officer`), architecture-level fix
  needed, not verifiable in this sandbox (network-gated v8 build dependency).
  CER-GSL-002's fix narrows exposure (the runtime is no longer on-by-default)
  but does not touch the underlying bypass — it reawakens the moment an
  operator opts the runtime back on. Awaits an operator decision on when/who
  picks up the architecture-level remediation.

All changes across this engagement (10 modified source files + 4 new report
docs) remain **uncommitted**, per instruction to every specialist not to touch
git history.

Nothing has been committed. All fixes remain as uncommitted working-tree
changes pending the operator's go-ahead.
