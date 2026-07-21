# Live Scenarios Defect Repair Campaign

Date: 2026-07-20
Source record: `docs/cloud/2026-07-20-live-all-scenarios-playtest.md`
Branch: `repair/live-scenarios-campaign-20260720`
Base: `main` at `4b3bb44d4`

## Scope

This campaign covers every confirmed finding in the latest live all-scenarios playtest report. The uncommitted report is an intentional campaign input. Unrelated deferred work in `docs/TODO.md` is protected.

## Baseline

- `cargo fmt --all -- --check`: pass.
- `cargo test -p gosling-cli --lib`: pass, 229 tests.
- `cargo test -p gosling --lib`: baseline red, 1520 passed and 4 environment-sensitive tests failed (`kill_terminates_exec_process_without_stopping_container` and three Claude Code cancellation/permission tests).
- `pnpm --dir ui/desktop run typecheck`: pass.
- `pnpm --dir ui/text run build`: pass.
- `pnpm --dir ui/desktop run test:run`: baseline red, 533 passed and 5 `Hub.test.tsx` tests failed because the test harness does not provide `ModelAndProviderContext` after Hub began consuming it.

Baseline-red tests are not accepted as repair evidence. Campaign-specific tests and unaffected targeted suites must pass, and final verification must distinguish pre-existing failures from regressions.

## Complete finding inventory

| Group | Finding | Severity | Repair boundary | Intended verification |
| --- | --- | --- | --- | --- |
| A | GSL-PLAY-001 | High | Desktop new-chat error state and draft recovery | Hub submission regression tests and Desktop typecheck |
| A | GSL-PLAY-008 | Medium | Desktop onboarding model selection from live provider models | Onboarding model-resolution tests and Desktop typecheck |
| B | GSL-PLAY-002 | High | ACP provider/model validation against live provider inventory | ACP model validation tests and `gosling` library tests |
| B | GSL-PLAY-011 | Medium | ACP protocol-version negotiation and stdio EOF lifecycle | ACP protocol/EOF regression tests |
| C | GSL-PLAY-003 | High | CLI agent error propagation and machine-output terminal status | CLI session output/error tests and targeted live command |
| C | GSL-PLAY-005 | Medium | Headless JSON/stream JSON stdout purity | Session-builder and CLI output tests |
| C | GSL-PLAY-009 | Medium | Empty headless instruction validation | Run-input parser tests |
| C | GSL-PLAY-012 | Medium | Authoritative execution-budget disclosure and incomplete status | Session execution-limit tests |
| D | GSL-PLAY-004 | High | Interrupted-turn removal from persisted conversation state | CLI cancellation persistence regression test |
| E | GSL-PLAY-006 | Medium | Explicit malformed config and context-name fallback diagnostics | Config and hints tests |
| E | GSL-PLAY-007 | Medium | Deterministic, non-chat `gosling doctor` | Doctor command tests and targeted invocation |
| E | GSL-PLAY-013 | Medium | Terminal-visible unauthenticated serve warning | Serve warning helper test |
| E | GSL-PLAY-014 | Low | Reject blank session names at the persistence boundary | Session-manager validation test |
| F | GSL-PLAY-010 | Medium | Strict plugin skill frontmatter/name/description validation | Gemini and Open Plugins install tests |

## Locality groups and touch sets

### Group A: Desktop new-session UX

Expected touch set:

- `ui/desktop/src/components/Hub.tsx`
- `ui/desktop/src/components/Hub.test.tsx`
- `ui/desktop/src/components/ChatInput.tsx`
- `ui/desktop/src/components/onboarding/OnboardingGuard.tsx`
- `ui/desktop/src/components/onboarding/OnboardingGuard.test.tsx`

### Group B: ACP model and connection lifecycle

Expected touch set:

- `crates/gosling/src/acp/server.rs`
- `crates/gosling/src/acp/server/config.rs`

### Group C: Headless execution contract

Expected touch set:

- `crates/gosling-cli/src/cli.rs`
- `crates/gosling-cli/src/session/builder.rs`
- `crates/gosling-cli/src/session/mod.rs`

### Group D: Interactive cancellation persistence

Expected touch set:

- `crates/gosling-cli/src/session/mod.rs`

### Group E: Startup and persistence guardrails

Expected touch set:

- `crates/gosling/src/config/base.rs`
- `crates/gosling/src/hints/load_hints.rs`
- `crates/gosling-cli/src/commands/doctor.rs`
- `crates/gosling-cli/src/cli.rs`
- `crates/gosling/src/session/session_manager.rs`

### Group F: Plugin skill validation

Expected touch set:

- `crates/gosling/src/plugins/formats/gemini.rs`
- `crates/gosling/src/plugins/formats/open_plugins.rs`

## Gate sequence

For each locality group:

1. Confirm the group-local behavior and tests.
2. Apply the smallest coherent repair plus regression tests.
3. Run formatter and group-targeted verification.
4. Perform adversarial review for bypasses, alternate entry points, error-channel correctness, and state persistence.
5. Review the complete group diff.
6. Create one local conventional commit. Do not push.

After all groups, run final regression, Clippy, Desktop checks, report closure, and campaign-log closure.

## Oversized-file routing

The following intended touch files exceed 2,000 lines and will receive only narrow campaign patches: `crates/gosling/src/acp/server.rs`, `crates/gosling-cli/src/cli.rs`, `crates/gosling-cli/src/session/mod.rs`, `crates/gosling/src/config/base.rs`, and `crates/gosling/src/session/session_manager.rs`. Structural decomposition is routed as follow-up work rather than mixed into defect repair. `crates/gosling/src/agents/agent.rs` is intentionally excluded from the touch set; execution-limit disclosure will be enforced at the CLI output contract without widening the agent loop.

## Completion criteria

- Every GSL-PLAY finding has a repair or an explicit evidence-backed disposition.
- Every repair has a regression test or a documented live verification where deterministic automation is unavailable.
- Machine-readable output remains machine-readable and incomplete/error states cannot report success.
- The source report records closure date, commit pointer, and verification evidence for every finding.
- No push or remote mutation occurs without explicit authorization.
