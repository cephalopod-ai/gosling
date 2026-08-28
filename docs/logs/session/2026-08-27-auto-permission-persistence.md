# Autonomous permission persistence

Date: 2026-08-27

## Task

Make Autonomous the default low-interruption mode, persist “Always Allow”
across sessions and threads, reserve prompts for explicit security warnings,
and recover the installed-app research session trapped in repetitive
WebSearch/WebFetch approvals.

## Root cause and policy decision

- External-tool providers were normalized from Auto to Approve in the ACP
  setup path, and the agent reply boundary independently rejected them in Auto.
- The Claude Code provider denied every provider-native permission request in
  Auto. ACP permission reuse was not durable at the provider/tool boundary.
- The operator explicitly superseded the earlier conservative external-provider
  policy: ordinary provider-native work should proceed autonomously; only
  explicit security-warning requests should require human review.

## Files and outcomes

- Auto is now the Rust and Desktop default, is advertised for external-tool
  providers, and is retained through session setup and provider changes.
- Claude Code accepts provider-native tool requests in Auto instead of failing
  or prompting for each search.
- Ordinary ACP requests auto-continue in Auto and Smart Approve. Chat denies,
  Approve defers to the operator, and requests carrying explicit security
  content continue to defer.
- ACP “Always Allow” is stored in the existing atomic `permission.yaml`, keyed
  by provider and tool. The shared manager registry makes the same durable
  decision visible across live threads; disk persistence makes it survive new
  sessions and restarts. Domain-specific and explicit-security approvals are
  not converted into reusable tool-wide grants.
- Session `20260828_51` was backed up, migrated from Approve to Auto, reopened
  in the patched Desktop app, and visibly reported `Autonomous` in the composer.
  A live retry exposed and led to removal of the second agent-boundary Auto
  rejection before final packaging. The final installed build reopened the
  same thread and began streaming its resumed turn in Autonomous mode without
  that rejection.
- The recovered external-provider run then exposed a completion-verifier gap:
  its finished report was safely contained and reported, but external-provider
  file operations appear as final-response artifact references rather than
  Gosling-hosted `created` operations. The verifier now accepts that provenance
  only while retaining configured-root containment, deliverable-type, a
  persisted assistant-message reference, matching-filename, size bound, and
  byte-identical Output/Research Library hash checks.

## Validation

- `cargo test -p gosling`: passed after the ACP fixture retained its explicit
  Smart Approve test default.
- Focused ACP provider/server persistence tests: passed.
- Agent unit tests: passed, 27/27.
- `cargo clippy --all-targets -- -D warnings`: passed.
- Desktop `pnpm run typecheck`: passed.
- `just package-ui`: passed for the final patched build; ad-hoc code-signature
  verification and installed sidecar/CLI hash equality passed.
- Installed Desktop smoke: session `20260828_51` displayed `Autonomous` and its
  recovery turn entered the streaming state without an ordinary-tool prompt or
  the former pre-stream Auto rejection.
- Research completion verifier tests: passed, 5/5, including an external
  provider reference pair and the existing mismatch/out-of-root failures.
- `git diff --check`: passed before documentation updates and is rerun at
  completion.

## Recovery and rollback evidence

- Session database backup:
  `/Users/eric/.local/share/gosling/session-backups/sessions-20260828_51-before-auto-20260827.db`
- Pre-artifact-ledger-repair database backup:
  `/Users/eric/.local/share/gosling/session-backups/sessions-20260828_51-before-artifact-ledger-repair-20260827.db`
- Previous application backup:
  `/Users/eric/.local/share/gosling/install-backups/Gosling-before-auto-permission-fix-20260827-2148.app`
- Previous CLI backup:
  `/Users/eric/.local/share/gosling/install-backups/gosling-cli-before-auto-permission-fix-20260827-2148`

## Risks and follow-ups

- Auto intentionally delegates ordinary provider-native tool authorization to
  the external provider. Gosling can require review only when that provider
  emits an explicit security-warning permission request.
- A locally ad-hoc-signed replacement may trigger one macOS Keychain
  authorization. That system security prompt remains a deliberate human gate.
