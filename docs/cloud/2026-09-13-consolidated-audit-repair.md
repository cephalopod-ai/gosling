# Consolidated audit reconciliation and repair — 2026-09-13

## Outcome

The live all-scenarios playtest and Gemini architecture/dataflow audit added in
`829b09054` contained 18 distinct findings. This campaign reconciled every finding against the
repository's current contracts and source, then repaired the confirmed defects that were safe to
change without a new architecture or ownership decision.

- 13 findings repaired.
- 4 findings retained as explicit design, evidence, or migration decisions.
- 1 finding rejected after verification against ADR-0018.
- No exact duplicates were collapsed. Three pairs describe adjacent seams but different failure
  mechanisms.

The repair branch is `codex/consolidated-audit-repair-20260913`. All changes are local; no push,
PR, merge, release, or installed-application replacement was performed.

## Inputs and method

The two source records are:

- [2026-09-13 live all-scenarios playtest](2026-09-13-live-all-scenarios-playtest.md)
- [2026-09-13 Gemini architecture/dataflow audit](20260913_Gemini_Audit_Data_gosling.md)

The reports were treated as historical evidence rather than edited into agreement. Each candidate
was checked against current source, tests, ADRs, and explicit product contracts. Confirmed repairs
were grouped by locality, reproduced before repair, tested after repair, reviewed for interaction
effects, and committed independently. The detailed gate record is in the
[campaign session log](../logs/session/2026-09-13-consolidated-audit-repair-campaign.md).

## Canonical disposition

| ID | Canonical disposition | Result and rationale |
|---|---|---|
| GSL-PT-20260913-001 | Repaired | `doctor` now makes a bounded request to the configured provider and succeeds only after receiving a provider event. This correctively supersedes the earlier configuration-only REL-GSL-011 decision because current help and playtest contracts promise a working-setup check. |
| GSL-PT-20260913-002 | Repaired | Text-only cancellation preserves the submitted prompt, removes only the incomplete suffix, and persists an explicit cancellation terminator. |
| GSL-PT-20260913-003 | Repaired | Subagent notifications carry their durable tool-request identity and duplicate deliveries are suppressed per batch without hiding distinct repeated requests. |
| GSL-PT-20260913-004 | Repaired | `--no-session` runs use an ephemeral session store for the complete agent graph and suppress full-content request/transcript persistence for the process lifetime, closing the retention left partial under 2026-09-12 F-3. |
| GSL-PT-20260913-005 | Repaired | Desktop new-session and cleanup failures render existing JSON-RPC `error.data`, preserving missing-folder and recovery detail. |
| GSL-PT-20260913-006 | Open design decision | This is the previously deferred 2026-09-12 C-9 gap. Secret references do not prove exclusive extension ownership. Safe removal needs ownership metadata and shared-secret semantics; blind deletion could remove another extension's credential. |
| AUD-DAT-001 | Open architecture decision | Global cross-session memory is real, but `MEMORY_MANAGER_EVAL.md` currently specifies cross-session recall. Changing scope requires an explicit workspace-isolation decision and migration plan. Privacy risk remains High. |
| AUD-DAT-002 | Repaired | This corrective follow-up to MEM-GSL-002 makes the bounded JSONL reader use a line-aligned tail window, so recent append-only memories remain visible after the file exceeds 8 MiB. |
| AUD-DAT-003 | Repaired as Low hardening | The reported High silent-success path was overstated because primary ACP verification already fails closed. The residual nudge helper now falls back to a bounded disk check on inventory error instead of assuming success. |
| AUD-DAT-004 | Repaired | Verification now ignores unavailable secondary output roots consistently with closeout while still failing if no usable output root exists. |
| AUD-DAT-005 | Repaired | Atomic truncation clears current-context token counters while retaining accumulated usage and cost. |
| AUD-DAT-006 | Repaired as Low efficiency issue | Tool dispatch now awaits the cancellation future directly instead of waking every 100 ms. |
| AUD-DAT-007 | Repaired; confirmed P0 | Fresh database initialization is serialized across processes with an owner-only advisory lock held across WAL, schema, migration, and legacy-import setup. |
| AUD-DAT-008 | Verified not a defect | ADR-0018 deliberately keys revision history by canonical output path and retains it independently of chat deletion. The reported session-cascade expectation conflicts with that accepted contract. |
| AUD-DAT-009 | Repaired | Import provenance is filtered in SQLite and only a matching session is materialized; unrelated malformed rows no longer poison the lookup. |
| AUD-DAT-010 | Open maintenance decision | Removing legacy columns only from the fresh schema would create fresh/upgraded drift. A complete schema migration and parity decision is required. |
| AUD-DAT-011 | Repaired | Handoff activation validates metadata before mutation, then performs one session- and row-bounded JSON update that preserves unknown fields. |
| AUD-DAT-012 | Open evidence/architecture decision | The synchronous lock is required by ADR-0001, and no contention evidence established executor starvation. Moving the boundary requires a coherent async store design and benchmark. |

