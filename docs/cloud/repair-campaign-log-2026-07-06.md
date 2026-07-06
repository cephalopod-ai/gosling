# Gosling Repair Campaign Log - 2026-07-06

## Gate 0 - Orientation, Safety, Git Posture

Target repository: `/Users/eric/Work/vscode/forked/gosling`

Branch and baseline commit: `main` at `7c08048a6`

Findings source: `docs/cloud/2026-07-06-comprehensive-audit.md`

Repair skill: `~/Work/vscode/agent-skills/020_repair/repair-defect-campaign`

Git posture:

- Git is available.
- Remotes exist: `origin`, `sandbox`, and `gosling2`.
- The worktree contains the audit report created immediately before this campaign:
  `docs/cloud/2026-07-06-comprehensive-audit.md`.
- Commit policy for this run: no commits on `main` without explicit user authorization;
  record logical checkpoints in this log instead.

Repo rules observed:

- Source Hermit before Cargo commands.
- Run `cargo fmt` after Rust code changes.
- Do not run `cargo build`, `cargo test`, or `cargo clippy` unless the user asks to
  build/test changes.
- Desktop UI must not import generated OpenAPI client code from `ui/desktop/src/api`.
- Do not change Goose compatibility links or normalization scripts without reviewing
  `documentation/GOOSE_COMPATIBILITY.md`.

Validation inventory:

- Required by repo for changed Rust code: `cargo fmt`.
- Potential targeted checks if needed: Rust unit tests for touched modules, desktop
  `pnpm run typecheck`, and desktop `pnpm test`. Cargo build/test/clippy are held unless
  explicitly authorized.

Documentation convention:

- Audit and campaign artifacts live under `docs/cloud/`.
- This log records gate evidence, logical checkpoints, and residual risk.

## Gate 1 - Defect Inventory

| ID | Priority | Complexity | Domain | Disposition | Touch set |
| --- | --- | --- | --- | --- | --- |
| GSL-SEC-001 | P0 | Medium | security | in-scope | `GoslingMode`, config defaults, permission tests |
| GSL-SEC-002 | P0 | Medium | security | in-scope | prompt scanner config, shell-like tool classification |
| GSL-SEC-003 | P1 | Medium | security | in-scope | egress inspector policy result |
| GSL-SEC-004 | P0 | Medium | security | in-scope | MCP stdio process environment |
| GSL-SEC-005 | P0 | High | security | in-scope | plugin discovery, hook/MCP activation, config tests |
| GSL-SEC-006 | P1 | Medium | security | in-scope | malware check policy around unknown commands/errors |
| GSL-CON-007 | P0 | High | security/internal API | in-scope | ACP app tool dispatch path, permission/hook reuse |
| GSL-DAT-008 | P1 | Low | data integrity | in-scope | config read-modify-write parse errors |
| GSL-NODE-009 | P0 | Medium | Electron security | in-scope | preload file APIs, main IPC handlers |
| GSL-NODE-010 | P0 | Medium | Electron security | in-scope | CSP, ACP secret exposure, MCP app renderer |
| GSL-NODE-011 | P1 | Low | Electron security | in-scope | permission request handler |
| GSL-NODE-012 | P1 | Low | Electron security | in-scope | external URL protocol validation |
| GSL-CON-013 | P1 | Medium | internal API | in-scope | MCP app proxy auth contract |
| GSL-REL-014 | P2 | Low | reliability | in-scope | provider retry defaults and tests |
| GSL-ARC-015 | P2 | High | architecture | route-elsewhere for broad split; in-scope for guardrails | direct dispatch/global access guard tests where feasible |
| GSL-ARC-016 | P3 | Low | dependency contract | in-scope | `ui/text/package.json`, lockfile/README drift |

Inventory counts:

- Domains: security 7, Electron security 4, internal API 2, data integrity 1,
  reliability 1, architecture/dependency 2.
- Priorities: P0 7, P1 6, P2 2, P3 1.
- Complexity: low 4, medium 9, high 3.

## Gate 2 - Locality Grouping And Campaign Plan

Group 1: Rust agent security controls

- Defects: GSL-SEC-001, GSL-SEC-002, GSL-SEC-003, GSL-SEC-004, GSL-SEC-005,
  GSL-SEC-006, GSL-DAT-008, GSL-REL-014, architecture guardrail portion of GSL-ARC-015.
- Files/functions touched: config defaults, permission inspector, security scanner,
  egress inspector, plugin discovery, hook/MCP activation, malware check, extension
  manager, config read/write, retry config.
- Modularization decision: no in-band split. Large files over 2000 lines are routed
  for a future `repair-source-modularization` pass; this campaign will apply the
  smallest safe fixes.
- Regression surface: Rust unit tests in touched modules; static inspection; `cargo fmt`.
- Commit boundary: logical checkpoint only unless commits are explicitly authorized.

Group 2: ACP app tool and MCP app proxy contract

- Defects: GSL-CON-007 and GSL-CON-013.
- Files/functions touched: ACP app tools route, MCP app proxy core/server routes,
  proxy template/renderer.
- Modularization decision: no in-band split; files changed narrowly.
- Regression surface: Rust unit tests where existing harnesses allow; static
  request/auth contract review.
- Commit boundary: logical checkpoint only.

Group 3: Desktop Electron boundary hardening

- Defects: GSL-NODE-009, GSL-NODE-010, GSL-NODE-011, GSL-NODE-012.
- Files/functions touched: desktop preload API, main IPC handlers, CSP builder, URL
  security helper, MCP app renderer.
