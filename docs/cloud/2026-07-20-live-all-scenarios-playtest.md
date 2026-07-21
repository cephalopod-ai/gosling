# Gosling live playtest — all 110 scenario cards

Date: 2026-07-20 (America/New_York)  
Lens: private agent-skills catalog `audit-playtest-app`  
Scope: every card in `docs/test_scenarios/01` through `18`  
Build: local debug build from this worktree, reported version `0.1.0`  
Authority: playtest and report only; scenario cards and product source were not edited

## Verdict

The pass found working core CLI/session persistence, context isolation, local MCP,
plugin update, approval, model switching, review dry-run, and authenticated serve
paths. It also found release-blocking failures in Desktop chat, TUI/ACP session
creation, headless exit semantics, and interrupt recovery.

Outcome count: **46 Pass · 32 Fail · 32 Blocked · 0 N/A · 0 Not executed = 110**.

`Blocked` means the card was reached and assessed but a named prerequisite or an
earlier product failure prevented its decisive step. It is not counted as a pass.

## Test environment and evidence

- Repository: `/Users/eric/Work/vscode/forked/gosling`
- Isolated state root: `/tmp/gosling-playtest-20260720.uhtKri`
- Raw evidence: `/tmp/gosling-playtest-20260720.uhtKri/evidence`
- Disposable workspaces: `projects/alpha`, `projects/beta`, `context-root`, and
  `review-fixture` below that root.
- Provider: local Ollama at `localhost:11434`; installed models observed were
  `qwen2.5:latest` and `gemma4:latest`.
- Desktop: development Electron build with isolated `--user-data-dir`; the installed
  user application and normal Gosling state were not used.
- Build checks exercised: `cargo build -p gosling-cli --bin gosling` and
  `pnpm --dir ui/text build` both passed.
- No real cloud credentials were used. One deliberately invalid OpenAI key and
  generated local server/MCP secrets were synthetic and are not reproduced here.

## Confirmed findings

### GSL-PLAY-001 — Desktop Send discards a new-chat draft without creating a session

Severity: **High** · Cards: CH-01, WS-01, WS-04, DT-06, DT-07

Observed: a validated and active `Playtest Alpha` workspace was selected, the draft
`Reply with exactly DESKTOP-PONG` was entered, and Send was clicked. The composer
cleared, but no conversation, error, toast, backend event, or persisted session
appeared. CLI inventory was unchanged.

Expected: the draft should become a visible/persisted user turn, or Send should fail
visibly while preserving recoverable input.

Impact: the primary Desktop happy path loses user input silently and blocks artifact
workbench and workspace-pinned chat testing.

### GSL-PLAY-002 — ACP rejects an installed local model, breaking `gosling tui`

Severity: **High** · Cards: HS-03, AC-09, AP-05, AP-07

Observed: CLI one-shot and interactive sessions successfully used
`qwen2.5:latest`. ACP `initialize` succeeded, but an otherwise valid `session/new`
returned JSON-RPC `-32602 Invalid params` with `Model 'qwen2.5:latest' is not
available for provider 'ollama'`. After building `ui/text`, both interactive TUI and
`gosling tui --text ...` failed at session creation with `Invalid params`.

Expected: all surfaces should use the same provider/model availability result.

Impact: configured local-provider users can use CLI chat but cannot use the shipped
TUI or create ACP sessions with the same model.

### GSL-PLAY-003 — Hard provider failures return process success

Severity: **High** · Cards: HS-01, PM-03, PN-05, PN-07

Observed: a nonexistent Ollama model, an empty/malformed provider response, and an
OpenAI 401 authentication error all produced useful error prose but exited `0`.
The invalid OpenAI key was redacted in output. A failed delegated subagent workflow
also exited `0`.

Expected: headless automation should receive nonzero status for a terminal provider
or workflow failure; a success result must not contain only an error.

Impact: CI and scripts can publish false success.

### GSL-PLAY-004 — Ctrl-C leaves a stale request that replays over later prompts

Severity: **High** · Cards: CH-03, PA-03, PN-03

Observed: an interactive long request was interrupted, then
`Reply with exactly AFTER-CANCEL` was sent twice. Both follow-ups resumed the earlier
essay task instead of answering the new prompt. The session also displayed a
`todo_write` tool call while its status said chat mode restricted tool use.

