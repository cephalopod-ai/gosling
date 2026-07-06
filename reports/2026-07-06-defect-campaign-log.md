# Defect repair campaign - 2026-07-06

Scope: repair campaign following `reports/2026-07-06-fresh-comprehensive-audit.md`.

Inputs: current source tree and fresh audit findings only. Prior session logs and prior audit/repair reports were not used as repair inputs.

## Gate summary

| Gate | Status | Notes |
| --- | --- | --- |
| Gate 0: orientation | Passed | Worktree was clean before the new audit/report edits. |
| Gate 1: inventory | Passed | Five defects tracked as F-001 through F-005. |
| Gate 2: grouping | Passed | Groups split by file/data path: server CSP, egress classifier, CI secret log, CI pinning, plugin discovery compile break. |
| Gate 3: patch | Passed | All five groups patched. No modularization needed; touched Rust files are under 1000 lines. |
| Gate 4: verify | Passed | Formatting, targeted tests, clippy, and lightweight scans passed. |
| Gate 5: adversarial review | Passed | Rechecked bypass paths for CSP source tokens, namespaced command tools, derived secret logging, mutable action refs, and project-plugin trust semantics. |
| Gate 6: change review | Passed | Diff remained scoped to the confirmed findings and reports. |
| Gate 7: commit | Not run | No commit was requested. |

## Repairs

### G1: MCP app proxy CSP source validation

Finding: F-001.

Changed `crates/gosling-server/src/routes/mcp_app_proxy.rs` to normalize CSP source values before they enter the proxy meta CSP or guest CSP header. Allowed schemes are limited to `http`, `https`, `ws`, and `wss`; wildcard-only, quoted, semicolon, comma, whitespace, credentialed, and unsupported scheme inputs are rejected. Added regression tests for URL normalization, wildcard host sources, unsafe source rejection, parse filtering, and final CSP omission of rejected tokens.

Post-patch references: `crates/gosling-server/src/routes/mcp_app_proxy.rs:122`, `crates/gosling-server/src/routes/mcp_app_proxy.rs:202`, `crates/gosling-server/src/routes/mcp_app_proxy.rs:391`.

### G2: Egress namespaced command classification

Finding: F-002.

Changed `crates/gosling/src/security/egress_inspector.rs` to classify `__execute_command` and `__run_command` suffixes as shell tools, matching the adjacent prompt-injection scanner. Added a regression test proving outbound uploads through `developer__run_command` and `developer__execute_command` require approval.

Post-patch references: `crates/gosling/src/security/egress_inspector.rs:275`, `crates/gosling/src/security/egress_inspector.rs:593`.

### G3: CI secret-prefix logging

Finding: F-003.

Changed `.github/workflows/docs-update-cli-ref.yml` so the diagnostics step only reports whether `ANTHROPIC_API_KEY` is configured, without printing any derived substring.

Post-patch reference: `.github/workflows/docs-update-cli-ref.yml:174`.

### G4: GitHub Actions SHA pinning

Finding: F-004.

Pinned the remaining mutable action refs to commit SHAs while preserving version comments:

- `.github/workflows/scorecard.yml`
- `.github/workflows/bundle-desktop-windows.yml`
- `.github/workflows/pr-smoke-test.yml`
- `.github/workflows/pr-comment-build-cli.yml`
- `.github/workflows/autoclose`
- `.github/workflows/cargo-deny.yml`

### G5: Plugin discovery compile break

Finding: F-005.

Changed `crates/gosling/src/plugins/discovery.rs` so the project-plugin enablement boolean no longer shadows the `enabled` vector. Updated stale test fixtures to include `trusted`, and corrected the new-project-plugin test to assert the documented trust model: newly discovered project plugins are persisted disabled and untrusted unless explicitly enabled.

Post-patch references: `crates/gosling/src/plugins/discovery.rs:116`, `crates/gosling/src/plugins/discovery.rs:403`, `crates/gosling/src/plugins/discovery.rs:441`, `crates/gosling/src/plugins/discovery.rs:471`.

## Verification

Passed:

- `source bin/activate-hermit && cargo fmt --all`
- `source bin/activate-hermit && cargo test -p gosling-server mcp_app_proxy`
- `source bin/activate-hermit && cargo test -p gosling egress_inspector`
- `source bin/activate-hermit && cargo test -p gosling plugins::discovery`
- `source bin/activate-hermit && cargo clippy -p gosling -p gosling-server --all-targets -- -D warnings`
- `git diff --check`
- `rg -n "uses: [^@[:space:]]+/[^@[:space:]]+@(v[0-9]+|main|master)" .github/workflows -S` returned no matches.
- `rg -n "\\$\\{[A-Z0-9_]*(API_KEY|TOKEN|SECRET)[A-Z0-9_]*:0:[0-9]+\\}" .github/workflows scripts bin -S` returned no matches.

Notes:

- The first server test run exposed F-005 and failed before the new server proxy tests could run. After G5 was patched, the server proxy tests passed.
- The first egress test run exposed stale `PluginConfigEntry` test fixtures. After G5 was completed, egress tests and discovery tests passed.

## Residual risk

- Full workspace `cargo test` was not run.
- Desktop UI typecheck/playtest was not run because the repaired defects were server, core security, plugin discovery, and workflow posture issues.
- GitHub workflows were statically checked but not executed in GitHub Actions.
