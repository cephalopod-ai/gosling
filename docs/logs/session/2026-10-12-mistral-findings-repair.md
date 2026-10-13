# 2026-10-12 — Mistral findings repair

## Task and baseline

Repair the 45 findings and reconcile the four observations in
`mistral-audit-comprehensive-12OCT2026.md`, supplied through the operator's attachment.
Baseline: `main` at `29d31f7072f8a5d369c363444fdc4532b9bd95cb`.
Initial user work: modified `docs/INDEX.md` and untracked audit report; preserved.
Applied catalog workflow: `repair-defect-patchset` with its repair, readability,
architecture-drift, evidence-authority and record-closure contracts.

Authorization covers local source, tests, documentation and workflow repairs. No
publication, installation, live account changes, or credential rotation is authorized.
Rust build/test/clippy scope was requested asynchronously because AGENTS.md limits those
commands to an explicit request. Until answered, use formatting and static validation.
An independent read-only critic is required by the skill and is reviewing the findings.
Dory exposes read-only tools here; this repo-native log supplies the durable checkpoint.

## Stage plan and interaction analysis

Each stage verifies the cited source and active declarations before its edits. Findings
retain the original severity and IDs; pending rows are not claims of confirmed defects.
Source baselines use inspected branches and existing tests; regression evidence is recorded
below. Independent reads/checks can run concurrently; mutations to shared functions stay
sequential. No modularization or unrelated repair is included.

| Stage | IDs | Shared surface and regression boundary | Initial status |
| --- | --- | --- | --- |
| 1: tool authorization and credentials | SEC-GSL-001, SEC-GSL-002, LLM-GSL-001, LLM-GSL-004, ARC-GSL-006, OBS-GSL-004 | website_logins, inspectors, delegation/orchestrator, hooks; modes, denial precedence, destination checks | in progress |
| 2: renderer grants and edit recovery | SEC-GSL-201, WFG-GSL-003 | rendererIpc, directory registry, chatSessionController; picker mediation, path aliases, lost response | pending |
| 3: CI credentials and supply chain | SECN-GSL-201/202/203/204/205, SEC-GSL-202/203/204 | eight workflow files, catalog generation; trusted ref, token scope, dependency and publish boundaries | pending |
| 4: session integrity and history | DAT-GSL-001/002/004, STT-GSL-001/002/003, IOP-GSL-003, OBS-GSL-003 | revision storage, handoff storage, migrations, legacy import, CLI lifecycle; retention, transitions and imports | pending |
| 5: input formats and workspace writes | IOP-GSL-002/004, DAT-GSL-003, ARC-GSL-003 | shell handlers/compilation/formats, workspace store/service, settings IPC; explicit selection, bounded decode, atomic writes | pending |
| 6: temporary resources and OAuth | IOP-GSL-001/005, FSR-GSL-001/002, REL-GSL-001 | large-response handler, extension manager, Peekaboo, plugin staging, two OAuth caches/CLI; cleanup and error propagation | pending |
| 7: backend process registry/startup | CON-GSL-001, OBS-GSL-001/002 | backendProcessRegistry, config atomic writer, goslingServe; cross-process updates, durability, bind failure | pending |
| 8: network and content egress | SEC-GSL-003, LLM-GSL-002/003 | ACP aux router, classifier and telemetry; authenticated routes and explicit content scope | pending |
| 9: terminal bounds and dead API | WFG-GSL-001/002, FSR-GSL-003, NEG-GSL-001, ARC-GSL-005 | DiffViewer, PermissionPrompt, markdown cache, tui, permission_store; narrow/short terminals, bounded memory, public compatibility | pending |
| 10: source-grounded documentation | ARC-GSL-001/002/004, CMP-GSL-001/002 | architecture, README/releases, tagteam plan, advisory metadata; accurate paths/versions, no compliance promotion | pending |

