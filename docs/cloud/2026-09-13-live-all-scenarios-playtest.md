# Gosling live all-scenarios playtest — 2026-09-13

## Summary

The authoritative 127-card library under `docs/test_scenarios/` was dispositioned against clean
`main` at `b14427e57c24` (gosling 1.2.5, macOS 26.6.2 arm64). This was a test-only pass: no
runtime repairs were made.

Result: **55 Pass · 31 Partial · 7 Fail · 12 Blocked · 22 Not executed**.

Six distinct confirmed product findings remain: five Medium and one Low. The strongest held
seams were autonomous and approval-gated file writes, session import/export, extension failure
recovery, plugin install/update, HTTP ACP auth/origin/TLS enforcement, context scoping, concurrent
session creation, 40-turn history persistence, and Desktop artifact filtering across relaunch.

"Partial" means that the primary path was exercised but at least one material step or variation
was not. "Blocked" means a required safe fixture or external capability was unavailable. "Not
executed" is explicit coverage debt; it is not rounded up to success.

## App Understanding

Gosling is a Rust agent framework exposed through an interactive CLI, one-shot `run`, stdio and
HTTP/WebSocket ACP transports, MCP extensions, skills/plugins/subagents, and an Electron Desktop
client. Durable sessions and configuration are rooted by `GOSLING_PATH_ROOT`; Desktop talks to a
`gosling serve --platform desktop` child over authenticated TLS. Workspaces pin new Desktop chats
to folders, output routes, provider/model defaults, and optional credential-profile references.

The highest-risk boundaries for this pass were durable state after interruption, the difference
between permission modes, provider retry/stream termination, auth and origin checks on ACP,
workspace path failure, and agreement between Desktop, ACP, the provider trace, and disk.

## Scenario Library

- Source: `docs/test_scenarios/README.md` and numbered files 01–20.
- Cards: 127; no scenario files were edited.
- Run ID: `GSL-PT-20260913`.
- Finding IDs: `GSL-PT-20260913-001` through `-006`.
- Library drift: root help now advertises `secret` and `shell-validate`, but CL-01's enumerated
  command list does not mention them.

## Inputs

- Debug CLI built from `b14427e57c24` with `cargo build -p gosling-cli --bin gosling`.
- Installed app binary was spot-checked for CLI version parity; Desktop behavior used the current
  debug backend and source renderer through the documented development launcher.
- Deterministic loopback OpenAI-compatible fixture on `127.0.0.1:18473`, with request/model/tool
  capture and scripted success, 401, 429, context-limit, delay, disconnect, empty-body, malformed
  body, write, MCP, skill, and delegate responses.
- Disposable roots and workspaces under
  `/tmp/gosling-playtest-GSL-PT-20260913.psN6hD`; no production credentials or accounts.
- Local stdio MCP fixture, local git plugin fixture, generated test TLS certificates, and a
  scratch Python Playwright environment used only for renderer automation.

## Outputs

- This report.
- Session log: `docs/logs/session/2026-09-13-live-all-scenarios-playtest.md` (ignored by repo
  policy but retained locally).
- Raw provider requests, exports, diagnostics, process output, and Desktop screenshots retained
  under the disposable test root above for follow-up.
- No source, test, schema, or scenario-library files were changed.

## Scenarios Executed

| File | Pass | Partial | Fail | Blocked | Not executed |
|---|---|---|---|---|---|
| 01 lifecycle | LC-01, LC-02, LC-04 | — | LC-03 | — | — |
| 02 chat | CH-01, CH-02, CH-05, CH-06 | CH-04 | CH-03 | — | — |
| 03 workspaces | WS-01 | — | WS-03 | WS-02 | WS-04 |
| 04 providers | PM-01–04 | — | — | — | — |
| 05 extensions | EX-01–04 | — | — | — | — |
| 06 skills/plugins/subagents | SK-01, SK-02 | — | SK-03 | — | — |
| 07 import/export | SE-02, SE-03 | SE-01 | — | — | — |
| 08 permissions | PA-01, PA-02 | PA-03 | — | — | — |
| 09 CLI | CL-01–04 | — | — | — | — |
| 10 settings | ST-03 | ST-01, ST-02 | — | — | — |
| 11 headless/serve/ACP | HS-01–03 | — | — | — | — |
| 12 stress | SX-01, SX-04, SX-06 | SX-05, SX-08, SX-09 | — | — | SX-02, SX-03, SX-07 |
| 13 advanced CLI | AC-02, AC-04, AC-05, AC-07, AC-10 | AC-01, AC-03 | — | — | AC-06, AC-08, AC-09 |
| 14 Desktop | DT-13 | DT-01–03, DT-05–07, DT-14 | — | DT-10, DT-11 | DT-04, DT-08, DT-09, DT-12, DT-15, DT-16 |
| 15 context/filesystem | CX-01, CX-05, CX-08, CX-10 | CX-02, CX-06 | CX-07 | — | CX-03, CX-04, CX-09 |
| 16 provider resilience | PN-01, PN-02, PN-05, PN-06, PN-10 | PN-03, PN-04, PN-07 | PN-08 | PN-09, PN-11 | PN-12 |
| 17 ACP protocol | AP-01–05 | AP-06, AP-09, AP-10 | — | — | AP-07, AP-08 |
| 18 state/extension depth | SI-04 | SI-01, SI-05, SI-07, SI-09, SI-10 | SI-08 | SI-06 | SI-02, SI-03 |
| 19 deep research | — | — | — | DR-01, DR-02 | — |
| 20 research regressions | DR-07 | DR-09 | — | DR-03, DR-04, DR-06, DR-08 | DR-05 |
| **Total** | **55** | **31** | **7** | **12** | **22** |

