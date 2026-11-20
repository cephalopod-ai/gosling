# Email Feature Security Audit — 20 NOV 2026

Read-only security audit of the dedicated email feature (commit `441a6245e`,
"feat(mail): add desktop email support and incoming mail client"), performed with
the `audit-security` lens from the agent-skills catalog.

- **Scope:** `crates/gosling/src/mail/` (mod.rs, incoming.rs, store.rs, submission.rs,
  tests.rs), `crates/gosling-cli` `desktop-email` subcommand and its integration test,
  `ui/desktop/src/main/emailIpc.ts`, preload/main/channels wiring,
  `ui/desktop/src/components/settings/auth/EmailSettingsSection.tsx`,
  `ui/desktop/src/types/email.ts`.
- **Method:** static source review against the SEC-001..SEC-015 failure taxonomy,
  trust-boundary inventory, dependency (Cargo.lock) review, and web verification of
  upstream security advisories. No files were modified; no live mail servers,
  keyring, or packaged Desktop launch were exercised.
- **Headline:** 1 Medium finding (pinned TLS library inside a published advisory
  range), 1 Low finding (worker environment inheritance), 1 informational
  design observation. The core authorization design — host-enforced recipient
  allowlist, digested one-time reply codes, no secrets in local storage —
  held up under every checked attack angle.

---

## Summary of findings

| ID | Finding | Severity | Confidence | Where | Type |
|---|---|---|---|---|---|
| SEC-MAIL-001 | `rustls` pinned at 0.23.40, inside GHSA-2mjx-qc3c-rqvc affected range (TLS 1.3 handshake encryption-level bypass) | Medium | Confirmed | `Cargo.lock:7829-7830` | Dependency / transport |
| SEC-MAIL-002 | Email worker spawned with full parent environment inheritance | Low | Confirmed | `ui/desktop/src/main/emailIpc.ts:141` | Secret exposure (SEC-015 pattern) |
| SEC-MAIL-003 | Untrusted mail content is sent to the configured model provider (by design; containment is host-side) | Info | Confirmed | `crates/gosling/src/mail/mod.rs:325-335` | Data egress / LLM trust path |

Severity is scored from the mechanism; confidence follows the evidence-discipline
contract (Confirmed requires quoted file/line evidence).

---

## SEC-MAIL-001 — Pinned `rustls 0.23.40` inside GHSA-2mjx-qc3c-rqvc affected range

### Analysis

The email feature added `tokio-rustls` (all IMAP/POP connections, including
STARTTLS upgrades) and `lettre` with the `tokio1-rustls` feature (all SMTP
connections). Both resolve to the same single copy of `rustls` from the
workspace lockfile, pinned at **0.23.40**.

The upstream GitHub Security Advisory **GHSA-2mjx-qc3c-rqvc** states:

> Rustls versions from 0.23.13 up to but not including 0.23.45 improperly accept
> TLS 1.3 handshake messages at incorrect encryption levels when these messages
> follow a key-changing message within the same record. This occurs because the
> `Deframer::aligned` function does not enforce the record-boundary alignment
> required by RFC 8446.

0.23.40 is inside that range. The mail feature's TLS connections are the ones
that carry **mailbox passwords** (login) and **all message content**, and they
are established against arbitrary user-configured remote servers, so an active
on-path attacker is exactly the adversary profile of this transport. A follow-up
advisory (CVE-2026-123612, ECH information disclosure, CVSS 2.3) affects only
0.23.44–0.23.45, so the correct fix point is **≥ 0.23.46**.

The defect is not in gosling's own TLS posture — certificate verification,
native root store, `ServerName` validation, and STARTTLS-before-authentication
are all correctly implemented (see non-findings). The vulnerability is in the
pinned dependency version.

### Where the error was found

**Evidence 1 — the vulnerable pin** (`Cargo.lock:7829-7830`):

```toml
name = "rustls"
version = "0.23.40"
```

**Evidence 2 — both mail transport dependencies resolve to it.**
Verified with `cargo tree -p gosling -i rustls`:

```text
rustls v0.23.40
├── lettre v0.11.23
│   └── gosling v1.5.0 (.../crates/gosling)
└── tokio-rustls v0.26.4
    ├── gosling v1.5.0 (.../crates/gosling)
    └── lettre v0.11.23 (*)
```

