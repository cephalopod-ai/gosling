# 01 — First Run and Lifecycle

Nothing else matters if a new operator cannot get from a clean install to a
working agent. These scenarios confirm configure, health checks, and
config-file tolerance. Run these first with `GOSLING_PATH_ROOT` pointed at a
new disposable absolute directory.

---

### LC-01 — Fresh install to first successful reply (primary happy path)
- Goal: a brand-new operator reaches a usable agent and gets one real response.
- Category: happy path / first launch
- Preconditions: source build or installed CLI; disposable home; at least one reachable provider (prefer local Ollama or a cheap/test key).
- Steps:
  1. `source bin/activate-hermit && cargo build -p gosling-cli` (or use a release binary).
  2. `gosling configure` — set provider + model; complete any auth prompts.
  3. From a disposable project directory: `gosling session` (or Desktop launch + new chat).
  4. Send: `Reply with exactly the word PONG and nothing else.`
- Expected: configure exits successfully without stack traces; session starts; the completed assistant text is exactly `PONG` after trimming line endings; the run reaches idle within the execution-contract deadline.
- Observe: does configure tell the operator what to do next? Does `gosling info` show the provider that actually answered? Are missing engines hidden rather than shown broken?
- Variations: Desktop path via `just run-ui` / installed app if available; second `gosling configure` (idempotency — must not clobber secrets silently).

### LC-02 — First-launch empty / unconfigured states
- Goal: see what a new user sees before any provider is configured.
- Category: empty state / recovery
- Preconditions: fresh disposable home with no provider credentials.
- Steps:
  1. Run `gosling` / `gosling session` with no prior configure.
  2. On Desktop (if in scope), open the app and walk onboarding without completing provider setup, then cancel/back where possible.
  3. Run `gosling info`, `gosling info --check`, and `gosling doctor`.
- Expected: unconfigured state is explicit and actionable ("run gosling configure" / provider picker), not a raw panic, empty hang, or opaque network error.
- Observe: does doctor/info distinguish "not configured" from "configured but unreachable"?
- Variations: set `GOSLING_PROVIDER` to a nonsense name via env and re-run — named failure preferred.

### LC-03 — `info` / `doctor` honesty
- Goal: health commands report state that matches reality.
- Category: happy path / error clarity
- Preconditions: LC-01 completed once; disposable home still in use.
- Steps:
  1. Run `gosling info`, `gosling info -v`, and `gosling info --check`; record each exit code and stdout/stderr separately.
  2. Run `gosling doctor` and record its exit code.
  3. Break the active provider temporarily (revoke key, stop Ollama, or set a bad env override); re-run doctor/info; restore.
- Expected: healthy run lists version, config path, session storage, and enabled extensions truthfully; broken provider produces a clear fail signal; exit codes distinguish success from failure.
- Observe: does verbose mode leak secrets to the terminal? (secrets in stdout = finding.)
- Variations: run info while a long session is active in another terminal — must still complete.

### LC-04 — Config hand-edit then relaunch
- Goal: users edit `~/.config/gosling/config.yaml` directly; verify tolerance.
- Category: invalid input / boundary / recovery
- Preconditions: gateway/CLI stopped relative to edits; back up the disposable config first.
- Steps:
  1. Make a benign valid edit to a key advertised by the current build (for example `GOSLING_CLI_SHOW_COST`); start a session and confirm the value is reflected by its documented behavior.
  2. Introduce a YAML syntax error (bad indent); attempt `gosling session` / Desktop launch.
  3. Restore valid YAML but with an unknown key and a wrong-typed value (string where number expected); start again.
- Expected: valid edits take effect or are clearly ignored with defaults; broken YAML produces an error naming the file/problem — not silent overwrite of the user's file and not an opaque crash; unknown/wrong-typed keys are ignored or reported, never state-corrupting.
- Observe: is the broken file preserved for the user to fix?
- Variations: empty `config.yaml`; zero-byte file; only comments.
