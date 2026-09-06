# Working-directory scope prompts for read-only shell diagnostics (2026-09-06)

- Date: 2026-09-06
- Task: the desktop chat "this is a macos" (session `20260906_50`, workspace `mac-control`,
  Autonomous mode, folders `/Users/eric` and `/Users/eric/Documents`) stopped three times on an
  approval card reading `"shell" would modify /dev/null, which is outside this workspace
  session's folders`. The operator's standing rule is that Autonomous sessions do not prompt
  for benign work; find out why the `working_dir_scope` inspector still does.

## Evidence

Server log `~/.local/state/gosling/logs/cli/2026-09-06/*.log` records three
`inspection result applied ... ALERT` events from `working_dir_scope` for request ids
`call_SwHbwpw9MwKDBkn1nCM86PMM`, `call_9RtsSnQRIHvZxqSQ0ziQXbiX`, and
`call_1Fn3iPEk5xwRw9muzHtk2cEt`. Their commands (read from `sessions.db`) are diagnostic
pipelines: `sample ... -file <Documents path> >/dev/null 2>&1`, `head`/`tail`/`find`/`lsof`/`ps`
over `~/Library` log files with `2>/dev/null`, and `find ... 2>/dev/null | head` plus
`grep ... 2>/dev/null | head` after `cd` into a project under `/Users/eric`.

A throwaway probe test fed the three real commands to `mutation_paths` with the session's
folders. Every one produced `/dev/null` (or `/dev/null;`) as the out-of-scope mutation path.

## Root causes (`crates/gosling/src/permission/working_dir_scope_inspector.rs`)

1. **Device streams counted as files.** `path_from_shell_token` trimmed the `2>` / `>` prefix
   and then treated `/dev/null` as an explicit absolute path. It canonicalizes to `/dev/null`,
   which is outside every session folder, so any segment with `>/dev/null` or `2>/dev/null`
   that was not on the read-only command list prompted.
2. **Segment splitting only recognized free-standing separators.** `shell_words::split`
   does not treat `;`, `|`, or newlines as operators, so `head -55 file;` yielded the token
   `file;` and the whole command collapsed into one segment. Per-segment read-only judgement
   (the design that lets a `cat` of reference material sit next to an in-scope write) never
   applied to the common `a; b; c` shape, and the collected path carried a trailing `;`.
3. **Read-only list too narrow.** `printf`, `echo`, `ps`, `lsof`, `which`, `env`, `date`,
   `diff`, `du`, `df`, `jq`, hash tools, and similar commands were classified as mutating,
   so their explicit path arguments were checked as writes.

## Repair

- `is_device_stream` recognizes exactly `/dev/null`, `/dev/zero`, `/dev/random`,
  `/dev/urandom`, `/dev/stdin`, `/dev/stdout`, `/dev/stderr`, `/dev/tty`, and `/dev/fd/<n>`.
  Those never count as touched paths, for shell tokens or structured tool arguments. Raw
  disks and other device nodes (`/dev/disk2`) are still treated as out-of-scope files.
- `split_shell_command` splits at unquoted `;`, newline, `|`, `||`, and `&&`, honoring single
  quotes, double quotes, and backslash escapes, before word-splitting each segment.
- `redirects_output_to_file` detects `>`, `>>`, `>|`, `2>`, `&>`, and glued forms, ignores
  descriptor duplication (`2>&1`), and treats a device-stream target as not a write. As a
  side effect `ls 2>/outside/err.log` is now correctly a write, which the old check missed.
- The read-only command list gains the commands above; `sort` is read-only unless `-o` or
  `--output` is present.

Guards added: `device_streams_are_recognized_narrowly`,
`redirections_to_device_streams_do_not_count_as_writes`,
`segments_split_at_glued_separators_and_newlines`, and
`workspace_session_does_not_prompt_for_diagnostic_pipelines`, which replays anonymized
versions of the three real commands against a workspace session and asserts that only the
genuine out-of-scope append is flagged.

## Validation

| Check | Result |
| --- | --- |
| `cargo test -p gosling --lib -- working_dir_scope_inspector` | 29 passed; 0 failed |
| `cargo test -p gosling --lib` (with the host `MUNINN_MCP_BEARER_TOKEN` unset) | 1852 passed; 0 failed; 3 ignored |
| `cargo clippy -p gosling --all-targets -- -D warnings` | Passed |
| `cargo fmt --all -- --check`, `git diff --check` | Passed |

The unrelated `merge_environments_keeps_the_original_error_when_nothing_is_declared` test fails
whenever the host shell exports `MUNINN_MCP_BEARER_TOKEN`; it is not touched here.

## Deployment note

The running `/Applications/Gosling.app` still carries the old inspector. The fix takes effect
after rebuilding the server binary and relaunching the app (see the server-only shortcut in
the rebuild notes); a live turn was in progress, so the binary was not swapped in this session.

## Not changed

- `cd <dir>` followed by relative mutations still flags the `cd` target when it is outside the
  session folders; that accidental guard is kept because relative paths after a `cd` are not
  tracked.
- Heredoc bodies are still scanned token by token; explicit absolute paths inside a script
  body remain candidates, as before.
- Egress and permission inspectors were not reviewed in this pass.