Expected: cancellation should reach one terminal state within ten seconds, rewind
only incomplete assistant work, and let the next user turn run exactly once.

Impact: the user cannot trust which instruction is active after cancellation.

### GSL-PLAY-005 — Machine-readable `run` formats include a human banner

Severity: **Medium** · Card: CL-03

Observed: both `--output-format json` and `stream-json`, even with
`--no-session --no-profile`, began stdout with the Gosling ASCII/session banner.
The payload following the banner was valid JSON/JSONL.

Expected: stdout should contain parseable protocol/artifact data only; human chrome
belongs on stderr or should be suppressed.

### GSL-PLAY-006 — Invalid `config.yaml` syntax is silently discarded

Severity: **Medium** · Cards: LC-04, ST-03, CX-03

Observed: `GOSLING_PROVIDER: [` was preserved on disk, while `session` behaved as
unconfigured and `info -v` exited `0` showing defaults without naming the file or
parse error. Malformed `CONTEXT_FILE_NAMES=not-json` likewise produced no warning.

Expected: startup should name the malformed value/file and either stop or explicitly
announce a documented fallback.

### GSL-PLAY-007 — `gosling doctor` submits `/doctor` to the model

Severity: **Medium** · Card: LC-03

Observed: with a configured provider, `gosling doctor` opened an interactive session,
sent `/doctor` to qwen, received conversational text, then waited for input until
cancelled. It did not produce bounded deterministic diagnostics.

Expected: a health command should run local/provider checks, report them, and exit.

### GSL-PLAY-008 — Fresh Ollama onboarding records an unavailable model

Severity: **Medium** · Cards: LC-01, PM-01, DT-01

Observed: Desktop reported “Connected to Ollama” and saved `GOSLING_MODEL: qwen3`
without presenting a model choice. Only `qwen2.5:latest` and `gemma4:latest` were
installed. `gosling info --check` then failed with a 404 for `qwen3`.

Expected: onboarding should select an installed model or require an explicit custom
model choice and validate it before declaring success.

### GSL-PLAY-009 — Empty instructions are accepted and billed

Severity: **Medium** · Cards: CH-02, CX-08

Observed: `gosling run -t ''` and `run -i empty.json` invoked the provider and returned
generic model responses with exit `0`.

Expected: empty input should fail before provider use with a concise input error.

### GSL-PLAY-010 — Malformed plugin skill is installed into the live catalog

Severity: **Medium** · Card: SI-09

Observed: a plugin with a valid manifest but a `SKILL.md` missing required YAML
frontmatter installed successfully. `skills list` exposed `bad-plugin:broken` with a
blank description; an unaffected valid skill remained usable.

Expected: invalid skill content should be rejected or quarantined with its path and
parse reason.

### GSL-PLAY-011 — ACP lifecycle and version negotiation are too permissive

Severity: **Medium** · Cards: AP-05, AP-10

Observed: ACP stdout stayed clean NDJSON and recovered from malformed JSON, but stdin
EOF did not terminate within five seconds. Initialize accepted versions `0`, `2`, and
`65535`, echoed each as negotiated, and accepted a string version by returning `0`.

Expected: EOF should stop the stdio agent promptly; malformed/incompatible versions
should fail before session creation with a supported contract.

### GSL-PLAY-012 — Tool budget enforcement is not disclosed to the user

Severity: **Medium** · Cards: HS-01, SX-05

Observed: a request for ten identical `pwd` calls with repetition/turn limits executed
two calls, then stated that ten had run. It exited `0` without a budget/truncation
reason.

Expected: the terminal result should state that the repetition or turn budget stopped
the run and must not present incomplete work as complete.

### GSL-PLAY-013 — Dangerous unauthenticated serve warning is not user-visible

Severity: **Medium** · Card: AP-01

Observed: `serve --dangerously-unauthenticated` accepted unauthenticated ACP traffic,
but its terminal emitted no warning. The warning existed only in the JSON log file.

Expected: dangerous mode should print a prominent warning on the invoking terminal,
especially because the default builtin can execute shell-capable actions.

### GSL-PLAY-014 — Empty-name sessions become cross-surface ghosts

Severity: **Low** · Cards: CL-02, SE-01, SX-09

