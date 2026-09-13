# Gosling live all-scenarios playtest and repair report — 2026-09-12

## Executive result

The authoritative 127-card library (`docs/test_scenarios/`, files 01–20) was run against
`main` at `63fb501f26e9f73491755dcc39605b8efeafbf8e` (gosling 1.2.5, macOS arm64) by eight
parallel playtest groups, each with a disposable `GOSLING_PATH_ROOT`, its own port range and a
deterministic loopback OpenAI-compatible provider fixture.

Before repair: **48 Pass · 76 Fail · 3 Blocked** (CLI, TUI, ACP, `serve`, and a scratch copy of
the packaged Desktop app driven over CDP). This is a far stricter baseline than the 2026-08-18
run (31 Pass · 79 Blocked): the fixture let almost every card produce atomic evidence, so cards
that were previously Blocked now exposed real failures.

Repair branch `claude/playtest-repair-20260912` carries **61 commits** closing **62 findings**
(each with a regression test that failed before the fix, except where noted) plus one
interaction follow-up. Card-level post-repair statuses were not re-run as a full pass; see
“Post-repair verification”.

## Target and method

- Repository: `/Users/eric/Work/vscode/forked/gosling`
- Baseline: clean `main` at `63fb501f2`; repair branch `claude/playtest-repair-20260912`
- Run ID: `GSL-PT-20260912`; finding IDs `GSL-PT-20260912-<group>-<n>`
- Provider oracle: loopback fixture (markers for delay, slow stream, 429/500/401, in-band and
  HTTP context errors, empty/malformed bodies, disconnects, scripted tool calls, context-marker
  scans, usage) with full request capture. No real provider credentials were used.
- Desktop: prebuilt package copied to scratch with its backend swapped for the debug CLI,
  disposable user-data dir, CDP automation, scripted native-dialog responder.
- Repairs: nine locality groups in separate git worktrees, each finding reproduced against code,
  fixed minimally with a fail-before/pass-after regression test and (mostly) a live replay of the
  original repro, then cherry-picked onto the repair branch.

## Scenario outcome ledger (pre-repair)

| Group / files | Cards | Pass | Fail | Blocked |
|---|---:|---|---|---|
| A — 01, 02, 09 | 14 | LC-01, LC-02, LC-04, CH-05, CL-01–04 | LC-03, CH-01–04, CH-06 | — |
| B — 04, 16 | 16 | PM-01, PM-02, PN-06, PN-08 | PM-03, PM-04, PN-01–05, PN-07, PN-09–12 | — |
| C — 05, 06, SI-07–09 | 10 | EX-01–04, SK-01, SK-03 | SK-02, SI-07, SI-08, SI-09 | — |
| D — 07, 08, 13, SI-04–06, SI-10 | 20 | PA-01, AC-02, AC-08 | SE-01–03, PA-02, PA-03, AC-01, AC-03–07, AC-09, AC-10, SI-04–06, SI-10 | — |
| E — 11, 17 | 13 | HS-02, HS-03, AP-01, AP-02, AP-05, AP-06 | HS-01, AP-03, AP-04, AP-07–10 | — |
| F — 12, 15 | 19 | CX-01, CX-02, CX-06, CX-10, SX-01, SX-02, SX-04, SX-06, SX-08, SX-09 | CX-03–05, CX-07, CX-08, SX-03, SX-05 | CX-09, SX-07 |
| G — 03, 10, 14, SI-01–03 | 26 | 9 (DT-01, DT-11, DT-16 partial) | 16 | DT-10 |
| H — 19, 20 | 9 | DR-05, DR-07, DR-09 (partial) | DR-01–04, DR-06, DR-08 | — |
| **Total** | **127** | **48** | **76** | **3** |

Desktop sub-portions of CLI-owned cards were owned by group G; supplementary Desktop results for
CH-01/04/05 passed and CH-03 failed (G-6). Per-card evidence lives in the run's scratch results
(`results/{A..H}.md`); it is not committed.

