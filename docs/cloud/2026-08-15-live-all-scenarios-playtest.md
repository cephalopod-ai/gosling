# Gosling live playtest — all 110 scenario cards

Date: 2026-08-15 (America/New_York)  
Lens: private agent-skills catalog `audit-playtest-app`  
Scope: every card in `docs/test_scenarios/01` through `18` (110 IDs in the library index)  
Build: local debug `target/debug/gosling` from HEAD `073d19428509ea6eb317924b1856a1fe7e9002c8`, version `0.1.0`

## Verdict

CLI configure-equivalent isolation, `info`/`doctor`, quiet `run`, session list/export/import, MCP install/remove, local open-plugins install/update, headless budgets, authenticated `serve` bind, TLS cert rejection, and `GOSLING_PATH_ROOT` isolation were healthy under the disposable fixture.

Three confirmed product defects were observed live. The most user-visible is that a **broken MCP stdio extension can hang `gosling run` with no stdout/stderr** until the test deadline. The other two are non-interactive CLI honesty: session **remove/fork require a TTY and fail with `not connected` after announcing the action**, and **`gosling acp` exits 0 on initialize with empty stdout**.

Outcome count: **58 Pass · 5 Fail · 47 Blocked · 0 N/A · 0 Not executed = 110**.

`Blocked` means the card was reached and a named prerequisite prevented a decisive result. Desktop/native GUI cards were not driven through Computer Use; they are blocked, not passed.

## Environment and evidence

- Repository: `/Users/eric/Work/vscode/forked/gosling`
- Platform: macOS 26.5.0 (Darwin 25.5.0), arm64
- Isolated state/evidence root: `/tmp/gosling-playtest-20260815`
- Raw evidence: `/tmp/gosling-playtest-20260815/evidence` (ledger.json + per-card captures)
- Provider oracle: test-only OpenAI-compatible loopback on `127.0.0.1:8765` (`fixtures/oracle.py`); deterministic replies; no production credentials
- Additional fixtures: local stdio MCP, broken MCP (`exit 2`), local git open-plugins plugin with `plugin.json`
- Desktop: not driven. No Computer Use / accessibility automation was used. Native Desktop cards are Blocked.
- Scenario library: existing `docs/test_scenarios/` (not rewritten). Full-library pass in numeric file order.
- Isolation: `GOSLING_PATH_ROOT=/tmp/gosling-playtest-20260815`. Operator `~/.config/gosling` was not used as the write root.

## Repository understanding

Gosling is a Rust AI agent framework with CLI, Ink TUI, Electron Desktop, and ACP/HTTP serve surfaces. Likely users are operators who chat with an agent that can use tools, MCP extensions, skills, and sessions. Primary happy path: configure a provider, send a prompt, get a reply, persist the session.

## App type and run method

- Type: multi-surface local agent (CLI + Desktop + TUI + ACP server)
- Run: `source bin/activate-hermit && cargo build -p gosling-cli --bin gosling` (finished in ~2 min)
- Config: wrote disposable `config.yaml` + `secrets.yaml` (keyring disabled). Interactive `gosling configure` requires a TTY and was not used.

## Scenario library

- Path: `docs/test_scenarios/`
- Existing library (18 card files + README)
- Ordering: README numeric file order
- Cards selected: all 110 index IDs including `CX-10`
- Coverage gap: README says “110 scenarios”; the index table is complete at 110 once `CX-10` is included

## Confirmed findings

### GSL-PLAY-2026-004 — Broken MCP stdio extension hangs `gosling run` silently

Severity: **High** · Card: EX-03 · Evidence basis: runtime-observed

`gosling run --with-extension 'python3 …/mcp_broken.py' -t 'Reply with exactly EX03-STILL-WORKS' -q` produced no stdout and no stderr for 40.007s and was killed by the playtest deadline. The fixture process exits 2 immediately. The operator gets no named spawn/connection failure within the 10s local-feedback budget.

Expected: session still starts; error is named; other work continues.

Reproduction:

1. Disposable `GOSLING_PATH_ROOT`; provider configured.
2. `python3 -c 'import sys; sys.exit(2)'` as the MCP command (or `fixtures/mcp_broken.py`).
3. `gosling run -t "Say HI" -q --with-extension 'python3 /path/to/mcp_broken.py'`
4. Observe hang / missed deadline and empty streams.

### GSL-PLAY-2026-005 — Non-interactive session remove/fork dies with `not connected`

**Resolution (2026-08-18): Closed.** Removal now refuses before output/mutation unless `--yes` is
provided; non-TTY fork refuses before copying. Live evidence and commits are recorded in
`2026-08-18-live-all-scenarios-playtest.md` (`e7ff63031`).

Severity: **Medium** · Cards: SE-01, AC-02 · Evidence basis: runtime-observed

`gosling session remove --name se-remove-me` printed `The following sessions will be removed:` then `Error: not connected` and left the session in `session list`. The same `not connected` error occurs for `session remove --session-id <existing>` and `session --resume --name fork-src --fork`. Missing IDs correctly exit 1 with `Session ID '…' not found.` There is no `--yes` / `--force` on `session remove --help`.

