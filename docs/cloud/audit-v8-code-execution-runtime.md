# Audit — v8 Code-Execution Runtime & Gating (delta-only)

Lens: resource-lifecycle + dataflow-concurrency + blast-radius (Concurrency &
Pipeline Engineer slice). Authority: **audit + localized-fix** on the working
tree. Builds on `docs/cloud/00-orientation.md` and `99-master-report.md`
(Cluster A theme kept in view). IDs: `CER-GSL-NNN`.

Scope: **only** the new "code execution runtime (v8)" feature and its gating,
across the 8 new commits `713f1eef2..9d9df730f` (2026-07-05). The
session-resume-paging hunks that co-touch `agent.rs`, `types.rs`, the server
routes, and the test SessionConfig literals are a **sibling engagement
(dataflow-architect)** and are reviewed by-hunk only, not owned here.

Confidence calibration per `evidence_discipline.md`: **Confirmed** = a quoted
`file:line` actually read; else capped at **Likely/Plausible**. Severity is
independent of confidence.

---

## 1. What the delta actually does

Three commits build the feature:

- `8f029d770` "update execution manager" — `DEFAULT_MAX_SESSION 100→5`
  (`execution/manager.rs:15`); threads the resolved runtime into the desktop
  `AgentManager` (`:79`); workspace/README version churn `1.40.0→0.0.5`.
- `17611155e` "Add code execution runtime configuration - v8 option on loading"
  — the `CodeExecutionRuntime{Enabled,Disabled}` config enum + resolver
  (`config/base.rs:90-110,1234-1247`), the `ExtensionManager` gate + persistence
  bookkeeping, the `AgentConfig` field/builder, subagent inheritance, and the
  desktop `CodeExecutionRuntimeSection` control.
- `9d9df730f` "Fix code execution runtime gating gaps found in ultrareview" —
  CLI fail-fast on `--with-builtin code_execution` while disabled
  (`gosling-cli/.../builder.rs:414-431`); a clear "not available in this build"
  error when `code-mode` isn't compiled (`extension_manager.rs:1015-1021`);
  `reply_parts.rs` now gates code-mode disclosure on **runtime AND extension**;
  persistence/resume regression tests.

The v8 runtime itself (`pctx_code_mode` crate, deno_core/v8, and
`platform_extensions/code_execution.rs`) **predates this delta** — the delta adds
the enable/disable *gating and config* layer around a pre-existing capability.

**Verification (this environment):**
`cargo check -p gosling --features nostr` → green (1m13s).
`cargo test -p gosling --features nostr --lib` → **1289 passed / 0 failed**
(campaign baseline was 1282; +7 new v8 config/gating tests all pass).
No working-tree changes were made by this pass (nothing met the eligible-fix bar
— see §4).

---

## 2. What actually gates a v8 invocation (independent trace)

The runtime toggle gates **whether the `execute_typescript` tool is registered
at all**, in four checkpoints, all confirmed:

1. `ExtensionManager::add_extension` — if `normalized_name == "code_execution"`
   and `!runtime.is_enabled()`, the extension is refused (config stashed in
   `runtime_blocked_extensions` for honest persistence) — `extension_manager.rs:1002-1013`.
2. CLI builder fail-fast for an explicit `--with-builtin code_execution` while
   disabled — `gosling-cli/src/session/builder.rs:414-431`.
3. `reply_parts.rs:155-159` — code-mode prompt disclosure requires
   `is_code_execution_runtime_enabled() && is_extension_enabled(...)`.
4. Persistence: `extension_configs_for_persistence` includes blocked configs (so
   re-enabling restores them) while `get_extension_configs` does not (honest
   active view) — `agent.rs:949-955,1374-1380`; resume regression test at
   `agent.rs:3724+`.

**Gating completeness — Confirmed good.** Every *production* agent-construction
entrypoint threads the resolved runtime setting:
- CLI `Agent::new` → `.with_code_execution_runtime(config.resolve_…())` `agent.rs:318-325`
- Desktop `AgentManager::instance` `execution/manager.rs:79`
- ACP server `acp/server.rs:960`
- Subagents inherit the parent's value `summon.rs:983,1400`