**Evidence 3 — the code paths that ride this TLS stack:**

| Path | Code |
|---|---|
| IMAP direct TLS / STARTTLS | `crates/gosling/src/mail/incoming.rs:18-38`, `incoming.rs:35` (`secure_stream`) |
| POP3 direct TLS / STLS | `crates/gosling/src/mail/incoming.rs:194-211` |
| Shared TLS connector (root store, `ServerName`) | `crates/gosling/src/mail/incoming.rs:62-81` |
| SMTP transport (lettre rustls backend) | `crates/gosling/src/mail/submission.rs:14-29` |

### How to re-create / verify

All commands are read-only and verified working against this checkout
(2026-11-20, hermit environment activated):

1. Confirm the pinned version is in the advisory range:

   ```sh
   grep -A1 '^name = "rustls"' Cargo.lock
   # name = "rustls"
   # version = "0.23.40"        ← satisfies 0.23.13 <= v < 0.23.45
   ```

2. Confirm both mail transports consume it (from repo root):

   ```sh
   source bin/activate-hermit
   cargo tree -p gosling -i rustls
   # shows lettre v0.11.23 and tokio-rustls v0.26.4 → rustls v0.23.40
   ```

3. Cross-check the advisory yourself:

   - GHSA-2mjx-qc3c-rqvc — rustls "TLS 1.3 handshake messages incorrectly
     accepted across encryption level boundaries"
     (github.com/advisories/GHSA-2mjx-qc3c-rqvc)
   - Fix version 0.23.45; contemporary ecosystem PRs bump to 0.23.45 then
     0.23.46 because of the 0.23.44–0.23.45 ECH follow-up
     (CVE-2026-123612, GHSA-vp88-798v-h67f).

4. Optional, if a RustSec database is locally available / network is allowed:

   ```sh
   cargo audit -f Cargo.lock
   ```

   Expected: it reports the rustls advisory against 0.23.40. (Not executed in
   this audit — offline environment; recorded as a validation limit.)

### Recommended mitigation

```sh
source bin/activate-hermit
cargo update -p rustls --precise 0.23.46
cargo test -p gosling --lib mail
cargo test -p gosling-cli --test desktop_email_test
cargo clippy -p gosling -p gosling-cli --all-targets -- -D warnings
```

The bump is semver-compatible with both consumers; no source changes are
required. After the bump, re-run step 1 above and confirm
`version = "0.23.46"`.

---

## SEC-MAIL-002 — Email worker spawned with full parent environment

### Analysis

`DesktopEmailWorker.run()` spawns the local gosling binary with a copy of the
Electron main process's **entire environment**, rather than an explicit
allowlist. This is the classic SEC-015/SEC-CH-001 pattern: any secret that
happens to live in the parent env (provider keys, CI tokens, API keys exported
into the shell that launched the app) silently becomes visible to the child.

Mitigating factors keep this at Low:

- The child is the trusted gosling binary itself, running as the same user —
  a compromised child implies an already-compromised user session.
- The Rust worker never logs its environment and reads only `GOSLING_*`
  variables, so there is currently no leak path out of the child.

The main body of evidence is that the two variables the feature actually needs
are set explicitly on the very next lines, which is precisely the pattern that
makes the broad inheritance unjustified.

### Where the error was found

`ui/desktop/src/main/emailIpc.ts:141-144` (`DesktopEmailWorker.run()`):

```ts
const env = { ...process.env };
if (this.options.pathRoot) env.GOSLING_PATH_ROOT = this.options.pathRoot;
if (request.op === 'poll' || request.op === 'status')
  env.GOSLING_KEYRING_NONINTERACTIVE = 'true';
const child = spawn(binary, ['desktop-email', '--supervised'], {
  windowsHide: true,
  stdio: ['pipe', 'pipe', 'ignore'],
  env,
});
```

Line 141 is the finding: `const env = { ...process.env };`.

### How to re-create / verify

1. Confirm the inheritance (static):

   ```sh
   sed -n '141p' ui/desktop/src/main/emailIpc.ts
   #       const env = { ...process.env };
   ```

