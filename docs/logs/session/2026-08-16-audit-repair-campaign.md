# 2026-08-16 — Audit repair campaign (2026-08-15 findings)

**Branch:** `repair/audit-2026-08-15-campaign` (off `main` @ `6bd02c9f`)
**Skill:** `repair-defect-campaign` (governed_repair)
**Scope agreed with operator:** all live High + Medium findings from
`docs/cloud/2026-08-15-*` lens reports.

## Inventory correction

The master report's per-lens files contain **128 distinct canonical finding
IDs**, not the 307 first estimated in session (that figure was a raw regex
count that swept up cross-references, CWE citations, and ADR pointers).

| Severity | Count |
|---|---|
| High | 21 |
| Medium | 65 |
| Low | 32 |
| Info | 2 |
| Unspecified (orchestration lens) | 8 |

High+Medium selection: **94 findings**.

`**Held**` (122 occurrences) belongs to the security lens **boundary map** —
protections that hold — and is *not* a finding disposition. All 94 High+Medium
findings were re-verified against source at HEAD: **92 OPEN, 2 UNCLEAR**
(MEM-GSL-001, MEM-GSL-003 — mechanism real, never measured).

## Baseline

`cargo check --workspace --all-targets` clean; `cargo clippy --all-targets`
exit 0. One **pre-existing** test failure,
`context_mgmt::summarizer::tests::defaults_to_off`, confirmed failing on a
stashed clean HEAD. It is unrelated to this campaign and was left alone.

## Groups completed

### Group 1 — `3406ad17f` — Auto islands (Cluster A, ship-gating)

SEC-GOS-003, SEC-GOS-013, LLM-GSL-001, LLM-GSL-003, LLM-GSL-006, AOC-GOS-001,
NEG-GSL-001, PGR-GSL-001, INV-GSL-004.

Auto ran with no operator attached yet allowed any tool lacking an explicit
permission. Execution and write tools now require an explicit user permission
in Auto; reads still pass. The LLM read-only verdict no longer unlocks
execution or write authority. `EgressInspector` opts out of the Auto
downgrade. Four rival shell-tool predicates collapsed into
`permission::tool_class` — the loose `contains()` form had been missing
`computercontroller__automation_script`.

The test `external_egress_is_allowed_in_auto_mode` asserted the vulnerability
as intended behavior; it now asserts the approval requirement.

Validation: gosling lib 1654 passed / 1 pre-existing failure; clippy clean.

### Group 2 — `6634ece38` — ACP secret out of the URL (Cluster B)

SEC-GOS-001. The secret moved from `?token=` to `Sec-WebSocket-Protocol`.
Browser `WebSocket` cannot set headers, which is why the query string existed;
the upstream `agent-client-protocol-http` server does not handle the
subprotocol header, so `check_acp_token` echoes the accepted value on the 101
response (without it a browser aborts the connection).

Verified against a live `gosling serve`:

| case | result |
|---|---|
| correct subprotocol | 101 + `sec-websocket-protocol` echoed |
| correct `X-Secret-Key` | 101 |
| old `?token=` query string | 401 |
| wrong secret | 401 |
| no credential | 401 |

Validation: desktop `tsc` clean, `eslint` clean, `pnpm test` 110 files / 798
tests passed; gosling lib 1658 passed / 1 pre-existing failure.

### Group 3 — `a958f06ea` — secret masking, panic, retry

SEC-GOS-010 (mask preserves length only, no plaintext prefix), REL-GSL-005
(`assert!` on cross-session MCP reuse became a propagated error), CAS-GSL-001 /
REL-GSL-012 (`RetryConfig::new` said `transient_only: false` while `Default`
said `true`, so Bedrock/Vertex/Databricks replayed permanent failures).

Validation: workspace check + clippy clean; gosling-providers 439 passed;
gosling-server 36 passed including two new mask regression tests.

### Group 4 — `b831abfd5` — operator truth (Cluster E)

REL-GSL-010, REL-GSL-011, AOC-GOS-003. `/status` now runs the real
`SessionManager::healthy` probe instead of returning the same static `"ok"` as
`/health`; `gosling doctor` no longer claims "local diagnostics complete"
without contacting a provider; a review check that fails to run now enters the
findings stream at `high` instead of being indistinguishable from a clean run.

Verified live: healthy store → `/health` 200, `/status` 200; unopenable store
→ `/health` 200, `/status` 503. Corrupting the `.db` on disk does **not** trip
the probe, because SQLite keeps serving an already-open pool — the probe
catches an unreachable store, not post-open corruption.