Stage 1 establishes credential validation used by dispatch and inspector together. Stage 2
must preserve native picker grants while removing renderer self-authorization. Stage 4 must
resolve ADR-0018's explicit retention/recovery semantics before any schema change; Stage 10
documents dispositions, rather than silently changing an accepted contract. Stage 7 owns
registry durability alongside locking so the shared write function is reviewed once.
Every stage carries focused tests and a second adversarial review; final gates re-check
the union and separately inspect completeness.

## Contract baseline

- AGENTS.md is canonical; .giles YAML is advisory and stale. No fleet-compliance claim.
- Website-login guide promises origin binding; substitution lacks that check: confirmed drift.
- ADR-0017's 2026-09-19 amendment permits renderer-supplied session grants; the supplied
  High finding explicitly requests repairing that trust boundary. Native selection remains.
- ADR-0018 explicitly specifies path-wide revisions retained after chat deletion and DB
  commit before replacement for recoverability. DAT-GSL-001/002/004 need disposition against
  this declaration; changing those semantics would be a product/contract amendment.
- ADR-0015 expressly permits operator-selected input files outside the working directory;
  IOP-GSL-002's proposed containment would contradict that contract. Check the main-owned
  picker/authenticated ACP boundary before deciding the finding.

## Validation and checkpoint

- Pre-change Desktop: `pnpm exec vitest run src/main/rendererIpc.test.ts src/utils/rendererDirectoryGrants.test.ts --maxWorkers=2` — 12 tests passed.
- Rust validation request pending; baseline Rust runtime status unknown.
- Current next action: implement and verify origin-bound substitution and native confirmation
  for newly requested renderer directory roots; continue staged triage of all supplied IDs.

## Remaining work and limits

All stages remain open. No finding is closed yet. GitHub-side credential rotation and live
CI executions remain external owner work; local workflow tests do not prove those actions.

## Source repairs and static-only validation — current disposition

While work continued, the checkout advanced to `89dcef6920d55363035753456e9e4ffdf654240b`; another actor committed the earlier local repairs. No reset/revert was performed. Later repairs remain in the working tree.

The operator selected **formatting and static scans only** and **per-call password approval with explicit tool-trust disclosure**. These replace the pending questions above. No further runtime tests, Rust builds or clippy are authorized. New regression tests are written but unexecuted. Typechecking, syntax/format checks and config/source scans remain allowed.

Independent critique confirmed the contract counterfindings and inspected credential/grant, hook/OAuth/CI, Office, identity/CAS, terminal and edit-recovery changes. The Office preflight was narrowed after critique to preserve CRC tolerance and duplicate ZIP entries, while reading the exact snapshot that is later parsed. The source uses bounded actual expansion rather than a second open of the mutable file.