## Findings closed on the repair branch

| Finding | Sev | Card | Summary of repair | Commit |
|---|---|---|---|---|
| F-2 | High | CX-04 | Lazily loaded subdirectory hints honor `.gitignore` (secrets no longer imported) | `eb7e6d177` |
| H-1 | High | DR-01 | Report pair identity ignores Gosling's output-history footer | `029d525ff` |
| E-3 | High | AP-08 | Cancelled ACP turns are closed so they don't merge into and re-run with the next prompt | `f68c9e8cf` |
| A-2 | High | CH-03 | Ctrl-C during a tool keeps the turn well-formed (no orphan tool message) | `1c1c6ef7e` |
| A-3 | High | CH-02 | TUI keeps every chunk of a multi-chunk paste | `ab5530546` |
| B-2 | High | PN-01/03/05/08 | Streams that fail to start are not re-issued on top of provider retries (16 → 4 requests) | `c7e3de7d9` |
| B-6 | High | PN-07 | ACP `session/new` accepts an unverified model when model listing fails | `47a64f863` |
| B-9 | High | PN-04 | Compaction shrinks small histories and never resends a rejected payload (112 → few requests) | `609e0191a` |
| G-3 | High | DT-09 | Desktop external-backend probe authenticates by header (was always "unreachable") | `89dec7a63` |
| G-4 | High | DT-09 | Relaunch with a settings-enabled external backend and no secret can recover | `58777e437` |
| F-8 / E-10 | Med | SX-05, HS-01 | `--max-tool-repetitions` is honored (was hard-coded to 3). **Partial:** the run still continues after denials (61 round-trips, exit 0, no stop reason) | `c535f3f1e` |
| F-1 / C-3 | Med | CX-01 | Reused OpenAI response ids no longer overwrite earlier assistant messages | `a145ccf4f` |
| F-3 | Med | CX-07 | `--no-session` leaves no resumable/exportable session and skips title generation. **Partial:** prompts remain in `state/logs/llm_request.*.jsonl` and the banner still prints a session id | `95b0bc8f9` |
| F-5 | Low | CX-03 | Duplicate `CONTEXT_FILE_NAMES` load once | `3b9e87505` |
| F-6 | Med | CX-08 | No-op auto-compaction is skipped honestly; title input bounded | `1b8ba326c` |
| F-7 | Med | SX-03 | `/model` rejects names containing whitespace | `7e3d1cd53` |
| A-4 | Med | CH-02, LC-01 | Type-ahead lines are submitted one at a time | `899038a5c` |
| A-5 | Med | CH-06 | TUI handles `/help`, `/exit` locally; unknown `/word` refused locally | `abb8ee446` |
| A-6 | Med | CH-03 | TUI first Esc/Ctrl-C cancels the running turn | `5b21bfa78` |
| A-7 | Med | LC-02/03 | `doctor` exits non-zero without a usable provider | `70ef18506` |
| A-8 | Med | LC-04 | `configure` refuses an unparseable config instead of probing a default host with the key | `a0ed4fefc` |
| A-9, A-11, A-12/C-15 | Low | CH-03/04/06 | Lease-loss message, `--history` separators, editor hints, bare `/mode` | `26fdffd9f`, `c98faca0a`, `f84548508` |
| B-4 | Med | PN-02/03 | Terminal-failure partial replies and notices kept out of model context; turn closed for the model | `ed00f81aa`, `1ec28efae` |
| B-7, B-8 | Med | PM-04 | Planner failure keeps plan mode; "clear history & act" clears stored history | `cb7b003ae` |
| B-10 | Med | PN-04 | In-band SSE `context_length_exceeded` classified for compaction | `e5b768484` |
| B-12 | Med | PN-12 | Invalid auto-compact reduction falls back to default | `0c0a106a6` |
| B-13 | Med | PN-05, PM-03 | Provider error bodies redacted and bounded | `18a41aa24` |
| B-16 | Low | PN-03 | Stalled stream bodies reported as timeouts | `8c75102a0` |
| B-18 | Low | PM-01/03 | `configure` exits non-zero when provider setup does not complete | `27cb0c34e` |
| C-1, C-8 | Med/Low | SK-02, SI-09 | Plugin installs/updates staged and renamed atomically; abandoned staging cleaned | `5888d6dbd`, `f3fab29ce` |
| C-2 | Med | SK-02 | Plugin git URL credentials kept out of output and metadata | `10c37943b` |
| C-4, C-5 | Med/Low | EX-03 | Interrupted extension startup kills the process group; timeouts reported as timeouts | `a52a91060`, `d247a8d89` |
| C-6 | Med | SI-07 | `mcp install` rejects blank names and key collisions | `228ca80f7` |
| C-13 | Low | SK-01 | `skills list` warns about skipped catalogs | `0f7d3e073` |
| D-1 | Med | SI-05 | Live prompts are not merged into imported untrusted history | `88d241d5e` |
| D-2 | Med | AC-01, SI-05 | Tools use the directory a resumed session actually stays in | `4d04fa7ad` |
| D-3 | Med | PA-03 | CLI approval menu defaults to Cancel | `1d1d6f3fe` |
| D-4 | Med | AC-04 | Diagnostics include only the session's LLM logs, redacted | `01f2429ea` |
| D-7 | Med | AC-06 | Projects tracked by the sessions started in them | `78f978a55` |
| D-8 | Low | SI-04 | Exports written atomically; symlink destinations refused | `9d90ee7ee` |
| D-16 | Low | AC-09 | `GOSLING_TUI_SCRIPT` honored; missing node/npx named | `b0cec8819` |
| D-20 | Med | PA-02/03 | CLI resume keeps the session's stored permission mode | `9705ef751` |
| E-1, E-9 | Med/Low | AP-09, AP-01 | HTTP shutdown bounded at 5 s; `--host ::1`/`localhost` accepted | `ce0b10c85` |
| E-2 | Med | AP-07 | First-use session store initialization serialized | `6d9295449` |
| E-7, E-11 | Low | AP-04, AP-10 | TLS errors name file/role; legacy string protocolVersion explained | `437d9956a`, `325b27947` |
| E-8 | Low | AP-03 | Origin allowlist enforced on HTTP ACP requests | `cc86225c4` |
| G-12 | Med | ST-01, DT-04 | Reserved/modifier-less shortcuts rejected; restart notice shown | `6f07da476` |
| G-14 | Med | DT-13 | Repository filter re-runs after artifact capabilities apply | `bd910b2f5` |
| G-17 | Low | ST-02 | Unknown routes redirect home | `e1b8a9c58` |
| H-2 | Med | DR-01 | Research stall names policy-denied tools instead of "answer the question above" | `dfd6021ea` |
| H-3 | Med | DR-02 | Empty/null delegate `source` rejected before launch | `7a2d71556` |
| H-4 | Med | DR-03 | Concurrent system-prompt appends all persist | `ff79602db` |
| H-8 | Med | DR-06 | A delegate whose provider failed reports failure | `89a8440b2` |
| H-10 | Med | DR-08 | Failed tool error text reaches ACP Activity content | `d5335ee29` |

