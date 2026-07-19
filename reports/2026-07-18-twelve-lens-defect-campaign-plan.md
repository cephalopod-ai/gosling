# Twelve-lens defect-repair campaign plan — 2026-07-18

Skill: private catalog `repair-defect-campaign`  
Mode: Existing Findings  
Finding source: `reports/2026-07-18-twelve-lens-agent-skills-findings.json`  
Campaign scope: all 10 validated findings in `cephalopod-ai/gosling`  
Baseline: `main` at `05ae05c7b10ead7c66e5f8c52fb217a1d10ba2e7`, equal to `origin/main`

## Gate 0 — repository and git posture

- The worktree started with only the two audit outputs untracked. They are the source and evidence
  for this campaign; no unrelated user changes are present.
- The repository has an `origin`, but the user did not authorize commit, push, merge, or remote
  mutation in this request. The campaign therefore records logical stage checkpoints in the
  session log and leaves a reviewable working-tree patch.
- Root `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, and `SECURITY.md` were read. No nested
  instruction file applies to the planned source/report paths.
- Rust changes require `cargo fmt`. The invoked repair skill explicitly requires regression,
  typecheck, lint, and broader validation, satisfying the repository's test-authorization gate.
- Repository documentation convention: dated reports and campaign logs under `reports/`, durable
  backlog status in `docs/TODO.md`, product behavior in `documentation/docs/` only when needed.
- The repair skill's file-size rule governs modularization. Files at or above 2000 lines are patched
  minimally and routed to a dedicated modularization follow-up; they are not split in this campaign.

Baseline checks:

| Command | Result |
|---|---|
| `cd ui/desktop && pnpm run typecheck` | Passed |
| `cd ui/desktop && pnpm test -- --run` | Passed: 70 files, 502 tests |
| `cargo test -p gosling workspace --lib` | Passed: 26 tests |
| `cargo test -p gosling import_formats --lib` | Passed: 8 tests |
| `cargo test -p gosling-server configuration --bin goslingd` | Passed: 1 test |

## Gate 1 — frozen inventory

| ID | Domain / priority / complexity | Disposition | Required touch set and proof |
|---|---|---|---|
| AUD-GOS-001 | security / P0 / high | in-scope | Desktop settings/secret-profile UI/chat construction plus secure migration or removal; sentinel absence across settings, renderer, messages, tracing, hooks, prompts |
| AUD-GOS-002 | security / P0 / medium | in-scope | Electron root issuance, preload, chooser/workspace grants, recent dirs; ungranted path denial and grant lifecycle tests |
| AUD-GOS-003 | data integrity / P0 / high | in-scope | Workspace SDK model, session schema/snapshot, ACP create/resume, permission/MCP roots; read allowed/write denied/output allowed tests |
| AUD-GOS-004 | security / P0 / high | in-scope | summon agent metadata/delegate schema/task config/resolved authority; fake-agent capability matrix |
| AUD-GOS-005 | correctness / P1 / low | in-scope | three foreign JSONL converters and import status; valid-corrupt-valid regression |
| AUD-GOS-006 | security/data integrity / P1 / high | in-scope | foreign import working-directory and provenance transition; confirmation/override and resume behavior |
| AUD-GOS-007 | architecture/data integrity / P1 / medium | in-scope | Desktop `set-setting` runtime validation and table-driven invalid-input tests |
| AUD-GOS-008 | reliability / P1 / medium | in-scope | atomic Desktop JSON persistence, previous-good/quarantine recovery, owner-only permission tests |
| AUD-GOS-009 | reliability / P2 / low | in-scope | shared import byte cap at CLI/Desktop/remote seams; boundary tests |
| AUD-GOS-010 | reliability / P2 / low | in-scope | typed server bind-address parsing and invalid configuration tests |

No finding is excluded or intentionally deferred. AUD-GOS-004 was previously routed as ORCH-002,
but the user's instruction to repair all findings explicitly reopens it. If compatibility cannot be
preserved without a broad architecture rewrite, the campaign must stop at that stage rather than
claim a partial fix.

## Gate 2 — locality grouping and ordered campaign

### Stage 1 — Desktop privileged data and filesystem boundary

- Defects: AUD-GOS-001 (P0/high), AUD-GOS-002 (P0/medium), AUD-GOS-007 (P1/medium),
  AUD-GOS-008 (P1/medium).
- Files/functions: `main.ts` settings/root/file IPC; `preload.ts`; `utils/settings.ts`,
  `recentDirs.ts`, and renderer file access; Local Secret Profiles and `ChatInput`; related tests.
- Data paths: renderer IPC → privileged main state/filesystem; settings/recent JSON persistence;
  saved credential values → chat/provider path.
- Modularization: `main.ts` (3295) and `ChatInput.tsx` (2036) are >=2000 and routed; apply minimal
  changes only. All other planned files are <=1000 and patched in place.
- Regression surface: sentinel secret absence; runtime setting schema; atomic/fault recovery;
  chooser-issued per-window grants; direct renderer self-grant denial.
- Why grouped: all four defects share Desktop settings and main-process authority; separating them
  would repeatedly reopen the same serialization and IPC contracts.
- Logical checkpoint: stage log entry; no git commit without user authorization.

### Stage 2 — Workspace folder-policy propagation

- Defect: AUD-GOS-003 (P0/high).
- Files/functions: shared Workspace/session types, session schema/migrations, `prepare_session`, ACP
  new/load/fork activation, working-directory inspection, MCP roots, Desktop session DTO/UI.
- Data paths: Workspace folder access → pinned session snapshot → local tool/MCP authorization.
- Modularization: `session_manager.rs`, `acp/server.rs`, and `agent.rs` are >=2000 and routed;
  smallest safe changes only. Other touched files are <=1000.
- Regression surface: create/resume pinning; read-only mutation denial; output/read-write access;
  canonical/symlink handling; deleted workspace and switching behavior.
- Logical checkpoint: stage log entry.

### Stage 3 — Least-authority delegation

- Defect: AUD-GOS-004 (P0/high; explicitly reopened known item).
- Files/functions: agent-file metadata parsing, delegate schema/spec, child extension resolution,
  resolved-authority display/tests.
- Data path: repo/global role definition + parent authority + explicit request → child TaskConfig.
- Modularization: `summon.rs` (2482) is >=2000 and routed; smallest coherent contract repair only.
- Regression surface: legacy/advisory role defaults, explicit allowlists, parent intersection,
  unknown extension rejection, ad-hoc compatibility, Auto fail-closed inspectors.
- Logical checkpoint: stage log entry.

### Stage 4 — Session import completeness, trust, and resource bounds

- Defects: AUD-GOS-005 (P1/low), AUD-GOS-006 (P1/high), AUD-GOS-009 (P2/low).
- Files/functions: foreign converters, common import-format module, session import request/storage,
  CLI handler, Desktop import picker/flow, Nostr share fetch/decrypt, tests.
- Data paths: local/remote JSON(L) → conversion/provenance/effective directory → canonical session.
- Modularization: `session_manager.rs` and `main.ts` are >=2000 and routed; converter files and
  common import module are <=1000.
- Regression surface: malformed middle record, explicit imported-directory handling, durable
  provenance/resume, byte limits at every entrypoint, native import compatibility.
- Why grouped: all three defects share the same import payload and status transition.
- Logical checkpoint: stage log entry.

### Stage 5 — Server configuration startup boundary

- Defect: AUD-GOS-010 (P2/low).
- Files/functions: `configuration::Settings::socket_addr` and its two callers/tests.
- Data path: `GOSLING_HOST`/port → validated bind address → startup error.
- Modularization: none; file is <=1000 and change is localized.
- Regression surface: invalid, hostname-policy, IPv4, and IPv6 configuration.
- Logical checkpoint: stage log entry.

## Cross-stage risks and ordering

- Stage 1 removes the unsafe generic credential-prompt path before later policy work can expose it
  through additional session roots or delegation.
- Stage 2 establishes a pinned folder capability representation that Stage 3 child sessions must
  inherit without widening.
- Stage 4 may add session/import metadata. Re-read the Stage 2 migration and DTO touch set before
  editing to avoid parallel schema assumptions.
- `main.ts`, `ChatInput.tsx`, `session_manager.rs`, `acp/server.rs`, `agent.rs`, and `summon.rs`
  remain routed for dedicated behavior-preserving modularization after the campaign because each is
  >=2000 lines.

## Campaign completion gate

The campaign is complete only after all 10 findings have a regression test, stage adversarial and
diff review, targeted validation, final formatting/typecheck/tests/lint, documentation refresh,
and an honest final status in the session log and audit report.
