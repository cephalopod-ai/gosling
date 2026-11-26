# 2026-11-25 — Provider, session, and mail audit

## Scope and method

The audit started on clean `main` at `115f08b9e` and used current source, Git
history, the transport and session ADRs, and repository tests. The first pass
followed the recent Muse Code, desktop mail, and model-context changes from setup
and user input through execution, persistence, and the visible result. The second
pass started at process output, a failed turn, a reopened session, and a restarted
mail worker. It traced those states back to their producers and checked partial
writes, invalid event order, unknown counts, full pipes, and oversized input.

## Findings and repairs

| Finding | Evidence and change |
| --- | --- |
| An errored Muse turn could run again | `exit_chat` bypassed goal, grind, and research nudges, but queued steering and a Stop hook could reopen the reply loop. The generic retry branch also trusted the absence of gosling tool requests. Muse now closes its steering window before publishing the error, records queued messages and partial output for the user, and exits the reply loop. Queued text must be explicitly resent to reach a provider. |
| A streamed prefix could acknowledge a handoff | The agent acknowledged the checkpoint on the first visible chunk. Muse now acknowledges it after the protocol supplies a matched terminal answer and a successful process exit. The reply loop also skips background tool-pair summaries for Muse because that separate call runs through the same external tool runtime. |
| Active-context usage fell to zero when a Muse turn ended | The baseline ACP stdio replay reported an active estimate of 3,067 tokens and then a final context count of zero, alongside a valid 17-token run total. The reply loop now persists its local context estimate and the optional run total in one update. ACP returns reported run totals in the prompt response. A run with unknown usage clears the old last-request figure and preserves accumulated counts. Standard per-request accounting uses the provider that answered, including on an allowed failover. |
| Malformed Muse records could be ignored or detected late | Known run events require a valid identity. A first link that contradicts the selected root fails, and foreign run output remains excluded. Conflicting terminal text fails when the event arrives, including when the process keeps stdout open. Cache totals require a consistent breakdown for every model. Model discovery uses a set to deduplicate IDs in output order without scanning the growing list for each model. |
| One oversized POP message could stall the inbox | When the body exceeded the per-line limit despite passing `LIST`, the worker treated the message as a connection error and revisited it on the next poll. The retrieval now returns a skipped message, persists the UIDL, and drops that connection before processing later mail. Control-line and framing errors still surface as connection errors. |
| A connector producing oversized output could block until timeout | The cap was checked after both pipe operations completed. If a connector wrote its response before reading enough stdin, the reader stopped at the cap while the writer still waited. The read future now reports the limit immediately and releases the owned process. Workflow settings are bounded at read time. PDF text and results use Unicode-safe truncation. |
| A recovered research record could advertise a checksum for different bytes | A worker can stop after storing the PDF and before recording the metadata. Re-fetching a mutable arXiv reference could produce different bytes while the store retained the original file. The next poll checks for a retained file first and derives its checksum from the bounded file on disk. |

## Reviewed boundaries and follow-ups

- The Muse billing probe and inference run as separate processes against the CLI's
  credential store. The existing fail-closed status checks and the explicit
  acknowledgment for unavailable evidence still apply. A live positive allowance
  response and a login change between probe and execution remain unqualified.
- Automatic and manual context compaction can still invoke a Muse inference when
  it is the selected provider. Current source uses a summary prompt and an empty
  gosling tool list. Muse retains its own hooks, configured tools, and sandbox.
  A host-enforced tool-free mode for those calls needs a qualified Muse posture
  or a separate compaction provider. The audit patch retains the existing
  compaction behavior.
- The mail audit also traced mailbox identity changes, SMTP recipient checks and
  uncertain delivery, the signed-task nonce ledger, IMAP restore, and Electron
  worker supervision. The model-settings review traced metadata, override
  persistence, and the route cap through the desktop chooser and ACP.
- Broader extension, plugin, and shell workflows and other provider families
  were outside this audit. Live Meta inference, live mail and research
  destinations, packaged Electron, and Windows and Linux process behavior
  remain outside the executed matrix.

## Validation

- Focused Rust tests passed for the Muse failure and cache regressions (2),
  run-usage storage (1), mail (21), ACP usage projection (8), the Muse provider
  integration target (13), Codex route and provider behavior (52), saved context
  overrides (2), Stop hooks (5), and ACP handoff (6). The installed-Muse echo test passed with a
  disposable profile, with shell and write disabled and no authenticated model
  request.
- Desktop typecheck and 22 focused tests passed across the model-switch dialog,
  provider configuration, email settings, and email IPC.
- Both changed guides compiled with MDX.
- The rebuilt CLI passed the supervised desktop email smoke test. The ACP stdio
  replay then verified a reported 17-token run, a run with no usage report, and
  usage after closing the backend and loading the same session. The saved active
  estimate stayed positive and the accumulated input stayed at 11 tokens. The
  unknown run cleared last-request usage from both the notification and the prompt
  response. The same replay passed with automatic compaction disabled.
- Scoped Clippy passed with warnings treated as errors for the `gosling` library
  using `rustls-tls,system-keyring`.
- Cargo formatting, Git whitespace checks, the audit index link, and the Cargo
  and pnpm lockfile comparisons passed.
