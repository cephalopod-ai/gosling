# Repository defect closeout — final report

Date: 2026-08-27

Branch: `work`

Baseline: `8c45b5e`

## Executive verdict

The current branch contains the completed High, Medium, and Deep Research repair
campaigns recorded by the canonical backlog and the dated verification reports.
The continuation review repaired a build-breaking Deep Research path comparison,
Linux zombie-process liveness misclassification, a root-sensitive persistence
regression, and four Low-severity contract/documentation defects. It also
reconciled one stale signal finding after source inspection.

Validation remains partial at the whole-workspace/Desktop level, but the
`gosling` library now compiles and the focused regressions pass. Full workspace
Clippy still cannot download the prebuilt `rusty_v8` archive because the
environment's network proxy returns HTTP 403. Desktop validation cannot start
because the installed pnpm 10.28.1 is older than the repository's required pnpm
10.30.0, and Hermit bootstrap is also blocked by HTTP 403.
A fresh closeout review found no additional source-confirmed defect that could
be repaired without changing an unresolved product, security, or architecture
contract. No runtime code was changed in this closeout rather than presenting a
speculative change as a defect repair.

Validation was partial. Rust formatting and repository whitespace checks pass.
Full workspace Clippy reached dependency compilation but could not download the
prebuilt `rusty_v8` archive because the environment's network proxy returned
HTTP 403. Desktop validation could not start because the installed pnpm 10.28.1
is older than the repository's required pnpm 10.30.0, and Hermit bootstrap was
also blocked by HTTP 403. These are environment limitations, not test failures
attributed to the source tree.

## Reviewed evidence

- `docs/TODO.md`, the canonical backlog and disposition register.
- `docs/cloud/2026-08-26-clean-independent-audit.md`, the latest independent
  all-lens source audit.
- `reports/2026-08-27-medium-defect-campaign.md`, the completed Medium repair
  campaign and installed-application evidence.
- `reports/2026-08-27-deep-research-regression-playtest-audit.md` and
  `reports/2026-08-27-deep-research-repair-verification.md`, the latest
  Deep Research defect inventory and closure evidence.
- Recent session records under `docs/logs/session/` and the current clean Git
  state.

## Repairs in this continuation

| Finding | Repair |
|---|---|
| BUILD-GSL-001 | Replaced an invalid `String::as_path` call in Deep Research workspace validation with a typed `Path` comparison, restoring the Rust test build. |
| PROC-GSL-001 | Treats Linux `/proc` zombie state as terminated so recovery ownership and process-tree cleanup do not mistake a dead descendant for live work. |
| TEST-GSL-001 | Replaced a mode-bit-dependent permission failure fixture with a deterministic non-directory parent failure that also works as root. |
| INV-GSL-002 | Aligned the SQLite schema, historical add-column migration, and foreign transcript normalization with the runtime `SmartApprove` default. |
| INV-GSL-003 | Made slash-command metadata own a typed builtin handler, removing drift between the advertised command list and dispatch match. |
| CMP-GSL-002 | Corrected coexistence claims to disclose intentionally shared AAIF `~/.agents` paths. |
| AID-GSL-002 | Replaced the nonexistent, forbidden Desktop OpenAPI-client path with the existing TypeScript SDK path. |
| SIG-GSL-005 | Reinspected and closed as stale: enabled ML classification already fails when no classifier initializes and warns on partial initialization. |
## Repair disposition

All findings that the current canonical records classify as mechanically
repairable are already closed on this branch. The review did not reopen closed
items without contradictory source or test evidence. It also did not turn
historical findings, generated assets, test fixtures, or explanatory TODO text
into new defects.

## Deferred items requiring operator input or external prerequisites

| Finding | Required input or prerequisite |
|---|---|
| DAT-GSL-002 | Decide whether workspace deletion may remove project-library data still referenced by preserved pinned sessions. |
| NEG-GSL-001 | Define the MCP App actor, transcript, visibility, and permission contract. |
| RSP-GSL-001 | Choose the response to RUSTSEC-2023-0071 and provide a working `cargo-deny` environment. |
| ARC-GSL-003 / ARC-GSL-004 / ARC-GSL-005 | Approve provider-port, MCP-dependency, and process-global-state architecture changes. |
| INV-GSL-001 | Define which credential, provider/model, workspace, and extension authority imported snapshots may restore. |
| CMP-GSL-004 | Run a fresh Giles scan and decide whether advisory metadata should be promoted. |
| ACP-GSL-003 | Decide whether managed-context or external-tool providers may occupy Deep Research delegate seats and define an approval-capable subagent contract. |
| NEG-GSL-005 | Decide whether the official remote `goslingd` control-plane behavior fits the local-first product contract. |
| PATH-GSL-001 | Choose between isolating `~/.agents` from Goose and explicitly supporting shared discovery. |
| CON-GSL-001 | Choose a cross-process session lease contract versus allowing resume to open a new serve process. |
| SEC-GSL-003 / SEC-GOS-007 | Choose between retaining documented remote MCP App deployments and requiring loopback-only peers. |
| RSP-GSL-002 / RSP-GSL-003 | Provide approved secret/history scanning and `cargo-deny` validation before adding unverified CI policy. |

These items remain visible in the canonical backlog; this report does not claim
they are fixed.

## Validation

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | Pass. |
| `git diff --check` | Pass before report creation; repeated at closeout. |
| `cargo test -p gosling --lib foreign_sessions_default_to_smart_approve` | Pass. |
| `cargo test -p gosling --lib session_schema_default_matches_runtime_mode_default` | Pass. |
| `cargo test -p gosling --lib builtin_command_names_are_unique` | Pass. |
| `NO_PROXY=127.0.0.1,localhost no_proxy=127.0.0.1,localhost cargo test -p gosling --lib` | Pass: 1,753 tests. |
| `cargo clippy -p gosling --all-targets -- -D warnings` | Pass. |
| `cargo clippy --all-targets -- -D warnings` | Environment-blocked while downloading `rusty_v8`; proxy returned HTTP 403. |
| `cd ui/desktop && pnpm run typecheck && pnpm run test:run` | Environment-blocked before execution: pnpm 10.28.1 does not satisfy the declared `>=10.30.0` engine. |
| `source bin/activate-hermit` | Environment-blocked while bootstrapping Hermit; GitHub download returned HTTP 403. |

## Residual risk

This is a source-and-validation closeout, not proof that no latent defect exists.
Full Rust Clippy/tests and Desktop typecheck/tests must be rerun in an environment
with the pinned toolchain and dependency archives available. Credentialed
provider, packaged Desktop, multi-platform, and remote-deployment behavior were
not replayed in this closeout; the earlier dated campaigns remain the available
evidence for those surfaces.

Final status: `completed_with_partial_validation`.