Observed: `gosling session -n '' </dev/null` exited `0` and created an empty, zero-turn
session. CLI listed it; Desktop Session History hid it.

Expected: reject an empty name before persistence, or render the same valid entity on
all surfaces.

## Scenario ledger (110/110)

| ID | Outcome | Live result / blocker |
|---|---|---|
| LC-01 | Fail | CLI manual configuration reached PONG; Desktop saved unavailable `qwen3`. |
| LC-02 | Pass | Unconfigured CLI/Info/Check states were actionable and non-destructive. |
| LC-03 | Fail | `doctor` became an unbounded model chat rather than diagnostics. |
| LC-04 | Fail | Invalid YAML was silently ignored. |
| CH-01 | Fail | CLI chat worked; Desktop Send silently discarded the draft. |
| CH-02 | Fail | 16 KiB persistence passed, but empty inputs invoked the provider. |
| CH-03 | Fail | Cancelled request replayed over two later prompts. |
| CH-04 | Pass | Sessions, workspace selection, and response-style setting survived relaunch. |
| CH-05 | Pass | Three parallel sessions and a 12-run stampede retained distinct IDs/directories. |
| CH-06 | Pass | `/help`, valid slash commands, and `/halp` feedback behaved coherently. |
| WS-01 | Fail | Workspace create/validate/activate passed; pinned Desktop send disappeared. |
| WS-02 | Blocked | No real credential profile/cloud secret was authorized; Desktop chat was also broken. |
| WS-03 | Blocked | Native relink picker could not be driven reliably by accessibility automation. |
| WS-04 | Blocked | Artifact generation could not begin after Desktop chat submission failed. |
| PM-01 | Fail | CLI model configuration passed; Desktop onboarding chose a missing model. |
| PM-02 | Pass | `/model gemma4:latest` switched live and returned `SWITCHED-MODEL`. |
| PM-03 | Fail | Invalid key was clear/redacted but returned exit `0`. |
| PM-04 | Pass | Planner logs used `gemma4:latest` while the main session remained qwen2.5. |
| EX-01 | Pass | Bundled developer tool was approval-gated and created only the approved file. |
| EX-02 | Pass | Local stdio memory MCP round-trip persisted the unique marker. |
| EX-03 | Pass | A broken MCP command warned and normal chat continued without its tool. |
| EX-04 | Pass | Removal cleared the extension; a second removal was a named miss. |
| SK-01 | Pass | Skills listed; a deterministic plugin skill loaded its sentinel. |
| SK-02 | Pass | Local git plugin updated atomically from V1 to V2. |
| SK-03 | Fail | Two delegate calls targeted a nonexistent source and the failed run exited `0`. |
| SE-01 | Fail | List/remove paths worked, but an empty-name ghost session was persisted. |
| SE-02 | Pass | Stdout/file export and missing-ID behavior were correct. |
| SE-03 | Pass | Valid/duplicate imports worked; truncated, empty, and binary imports were rejected. |
| PA-01 | Pass | Approval blocked side effects until Allow; Deny left no file; follow-up worked. |
| PA-02 | Pass | Denied tool side effect remained absent and the session stayed usable. |
| PA-03 | Fail | Chat-mode/cancel sequence displayed a tool call and replayed stale work. |
| CL-01 | Pass | Top-level and every advertised subcommand help path rendered and exited. |
| CL-02 | Pass | Typos, bad flags, bad selectors, and conflicting inputs failed cleanly. |
| CL-03 | Fail | JSON/stream-JSON stdout was polluted by the human banner. |
| CL-04 | Pass | zsh, bash, fish, and nu completions were nonempty; bad shell exited `2`. |
| ST-01 | Pass | Detailed response style and workspace state persisted across Electron relaunch. |
| ST-02 | Pass | Settings tabs, sidebar, extensions, history, and skills remained navigable. |
| ST-03 | Fail | Invalid YAML/value input fell back without a warning. |
| HS-01 | Fail | Success path worked; budget/provider terminal failures could still exit `0`. |
| HS-02 | Fail | Auth/port/restart worked, but ACP could not create a session with the configured model. |
| HS-03 | Fail | Initialize was valid; new-session failed and EOF did not terminate. |
| SX-01 | Pass | Twelve runs at concurrency six completed with unique markers and no orphan workers. |
| SX-02 | Blocked | Desktop could not create the first shared session due GSL-PLAY-001. |
| SX-03 | Blocked | No usable Desktop/ACP active stream remained on which to race model switches. |
| SX-04 | Blocked | No version-pinned deterministic cheap provider fixture was available for 100 turns. |
| SX-05 | Fail | Two-call budget stopped work but the result falsely claimed ten calls and success. |
| SX-06 | Pass | SIGKILL of a disposable run left no lock; immediate `RECOVERED` run succeeded. |
| SX-07 | Blocked | Desktop artifact save was unreachable after Send failed. |
| SX-08 | Blocked | Safe atomic config-writer fixture for concurrent mutation was unavailable. |
| SX-09 | Fail | CLI and Desktop disagreed about the persisted empty session. |
| AC-01 | Pass | Recency/name/ID selectors resolved correctly; invalid combinations failed early. |
| AC-02 | Pass | Fork/source exports had distinct IDs and isolated post-fork markers. |
| AC-03 | Pass | Editor failure preserved hash; fork+edit changed only the fork. |
| AC-04 | Pass | Diagnostics artifact was created; missing ID did not create a false artifact. |
| AC-05 | Pass | JSON, order, cwd filter, and limit variants were stable and parseable. |
| AC-06 | Blocked | Project picker was visible, but a safe observable platform-opener fixture was absent. |
| AC-07 | Pass | zsh/bash init scripts syntax-checked; all shells were stdout-only; bad shell failed. |
| AC-08 | Pass | Two terminal IDs stayed distinct and resumed independently. |
| AC-09 | Fail | npm fallback 404ed before build; built local TUI then failed `Invalid params`. |
| AC-10 | Pass | Review dry-run selectors/custom prompt worked; invalid range failed with git context. |
| DT-01 | Fail | Fresh onboarding claimed success while saving missing `qwen3`. |
| DT-02 | Blocked | Native close-vs-quit lifecycle could not be isolated from the dev harness confidently. |
| DT-03 | Fail | Fresh-onboarding CTA lacked a usable accessibility button role; coordinate click required. |
| DT-04 | Blocked | Shortcut conflict/rebinding controls were not reachable in the failed onboarding root. |
| DT-05 | Blocked | Reliable scripted narrow-window geometry was unavailable in the GUI harness. |
| DT-06 | Blocked | Artifact preview matrix was unreachable after chat submission failed. |
| DT-07 | Blocked | Artifact workbench state was unreachable after chat submission failed. |
| DT-08 | Blocked | Missing archive path redirected correctly; native folder picker blocked completion. |
| DT-09 | Blocked | Serve auth worked, but Desktop reconnect could not pass ACP session creation. |
| DT-10 | Blocked | OS notification permission state/denial was not safely controllable. |
| CX-01 | Pass | Root sentinel appeared in-project and not from the outside sibling. |
| CX-02 | Pass | Child A/B rules stayed scoped; direct-A loaded root plus A. |
| CX-03 | Fail | Valid custom file loaded, but malformed JSON was silently accepted. |
| CX-04 | Pass | Ignored secret/symlink stayed out of automatic context and explicit chat read was denied. |
| CX-05 | Blocked | A persistent-instruction file mutation oracle was not completed in the live session. |
| CX-06 | Pass | CLI/Desktop/config/session/plugin state remained below the disposable root. |
| CX-07 | Pass | Successful `--no-session` markers never appeared in resumable session inventory. |
| CX-08 | Fail | Empty input was accepted; binary file error began with misleading “not found” text. |
| CX-09 | Pass | Disabled code runtime failed fast naming the setting; ordinary paths remained usable. |
| CX-10 | Pass | One-shot system marker did not change the config hash or contaminate baseline. |
| PN-01 | Blocked | No deterministic 429/retry fixture or authorized paid-provider quota was available. |
| PN-02 | Blocked | No controllable mid-stream disconnect provider fixture was available. |
| PN-03 | Fail | Ctrl-C recovery replayed stale work; unreachable endpoint also hung beyond 30 seconds. |
| PN-04 | Blocked | 1 MiB input began auto-compaction but did not reach a bounded terminal result. |
| PN-05 | Fail | Empty provider response was clear but returned exit `0`. |
| PN-06 | Pass | qwen/gemma overrides produced distinct markers without persisting the override. |
| PN-07 | Fail | Missing model was actionable but returned `0`; Desktop saved a missing custom ID. |
| PN-08 | Fail | Unreachable Ollama endpoint hung with no output past 30 seconds; restored host recovered. |
| PN-09 | Blocked | No OAuth test tenant/token lifecycle was authorized. |
| PN-10 | Blocked | Local Ollama supplies no billable-cost oracle for cross-surface reconciliation. |
| AP-01 | Fail | Secret requirement worked; dangerous-mode warning existed only in file logs. |
| AP-02 | Pass | Missing/wrong token returned 401; correct upgraded; restart invalidated old secret. |
| AP-03 | Pass | Defaults allowed loopback/rejected evil; explicit origin replaced defaults exactly. |
| AP-04 | Blocked | Missing/cert-only/key-only paths failed; no generated trusted/mismatch certificate matrix. |
| AP-05 | Fail | Stdout was clean NDJSON, but valid new-session failed and EOF hung. |
| AP-06 | Pass | Malformed JSON returned parse error; later valid initialize succeeded on same process. |
| AP-07 | Blocked | Two real ACP sessions could not be created due the model-catalog mismatch. |
| AP-08 | Blocked | No active ACP prompt could be established for protocol cancellation. |
| AP-09 | Blocked | Active-request termination was unreachable because ACP session creation failed. |
| AP-10 | Fail | Unsupported and malformed versions were accepted as negotiated. |
| SI-01 | Blocked | Duplicate/rename sequence required a functioning pinned Desktop chat. |
| SI-02 | Blocked | Delete-with-open-session sequence required a functioning Desktop chat. |
| SI-03 | Blocked | Symlink artifact routing required Desktop save, blocked by GSL-PLAY-001. |
| SI-04 | Pass | Markdown/JSON/YAML parsed; overwrite was stable; bad format preserved no artifact. |
| SI-05 | Pass | Default import used current cwd; explicit safe cwd won; duplicates were non-destructive. |
| SI-06 | Blocked | No supported-old-release config/session fixture was present. |
| SI-07 | Pass | Duplicate MCP install explicitly updated; one active marker; double-remove named miss. |
| SI-08 | Pass | Synthetic MCP secret was absent from config, info output, and logs; removal succeeded. |
| SI-09 | Fail | Malformed-frontmatter skill installed into the live catalog. |
| SI-10 | Blocked | Competing Desktop/ACP clients could not both reach usable sessions. |

