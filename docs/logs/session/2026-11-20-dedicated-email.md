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
