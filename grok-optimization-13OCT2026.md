# Optimization opportunity register — 13 Oct 2026

Read-only audit of avoidable work in gosling, with extra weight on processes that start when a cheaper or already-completed result would do. Method: `audit-optimization-opportunities` (OPT-001..020). No application code was changed. No timings were collected.

## Executive verdict

The register contains **counted process waste** and **structural process waste**. It does not contain a measured speedup.

Two mechanisms are ready to scope:

1. Desktop PATH recovery starts an interactive login shell, and two other call sites start their own login shells instead of reading the first result.
2. `gosling review` starts a `git diff --name-only` process for a range it also asks `git diff` to print, and `--summary-only` still builds the full patch before it asks git for `--stat`.

A third, smaller ready item is Linux `computer_control` startup, which runs one `which` process per dependency even though this repo already links the `which` crate.

No dollar, latency, or percentage claim below is a measurement from this run. Where the source itself says "~hundreds-of-ms", that phrase is quoted as an author comment, not as a result of this audit.

## Preflight

```text
Target:      /Users/eric/Work/vscode/forked/gosling (branch main, HEAD 4eb529775)
Dirty in:    docs/logs/session/2026-10-13-resource-handoff-audit.md (pre-existing; left untouched)
Involvement: inferred L2 — the request named the report file and the question; no extra questions were required
Ceiling:     read_only for code — lower of the skill declaration and this task; the named report is the requested deliverable
Assumed:     scope = whole repository; objective = avoidable work, especially processes started without a new result
Validation:  source trace plus documentation checks after the report is written; no cargo, pnpm, or runtime drill
Execution:   parallel source searches, then sequential reads of the process-spawn call chains
Restart:     this file is the checkpoint; unreviewed surfaces are listed under Validation limits
```

Phase 0 authority: report generation. Patching product code was out of scope.

## Optimization objective

No single metric was supplied. "Optimize" was not translated into request latency.

| Field | Content |
|---|---|
| Beneficiary | Desktop and CLI operator; review command user; service owner paying for repeated child processes |
| Metric | Process starts per session, per review, and per Linux computer-control init. Secondary proxies: repeated plugin scans and bytes of diff materialized on `--summary-only` |
| Workload | One desktop `gosling serve` process hosting session agents; one `gosling review` invocation; first Linux `LinuxAutomation::new` in a process |
| Baseline | Counted from source. No same-workload timing, telemetry, or billing sample |
| Constraint | PATH must still see user tools under a desktop-launched minimal PATH. Review output for tracked and untracked files must stay complete. Dependency checks must still fail closed when a Linux tool is missing. Security checks, approval prompts, and process-group cleanup stay in place |
| Success test | A desktop session that loads Developer, runs one command hook, and runs one computer-control script sources the login profile once. `gosling review` and `gosling review --summary-only` issue fewer `git` processes and still list the same files. Linux init resolves the same dependency set without a `which` process per name |

## Scope and effort budget

| Surface | Budget used |
|---|---|
| Child-process spawn sites in `crates/gosling`, `crates/gosling-cli`, `crates/gosling-mcp`, and `ui/desktop/src` | Inventoried with search, then deep-read for login PATH, extension stdio, review git, desktop serve, and Ollama/git IPC |
| Agent and subagent construction | Read `Agent::with_config`, session extension resume, and delegate extension filtering |
| Polling | Read run-status interval, text-UI animation interval, and backend-registry cleanup |
| Build/CI, provider HTTP retries, UI render cost, memory retention | Sampled or not reviewed. See the matrix |

Stop condition: the process-spawn inventory's high-frequency paths were read. Remaining provider, CI, and UI-render surfaces are marked Not Reviewed rather than cleared.

## Recurring-work map