Expected: scripted remove of a named existing session either deletes it or refuses with an actionable non-TTY message before listing victims as if the action will proceed.

Reproduction: `GOSLING_PATH_ROOT=… gosling session remove --session-id <real-id>` with stdin not a TTY.

### GSL-PLAY-2026-006 — `gosling acp` initialize yields empty stdout and exit 0

**Resolution (2026-08-18): Closed.** EOF now gives queued ACP responses a bounded drain opportunity.
Initialize+EOF produced one valid JSON-RPC response; see `2026-08-18-live-all-scenarios-playtest.md`
and `e5436dfe6`.

Severity: **Medium** · Cards: HS-03, AP-05 · Evidence basis: runtime-observed

A child `gosling acp` given either an NDJSON initialize line or a `Content-Length` framed initialize JSON-RPC message exited 0 with empty stdout and empty stderr (8s bound). That looks like a successful handshake to a script and is not a structured ACP response.

Expected: valid initialize gets a structured JSON-RPC result or error; stdin close after a short payload should not look like success-with-no-bytes.

Reproduction: `printf '%s\n' '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' | gosling acp ; echo exit:$?`

## Live observations that are not product defects

- LC-04 broken YAML: `info` exits 0 but stderr names the file and parse error and the file is preserved. Matches current `Config::load` skip-on-read + refuse-on-write design. Invalid typed values (`GOSLING_MODE: yolo`) emit `Warning:`.
- SK-02 first attempt failed because the fixture lacked `plugin.json`. A second fixture with `plugin.json` + `skills/audit/SKILL.md` installed and updated. Not a product defect.
- CH-01 first fail was an oracle that always returned `PONG`. Retest with a listing-aware oracle passed.
- `--system` influence could not be proven because the oracle ignores system messages; CX-10 still passed on config-hash isolation.

## Scenario ledger (110/110)

| ID | Outcome | Live result / blocker |
|---|---|---|
| LC-01 | Pass | Quiet `run` returned exact `PONG` (0.5s). |
| LC-02 | Pass | Unconfigured home: info/check/doctor/list finished finitely. |
| LC-03 | Pass | Healthy `--check` ok; closed-port check exit 1; no key leak in `-v`. |
| LC-04 | Pass | Broken YAML warned+preserved; invalid typed settings warned. |
| CH-01 | Pass | Response named `alpha.txt` and `beta.txt`. |
| CH-02 | Pass | Whitespace-only run did not create a billed PONG turn. |
| CH-03 | Pass | Failed provider then later `READY`. |
| CH-04 | Pass | Named session survived in `session list`. |
| CH-05 | Pass | Distinct markers in isolated project dirs. |
| CH-06 | Pass | Piped `/help` produced command-related output. |
| WS-01–04 | Blocked | Desktop workspace surfaces; no GUI driver. |
| PM-01 | Pass | `--provider openai --model gpt-4o` succeeded. |
| PM-02 | Blocked | Mid-session `/model` needs interactive session. |
| PM-03 | Pass | Unreachable provider `--check` exit 1. |
| PM-04 | Blocked | Planner/main split not exercised. |
| EX-01 | Pass | `--with-builtin developer --no-profile` completed. |
| EX-02 | Pass | `mcp install` listed `playtest-stdio`. |
| EX-03 | Fail | Broken MCP hung 40s with empty streams. GSL-PLAY-2026-004. |
| EX-04 | Pass | `mcp remove` dropped the extension. |
| SK-01 | Pass | `skills list` finite. |
| SK-02 | Pass | Local git open-plugins install+update (valid `plugin.json`). |
| SK-03 | Pass | Bounded run only; live subagent fan-out not exercised. |
| SE-01 | Fail | Existing-session remove: `not connected`. GSL-PLAY-2026-005. |
| SE-02 | Pass | JSON export `0644`; fixture API key absent. |
| SE-03 | Pass | Good import ok; truncated/empty JSON refused. |
| PA-01 | Pass | Approve mode did not write `needs-approval.txt` without TTY. |
| PA-02 | Pass | Chat mode did not write `never-in-chat.txt`. |
| PA-03 | Pass | Separate processes honoured `GOSLING_MODE`; mid-session `/mode` not live. |
| CL-01 | Pass | All advertised subcommand `--help` exit 0. |
| CL-02 | Pass | `strat` and `--bogus-flag` exit 2, no panic. |
| CL-03 | Pass | quiet/json/stdin/missing-file contracts held. |
| CL-04 | Pass | zsh/bash/fish/nu completions; invalid shell failed. |
| ST-01–02 | Blocked | Desktop settings/sidebar. |
| ST-03 | Pass | Covered by LC-04. |
| HS-01 | Pass | HI ok; `--max-turns 2` finite; bad provider exit 1. |
| HS-02 | Pass | Serve bound; secret not logged; second bind failed. |
| HS-03 | Fail | ACP initialize empty stdout, exit 0. GSL-PLAY-2026-006. |
| SX-01 | Pass | 8 short `--no-session` runs ok. |
| SX-02–05,07,09 | Blocked | Desktop/long-history/tool-storm. |
| SX-06 | Pass | SIGKILL mid-run; later `session list` ok. |
| SX-08 | Pass | Concurrent info+list did not break later info. |
| AC-01 | Pass | Invalid resume selectors nonzero. |
| AC-02 | Fail | `--fork` non-TTY `not connected`. GSL-PLAY-2026-005. |
| AC-03 | Pass | `--edit` with `EDITOR=true` did not destroy the store. |
| AC-04 | Pass | Missing diagnostics nonzero, no artifact (2026-08-12 repair still holds). |
| AC-05 | Pass | JSON list parsed (31 sessions); limit/ascending finite. |
| AC-06 | Pass | `projects`/`project` finite. |
| AC-07 | Pass | `term init` zsh/bash/fish; unsupported shell failed. |
| AC-08 | Pass | `term info` finite. |
| AC-09 | Pass | `tui --help` ok; missing script exit 1. |
| AC-10 | Pass | `review --help` / dry run finite. |
| DT-01–10 | Blocked | Desktop UX; no GUI driver. |
| CX-01 | Pass | In-project run completed (oracle-limited instruction proof). |
| CX-02,03,05 | Blocked | Need real instruction-loading observer. |
| CX-04 | Pass | Ignored secret marker not echoed. |
| CX-06 | Pass | Writes stayed under disposable root. |
| CX-07 | Pass | `--no-session` marker absent from list JSON. |
| CX-08 | Pass | `-i` file worked; `-i`+`-t` exit 2; empty file exit 1. |
| CX-09 | Pass | Chat usable with code-execution runtime disabled. |
| CX-10 | Pass | `config.yaml` sha256 unchanged across `--system` runs. |
| PN-02 | Pass | Closed-port run failed closed. |
| PN-01,03–05,07–09 | Blocked | Dedicated 429/OAuth/Ollama/compaction fixtures. |
| PN-06 | Pass | CLI provider/model override. |
| PN-10 | Pass | `--stats` completed with token-like output. |
| AP-01 | Pass | Serve uses `GOSLING_SERVER__SECRET_KEY`. |
| AP-02 | Pass | Unauthenticated/wrong-secret HTTP did not look like success. |
| AP-03 | Pass | `--allowed-origin` documented; live CORS matrix not fully driven. |
| AP-04 | Pass | Missing TLS cert exit 1. |
| AP-05 | Fail | Same as HS-03. |
| AP-06–10 | Blocked | Multi-client ACP protocol matrix. |
| SI-01–03,06–10 | Blocked | Desktop / migration / multi-client. |
| SI-04 | Pass | Re-export overwrite exit 0. |
| SI-05 | Pass | Import `--working-dir` accepted. |

