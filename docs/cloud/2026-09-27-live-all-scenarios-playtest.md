# Gosling live all-scenarios playtest — 2026-09-27

## Summary

The full 127-card library under `docs/test_scenarios/` was run against gosling 1.3.0 (debug build of
`main` @ `186c96a01`) on macOS 26.6.2 arm64, across the CLI, the interactive session, `run`, stdio ACP,
`gosling serve` (HTTP/WebSocket ACP) and the Electron Desktop (dev build of the same commit). The pass was
test-only; repairs were made afterwards on a separate branch (see **Repair status**).

Card results: **65 Pass · 8 Partial · 47 Fail · 3 Blocked · 4 pending** (the four Deep Research
Desktop cards DR-01, DR-04, DR-07 and DR-09 were still running when this report was written; see
**Untested**). A card is Pass only when every assertion held on every surface that was exercised;
variations are separate subcases and do not fail the base card.

Findings: **211** (IDs `GSL-PT-20260927-<group><nn>`): 12 High, 62 Medium, 113 Low, 24 Notes. The twelve
High findings cover ten distinct problems:

- **Interrupted turns re-executed** (F10, A12): a cancelled, abandoned (client disconnect) or killed turn
  left its prompt dangling; conversation repair merged it into the next prompt and the model carried out
  the cancelled action. In Auto mode this meets the Critical descriptor.
- **Auto-compaction replayed completed tool calls** (B08): compacting mid-turn folded the turn's own tool
  work into the summary and re-appended the prompt, so a side-effecting tool ran three times.
- **Extension secrets collided with provider keys** (C20): `mcp install --secret OPENAI_API_KEY=…` silently
  replaced gosling's provider key; any extension could read any stored secret, including website-login
  passwords, by naming it.
- **Session IDs were reused after deletion** (D01): stale IDs in scripts, `gosling project`, `term` shells or
  open windows attached to, rewrote or deleted an unrelated session.
- **`.goslingignore` documented but not implemented** (E01, S19): the access control the docs describe was
  removed upstream (ced5c1b10) and never existed in gosling.
- **One failure blocked a command for the whole session** (E02): an identical call that failed or was
  declined once was denied forever, breaking "run tests → fix → rerun".
- **Approve mode delegated without approvals** (E03): one `delegate` approval let the subagent run
  `shell`/`write` unprompted.
- **Working-directory restriction escaped with `cd ..`** (E04).
- **Desktop "Create workspace" started a real ChatGPT OAuth flow** (G101) for an unconfigured provider.
- **An invalid `GOSLING_MODE` failed open to Autonomous** (G130).

The strongest held seams: HTTP ACP authentication, origin and TLS enforcement; 20-way concurrent session
creation and 100-turn histories; hard-kill recovery (port released in 0.06 s, session answered in 1.2 s);
approval gates in Manual mode on CLI and ACP; context scoping for root instructions; `--no-session` no longer
leaves prompt text on disk (2026-09-13 finding 004 is fixed); Desktop artifact trash, clipboard, repository
filter, timestamps and cross-surface consistency after chaos.

## Scenario library and run identity

- Source: `docs/test_scenarios/README.md` and card files 01–20 (127 cards), full-library pass shape, cards
  run in numeric order within each group. No scenario card was edited.
- Run ID `GSL-PT-20260927`. Test groups (one agent each, own ports and disposable roots): A lifecycle/chat/CLI,
  B providers/resilience, C extensions/skills/state, D sessions/advanced CLI, E context/permissions,
  F headless/serve/ACP, S stress, H Deep Research protocol, G1 Desktop core, G2 Desktop artifacts/research.
- Build under test: a scratch copy of the debug `gosling` binary (sha256 `6fb16a48e3e2e625…`) built from `186c96a01`. Local `main` was
  fast-forwarded by another process to `eb2561903` during the pass (documentation and one
  `developer/edit.rs` home-path fix only); source hypotheses were read at `eb2561903`, and repairs are based
  on it.
- Library drift found while running (details in the group results): CL-01's command list lacks `secret` and
  `shell-validate`; research "Solo/Dual/Trio" modes are prompt-only on the backend, so DR-02/DR-05 model
  assertions cannot be judged with a scripted lead; DR-03 does not state the lock-hold duration; CX-09 needs a
  `code-mode` build; PN-04's "oversized instructions" applies to the Responses API; PM-04 only supports a
  planner that matches the session model; PN-12 "just under 1" is rejected because the reduction must be
  below the threshold; several cards name Desktop steps that need native dialogs.

## Environment and harness

