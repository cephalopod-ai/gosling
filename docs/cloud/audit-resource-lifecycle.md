# Audit — Resource Lifecycle Lens (process / FD / socket / thread / timer / connection)

Lens: `audit-resource-lifecycle` (v0.2). Authority: **audit-only / read-only**.
Builds on `docs/cloud/00-orientation.md`. IDs: `RES-GSL-NNN`.

Primary lens focus per tasking: **leaked / zombie / defunct child processes** from
MCP subprocesses and ACP/CLI provider subprocesses, plus FD/thread/timer/connection
lifecycle. Priority given to subprocess spawn → wait/kill → cleanup on every exit
path (success, error, timeout, cancel, drop, abnormal parent death).

---

## 1. Intake summary

- **Runtime**: Rust (tokio async), plus an Electron/Node desktop shell (not reviewed
  here). Deployment shapes that matter for this lens:
  - long-running **CLI/TUI session** and **`goslingd` server** daemon — spawn and
    hold MCP extension subprocesses and ACP provider CLIs for the session lifetime;
  - **per-turn CLI provider** invocations (gemini/codex/cursor) — spawn one child
    per stream request.
- **Resource surface**: child processes (MCP stdio servers, ACP agent CLIs,
  developer-shell commands, docker `exec`, `uvx` inline-python), the tokio tasks
  reading their stdio, HTTP clients (reqwest), OAuth local callback servers, and
  subagent background tasks.
- **Symptom class in scope**: zombie/defunct accumulation, orphaned children after
  timeout/cancel/parent-death, unbounded task/subscriber growth, poll loops.
- **Central finding**: subprocess lifecycle is, on the whole, **well-engineered**.
  A shared `configure_subprocess()` helper applies `kill_on_drop(true)` +
  (Linux) `PR_SET_PDEATHSIG`, and every provider spawn path pairs it with an
  explicit `wait()`/`kill()` or moves the child into the consuming stream. The
  residual risks are (a) macOS abnormal-parent-death orphaning (self-documented),
  (b) shell-command descendant orphaning on timeout, and (c) best-effort-only
  subagent-task cancellation on drop.

Assumptions: no runtime measurement was possible in this pass (no `ps`/`lsof`/fd
census under load); all findings are `Potential` / `source-evidenced` and capped at
`Likely` per `confidence_calibration.md` (resource exhaustion without measurement).

---

## 2. Resource surface map

