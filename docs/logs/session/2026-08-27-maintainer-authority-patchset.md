# 2026-08-27 Maintainer-authority patch set

## Gate 0 — orientation and baseline

- Task: resolve the twelve records explicitly reopened by the operator after the
  all-remaining-TODO campaign.
- Target: `/Users/eric/Work/vscode/forked/gosling`, branch
  `codex/resolve-maintainer-authority-items`, baseline `30d19041124dcb5f04aa5a03d865a2c00313a358`.
- Initial worktree: clean. No history rewrite, force push, release publication,
  signing, tag, repository-permission change, or secret mutation is implicit in
  this patch set.
- Involvement: L2 standard. The operator explicitly reopened the twelve records
  and authorized repository-local repair. Outward actions remain hard gates.
- Baseline: the immediately preceding repair campaign passed Rust format,
  build, targeted crate suites, all-target Clippy, Desktop typecheck and 1,069
  tests, shell conformance, documentation typecheck/tests/build, and OIDC tests.
  Current remote revision `30d190411` newly exposed Cargo Deny and cargo-machete
  failures; those failures are understood release-stage inputs, not an unknown
  baseline.

## Gate 1 — supplied set classification

| Record | Domain | Priority | Complexity | Disposition entering patch set |
|---|---|---:|---:|---|
| DAT-GSL-002 | data integrity | P1 | low | Close on the accepted preserve-pinned-data contract and existing behavior. |
| NEG-GSL-001 | security/workflow | P1 | medium | Require an explicit user confirmation before an MCP App may submit chat text; app tool calls retain Gosling permission inspection. |
| INV-GSL-001 | security/data | P1 | low | Make import quarantine explicit and regression-test that provider/model/workspace/credential authority is not restored. |
| ACP-GSL-003 | workflow/security | P1 | medium | Exclude managed-context/external-tool providers from multi-model Deep Research delegate seats. |
| NEG-GSL-005 | security/product | P0 | medium | Adopt the README's local-first boundary: official `goslingd` binds loopback only. |
| CON-GSL-001 | concurrency/data | P0 | high | Add a durable cross-process per-session turn lease held for the reply stream. |
| SEC-GSL-003 | security | P0 | medium | Require loopback peers for unauthenticated MCP App proxy/guest routes. |
| SEC-GOS-007 | security | P1 | medium | Reopen the prior remote-deployment exception; close it with the local-only server and peer check. |
| REL-GATE-001 | release | P0 | high | Complete all locally reachable release checks and record evidence; signing/installed-platform evidence remains a hard gate. |
| REL-GATE-002 | release | P0 | high | Prepare the candidate but do not create/push a tag or publish without the mandatory outward-action confirmation. |
| REPO-GATE-001 | CI/repository | P0 | medium | Make BuildNotify succeed without a configured webhook and repair current source-controlled CI failures; enabling the GitHub ruleset remains a hard gate. |
| DOC-GOV-001 | governance | P2 | low | Declare `.dory/` ignored local operational state; durable evidence belongs in explicit committed session logs. |

## Gate 2 — locality groups and interaction analysis

### Stage 1 — existing safe contracts

- Findings: DAT-GSL-002, INV-GSL-001, DOC-GOV-001.
- Surfaces: workspace deletion, session import transfer/tests, documentation
  inventory/index/TODO.
- Baseline invariant: workspace deletion preserves pinned session snapshots and
  project-library rows; imports select a caller-provided working directory and
  quarantine provider/model/workspace/credential authority; `.dory/` is ignored.
- Regression surface: workspace-service deletion tests and session import tests.

### Stage 2 — interactive and delegated actors

- Findings: NEG-GSL-001, ACP-GSL-003.
- Surfaces: MCP App renderer message callback, Research Model Team selector,
  user documentation and component tests.
- Baseline invariant: app tool calls continue through the existing visibility
  and permission inspectors; Solo Research may use the selected lead provider;
  hosted-tool providers remain available for all eligible seats.
- Intended delta: app-authored text becomes a user-confirmed proposal; providers
  that manage their own context cannot occupy multi-model delegate seats.

### Stage 3 — cross-process turn ownership

- Finding: CON-GSL-001.
- Surfaces: session schema/migration, session manager, `Agent::reply`, focused
  concurrency tests, architecture documentation.
