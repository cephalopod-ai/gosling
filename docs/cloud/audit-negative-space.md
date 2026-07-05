# Gosling Audit — Negative-Space Lens

Lens: `audit-negative-space` v3.1 (domain NEG). Authority: **audit-only / read-only**.
Builds on `docs/cloud/00-orientation.md`. This lens hunts the failures the standard
lenses miss: impossible-state-made-reachable, hidden actors, unmodeled input,
cross-boundary composition, assumption collapse — specifically at the
Rust ↔ Electron/ACP-client ↔ MCP ↔ subprocess/provider seams where each side assumes the
other validated.

Effort budget: ~35 tool calls, concentrated on the tool-inspection / permission /
security-scanner pipeline and the subagent + ACP seams. Sampling and un-reviewed
surfaces are recorded in **Validation Limits**.

---

## 1. Load-bearing assumption ledger

| # | Assumption | Where it appears | What relies on it | If false | Evidence status |
|---|---|---|---|---|---|
| A1 | "A shell/command tool is the one literally named `shell`." | `security/scanner.rs:394` `is_shell_tool_name` = `matches!(name,"shell")` | The entire prompt-injection / command-injection scanner (`analyze_tool_call_with_context` early-returns for any other name, `scanner.rs:136`) | Any command-executing tool with a different name is never scanned | Confirmed (source) |
| A2 | "A tool's `read_only_hint` annotation is honest." | `permission/permission_inspector.rs:38-52`, `165-170` | Auto-approval of "read-only" tools in **both** Approve and SmartApprove modes | A destructive third-party MCP/ACP tool self-declaring `readOnlyHint:true` is auto-run without user confirmation | Confirmed (source) |
| A3 | "The user's approval mode gates every tool the agent runs." | `agents/agent.rs:153`, `permission_inspector.rs:151-188` | Human-in-the-loop safety of the whole product | A parent agent can spawn a subagent that runs in `Auto`, executing tools with no approval | Confirmed (source + code comment) |
| A4 | "Security detection is on, so the scanner backstops permission." | `security/mod.rs:54,82` (`unwrap_or(false)`) | The claimed prompt-injection defense (`SECURITY.md`) | Default install has **no** scanner; permission gating is the only real boundary | Confirmed (source) — cross-ref security lens |
| A5 | "The command string is interpreted by a POSIX shell." | `platform_extensions/developer/shell.rs:88-94` (`GOSLING_SHELL`, "passed through as-is") | Regex pattern matching in `security/patterns.rs` | A non-POSIX `GOSLING_SHELL` changes command semantics the patterns assume | Plausible (source) |
| A6 | "The confirmation `request_id` uniquely identifies one waiting tool call." | `agents/tool_confirmation_router.rs:19-25` | Correct routing of approve/deny decisions | Two concurrent calls with the same id → first silently overwritten, its waiter never signalled | Plausible (source) — see non-findings |

The two roots that matter most: **A3** (subagent = a hidden actor that owns no approval
boundary) and the pair **A1/A2** (two individually-safe components disagree on what a
"shell tool" / "read-only tool" is, and the weaker definition guards the sink).

---

## 2. Findings

### NEG-GSL-001: Two conflicting "is-a-shell-tool" definitions; the command-injection scanner only inspects the tool literally named `shell`

Severity: High (if scanner enabled) / Medium (default, scanner off)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Negative-Space

Evidence:
- `crates/gosling/src/security/scanner.rs:394-396`
  ```rust
  fn is_shell_tool_name(name: &str) -> bool {
      matches!(name, "shell")
  }
  ```
