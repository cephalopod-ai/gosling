# 09 — CLI Surface Robustness

A CLI that panics on a typo fails the same user a broken button fails.
Lifecycle/configure are in 01; this file covers help, misuse, `run`, and
completion.

Run from a source checkout (`./target/debug/gosling …`) and, if a packaged
install exists, spot-check the same commands for drift.

---

### CL-01 — Help and discoverability
- Goal: a user can discover the CLI from the CLI.
- Category: happy path / navigation
- Steps:
  1. `gosling` with no arguments (configured vs unconfigured homes — note both).
  2. `gosling --help`; `gosling --version`.
  3. `--help` on every advertised subcommand: `configure`, `info`, `doctor`, `mcp`, `acp`, `serve`, `session`, `project`, `projects`, `run`, `skills`, `plugin`, `update`, `term`, `tui`, `completion`, and `review`.
- Expected: every advertised command has help; help matches behavior (spot-check two); exit code 0 for help/version; no panic on help.
- Variations: `gosling session` with no subcommand — usage or interactive start per design, not a crash.

### CL-02 — Unknown commands and bad flags
- Goal: misuse fails politely.
- Category: invalid input
- Steps / Variations:
  1. `gosling strat` (typo) — unknown-command error; ideally a suggestion; never a stack trace.
  2. `gosling session --bogus-flag`; `gosling run` with both `-t` and `-i` if mutually exclusive.
  3. `gosling mcp remove` with no name; empty-string args where a path is required.
  4. Unicode / emoji in `--name`: `gosling session -n "日本語-🧪"`.
- Expected: clear one-line errors with usage pointers; consistent non-zero exit codes; no partial config corruption from failed invocations.
- Observe: does `run` without input hang waiting on stdin? If so, is that documented?

### CL-03 — `gosling run` one-shot formats
- Goal: headless run is scriptable and format-honest.
- Category: happy path / boundary
- Preconditions: provider configured; disposable cwd.
- Steps:
  1. `gosling run -t "Reply with exactly PONG" -q` — only model text on stdout (quiet).
  2. `gosling run -t "Reply with PONG" --output-format json` — parse with a JSON tool.
  3. `gosling run -t "Reply with PONG" --output-format stream-json` — line-delimited JSON or documented stream.
  4. `gosling run -i /path/to/missing.txt`; `gosling run -i -` with piped `echo hi`.
- Expected: quiet mode suppresses chrome; json/stream-json are machine-parseable; missing file errors clearly; stdin works; non-zero exit on hard failure.
- Variations: `--max-turns 1` on a tool-heavy prompt; `--provider` / `--model` overrides for one run.

### CL-04 — Completion generation
- Goal: shell completion scripts generate without error.
- Category: happy path / files
- Steps:
  1. `gosling completion zsh` / `bash` / `fish` / `nu` (as supported) → capture stdout.
  2. Ensure exit 0 and non-empty script-like output.
  3. Optionally install into a disposable shell rc snippet and tab-complete `gosling se<TAB>`.
- Expected: scripts generate; no secrets embedded; invalid shell name fails cleanly.
- Observe: manpage generator path is out of band unless packaged — Note only.
