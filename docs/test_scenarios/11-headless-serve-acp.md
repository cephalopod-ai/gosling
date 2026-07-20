# 11 — Headless Run, Serve, and ACP

Non-interactive and server surfaces are how CI, IDE clients, and Desktop's
backend talk to gosling. They must fail legibly and shut down cleanly.

---

### HS-01 — Headless `run` with budgets
- Goal: one-shot runs respect turn/tool budgets and exit codes.
- Category: boundary / recovery
- Preconditions: provider configured; disposable cwd.
- Steps:
  1. `gosling run -t "Say HI" -q` — success path.
  2. `gosling run -t "Keep calling tools forever to explore the repo" --max-turns 2` (and/or `--max-tool-repetitions 2`).
  3. `gosling run -t "hi" --provider not-a-real-provider`.
- Expected: success exits 0 with answer; budgeted run stops with a clear reason and non-success or documented success-with-truncation; bad provider fails fast with a named error; no orphan child processes after exit (`ps` spot-check).
- Observe: JSON output still valid when the run hits a budget.
- Variations: `--system "Answer only in haiku"` influences the reply; `--with-builtin developer` on a fresh `--no-profile` run.

### HS-02 — `gosling serve` lifecycle
- Goal: ACP HTTP/WebSocket server starts, serves, and stops without orphaning.
- Category: happy path / recovery
- Preconditions: free local port; disposable home; know default bind from `--help`.
- Steps:
  1. Start `gosling serve` with an explicit port (e.g. high ephemeral port).
  2. Hit a documented health/ready endpoint or open Desktop against it if that is the product path; otherwise use a minimal WebSocket/HTTP probe from docs.
  3. Start a second serve on the **same** port.
  4. Stop the first (Ctrl-C / SIGTERM); confirm port is released; restart once.
- Expected: first start binds and logs the URL/port; second start fails with port-in-use clarity; graceful stop leaves no listener; restart works.
- Observe: auth/allowlist on serve if documented — unauthenticated probe should fail closed when auth is on.
- Variations: `GOSLING_SANDBOX=true` Desktop path is separate; do not conflate with CLI serve unless docs say they share code.

### HS-03 — `gosling acp` stdio smoke
- Goal: ACP agent on stdio speaks a coherent initialize handshake.
- Category: happy path / boundary
- Preconditions: `gosling acp --help`; ability to send a minimal ACP initialize message (from `guides/acp-clients.md` or SDK types). If no fixture client: use a short script or mark partial.
- Steps:
  1. Launch `gosling acp` (stdio).
  2. Send a minimal valid initialize/new-session sequence per current ACP contract.
  3. Send an invalid JSON line / wrong method; then a valid cancel or shutdown.
- Expected: valid handshake gets a structured response; invalid input does not kill the process without an error response if the protocol requires one; clean shutdown on stdin close or documented exit.
- Observe: Desktop's embedded backend vs CLI `acp` drift (versions, methods).
- Variations: spawn acp with missing provider — error should surface to the client, not hang.