## Blocked prerequisite register

The 32 blocked outcomes cluster around five concrete prerequisites:

1. **Desktop chat submission failure:** downstream workspace, artifact,
   archive-race, and competing-client operations could not start honestly.
2. **ACP configured-model mismatch:** session concurrency, cancellation,
   reconnect, and active-request termination were unreachable.
3. **External service fixtures:** deterministic 429, disconnect, OAuth,
   paid cost, old release, and certificate-trust fixtures were not present or authorized.
4. **Native OS automation limits:** folder picker, notification permission,
   window lifecycle/geometry, and platform opener lacked a safe deterministic oracle.
5. **Dedicated stress/config fixtures:** 100-turn cheap deterministic history,
   atomic config thrash, and persistent-instruction mutation were not completed.

## Strong seams that held

- `GOSLING_PATH_ROOT` isolated the test state; normal installed Gosling was not used.
- Manual CLI configure, one-shot and interactive local replies, model switching, and
  planner/main split worked.
- Approval denied side effects before execution and left the session recoverable.
- Session list/export/import/remove/fork/editor-failure flows preserved data.
- Plugin V1→V2 update and stdio MCP memory round-trip worked.
- Authenticated serve token checks, exact origin replacement, port conflict, shutdown,
  and restart behaved correctly.
