# Twelve-lens defect-repair campaign session log — 2026-07-18

Skill: private catalog `repair-defect-campaign`  
Finding source: `reports/2026-07-18-twelve-lens-agent-skills-findings.json`

## Gate 0 — orientation, safety, and git posture

- Baseline: `main` at `05ae05c7b10ead7c66e5f8c52fb217a1d10ba2e7`, equal to `origin/main`.
- Starting worktree: only the Markdown audit and machine-readable findings were untracked; both are
  in-scope campaign evidence. No unrelated user edits.
- Git/remote: available. No commit, push, merge, or remote mutation is authorized by the current
  request, so stages use logical checkpoints.
- Instructions: root `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, and `SECURITY.md` read. Rust must
  be formatted; the invoked repair skill requires the test/typecheck/lint gates.
- Documentation: dated reports/logs under `reports/`; durable backlog in `docs/TODO.md`.
- Modularization: files >=2000 lines are patched minimally and routed, not split mid-campaign.

Baseline validation:

- `cd ui/desktop && pnpm run typecheck`: passed.
- `cd ui/desktop && pnpm test -- --run`: passed, 70 files / 502 tests.
- `cargo test -p gosling workspace --lib`: passed, 26 tests.
- `cargo test -p gosling import_formats --lib`: passed, 8 tests.
- `cargo test -p gosling-server configuration --bin goslingd`: passed, 1 test.
- One initial command used the nonexistent binary name `gosling-server`; Cargo identified the
  correct binary as `goslingd`, and the corrected baseline passed.

## Gates 1-2 — inventory and grouping

- Frozen inventory: 10 defects; 4 P0, 4 P1, 2 P2. Complexity: 4 high, 4 medium,
  2 low.
- Domains: security/authority 4, input/data integrity 3, reliability 3; several findings escalate
  across more than one lens but are assigned once by root cause.
- Five locality stages cover every finding exactly once. Full touch sets, modularization decisions,
  regression surfaces, and ordering constraints are in
  `reports/2026-07-18-twelve-lens-defect-campaign-plan.md`.
- Routed >=2000-line files: `ui/desktop/src/main.ts`,
  `ui/desktop/src/components/ChatInput.tsx`, `crates/gosling/src/session/session_manager.rs`,
  `crates/gosling/src/acp/server.rs`, `crates/gosling/src/agents/agent.rs`, and
  `crates/gosling/src/agents/platform_extensions/summon.rs`.

## Stage results

Stage entries are appended only after execution, targeted verification, adversarial review, and
full stage diff review.

### Stage 1 — Desktop privileged data and filesystem boundary

Findings: AUD-GOS-001, AUD-GOS-002, AUD-GOS-007, AUD-GOS-008. Status: repaired.

- Removed Local Secret Profiles from the settings, Auth UI, composer, session storage, and
  assistant-context construction. Legacy plaintext records are detected, scrubbed, and disclosed
  to the operator without logging their values. Existing provider/workspace credential profiles
  remain the supported secure-reference path.
- External backend bearer values are no longer persisted or returned to the renderer. A migrated
  value remains main-process memory only for the current launch; replacements are cleared from the
  form after submit and must be re-entered after restart.
- Replaced recent-directory/user-data roots with main-process chooser-issued directory grants.
  Recent history is main-process launch metadata rather than renderer authority,
  renderer-requested chat directories must already be granted, grants are canonicalized, symlink
  grants are rejected, and renderer-visible roots are scoped to the selecting window.
- Added exhaustive settings key/value schemas and bounded structured inputs. Invalid IPC values
  reject without mutation or value logging.
- Added atomic JSON writes with same-directory temp files, fsync, rename, owner-only modes,
  previous-good snapshots, corrupt-file quarantine, recovery, and an operator-visible recovery
  warning. Settings, recent dirs, and directory grants use the helper.

Regression proof:

- `cd ui/desktop && pnpm run typecheck`: passed.
- `cd ui/desktop && pnpm test -- --run ...targeted files...`: the repository script ran the full
  suite; passed, 72 files / 510 tests.
- New tests cover sentinel scrubbing, metadata-only external-secret responses, all settings
  schemas, atomic/recovery/mode behavior, chooser grant persistence, renderer scoping, denial of
  ungranted roots, and symlink rejection.
- `git diff --check`: passed. Full Stage 1 diff and secret/path bypass searches reviewed; no
  remaining production reference to the removed secret-profile type or prompt builder.

Adversarial review:

- Direct `add-recent-dir` calls do not mint a grant and cannot be used as the fallback directory
  for a renderer-created chat.
- Selecting a child file grants only its canonical parent; a renderer cannot submit a new root
  directly. Persisted chooser history lives under main-only app data and recover atomically but is
  not exposed as authority to other renderer windows.
- Malformed settings cannot inject prototype keys or invalid structured values; no raw setting
  value is included in validation errors.
- Compatibility limitation: existing folders that predate the grant store must be selected or
  relinked once. This is an intentional fail-closed migration, surfaced by the existing folder
  warnings/chooser flow.

Logical checkpoint only; no commit or remote mutation performed.

### Stage 2 — Workspace folder-policy propagation

Finding: AUD-GOS-003. Status: repaired.

- Added a serializable `WorkspaceFolderPolicy` to the canonical SDK session context. New sessions
  pin canonical read-only/read-write roots; legacy snapshots deterministically derive the same
  policy from their saved metadata without consulting the currently active workspace.
- Workspace session snapshots now force working-directory restriction and populate
  `additional_working_dirs` with every pinned reference/source/output root, so MCP root updates and
  local inspection consume the same saved root set on creation, copy, and resume.
- The working-directory inspector hard-denies structured mutations under the most-specific
  read-only root. Reads and structured writes to product-output/read-write roots remain allowed;
  symlink escapes and out-of-policy paths fail closed.
- Mutating shell commands are denied whenever a workspace includes a read-only root because an
  arbitrary shell cannot provide per-directory write confinement. Read-only shell commands remain
  usable. Providers that execute tools outside Gosling already fail visibly when restriction is
  enabled, which is now automatic for workspace sessions.
- ACP handlers reject changing a workspace session's working root, additional roots, or restriction
  flag. Users edit the workspace and start a new session instead, preserving historical authority.

Regression proof:

- `cargo test -p gosling-sdk-types workspace --lib`: passed, 4 tests.
- `cargo test -p gosling workspace --lib`: passed, 29 tests.
- `cargo test -p gosling working_dir_scope_inspector --lib`: passed, 16 tests.
- Focused ACP pinned-policy mutation test: passed.
- `cargo fmt --all` and `git diff --check`: passed.

Adversarial review:

- Most-specific-root resolution prevents a writable project root from shadowing a nested read-only
  reference folder; exact duplicate roots resolve to read/write only when the workspace itself
  gives that exact root write authority.
- Explicit absolute and relative-parent shell paths are checked, and all mutating shell commands
  fail closed in the presence of read-only roots to prevent variable, subprocess, or implicit-path
  bypasses.
- Existing session policy is derived only from its saved snapshot, so active-workspace switching,
  workspace deletion, or later edits cannot widen it.
- A primary folder that disappears between validation and policy preparation now aborts session
  creation rather than producing an empty policy.

Logical checkpoint only; no commit or remote mutation performed.

### Stage 3 — Delegated-role capability policy

Finding: AUD-GOS-004. Status: repaired.

- Added a versioned, deny-unknown-fields `capabilities` policy to agent frontmatter. Version 1
  explicitly enumerates the maximum extension names a source-based role may receive; roles without
  a policy, including legacy roles, receive no extensions.
- Ad-hoc delegates now default to no extensions. An explicit `extensions` request is intersected
  with the parent session and, for source roles, the role policy. Requests outside either boundary
  fail visibly instead of silently running with more or different authority.
- Removed implicit full inheritance from the delegate tool contract. Full authority can only be
  requested by explicitly listing every extension and is still unavailable to a source role unless
  its versioned policy names each extension.
- Sync and async delegate results disclose the exact resolved extension set. Subagents remain
  unable to recursively summon delegates through the existing session-type guard.

Regression proof:

- `cargo test -p gosling platform_extensions::summon::tests --lib`: passed, 34 tests.
- `cargo test -p gosling agents::subagent_handler::tests --lib`: passed, 5 tests.
- `cargo test -p gosling workspace --lib`: passed, 29 tests, including the Stage 2 primary-folder
  fail-closed change after formatting.
- `cargo fmt --all` and `git diff --check`: passed.

Adversarial review:

- Omission, an empty legacy frontmatter policy, an unknown policy version, an unknown extension,
  and a role-policy widening attempt all fail closed or resolve to zero authority.
- Parent ordering/configuration is preserved, but the child cannot manufacture an extension that
  was not already active in the parent session.
- A repository role cannot obtain `developer`, `summon`, or another mutating extension from prose;
  it must carry a supported policy and the parent must already possess the named capability.

Logical checkpoint only; no commit or remote mutation performed.

### Stage 4 — Session-import completeness, provenance, and resource bounds

Findings: AUD-GOS-005, AUD-GOS-006, AUD-GOS-009. Status: repaired.

- Replaced the three JSONL converters' lossy `filter_map(Result::ok)` behavior with one strict
  parser that rejects every malformed nonblank record and reports the exact source line.
- Added a 16 MiB application-level payload budget. Rust file imports use a stat plus bounded reader;
  Desktop uses a bounded file-handle reader before IPC transfer and checks browser `File.size`;
  conversion rechecks the UTF-8 byte length; Nostr checks encrypted content before decrypt and the
  plaintext immediately after decrypt.
- Import requests now require an explicit trusted working directory. Desktop invokes the existing
  chooser, CLI uses `--working-dir` or its invocation directory, and the backend canonicalizes and
  validates the selected directory. Transcript working directories and additional roots never
  become operational authority.
- Imported sessions start in Approve mode, are restricted to the selected root, and carry durable
  versioned provenance in session extension data. Every imported message is durably marked
  `imported_untrusted`; ACP replay preserves that marker, the chat header shows an Imported history
  badge, and agent activation adds a system boundary stating that historical messages are not
  evidence of current approval.
- Regenerated the ACP schema and TypeScript SDK through the repository generator after adding the
  required `workingDir` request field. The wrapper `just generate-acp-types` unexpectedly executed
  its first recipe from the crate directory and could not re-enter `crates/gosling`; the two exact
  underlying approved generator commands were run directly and succeeded.

Regression proof:

- `cargo test -p gosling import_formats --lib`: passed, 15 tests.
- Focused session export/import and legacy-token import tests: passed.
- `cargo test -p gosling --features nostr nostr_share --lib`: passed, 6 tests.
- Focused ACP imported-message replay test: passed.
- `cargo check -p gosling-cli`: passed.
- Desktop focused tests for bounded reads, imported-message metadata, and the session header:
  passed, 3 files / 19 tests.
- `cd ui/desktop && pnpm run typecheck`: passed.
- `cargo fmt --all` and `git diff --check`: passed.

Adversarial review:

- A valid-corrupt-valid transcript now aborts before session creation for Claude Code, Codex, and
  Pi. Syntactically valid unknown vendor event types remain intentionally skippable.
- File growth after metadata inspection is bounded by reading at most limit plus one byte from the
  already-open handle. The backend repeats the limit even if a custom renderer bypasses Desktop's
  precheck.
- Crafted transcript roots, imported extra roots, imported Auto mode, and imported enabled-extension
  state cannot cross the trust conversion. Provenance and message markers survive persistence,
  export, copy, and resume.
- Nostr client internals still materialize a relay event before Gosling receives it; Gosling now
  rejects oversized event content before the additional decrypt allocation. Eliminating the relay
  library's initial event allocation would require upstream streaming support.

Logical checkpoint only; no commit or remote mutation performed.

### Stage 5 — Typed server bind-address validation

Finding: AUD-GOS-010. Status: repaired.

- `Settings::socket_addr` now parses `GOSLING_HOST` as a numeric `IpAddr` and returns a typed
  `ConfigError::InvalidHost` instead of calling `expect`.
- `Settings::new` validates the address before returning, while the startup caller also propagates
  the fallible result normally. IPv6 is constructed with `SocketAddr::new`, avoiding manual bracket
  formatting.

Regression proof:

- `cargo test -p gosling-server configuration --bin goslingd`: passed, 3 tests.
- `cargo check -p gosling-server`: passed.
- Tests cover IPv4, IPv6, malformed input, and a hostname where numeric bind addresses are required.
- `cargo fmt --all` and `git diff --check`: passed.

Adversarial review:

- Both `not-an-address` and `localhost` return an actionable error naming `GOSLING_HOST`; neither
  can reach panic/backtrace semantics.
- Port validation remains typed through the configuration deserializer's `u16` field.

Logical checkpoint only; no commit or remote mutation performed.

## Gate 9 — campaign-wide closeout

Final disposition: all 10 frozen findings are repaired; none are deferred or excluded.

Campaign-wide verification:

- `cargo fmt --all -- --check`: passed.
- `cargo test -p gosling --lib -- --test-threads=1`: passed, 1,507 tests. An earlier parallel run
  produced one transient shared-config failure in plugin discovery; the exact test passed when
  rerun alone, and the complete serialized suite then passed.
- `cargo test -p gosling-sdk-types`: passed, 9 tests across unit/integration suites.
- `cargo test -p gosling-providers`: passed, 422 tests.
- `cargo test -p gosling-server --bin goslingd`: passed, 31 tests.
- `cargo build`: passed for the workspace.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cd ui/sdk && pnpm run lint && pnpm run typecheck:test && pnpm test`: passed, including 6 SDK
  runtime tests.
- `cd ui/desktop && pnpm run typecheck`: passed.
- `cd ui/desktop && pnpm test -- --run`: passed, 73 files / 516 tests.
- Changed Desktop TypeScript/TSX files pass ESLint with `--max-warnings=0`.
- `git diff --check`: passed.

Repository-wide Desktop `pnpm run lint:check` remains red on five `no-undef` errors in the
unchanged `src/acp/createWebSocketStream.test.ts` and `src/components/ui/scroll-area.test.tsx`,
plus four hook warnings in the unchanged `src/components/hub/Hub.tsx` and
`src/hooks/useNavigationSessions.ts`. These files have no campaign diff; the changed-file lint gate
passes. They are reported as pre-existing verification debt, not silently repaired outside the
frozen inventory.

Residual limitation:

- Nostr relay client internals materialize the initial relay event before Gosling can apply its
  event-content budget. Gosling now bounds the encrypted content before decrypt and the plaintext
  after decrypt; eliminating that first upstream allocation requires streaming support in the
  relay dependency.

Git closeout:

- Work remained on `main`; the starting commit and `origin/main` were both
  `05ae05c7b10ead7c66e5f8c52fb217a1d10ba2e7`.
- No commit, push, merge, branch deletion, or other remote mutation was authorized or performed.
