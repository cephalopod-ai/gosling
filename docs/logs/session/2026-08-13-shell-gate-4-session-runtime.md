# 2026-08-13 Gosling shell Gate 4 session-runtime checkpoint

- Task: continue the accepted shared shell productization plan with the main-owned session
  create/resume seam after compatibility; no renderer authority, domain shell, release activation,
  or production persistence policy was added.
- Implementation: `connectShellAcp` now checks canonical load-session and required custom-method
  capabilities, binds new sessions to the absolute main-owned working directory, and resumes by
  reading the server-owned session ID/directory before compacted load. The runtime controller passes
  its fixed working directory; shell IPC/preload remains the same frozen eight-channel surface.
- Profile contract: `_gosling/unstable/session/info` joined the exact shared-host required-method set;
  source profiles must include every set member, preventing a profile from omitting a method the
  adapter assumes after compatibility.
- Live proof: a neutral isolated harness ran runtime controller → minimal host → real child →
  authenticated ACP → compatibility → create → cleanup → child restart → resume, then read one row
  from the namespaced SQLite store and an empty process registry. A mismatched core path produced
  `CORE_MISMATCH`, no ACP connection, zero durable sessions, and an empty registry.
- Credentials/provider boundary: the harness wrote only test provider/model names (`openai`,
  `gpt-4o`) so ACP could initialize session metadata. It supplied no provider credential and made no
  prompt/model request.
- Validation: profile/package Node suite 41/41; session Vitest 24/24; live integration 2/2;
  full Desktop suite 660/660 across 100 files; full lint/typecheck/i18n; focused
  ESLint/Prettier; `git diff --check`.
- Remaining Gate 4 blocker: full Desktop `gosling://handoff` receiving through a focused
  parser/router module. Packaged session/restart and coexistence remain Gate 6.
- Test ledger: no `docs/testing/test-ledger.yaml` or equivalent required ledger exists.
- Authority honored: local implementation/validation/commit only; no push, merge, signing,
  notarization, publication, upload, updater promotion, production identity/destination, or domain
  behavior.
