# 2026-09-16 — Branch input inheritance

## Task and preflight

Repair the reported loss of Inputs when branching a chat, retaining pointers to linked files,
and label new branches with `branch:` as requested. Target: `/Users/eric/Work/vscode/forked/gosling`,
clean `main` at `a492bbe1be5891e6a0d93e7a4d0e8db9eff0d494`. Catalog workflow:
`repair-defect-lowpriority`, supplied-defect mode, inferred standard involvement (L2),
`low_risk_repair` authority. No external publication or destructive operation is in scope.
Dory's deterministic current-state read confirms the target and no prior session checkpoint.

## Diagnosis and gate plan

One defect selected: same-store copy/fork transfers messages and Outputs metadata but omits
`session_library_items`. The branch has no private Inputs even though its copied conversation
refers to them. Independently created sibling sessions must retain their existing isolation.
The additional requested label changes only the ACP branch title. That explicit user request
authorizes this narrow visible label change alongside the repair workflow.

Patch `session_transfer.rs` to clone private library rows inside its existing transaction,
assigning fresh opaque IDs and retaining linked paths, pasted payloads, metadata, and timestamps.
Project inputs remain shared through the existing project/workspace/directory key. Update
`fork_session.rs` to use `branch:` and protect that title from automatic renaming. Add focused
regression coverage to the existing ACP fork integration target. Outputs remain metadata references;
no input or output file needs copying. Existing branches are outside the forward creation repair.

ADR-0015 and the live library schema/storage govern input scopes and resolution; ADR-0013 and
`docs/architecture.md` govern Outputs metadata and file authority. Baseline: linked-file pointers,
opaque IDs, scope isolation, and atomic session transfers conform; explicit branch inheritance is
the supplied missing behavior. The requested inheritance will be documented in ADR-0015.
No schema, capability, provider, or Desktop API changes are planned.

## Validation and final checkpoint

Orientation, diagnosis, and patch plan complete. Baseline `cargo fmt --all -- --check` passed.
Independent reads are batched; edits and database transfer operations are sequential because they
depend on the same transaction. Planned validation: formatting, Rust typechecking, a source-derived
SQLite behavior check, diff review, and contract comparison. Repository instructions reserve
`cargo build`, `cargo test`, and `cargo clippy` for user-requested build/test work; none was requested.

Implementation complete in `session_transfer.rs` and `fork_session.rs`. The existing ACP fork
integration target now includes mixed linked/text/image inheritance, original-file updates through
the real library-resolve request, project sharing, unrelated-session isolation, independent removal,
source deletion, nested branching, reopen persistence, label retention against automatic naming,
empty branch-label handling, and source-input preservation after failed activation. ADR-0015
documents the explicit inheritance; `.gitignore` admits this log, and `docs/INDEX.md` links it.

Validation commands ran from the repository root after Hermit activation:

- `cargo fmt` and `cargo fmt --all -- --check`: passed, including the final Rust patch.
- `DEVELOPER_DIR=/Library/Developer/CommandLineTools cargo check -p gosling --test acp_fork_session_test`:
  passed. This compiles/typechecks the changed core and the integration target; it does not execute
  the Rust tests. The SDK override follows existing local validation evidence and changes no host settings.
- `python3 - <<'PY'` source-derived SQLite check: four groups passed. It extracted the library
  table declaration from `schema.rs` and the exact SELECT/INSERT statements from `copy_session`,
  executed them in an in-memory database, and asserted complete path/payload/metadata preservation,
  fresh IDs, project deduplication, unrelated scope isolation, independent removal/source deletion,
  nested inheritance, 64-item and empty scopes, and rollback of both session creation and input rows
  under an injected insert failure. Static checks also confirmed transaction placement and Rust binds.
- `git diff --check`, documentation link/ignore/governance-marker checks, and final scoped diff
  inspection: passed.

Self-review found no additional in-scope defects. The intermediary and final regression walkthrough
checked the existing library add/list/get/remove and session-delete scope keys, copied workspace and
project metadata, fork cutoff ordering, title-update failure cleanup, and deferred linked-file
resolution. No filesystem reads/copies are introduced by input transfer. The existing stale-plan
history clone remains inside the same transaction, consistent with active ARC-012 in
`.architecture/invariants.yaml` and `core.session_planning` in `.architecture/components.yaml`.
No public API, persisted schema, security capability, or dependency changes were introduced.

Drift result: the explicitly requested branch inheritance is documented consistently in ADR-0015;
the existing linked-file and scope contracts, Outputs metadata-only contract, and plan-authority
transfer contract are preserved. No new unexplained drift. No prior mutable finding record or
in-code defect marker was found for the reported issue; this log records its disposition, and the
repo-native `docs/polish/test-ledger.md` records the validation scope.

Gates 0–7 complete with partial verification. One defect repaired, plus the requested branch title
label; no intermediary audit patches, unrelated repairs, deferred items, or routed findings. The
ten-defect cap has nine remaining slots, unused. Final status: `completed_with_partial_verification`.
Rust runtime tests, packaged Desktop acceptance, and existing-branch backfill were not performed.
Linked references continue to depend on files remaining at their original locations. Follow-up:
when build/test work is requested, execute the focused ACP fork target and exercise a new branch in
Desktop. No merge, commit, publication, or installation was performed.
