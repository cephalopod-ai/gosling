# 2026-08-13 session artifact inventory

- Task: implement a durable, session-scoped Outputs inventory independent from preview tabs.
- Branch: `codex/session-artifact-inventory` from `main` at `0a1fecc45`.
- Core: added schema v26 artifact metadata, ordered discovery, idempotent upserts, message-only legacy
  backfill, fork metadata copy, and deletion cascade.
- ACP/SDK: added paginated artifact listing and durable artifact update notifications; regenerated the
  checked-in Gosling schema and TypeScript SDK.
- Desktop: added per-session artifact state, older-backend reconstruction, inventory/preview separation,
  session-scoped tabs, automatic `Outputs N`, and code-file classification.
- Electron replay: added an isolated Playwright fixture path and a real four-file replay. The harness
  uses disposable Gosling/Electron roots and bypasses single-instance/onboarding behavior only when
  `ENABLE_PLAYWRIGHT=true`; the installed app and user data remain untouched.
- Documentation: recorded ADR-0013, architecture boundaries, the Workspaces guide, and Desktop
  scenario cards DT-06/DT-07.
- Security: inventory insertion remains metadata-only and does not extend Electron read/open/reveal/copy
  authorization. No directory scanning, file creation, movement, or copying was added.
- Validation: targeted Rust artifact tests (7/7); `gosling-sdk-types` tests (13/13 including
  integration tests); Desktop typecheck/lint/i18n; full Desktop Vitest (692/692);
  Electron/Playwright four-file replay (1/1); full Rust Clippy with warnings denied; Docusaurus
  production build; focused Prettier; docs governance marker; and `git diff --check`.
