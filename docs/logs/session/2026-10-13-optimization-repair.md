# 2026-10-13 — Optimization findings repair

## Scope and preflight

Target: `/Users/eric/Work/vscode/forked/gosling`, main at `4eb529775`.
Supplied scope: REL-OPT-001/002/003 from `grok-optimization-13OCT2026.md` and the pasted report. Involvement inferred L2; authorized local code repair, no deployment/commit. Catalog workflow: repair-defect-patchset. The discovery search also suggested a new optimization audit; that stage is inapplicable to this request to patch a completed audit.
Initial user work: modified docs/INDEX.md and docs/logs/session/2026-10-13-resource-handoff-audit.md, untracked grok-optimization-13OCT2026.md. Preserve all existing content.
Baseline runtime checks unknown. AGENTS.md prohibits cargo build/test/clippy absent an explicit request; use cargo fmt, cargo check with tests, source trace and disposable git diagnostics. Runtime regressions will be compiled, not executed.

## Stage plan and baselines

1. REL-OPT-001 (performance/reliability, P2, medium): gosling-mcp subprocess cache/capture; Developer resolver and LoginPath; hooks PATH recovery. Existing behavior: eager background probe for enabled Developer, no probe for disabled flag, separate first-hook and MCP caches, five-second deadline, bounded 64 KiB output, process-session kill/reap, custom shell/Nushell/Flatpak selection. Share the process-local cache and capture, preserving first-caller selection and disabled behavior. Test concurrent cache calls and actual consumer spawn counts in an isolated process fixture.
2. REL-OPT-002 (performance, P3, medium): review handler collection/early summary. Existing behavior: HEAD or explicit range and file filters; tracked name-only plus patch; untracked text patches (binary/unreadable skipped); summary prints tracked stat only. Use raw NUL metadata plus patch in one git command; summary stat and untracked text readability determine emptiness without patch allocation. Cover rename/copy/deletion/mode/binary/unusual names and untracked inputs.
3. REL-OPT-003 (performance/reliability, P3, low): Linux dependency check, gosling-mcp dependency manifest/lock. Preserve dependency order and first-missing NotFound message. Replace per-tool which child with existing workspace which crate. Fixture PATH contains executable/nonexecutable/missing names, no which utility.

Stages have separate runtime surfaces; stage 1 owns shared subprocess support before stage 3 adds its dependency. No persistent schema or security-policy change. Sequential edits/checkpoints enforce stage gates; independent reads/checks can run concurrently. Cargo checks are incremental and share one target directory, so run sequentially.

## Governing declarations

AGENTS.md and GEMINI.md govern execution; docs/architecture.md dependency direction remains unchanged (gosling previously had a test-only dependency on gosling-mcp). The CLI commands guide review section governs ranges/files/summary. Shell/hook/MCP source and existing tests govern selection, timeout and capture; Linux source governs dependency order/error. No separate PATH-cache or review wire-format ADR found in docs/architecture.md/index or guide search. Baseline has the reported duplicate work; compatibility and cleanup boundaries remain active.

## Checkpoint

Orientation and source reproduction complete; no source edits yet. Next: shared PATH cache/capture and regressions, then compile check.

### Stage 1 checkpoint

Shared cache and bounded capture are implemented; eager Developer and hook settings remain. Hooks merge current PATH on each call instead of caching a stale merge. First caller selects its existing probe; success or failure is process-wide. Cargo check initially found gosling-mcp was test-only; promoted via cargo add/rm to a runtime dependency, with no lockfile delta needed. `cargo fmt --all` and `cargo check -p gosling -p gosling-mcp --tests --no-default-features` pass. Consumer spawn-count regression is compiled, not executed. Adversarial source review covered concurrent OnceLock callers, cached failure, disabled flags, custom shell/Nushell/Flatpak and kill/reap; no new policy or persistence drift found. Next: review raw metadata/stat collection.

### Stage 2 checkpoint