| Work unit | Actor | What starts | Consumer | Cadence from source |
|---|---|---|---|---|
| Desktop backend | Electron `startGoslingServe` | One `gosling serve` process | ACP session host | Once per app launch, unless an external backend is configured (`ui/desktop/src/main.ts`) |
| Session agent | `AgentManager::create_agent_locked` | In-process agent, then each enabled extension | One cached session | Once per session until LRU eviction. Default cache size is 5 (`DEFAULT_MAX_SESSION`) |
| Developer extension | `DeveloperClient::new` | Interactive login shell when `use_login_shell_path` is true | Later `shell` tool PATH | At extension load, before any shell command |
| Command hook | `HookManager::emit` | `sh -c` per hook action; login shell on first hook when the flag is set | Plugin policy | Per matching hook event |
| Computer control | `merged_path()` | Another login shell, cached only inside `gosling-mcp` | Script and Peekaboo child PATH | First script or Peekaboo call in the process |
| Stdio / inline Python extension | `ExtensionManager` | The configured command, or `uvx`, or `docker exec` | Tool catalog for that agent | At extension load |
| Subagent | `run_subagent_task` | New agent; extensions only when the delegate list names them | One delegated task | Per delegate |
| `gosling review` | `handle_review` | Several `git` processes, then one `gosling run --no-profile` per file and per check | Review findings | Per invocation |
| Ollama probe | `check-ollama` IPC | `ps aux` piped to `grep` | No in-repo renderer caller found | Only if something invokes the IPC |
| Run-status heartbeat | `useRunStatus` | ACP status request, not an OS process | Quiet/stale indicator | Every 15 minutes, and only while a prompt or background task is active |

## Evidence-source and trust-limit table

| Source | State | Trust limit |
|---|---|---|
| Source lines cited below | Counted or structural | Shows the process will be constructed. It does not show wall-clock cost or how often operators hit the conditional paths |
| In-source comment "~hundreds-of-ms" on login-shell overlap | Author estimate | Not a measurement from this audit |
| Creation-lock comment in `execution/manager.rs` | Structural | Explains a past double-initialize. This audit did not replay that race |
| Absence of a renderer caller for `checkForOllama` | Counted within `ui/desktop/src` | An external preload consumer could still call it. None is present in this tree |
| No `cargo test`, no `ps` sample, no review timing | Unavailable | Benefit direction is supported. Benefit magnitude is not |

Oracle integrity: this report does not treat a green test suite as proof of production process counts. No end-to-end workflow was closed on test-fixture evidence.

## OPT-001..020 coverage matrix

| Code | State | Basis |
|---|---|---|
| OPT-001 | Finding | `REL-OPT-002`: `git diff --name-only` repeats work the following `git diff` already returns |
| OPT-002 | Candidate | Linux `which` is one process per name (`REL-OPT-003`). No bulk git or bulk model contract was traced beyond review |
| OPT-003 | Finding | `REL-OPT-002`: `--summary-only` still materializes the full diff. Agent LRU rebuilds a whole extension set after eviction; that restart is Measure, not a finding |
| OPT-004 | Held | Extension startup already uses a `JoinSet`. Review checks and per-file passes already run concurrently. No sequential stage was shown to be avoidable without changing isolation |
| OPT-005 | Not Reviewed | Representation conversions (provider JSON, ACP frames) were not traced for repeated parse/clone cost |
| OPT-006 | Finding | Same as `REL-OPT-002` for `--summary-only`: the full patch is built and then discarded |
| OPT-007 | Finding | `REL-OPT-001` login-shell setup; `REL-OPT-003` per-dependency `which` |
| OPT-008 | Candidate | Login PATH and tool catalogs are cached, but the login cache is not shared across the three spawn sites. Cross-agent tool-schema reuse was not proven safe |
| OPT-009 | Held | Run-status polling is 15 minutes and activity-gated. Text-UI `setInterval` ticks only while the banner or spinner is visible |
| OPT-010 | Not Reviewed | Log, snapshot, and bundle retention were not traced to a live consumer gap in this pass |
| OPT-011 | Not Reviewed | Provider retry and model-list refresh were not reconciled to billed call counts |
| OPT-012 | Not Reviewed | No utilization or queue sample. Static worker caps were not treated as over-provisioning |
| OPT-013 | Not Reviewed | CI job DAG and cache keys were not mapped |
| OPT-014 | Not Reviewed | Selective test gates were not compared with the full suite |
| OPT-015 | Held | No repeated manual operator procedure was observed in this pass. Review `--summary-only` is already a command, not a manual copy step |
| OPT-016 | Candidate | Three login-PATH implementations derive the same string. Deep split-brain review belongs to `audit-invariant-sync` only if the derivations start to disagree; today they duplicate work |
| OPT-017 | Finding | `REL-OPT-001`: Developer shell, hooks, and `gosling-mcp` each implement login-PATH capture |
| OPT-018 | Not Reviewed | Abstraction count was not used as a cost signal |
| OPT-019 | Not Reviewed | Dependency install and bundle cost were not measured. The `which` crate is already present and is the lever in `REL-OPT-003` |
| OPT-020 | Held | `check-ollama` has an IPC handler and no in-repo caller. Removal analysis belongs to `audit-deadcode-cleanup`; it is not a live extra process |