| Resource | Acquisition site | Reap / release mechanism |
|---|---|---|
| Shared spawn config | `crates/gosling/src/subprocess.rs:51-65` | `kill_on_drop(true)`, `process_group(0)`, Linux `PR_SET_PDEATHSIG` |
| MCP stdio child | `agents/extension_manager.rs:400-402` (`TokioChildProcess::builder(command).spawn()`) after `configure_subprocess` at `:384` | Child owned by rmcp `RunningService`; reaped via `kill_on_drop` when `McpClient` drops (session LRU eviction / `remove_extension`) |
| MCP stderr drain task | `extension_manager.rs:407-428` | bounded capture (64 KiB) but keeps reading; ends at child EOF |
| Docker MCP child | `extension_manager.rs:1022-1030`, `:1093-1101` (`docker exec -i`) | `configure_subprocess` via `child_process_client`; container itself is **externally owned** (user-supplied `container_id`) |
| Inline-python child | `extension_manager.rs:1133-1139` (`uvx`) | `configure_subprocess`; temp dir held in `temp_dir` guard |
| ACP agent CLI child | `acp/provider.rs:1032-1048` (`spawn_acp_process`, `kill_on_drop(true)` + `configure_subprocess`) | explicit `child.kill().await; child.wait().await` on **every** exit of `run_with_child` (`:754-757`) |
| ACP client loop thread | `acp/provider.rs:197-205`, `:271` | `AcpProvider::drop` drops `tx` then `join()`s thread (`:686-694`) → loop ends → child killed |
| ACP child stderr task | `acp/provider.rs:750` (`forward_child_stderr`) | ends at child stderr EOF (child killed) |
| Claude Code CLI child | `providers/claude_code.rs` `CliProcess` | `Drop` aborts stderr handle + `child.start_kill()` (`:290-294`) |
| Per-turn gemini child | `providers/gemini_cli.rs:139` | `kill_on_drop(true)` + `child.wait()` (`:297/307/316`), child moved into stream |
| Per-turn codex child | `providers/codex.rs:203` | `configure_subprocess` + `child.wait()` (`:277`) |
| Per-turn cursor child | `providers/cursor_agent.rs:220` | `configure_subprocess` + `child.wait()` (`:255`); status check uses `.output().await` (self-reaping) |
| Developer-shell child | `agents/platform_extensions/developer/shell.rs:525-527` | timeout → `start_kill()` + `wait()` (`:548-553`); `kill_on_drop(true)` (`:667`) |
| Shell output task | `shell.rs:539` | `abort_handle.abort()` on drain timeout (`:584`) |
| OAuth callback servers | `providers/xai_oauth.rs:523`, `providers/chatgpt_codex.rs:769` | `ServerHandleGuard::drop` → `abort()` |
| Subagent background task | `agents/platform_extensions/summon.rs:1441` | capped at `max_background_tasks()` (`:1366-1372`); `Drop` best-effort `cancellation_token.cancel()` via `try_lock` (`:334-343`) |
| PostHog HTTP client | `posthog.rs:47` (`Lazy`) | static, reused |
| Classification HTTP client | `security/classification_client.rs:54` | stored on struct, reused, has timeout |
| OAuth device-flow poll | `providers/oauth_device_flow.rs:140` | deadline stop-condition (`:141`) + RFC 8628 `slow_down` backoff (`:160`) |

---

## 3. Ownership matrix — subprocess exit paths (the priority surface)

For each spawner, release trigger on: **success / error / timeout / cancel / drop /
abnormal-parent-death (APD)**.

| Spawner | success | error | timeout | cancel | drop | APD |
|---|---|---|---|---|---|---|
| ACP agent CLI (`run_with_child`) | kill+wait `:755-756` | kill+wait (result not `?`-ed before kill) | via inner timeouts→loop exit→kill+wait | tx-drop→loop exit→kill+wait | `kill_on_drop` | Linux: PDEATHSIG; **macOS: orphan** |
| MCP stdio child | LRU/remove→drop→`kill_on_drop` | connect-fail path drops transport→`kill_on_drop` | rmcp per-request timeout only kills the *request*, not the child (persistent server) | request cancel → request-level, child persists (intended) | `kill_on_drop` | Linux PDEATHSIG; **macOS: orphan** |
| Developer shell | `wait()` `:557` | error return → `kill_on_drop` | `start_kill`+`wait` `:548-553` (**shell PID only**, not descendants) | future-drop → `kill_on_drop` (shell PID only) | `kill_on_drop` | Linux PDEATHSIG; macOS orphan |
| Per-turn provider CLIs | `wait()` | early return → `kill_on_drop` | n/a (no per-turn timeout kill traced) | stream-drop → `kill_on_drop` | `kill_on_drop` | Linux PDEATHSIG; macOS orphan |
| Subagent task | task ends | task ends | `max_turns` bound | token cancel (if wired) | **detach** (JoinHandle dropped) + best-effort token cancel | task keeps running until `max_turns` |

No spawner was found with a fully-missing reap on the happy path. The gaps are on
specific non-happy paths, captured below.

---

## 4. Findings

### RES-GSL-001: MCP / ACP / provider children orphan on hard parent SIGKILL on macOS

Severity: Medium (Low on Linux) · Confidence: Likely · Evidence basis: source-evidenced
Domain: Reliability (RES-002 / shutdown angle)

Evidence:
- `crates/gosling/src/subprocess.rs:52-63` — the reap strategy is
  `kill_on_drop(true)` (fires only on **graceful** handle drop) backstopped on
  Linux by `PR_SET_PDEATHSIG`. The code comment states the gap explicitly:
  > "macOS has no in-process equivalent, so a hard parent SIGKILL can still orphan children."
