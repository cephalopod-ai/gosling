# Grok comprehensive audit — 13 October 2026

Current repair disposition: see the [13 October repair addendum](#repair-addendum--13-october-2026) and [session record](docs/logs/session/2026-10-13-grok-findings-repair.md). The audit below remains historical evidence from e61fc45ab.

Read-only screening of gosling at `e61fc45abdc38c7c06e66685a4dff63d0fab00f5` on `main`. Authority for this run is read-only. No source, test, workflow, or config file was modified. The uncommitted cancel-token accessor in `active_runs.rs` and `manage_sessions.rs` was already in the tree during synthesis, was not produced by this audit, and was left untouched.

## Executive Verdict

Three High findings are source-confirmed and should be patched before Completely Autonomous mode, folder restriction, or non-macOS computer control are treated as matching their published contracts. They are `SEC-GROK-003` (undecided inbound hosts are Allowed, and Auto downgrades the remaining egress prompts), `LLM-GROK-001` (folder restriction does not score a complete shell command that has no explicit path token), and `MCP-GROK-001` (Windows and Linux `computer_control` report success without reading the process exit status).

The October 12 highs (`SEC-GSL-001`, `SEC-GSL-201`, `SECN-GSL-201`) were re-checked against current source and remain repaired. They are not re-filed. Rotation of the CI secrets that were exposed before that repair is still an owner action outside this repository.

The rest of this pass is 13 Medium and 4 Low findings. Several Medium process-survival outcomes are capped at Likely because no process was killed or observed. The false-success flags in orchestrated replies and `automation_script` are Confirmed code properties; the orchestrator item is Medium because the error sentence is present in the success body and a turn cap exists.

Dataflow lenses produced no new defect. That is not a dataflow clearance: most IOP, STT, DAT, CON, CAS, TMP, and INV procedure codes were Not Reviewed. Playtest, measured performance, the live model catalog, and design-webapp gates 2–6 were not executed. Patching the three Highs is recommended now. A full merge freeze is not recommended on this evidence. Do not treat this report as fleet compliance. `.giles/*.yaml` remains advisory.

## Scope

- Repository/project / branch / commit: `gosling`, branch `main`, commit `e61fc45abdc38c7c06e66685a4dff63d0fab00f5` (`fix(workspace): synchronize gitignore updates using process file locks`). Working tree at synthesis: uncommitted edits only in `crates/gosling/src/acp/server/active_runs.rs` and `crates/gosling/src/acp/server/manage_sessions.rs` (a `cancel_token()` accessor). Findings are against HEAD behavior. The dirty hunks call the same cancel token the HEAD clone used.
- Prompt or session log reviewed: operator request to run a comprehensive audit of all applicable agent skills and write `grok-comprehensive-audit-13oct2026.md`. Prior baseline: `mistral-audit-comprehensive-12OCT2026.md` and `docs/logs/session/2026-10-12-mistral-findings-repair.md`. Historical findings were re-verified, not copied.
- Skills (lenses) invoked: the thirteen required lenses (`audit-architecture-seam`, `audit-dataflow-cascade`, `audit-dataflow-concurrency`, `audit-compliance-posture`, `audit-dataflow-integrity`, `audit-dataflow-input-output`, `audit-invariant-sync`, `audit-negative-space`, `audit-reliability`, `audit-security`, `audit-dataflow-state-transition`, `audit-dataflow-temporal`, `audit-workflow-gui`) plus the applicable standalones assigned in the Coverage Matrix. `audit-multiagent-consensus` was not requested. No gosling-specific project overlay exists in `000_common/audit-base/project_overlays.md`.
- Files/directories inspected: permission and security inspectors, tool inspection, developer shell, orchestrator, computer-control MCP, large-response spill, extension startup, text UI shutdown, desktop backend registry and serve spawn, workspace ACP handlers, provider guide and config guide, remote-server guide, GitHub Copilot sign-in removal, `pr-smoke-test.yml`, `docs/INDEX.md`, `README.md`, `Cargo.toml`, `ui/desktop/package.json`. Lane notes cover additional held seams. Detection-procedure bodies were not loaded file by file.
- Commands/tests run: read-only `git status`, `git rev-parse`, and source reads. No `cargo`, `pnpm`, clippy, test runner, server, browser, playtest, or network catalog call.
- Effort budget (per-lens) and what it bought: five read-only lanes (security, data, architecture, reliability, operations) plus lead re-read of every High and every Medium that is filed below. The data lane’s first pass did not emit a disposition file; a resume wrote the disposition from evidence already collected and filed no new defect.
- Constraints: read-only target. No exploit procedures, payloads, or sample command lines. No source format or repair. Documentation writes limited to this report, one session log, and one `docs/INDEX.md` line. Confidence calibration v3.1: unreproduced process-survival outcomes cap at Likely.

## Draft Prompt Assessment

The mission was an exhaustive applicable-skill audit and a report that follows `000_common/audit-base/report_template.md` and `finding_format.md`. The catalog selection was an audit suite (`selection.kind=audit_suite`, breadth exhaustive). Retrieval used a deterministic fallback because the semantic skill index was stale. The suite membership was not changed, and the catalog was not refreshed.

Assumptions checked against current source: October 12 repairs still hold where re-read; ADR-0015, ADR-0017, and ADR-0018 still explain the historical path, migration, and output-revision dispositions; the permissions guide still says Completely Autonomous keeps stricter egress and workspace decisions. Those checks changed what was filed. The guide sentence is why `SEC-GROK-003` stays High.

Adjacent work that was applicable and not performed: playtest of `docs/test_scenarios` (needs a running GUI and credentials), measured test-suite economics and performance profiles, a live provider-catalog drift check, design-webapp visual and WCAG gates, and a full negative-space playbook walk of every NEG code. Those are recorded as Not Reviewed or Applicable, Not Executed.

## Surface Inventory

| Surface | Actor | Input/Trigger | State/Output | Boundary | Reviewed |
|---|---|---|---|---|---|
| Permission inspectors | Model tool call | Shell, web, and file arguments | Allow, approval, or deny | `permission.yaml`, Auto, NeverAllow | Reviewed for egress, folder scope, security log, website-login hold |
| Developer shell | Agent | Command string | Process via `-c` | Working-directory restriction | Reviewed as the sink for `LLM-GROK-001` |
| Computer control MCP | Agent | Automation script | `CallToolResult` | Exit status | Reviewed. macOS Peekaboo checks status. Windows/Linux `computer_control` does not |
| Orchestrator `send_message` | Parent agent | Child reply stream | Tool result | Depth cap, turn cap, `is_error` | Reviewed. Depth and turn cap hold. Success flag does not |
| Desktop chat composer | Operator | Enter, shortcut, form | User message | Disabled Send | Reviewed. Button and keyboard predicates differ |
| Desktop backend process | Electron | Spawn, crash, quit | `gosling serve` group | Group signal | Reviewed. Live supervisor uses a negative pid. Registry sweep uses a positive pid |
| Extension stdio startup | Agent | MCP connect | Process group | `killpg` until connect | Reviewed. Guard is disarmed after success |
| Text UI | Operator | `/exit`, Ctrl-C, `--text` | ACP child | Grace then SIGKILL | Reviewed. Helper exists. Normal quit calls `cleanup` |
| Workspace ACP | Desktop / CLI | Create, update, delete | ACP error code | Architecture taxonomy | Reviewed for the mutation helpers |
| Session store | Agent | Delete, spill, extension state | SQLite / temp files | ADR-0018, spill cap | Spill refusal re-inlines the body. Delete/revision contract held on sampled paths. Most dataflow codes Not Reviewed |
| CI live-provider workflow | GitHub Actions | Push to `main`, `workflow_dispatch` | Secrets | Main-only gate | Trigger and job `if` re-read. Secret rotation not verified |
| Provider setup guide | Operator | Configure Providers | Registry ids | Shipped declarative JSON | Reviewed for FuturMix, Routstr, EmpirioLabs |

## Boundary Map

| Surface | Intended Boundary | Enforced At | Status |
|---|---|---|---|
| Completely Autonomous egress | Saved NeverAllow and stricter egress decisions still apply (`gosling-permissions.md` permission-mode table) | `EgressInspector` inbound branch allows undecided hosts; trait default downgrades remaining prompts | Broken for undecided hosts (`SEC-GROK-003`) |
| Folder restriction | Every out-of-scope path requires approval (`WorkingDirScopeInspector` struct comment) | Path tokens that look explicit; path-free complete commands emit no result | Broken for that command class (`LLM-GROK-001`) |
| Incomplete shell syntax | Require approval (`SEC-GSL-901`) | `analyze_shell(...).complete` | Holds |
| Read-only workspace roots | Mutating shells denied (`AUD-GOS-003`) | Inspector early branch | Holds |
| Website login | Destination check plus approval, including Auto | `website_logins` and `website_login_inspector` (earlier pass this engagement) | Holds. Not re-filed |
| Renderer directory grant | Native confirmation; skip sensitive and overly broad roots from the renderer | `rendererIpc.ts` (earlier pass) | Holds. Native picker can still persist a user-chosen sensitive directory after confirmation |
| `computer_control` result | Non-zero exit is a tool error | macOS Peekaboo returns `Err`. Non-macOS wrapper always returns success | Broken on Windows and Linux (`MCP-GROK-001`) |
| `automation_script` result | Non-zero exit is `is_error` | Text says failed; result is `success` | Broken (`MCP-GROK-002`) |
| Managed-agent stream error | Failed child is not a successful tool result | Error text is appended, then `CallToolResult::success` | Broken as a flag (`ORCH-GROK-001`) |
| Disabled Send | Every submit path shares the button predicate | Enter, shortcut, and form use a narrower `canSubmit` | Broken (`WFG-GROK-001`) |
| Crash sweep of `gosling serve` | Same process-group signal as the live supervisor | `terminateProcess` signals a positive pid | Missing group signal. Orphan outcome Likely (`FSR-GROK-001`) |
| Spill cap | A body the cache refuses is not inserted whole | `Err` arm appends `text_content.text` | Broken (`REL-GROK-001`). OOM was not observed |
| Workspace ACP errors | Not-found and storage use different codes (`docs/architecture.md` error taxonomy) | Mutations use `invalid_params_err` | Broken (`ARC-GROK-004`) |
| `/health` vs `/status` | Liveness vs database readiness | `health` does no I/O; `status` calls `sessions.healthy()` (lane-cited; guide sentence re-read) | Code split holds. One guide sentence groups them (`WFG-GROK-002`) |
| Output revisions after session delete | Path-wide history survives (`ADR-0018`) | No `session_id` on revisions; delete does not remove them | Holds by ADR. Not re-filed |
| PR live secrets | Not available to pull-request head code | `pr-smoke-test.yml` triggers are `push` to `main` and `workflow_dispatch`; job requires `refs/heads/main` | Holds as a trigger property |

## Findings Table

| ID | Severity | Confidence | Evidence Basis | Domain | Title | Patch Priority | Blast Radius | Complexity | Cost | Nominal Agent |
|---|---|---|---|---|---|---|---|---|---|---|
| SEC-GROK-003 | High | Confirmed | source-evidenced | Security | Undecided inbound hosts are Allowed, and Auto downgrades remaining egress prompts | P1 | Service | local_guardrail | S | grok |
| LLM-GROK-001 | High | Confirmed | source-evidenced | Security | Folder restriction ignores complete shells with no explicit path token | P1 | Service | workflow_protocol | S | grok |
| MCP-GROK-001 | High | Confirmed | source-evidenced | Workflow-GUI | Non-macOS computer_control reports success without an exit check | P1 | Workflow | local_guardrail | S | codex |
| ORCH-GROK-001 | Medium | Confirmed | source-evidenced | Workflow-GUI | Managed-agent stream errors are successful tool results | P2 | Workflow | workflow_protocol | S | codex |
| MCP-GROK-002 | Medium | Confirmed | source-evidenced | Workflow-GUI | automation_script failure is still a successful tool result | P2 | Workflow | local_guardrail | XS | codex |
| REL-GROK-001 | Medium | Confirmed | source-evidenced | Reliability | Spill refusal copies the full tool body back into the conversation | P2 | Workflow | local_guardrail | S | codex |
| SEC-GROK-001 | Medium | Confirmed | source-evidenced | Security | Above-threshold explanations copy command text into info logs | P3 | Local | local_guardrail | S | grok |
| SEC-GROK-002 | Medium | Confirmed | source-evidenced | Security | Egress logs and approval reasons keep the full URL match | P3 | Local | local_guardrail | S | grok |
| WFG-GROK-001 | Medium | Confirmed | source-evidenced | Workflow-GUI | Disabled chat Send is still invoked by Enter | P3 | Workflow | operator_ux | S | codex |
| FSR-GROK-001 | Medium | Likely | source-evidenced | Failsafe | Stale-backend sweep signals the serve pid, not its process group | P3 | Local | local_guardrail | S | codex |
| FSR-GROK-002 | Medium | Likely | source-evidenced | Failsafe | Extension process-group kill is disarmed after a successful MCP start | P3 | Local | cross_process_coordination | M | codex |
| FSR-GROK-003 | Medium | Likely | source-evidenced | Failsafe | Text UI quit and text mode skip the grace-and-SIGKILL shutdown | P3 | Local | local_guardrail | S | codex |
| ARC-GROK-004 | Medium | Confirmed | source-evidenced | Architecture | Workspace mutations report not-found and storage failures as invalid params | P4 | Service | local_guardrail | S | codex |
| FSR-GROK-004 | Medium | Confirmed | source-evidenced | Failsafe | Rejected GitHub sign-in is reported removed even when deletion fails | P4 | Workflow | local_guardrail | XS | codex |
| REL-GROK-002 | Medium | Confirmed | source-evidenced | Reliability | Extension-state save failure is logged and the turn still treats the install as durable | P4 | Workflow | workflow_protocol | S | codex |
| ARC-GROK-001 | Medium | Confirmed | source-evidenced | Architecture | Guide registers providers whose declarative catalogs are not shipped | P4 | Service | local_guardrail | S | grok |
| ARC-GROK-002 | Low | Confirmed | source-evidenced | Architecture | Guide says tool_permissions.json is auto-managed; no product caller exists | P5 | Local | local_guardrail | XS | grok |
| ARC-GROK-003 | Low | Confirmed | source-evidenced | Architecture | docs/INDEX.md still calls the v1.4.0 notes the current source | P5 | Local | local_guardrail | XS | grok |
| WFG-GROK-002 | Low | Confirmed | source-evidenced | Workflow-GUI | Guide attributes database readiness to /health and /status together | P5 | Local | local_guardrail | XS | grok |
| ARC-GROK-005 | Low | Confirmed | source-evidenced | Architecture | Shell-productization resume commands cd to this checkout | P5 | Local | local_guardrail | XS | grok |

Counts: 3 High, 13 Medium, 4 Low. No Critical. No new dataflow finding.

## Detailed Findings

### SEC-GROK-003: Undecided inbound hosts are Allowed, and Auto downgrades remaining egress prompts

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `documentation/docs/guides/managing-tools/gosling-permissions.md` permission-mode table: Completely Autonomous still applies saved Never Allow rules and stricter security, egress, hook, workspace, and planning decisions.
- `crates/gosling/src/security/egress_inspector.rs:298-357` `detect_direction`: `git clone` / `git pull` / `git fetch` return inbound; `docker pull` returns inbound; plain `curl` / `wget` that missed the upload regexes return inbound.
- `crates/gosling/src/security/egress_inspector.rs:500-521`: NeverAllow denies; loopback or stored AlwaysAllow allows; otherwise Inbound is Allow and Outbound or Unknown requires approval.
- `crates/gosling/src/tool_inspection.rs:60-64` trait default `auto_downgrades_require_approval` is true. `EgressInspector` does not override it.
- `crates/gosling/src/tool_inspection.rs:135-148` rewrites RequireApproval to Allow in Auto when that default is true.
- `crates/gosling/src/permission/permission_inspector.rs:165-169` Auto allows a tool with no stored decision.

Observed behavior:
- A destination that is not NeverAllow and is not pre-cleared is Allowed when the direction heuristic returns inbound.
- Directions that still require approval are downgraded to Allow in Auto because egress keeps the trait default.
- In Approve mode, a permission RequireApproval is not relaxed by an egress Allow. NeverAllow still denies. Loopback and stored AlwaysAllow stay Allow.

Expected boundary:
- A host with no stored decision does not transfer until the operator approves it, including in Auto, the same way website-login and the security scanner refuse to auto-downgrade.

Failure mechanism:
- Direction is a substring heuristic, and inbound is defined as Allow. The missing override then drops the remaining prompts in the mode that already Allows the tool at the permission layer.

Break-it angle:
- In Auto, a shell or web tool whose text is classified inbound reaches a host that has no stored grant and is not denied. Outbound text that would have required approval is converted to Allow by the default downgrade.

Impact:
- Autonomous mode can move data to, or fetch data from, hosts the operator has not approved. This is the operator’s own local agent after Auto was selected. It is not an unauthenticated remote service bug.

Operational impact:
- Blast radius: Service
- Side-effect class: network
- Reversibility: irreversible
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- `LLM-GROK-001` is the separate filesystem gap on the same Auto shell.
- `SEC-GROK-002` logs the same destination strings.

Recommended mitigation:
- Remediation patterns: fail closed on an unknown domain for every direction except loopback or a stored AlwaysAllow. Keep NeverAllow as Deny.
- Minimal repair: override `auto_downgrades_require_approval` to false on `EgressInspector`, and stop mapping Inbound to Allow for hosts that are not pre-cleared.
- Local guardrail: an undecided host does not Allow; Auto does not downgrade an egress RequireApproval.
- Behavior test: those two cases, plus a stored NeverAllow that still denies.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests, docs
- Nominal implementation agent: grok
- Rationale: two policy branches and tests. The written guide already states the intended Auto boundary. A human owner should change the guide only if the product decision is the opposite of that sentence.

Validation:
- Static read only. The tests above were not run.

Non-goals:
- Do not block loopback.
- Do not add a new domain denylist.
- Do not include exploit strings.

### LLM-GROK-001: Folder restriction ignores complete shells with no explicit path token

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `crates/gosling/src/permission/working_dir_scope_inspector.rs:15-21` says that with the restriction on, every out-of-scope path requires approval.
- `working_dir_scope_inspector.rs:46-47` returns no results when restriction is off and there is no workspace.
- `working_dir_scope_inspector.rs:104-122` requires approval when `analyze_shell` reports the command incomplete (`SEC-GSL-901`).
- `working_dir_scope_inspector.rs:524` sets `complete` from a tree-sitter parse with no error. `557` forces incomplete for `env -S`.
- `working_dir_scope_inspector.rs:384-420` `looks_like_explicit_path` and `path_from_shell_token` accept only a whole token that already looks like a path, or an `=` value that does.
- `working_dir_scope_inspector.rs:761-768` and `791-802` collect only those tokens, plus `cd` targets.
- `working_dir_scope_inspector.rs:1009-1012` says path-free calls are left alone.
- `working_dir_scope_inspector.rs:142-143` continues when no out-of-scope path is returned. `186-188` would keep a result in Auto, but no result is emitted.
- `working_dir_scope_inspector.rs:68-84` still denies mutating shells when any workspace root is read-only.
- `permission_inspector.rs:165-169` Auto allows the shell tool when no stricter result exists.
- `crates/gosling/src/agents/platform_extensions/developer/shell.rs:83-84` and `652-701` run the command string with `-c`.

Observed behavior:
- With folder restriction on, or with a workspace context, an incomplete shell goes to approval.
- A complete command is checked only for tokens that already look like paths. A non-read-only program whose arguments do not match that shape adds no path, and the inspector emits nothing.
- The shell then runs that string.

Expected boundary:
- While folder restriction is on, a shell that can read or write outside the session directories requires approval even when the command line has no standalone path token.

Failure mechanism:
- Scope enforcement is an allowlist of tokens that look like paths. The comment on `out_of_scope_path` states that path-free calls are left alone. That disagrees with the struct-level guarantee for a general shell.

Break-it angle:
- In Auto, with folder restriction on and without a read-only workspace root, the permission inspector Allows the shell and the scope inspector returns no result for a complete command that contains no explicit path token. `build_shell_command` then runs it.

Impact:
- The folder restriction does not bind the developer shell for this class of command. Files outside the session directory can be read or written without a prompt when Auto, or a stored AlwaysAllow on the shell, is in effect. Confidence is Confirmed for the missing inspection result and the `-c` sink.

Operational impact:
- Blast radius: Service
- Side-effect class: file
- Reversibility: irreversible
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- `SEC-GROK-003` is the network gate on the same Auto shell.
- Incomplete parses, nested `-c` that fails re-analysis, and `env -S` still require approval.
- A workspace with any read-only root still denies mutating shells.

Recommended mitigation:
- Remediation patterns: treat “no path extracted” as unscored, not as in scope.
- Minimal repair: when restriction is on, a complete shell segment that is not read-only and that yields no in-scope path should RequireApproval or Deny. Keep the incomplete-parse branch. Do not try to decode every embedded path.
- Local guardrail: the inspector’s existing `auto_downgrades_require_approval() == false` then keeps that prompt in Auto.
- Behavior test: a path-free non-read-only segment is gated; an explicit in-scope read of a read-only executable is not.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: grok
- Rationale: one new result after path collection, plus tests. A human owner should confirm how read-only executables that also lack path tokens should be treated. `shell_segment_is_read_only` already returns true for a fixed name list.

Validation:
- Static read only. Tests were not run.

Non-goals:
- No sample command lines.
- No change to the read-only workspace deny.
- No race claim.

### MCP-GROK-001: Non-macOS computer_control reports success without an exit check

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `crates/gosling-mcp/src/computercontroller/mod.rs:1112-1146` (`computer_control_impl`, not built on macOS) always formats “Script completed successfully.” and returns `CallToolResult::success`.
- `crates/gosling-mcp/src/computercontroller/platform/linux.rs:133-163` `execute_x11_command` uses `.output()?` or spawn and then `Ok(...)` without reading `ExitStatus`. An unrecognized command returns `Ok(String::new())`.
- The lane read the Windows PowerShell helper at `platform/windows.rs:9-22` as returning stdout without `output.status`. That Windows body was not re-opened during synthesis. The shared non-macOS wrapper above does not consult a status value at all.
- macOS `computer_control` is the Peekaboo path. The lane read `run_peekaboo_cmd` checking `output.status.success` and `ensure_peekaboo` returning an error without installing Peekaboo. That macOS body was not re-opened during synthesis. This finding is the non-macOS wrapper.

Observed behavior:
- On the non-macOS `computer_control` path, the tool result is success text whenever the platform helper returns `Ok`, including a Linux command the matcher does not recognize and a helper that ignores exit status.

Expected boundary:
- A non-zero automation exit and an unrecognized command are tool errors. The success sentence is used only when the process status is success.

Failure mechanism:
- Platform helpers drop the exit status, and the wrapper hard-codes a success sentence and `CallToolResult::success`.

Break-it angle:
- A Windows or Linux automation action that exits non-zero, or a Linux command the matcher does not recognize, still comes back as a successful tool result. Partial effects may already have happened.

Impact:
- The model continues as if a destructive computer action succeeded. `destructive_hint` is set and is not the defect. The defect is the result flag.

Operational impact:
- Blast radius: Workflow
- Side-effect class: process
- Reversibility: irreversible
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- `MCP-GROK-002` is the same family on `automation_script`, which does read status and then still returns success.
- `ORCH-GROK-001` is a different success-flag sink.

Recommended mitigation:
- Remediation patterns: return `CallToolResult::error` when status is missing or not success, and do not say the script completed successfully.
- Minimal repair: thread `ExitStatus` out of the Windows and Linux helpers. Map unrecognized commands to an error.
- Local guardrail: keep the macOS Peekaboo failure path as the comparison.
- Behavior test: a non-zero status and an unknown Linux command produce `is_error: true`.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: three helpers and one wrapper. A unit test can inject status without a live desktop session.

Validation:
- Static read of the wrapper and the Linux X11 helper. Windows and macOS bodies were read in the operations lane and not re-opened here. No test was run.

Non-goals:
- Do not change Peekaboo install policy.
- Do not add an automation command.
- Do not write a sample script.

### ORCH-GROK-001: Managed-agent stream errors are successful tool results

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `crates/gosling/src/agents/platform_extensions/orchestrator.rs:760-770`: on `Some(Err(e))`, `drain_managed_agent_reply` pushes `Error during agent processing: {e}` and returns `Ok(response_parts)`.
- `orchestrator.rs:569-587`: that vector becomes `CallToolResult::success`, including the empty-output sentence.
- Parent cancel and tool-confirmation return `Err` (`736-751`).
- Depth cap is present (`orchestrator.rs:400-402`, lane-cited). `send_message` sets `max_turns: None` (`559`). The reply stream then uses `DEFAULT_MAX_TURNS` (1000) or `GOSLING_MAX_TURNS` (lane-cited `agent.rs:83` and `reply_stream.rs:140-200`). Subagent tasks separately default to 25. This is not an unlimited-turns finding.

Observed behavior:
- A child stream error is delivered to the parent as `is_error: false`, with the error sentence inside the success content.

Expected boundary:
- A stream error is `CallToolResult::error`. Partial text before the error is labeled partial.

Failure mechanism:
- The drain converts the error into a normal text part and returns `Ok`. The call wrapper treats `Ok` as success.

Break-it angle:
- A provider or agent error mid-reply becomes a successful `send_message` result. The parent can continue the plan. The error sentence is visible inside that body, so this is a false protocol flag, not a silent empty success.

Impact:
- Orchestrated work is treated as done when the child failed. The 1000-turn default can let a parent keep issuing work after that false success. The turn cap itself is present.

Operational impact:
- Blast radius: Workflow
- Side-effect class: external API
- Reversibility: compensatable
- Operator visibility: UI-visible
- Rerun safety: unsafe

Adjacent failure modes:
- `MCP-GROK-001` and `MCP-GROK-002` are the computer-control success flags. They stay separate findings.

Recommended mitigation:
- Remediation patterns: return `Err` from the drain on stream `Err`, or return `CallToolResult::error` without a success heading.
- Minimal repair: that flag change. Keep the approval-unavailable `Err` path and the depth cap.
- Local guardrail: a stream that yields `Err` produces `is_error: true`.
- Behavior test: the existing approval-hang tests plus one error-event case.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: the operations lane filed this High. The lead downgrades it to Medium because the error text is in the success body and a turn cap exists. The flag is still wrong. Confidence stays Confirmed for the flag, not for any unreproduced retry storm.

Validation:
- The drain and the success wrap were re-read. Tests were not run.

Non-goals:
- Do not add per-parent session ownership. Global send, list, and interrupt remain the accepted single-operator behavior.
- Do not remove the depth cap or claim the turn cap is missing.

### MCP-GROK-002: automation_script failure is still a successful tool result

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `crates/gosling-mcp/src/computercontroller/mod.rs:994-1014` formats “Script failed with error code …” when `status.success()` is false, then returns `Ok(CallToolResult::success(...))`.
- Developer shell uses `CallToolResult::error` on a non-zero exit (operations lane cited `developer/shell.rs:469-502`; that shell arm was not re-opened during synthesis).

Observed behavior:
- The words say the script failed. The MCP flag says success.

Expected boundary:
- A non-zero script exit is `is_error: true`, consistent with the developer shell.

Failure mechanism:
- The status is interpolated into a success result.

Break-it angle:
- A script that exits non-zero is shown to the model as a successful tool call whose text contains “failed.”

Impact:
- Callers that trust `is_error` treat a failed automation script as success. The text is a partial mitigation.

Operational impact:
- Blast radius: Workflow
- Side-effect class: process
- Reversibility: unknown
- Operator visibility: UI-visible
- Rerun safety: unsafe

Adjacent failure modes:
- `MCP-GROK-001` does not even read the status on the non-macOS computer-control path.

Recommended mitigation:
- Remediation patterns: return `CallToolResult::error` when `status.success()` is false.
- Minimal repair: that return.
- Local guardrail: `is_error` follows status.
- Behavior test: success status stays success; non-success sets `is_error`.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: tests
- Nominal implementation agent: codex
- Rationale: one return beside an existing shell pattern.

Validation:
- The return was re-read. Tests were not run.

Non-goals:
- Do not merge computer control and the developer shell into one runner in this change.

### REL-GROK-001: Spill refusal copies the full tool body back into the conversation

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Reliability

Evidence:
- `crates/gosling/src/agents/large_response_handler.rs:11-12` caps the cache at 64 MiB and 1000 files.
- `large_response_handler.rs:137-141` returns an error before creating a file when the body exceeds the byte cap.
- `large_response_handler.rs:186-191` returns an error and keeps existing files when the cache is full.
- `large_response_handler.rs:63-74` on that `Err` formats “Showing full content instead” and appends `text_content.text`.
- `large_response_handler.rs:93-94` returns `Ok(result)` with that content.
- Spill is attempted only after the character threshold (default 200_000). The byte cap is separate.

Observed behavior:
- The file cache is capped. When the write is refused, the agent still accepts the tool result and puts the original text in the message the model will see next.

Expected boundary:
- A response that cannot be spilled is not inserted whole. The tool result is a short refusal that names the limit.

Failure mechanism:
- The cap is enforced inside `write_large_text_to_file`. The caller treats every write error as a reason to inline the payload.

Break-it angle:
- A tool text larger than 64 MiB, or another over-threshold text after the cache is full, is embedded in the next conversation message. Heap effect was not measured and is not claimed.

Impact:
- One tool result can put an arbitrarily large string into session history after the spill guard refused to store it. That defeats the memory bound the cap was added for. Process death was not observed.

Operational impact:
- Blast radius: Workflow
- Side-effect class: none
- Reversibility: compensatable
- Operator visibility: UI-visible
- Rerun safety: safe

Adjacent failure modes:
- Image and non-text parts pass through unchanged (`86-88`). No size cap was read for those parts. Not filed.
- The dataflow lane filed no second spill finding. This is the only spill defect in this report.

Recommended mitigation:
- Remediation patterns: `admission_bound`, `fail_visible`.
- Minimal repair: on write failure, push a short error that includes the error string and the character count, not `text_content.text`.
- Local guardrail: keep the file cap and the startup sweep.
- Behavior test: a body over `MAX_RESPONSE_CACHE_BYTES` yields a tool message that does not contain the body and whose length is under a small fixed bound.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: one error arm. Confidence is Confirmed for the copy. It is not Confirmed for OOM.

Validation:
- The `Err` arm and the cap returns were re-read. Tests were not run.

Non-goals:
- Do not delete previously advertised spill files when the cache is full.
- Do not change the 200_000 character default.

### SEC-GROK-001: Above-threshold explanations copy command text into info logs

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `crates/gosling/src/security/scanner.rs:434-471` `build_explanation`. The degraded branch returns before the copy (`440-444`). Once confidence is at least the threshold, the function copies up to 300 characters of command preview and up to 50 characters of `matched_text`.
- `crates/gosling/src/security/security_inspector.rs:44-47` assigns `reason: security_result.explanation.clone()`.
- `crates/gosling/src/tool_inspection.rs:299-307` logs `inspector.reason` at `tracing::info` for every inspection result.
- `security_inspector.rs:62-68` does not auto-downgrade. That keeps the approval. It does not stop the log line.
- The dedicated scan event is key-only (security lane cited `security/mod.rs:26-38`). That event is held. This finding is the inspection-result event.

Observed behavior:
- An above-threshold scan puts command text into the explanation, and the inspection-result info log stores that explanation.

Expected boundary:
- Security logs keep keys and a finding id. Command text and token-bearing argument values stay out of `tracing` fields. The approval dialog may still show the explanation to the operator.

Failure mechanism:
- The operator-facing explanation is also the structured log field.

Break-it angle:
- A tool call at or above threshold causes the info log, and any export of that line, to contain the command preview and the matched snippet.

Impact:
- Local disclosure of command text, and of secrets that were inside the flagged arguments, to anyone who can read gosling logs. The call is still forced to approval or block. This is not an execution bypass.

Operational impact:
- Blast radius: Local
- Side-effect class: none
- Reversibility: compensatable
- Operator visibility: log-only
- Rerun safety: unsafe

Adjacent failure modes:
- `SEC-GROK-002` is a separate URL log sink.

Recommended mitigation:
- Remediation patterns: keep the preview in the approval message, and log a redacted reason (finding id, risk label, argument keys).
- Minimal repair: stop placing `command_preview` and `matched_text` in the `tracing` field.
- Local guardrail: an info event for an above-threshold shell call does not contain the command body.
- Behavior test: that assertion.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: grok
- Rationale: split the UI reason from the log reason. A human should confirm log consumers do not depend on the preview.

Validation:
- `build_explanation`, the reason clone, and the info log were re-read. Tests were not run.

Non-goals:
- No change to detection thresholds.
- The approval UI showing the explanation is intentional.

### SEC-GROK-002: Egress logs and approval reasons keep the full URL match

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `crates/gosling/src/security/egress_inspector.rs:51-63` stores the full URL regex match as `destination`.
- `egress_inspector.rs:451-482` logs that string at info (`network.destinations` and `network.destination`) and formats `reason` from the same full strings.
- `tool_inspection.rs:306` logs `inspector.reason` again. The Auto downgrade line at `tool_inspection.rs:145` logs `inspector.reason` as well.

Observed behavior:
- For a shell or web tool that yields at least one destination, the inspector logs the full match. The branch that then decides the literal is not egress still logs the joined destinations (`450-462`).

Expected boundary:
- Domain and direction are enough to enforce and to audit. Userinfo and query strings stay out of info logs.

Failure mechanism:
- The captured match is not reduced to scheme plus host before it is placed in a tracing field or in `InspectionResult.reason`.

Break-it angle:
- Any inspected command or URL argument whose literal contains userinfo or a query string is written to info logs as part of normal egress detection.

Impact:
- Secrets placed in URL userinfo or query parameters are copied into operator logs. This is independent of whether the request is later allowed, denied, or sent for approval.

Operational impact:
- Blast radius: Local
- Side-effect class: none
- Reversibility: compensatable
- Operator visibility: log-only
- Rerun safety: unsafe

Adjacent failure modes:
- `SEC-GROK-001` is the scanner preview sink.
- `SEC-GROK-003` is the allow decision that uses these destination objects.

Recommended mitigation:
- Remediation patterns: log and display the normalized host. Keep the raw match only in memory for the decision, or redact userinfo and query before formatting `reason`.
- Minimal repair: that reduction at the format sites.
- Local guardrail: info fields equal the host, not the raw match.
- Behavior test: that assertion.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: grok
- Rationale: local formatting change. No change to which hosts are allowed.

Validation:
- The regex, the info fields, and the reason format were re-read. Tests were not run.

Non-goals:
- No change to the allow policy (`SEC-GROK-003`).
- No sample URLs.

### WFG-GROK-001: Disabled chat Send is still invoked by Enter

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/components/ChatInput.tsx:1422-1430` `isSubmitButtonDisabled` includes image loading, dropped-file loading, recording, transcribing, and `ChatState.RestartingAgent`.
- `ChatInput.tsx:1162-1169` `canSubmit` does not include those conditions.
- `ChatInput.tsx:1274-1275` bare Enter calls `performSubmit` when `canSubmit` is true.
- `ChatInput.tsx:1291-1293` the submit shortcut does the same.
- `ChatInput.tsx:1296-1314` form submit returns early only for `submitDisabled` and `queueProcessingBlocked`, then uses a local `canSubmit` that also omits the loading, recording, and restarting checks.
- `ChatInput.tsx:1171-1179` `performSubmit` returns early only for `submitDisabled`, then sends finished images. `clearInputState` follows an accepted submit (`1158`).

Observed behavior:
- The Send control is disabled while an image is loading, while dictation is active, or while the agent is restarting. The keyboard and form paths do not share that guard.
- Text already in the composer can be submitted, and a still-loading image is omitted and then cleared.

Expected boundary:
- A disabled send control is also blocked on Enter, the submit shortcut, and the form event. Images that were not included stay in the composer.

Failure mechanism:
- The button’s `disabled` expression is wider than `canSubmit` and wider than `onFormSubmit`.

Break-it angle:
- Start an image paste so the button is disabled, leave text in the composer, and press Enter. The send path runs and the unfinished image is dropped.

Impact:
- The operator can send a message the button said they could not send, without the attachment that was still loading.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- Negative-space alternate path (`NEG-009`). Filed here, not as a second ID.

Recommended mitigation:
- Remediation patterns: one predicate for the button, Enter, the shortcut, and form submit.
- Minimal repair: that shared predicate, and do not clear images that were not included.
- Local guardrail: Enter does not submit while an image is loading, while recording, or while restarting.
- Behavior test: those three states, and the loading image remains.

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Cost drivers: tests
- Nominal implementation agent: codex
- Rationale: one component and a UI test.

Validation:
- The predicates and Enter path were re-read. The UI was not driven. No browser session was opened.

Non-goals:
- Do not change queue-while-streaming behavior.

### FSR-GROK-001: Stale-backend sweep signals the serve pid, not its process group

Severity: Medium
Confidence: Likely
Evidence basis: source-evidenced
Domain: Failsafe

Evidence:
- `ui/desktop/src/goslingServe.ts:509-518` spawns `gosling serve` with `detached: true`.
- `goslingServe.ts:698-723` `signalBackendTree` uses `process.kill(-goslingProcess.pid, signal)` on Unix.
- `ui/desktop/src/backendProcessRegistry.ts:208-239` `terminateProcess` uses `process.kill(pid, 'SIGTERM')` and later `SIGKILL` with a positive pid. Windows uses `taskkill /t`.
- The reliability lane cited shell-owned adapters that do not call `process_group(0)` (`subprocess.rs:59-66`) so they stay in the backend group, and sweep call sites at `main.ts:1717-1720`, `main.ts:1935-1938`, and `shellHost.ts:51-52`. Those call sites were not re-opened during synthesis.

Observed behavior:
- A live lease cleanup signals the backend process group. The registry sweep used after a crash, and again at quit, signals only the recorded pid.

Expected boundary:
- The crash and quit sweep uses the same group signal as `signalBackendTree`, then treats a surviving group member as still running.

Failure mechanism:
- `detached: true` makes the serve pid the process-group id. `process.kill(pid)` does not deliver the signal to other members. Default Unix process death does not run Rust `Drop`.

Break-it angle:
- Kill the Electron parent without going through lease cleanup. On the next launch the sweep SIGKILLs the recorded serve pid. A domain-adapter child that was still in that process group is left running and is no longer in the registry. That orphan was not observed.

Impact:
- Static evidence confirms the sweep does not signal the group. A leftover shell-owned child after a desktop crash is Likely, not Confirmed.

Operational impact:
- Blast radius: Local
- Side-effect class: process
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- `FSR-GROK-002` covers extension groups that are not in this backend group.

Recommended mitigation:
- Remediation patterns: `safe_shutdown`.
- Minimal repair: on Unix, signal `-record.pid` the same way `signalBackendTree` does, then wait. Keep `taskkill /t` on Windows.
- Local guardrail: do not drop a record solely because the leader pid is gone unless the group is confirmed dead.
- Behavior test: a detached parent plus a child in the same group; after cleanup, the child pid is not alive.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests, runtime_verification
- Nominal implementation agent: codex
- Rationale: one terminate function. Needs a Unix process-group fixture. Confidence stays Likely until that fixture runs.

Validation:
- Spawn options, the negative-pid supervisor, and the positive-pid sweep were re-read. No process was killed.

Non-goals:
- Do not change which adapters stay in the backend group.
- Do not kill unrelated processes that match `gosling serve` on the command line.

Resilience mapping:
- Phase: recover
- Objective(s): constrain, reconstitute
- Safe state: fail_visible

Failure analysis (FMECA row):
- Failure mode: crash sweep reaps the backend leader and does not signal same-group children
- Likely cause: `terminateProcess` signals a positive pid
- Operational phase: recovery
- Local effect: leader can exit while other group members are not signaled
- Workflow effect: the next desktop launch can believe the backend tree is gone
- System-or-operator effect: adapter processes can keep running with no registry entry
- Detection method: none
- Detection latency: delayed
- Operator visible: false
- Compensating provision: `signalBackendTree` is a different caller

### FSR-GROK-002: Extension process-group kill is disarmed after a successful MCP start

Severity: Medium
Confidence: Likely
Evidence basis: source-evidenced
Domain: Failsafe

Evidence:
- `crates/gosling/src/agents/extension_manager/child_process.rs:134-157` `StartupProcessGroup` sends `killpg(SIGKILL)` on drop. The comment says rmcp kills only the direct child.
- `child_process.rs:299-306` calls `startup_process_group.disarm()` when `McpClient::connect_with_container` succeeds, and drops the guard only on the error path.
- The reliability lane cited `configure_subprocess` setting `process_group(0)` (`subprocess.rs:52-73`) and rmcp 1.7.0 `ChildWithCleanup::drop` killing the direct child only. Those two sites were not re-opened during synthesis.

Observed behavior:
- Abandoned startup kills the extension process group.
- After a successful connect, the group-kill guard is cleared.

Expected boundary:
- The same group kill runs when the extension client is closed, dropped, or the parent is leaving.

Failure mechanism:
- The guard that exists because rmcp does not kill the group is cleared on the success path. Descendants in the extension’s own process group are then outside the desktop backend group as well.

Break-it angle:
- Start a stdio extension whose launcher forks a worker, then remove the extension or SIGKILL the agent. The worker can remain. That outcome was not observed.

Impact:
- The missing `killpg` after success is a Confirmed code property in the sense that `disarm` clears the guard. Survival of grandchildren is Likely. Confidence for the finding is Likely.

Operational impact:
- Blast radius: Local
- Side-effect class: process
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- `FSR-GROK-001` does not reach this group even after the sweep is fixed.

Recommended mitigation:
- Remediation patterns: `safe_shutdown`.
- Minimal repair: keep the process-group id on the running client and `killpg` it from close and from a drop fallback, after rmcp’s direct-child shutdown.
- Local guardrail: do not disarm the guard until the group has been reaped or handed to that lifetime owner.
- Behavior test: after client close, a descendant started in that group is not alive. The startup-abandon test still passes.

Implementation assessment:
- Complexity: cross_process_coordination
- Cost: M
- Cost drivers: modules, tests, runtime_verification
- Nominal implementation agent: codex
- Rationale: the pid is known at spawn. The missing piece is owning it for the session. Must not kill a group the extension intentionally daemonized outside its group.

Validation:
- The disarm-on-success match was re-read. No process was killed.

Non-goals:
- Do not put extension servers back into the Gosling process group.

Resilience mapping:
- Phase: recover
- Objective(s): constrain
- Safe state: fail_closed

Failure analysis (FMECA row):
- Failure mode: extension descendants can survive a successful session
- Likely cause: group kill is armed only until connect succeeds
- Operational phase: shutdown
- Local effect: the direct child may be killed; other group members are not signaled by this guard
- Workflow effect: extension removal can report the client closed
- System-or-operator effect: worker processes can remain with no owner in Gosling
- Detection method: none
- Detection latency: delayed
- Operator visible: false
- Compensating provision: `kill_on_drop` on the direct child only, and only if that handle is dropped

### FSR-GROK-003: Text UI quit and text mode skip the grace-and-SIGKILL shutdown

Severity: Medium
Confidence: Likely
Evidence basis: source-evidenced
Domain: Failsafe

Evidence:
- `ui/text/src/tui.tsx:1590-1629` `shutdown` waits up to `SHUTDOWN_GRACE_MS` (2000) then sends SIGKILL. The comment says `cleanup(); process.exit` orphaned a child that ignored or was slow to handle SIGTERM.
- `tui.tsx:1594-1597` `cleanup` only calls `serverProcess.kill()` and returns.
- `tui.tsx:1574-1577` text mode calls `cleanup` and returns.
- `tui.tsx:1586-1587` interactive mode calls `cleanup` after `waitUntilExit`.
- `tui.tsx:1631-1637` `shutdown` is wired to `SIGINT`, `SIGTERM`, and `main().catch`. `process.on("exit", cleanup)` runs SIGTERM while the process is already exiting.

Observed behavior:
- The grace period and SIGKILL exist. `/exit`, the Ink `exit()` path, and a finished `--text` run call `cleanup`, not `shutdown`.

Expected boundary:
- Every parent exit that owns the ACP child uses `shutdown`.

Failure mechanism:
- The repair added a correct function and attached it to OS signals. Ink’s `exit()` resolves `waitUntilExit`, and `main` then calls the one-shot kill.

Break-it angle:
- Quit the TUI with `/exit` or a second Ctrl-C while `gosling acp` is still shutting down. The parent does not wait or escalate. The leftover child was not observed.

Impact:
- The missing call is in source. A child left mid-shutdown is Likely. This pass did not run the TUI.

Operational impact:
- Blast radius: Local
- Side-effect class: process
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- `FSR-GROK-002` if the ACP child’s extension groups are not reaped when it dies by signal.

Recommended mitigation:
- Remediation patterns: `safe_shutdown`, `bounded_wait`.
- Minimal repair: route `cleanup` callers and the text-mode path through `shutdown`.
- Local guardrail: a child that traps SIGTERM is dead within the grace bound after the TUI quit path.
- Behavior test: that sequence, and `--text` failure uses the same sequence.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: the helper already exists. The change is which function the quit paths call.

Validation:
- `cleanup`, `shutdown`, and the text-mode and interactive callers were re-read. The TUI was not run.

Non-goals:
- Do not change `session/cancel` for the first Ctrl-C during a turn.
- Do not detach the ACP child.

Resilience mapping:
- Phase: recover
- Objective(s): constrain, prepare
- Safe state: fail_closed

Failure analysis (FMECA row):
- Failure mode: normal text-UI exit does not escalate a slow ACP child
- Likely cause: quit calls `cleanup` rather than `shutdown`
- Operational phase: shutdown
- Local effect: one SIGTERM and the parent continues exiting
- Workflow effect: no grace timer is armed on those paths
- System-or-operator effect: the child can outlive the UI
- Detection method: none
- Detection latency: delayed
- Operator visible: false
- Compensating provision: `shutdown` on SIGINT and SIGTERM only

### ARC-GROK-004: Workspace mutations report not-found and storage failures as invalid params

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `docs/architecture.md:347-356` taxonomy: validation is invalid params; not found is resource not found; storage (malformed file, lock or rename failure) is internal error.
- `crates/gosling/src/acp/server/workspace_handlers.rs:40-44` and `54-58` map `create` and `update` through `invalid_params_err`. `get` and `duplicate` at `66-75` do the same. List uses `internal_err_ctx` (`23-26`).
- `crates/gosling/src/workspace/service.rs:210` returns `anyhow!("workspace not found")` from `update`.
- The architecture lane cited the same helper on the remaining workspace mutations and cited `store.rs` lock and IO errors sharing that `Result`. Those extra call sites were not each re-opened during synthesis. The create and update wraps are enough to show the collapse.

Observed behavior:
- Create and update collapse validation failures, `workspace not found`, and any other `Err` from the service into ACP `invalid_params`.

Expected boundary:
- Validation stays invalid params. A missing workspace is resource-not-found. Lock, read, and rename failures are internal errors with a safe summary.

Failure mechanism:
- `invalid_params_err` keeps `Display` text and discards the error class. Clients that branch on ACP error code cannot tell a bad name from a missing id or a failed lock.

Break-it angle:
- Update of an unknown id returns `workspace not found` and the handler wraps it as `invalid_params`. A lock error from the same `Result` takes the same wrap. Whether Desktop branches on the code was not traced.

Impact:
- Wrong recovery: a client can show a form error or retry a “bad request” while the store is locked or the id is gone. No evidence this bypasses authorization.

Operational impact:
- Blast radius: Service
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: log-only
- Rerun safety: safe

Adjacent failure modes:
- Session-schema error handling was not classified.

Recommended mitigation:
- Remediation patterns: a typed error enum, mapped in the workspace handlers.
- Minimal repair: not-found to resource-not-found, storage and lock errors to internal error, validation left on invalid params.
- Local guardrail: a missing id and a forced lock error produce different ACP codes, with no secret text in `.data`.
- Behavior test: those two codes.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: the taxonomy is already written. The helper is the wrong default for this seam.

Validation:
- Taxonomy, create, update, and the not-found string were re-read. Tests were not run.

Non-goals:
- Do not change session-schema error handling in this slice.
- Do not rewrite `ResultExt` for every ACP method.

### FSR-GROK-004: Rejected GitHub sign-in is reported removed even when deletion fails

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Failsafe

Evidence:
- `crates/gosling/src/providers/githubcopilot.rs:414-427` `forget_rejected_sign_in`.
- Both `delete_secret` and `cache.clear` errors are `tracing::warn` only.
- The returned `ProviderError::Authentication` always says the sign-in was removed and that the next request will start GitHub sign-in again.
- `refresh_api_info` at `429-432` uses the secret when `get_secret` succeeds.

Observed behavior:
- If delete or cache clear fails, the operator still gets the removed-and-sign-in-again message. A secret that is still stored will be loaded by the next refresh.

Expected boundary:
- If removal fails, the error says the token is still stored and includes the removal error.

Failure mechanism:
- The success sentence is unconditional. Warn logs are not the error returned to the operator. The explicit cleanup path repaired as `REL-GSL-001` is a different function.

Break-it angle:
- A secret store that rejects the delete still produces the removed sentence. The next request can load the same token. The store rejection was not executed.

Impact:
- The message the operator follows can be false. The false sentence itself is Confirmed. A live retry loop was not run.

Operational impact:
- Blast radius: Workflow
- Side-effect class: external API
- Reversibility: reversible
- Operator visibility: UI-visible
- Rerun safety: unsafe

Adjacent failure modes:
- Explicit provider removal still returns errors (held `REL-GSL-001`, earlier pass and reliability lane).

Recommended mitigation:
- Remediation patterns: `fail_visible`.
- Minimal repair: if either delete fails, return an authentication error that includes that failure and says the saved sign-in is still present. Use the current sentence only when both deletes return `Ok`.
- Local guardrail: a `delete_secret` error does not say the sign-in was removed.
- Behavior test: that message, and a following token read still sees the secret.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: one function. The explicit cleanup path already returns the error.

Validation:
- The function was re-read. Tests were not run.

Non-goals:
- Do not change when a 4xx counts as a rejected sign-in.
- Do not change `cleanup()`.

Resilience mapping:
- Phase: withstand
- Objective(s): understand
- Safe state: fail_visible

Failure analysis (FMECA row):
- Failure mode: the operator is told a dead OAuth token was removed when the delete was only logged
- Likely cause: removal errors are warned and the success text is unconditional
- Operational phase: normal_run
- Local effect: the secret and the disk cache can both remain
- Workflow effect: the next request can load the same token
- System-or-operator effect: the user follows a sign-in instruction that will not start
- Detection method: log
- Detection latency: delayed
- Operator visible: false
- Compensating provision: none on this function

### REL-GROK-002: Extension-state save failure is logged and the turn still treats the install as durable

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Reliability

Evidence:
- `crates/gosling/src/agents/agent/reply_stream.rs:961-965`: if `save_extension_state` fails, the code warns and still sets `tools_updated = true`.
- The reliability lane cited `reply_stream.rs:1384-1391` reloading tools when `tools_updated` is set, and `persist_extension_state` in `agents/agent/extensions.rs:48-75` as the write. Those later sites were not re-opened during synthesis. The warn-and-continue at `961-965` is the defect.

Observed behavior:
- Runtime extension installs that returned success continue as a tool-updated turn when the session metadata write fails. The transcript gets no error from this branch.

Expected boundary:
- A failed `save_extension_state` fails the turn or appends an operator-visible message that the extension set was not saved. `tools_updated` does not imply durability.

Failure mechanism:
- The error is discarded after `warn!`.

Break-it angle:
- A session store that rejects the extension-state merge after a successful enable still lets the model proceed with the new tools. After restart, the enabled set is the previous one. The rejection was not executed. The discarded error is in source.

Impact:
- Session resume can disagree with the conversation the operator just saw. The code property is Confirmed. A restart demonstration was not run.

Operational impact:
- Blast radius: Workflow
- Side-effect class: DB
- Reversibility: compensatable
- Operator visibility: log-only
- Rerun safety: unsafe

Adjacent failure modes:
- Tool-response persistence failure aborts (reliability lane held that nearby path; not re-opened here).
- `FSR-GROK-004` is a different swallowed error.

Recommended mitigation:
- Remediation patterns: `fail_visible`.
- Minimal repair: propagate `save_extension_state` as a turn error, or yield a visible system message and skip `tools_updated` until a later successful save.
- Local guardrail: a save error does not leave the stored enabled set looking current.
- Behavior test: the error is visible, and a successful save still sets `tools_updated`.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: codex
- Rationale: the abort pattern for tool persistence already exists nearby. The behavior change is that an install can surface a save failure.

Validation:
- Lines `961-965` were re-read. Tests were not run.

Non-goals:
- Do not change extension enable permission checks.
- Do not rewrite the whole transcript on this failure.

### ARC-GROK-001: Guide registers providers whose declarative catalogs are not shipped

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `documentation/docs/getting-started/providers.md:33` EmpirioLabs row with `EMPIRIOLABS_API_KEY`.
- `providers.md:34` FuturMix row with `FUTURMIX_API_KEY`.
- `providers.md:54` Routstr row with `ROUTSTR_API_KEY` and `ROUTSTR_HOST`.
- The architecture lane also cited guide links to `futurmix.json`, `routstr.json`, and `empiriolabs.json`, and Configure Providers steps for those names. Those later anchors were not re-opened during synthesis.
- Directory listing of `crates/gosling/src/providers/declarative/`: 23 JSON files. `futurmix.json`, `routstr.json`, and `empiriolabs.json` are absent. `novita.json` is present.
- `crates/gosling-providers/src/canonical/data/provider_metadata.json` contains `"id": "empiriolabs"`. FuturMix and Routstr ids are absent from that file.
- `crates/gosling/src/config/declarative_providers.rs` loads fixed providers from that directory (architecture lane cited `include_dir!` at line 21 and registration at `472-478`).

Observed behavior:
- The guide tells operators to use FuturMix, Routstr, and EmpirioLabs and points at declarative catalogs. Those three JSON files are not in the tree, so they are not registered as fixed providers.
- EmpirioLabs exists only as canonical metadata. FuturMix and Routstr are absent from that metadata as well.

Expected boundary:
- A provider the guide tells the operator to select is registered from a file that exists, or the guide describes the custom-provider catalog and does not cite a missing JSON path.

Failure mechanism:
- `include_dir!` embeds only files in that directory. Configure lists registry metadata. An operator following the guide reaches a list that cannot contain these ids.

Break-it angle:
- Resolve the three JSON paths. They are missing. No configure session was run.

Impact:
- Documented setup for three named providers fails closed at selection. EmpirioLabs might still be addable through the custom-provider catalog because metadata contains it. That is not the Configure Providers path the guide describes.

Operational impact:
- Blast radius: Service
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- Novita remains a shipped declarative provider. It is not part of this finding.

Recommended mitigation:
- Remediation patterns: point the guide at providers the registry actually loads, or add the declarative JSON and curated setup entries in one change.
- Minimal repair: retarget or remove the FuturMix and Routstr sections. For EmpirioLabs, either add `empiriolabs.json` plus a setup entry or document Custom Providers and stop linking a missing file.
- Local guardrail: every declarative JSON link in that guide exists, and every Configure Providers name is a registry id.
- Behavior test: `providers()` display names include only shipped ids.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: docs
- Nominal implementation agent: grok
- Rationale: a file-and-doc mismatch. No runtime algorithm change is required to stop advertising missing providers.

Validation:
- The three table rows and the declarative directory were re-read. No configure session was run.

Non-goals:
- Do not invent model lists for FuturMix or Routstr.
- Do not treat `provider_metadata.json` as the Configure Providers list.

### ARC-GROK-002: Guide says tool_permissions.json is auto-managed; no product caller exists

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `documentation/docs/guides/config-files.md:21` says `permissions/tool_permissions.json` is runtime permission decisions and auto-managed.
- `config-files.md:19` names `permission.yaml` as the file `gosling configure` writes.
- `crates/gosling/src/permission/permission_store.rs:23-24` documents `ToolPermissionStore` as a legacy public store and says live tool policy uses `PermissionManager`.
- A repo search for `ToolPermissionStore` in Rust and TypeScript matches only `permission/mod.rs` (the re-export) and `permission_store.rs` (the type). No product caller constructs it.

Observed behavior:
- The guide tells operators the JSON file is auto-managed runtime state. Nothing in the product calls the store.

Expected boundary:
- The config guide names the file the running agent reads and writes. A public legacy type, if kept, is described as unused by the product.

Failure mechanism:
- An operator editing `permissions/tool_permissions.json` does not change tool policy, because no product path calls load or save. The live file is `permission.yaml`.

Break-it angle:
- Symbol census: `ToolPermissionStore` has no use site outside its module and the re-export.

Impact:
- Misdirected diagnosis of permission denials. No second policy is applied. The public type remains callable by external crate users, so this is not a deletion recommendation.

Operational impact:
- Blast radius: Local
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- The durability repair on the unwired `save` path (historical `ARC-GSL-005`) can stay. It does not make the file live.

Recommended mitigation:
- Remediation patterns: change the guide sentence. Keep the public type until an explicit API removal.
- Minimal repair: say the JSON file is a legacy API surface the app does not write, and that live decisions are `permission.yaml`.
- Local guardrail: the guide no longer says auto-managed for that file.
- Behavior test: the caller census stays empty.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: docs
- Nominal implementation agent: grok
- Rationale: one sentence. The source comment is already accurate.

Validation:
- The guide line, the struct comment, and the caller census were re-read.

Non-goals:
- Do not delete `ToolPermissionStore` in the doc fix.

### ARC-GROK-003: docs/INDEX.md still calls the v1.4.0 notes the current source

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `docs/INDEX.md:96` labels `documentation/docs/release-notes/v1.4.0.md` as current source and local build notes.
- `README.md:76-79` says the current source version is 1.5.0 and that the v1.4.0 notes are installation evidence that does not verify a later source revision.
- `Cargo.toml:13` `version = "1.5.0"`.
- `ui/desktop/package.json:4` `"version": "1.5.0"`.

Observed behavior:
- README and the manifests say 1.5.0. The documentation index, which `AGENTS.md` tells agents to read, still labels v1.4.0 as current source.

Expected boundary:
- The index entry matches the workspace version and README.

Failure mechanism:
- An agent or operator who follows the index reads v1.4.0 behavior as current.

Break-it angle:
- Compare the index link target with `Cargo.toml` `workspace.package.version`. They disagree. No install was performed.

Impact:
- Stale operating instructions. No runtime binary reads this line. This audit’s new index line does not change line 96. The finding stays open until that phrase is edited on purpose.

Operational impact:
- Blast radius: Local
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- None.

Recommended mitigation:
- Remediation patterns: point the index at `v1.5.0.md` as current source, and keep v1.4.0 labeled as the September 29 verified install, matching README.
- Minimal repair: that one line.
- Local guardrail: the index version phrase equals `workspace.package.version`.
- Behavior test: a diff review. No runtime test.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: docs
- Nominal implementation agent: grok
- Rationale: README already has the distinction.

Validation:
- The four citations were re-read. The index was not “fixed” by this audit.

Non-goals:
- Do not rewrite the v1.4.0 notes.

### WFG-GROK-002: Guide attributes database readiness to /health and /status together

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Workflow-GUI

Evidence:
- `documentation/docs/guides/remote-gosling-server.md:79-80` says `/health` and `/status` disclose liveness and database readiness.
- `remote-gosling-server.md:85-90` tells the operator to curl `/status` and says a successful status response proves the database.
- The operations and reliability lanes cite `crates/gosling/src/acp/transport/mod.rs:259-262` (`health` returns `"ok"` and does no I/O) and `270-276` (`/status` calls `sessions.healthy()` and returns 503 on failure). Those handler bodies were re-read in the earlier pass of this engagement and were not re-opened during synthesis. The grouped sentence was re-read.

Observed behavior:
- The gloss treats `/health` as a database-readiness signal. The handler is process liveness. The curl example that follows is `/status` and matches the code.

Expected boundary:
- `/health` is described as liveness only. Database readiness is attributed only to `/status`.

Failure mechanism:
- One sentence groups the two routes.

Break-it angle:
- With the session database unavailable, `GET /health` still returns ok. An operator who trusts the grouped sentence will treat that as readiness. This pass did not call the routes.

Impact:
- Mis-diagnosis if `/health` is used instead of the curl the guide prints. Unauthenticated probes are documented and intentional. `SEC-GSL-003` stays repaired.

Operational impact:
- Blast radius: Local
- Side-effect class: none
- Reversibility: reversible
- Operator visibility: UI-visible
- Rerun safety: safe

Adjacent failure modes:
- None.

Recommended mitigation:
- Remediation patterns: split the sentence. `/health` is liveness and does not check the database. `/status` is readiness.
- Minimal repair: that sentence.
- Local guardrail: the word “database” is not applied to `/health`.
- Behavior test: doc review.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: docs
- Nominal implementation agent: grok
- Rationale: one sentence.

Validation:
- The guide sentences were re-read. The handlers were not re-opened in the synthesis pass. No HTTP call was made.

Non-goals:
- Do not add a database check to `/health`.
- Do not re-open unauthenticated health as a defect.

### ARC-GROK-005: Shell-productization resume commands cd to this checkout

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `docs/build/shell-productization/build-state.md:345` under “Verify-don't-trust resume commands” is `cd /Users/eric/Work/vscode/forked/gosling`.
- The architecture lane cited the same absolute `cd` in `docs/build/shell-productization/gui/gate-3-build-record.md:200`. That second file was not re-opened during synthesis.

Observed behavior:
- The resume procedure’s first command is an absolute path of this operator’s current tree.

Expected boundary:
- A resume block says to run from the repository root so a moved checkout can run it.

Failure mechanism:
- Copying the block after the repo is relocated fails at `cd` before the rest of the procedure. Runtime gosling does not read this file.

Break-it angle:
- Static path read. Relocation was not performed.

Impact:
- Operator resume friction only.

Operational impact:
- Blast radius: Local
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: UI-visible
- Rerun safety: safe

Adjacent failure modes:
- Historical logs and `.giles/` `repo_path` fields also contain absolute paths. Those are run records, not this finding.

Recommended mitigation:
- Remediation patterns: replace the absolute `cd` with “from the repository root” in the procedure files.
- Minimal repair: the resume block in `build-state.md`, and the same command in the gate-3 record if it is still a procedure.
- Local guardrail: procedure fences under `docs/build/` do not hard-code this home path.
- Behavior test: a diff review.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: docs
- Nominal implementation agent: grok
- Rationale: one root, used as a procedure.

Validation:
- `build-state.md:345` was re-read. Relocation was not performed.

Non-goals:
- Do not scrub `.giles/` or `docs/cloud/` historical reports.

## Non-Findings / Checked But Not Confirmed

October 12 repairs that still match current source, so they are not re-filed:

- Website-login destination check refuses code and write tools, requires an explicit http(s) URL on the login origin, and the inspector requires approval including in Auto. Earlier pass this engagement: `website_logins.rs` checked destination and `website_login_inspector.rs`.
- Renderer directory grants are requests. Sensitive and overly broad roots are skipped, and new roots need a native confirmation. Earlier pass: `rendererIpc.ts` grant handler. A native picker can still persist a user-chosen sensitive directory after that confirmation. That is operator consent, not the old self-grant.
- `pr-smoke-test.yml` triggers are `push` to `main` and `workflow_dispatch`. The check-fork job requires `github.ref == refs/heads/main`. Secrets remain on that main-only workflow. Owner rotation of secrets exposed before this gate is external and unverified.
- Peekaboo is not installed by `ensure_peekaboo` (operations lane; not re-opened during synthesis).
- `/health` and `/status` are unauthenticated by design. The remote-server guide says they do not require a token. `/status` checks the session store. The grouped readiness sentence is `WFG-GROK-002`, not a new auth defect.
- Output revisions have no `session_id` and survive session deletion on purpose (`ADR-0018`). The data lane held the delete transaction and the commit-before-rename restore.
- Historical migration v24 reset of the workspace tool restriction remains the accepted historical disposition (`ADR-0017`, `STT-GSL-003`).
- Operator-selected files outside the workspace via the native picker remain allowed (`ADR-0015`).
- Explicit OAuth removal returns the removal error (`REL-GSL-001`, reliability lane). `FSR-GROK-004` is a different function.
- Provider `Retry-After` is capped (reliability lane; the old “uncapped” claim stays false).
- Session schema constant, migration arm, and `docs/architecture.md` agree on version 38 (data and architecture lanes).
- ACP custom method strings match the generated client (architecture lane: both set differences empty).
- Desktop `nodeIntegration: false` and `sandbox: true` (security lane cited `main.ts:1256` and `1261`).
- Smart-approve cannot Allow an unknown or side-effecting tool from the judge alone (security lane cited `permission_inspector.rs:241-248`).
- Sub-agent sessions cannot delegate further (security lane).
- Orchestrator list, send, and interrupt remain process-global. The repair log accepts that as single-operator behavior. Not re-filed.
- Developer shell non-zero exits set `is_error` (operations lane). That is the contrast for `MCP-GROK-002`.
- Gitignore updates take the shared process file lock (architecture lane, matching commit `e61fc45ab`).
- `ToolPermissionStore::save` uses the secrets writer. The type is unwired (`ARC-GROK-002`). The old non-fsync implementation is not present.
- Giles YAML is stale (operations lane: dated 2026-07-07, `compliance_status: blocked`) and its own authority block says advisory. No release workflow was found that fails on that file. No new compliance finding.

Dirty tree: `active_runs.rs` adds `cancel_token()` and `manage_sessions.rs` calls it. HEAD cloned the same token before delete. Same cancel-before-delete behavior. Not a finding. Not reverted.

## Break-It Review

No process was killed, no server was started, and no UI was driven. The attacks below are source traces.

Attacks the reviewed code does not survive on inspection:

- Auto plus an undecided host classified inbound: egress Allows (`SEC-GROK-003`).
- Auto plus a complete shell with no explicit path token while folder restriction is on: no scope result, then `-c` (`LLM-GROK-001`).
- Non-macOS `computer_control` with a helper that ignores exit status: success text (`MCP-GROK-001`).
- `automation_script` with a non-zero status: failure sentence inside `CallToolResult::success` (`MCP-GROK-002`).
- Child reply stream `Err`: success tool result containing the error sentence (`ORCH-GROK-001`).
- Enter while Send is disabled for a loading image: `performSubmit` (`WFG-GROK-001`).
- Tool body over 64 MiB: spill returns an error and the caller inlines the body (`REL-GROK-001`).
- Workspace update of a missing id: `invalid_params` (`ARC-GROK-004`).
- Registry sweep versus a same-group child: positive pid only. Survival is Likely (`FSR-GROK-001`).
- Successful MCP start, then parent death: group kill already disarmed. Survival is Likely (`FSR-GROK-002`).
- Text UI `/exit`: grace helper is not called. Survival is Likely (`FSR-GROK-003`).
- GitHub secret delete error: user-visible text still says removed (`FSR-GROK-004`).

Attacks the reviewed code survives on inspection:

- Stored NeverAllow still denies egress.
- Incomplete shell syntax still requires approval (`SEC-GSL-901`).
- A workspace with a read-only root still denies mutating shells (`AUD-GOS-003`).
- Website-login Auto downgrade stays off (earlier pass).
- Parent cancel and tool confirmation in the orchestrator return `Err`.
- Orchestrator depth cap and the 1000-turn default exist.
- `/status` is the route the guide’s curl uses for database readiness.
- Explicit OAuth cleanup errors propagate (held repair).
- PR live-provider secrets are gated to `main`.

## Recommended Patch Order

1. `SEC-GROK-003` and `LLM-GROK-001`. Align Auto egress and folder restriction with the written permission contract.
2. `MCP-GROK-001`. Thread exit status into non-macOS `computer_control` and stop the hard-coded success sentence.
3. `ORCH-GROK-001`, `MCP-GROK-002`, and `REL-GROK-001`. Make failure flags and the spill refusal tell the truth, and keep oversized tool bodies out of the conversation.
4. `FSR-GROK-003`, `FSR-GROK-001`, and `FSR-GROK-002`. Point text-UI quit at the existing shutdown helper, signal the recorded backend process group, and keep extension `killpg` for the life of the client.
5. `SEC-GROK-001` and `SEC-GROK-002`. Stop copying command previews and raw URL matches into info logs.
6. `WFG-GROK-001`, `FSR-GROK-004`, `REL-GROK-002`, and `ARC-GROK-004`. Share the Send predicate, tell the truth when GitHub token removal fails, surface extension-state save failure, and map workspace ACP errors to the taxonomy.
7. `ARC-GROK-001` and the Low documentation items (`ARC-GROK-002`, `ARC-GROK-003`, `WFG-GROK-002`, `ARC-GROK-005`).

Owner follow-up, not a code finding: rotate the CI LLM and Databricks secrets that were exposed before `SECN-GSL-201` was repaired. This pass confirmed the workflow gate. It did not confirm rotation.

## Regression Test Strategy

| Test | Purpose | Finding |
|---|---|---|
| Undecided host classified inbound does not Allow; Auto does not downgrade an egress RequireApproval; NeverAllow still denies | Egress contract | SEC-GROK-003 |
| Complete path-free non-read-only shell segment requires approval when folder restriction is on; explicit in-scope read of a read-only executable does not | Folder restriction | LLM-GROK-001 |
| Non-zero status and an unrecognized Linux automation command set `is_error` and omit the success sentence | Computer control | MCP-GROK-001 |
| Script status not success sets `is_error` | automation_script | MCP-GROK-002 |
| Reply stream `Err` sets `is_error` and is not prefixed as a normal completed response | Orchestrator | ORCH-GROK-001 |
| Body over `MAX_RESPONSE_CACHE_BYTES` yields a short tool message that does not contain the body | Spill cap | REL-GROK-001 |
| Above-threshold inspection info event does not contain the command body | Log redaction | SEC-GROK-001 |
| Egress info field equals the host, not the raw match | Log redaction | SEC-GROK-002 |
| Enter, shortcut, and form do not submit while an image is loading, while recording, or while restarting; the loading image remains | Composer | WFG-GROK-001 |
| Detached parent plus same-group child; after registry cleanup the child is not alive | Backend sweep | FSR-GROK-001 |
| After extension client close, a descendant in that group is not alive; startup-abandon still kills the group | Extension lifetime | FSR-GROK-002 |
| TUI quit path and `--text` failure kill a SIGTERM-trapping child within the grace bound | Text UI shutdown | FSR-GROK-003 |
| `delete_secret` error does not say the GitHub sign-in was removed | Copilot removal | FSR-GROK-004 |
| `save_extension_state` error is visible and does not mark the stored set current | Extension durability | REL-GROK-002 |
| Missing workspace id and a lock error produce different ACP codes | Workspace taxonomy | ARC-GROK-004 |
| Doc links for declarative providers resolve, and the index version phrase matches `Cargo.toml` | Docs | ARC-GROK-001, ARC-GROK-003 |

Assert behavior (flags, message bounds, process absence, error text). Do not assert source strings. None of these tests were run in this audit.

## Deferred Risks

- Live model-catalog drift (`LCD-001..003`). No network call. In-repo samples that were read (xAI context numbers, `claude-sonnet-4-0` alias) were held by the operations lane.
- Design-webapp gates 2–6, WCAG, focus order, contrast, and browser layout. Electron is a web-technology GUI. Gate 1 was a source sample and is not scored.
- Playtest of `docs/test_scenarios`. Applicable. Not executed. No scenario card was rewritten and no result was invented.
- Measured test-suite cost and performance. Static structure was not timed. No optimization finding met a measured-benefit bar. The orchestrator’s 1000-turn default versus the subagent default of 25 is a cost hypothesis, not a defect.
- OAuth HTTP redirect policy and Databricks discovery documents. Not re-read.
- Adversary-inspector prompt contents. Not re-filed.
- `configured_trusted_dirs` defaults. Relative entries are dropped. Whether any default is a wide absolute path was not surveyed.
- Dependency CVEs, GitHub branch protection, Dependabot, CodeQL, and secret-scanning enablement. No network and no GitHub settings API.
- Races, lock storms, OOM, and multi-worker behavior. None were reproduced. None are Confirmed.
- Image and non-text tool parts have no size cap in the spill function. Not filed.
- A poisoned live SQLite pool could still look ready if `/status` opens its own connection to a readable file (reliability lane). Not filed.
- Future multi-user use of process-global orchestrator sessions. Speculative. The current product decision is single-operator.

## Validation Limits

- No `cargo build`, `cargo test`, `cargo clippy`, `pnpm` typecheck, or UI test was run. Do not read this report as saying those passed.
- No GUI, browser, Ink render, or playtest.
- No process was launched or killed. `FSR-GROK-001`, `FSR-GROK-002`, and `FSR-GROK-003` therefore stay Likely.
- No live provider or model catalog.
- Detection-procedure bodies for the dataflow skills, and most shared `000_common/audit-base` files beyond the report template, finding format, confidence calibration, and cross-lens escalation, were not loaded file by file. Inventory codes whose procedures were not applied are Not Reviewed in the Coverage Matrix. They are not Held.
- `audit-architecture-seam` and `audit-invariant-sync` skill payloads were truncated in the earlier MCP load. On-disk `SKILL.md` files are the source. Truncated tails were not invented.
- The semantic skill index was stale. Suite selection used the deterministic fallback. The catalog was not refreshed.
- Findings are against HEAD `e61fc45ab`. The uncommitted `cancel_token()` accessor was excluded from the evidence and was not reverted.
- Windows `computer_control` helper and the macOS Peekaboo status check were read by the operations lane and not re-opened during synthesis. The non-macOS wrapper and the Linux X11 helper were re-read.
- This is a partial audit of a large tree. Absence of a finding in an unreviewed procedure code is a coverage fact.

## Final Confidence

Medium for the audit as a whole. High for the three High findings as code properties: the egress action table, the missing folder-scope result, and the non-macOS success wrapper were re-read. Medium overall because dataflow procedure coverage is mostly Not Reviewed, playtest and measurements were not run, and process-survival outcomes were not observed.

## v3.1 Mandatory Calibration Addendum

`confidence_calibration.md` v3.1 was applied. Source properties that deterministically lack a guard are Confirmed: the egress Allow, the missing scope result, the success wrappers, the spill re-inline, the log copies, the Send predicate split, the workspace error class, the GitHub removal sentence, and the extension-state warn-and-continue.

Unreproduced process-survival outcomes are Likely: `FSR-GROK-001`, `FSR-GROK-002`, and `FSR-GROK-003`. The missing signal, the disarmed guard, and the wrong quit function are in source. The orphan process was not observed.

`REL-GROK-001` is Confirmed for the copy into the tool message. It is not Confirmed for OOM or process death.

No race, lock storm, gateway timeout, or multi-worker outcome is marked Confirmed.

`ORCH-GROK-001` was downgraded from the lane’s High to Medium. The error sentence is inside the success body, and a turn cap exists. The protocol flag remains a Confirmed defect.

## Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| SEC-GROK-003 | Security | Workflow-GUI | Auto’s published contract and the missing prompt are an operator-belief failure as well as an allow decision |
| LLM-GROK-001 | Security | Negative-Space | A complete parse is an alternate path around the incomplete-syntax gate |
| MCP-GROK-001 | Workflow-GUI | Reliability | A destructive tool reports success with no exit status, so later recovery is aimed at the wrong state |
| ORCH-GROK-001 | Workflow-GUI | Cascade | A false success becomes the parent agent’s authority for the next step |
| MCP-GROK-002 | Workflow-GUI | Negative-Space | The failure text is present and the protocol flag still says success |
| REL-GROK-001 | Reliability | Input-Output-Path | The spill cap exists; the caller puts the refused body back on the model path |
| SEC-GROK-001 | Security | Input-Output-Path | Command text crosses into logs |
| SEC-GROK-002 | Security | Input-Output-Path | URL userinfo and query cross into logs and approval reasons |
| WFG-GROK-001 | Workflow-GUI | Negative-Space | Enter is an alternate path around a disabled control |
| FSR-GROK-001 | Failsafe | Concurrency | Crash recovery and the live supervisor do not use the same process-group signal |
| FSR-GROK-002 | Failsafe | Resource lifecycle | The group owner is dropped at the success transition |
| FSR-GROK-003 | Failsafe | Workflow-GUI | The quit control the operator uses is not the shutdown helper |
| ARC-GROK-004 | Architecture | State-Transition | Not-found and storage failures share a validation code, so the client cannot pick the taxonomy’s recovery |
| FSR-GROK-004 | Failsafe | Workflow-GUI | The operator-visible sentence does not match the delete result |
| REL-GROK-002 | Reliability | State-Transition | The turn advances to “tools updated” when the durable extension record did not |
| ARC-GROK-001 | Architecture | Workflow-GUI | The setup guide names providers the registry cannot list |
| ARC-GROK-002 | Architecture | Workflow-GUI | The config guide names a file the product does not write |
| ARC-GROK-003 | Architecture | Compliance-Posture | The index agents are told to read states the wrong current version |
| WFG-GROK-002 | Workflow-GUI | Reliability | A liveness route is described as database readiness |
| ARC-GROK-005 | Architecture | Workflow-GUI | A resume procedure is bound to one absolute checkout |

Patch prompts should repair the primary defect and add the secondary lens’s guardrail. The nominal agent column does not grant patch authority.

## Coverage Matrix

Required lenses:

| Lens | Disposition |
|---|---|
| audit-security | Applied. Four findings. October 12 highs re-checked and held. |
| audit-workflow-gui | Applied. Composer, health gloss, and success-flag findings. |
| audit-architecture-seam | Applied. Five findings. Schema v38 and workspace module table held. |
| audit-reliability | Applied, with the failsafe family. Six findings. Process outcomes capped Likely. |
| audit-dataflow-input-output | Partial. Sampled holds (office reader, legacy jsonl, spill caps). Most IOP codes Not Reviewed. The spill caller defect is `REL-GROK-001`, not a second IOP id. |
| audit-dataflow-state-transition | Partial. Handoff CAS and the historical v24 disposition held. Most STT codes Not Reviewed. No new STT finding. |
| audit-dataflow-integrity | Partial. Session-delete child order and ADR-0018 revision retention held on the cited paths. Most DAT codes Not Reviewed. No new DAT finding. |
| audit-dataflow-concurrency | Partial. Session delete is one immediate transaction; gitignore and registry updates take the process file lock. Most CON codes Not Reviewed. No new CON finding. |
| audit-dataflow-cascade | Skill loaded. CAS-001..015 Not Reviewed. No CAS finding. |
| audit-dataflow-temporal | Partial. Migration ladder v1–v38 and revision restore order held on the cited paths. Most TMP codes Not Reviewed. No new TMP finding. |
| audit-invariant-sync | Partial. Handoff and plan status checks match their enums on the cited paths. Schema version agrees. Most INV codes Not Reviewed. No new INV finding. |
| audit-negative-space | Dispositioned below. Playbook probes were not executed one by one. Mechanisms that match a NEG code are filed under the primary lens. |
| audit-compliance-posture | Light coverage. Giles YAML held as advisory. CMP-001..015 procedures were not walked. No new CMP finding. This report does not declare fleet compliance. |

Standalone triage:

| Skill | Disposition |
|---|---|
| audit-security-code | Applied to Rust, SQLite, and Electron/Node flaw classes. Python and Supabase paths are N/A. |
| audit-security-llm | Applied to folder scope, egress, website-login hold, and delegation hold. |
| audit-security-nodejs | Bounded to `ui/desktop` and Node scripts. Renderer sandbox and grant path held on the cited lines. |
| audit-security-owasp | Used as the flaw labels on the security findings. Not a separate finding set. |
| audit-security-repo-posture / audit-security-repo-triage | Applied to the workflow gate. `pr-smoke-test.yml` trigger re-read. GitHub settings Not Reviewed. |
| audit-security-vuln-harness | Defensive only. Mechanisms and impact are named. No exploit procedure, payload, or PoC. |
| audit-architecture-drift | Applied. Version and provider-guide drift filed. |
| audit-architecture-nodejs | Bounded to desktop and scripts. Directory-rename lock held. Node module-system survey Not Reviewed beyond that. |
| audit-contract-internalapi | Sampled. ACP method-string sets matched (architecture lane). Full handler taxonomy beyond workspace mutations Not Reviewed. |
| audit-deadcode-cleanup | `ToolPermissionStore` census filed as `ARC-GROK-002`. Broader dead-code sweep Not Reviewed. |
| audit-repo-path-consistency | Bounded. ACP method strings held. |
| audit-repo-state-reconciliation | Bounded. Schema v38 held. |
| audit-failsafe-readiness, dependency-criticality, recovery-idempotency, operator-signal, resource-lifecycle, memory-lifecycle | Applied inside the reliability lane. Findings use the FSR and REL prefixes. Memory measurement Not Reviewed. |
| audit-mcp-server | Applied to computer control and `automation_script`. Most MCP inventory codes Not Reviewed (operations lane list). |
| audit-agent-orchestration-code | Applied to `send_message` drain, depth, and turn cap. |
| audit-pipeline-externalapi | Status mapping sampled. Timeouts, retries, idempotency, and quota across every provider Not Reviewed. |
| audit-llm-config-drift | In-repo only. Live catalog Not Reviewed. |
| audit-optimization-opportunities | No measured benefit. No finding promoted. |
| audit-design-webapp | Partial. Static only. Gates 2–6 Not Reviewed and not scored. |
| audit-pipeline-graph | Two paths only (data lane). Not a graph clearance. |
| audit-playtest-app | Applicable because `docs/test_scenarios` exists. Not executed. |
| audit-testsuite-economics, audit-performance-profile | Applicable and measure-first. Not measured. Not Review for timing claims. |
| audit-contract-crossrepo | N/A. Single repo. In-repo seams stayed on internal API and architecture. |
| design-handheld | N/A. No handheld or touch game. |
| audit-multiagent-consensus | Not requested. Not run. |
| Catalog exclusions | `audit-equation-sourcebase`, `audit-flutter-ios`, `audit-go-repo-hardening`, `audit-graphdb-design`, `audit-security-supabase`. Not opened. |

### Negative-space disposition

The negative-space skill was loaded. Its detection playbook was not applied code by code. Unprobed codes are Not Reviewed. Where a filed finding is already the mechanism, it is cross-referenced and not given a second ID.

| Code | Disposition |
|---|---|
| NEG-001 Impossible state | Not Reviewed |
| NEG-002 Hidden actor | Not Reviewed as a survey. Process-global orchestrator sessions are the accepted single-operator residual. |
| NEG-003 Unmodeled input | Not Reviewed |
| NEG-004 Cross-boundary composition | Not Reviewed |
| NEG-005 Assumption collapse | Adjacent to `SEC-GROK-003`: the guide says stricter egress still applies, and the inbound branch Allows. Filed as security. |
| NEG-006 Rare timing window | Not reproduced. Process-survival findings stay Likely. |
| NEG-007 Catastrophic low probability | Not Reviewed. No speculative catastrophe was promoted. |
| NEG-008 Negative test missing | The regression table names tests that do not exist yet. Not a separate finding. |
| NEG-009 Alternate path | `WFG-GROK-001` (Enter versus disabled Send) and `LLM-GROK-001` (complete parse versus the incomplete-syntax gate). |
| NEG-010 Operator misuse | Adjacent to the documentation findings. Not separately filed. |
| NEG-011 Model output trusted | `MCP-GROK-001`, `MCP-GROK-002`, and `ORCH-GROK-001`. |
| NEG-012 Future integration | Speculative multi-user use is deferred. Not filed. |
| NEG-013 Local-first assumption | Current product is local and single-operator. Not filed as a present defect. |
| NEG-014 Compliance language | Giles YAML held as advisory. `ARC-GROK-003` is the stale “current source” line. |
| NEG-015 Harmful recovery | `FSR-GROK-001` sweep can forget same-group children. `REL-GROK-001` recovery from a refused spill inlines the body. |

### Dataflow Not Reviewed

The data lane’s disposition is the record. Held samples include office in-memory reads, legacy jsonl fail-closed, spill caps, handoff compare-and-swap, one-transaction session delete, ADR-0018 revision retention, migration ladder through v38, and the handoff and plan status checks. Every CAS code is Not Reviewed. Remaining IOP, STT, DAT, CON, TMP, and INV codes that the lane marked Not Reviewed stay Not Reviewed. Empty finding prefixes for those domains mean no defect met the cite-and-contradict bar on the paths that were read.

## Repair Verification (October 12)

| Prior ID | This pass | Result |
|---|---|---|
| SEC-GSL-001 website-login destination | Earlier pass this engagement | Holds. Not re-filed |
| SEC-GSL-201 renderer self-grant | Earlier pass this engagement | Native confirmation and sensitive-dir skip hold. Not re-filed |
| SECN-GSL-201 PR secret exposure | Trigger and job gate re-read | Main-only gate holds. Rotation unverified |
| SEC-GSL-003 unauthenticated health | Guide re-read; handlers earlier pass | Intentional and documented. Gloss split is `WFG-GROK-002` |
| IOP-GSL-005 Peekaboo auto-install | Operations lane | `ensure_peekaboo` does not install. Not re-filed |
| REL-GSL-001 OAuth removal errors | Reliability lane | Explicit cleanup propagates. `FSR-GROK-004` is the automatic GitHub path |
| DAT / ADR-0018 revision survival | Data lane | Matches the ADR. Not re-filed |
| STT-GSL-003 v24 flag clear | Data lane | Historical only. Not re-filed |
| Retry-After “uncapped” | Reliability lane | Capped. Not re-filed |
| SEC-GSL-002 log preview | Re-read `build_explanation` | Regressed into the inspection-result info log. Filed as `SEC-GROK-001` |


## Repair addendum — 13 October 2026

The operator authorized repairs after this read-only audit. All 20 supplied source/document defects now have local repairs; the current per-ID dispositions and fix pointers are in [2026-10-13-grok-findings-repair.md](docs/logs/session/2026-10-13-grok-findings-repair.md). The original findings and confidence labels above describe the audited revision and are retained.

Validation: Cargo formatting and compile checks including tests passed; 46 focused Desktop tests and three terminal tests passed; Desktop/terminal typechecks, Desktop changed-file lint/format, i18n and document scans passed. Rust tests were compiled but not executed under the repository's explicit-request rule. Native Windows/Linux automation, installed GUI, interactive quit/repeated signals and injected persistence/account failures remain unverified. Overall repair status is `completed_with_partial_verification`; this does not establish fleet compliance, full platform parity or release readiness. CI secret rotation remains an owner action.