## Prioritized opportunity register

### 1. REL-OPT-001 — one login shell is resolved three times

| Field | Value |
|---|---|
| Evidence state | Structural for cost; Counted for spawn sites |
| Disposition | Ready |
| Current work | Desktop agents set `use_login_shell_path` by default. Loading Developer starts a login shell immediately. The first command hook and the first computer-control script each start another one. A delegate that includes Developer starts yet another, because the cache lives on that `ShellTool` |
| Benefit proxy | One avoided login-shell process per extra consumer in the same OS process. Upper bound is the 5-second capture deadline in each implementation. The source comment calls the first probe "~hundreds-of-ms"; this audit did not time it |
| Lever | One process-wide cache owned by the existing `shell::resolve_login_shell_path`, used by hooks and by `gosling-mcp`. Keep the eager first probe if first-command latency still matters |
| Invariants | Desktop-launched processes must still see the user's login PATH. A failed probe must still fall back to the inherited PATH. Subagents must not observe another user's environment |
| Regression risk | Sharing a cache across threads is safe only if publication is a `OnceLock` or equivalent. Interactive login shells can hang; the existing deadline and kill path have to stay on the single implementation |
| Size / reversibility | S, reversible |
| Validation | A test or trace that loads Developer, runs one command hook, and runs one computer-control script, and observes one login-shell spawn |
| Owner route | Implement from this register. Time the remaining first probe with `audit-performance-profile` only if the eager spawn itself is in question |

### 2. REL-OPT-002 — review asks git for the same diff more than once

| Field | Value |
|---|---|
| Evidence state | Counted |
| Disposition | Ready |
| Current work | Every review runs `git rev-parse`, `git diff --name-only`, `git diff`, and, when no `--range` is set, `git ls-files --others`. `--summary-only` then runs `git diff --stat` and returns, after the full patch was already built |
| Benefit proxy | One avoided `git` process on a normal review (the name-only diff). On `--summary-only`, the full patch process and the name-only process are both unused except for an emptiness check the stat and untracked listing can answer. No elapsed-time claim |
| Lever | Parse touched paths from the full diff. On `--summary-only`, skip the full patch and the name-only diff |
| Invariants | Untracked files still enter the review when `--range` is absent. Non-ASCII paths stay unquoted (`core.quotePath=off`). Empty reviews still print "no changes" |
| Regression risk | A hand-rolled parser of `diff --git` lines can miss copies and renames that `git diff --name-only` reports. Prefer one git invocation whose output includes names, or test rename and copy fixtures |
| Size / reversibility | S, reversible |
| Validation | Fixture repo with a tracked edit, a rename, and an untracked file. Compare file lists and summary text before and after, and count `git` spawns |
| Owner route | Local CLI change. No specialist audit required |

### 3. REL-OPT-003 — Linux dependency checks spawn `which` per name