| Finding | Current disposition / evidence |
| --- | --- |
| SEC-GSL-001 | Implemented user-selected policy: declared URL checks and mandatory approval, with external-tool trust limitation disclosed. |
| SEC-GSL-201 | Implemented native main-process approval; sensitive filtering on renderer requests; 13 focused tests passed before static-only instruction. |
| SECN-GSL-201 | Local workflow repaired: trusted main only; external secret rotation/revocation remains owner work. |
| LLM-GSL-001 | Implemented resolved host-identity gate for judge auto-approval; external/unknown tools require review absent explicit user grants. |
| LLM-GSL-004 | Falsified against current source: summon/delegation.rs rejects every SubAgent before either sync/async delegation. Auto-parent, role and extension ceilings already exist; no recursion patch needed. |
| IOP-GSL-001 | Repaired (static): process-owned private spill cache, 64 MiB/1,000-file cap and dead-owner startup sweep in agents/large_response_handler.rs. Overflow reports inline fallback and preserves all advertised paths, including same-batch results. Legacy unowned files remain a manual residue follow-up. |
| IOP-GSL-005 | Implemented: missing Peekaboo returns manual-install guidance; tool dispatch no longer calls the installer. |
| DAT-GSL-004 | Not-a-defect against accepted ADR-0018: revisions intentionally survive chat deletion; a purge/retention amendment would be separate product work. |
| ARC-GSL-003 | Repaired (static): disclosed output-folder side effect and separate native Library confirmation; equivalent Git queries and literal escaping; shared adjacent directory lock protocol, re-read then append/fsync in workspace/service.rs and Desktop settingsIpc/processFileLock. Duplicate-addition and held Desktop-compatible writer regressions added, unexecuted; actual cross-process execution remains unverified. |
| SECN-GSL-202 | Repaired (static): docs synthesis runs Chat/no-profile/no-session in isolated cwd/state with sanitized environment; read-only token and nonpersisted checkout. Separate write-token job publishes only fixed guide as untrusted review-required PR. scripts/synthesize-ci-guide.py syntax inspected only. |
| SECN-GSL-203 | Implemented: unnecessary six provider API keys removed from version-bump workflow and caller. |
| SECN-GSL-204 | Repaired (static): manifest/ZIP generators share reviewed commit 329a55d1500748a0a48d64629757cbe2622e34bf and bounded validation. Per-run owned checkout prevents user-work deletion/concurrent-source collision. Existing live catalog fallback/install sources retain external trust; generators and hosted deployment unexecuted. |
| REL-GSL-001 | Implemented removal error propagation in both actual gosling/src/providers caches, CLI removal and reauthentication. |
| SEC-GSL-002 | Implemented: raw scanner explanation omitted from log fields; detailed operator-facing explanation retained. Existing argument logs are keys only. |
| SEC-GSL-003 | Documentation corrected: health/readiness probes are unauthenticated; ACP probe verifies the token. Readiness clients preserved. |
| LLM-GSL-002 | Implemented enable-time Desktop content-scope notice and endpoint/override documentation; classification data retained. |
| LLM-GSL-003 | Documentation repaired: full message/span egress disclosed before Langfuse configuration; OTLP scope included. |
| STT-GSL-001 | Implemented allowed state edges plus expected-status SQL CAS and affected-row assertion; regression added, unexecuted. |
| STT-GSL-002 | Contract disposition plus documentation: archive suppresses active listing/prompting/planning rather than freezing all metadata. Explicit CLI resume already announces restoration. Session-management guide now states exact semantics. |
| CON-GSL-001 | Repaired (static): backendProcessRegistry.ts re-reads under crash-recoverable processFileLock, serializes cross-process updates and merges sweep changes by pid plus startedAt. Regression source inspected, unexecuted. |
| DAT-GSL-002 | Not-a-defect against accepted ADR-0018: path-wide authorized revision history is intentional; no session FK added. |
| IOP-GSL-002 | Not-a-defect against ADR-0015: native-main-selected references may lie outside the workspace; authenticated ACP clients are trusted control-plane actors. |
| IOP-GSL-003 | Implemented strict message parsing and I/O error propagation with line numbers; malformed files are preserved and never imported partially as success. |
| IOP-GSL-004 | Implemented bounded actual entry expansion and declared sizes on the immutable bytes passed to Office parsing; CRC tolerance and duplicate-name behavior preserved. |
| DAT-GSL-001 | Not-a-defect against accepted ADR-0018: commit-before-replace preserves recovery snapshots; existing phantom-write conflict guard retained. |
| DAT-GSL-003 | Repaired (static): malformed original is preserved and actionable loader error replaces silent default reset. Valid temporary-snapshot recovery retains private corrupt backup. workspace/store.rs regression rewritten for failed load and unchanged bytes. |
| SEC-GSL-202 | Implemented environment indirection for PR author, quarantine list and PR number; no event interpolation remains in run script. |
| SECN-GSL-205 | Implemented secret-free PR build and separate no-checkout deploy job; Wrangler pinned to registry-observed 4.149.0. |
| SEC-GSL-203 | Repaired (static): release caller maps only named signing secrets on tags; unsigned branch jobs get no signing secrets or OIDC. Windows signed job is separate; reusable callees declare matching optional secrets. No live signing/CI verification. |
| SEC-GSL-204 | Implemented trusted-base checkout and same-repository release-branch/bot gate; PR history is data, not executed code. |
| ARC-GSL-001 | Documentation repaired: module table matches workspace/mod.rs and workspace_handlers; bootstrap/planning_access added. |
| ARC-GSL-002 | Documentation repaired: source 1.5.0 distinguished from historical verified 1.4.0 install; notes and release checklist synchronized. |
| ARC-GSL-005 | Implemented secure atomic legacy-store persistence and explicit unused-runtime annotation; public API preserved. |
| STT-GSL-003 | Historical policy disposition: v24 intentionally resets all workspace flags under opt-in policy; lost operator intent cannot be recovered. ADR0017 now states exact reset and re-enable requirement. Historical migration unchanged; non-workspace/imported flags and independent workspace policy retained. |
| ARC-GSL-006 | Start-agent repair (static): current Auto parent, no SubAgent recursion, contained child directory, inherited folder/admission/extension/authentication/credential snapshots, valid scoped destinations and fresh provider; external-tool provider child uses Chat. Global view/send/list/interrupt remain the documented single-operator capability, without per-parent ownership isolation. |
| CMP-GSL-001 | Documentation repaired: current stewardship notice marks dated Giles artifacts as historical advisory evidence; no fabricated scan/compliance promotion. |
| CMP-GSL-002 | Documentation repaired: Tagteam plan marked historical/removed with source commits and migration v30; original body retained. |
| WFG-GSL-001 | Implemented cell-budget truncation of DiffViewer header/footer; navigation unchanged; terminal typecheck passed. |
| WFG-GSL-002 | Implemented full row budgeting and option window around selection; hidden choices remain keyboard reachable; static typecheck passed. |
| WFG-GSL-003 | Implemented forced backend reconciliation after uncertain truncation, mutation fence until refresh and no destructive retry; typecheck passed. |
| FSR-GSL-001 | Repaired (static): extension secret/inline-code staging uses locked OwnedTempDir under session data root and startup sweep; live creation is protected by root publication lock. Public add_client Option<TempDir> retained through separately held caller ownership. Earlier anonymous/unowned residues cannot safely be auto-deleted. |
| FSR-GSL-002 | Repaired (static): owned plugin staging lock enables dead-owner recovery on all platforms; Unix legacy cleanup retained and removal failures reported. Unknown pre-patch Windows directories are preserved. Windows behavior unexecuted. |
| FSR-GSL-003 | Implemented 200-entry LRU and total-character budget; cache identity includes source/width; typecheck passed. |
| ARC-GSL-004 | Documentation repaired: current schema v38, v37 admissions and v38 session ID high-water allocator described. |
| NEG-GSL-001 | Implemented removal of unreachable interactive initialPrompt prop/branches; separate --text route preserved; typecheck passed. |
| OBS-GSL-001 | Repaired (static): Rust atomic/secret writers sync parent after rename on Unix; registry writes private unique temp, syncs file, renames and syncs directory on non-Windows. Windows directory durability remains platform-limited; no power-loss test. |
| OBS-GSL-002 | Repaired (static): goslingServe retries at most three fresh ports only after observed official address-in-use child failure and cleanup. Other failures propagate. Bounded collision regression added, unexecuted; readiness alone still has ordinary unauthenticated-probe limitations. |
| OBS-GSL-003 | Repaired (static): ACP queues prompt fence, cancels local run, waits at most30s for idle, claims local turn and shuts down before deletion. DB transaction refuses even expired lease from live owner; failed deletion invalidates loaded agent and asks reload. Regression confirms active-turn history preservation in source only. |
| OBS-GSL-004 | Implemented policy check at each actual hook emission, including deferred extended hooks and blocking Stop; no-hook path avoids extra DB query. |