The only non-threading `AgentConfig::new` site outside tests is
`gosling-cli/src/scenario_tests/scenario_runner.rs:205`, which is `#[cfg(test)]`
(mock provider, TempDir) — **not a production path** (Confirmed via
`scenario_tests/mod.rs:1` `#[cfg(test)]`). No fail-open-to-Enabled gap exists in
a real path.

**Config default posture — Confirmed.** `CodeExecutionRuntime` derives
`#[default] Enabled` (`config/base.rs:92-97`); `resolve_…` returns `Enabled` when
the key is unset and **fails closed to `Disabled` on a parse error**
(`config/base.rs:1234-1247`). Fail-closed-on-garbage is the correct half; the
**unset→Enabled** default is the policy question (CER-GSL-002).

---

## 3. Findings

### CER-GSL-001 — Code-mode callbacks bypass the permission inspector + PreToolUse hooks *(High / blast-radius; CONFIRMED mechanism; ESCALATE-SECURITY; cross-refs Cluster A CTR-GSL-001)*

**Mechanism.** When the model calls `execute_typescript`, the TypeScript it runs
can call back into *any* Gosling tool via code-mode callbacks. Each callback is
dispatched through `manager.dispatch_tool_call(&ctx, tool_call, token)` —
`platform_extensions/code_execution.rs:353-354` — which is
`ExtensionManager::dispatch_tool_call` (`extension_manager.rs:1822`), the **raw**
dispatch. The permission inspector and confirmation flow run at the **Agent**
level, upstream of dispatch (`agent.rs:2211` `inspect_tools`, `:2220`
`process_inspection_results_with_permission_inspector`), and the PreToolUse hooks
live in `Agent::dispatch_tool_call` (`agent.rs:989-1003,1052`). The code-mode
callback path enters `ExtensionManager::dispatch_tool_call` directly, so nested
tool calls made from inside a script are **not** permission-inspected and do
**not** fire PreToolUse hooks.

**Blast radius.** In a *hardened* config (`GoslingMode = Approve/SmartApprove`)
where a direct `developer.shell` call would prompt, the same shell command
issued from inside `execute_typescript` runs with **no per-call confirmation**.
The human approving `execute_typescript` sees a blob of TypeScript, not the
`rm -rf`/`curl | sh` inside it — this is the Cluster A "args hidden before
approval" problem compounded: one approval of one opaque tool authorizes an
unbounded, ungated sequence of real tool calls. This delta's **default-Enabled**
posture (CER-GSL-002) makes the `execute_typescript` tool present by default,
turning a latent bypass into a **default-reachable** one.

**Provenance.** `code_execution.rs` predates this delta, so the delta did not
*introduce* the bypass — but it did make it default-reachable, and the
"gating-gap fix" (9d9df730f) closed the enable/disable-toggle gaps **without**
closing this deeper one. This is the same class as Cluster A `CTR-GSL-001` (ACP
entrypoint calls raw `dispatch_tool_call`).

**Disposition — NOT blind-fixed.** Routing code-mode callbacks through the
permission/inspection pipeline is a **feature/architecture change** (the entire
point of code-mode is to batch many tool calls inside one script; per-callback
confirmation changes its semantics and UX) with high regression surface that
cannot be integration-tested here (code-mode is network-gated, §5). Per the
`repair-defect-priority` eligibility bar this is dispositioned, not patched.
**Recommended owner:** senior-security-officer + dataflow-architect. **Options to
evaluate:** (a) run each callback dispatch through the same permission-inspector
verdict the Agent loop applies, at least for `Shell`/`Write` tool categories;
(b) subject the code-mode callback registry to a per-session allowlist of tools
the operator has already approved; (c) if code-mode is intended to be a trust
boundary of its own, gate the *`execute_typescript` tool itself* behind an
explicit, non-Auto confirmation and document that approving it grants blanket
tool access. **Proving test:** a `SmartApprove`-mode session where a script does
`developer.shell("id")` must produce a confirmation request or a denial, not a
silent execution.

### CER-GSL-002 — Code-execution runtime defaults to Enabled *(Medium / unsafe default; CONFIRMED; ESCALATE-SECURITY; mirrors Cluster A default=Auto)*