Review gets raw NUL-delimited metadata and patch together; rename/copy destination matches git name-only. Summary returns early from stat and untracked text eligibility (streaming UTF-8 validation, bounded buffer), preserving tracked-stat-only output and binary/unreadable skipping. Added actual git fixture comparisons and a CLI git-wrapper spawn-count fixture (3 calls default, 2 with explicit range; no patch for summary). `cargo check -p gosling-cli --tests --no-default-features` rejected the unsupported no-TLS configuration; rerun with `--features rustls-tls` passed. Formatting passed. Disposable Python/git diagnostic confirmed actual raw-format equivalence for rename/copy, deletion, binary and tab/newline/Unicode paths; this is format evidence, not execution of the Rust parser or CLI. Stage 1 remains compile-valid. Independent critic underway. Dory state tool failed on unrelated missing registry paths; this local session log supplies the durable checkpoint. Next: Linux in-process dependency lookup and final combined checks.

### Stage 3 and corrective-review checkpoint

Linux checks now use which::which and preserve exact dependency order and first-missing NotFound message. A temp-PATH fixture covers executable, nonexecutable and missing tools without requiring a which executable. cargo add initially rejected an unsupported --workspace flag; ordinary cargo add inherited the workspace dependency correctly. Cargo.lock adds only which 8.0.3 to gosling-mcp's dependency list.

