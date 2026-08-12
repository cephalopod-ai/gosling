# Gosling live playtest — all 110 scenario cards

Date: 2026-08-12 (America/New_York)  
Lens: private agent-skills catalog `audit-playtest-app`  
Scope: every card in `docs/test_scenarios/01` through `18`, followed by repairs for confirmed findings  
Build: local debug build from base commit `6044438f27154db0f217ff888879a441a98f0688`, version `0.1.0`

## Verdict

Core CLI, session persistence, context isolation, local MCP, plugin updates,
approval gating, headless formats, authenticated serving, TLS, and ACP stdio
framing were healthy under the exercised fixtures. The playtest found three
product defects and repaired all three. The highest-risk defect allowed a
cancelled approval request to remain unmatched in durable history and be proposed
again on a later turn after the operator switched to Auto mode.

Outcome count: **61 Pass · 4 Fail (repaired) · 37 Blocked · 8 Not executed · 0 N/A = 110**.

`Fail (repaired)` preserves the original live outcome in the ledger; the repair
closure below records the successful retest. `Blocked` means the card was reached
but a named prerequisite prevented a decisive result. `Not executed` means no
decisive run was attempted and is not counted as coverage.

## Environment and evidence

- Repository: `/Users/eric/Work/vscode/forked/gosling`
- Platform: macOS 26.5.2, arm64
- Isolated state/evidence root: `/tmp/gosling-playtest-20260812.xHd6A7`
- Raw evidence: `/tmp/gosling-playtest-20260812.xHd6A7/evidence`
- Provider oracle: test-only OpenAI-compatible fixture on loopback; unique response
  IDs, exact token metadata, bounded slow/401/429/empty/malformed modes, and
  deterministic tool requests
- Additional fixtures: loopback streamable-HTTP MCP server, local stdio MCP,
  generated TLS certificates, local git plugin repository, and disposable project
  trees
- Provider cross-check: local Ollama was attempted, but its available models
  produced empty/unstable replies in this environment; those observations are
  recorded as environmental and were not treated as Gosling defects
- Desktop: the development Electron renderer and embedded backend started against
  isolated state. The required Computer Use accessibility bridge repeatedly hung
  while reading Electron app state, so native Desktop cards were blocked rather
  than tested through an unapproved alternate automation path
- No production credentials or user Gosling state were used. All keys, secrets,
  certificates, plugins, sessions, workspaces, and side effects were disposable.

## Confirmed findings

### GSL-PLAY-2026-001 — Cancelled approval can replay after a mode switch

Severity: **High** · Card: PA-03

In Approve mode, the fixture proposed a `write` call for
`cancel-replay-2.txt`. Pressing Escape cancelled the approval and left the file
absent. After `/mode auto`, the harmless follow-up `Say READY` caused the same
write request to be proposed again and executed before `READY`; the file appeared.
The exported conversation showed the original assistant tool request without a
durable tool response, followed by the repeated request and successful write.

Cause: the CLI created the cancellation tool response only in its local
conversation. It cancelled and dropped the agent stream before that response could
be persisted, leaving the durable assistant tool request unmatched.

Repair: give the cancellation response a stable ID and persist it before cancelling
the stream. A unit regression verifies the durable request/response pair. The live
retest cancelled `cancel-replay-fixed.txt`, switched to Auto, sent `Say READY`, and
received only `READY`; the file remained absent. Before/after transcripts are
`evidence/PA-03-cancel-replay.json` and
`evidence/PA-03-cancel-replay-fixed.json` under the evidence root.

### GSL-PLAY-2026-002 — Missing-session diagnostics creates a misleading bundle

Severity: **Medium** · Card: AC-04

`gosling session diagnostics --session-id does-not-exist --output ...` exited `0`
and created an approximately 143 KiB bundle whose session field was null while
including unrelated config, logs, and prompts. That looked like successful
diagnostics for the requested session and disclosed more local diagnostic material
than the failed request required.

Repair: resolve the requested session before generating or writing a bundle.
Missing sessions now exit nonzero, name the missing ID, and leave no output file.
An integration regression covers the command contract.

### GSL-PLAY-2026-003 — Invalid typed runtime settings silently fall back

Severity: **Medium** · Cards: LC-04, ST-03

`GOSLING_MODE: yolo`, `GOSLING_MAX_TURNS: plenty`, and
`GOSLING_AUTO_COMPACT_THRESHOLD: 5` were silently ignored or normalized while the
command continued. This obscured operator intent, including the active approval
mode and execution limits.

