# 2026-11-20 Dedicated email

Implemented a dedicated Desktop mailbox with IMAP/POP intake, SMTP replies, native credential
storage, provider presets, and a host-enforced recipient allowlist. The target was clean main at
`a10224ae2`. The private catalog supplied its Rust implementation workflow. The prior P3
plan and ADRs 0002, 0011, and 0023 informed the review.

## Plan decisions

- Electron main supervises the local Rust worker over private stdin/stdout. A per-mailbox lock
  serializes operations and the child stops when its supervising pipe closes. This fits the
  local Desktop lifecycle and keeps the feature independent of chat and remote ACP backends.
- The operating system credential store uses a separate service for mail. Provider and extension
  credential enumeration does not return its entries. Outbound policy lives in Rust at SMTP
  message construction. The model sees a bounded message, receives no mailbox credentials and
  proposes a typed action without tools.
- An allowed From address qualifies a message for a draft. The user can authorize unattended
  delivery by including a one-time code issued for that exact address. Only the digest persists.
  Authorization, replay prevention and recipient revocation are checked in the worker. This
  replaces the plan's unverified assumption about provider-authentication metadata.
- The Gmail preset uses Google app passwords for eligible accounts. Public Google OAuth still
  requires a client registration and verification path. POP keeps server copies, stores local
  text, and uses UIDL. IMAP uses UID and UIDVALIDITY. Trash restoration uses COPYUID when supplied.
- The ledger records uncertainty before submitting mail or moving a server message. A process
  restart never automatically resubmits an uncertain reply.

## Files changed

Rust mail service, mail transport dependencies, Desktop worker command, Electron main and
preload wiring, Email settings, focused Rust and Desktop tests, and the dedicated-email guide.

## Validation

- `cargo fmt --check` passed.
- `cargo test --locked --offline -p gosling --no-default-features --features rustls-tls,system-keyring --lib mail:: --quiet` passed eight tests, including a captured SMTP envelope, IMAP COPYUID, POP STARTTLS refusal, restart recovery, and one-time code replay.
- `cargo test --locked --offline -p gosling-cli --no-default-features --features rustls-tls,system-keyring --test desktop_email_test --quiet` passed the supervised binary smoke test against an isolated app directory.
- `cargo clippy --locked --offline -p gosling-cli --no-default-features --features rustls-tls,system-keyring --bin gosling -- -D warnings` passed.
- Desktop `pnpm run typecheck` and 11 tests across Email settings, Settings, email IPC, and settings IPC passed using the repository's pnpm 10.30.3.
- `git diff --check`, scoped Prettier, and the dedicated-email documentation links passed.

Live provider sign-in, native keyring integration, a packaged Desktop launch, and non-macOS
execution remain unverified. The shipped Gmail preset in this patch requires an eligible app
password. Google OAuth is a separate release dependency.

## Post-commit review repairs

A review of `441a6245e` found these problems, now repaired:

- `EmailSettingsSection` named a plain helper `useSaved`, which failed `react-hooks/rules-of-hooks`.
  It is now `applySaved`.
- A message the model could not classify blocked the inbox, because the poll retried it forever.
  The ledger now counts failures for each message. After three failed inbox checks the message
  stays in the inbox, the failure is recorded under activity, and processing continues.
- The Electron reaper timeout of 165 s was shorter than a legitimate three-message poll. It is now
  10 minutes, and the Rust worker still bounds each network step.
- When the worker failed to spawn, its kill timer and child reference were left behind. Both are
  now released on `error`.
- The guide now states that **Forget credentials** keeps local activity, drafts, and copies.

Validation: `cargo fmt`; `cargo clippy -p gosling -p gosling-cli --all-targets -- -D warnings`
clean; `cargo test -p gosling --lib mail` (9 passed); `cargo test -p gosling-cli --test
desktop_email_test` (1 passed); ESLint and Prettier on the changed Desktop files; `pnpm run
typecheck`; vitest for the email IPC and settings tests (3 passed). Live mail servers, the
keychain, and a packaged Desktop launch remain unverified.

