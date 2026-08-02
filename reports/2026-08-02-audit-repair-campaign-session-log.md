# Audit and repair campaign — 2026-08-02

## Method

This campaign used the mapped `010_audit` workflows for MCP boundaries, workflow/UI contracts, dataflow/pipeline state, and reliability, followed by the mapped defect-repair workflow. Discovery was performed against the current worktree before consulting prior reports, so historic findings did not steer new-finding discovery.

Historic inputs were then rolled up from `docs/build/defects.md`, `docs/cloud/audit-dataflow-temporal.md`, and the July 18, July 20, and July 27 campaign records in this directory. Each item below was either repaired and regression-tested in this campaign or reclassified with evidence.

## Closed findings

| ID | Source | Finding and closure |
| --- | --- | --- |
| AUD-001 | Independent MCP/dataflow audit | Tagged memory retrieval overwrote prior entries with the same tag. It now appends every matching entry; regression test covers duplicate tags. |
| AUD-002 / REL-GOS-012 | Independent audit and July reliability roll-up | File-backed memory operations could race across processes. A sibling lock file now serializes remember, retrieve, remove, and clear operations. |
| AUD-003 | Independent ACP workflow audit | A failed fork after persistent copy left an orphan session. The failure path now removes the copied session. |
| AUD-004 | Independent ACP workflow audit | Working-directory add/remove persisted state before extension reconfiguration could fail. Persistence and live state are now rolled back transactionally. |
| AUD-005 | Independent MCP audit | Spreadsheet row/column coordinates silently narrowed from `u64` to `u32`; zero and out-of-range values are now rejected. Updates use the actual default worksheet and no longer claim an unsaved write succeeded. |
| CON-GOS-101 | July 27 roll-up | Usage and cost updates performed a read-modify-write cycle that could lose concurrent updates. `record_usage` now performs one atomic SQL accumulation. |
| REL-GOS-011 | July 27 roll-up | A pre-cancelled reply could persist setup state before ending. The agent now returns an empty stream before mutation and ACP recognizes cancellation after streaming. |
| REL-GOS-013 | Independent reliability audit | MCP child error handling could wait indefinitely for stderr inherited by descendants. Stderr collection is now bounded and aborted after the timeout. |
| SEC-GOS-001 | Historic transport review | Auxiliary ACP routes and REST serving used permissive CORS behavior. Both now use explicit loopback/desktop origin policies and fixed method/header sets. |
| SEC-GOS-002 | Historic configuration review | Secret masking revealed the exact secret length. Redaction now has a constant visible shape. |
| TMP-001 | `audit-dataflow-temporal.md` | Databricks OAuth tokens without `expires_in` were cached indefinitely. New tokens receive a conservative expiry and legacy unbounded tokens refresh. |
| TMP-002 | `audit-dataflow-temporal.md` | Azure accepted locally stale JWT-form bearer tokens. JWT expiry is now checked with a 30-second margin; opaque tokens remain supported. |
| TMP-003 | `audit-dataflow-temporal.md` | ChatGPT Codex could reject a newly rotated signing key until cache expiry. A cached unknown key id causes one forced JWK refresh before failure. |
| AUD-006 | Final scenario regression | One provider response with multiple tool calls was persisted as separate assistant messages, detaching shared reasoning context. Tool calls are now merged into one durable assistant message; thinking-preservation scenarios cover both single and multiple calls. |
| REL-GOS-014 | Full-suite scenario regression | Login-shell PATH discovery consumed the command-hook execution timeout. Discovery now completes before the hook timeout begins. |
| TEST-001 | ACP scenario audit | Fixtures put their mock host only in unscoped configuration and could prompt macOS Keychain. They now write runtime-scoped configuration and disable keyring for test fixtures. |
| TEST-002 | ACP scenario audit | Several scenarios requested cancellation while asserting successful tool work. Their explicit confirmations now match the intended successful paths, and scoped configuration assertions read the scoped store. |
| TEST-003 | Full-suite scenario regression | Docker background-process setup was transiently flaky. The test helper retries bounded detached starts and reports a useful terminal failure. A container left by an interrupted test run was removed by exact name. |
| TEST-004 | Full-suite scenario regression | Merely installing an agentic CLI made default tests execute a live external provider with no timeout or opt-in. Those tests now require `GOSLING_RUN_LIVE_PROVIDER_TESTS=1`; standard tests remain offline and deterministic. |

`TMP-004` (provider model-catalog bootstrap constants) was rechecked and is not a defect: configured provider model names are supported directly, and the constants are bootstrap fallbacks rather than an authoritative served catalog.

## Scenario and verification evidence

- `cargo test -- --test-threads=1` passed: 1,687 core tests plus all workspace integration tests and doctests.
- `cargo clippy --all-targets -- -D warnings` passed, and `cargo fmt` was applied.
- Recorded CLI scenarios passed for OpenAI: identity, image analysis, and weather tool (3/3).
- MCP command scenarios passed (14/14).
- Desktop `typecheck` passed; Vitest passed 82 files and 555 tests.
- Targeted regressions passed for memory tag aggregation and locking, session fork and directory rollback, usage concurrency, cancellation, CORS, secret masking, OAuth/JWK/Azure expiry, spreadsheet bounds/default worksheet, multi-tool reasoning, hook PATH timing, provider live-test gating, and Docker detached-process startup.

## Coverage boundaries

The repository's four recorded MCP replay tests remain explicitly ignored because the replay capture wrapper is intentionally blocked by malware checks. The separate `scripts/test_mcp.sh` needs externally supplied Anthropic credentials. Agentic live-provider scenarios are available through `GOSLING_RUN_LIVE_PROVIDER_TESTS=1` but were deliberately not run during this deterministic repair gate.