- Linux backstop is proven by `crates/gosling/tests/subprocess_cleanup.rs:44-75`
  (`child_process_exits_when_parent_process_dies`, gated `#![cfg(target_os = "linux")]`).

Observed behavior:
- On graceful shutdown, agent LRU eviction, or extension reconfigure, children are
  killed via `kill_on_drop`. If the parent (`goslingd`/CLI) is `SIGKILL`ed
  (OOM-killer, crash, `kill -9`, desktop force-quit), macOS has neither
  `kill_on_drop` (never runs — no drop on SIGKILL) nor a PDEATHSIG equivalent.

Failure mechanism:
- Every live MCP stdio server, ACP agent CLI, and any long-running provider child
  is reparented to init and survives. There is no startup sweep that reclaims
  orphans from a previous crashed parent.

Break-it angle:
- On macOS, start a session with several MCP extensions + an ACP provider, then
  `kill -9` the gosling parent. Expect the MCP/ACP children (and their own grand-
  children) to persist. Repeat across crash-restart cycles → monotonic orphan
  accumulation, each holding fds and possibly network sockets.

Impact:
- Process-table and fd growth across crash cycles on macOS; stale MCP servers may
  hold locks/ports. Bounded per-crash (not per-request), so slow-growing.

Operational impact:
- Blast radius: Local (workstation) · Side-effect class: process · Reversibility:
  compensatable (manual kill) · Operator visibility: silent · Rerun safety: unknown
  (a stale server on a fixed port could collide with the new one).

Adjacent failure modes: RES-GSL-002 (shell descendants), RES-GSL-003 (subagent tasks).

Recommended mitigation:
- macOS: spawn children in a dedicated process group and register a `kqueue`
  `NOTE_EXIT` watcher, or adopt a supervisor/`launchd`-reaper, or a **startup sweep**
  that kills orphaned children tagged with the prior parent's session id.
- Minimal repair: at startup, scan for and reap children matching a gosling session
  marker env var left in the child environment.
- Behavior test: macOS — `kill -9` the parent with N children live; assert child PIDs
  are gone (or reaped at next start).

Implementation assessment:
- Complexity: cross_process_coordination · Cost: M · Cost drivers: platform code,
  runtime verification on macOS · Nominal agent: claude · Rationale: platform-specific
  supervision spanning spawn + startup sweep, needs macOS runtime validation.

Validation: forced-`SIGKILL` parent drill on macOS + Linux; assert no orphaned MCP/ACP
children (`requires-authorized-drill`).

Non-goals: do not change the Linux PDEATHSIG path (proven working).

---

### RES-GSL-002: Developer-shell timeout/cancel kills only the shell PID, not its descendants

Severity: Low · Confidence: Likely · Evidence basis: source-evidenced
Domain: Reliability (RES-002 / RES-004)

Evidence:
- `agents/platform_extensions/developer/shell.rs:548-553` — on timeout:
  `child.start_kill(); child.wait().await;` targets only the direct shell process.
- `shell.rs:604-670` (`build_shell_command`) sets `kill_on_drop(true)` and
  `set_no_window()` but **not** a process group, and there is no group-kill
  (`kill(-pgid)`) anywhere. `configure_subprocess` (which does set
  `process_group(0)`) is deliberately **not** used on this path.
- The intended-orphan case for *backgrounded* children is documented by the test
  `shell.rs:1102-1150` (`shell_does_not_hang_on_backgrounded_process`).

Observed behavior:
- A command like `sleep 300` (foreground) that exceeds `timeout_secs` has its shell
  killed, but any child processes the shell already `fork`ed are reparented to init
  and continue. Backgrounded children (`&`) are intentionally left (documented).

Failure mechanism:
- SIGKILL to the shell PID does not propagate to descendants; no process-group kill.

Break-it angle:
- Run a tool call whose command spawns a long-lived foreground grandchild
  (`bash -c 'python -c "import time;time.sleep(600)"'`) with a short `timeout_secs`.
  Shell is killed; the python grandchild persists.