| Field | Value |
|---|---|
| Evidence state | Counted |
| Disposition | Ready |
| Current work | First `LinuxAutomation::new` with a display runs `which` for `bash` and `python3`, then four X11 names or three Wayland names. `Once` limits this to once per process. It stops at the first missing tool |
| Benefit proxy | Up to six short-lived processes per Linux process that constructs the automation object. Not on the macOS desktop path |
| Lever | `which::which`, already used by the Developer shell, or direct `Path` checks |
| Invariants | A missing tool still fails the same initialization. PATH search semantics stay equivalent to `which` |
| Regression risk | Low. Do not skip the check |
| Size / reversibility | XS, reversible |
| Validation | Unit test with a temp PATH that contains only some of the names |
| Owner route | Local `gosling-mcp` change |

### 4. Session-cache eviction restarts extension processes

| Field | Value |
|---|---|
| Evidence state | Structural |
| Disposition | Measure |
| Current work | `AgentManager` keeps 5 agents unless `GOSLING_MAX_ACTIVE_AGENTS` overrides it. Eviction drops the agent. The next visit builds a new agent and starts that session's extensions again, including stdio processes and another Developer login shell |
| Why it is not Ready | The cache exists so idle sessions do not keep processes. Restart is the cost of that bound. This audit did not count how often a sixth session evicts a live one |
| Next measurement | Log agent create versus cache hit for a desktop session switch, with the operator's real extension list |
| Route | `audit-resource-lifecycle` if the measurement shows churn; do not raise the default until that count exists |

### 5. Review workers are full `gosling run` processes

| Field | Value |
|---|---|
| Evidence state | Structural |
| Disposition | Measure |
| Current work | One subprocess per touched file and one per check, capped by the worker semaphore. `--no-profile` skips configured extensions and plugin MCP servers, so those child processes are not started |
| Why it is not Ready | The orchestrator comments document why the in-process path was replaced: interleaved stdout and oversized prompts. Each process does distinct model work. The avoidable part, if any, is process startup tax, which was not timed |
| Next measurement | Time from `spawn` to first provider request versus model time, on a 1-file and a 20-file diff |
| Route | `audit-performance-profile` |

## Material findings

### REL-OPT-001: Login PATH starts a separate shell for each consumer

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Reliability

Evidence:
- `crates/gosling/src/agents/agent.rs:424-425` — desktop defaults `use_login_shell_path` to true
- `crates/gosling/src/agents/agent.rs:640-675` — that flag is passed into `ExtensionManager` and `HookManager::load`
- `crates/gosling/src/agents/platform_extensions/mod.rs:148-157` — Developer is `default_enabled: true`
- `crates/gosling/src/session/extension_data.rs:313-328` — a session with no saved extension list loads the configured enabled set
- `crates/gosling/src/agents/platform_extensions/developer/mod.rs:80-87` — `DeveloperClient::new` always builds `ShellTool::new(context.use_login_shell_path)`
- `crates/gosling/src/agents/platform_extensions/developer/shell.rs:312-327` and `:369-380` — `ShellTool::new(true)` spawns `resolve_login_shell_path` immediately
- `crates/gosling/src/agents/platform_extensions/developer/shell.rs:214-248` — that function runs the user's shell with login and interactive flags
- `crates/gosling/src/hooks/mod.rs:528-538` and `:605-632` — the first command hook calls `shell::resolve_login_shell_path` again through a hooks-only `OnceLock`
- `crates/gosling-mcp/src/subprocess.rs:43-73` and `:192-199` — `user_login_path` spawns a third login shell into its own `OnceLock`
- `crates/gosling-mcp/src/computercontroller/mod.rs:974-981` and `:1168-1172` — script execution and Peekaboo call `merged_path()`, which calls `user_login_path`
- `crates/gosling/src/agents/platform_extensions/summon/delegate_config.rs:101-109` — a subagent copies the parent's login-path flag
- `crates/gosling/src/agents/subagent_handler.rs:183-203` — the subagent constructs a new agent and adds each requested extension, which constructs a new `ShellTool` when Developer is requested

