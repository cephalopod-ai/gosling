# Audit Lens — Playtest App (Static / Simulation-Reasoned)

Lens: `audit-playtest-app` · Scope: `gosling` CLI + TUI + desktop user journeys
Authority: **audit-only / read-only**. Only this file was written.
Builds on `docs/cloud/00-orientation.md`.

## Evidence basis banner — READ FIRST

**Nothing in this report was executed live.** The skill is designed to playtest a
*running* application; a full Electron + Rust release build with a configured LLM
provider is not available in this environment. I attempted `cargo build -p gosling-cli`
(as permitted). **The build failed and produced no runnable binary** — it aborted inside
a dependency build script (`build_script_build::download_static_lib_binaries` →
`build failed`), i.e. a compile-time step that tries to **download a static-lib binary
over the network**, which the sandbox blocks. `target/debug/gosling` never appeared, and
`./target/debug/gosling --help` returned exit 127 (No such file). The failure is an
environment/network limitation, not a code defect in gosling. No `--help`, no interactive session, no provider call, no MCP subprocess
was observed. Every behavioral claim below is **simulation-reasoned** from the code
paths I read, and every finding is capped at **Likely / Plausible** per
`evidence_discipline.md` (no `Confirmed` — confirmation requires a live run I could
not perform). File:line citations are real (I read them); the *runtime behavior* they
imply is inferred, not witnessed.

## What the app is (user-facing view)

A terminal-first AI agent. Primary user surfaces, from `crates/gosling-cli/src/cli.rs`:

- **Default `gosling`** (no subcommand) → first-run config wizard if unconfigured,
  else an interactive REPL chat session (`cli.rs:1568-1599`).
- **`gosling session` / `s`** — start/resume/fork/edit interactive sessions; subcommands
  `list`, `remove`, `export`, `import`, `diagnostics` (`cli.rs:430-526`).
- **`gosling run`** — headless/one-shot from `-t TEXT`, `-i FILE`, or `-i -` (stdin);
  `--output-format text|json|stream-json` (`cli.rs:707-729`, `1427-1540`).
- **`gosling configure` / `info [--check]` / `doctor`** — setup + health.
- **`gosling mcp | acp | serve`** — run bundled MCP servers / ACP agent / ACP HTTP server.
- **`gosling term ...` / `tui` / `plugin` / `skills` / `review` / `completion` / `update`**.
- In-REPL **slash commands** (`session/input.rs:189+`): `/exit /quit /help /model /mode`
  `/plan /clear /compact /edit /skills /extension /builtin /prompt(s) /t /r`.

Likely users: developers running an autonomous coding/agent loop in a trusted local
shell. Trust boundaries per orientation §4 (LLM output, MCP results = untrusted).

## User journeys traced (static)

Happy path (chat), first-run-no-provider, bad/expired key, Ctrl-C mid-turn, MCP server
that won't start, mistyped slash command, `run` with missing file / missing provider,
resume nonexistent session, non-interactive configure. Traced through
`cli.rs`, `session/builder.rs`, `session/mod.rs`, `session/input.rs`, `commands/info.rs`,
`commands/configure.rs`, `commands/doctor.rs`, `signal.rs`.

---

## Findings

### PLAY-GSL-001: Mistyped / unknown slash commands are silently sent to the LLM as a prompt

Severity: Medium
Confidence: Likely
Evidence basis: simulation-reasoned
Domain: Workflow-GUI
Repair status: Repaired on 2026-07-20. Inline input parsing now converts unknown
slash commands to a local `Retry` after printing an actionable `/help` hint;
regression coverage includes typos, invalid command prefixes, arguments, and
leading whitespace.

Evidence:
- `crates/gosling-cli/src/session/input.rs:182-187` — `match handle_slash_command(&input) { Some(result) => Ok(result), None => Ok(InputResult::Message(input.trim().to_string())) }`
- `crates/gosling-cli/src/session/input.rs:504-505` and `585-586`, `719-720` — tests assert `/unknown`, `/promptxyz`, `/editfoo` return `None`.

Observed behavior (inferred):
- Any input starting with `/` that does not match a known command (e.g. `/exti`,
  `/halp`, `/doctro`, `/quti`, `/moel gpt-4o`) falls through to `None` and is then
  forwarded verbatim to the model as a user turn — a real provider request that costs
  tokens/latency/money — with **no "unknown command" feedback**.

Expected boundary:
- A `/`-prefixed token that matches no command should produce an inline
  "Unknown command: /exti (try /help)" and return `Retry`, not a billed model call.