Impact:
- Orphaned grandchildren per timed-out/cancelled shell call; process/fd growth
  proportional to timeout frequency. Local blast radius, silent.

Operational impact:
- Blast radius: Local · Side-effect class: process · Reversibility: compensatable ·
  Operator visibility: silent · Rerun safety: safe.

Recommended mitigation:
- Put the shell in its own process group and, on timeout/drop, kill the whole group
  (`killpg`/`kill(-pgid, SIGKILL)`) — matching the "don't hang" goal while reaping
  the tree. Preserve the existing backgrounded-process non-hang behavior via the
  output-drain timeout, not by leaving the group alive.
- Behavior test: timeout a command with a foreground grandchild; assert the
  grandchild PID is gone.

Implementation assessment:
- Complexity: local_guardrail · Cost: S · Cost drivers: unix process-group handling,
  1 test · Nominal agent: codex · Rationale: localized to `run_command`/`build_shell_command`.

Non-goals: do not start waiting on backgrounded jobs (would reintroduce the hang).

---

### RES-GSL-003: Subagent background tasks are detached (not aborted) on SummonClient drop; cancellation is best-effort

Severity: Low · Confidence: Plausible · Evidence basis: source-evidenced
Domain: Reliability (RES-005 / repetition + shutdown angle)

Evidence:
- `agents/platform_extensions/summon.rs:334-343` — `Drop` cancels running tasks only
  `if let Ok(tasks) = self.background_tasks.try_lock()`; on lock contention the loop
  is skipped and **no** task is cancelled.
- The `JoinHandle` stored at `summon.rs:1458-1464` is dropped with the map, which
  **detaches** the tokio task rather than aborting it.

Observed behavior:
- If `SummonClient` is dropped while `background_tasks` is locked (a concurrent
  delegate/collect in flight), spawned subagent loops keep running to completion.
  Even on the success path, dropping the `JoinHandle` does not stop the task; only
  the cooperative `cancellation_token` would, and it only fires when `try_lock`
  succeeds.

Failure mechanism:
- tokio `JoinHandle` drop ≠ abort; best-effort `try_lock` can no-op under contention.

Break-it angle:
- Start N async delegations, then drop the extension while a `collect`/`delegate`
  holds the lock; the running subagent tasks continue (each still doing model calls
  and tool execution) until `max_turns`.

Impact:
- Bounded work amplification: each stray task is capped by `max_turns` and total
  concurrency by `max_background_tasks()` (`summon.rs:1366-1372`), so this is
  self-limiting, but it can run LLM/tool calls after the operator has torn down the
  extension. Local blast radius; may incur provider cost.

Operational impact:
- Blast radius: Workflow · Side-effect class: process/network/external API ·
  Reversibility: compensatable · Operator visibility: silent · Rerun safety: safe.

Recommended mitigation:
- Hold `handle.abort_handle()` and abort on drop (not just token-cancel), and replace
  `try_lock` with a `blocking_lock`/drain that guarantees every token is cancelled.
- Behavior test: drop the client mid-delegation; assert the spawned task is aborted
  (task no longer advances `turns`).

Implementation assessment:
- Complexity: local_guardrail · Cost: S · Cost drivers: 1 module, 1 test ·
  Nominal agent: codex · Rationale: localized to `SummonClient::drop` + task struct.

Non-goals: do not remove the `max_background_tasks` cap or change `max_turns`.

---

### RES-GSL-004: MCP graceful shutdown relies on `Drop` + `kill_on_drop`, with no explicit `RunningService` cancel and unbounded-lifetime stderr drain task

Severity: Info · Confidence: Plausible · Evidence basis: source-evidenced
Domain: Reliability (RES-001 / RES-005)

Evidence:
- `agents/extension_manager.rs:1212-1217` — `remove_extension` only does
  `self.extensions.lock().await.remove(&sanitized_name)`; there is no explicit
  `RunningService::cancel()`/shutdown await. Reaping the MCP child depends entirely
  on `McpClient` → `RunningService` → `TokioChildProcess` drop firing `kill_on_drop`.
  `McpClient` (`agents/mcp_client.rs:553-559`) has **no** `Drop` impl.