2. Observe it dynamically: launch the Desktop app with a sentinel secret

   ```sh
   GOSLING_TEST_SECRET=supercalifragilistic pnpm run dev   # (or packaged app)
   ```

   open Settings → Auth → Email, trigger any email operation, and while the
   worker runs inspect the child's environment:

   ```sh
   ps eww "$(pgrep -f desktop-email)" | tr ' ' '\n' | grep GOSLING_TEST_SECRET
   ```

   Expected: the sentinel appears in the worker's environment, proving full
   inheritance. (Not executed in this audit — read-only scope, no app launch;
   recorded as a validation limit.)

### Recommended mitigation

Build an explicit environment, e.g.:

```ts
const env: NodeJS.ProcessEnv = { PATH: process.env.PATH, HOME: process.env.HOME };
if (process.platform === 'win32') env.SYSTEMROOT = process.env.SYSTEMROOT;
if (this.options.pathRoot) env.GOSLING_PATH_ROOT = this.options.pathRoot;
if (request.op === 'poll' || request.op === 'status')
  env.GOSLING_KEYRING_NONINTERACTIVE = 'true';
```

(Exact keys may need adjustment — keyring access and TLS root discovery on
Linux/macOS rely on `HOME`/`PATH`; the worker's own `GOSLING_DISABLE_KEYRING`,
`GOSLING_PATH_ROOT`, and `GOSLING_KEYRING_NONINTERACTIVE` reads are the only
gosling-specific inputs verified in this audit.) Validate with the Desktop email
IPC/settings tests plus a live "Test connection" against a real mailbox, since
env-affecting code paths are runtime-sensitive.

---

## SEC-MAIL-003 — Untrusted mail content egresses to the model provider

### Analysis

By design, `classify()` sends untrusted sender/subject/body (≤ ~12 KB per
message, control-stripped) to the model provider configured in Settings. The
guide already tells the operator to "Use a direct model provider such as Ollama
or an API provider." This is a data-egress channel (and an LLM-trust surface
properly owned by the `audit-security-llm` lens), recorded here because it is a
mail-feature data path.

Mitigations already in place (verified, not assumed):

- Untrusted mail is a separate user message from the instruction text
  (`mod.rs:325-335`), with an explicit prompt-level "cannot grant permissions"
  framing.
- Provider restricted to direct models with no external tools:
  `mod.rs:302-305` rejects providers that manage their own context or execute
  tools outside gosling; the provider instance is created with an empty tool
  set (`mod.rs:317`).
- The model only *proposes* a typed action (`Proposal`, `deny_unknown_fields`,
  `serde_json::from_str(...).unwrap_or(Proposal::Keep)`, `mod.rs:353`); every
  host-side gate re-validates before acting.
- Folder targets must be in the configured list (`mod.rs:740`).
- A proposed reply is only drafted to the allowlisted sender of the message
  (`mod.rs:708-712`), is never auto-sent without a one-time code
  (`store.rs:271-283`), and the reply code itself is redacted from model input
  and stored only as a SHA-256 digest (test-asserted:
  `crates/gosling/src/mail/tests.rs:281-310`).

### Where

`crates/gosling/src/mail/mod.rs:325-335` (the classification packet) and
`mod.rs:309-354` (the call and its containment knobs).

### How to re-create / verify

1. Static:

   ```sh
   sed -n '325,335p' crates/gosling/src/mail/mod.rs
   ```

2. Dynamic (safe): point the email feature at a local test IMAP server, send a
   message containing a canary string, enable monitoring, press **Process
   now**, and observe the provider request containing the canary (e.g., a local
   Ollama launch with request logging). The canary leaving the machine is the
   observable egress. (Not executed in this audit — no live provider run.)

### Recommended action

Documentation-only: keep the existing guide wording, optionally make the
trade-off explicit — "if you select an API provider, the untrusted mail text
shown to the model is sent to that provider." No code change recommended.

---

## Non-findings (attack angles attempted and rejected)