Observed behavior:
- A desktop session that loads the default Developer extension starts one interactive login shell at extension load, before the user runs a shell command.
- A later command hook does not read that result. It starts another login shell and caches it only for hooks.
- A later computer-control script or Peekaboo call starts a third login shell and caches it only inside `gosling-mcp`.
- A delegate that includes Developer starts one more login shell, because `LoginPath` is per `ShellTool`.

Expected boundary:
- One successful login-PATH capture per OS process should satisfy every later child that only needs that PATH.

Failure mechanism:
- The three caches were added separately. Nothing publishes the first result to the other two, and `LoginPath` is not process-global, so each Developer client repeats the spawn.

Break-it angle:
- A desktop session with Developer, one plugin command hook, and one computer-control script pays for three probes. Two concurrent subagents that both include Developer pay for two more. A hung profile is killed at 5 seconds in each implementation, so the duplicates can stack those waits when they overlap.

Impact:
- Extra interactive shells on the desktop session path and on any delegate that includes Developer. CLI agents stay off this path unless the flag is set explicitly: `resolve_use_login_shell_path` defaults to false for `GoslingCli`.

Operational impact:
- Blast radius: Workflow
- Side-effect class: process
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- `REL-OPT-003` is the same shape at a smaller scale: a process per lookup where an in-process lookup exists.
- Evicting a cached agent (`DEFAULT_MAX_SESSION` is 5) drops the `ShellTool` and the next visit starts the Developer probe again. That restart is listed as Measure, not as this finding.

Recommended mitigation:
- Remediation patterns: collapse duplicate derivation; keep the existing deadline and kill behavior on the single implementation.
- Minimal repair: move the hooks `OnceLock` and `gosling_mcp::subprocess::user_login_path` onto one shared cache. Optionally store that cache in a process-level cell that `LoginPath::spawn` consults before starting a shell.
- Local guardrail: a test that the second and third callers do not spawn when the first probe has completed.
- Behavior test: parent plus one Developer subagent, one command hook, and one `merged_path()` call yield a single login-shell spawn.

Implementation assessment:
- Complexity: cross_process_coordination
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: grok
- Rationale: three small Rust call sites and a regression test. The cache must stay process-local and must preserve the 5-second kill path. No product-policy decision is required to share the result. Deciding to stop the eager first spawn is a separate latency tradeoff and is not part of this minimal repair.

Validation:
- Assert spawn count, not source text. Cover desktop flag true, CLI flag false, hook with the flag on and off, and `merged_path` before and after a completed probe.

Non-goals:
- Do not remove login-PATH recovery for desktop launches.
- Do not change shell command execution, process groups, or Flatpak spawning.
- Do not delay the first probe in the same patch unless a measurement shows the eager spawn is the dominant cost.

### REL-OPT-002: `gosling review` runs overlapping git processes

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Reliability

Evidence:
- `crates/gosling-cli/src/commands/review/handler.rs:74-120` — `find_repo_root`, `touched_files`, and `collect_diff` run before the `--summary-only` return, which then calls `collect_diff_stat`
- `crates/gosling-cli/src/commands/review/handler.rs:352-365` — `git rev-parse --show-toplevel`
- `crates/gosling-cli/src/commands/review/handler.rs:379-407` — `git diff --name-only` for the same range
- `crates/gosling-cli/src/commands/review/handler.rs:410-431` — `git diff` for that range
- `crates/gosling-cli/src/commands/review/handler.rs:434-458` — `git diff --stat` for that range
- `crates/gosling-cli/src/commands/review/handler.rs:465-485` — `git ls-files --others --exclude-standard` when `--range` is absent

Observed behavior:
- A normal review without `--range` starts four git processes: rev-parse, name-only diff, full diff, and untracked listing.
- The name-only diff and the full diff cover the same tracked range.
- `--summary-only` still builds the name list and the full patch, uses the patch only to decide emptiness, then starts a fifth git process for `--stat` and prints that.

Expected boundary:
- One git invocation should be enough to know whether the range is empty and which tracked paths it contains. Summary mode should not materialize the patch it does not print.