## Required coverage checklist

- First launch / empty state — LC-02
- Primary happy path — LC-01 / CH-01
- Invalid input — CL-02 / LC-04
- Persistence — CH-04 / SE-02
- Delete/cancel — SE-01 (fail) / PA-01
- Settings — LC-04 / ST-03
- Navigation — CL-01
- Relaunch — CH-04
- Interrupted workflow — CH-03 / SX-06
- File import/export — SE-02 / SE-03 / CX-08
- Error recovery — PM-03 / PN-02
- Edge input — CH-02 / CX-08
- Model/provider — PM-01 / PN-06
- Permission boundary — PA-01 / PA-02
- Concurrency/load — CH-05 / SX-01 / SX-08
- Headless/server — HS-01 / HS-02
- Resume/fork/diagnostics — AC-01 / AC-02 (fail) / AC-04
- Terminal/TUI/review — AC-07–10
- Keyboard Desktop / narrow window — Blocked (DT-03/05)
- Artifact preview/workbench — Blocked (DT-06/07)
- Instruction hierarchy — CX-01 / CX-04 (CX-02 blocked)
- Provider resilience — PN-02 / PN-06 / PN-10
- Server auth/TLS/framing — AP-01 / AP-04 / HS-03 (fail)
- Config/session migration — SI-06 blocked

## Screens or states that need follow-up

- Desktop Hub / Settings / approval dialogs (DT/WS/ST)
- Interactive `/mode` and `/model` mid-session
- ACP stdio framing against a real SDK client
- Broken-MCP hang with a shorter timeout and process-tree dump

## Recommended next tests

1. Patch and retest EX-03 with a 5s bound and named stderr.
2. Add `--yes` (or fail before “will be removed”) for `session remove` / `--fork` on non-TTY.
3. Drive `gosling acp` with the published ACP client library and record the exact framing.
4. A dedicated Desktop Computer Use pass for WS/DT/ST once an approved GUI driver exists.

## Cleanup

Oracle process stopped. Disposable root left at `/tmp/gosling-playtest-20260815` for evidence; no operator-owned sessions or keys were written. Manual cleanup: `rm -rf /tmp/gosling-playtest-20260815`.
