# 2026-09-19 — Hot-path performance fixes from a prior playtest's findings list

## Task

The operator asked to fix a 10-item "performance findings not applied" list
that a prior session in this same conversation had produced (not a fresh
audit — the items were already identified). Each item was read against
current source before any change, fixed with the smallest safe diff, and
verified with the relevant existing test suite plus new targeted tests. Two
items surfaced a genuine conflict with this repo's own governance and are
called out below rather than resolved silently.

## Items and outcome

| # | Finding | Outcome | Files |
| --- | --- | --- | --- |
| 1 | 4 full conversation clones per provider round trip | 2 of 4 removed (`fix_conversation` shadow map now moves messages; reply loop's `conversation_to_compact` clone is now conditional). 2 remain, needing a broader `Cow`/`Arc` refactor not attempted here. | `gosling-providers/src/conversation.rs`, `gosling/src/agents/agent/reply_entry.rs` |
| 2 | Whole-conversation re-tokenization every loop iteration | `ConversationTokenAccumulator` sums only the appended suffix; wired into the reply loop's per-round compaction check. Full-recompute and incremental paths share one decision function so they cannot disagree. Also PERF-GSL-003 — see reconciliation below. | `gosling/src/token_counter.rs`, `gosling/src/context_mgmt/mod.rs`, `gosling/src/agents/agent/reply_stream.rs` |
| 3 | `get_session(id, false)` full per-session index scan for the compaction check | Added `get_session_without_message_stats`, a lightweight sibling that skips the `messages` table `COUNT`/`MAX` scan; only the one confirmed hot-loop caller was switched. Every other `get_session(id, false)` call site (~30+) is untouched — verifying each one's use of `message_count`/`last_message_at` was out of scope for this pass. | `gosling/src/session/session_manager/session_crud.rs`, `session_manager.rs`, `agents/agent/reply_stream.rs` |
| 4 | Full plan snapshot (5 queries) fetched per tool call to check one boolean | Added `latest_plan_status`, a single-row sibling of `plan_snapshot` reused by `PlanService::interaction_policy`. Deliberately drops `snapshot_for_plan`'s defensive active-revision corruption assertion in the new path (documented in code) — that assertion can't affect the open/closed decision, only whether an already-impossible-by-construction data state gets flagged, and it stays enforced on every path that renders a plan. | `gosling/src/session/session_manager/plan_storage.rs`, `gosling/src/session/plans.rs` |
| 5 | Streaming checkpoint re-serializes the whole message every 250ms | **Not changed.** Investigated in depth: the in-memory accumulation (`Conversation::push`) is already amortized O(1) via `String::push_str`, so the only real quadratic cost is the checkpoint's JSON re-serialization, and it is on the critical path (awaited before the next stream event is yielded). A safe fix needs either an incremental JSON cache (requires trusting that a content block, once no longer last, never mutates again — true today by construction in `Conversation::push`, but a correctness-sensitive assumption to lean on in the crash-recovery write path) or moving the write off the critical path (a concurrency-model change to the durability anchor). Estimated real cost is low single-digit milliseconds even at extreme response lengths (JSON string escaping is fast; SQLite is WAL + `synchronous=NORMAL`, no per-commit fsync) — not a user-visible stall. Left as a documented follow-up rather than forced. |  |
| 6 | No batch endpoint for `sessionOutputsHistory` | Added `_gosling/unstable/session/outputs/latest_batch` (`GetLatestOutputRevisionsRequest`/`Response`), authorizing each path individually but querying latest revisions in one batched SQL statement. Regenerated the ACP schema and TypeScript SDK (`just build-sdk`). Rewrote `getLatestOutputRevision` in the Desktop client as a same-tick request coalescer over the new endpoint; the old per-path concurrency semaphore and its "drop already-aborted rows before sending" test coverage no longer apply and were replaced with coalescing-specific tests. | `gosling-sdk-types/src/custom_requests.rs`, `gosling/src/session/session_manager/output_revisions_storage.rs`, `gosling/src/acp/server/custom_dispatch.rs`, `ui/sdk/src/generated/*` (regenerated), `ui/desktop/src/acp/outputRevisions.ts` (+ test) |
| 7 | Mention popover: wasted `listFiles` IPC per extensionless entry, uncapped fuzzy sort | `listFiles` now returns `{name, isDirectory}` via `fs.readdir(..., {withFileTypes: true})` instead of plain names, so the popover classifies files/directories directly instead of guessing by extension and then probing extensionless entries with a second `listFiles` call. Also fixes a latent misclassification (a directory named e.g. `image.png` was previously shown as a file). Added a `MAX_DISPLAY_RESULTS = 50` cap after the sort in both the browse and fuzzy-match branches. `MentionPopover.tsx` has no pre-existing test harness; verified via typecheck and lint only. | `ui/desktop/src/preload.ts`, `ui/desktop/src/main/fileIpc.ts`, `ui/desktop/src/components/MentionPopover.tsx` |
| 8 | Artifact-routing publish broadcasts a global timestamp re-read; panel switches redo filesystem I/O | Added a shared, short-TTL (3s) cache in `useArtifactFileTimestamps`, consulted only on a plain remount (e.g. switching back to a tab); a `focus` event or an explicit `ARTIFACT_TIMESTAMPS_REFRESH_EVENT` always bypasses it, so a genuine change is never hidden behind a stale value. Did **not** change the event itself to carry a payload of affected paths — `ArtifactPane`'s two other listeners (repository classification, title refresh) still redo their full re-check on every global signal; making that precise would mean redesigning the event payload and three separate invalidation paths, a larger change than this pass attempted. | `ui/desktop/src/hooks/useArtifactFileTimestamps.ts` (+ test), `ui/desktop/src/test/setup.ts` |
| 9 | Desktop CI (`macos-latest`) has pnpm caching commented out for a runner bug | Re-enabled. Checked the two linked issues (`actions/runner-images#13341`, `actions/runner/#4134`) via `gh issue view`: both are closed as completed (2026-04-03 and 2025-12-12), with a GitHub Actions team member confirming a server-side fix ("all affected runners have been restored... a recurrence is not expected"). Verified the YAML parses; **not** verified against a real CI run — that happens on the next push. | `.github/workflows/ci.yml` |
| 10 | PERF-GSL-003 marked partial, pending a real turn profile | See reconciliation below. |  |

## PERF-GSL-003 record reconciliation

Items 1 and 2 above are exactly `audit-performance-profile.md`'s PERF-GSL-003
finding (per-turn conversation clones and full-history re-tokenization).
That audit's own §6 and §9 explicitly say: **"Do not touch the turn-loop
clones... until a profile (PERF-GSL-003 break-it harness) shows a
non-trivial `p`,"** and estimates `p ≲ 0.01` (turn latency) for typical
sessions — i.e. this finding was deliberately left unfixed by this repo's
own governance across at least three prior passes
(`2026-08-27-all-remaining-todos-repair-campaign.md`,
`2026-09-06-snippet-optimization.md`, `2026-09-06-session-storage-optimization.md`),
each one re-confirming the same "Candidate, Measure — pending a turn
profile" disposition rather than fixing it.

This session fixed it anyway, at the operator's explicit, direct request
made in the current conversation (not as a fresh audit finding). The
`docs/TODO.md` PERF-GSL-003 entry is updated in place to describe what
changed, kept at `[~]` (partial, not resolved) because:

- The formal break-it harness (synthetic 300-turn session, `t(2T)/t(T)`
  wall-clock scaling ratio) was not built. This remains a real, separate
  task if a measured number is ever wanted.
- Two of the four original clone sites remain, by design (see item 1).
- The adjacent "two `get_session` reloads per turn" finding, called out in
  the same PERF-GSL-003 entry, is untouched.
- No count-based scaling-ratio guardrail test (the audit's own recommended
  validation method, "immune to timing noise") was added; the new tests
  verify correctness of the incremental accumulator and the reduced clone
  count, not that per-turn cost stays flat as history grows.

The risk the audit was guarding against — an unverified micro-optimization
to a correctness-sensitive turn loop — was mitigated by keeping every change
narrow and re-running the full conversation/moim/compaction/agent-integration
suites (test counts recorded in this conversation, not restated here) rather
than by skipping the fix.

## Validation

- `cargo check --workspace`, `cargo fmt -p gosling -p gosling-providers -p
  gosling-sdk-types`.
- Targeted `cargo test` runs per item (`context_mgmt::`, `session_manager::`,
  `agents::agent::`, `plan_storage::`, `session::plans::`,
  `output_revisions_test`, `acp_custom_requests_test`, `moim::`,
  `conversation::`) — all green; no pre-existing failures encountered this
  session.
- `just generate-acp-schema`, `just generate-acp-types`, `just build-sdk` for
  item 6's new wire method.
- `pnpm run typecheck`, `pnpm run lint`, and the full `pnpm exec vitest run`
  suite (179 files / 1425 tests) in `ui/desktop` — all green, including after
  the global test-setup change in item 8 (which required finding and fixing a
  cross-test-file cache-pollution failure before it was clean).
- Rebuild/reinstall of the packaged app was not performed in this pass; all
  verification is `cargo test` / `vitest` / `tsc` / `eslint`, not a live GUI
  playtest.

## Risks and follow-ups

- Item 5 (streaming checkpoint) and the "broadcast is untargeted" half of
  item 8 are documented but not fixed — see their rows above for why.
- Item 9's fix is unverified against real CI; if the `hashFiles()` step
  fails again on the next `macos-latest` run, that regression report is the
  first thing to check, and the two linked upstream issues are the place to
  look for a recurrence report.
- Item 3 only converts the one hot-loop `get_session(id, false)` call site;
  the other ~30 call sites were not audited for the same opportunity.
- No commit, merge, or publication was requested or performed in this
  session.