## Open findings requiring a decision (not patched)

| Finding | Sev | Why it was not patched |
|---|---|---|
| A-1 / G-1 / E-15 | High | Auto/“Autonomous” mode denies `write`/`shell` without an explicit permission. This is the deliberate SEC-GOS-003 repair (`3406ad17f`), but Desktop describes Autonomous as editing and creating files freely, and the denial text says “no operator” in interactive sessions. Product/security decision: interactive Auto allowance vs. copy/onboarding change. |
| H-7 | Med | External ACP provider security requests are auto-denied in Auto by the same “no operator” rule (`reply_stream.rs`). |
| F-4 | Med | `GOSLING_MOIM_MESSAGE_TEXT/_FILE` were removed in `65319814b`, but `documentation/docs/guides/context-engineering/using-persistent-instructions.md` and `environment-variables.md` still document them. Restore or remove the docs. |
| D-5 | Med | Exports are verbatim; CLI docs promise a complete backup, while SE-02 and INTENT INV-003 imply no raw secrets. |
| D-19 | Med | Always Allow is keyed by extension name, so a different server under a trusted name runs unprompted. Needs identity design. |
| E-4 | Med | stdio/WS accept `session/new` before `initialize`; enforcement touches every in-process/SDK client. |
| G-2 | Med | Create-workspace dialog preselects `chatgpt_codex` — required by INTENT REQ-031. |
| G-8 | Med | Primary-folder images/HTML refused and PDF preview blank — matches ADR-0013 and CSP `frame-src`; needs product/security call. |
| F-6 (remainder) | Med | Over-limit request still sent after local estimate; pre-send rejection is a product call. |
| C-9 | Low | `mcp remove` leaves secrets; there is no secret ownership record. |
| B-17 | Low | `run -r --model` changing the session model matches docs; `--help` wording differs. |
| B-1 | — | Not a defect: PTY driver artifact (single Ctrl-C cancels when the terminal is read). |