- `agents/extension_manager.rs:407-428` — the stderr drain task is `tokio::spawn`ed
  and, on the success path, "is detached and lives as long as the MCP server"
  (per the in-code comment). It is bounded in *memory* (64 KiB cap) but its
  *task/fd lifetime* is coupled to the child's stderr closing.

Observed behavior:
- Correct teardown depends on rmcp promptly dropping the child when the client
  handle drops. This is dependency-internal behavior not verifiable from this repo.
  If rmcp's `RunningService` drop does not synchronously drop the transport's
  `Child`, the child (and its reap) could lag until the service event-loop task ends.

Failure mechanism:
- Implicit-drop teardown with no explicit cancel/await; reap timing is not asserted.

Break-it angle:
- Loop add/remove of a stdio MCP extension N×100 and census child PIDs + fds:
  confirm plateau (Drop reaps promptly) vs monotonic (reap lags behind removals).

Impact:
- If reap lags, transient zombie/fd accumulation under rapid extension churn.
  Likely benign given `kill_on_drop`, but unproven.

Operational impact:
- Blast radius: Local · Side-effect class: process · Reversibility: compensatable ·
  Operator visibility: silent · Rerun safety: safe.

Recommended mitigation:
- Add an explicit async shutdown on `remove_extension` (call rmcp `cancel().await`
  or an equivalent) and a regression test that asserts the child PID is gone after
  removal returns.
- Behavior test: add→remove MCP extension; assert child PID absent and fd count
  returns to baseline.

Implementation assessment:
- Complexity: workflow_protocol · Cost: S · Cost drivers: 1 module, 1 integration
  test · Nominal agent: codex · Rationale: small change plus a census-asserting test.

Non-goals: do not remove `kill_on_drop` (it is the correct backstop).

---

## 5. Non-findings (seams checked and held)

- **ACP agent subprocess — full lifecycle held.** `acp/provider.rs:1032-1048`
  spawns with `kill_on_drop(true)` + `configure_subprocess`; `run_with_child`
  (`:754-757`) runs `child.kill().await; child.wait().await` after `run` returns on
  **both** success and error (result is not `?`-propagated before the kill). Early
  `?` on `stdin/stdout.take()` (`:747-748`) drops `child` → `kill_on_drop`. `Drop`
  (`:686-694`) closes `tx` then joins the loop thread so the child is reaped before
  the provider is gone. Robust across success/error/cancel/drop.

- **Per-turn provider CLIs held.** gemini_cli (`:139` `kill_on_drop`, `:297/307/316`
  `wait`), codex (`:203` `configure_subprocess`, `:277` `wait`), cursor_agent
  (`:220`/`:255`). Children are moved into the returned stream so they are not
  dropped-early; error returns fall back to `kill_on_drop`.

- **Claude Code `CliProcess` held.** `providers/claude_code.rs:290-294` `Drop`
  aborts the stderr task and `start_kill()`s the child.

- **Linux abnormal-parent-death held & tested.** `subprocess.rs:6-23`
  (`PR_SET_PDEATHSIG` + getppid re-check to close the spawn race) proven by
  `tests/subprocess_cleanup.rs`.

- **Docker MCP container held (external ownership).** The container is **not**
  created by gosling — `container_id` is user-supplied
  (`gosling-server/src/routes/agent.rs:617`, `gosling-cli/src/cli.rs:1373/1514`);
  `agents/container.rs` is an id wrapper with no lifecycle. gosling runs
  `docker exec -i` whose child is reaped via `configure_subprocess`. Stopping the
  container is correctly the caller's responsibility, not a gosling leak.

- **OAuth local callback servers held.** `providers/xai_oauth.rs:523-526` and
  `providers/chatgpt_codex.rs:769-772` `ServerHandleGuard::drop` → `abort()`.