- Baseline invariant: all non-concurrent reply behavior, stream ordering,
  persistence, cancellation, and compaction behavior remain unchanged.
- Intended delta: a second live process cannot run the same session concurrently;
  crashed/stale owners are recoverable.

### Stage 4 — local control-plane boundary

- Findings: NEG-GSL-005, SEC-GSL-003, SEC-GOS-007.
- Surfaces: gosling-server configuration/bind path, request authentication
  middleware, server docs and tests.
- Baseline invariant: loopback Desktop/server startup, authenticated routes, TLS,
  CSP derivation, and one-use guest nonces remain unchanged.
- Intended delta: non-loopback `goslingd` binds fail closed and unauthenticated
  MCP App proxy/guest requests require a loopback peer.

### Stage 5 — repository and release readiness

- Findings: REL-GATE-001, REL-GATE-002, REPO-GATE-001.
- Surfaces: BuildNotify workflow, Rust lock/dependency posture, current CI
  failures, release checklist and release ledgers.
- Baseline invariant: failure notifications still post when a webhook exists;
  no check is marked complete without observed evidence; no release is published.
- Intended delta: an absent webhook is an explicit successful no-op, current
  source-controlled CI defects are repaired, and all reachable release evidence
  is refreshed. Branch protection, signing, tag push, publication, and
  announcement stop at their mandatory confirmation gate.

## Architecture and contract sources

- `AGENTS.md` is canonical repository governance.
- `README.md`, `docs/architecture.md`, ADR-0015, and the 2026-08-26 audit govern
  workspace/session ownership and the local-first product boundary.
- MCP Apps source, the MCP Apps guides, and the Gosling permission-inspection
  tests govern app capabilities.
- `RELEASE.md` and `RELEASE_CHECKLIST.md` govern publication and explicitly
  retain human release authority.
- Pre-repair disposition: the eight code/policy records are evidenced
  pre-existing gaps or undocumented safe behavior; the release records are
  incomplete external gates. No competing active architecture source was found.

## Checkpoint

Gate 2 complete. Next action: execute Stage 1, run its focused regressions, and
record behavioral-equivalence evidence before Stage 2.

## Stage 1 result — existing safe contracts

- `DAT-GSL-002`: closed on the existing workspace-service behavior and
  ADR-0015 contract. Deleting a workspace does not touch `sessions.db`, pinned
  snapshots, or workspace-keyed project-library rows.
- `INV-GSL-001`: the import boundary now states that untrusted files do not
  transfer provider/model, workspace, credential, folder-grant, workflow, or
  executable-extension authority. The round-trip fixture now seeds those
  authority fields and proves they are absent after import.
- `DOC-GOV-001`: `.dory/` is explicitly ignored local operational state;
  durable evidence requires a reviewed, committed repository log.
- Validation: `cargo test -p gosling test_export_import_roundtrip` passed;
  `cargo test -p gosling create_duplicate_switch_and_delete_preserve_default`
  passed; `cargo fmt --all` completed.
- Behavioral equivalence: imported history, usage, accumulated usage, names,
  selected working directory, Approve mode, non-executable extension state,
  and untrusted provenance remain unchanged. Workspace create/duplicate/switch/
  delete behavior is unchanged.
- Adversarial review: the import test would fail if any seeded authority field
  survives. `.dory/` remains ignored and no local state was copied, deleted, or
  promoted.
- Architecture drift: ADR-0015 and the workspace snapshot boundary are now
  documented consistently. Drift delta: no new drift.

Stage 1 passed. Next action: Stage 2 interactive and delegated actors.

## Stage 2 result — interactive and delegated actors

- `NEG-GSL-001`: an MCP App can no longer inject indistinguishable user text
  into the transcript. The host identifies the extension, shows a bounded exact
  preview, and calls the chat submit path only after explicit user confirmation.
  Denial is returned to the app. Existing app tool calls still pass through
  visibility and permission inspection without contract changes.
- `ACP-GSL-003`: providers that manage their own context remain selectable for
  Solo Research but are unavailable in every Dual/Trio seat. Seat filling,
  availability, option disabling, and validation use the same eligibility rule.
  The existing research prompt regression still requires ad-hoc delegate
  payloads to omit `source`.