- Modularization decision: no in-band split of `main.ts` because it is >= 2000 lines;
  route full split to `repair-source-modularization` / frontend architecture follow-up.
- Regression surface: TypeScript static inspection and, if practical, desktop typecheck.
- Commit boundary: logical checkpoint only.

Group 4: Dependency contract and documentation closeout

- Defects: GSL-ARC-016 and campaign docs.
- Files/functions touched: `ui/text/package.json`, `ui/pnpm-lock.yaml`, `ui/text/README.md`,
  campaign log, audit report status.
- Modularization decision: none.
- Regression surface: package lock consistency and final diff review.
- Commit boundary: logical checkpoint only.

Cross-stage risks:

- Changing default approval posture can affect many tests and provider integrations.
- Project plugin trust and ACP app tool gating both depend on a shared permission
  enforcement story.
- Desktop secret removal must stay compatible with MCP app proxy behavior.
- Lockfile changes may require package-manager tooling.

## Stage Results

### Group 1 - Rust Agent Security Controls

Status: complete.

- GSL-SEC-001: default `GoslingMode` is now `Approve`; production fallbacks that
  previously used `GoslingMode::Auto` now use the safer default.
- GSL-SEC-002: prompt injection scanning defaults on and scans namespaced shell
  tools plus non-shell tools with inspectable command/input/URL arguments.
- GSL-SEC-003: outbound or unknown egress is now `RequireApproval`; inbound
  egress remains allowed.
- GSL-SEC-004: non-container MCP stdio processes now run with a cleared child
  environment plus explicit extension env and a minimal runtime allowlist.
- GSL-SEC-005: project plugins no longer auto-enable on discovery; explicit
  settings trust is required.
- GSL-SEC-006: OSV launcher gaps, HTTP failures, and parse failures now block
  extension startup instead of failing open.
- GSL-DAT-008: corrupt writable config files now fail read-modify-write updates
  instead of being replaced with a fresh mapping.
- GSL-REL-014: default provider retry behavior is transient-only.
- GSL-ARC-015: full modularization is routed out of this campaign; a guardrail
  was added by moving app tool dispatch through `Agent::dispatch_app_tool_call`.

### Group 2 - ACP App Tool And MCP App Proxy Contract

Status: complete.

- GSL-CON-007: ACP app tool calls now pass through agent security inspection,
  permission inspection, session lookup, and hook-aware dispatch instead of
  calling `ExtensionManager` directly.
- GSL-CON-013: MCP app proxy no longer authenticates through query-string
  secrets. The renderer gets a main-process-built proxy URL with the secret in
  the fragment, the proxy POST still authenticates with the body secret, and
  guest HTML entries are consumed on first read.
- Additional hardening: the proxy document uses per-response script nonces
  instead of `script-src 'unsafe-inline'`; the guest iframe compatibility CSP is
  kept separate.

### Group 3 - Desktop Electron Boundary Hardening

Status: complete.

- GSL-NODE-009: renderer file IPC is constrained to approved roots
  (`userData`, recent directories, and configured archive folder) and no longer
  shells out through `cat`.
- GSL-NODE-010: top-level desktop CSP removes inline script permission, narrows
  loopback `connect-src` to the active ACP endpoint, narrows `frame-src`, and
  removes the renderer `getSecretKey` bridge.
- GSL-NODE-011: Electron permission requests are limited to `media`.
- GSL-NODE-012: external URL opening now routes through a protocol allowlist
  helper instead of scattered denylist checks.

### Group 4 - Dependency Contract

Status: complete.

- GSL-ARC-016: `ui/text` now consumes `@repo-makeover/gosling-sdk` through the
  workspace package, with lockfile and README updated.

## Verification

Completed:

- `source bin/activate-hermit && cargo fmt`
- `source bin/activate-hermit && cd ui && pnpm install --lockfile-only`
- `source bin/activate-hermit && cd ui/desktop && pnpm run typecheck`
- `source bin/activate-hermit && cd ui/desktop && pnpm test:run src/utils/__tests__/csp.test.ts`
  - Result: 20 tests passed.
- `source bin/activate-hermit && cd ui/desktop && pnpm exec prettier --check index.html src/main.ts src/preload.ts src/theme-init.ts src/components/McpApps/McpAppRenderer.tsx src/utils/csp.ts src/utils/__tests__/csp.test.ts`
- `git diff --check`
- Residual-pattern scan for former high-risk signatures:
  `unwrap_or(GoslingMode::Auto)`, query-string MCP secrets, unsafe script CSP,
  broad loopback wildcard CSP, broad frame CSP, and direct unsafe external open.
  Remaining matches are the central safe wrappers and negative assertions in
  CSP tests.

Not run because repo instructions require an explicit build/test request:

- `cargo build`
- `cargo test`
- `cargo clippy --all-targets -- -D warnings`

## Residuals And Follow-Up Routing

- GSL-ARC-015 broad source modularization remains routed to a dedicated
  `repair-source-modularization` campaign. This run intentionally avoided
  splitting `agent.rs`, `config/base.rs`, and `ui/desktop/src/main.ts` while
  still adding policy guardrails for the audited dispatch bypass.

## Final Status

All actionable audit findings from `docs/cloud/2026-07-06-comprehensive-audit.md`
were patched or routed according to the repair campaign rules. No commits were
created because the active branch is `main` and explicit commit authorization
was not provided.
