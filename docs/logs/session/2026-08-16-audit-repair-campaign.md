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

## Deliberately not fixed

**SEC-GOS-011** (absent `Origin` skips the WebSocket origin check). The
audit's recommended fail-closed behavior was implemented and tested live: it
returns 403 for every non-browser ACP client while blocking no browser attack,
because the WebSocket spec requires browsers to send `Origin`. Reverted with
the reasoning recorded at the call site. The cross-origin threat is covered by
the policy for real origins and by `check_acp_token`, which runs first.

## Open / not yet started

SEC-GOS-002 (client-supplied CSP), SEC-GOS-004/005/006/007/009/012,
AOC-GOS-002..005, the Cluster D durability set (DAT-GSL-001, CON-GSL-001..005),
Cluster E operator-truth (WFG-GOS-001..009, REL-GSL-010/011, FSR-GSL-012),
repo posture (RSP-*, RST-*, RPC-*), and the nodejs/GUI set. SEC-GOS-007 is
blocked on the same browser-cannot-set-headers constraint as SEC-GOS-001 and
needs a nonce/design decision rather than a mechanical patch.

Operator authorized CI/CD workflow changes (RSP-GSL-004, RST-GSL-001/002) and
bounding the unmeasured caches (MEM-GSL-001, MEM-GSL-003); neither is started.
