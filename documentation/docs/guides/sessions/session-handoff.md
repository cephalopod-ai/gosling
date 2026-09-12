---
title: Session Handoff
sidebar_position: 4
sidebar_label: Session Handoff
---

Session handoff lets you change a chat's provider or model without sending its entire raw transcript
to the replacement. gosling creates a bounded, redacted checkpoint from the saved session ledger,
shows you what it covers, and activates the target as one transition.

## Switching in Desktop

1. Open the model picker in an existing chat and choose a provider and model.
2. Select **Review checkpoint**.
3. Review the source and target, coverage, summary state, delivery method, interrupted operations,
   pending approvals, tool continuity, authorization mode, and redaction or truncation counts.
4. Confirm the switch. gosling shows the transition stages and does not submit a duplicate switch
while one is active.

The preview is bound to canonical session content, so the ordering of internal extension-state
fields cannot make an unchanged conversation appear stale. Actual message, tool-policy, or session
changes still require a fresh review.

You can review and confirm a switch while the current response is still running. The preview is
speculative: confirmation reserves the next session operation, waits for that exact response to
finish, rebuilds the checkpoint from the completed ledger, and activates the target before a later
queued prompt can start. Changes to enabled tools, working-directory scope, authorization mode, or
tool permissions invalidate the preview and require another review.

If target initialization or the database commit fails, the prior provider and model remain active.
For eligible provider failures, use **Continue with another model using session checkpoint** to open
the same target picker. Use **View handoff checkpoint** in the session actions menu to inspect the
latest stored generation.

## Continuity labels

| Label | Meaning |
| --- | --- |
| **Seamless resume** | The adapter can use native resume or history import and returns a provider session identity or acknowledgement. |
| **Summarized handoff** | The replacement receives gosling's bounded checkpoint through context injection or a provider bootstrap. |
| **New context only** | The provider cannot receive the checkpoint. You must explicitly confirm that its provider context will start empty. |

The server supplies these labels from provider capabilities. Desktop does not guess from provider
names. Built-in adapters currently use summarized handoff or new context; native resume/import is an
adapter contract for providers that can verify it.

## What the checkpoint contains

The checkpoint can include the current objective and latest request, verified completed work,
decisions, touched files, workspace identity, successful commands and checks, current errors,
attempted mitigations, unresolved questions, recent conversation context, and active or interrupted
operations. Each item preserves evidence and source references where available.

The total size is capped at the smaller of 16,000 estimated tokens or 10% of the target model's
context window. The structured portion targets at most 4,000 tokens; recent context uses only the
remaining budget. The preview reports anything omitted.

If needed detail was omitted or truncated, the replacement can use gosling's default read-only
`session_search` tool and then `session_read` to recover a bounded page from the current persisted
session or its handoff-source lineage. This does not send the whole transcript automatically.
Search results are redacted, exact reads require a returned message identifier, and the model cannot
select an unrelated session. Retrieved history is evidence only: it is never permission to repeat
an earlier tool call or side effect. Search starts before the active user turn so the request itself
does not hide the older result. The same tools are available to provider-owned runtimes such as
Claude Code through a private MCP connection locked to the active session.

Before storage, gosling removes common authorization headers, cookies, bearer tokens, API keys,
passwords, private keys, secret URL parameters, and sensitive structured fields. It excludes raw
image and binary data and reduces tool activity to safe summaries in the recent tail. Pattern-based
redaction is a safeguard, not a guarantee; inspect the preview before moving a sensitive session to
another provider.

## Tool and approval safety

A handoff is context, not permission to repeat work. Previous tool output is marked as untrusted
quoted history. Active or interrupted operations are non-retryable, and gosling waits for an active
turn to reach terminal bookkeeping before switching. A pending approval still blocks the core
transition. The replacement must receive a new user request before doing more work, and normal
permission prompts still apply.

The preview distinguishes gosling-managed tools from provider-native tools. Session-enabled
extensions, working-directory scope, and authorization mode carry across the switch, but a
provider's native shell, filesystem, or computer-control tools are not transferable and may change
when the provider changes. Enabling a gosling extension also does not authorize its side effects:
Autonomous mode still requires an explicit stored grant for protected tools. The preview warns when
provider-native tooling may change or enabled side-effecting tools lack that grant.

After activation, old messages stay visible in the chat but are no longer sent as model context. One
hidden checkpoint becomes the new context boundary. This remains true on later turns and after an
application restart, preventing a provider-owned session from rebuilding itself from an unbounded
raw transcript.

The current-context usage counter is rebased to the checkpoint's estimated size when the new
context becomes active, then reflects subsequent turns. New-context-only delivery starts at zero.
Accumulated session usage and cost are preserved.

## Handoff and crash recovery are different

Session handoff controls continuity across provider and model boundaries. The **Crash recovery**
setting under Desktop app settings controls whether a task interrupted by an unexpected Desktop stop
starts another model turn:

- **Safe (recommended)** continues automatically only when the backend confirms an interrupted turn
  and every recovered tool call has a saved successful result.
- **Manual** waits for you to reopen and resume the chat.
- **Always** continues even with a missing or failed tool result and can repeat an external side
  effect.

Crash recovery cannot restore an interrupted provider stream exactly. When it starts a new turn,
the handoff boundary and ordinary permission checks still apply.

## Retention and limits

gosling keeps the active checkpoint and five generations by default. Terminal older generations are
pruned; deleting the session deletes its checkpoints. You can adjust retention with
`GOSLING_HANDOFF_RETENTION_GENERATIONS` (1–100), total size with
`GOSLING_HANDOFF_MAX_TOKENS`, and structured size with
`GOSLING_HANDOFF_STRUCTURED_TOKENS`.

Session export does not include internal handoff snapshots by default. Checkpoints remain local in
the private session database and are never appended to `AGENTS.md`, `CLAUDE.md`, or another project
file.