### Group 5 — `42dd4fb84` — CI permission scoping

RST-GSL-002 (dropped `id-token: write` from the job that checks out an
untrusted PR head), RST-GSL-001 (paused-but-dispatchable workflow dropped to
`contents: read` at top level), RSP-GSL-004 (dependabot auto-merge moved to
`permissions: {}` at workflow level with job-scoped grants and explicit trigger
types).

**RSP-GSL-002 and RSP-GSL-003 stay open**: cargo-deny is not available in this
environment, so neither a secret-scanning job nor `[licenses]`/`[bans]`/
`[sources]` rules could be validated. Unverified CI config that fails on first
run is worse than the open finding.

### Group 6 — `6ffd57176` — bounded caches

MEM-GSL-001 (digest cache, 512-entry cap with insertion-order eviction),
MEM-GSL-002 (8 MiB read cap on `memories.jsonl`), MEM-GSL-003 (Desktop
`sessionsById` evicts unobserved entries past 50).

MEM-GSL-001 and MEM-GSL-003 remain **unmeasured**. These bound a mechanism
that is provably unbounded in code; they are not evidence a leak was observed.

## Unresolved conflict — repository identity (REC-GSL-001)

**Not resolved, deliberately.** `Cargo.toml:14` declares
`https://github.com/repo-makeover/gosling`; the actual remote is
`https://github.com/cephalopod-ai/gosling.git`.

The consequence is larger than a stale URL: **nine workflows are gated on
`if: github.repository == 'repo-makeover/gosling'` and therefore never run on
this remote** — `cargo-deny`, `scorecard`, `dependabot-auto-merge`,
`cargo-machete`, `stale`, `minor-release`, `update-health-dashboard`,
`rebuild-skills-marketplace`, `update-hacktoberfest-leaderboard`.

Flipping the slug would *activate* nine dormant workflows at once, including
release and auto-merge automation. That is an operator decision, not a
mechanical repair, so per AGENTS.md ("preserve the conflict explicitly and log
it as a follow-up") it is recorded here rather than resolved.

Note this also limits the value of the RSP-GSL-004 hardening in Group 5: the
dependabot auto-merge workflow is currently dormant on this remote regardless.

## Deliberately not fixed

**SEC-GOS-011** (absent `Origin` skips the WebSocket origin check). The
audit's recommended fail-closed behavior was implemented and tested live: it
returns 403 for every non-browser ACP client while blocking no browser attack,
because the WebSocket spec requires browsers to send `Origin`. Reverted with
the reasoning recorded at the call site. The cross-origin threat is covered by
the policy for real origins and by `check_acp_token`, which runs first.

## Open / not yet started

Roughly 60 of the 94 High+Medium findings remain. Notable clusters:

- **Cluster B remainder**: SEC-GOS-002 (client-supplied CSP), SEC-GOS-007
  (unauthenticated `/mcp-app-proxy`, `/mcp-app-guest`), SEC-GOS-012 (unauth
  serve does not force loopback), SECN-GSL-001. SEC-GOS-007 hits the same
  browser-cannot-set-headers constraint as SEC-GOS-001 and needs a nonce or
  design decision, not a mechanical patch.
- **Cluster C**: NEG-GSL-002 / LLM-GSL-004 (repo `AGENTS.md` and skills load
  as instructions with no workspace-trust gate), AOC-GOS-004.
- **Cluster D durability**: DAT-GSL-001 (compacted resume has no freshness
  gate), CON-GSL-001 (recover can mark a live peer tool `in_doubt`),
  CON-GSL-002..005, STT-GOS-001..005, TMP-GOS-001..007, RPC-GSL-001/002.
- **Cluster E remainder**: WFG-GOS-001..009 (Desktop drops tool `error`
  strings, Settings default-mode copy, TUI Allow-always on security prompts),
  FSR-GSL-012, CMP-GOS-001/002.
- **Repo posture**: RSP-GSL-001..003, REC-GSL-001, DEAD-GSL-001, XREP-GOS-001.
- **Orchestration**: AOC-GOS-002 (tagteam puts the full prompt on argv),
  AOC-GOS-005, MCP-GOS-001, IAPI-GOS-001.
- **Architecture**: ARC-GSL-001..006, INV-GSL-001. ARC-GSL-001 names three
  files over 4000 lines; per the campaign's own rule these are routed to a
  dedicated modularization pass rather than split mid-repair.
