# Gosling Playtest Scenario Library

End-to-end, user-facing test scenarios for exercising gosling like a real
operator would — through the CLI, Ink/text UI, and Electron Desktop, against
a configured local install. This library is the standing plan for exploratory
playtest passes; each pass executes these scenario cards and records results
in a separate report.

## Provenance and baseline

The methodology is derived from the `audit-playtest-app` skill
(`agent-skills` repo, `010_audit/audit-playtest-app/`): discover the app,
act like a curious/impatient/occasionally mistaken user, exercise happy
paths, invalid and boundary inputs, persistence, interruption, and recovery,
and report every issue with reproduction steps, severity, and user impact.
Organization and card density follow the Cuttlefish
`docs/test_scenarios/` pattern; **content is gosling-specific** (CLI,
Desktop, workspaces, ACP/serve, MCP extensions, skills, subagents).

Evidence discipline applies verbatim:

- **Confirmed** issues are observed by running the app. **Suspicions** are
  inferred from code or docs. Label every finding as one or the other.
- Never claim a scenario was executed if it was only inferred. If the
  binary/Desktop could not be launched, say so and report what blocked it.
- Capture exact inputs, screen/command, visible error text, CLI/log output,
  and observed state for every issue.

## Scope: what this library covers, and what it deliberately does not

This library targets behavior that is **only detectable by running the app
and using it as a user** — the gaps left by unit/integration tests and static
audits:

| Already covered elsewhere | Where | Excluded here |
|---|---|---|
| Unit/integration correctness | `cargo test`, crate `tests/` | Yes |
| Desktop unit/component tests | `ui/desktop` Vitest suites | Yes |
| Static security / dataflow / architecture | `docs/cloud/audit-*` | Yes |
| Goose catalog compatibility normalization | `documentation/GOOSE_COMPATIBILITY.md` | Yes (except live install/use paths) |

What remains is the **lived operator experience**: first-run configure,
provider honesty, chat and tool loops, workspaces and credential profiles,
MCP extensions, skills/plugins, session import/export, permission gates,
CLI robustness, headless `run` / `serve` / ACP, Desktop multi-window
navigation, and load/stress seams.

## Safety rails (binding for every pass)

- Prefer a **disposable config + data home**. On macOS/Linux, point
  `XDG_CONFIG_HOME` and `XDG_DATA_HOME` (and any documented gosling data
  overrides) at a temp directory so `config.yaml`, sessions, workspaces, and
  secrets never touch an operator's live `~/.config/gosling`. Never playtest
  against production credentials or customer workspaces.
- Use test/sandbox LLM credentials only. Prefer the cheapest path available
  (local Ollama, free-tier router, or a throwaway key with a spend cap).
- No real personal data. No malicious payloads or exploitation — invalid
  input means *safe-but-wrong* (empty required fields, oversized text,
  wrong file types, mistyped flags).
- Tool-permission scenarios must run in a sandbox directory you own; do not
  approve shell/file tools against system paths you care about.
- Everything written during a pass stays in the disposable home; clean up
  afterward.
- Stress cards: stop if the host is unsafe (thermal, disk &lt; 1 GiB free).

## How to run a pass

1. **Environment** (source checkout):
   ```bash
   source bin/activate-hermit
   cargo build -p gosling-cli
   # optional Desktop:
   # just run-ui   # or ui/desktop docs
   ```
   Ensure at least one provider is configured (`gosling configure`) under
   the disposable home. CLI binary: `./target/debug/gosling` or install path.
2. **Order**: execute files in numeric order. Lifecycle and a primary chat
   happy path first; stress last (after a green smoke). Within a file, run
   cards top to bottom; later cards often depend on state from earlier ones.
3. **Record**: for each card, fill in *Actual result*, *Status*
   (Pass / Fail / Blocked / Not applicable / Not executed), *Confirmation*
   (Confirmed / Suspicion), and issues with reproduction steps. Keep the
   library itself clean — record results in the pass report, not by editing
   cards.
