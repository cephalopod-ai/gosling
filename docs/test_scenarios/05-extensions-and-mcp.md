# 05 — Extensions and MCP

Extensions (bundled and MCP) are how gosling gains tools. These cards cover
enable/use, add, fail-closed, and remove — CLI and Desktop where both exist.

---

### EX-01 — Enable bundled extension and use a tool
- Goal: a bundled extension (e.g. Developer) can be enabled and exercised.
- Category: happy path
- Preconditions: configured provider; `GOSLING_MODE` not `chat` (tools allowed); disposable cwd.
- Steps:
  1. Enable Developer (or already-on default) via `gosling configure` / Desktop Extensions.
  2. In session: `Create a file named playtest-ext.txt containing hello-ext in the current directory.`
  3. Verify the file on disk; disable the extension; ask to create `playtest-ext-2.txt`.
- Expected: tool calls visible with status; file created when enabled; after disable, agent cannot use those tools (refuses or lacks tools) without crashing the session.
- Observe: tool permission prompts under approve/smart modes; extension loading toasts on Desktop.
- Variations: enable more than ~25 tools total and note any performance/warning (docs recommend &lt;25).

### EX-02 — Add streamable HTTP / stdio MCP extension
- Goal: operator can add an MCP server and call one tool.
- Category: happy path / files
- Preconditions: a **test** MCP server (local stdio fixture or disposable HTTP MCP). Prefer something you own. No production SaaS tokens unless sandbox.
- Steps:
  1. CLI: `gosling mcp add …` / `gosling session --with-extension '…'` / `--with-streamable-http-extension 'http://…'` as documented — or Desktop install UI.
  2. `gosling mcp list` (or Extensions view) shows it enabled.
  3. In session, ask the agent to use a simple tool from that server.
- Expected: extension appears in list; tool discovery succeeds; one successful tool round-trip; config persists after relaunch.
- Observe: timeout defaults; env vars for stdio commands not printed in full if secret-bearing.
- Variations: `--no-profile` with only CLI-specified extensions — default profile extensions stay off.

### EX-03 — Broken MCP extension fails closed
- Goal: a bad extension does not take down the whole agent.
- Category: recovery / invalid input
- Preconditions: ability to add a deliberately broken extension (bad command, wrong URL, immediate exit).
- Steps:
  1. Add extension with command `false` or URL `http://127.0.0.1:1/mcp-does-not-exist`.
  2. Start a session; send a normal chat message that does not need that extension.
  3. Attempt to use a tool from the broken extension.
- Expected: session still starts; error is named (spawn failed / connection refused); other extensions still work; no crash loop; Desktop shows a recoverable error toast/state.
- Observe: does listing mark it unhealthy? Can the user remove it after failure?
- Variations: extension that hangs on startup — session should not block forever without feedback.

### EX-04 — Remove extension; session no longer offers tools
- Goal: removal is durable and takes effect cleanly.
- Category: delete-undo / settings
- Preconditions: a non-critical test extension installed (from EX-02) or a toggleable bundled extension.
- Steps:
  1. Confirm tools work.
  2. Remove/disable via CLI `gosling mcp remove` / configure / Desktop.
  3. Restart session (and fully relaunch app once).
  4. Ask agent to use the removed tool by name.
- Expected: list no longer includes it; new sessions lack tools; open sessions either refresh tool list or document that restart is required — no half-registered tool that errors opaquely on every turn.
- Observe: allowlist (`GOSLING_ALLOWLIST`) if configured — unauthorized install should refuse.