Failure mechanism:
- The command table is a `match` with a catch-all that treats "not a command" as
  identical to "ordinary chat message"; the `/` sigil intent is discarded.

Break-it angle:
- Impatient user fat-fingers `/eixt` to leave → instead pays for a model round-trip and
  stays in the session, possibly with the model "answering" the stray `/eixt`.

Impact:
- Silent cost + confusing UX; the very characters (`/`) the user used to signal "this is
  a command, not a prompt" are ignored.

Operational impact:
- Blast radius: Workflow · Side-effect class: external API (provider call, billable)
- Reversibility: reversible · Operator visibility: silent · Rerun safety: safe

Adjacent failure modes:
- Plain-text `exit`/`quit` (no slash) is intercepted as Exit
  (`input.rs:169-178`), so a user can never send the single word "quit"/"exit" to the
  model — the inverse seam.

Recommended mitigation:
- Minimal repair: in the `None` arm, if `input.starts_with('/')` and the first token
  contains no whitespace, print an unknown-command hint and return `InputResult::Retry`;
  only fall through to `Message` for genuine prose.
- Behavior test: assert `/eixt` yields a non-`Message` result and no provider call.

Implementation assessment:
- Complexity: local_guardrail · Cost: XS · Cost drivers: 1 module, 1 test
- Nominal agent: codex
- Rationale: single function, deterministic, no cross-boundary surface.

Validation:
- Unit test on `get_input`/`handle_slash_command` boundary asserting slash-prefixed
  unknown tokens do not become `InputResult::Message`.

Non-goals:
- Redesigning the command grammar or adding fuzzy "did you mean" suggestions.

---

### PLAY-GSL-002: `gosling run -i -` panics (via `.expect`) instead of erroring gracefully on stdin read failure

Severity: Low
Confidence: Likely
Evidence basis: simulation-reasoned
Domain: Failsafe

Evidence:
- `crates/gosling-cli/src/cli.rs:1429-1438` — stdin branch:
  `std::io::stdin().read_to_string(&mut contents).expect("Failed to read from stdin");`

Observed behavior (inferred):
- Every other input error in `parse_run_input` uses `eprintln! + process::exit(1)`
  (e.g. file-not-found at `cli.rs:1440-1450`), but the stdin path `.expect(...)` — a read
  error (non-UTF-8 bytes, closed pipe) yields a Rust panic + backtrace rather than a clean
  message and exit code.

Expected boundary:
- Symmetric graceful handling: print an error and `exit(1)`, matching the sibling arms.

Failure mechanism:
- `read_to_string` fails on invalid UTF-8 or I/O error; `.expect` converts that into a
  panic instead of a handled `Result`.

Break-it angle:
- `printf '\xff\xfe' | gosling run -i -` (non-UTF-8 on stdin) → panic backtrace instead
  of "invalid input" message.

Impact:
- Ugly crash surface for a scriptable/automation entry point; harder to distinguish from
  a real bug in CI logs.

Operational impact:
- Blast radius: Local · Side-effect class: user-visible · Reversibility: reversible
- Operator visibility: UI-visible (panic) · Rerun safety: safe

Recommended mitigation:
- Replace `.expect(...)` with a match that `eprintln!`s and `process::exit(1)`,
  mirroring the file-read arm.

Implementation assessment:
- Complexity: local_guardrail · Cost: XS · Nominal agent: codex

Validation:
- Test feeding invalid-UTF-8 bytes to the stdin parser asserts a handled error, not panic.

Non-goals:
- Changing how valid stdin content is processed.

---

### PLAY-GSL-003: Latent `panic!` if any extension leaves a live `Arc<Agent>` reference at session build

Severity: Low
Confidence: Plausible
Evidence basis: simulation-reasoned
Domain: Failsafe

Evidence:
- `crates/gosling-cli/src/session/builder.rs:622-623` —
  `Arc::try_unwrap(agent_ptr).unwrap_or_else(|_| panic!("There should be no more references"))`
- Extensions are added via spawned `JoinSet` tasks holding `agent_ptr` clones
  (`builder.rs:154-193`), all joined before return — so under normal flow `try_unwrap`
  succeeds.

Observed behavior (inferred):
- If any `add_extension` implementation spawns a background task (or stores a callback)
  that retains an `Arc<Agent>` beyond the join, `try_unwrap` returns `Err` and the CLI
  **panics on session startup** rather than degrading.

Expected boundary:
- Session construction should not depend on a strong-count invariant that third-party /
  builtin extension code can violate; a stray reference should degrade or error, not panic.

Break-it angle:
- A future or third-party builtin extension that keeps an agent handle for async
  notifications turns a normal `gosling` launch into a hard panic.

