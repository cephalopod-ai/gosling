# 2026-10-13 — Grok findings repair

## Scope and baseline

Repair the 20 supplied findings in `grok-comprehensive-audit-13oct2026.md`, against main at `e61fc45ab`. Initial user work: cancellation accessor changes in active_runs.rs/manage_sessions.rs, an index addition and the untracked audit report. Preserve those changes. Baseline runtime validation is unknown; findings are reproducible by source traces. No account changes, secret rotation, packaging, installation, merge or publication.

Workflow: catalog `repair-defect-patchset`. The supplied set is frozen; unrelated discoveries become follow-ups.

## Stages and interaction analysis

1. Permission and log boundaries: SEC-GROK-001/002/003 (security, P0/P2, low), LLM-GROK-001 (security, P0, medium). EgressInspector::inspect and Auto policy; WorkingDirScopeInspector::inspect and shell analysis; ToolInspectionManager log fields. Regressions cover approval preservation, host-only disclosure, path-free mutations, existing loopback/grants/denials and read-only commands.
2. Tool outcomes and durability: MCP-GROK-001/002 (correctness, P0/P2, medium/low), ORCH-GROK-001, REL-GROK-001/002, FSR-GROK-004 (reliability, P2, low/medium). Computer controller and Windows/Linux helpers, managed reply drain, spill caller, reply_stream extension save, GitHub rejected-sign-in cleanup. Keep successful outcomes and partial content; make failures visible with truthful flags/bounds.
3. Process ownership: FSR-GROK-001/002/003 (reliability, P1, medium). Backend registry termination/sweep, child_process startup ownership and McpClient::close/drop, text UI main/shutdown. Preserve live-owner and process identity checks, stdio grouping and bounded grace; test owned disposable processes.
4. Composer: WFG-GROK-001 (frontend bug, P1, low). ChatInput submission/queue/keyboard/form guards. Preserve streaming queue behavior and prevent clearing unfinished attachments.
5. Workspace taxonomy: ARC-GROK-004 (backend correctness, P2, medium). Workspace service errors and ACP handlers. Keep validation/not-found/storage distinct without disclosing storage detail.
6. Documentation: ARC-GROK-001/002/003/005 and WFG-GROK-002 (docs defects, P2/P3, low). Provider/config/remote guides, index and portable resume blocks. Keep historical evidence and public legacy API intact.

Stages 1–4 have distinct source ownership. Stage 1 changes stricter permission behavior consumed by orchestration; unavailable approval remains a failure. Stage 2 failure flags must survive spill handling. Stage 3 transfers process ownership rather than duplicating it. No persisted schema or configuration format changes are planned. Stage 6 reconciles records after source verification.

## Governing contracts

AGENTS.md/GEMINI.md are active. docs/architecture.md governs workspace error taxonomy; ADR-0017 and WorkingDirScopeInspector's contract govern additive folder grants; the permissions guide governs stricter Auto decisions. MCP result/error types and existing shell status handling govern tool outcomes. Existing process supervision and lifecycle tests govern owned children. Provider include_dir registration and manifests govern guide claims. Baseline: the supplied findings are pre-existing drift from these contracts. Giles YAML is advisory.

## Validation and closure

Repository policy limits cargo build/test/clippy to explicit user requests. Cargo compile checks, UI tests/checks and source/document scans were executed; Rust regression tests were compiled but not run. Final status: `completed_with_partial_verification`.

## Finding dispositions

All 20 supplied source/document defects are repaired locally. Source-repaired entries below are not claims of executed Rust or native-platform behavior.