Failure mechanism:
- `touched_files`, `collect_diff`, and `collect_diff_stat` each construct a git command. The summary short-circuit sits after the two expensive calls.

Break-it angle:
- A large diff pays for a full patch process whose bytes are discarded when the operator asked only for the stat. A rename-heavy diff is the fixture that can make a naive `diff --git` parse disagree with `git diff --name-only`.

Impact:
- One extra git process on every review, plus a full patch read on `--summary-only`. Local to the review command. No model call is implied by the extra git process.

Operational impact:
- Blast radius: Local
- Side-effect class: process
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- Review worker subprocesses are a different cost and stay in the Measure row. `--no-profile` already suppresses their extension processes.

Recommended mitigation:
- Remediation patterns: derive the name list from one diff; narrow `--summary-only` to stat plus the untracked listing needed for the emptiness check.
- Minimal repair: stop calling `touched_files` when `collect_diff` output is already in hand, and return before `collect_diff` when `summary_only` is set.
- Local guardrail: rename, copy, and untracked fixtures.
- Behavior test: spawn count and identical path lists.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: modules, tests
- Nominal implementation agent: grok
- Rationale: one CLI file and existing review tests. The risky part is path equivalence, which a fixture covers.

Validation:
- Compare path lists and summary text on a fixture with a modification, a rename, and an untracked file. Count git child processes.

Non-goals:
- Do not merge the per-file review workers into the parent process in this change.
- Do not drop untracked files from the default review.

### REL-OPT-003: Linux computer-control init runs `which` once per tool

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Reliability

Evidence:
- `crates/gosling-mcp/src/computercontroller/platform/linux.rs:29-42` — `new` calls `initialize` once per process when a display is present
- `crates/gosling-mcp/src/computercontroller/platform/linux.rs:65-104` — `initialize` calls `Command::new("which")` for each of `bash`, `python3`, and either four X11 tools or three Wayland tools
- `crates/gosling/src/agents/platform_extensions/developer/shell.rs:145-149` — the Developer shell already resolves `bash` with `which::which` and caches that result

Observed behavior:
- The first Linux automation object in a process starts up to six `which` processes, sequentially, and stops at the first missing name.

Expected boundary:
- A PATH lookup that this binary can do in process should not spawn a shell utility per name.

Failure mechanism:
- `check_dependencies` uses `Command::new("which")` inside a loop. The `which` crate is already a dependency of the workspace.

Break-it angle:
- A PATH where `which` itself is missing fails every check even when the tools are present. An in-process lookup fails only when the tool is missing.

Impact:
- A handful of short processes once per Linux process that constructs `LinuxAutomation`. No effect on macOS Peekaboo, which already checks well-known paths before any `which`.

Operational impact:
- Blast radius: Local
- Side-effect class: process
- Reversibility: reversible
- Operator visibility: log-only on initialization failure
- Rerun safety: safe

Adjacent failure modes:
- `REL-OPT-001` is the larger instance of an unshared process probe.

Recommended mitigation:
- Remediation patterns: replace the per-name process with `which::which` or `Path` lookup.
- Minimal repair: change `check_dependencies` only.
- Local guardrail: keep failing closed on the first missing tool.
- Behavior test: temp `PATH` with a subset of the names.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: modules, tests
- Nominal implementation agent: grok
- Rationale: one function. The crate is already used next door.

Validation:
- Assert which names are reported missing. Assert no child process when the lookup crate can answer.

Non-goals:
- Do not change X11 or Wayland command execution.
- Do not auto-install missing tools.

## Held and rejected candidates