- ACP parse errors were structured and recoverable; stdout framing was clean.
- Context root/nested/ignored-file isolation held under the exercised fixtures.
- Twelve concurrent one-shot runs completed without orphan Gosling workers.

## Cleanup and residual risk

All Gosling/Electron/serve processes started by this pass were stopped and tested ports
were released. The disposable root and raw evidence were intentionally retained for
failure reproduction. The repository worktree was clean before this report was added;
no product source or scenario card was changed.

Residual risk is highest in the blocked Desktop/ACP cascades. Fix and retest
GSL-PLAY-001 and GSL-PLAY-002 first; doing so unlocks most of the currently blocked
cards. Then retest GSL-PLAY-003 and GSL-PLAY-004 before treating Gosling as safe for
headless automation or interruption-heavy interactive work.

## Repair campaign closure - 2026-07-20

This section supersedes the original residual-risk recommendation above while
preserving the initial report and 110-card ledger as historical evidence. All 14
original findings were repaired. A fifteenth finding was discovered during the
required installed-Desktop windowing replay and repaired in the same campaign.

### GSL-PLAY-015 - Native New Chat Window crashes the renderer

- Severity: High
- Surface: installed macOS Electron Desktop
- Reproduction: launch the signed Apple Silicon package, choose `File -> New Chat
  Window`, and observe `Cannot read properties of undefined (reading 'sender')`.
