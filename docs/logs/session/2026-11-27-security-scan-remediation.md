# 2026-11-27 — Security scan remediation

## Task

Verify the external static security scan of `041edfb4c` against source and patch the
findings that hold up. Work is on branch `fix/security-scan-findings`.

## Changes

- **SEC-GOS-003 / SEC-GOS-004 (working-dir scope inspector).** Shell analysis now
  tracks the script's own variable assignments and `for` loop values and expands them
  before collecting paths, so `V=/etc/passwd; cat $V` is checked like a literal path.
  An expansion it cannot resolve (unknown variable, command or process substitution,
  `$'...'`, or anything after `read`, `eval`, `source`, `printf -v`, `unset`, or a
  declaration with options) marks the segment opaque. Opaque segments in restricted
  sessions require approval unless the command never opens its operands (`echo`,
  `printf`, `test`, and similar). Option-sensitive commands (`sort`, `sed`, `awk`,
  `find`, `git`, `cd`) are no longer read-only when any of their words is opaque.
  `git diff/log/show/grep` stop counting as read-only with `--output`, `--ext-diff`,
  or `grep -O`/`--open-files-in-pager`. Relative paths with a `..` component anywhere
  in them, such as `sub/../../x`, now count as explicit paths. Before, only a leading
  `../` counted, so `cp ./a sub/../../outside` escaped workspace write scope.
- **SEC-GOS-006.** The shell tool removes the `AWS_*` variables gosling exported from
  its config or keyring for the Bedrock and SageMaker SDKs. Variables the operator
  set in the real environment are left alone.
- **SEC-GOS-007.** Bounded-client approvals now carry a message. Auto mode only
  auto-grants approvals without one, and ACP withholds "always allow" for prompts
  that have one. `session/set_mode` rejects modes other than Approve and Chat for
  bounded sessions.
- **SEC-GOS-014.** `gosling review --range` rejects values that start with `-`.
- **SEC-GOS-005.** `download_cli.sh` and `download_cli.ps1` run
  `gh attestation verify --repo cephalopod-ai/gosling` when a signed-in GitHub CLI
  exists, and abort if verification fails. With `GOSLING_REQUIRE_PROVENANCE=true` they
  also abort when no such CLI exists.

## Not changed (operator decision or not material)

- SEC-GOS-001 (default Auto mode) and SEC-GOS-002 (project skills without a trust
  gate) change operator-facing behavior. Project skills are in active use in local
  repositories, so a gate would hide them until the operator trusts each one.
- SEC-GOS-008 through 013 and the info-level items were judged hygiene or not
  material.

## Validation

- Scope inspector: 4 new tests pass. The module has 50 tests: 43 pass and 7 fail.
  Clean `main` already had the same 7 failures with identical assertions. They come
  from the `LLM-GROK-001` unscoped-mutation gate added in `77da40dda`, which prompts
  for commands such as `curl` and `echo hi>out.txt` that those tests expect to stay
  quiet.
- The shell tool tests pass with `aws-providers` enabled, including the new
  exported-variable probe. `client_tool_policy_test` passes. The new
  `gosling review` range test passes.
- `cargo clippy -p gosling -p gosling-cli --all-targets -- -D warnings` and
  `cargo clippy -p gosling --features aws-providers --lib -- -D warnings` are clean.
  `cargo fmt` is applied.
- `bash -n download_cli.sh` passes. No test covers the successful attestation path,
  because the repository's GitHub releases currently publish no archives. There is
  no `stable` release, and `v1.2.5` has no assets. `download_cli.ps1` was not
  parsed: PowerShell is not installed.
- No test covers the bounded-session `set_mode` guard. It is checked only by the
  compiler and Clippy.

## Follow-up

- Decide whether the `LLM-GROK-001` gate or the 7 tests describe the intended
  behavior. The gate conflicts with the no-false-positive rule for Autonomous
  workspace sessions.
- SEC-GOS-001 and SEC-GOS-002 need an operator decision.
- Releases need published archives before the installers can install anything.