Selected concrete evidence:

- Twenty concurrent `run` processes produced 20 exact markers and 20 distinct stored sessions.
- One session reached 40 turns / 80 messages; export retained every numbered marker.
- SIGKILL left a one-message interrupted session that resumed successfully without manual store
  repair, although the first recovery took about 30 seconds.
- Manual approval held the write absent until approval; Allow created the file, Deny did not, and
  Chat mode advertised no write tool and created nothing.
- JSON/YAML exports parsed, files were mode 0600, directory writes failed, and symlink export was
  refused without changing the target. Duplicate import was idempotent and malformed imports did
  not alter the store.
- HTTP ACP rejected missing/wrong secrets with 401; default origins allowed loopback and rejected
  `null`/lookalikes; an explicit allowlist replaced the defaults. TLS mismatch/malformed/missing
  paths failed before bind, a trusted generated certificate worked, and an untrusted client failed.
- Root project instructions appeared exactly once in provider system context, direct child launch
  received root plus child instructions, and an outside sibling received neither. Updated
  `.goslinghints` replaced the old sentinel on the next turn.
- Desktop sent an exact provider-backed PONG, reached Chat History, New Research, New Chat, and all
  sampled settings tabs without renderer errors, and had no page-level horizontal escape at
  700 px. Workspace validation named a missing folder before save. Autonomous Desktop writes
  produced Markdown and Rust artifacts; the inventory and repository-file filter persisted after
  relaunch. An 8,000-character Initial Input remained within the 620 px research dialog and was
  present after reopening.

## Issues

### GSL-PT-20260913-001 — `doctor` reports success while the configured provider is unreachable

- Severity: **Medium**
- Cards: LC-03, PN-08
- Confidence: high; deterministic loopback outage.
- Evidence: with `OPENAI_HOST` pointed to a closed port, `info --check` exited 1 with a connection
  error, while `doctor` exited 0 and reported `configuration present (not verified — no provider
  request was made)`. A restored endpoint immediately served PONG.
- Impact: automation and operators cannot use `doctor` as the health check implied by these cards;
  the two diagnostics disagree about the same hard-down provider.

### GSL-PT-20260913-002 — cancelled turns are not represented honestly in resumed history

- Severity: **Medium**
- Cards: CH-03, CH-04
- Confidence: high; delayed fixture plus export/resume evidence.
- Evidence: Ctrl-C stopped a delayed response in 4.3 seconds and the next turn worked, but the CLI
  printed the assistant-like canned line `Yes — what would you like me to do?` rather than a
  cancellation state. On `--history` resume, both the submitted cancelled prompt and a terminal
  cancelled record were absent.
- Impact: the live interruption works, but durable history cannot explain that a user submitted
  and cancelled work, weakening continuity and auditability.

### GSL-PT-20260913-003 — each subagent's single tool action is rendered three times

- Severity: **Medium**
- Card: SK-03
- Confidence: high; provider trace and filesystem are independent oracles.
- Evidence: the parent launched three parallel delegates. Each subagent made exactly one provider
  tool-response round trip and produced one one-byte file (`a.txt=A`, `b.txt=B`, `c.txt=C`), but
  the CLI activity stream displayed the same write card three times for each subagent ID.
- Impact: operators cannot infer execution count from activity, which is especially risky for
  non-idempotent delegated actions.

### GSL-PT-20260913-004 — `--no-session` retains full prompt markers in durable state

- Severity: **Medium**
- Card: CX-07
- Confidence: high; exact marker search after process exit.
- Evidence: one successful and one failed `run --no-session` left the public session count
  unchanged, but both exact unique prompt markers were present in `data/sessions/sessions.db-wal`
  and `state/logs/llm_request.*.jsonl` after relaunch.
- Impact: the option prevents resumability but does not provide the expected content-retention
  boundary for stateless automation.

### GSL-PT-20260913-005 — missing Desktop workspace folder collapses to `Invalid params`

- Severity: **Medium**
- Card: WS-03
- Confidence: high; folder move/restore was performed in the disposable tree.
- Evidence: create-workspace validation initially gave the useful message `primary working folder
  is unavailable; relink it before starting a session`. After saving that workspace, moving its
  primary folder and sending from a new pinned chat produced only `Could not start the chat:
  Invalid params`. Restoring the folder restored chat creation.