Repair: startup now emits actionable stderr warnings for invalid mode, max-turn,
and compaction-threshold values while retaining the existing documented fallback.
Valid values remain quiet. Integration regressions cover all three invalid cases
and a valid control.

## Repair closure

| Finding | Patch | Regression and live result | State |
|---|---|---|---|
| GSL-PLAY-2026-001 | Persist cancelled tool response before stream cancellation | Unit regression passed; exact live replay stayed absent after `/mode auto` | Closed |
| GSL-PLAY-2026-002 | Preflight session existence before diagnostics generation | Integration regression passed; missing ID exits nonzero and creates no artifact | Closed |
| GSL-PLAY-2026-003 | Warn on invalid typed runtime settings | Integration regressions passed; all three warnings observed and valid control stayed quiet | Closed |

## Scenario ledger (110/110)

| ID | Outcome | Live result / blocker |
|---|---|---|
| LC-01 | Pass | Fresh build/configure path completed; deterministic provider returned exact `PONG`. |
| LC-02 | Pass | Unconfigured info/check/doctor/session-list states were finite and actionable. |
| LC-03 | Pass | `info` and `doctor` exited finitely; broken provider check returned nonzero. |
| LC-04 | Fail (repaired) | Malformed YAML was named and preserved, but invalid typed settings were silent; GSL-PLAY-2026-003 repaired this. |
| CH-01 | Pass | Exact `.txt` inventory and first `PONG` response matched the fixture. |
| CH-02 | Pass | Whitespace was rejected before provider use; 16 KiB markers and Unicode persisted. |
| CH-03 | Pass | Ctrl-C stopped the slow provider request; the next turn returned `READY` with no late output. |
| CH-04 | Pass | Named sessions and history survived process relaunch. |
| CH-05 | Pass | Twenty concurrent marked runs exited `0` with twenty unique session IDs. |
| CH-06 | Pass | `/help` listed commands; `/halp` produced a clear unknown-command hint. |
| WS-01 | Blocked | Desktop workspace interaction was blocked by the Electron accessibility bridge. |
| WS-02 | Blocked | No authorized cloud credential profile and no operable Desktop bridge. |
| WS-03 | Blocked | Native relink picker could not be reached through the required bridge. |
| WS-04 | Blocked | Desktop artifact routing could not be driven through the required bridge. |
| PM-01 | Pass | Config, `info`, and captured request model/provider identifiers agreed. |
| PM-02 | Pass | `/model fixture/custom-v1` applied on the next turn; session returned `READY`. |
| PM-03 | Pass | Synthetic 401 was clear, bounded, nonzero, and redacted; fixed key returned `RECOVERED`. |
| PM-04 | Blocked | The deterministic fixture did not provide two distinguishable planner/main model oracles. |
| EX-01 | Pass | Developer write produced exact requested content inside the disposable project. |
| EX-02 | Pass | HTTP MCP returned the exact UUID marker; stdio memory MCP also initialized. |
| EX-03 | Pass | Broken stdio MCP warned and ordinary `PONG` chat continued. |
| EX-04 | Pass | Removal cleared the extension; a second removal named the miss. |
| SK-01 | Blocked | Skills listed and a plugin skill installed, but the simple provider fixture did not demonstrate instruction-dependent invocation. |
| SK-02 | Pass | Local git plugin updated atomically from sentinel V1 to V2. |
| SK-03 | Blocked | Fixture never emitted delegate calls, so fan-out and sibling isolation had no decisive oracle. |
| SE-01 | Pass | List, rename, remove, and interactive confirmation behaved as documented. |
| SE-02 | Pass | JSON, YAML, and Markdown exports completed and parsed. |
| SE-03 | Pass | Valid import/duplicate behavior passed; truncated, empty, and binary inputs failed. |
| PA-01 | Pass | Approval waited without side effect; Allow created exact content; Deny created nothing. |
| PA-02 | Pass | Chat-mode boundary skipped the write and surfaced a structured tool result. |
| PA-03 | Fail (repaired) | Cancelled write replayed after Auto switch; GSL-PLAY-2026-001 repaired and live-retested it. |
| CL-01 | Pass | Top-level and advertised subcommand help paths exited `0`. |
| CL-02 | Pass | Typo, bad flags, conflicting inputs, and missing arguments failed clearly. |
| CL-03 | Pass | Quiet output was exact; JSON and every stream-JSON line parsed; stdin `-i -` worked. |
| CL-04 | Pass | zsh, bash, fish, and Nushell completions were nonempty. |
| ST-01 | Blocked | Desktop persistence controls were inaccessible through the required bridge. |
| ST-02 | Blocked | Desktop navigation stress was inaccessible through the required bridge. |
| ST-03 | Fail (repaired) | Invalid mode/turn/threshold values were silent; GSL-PLAY-2026-003 repaired this. |
| HS-01 | Blocked | Success and bad-provider paths passed, but the fixture did not produce a decisive repeated-tool budget loop. |
| HS-02 | Pass | Secret requirement, 401/200 auth, bind conflict, Ctrl-C release, and restart all passed. |
| HS-03 | Pass | Delayed stdio handshake and malformed-then-valid sequence produced clean JSON frames and exited on EOF. |
| SX-01 | Pass | Twenty concurrent short sessions retained unique IDs and markers. |
| SX-02 | Blocked | Same-session multi-view concurrency required an operable Desktop or second ACP client. |
| SX-03 | Blocked | Only one deterministic model oracle was available for active-stream thrash. |
| SX-04 | Pass | Forty-turn allowed variation produced 80 ordered messages; unique markers survived export/reload. |
| SX-05 | Blocked | Provider fixture did not generate a tool storm, so the termination reason was not decisive. |
| SX-06 | Pass | SIGKILL during a slow owned run left the store usable; same session returned `RECOVERED`. |
| SX-07 | Blocked | Desktop workspace/artifact save race was inaccessible through the required bridge. |
| SX-08 | Pass | Concurrent info/list/skills loops stayed parseable; final `PONG` succeeded. |
| SX-09 | Blocked | Cross-surface inventory could not include Desktop state. |
| AC-01 | Pass | Recency/name/ID selection and invalid selector combinations behaved correctly. |
| AC-02 | Pass | Fork/source IDs and post-fork markers remained independent. |
| AC-03 | Pass | Failing editor was clear; source export hash stayed stable. |
| AC-04 | Fail (repaired) | Missing ID produced a misleading diagnostics bundle; GSL-PLAY-2026-002 repaired this. |
| AC-05 | Pass | Filters, ordering, limits, and JSON output were stable. |
| AC-06 | Blocked | No safe observable platform-opener fixture was available. |
| AC-07 | Not executed | Shell initialization matrix was not run in this campaign. |
| AC-08 | Not executed | Independent terminal identity attach/resume was not run in this campaign. |
| AC-09 | Pass | Locally built text UI `--text 'Say HI'` exited `0` with exact `HI`. |
| AC-10 | Not executed | Review dry-run discovery/scoping was not run in this campaign. |
| DT-01 | Blocked | Desktop onboarding interaction was blocked by the Electron accessibility bridge. |
| DT-02 | Blocked | Native close-versus-quit lifecycle could not be driven through the required bridge. |
| DT-03 | Blocked | Keyboard/focus inspection required the unavailable Electron app-state read. |
| DT-04 | Blocked | Shortcut settings could not be reached through the required bridge. |
| DT-05 | Blocked | Window geometry/layout could not be controlled through the required bridge. |
| DT-06 | Blocked | Artifact preview matrix could not be reached through the required bridge. |
| DT-07 | Blocked | Artifact workbench navigation/relaunch could not be reached through the required bridge. |
| DT-08 | Blocked | Archive/restore picker flow could not be reached through the required bridge. |
| DT-09 | Blocked | Desktop external-backend reconnect could not be exercised through the required bridge. |
| DT-10 | Blocked | Notification permission state could not be safely controlled through the required bridge. |
| CX-01 | Pass | Root rule appeared in root and child scope, and not in the outside sibling. |
| CX-02 | Pass | Child-A loaded root plus child rule without leaking it outside its scope. |
| CX-03 | Pass | Ordered/empty/duplicate/custom filenames worked; malformed/non-string values warned and fell back. |
| CX-04 | Pass | Ignored fake secret was absent from automatic request logs and evidence. |
| CX-05 | Pass | Editing `.goslinghints` between turns changed captured context from PERSIST-A to PERSIST-B. |
| CX-06 | Pass | Test config, data, state, plugins, and sessions stayed under disposable roots. |
| CX-07 | Pass | Successful and failed `--no-session` runs left session count and markers unchanged. |
| CX-08 | Pass | UTF-8/CRLF/space/Unicode/leading-hyphen/1 MiB inputs were bounded; empty and binary failed. |
| CX-09 | Pass | Disabled code runtime named its setting and failed while ordinary Developer write still worked. |
| CX-10 | Pass | Parallel `--system A/B` requests stayed scoped; config hash stayed stable. |
| PN-01 | Pass | Perpetual 429 retried finitely, exited nonzero with rate-limit clarity, and recovered after restoration. |
| PN-02 | Not executed | A deterministic mid-stream disconnect route was not implemented in the provider fixture. |
| PN-03 | Pass | Slow request cancelled within the deadline; follow-up returned `READY` with no stale output. |
| PN-04 | Blocked | A 1 MiB turn triggered compaction, but full post-compact history/export/relaunch acceptance was not completed. |
| PN-05 | Pass | Empty/malformed replies exited nonzero with parseable JSON errors; restored provider returned `RECOVERED`. |
| PN-06 | Pass | Exact custom model reached the request log; run override left config hash unchanged. |
| PN-07 | Blocked | Custom ID succeeded, but the fixture's list-model endpoint was not failed independently. |
| PN-08 | Blocked | Local Ollama was unstable and its daemon lifecycle was not placed under test control. |
| PN-09 | Blocked | No sandbox OAuth provider/token lifecycle was available. |
| PN-10 | Blocked | Exact output-token count matched, but price/retry/Desktop reconciliation lacked fixture metadata. |
| AP-01 | Not executed | Authenticated startup paths ran, but the explicit dangerous-mode branch was not run. |
| AP-02 | Pass | Missing/wrong secrets returned 401; correct secret initialized; secret stayed out of output. |
| AP-03 | Pass | Explicit Origin allowlist matched only its exact value, excluding loopback defaults and lookalikes. |
| AP-04 | Pass | Missing/cert-only/mismatch failed; valid trusted TLS served; untrusted client failed closed. |
| AP-05 | Blocked | Stdout framing/EOF passed, but full create-session/prompt/provider-error matrix was not completed. |
| AP-06 | Pass | Malformed JSON returned a structured parse error; later initialize succeeded on the same process. |
| AP-07 | Not executed | Concurrent authenticated ACP clients were not established. |
| AP-08 | Not executed | ACP cancellation timing/tool checkpoints were not run. |
| AP-09 | Not executed | Server loss during an active ACP request was not run. |
| AP-10 | Pass | Current version initialized; unknown version `999` failed with structured invalid params. |
| SI-01 | Blocked | Duplicate workspace identity/rename required operable Desktop interaction. |
| SI-02 | Blocked | Workspace deletion with pinned sessions required operable Desktop interaction. |
| SI-03 | Blocked | Symlinked workspace/output routing required operable Desktop interaction. |
| SI-04 | Pass | Markdown/JSON/YAML exports parsed and represented the exercised conversation. |
| SI-05 | Pass | Explicit safe import cwd won; duplicate import remained non-destructive. |
| SI-06 | Blocked | No supported-old-release state fixture was available. |
| SI-07 | Pass | Duplicate MCP install updated explicitly; exact command/env arrived; double-remove named the miss. |
| SI-08 | Pass | Fake secret existed only in mode-0600 `secrets.yaml`, absent from config/info/list/logs. |
| SI-09 | Pass | Missing-frontmatter plugin was rejected with its path; valid plugin remained usable. |
| SI-10 | Blocked | Competing CLI/Desktop approval ownership required an operable Desktop client. |

