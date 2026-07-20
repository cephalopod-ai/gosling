# 15 — Context and Filesystem Boundaries

Project instructions are powerful only when their source and scope are
predictable. These cases use unique sentinel phrases and filesystem evidence;
model prose alone is not enough where a tool or protocol trace can corroborate.

---

### CX-01 — Root `AGENTS.md` instruction loading
- Goal: default project instructions apply at session start from the correct root.
- Category: files / happy path
- Preconditions: disposable git project with root `AGENTS.md` requiring exact sentinel `ROOT-RULE-731` in a response; no other context files.
- Steps: start new sessions from project root and a child directory; ask for the sentinel and source; start once from a sibling outside the project.
- Expected: in-project sessions follow the rule once; outside session does not receive it; UI/debug context identifies the loaded path when that visibility is supported; file content is not duplicated per turn.
- Observe: behavior with an empty and unreadable `AGENTS.md`.

### CX-02 — Nested context loads only when scoped
- Goal: nested `AGENTS.md`/`.goslinghints` enter context only after work enters their directory.
- Category: boundary / files
- Preconditions: root and two sibling subdirectories each with distinct sentinel rules; a file in each subdirectory.
- Steps: start at root; query root sentinel; access a file in child A and query A/B sentinels; then access B; start a fresh session directly in A.
- Expected: root applies initially; A appears after A is accessed without leaking B; B appears only after B access; direct-A session receives root plus A in deterministic documented order.
- Observe: repeated access does not append duplicate copies or unbounded tokens.

### CX-03 — Custom context filenames and ordering
- Goal: `CONTEXT_FILE_NAMES` accepts valid arrays and rejects malformed configuration safely.
- Category: settings / invalid input
- Preconditions: project containing `AGENTS.md`, `.goslinghints`, `.cursorrules`, and `project_rules.txt`, each with a unique non-conflicting sentinel.
- Steps: run with one filename, several ordered filenames, an empty array, duplicate entries, malformed JSON, a non-string element, and a missing filename.
- Expected: only configured existing files load, each once, in documented precedence; empty selection loads none if supported; malformed values fail at startup or use a clearly warned default without rewriting config.
- Observe: env value versus config value precedence.

### CX-04 — Ignored and sensitive files stay out of context
- Goal: automatic context discovery does not ingest ignored secrets.
- Category: files / authorization
- Preconditions: git repo with ignored `secret.env` containing unique fake secret, non-ignored source, symlink to the secret, and context instructions that ask the agent to summarize the tree.
- Steps: ask for project context without naming the secret; search/read the parent directory through normal agent behavior; explicitly request the ignored file and separately the symlink under each permission mode.
- Expected: fake secret never appears through automatic context; explicit access follows tool permissions and ignored-file policy with a visible action; denial leaks neither content nor partial value.
- Observe: diagnostics/export/logs for the fake marker.

### CX-05 — Persistent instructions refresh between turns
- Goal: persistent instructions are re-read while session-start hints remain snapshot/lazy scoped as documented.
- Category: persistence / settings
- Preconditions: configured persistent instruction sentinel `PERSIST-A` and project hint sentinel `HINT-A` in a new session.
- Steps: get one response; edit persistent instruction to `PERSIST-B` and hint to `HINT-B`; send another turn without restart; start a new session.
- Expected: existing session uses `PERSIST-B` on the next turn; existing hint behavior matches documented snapshot/lazy semantics and is not silently half-reloaded; new session uses both B values.
- Observe: deleted or temporarily malformed instruction file produces a named warning without erasing the session.

### CX-06 — `GOSLING_PATH_ROOT` provides complete isolation
- Goal: the root override contains every gosling-owned state write for a pass.
- Category: files / persistence
- Preconditions: two empty temp roots A/B; hashes or directory snapshot of the operator's normal gosling locations; no Desktop process already running.
- Steps: under A configure provider, create session/workspace/extension, and change settings; repeat minimal setup under B; relaunch each root; compare normal paths and roots.
- Expected: A and B cannot see each other's entities; all expected state is under the active root except documented OS facilities such as Keychain; normal gosling files are unchanged; switching roots is reversible.
- Observe: Electron cache/log locations and secret-store service/account names.

### CX-07 — `--no-session` leaves no resumable history
- Goal: stateless automation does not create a durable session accidentally.
- Category: files / persistence
- Preconditions: record `session list --format json` and state-tree hash in a clean root.
- Steps: run one successful, one provider-failed, and one cancelled `gosling run --no-session`; search list/state for unique prompt markers; relaunch and repeat search.
- Expected: no run is resumable or listed; prompt/response content is absent from durable session storage; necessary bounded logs may record metadata but not full secret-like marker; exit/output behavior otherwise matches normal run.
- Observe: temp files removed after abnormal termination.

### CX-08 — Instruction file and stdin boundaries
- Goal: `run -i` handles encoding, size, and stream termination predictably.
- Category: boundary / files
- Preconditions: instruction fixtures for UTF-8 with/without trailing newline, CRLF, zero bytes, 1 MiB text, invalid UTF-8, and a named FIFO/slow pipe if safe.
- Steps: run each with `-i <file>`; pipe equivalent input with `-i -`; close stdin normally and once mid-codepoint; invoke `-i` and `-t` together.
- Expected: supported UTF-8 forms preserve markers; empty/binary/invalid input is rejected clearly or handled per docs; large input is bounded without silent truncation; stdin EOF exits; conflicting sources fail before provider use.
- Observe: filenames with spaces, unicode, and a leading hyphen.

### CX-09 — Code-execution runtime disable gate
- Goal: `GOSLING_CODE_EXECUTION_RUNTIME=disabled` prevents Code Mode runtime loading while leaving normal chat usable.
- Category: authorization / settings
- Preconditions: provider configured; Code Mode and one ordinary non-code tool available; disposable cwd.
- Steps: start a new process with runtime disabled; request Code Mode explicitly; request an ordinary allowed tool; switch env to enabled without restarting, then restart and retry.
- Expected: disabled process refuses Code Mode with the named setting and executes no code-runtime side effect; ordinary allowed behavior still works; change takes effect only on a new process; enabled retry follows normal approval policy.
- Observe: extension/tool listings do not advertise an unusable runtime without explanation.

### CX-10 — One-shot system prompt remains scoped
- Goal: `run --system` influences only that invocation and does not contaminate saved defaults or other sessions.
- Category: settings / persistence
- Preconditions: two marker system prompts with deterministic response format; baseline config/export retained.
- Steps: run with system marker A; run without `--system`; run concurrently with marker B; resume A if a session was created; start an unrelated interactive session.
- Expected: A and B affect only their own runs; baseline and unrelated session do not contain either marker; resuming A follows documented persistence of run-level system context; config hash remains unchanged.
- Observe: system text exposure in exports/diagnostics and whether that matches documentation.
