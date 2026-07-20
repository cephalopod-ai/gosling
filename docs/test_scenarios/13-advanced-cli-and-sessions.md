# 13 — Advanced CLI and Session Workflows

These cards cover advertised CLI behavior omitted by the core smoke: precise
resume selection, forks, editor integration, diagnostics, project shortcuts,
terminal integration, TUI launch, and review assembly.

---

### AC-01 — Resume selection by recency, name, and ID
- Goal: every resume selector opens the intended session and rejects ambiguity.
- Category: persistence / navigation
- Preconditions: three sessions with unique names, IDs, cwd paths, and marker turns; record `gosling session list --format json`.
- Steps: resume the newest with `gosling session --resume`; resume each other session once with `--resume --name <name>` and `--resume --session-id <id>`; try an unknown name/ID and `--session-id` without `--resume`.
- Expected: each valid selector opens only its matching marker history; invalid combinations exit non-zero before creating a session; recency order changes only after a successful resume.
- Observe: title, cwd, model, and last-used timestamp before and after each command.

### AC-02 — Fork creates an independent history
- Goal: `--fork` copies history without linking later mutations.
- Category: persistence / files
- Preconditions: a completed source session containing marker `SOURCE-BASE`; record source ID and export hash.
- Steps: run `gosling session --resume --session-id <id> --fork`; add `FORK-ONLY`; resume source and add `SOURCE-ONLY`; export both.
- Expected: IDs differ; both contain `SOURCE-BASE`; only the fork contains `FORK-ONLY`; only the source contains `SOURCE-ONLY`; source export before forking is unchanged except documented metadata.
- Observe: whether fork name and cwd make lineage understandable.

### AC-03 — External-editor resume and failure handling
- Goal: `--edit` safely applies intentional edits and survives editor failure.
- Category: files / recovery
- Preconditions: disposable session; deterministic test editor command selected with `GOSLING_PROMPT_EDITOR`; untouched export retained as an oracle.
- Steps: resume with `--edit` and have the editor replace one marker; cancel once without writing; repeat with an editor command that exits non-zero; combine `--fork --edit` once.
- Expected: saved valid edits affect only the selected target; cancel and editor failure leave the original byte-equivalent; fork+edit changes only the fork; parse errors identify the temporary edit artifact without destroying history.
- Observe: temporary files are removed or retained with an explicit recovery path.

### AC-04 — Session diagnostics artifact
- Goal: diagnostics are useful, bounded, and safe to share.
- Category: files / error clarity
- Preconditions: one successful and one failed disposable session; known fake secret marker placed in provider config and tool output.
- Steps: run `gosling session diagnostics --session-id <id> --output <path>` for both; run without output if supported; inspect size, permissions, and content.
- Expected: command exits deterministically; artifact identifies session/build and relevant failure; raw credentials and fake secret marker are absent or redacted; missing ID fails without creating a misleading artifact.
- Observe: whether absolute personal paths are minimized or clearly documented.

### AC-05 — Session list filters, ordering, and JSON
- Goal: session inventory is stable enough for scripts.
- Category: boundary / files
- Preconditions: at least six sessions across two cwd values with known creation and last-used order.
- Steps: run list in text and JSON; test `--ascending`, `--working_dir`, `--limit 0`, `--limit 2`, a missing cwd, and a limit larger than the store.
- Expected: JSON parses with one object per returned session; filtering never leaks other cwd entries; order matches the documented date field; limits are exact; empty results are successful and machine-readable.
- Observe: names and paths containing spaces, unicode, and terminal control-like text render inert.

### AC-06 — Recent project discovery and launch
- Goal: `project` and `projects` use real session history without choosing stale paths.
- Category: navigation / recovery
- Preconditions: sessions created from two existing fixture directories plus one directory removed after use; safe way to observe the platform opener.
- Steps: run `gosling projects`; activate sessions in a known order; run `gosling project`; remove the newest directory and repeat.
- Expected: projects are unique and ordered by documented recency; project opens the latest existing path; a missing path produces an actionable error or explicit fallback and never opens `$HOME` silently.
- Observe: paths with spaces and symlinks; CLI exit code after handing off to the opener.

### AC-07 — Terminal shell initialization is non-destructive
- Goal: generated shell integration is valid and does not edit shell startup files itself.
- Category: files / boundary
- Preconditions: disposable shell home or isolated shell process; bash, zsh, fish, nu, or PowerShell as locally available.
- Steps: capture `gosling term init <shell>` with and without `--default`; syntax-check/source it in isolation; hash real shell rc files before and after; request an unsupported shell.
- Expected: stdout contains sourceable code only; stderr carries diagnostics; real rc hashes do not change; aliases/functions resolve to gosling; unsupported input exits non-zero with allowed values.
- Observe: generated code quoting when binary/config paths contain spaces.

### AC-08 — Terminal session identity and isolation
- Goal: each terminal keeps a stable session without cross-terminal context bleed.
- Category: concurrency / persistence
- Preconditions: two isolated shells initialized with distinct terminal-session identity and a cheap provider.
- Steps: in shell A run `gosling term run remember TERM-A`; in B use TERM-B; call `term info` in each; ask each to recall its marker; restart shell A with the same identity and once with a new identity.
- Expected: A and B have distinct session IDs/context; `term info` is compact and non-blocking; same identity resumes A; new identity creates a session without either marker.
- Observe: token/model display after failed and successful turns.

### AC-09 — TUI resolution, launch, and dependency failure
- Goal: `gosling tui` selects the documented implementation and reports missing runtimes cleanly.
- Category: navigation / recovery
- Preconditions: local `ui/text/dist/tui.js` if built, or network-approved npm fallback; disposable root.
- Steps: launch with the local script path; pass a harmless forwarded argument; set `GOSLING_TUI_SCRIPT` to a missing path; run with Node/npx hidden from PATH in an isolated shell.
- Expected: valid TUI reaches a usable initial screen and exits cleanly; forwarded arguments arrive once; missing candidates produce a bounded prerequisite error with no partial install in the gosling root.
- Observe: terminal restoration after Ctrl-C, narrow dimensions, and non-interactive stdin.

### AC-10 — Review dry-run discovery and scoping
- Goal: `gosling review --dry-run` assembles the intended diff and checks without contacting a provider.
- Category: files / boundary
- Preconditions: disposable git repo with one modified file, one untracked file, `.agents/checks/` fixtures, and nested `.agents/REVIEW.md` scopes.
- Steps: run default dry-run; repeat with an explicit range, `--files`, `--check-filter`, `--check-scope`, `--checks-only`, custom `--prompt`, and an invalid range.
- Expected: dry-run makes no provider request and no worktree change; included files/checks match each selector exactly; scoped prompts apply only below their directory; invalid range exits non-zero with git context.
- Observe: secret-like diff text is not copied into logs beyond the explicit dry-run output.