- Cause: the native menu synthesized `ipcMain.emit(createChatWindow)` without an
  Electron IPC event, while the renderer IPC handler requires `event.sender`.
- Repair: native menu actions now invoke the existing window factory directly;
  renderer-originated requests retain sender-based directory authorization.
- Verification: the repackaged installed app created two healthy windows, closed one
  without affecting the other, reopened persisted state, and terminated Electron plus
  the embedded backend within one second on `Cmd+Q`.

| Finding | Closure | Repair commits | Verification |
| --- | --- | --- | --- |
| GSL-PLAY-001 | Repaired | `3dce5e6bb` | Desktop Hub/Onboarding component suite; full 542-test Desktop suite |
| GSL-PLAY-002 | Repaired | `2338d7e85` | 71 ACP server tests; live installed Ollama model accepted |
| GSL-PLAY-003 | Repaired | `e1b7ded64`, `564b62e07`, `100368130` | Live Ollama 404 exited 1 with valid JSON `error` metadata; CLI suite passed |
| GSL-PLAY-004 | Repaired | `cc441aa27` | Interrupted-turn persistence regression and full 235-test CLI suite |
| GSL-PLAY-005 | Repaired | `e1b7ded64`, `564b62e07` | JSON/banner regressions and live valid JSON replay |
| GSL-PLAY-006 | Repaired | `4b4ac51b8` | Malformed config/context-key regressions and CLI suite |
| GSL-PLAY-007 | Repaired | `4b4ac51b8` | Live finite local `gosling doctor` exited 0 |
| GSL-PLAY-008 | Repaired | `3dce5e6bb` | Onboarding live-inventory tests; installed onboarding UI remained operable |
| GSL-PLAY-009 | Repaired | `e1b7ded64`, `564b62e07` | Live whitespace input rejected before provider with exit 1 |
| GSL-PLAY-010 | Repaired | `b89d5d269` | 11 plugin-format tests plus shared malformed-metadata regression |
| GSL-PLAY-011 | Repaired | `2338d7e85` | Live ACP v0 rejection, v1 acceptance, and EOF shutdown; ACP suite passed |
| GSL-PLAY-012 | Repaired | `e1b7ded64`, `564b62e07` | Budget disclosure regressions and full CLI suite |
| GSL-PLAY-013 | Repaired | `4b4ac51b8` | Live warning appeared before invalid bind and process exited nonzero |
| GSL-PLAY-014 | Repaired | `4b4ac51b8` | Live blank session name rejected with exit 1; storage regression passed |
| GSL-PLAY-015 | Repaired | `66309eac7` | Installed signed `arm64` package multi-window and lifecycle replay |

Final regression passed formatter, all-target Clippy with warnings denied, all 235 CLI
library tests, all 542 Desktop tests, Desktop typecheck, and the text UI build. The core
library result was 1528 passed and the same four Gate 0 environment-sensitive failures:

- `agents::container::tests::kill_terminates_exec_process_without_stopping_container`
- `providers::claude_code::tests::test_can_use_tool::allow`
- `providers::claude_code::tests::test_can_use_tool::deny`
- `providers::claude_code::tests::test_can_use_tool_cancel_on_drop`

No new core failure was introduced. Campaign execution details are recorded in
`reports/2026-07-20-live-scenarios-defect-campaign-session-log.md`.