| Candidate | Disposition | Why |
|---|---|---|
| Double MCP `initialize` when two callers create the same session agent | Held | `execution/manager.rs:181-185` serializes creation and re-checks the cache under that lock so two callers do not each start every server |
| `check-ollama` spawning `ps aux` and `grep` | Held as live waste; route for dead code | `ui/desktop/src/main/fileIpc.ts:202-207` builds both processes, and the preload exposes `checkForOllama`. No call site under `ui/desktop/src` invokes it. It is not a process that runs today |
| Backend registry `ps -p <pid> -o command=` | Held | `backendProcessRegistry.ts:155-172` runs it once per recorded pid at cleanup to tell a recycled pid from a real `gosling serve`. That check is the isolation boundary |
| One `gosling serve` per desktop launch | Held | `main.ts` starts it once. An external backend skips the spawn |
| Shell tool spawning a shell per user command | Held | `build_shell_command` is the command the user asked for. PATH lookup for `bash` is cached in `unix_shell`'s `OnceLock` |
| Eager Developer login shell, considered alone | Reject as "delete it" | `shell.rs:312-317` starts it on purpose so the probe overlaps agent setup. The waste is the second and third probes (`REL-OPT-001`), not the existence of the first |
| Review's per-file and per-check `gosling run` processes | Reject as unnecessary calls | `orchestrator.rs:298-312` records why the in-process path lost findings. `--no-profile` (`worker.rs:77-83`, `builder.rs:637-639` and `:675-680`) already skips configured and plugin MCP processes. Startup tax is unmeasured |
| Subagents always restart every parent MCP server | Reject | `resolve_delegate_extensions` (`source_discovery.rs:69-127`) returns no extensions when neither the role nor the request names any. Processes start only for the named subset |
| 15-minute run-status polling | Held | `ui/desktop/src/acp/runStatus.ts:11` sets `RUN_STATUS_INTERVAL_MS` to 15 minutes. `useRunStatus.ts:122-125` starts it only while a prompt or background task is active, and the call is an ACP request, not an OS process |
| Text-UI 300ms interval | Held | `ui/text/src/tui.tsx:594-605` runs the timer only while the banner or the loading spinner is visible. The comment records that an always-on timer was already removed |
| `git check-ignore` when creating an output folder | Held | `workspace/service.rs:943-968` runs git when the path is inside a repo, then falls back to the in-process matcher only if git does not return 0 or 1. The process is the accurate check, not a duplicate of a successful in-process check |
| Linux notification settings probing `which` | Reject for this register | `systemIpc.ts:114-137` runs `which` only when the operator opens notification settings, and only until the first desktop environment matches. Frequency is one user action |
| Raising `DEFAULT_MAX_SESSION` above 5 | Reject without a count | A larger cache keeps more stdio processes resident. That can move cost onto memory. Measure switch-and-evict frequency first |

## Measurement backlog

| Item | One discriminating measurement |
|---|---|
| Eager login shell | Time from `ShellTool::new(true)` to probe completion on a desktop-launched process, cold and warm profile. Compare with a deferred probe's first `shell` call |
| Agent LRU | Count agent creates versus cache hits across ten session switches with the operator's real extension list |
| Review workers | Time from subprocess spawn to first provider byte, for 1 file and for 20 files, next to model time |
| `--summary-only` | Bytes read from the full `git diff` that are never printed |

## Specialist routing

| Follow-up | Why | Evidence pointer |
|---|---|---|
| `audit-performance-profile` | Login-shell and review-worker startup time | `shell.rs:312-317`, `review/worker.rs:77-90` |
| `audit-resource-lifecycle` | Extension process lifetime across the 5-agent cache | `execution/manager.rs:15`, `:340-358` |
| `audit-deadcode-cleanup` | `check-ollama` handler with no in-repo caller | `fileIpc.ts:202-250`, preload `checkForOllama` |
| `audit-reliability` | Only if a later trace shows retry-amplified provider calls | Not reviewed here |
| `audit-memory-lifecycle` | Only if a later trace shows the full review diff or tool catalogs retained past the turn | Not reviewed here |

Do not start those audits as part of this register. Each one needs its own pass.

## Skill escalation

