# Gosling audit — orchestration + contracts

**Date:** 2026-08-15  
**Target:** `/Users/eric/Work/vscode/forked/gosling`  
**HEAD:** `073d19428509ea6eb317924b1856a1fe7e9002c8`  
**Authority:** `read_only` / static-read. Source was not modified. No live provider, MCP inspector, or fake-agent break-it run was executed.  
**Lenses (this report):** `audit-agent-orchestration-code` (AOC), `audit-mcp-server` (MCP, limited apply), `audit-pipeline-externalapi` (XAPI), `audit-contract-internalapi` (IAPI), `audit-contract-crossrepo` (XREP, limited apply).

The supplied prompt is treated as a draft. The intended mission is preserved: re-verify current source at this HEAD and write one combined report covering the assigned taxonomies. Review is expanded to adjacent failure mechanisms implied by the seams (review-check empty-success, subagent Auto-mode escape, tagteam argv leakage, Goose catalog float).

---

## Limited-apply scopes (binding)

| Lens | Scope as assigned | What this report did **not** treat as in-scope |
|---|---|---|
| **MCP** (`audit-mcp-server`) | `crates/gosling-mcp` plus **host MCP wiring**. Gosling is primarily an MCP **host**. | Not a standalone published MCP-server product audit of every third-party connector a user may add. Host acceptance/quarantine of untrusted servers routes to `audit-security-llm`. |
| **XREP** (`audit-contract-crossrepo`) | **Rust ↔ Electron ACP/SDK** and **Goose catalog fallback** only. Not a sibling-repo contract. | No second repository was opened. Goose is a live HTTP catalog (`https://goose-docs.ai/...`), not a pinned sibling checkout. |

`crates/gosling/src/tagteam/` is **feature-gated** (`tagteam-workflow`, default **off**; `crates/gosling/src/lib.rs:42-43`, `crates/gosling/Cargo.toml:22`). Phase-1 steward/reducer/contracts are source-audited as future surface, not as default-binary behavior. Production multi-agent behavior at this HEAD is:

1. `TagteamProvider` shelling out to the external `tagteam` CLI.
2. `gosling review` (Rust-driven check + per-file main pass).
3. `summon::delegate` / `orchestrator` platform extensions (in-process subagents).

---

## Phase A — Architecture reconnaissance

### A1. Role / mode matrix

| Role / mode | Documented intent | Adapter / construction | Filesystem / shell / network | Input contract | Output contract | Failure policy | Fallback |
|---|---|---|---|---|---|---|---|
| **Parent agent** (`GoslingMode::{Auto, SmartApprove, Approve, Chat}`) | Interactive / headless session; tools gated by mode | `Agent` + selected `Provider` | Mode-dependent. CLI adapters: Auto → Claude `--dangerously-skip-permissions` (`claude_code.rs:439-443`); Codex `--yolo` (`codex.rs:131-134`); Chat → Codex `--sandbox read-only` | System prompt + conversation + tools | Streaming messages + tool calls | Provider retry (transient); max turns 1000 | Compaction / summarizer stub |
| **Ad-hoc delegate** (`summon` `delegate` without `source`) | Isolated subagent; default **no** extensions | `run_subagent_task` + `TaskConfig`; **forced `GoslingMode::Auto`** (`summon.rs:1097-1118`) | Parent-extension subset only if `extensions` requested; working dir must stay under parent | `instructions` + optional `context` | Last-message text, prefixed with resolved authority | Failed extensions prefixed, not aborting (`subagent_handler.rs:90-105`) | Empty tool surface if extensions fail |
| **Source-based delegate** (`.gosling/agents`, `.agents/agents`, `.claude/agents`) | Named role from repo/global markdown | Same as ad-hoc; `capabilities.version=1` allowlist (`summon.rs:122-194, 1264-1269`) | Repo file can grant any **parent-enabled** extension | Agent body as instructions | Same as ad-hoc | Same | Missing source → typed error |
| **Review check subprocess** (`gosling review` orchestrator) | JSON-only check; docs claim `tools:` allowlist | `gosling run --no-session --quiet --no-profile` (`worker.rs:77-90`) | **No profile extensions, no builtins** → no developer/shell on the default path | Diff + check body; JSON schema in prompt | First `{...}` extracted as `{"findings":[...]}` | Failed check → **empty findings** + stderr warning (`orchestrator.rs:76-81, 138-142`) | Continue other checks |
| **Review main-pass file worker** | Per-file correctness | Same worker command | Same: tool-free | File-scoped diff | Same JSON extract | Same empty-on-fail | Continue other files |
| **Review legacy** (`--no-orchestrate`) | Main agent dispatches checks via `delegate` | In-process `session.headless` with `developer` + `summon` (`handler.rs:210-222`) | Full developer tools | Assembled prompt including check table | Free text / JSONL depending on path | Model-driven | Orchestrator used only for `--checks-only` |
| **Tagteam profiles** `coding-adversarial` / `relay` / `supervisor-worker` | External tagteam CLI runs coder/reviewer/scout/supervisor | Hardcoded argv (`tagteam.rs:38-72, 176-186`); **Auto only** (`tagteam.rs:198-202`) | External CLI + vendor CLIs; inherits process env; prompt on argv | Concatenated system + conversation as last argv | `TagteamFinalRun` JSON **or** raw stdout | JSON parse ⇒ success even if `exit_code != 0` (`tagteam.rs:235-237`) | Non-JSON + nonzero ⇒ `ExecutionError` |
| **Tagteam steward** (feature `tagteam-workflow`) | Phase-1 read-only status/plan/findings | `StewardCapabilityPolicy::phase_one` (`policy.rs:41-75`) | Deny shell/file/delegate/widen | Typed contracts v1 | Validated pages | Not wired in default binary | N/A |
| **MCP host** | Launch/call user + builtin MCP servers | `McpClient` / `extension_manager` | Stdio children get **minimal env** (`extension_manager.rs:390-435`) | JSON-RPC tools | `CallToolResult` | 5s close timeout (`mcp_client.rs:45-49`) | Failed extension load is per-session |
| **computercontroller** (builtin MCP server) | Web scrape, cache, docs, **script/UI automation** | `crates/gosling-mcp` stdio / in-process | `automation_script` / `computer_control` execute model scripts; `web_scrape` public-HTTP only | Tool schemas via `rmcp` | Text + cache paths; resources enabled | HTTP fail → `INTERNAL_ERROR` | Headless: `computer_control` route removed (`computercontroller/mod.rs:599-602`) |
| **autovisualiser** | Chart UI resources | Same crate | Read/additive visualization | JSON data | MCP Apps UI resources | Schema validation on `data` | N/A |

Documented vs constructed mismatches that the matrix itself surfaces: review `tools:` field; review “tool-free prompt” vs legacy `developer`; subagent Auto vs parent approval mode; tagteam Auto-only vs Gosling SmartApprove.

### A2. Trust-boundary map

