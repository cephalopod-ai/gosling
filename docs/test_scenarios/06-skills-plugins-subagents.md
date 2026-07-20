# 06 — Skills, Plugins, and Subagents

Skills and plugins extend agent behavior; subagents isolate parallel work.
Use disposable catalogs/repos and cheap models.

---

### SK-01 — Skills list and invoke
- Goal: operator can list skills and successfully use one.
- Category: happy path
- Preconditions: at least one skill available (bundled, local catalog via `GOSLING_SKILL_CATALOGS`, or installed skill). Prefer offline/local.
- Steps:
  1. `gosling skills list` (and Desktop Skills view if available).
  2. In session: `/skills` or natural language that should load a known skill.
  3. Complete a small task that depends on that skill's instructions.
- Expected: list is non-empty or honestly empty; invocation loads skill content into context (visible or effective); task completes without crash.
- Observe: Goose compatibility fallback — if catalogs resolve through the Goose adapter, note source; broken catalog path should error clearly (see `documentation/GOOSE_COMPATIBILITY.md`).
- Variations: empty skill name; skill path that does not exist.

### SK-02 — Plugin install/update from git
- Goal: git-backed plugin install and update work without corrupting the plugin tree.
- Category: files / recovery
- Preconditions: network access to a **small public test plugin repo** you trust, or a local git remote. If none: Not executed — environment unavailable.
- Steps:
  1. `gosling plugin install <git-url>` (exact subcommand per `--help`).
  2. List/confirm install; start a session that exercises the plugin if it exposes one.
  3. `gosling plugin update …` (or re-install); list again.
- Expected: install is idempotent or clearly versioned; update does not leave a half-checked-out tree; failure mid-clone is recoverable with a retry; secrets in URLs are not logged in full.
- Observe: concurrent update while a session is open (light version of SX skill thrash).

### SK-03 — Subagent parallel fan-out
- Goal: parallel subagents complete without poisoning the parent session.
- Category: concurrency / happy path
- Preconditions: permission mode that allows subagents (docs: autonomous/default `auto`; disabled in manual/smart/chat-only); cheap model.
- Steps:
  1. In a disposable directory, prompt: `Create three tiny files a.txt, b.txt, c.txt with contents A, B, C respectively, using subagents in parallel.`
  2. Watch parent UI/CLI for subagent tool indicators.
  3. Verify files; send a parent follow-up `Summarize what the subagents did in one sentence.`
- Expected: files exist with correct contents; parent remains responsive; subagent tool calls are attributable; a failing subagent (if forced) does not wipe successful siblings' results (docs: parallel failures return only successful results).
- Observe: 5-minute timeout behavior if a subagent is stuck — parent should not hang forever without signal.
- Variations: sequential "first…then" wording; request a subagent with only the developer extension.