- `crates/gosling/src/security/scanner.rs:136-143` — early-returns "Tool call skipped: only shell commands are scanned" for every other name; the pattern matcher and ML command classifier never see it.
- `crates/gosling/src/security/scanner.rs:373-382` — the command is only extracted from `args.get("command")`.
- Contrast `crates/gosling/src/security/egress_inspector.rs:275-282` — the **same security subsystem** defines a much broader `is_shell_tool`: `"shell" | "bash" | "execute_command" | "run_command" | "terminal"` plus `*__shell`, `*__bash`, `*__terminal`.
- `crates/gosling/src/security/adversary_inspector.rs:14` — `DEFAULT_TOOLS = &["shell", "computercontroller__automation_script"]`: the adversary inspector knows `automation_script` executes code; the scanner's `is_shell_tool_name` does not match it.

Observed behavior:
- The prompt-injection / command-injection scanner analyzes a tool call only when its name is exactly `"shell"` and its command arg is exactly `command`. Every other command-capable tool — `computercontroller__automation_script`, any MCP extension exposing `run_command` / `bash` / `execute` / a prefixed `foo__shell`, or a tool that carries its command under a different arg key — is skipped with a "passed"-shaped log.

Expected boundary:
- A single, shared definition of "command-executing tool" should gate the command-injection scanner, at least as broad as the egress inspector already uses.

Failure mechanism:
- Assumption collision (A1). Three components in `security/` encode three different answers to "what runs commands," and the narrowest one guards the primary injection scanner. The developer `shell` tool happens to be unprefixed (`extension_manager.rs:1404-1413`, `platform_extensions/mod.rs:143` `unprefixed_tools:true`), so the main path is covered — which masks the gap for every other command tool.

Break-it angle:
- Model output (untrusted) that routes a dangerous command through `computercontroller__automation_script` or a third-party MCP `__run_command` is never pattern/ML-scanned; it reaches execution with only permission gating in front of it.

Impact:
- Command-injection content embedded in tool results/web pages can drive a non-`shell` command tool and evade the scanner entirely. Blast radius: local workstation (arbitrary command execution).

