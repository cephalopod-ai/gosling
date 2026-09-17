# 2026-09-16 — Long-running chat heartbeat and status indicator

## Task and scope

The user requested a heartbeat and status check for long-running work, suggesting
every 15 minutes and supplying a Desktop screenshot of assurances about background
agents. Baseline: `main` at `87e295606`, initially clean. Required repository read
order was followed. The private catalog's `audit-operator-signal` skill guided the
focused assessment of liveness, progress, and missing-signal visibility; this was
not a repository-wide audit or an independent review.

The existing `LoadingGosling.tsx:65` presentation chooses its icon and message from
chat state. It does not itself check liveness or last output. Existing Summon task
tracking supplies actual task handles, turn counters, idle time, and non-consuming
`load(source, peek: true)` checks. The change connects those existing contracts to
Desktop without changing Rust execution or adding model requests.

## Detection and signal map

| Event | Detection | Visible signal | Timing | Next action |
|---|---|---|---|---|
| Active turn produces no new output | Activity timestamp and timer | No recent progress | 15 minutes without output | Review activity and Check now |
| Background agent remains idle | Existing Summon peek reports idle time | Agent state, turns, idle time | Automatic checks every 15 minutes | Check now or Open agent chat |
| Backend/task check hangs, fails, or is denied | Bounded request and failed-result handling | Status unavailable or Unverified, with reason | 10-second check timeout | Check again or inspect the saved agent chat |
| Agent completes, fails, or is cancelled | Live task status or collected terminal result | Completed, Failed, or Cancelled | Next check or matching tool response | Review the saved result |
| Turn pauses for user input | Existing chat state | Waiting for you | Immediate | Answer or approve the pending request |

## Changes

- `ui/desktop/src/acp/chatSessionStore.ts`: retain prompt start time, last observed
  output, and background-task references. User echoes, metadata-only updates, and
  empty stream chunks do not refresh the activity clock. Task discovery runs on
  tool events and history loads rather than every streamed text chunk.
- `ui/desktop/src/acp/backgroundTasks.ts`: discover only matched asynchronous
  Summon delegate results; ignore imported history and assistant prose. Recognize
  multiline task descriptions and collected terminal task results.
- `ui/desktop/src/acp/runStatus.ts`: check existing session-info and Summon peek
  endpoints. Classify stale output separately from unavailable checks. Bound the
  displayed check to 10 seconds and reuse outstanding requests to avoid overlapping
  hung calls. Peek preserves task results and does not cancel them.
- `ui/desktop/src/hooks/useRunStatus.ts`: check immediately and every 15 minutes,
  refresh overdue checks on focus/visibility changes, queue checks for newly
  discovered agents, and clean up timers/listeners. Late results cannot update a
  different session or connection generation. Stop polling terminal background work.
- `ui/desktop/src/components/RunStatusControl.tsx` and `BaseChat.tsx`: accessible
  header control with check/output ages, backend-response evidence, background
  details, Check now, and Open agent chat. Added header spacing prevents overlap
  with the conversation.
- Focused tests cover those boundaries; existing notification/controller fixtures
  include the new snapshot fields.
- Generated English/source fingerprints and 15 locale catalogs were synchronized
  and compiled. There are 25 new status messages using the established English
  fallback for untranslated locales. Extraction also added the two existing source
  keys `navigationPanel.branchFailed` and `navigationPanel.branchSession`, which
  were missing from the baseline catalogs. All previous locale entries are intact.
- `.gitignore`: allow this required session evidence record to be tracked.

## Validation

- `cargo fmt` — passed; no Rust files changed.
- `pnpm run typecheck` — passed.
- Targeted ESLint with `--max-warnings 0` over all touched TypeScript files — passed.
- Targeted Prettier check over all touched TypeScript files — passed.
- `pnpm exec vitest run` for `runStatus`, `useRunStatus`, `RunStatusControl`,
  `chatSessionStore`, `chatSessionController`, `chatNotifications`, `useChatSession`,
  and `sessionNotificationAdapter` — 8 files, 97 tests passed.
- `pnpm i18n:compile` and `pnpm i18n:check` — passed; 21 synchronization tests and
  validation of all 15 non-English catalogs passed.
- Locale comparison against `HEAD` — passed after accounting for the two existing
  navigation source keys; exactly 25 status additions, no previous entries changed.
- `git diff --check` — passed. The new record appears in untracked-file discovery,
  is not ignored, and both required documentation-governance markers remain intact.

## Limits and follow-ups

Validation is focused automated coverage, not a live 15-minute Electron run. The
installed Desktop was not rebuilt or replaced, and the full UI suite was not run.
Rust build/test/clippy were not run under the repository's command restriction.

Background detail covers agents managed by Gosling's Summon delegate tool. Opaque
provider-native background jobs and detached shell processes are not independently
enumerated or probed. During an active turn, their visible stream output contributes
to the foreground activity clock. A responding backend alone never establishes that
those processes or agents are making progress. Missing handles or denied checks stay
unverified; silence is a possible stall, not proof of death. No automatic interruption
or restart was added.
