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
