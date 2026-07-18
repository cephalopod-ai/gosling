# Gate 4 data-integrity audit

Skill: `audit-dataflow-integrity` v3.1
Authority: read-only audit; repairs follow only after this report is closed
Scope: workspace JSON persistence, credential metadata/secure-storage coordination, workspace import/export, session schema v22, path normalization, and the Desktop workspace consumer boundary
Budget: 12 implementation/schema files and their focused tests; state-mutating paths first

The supplied prompt was treated as a draft. Its persistence and credential-security mission was preserved while the review expanded to crash recovery, cross-store partial commits, foreign-platform paths, semantic corruption, and producer/consumer parity.

## Surface inventory

| Entity                      | Source                                  | Owner                   | Writers                           | Readers                           | Scope key                       | Provenance                        | Integrity constraint                                        |
| --------------------------- | --------------------------------------- | ----------------------- | --------------------------------- | --------------------------------- | ------------------------------- | --------------------------------- | ----------------------------------------------------------- |
| Workspace store document    | Gosling data directory                  | `WorkspaceStore`        | initialize, mutate, recovery      | `WorkspaceService`                | one desktop data root           | schema version + timestamps       | one active/default workspace, atomic file replacement       |
| Workspace                   | editor/template/import                  | `WorkspaceService`      | create, update, duplicate, delete | Desktop, session preparation      | workspace UUID                  | created/updated/opened timestamps | unique stable identity, valid paths/output/default bindings |
| Credential-profile metadata | credential UI/global migration/template | `WorkspaceService`      | create, update, delete, bootstrap | renderer metadata, provider scope | profile UUID                    | source enum                       | no raw secret, required fields accurately reflected         |
| Credential values           | credential form/provisioner             | `Config` secure storage | secure set/update/delete          | scoped provider construction      | namespaced profile UUID + field | secure-store key                  | never persisted in workspace/session/renderer settings      |
| Session workspace snapshot  | new-session preparation                 | `SessionManager`        | one SQL update, copy              | resume/header/sidebar             | session UUID                    | workspace/profile name snapshots  | nullable for legacy, immutable on workspace switch          |
| Distribution template       | layered Gosling config                  | workspace bootstrap     | first-launch materialization      | workspace store                   | template UUID                   | distribution source enum          | non-secret only, allowlisted path placeholders              |
| Session workspace filter    | local UI preference                     | `WorkspaceContext`      | user filter/switch                | sidebar query                     | window preference               | none required                     | never authoritative for session ownership                   |

## Boundary map

- JSON store boundary: advisory file lock, validate-before-write, fsync temporary file, atomic rename, owner-only permissions.
- Credential boundary: renderer sends a short-lived secret once; backend maps it to `workspace-credential::<profile UUID>::<field>` and provider construction receives only a task-local indirection scope.
- Session boundary: new-session request carries workspace ID; backend resolves the authoritative cwd/context and writes all snapshot fields in one SQL update. Resume reads only the session snapshot/profile reference.
- Import/template boundary: secret-shaped keys and traversal are rejected; templates use only known placeholders.
- UI boundary: generated SDK DTOs are the producer/consumer contract; local storage contains only the harmless session-list filter/collapse preferences.

## Findings

### DAT-GOS-001: A stale temporary file can corrupt the next workspace-store write

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:

- `crates/gosling/src/workspace/store.rs:223-230` writes the serialized document into the shared temporary path and renames it over the canonical file.
- `crates/gosling/src/workspace/store.rs:263-271` opens that path with `truncate(false)`.

Observed behavior: an interrupted longer temporary document can retain trailing bytes when the next serialized document is shorter, after which the malformed temporary file replaces the valid canonical store.
Expected boundary: every atomic replacement must start with an empty, process-owned temporary file.
Failure mechanism: the lock prevents concurrent writers but does not remove stale bytes from the reused temp path.
Break-it angle: leave a valid oversized temp file beside a valid canonical store, then persist a shorter document.
Impact: Desktop can recover to a new Default workspace, but the latest valid canonical definitions are displaced into corruption recovery.
Operational impact: Workflow blast radius; file side effect; compensatable; UI-visible on next load; rerun unsafe until repaired.
Adjacent failure modes: crash recovery and multi-window writes.
Recommended mitigation: use truncate-on-open for the temporary document and add a stale-longer-temp regression test.
Implementation assessment: local guardrail, XS, Codex; one file plus a behavior test.
Validation: persist a shorter document over a longer stale temp and assert exact parseable bytes.
Non-goals: replacing the existing file-lock strategy.

### DAT-GOS-002: Credential metadata and secure values can partially commit

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:

- `crates/gosling/src/workspace/credentials.rs:161-195` updates secure values before writing profile metadata, with no recovery record or compensation for the second write.
- `crates/gosling/src/workspace/credentials.rs:215-231` deletes secure values before deleting metadata, again without a durable recovery record.

