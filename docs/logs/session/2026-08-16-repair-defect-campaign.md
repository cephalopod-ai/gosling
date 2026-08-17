# 2026-08-16 — Repair defect campaign: remaining High-severity findings

**Branch:** `main` (committed directly per stage; no protected-branch or
remote push involved)
**Skill:** `repair-defect-campaign` (governed_repair)
**Scope:** the four High-severity 2026-08-15-audit findings recorded as
"never picked up by a repair batch" in `docs/TODO.md`, plus (added mid-run at
operator request) the one pre-existing failing test also tracked there.

## Gate 0 — baseline

Working tree clean at `56b56c468`. `cargo build`/`cargo clippy
--all-targets -- -D warnings` both clean. `cargo test -p gosling --lib`:
1682 passed, 1 pre-existing failure
(`context_mgmt::summarizer::tests::defaults_to_off`, already tracked).
Commit policy: local commits per stage, no push (no push authorized this
session).

## Gate 1 — inventory

| ID | Domain | Severity | Complexity | Touch set |
|---|---|---|---|---|
| AOC-GOS-004 | Security | High | local_guardrail | `agents/platform_extensions/summon.rs` — agent discovery + capability policy resolution |
| CON-GSL-001 | Concurrency | High | persistence_recovery | `session/session_manager.rs` — `recover_tool_operations`, tool-op dispatch INSERT, schema migrations |
| MCP-GOS-001 | Security | High | workflow_protocol (minimal-repair slice: local_guardrail) | `gosling-mcp/src/computercontroller/mod.rs` — tool registration |
| ARC-GSL-002 | Architecture | High | workflow_protocol, Cost L | `gosling-providers`/`gosling` crate boundary — many import sites |
| (added mid-run) known-failing test | Test hygiene | — | trivial | `context_mgmt/summarizer/mod.rs` test module |

All four re-read against current source and the 2026-08-15 audit docs
(`docs/cloud/2026-08-15-audit-orchestration-contracts.md`,
`-audit-dataflow-core.md`, `-audit-architecture-invariants.md`,
`docs/cloud/audit-architecture-seam.md`) before grouping; re-confirmed open
against `docs/logs/session/2026-08-16-audit-repair-campaign.md`'s "Open / not
yet started" list and `docs/logs/session/2026-08-16-acp-mcp-repair.md`'s
inventory table.

## Gate 2 — grouping, modularization, and scope decision

Each finding touches a disjoint file/crate, so four single-finding groups
(no shared-surface consolidation available):

- **Group A** — AOC-GOS-004 (`summon.rs`, 2684 lines).
- **Group B** — CON-GSL-001 (`session_manager.rs`, 8856 lines).
- **Group C** — MCP-GOS-001 (`computercontroller/mod.rs`, 2101 lines).
- **Group D** — ARC-GSL-002 — **not patched**. All three other touched files
  are already >= 2000 lines, so the modularization rule (route, don't split
  mid-repair) applied uniformly: no in-stage modularization anywhere in this
  campaign.

ARC-GSL-002's own recommended mitigation is moving `conversation` /
`gosling_mode` / `thinking` / `permission` out of `gosling-providers` into a
domain crate — a crate-boundary move across `gosling`, `gosling-cli`,
`gosling-server`, and generated SDK types (Cost L, "many import sites" per
the audit), with the audit's own non-goal saying not to fold provider
consolidation into the same slice. That is a broad architectural rewrite
under this skill's stop conditions, not a boundable patch. Routed to a
dedicated architecture pass instead of attempted here, same treatment
`docs/TODO.md` already gives ARC-GSL-001's >= 4000-line files.

## Results

### Group A — AOC-GOS-004 (`1bf5a6ddb`)

`build_spec_from_agent` treated a repo-committed agent file's `capabilities`
policy as an authoritative allowlist. Discovery includes
`working_dir/.gosling/agents`, `.claude/agents`, `.agents/agents` — repo
content, not operator config — so a cloned repo declaring
`capabilities: {extensions: [developer]}` got that extension whenever the
parent session had it enabled, with no confirmation.

`SourceEntry.global` already existed to distinguish operator-owned agent
dirs from repo-committed ones but was never actually set (`parse_agent_content`
hardcoded `global: false` for both). Threaded the real value through
`scan_agents_from_dir`/`parse_agent_content`; `build_spec_from_agent` now
drops a declared policy when `!source.global`, logging a security event, and
falls back to the same `Some(Vec::new())` empty-policy result a legacy agent
file with no policy at all already gets — not `None`, which would have
regressed that legacy case into the model-requested grant path.

Two new regression tests: a repo-local agent declaring `developer` resolves
to zero granted extensions even though the parent has it enabled; a global
agent with the identical declaration is still honored.

### Group B — CON-GSL-001 (`c314dae6a`)

`recover_tool_operations` treated any foreign `owner_id` on a `started` row
as an abandoned/crashed owner. `owner_id` is only a per-instance UUID with no
liveness signal, and CLI + desktop share the default session dirs, so
opening or continuing a session in process B while process A was mid-dispatch
synthesized a permanent "in doubt, do not retry" response into the shared
conversation and flipped the ledger under the still-running peer.