Impact:
- Startup crash; no graceful message.

Operational impact:
- Blast radius: Local · Side-effect class: process · Reversibility: reversible
- Operator visibility: UI-visible (panic) · Rerun safety: safe

Recommended mitigation:
- Prefer keeping `Arc<Agent>` (adjust `CliSession::new` to take `Arc<Agent>`), or replace
  the panic with a handled error path.

Implementation assessment:
- Complexity: workflow_protocol · Cost: S · Nominal agent: codex
- Rationale: touches the agent-ownership contract of session construction; small but
  crosses the extension lifecycle boundary.

Validation:
- Test asserting session build tolerates an extension that retains a clone.

Non-goals:
- Reworking extension lifecycle broadly.

---

## Non-findings (seams checked and held)

These are the "curious/impatient user tries to break it" scenarios where the code path
looks graceful (still simulation-reasoned — not run live):

- **MCP server that won't start** → `load_extensions` collects failures and prints a
  yellow "Warning: Failed to start extension '…' … continuing without it" + a debug hint;
  the session proceeds without that extension (`builder.rs:183-218`). Good degradation.
- **Bad `--with-extension` value** → parse error becomes a yellow "Invalid … ignoring"
  warning, not a crash (`builder.rs:44-54`).
- **No provider / no model configured** → clear `render_error("No provider configured.
  Run 'gosling configure' first.")` + `exit(1)` (`builder.rs:240-253`); resume path has a
  fallback-provider attempt with a keyring troubleshooting URL (`builder.rs:526-586`).
- **Bad/expired key (resume)** → provider-create failure prints keychain guidance +
  docs URL and `exit(1)` (`builder.rs:576-585`). Not a crash. (Live behavior of the
  *runtime* provider-auth error mid-turn is separately handled, below.)
- **Ctrl-C mid-turn** → spawned `ctrl_c()` task cancels the token; the select loop drops
  the stream, rewinds conversation to the last user message, and tells the user "depending
  on the error you may be able to continue" (`session/mod.rs:1128-1134, 1303-1330`).
- **Provider/stream error mid-turn** → `handle_agent_error` + rewind + explanatory
  message ("often related to connection or authentication") (`mod.rs:1303-1317`).
- **Double-Ctrl-C to exit at the prompt** → first Ctrl-C sets `MaybeExit` and repaints
  (hint), second interrupts/exits; Ctrl-C with text clears the line
  (`session/input.rs:56-83`). Standard, discoverable.
- **`gosling info --check`** → categorized provider check (NotConfigured / InvalidModel /
  ProviderCreate / ProviderRequest) with API-key hints and timing (`commands/info.rs:57-103,
  163-199`). Strong health surface.
- **Resume a nonexistent session** → "Cannot resume session … no such session exists" +
  `exit(1)` (`builder.rs:301-312`); resume with none present → "no previous sessions found".
- **`configure` in a non-interactive shell** → bails with a helpful message about running
  it separately (`commands/configure.rs:37-42`).
- **Invalid `--output-format`** → rejected at parse time by clap `PossibleValuesParser`
  (`cli.rs:246-253`).
- **Resume in a different working dir** → interactive confirm to switch back;
  non-interactive prints a warning and stays put (`builder.rs:327-387`).
- **First-run onboarding** → "Welcome to gosling!" wizard with OpenRouter/Tetrate/manual,
  auth-failure fallbacks that `config.clear()` and re-prompt (`configure.rs:60-149`).
- **`serve` without secret** → refuses to start unless `--dangerously-unauthenticated`
  (`cli.rs:1128-1138`); wildcard `--allowed-origin` rejected (`cli.rs:1139-1150`).
- **Huge pasted input** → no explicit size cap in `get_input`, but no crash path either;
  message goes to normal context-management/truncation (orientation §5.7). Not verified.

## Required-coverage matrix (skill §"Required Scenario Coverage")

| # | Category | Status | Basis |
|---|---|---|---|
| 1 | First launch / empty state | Traced | configure.rs:60-121 |
| 2 | Primary happy path (chat) | Traced (not run) | mod.rs:482-538 |
| 3 | Happy path w/ invalid input | Traced | input.rs:182-187 (PLAY-GSL-001) |
| 4 | Save / persistence | Partial — session DB write not exercised | session_manager (not read this lens) |
| 5 | Delete / cancel / undo | Traced | `session remove` cli.rs:460-470; tool-cancel mod.rs:1187-1206 |
| 6 | Settings / preferences | Traced | configure.rs; `/mode` `/model` `/t` input.rs |
| 7 | Navigation across screens | N/A (CLI/REPL, not multi-screen) | — |
| 8 | Close & relaunch | Partial — resume traced, on-disk round-trip not run | builder.rs:301-325 |
| 9 | Interrupted workflow | Traced | mod.rs:1303-1330; signal.rs |
| 10 | File import/export | Traced (not run) | `session import/export` cli.rs:471-517 |
| 11 | Error recovery | Traced | builder.rs:526-586; mod.rs:1307-1316 |
| 12 | Edge / boundary input | Partial | slash parsing; stdin (PLAY-GSL-002) |