### Observed validation

- Pre-directive Desktop grant regression run: 13 passed (12 baseline); no subsequent runtime execution.

- Desktop and terminal `pnpm exec tsc --noEmit`: passed on the covered changed source; final repeated static checks remain required after later edits.

- Six changed workflows: YAML parsed and static security assertions passed. Initial YAML scan had a wrong dependency path and failed before inspection; corrected to the installed `ui/node_modules/yaml`.

- `cargo fmt --all` and `cargo fmt --all -- --check`: passed before latest Rust changes; final repeat pending. Rust type/build/runtime verification is intentionally excluded.

- Desktop English message catalog regenerated by static extraction; `node scripts/i18n-check.js` passed. New disclosure uses the normal English fallback where translation is absent.

- Native pickers, real browser destination behavior, Windows cleanup, live CI, credentials, packaged app and all new regressions remain unexecuted. No global behavioral-equivalence or fleet-compliance claim.

### Final source and record checkpoint

All 49 detailed report IDs are reconciled in the current table above. The original report remains historical, with a current addendum linked here. The ten repair stages have local source or contract dispositions; runtime verification is excluded by the operator. This record is explicitly allowlisted in .gitignore, indexed in docs/INDEX.md and trackable.

Another workspace process committed the source patch to `5fc8dc068` after `89dcef692`; this run did not create those commits or reset the checkout. Final shared-ignore-lock and record/disclosure amendments remain in the working tree until the operator's normal commit flow.