| Finding | Repair and evidence | Disposition |
| --- | --- | --- |
| SEC-GROK-003 | EgressInspector requires approval for every undecided non-loopback domain and opts out of Auto downgrades. Existing grants and NeverAllow precedence remain. Updated inbound/Auto tests and the integration gate compile. | Source repaired; Rust runtime pending |
| LLM-GROK-001 | Every non-read-only shell segment without an extracted target gets an approval result; a benign segment cannot mask it. Restriction/workspace early guards and explicit paths retain their checks. Restricted-session and adjacent read regressions compile. | Source repaired; Rust runtime pending |
| MCP-GROK-001 | Windows and Linux direct outputs check ExitStatus; clipboard writes check wait status; unsupported Linux commands fail; generated multi-command scripts propagate subprocess failure. The wrapper maps platform failure to CallToolResult::error. Linux helper tests compile on this Unix host. | Source repaired; native Windows/Linux runtime pending |
| MCP-GROK-002 | automation_script chooses success/error from status after retaining requested output saving. Both status cases have a compiled regression. | Source repaired; Rust runtime pending |
| ORCH-GROK-001 | Managed stream errors return Err and label accumulated text as partial; existing tool_call mapping returns an error flag. New partial-stream regression compiles. | Source repaired; Rust runtime pending |
| REL-GROK-001 | Refused spills return a bounded omission and error flag instead of copying the body. Existing cache limits and successful file references remain. Over-cap regression compiles. | Source repaired; Rust runtime pending |
| SEC-GROK-001 | Removed operator-facing inspection reasons from both tracing sinks; confidence, action, inspector and finding ID remain. Operator approval explanations remain intact. Captured-log regression compiles. | Source repaired; Rust runtime pending |
| SEC-GROK-002 | Egress tracing fields and approval reason use domain, not raw URL/userinfo/query/path. Captured-log and reason-redaction regressions compile. | Source repaired; Rust runtime pending |
| WFG-GROK-001 | Button, queue, keyboard, form and delayed dictation share blockers. Delayed voice submission uses the current callback. Tests retain pending images/files, recording/transcribing/restarting drafts; loaded image submission, streaming queue and completed voice submission still work. | Verified in component tests; native GUI pending |
| FSR-GROK-001 | Unix sweep signals and polls the owned group; retains surviving groups, including when their leader is gone. Existing live-owner/identity checks and Windows taskkill remain. Two actual group/descendant fixtures pass, plus registry/supervisor regressions. | Verified with disposable macOS processes; Windows pending |
| FSR-GROK-002 | Startup group guard transfers to McpClient on connect and remains armed for close/drop. Termination is idempotent; startup failure still drops the guard. Close/drop descendant regression and the existing abandonment fixture compile. | Source repaired; Rust runtime pending |
| FSR-GROK-003 | Normal Ink exit, successful/failed text mode, spawn errors and OS signals use one shutdown promise. Child.killed is no longer treated as exit; TERM escalates after two seconds, and exit is awaited. Text success/failure with TERM-ignoring ACP fixtures pass. | Text paths runtime verified; interactive /exit and repeated-signal replay pending |
| ARC-GROK-004 | Typed workspace validation/not-found markers survive anyhow boundaries; handlers map missing resources to resource-not-found and unknown storage/lock failures to internal error with a safe summary. Classification and actual forced-lock regressions compile. Public Result and persistent schemas remain. | Source repaired; Rust runtime pending |
| FSR-GROK-004 | GitHub removal/recovery success sentence requires both deletion and cache cleanup to succeed. Partial cleanup gives manual recovery instructions; updated success expectation compiles. | Source repaired; deletion/cache fault runtime pending |
| REL-GROK-002 | Extension-state save failure aborts the reply with explicit durability context before tools_updated is set; runtime effects are acknowledged rather than described as durably saved. Source trace and compile check cover propagation. | Source repaired; injected save-failure runtime pending |
| ARC-GROK-001 | Removed unshipped FuturMix/Routstr setup and table claims. EmpirioLabs follows the existing custom-provider setup, grounded in canonical metadata/catalog code. Remaining declarative links exist and tabs balance. | Documentation verified |
| ARC-GROK-002 | Live tool/egress decisions are permission.yaml; legacy public JSON store is explicitly not an app-managed file. Caller census confirms the existing legacy comment. | Documentation verified |
| ARC-GROK-003 | Index points to v1.5.0 as current source and retains v1.4.0 as dated installation evidence. Version/manifests and link scan agree. | Documentation verified |
| WFG-GROK-002 | Guide separates liveness-only /health from database readiness /status, matching the current handlers. | Documentation verified |
| ARC-GROK-005 | Both active resume blocks say to run from the checkout root instead of a user-specific absolute cd. Historical paths remain provenance. | Documentation verified |

