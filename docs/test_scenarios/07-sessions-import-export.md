# 07 — Session List, Remove, Import, Export

Session portability and cleanup must be safe under operator impatience
(wrong id, half-finished export, import of odd files).

---

### SE-01 — Session list, rename, remove
- Goal: lifecycle management of stored sessions is predictable.
- Category: delete-undo / persistence
- Preconditions: ≥3 sessions with distinct names/markers.
- Steps:
  1. `gosling session list` (and Desktop Session History).
  2. Rename one session (Desktop edit description / CLI if supported) to a 200-char boundary string and to a unicode name.
  3. Remove one disposable session by id; attempt remove of a non-existent id; attempt interactive remove cancel if prompted.
  4. Relaunch; confirm removed stays gone and rename stuck.
- Expected: list matches reality; rename enforces documented length; remove is confirmed when interactive; bad ids error without deleting the wrong session; Desktop and CLI agree on membership.
- Observe: removing a session still open in another window — honest close vs zombie view.
- Variations: remove by regex if supported; empty list on fresh home.

### SE-02 — Export session
- Goal: export produces a usable artifact without secrets leakage.
- Category: files / persistence
- Preconditions: a completed multi-turn session, ideally with a tool call.
- Steps:
  1. `gosling session export` (flags per `--help`) to a path under the disposable home.
  2. Open the export; spot-check messages and metadata.
  3. Export while a session is mid-run if the CLI allows.
- Expected: export completes; content matches history; mid-run either snapshots cleanly or refuses with a clear message; API keys / raw secrets from tools are redacted or absent.
- Observe: file permissions on export; overwrite behavior if the target exists.
- Variations: Desktop export / download path if present.

### SE-03 — Import session (JSON / foreign jsonl)
- Goal: import accepts supported formats and rejects garbage safely.
- Category: files / invalid input
- Preconditions: a prior export from SE-02; optionally a small Claude Code / Codex / Pi `.jsonl` fixture if available.
- Steps:
  1. Import the good export into the disposable home; list sessions; open and continue with one turn.
  2. Import a truncated JSON file; a zero-byte file; a random binary renamed `.json`.
  3. Import the same good file twice.
- Expected: good import becomes a usable session; bad files fail with a named parse/format error and do not corrupt the session store; double import either creates a distinct copy or rejects duplicates — never merges histories destructively without saying so.
- Observe: encrypted Nostr share link path — Not executed unless test material exists.
- Variations: import while Desktop is open and watching the session list (refresh honesty).