4. **Report**: produce a playtest report per the baseline skill's
   `templates/playtest-report.md` shape. Durable summaries can live under
   `docs/cloud/` when the pass is part of a formal audit.

## Scenario card format

Each scenario uses this condensed card (aligned with Cuttlefish / the
skill's `templates/scenario-card.md`):

```
### <ID> — <Name>
- Goal: what the user is trying to accomplish
- Category: happy path / invalid input / boundary / empty state /
  interruption / persistence / delete-undo / settings / navigation /
  recovery / files / concurrency
- Preconditions: required state, data, config
- Steps: numbered user actions
- Expected: what a correct app does
- Observe: seams and secondary effects to watch for
- Variations: additional input/order permutations under the same card
```

Severity scale: **Critical** (crash, data corruption, lost work, blocked
primary workflow, irreversible action without warning) · **High** (major
workflow fails, wrong saved data, broken relaunch, unrecoverable without
technical help) · **Medium** (secondary workflow fails, unclear errors,
stuck UI, non-persisting settings) · **Low** (confusing label, glitch,
awkward navigation) · **Note** (observation or product question).

## Files (50 scenarios)

| File | Surface | Cards | Core question |
|---|---|---|---|
| [`01-first-run-and-lifecycle.md`](01-first-run-and-lifecycle.md) | install, configure, info, doctor | LC-01–04 | Can a new operator get from zero to a working agent? |
| [`02-chat-sessions.md`](02-chat-sessions.md) | CLI session, Desktop chat, slash cmds | CH-01–06 | Does the primary chat loop work, interrupt, and persist? |
| [`03-workspaces.md`](03-workspaces.md) | Desktop workspaces, credentials, artifacts | WS-01–04 | Do workspace pins, profiles, and outputs stay honest? |
| [`04-providers-and-models.md`](04-providers-and-models.md) | providers, model switch, planner | PM-01–04 | Is model selection truthful across surfaces? |
| [`05-extensions-and-mcp.md`](05-extensions-and-mcp.md) | extensions, MCP add/list/remove | EX-01–04 | Do MCP extensions install, load, fail, and unload cleanly? |
| [`06-skills-plugins-subagents.md`](06-skills-plugins-subagents.md) | skills, plugins, subagents | SK-01–03 | Do skill/plugin/subagent paths work end to end? |
| [`07-sessions-import-export.md`](07-sessions-import-export.md) | list/remove/export/import | SE-01–03 | Can sessions be listed, removed, and moved safely? |
| [`08-permissions-and-approvals.md`](08-permissions-and-approvals.md) | modes, tool perms, approvals UI | PA-01–03 | Do gates hold and resume as the operator expects? |
| [`09-cli-surface.md`](09-cli-surface.md) | help, flags, run, completion | CL-01–04 | Is the CLI robust to real terminal usage and misuse? |
| [`10-settings-config-navigation.md`](10-settings-config-navigation.md) | Settings, config.yaml, sidebar | ST-01–03 | Do settings persist and screens agree? |
| [`11-headless-serve-acp.md`](11-headless-serve-acp.md) | `run`, `serve`, `acp` | HS-01–03 | Do non-interactive and server surfaces stay coherent? |
| [`12-stress-and-adversarial.md`](12-stress-and-adversarial.md) | load, races, recovery under pain | SX-01–09 | Does gosling stay coherent when the operator is impatient? |

### Suggested pass shapes

| Pass | Files | Intent |
|---|---|---|
| Smoke / first day | 01 → 02 → 09 | Configure, one chat works, CLI is sane |
| Core product | 01 → 05, 08, 10 | Chat, workspaces, providers, MCP, perms, settings |
| Resilience | 04, 07, 11 | Model honesty, session portability, headless/serve |
| Stress | 12 (after green smoke) | Concurrency, bloat, restart-under-load, env seams |
| Full library | 01 → 12 numeric order | Release or major-regression playtest (**50 cards**) |

## Required coverage checklist

A pass is complete only when every category below has at least one executed
scenario (or an explicit not-applicable/blocked note):

- [ ] First launch / initial empty state
- [ ] Primary happy-path workflow (prompt → response)
- [ ] Primary workflow with invalid input
- [ ] Save / persistence behavior
- [ ] Delete, cancel, or undo behavior
- [ ] Settings or preferences persistence
- [ ] Navigation across major Desktop surfaces (or CLI equivalent)
- [ ] Close and relaunch behavior
- [ ] Interrupted or stopped workflow
- [ ] File attach / export / workspace outputs
- [ ] Error recovery (provider unavailable, extension fail, crash)
- [ ] Edge or boundary input
- [ ] Model / provider selection and mid-session switching
- [ ] Permission / approval boundary
- [ ] Concurrency or load stress
- [ ] Headless or server surface (`run` / `serve` / ACP)

## Scenario index (50)

| ID | File | Name |
|---|---|---|
| LC-01 | 01 | Fresh install to first successful reply |
| LC-02 | 01 | First-launch empty / unconfigured states |
| LC-03 | 01 | `info` / `doctor` honesty |
| LC-04 | 01 | Config hand-edit then relaunch |
| CH-01 | 02 | First message to first response |
| CH-02 | 02 | Composer / REPL input seams |
| CH-03 | 02 | Interrupt mid-run then continue |
| CH-04 | 02 | Session persistence across relaunch |
| CH-05 | 02 | Parallel sessions isolation |
| CH-06 | 02 | Slash-command discoverability and typos |
| WS-01 | 03 | Create workspace and pin new chat |
| WS-02 | 03 | Credential profile bind and secret non-echo |
| WS-03 | 03 | Missing primary folder / relink |
| WS-04 | 03 | Artifact save routes to product outputs |
| PM-01 | 04 | Configure provider and model |
| PM-02 | 04 | Mid-session model or provider switch |
| PM-03 | 04 | Bad / expired API key failure clarity |
| PM-04 | 04 | Planner vs main model split |
| EX-01 | 05 | Enable bundled extension and use a tool |
| EX-02 | 05 | Add streamable HTTP / stdio MCP extension |
| EX-03 | 05 | Broken MCP extension fails closed |
| EX-04 | 05 | Remove extension; session no longer offers tools |
| SK-01 | 06 | Skills list and invoke |
| SK-02 | 06 | Plugin install/update from git |
| SK-03 | 06 | Subagent parallel fan-out |
| SE-01 | 07 | Session list, rename, remove |
| SE-02 | 07 | Export session |
| SE-03 | 07 | Import session (JSON / foreign jsonl) |
| PA-01 | 08 | Manual approval mode gates a tool |
| PA-02 | 08 | Never-allow tool is refused |
| PA-03 | 08 | Mode switch mid-session |
| CL-01 | 09 | Help and discoverability |
| CL-02 | 09 | Unknown commands and bad flags |
| CL-03 | 09 | `gosling run` one-shot formats |
| CL-04 | 09 | Completion generation |
| ST-01 | 10 | Desktop settings persist across relaunch |
| ST-02 | 10 | Sidebar navigation stress |
| ST-03 | 10 | Invalid config.yaml values |
| HS-01 | 11 | Headless `run` with budgets |
| HS-02 | 11 | `gosling serve` lifecycle |
| HS-03 | 11 | `gosling acp` stdio smoke |
| SX-01 | 12 | Session stampede (many short chats) |
| SX-02 | 12 | Multi-window / multi-tab race on one session |
| SX-03 | 12 | Rapid model thrash under active stream |
| SX-04 | 12 | Hundred-turn history bloat |
| SX-05 | 12 | Extension/tool storm and max-turns budget |
| SX-06 | 12 | Hard kill mid-run then recover |
| SX-07 | 12 | Workspace switch race during artifact save |
| SX-08 | 12 | Config thrash + concurrent CLI invocations |
| SX-09 | 12 | Cross-surface consistency after chaos |