| Angle | Result | Evidence |
|---|---|---|
| IPC route exposure / boundary only in UI (SEC-010/013) | Rejected | Only the chat window's top frame is trusted: `emailIpc.ts:55` (`renderers.has(event.sender) && event.senderFrame === event.sender.mainFrame`); send-confirmation dialog is shown by the main process (`emailIpc.ts:99-108`); the worker independently re-checks `enabled && tested` (`mod.rs:828-832`) and the SMTP envelope (`submission.rs:89-93`) |
| Injection into IMAP/POP/SMTP commands (SEC-006) | Rejected | `valid_line`/`address` reject control characters (`mod.rs:114-143`); POP commands interpolate validated usernames/integers only (`incoming.rs:212-215`, `incoming.rs:268`, `incoming.rs:284`); `UID MOVE` quotes and escapes `\` and `"` (`incoming.rs:148-155`); subject control chars replaced (`submission.rs:64`) |
| Credential exposure from local storage (SEC-007) | Rejected | Passwords live only in the OS keyring under a dedicated service (`store.rs:505`); `mail.json` holds no secrets and tests assert `secret_id`/codes never reach snapshots (`tests.rs:264-278`, `tests.rs:281-310`); keyring references validated as UUIDs before use (`store.rs:497-504`) |
| Keyring service collision with provider credentials | Rejected | Mail uses `Entry::new("gosling-dedicated-email", id)`; config secrets use `Entry::new(service, KEYRING_USERNAME)` (`crates/gosling/src/config/base.rs:1498-1502`) — distinct service and account namespaces |
| STARTTLS downgrade / plaintext-credential (SEC-009) | Rejected | Credentials sent only after TLS; STARTTLS refusal aborts before `USER`/`PASS` (`incoming.rs:196-215`) with a dedicated test (`incoming.rs:369-397`); no `danger_accept_invalid_certs`; `ServerName::try_from` pinning (`incoming.rs:75-79`) |
| IDOR / object-scope bypass (SEC-003) | Rejected | Activity IDs are worker-generated UUIDs; caller-supplied IDs are looked up in the worker's own ledger with a revision token (`store.rs:155-168`, `store.rs:162-168`) |
| Unsafe path/file access (SEC-008) | Rejected | `0o700` data dir creation (`store.rs:397-401`), atomic tempfile write with sync (`store.rs:434-446`), per-worker exclusive file lock (`store.rs:405-412`) |
| Worker request smuggling / reuse (SEC-010) | Rejected | One 64 KB request per process; supervised mode exits on any second input (`mod.rs:362-367`) |
| Malicious local invocation of the hidden subcommand | Rejected (out of threat model) | Same-user local execution already owns the user keyring and data dir; the supervised pipe adds no new authority |
| Renderer XSS via mail content | Rejected | No `dangerouslySetInnerHTML`/HTML sinks in `EmailSettingsSection.tsx` or email types; React text escaping holds |
| Unsafe external tool invocation (SEC-012) | Rejected | `spawn(binary, ['desktop-email', '--supervised'], ...)` — fixed argv array, no shell (`emailIpc.ts:145-149`) |
| Python `mail-parser` advisories (CVE-2026-129299/300/301) | Not applicable | Those apply to the Python `spamscope/mail-parser` package; gosling uses the unrelated Rust crate `mail-parser` (stalw) 0.11.2. No Rust-crate advisory surfaced — recorded as an evidence gap, not proof of safety |
| lettre CVE-2020-28247 (sendmail transport injection) | Not applicable | Feature uses the SMTP transport with explicit `Tls` parameters; the sendmail transport is not compiled in this path. lettre Boring-TLS hostname CVE also not applicable (rustls backend in use) |

## Patch order

1. **SEC-MAIL-001** — `cargo update -p rustls --precise 0.23.46`, then the mail
   test suites and clippy (commands listed with the finding). Highest
   mechanism-risk, zero API change.
2. **SEC-MAIL-002** — explicit env allowlist in `emailIpc.ts:141`. Small,
   isolated; requires live "Test connection" validation because keyring and
   root-cert discovery are environment-sensitive.
3. **SEC-MAIL-003** — optional guide sentence; no code change.

## Validation limits

- Read-only static audit: no live IMAP/POP/SMTP sessions, no keyring
  interaction, no packaged Desktop launch, no worker subprocess inspection
  (SEC-MAIL-002 step 2 is the suggested dynamic confirmation).
- Advisory verification was done via GHSA and advisory aggregators through web
  search; `cargo audit` was not run (offline environment).
- macOS-focused review; keyring behavior on Windows/Linux and Windows ACL
  inheritance on `mail.json` were not runtime-verified.
- Reproduction commands marked "verified" were executed against this checkout
  on 2026-11-20; steps marked "not executed" are recorded as limits, not claims.