- **HTTP clients reused (no per-request pools).** PostHog is a static `Lazy`
  (`posthog.rs:47`); `ClassificationClient` stores one `reqwest::Client` with a
  timeout (`security/classification_client.rs:54`). reqwest pools connections
  internally; no per-request client construction in these hot paths.

- **MCP notification subscribers bounded.** `agents/mcp_client.rs:351-358`,
  `:366-373` prune closed senders on send (`retain`), and `subscribe` prunes on
  registration (`:872`). No unbounded subscriber `Vec` growth.

- **`ActiveToolCallGuard` held.** `mcp_client.rs:159-174` `Drop` unregisters the
  active tool call; the guard covers cancellation and dropped reply streams
  (comment at `:650-652`), not just success.

- **MCP stderr capture bounded.** `extension_manager.rs:414-427` caps retained
  bytes at 64 KiB while continuing to drain so the child never blocks on a full pipe.

- **Shell output task bounded.** `shell.rs:539-587` aborts the collector task on
  drain timeout (`:584`); `rx.close()` + drain afterwards.

---

## 6. Polling / update-frequency assessment

- **OAuth device flow** (`providers/oauth_device_flow.rs:117-165`) is the only
  material poll loop reviewed. It is RFC 8628-correct: a **deadline** stop-condition
  (`:137-142`, from `expires_in`), honors server-provided `interval`
  (`:178`, default `DEFAULT_POLL_INTERVAL_SECS`), and applies **backoff** on
  `slow_down` (`:159-160`). It is a short, user-driven, one-shot interactive flow
  (not a background daemon poller). **No waste-ratio concern** — the change rate is
  "user authorizes once," the poll rate is server-dictated with backoff. Held.
- No fixed-rate/no-backoff background pollers were found in the core crate during
  this pass. Provider streaming uses event/stream reads, not polling.

---

## 7. Time-to-exhaustion

No monotonic-per-request leak was identified, so no per-request exhaustion estimate
applies. The two accumulation vectors are **per-crash-cycle** (RES-GSL-001, macOS)
and **per-timeout** (RES-GSL-002) — both slow and bounded by operator behavior, not
request rate. Arithmetic cannot be completed without a measured orphan-per-event
rate and the workstation's `kern.maxproc`/`ulimit -n`, which were not sampled in
this read-only pass.

---

## 8. Highest-leverage next action

Run the **add/remove-MCP-extension × N** and **forced-SIGKILL-parent** census drills
(RES-GSL-004 and RES-GSL-001) on macOS and Linux with `ps`/`lsof` snapshots. Those
two measurements convert every `Likely`/`Plausible` here to `Confirmed`-or-cleared
and directly settle the only reap paths that depend on OS/dependency behavior rather
than on code visible in-repo.

---

## 9. Validation limits (what was NOT reviewed / could not be measured)

- **No runtime measurement.** No `ps`/`lsof`/`/proc` fd or thread census under load;
  all findings are static (`source-evidenced`), capped at `Likely` per calibration.
- **rmcp / `TokioChildProcess` internals not traced.** Whether dropping
  `RunningService` promptly drops the child (firing `kill_on_drop`) is
  dependency-internal and unverified (drives RES-GSL-004).
- **Electron/`ui/desktop` not reviewed.** How the desktop shell spawns/tears down
  `goslingd`, and Node-side child/socket lifecycle, are out of this pass.
- **`gosling-server` daemon FD/socket accounting not reviewed** (session_event_bus,
  SSE/websocket connection lifetime, per-connection task growth) — only the
  `RequestGuard` drop at `session_event_bus.rs:211` was noted in passing.
- **Remote streamable-HTTP MCP** (`StreamableHttpClientTransport`) connection
  pooling / reconnect lifetime not assessed.
- **On-disk growth (RES-017/018)** — temp files (inline-python `tempdir`,
  `save_full_output` slots) and log/session growth were only spot-checked; a full
  disk-retention sweep belongs to a dedicated pass.
- **Full call-graph of MCP child reaping on session LRU eviction** was inferred from
  the drop chain + the `subprocess.rs:52-56` comment, not traced end-to-end through
  the session manager.