Observed behavior: a metadata write failure after secure mutation can leave metadata claiming removed values still exist, or can leave updated values orphaned from their metadata. A delete failure can remove secrets while retaining a referenced profile.
Expected boundary: cross-store operations must fail closed and remain recoverable across errors and process interruption without persisting raw values outside secure storage.
Failure mechanism: two independently atomic stores are treated as one transaction without a non-secret intent/recovery protocol.
Break-it angle: fail the workspace rename after secure update/delete, or terminate between the two writes.
Impact: new and resumed sessions can lose their pinned credential unexpectedly or retain inaccessible secure residue.
Operational impact: Workflow blast radius; file/keyring side effects; compensatable; UI-visible only when the profile/session is used; rerun safety unknown.
Adjacent failure modes: stale credential status and orphan cleanup.
Recommended mitigation: persist metadata/cleanup intent first, derive configured status from secure storage, keep a non-secret pending-deletion journal, and drain it during startup.
Implementation assessment: persistence recovery, M, Codex; workspace store, credential service, startup, and failure tests.
Validation: inject failure between metadata and secure writes; assert fail-closed status and restart cleanup.
Non-goals: storing secret values in a journal or replacing Gosling's secure-storage abstraction.

### DAT-GOS-003: Incomplete credential profiles are promoted as configured

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:

- `crates/gosling/src/workspace/credentials.rs:89-103` sets every newly created profile to `Configured` even when required provider fields were omitted.
- `crates/gosling/src/workspace/validation.rs:102-114` checks only that a referenced profile ID exists, not whether its status is missing or needs authentication.

Observed behavior: the renderer can show “configured,” and the workspace row can be warning-free, while task-local provider construction later fails closed for a required field.
Expected boundary: configured status must be derived from declared required fields and actual secure-store presence; workspace validation must surface unavailable profiles before session creation.
Failure mechanism: metadata presence is promoted to credential readiness.
Break-it angle: create an Anthropic profile with no API key, bind it, then inspect list validation/start a session.
Impact: misleading readiness and late, avoidable new-session failure.
Operational impact: Workflow blast radius; user-visible side effect; reversible; UI-visible late; rerun safe.
Adjacent failure modes: distribution provisioning and deleted-profile resume.
Recommended mitigation: derive configured fields/status on read and resolution, issue a credential warning during workspace validation, and refuse preparation with an actionable error.
Implementation assessment: workflow protocol, S, Codex; credential and validation modules plus tests.
Validation: missing required secure value reports `missing`, produces a workspace warning, and cannot resolve silently.
Non-goals: provider network validation for providers that do not expose it.

### DAT-GOS-004: Foreign-platform paths can be treated as session-ready

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:

- `crates/gosling/src/workspace/validation.rs:167-201` performs existence/directory checks only when `is_native_path` is true, but returns the non-native path without an issue.
- `crates/gosling/src/workspace/service.rs:223-241` converts that normalized output string directly to `PathBuf` and calls `create_dir_all`.

Observed behavior: a Windows path loaded on macOS/Linux (or the inverse) can validate as usable; explicit output creation can interpret it as a relative native filename.
Expected boundary: cross-platform path syntax may round-trip, but it must be unavailable for native I/O and session creation until relinked.
Failure mechanism: normalization support is conflated with native reachability.
Break-it angle: validate and create `C:\\Projects\\Output` on Unix.
Impact: session setup fails later or creates a wrongly named local directory.
Operational impact: Local blast radius; file side effect; reversible; UI-visible late; rerun unsafe for repeated creation.
Adjacent failure modes: cross-device workspace imports and distribution templates.
Recommended mitigation: emit required/optional platform-unavailable issues and guard all native folder creation.
Implementation assessment: local guardrail, S, Codex; validation/service plus platform-conditional tests.
Validation: non-native primary blocks sessions and non-native output creation is rejected without filesystem mutation.
Non-goals: translating drive letters or network shares between operating systems.

### DAT-GOS-005: Store validation permits duplicate canonical identities

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:

- `crates/gosling/src/workspace/store.rs:84-109` validates only schema upper-bound, non-empty workspaces, and existence of active/default IDs.

Observed behavior: duplicate workspace/profile IDs or names and unsupported older schema versions deserialize as authoritative state; subsequent reads and mutations select the first matching record.
Expected boundary: the source-of-truth store must reject semantic identity ambiguity and quarantine recoverable malformed state.
Failure mechanism: service writers enforce some uniqueness, but the persistence reader does not enforce the same invariants.
Break-it angle: duplicate an ID in the JSON store while keeping active/default references valid.
Impact: edits, session preparation, or profile resolution can target a different record than the operator sees.
Operational impact: Local blast radius; file side effect; compensatable; silent until mutation; rerun unsafe.
Adjacent failure modes: corrupted import/template materialization and future schema migration.
Recommended mitigation: validate schema equality and unique IDs/names at load, preserve malformed bytes in the existing corruption backup, and log only the backup path/class.
Implementation assessment: persistence recovery, S, Codex; store validator and corruption tests.
Validation: duplicate IDs/names are quarantined and Default is reinitialized while the original bytes remain recoverable.
Non-goals: silently merging ambiguous user-edited records.