The independent critic performed a union re-audit and a distinct completeness pass. It caught and drove corrections to Office compatibility, public TempDir API, live staging publication, cache result availability, catalog ownership, fenced-heading parsing, child provider lifetime, authentication inheritance and failed-delete cached state. The two closure requirements were a durable 49-ID record and truthful Orchestrator global-capability disclosure; both are present. The final shared ignore-lock amendment received a separate targeted static review. It corrected disappearing-lock enumeration and conservative Windows tasklist failure handling in subprocess.rs. The critic found no further concrete source defect. A held Desktop-compatible marker now characterizes contention/cancellation and preservation of the other writer's rules; it remains unexecuted and is not actual cross-process evidence.

### Final validation scope

- Rust formatting and formatting check: passed; no Rust compilation, tests, clippy or packaging.
- Desktop and terminal `pnpm exec tsc --noEmit`: passed after source changes.
- Desktop changed files formatted with Prettier; terminal changed components/cache/tests Prettier check passed.
- All12 changed workflow YAML files parsed; static assertions passed for trusted main smoke, isolated model/publisher tokens and keys, guide-only publication, secret-free PR build, provider-key removal, named tag-only signing secrets, unsigned Windows no OIDC and shell env indirection.
- Three catalog JavaScript files passed Node syntax check. CI synthesis helper passed Python AST parsing; helper/model/generators were not executed.
- English message extraction/check passed. New links/ADR paths and unchanged governance markers scanned; diff whitespace check passed before final record amendment.
- Earlier grant tests:12 baseline,13 post-repair passed before the static-only directive. All other new regressions are unexecuted.

### Equivalence and residual follow-ups

The source preserves native operator pickers, authenticated single-operator ACP access, path-wide output history and its recovery ordering, explicit archive restoration, historical migration semantics, CRC-tolerant/duplicate-name Office input, hosted/external provider mode distinctions and the public add_client TempDir API. Behavioral equivalence is source-inspected, not runtime-proven.

- Owner action: rotate/revoke and investigate the10 previously exposed CI LLM/Databricks secrets (SECN-GSL-201), and verify live branch protections/environments. No account changes or rotation performed.
- Saved passwords: declared destination validation plus mandatory per-call approval, as explicitly selected by the operator. The approved external tool can still ignore its declared URL or redirect; no generic origin-confinement guarantee.
- Orchestrator global session management remains a broad single-operator capability. Per-parent ownership or multi-user isolation would require a separate policy/architecture change, not silently changing current product semantics.
- Earlier unowned temporary spill/secret staging files need operator review; cleanup intentionally cannot infer ownership from anonymous directories. Windows ACL/cleanup/durability, actual Office decompression, cancellation/provider lifecycles, small-terminal rendering, native dialogs and live workflow behavior need runtime verification when authorized. The pre-existing terminal40x10 minimum layout is outside the two component budget fixes.
- Global output-revision purge/retention changes, advisory Giles refresh/compliance and production catalog rebuild/deploy were not performed. Existing catalog/install fallback remains externally trusted.

Final status: `completed_with_partial_verification` within the authorized local repair scope; no claim of runtime/security certification or release readiness.