A prior pass (`ecddfe1ba`) had already investigated this and left a comment
naming the two real fixes: a heartbeat, or an owner PID/lease the recovering
process can probe. Went with the PID probe, reusing
`subprocess::process_is_alive` (already used elsewhere in-crate for exactly
this, including the Windows `tasklist` fallback) rather than inventing a new
mechanism. Added `tool_operations.owner_pid` (schema v27; the ALTER is
guarded by the repo's existing `pragma_table_info` idiom, since `create_schema`
builds the current-version table directly for fresh databases and doesn't
replay the incremental migrations). The liveness probe applies only to
foreign `started` rows — same-owner rows keep the existing in-process
`active_tool_operations` check unchanged, and `completed`/`in_doubt` rows are
already terminal and safe to finalize regardless of dispatcher.

Adapted the existing `interrupted_tool_operation_recovers_as_visible_in_doubt_response`
test, which had (accidentally) relied on two `SessionManager`s in one test
process sharing a real, live PID to represent "crashed owner" — it now kills
the owner PID explicitly first, so it tests an actual crash instead of the
bug it used to depend on. Added
`live_peer_tool_operation_survives_a_concurrent_recover`: a live peer's
in-flight tool now survives a concurrent recover call with no synthetic
response written, and the real owner can still complete it normally
afterward.

### Group C — MCP-GOS-001 (`72b23086d`)

`automation_script` (writes a model-supplied script to a 0o755 temp file and
executes it) and `computer_control` (OS-level UI automation) carried no MCP
`annotations`, so any host other than Gosling's own ACP layer (which already
gates both by name via `permission::tool_class::CODE_EXECUTION_TOOL_NAMES`)
had no signal distinguishing them from the read-only tools on the same
server (web_scrape, cache, pdf/docx extract). `computercontroller` was
already disabled by default in both extension registries
(`ui/desktop/src/built-in-extensions.json`,
`.../settings/extensions/bundled-extensions.json`) — that half of the
audit's minimal repair needed no change.

Added `destructive_hint = true, open_world_hint = true` to all `#[cfg]`
platform variants of both tools (3 for `automation_script`: Windows/macOS/
other-Unix; 4 for `computer_control`: Windows/macOS/Linux/fallback) via the
rmcp `#[tool(annotations(...))]` the macro already supports. Two new tests
assert the registered `Tool` definition carries both hints regardless of
which `#[cfg]` variant compiles on the test host.

Non-goal, per the audit: did not touch the read/extract tools or restructure
the server into separate read/exec servers — that split is a product-policy
decision the audit itself routes to a human owner.

### Added mid-run — known-failing test (`93a19738d`)

Operator asked to also fix `context_mgmt::summarizer::tests::defaults_to_off`
while this campaign was already running. Root cause: the test called the
bare `summarizer_mode()`, which reads `Config::global()` — the real,
process-wide config singleton keyed by this machine's actual config
directory, not an isolated fixture. This dev environment's own
`~/.config/gosling/config.yaml` has `GOSLING_SUMMARIZER: on` set (a
deliberate personal setting, left untouched), so the "defaults to off" test
was failing against a real user's real config, not a production defect.
Fixed by testing `summarizer_mode_from` against an isolated, temp-file-backed
`Config`, matching the neighboring
`settings_file_values_are_honored_and_env_overrides_them` test's existing
pattern.

## Validation

Per stage: targeted crate tests + `cargo clippy -p <crate> --all-targets --
-D warnings` + `cargo fmt`. Campaign-final:

- `cargo build` (full workspace) — clean.
- `cargo clippy --all-targets -- -D warnings` (full workspace) — 0 issues.
- `cargo test --workspace --lib` — all green, including
  `gosling` (1683/1683, the prior 1 pre-existing failure now fixed),
  `gosling-mcp` (87/87), `gosling-server`, `gosling-cli`, `gosling-providers`,
  `gosling-sdk-types`, `gosling-sdk`, `gosling-acp-macros`.

No `ui/desktop` changes in this campaign, so no `pnpm` validation was run.

## Architecture and contract drift

Authoritative source for all three patched findings is the 2026-08-15 audit
itself (`docs/cloud/2026-08-15-audit-*.md`) — no competing ADR or contract
doc governs delegate capability trust, cross-process tool-operation recovery,
or MCP tool annotations. Pre-repair disposition: evidenced pre-existing
drift (each finding's own "Observed behavior" section). Post-repair: no new
drift — each fix implements the audit's own "Expected boundary" without
touching any other declared contract, public API, or persisted format beyond
the additive `tool_operations.owner_pid` column (backward-compatible,
migration-guarded). ARC-GSL-002: N/A for this run — not patched, routed.

## Record closure

`docs/TODO.md` updated: AOC-GOS-004, CON-GSL-001, MCP-GOS-001, and the
known-failing test marked closed with commit pointers; ARC-GSL-002 left open
with the routing reasoning recorded in-line, matching how ARC-GSL-001's
oversized files are already handled.
