# Live Scenarios Defect Campaign Session Log

Date: 2026-07-20
Status: in progress

## Gate 0: repository and baseline

- Confirmed `main` matched `origin/main` at `4b3bb44d4` before branching.
- Confirmed the only initial worktree path was the user-designated uncommitted audit report.
- Created local branch `repair/live-scenarios-campaign-20260720`.
- Read repository policy, contribution guidance, deferred-work register, and the complete repair-defect-campaign skill package.
- Searched the private skill catalog first; it returned no reusable catalog match, so the explicitly requested local campaign skill was loaded and followed.
- Captured formatter, Rust library, Desktop typecheck/test, and text-UI build baselines in the campaign plan.

## Gates 1-2: inventory and grouping

- Inventoried all 14 confirmed report findings; none were omitted as blocked-card prerequisites.
- Grouped findings by shared runtime boundary rather than report order.
- Recorded expected touch sets, verification, oversized-file constraints, and protected deferred work in the campaign plan.

## Decisions

- Preserve safe fallback behavior for malformed optional configuration, but surface the exact source and fallback through the user-visible terminal channel.
- Validate dynamic provider models through the provider itself when static inventory is insufficient.
- Treat provider failures and execution-budget exhaustion as non-success in machine-readable modes.
- Remove cancelled turns from persistent history, not only the CLI render buffer.
- Reject malformed plugin skills before installation rather than installing content that discovery later degrades or skips.

## Group progress

| Group | Findings | Status | Commit | Evidence |
| --- | --- | --- | --- | --- |
| A | GSL-PLAY-001, GSL-PLAY-008 | verified | `3dce5e6bb` | `pnpm --dir ui/desktop exec vitest run src/components/Hub.test.tsx src/components/onboarding/OnboardingGuard.test.tsx` (9 passed); Desktop typecheck passed |
| B | GSL-PLAY-002, GSL-PLAY-011 | verified | `2338d7e85` | `cargo test -p gosling acp::server::tests::` (71 passed); formatter passed; live Ollama replay remains in the final gate |
| C | GSL-PLAY-003, GSL-PLAY-005, GSL-PLAY-009, GSL-PLAY-012 | verified | `e1b7ded64` plus pending cleanup | `cargo test -p gosling-cli --lib` (231 passed); formatter passed |
| D | GSL-PLAY-004 | pending | | |
| E | GSL-PLAY-006, GSL-PLAY-007, GSL-PLAY-013, GSL-PLAY-014 | pending | | |
| F | GSL-PLAY-010 | pending | | |

## Final gates

Pending group repairs, adversarial review, full regression, Clippy, source-record closure, and final campaign closure.

## Group A adversarial review

- Rejected text-only draft reconstruction because it would lose image attachments.
- Moved clear/history side effects behind asynchronous submit acceptance in `ChatInput`, preserving the entire draft on a rejected session creation.
- Confirmed onboarding rejects unavailable explicit models and empty live inventories instead of persisting a known-bad default.
- Routed a Desktop GUI replay of failed new-chat submission to the final campaign gate because there is no existing dedicated ChatInput component harness.

## Group B adversarial review

- Kept static provider inventory as the fast validation path and consulted the live provider only for otherwise unknown models.
- Reused the shared model validator for defaults-save so onboarding and session creation cannot disagree about a dynamic model.
- Moved protocol negotiation into a directly tested helper and reject the schema's legacy/string fallback version before accepting client capabilities.
- Tested both EOF detection and the race that terminates an otherwise pending ACP connection.
- Routed installed-Ollama `qwen2.5:latest` and end-to-end stdio replay to the final live gate.

## Group C adversarial review

- Confirmed terminal agent errors set JSON status to `error`, suppress stream completion, and propagate a nonzero process result.
- Confirmed JSON and stream-JSON suppress the human session banner while text output remains unchanged.
- Reject empty and whitespace-only instructions before session/provider creation.
- Persist an authoritative execution-limit notice after repetition or turn-budget exhaustion and return non-success so a model completion claim cannot become the terminal contract.
- Restricted repetition detection to user-role tool responses and max-turn detection to assistant-role notices to avoid phrase-based false positives in arbitrary prose.
