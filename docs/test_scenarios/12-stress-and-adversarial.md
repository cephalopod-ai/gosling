# 12 — Stress, Load, and Adversarial-but-Safe Seams

These cards push gosling the way an impatient developer will: many sessions,
flapping settings, fat histories, racing UIs, and recovery under load. They
intentionally overlap surfaces from files 01–11 but change *intensity* and
*combination*. Prefer a disposable home; cap spend with local/cheap models;
stop if the host itself is unsafe (thermal, disk &lt; 1 GiB free).

None of these scenarios are exploits. "Adversarial" means wrong order,
duplicate clicks, hostile scheduling, and environmental pain — not
vulnerability research.

Run only after a green smoke (01 → 02 → 09).

---

### SX-01 — Session stampede (many short chats)
- Goal: a burst of short sessions leaves the agent and session store healthy.
- Category: concurrency
- Preconditions: working cheap provider; ability to script `gosling run -t "ping N" -q` in a loop or open many Desktop chats quickly.
- Steps:
  1. Fire ~20 short one-shot runs (or ~10 Desktop chats) with distinct markers `MARKER-01` … in the prompt.
  2. Wait for all to terminal state.
  3. `gosling session list` (and Desktop history); open five at random; fully relaunch once.
- Expected: all runs accounted for (no silent drops; no duplicate ids); histories intact for samples; process remains responsive; no runaway CPU after completion.
- Observe: rate-limit errors should be explicit and retryable, not corrupting.
- Variations: mix `run` and interactive `session` in the same burst.

### SX-02 — Multi-window / multi-tab race on one session
- Goal: two views of the same session do not corrupt turn order.
- Category: concurrency
- Preconditions: Desktop multi-window **or** CLI + Desktop on the same session id if supported; else two terminals only if product allows dual attach (otherwise Not applicable).
- Steps:
  1. Open the same session in two windows (or two clients).
  2. Send different messages within the same second (`TAB-A`, `TAB-B`).
  3. Refresh/reopen both; inspect full history.
- Expected: both messages appear exactly once in a defined order; no merged/garbled turns; at most one active agent run at a time per design (second may queue or reject clearly).
- Observe: approval dialogs — which window owns the prompt?

### SX-03 — Rapid model thrash under an active stream
- Goal: hammering the model picker during a run cannot wedge the session.
- Category: interruption / settings
- Preconditions: ≥2 models; long-running prompt.
- Steps:
  1. Start a long task (slow count or large summarize).
  2. Alternate models as fast as the UI/CLI allows for ~10 seconds.
  3. Stop or let complete; send one clean follow-up on a chosen model.
- Expected: no crash; final selection is well-defined; follow-up runs; no zombie "running" state; cost indicators (if any) remain plausible.
- Observe: does thrash cancel the in-flight provider request cleanly?

### SX-04 — Hundred-turn history bloat
- Goal: a single session with a very long history remains usable and exportable.
- Category: boundary / persistence
- Preconditions: cheap/local model; ability to script ~50–100 short turns (`run` resume loop or API/ACP if available). Even ~40 turns is useful if budget-limited — record actual count.
- Steps:
  1. Grow one session with unique markers every 10 turns (`MARKER-n`).
  2. Open in Desktop/CLI; scroll or page history; send a follow-up referencing `MARKER-1` and a mid marker.
  3. Export (SE-02 path); trigger auto-compact if near `GOSLING_AUTO_COMPACT_THRESHOLD`; relaunch; reopen.
- Expected: UI does not freeze indefinitely; history is complete or intentionally compacted with a clear cue; follow-up either uses history correctly or states limits honestly; export finishes without OOM; restart preserves the session.
- Variations: open the fat session in two windows while sending one more turn.