**Mechanism.** `#[default] Enabled` (`config/base.rs:92-97`) and
`resolve_gosling_code_execution_runtime` returning `Enabled` on
`NotFound` (`config/base.rs:1237`); `AgentConfig::new` also defaults the field to
`Enabled` (`agent.rs:193`); the desktop control displays `Enabled` for absent
config (`CodeExecutionRuntimeSection.tsx:45`). So a fresh install exposes the v8
`execute_typescript` capability with no operator action.

**Why it matters.** This is the exact Cluster A pattern the master report calls
ship-gating: a powerful control ships **on/permissive by default**. Combined with
default `GoslingMode = Auto` (auto-approve) and CER-GSL-001, the default install
can execute model-authored TypeScript — and any tool it calls — with no gate.

**Disposition — NOT blind-fixed (policy).** Flipping a product default trust
posture is a maintainer decision, exactly as the campaign log dispositioned
"Default GoslingMode = Auto" and "prompt-injection scanner default-off"
(`repair-campaign-log.md` §Deferred). **Recommended:** default `Disabled`, or
require explicit opt-in the first time `execute_typescript` would be offered.
**Owner:** human maintainer + senior-security-officer. **Proving test:** a config
with `GOSLING_CODE_EXECUTION_RUNTIME` unset must not register `code_execution`
(assert `execute_typescript` absent from the tool list / system prompt).

### CER-GSL-003 — `DEFAULT_MAX_SESSION 100→5` amplifies MCP subprocess churn *(Low / resource-lifecycle blast-radius; CONFIRMED; disposition to maintainers)*

**Mechanism.** `execution/manager.rs:15` drops the in-memory session LRU
capacity from 100 to 5. On the 6th concurrently-active session, the LRU evicts
the least-recently-used `Agent`; eviction drops that agent's `ExtensionManager`,
which (per `audit-resource-lifecycle.md` §2) tears down its MCP stdio
subprocesses via `kill_on_drop`. Re-accessing an evicted session reconstructs the
agent and **re-spawns** its MCP extension subprocesses.

**Why it matters (blast radius, not a correctness bug).** For an operator
juggling >5 sessions (desktop/server multi-session), this turns steady-state into
a spawn/teardown treadmill of extension subprocesses — extra process churn,
re-initialization latency, and repeated `configure_subprocess` cost. No data loss
(sessions are disk-persisted; the LRU holds only the live agent). It is a
deliberate footprint-reduction tuning choice consistent with the README's "bound
… with LRUs" narrative.

**Disposition — NOT fixed (tuning decision).** The right value is a
product/capacity call, not a bug. **Recommended:** make it configurable
(`GOSLING_MAX_SESSIONS`-style) and/or pick a middle value (e.g. 16–32) that
bounds memory without thrashing extension lifecycles; measure eviction rate under
a realistic multi-session desktop workload. **Owner:** maintainer / dataflow-lead.

### CER-GSL-004 — Desktop restart-required notice is not screen-reader announced *(Low / a11y; Plausible; disposition to design-webapp)*

`CodeExecutionRuntimeSection.tsx:133-139` renders the restart-required copy
without `aria-live`, so a screen-reader user toggling the runtime is not told the
change needs a restart. Consistent with the prior Cluster A desktop-a11y findings
(WEB-GSL-00x). TS not runnable here (§5) — capped at Plausible. **Recommended:**
`role="status"`/`aria-live="polite"` on the notice. **Owner:** design-webapp.

---

## 4. Explicit non-findings (checked and held)

- **v8 runtime resource lifecycle — verified good.** `run_in_deno_runtime`
  (`code_execution.rs:294-332`) bounds every execution with the extension timeout
  **and** cancellation, cancels the child `dispatch_token` on timeout/cancel so an
  in-flight nested tool call is told to stop, and (per the file's own comment +
  the `real_v8_hung_script_times_out_and_frees_the_runtime` test) releases the
  process-wide V8 mutex so one hung script can't wedge other sessions. The v8
  "runtime" is **in-process** (a Deno isolate under a process-wide mutex), not an
  OS subprocess — there is no runtime *process* to leak or orphan. No
  unbounded-spawn: concurrent executions are bounded by active sessions and
  serialized by the V8 mutex, with the timeout wrapping mutex acquisition too.
