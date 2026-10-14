# 2026-10-13 — Resource and handoff audit / repair

## Task and authority

Audit and patch process/task ownership, excessive work, output handoffs, deadends,
redundancy and races. Baseline: `main` at `77da40dda`, clean checkout.
Catalog: 13 audit lenses plus standalone applicability triage; source findings
frozen in `docs/cloud/2026-10-13-resource-handoff-audit.md`, then
`repair-defect-patchset`. The user's patch request authorizes local source repairs.
No installation, merge, publication, account changes or unrelated process kills.
The operator explicitly approved targeted Rust tests and clippy during this run.

## Locality stages and interaction analysis

1. F01/F02: `developer/shell.rs`, `run_command`, collector: bound both bytes and
   line count, own the collector through cancellation. Normal output, process
   exit codes and continued pipe draining remain the baseline.
2. F03: `summon/task_tracking.rs`, BackgroundTask ownership, load and completion:
   cancel/abort when an owned task is dropped; preserve successful collection,
   timeout reinsertion and semaphore admission.
3. F04/F06/F07: Desktop output-history batches, heartbeat hook and elicitation
   cancellation: remove abandoned waiters,
   fence status to the prompt attempt and clear pending input on cancellation.
   Preserve batch cap, per-path errors, coalescing and session isolation.
4. F08/F09: ACP domain status and notification ownership: inspect already-known
   exit, prevent terminal status regression, and own notification tasks once per
   connection. Preserve notification schemas and negotiated capabilities.
5. F10: MCP and core PATH probes: bound wait/output and kill/reap the owned process group
   on deadline. Preserve cached PATH merge and normal profile output parsing.
6. Review the union, validate, reconcile the finding report and index.

The stages share cancellation/ownership semantics but no persisted schema.
Stage 4 uses the existing Tokio abort-on-drop helper. The first compile check
showed that gosling lacked its `rt` feature, so `cargo add tokio-util --features
rt -p gosling --offline` enabled it without changing the version or lockfile.
F05 was rejected after tracing the event-loop scheduling and remains unchanged.
The final sibling review extended F10 to core PATH capture: its five-second
stdout timeout was followed by an unbounded exit wait, with an unbounded reader.
Edits are sequential to preserve reviewable gates; independent reads
and validation checks may run concurrently. Checkpoint after each stage.

## Validation

- `cargo check -p gosling -p gosling-mcp --tests --no-default-features` — passed.
- `cargo test -p gosling --lib agents::platform_extensions::developer::shell::tests --no-default-features` — 22 passed.
- `cargo test -p gosling --lib agents::platform_extensions::summon::tests --no-default-features` — 50 passed.
- `cargo test -p gosling --lib acp::domain_adapter::tests --no-default-features` — 12 passed.
- `cargo test -p gosling --lib acp::server::tests --no-default-features` — 88 passed.
- `cargo test -p gosling-mcp --lib subprocess::tests --no-default-features` — 3 passed.
- `cargo test -p gosling --test permission_audit_regressions security_info_logs_keep_ids_and_hosts_without_command_bodies --no-default-features` — 1 passed.
- `cargo clippy -p gosling -p gosling-mcp --all-targets --no-default-features -- -D warnings` — passed.
- Desktop `pnpm exec vitest run --maxWorkers=3 --testTimeout=10000` — 214 files, 1,675 passed.
- Desktop `pnpm run typecheck`, scoped ESLint `--max-warnings 0` and changed TS Prettier — passed.
- `cargo fmt --all` — executed after Rust edits; `cargo fmt --all -- --check` and `git diff --check` passed.
- Targeted documentation link/disposition/20-row optimization coverage checks and governance-marker scans — passed; git check-ignore returned no ignored log, confirming its exception.

Intermediate failures: first compile lacked Tokio-util's rt feature; the new
connection test initially used the wrong builder call syntax and an ambiguous
metadata type, corrected before execution. All-target Clippy found a pre-existing
cloned-reference slice in permission_audit_regressions.rs; replaced with
std::slice::from_ref and ran that regression. The broader ACP test run exposed the
pre-existing LRU-pin fixture's Session not found failure, reproduced alone.
The October 10 test ledger already records that failure on clean HEAD; the fixture
now creates durable sessions and preserves the LRU/busy-pin assertions. Those
fixture corrections are validation preconditions, separately recorded from F01–F10.

The before/after review preserved ordinary output, nonzero exit errors, same-session
batching, 500-path chunking, duplicate waiter isolation, timeout reinsertion,
permission semantics, notification schemas and stored formats. F01's post-review
large-line case also propagates its existing truncation marker into the structured
flag. No unrelated dependency version, migration or authority change was made.
Source contracts compared against ADR-0019/0020 and shell lifecycle contracts:
no new schema/transport drift; intended cleanup/bounds defects corrected.

The checkout advanced externally to `05259746c` during the run, containing the
earlier patch group; preserved without this agent committing or resetting it.
The session log is explicitly admitted through .gitignore. Audit dispositions,
docs/INDEX.md, docs/TODO.md, the test ledger and Developer shell capture guide
were reconciled. Current status: `completed_with_partial_verification`.

## Risks / follow-ups

See the audit's residual register and 13-lens/31-standalone coverage matrix.
No installed rebuild, native GUI playtest, Windows/Flatpak test, parent-SIGKILL
drill, heap/fd/disk soak or optional Cargo feature qualification was performed.
F02 is source repaired with a native retained-pipe drill still open. Existing
shared in-flight ACP RPCs are not cancellable through the per-row AbortSignal;
the repaired waiter is removed promptly while the shared request remains owned.
Historic unrelated full-library failures have not been declared cleared.
The synchronous PATH reader threads depend on owned-process-group termination
closing their pipes; a deliberately escaped descendant holding a pipe remains
an untested resource-retention case. No zero-leak claim is made.

## Authorized build and reinstall follow-up

The operator subsequently requested "build and reinstall". Built the clean
`4eb5297757d5b6790dde0d563adb3cc12bc3c63e` checkout using
`DEVELOPER_DIR=/Library/Developer/CommandLineTools just package-ui` with the
Hermit environment. The full default-feature Rust release build, production
Desktop packaging and local ad-hoc signing passed; version remains 1.5.0.
pnpm emitted other-platform binary/bin-link warnings but packaging exited zero.

Installed the verified bundle at `/Applications/Gosling.app` and atomically
replaced `/Users/eric/.local/bin/gosling`. Deep strict signature verification
passed before and after installation. The release, staged, embedded and
installed CLI/backend SHA-256 hashes match:
`9bdee68d5c040917ac9ef71c3985aa40dd6f7236c929cf72baa6252e1e0ba704`.
The old app quit through its native menu, and no processes from its installed
bundle remained before replacement. Existing user configuration and session
stores were preserved.

Both installed binaries passed `--version`; the standalone CLI passed `--help`.
The relaunched installed Desktop completed loading and native accessibility
readback showed restored history, navigation, composer and Outputs. Screenshot
capture failed in ScreenCaptureKit; no new model request or lifecycle stress
drill was submitted, so this is startup validation only. The earlier residual
test gaps remain open. `cargo fmt --all -- --check` passed.

Verified rollback copies and the install receipt are retained locally at
`/Users/eric/.local/share/gosling/install-backups/build-reinstall-20261013-22l559tj/`.
No merge, publication, commit or unrelated process termination was performed.
