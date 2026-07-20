# 02 — Chat Sessions (CLI and Desktop)

The primary value proposition: send a message, watch the agent work, get a
result, keep the conversation. Covers happy path, input seams, interrupt,
persistence, isolation, and slash commands.

---

### CH-01 — First message to first response (primary happy path)
- Goal: one message in, one useful agent response out, on CLI and/or Desktop.
- Category: happy path
- Preconditions: provider configured (LC-01); disposable project cwd.
- Steps:
  1. CLI: `gosling session` → send `List the files in the current working directory in a short bullet list.`
  2. Desktop (if available): new chat in the same directory; same prompt.
- Expected: message appears; streaming/progressive output where designed; session reaches a completed/idle state; tools (if used) show status; response is grounded in the cwd.
- Observe: is it clear which provider/model answered? Does the session get a sensible title in the list?
- Variations: follow-up that depends on the first answer; confirm context is retained.

### CH-02 — Composer / REPL input seams
- Goal: the input surface tolerates realistic messy input.
- Category: invalid input / boundary
- Preconditions: an open session (CLI or Desktop).
- Steps / Variations (each is a send attempt):
  1. Empty message; whitespace-only message.
  2. Very long message (thousands of characters, e.g. a pasted log).
  3. Emoji, accented text, non-Latin scripts (`日本語`, `العربية`), RTL text.
  4. Markdown/HTML-ish text (`<script>alert(1)</script>`, backticks, `# heading`) — rendering check, not an exploit.
  5. Multi-line input with newlines; paste with trailing whitespace.
  6. Rapid double-send of the same text.
- Expected: empty/whitespace sends are prevented or harmless; long input neither freezes nor truncates silently without feedback; unicode round-trips into history; markup renders inert; double-send does not duplicate the user turn or fork two agent runs for one intent.
- Observe: Desktop markdown rendering of user vs assistant content; CLI wrap/scroll behavior.

### CH-03 — Interrupt mid-run then continue
- Goal: stop/cancel mid-run behaves like a user expects.
- Category: interruption
- Preconditions: session able to start a long task (tooling or long generation).
- Steps:
  1. Send a deliberately long task (`Count slowly from 1 to 200, narrating each number on its own line.` or a recursive file walk).
  2. Interrupt (Ctrl-C in CLI per product norms; Desktop stop control).
  3. Observe session state; send `Say READY` as a new turn.
- Expected: work stops promptly; session lands in a clear stopped/idle state (not forever "running"); session remains usable; follow-up produces `READY` without requiring a brand-new session unless documented.
- Variations: interrupt twice quickly; navigate away mid-stream on Desktop and return; close the Desktop window mid-stream and reopen the session.

### CH-04 — Session persistence across relaunch
- Goal: sessions are truly persisted, not only in memory.
- Category: persistence / relaunch
- Preconditions: several sessions with multi-turn history; at least one with an attachment if Desktop supports it.
- Steps:
  1. Note session ids/names and a unique marker string in history.
  2. Fully quit CLI sessions and/or quit Desktop.
  3. Relaunch; list sessions (`gosling session list` / Desktop history); reopen each.
- Expected: list, titles, full history (or documented window), and attachments survive; no duplicate or ghost sessions; interrupted mid-run sessions show an honest terminal state.
- Observe: timestamps not re-stamped to "now"; working directory still correct on resume.

### CH-05 — Parallel sessions isolation
- Goal: two concurrent sessions do not bleed context or tool side effects into each other.
- Category: concurrency
- Preconditions: ability to open two CLI sessions or two Desktop chats; two distinct disposable directories A and B.
- Steps:
  1. Session A in dir A: ask `What is your cwd? Create a file named only-in-A.txt with content A.`
  2. Session B in dir B: ask `What is your cwd? Create a file named only-in-B.txt with content B.`
  3. Confirm filesystem results; ask each session to read the other's file.
- Expected: each session reports its own cwd; files land only in the intended directory; neither session silently uses the other's context as if it were local unless tools explicitly reach across (and then paths must be clear).
- Observe: Desktop header/workspace pin matches each session.

### CH-06 — Slash-command discoverability and typos
- Goal: in-session slash commands help the user; typos fail safely.
- Category: invalid input / navigation
- Preconditions: interactive CLI session (Desktop slash/command palette if present).
- Steps:
  1. Run `/help` (and any documented help surface).
  2. Exercise a few real commands: `/model`, `/mode`, `/skills`, `/clear` or `/compact` as documented — prefer non-destructive first.
  3. Type a typo that looks like a command: `/halp`, `/eixt`, `/moel`.
- Expected: help lists real commands; real commands do what docs say; unknown `/…` tokens either give an "unknown command" hint **or** are documented as forwarded to the model — silent billable mis-routes without feedback are a Medium+ finding (known static suspicion: unknown slash → model prompt).
- Observe: does plain `exit`/`quit` without slash leave the session? Can the user send the word "exit" to the model if needed?
- Variations: `/prompt` with missing args; empty `/`.