## Executed validation

- `cargo fmt --all` and `cargo fmt --all --check`: passed.
- `cargo check -p gosling -p gosling-mcp --tests --no-default-features`: passed, including the new regression targets. This compiles tests; it does not execute them.
- Desktop `pnpm exec tsc --noEmit`: passed.
- Terminal `pnpm exec tsc --noEmit`: passed.
- Desktop `pnpm exec vitest run src/backendProcessRegistry.test.ts src/components/ChatInput.test.tsx src/goslingServe.test.ts src/goslingServeLeaseRegistry.test.ts --maxWorkers=2`: 46 tests passed across four files.
- Terminal `node --import tsx --test src/tui.test.ts`: three tests passed, including two TERM-ignoring child journeys (normal completion and prompt failure) inside one test.
- ESLint on all four changed Desktop files with `--max-warnings 0`: passed.
- Prettier check on all four changed Desktop files: passed. Terminal shutdown/test edits formatted; two unrelated existing formatting hunks were preserved.
- `node scripts/i18n-check.js`: passed; no new translation messages were introduced.
- Document scans: two remaining declarative JSON links exist; provider Tabs/TabItem tags balance; index links resolve; source version matches 1.5.0; active resume blocks are portable; legacy-file claim is corrected; both governance markers remain.
- `git diff --check`: passed. Session log is allowlisted in .gitignore and indexed.

## Stage review, equivalence and closing inspections

Each stage was reviewed against its pre-edit source trace and relevant declared contract. Successful tool stdout, saved domain decisions, loopback, literal URLs without network clients, read-only folder denials, incomplete-shell approvals, successful spills, unavailable child approval errors, live backend owners, Windows taskkill, public legacy store and persisted workspace formats remain. Behavioral equivalence is tested for the Desktop/terminal fixture paths and source-inspected elsewhere; it is not an exhaustive proof over inputs.

The first closing pass walked the changed workflows and their callers: inspector aggregation/Auto downgrade and permission application; shell segments/path collection; platform execution/tool wrappers; managed reply/tool error mapping; response spill/cache bounds; runtime extension save/update ordering; stdio startup/client close/drop; backend identity/live-owner/sweep/registry merge; all composer submission paths; text lifecycle paths; workspace service/ACP mapping; and provider/guide sources. This review found a repair-induced stale voice callback and corrected it with the current callback plus completed-voice regression. It also preserved the existing streaming queue and original text formatting outside the shutdown slice.

A distinct completeness pass reconciled all 20 IDs against the original mechanisms, checked test expectations for changed inbound/Auto and GitHub-success behavior, repeated compilation/targeted UI validation after corrections, inspected the union diff for unintended schema/API/config changes, and scanned doc links/markers and whitespace. No new architecture/contract drift was found in the inspected patch. Giles remains advisory; no fleet compliance is claimed.

Source record: the historical Grok report has a dated addendum pointing here. The October 12 provider/lifecycle R04 follow-up has a dated repair pointer. Initial cancellation-accessor edits remain untouched; the original index audit entry remains. No source commits were created.

## Limits and follow-ups

- No cargo build, cargo test or clippy was run because AGENTS.md restricts those commands to explicit build/test requests. The Rust regressions, including security logging/permissions, spill, automation status, orchestrator, extension close/drop and workspace lock taxonomy, remain unexecuted despite compiling.
- No installed GUI, interactive /exit, repeated-signal runtime replay, live extension/provider, live account or packaged build was tested. Windows helpers were source-inspected; Linux helper/test code compiled on macOS, without native display automation. These gaps prevent release/readiness certification.
- The extension guard covers normal startup abandonment, close and drop. SIGKILL of the owner cannot run Rust Drop; no OS death-watch or daemon containment was added, and separately daemonized groups remain outside ownership.
- Owner action from the prior audit: rotate/revoke the previously exposed CI LLM/Databricks secrets and verify account/CI settings. No credentials were accessed or rotated.
- Existing broader Windows process identity, provider replay fidelity and failed-turn steer durability follow-ups remain outside this supplied set. No new independent defect was absorbed into the patch set.