Remaining Medium/Low findings not assigned to a repair group (for example B-5, B-11, B-14, B-15,
C-7, C-10–12, D-6, D-9–15, D-17, D-18, E-5, E-6, E-12–14, F-9–13, G-5–7, G-9–11, G-13, G-15–20,
H-5, H-6, H-9, H-11) remain open follow-ups with repro details in the run results.

## Scenario-library assessment

Cards were not edited during the run. Drift observed for a separately authorized card update:
SK-03 subagent timeout (card 5 min, default 1800 s); G-13 default output extension list; DT-13
wording vs. commit `a48108750`; SE-02 vs. export backup contract (D-5); CX-05 still describes the
removed persistent-instruction variables (F-4).

## Process incidents

- One group-F PTY launch omitted `GOSLING_PATH_ROOT` and wrote an empty session (`20260913_1`,
  working dir `…/scratchpad/F/fx/cx05`, 0 messages), a `projects.json` entry, and a CLI log into
  the operator's real state. It was left in place for operator cleanup.
- One PM-03 variation reached the operator's real `claude` CLI through its fallback search paths
  (`~/.local/bin`, `/opt/homebrew/bin`) and made one real Claude Code call; it was re-run with a
  nonexistent `CLAUDE_CODE_COMMAND`. The fallback search under an isolated root is itself a seam.
- Desktop test instances showed native alert dialogs on the real screen; the operator's running
  Gosling.app logged a normal quit shortly after. No operator PID was signaled.

## Post-repair verification

Integrated build `9d90ee7ee` (debug CLI), isolated roots, same fixture:

- Original repros re-run for 15 repaired findings: **13 Fixed, 2 Partial (F-3, F-8), 0 Regressed.**
  F-2, H-1, E-3, A-2, B-2 (16 → 4 requests), B-9 (112 requests for one failed turn → 30 over nine
  recovering turns), B-6, D-20, F-1, D-1, E-1 (SIGTERM exits in 5.0 s with SSE open), B-13, C-2.
- Cross-cutting smoke on the same build: CL-03, CH-01, CH-04, HS-03, AP-02, SX-01, PA-01 — all Pass.
- Not re-run on the integrated build: Desktop (G-*) and TUI (A-3/A-5/A-6) repairs, which were
  replayed by their repair groups against their worktree builds, and the remaining 120 cards.

Validation commands and results are in `docs/logs/session/2026-09-12-live-playtest-repair-campaign.md`.

Retest note: this machine's system git config uses the macOS keychain credential helper, so a
credentialed plugin URL made `git clone` try to store the (fake) credential and `plugin install`,
which has no timeout, hung until killed. Nothing was stored. C-2 still passes the credentialed URL
to `git clone`; a bounded clone and credential-helper isolation are follow-ups.