- Impact: the user is blocked but is not told which folder failed or how to relink it.

### GSL-PT-20260913-006 — `mcp remove` leaves the extension's stored secret behind

- Severity: **Low**
- Card: SI-08
- Confidence: high; unique fake value and isolated file-backed secret store.
- Evidence: `mcp install --secret PLAYTEST_SECRET=<fake>` kept the value out of `config.yaml`,
  `mcp list`, and `info -v`, but after `mcp remove secretcheck` the unique value remained in the
  mode-0600 `secrets.yaml`. This matches the open C-9 design gap from the 2026-09-12 report.
- Impact: credentials can outlive the extension that owned them and require manual cleanup.

Notes, not promoted to findings:

- Both the debug and installed 1.2.5 binaries emit `--version` as ` 1.2.5` (leading space, no
  executable name). Exit status remains 0.
- The invalid-JSON provider fixture was reported as an empty provider response; the turn still
  failed safely and stream-JSON remained parseable.
- The Desktop main log emitted repeated `get-git-branch-info` access-denied diagnostics while a
  workspace folder was moved and once after relaunch; no renderer error or wrong-path access was
  observed.

## Seam Tests

| Seam | Result |
|---|---|
| CLI → provider → session store | Pass, with findings 001/002/004 on health and interruption semantics |
| CLI → permission manager → filesystem | Pass for Auto, Manual Allow/Deny, and Chat-only brake |
| CLI → stdio MCP | Pass for install, use, timeout/crash recovery, removal, and duplicate-name update |
| CLI → plugin/skills/subagents | Plugin and skill paths pass; subagent rendering fails per 003 |
| CLI → export/import/diagnostics | Pass for core format, permissions, malformed input, idempotency, and redaction |
| HTTP ACP → auth/origin/TLS/store | Pass for exercised startup and boundary matrix |
| Context resolver → provider system prompt | Pass for root/outside/refresh; nested lazy-entry remains partial |
| Desktop renderer → authenticated backend → provider | Pass for chat/navigation/workspace/artifact smoke; missing-folder error fails per 005 |
| Stress → session store | Pass for 20-way creation and 40-turn history; recovery latency noted |

## Untested

The main gaps are multi-window same-session races; rapid model thrash; native save, Trash,
clipboard, notification, keyboard rebinding, external-backend, and multi-dialog Desktop flows;
prior-release migration; symlinked workspace/reference depth; OAuth refresh; provider failover;
full compaction-reduction matrices; concurrent ACP clients and ACP cancellation; and the specialized
Deep Research delegate/extension regression fixtures. These are listed as Blocked or Not executed
in the ledger rather than inferred from unit tests or the 2026-09-12 campaign.

The packaged-app copy did not create a renderer even though its CDP socket opened. Desktop coverage
therefore used the repository's development launcher with the current debug backend. That is a
harness limitation and means installed-bundle lifecycle/signing behavior was not revalidated.

## Process Incidents

- Building the optional `gosling-test-support` MCP fixture example failed in unrelated helper code
  because OpenTelemetry metrics imports/functions were unavailable under its active feature set.
  The target gosling CLI build was already green, so the run used a standalone protocol fixture;
  this helper failure was not classified as a product finding.
- The first provider-fixture revision considered prior user messages for delayed-response routing.
  It was corrected to use only the latest user message before affected recovery checks were
  accepted.
- An unrelated untracked file, `docs/cloud/20260913_Gemini_Audit_Data_gosling.md`, appeared during
  the run. It was not created, edited, or deleted by this playtest.

## Recommended Next Pass

1. Repair and add regression coverage for findings 001–005; retain finding 006 as the already-known
   secret-ownership design decision unless ownership metadata is introduced.
2. Re-run CH-03/04 and SK-03 first because both affect whether operators can trust activity and
   history after asynchronous work.
3. Build a version-pinned ACP client that handles server notifications/requests and use it for
   AP-07–10 plus PN-11, rather than expanding ad-hoc framing scripts.
4. Run the Deep Research fixture suite separately with deterministic report, delegate-source,
   queued-writer, external-provider, and invalid-extension modes.
5. Run a signed packaged Desktop pass for native dialogs, external backend, notification, Trash,
   clipboard, and multi-window cards.

## Disposition addendum — 2026-09-13

This report remains the immutable playtest record. The later
[consolidated audit repair](2026-09-13-consolidated-audit-repair.md) reconciled it with the Gemini
dataflow audit and source contracts.

- GSL-PT-20260913-001 through GSL-PT-20260913-005 were repaired and regression-tested.
- GSL-PT-20260913-006 remains an explicit credential-ownership design item; deleting an
  extension's referenced secret is unsafe until shared ownership can be represented.
- The repair campaign replayed the repaired paths and ran broad source regressions, but did not
  perform another complete 127-card or signed installed-application pass.