- **Config parse fails closed.** Invalid `GOSLING_CODE_EXECUTION_RUNTIME` →
  `Disabled` with a warning (`config/base.rs:1240-1245`; test
  `test_code_execution_runtime_invalid_value_fails_closed`).
- **Persistence/resume does not resurrect a disabled runtime.** Regression test
  `disabled_code_execution_runtime_does_not_resurrect_persisted_extension_on_resume`
  (`agent.rs:3724+`) and `reply_parts.rs:155-159` double-gate.
- **`runtime_blocked_extensions` two-Mutex non-atomicity is inert.** The
  `remove`-from-blocked at `extension_manager.rs:1193-1196` can only run for a
  successful `code_execution` add, which requires `runtime.is_enabled()`; since the
  runtime field is immutable per process, a `code_execution` that reached the
  success path was **never** blocked in this process, so the remove is a no-op and
  the "momentarily in neither map" window is unreachable. No race.
- **Subagent runtime inheritance.** Subagents inherit the parent's runtime value
  (`summon.rs:983,1400`) — the disable propagates. (They still run forced-Auto per
  Cluster A, which is a pre-existing, separately-dispositioned issue.)
- **Supply chain — clean for this delta.** No new dependency was introduced for
  the v8 feature: `deno_core`, `v8`, `pctx_code_mode`,
  `pctx_code_execution_runtime`, `pctx_config`, `pctx_registry`, `pctx_codegen`
  all exist in `713f1eef2^:Cargo.lock`. The Cargo.lock delta is the workspace
  version bump `1.40.0→0.0.5` only.
- **README execution-manager claim.** The new "Memory & Event Bus Bounds /
  bound … with LRUs" narrative is consistent with the `DEFAULT_MAX_SESSION`
  reduction; no contradicting claim found. (The `v0.0.5` vs `1.40.0` version churn
  is the pre-flagged human-owner README-identity item, not a v8 defect.)
- **Providers `chatgpt_codex.rs` / `gcpauth.rs` touches** are `#[cfg]` test-import
  gating only — no runtime behavior, not v8-related.

---

## 5. Validation Limits

- **`code-mode` / v8 not compiled here.** Per `README.md`, the `code-mode`
  feature's `v8-goose` static-lib download is blocked by this environment's
  network policy. All `#[cfg(feature = "code-mode")]` code (`code_execution.rs`,
  the code-mode tests in `agent.rs`) was **read but not compiled/executed**.
  CER-GSL-001's mechanism is source-confirmed but **not runtime-reproduced**.
- **`-p gosling-cli` not buildable** (same static-lib network gate; pre-existing
  environment limit). The CLI fail-fast guard (`builder.rs`) was read, not
  compiled.
- **TS not runnable.** `ui/desktop/node_modules` is a partial install —
  `typescript` and `vitest` packages are absent and `pnpm install` needs network.
  `tsc --noEmit` and `vitest` could not run. `CodeExecutionRuntimeSection.tsx` and
  its `.test.tsx` were read and are statically consistent (asserted values match
  the component); CER-GSL-004 is therefore Plausible, not Confirmed.
- **Pre-existing dirty tree.** At engagement start the working tree already had 6
  uncommitted modifications (`documentation/…goose-compat*`, `mcp-servers.ts`,
  `skills.ts`, `ui/desktop/src/hooks/useChatSession.ts`) — none in the v8 slice,
  none touching `code_execution`/v8 (verified by grep). This pass added **zero**
  changes.

---

## 6. Cross-cutting / escalation flags

- **CROSS-CUTTING (dataflow-architect):** the session-resume-paging hunks that
  co-touch `agent.rs` (`get_session(…,false)`, `is_first_turn`,
  `get_session_for_compacted_resume`, `spawn_session_rollup`), `agents/types.rs`
  (`compacted_context`/`tail_limit`), the server routes, `session/mod.rs`, and the
  test `SessionConfig` literals. Reviewed by-hunk only; not owned or modified here.
- **ESCALATE-SECURITY:** CER-GSL-001 (code-mode callback permission bypass) and
  CER-GSL-002 (default-Enabled posture) — both are policy/architecture calls, not
  localized coding bugs. Route to senior-security-officer; CER-GSL-001 also to
  dataflow-architect. Both feed the still-open Cluster A "enforce-the-controls"
  follow-up.