- Every gosling process ran with a disposable `GOSLING_PATH_ROOT` and `GOSLING_DISABLE_KEYRING: true`
  (the keyring service name is `gosling` regardless of the root, so an enabled keyring would have touched the
  operator's keychain).
- Deterministic loopback OpenAI-compatible fixture (`fxprovider.py`) with scripted directives (text, single,
  sequential and parallel tool calls, delays, stalls, disconnects, HTTP errors, malformed streams, context
  limits, exact usage) and full request capture; stdio MCP fixture (`fxmcp.py`); a stdio ACP client; a scripted
  external ACP agent standing in for the `pi-acp` provider (DR-06); Playwright over CDP for the Desktop.
- Electron's `dist/Electron.app` was missing from `ui/node_modules/electron` at the start and was restored
  from the local Electron download cache before the Desktop cards.
- Evidence (captures, transcripts, screenshots, roots) stays in the session scratchpad; captures from before
  finding E06 was understood contain the operator's personal skill names and should not be shared unredacted.

## Card ledger

| File | Pass | Partial | Fail | Blocked | Not executed |
|---|---|---|---|---|---|
| 01 lifecycle | LC-01, LC-02, LC-03 | — | LC-04 | — | — |
| 02 chat | CH-01, CH-02, CH-03, CH-05, CH-06 | — | CH-04 | — | — |
| 03 workspaces | — | — | WS-01, WS-02, WS-03 | WS-04 | — |
| 04 providers | PM-01, PM-02, PM-03, PM-04 | — | — | — | — |
| 05 extensions/MCP | EX-01, EX-02, EX-03 | — | EX-04 | — | — |
| 06 skills/plugins/subagents | SK-01, SK-02 | — | SK-03 | — | — |
| 07 import/export | SE-03 | — | SE-01, SE-02 | — | — |
| 08 permissions | PA-01, PA-03 | — | PA-02 | — | — |
| 09 CLI | CL-01, CL-02, CL-04 | — | CL-03 | — | — |
| 10 settings | ST-02 | ST-01 | ST-03 | — | — |
| 11 headless/serve/ACP | HS-01, HS-02, HS-03 | — | — | — | — |
| 12 stress | SX-01, SX-02, SX-03, SX-04, SX-05, SX-06, SX-08 | — | SX-07, SX-09 | — | — |
| 13 advanced CLI | AC-01, AC-02, AC-07, AC-08 | AC-09 | AC-03, AC-04, AC-05, AC-06, AC-10 | — | — |
| 14 Desktop | DT-11, DT-12, DT-13, DT-14, DT-15 | DT-01, DT-04 | DT-02, DT-03, DT-05, DT-06, DT-07, DT-08, DT-09, DT-16 | DT-10 | — |
| 15 context/filesystem | CX-01, CX-02, CX-05, CX-07, CX-10 | CX-09 | CX-03, CX-04, CX-06, CX-08 | — | — |
| 16 provider resilience | PN-06, PN-08, PN-11 | PN-09 | PN-01, PN-02, PN-03, PN-04, PN-05, PN-07, PN-10, PN-12 | — | — |
| 17 ACP protocol | AP-01, AP-02, AP-03, AP-04, AP-05, AP-06 | AP-07 | AP-08, AP-09, AP-10 | — | — |
| 18 state/extension depth | SI-05, SI-07, SI-10 | SI-03 | SI-01, SI-04, SI-06, SI-08, SI-09 | SI-02 | — |
| 19 deep research | DR-02 | — | — | — | DR-01 |
| 20 research regressions | DR-03, DR-05, DR-06, DR-08 | — | — | — | DR-04, DR-07, DR-09 |
| **Total** | **65** | **8** | **47** | **3** | **4** |

Blocked cards: WS-04 and SI-02 need native Save/confirm/chooser dialogs that the CDP harness cannot drive;
DT-10's macOS notification-permission steps are blocked for the same reason (its in-app setting persisted).
Partial cards exercised the primary path but not every step (for example PN-09 OAuth refresh ran against a
loopback token exchange, while cancelled re-auth needs an https device-flow endpoint; CX-09's enabled path
needs a `code-mode` build).

## Safety events and process incidents

- **OAuth grant minted during WS-01 (G101).** Opening Desktop "Create workspace" in an isolated root started
  a real ChatGPT Codex OAuth flow in the operator's browser and stored `tokens.json` (4033 B) in the
  disposable root at 19:47 CDT on 2026-09-27. The file was deleted unread; the operator may want to revoke
  that grant in the ChatGPT account. The dialog was not opened again.
- **Empty log files in operator state (E13).** About eight `gosling … --help` commands were run without
  `GOSLING_PATH_ROOT` early in group E; because every CLI invocation creates a log file, 0-byte files were
  created under `~/.local/state/gosling/logs/cli/2026-09-27/` between 19:36 and 19:53. They were left in
  place (other processes write there too); they are safe to delete.
- **Unit tests escaped `GOSLING_PATH_ROOT` (found during repair).** Several gosling tests set and then remove
  the variable while parallel tests read `Config::global()`, and a declared `secret_sources` keychain read is
  not gated by `GOSLING_DISABLE_KEYRING`; one test process blocked on a real keychain read and was killed. No
  secret was read. All later test runs used a throwaway `HOME` as well.
- Group C built legacy stores for SI-06 by running the operator-installed gosling 1.1.0 and a 1.2.5 dev build
  with an isolated `HOME`, root and keyring disabled; all assertions used the build under test. During SK-02
  git's `osxkeychain` helper tried to store a loopback test credential; the store was refused (-60006).
- Group A briefly wrote two Python wheel files into the repository root and removed them within a minute;
  `git status` was clean before and after. Group G2 moved only its own `gslpt-g2-*` test files to the
  operator's Trash (DT-11) and put test text on the clipboard (DT-12).
- Account usage limits interrupted the pass three times (20:20, 03:07 and 08:45 CDT); agents resumed from
  their transcripts and evidence, and results were checkpointed after every card from the second
  interruption on.

## Untested

DR-01, DR-04, DR-07 and DR-09 (Deep Research Desktop UI) were still in progress; G2 found a safe way to point
the Research Library at a disposable folder (`researchLibraryPath` pre-seeded in the Desktop settings before
first launch). Native-dialog steps (Save a copy, directory chooser, native confirms, notification permission),
menu accelerators (CDP key events do not reach them), a signed packaged app, real OAuth re-authentication,
real CLI-backed providers, and the keychain-enabled variants of SI-08/WS-02 were not exercised.

## Repair status

Repairs run on branch `claude/playtest-repair-20260927` (local, not pushed), in locality groups (R1a turn
closure, R2 compaction, R3 repetition, R4 session store, R5 secrets/config, R6 permissions, …). The
**Disposition** column of the register below reflects the branch state when this report was written; the
repair record (`docs/logs/session/2026-09-27-live-playtest-repair-campaign.md`) has commits, tests and live
replays per finding. Findings marked "open — queued" were not yet repaired.

## Recommended next pass

1. Replay every High finding's original reproduction against the repaired branch binary, then the smoke
   shape (01 → 02 → 09) and files 16–18 where most Fails concentrated.
2. Finish the four Deep Research Desktop cards with the Research Library redirected.
3. Add a native-dialog driver (or test hooks) so WS-04, SI-02, SX-07 and DT-10 can run.
4. Decide the open product questions recorded in the repair record (ignore-file enforcement, delegate retry
   budget, keychain gating for declared secret sources, interrupted-session sweep at startup).

## Findings register

| Severity | ID | Finding | Disposition |
|---|---|---|---|
| High (Critical in Auto) | F10 | Cancelled or abandoned prompt resurfaces and is executed by the next, unrelated turn | fixed (R1a) |
| High | A12 | After a hard kill mid-turn, the unanswered prompt is silently merged into (and re-executed with) the next prompt on resume | fixed (R1a) |
| High | B08 | Mid-turn auto-compaction re-appends the in-flight prompt after the summary, so completed tool calls are executed again | fixed (R2) |
| High | C20 | Extension secrets share one un-namespaced store with the provider key: installs overwrite other credentials and any extension can read any stored secret by name | fixed (R5) |
| High | D01 | Session IDs are recycled after deletion; stale IDs silently attach to, modify, or delete an unrelated session | fixed (R4) |
| High | E01 | `.goslingignore` (and the documented `.gitignore` fallback) is not enforced at all | docs fixed; enforcement deferred (product decision) (R6) |
| High | E02 | A tool call that failed (or was declined) once is denied forever in that session, even after the cause is fixed | fixed (R3) |
| High | E03 | Manual-approval mode: one `delegate` approval lets the subagent run `shell`/`write` with no further approval (docs say subagents are disabled in manual mode) | fixed (R6) |
| High | E04 | "Restrict tools to working directories" is escaped with `cd ..` (reads and writes outside scope without approval) | fixed (R6) |
| High | G101 | Opening "Create workspace" starts a ChatGPT Codex OAuth flow and a live authenticated model-catalog call for an unconfigured provider | fixed (R13a) |
| High | G130 | An invalid GOSLING_MODE silently falls back to Autonomous (fails open, no warning) | fixed (R6) |
| High | S19 | `.goslingignore` is documented as a Developer-tool access control but is not enforced | docs fixed; enforcement deferred (product decision) (R6) |
| Medium | A01 | Failed CLI start leaves an empty "CLI Session" ghost row, which then hijacks `gosling session -r` | fixed (R4) |
| Medium | A03 | Docs-advertised root-level `GOSLING_MODEL`/`GOSLING_PROVIDER` hand edits are silently ignored; `info -v` shows the derived value instead of the file's | fixed (reported + documented) (R5) |
| Medium | A08 | Lease revocation does not stop an in-flight provider call; fenced interactive turn keeps "working" until the provider answers, then the whole CLI exits | fixed (R1b) |
| Medium | A16 | `run` startup failures print human text on stdout (stderr empty), even with `--output-format json`/`stream-json` | fixed (R8a) |
| Medium | A17 | `run -q` ("printing only the model response to stdout") prints tool-call chrome, raw tool output and error text on stdout | fixed (R8a) |
| Medium | A22 | Session subcommands without a selector fail off-TTY with "Error: not connected" and exit 0 | fixed (R8a) |
| Medium | B01 | Aborted `gosling configure` persists the new host/API key while reporting "the active provider was not changed" | fixed (R9) |
| Medium | B02 | Re-running `configure` preselects the first listed model, not the saved one; Enter silently replaces the saved model | fixed (R9) |
| Medium | B03 | `configure` cannot complete when the model list cannot be fetched; no manual model entry is offered | fixed (R9) |
| Medium | B04 | Rate-limit backoff is invisible: Retry-After waits show only a generic spinner (CLI) and nothing (ACP) | fixed (R9) |
| Medium | B05 | `OPENAI_TIMEOUT` is per attempt; a stalled provider errors only after ≈4× the timeout plus backoff, with no feedback | fixed (retries announced; per-attempt meaning documented) (R9) |
| Medium | B06 | ACP streams text from retracted (interrupted) attempts with no retraction signal; live transcript differs from persisted/replayed history | fixed (R1b) |
| Medium | B09 | Compaction shrink ladder bottoms out at payload/8, so large histories never reach small chunks | fixed (R2) |
| Medium | B10 | Compaction completion notice under-reports the resulting context (system prompt and tool schemas excluded) | fixed (R2) |
| Medium | B11 | Malformed-stream errors echo raw provider lines unredacted (incl. bearer/API-key-like strings) into the error, session history and log | fixed (R5) |
| Medium | B18 | CLI cost line shows only the last request's cost without saying so; `--stats` silently ignored with json/stream-json | fixed (R8a) |
| Medium | B21 | Rejected Copilot token refresh surfaces as "failed to get api info after 3 attempts" + "Please retry"; no re-auth request | fixed (R9) |
| Medium | C01 | ACP client `mcpServers` replace all of gosling's configured extensions | open — queued R10 |
| Medium | C02 | A hanging MCP extension blocks session start for 300 s with no feedback (run/ACP) | open — queued R11 |
| Medium | C03 | Removed MCP extensions come back when an older session is resumed/loaded | open — queued R11 |
| Medium | C05 | Skill discovery ignores `GOSLING_PATH_ROOT` and reads/writes HOME-based skill dirs | fixed (R7) |
| Medium | C06 | `disabledPlugins` does not disable an installed plugin's skills | open — queued R11 |
| Medium | C08 | ACP: parallel subagents' tool activity is attributed to the first delegate call | open — queued R11 |
| Medium | C10 | A stalled provider stream inside a synchronous subagent blocks the parent indefinitely | open — queued R11 |
| Medium | C14 | No guard against opening a store written by a newer schema (downgrade not blocked) | fixed (R4) |
| Medium | D03 | Failed `--fork --edit` leaves an orphan fork with the source's name that then hijacks `--resume` | fixed (R4) |
| Medium | D04 | `--edit` with invalid YAML discards the user's edits without naming or keeping the temp file; wrong "failed to launch" wording | open — queued R8b |
| Medium | D05 | Diagnostics bundle leaks credentials that are not regex-shaped (including the configured provider key) | fixed (R5) |
| Medium | D07 | `session list -w` leaks sibling directories (case-insensitive substring) and misses paths with a trailing slash | open — queued R8b |
| Medium | D08 | Terminal control sequences in session names are printed raw; imported transcripts can inject them | open — queued R8b |
| Medium | E05 | CLI text mode never shows a permission refusal; the operator sees a tool card as if the call ran | fixed (R8a) |
| Medium | E06 | `GOSLING_PATH_ROOT` does not isolate skills/agents; operator's personal skill catalog is sent to the disposable root's provider (suspicion) | fixed (R7) |
| Medium | E07 | Nested (subdirectory) context files are injected as a plain `user` message without the "untrusted project hints" framing | fixed (R7) |
| Medium | F01 | `--max-tool-repetitions` denies the repeated call but lets the turn loop to the 1000-turn default | fixed (R3) |
| Medium | F14 | Interrupted turns stay `in_progress` forever (EOF, disconnect, SIGTERM, SIGKILL); nothing reconciles them | fixed (R1a) |
| Medium | F15 | SIGTERM "graceful" shutdown keeps running turns alive for 5 s, then drops every client without a terminal event | fixed (R1b) |
| Medium | G102 | Credential picker/profile manager marks 14 alias profiles "configured", including providers the app reports as unconfigured | fixed (R13a) |
| Medium | G104 | Workspace save/duplicate errors show only "Invalid params" (reason dropped) and the editor error renders out of view | fixed (R13a) |
| Medium | G105 | A chat's workspace label is a creation-time snapshot; after rename + name reuse it names a different workspace | open — queued R13b |
| Medium | G108 | Historical session with a moved working folder fails with "Invalid params: invalid directory path" and cannot be recovered by relinking | partial (named error + restore works; re-home of pinned chats needs product decision) (R13a) |
| Medium | G122 | Quitting while a tool approval is pending leaves the tool "pending" forever; the next message silently re-submits the old request | fixed (R1a+R13a) |
| Medium | G128 | Keyboard-only users cannot open existing chats, lose focus after dialogs, and get no focus ring on primary navigation | open — queued R13b |
| Medium | G129 | A reply interrupted by window close/quit is shown after relaunch as a normal complete message; the session stays "in_progress" | fixed (R1a+R13a) |
| Medium | G131 | Invalid config values are invisible in Desktop; Configuration Editor shows "[object Object]" for providers | open — queued R13b |
| Medium | G132 | Onboarding "OpenAI" API-key field shows the secret in clear text (and offers no host/base-path for "OpenAI compatible" endpoints) | fixed (R13a) |
| Medium | G201 | Opening a large Markdown output freezes the whole Desktop window for 15–20 s | open — queued R13b |
| Medium | G207 | "Open in new window" on a session in a non-active typed workspace replaces the current window with a fatal error screen | open — queued R13b |
| Medium | G211 | Archiving does not reach other windows: the archived chat stays open there and keeps accepting turns while staying archived | open — queued R13b |
| Medium | G212 | External-backend secret is put in the ACP WebSocket URL (`?token=<secret>`) and printed in the renderer console on every failed connect | fixed (R13a) |
| Medium | G221 | Downloads started right after switching chats are saved into the previous chat's workspace | fixed (R13a) |
| Medium | H01 | Repeated identical failing tool calls are denied but the turn never ends before 1000 turns | fixed (R3) |
| Medium | H02 | Delegate launch failures are not remembered: alternate `source` retries all execute and the next valid shape launches | deferred (product decision) (R3) |
| Medium | H03 | CLI shows nothing for failed (`isError`) tool results; failures look like successes | fixed (R8a) |
| Medium | H04 | External-tool ACP delegate in Chat mode: a self-executed tool is reported as "Tool call was denied." and the agent's answer is dropped | open — queued R11 |
| Medium | H05 | ACP cancel is not honoured while the prompt waits on storage; latency equals the remaining lock hold | fixed (R1b) |
| Medium | S03 | `--max-tool-repetitions` denies repeats but never ends the turn (1000 provider calls) | fixed (R3) |
| Medium | S04 | A one-off `GOSLING_CONTEXT_LIMIT` is frozen into a resumed session forever | fixed (R2) |
| Medium | S07 | ACP: selecting the model already in use still runs a full provider transition | fixed (R9) |
| Medium | S11 | Sessions created over `gosling serve`/`gosling acp` are invisible to `gosling session list` | open — queued R8b |
| Medium | S13 | Corrupt config.yaml silently drops `GOSLING_DISABLE_KEYRING: true` (keyring re-enabled) (suspicion) | fixed (R5) |
| Medium | S18 | Corrupt permission.yaml: CLI/ACP panic, serve hangs `initialize`, `doctor` stays green, denials mislead | fixed (R6) |
| Medium | S20 | Subdirectory AGENTS.md from any touched directory (including ignored ones) is injected into the user turn | fixed (R7) |
| Low | A02 | Unknown provider from `GOSLING_PROVIDER` env is reported as "No model configured" | open — queued R8b |
| Low | A04 | Ctrl-C in `gosling configure` leaves the terminal cursor hidden | open — queued R8b |
| Low | A05 | Broken config.yaml: session/run say "Run 'gosling configure' first" but configure refuses to run | fixed (R5) |
| Low | A07 | REPL silently discards input that arrives in the same burst as a submitting Enter | open — queued R8b |
| Low | A09 | Opening a session with `--resume` and exiting without sending re-stamps `updated_at` | fixed (R4) |
| Low | A10 | Provider picker shows a cryptic "Groq (d)" label | open — queued R9 |
| Low | A11 | `gosling session export --help` documents resume semantics that don't apply | open — queued R8b |
| Low | A13 | `/model` switch injects a ~3 KB "Gosling session checkpoint" into the next prompt (even for an empty session) and the session title is generated from it | open — queued R9 |
| Low | A14 | Documented slash commands without their argument are reported as "Unknown command" | open — queued R8b |
| Low | A15 | Version/help text gaps: `--version` prints " 1.3.0" with no program name; undocumented `session diagnostics` and `shell-validate` options | open — queued R8b |
| Low | A18 | Tool output and the following assistant text are printed with no separator (text and quiet modes) | fixed (R8a) |
| Low | A19 | Hidden internal subcommands leak into shell completion and typo suggestions | open — queued R8b |
| Low | A20 | (exploratory) `gosling secret` reports the wrong storage location and "removes" servers that don't exist | fixed (R5) |
| Low | A21 | (exploratory) `shell-validate` has side effects and accepts unknown builtins | fixed (R7) |
| Low | B07 | CLI stdout keeps the retracted partial attempt (text/-q/stream-json) | fixed (R1b) |
| Low | B12 | Auth failures end with "Please retry if you think this is a transient or recoverable error" | open — queued R9 |
| Low | B13 | Empty or space-containing `--model` / `GOSLING_MODEL` accepted and sent verbatim | open — queued R9 |
| Low | B14 | CLI output hygiene (cosmetic) | fixed (typed-input echo deferred: needs design) (R8a) |
| Low | B15 | stream-json on provider failure has no terminal `error`/`complete` event | fixed (R8a) |
| Low | B19 | Reduction validation differs between CLI and ACP; out-of-range CLI warning does not state the fallback | fixed (R2) |
| Low | C04 | Hidden host-policy `planning` extension is offered in `configure`/`mcp list`; enabling it warns on every session | open — queued R11 |
| Low | C07 | Duplicate plugin skill names are silently shadowed; precedence set by directory order | open — queued R11 |
| Low | C09 | Subagent docs say delegates inherit parent extensions; code defaults ad-hoc delegates to none | open — queued R11 |
| Low | C11 | Imported sessions silently run in `approve` mode; CLI denial names the wrong mode | fixed (R6) |
| Low | C12 | `session import` announces a working directory before it knows the outcome; raw serde errors | open — queued R8b |
| Low | C13 | CLI resume rebinds an imported session's trusted working dir to the current directory | fixed (R4) |
| Low | C15 | Session exports embed extension `--env` values; pasted secrets exported verbatim | fixed (R5) |
| Low | C16 | Markdown export drops the tool-error flag | open — queued R8b |
| Low | C17 | CLI help/docs drift found while executing | open — queued R8b |
| Low | C18 | `/skills <name>` differs by surface and never validates the name | open — queued R11 |
| Low | C19 | Extension failure messages are noisy and lose the cause; no health state in listings | open — queued R11 |
| Low | C21 | `mcp remove` leaves the extension's secrets in `secrets.yaml` | fixed (per-extension secrets) (R5) |
| Low | C22 | "Always Allow" grants survive replacing the server behind an extension name | fixed (R6) |
| Low | C23 | Approval prompts do not show cwd or the persistence scope; ACP option names are raw ids | fixed (R6) |
| Low | C24 | `gosling secret set` says it stored credentials in `config.yaml` | fixed (R5) |
| Low | C25 | Workspace `defaultExtensions` accepts unknown names silently | open — queued R11 |
| Low | D02 | Open CLI session crashes with a raw FOREIGN KEY error after the session is removed elsewhere; typed prompt lost | fixed (R4) |
| Low | D06 | Session exports write raw secrets from tool output (diagnostics redacts, export does not) | fixed (R5) |
| Low | D09 | Session pickers are shuffled on every run; diagnostics/context-history picker says "Select a session to export:" | open — queued R8b |
| Low | D10 | Ctrl-C in the `session remove` picker kills the process by SIGINT and leaves the cursor hidden | open — queued R8b |
| Low | D11 | `session export\|diagnostics\|context-history list` without an identifier outside a TTY: "Error: not connected", exit 0 | fixed (R8a) |
| Low | D12 | `session remove -r` matches IDs only, but the CLI guide's example implies names | open — queued R8b |
| Low | D13 | Failed imports print "Imported session working directory: …" first; errors lack file/format context | open — queued R8b |
| Low | D14 | `gosling project` stops (exit 0) when the newest project is gone and hides child failures | open — queued R8b |
| Low | D15 | `term init` aliases break when the gosling binary path contains a space | open — queued R8b |
| Low | D16 | CLI guide's `--resume --path ./session.json  # exported session` does not work | open — queued R8b |
| Low | D17 | Forks lose their lineage (auto-renamed, banner says "resuming", no forked-from metadata) | fixed (cheap parts) (R4) |
| Low | D18 | Typeahead during interactive startup is not submitted; later lines (even `/exit`) are merged into one prompt | open — queued R8b |
| Low | D19 | `session list --format xml` silently falls back to text (exit 0) (suspicion) | open — queued R8b |
| Low | D24 | Import de-duplication blocks importing new turns of a grown transcript | note (product question) |
| Low | D25 | `gosling tui` with non-interactive stdin renders a frame, then dumps an Ink/React stack trace | open — queued R8b |
| Low | D26 | Default `review --dry-run` does not show what would run: `--checks-only` and `--instructions` produce byte-identical output that still announces a main pass | open — queued R8b |
| Low | E08 | Nested hints are re-appended on every resume / `session/load` + access (unbounded duplication) | fixed (R7) |
| Low | E09 | Unreadable or invalid-UTF-8 `AGENTS.md`/`.goslinghints` are dropped silently (warning only in the log file) | fixed (R7) |
| Low | E10 | `run -i` reports every read failure as "Instruction file not found" | open — queued R8b |
| Low | E11 | Code-execution gate blames `GOSLING_CODE_EXECUTION_RUNTIME=disabled` when the variable is unset or invalid | fixed (R8a) |
| Low | E12 | `run --no-session` banner still announces "● new session" with an ID that cannot be resumed | fixed (R8a) |
| Low | E13 | Every CLI invocation, including `--help` and `--version`, creates a new log file in the state directory | fixed (R7) |
| Low | E14 | Session-start errors go to stdout (rc 1), unlike other CLI errors | fixed (R8a) |
| Low | E15 | Context-file documentation disagrees with runtime (default order, fallback text) | fixed (R7) |
| Low | F02 | `run --output-format json\|stream-json`: startup failures are printed as human text on stdout | fixed (R8a) |
| Low | F03 | `gosling serve` is silent on the console and its log misreports failed starts | open — queued R10 |
| Low | F04 | WebSocket closes never carry a server close frame (client always sees 1006) | upstream crate (agent-client-protocol-http) |
| Low | F05 | `--allowed-origin` accepts values that can never match and gives no diagnostics for rejections | open — queued R10 |
| Low | F06 | TLS startup validation/messaging gaps | open — queued R10 |
| Low | F07 | Structurally invalid requests get -32700 Parse error with id:null | upstream crate (agent-client-protocol) |
| Low | F08 | `session/prompt` with an empty prompt array sends the model a fabricated "Hello" | open — queued R10 |
| Low | F09 | Streamable-HTTP connections that never open a stream or DELETE are never reaped (suspicion) | open — queued R10 |
| Low | F11 | Losing the turn lease does not stop the in-flight provider call; the revoked turn lingers until the provider returns | fixed (R1b) |
| Low | F12 | Two connections can drive one session; the owner gets no updates and a misleading refusal | open — queued R10 |
| Low | F13 | ACP reports provider/config problems as generic -32603 "Internal error" | open — queued R10 |
| Low | F16 | The protocol-version gate is advisory on stdio and WebSocket | open — queued R10 |
| Low | F17 | Advertised capabilities do not match what is callable | open — queued R10 |
| Low | G103 | Chat header labels overlap at the default 940px window (and all narrower widths) | open — queued R13b |
| Low | G106 | Unicode-equivalent workspace names are both accepted (NFC vs NFD) | open — queued R13b |
| Low | G107 | Duplicate fails for a max-length (100-char) workspace name; over-length input silently truncated | open — queued R13b |
| Low | G109 | Workspace warnings never name the folder they refer to | open — queued R13b |
| Low | G110 | Symlinked output/primary folders that resolve outside the declared tree pass validation silently; the grant is on the resolved outside path while the UI shows the link path (suspicion) | open — queued R13b |
| Low | G111 | Hub uses stale workspace validation; failure only surfaces on submit with "Invalid params:" prefix | open — queued R13b |
| Low | G112 | With the Inputs/Outputs pane open at 940px, the New Chat hub is crushed (workspace selector 18 px wide) | open — queued R13b |
| Low | G113 | Renderer IPC listener leak warning after several chats (suspicion) | open — queued R13b |
| Low | G114 | Chat header says "No credential" while the app-default global key is in use | open — queued R13b |
| Low | G116 | Shortcut recorder: Escape is recorded as a key instead of cancelling; no per-binding reset | open — queued R13b |
| Low | G117 | Settings controls lack accessible names | open — queued R13b |
| Low | G119 | Toasts clipped at the right window edge (940px) | open — queued R13b |
| Low | G120 | Header title stays "New Chat" after the session is titled | open — queued R13b |
| Low | G123 | Automatic navigation collapse at narrow width is persisted as the user's preference | open — queued R13b |
| Low | G124 | At the minimum window (480x400) with navigation open, the Chats list has zero height | open — queued R13b |
| Low | G125 | Renderer CSP lists invalid IPv6 sources (console error on every page load) | open — queued R13b |
| Low | G126 | Concurrent-turn rejection is shown as "Internal error / Task failed" with an unrelated recovery action, and the rejected text is lost | open — queued R13b |
| Low | G133 | A failing extension is reported only as "Failed to add extension" and is not flagged in the Extensions list | open — queued R11 |
| Low | G202 | HTML artifact preview never runs its scripts: the app page's CSP blocks the inline scripts the preview sandbox is designed to allow | open — queued R13b |
| Low | G203 | Malformed, empty and missing previewable outputs are not reported with a clear, bounded message | open — queued R13b |
| Low | G204 | Markdown preview shows gosling's own output-history marker as a visible code block | open — queued R13b |
| Low | G205 | Output rows show the absolute path truncated at the end, hiding the file name | open — queued R13b |
| Low | G208 | An assistant reference to an in-workspace symlink grants the renderer read access to the outside-root target | open — queued R13b |
| Low | G209 | Two windows overwrite each other's persisted artifact tabs; tabs closed in one window come back after relaunch | open — queued R13b |
| Low | G213 | External backend faults are indistinguishable and undetected while idle | open — queued R13b |
| Low | G214 | An invalid external-backend URL is saved when the Secret field loses focus | open — queued R13b |
| Low | G215 | Desktop per-session state is keyed by session id only, so sessions of different backends share UI state | open — queued R13b |
| Low | G217 | Copy/IPC failure toasts show Electron's internal "Error invoking remote method '…': Error:" prefix | open — queued R13b |
| Low | G218 | After a turn with more than 200 outputs, the live Outputs list and count silently stop at 200 | open — queued R13b |
| Low | G219 | Escape inside an open provider/model dropdown closes the entire model-switch dialog | open — queued R13b |
| Low | H06 | After a normal exit all session data lives only in `sessions.db-wal`; `sessions.db` is an empty 4 KB file | fixed (R4) |
| Low | H07 | Delegate activity title echoes the raw `source` argument (`null`, blanks, normalized sentinel) | open — queued R11 |
| Low | S01 | Stale turn-lease rows left by concurrent `gosling run` processes | fixed (R4) |
| Low | S02 | Interactive type-ahead under load: Enter becomes a newline, a later `/exit` is sent to the model | open — queued R8b |
| Low | S05 | `session -r --history` renders at ~32 ms per message (6.7 s for 206 messages) | open — queued R8b |
| Low | S08 | `/model` typed during a stream queues, and every switch persists a hidden checkpoint | open — queued R9 |
| Low | S09 | Markdown export includes hidden internal checkpoint messages | open — queued R9 |
| Low | S10 | kill -9 mid-stream leaves the truncated reply stored as a normal, complete assistant message | fixed (R1a) |
| Low | S12 | Corrupt config.yaml: `run`/`doctor` blame "No provider configured" | fixed (R5) |
| Low | S16 | Empty sessions: listed by the CLI, hidden by ACP; a failed `run` leaves one behind | failed-start part fixed; listing parity queued (R4/R8b) |
| Note | A06 | (Suspicion, safety) A config.yaml parse failure silently drops `GOSLING_DISABLE_KEYRING: true` and re-enables the OS keychain (suspicion) | fixed (R5) |
| Note | B16 | `run --resume` ignores provider/model env vars but `--provider/--model` permanently re-pin the session | note |
| Note | B17 | ACP model options fall back to the static OpenAI catalogue when a custom host's `/models` fails; inventory says `stale: false` | note |
| Note | B20 | Partial reduction can fold more history than "full" mode and splits a user message from its reply | not a defect (documented behaviour) (R2) |
| Note | D20 | TUI: Enter does not submit in a PTY harness; checked-in `ui/text/dist` is stale (suspicion) | note (needs a real terminal) |
| Note | D21 | Terminal integration: hidden session per shell start; typed shell commands forwarded to the provider | note (product question) |
| Note | D22 | Headless resume from another directory permanently re-homes the session | documented (behaviour kept) (R4) |
| Note | D23 | Session metadata changes on every no-op resume (updated_at, shuffled enabled_extensions) | fixed (R4) |
| Note | D27 | Review discovers global checks from `$HOME` regardless of `GOSLING_PATH_ROOT`; `--summary-only` omits untracked files; scoped REVIEW.md mislabeled under `--check-scope` | fixed (R7) |
| Note | G115 | "Gosling secure storage" wording when keyring is disabled (plaintext secrets.yaml) | open — queued R13b |
| Note | G118 | Skills tab in an isolated GOSLING_PATH_ROOT lists the operator's real skills | fixed (R7) |
| Note | G121 | Duplicate element id / test id for the composer | open — queued R13b |
| Note | G127 | Isolated Desktop instance lists and grants the operator's real Research Library | open — queued R7 |
| Note | G134 | Renderer asks for git-branch info on folders it was never granted (37 main-process errors); persisted grants contain only the operator's Research Library | open — queued R13b |
| Note | G206 | Switching to an image/SVG tab logs `net::ERR_INVALID_URL` every time | open — queued R13b |
| Note | G210 | The window's renderer roots include the whole home directory by default | open — queued R13b |
| Note | G216 | "Gosling finished the task." is sent for turns that ended without success (suspicion) (suspicion) | open — queued R13b |
| Note | G220 | PermissionModal renders a `<button>` inside a `<button>` | open — queued R13b |
| Note | H08 | Note: InvalidParams feedback identifies the contract only if the extension's own message does | note (product question) |
| Note | H09 | Note: `dummy` sentinel matching is trim-insensitive and shadows a genuine `dummy` agent | note (product question) |
| Note | S06 | Compaction token figures disagree between the CLI cue and context-history | fixed (R2) |
| Note | S14 | kill -9 during a shell tool: the child keeps running; the operation is honestly marked in_doubt | note (honest in-doubt marker) |
| Note | S15 | Headless limit message is glued and asks a question nobody can answer | fixed (R3) |
| Note | S17 | `gosling acp`/`serve` create a Default workspace rooted at the server process's cwd | not a defect (operator confirmed the workspace default; documented) (R7) |