### SX-05 — Extension/tool storm and max-turns budget
- Goal: runaway tool loops hit budgets instead of hanging the machine forever.
- Category: boundary / recovery
- Preconditions: Developer (or similar) enabled; set low `--max-turns` / `GOSLING_MAX_TURNS` (e.g. 5) and/or `--max-tool-repetitions 2`.
- Steps:
  1. Prompt: `Stat every file recursively under this directory and narrate each path; do not stop.`
  2. Watch tool call volume and termination reason.
  3. Start a second session with normal limits to confirm the machine is still healthy.
- Expected: session stops with a budget/repetition-related reason; does not continue tooling after the cap; no multi-GB log growth in one run; second session still works.
- Observe: Desktop cost/context indicators during the storm.

### SX-06 — Hard kill mid-run then recover
- Goal: unclean process death recovers to an honest world.
- Category: recovery / interruption
- Preconditions: a running interactive session and/or `gosling serve`; shell access to PIDs; disposable home.
- Steps:
  1. Start a long run; note session id.
  2. `kill -9` the gosling process (CLI session or serve — not the whole machine).
  3. Relaunch; `gosling session list` / Desktop history; open the interrupted session; send `Say RECOVERED`.
  4. If serve was killed, confirm port release and clean restart (HS-02).
- Expected: start succeeds without manual home surgery; interrupted work is not shown as forever streaming; history up to last persist is present; follow-up works or requires an explicit resume path that is documented; no duplicate session ids.
- Observe: difference vs graceful Ctrl-C (CH-03) — extra corruption here is High/Critical.
- Variations: kill only a child provider CLI if one is spawned; parent should surface crash.

### SX-07 — Workspace switch race during artifact save
- Goal: rapid workspace switches cannot mis-route saves or resurrect stale download destinations.
- Category: concurrency / files
- Preconditions: Desktop; ≥2 workspaces with different export/output folders; a saveable artifact in a session pinned to workspace A.
- Steps:
  1. Begin Save a copy / export / native download from the A-pinned session.
  2. While the dialog or save is in flight, rapidly alternate the workspace chat filter A ↔ B.
  3. Complete the save; inspect the filesystem destination.
  4. Repeat with workspace B deleted mid-save if safe (disposable only).
- Expected: file lands in A's product path (pinned), not B's; deleted pin fails with relink/warning — never silent success into the wrong tree; slower validation of an old selection cannot override a newer one (docs: ordered switches).
- Observe: collision-safe filenames under thrash.

### SX-08 — Config thrash + concurrent CLI invocations
- Goal: concurrent CLI and settings writes do not tear config or deadlock.
- Category: concurrency / settings
- Preconditions: disposable home; several terminals.
- Steps:
  1. Start 2–3 long `gosling run` jobs.
  2. In parallel loops (careful): `gosling info`, `gosling session list`, `gosling skills list` ~20×.
  3. Alternate a harmless config toggle (theme / `GOSLING_CLI_SHOW_COST`) via file edit or configure ~10 times; include one invalid YAML save that must fail.
  4. After calm: validate config parses; run one clean `gosling run -t PONG -q`.
- Expected: valid writes are atomic; invalid YAML never left as the only boot-breaking copy without recovery path; no deadlocks; list/info stay consistent; final PONG succeeds.
- Observe: file-lock errors should be transient and named if present.

### SX-09 — Cross-surface consistency after chaos
- Goal: after the stress cards above, CLI, Desktop, and on-disk state tell the same story.
- Category: navigation / recovery
- Preconditions: residual state from SX-01–SX-08 in the disposable home.
- Steps:
  1. Inventory: session list (CLI + Desktop), enabled extensions, active/default workspace, config provider/model, a sample export.
  2. Pick three entities (session, workspace, extension) and verify each appears consistently wherever linked.
  3. Run `gosling doctor` / `info -v`; fully relaunch Desktop + CLI once more; re-check the three.
- Expected: no surface claims an entity another says is gone (unless intentional delete); no permanent spinners; doctor does not report healthy when chat is hard-down; console free of unhandled exceptions on navigation.
- Observe: leftover temp files, orphan worktrees, or ballooning logs under the disposable home — Note or Medium depending on severity.