## Mistral security audit repairs

Rechecked the supplied findings from `441a6245e` against `main` at `f3f6ac642`.
The audit report was already present as an untracked file and was left intact.

- **SEC-MAIL-001** — `Cargo.lock` now resolves rustls 0.23.46 for lettre, tokio-rustls,
  and the other workspace consumers. Cargo also updated the rustls cryptography and certificate
  validation dependencies required by that release. The
  [upstream advisory](https://github.com/rustls/rustls/security/advisories/GHSA-2mjx-qc3c-rqvc)
  confirms the handshake encryption-level check and states that handshake transcript
  authentication was preserved.
- **SEC-MAIL-002** — Electron builds the email worker environment from explicit keys. The
  list preserves app paths, keyring policy, proxy and CA settings, and known model destinations.
  Provider API keys and unrelated parent tokens are omitted. Provider credentials can come
  from the saved gosling configuration. The IPC test covers both the exclusion and the
  preserved inputs, including the resolved mail data root and unattended keyring behavior.
- **SEC-MAIL-003** — Informational. The guide now names the data sent to a remote model
  endpoint and the local Ollama option. The classification path retains its existing scope.

The locked rustls build passed 9 mail tests, 4 ACP TLS tests, and the supervised CLI smoke test
with the `rustls-tls,system-keyring` features. The reverse dependency tree confirmed rustls
0.23.46 for the mail transports. `cargo fmt --check` and `git diff --check` passed.
Desktop typecheck, scoped ESLint and Prettier, and all 3 focused Desktop tests passed.
Live mail providers, the desktop keyring, and packaged Windows and Linux runs remain unverified.

## Mailbox model and instructions controls

Email settings use the shared Desktop model picker with configured direct providers. The
selection is saved with the mailbox settings. The Instructions dialog saves optional triage
and reply guidance, which the worker places in its system prompt under the fixed mail rules.
Existing mailboxes load with empty instructions. The Send allowlist section exposes the saved
recipient policy and contains the automatic reply code controls.

Updated the Rust mail settings and prompt, Desktop Email settings and model picker, provider
capability mapping, focused tests, and the dedicated-email guide.

Validation passed for 10 targeted Rust mail tests, 29 targeted Desktop tests, Desktop typecheck,
scoped ESLint and Prettier, and `cargo fmt --check`. Live mailbox processing and a packaged
Desktop build were outside this validation.

## Inbox connection when SMTP is unavailable

Reproduced the installed worker's SMTP timeout with the saved Purelymail configuration in an
isolated data directory. TCP connected to Purelymail IMAP on port 993. Connections to both
Purelymail SMTP ports (465 and 587) timed out. Gmail SMTP on the same ports timed out as well.

The worker now tests incoming mail and SMTP independently. A successful incoming test enables
inbox processing. Sending and creating automatic reply codes require a successful SMTP test.
If an SMTP retest fails, existing codes are revoked and drafts remain available for review.
Older mailboxes that passed the combined test retain both verified connections.

Validation passed for 11 Rust mail tests, 20 focused Desktop tests, Desktop typecheck, scoped
ESLint and Prettier, and `cargo fmt --check`.

Rebuilt and installed the signed macOS arm64 app and matching CLI without retaining the previous
copies. The installed app, renderer, and backend launched. The supervised worker read the saved
paused mailbox and returned its SMTP readiness. The rebuilt worker's unattended live connection
test stopped at Keychain access. An interactive connection test is needed to verify the new
build against the saved mailbox.

## Email monitoring button

The active monitoring button now reads **Disable email**, paired with **Enable email** when
monitoring is off. The settings guide uses the same label. The focused test reproduced the
missing label and now verifies the round trip through the existing enable and pause requests.
All four focused Desktop tests, typecheck, ESLint, and formatting checks passed. Repackaged and
installed the signed macOS Desktop app with the existing mail worker.

## Separate sender and recipient allowlists

The Receive allowlist gates classification and inbox actions. The Send allowlist gates SMTP
recipients. Accepted messages can receive an automatic reply when the sender is also on the
Send allowlist and SMTP has passed its test. A reply body can be retained as a draft when SMTP
is unavailable. The prior send list is preserved on load. The receive list starts empty, and
mailbox checkpoints stay in place until the user adds a sender.

Removed the reply-code workflow from the Rust worker and Desktop bridge and settings. Older
ledgers load with their reply-code digests ignored. Previously issued codes in message content
remain redacted. Each list keeps its own address validation, with the bounded IPC request size
increased to accommodate both lists.

Passed 13 Rust mail tests, 20 focused Desktop tests, typecheck, ESLint, Prettier, and Rust
formatting checks.

The packaged worker passed a supervised smoke check using an isolated legacy ledger. It read
the old Send allowlist, left the checkpoint in place with an empty Receive list, saved distinct
lists, and rejected the retired reply-code request. Rebuilt and installed the signed macOS app
and matching CLI. The installed worker loaded the existing mailbox and both policy fields.
The Desktop process reached a macOS Keychain prompt before its window opened. Live SMTP
delivery was not part of this check.

## Email capture and Desktop tool approval

Added an opt-in capture mode to the dedicated mailbox. The mail worker applies the Receive
allowlist, selects a configured folder or Unsorted, and stores bounded text in the local
mail ledger. A captured task can open in the normal Desktop composer as an editable draft.
The mode does not enact existing IMAP filing, Trash, or automatic replies.

Added the persisted **Auto approve tool requests** setting for Desktop chats. The ACP client
selects Allow once for each offered approval while the switch is on. The client cancels a
request that has no one-time approval option. Turning the switch off restores interactive
requests. Tests cover selection, missing Allow once, a cancelled setting lookup, and the
settings toggle. Other clients, host-side denials, and native dialogs are outside this setting.

Changed the Rust mail worker and ledger in `crates/gosling/src/mail/`, Desktop ACP request
handling and App and Email settings in `ui/desktop/src/`, and the dedicated email and
permissions guides under `documentation/docs/guides/`.

Validation includes 14 focused Rust mail tests, 32 tests across five Desktop files, Desktop
typecheck, scoped ESLint, formatting, and whitespace checks. No live mailbox or research store was changed.

Remaining plan work includes authenticated unattended task submission, attachment intake,
paper summaries, and verified writes to Zotero, Math MCP, and Alfred Physics. The current
mail sender filter matches the From address, and the existing physics shell exposes only
read access to Math MCP. Those behaviors are insufficient for unattended task admission
and research ingestion.

## Signed email tasks and research intake

Extended local capture with HMAC-signed task admission and a durable replay ledger. Tasks
create a Gosling transcript and persist the answer. The default model path runs without tools.
An explicitly enrolled sender can use a fixed local runner with a configured workspace and
built-ins. The worker rechecks enrollment before launching a queued task.

Increased the capture-mode MIME limit for bounded PDF attachments. The new workflow retains
papers by key, extracts bounded page text, summarizes available PDF text or a Crossref abstract,
and accepts arXiv IDs and links. Signed task responses, research summaries, and independent
destination receipts are visible in Mail activity. The retry action preserves successful receipts.
Unknown outcomes are shown for review and can be retried explicitly.

Added the signed-message, CLI runner, and research adapter scripts. The adapters use Zotero's
v3 item and attachment API, Sampo and Math MCP's source custody and capture-import commands,
and Alfred's research record ingestion. Local configuration selects the exact write scopes.

Verified Math MCP and Alfred against disposable stores. Two Math imports returned the same
package receipt. Verified the Zotero item, summary note, PDF upload, and replay sequence using a
mock transport, and exercised the task runner's arguments, workspace, mode, and stdin with a
fixture executable. The 16 focused Rust mail tests and 19 focused Desktop tests passed,
along with Desktop typecheck, scoped lint, Rust formatting, Python syntax, and whitespace validation. Production mailbox, live Zotero writes, and a packaged app
launch were outside these checks.
