# Live Scenarios Defect Campaign Session Log

Date: 2026-07-20
Status: repair and verification complete; authorized integration to `main` pending

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
| C | GSL-PLAY-003, GSL-PLAY-005, GSL-PLAY-009, GSL-PLAY-012 | verified | `e1b7ded64`, `564b62e07`, `100368130` | `cargo test -p gosling-cli --lib` (235 passed); live Ollama 404 exited 1 with valid JSON `error` metadata; live successful Ollama JSON run exited 0 |
| D | GSL-PLAY-004 | verified | `cc441aa27` | `cargo test -p gosling-cli --lib` (232 passed); formatter passed; live Ctrl-C replay remains in the final gate |
| E | GSL-PLAY-006, GSL-PLAY-007, GSL-PLAY-013, GSL-PLAY-014 | verified | `4b4ac51b8` | Targeted config, hints, doctor, serve-warning, and session-name tests passed; formatter passed |
| F | GSL-PLAY-010 | verified | `b89d5d269` | `cargo test -p gosling --lib plugins::formats::` (11 passed); shared validator regression passed; formatter passed |
| G | GSL-PLAY-015 | verified | `66309eac7` | Installed Apple Silicon Electron replay: `File -> New Chat Window` created two healthy windows; close/reopen and `Cmd+Q` lifecycle passed |

## Final gates

- `cargo fmt --all -- --check`: passed.
- `cargo test -p gosling-cli --lib`: 235 passed.
- `cargo test -p gosling --lib`: 1528 passed; the same four Gate 0 environment-sensitive tests failed, with no new failures.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Desktop typecheck: passed.
- Desktop Vitest: 78 files and 542 tests passed.
- Text UI TypeScript build: passed.
- Live CLI: empty input, finite doctor, empty session name, unauthenticated warning, provider error status, successful Ollama JSON, ACP version rejection/acceptance, and EOF shutdown passed.
- Native package: Electron Forge produced, signed, and installed an `arm64` app at `/Applications/Gosling.app`.
- Live Desktop: onboarding advance, fullscreen/window restore, multi-window creation, close/reopen persistence, and bounded quit passed.
- Source report closure: completed with per-finding commit and evidence mapping.

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

## Group D adversarial review

- Generate a stable ID for the user turn before `Agent::reply` persists it.
- Truncate persistent conversation state from that exact ID and remove only the matching local suffix, avoiding same-timestamp collateral deletion.
- Preserve the pre-interruption history and emit a neutral prompt so the next user message becomes the sole active instruction.
- Suppress interruption prompt rendering in JSON modes so hard-error cleanup cannot pollute machine-readable stdout.

## Group E adversarial review

- Keep missing optional context configuration silent while naming malformed `CONTEXT_FILE_NAMES` and the documented fallback.
- Preserve malformed config files on disk and surface their exact path/parse error through stderr before continuing without that layer.
- Replace model-backed interactive doctor behavior with a finite local system/config report that exits.
- Print the unauthenticated warning only when no server secret is active, before binding the listener.
- Reject empty and whitespace-only names in `SessionManager::create_session`, before any CLI, Desktop, ACP, import, or terminal caller can persist them.

## Group F adversarial review

- Validate every discovered plugin skill before the installer copies the checkout, preventing partial installation for malformed metadata.
- Require parseable YAML frontmatter, a normalized nonblank name, a nonblank description, and a discovery-safe name without `/`.
- Apply the same validator to Gemini and Open Plugins, including custom skill roots.
- Confirm malformed installation leaves no destination directory and valid neighboring format behavior remains intact.

## Group G adversarial review

- Reproduced `File -> New Chat Window` against the installed package and captured the renderer failure `Cannot read properties of undefined (reading 'sender')`.
- Traced the failure to a native menu callback synthesizing an IPC event without an Electron sender.
- Kept renderer-originated window requests on the sender-validating IPC path and routed the native menu directly to the existing window factory.
- Repackaged and reinstalled the signed `arm64` app, then replayed the exact menu action and confirmed two healthy native windows.
- Closed the test windows, reopened the real installed app with prior session state intact, and confirmed `Cmd+Q` terminated Electron and embedded `gosling serve` children in under one second.
