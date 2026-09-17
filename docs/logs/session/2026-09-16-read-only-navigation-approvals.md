# Read-only shell navigation approvals (2026-09-16)

## Scope and evidence

- Agent: Codex; checkout `main` at `87e295606`.
- Input: the operator's repeated approval report and screenshot from the Dawes
  chat "I'm creating a space". The request authorizes a bounded source repair.
- Existing uncommitted heartbeat changes are preserved. No session grants,
  workspace definitions, saved permissions, or installed application are changed.
- Workflow: catalog `repair-defect-priority`, one backend/permission defect
  **WDS-GSL-002**, P1 because repeated false write approvals block research.
  Involvement is low, inferred from the report that approval interruptions slow work.
- Read-only inspection of `sessions.db` confirms session `20260916_4`, mode
  `auto`, directory restriction off, cwd `/Users/eric/Work/projects/dawes-core`,
  additional directory `/Users/eric/Work/projects/dawes-output-folder`.
- Stored requests at `2026-09-17 02:36:06` and `02:36:35` UTC use `cd` into
  the Gosling/agent-skills repositories followed by `grep`, `sed`, `head`, `cat`,
  and `date`. A request at `02:27:35` also uses `git rev-parse HEAD`.
  These commands were inspected as data, not replayed against the host.
- The source whitelist omits `cd` and `git rev-parse`; `mutation_paths` therefore
  collects the outside `cd` target as a write. Auto deliberately preserves
  workspace inspector approvals. The earlier diagnostic fixes explicitly kept
  this conservative directory-change guard for relative writes.

## Selected patch plan and contract baseline

One selected item, no deferred higher-priority items in this bounded engagement.
Change `working_dir_scope_inspector.rs` to recognize unambiguous `cd` navigation
and `git rev-parse` as read-only. Preserve the existing directory-change path
guard whenever any shell segment is mutating, including redirects, substitutions,
and nested executable syntax. This avoids widening approval exemptions for bare
relative writes after navigation. Unsupported syntax still requires approval.

Add inspector regressions in `tests/permission_audit_regressions.rs` covering the
reported diagnostic command shape, multiple separators/directories, read-only
workspace roots, actual relative/absolute writes, redirects, and explicit
directory restriction. Update the existing TODO backlog and active ledger with
the actual validation status. No configuration, API, schema, or grant changes.

Governing sources: canonical `AGENTS.md`; `docs/architecture.md` (Rust core owns
permissions and workspace snapshots); active ADR-0017, amended 2026-09-05 and
2026-09-08 (unrestricted workspace reads pass, real writes retain their boundary,
scratch allowance is canonicalized); TODO AUT-GSL-001–003 (Auto preserves explicit
security findings). Historical September 6/7 repair logs are provenance, not
competing active declarations. `.giles/repo.yaml` is advisory.

Pre-repair disposition: pre-existing classifier drift from ADR-0017's read-only
allowance; the saved folder/Auto policies themselves are conformant. Out of scope:
changing Auto policy, broad shell-parser changes, adding external directory grants,
and writes from this research session into external repositories. Stored mixed
Python/catalog-building commands can still legitimately require approval.

## Validation boundary and checkpoint

Independent source/record/session reads were batched. Source mutations and checks
are sequential because verification depends on the patch. This log is the durable
checkpoint; on resume inspect the current diff before continuing.

`AGENTS.md` reserves Cargo tests/builds/Clippy for an explicit user request.
Planned checks: Hermit `cargo fmt`, diff checks, targeted source comparison of the
two changed production functions, and record/link/governance checks. These do not
substitute for regression execution. Planned runtime checks when authorized:
`cargo test -p gosling --lib -- working_dir_scope_inspector` and
`cargo test -p gosling --test permission_audit_regressions`, followed by relevant
Clippy/build checks and an installed-app read-only interaction.

## Patch and review results

The source patch recognizes `cd` only when its existing target helper can track
the directory. `git rev-parse` joins the existing read-only Git subcommands.
`mutation_paths` checks for any mutating segment before deciding whether to keep
the directory-change guard. Pure read-only navigation no longer yields a mutation
path; mixed scripts retain the previous guard. Unsupported/ambiguous navigation
classification, shell syntax handling, and genuine write controls are preserved.

Four integration regressions are added. The read-only fixture covers standalone
navigation, semicolon-separated research reads across two directories, `&&` Git
diagnostics, and `builtin cd --` followed by a newline. It runs against both read
and read/write workspace policies. Other fixtures expect approval for bare relative
redirects/removal, absolute writes, nested shell writes and command substitutions;
deny writes with read-only roots; and require approval for outside reads when
explicit restriction is on. They inspect requests without executing shell commands.

| Check | Result and scope |
| --- | --- |
| `source bin/activate-hermit` then `cargo fmt` | Passed; Rust source formatted |
| `git diff --check` | Passed |
| Inline Python production-function comparison against `git show HEAD:<source>` | Passed: only `mutation_paths` and `shell_segment_is_read_only` changed; inspector policy, analyzer, path resolution/canonicalization, scratch allowance and strict path collection remain byte-identical |
| Inline Python regression inventory comparison against `git show HEAD:<tests>` | Passed: exactly four new test functions; pre-existing test source unchanged |
| Inline Python record/link/ignore/governance-marker checks | Passed: the log is trackable, TODO and active ledger retain WDS-GSL-002 as pending verification, canonical governance markers remain present |
| Regression execution, Cargo build and Clippy | Not run; AGENTS requires an explicit test/build request |
| Original installed-app interaction | Not rerun; installed binary unchanged |

The implementing agent reviewed the diff and then performed a separate completeness
pass. This is self-review, not an independent audit. The touched paths were traced
for redirects, nested/substituted writes, read-only-root denials, and strict read
restriction. No public interface, persisted format, dependency, policy grant,
configuration, or renderer behavior changed. Comments explain the mixed-script
guard because bare write targets are not otherwise collected as explicit paths.
There are no stale in-code defect markers at the changed functions.

Post-repair declared-source comparison: the source change repairs the identified
read-only-classification drift while preserving the ADR-0017 mutation boundary and
Auto security exceptions. Drift delta: **no new drift by source inspection**;
runtime contract verification remains pending. Conservative mixed-script approvals
can still occur if a call navigates outside its roots and also contains a write.
No claim of general shell-parser correctness is made.

Files changed for this repair: `working_dir_scope_inspector.rs`,
`tests/permission_audit_regressions.rs`, `.gitignore`, `docs/TODO.md`,
`docs/polish/active-todo-ledger.md`, and this log. The new screenshot finding has no
external issue record to close. Canonical TODO and its active mirror now track
WDS-GSL-002 as **source-patched, needs verification**; no earlier historical finding
or release result was rewritten. The heartbeat source changes are preserved.

Status: **completed_with_partial_verification** for the source repair. Next action
when tests/builds are authorized: execute the two targeted Cargo suites above,
review any failures, then complete the relevant lint/build and installed-app
validation. No additional patch batch is selected.