## Blocker register

The 37 blocked outcomes cluster around five prerequisites:

1. The mandatory Computer Use bridge could start Electron but could not read its
   native app state, blocking Desktop interaction without substituting another GUI
   automation mechanism.
2. The deterministic provider exposed one effective model behavior and did not
   generate planner/subagent/tool-storm patterns on demand.
3. OAuth, old-release migration, price metadata, and controlled local-engine
   lifecycle fixtures were unavailable.
4. Full multi-client ACP orchestration was outside the raw framing fixture used in
   this pass.
5. Several optional advanced CLI cards were not attempted; they are named as Not
   executed rather than inferred from historical reports.

## Validation

- `cargo fmt --all`
- `cargo test -p gosling-cli`
- `cargo build -p gosling-cli --bin gosling`
- `cargo clippy --all-targets -- -D warnings`
- Live PA-03 exact reproduction after rebuild
- Live missing-diagnostics and invalid-config command retests
- `git diff --check` and repository documentation-governance marker checks

The command results and any deviations are recorded in the accompanying session
log. Standing scenario cards were not edited.

## Cleanup and residual risk

All owned Gosling, Electron development, fixture-provider, MCP, ACP, and serve
processes were stopped after evidence capture, and owned loopback ports were
released. The disposable evidence root was intentionally retained for inspection.
The installed user Gosling application and normal user state were not modified.

Residual risk remains concentrated in the blocked Desktop and multi-client ACP
cards. Those outcomes are not evidence that the surfaces pass or fail; rerun them
when the Electron accessibility bridge and a version-pinned ACP client harness are
available.