The independent critic identified that the stage-1 first-caller resolver choice could let MCP bypass GOSLING_SHELL/Nushell/Flatpak handling. That intermediate design is superseded: the final implementation has one canonical probe in gosling-mcp, selected consistently as GOSLING_SHELL, then existing SHELL, then /bin/bash/sh. This intentionally unifies login-profile capture; Developer command execution still defaults to bash-on-PATH/sh, with the existing explicit override. All consumers cache success or failure process-wide. Automatic Flatpak shell paths are normalized to basenames for host lookup; explicit overrides remain verbatim. Fish uses quoted PATH to keep colon delimiters; [official fish variable-expansion documentation](https://fishshell.com/docs/current/language.html#quoting-variables) confirms that behavior. Nushell keeps the existing list-join probe.

Critic also caught Windows-invalid filenames and mismatched Git config environments in fixtures; Unix gating and matched environments correct them. Final critic inspection found no remaining defect in these corrections, the production raw parser, summary eligibility or Linux lookup. Reviewed independently by /root/patch_review under the skill's critic_required rule, without source edits or test execution.

## Finding dispositions and files

| Finding | Final repair | Evidence and limits |
| --- | --- | --- |
| REL-OPT-001 (Medium/P2, medium complexity) | Canonical process-wide login-PATH cache and bounded capture in gosling-mcp/src/subprocess.rs; Developer and hooks consume it. Removed duplicate capture/probe and hooks-only cache. Promoted gosling-mcp from dev dependency to runtime dependency with cargo add/rm. | Combined Cargo check passes. Existing deadline/output/reap regressions and moved Nushell tests compile. New hook fixture covers Developer/delegate/later-session plus hook/MCP in both caller orders; new MCP integration fixture covers eight concurrent callers, success/failure and SHELL-selected fish/nu argument handling. Runtime execution pending. |
| REL-OPT-002 (Low/P3, medium complexity) | Review handler uses raw NUL metadata plus patch in one git call; summary uses stat and bounded-buffer untracked readability before returning. | Combined Cargo check passes. Disposable real-git diagnostic confirms raw paths and patch equivalence for rename/copy/deletion/binary/tab/newline/Unicode inputs. Unit fixtures compare actual Git outputs/range/file filters and summary eligibility; CLI wrapper fixture expects three git children normally and two with an explicit range, with no patch in summary mode. Rust/CLI fixture execution pending. |
| REL-OPT-003 (Low/P3, low complexity) | Linux dependency loop uses workspace which crate in process; first-missing and successful paths remain. Added dependency with cargo add and updated lockfile. | Linux module and temp-PATH fixture compile on this macOS host via existing test cfg. Native Linux/runtime execution pending; no display automation changed. |

Source files: crates/gosling-mcp/src/subprocess.rs, crates/gosling/src/agents/platform_extensions/developer/shell.rs, crates/gosling/src/hooks/mod.rs, crates/gosling-cli/src/commands/review/handler.rs, crates/gosling-mcp/src/computercontroller/platform/linux.rs. Regression files: existing unit modules, crates/gosling-mcp/tests/login_path_cache.rs and crates/gosling-cli/tests/review_dry_run_test.rs. Dependency files: crates/gosling/Cargo.toml, crates/gosling-mcp/Cargo.toml and Cargo.lock.

## Final validation and equivalence evidence

- cargo fmt --all: passed. cargo fmt --all --check: passed.
- cargo check -p gosling -p gosling-mcp -p gosling-cli --tests --no-default-features --features gosling-cli/rustls-tls: passed on the final source and regression fixtures. This compiles tests; it does not execute them.
- Disposable Python/git diagnostic: raw metadata yields exactly git diff --name-only -z paths; patch bytes equal separate git diff for modification, rename, copy, deletion, binary and unusual UTF-8 names. The actual Git format was exercised; the Rust parser was not executed.
- git diff --check: passed. Governance markers remain. New index/report links resolve; the session log is allowlisted and discoverable. Initial index addition is retained; the pre-existing resource-handoff session log is untouched.
- Earlier failed checks: missing runtime gosling-mcp dependency corrected via cargo add/rm; CLI no-TLS compile rejection corrected by selecting rustls-tls; missing which dependency following unsupported cargo-add option corrected via workspace inheritance. No test failure was hidden or retried unchanged.

Behavioral preservation is supported by source traces and compile checks, not exhaustive runtime equivalence. Unchanged boundaries include disabled login recovery, eager first probe, five-second bounded cleanup, Developer shell-command execution and process groups, hook command/payload execution, configured file filters/ranges, untracked review synthesis, tracked-stat-only summary output, review worker isolation, Linux dependency order and automation commands. The intended login-profile selection change is documented in the environment-variable guide.

## Final workflow re-audit and completeness inspection

First closing pass traced ShellTool construction/background wait and delegate construction through shared cache/capture; hooks flag/merge/command path; MCP merged_path and computer-control consumers; review entry, severity validation, early summary, metadata extraction, untracked synthesis, discovery/scoping and orchestration; Linux initialization/display detection through availability checks and later execution. It found and corrected the caller-order, fish and Flatpak regression risks above. Shared caches retain one probe deadline and do not add invalidation or policy widening.

A distinct completeness pass matched all three supplied IDs to their original duplicate-work mechanism and reviewed the union diff, regression environments, dependency/lock consistency and source-record closure. No remaining in-scope gap in the source repair was identified. The independent critic confirms the corrected source and fixtures. No new persisted format, security boundary or review-worker behavior drift was found in the inspected scope. Giles remains advisory.

## Record closure and remaining limits

- grok-optimization-13OCT2026.md: original historical audit retained; appended dated disposition marking all three findings source-repaired with runtime limits and this evidence link.
- documentation/docs/guides/environment-variables.md: shared Unix login-PATH selection/override documented; Developer command fallback corrected from source.
- docs/INDEX.md: appended repair evidence link while preserving the user's audit entry.
- .gitignore: allowlisted this session log per docs/logs/README.md.
- Other REL-OPT-001/002/003 entries in docs/TODO.md refer to different September snippet/storage defects and were not changed. Held/rejected/measurement-backlog candidates remain protected.

Final status: completed_with_partial_verification. All three source repairs are implemented and compile-validated, with regression coverage added and records reconciled. No cargo build, cargo test, clippy, installed application, live provider or native Linux/Flatpak test was run; AGENTS.md restricts the three Cargo commands absent an explicit build/test request. Runtime process counts and timing benefits remain unmeasured. To close the runtime gap, execute the compiled targeted regressions under explicit test authorization and replay native Linux/Flatpak behavior. No commit, merge, install or publication occurred. Dory registry errors prevented its state/checkpoint support; this tracked local log is the restart record.