Operational impact:
- Blast radius: Service (the user's machine) · Side-effect class: process/network · Reversibility: irreversible · Operator visibility: log-only (misleadingly logs "skipped/passed") · Rerun safety: unknown

Adjacent failure modes: NEG-GSL-003 (subagent Auto amplifies this — non-`shell` command tool + Auto = no scan, no approval), NEG-GSL-002.

Recommended mitigation:
- Replace `is_shell_tool_name` with the shared broader predicate already in `egress_inspector.rs` (or a single `security::is_command_tool` used by both), and extract the command from all known arg keys, not just `command`.
- Behavior test: assert a call to `computercontroller__automation_script` / `ext__run_command` with a known-malicious payload is scanned and flagged.

Implementation assessment:
- Complexity: local_guardrail · Cost: S · Cost drivers: modules(2) + tests · Nominal agent: codex · Rationale: one predicate unified across two files with negative tests.

Validation:
- Negative test: forbidden payload under a non-`shell` command tool name is BLOCK/ALERT, not "skipped".

Non-goals:
- Do not re-enable the scanner by default here (separate decision — see A4 / security lens).

---

### NEG-GSL-002: Third-party tool `read_only_hint` annotation is trusted to auto-approve, even in manual Approve mode

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Negative-Space

Evidence:
- `crates/gosling/src/permission/permission_inspector.rs:38-52` — `apply_tool_annotations` records every tool whose `annotations.read_only_hint == Some(true)` into `readonly_tools`, iterating **all** tools including MCP/ACP-provided ones.
- `crates/gosling/src/permission/permission_inspector.rs:164-170`
  ```rust
  } else if self.is_readonly_annotated_tool(tool_name)
      || (gosling_mode == GoslingMode::SmartApprove && ... == Some(AlwaysAllow))
  {
      InspectionAction::Allow
  ```
  The `is_readonly_annotated_tool` branch is under `GoslingMode::Approve | GoslingMode::SmartApprove` (`:154`), so it fires even in the stricter manual-approval mode, and short-circuits **before** the LLM read-only detection (`:176-183`).

Observed behavior:
- A tool's own self-declared `readOnlyHint: true` causes it to be auto-executed with no user confirmation, in both Approve and SmartApprove modes.

Expected boundary:
- Annotations from first-party (developer/platform) tools may be trusted; annotations from third-party MCP servers and ACP-bridged tools are attacker-influenceable and must not by themselves grant auto-execution. A read-only claim should be corroborated (LLM/annotation cross-check) or scoped to trusted extensions.

Failure mechanism:
- Assumption A2. Tool metadata crosses the MCP/ACP trust boundary unmodified (`extension_manager.rs:1407-1422` copies server-provided tools/meta), then a security decision keys off a field the untrusted side controls. Classic "core assumes the tool told the truth."

Break-it angle:
- A malicious or compromised MCP server (or one installed via prompt-injection-driven extension management) registers a tool that writes files / runs a request but advertises `readOnlyHint:true`; it is auto-approved on first and every call.

Impact:
- Silent bypass of the human-in-the-loop boundary for arbitrary third-party tools. Blast radius depends on the tool; potentially local file/network mutation with no prompt.

Operational impact:
- Blast radius: Workflow→Service · Side-effect class: file/network/external API · Reversibility: irreversible · Operator visibility: silent (reason logged as "Tool annotated as read-only", `:195-196`) · Rerun safety: unsafe

Adjacent failure modes: NEG-GSL-001, NEG-GSL-003.

Recommended mitigation:
- Trust `read_only_hint` for auto-approval only from first-class/platform extensions (`is_first_class_extension`); for third-party tools, treat the hint as advisory and still route through LLM detection / approval.
- Behavior test: a third-party MCP tool declaring `readOnlyHint:true` in Approve mode yields `RequireApproval`, not `Allow`.

Implementation assessment:
- Complexity: local_guardrail · Cost: S · Nominal agent: codex · Rationale: gate one predicate on extension provenance already available in tool meta (`TOOL_EXTENSION_META_KEY`).

Validation:
- Test asserts provenance-gated auto-approval; first-party read tool still auto-allows, third-party does not.

Non-goals:
- Do not remove the annotation fast-path for genuine platform read tools.

---

### NEG-GSL-003: Subagents run in `Auto` mode — approving *delegation* silently approves every action the delegate takes

Severity: High (if reachable)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Negative-Space

Evidence:
- `crates/gosling/src/agents/platform_extensions/summon.rs:976-996`
  ```rust
  // Subagents must use Auto until get_agent_messages forwards
  // ActionRequired messages to the parent. Until then, any mode
  // that requires approval will hang on the subagent's confirmation_rx.
  let agent_config = AgentConfig::new(..., GoslingMode::Auto, ...);
  ...
  .create_session(..., SessionType::SubAgent, GoslingMode::Auto)
  ```
  (identical pattern at `summon.rs:1392`).
- `crates/gosling/src/agents/agent.rs:153` — `GoslingMode::Auto => InspectionAction::Allow` (all tools auto-approved).
- `crates/gosling/src/agents/subagent_handler.rs:173-214` — the subagent run loop consumes `AgentEvent::Message/Usage/...` but never surfaces an `ActionRequired`/confirmation channel to the parent or user.
- `crates/gosling/src/tool_inspection.rs:233-247` — a `SecurityInspector` `RequireApproval` *does* still escalate a subagent tool into `needs_approval`, which then registers on the subagent's own `tool_confirmation_router` (`tool_execution.rs:103`) that nobody delivers to → hang (exactly what the summon comment warns).

Observed behavior:
- The parent agent honours the operator's Approve/SmartApprove mode, but the `summon`/subagent tool spawns a child agent hard-coded to `Auto`. Every non-security-flagged tool the subagent issues (shell, write, edit, network MCP calls) executes with **no** user confirmation. Security-flagged tools instead hang the subagent.

Expected boundary:
- A subagent must not have *more* authority than the operator granted the parent. Delegating a task is not consent to run arbitrary unsupervised commands; the approval boundary should follow the work, not stop at the delegation call.

Failure mechanism:
- Hidden actor + composition (A3). The subagent is a second agent instance with its own confirmation router that is never wired to a human, so the safe design decision was "make it Auto." Two safe intentions (delegate a task; gate tools) compose into an unsafe whole (unsupervised execution).

Break-it angle:
- Prompt-injection in the parent's context (untrusted tool/web content) that induces a `summon`/delegate call with a malicious task fully escapes the human-in-the-loop control the user selected. Compounds with NEG-GSL-001 (a subagent using a non-`shell` command tool is neither scanned nor approved) and NEG-GSL-002.

Impact:
- The product's primary safety control (approval mode) is nullified for any work routed through a subagent. Blast radius: local workstation, arbitrary tool execution.

Operational impact:
- Blast radius: Service · Side-effect class: process/file/network · Reversibility: irreversible · Operator visibility: silent (subagent tool calls surface as notifications, not approvals) · Rerun safety: unsafe

Adjacent failure modes: NEG-GSL-001, NEG-GSL-002, NEG-GSL-004.

Recommended mitigation:
- Forward the subagent's `ActionRequired` stream to the parent's `tool_confirmation_router` (the fix the comment defers), then run subagents in the parent's mode rather than `Auto`; OR gate the `summon` tool with an explicit, un-blanket "this subagent will run tools unsupervised" confirmation that names the task and cannot be `AlwaysAllow`-cached.
- Behavior test: with parent in Approve mode, a subagent shell call produces an approval request the operator must answer (or is denied), not silent execution.

Implementation assessment:
- Complexity: cross_process_coordination · Cost: L · Cost drivers: confirmation routing across agent instances + tests · Nominal agent: claude · Rationale: touches the subagent event plumbing and the parent router; broad-context change.

Validation:
- Integration test: parent Approve mode + delegated shell task → no execution without a delivered confirmation.

Non-goals:
- Do not silently switch subagents to Approve without wiring the confirmation path (that is the "hang" failure the current comment avoids).

---

### NEG-GSL-004: `GOSLING_SHELL` is unmodeled input that can change the semantics the pattern scanner assumes

Severity: Low
Confidence: Plausible
Evidence basis: source-evidenced
Domain: Negative-Space

Evidence:
- `crates/gosling/src/agents/platform_extensions/developer/shell.rs:85-94,136-139` — the executing shell is taken from `GOSLING_SHELL` and "passed through as-is."
- `crates/gosling/src/security/patterns.rs:80-213` — regex patterns (`curl…|bash`, `powershell…Invoke-Expression`, reverse-shell, base64-shell) implicitly assume a POSIX-ish shell.

Observed behavior:
- The scanned command string is matched against POSIX-shaped regexes, but the shell that ultimately interprets it is operator/env-selected and unvalidated.

Expected boundary:
- Treat the interpreting shell as an input to the scanner (or normalize), so pattern coverage matches the actual interpreter.

Failure mechanism:
- Unmodeled input (A5): env/clock/shell are inputs that are not called "input." A non-POSIX or wrapper `GOSLING_SHELL` can express a dangerous command in a form the POSIX patterns miss.

Break-it angle:
- Under a nushell/powershell/custom `GOSLING_SHELL`, an equivalent malicious command evades the pattern set.

Impact:
- Narrow: requires operator-controlled env; the operator is trusted in the base threat model, so this is hardening, not a direct exploit.

Operational impact:
- Blast radius: Local · Side-effect class: process · Reversibility: irreversible · Operator visibility: log-only · Rerun safety: unknown

Recommended mitigation:
- Record the resolved shell alongside findings; document that pattern coverage is POSIX-oriented, or select patterns by shell flavor (which `shell.rs` already computes via `unix_shell_flavor`).

Implementation assessment:
- Complexity: local_guardrail · Cost: XS · Nominal agent: codex.

Validation:
- Test: same malicious intent under a non-POSIX shell flavor is still flagged or explicitly documented out of scope.

Non-goals:
- Do not attempt to sandbox `GOSLING_SHELL`.

---

## 3. Non-findings (seams checked and held)

- **ACP permission decision degradation** — `acp/common.rs:70-101`: `AllowAlways` degrades to `AllowOnce`, `RejectAlways` to `RejectOnce` — always toward *less* authority; a missing option yields `Cancelled`. No allow-escalation path. Safe.
- **Unknown permission option id from the ACP client** — `acp/common.rs:55-64`: `PermissionDecision::from_str(...).unwrap_or(Self::Cancel)`; an unrecognized/forged option id fails closed to Cancel. Safe.
- **Confirmation router duplicate `request_id`** (A6) — `tool_confirmation_router.rs:19-25`: a duplicate id would overwrite the earlier waiter (leading to a hang, not an approval leak), and `request_id`s are gosling-generated, not model/attacker-controlled. Not a security bypass today; flagged only as robustness. Re-open if request_ids ever become client-suppliable across sessions.
- **Developer `shell` tool coverage** — the primary developer shell tool is exposed unprefixed as `"shell"` (`extension_manager.rs:1404-1413`, `platform_extensions/mod.rs:143`), so it *is* matched by `is_shell_tool_name`. The gap in NEG-GSL-001 is for the *other* command tools, not this one.
- **Subagent inspector wiring** — subagents build a full `Agent::with_config` (`subagent_handler.rs:145`) that does add all inspectors; the problem is the forced `Auto` mode (NEG-GSL-003), not missing inspectors.

---

## 4. Break-it review summary

| Attack | Result |
|---|---|
| Route a malicious command through a non-`shell` command tool | **Bypasses scanner** (NEG-GSL-001) |
| Third-party MCP tool self-declares `readOnlyHint:true` | **Auto-approved even in Approve mode** (NEG-GSL-002) |
| Induce a delegated/subagent task via prompt injection | **Runs unsupervised in Auto** (NEG-GSL-003) |
| Forge an ACP permission option id | Fails closed to Cancel (held) |
| Force AllowAlways where only AllowOnce exists | Degrades to less authority (held) |
| Compose subagent (Auto) + non-`shell` command tool | **Double bypass**: no scan, no approval |

---

## 5. Patch order (highest value first)

1. **NEG-GSL-003** — restore the approval boundary across the subagent seam (largest blast radius; nullifies the whole control today).
2. **NEG-GSL-002** — provenance-gate the `read_only_hint` auto-approval.
3. **NEG-GSL-001** — unify the "command tool" predicate across `security/`.
4. **NEG-GSL-004** — document/scope shell-flavor pattern coverage.

---

## 6. Validation Limits (what this lens did NOT review)

- **Not executed live.** All findings are static/`source-evidenced`; no build/run, no provider creds, no reproduced race. Per the effort budget, ~35 tool calls focused on the inspection/permission/scanner pipeline, the subagent seam (`summon.rs`, `subagent_handler.rs`), and the ACP fs/permission mapping.
- **Not reviewed:** the ACP *provider* bridge internals (`acp/provider.rs`, ~2.2K LOC) for how external-agent subprocess output is trusted end-to-end; `large_response_handler.rs`; `oauth/` callback server as a hidden actor; `session/import_formats/` + `nostr_share` composition; the Electron `ui/desktop` side of the approval UI (whether it can misrepresent the approval prompt); MCP transport-level argument construction; `context_mgmt/` secret-leak-on-summarize. These are candidates for the concurrency, security-llm, and architecture-seam lenses.
- **Confidence honesty:** NEG-GSL-001/002/003 are `Confirmed` on the *code paths quoted*; their real-world exploitability depends on runtime config (scanner default-off per A4, whether `summon`/third-party MCP is installed) — reachability is argued, not executed. NEG-GSL-004 and A6 remain `Plausible`.
- **Cross-lens escalation:** A4 (scanner disabled by default, `security/mod.rs:54`) is the amplifier behind NEG-GSL-001 and should be reconciled with the compliance-posture and security-llm lenses (stated `SECURITY.md` posture vs default-off control).