## DAT inventory disposition

| Check                               | Disposition              | Evidence                                                                                                                                                              |
| ----------------------------------- | ------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| DAT-001 scope leakage               | Held                     | Mutations resolve workspace/profile IDs only inside the single local store; session ownership uses a separate immutable snapshot.                                     |
| DAT-002 duplicate entity            | Finding                  | DAT-GOS-005; normal create/duplicate paths do reject names, but the reader accepted ambiguous persisted identity.                                                     |
| DAT-003 orphaned record             | Finding                  | DAT-GOS-002 secure-store residue; workspace deletion intentionally preserves sessions and profile deletion intentionally leaves relinkable bindings.                  |
| DAT-004 lost provenance             | Held                     | credential `source` and workspace/session name snapshots survive DTO/store/SQL transformations.                                                                       |
| DAT-005 corrupt merge               | Held                     | no record merge path exists; template/store mutation is in-memory then one atomic file replacement.                                                                   |
| DAT-006 incorrect normalization     | Finding                  | DAT-GOS-004 separates syntax normalization from native reachability.                                                                                                  |
| DAT-007 partial persistence         | Findings                 | DAT-GOS-001 and DAT-GOS-002.                                                                                                                                          |
| DAT-008 migration meaning loss      | Held                     | v22 columns are nullable; the migration test verifies legacy rows stay unassigned rather than inheriting current active state.                                        |
| DAT-009 round-trip loss             | Held with semantic limit | Workspace export preserves all workspace fields; import intentionally creates a new ID/timestamps and is therefore duplicate/import semantics, not restore semantics. |
| DAT-010 stale derived data          | Finding                  | DAT-GOS-003 profile status was persisted/promoted rather than derived from secure truth.                                                                              |
| DAT-011 evidence misclassification  | Not applicable           | no evidence/control mapping surface.                                                                                                                                  |
| DAT-012 advisory misrepresentation  | Not applicable           | no advisory/canonical promotion surface.                                                                                                                              |
| DAT-013 silent constraint violation | Finding                  | DAT-GOS-005. Session workspace columns deliberately omit FKs so deleted workspaces/profiles do not erase history.                                                     |
| DAT-014 cross-batch contamination   | Held                     | workspace filter is query-only UI state and is not used when resuming a session.                                                                                      |
| DAT-015 weak data promoted          | Finding                  | DAT-GOS-003 metadata existence was promoted to configured authority.                                                                                                  |

## Break-it review

- Foreign workspace/profile IDs: rejected or become explicit relink-required state; no cross-owner scope exists in this single-user store.
- Repeated import: duplicate name is rejected; import is clone semantics, not restore semantics.
- Export/import: secrets are absent; editable workspace fields survive; identity/timestamps intentionally change.
- Migration edge: nullable legacy row verified directly in SQLite.
- Parent deletion: workspace/session no-cascade behavior is deliberate and tested against both DB row and physical file preservation.
- Halfway file write: canonical rename is atomic, but the stale-temp truncation defect remains DAT-GOS-001.
- Halfway secure/profile write: DAT-GOS-002.

## Skill escalation

| Finding     | Primary lens   | Secondary lens            | Why                                          |
| ----------- | -------------- | ------------------------- | -------------------------------------------- |
| DAT-GOS-001 | Data Integrity | Reliability / Concurrency | interrupted writes and reused temp state     |
| DAT-GOS-002 | Data Integrity | Security / Recovery       | secure-store lifecycle and restart cleanup   |
| DAT-GOS-003 | Data Integrity | Workflow/GUI              | configured status changes operator belief    |
| DAT-GOS-004 | Data Integrity | Input/Output Path         | platform path syntax reaches native file I/O |
| DAT-GOS-005 | Data Integrity | Reliability               | malformed source-of-truth startup recovery   |

## Patch order

1. Make temp writes truncate and add the stale-temp regression.
2. Strengthen store invariants/recovery so later credential journaling has a reliable base.
3. Make credential truth derived/fail-closed and add pending secure deletion recovery.
4. Reject non-native I/O and surface credential readiness in validation.
5. Re-run migration, store, session, secret-scope, SDK, and Desktop tests.

## Validation limits

- No destructive process-kill drill was run; interruption outcomes are source-evidenced, not runtime-observed.
- No real OS keyring secret was created during the audit. Secure-store behavior relies on the existing Config abstraction tests and static producer/consumer tracing.
- The Desktop application was not launched against a real keyring/provider in this gate; runtime provider authentication remains a later acceptance check.
- Shared SQLite fixtures do not alter foreign-key or process-global database settings in the focused workspace tests. The migration tests inspect actual row fields, not return codes.

Stop condition: all DAT-001 through DAT-015 inventory items are findings, held non-findings, or explicitly not applicable.