## Cross-lens escalations

- **PLAY-GSL-001 → `audit-workflow-gui` / `audit-operator-signal`**: silent fallthrough of
  a `/`-command to a billable model call is an operator-signal gap (silent side effect) as
  well as a GUI seam.
- **`session import` (`cli.rs:471-517`, Claude Code / Codex / Pi `.jsonl` + Nostr share
  link) → `audit-security-*` / `audit-input-output`**: untrusted-provenance session import
  is a real injection/integrity surface (orientation §5.5). Not exercised by this lens;
  flag for the security + input-output lenses.
- **`serve --dangerously-unauthenticated` / `--allowed-origin` (`cli.rs:1128-1150`) →
  `audit-security-nodejs` / `contract-internalapi`**: ACP HTTP exposure surface; playtest
  only confirmed the CLI-level guard exists, not the served endpoint's live authz.
- **PLAY-GSL-003 → `audit-resource-lifecycle` / `audit-memory-lifecycle`**: the
  `Arc::try_unwrap` panic is an ownership-lifecycle invariant worth a second look there.

## Validation Limits (what was NOT exercised)

- **No live execution of anything.** `cargo build -p gosling-cli` was attempted and
  **failed in a dependency build script that requires a network download**
  (`download_static_lib_binaries`), which the sandbox blocks; no binary was produced
  (`gosling --help` → exit 127). No `--help`, no REPL, no headless run, no provider call,
  no MCP subprocess, no TUI, no Electron desktop was observed.
- **Desktop (`ui/desktop`) and TUI (`ui/text`) were not opened.** All GUI/Ink claims would
  require a live render; none were made as Confirmed. The Ink overflow risks noted in
  `AGENTS.md` were not visually verified.
- **Persistence round-trips** (session save → close → relaunch → reopen; export → re-import)
  were not run; `session/mod.rs` beyond the response loop, `session_manager.rs`,
  `import_formats/`, and `export.rs` were not read by this lens.
- **Slash commands beyond parsing** (`/plan`, `/compact`, `/skills`, `/edit`, `/mode`
  validation) — only the parse layer (`input.rs`) was read, not their handlers/effects.
- **Boundary/adversarial inputs** (emoji, very long strings, newlines, non-UTF-8 except the
  stdin-panic reasoning) were not fed to a running binary.
- **Provider error taxonomy at runtime** (401/expired-key vs rate-limit vs offline) — the
  handling *code* was read (`mod.rs:1303-1316`, `info.rs`), but the actual rendered
  messages and recovery were not observed.
- Because build+provider are unavailable here, a follow-up pass **with a release binary and
  a configured (or mock) provider** is required to promote any finding to `Confirmed` and to
  cover categories 4/8/10 (persistence, relaunch, import/export) live.

## PLAY-GSL repair campaign closure (2026-07-20)

| Finding | Final disposition | Closure evidence |
| --- | --- | --- |
| `PLAY-GSL-001` | Repaired before this campaign and re-verified. | Commit `9ad58caf` routes unknown slash commands through local help and returns `InputResult::Retry`; `session::input::tests::test_unknown_slash_commands_are_not_messages` passed in the 229-test CLI regression suite. |
| `PLAY-GSL-002` | Repaired. | Commit `832767e2a` replaces the stdin `expect` with propagated `anyhow` errors. Tests cover valid stdin, invalid UTF-8, and underlying reader failures. A rebuilt binary returned exit code 1 with a normal error and no panic for invalid UTF-8. |
| `PLAY-GSL-003` | Closed as verified not a current defect; no source change warranted. | `load_extensions` owns each temporary `Arc<Agent>` clone in a `JoinSet` task and drains every task with `join_next` before `Arc::try_unwrap`. Extension registration receives `&self`, not the outer `Arc<Agent>`, so no clone can escape through the current API. |

Campaign result: all `PLAY-GSL-*` records are closed with implementation or verification evidence. The repair plan and command ledger are in `reports/2026-07-20-play-gsl-defect-campaign-plan.md` and `reports/2026-07-20-play-gsl-defect-campaign-session-log.md`.