```
model output
  ├─ tool call args ──► permission_judge / inspectors ──► extension handler / MCP
  │                      (parent session; skipped for Auto subagents)
  ├─ review JSON ──► extract_json_object (first `{`) ──► Finding[] ──► stdout JSONL
  │                   (no severity arithmetic gate; fail ⇒ [])
  ├─ tagteam stdout ──► serde TagteamFinalRun? ──► format_final_run text
  │                      (verdict not recomputed; JSON wins over exit code)
  ├─ delegate result ──► last text + "[Resolved delegate authority: ...]"
  │                      (lossy: return_last_only=true)
  ├─ provider stream ──► format parsers (OpenAI/Anthropic/…) ──► Message
  │                      (finish_reason / [DONE] tracked in openai.rs)
  └─ Goose catalog JSON ──► normalizeGoose* ──► docs site install list
                             (no pin; first non-empty catalog wins)
```

Arrows **without** a validator that recomputes a decision from evidence: review empty-on-fail; tagteam `verdict`; first-JSON extraction; Goose fallback selection.

### A3. Run-state transition map

| Surface | States witnessed | Write sites | Crash / early-return risk |
|---|---|---|---|
| **Agent session** | created → streaming → tool ops → compaction → persisted messages | `SessionManager`; stream checkpoint every 250ms (`agent.rs:86, 2537-2546`); `persist_extension_state` | Tool op drop marks in-doubt (`agent.rs:106-120`) |
| **Delegate / background task** | spawned → running → completed/failed/panicked map | in-memory `background_tasks` / `completed_tasks` (`summon.rs:1406+`) | Process death loses in-flight task map (not a run dir) |
| **Review CLI** | discover → parallel checks + main → emit JSONL | **no run dir / no status file**; only stderr progress + stdout findings | Failed check leaves no persisted “blocked” item |
| **TagteamProvider** | spawn CLI → wait `output()` → format | artifacts only if tagteam writes `run_dir` (passed through as text) | Gosling writes no status of its own |
| **Tagteam-workflow (feature off)** | Configured…Cancelled (`reducer.rs:16-34`) | store/reducer designed with digests | Not in default binary |

---

## Findings (severity order)

### AOC-GOS-001: Delegated subagents are forced into Auto and skip parent approval

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security  
Taxonomy: AOC-001

Evidence:
- `crates/gosling/src/agents/platform_extensions/summon.rs:1097-1118` — comment and construction: “Subagents must use Auto until get_agent_messages forwards ActionRequired… any mode that requires approval will hang”; `AgentConfig::new(..., GoslingMode::Auto, ...)` and `create_session(..., GoslingMode::Auto)`.
- `crates/gosling/src/providers/claude_code.rs:439-443` — Auto ⇒ `--dangerously-skip-permissions`.
- `crates/gosling/src/providers/codex.rs:131-134` — Auto ⇒ `--yolo` (“dangerously-bypass-approvals-and-sandbox”).
- Same Auto force in async path (`summon.rs:1496-1511`) and orchestrator-started agents (`orchestrator.rs:416-422`).

Observed behavior:
- A parent session in `SmartApprove` / `Approve` can `delegate` work that runs as Auto. If that subagent is given `developer` (source policy or explicit `extensions`), vendor CLIs and local tools execute without the parent’s confirmation path.

Expected boundary:
- Child authority ⊆ parent mode. Approval-required modes must either relay confirmations or refuse write/shell tools.

Failure mechanism:
- Hang-avoidance is implemented by widening authority instead of implementing the relay.

Break-it angle:
- Parent in Approve; `delegate(source: "reviewer")` where the agent file lists `capabilities.extensions: ["developer"]` and the parent has developer enabled. Static: child is Auto.

Impact:
- Operator believes writes need approval; delegated work does not.

Operational impact:
- Blast radius: Workflow. Side-effect class: file + process. Reversibility: compensatable. Operator visibility: silent (subagent Auto is not surfaced as a mode change). Rerun safety: unsafe.

Adjacent failure modes:
- AOC-GOS-004 (repo file grants extensions); AOC-GOS-010 (env inheritance).

Recommended mitigation:
- Remediation patterns: relay ActionRequired to parent; or deny write extensions unless parent is Auto.
- Minimal repair: if parent mode ≠ Auto, reject `developer` / shell-capable extensions on the delegate.
- Behavior test: parent Approve + delegate with developer → error or confirmation event, never Auto+yolo.

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Cost drivers: modules, tests. Nominal implementation agent: claude.

Validation:
- Fake-agent: parent Approve, delegate developer shell; assert no spawn / confirmation forwarded.

Non-goals:
- Do not change Auto semantics for the parent session itself.

---

### AOC-GOS-002: Tagteam puts the full prompt (system + conversation) on argv

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security  
Taxonomy: AOC-003

Evidence:
- `crates/gosling/src/providers/tagteam.rs:145-186` — `build_prompt` concatenates system + conversation; `build_args` ends with `args.push(prompt.to_string())`.
- Tests pin this: `tagteam.rs:441-459` expect the prompt as the last argv token.
- `tagteam.rs:204-217` — `Command::new` + `configure_subprocess`; no stdin prompt path.

Observed behavior:
- `ps` / auditd / shell history can see repo instructions, conversation, and anything the parent stuffed into the system prompt (including secrets that landed in context).

Expected boundary:
- Prompts and secrets travel on stdin or a private fd, never argv.

Failure mechanism:
- The tagteam CLI is invoked like a one-shot `tagteam --mode … <prompt>`.

Break-it angle:
- Session that includes an API key in a pasted log; `ps -ww` on the tagteam child.

Impact:
- Prompt/secret leakage to every local observer of process listings.

Operational impact:
- Blast radius: Local / Repo. Side-effect class: process + user-visible. Reversibility: irreversible (once listed). Operator visibility: silent. Rerun safety: unsafe.

Adjacent failure modes:
- AOC-GOS-010 (full env also inherited); XAPI-GOS-001 (no timeout on the same spawn).

Recommended mitigation:
- Remediation patterns: stdin prompt; redact argv in logs.
- Minimal repair: write prompt to a 0600 tempfile or stdin; argv only flags.
- Behavior test: `build_args` / spawn inspect shows no prompt body.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: modules, tests. Nominal implementation agent: codex.

Validation:
- Unit: `build_args` does not contain prompt text; integration: `/proc/<pid>/cmdline` lacks the conversation.

Non-goals:
- Do not change tagteam profile model IDs in this slice.

---

### AOC-GOS-003: A failed review check is emitted as “no findings”

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Failsafe  
Taxonomy: AOC-006, AOC-026, AOC-005

Evidence:
- `crates/gosling-cli/src/commands/review/orchestrator.rs:76-81` — “A failed check (subprocess error, turn-limit exhaustion, malformed JSON) yields an empty findings list… a single broken check must never block the rest of the review.”
- `orchestrator.rs:138-142` and `341-346` — `Err` → `results[idx] = Vec::new()` (main pass same).
- `orchestrator.rs:644-651, 671-709` — parse requires a JSON object; first `{` wins; no retry.
- `orchestrator.rs:731-744` — emit path only prints findings; no `degraded` / `check_failed` record.

Observed behavior:
- Operator (or CI consuming JSONL) cannot distinguish “check ran and found nothing” from “check crashed / truncated / returned prose”. Both are zero lines for that check.

Expected boundary:
- Advisory-check failure must be an explicit degraded/blocked item with owner + reason. Empty findings only after a validated `{"findings":[]}`.

Failure mechanism:
- Continue-on-error is implemented by substituting the success shape.

Break-it angle:
- Check subprocess prints `Sure!` with no JSON; or exits 1 after turn limit. Review stdout is empty; exit of `gosling review` is still 0 (`handler.rs:266` `Ok(())`).

Impact:
- Rubber-stamp empty review; CI green on a dead check.

Operational impact:
- Blast radius: Workflow. Side-effect class: user-visible. Reversibility: compensatable. Operator visibility: log-only (stderr warning if not `--quiet`). Rerun safety: safe.

Adjacent failure modes:
- AOC-GOS-006 (first-JSON); AOC-GOS-007 (`tools:` unused); AOC-GOS-013 (no fake-agent).

Recommended mitigation:
- Remediation patterns: typed check outcome `{ok, findings}` vs `{failed, reason}`; nonzero review exit if any check failed.
- Minimal repair: emit a synthetic critical finding or a sidecar `check_status` JSONL; fail the process if any check failed unless `--allow-check-failure`.
- Behavior test: malformed JSON → not an empty success.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: modules, tests. Nominal implementation agent: codex.

Validation:
- Fake subprocess: exit 1 / prose / decoy JSON; assert review exit ≠ 0 or a `check_failed` record.

Non-goals:
- Do not serialize all checks again.

---

### AOC-GOS-004: Repo-committed agent files can grant parent extensions to a delegate

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security  
Taxonomy: AOC-021

Evidence:
- `crates/gosling/src/agents/platform_extensions/summon.rs:283-314` — discovery includes `working_dir/.gosling/agents`, `.claude/agents`, `.agents/agents`.
- `summon.rs:122-194, 1264-1269` — `capabilities.version == 1` + `extensions: [...]` becomes `role_extensions`; requested list can only **narrow**; omitted request uses the **full role list**.
- Combined with AOC-GOS-001, that role runs Auto.

Observed behavior:
- Untrusted repo content is a trust decision: a cloned repo can ship `capabilities: { version: 1, extensions: [developer] }` and, if the parent has developer enabled, the delegate gets it without an extra operator prompt.

Expected boundary:
- Repo-authored capability lists are untrusted config. Granting write/shell must be an explicit parent/operator decision, not a file in the worktree.

Failure mechanism:
- Capability policy is treated as an allowlist authored by a trusted role, but the author is the repo.

Break-it angle:
- Commit `.gosling/agents/helper.md` with developer capabilities; parent has developer on; model `delegate(source: "helper")`.

Impact:
- Hostile-repo widening of child authority (pairs with Auto).

Operational impact:
- Blast radius: Repo. Side-effect class: file + process. Reversibility: compensatable. Operator visibility: silent unless the model mentions the source. Rerun safety: unsafe.

Adjacent failure modes:
- AOC-GOS-001.

Recommended mitigation:
- Remediation patterns: ignore repo `capabilities` unless allowlisted in user config; default source-based delegates to no extensions (same as ad-hoc).
- Minimal repair: treat missing/untrusted policy as empty; require parent-session confirmation to enable developer on a source delegate.
- Behavior test: repo agent requesting developer does not receive it without confirmation.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: modules, tests. Nominal implementation agent: codex.

Validation:
- Fixture agent file in a temp repo; delegate; assert extension list empty.

Non-goals:
- Do not remove filesystem discovery of agent names/descriptions.

---

### MCP-GOS-001: computercontroller executes model-supplied scripts / UI control with no dry-run gate

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security  
Taxonomy: MCP-031, MCP-029  
Limited apply: in-tree server in `crates/gosling-mcp`; host must enable the extension.

Evidence:
- `crates/gosling-mcp/src/computercontroller/mod.rs:843-898` — `automation_script_impl` writes `params.script` to a temp file and executes it (`0o755` on Unix).
- `computercontroller/mod.rs:1082-1116` — non-macOS `computer_control_impl` runs `system_automation.execute_system_script(script)` and returns success text.
- `computercontroller/mod.rs:1014-1040, 1128-1145` — macOS path is Peekaboo CLI; missing binary triggers `brew` auto-install.
- No `readOnlyHint` / `destructiveHint` on these tools (registration is `#[tool]` name+description only, e.g. `:673-682`, `:780-794`).

Observed behavior:
- A model that can call this extension can run arbitrary shell/UI automation. Reads (`web_scrape`, `cache`, pdf/docx extract) live on the same server as destructive automation (MCP-008 mixed risk).

Expected boundary:
- Writes/exec gated (dry-run, confirmation, or separate server). Model input is not a script body for an ungated exec.

Failure mechanism:
- Product is designed as a power-user automation server; the protocol/host boundary does not split risk classes.

Break-it angle:
- `automation_script(language=shell, script="curl … | sh")` — no schema-level refusal.

Impact:
- Host-enabled extension is a full local RCE primitive for the model.

Operational impact:
- Blast radius: Local / Repo. Side-effect class: process + file + network. Reversibility: irreversible. Operator visibility: tool result only. Rerun safety: unsafe.

Adjacent failure modes:
- MCP-GOS-002 (brew install); MCP-GOS-003 (overlap).

Recommended mitigation:
- Remediation patterns: split read vs exec servers; `destructiveHint: true`; require host confirmation; dry-run flag.
- Minimal repair: do not enable `computercontroller` by default; document it as privileged; add annotations + confirmation metadata the host already knows how to honor.
- Behavior test: tool annotations + host confirmation path for `automation_script`.

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Cost drivers: modules, tests, operator_training. Nominal implementation agent: human-owner (product policy) then codex.

Validation:
- Registration snapshot includes destructive hints; host test refuses exec in Chat mode.

Non-goals:
- Do not remove document extract tools in this slice.

---

### AOC-GOS-005: Tagteam JSON stdout is success even when the run failed

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: State-Transition  
Taxonomy: AOC-005, AOC-016, AOC-022

Evidence:
- `crates/gosling/src/providers/tagteam.rs:232-237` — after `output()`, if stdout parses as `TagteamFinalRun`, return `Ok(format_final_run(...))` **before** checking `output.status`.
- `tagteam.rs:254-297` — `verdict` / `status` / `exit_code` / `degraded` are formatted into prose, not turned into `ProviderError`.
- `tagteam.rs:411-412` — `Usage::default()` always.

Observed behavior:
- `verdict=fail`, `degraded=true`, `exit_code=1` still look like a successful provider turn. The parent agent continues as if the orchestrator answered.

Expected boundary:
- Gate is arithmetic over evidence (`verdict`/`status`/`exit_code`/`degraded` + nonempty summary). Failed/degraded runs are typed errors or a structured tool result the loop cannot treat as a normal completion.

Failure mechanism:
- “Got JSON” is treated as “got a successful completion.”

Break-it angle:
- Fake `tagteam` prints `{"verdict":"pass","status":"passed",...}` on failure, or `{"verdict":"fail",...}` with exit 1. Gosling accepts both as `Ok`.

Impact:
- Operator/model may proceed after a failed multi-agent run; cost/usage invisible.

Operational impact:
- Blast radius: Workflow. Side-effect class: user-visible. Reversibility: compensatable. Operator visibility: buried in assistant text. Rerun safety: unknown.

Adjacent failure modes:
- AOC-GOS-008, AOC-GOS-011.

Recommended mitigation:
- Remediation patterns: map non-pass / nonzero exit / `degraded` to `ProviderError::ExecutionError` (or a first-class degraded event).
- Minimal repair: fail the stream unless `status`/`verdict` are an allowlisted success pair **and** exit is 0.
- Behavior test: fixture JSON fail ⇒ `Err`.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: modules, tests. Nominal implementation agent: codex.

Validation:
- Table-driven `TagteamFinalRun` fixtures for pass/fail/degraded/bad-exit.

Non-goals:
- Do not reimplement tagteam’s internal gate inside Gosling.

---

### AOC-GOS-006: Review parser takes the first JSON object and does not retry

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Input-Output-Path  
Taxonomy: AOC-004

Evidence:
- `crates/gosling-cli/src/commands/review/orchestrator.rs:644-651` — `extract_json_object` then `serde_json::from_str`.
- `orchestrator.rs:671-709` — first `{` … matching `}` at depth 0.
- Tests (`orchestrator.rs:879-884`) **require** chatter-before-JSON to succeed.

Observed behavior:
- A decoy `{"findings":[]}` before the real object wins. Invalid JSON after fence-strip is a hard fail → empty findings (AOC-GOS-003).

Expected boundary:
- Schema-validated last object, or reject if more than one top-level object / if leading prose exists. Retry once on malformed output, then typed failure.

Failure mechanism:
- First-JSON-in-prose extraction is the success path.

Break-it angle:
- `{"findings":[]} \n {"findings":[{high…}]}` → empty.

Impact:
- Findings dropped or decoyed.

Operational impact:
- Blast radius: Workflow. Side-effect class: user-visible. Reversibility: compensatable. Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- AOC-GOS-003.

Recommended mitigation:
- Remediation patterns: require exact JSON (no leading chatter) or parse all objects and refuse ambiguity.
- Behavior test: decoy-first JSON fails closed.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: tests. Nominal implementation agent: codex.

Validation:
- Existing chatter test inverted or supplemented with decoy-first.

Non-goals:
- Do not add a second live model retry by default.

---

### AOC-GOS-007: Check `tools:` is documented as construction, implemented as prompt text

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture  
Taxonomy: AOC-001, AOC-024, IAPI-009

Evidence:
- `crates/gosling/src/checks/mod.rs:56-60` — `tools: None` “means the subagent inherits the agent's full toolset.”
- Orchestrated workers: `worker.rs:77-90` `gosling run --no-profile` and no `--with-builtin` → `builder.rs:404-405` loads **no** configured extensions.
- Legacy path: `handler.rs:210-222` forces `developer` + `summon` regardless of `tools`.
- `default_review_prompt.md:89-95` — “Treat the per-check `tools` column … as informational guidance … not as an extensions filter.”

Observed behavior:
- Default orchestrator: every check is tool-free (stricter than `tools: None`).
- Legacy: full developer, ignore allowlist.
- Neither path constructs the Amp-shaped allowlist.

Expected boundary:
- Documented `tools` is either enforced at construction or the docs/schema drop the claim.

Failure mechanism:
- Prompt contract and construction contract diverged when the Rust orchestrator was added.

Break-it angle:
- Check with `tools: [Read, Grep]` cannot grep; check with `tools: []` on `--no-orchestrate` still has developer.

Impact:
- Checks that need verification tools silently under-fire; operators who trust `tools:` for containment are wrong on the legacy path.

Operational impact:
- Blast radius: Workflow. Side-effect class: none (orchestrator) / file (legacy). Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Adjacent failure modes:
- AOC-GOS-003.

Recommended mitigation:
- Remediation patterns: construct a tool allowlist, or delete the field from the public contract.
- Minimal repair: document orchestrator as tool-free; reject `tools:` that request unavailable tools; legacy path must not load `developer` unless listed.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: modules, docs, tests. Nominal implementation agent: codex.

Validation:
- Worker spawn argv/env snapshot; assert no developer unless allowlisted.

Non-goals:
- Do not implement Amp tool-name mapping in this slice unless construction is the chosen fix.

---

### AOC-GOS-008: Tagteam profiles and 1M context are hardcoded; discovery is a stub

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture  
Taxonomy: AOC-015, AOC-002, AOC-009

Evidence:
- `crates/gosling/src/providers/tagteam.rs:38-72, 124-142, 374-389` — three profiles; `fetch_supported_models` returns that list; `TAGTEAM_CONTEXT_LIMIT = 1_000_000`.
- `TAGTEAM_DOC_URL = "https://github.com/"` (`tagteam.rs:26`).
- Role argv (`-mc`, `--supervisor`, …) is stringly copied with no vendor-version gate.

Observed behavior:
- Model picker cannot refresh from the installed tagteam CLI. Context budget for packet assembly believes 1M tokens.

Expected boundary:
- One source of truth (tagteam `--help` / capabilities JSON) or an honest “static profile” label; context limit from the child models, not a million-token fiction.

Failure mechanism:
- Provider trait’s discovery methods are implemented as constants.

Break-it angle:
- User installs a tagteam that renamed `--scout`; Gosling still passes the old flags.

Impact:
- Stale profiles; context-manager under-compacts if it trusts 1M.

Operational impact:
- Blast radius: Workflow. Side-effect class: none. Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Probe `tagteam --json --help` / capabilities; fall back to the static list **marked degraded**.
- Test: unknown CLI version ⇒ visible degrade, not a hung spawn.

Implementation assessment:
- Complexity: external_service_semantics. Cost: M. Nominal implementation agent: gpt.

---

### AOC-GOS-009: Tagteam `output()` has no timeout and materializes unbounded stdout/stderr

Severity: Medium  
Confidence: Confirmed (missing bound); runtime hang/OOM cap **Likely**  
Evidence basis: source-evidenced  
Domain: Reliability  
Taxonomy: AOC-010, AOC-022, XAPI-005

Evidence:
- `crates/gosling/src/providers/tagteam.rs:225-233` — `build_command(...)?.output().await` with no `tokio::time::timeout`.
- Contrast: HTTP `ApiClient` default 600s (`gosling-providers/src/api_client.rs:17, 248-262`); computercontroller script capture is capped at 256 KiB (`computercontroller/mod.rs:86-116`).
- `configure_subprocess` sets `kill_on_drop` (`subprocess.rs:53-58`) but `.output()` holds the child until it exits.

Observed behavior:
- A wedged or chatty tagteam blocks the provider stream and can grow memory without a Gosling-side cap.

Expected boundary:
- Wall timeout (profile already has `TimeBudget` in the unused contracts module) + capped pipes + explicit truncation marker.

Failure mechanism:
- Vendor CLI treated as a bounded HTTP client.

Break-it angle:
- Fake binary that sleeps forever / writes gigabytes to stdout.

Impact:
- Session hang; possible OOM (runtime not reproduced).

Operational impact:
- Blast radius: Service. Side-effect class: process. Reversibility: compensatable. Operator visibility: silent until cancel. Rerun safety: unknown.

Recommended mitigation:
- `timeout` + `output_capped` pattern already in computercontroller.
- Test: fake child exceeds cap/time ⇒ typed error.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal implementation agent: codex.

---

### AOC-GOS-010: Tagteam and review workers inherit the full parent environment

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security  
Taxonomy: AOC-019

Evidence:
- Tagteam: `tagteam.rs:204-210` `Command::new` + optional `PATH` only.
- Review: `worker.rs:77-90` no `env_clear`.
- Contrast (held for MCP stdio): `extension_manager.rs:390-435` `env_clear()` + allowlisted keys (`PATH`, `HOME`, `GOSLING_PATH_ROOT`, …) — **API keys not forwarded**.

Observed behavior:
- Tagteam child (and every vendor CLI it launches) sees every `*_API_KEY` in the parent. Review workers do too (needed for their provider calls, but they also see unrelated keys).

Expected boundary:
- Pass only the credentials the selected provider/profile needs.

Failure mechanism:
- Default OS env pass-through.

Impact:
- Overexposure of sibling-provider secrets to a multi-vendor orchestrator.

Operational impact:
- Blast radius: Local. Side-effect class: process. Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Reuse `minimal_child_environment` + explicit provider secret injection (same as MCP).
- Test: spawned env keys are an allowlist.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal implementation agent: codex.

---

### AOC-GOS-011: Tagteam reports zero token usage

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Operator signal  
Taxonomy: AOC-030, XAPI-015

Evidence:
- `tagteam.rs:411-412` — `Usage::default()`.
- CLI adapters similarly zero usage (`cli_common.rs:103`, `cursor_agent.rs:232`).

Observed behavior:
- Multi-model tagteam spend is invisible in Gosling usage accounting.

Expected boundary:
- Parse usage from tagteam JSON or mark usage `unknown` in the UI.

Recommended mitigation:
- Surface `unknown` rather than zeros; plumb tagteam usage if the CLI emits it.

Implementation assessment:
- Complexity: operator_ux. Cost: S. Nominal implementation agent: gpt.

---

### AOC-GOS-012: Review diffs live `git diff HEAD` with no saved baseline

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Data-Integrity  
Taxonomy: AOC-011, AOC-012

Evidence:
- `crates/gosling-cli/src/commands/review/handler.rs:88-106, 344-397` — `git diff HEAD` (or `--range`) plus synthesized untracked files; no snapshot hash.
- Parallel workers each receive the string captured at start (good for that process) but a dirty tree / concurrent user edit mid-run is not frozen on disk.

Observed behavior:
- No baseline commit/tree recorded. Re-run after the user saves a file reviews a different change. Generated files in the worktree are in the diff.

Expected boundary:
- Capture patch + path list + numstat + hash; workers read the snapshot.

Impact:
- Nondeterministic reviews; user edits mid-run change the object under review.

Operational impact:
- Blast radius: Workflow. Side-effect class: none. Reversibility: reversible. Operator visibility: silent. Rerun safety: unsafe (different input).

Recommended mitigation:
- Write a temp patch file at start; pass that to workers.
- Test: mutate worktree after capture; workers still see the snapshot.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal implementation agent: codex.

---

### AOC-GOS-013: Orchestration seams lack a fake-agent harness

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Negative-Space  
Taxonomy: AOC-020

Evidence:
- Tagteam tests: profile lookup + argv equality only (`tagteam.rs:416-498`).
- Review tests: prompt/parse/split-diff unit tests; worker pool concurrency test does **not** spawn `gosling` or a fake model (`worker.rs:110-144`).
- No fixture that drives pass-first / malformed JSON / tagteam fail JSON / scout-degrade / interrupt.

Observed behavior:
- The High findings above cannot regress-fail in CI today.

Expected boundary:
- Deterministic fake `tagteam` + fake `gosling run` covering the skill’s pass-first / malformed / timeout / interrupt list.

Recommended mitigation:
- Add `PATH`-injected fakes in `crates/gosling-cli/tests` and `crates/gosling/tests`.

Implementation assessment:
- Complexity: local_guardrail. Cost: M. Cost drivers: tests. Nominal implementation agent: codex.

---

### AOC-GOS-014: macOS/Windows SIGKILL of Gosling can orphan tagteam / vendor CLIs

Severity: Medium  
Confidence: Likely (code property Confirmed; orphan manifestation not drilled)  
Evidence basis: source-evidenced  
Domain: Reliability  
Taxonomy: AOC-028

Evidence:
- `crates/gosling/src/subprocess.rs:53-57` — “macOS has no in-process equivalent, so a hard parent SIGKILL can still orphan children.” Linux uses `PR_SET_PDEATHSIG`.
- Tagteam uses `configure_subprocess` (`tagteam.rs:205`) → new process group on Unix (`subprocess.rs:63-66`).

Observed behavior:
- `kill_on_drop` covers graceful cancel. SIGKILL of the parent leaves the process-group children.

Expected boundary:
- Document the gap; desktop already polls parent death for `goslingd` (`subprocess.rs:103-113`) — tagteam children are not covered.

Recommended mitigation:
- Same parent-death waiter around tagteam, or don’t isolate the process group for this spawn.
- Drill: authorized kill -9.

Implementation assessment:
- Complexity: cross_process_coordination. Cost: M. Nominal implementation agent: claude.

---

### MCP-GOS-002: First `computer_control` on macOS may `brew install` Peekaboo

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security  
Taxonomy: MCP-021, MCP-030

Evidence:
- `crates/gosling-mcp/src/computercontroller/mod.rs:1128-1145` — “Peekaboo not found, attempting auto-install via brew”.

Observed behavior:
- A tool call mutates the machine package set without an approval artifact.

Expected boundary:
- Install is an operator action, not a model-triggered side effect.

Recommended mitigation:
- Fail with the brew command; never auto-install from a tool handler.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal implementation agent: codex.

---

### MCP-GOS-003: `automation_script` and `computer_control` overlap as exec god-tools

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture  
Taxonomy: MCP-014, MCP-008

Evidence:
- Both execute operator-OS scripts (`mod.rs:843+` and `1082+`); descriptions both say “system automation.”
- Same server also hosts `web_scrape`, `cache`, pdf/docx/xlsx (unrelated domain + mixed risk).

Model’s-eye view:
- Two tools can run a shell. A competent model can pick either for “run this command.”

Recommended mitigation:
- One exec tool; split document/web into a read server.

Implementation assessment:
- Complexity: governance_decision. Cost: M. Nominal implementation agent: human-owner.

---

### MCP-GOS-004: Upstream HTTP failures return JSON-RPC `INTERNAL_ERROR`, not teachable tool errors

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Input-Output-Path  
Taxonomy: MCP-003, MCP-015

Evidence:
- `computercontroller/mod.rs:705-719` — fetch/network/HTTP status → `ErrorData::new(ErrorCode::INTERNAL_ERROR, "HTTP request failed with status: …")`.
- Contrast: invalid URL uses `INVALID_PARAMS` (`:694-696`).

Observed behavior:
- Model sees a protocol error, not `isError: true` with “retryable vs terminal / status 404 vs timeout.”

Recommended mitigation:
- Return `CallToolResult::error` with status class + retryable flag.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal implementation agent: codex.

---

### MCP-GOS-005: `web_scrape` size cap is applied after the body is buffered

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Reliability  
Taxonomy: MCP-016, MCP-031

Evidence:
- `computercontroller/mod.rs:1772-1776` — comment: “Not a true streaming cap.”
- `MAX_WEB_SCRAPE_BYTES = 25 * 1024 * 1024` (`:1777`); check after `content_length` and after `.text()`/`.bytes()` (`:721-766`).
- Residual DNS-rebinding gap is documented (`:1875-1878`); redirect IP-literal check exists (`:1790-1798`).

Observed behavior:
- Missing `Content-Length` + huge body is fully materialized before reject.

Recommended mitigation:
- Streaming read with running byte count.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal implementation agent: codex.

---

### XAPI-GOS-001: HTTP provider timeout is 600s; tagteam has none

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Reliability  
Taxonomy: XAPI-005

Evidence:
- `crates/gosling-providers/src/api_client.rs:17, 248-262` — `DEFAULT_PROVIDER_TIMEOUT_SECS = 600`; `Client::builder().timeout(timeout)`.
- Tagteam: no timeout (AOC-GOS-009).

Observed behavior:
- A single HTTP call can occupy 10 minutes inside an agent turn; tagteam can occupy forever.

Recommended mitigation:
- Per-call-class budgets (connect 10s / read 120s for chat; wall for tagteam).

Implementation assessment:
- Complexity: external_service_semantics. Cost: M. Nominal implementation agent: gpt.

---

### XAPI-GOS-002: Subagent / tagteam / review fan-out has no spend cap

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Reliability  
Taxonomy: XAPI-015, AOC-030

Evidence:
- Review: up to `MAX_WORKERS=4` concurrent `gosling run` (`orchestrator.rs:39-42`) plus one per file/check.
- Delegate: unbounded async tasks aside from `max_turns` (default 25).
- Tagteam: multi-vendor CLI, `Usage::default()`.

Observed behavior:
- One user action can start N live model sessions with no quota object.

Recommended mitigation:
- Session-level max concurrent children + max estimated USD.

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Nominal implementation agent: claude.

---

### IAPI-GOS-001: ACP resource/tool-call responses use untyped `Value`

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture  
Taxonomy: IAPI-001

Evidence:
- `crates/gosling-sdk-types/src/custom_requests.rs:86-92` — `ReadResourceResponse.result: serde_json::Value`.
- `custom_requests.rs:98-117` — `GoslingToolCallRequest.arguments: Value`; response `content: Vec<Value>`.

Observed behavior:
- Desktop/SDK consumers cannot type-check MCP resource payloads at the ACP seam.

Expected boundary:
- Typed MCP result envelopes (already exist in `rmcp`) mapped at the ACP boundary.

Recommended mitigation:
- Replace `Value` with the shared MCP content schema already used internally.

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Nominal implementation agent: gpt.

---

### XREP-GOS-001: Goose catalog fallback is unpinned and silent-empty-then-swap

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture  
Taxonomy: XREP-004, XREP-016, XREP-001, AOC-016  
Limited apply: documentation site, not Electron.

Evidence:
- `documentation/GOOSE_COMPATIBILITY.md:7-13` — prefer local `/servers.json` / `/skills-manifest.json`, else `https://goose-docs.ai/...`.
- `documentation/src/utils/mcp-servers.ts:17-37` and `documentation/src/utils/skills.ts:90-129` — first catalog that returns a non-empty list wins; Goose is second; **no version / digest pin**.
- `documentation/src/utils/goose-compat.ts:4-12, 62-99` — live URLs; `code-review` / `testing-strategy` excluded.
- Desktop `ui/` has **no** `goose-docs` consumer (grep empty).

Observed behavior:
- Empty or 404 local catalog silently serves Goose’s current HEAD. Upstream add/rename/install-command change is what the docs site shows. Provenance fields exist (`sourceCatalog`, `compatibilityNote`) — degrade is visible **if** the UI renders them.

Expected boundary:
- Pin (commit, etag, or vendored snapshot) + explicit “using Goose fallback” banner; contract test against a recorded fixture.

Compatibility verdict: **breaking-silent** for install commands if Goose changes `command`/`url` while local is empty. Deployment order: not applicable (live fetch). Consumer-first is unsafe.

Recommended mitigation:
- Vendor a dated snapshot; CI drift job; never swap catalogs without a UI flag.
- Seam test: empty local + recorded Goose fixture; assert exclusions + gosling:// rewrite.

Implementation assessment:
- Complexity: external_service_semantics. Cost: S. Nominal implementation agent: gpt.

---

### IAPI-GOS-002: Hand-mirrored desktop unions remain a second source of truth

Severity: Low  
Confidence: Confirmed (control exists; residual duplication)  
Evidence basis: source-evidenced  
Domain: Architecture  
Taxonomy: IAPI-004, XREP-002  
Limited apply: in-repo Rust↔Electron, not a sibling repo.

Evidence:
- `crates/gosling-sdk-types/tests/provider_type_dto_ts_drift.rs:1-40` — pins `ProviderTypeDto` wire strings.
- `ui/desktop/src/types/enumDrift.test.ts:7-60` — hand unions vs generated zod from `acp-schema.json`.
- AGENTS.md forbids regenerating `ui/desktop/src/api` OpenAPI clients.

Observed behavior:
- The pin is real (`check-acp-schema` + these tests). Duplication remains for any union **not** listed in `enumDrift.test.ts`.

Recommended mitigation:
- Grow the enumDrift table whenever a new hand union is added; prefer generated SDK types in new desktop code.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal implementation agent: codex.

---

## Coverage tables

### AOC-001..030

| ID | Verdict | Pointer |
|---|---|---|
| AOC-001 | **Finding** AOC-GOS-001, AOC-GOS-007 | summon Auto; review `tools:` |
| AOC-002 | **Finding** AOC-GOS-008 | hardcoded tagteam argv |
| AOC-003 | **Finding** AOC-GOS-002 | prompt on argv |
| AOC-004 | **Finding** AOC-GOS-006 | first `{` |
| AOC-005 | **Finding** AOC-GOS-003, AOC-GOS-005 | empty findings; JSON⇒Ok |
| AOC-006 | **Finding** AOC-GOS-003 | check fail continues as [] |
| AOC-007 | **Non-finding** | Review packages (files/checks) run in parallel; one check does not block others (`orchestrator.rs:82-148`). Supervisor-as-single-gate is the unused feature-gated module. |
| AOC-008 | **Finding** (with AOC-GOS-003) | Review has no workplan artifact |
| AOC-009 | **Finding** AOC-GOS-008 | 1M context constant |
| AOC-010 | **Finding** AOC-GOS-009 | unbounded tagteam stdout |
| AOC-011 | **Finding** AOC-GOS-012 | live `git diff` |
| AOC-012 | **Finding** AOC-GOS-012 | no saved baseline |
| AOC-013 | **Non-finding** (agent); **Finding** (review/tagteam) | Agent persist + in-doubt (`agent.rs:106-120, 2537-2546`). Review/tagteam write no final status file. |
| AOC-014 | **Non-finding** (review); **Finding** (tagteam) | Review stderr per check (`orchestrator.rs:129-134`). Tagteam `output()` is silent until done. |
| AOC-015 | **Finding** AOC-GOS-008 | |
| AOC-016 | **Finding** AOC-GOS-005, XREP-GOS-001 | |
| AOC-017 | **Non-finding** (summon) | Ad-hoc delegates get instructions only (`summon.rs:608-610, 1198-1207`). Review checks each get the **full** diff (contamination by design of the check prompt). |
| AOC-018 | **Not reviewed / N/A** | No in-process tribunal. Tagteam adversarial mode is an external CLI. |
| AOC-019 | **Finding** AOC-GOS-010; **Held** for MCP stdio | `extension_manager.rs:390-435` |
| AOC-020 | **Finding** AOC-GOS-013 | |
| AOC-021 | **Finding** AOC-GOS-004 | |
| AOC-022 | **Finding** AOC-GOS-005, AOC-GOS-009 | |
| AOC-023 | **Non-finding** (review results) | Per-index slots, no shared status file. Session DB concurrency is out of this lens. |
| AOC-024 | **Finding** AOC-GOS-007 | `tools:` schema vs construction |
| AOC-025 | **Finding** (review) | No round counter; failed check not persisted |
| AOC-026 | **Finding** AOC-GOS-003 | |
| AOC-027 | **Non-finding** (review is stateless); **N/A** tagteam-workflow resume | Resume contracts exist only behind feature flag (`contracts.rs:457-460`) |
| AOC-028 | **Finding** AOC-GOS-014; **Held** kill_on_drop / Linux PDEATHSIG | `subprocess.rs:53-68` |
| AOC-029 | **Finding** (delegate) | `return_last_only: true` (`summon.rs:1138`) — summary without raw transcript pointer |
| AOC-030 | **Finding** AOC-GOS-011 | |

### MCP-001..031 (limited apply)

| ID | Host | `gosling-mcp` | Notes |
|---|---|---|---|
| MCP-001 | **Partial** | **Held** computercontroller | Server declares tools+resources (`mod.rs:1718-1721`) and implements `list_resources` (`:1730+`). Host `McpClientTrait` defaults `list_resources` → `TransportClosed` (`mcp_client.rs:109-116`) if a server is not wired through. |
| MCP-002 | N/A host | **Held** (rmcp schemars) | `WebScrapeParams` etc. derive `JsonSchema`. |
| MCP-003 | — | **Finding** MCP-GOS-004 | |
| MCP-004 | **Held** (stdio via rmcp) | **Held** for in-process builtins | Builtin servers use duplex, not stdout prints. Not observed-run. |
| MCP-005 | Host uses tools | **Partial** | Resources used for cache/UI; SOP lives in long `instructions` string. |
| MCP-006 | — | **Finding** (omission) | No hints; spec defaults conservative — usability only unless host over-trusts. |
| MCP-007 | — | **Held** | No `nextCursor` on computercontroller list. |
| MCP-008 | — | **Finding** MCP-GOS-001/003 | Mixed domains + exec |
| MCP-009 | — | **Partial** | Descriptions are long; `web_scrape` says not first tool (`:586`). |
| MCP-010 | — | **Held** | Unique names within server. |
| MCP-011 | — | **Held** | Small param sets; enums for language/save_as. |
| MCP-012 | Host large-response file | **Held** + MCP-GOS-005 | `large_response_handler.rs:42-55` spills with an explicit path message. |
| MCP-013 | — | **Partial** | Peekaboo command encyclopedia inlined in instructions (`:500-525`). |
| MCP-014 | — | **Finding** MCP-GOS-003 | |
| MCP-015 | — | **Finding** MCP-GOS-004 | |
| MCP-016 | — | **Finding** MCP-GOS-005; **Held** script cap 256KiB + marker (`:86-116`) | |
| MCP-017 | — | **Partial** | Scripts `kill_on_drop` (`:122-125`); no task/progress protocol. |
| MCP-018 | — | **Partial** | In-memory `active_resources` Mutex; lost on restart. |
| MCP-019 | — | **Held** (web_scrape) | HTTP fail ≠ empty cache. |
| MCP-020 | — | **Finding** (no dry-run) | MCP-GOS-001 |
| MCP-021 | — | **Finding** MCP-GOS-002 | brew install |
| MCP-022 | — | **Partial** | web_scrape maps to cache path, not raw 47-field JSON. |
| MCP-023 | — | **Held** (web 30s) | `WEB_SCRAPE_TIMEOUT` (`:1778`); scripts unbounded wall time. |
| MCP-024 | **Held** 5s close | **Held** kill_on_drop | `mcp_client.rs:45-49` |
| MCP-025 | Host keyring / env | **N/A** server | No OAuth in this server. |
| MCP-026 | — | **Held** for HTTP (errors, not empty) | Goose catalog is docs, not this server. |
| MCP-027 | Host mcp_integration_test | **Partial** | computercontroller has unit tests (size, path). No A4 live eval this run. |
| MCP-028 | — | **Partial** | tracing on peekaboo install; no write audit log. |
| MCP-029 | Host permission judge | **Finding** MCP-GOS-001 | |
| MCP-030 | Host enables extension | **Finding** MCP-GOS-001/002 | Process is a full user-level automation agent. |
| MCP-031 | — | **Finding** MCP-GOS-001; **Held** SSRF preflight | `ensure_public_http_url` (`:1881+`); `resolve_document_path` (`:1812-1853`). |

**A5 deployment map:** computercontroller/autovisualiser are locally authored stdio/in-process builtins. Marked locally trusted. Not a remote multi-tenant MCP.

**A4 workflows:** static-predicted only (no live model). Intents: scrape URL; run shell; click UI; extract PDF. Predicted tools: `web_scrape` / `automation_script` or `computer_control` / `pdf_tool`. Observed columns: Not Reviewed.

### XAPI-001..018 (P0/P1 sample)

Integrations inventoried (not all deep-read): OpenAI-compatible HTTP, Anthropic, Google, Bedrock, Databricks, Ollama, OpenRouter, CLI adapters (Claude Code, Codex, Gemini CLI, Cursor, Tagteam), OAuth (XAI/Gemini/Tetrate/OpenRouter signup), Goose HTTP catalogs.

| ID | HTTP providers (sampled) | Tagteam CLI | Goose catalog |
|---|---|---|---|
| XAPI-001 | **Held** — `Provider` + format modules | **Partial** — thin spawn, DTO `TagteamFinalRun` | **Held** — `normalizeGoose*` |
| XAPI-002 | **Not observable** (operator grant unknown) | N/A | N/A |
| XAPI-003 | **Not reviewed** this pass | — | Live URL is production catalog |
| XAPI-004 | **Held** `sanitize_url` (`http_status.rs:20-29`) | **Finding** argv (AOC-GOS-002) | — |
| XAPI-005 | **Held** 600s (long) | **Finding** none | browser `fetch` — not reviewed |
| XAPI-006 | **Held** `RetryConfig` transient + jitter (`retry.rs:23-108`) | **N/A** (no retry; good for non-idempotent multi-agent) | — |
| XAPI-007 | **Held** 429 + `Retry-After` (`http_status.rs:32-59`) | **N/A** | — |
| XAPI-008 | **Held** typed `ProviderError` (`errors.rs:8-52`) | Collapsed to text | — |
| XAPI-009 | **Held** OpenAI empty 200 / no `[DONE]` (`openai.rs:1050-1061`); Anthropic 200 auth test (`anthropic.rs:579`) | **Finding** JSON-is-success | empty local → Goose |
| XAPI-010 | Chat completions treated idempotent-enough for retry | Multi-agent write: no Gosling idempotency key | — |
| XAPI-011 | Agent turn fails; no breaker | Full path down | docs degrade to [] |
| XAPI-012 | **Not reviewed** (model list pagination) | — | — |
| XAPI-013 | **Held** `saw_done` (`openai.rs:1056-1061`) | whole stdout or nothing | — |
| XAPI-014 | **N/A** (no inbound webhooks in this slice) | — | — |
| XAPI-015 | **Finding** XAPI-GOS-002 | **Finding** AOC-GOS-011 | — |
| XAPI-016 | Tool args via provider schema; review JSON weak | verdict trusted as text | catalog JSON loosely typed `any` |
| XAPI-017 | Context manager + compaction | 1M fiction | — |
| XAPI-018 | Provider unit/replay tests exist (`gosling/tests`, recordings) | **Finding** AOC-GOS-013 | `goose-compat.test.js` exists (not re-run) |

Call-class vs policy: HTTP chat = long-running/streaming with 600s timeout (policy would want tighter connect). Tagteam = non-idempotent multi-write: no retry is correct; missing wall clock is not.

### IAPI-001..016

| ID | Verdict |
|---|---|
| IAPI-001 | **Finding** IAPI-GOS-001 (`Value` on ACP resource/tool call) |
| IAPI-002 | **Not reviewed** (session SQLite rows vs ACP DTOs) |
| IAPI-003 | **Partial held** — providers map into `Message` / `ProviderError`; tagteam `TagteamFinalRun` is internal |
| IAPI-004 | **Finding** IAPI-GOS-002 (hand unions); **Held** for generated ACP schema path |
| IAPI-005 | **Partial** — `ProviderError` taxonomy vs review `anyhow` vs MCP `ErrorData` |
| IAPI-006 | **Finding** AOC-GOS-003 (check fail collapsed) |
| IAPI-007 | **Not reviewed** (CLI→session service layer) |
| IAPI-008 | **Partial** — `AgentManager::instance()`, `Config::global()`, `PermissionManager::instance()` |
| IAPI-009 | **Finding** AOC-GOS-007 (`tools:` vs construction) |
| IAPI-010 | **Held** for `ProviderType` (tests). Tagteam contract `schema_version` (`contracts.rs:9, 413-417`) not in default binary |
| IAPI-011 | **Partial** — review `Option` fields defaulted (`orchestrator.rs:209-216`) |
| IAPI-012 | **Held** (summon) — no nested delegate (`summon.rs:1075-1076`) |
| IAPI-013 | **Finding** AOC-GOS-013 for orchestration; **Held** ProviderType/enumDrift |
| IAPI-014 | **Partial** — MCP session id headers (`mcp_client.rs:246-259`); review workers have no correlation id |
| IAPI-015 | **Finding** AOC-GOS-001 (approval not propagated to child) |
| IAPI-016 | **Finding** AOC-GOS-007 (orchestrator vs `--no-orchestrate`) |

### XREP-001..016 (limited)

| ID | Rust↔Electron ACP/SDK | Goose catalog |
|---|---|---|
| XREP-001 | **Held** — `gosling-sdk-types` + `acp-schema.json` + generate path | **Finding** XREP-GOS-001 (local vs live Goose) |
| XREP-002 | **Finding** IAPI-GOS-002 (hand unions) | Goose ids rewritten, not copy-pasted blindly |
| XREP-003 | **Not reviewed** exhaustively | Excluded skills are intentional (`goose-compat.ts:9-12`) |
| XREP-004 | **Held** if `check-acp-schema` is green (not run this pass) | **Finding** unpinned live JSON |
| XREP-005 | N/A this HEAD | Unknown without Goose snapshot |
| XREP-006 | Generated zod from schema — **Held** by process | N/A |
| XREP-007 | **Held** — desktop uses `@repo-makeover/gosling-sdk` | N/A |
| XREP-008 | **Not reviewed** | — |
| XREP-009 | **Not reviewed** field-by-field | `asString`/`asBoolean` coerce missing fields |
| XREP-010 | **Held** `parseProviderType` throws on unknown (`providers.ts:92-96`) | `status` unknown → `"stable"` (`goose-compat.ts:137`) — **fail-open** (Low) |
| XREP-011 | **Not reviewed** | fetch fail → `[]` |
| XREP-012 | camelCase serde on ACP types | `streamable_http` → `streamable-http` (`goose-compat.ts:55-56`) |
| XREP-013 | `_gosling/unstable/...` methods in sdk-types | `/servers.json` vs absolute Goose URL |
| XREP-014 | N/A | N/A |
| XREP-015 | **Not reviewed** | compat tests exist; not re-run |
| XREP-016 | enumDrift + rust dto test **Held** | **Finding** no pin to Goose HEAD |

Unknown Goose `status` → `stable` (`goose-compat.ts:137`) is XREP-010 fail-open: **Low**, not escalated.

---

## Explicit non-findings (controls that held)

- **MCP stdio env isolation:** `minimal_child_environment` + `env_clear` (`extension_manager.rs:390-435`). Sibling API keys are not the default MCP child environment.
- **Docker env-file, not argv:** `write_docker_env_file` comment (`extension_manager.rs:460-464`) avoids `ps` leakage of secrets.
- **Context truncation is marked:** `packet.rs:146-154` appends ` ... [truncated]`; summarizer refuses to cache malformed JSON (`summarizer/mod.rs:964` test name in source).
- **Large tool results spill with a path, not silent amputate:** `large_response_handler.rs:42-55`; Unix 0600 + reject symlink (`:125-170`).
- **Delegate extension fail is visible:** `subagent_handler.rs:90-105` prefixes failed extension names.
- **Ad-hoc delegates default to no extensions:** `resolve_delegate_extensions` `(None, None) => &[]` (`summon.rs:171`).
- **Nested delegate denied:** `summon.rs:1075-1076`.
- **Working dir confinement:** delegate `working_dir` must resolve under parent (`summon.rs:1298-1301` + schema text `:589-591`); document tools confined (`resolve_document_path` `:1845-1850`).
- **Tagteam refuses non-Auto Gosling modes:** `tagteam.rs:198-202, 358-362` — fail-closed vs silent yolo.
- **Review workers are tool-free on the default orchestrator path:** `--no-profile` and no builtins (`worker.rs:77-90`, `builder.rs:404-405`). Privilege *escape* via review checks is not the default path (legacy `--no-orchestrate` is).
- **Review prompt via stdin, not argv:** `worker.rs:63-67`.
- **Review `kill_on_drop`:** `worker.rs:89`.
- **web_scrape SSRF preflight + redirect policy:** `ensure_public_http_url`, `redirect_target_is_private`.
- **HTTP retry:** transient-only default, jitter, `Retry-After` cap 3600s, permanent Anthropic thinking-block marker (`retry.rs:84-108`).
- **OpenAI stream completeness:** `saw_done` / `yielded_any_content` (`openai.rs:1050-1061`).
- **ProviderType pin across Rust and desktop:** dto + enumDrift tests.
- **Goose install rewrite + exclusions:** `convertGooseCommand`, `GOOSE_EXCLUDED_SKILL_IDS`.
- **Phase-1 steward policy (source only):** deny shell/file/delegate/widen (`policy.rs:51-60`); mutating tools not in `exposed_tool_names`.
- **Subprocess Linux parent-death SIGTERM:** `subprocess.rs:7-23`.

---

## Success and failure paths traced (static)

**Success — `gosling review` (orchestrator):** `handle_review` → git diff + untracked synth → `discover` checks → `join!(run_main_pass_in_parallel, run_checks_in_parallel)` → `emit_findings` JSONL → `Ok(())`.

**Failure — check JSON missing:** subprocess success/fail → `parse_findings` err → empty vec → review still `Ok(())`.

**Success — tagteam profile:** `stream` → `build_prompt` → argv spawn → JSON `TagteamFinalRun` → formatted assistant message, usage zeros.

**Failure — tagteam missing binary:** `output()` `map_err` RequestFailed with install hint (`tagteam.rs:225-229`).

**Success — ad-hoc delegate:** validate params → Auto session → `run_subagent_task` → last text + authority prefix.

**Failure — extension load on subagent:** continue; warning prefix (`subagent_handler.rs:191-199, 90-105`).

**Success — web_scrape:** public URL → GET 30s → size check → cache path.

**Failure — private URL:** `INVALID_PARAMS` before connect.

---

## Skill escalation

| Finding | Primary lens | Secondary lens | Why |
|---|---|---|---|
| AOC-GOS-001 | AOC | Security / State-Transition | Approval mode not propagated |
| AOC-GOS-002 | AOC | Security | Argv secret/prompt leak |
| AOC-GOS-003 | AOC | Failsafe / Operator-signal | Empty success |
| AOC-GOS-004 | AOC | Security-llm / Negative-space | Hostile repo config |
| MCP-GOS-001 | MCP | Security-code | Exec sink |
| AOC-GOS-005 | AOC | Reliability | Failed orchestrator as success |
| XREP-GOS-001 | XREP | Reliability | Silent catalog swap |
| AOC-GOS-009 / XAPI-GOS-001 | XAPI | Resource-lifecycle | Unbounded wait/buffer |
| IAPI-GOS-001 | IAPI | XREP | Untyped ACP payload |

---

## Recommended patch order

1. **Gate / honesty:** AOC-GOS-003 (review check outcome), AOC-GOS-005 (tagteam JSON ≠ success).
2. **Authority construction:** AOC-GOS-001 (subagent mode), AOC-GOS-004 (repo capabilities), MCP-GOS-001/002 (exec + brew).
3. **Leakage:** AOC-GOS-002 (stdin prompt), AOC-GOS-010 (env allowlist).
4. **Contracts:** AOC-GOS-006/007, IAPI-GOS-001, XREP-GOS-001.
5. **Bounds:** AOC-GOS-009, XAPI-GOS-001/002, MCP-GOS-005.
6. **Polish:** AOC-GOS-008/011, MCP-GOS-003/004, IAPI-GOS-002.

## Fake-agent / seam tests to add

- Review: fake `gosling` exiting 1 / printing decoy-then-real JSON / printing `{"findings":[]}`.
- Tagteam: fake binary with fail JSON + exit 1; huge stdout; missing binary; prompt not on argv.
- Delegate: parent Approve + developer capability file → reject.
- Goose: empty local + recorded fixture; unknown `status` ≠ `stable` (if that fix is taken).
- web_scrape: oversized body without Content-Length.

## Break-it notes (all static-read)

| Attempt | Expected-by-policy | Observed in source | Verdict |
|---|---|---|---|
| Review malformed JSON | typed fail / degrade flag | empty findings | **Breaks** AOC-GOS-003 |
| Decoy first JSON | reject or last-object | first `{` | **Breaks** AOC-GOS-006 |
| Tagteam fail JSON | provider error | `Ok(format_final_run)` | **Breaks** AOC-GOS-005 |
| Parent Approve + delegate developer | confirmation or deny | Auto + possible yolo | **Breaks** AOC-GOS-001 |
| Prompt on `ps` | stdin | last argv | **Breaks** AOC-GOS-002 |
| Repo capabilities | untrusted | grant parent extensions | **Breaks** AOC-GOS-004 |
| Private web_scrape URL | refuse | `ensure_public_http_url` | **Holds** |
| MCP child env API keys | absent | `env_clear` allowlist | **Holds** |
| Tagteam SmartApprove | refuse | ExecutionError | **Holds** |
| Interrupt kill_on_drop | child dies | set on review + tagteam | **Holds** (SIGTERM/drop); SIGKILL macOS **open** |

## Coverage, budget, validation limits

- **Deep-read:** tagteam provider + contracts/policy/reducer headers; review CLI (handler/orchestrator/worker/prompt); summon delegate construction; computercontroller exec/web/SSRF; MCP host trait + extension env; provider retry/timeout/errors; sdk-types custom requests; Goose compat TS; desktop enumDrift; subprocess; context packet truncation; large_response_handler; agent checkpoint constants.
- **Inventoried only:** remaining HTTP providers (Bedrock, Databricks, XAI OAuth, …); autovisualiser tool bodies; peekaboo module internals; ACP server method table; session SQLite schema; Electron ACP client beyond enumDrift.
- **Not executed:** `cargo test`, `just check-acp-schema`, live tagteam, MCP inspector, Goose fetch, SIGKILL drill.
- **Oracle integrity:** no test suite used as production oracle. All verdicts are static-read; runtime claims capped per `confidence_calibration.md`.
- **Budget:** one combined report, five taxonomies; adapter surface sampled (tagteam + HTTP client + two CLI permission maps). Next-highest-value unread seam: full ACP custom-method table vs `ui/desktop/src/acp/*` field-by-field, and remaining CLI adapters (`gemini_cli`, `cursor_agent`, `amp_acp`).

Unresolved: whether production desktop ever enables `computercontroller` by default (config not exhaustively traced); whether `tagteam-workflow` is shipped in any distro feature set (default Cargo features do not include it).