The playtest's version presentation, invalid-JSON classification, moved-workspace git diagnostics,
and scenario command-list drift remain notes rather than promoted defects.

## Repair commits

| Group | Commit | Findings |
|---|---|---|
| Memory read window | `84e0b58fe` — `fix(memory): read recent bounded entries` | AUD-DAT-002 |
| Research fail-closed paths | `67f764f4f` — `fix(research): keep deliverable checks fail closed` | AUD-DAT-003, AUD-DAT-004 |
| Chat cancellation and usage | `327039c85` — `fix(session): preserve cancelled turns honestly` | GSL-PT-20260913-002, AUD-DAT-005, AUD-DAT-006 |
| Subagent notification identity | `4aab6a257` — `fix(subagent): deduplicate notification identity` | GSL-PT-20260913-003 |
| Stateless CLI retention | `9be94b4a9` — `fix(cli): make no-session runs ephemeral` | GSL-PT-20260913-004 |
| Provider health diagnostics | `249b08e49` — `fix(doctor): verify the configured provider` | GSL-PT-20260913-001 |
| Multi-process database setup | `4acf5d5ce` — `fix(session): serialize database initialization` | AUD-DAT-007 |
| Storage query efficiency | `4cada69ce` — `fix(session): streamline import and handoff storage` | AUD-DAT-009, AUD-DAT-011 |
| Desktop error fidelity | `109a66ab7` — `fix(desktop): preserve session error details` | GSL-PT-20260913-005 |
| Doctor regression alignment | `b0f387e1c` — `test(cli): keep doctor validation deterministic` | Test contract discovered by integrated validation |
| Multi-stream fan-in evidence | `a9d1f0322` — `test(subagent): cover multi-stream notification fan-in` | Adversarial-review evidence closure for GSL-PT-20260913-003 |

## Verification

### Repair-specific evidence

- The memory regression used a file larger than 8 MiB and failed before the tail-reader repair.
- Both research regressions failed before repair; all 27 focused research tests pass.
- CLI cancellation and usage regressions failed before repair. The complete CLI suite now passes:
  260 unit tests and all integration and doc tests.
- All 8 focused subagent-handler tests, the three-stream fan-in regression, and all 46 Summon
  tests pass.
- Unique prompt/response markers were absent after successful, provider-failed, and SIGINT
  `--no-session` runs.
- Doctor passes against a loopback OpenAI-compatible stream and fails against a closed endpoint.
- The pre-repair process race produced 3 SQLite lock failures in 40 launches across 5 fresh roots.
  The repaired binary produced 0 failures in 80 launches across 10 fresh roots.
- All 14 session-handoff tests and all 6 ACP handoff tests pass.
- All 16 focused Desktop Hub tests pass.

### Integrated evidence

- `cargo fmt --all -- --check` — pass.
- `cargo clippy --all-targets -- -D warnings` — pass.
- `cargo test -p gosling-cli` — pass, including 260 unit tests and every integration/doc test.
- `cargo test -p gosling` with the five documented baseline failures and four suite-order
  keychain cases filtered — every remaining library, integration, binary, and doc test passed;
  the library result alone was 1,888 passed, 3 ignored, 8 filtered.
- The four suite-order keychain cases pass independently: three
  `test_agent_inherits_session_mode` variants and `test_session_mode_isolation`.
- The other testable workspace crates passed 593 tests across `gosling-mcp`,
  `gosling-providers`, `gosling-sdk-types`, and provider integration suites; the remaining selected
  crates had no runnable tests.
- Desktop `pnpm test:run` — 167 files and 1,321 tests passed.
- Desktop `pnpm run typecheck` — pass.

The five known baseline failures remain unchanged: three prompt-manager snapshots and
`prompt_template::tests::test_get_template` read the operator's customized prompt, while
`acp_custom_requests_test::test_custom_preferences_validate_resulting_compaction_pair` fails the
same assertion recorded and baseline-verified on 2026-09-12. Running the full core suite without
filters also exposed a pre-existing test-isolation issue: extension tests mutate process-global
secret-source configuration, after which four execution-manager tests wait in macOS Keychain.
Those four tests pass immediately in isolation.

A literal `cargo test --workspace` remains unavailable because `gosling-test-support` does not
compile with its active feature set: OpenTelemetry metrics and global provider APIs are disabled or
absent. This is the same helper-build limitation recorded by the source playtest. It does not affect
the targeted gosling, CLI, provider, MCP, SDK, or Desktop evidence above.

## Remaining risk and coverage limits

The four open decisions remain in [TODO.md](../TODO.md) and the
[active TODO ledger](../polish/active-todo-ledger.md). In priority order, the material unresolved
risk is cross-workspace memory privacy (AUD-DAT-001), followed by safe credential reclamation,
schema maintenance, and an evidence-backed async workspace-lock design.

This campaign did not rerun all 127 live playtest cards after repair. It replayed the high-signal
repair paths listed above and ran broad source regressions. The signed/installed Desktop app,
native dialogs, signing, updater, multi-window behavior, external backend mode, and the source
playtest's other blocked cards were not revalidated.