| Signal | Route | This report's handling |
|---|---|---|
| Repeated process setup | `audit-optimization-opportunities` owns the register; `audit-resource-lifecycle` owns handle lifetime | Filed `REL-OPT-001` and `REL-OPT-003`. Lifetime of cached agents routed, not re-audited |
| Repeated git work | This skill | Filed `REL-OPT-002` |
| Latency numbers | `audit-performance-profile` | Not invented. Author comment quoted and labeled |
| Dead IPC | `audit-deadcode-cleanup` | Held, with the caller search recorded |

## Different-angle review

| Angle | Result |
|---|---|
| User waiting | The login shell sits on desktop session setup, before the first model reply, because Developer loads with the session. Magnitude is unmeasured |
| Operator | `--summary-only` still pays for a full diff the operator does not see |
| Developer | Review and extension startup are the feedback loops with extra processes. CI selective testing was not reviewed |
| Service owner | No external API amplification was counted. Stdio MCP servers still start at session load because the model needs their tool list; that start was not called unnecessary |
| Correctness | The first login probe, the git untracked listing, the backend `ps` identity check, and review subprocess isolation are guards and were held |
| Downstream consumer | `check-ollama`'s consumer was searched and not found in-repo. Login PATH has three consumers, which is why the duplicate probes are real |
| Scale | Waste scales with desktop sessions, delegates that include Developer, and review invocations. It does not depend on a hypothetical cluster size |
| Maintenance | One shared PATH helper removes two capture implementations. A second cache would add maintenance |

## Highest-leverage next action

Share one process-wide login-PATH cache across `ShellTool`, hooks, and `gosling-mcp`, and add a spawn-count test for Developer plus one hook plus one `merged_path()` call.

That removes the processes that repeat a result the same process already paid for. Leave the first probe in place until it is timed. The review git overlap is the next change, and it is smaller.

## Validation limits and unreviewed frontier

- No process was traced with `ps`, `dtruss`, or a runtime log. Spawn counts are what the source constructs, not what a live session was seen to do.
- Co-occurrence of all three login shells requires Developer (default on), a command hook that actually runs, and a computer-control script or Peekaboo call. The spawn sites are unconditional once those consumers run. A session that never hooks and never uses computer control pays only for the Developer probe.
- Provider HTTP retries, model-list refresh, CI job duplication, selective tests, artifact retention, and UI render cost are Not Reviewed.
- `documentation/`, `generated/`, and test-only `Command::new` call sites were not treated as production process waste.
- The pre-existing dirty session log `docs/logs/session/2026-10-13-resource-handoff-audit.md` was not read as evidence and was not modified.
- Partial: this is a complete OPT-001..020 disposition for the process-spawn question. It is not a performance profile and not a resource-lifetime audit.

## Assumptions

- The operator meant this repository, the current `main` worktree, and a report at the repo root under the requested filename.
- "Processes" means OS child processes first. Repeated in-process work is included only where the taxonomy required a disposition.
- Involvement was inferred. No guarded-autopilot downgrade was required: the task is read-only plus the requested report.

## Repair disposition — 13 Oct 2026

The three findings are source-repaired in the local working tree. See [the repair evidence](docs/logs/session/2026-10-13-optimization-repair.md) for validation and limits. The original audit above remains historical evidence.

- **REL-OPT-001:** one process-wide cache and canonical probe for Developer, hooks and computer control, including delegates/later sessions. Keeps eager startup, bounded capture, five-second kill/reap, custom shell/Nushell/Flatpak handling; adds fish PATH handling and caller-order/concurrency/failure regressions. Login profile selection is now consistent: GOSLING_SHELL, then SHELL, then bash/sh. Developer command execution remains bash/sh by default.
- **REL-OPT-002:** one git diff yields raw NUL-delimited path metadata and patch. Summary returns before patch collection, preserving tracked stat output and untracked eligibility. Tests cover names/patch equivalence and child-process counts.
- **REL-OPT-003:** dependency availability uses the workspace which crate in process; dependency order, first-missing error and automation execution remain.

Status: **completed_with_partial_verification**. Rust regression execution, native Linux/Flatpak behavior and runtime process counts remain pending; no speedup was measured.