- Validation: three focused Vitest files passed 10/10 tests; Desktop TypeScript
  typecheck passed; Prettier completed on the changed UI files.
- Behavioral equivalence: hosted-tool provider seat ordering and distinct-seat
  filling are unchanged; Solo remains optional and accepts managed-context
  providers; confirmed app messages still use the existing submit path.
- Adversarial review: unconfirmed app text is rejected, the confirmation preview
  is bounded, a managed-context current model cannot leak into a multi-model
  delegate seat, and insufficient eligible seats disable the mode.
- Architecture drift: app authority now matches Gosling's permission boundary;
  Deep Research seats now match the safe Summon execution contract. Drift delta:
  no new drift.

Stage 2 passed. Next action: Stage 3 cross-process turn ownership.

## Stage 3 result — cross-process turn ownership

- `CON-GSL-001`: schema v29 adds one durable turn lease per session. The lease
  is acquired before reply-side persistence, held by the returned stream,
  renewed every 15 seconds, and owner-matched on release.
- A competing process/window receives an actionable error while both the owner
  process and its heartbeat are live. A dead owner or heartbeat older than 90
  seconds can be replaced without manual database repair.
- The liveness probe is awaited outside the SQLite write transaction; the row
  is re-read under `BEGIN IMMEDIATE` before mutation so a racing owner cannot be
  overwritten from stale observation.
- Validation: both cross-manager exclusion/release and stale-owner recovery
  tests passed; the complete workspace suite later passed with 1,888 gosling
  library tests and all integration/doc tests.

Stage 3 passed.

## Stage 4 result — local control-plane boundary

- `NEG-GSL-005`: `Settings::socket_addr` accepts numeric IPv4/IPv6 loopback
  only. Wildcard, LAN, VPN, public, and documentation-only remote deployment
  paths are rejected.
- `SEC-GSL-003` / `SEC-GOS-007`: the unauthenticated MCP App proxy and guest
  paths additionally require loopback `ConnectInfo`; both TLS implementations
  and the plain HTTP server now supply peer metadata.
- The guide formerly recommending `0.0.0.0` now documents only a separately
  managed local process. Existing TLS, secret, nonce, and CSP protections remain.
- Validation: four configuration tests and the peer-policy test passed; all 38
  gosling-server library tests, 38 binary tests, and three TLS tests later passed.

Stage 4 passed.

## Stage 5 result — repository and release readiness

- BuildNotify exits successfully with a visible notice when its optional
  Discord webhook is absent, while the configured-webhook payload is unchanged.
- Removed unused `fs2` and `include_dir` MCP dependencies. Updated `webbrowser`,
  `async-utility`, `chacha20`, `nostr`, `nostr-relay-pool`, `spin`, and `h2` to
  compatible patched/non-yanked releases.
- The graph no longer contains RUSTSEC-2023-0071, so its obsolete exception was
  removed. `cargo-deny check advisories` and `cargo-machete` both pass.
- Six source-validation release checklist items now carry observed evidence.
  Installed artifacts, signatures, checksums, remote CI, tag creation,
  publication, and readback remain unchecked.
- The GitHub ruleset remains disabled. No push, PR, rule mutation, secret
  mutation, tag, signature, release, or announcement was performed.

## Final validation

- `cargo fmt --check`: passed.
- `cargo build`: passed.
- `cargo test --workspace`: passed, including 1,888 core tests and all workspace
  integration and documentation tests (declared ignored tests remain visible).
- `cargo clippy --all-targets -- -D warnings`: passed.
- Desktop TypeScript and Vitest: passed, 134 files / 1,071 tests.
- Documentation typecheck, 16 tests, and production build: passed; 165 Markdown
  pages exported.
- `cargo-deny check advisories`: passed.
- `cargo-machete`: passed.
- BuildNotify YAML parse, no-secret branch, `git diff --check`, and the required
  AGENTS documentation-governance marker: passed.

## Closure and residual gates

The eight code/product/security records and `.dory` governance record are
closed locally. The locally actionable portions of the three release/repository
records are complete. Three outward facts still require explicit authority and
external evidence: enable ruleset 18782969, push this revision and observe
remote CI, and later sign/tag/publish/read back only after every installed
artifact gate passes. They are gates, not unresolved implementation defects.
