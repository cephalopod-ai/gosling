# 2026-08-26 Clean independent audit

Gold-template master report for a fresh exhaustive agent-skills audit of
gosling. Prior audit reports, playtest ledgers, repair campaign logs, and
polish finding ledgers were **not** used as defect evidence. Playtest was
operator-excluded.

## Executive Verdict

This pass found **no Critical** (unauthenticated remote privileged write)
defect on the current local-first default. It did find a cluster of
**High, source-confirmed** defects on operator-facing security controls:
ACP reports permission grants as success when disk persist failed; Desktop
“Always Allow all extension tools” consumes the live approval before the
durable write; Auto and subagents can enable extensions without the approval
the UI advertises; headless CLI Auto auto-allows inspector-failure
confirmations that Desktop would still show; assistant-mentioned absolute
document paths become Desktop preview grants; and current-schema workspace
validation failure is recovered by wiping to a new Default workspace.

CLI/vendor providers (`executes_tools_outside_gosling`) discard the
`Provider::stream` tool list and Auto auto-acks their native confirmations.
Imported sessions are marked untrusted for artifacts and UI, then still
fed to the model. These are Confirmed code properties, not speculation.

**Do not pause local-single-user merge** for a remote-exploit pause, but
**patch the High persist/approval/grant findings before treating Desktop
permissions or Auto/summon as a security boundary.** Do not call `v1.1.0`
“hardened and fully compatible” until README compatibility claims and the
permission-persist lie are fixed.

Lenses completed: ARC, IAPI, IOP, DAT, STT, CON, TMP, CAS, INV, WFG, WEB,
PIP, EAPI, LLM, MCP, AOC, CMP, NEG, AID, ARCN, XREPO, DEAD, PATH, REL, FSR
(failsafe family). **Required lens `audit-security` (plus OWASP/Node/repo
posture/triage) did not finish** before this report was closed; that
inventory is `Not Reviewed` except where adjacent clusters quoted the same
code. Findings below are parent-adjudicated: Confirmed only when the cited
line was re-read in this run. The reliability cluster completed after the
first close and is folded in here.

## Scope

- Repository: `/Users/eric/Work/vscode/forked/gosling`
- Branch / commit: `main` `82e676be3`
- Prompt: clean independent comprehensive agent-skills audit; no prior
  audit/log evidence; do not apply playtest; then aggregate, deconflict,
  confirm, and update ledgers/docs
- Involvement: L1; ceiling: read_only for code; docs writes authorized
- Skills: see Coverage matrix
- Files/directories inspected: crates (`gosling`, `gosling-cli`,
  `gosling-mcp`, `gosling-providers`, `gosling-sdk-types`, `gosling-server`),
  `ui/desktop`, `ui/text`, `documentation/`, `docs/architecture.md`,
  `docs/adr/*`, `README.md`, `AGENTS.md`, `.giles/*.yaml` (CMP only)
- Commands/tests run: none against the target (static). `git status --short`
  at start and close
- Effort budget: eight parallel clusters, ~50 files each; parent
  confirmation of High citations
- Constraints: forbidden as evidence: `docs/cloud/**` (prior),
  `docs/logs/session/**` (prior), `docs/build/**/audits/**`, polish
  finding ledgers, `docs/TODO.md` as defect evidence

Dirty tree at start (preserved): `docs/INDEX.md` (+1 line), untracked
placeholder for this report, untracked
`ui/desktop/tests/e2e/focused-playtest.spec.ts` (untouched).

## Draft Prompt Assessment

The supplied prompt is treated as a draft. Intended mission preserved:
fresh exhaustive audit using only applicable catalog skills, then
aggregate/deconflict/confirm and update ledgers. Expanded to adjacent
seams implied by the product (ACP `_meta`, CLI provider `stream()`, MCP
Apps as a second initiator, import→model, Auto/subagent). Under-specified:
no per-lens file budget (applied ~50/cluster). Over-narrow: “do not use
audits/logs” forbids prior findings as evidence but still allows current
source, ADRs, and README claim-vs-code. Playtest exclusion is honored
(static GUI only).

Assumptions challenged: README hardening bullets are not proof; Chat mode
“no tools” is enforcement location, not advertisement; local-first is
already contradicted by the official remote-server guide.

## Coverage matrix

| Skill | Status | Notes |
|---|---|---|
| audit-architecture-seam | applied | ARC-GSL-* |
| audit-contract-internalapi | applied | IAPI-GSL-* |
| audit-architecture-drift | applied | AID bootstrap (no `.architecture/` registry) |
| audit-architecture-nodejs | applied | Electron/TS only |
| audit-contract-crossrepo | applied | Goose catalog seam |
| audit-deadcode-cleanup | applied sampled | no knip; no deletions |
| audit-repo-path-consistency | applied | `~/.agents`, RuntimePaths |
| audit-dataflow-input-output | applied | IOP-GSL-* |
| audit-dataflow-integrity | applied | DAT-GSL-* |
| audit-dataflow-state-transition | applied | STT-GSL-* |
| audit-dataflow-concurrency | applied | CON-GSL-*; races capped Likely |
| audit-dataflow-temporal | applied | |
| audit-dataflow-cascade | applied | |
| audit-invariant-sync | applied | |
| audit-workflow-gui | applied | static only |
| audit-design-webapp | applied thin | Gate 5 not reviewed |
| audit-dataflow-pipeline-graph | applied | |
| audit-pipeline-externalapi | applied | |
| audit-security-llm | applied | |
| audit-mcp-server | applied sampled | |
| audit-agent-orchestration-code | applied | |
| audit-compliance-posture | applied | many CMP codes N/A (not a scanner) |
| audit-negative-space | applied | last among completed lenses |
| audit-security | **incomplete** | cluster still running at close |
| audit-security-code / owasp / nodejs | **incomplete** | same cluster |
| audit-security-repo-posture / triage | **incomplete** | same cluster |
| audit-security-vuln-harness | deferred | bounded hunt folded into SEC cluster; full 6-phase not run |
| audit-reliability | applied (late fold-in) | REL-GSL-*; `/health` vs `/status` split held |
| audit-failsafe-readiness / dependency-criticality / recovery-idempotency / operator-signal | applied (late fold-in) | FSR/REC; FSR-GSL-001 merged into WFG-GSL-004 |
| audit-memory-lifecycle / resource-lifecycle | applied sampled | OOM capped Likely; RES-GSL-001 |
| audit-performance-profile | deferred | no target metric |
| audit-playtest-app | operator excluded | |
| audit-repo-state-reconciliation | light, this pass only | working tree + this report; no prior audits |
| audit-equation-sourcebase / security-supabase / flutter-ios / graphdb-design / go-repo-hardening | not_applicable | existing repo exclusions |
| audit-multiagent-consensus | not requested | |

## Surface Inventory

| Surface | Actor | Input/Trigger | State/Output | Boundary | Reviewed |
|---|---|---|---|---|---|
| Agent tool loop | model, operator, MCP App | tool requests | Allow/Deny/RequireApproval / dispatch | inspectors + mode | Yes |
| Auto / subagent / summon | parent + child | delegate + Auto | MCP enable, tools | explicit-grant class | Yes |
| ACP `tools/permissions/set` | Desktop | tool name + level | `permission.yaml` | persist must error | Yes |
| Desktop Always Allow extension | operator | pending approval | live allow + bulk AlwaysAllow | consume-then-write | Yes |
| CLI `-n` Auto | operator | confirmation | AllowOnce | should Deny | Yes |
| Chat mode | operator | prompt | skip tools as success | execution held; advertise not | Yes |
| CLI/vendor providers | operator | stream + tools | external agent | `executes_tools_outside_gosling` | Yes |
| Session import/export | operator | JSON | new session + history | untrusted flag / model | Yes |
| `sessions.db` | all processes | WAL | messages, artifacts, library | no session lease | Yes |
| `workspaces.json` | WorkspaceService | JSON | workspaces + profiles | flock; recover-on-validate | Yes |
| `permission.yaml` | CLI + Desktop | YAML | tool policy | in-process mutex only | Yes |
| Artifact preview | Desktop | assistant paths | file read | grantedFiles short-circuit | Yes |
| MCP cache / documents | model | path | read/delete | canonicalize prefix | Yes |
| Plugin `git clone` | operator | URL | plugin tree | `--` + `ext::` reject | Yes |
| MCP Apps guest | MCP HTML | tools/call, append | tools + composer | inspectors; default visibility | Yes |
| `gosling serve` / `goslingd` | ACP client | host/secret | control plane | loopback unauth refuse; remote shared secret | Yes |
| Goose catalog | docs site | live JSON | extension cards | dual JS/TS converters | Yes |
| README claims | operator | marketing | belief | vs code | Yes |
| Ink approval | operator | truncated args | permission | full args not shown | Yes |

## Boundary Map

| Surface | Intended Boundary | Enforced At | Status |
|---|---|---|---|
| Inspector **error** | Fail closed → RequireApproval | `tool_inspection.rs` Err arm | **Holds** |
| Auto advisory RequireApproval | Downgrade if inspector opts in | same; security/egress/adversary/working-dir opt out | Holds as designed |
| Auto write/shell | Explicit user permission | `requires_explicit_grant_in_auto` | **Holds for write/exec; misses HTTP, manage_extensions, mixed MCP** |
| Chat **execution** | No tools run | skip remaining + app path fail-closed | **Holds** |
| Chat **capability** | No tools offered | prompt text only; tools still sent | **Fails** |
| Permission persist | Durable grant/deny | manager returns Err; ACP swallows | **Fails** |
| Bulk Always Allow | Persist then resolve | resolve first | **Fails** |
| Headless Auto confirmation | Deny / abort | CLI auto-AllowOnce | **Fails** |
| Artifact preview | Roots / picker / policy | `grantedFiles` bypass | **Fails** |
| Workspace recover | Parse-fail only | also validation-fail | **Fails** |
| Import untrusted | Not model authority | UI/artifacts only | **Fails** (model) |
| MCP cache path | Inside cache dir | canonicalize + prefix | **Holds** |
| Plugin clone | No option/ext helper | `--` + `ext::` | **Holds** |
| Unauth serve non-loopback | Refuse | CLI serve | **Holds** |
| Config RMW | Cross-process flock | `.save.lock` | **Holds** |
| Permission RMW | Same as config | in-process mutex | **Fails** |
| Workspace CRUD | Backend SoT | WorkspaceService + ACP | **Holds** |
| Inner MCP guest origin | Opaque / other port | HTML + ACP guest listener | **Holds** (inner) |
| Session lease | One turn owner | in-memory per process | **Fails** (cross-process) |

## Findings Table

Parent-adjudicated. Duplicate cluster IDs are merged. Confidence is
independent of severity. Race manifestation stays **Likely**.

| ID | Severity | Confidence | Evidence Basis | Domain | Title | Patch Priority | Blast Radius | Complexity | Cost | Nominal Agent |
|---|---|---|---|---|---|---|---|---|---|---|
| WFG-GSL-002 | High | Confirmed | source-evidenced | Workflow-GUI | ACP `tools/permissions/set` succeeds when persist failed | 1 | Workflow | local_guardrail | S | codex |
| WFG-GSL-001 | High | Confirmed | source-evidenced | Workflow-GUI | Always Allow all extension tools consumes then bulk-writes | 1 | Workflow | workflow_protocol | S | codex |
| LLM-GSL-004 | High | Confirmed | source-evidenced | Security-LLM | Auto/subagent can `manage_extensions` without approval | 1 | Workflow | local_guardrail | S | codex |
| WFG-GSL-004 | High | Confirmed | source-evidenced | Workflow-GUI | CLI Auto non-interactive auto-allows confirmations | 1 | Service | workflow_protocol | S | codex |
| IOP-GSL-001 | High | Confirmed | source-evidenced | Input-Output-Path | Assistant absolute document paths become preview grants | 1 | Workflow | local_guardrail | S | codex |
| DAT-GSL-001 | High | Confirmed | source-evidenced | Data-Integrity | Current-schema workspace validation failure wipes to Default | 1 | Service | persistence_recovery | M | claude |
| AOC-GSL-001 | High | Confirmed | source-evidenced | Orchestration | Auto auto-acks vendor-CLI tool confirmations | 1 | Workflow | workflow_protocol | M | claude |
| ARC-GSL-002 | High | Confirmed | source-evidenced | Architecture | CLI providers discard `stream()` tools | 2 | Workflow | workflow_protocol | M | claude |
| LLM-GSL-001 | High | Confirmed (code) | source-evidenced | Security-LLM | Outbound HTTP tools skip Auto explicit-grant class | 2 | Workflow | local_guardrail | S | codex |
| LLM-GSL-002 | High | Confirmed | source-evidenced | Security-LLM | AlwaysAllow is tool-name-only | 2 | Workflow | local_guardrail | S | codex |
| LLM-GSL-003 | High | Confirmed | source-evidenced | Security-LLM | Auto exec/write class misses mixed-risk MCP tools | 2 | Workflow | local_guardrail | S | codex |
| IAPI-GSL-001 | High | Confirmed | source-evidenced | Architecture | Session wire contract is untyped ACP `_meta` | 2 | Workflow | local_guardrail | M | gpt |
| CON-GSL-001 | High | Likely | source-evidenced | Concurrency | No cross-process session lease; compact can wipe sibling | 2 | Workflow | cross_process_coordination | M | claude |
| REL-GSL-001 | High if ≥6 sessions / Medium typical | Confirmed (guard); Likely (dual-agent) | source-evidenced | Reliability | Agent LRU busy-skip does not see ACP in-flight turns | 2 | Service | cross_process_coordination | M | claude |
| FSR-GSL-002 | Medium | Confirmed persist; drop Likely | source-evidenced | Failsafe | Failed MCP load is persisted out of the enabled set | 3 | Workflow | persistence_recovery | S | codex |
| REL-GSL-002 | Medium | Confirmed missing guard; hang Likely | source-evidenced | Reliability | Shell tool has no default timeout | 3 | Workflow | local_guardrail | S | codex |
| RES-GSL-001 | Medium | Confirmed missing guard; hang Likely | source-evidenced | Failsafe | computercontroller scripts wait with no timeout | 3 | Local | local_guardrail | S | codex |
| REC-GSL-001 | Medium | Confirmed idiom; torn file Likely | source-evidenced | Failsafe | Desktop backend PID registry is in-place JSON write | 3 | Service | local_guardrail | S | codex |
| NEG-GSL-005 | High if remote / Low loopback | Confirmed | source-evidenced | Negative-Space | Official remote `goslingd` is a shared-secret multi-client plane | 3 | Service | external_service_semantics | L | human-owner |
| CAS-GSL-001 | Medium | Confirmed | source-evidenced | Cascade | Imported untrusted history is still model authority | 3 | Workflow | workflow_protocol | M | claude |
| CON-GSL-002 | Medium | Likely | source-evidenced | Concurrency | `permission.yaml` RMW is in-process only | 3 | Workflow | local_guardrail | S | codex |
| CON-GSL-003 | Medium | Likely | source-evidenced | Concurrency | Shared stem `.tmp` for atomic writes | 3 | Local | local_guardrail | S | codex |
| IOP-GSL-002 | Medium | Confirmed | source-evidenced | Input-Output-Path | CLI file import vs ACP/JSON import parity | 3 | Workflow | workflow_protocol | M | gpt |
| DAT-GSL-002 | Medium | Confirmed | source-evidenced | Data-Integrity | Workspace delete leaves session/library keys | 3 | Workflow | persistence_recovery | M | gpt |
| DAT-GSL-003 | Medium | Confirmed | source-evidenced | Data-Integrity | Session delete leaves session-scoped library rows | 3 | Local | local_guardrail | S | codex |
| WFG-GSL-005 | Medium | Confirmed | source-evidenced | Workflow-GUI | Chat still ships tools; skips render as success | 3 | Workflow | local_guardrail | S | codex |
| WFG-GSL-006 | Medium | Confirmed | source-evidenced | Workflow-GUI | Ink approval truncates the payload | 3 | Workflow | operator_ux | S | codex |
| NEG-GSL-001 | Medium | Confirmed | source-evidenced | Negative-Space | MCP App is a hidden tool/chat actor | 3 | Workflow | workflow_protocol | M | claude |
| NEG-GSL-006 | Medium | Confirmed | source-evidenced | Negative-Space | Session-dir `0o700` failure is not fail-closed | 3 | Local | local_guardrail | S | codex |
| PATH-GSL-001 | Medium | Confirmed | source-evidenced | Architecture | `~/.agents` is not gosling-namespaced | 3 | Local | governance_decision | S | human-owner |
| ARC-GSL-001 | High | Confirmed | source-evidenced | Architecture | God objects: Agent, SessionManager, ACP server, Electron main | 4 | Repo | workflow_protocol | L | claude |
| ARC-GSL-003 | Medium | Confirmed | source-evidenced | Architecture | MCP `Tool` leaks into Provider port | 4 | Repo | local_guardrail | M | gpt |
| ARC-GSL-004 | Medium | Confirmed | source-evidenced | Architecture | MCP is a hard compile-time core dependency | 4 | Repo | persistence_recovery | L | claude |
| ARC-GSL-005 | Medium | Confirmed | source-evidenced | Architecture | Process-global Config / SessionStorage | 4 | Service | cross_process_coordination | M | claude |
| XREPO-GSL-001 | Medium | Confirmed | source-evidenced | Architecture | Goose catalog live, unpinned, two converters | 4 | Repo | local_guardrail | S | gpt |
| INV-GSL-001 | Medium | Confirmed | source-evidenced | Invariant Sync | Import omits snapshot/provider/model that copy preserves | 4 | Workflow | workflow_protocol | M | gpt |
| INV-GSL-002 | Low | Confirmed | source-evidenced | Invariant Sync | Schema default `gosling_mode='auto'` vs SmartApprove | 5 | Local | local_guardrail | XS | codex |
| CMP-GSL-001 | Medium | Confirmed | source-evidenced | Compliance-Posture | “Full compatibility with 70+” overclaim | 5 | Repo | operator_ux | S | human-owner |
| CMP-GSL-003 | Medium | Confirmed | source-evidenced | Compliance-Posture | Prompt-injection docs still opt-in; default is on | 5 | Workflow | operator_ux | S | gpt |
| EAPI-GSL-001 | Medium | Likely | source-evidenced | Reliability | MCP HTTP client missing request timeout | 5 | Workflow | local_guardrail | XS | codex |
| WEB-GSL-001 | Medium | Confirmed | source-evidenced | Workflow-GUI | Tool status is color-only 2px dot | 6 | Workflow | operator_ux | XS | codex |
| IOP-GSL-005 | Medium | Likely | simulation-reasoned | Input-Output-Path | Self-update archive unbounded | 6 | Local | local_guardrail | S | codex |
| NEG-GSL-008 | High if reachable | Plausible | simulation-reasoned | Negative-Space | Outer MCP sandbox default still has `allow-same-origin` | 6 | Workflow | local_guardrail | S | gpt |
| CMP-GSL-002 | Low | Confirmed | source-evidenced | Compliance-Posture | “Fully deconflicted” hides shared `~/.agents` | 6 | Local | operator_ux | XS | human-owner |
| CMP-GSL-004 | Medium | Confirmed | source-evidenced | Compliance-Posture | Stale Giles YAML still says `compliance_status: blocked` | 6 | Repo | governance_decision | S | human-owner |
| AID-GSL-001 | Medium | Likely | source-evidenced | Architecture | architecture.md still says SessionManager v22; schema is 28 | 6 | Repo | operator_ux | XS | gpt |
| AID-GSL-002 | Low | Confirmed | source-evidenced | Architecture | CUSTOM_DISTROS.md points at missing `ui/desktop/src/api` | 6 | Repo | local_guardrail | XS | gpt |
| TMP-GSL-001 | Info | Confirmed | source-evidenced | Temporal | Recipe columns retained with no consumer | 7 | Local | governance_decision | XS | human-owner |

Merged aliases (do not patch twice): STT-GSL-001 → WFG-GSL-002;
WFG-GSL-003 / PIP-GSL-001 → LLM-GSL-004; NEG-GSL-004 → CAS-GSL-001;
DAT-GSL-004 → INV-GSL-002; DAT-GSL-006 field-drop → INV-GSL-001;
DEAD-GSL-001 → INV-GSL-003/XREPO-GSL-001; CMP-GSL-002 ↔ PATH-GSL-001;
FSR-GSL-001 → WFG-GSL-004.

## Detailed Findings

### WFG-GSL-002: ACP permission set reports success when persist failed

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Workflow-GUI

Evidence:
- `crates/gosling/src/acp/server/tools.rs:123-145` — `update_user_permission` `Err` is logged; handler still `Ok(SetToolPermissionsResponse {})`
- `crates/gosling/src/config/permission.rs:104-120,232-293` — in-memory map updates then `persist()`; persist failure returns `Err` and does not roll back; comment already requires the caller to tell the operator

Observed behavior:
- Desktop treats empty ACP success as a durable AlwaysAllow/NeverAllow. Disk may be unchanged. Restart restores the old policy. In-memory grant still applies this process.

Expected boundary:
- ACP error if persist failed; no success DTO; UI must not show a lasting grant.

Failure mechanism:
- Adapter converts a failed postcondition into an empty success envelope.

Break-it angle:
- Make `permission.yaml` unwritable; Always Allow from Desktop; toast success; restart; tool is allowed again.

Impact:
- Operator-deception on a security control; denials do not survive restart.

Operational impact:
- Blast radius: Workflow
- Side-effect class: file + user-visible
- Reversibility: compensatable
- Operator visibility: silent (log-only)
- Rerun safety: unsafe

Adjacent: WFG-GSL-001, CON-GSL-002

Recommended mitigation:
- Return ACP error if any entry failed to persist; optionally roll back in-memory like `remove_extension`.
- Test: persist-fail `tools/permissions/set` is not `Ok`.

Implementation assessment: local_guardrail / S / tests / codex

Non-goals: Do not change PermissionLevel semantics.

### WFG-GSL-001: “Always Allow all extension tools” consumes then bulk-mutates

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Workflow-GUI

Evidence:
- `ui/desktop/src/components/ToolApprovalButtons.tsx:158-175` — `resolveAcpPermissionRequest(..., 'always_allow')` **before** `listTools` / `setToolPermissions`
- `ui/desktop/src/acp/permissionRequests.ts:27-33` — `isAcpPermissionRequestPending` exists and is unused here

Observed behavior:
- Clicking the extension-wide grant immediately allows **this** call. Then it bulk-writes AlwaysAllow for every listed tool. If persist fails, the UI can show failure while the current tool already ran.

Expected boundary:
- Non-consuming liveness check → persist bulk grant → resolve only on persist success.

Failure mechanism:
- Irreversible ACP resolve is sequenced before the durable grant.

Break-it angle:
- Fail `setToolPermissions` after resolve: current shell runs; other tools not granted; operator sees an error.

Impact:
- Unintended execution of the pending tool; grants that do not match the toast.

Operational impact: Workflow / process + user-visible / compensatable / UI-visible (wrong) / unsafe

Recommended mitigation:
- Use `isAcpPermissionRequestPending`; persist first; on persist failure leave the request pending.
- Test: forced persist error keeps the tool blocked.

Implementation assessment: workflow_protocol / S / tests / codex

### LLM-GSL-004: Auto/subagent can enable extensions without approval

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security-LLM

Evidence:
- `crates/gosling/src/permission/permission_inspector.rs:163-192` — Auto Allow/Deny is **before** `MANAGE_EXTENSIONS_TOOL_NAME_COMPLETE` RequireApproval
- `crates/gosling/src/permission/tool_class.rs:70-76` — explicit Auto grant covers only code-execution and write tools
- `crates/gosling/src/agents/platform_extensions/ext_manager.rs` Enable starts `add_extension`

Observed behavior:
- In Auto (and every summon/delegate child forced Auto), the model can enable/disable extensions without a human prompt. Write/shell remain Auto-denied unless AlwaysAllow, but process start and non-write tools become live.

Expected boundary:
- Extension enable/disable always RequireApproval except an explicit AlwaysAllow on that tool, including Auto and subagents.

Failure mechanism:
- Auto short-circuit classifies extension management as “read-only.”

Break-it angle:
- SmartApprove parent approves `summon__delegate`; child Auto calls `manage_extensions` Enable on a networked MCP.

Impact:
- Confused deputy: parent never named the extension; child starts it.

Operational impact: Workflow / process + network / compensatable / transcript if watched / unsafe

Recommended mitigation:
- Treat `extensionmanager__manage_extensions` as `requires_explicit_grant_in_auto`.
- Test: Auto subagent Enable does not `add_extension`.

Implementation assessment: local_guardrail / S / tests / codex

### WFG-GSL-004: CLI Auto headless auto-allows confirmations Desktop would surface

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Workflow-GUI

Evidence:
- `crates/gosling-cli/src/session/mod.rs:1210-1232` — non-interactive Approve/SmartApprove refuse; Auto **auto-allows**
- `crates/gosling/src/tool_inspection.rs:137-162` — inspector error synthesizes `RequireApproval(Some(...))`
- `crates/gosling/src/agents/agent.rs:2501-2520` — Auto auto-acks provider-native confirmation ids (see AOC-GSL-001)

Observed behavior:
- Same Auto session: Desktop shows a security fallback prompt; CLI `-n` logs a warning and AllowOnce. Inspector-down fail-closed becomes fail-open on CLI.

Expected boundary:
- Headless Auto must Deny/abort any confirmation the inspector could not judge.

Failure mechanism:
- CLI maps “no TTY” to AllowOnce for every mode that is not Approve/SmartApprove.

Impact:
- Headless Auto is the documented unattended mode; a broken inspector then executes tools Desktop would hold.

Operational impact: Service / process / irreversible (command ran) / log-only / unsafe

Recommended mitigation:
- Non-interactive: DenyOnce / abort on any `action_required`, including Auto.
- Test: inspector failure in CLI Auto does not dispatch the tool.

Implementation assessment: workflow_protocol / S / tests / codex

### IOP-GSL-001: Assistant-mentioned document paths become Desktop preview grants

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Input-Output-Path

Evidence:
- `crates/gosling/src/session/artifacts.rs:107-138` — `from_path` rejects `..` but accepts **absolute** paths
- `ui/desktop/src/main.ts:392-407` — `artifactFiles` only `stat`s that the path is a file
- `ui/desktop/src/utils/artifactFileAccess.ts:9-15` — `grantedFiles.has(resolvedPath)` returns immediately, skipping root checks

Observed behavior:
- A completed assistant message that names an existing `.txt`/`.md`/`.json`/`.pdf` outside workspace roots can be inventoried, then previewed/copied via Outputs.

Expected boundary:
- Deliverable grants confined to working dirs / output roots; absolute paths not discovered unless already inside folder policy.

Failure mechanism:
- Discovery is treated as capability.

Break-it angle:
- Model mentions `/Users/me/Documents/notes.txt`; operator opens it from Outputs.

Impact:
- Conversation-controlled read of user document-like files via the preview surface (local, needs an open session).

Operational impact: Workflow / file / reversible (close session) / UI-visible / unsafe

Recommended mitigation:
- Root-constrain `artifactFiles` in `validateArtifactRoutingConfig`; require `from_path` under working dirs.
- Test: absolute `.txt` outside workspace must not pass `assertArtifactFileAccess`.

Implementation assessment: local_guardrail / S / tests / codex

### DAT-GSL-001: Workspace store recovery wipes current-schema invalid documents

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/workspace/store.rs:204-209,262-280,282-305` — `read_document` calls `validate()`; recoverable if JSON is invalid **or** `schema_version <= STORE_SCHEMA_VERSION`; then rename to `workspaces.corrupt-*.json` and write a new Default

Observed behavior:
- Valid JSON that fails `validate()` (empty `workspaces`, dangling active id, duplicates) is treated like truncated garbage. All workspace/profile records are replaced.

Expected boundary:
- Recover only on parse/truncation failure. Same-schema semantic validation must fail-closed.

Failure mechanism:
- “Malformed” conflates parse failure with invariant failure.

Impact:
- Durable loss of workspace definitions and credential catalog pointers.

Operational impact: Service / file / compensatable (corrupt backup) / log-only / unsafe

Recommended mitigation:
- If parse succeeds and schema is supported, return the validation error.
- Test: `workspaces: []` with `schema_version: 1` does not reinitialize.

Implementation assessment: persistence_recovery / M / tests / claude

### AOC-GSL-001: Auto auto-acks vendor-CLI tool confirmations

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Orchestration

Evidence:
- `crates/gosling/src/agents/agent.rs:2501-2520` — in Auto, confirmation request ids are `handle_confirmation(..., AllowOnce)` without inspectors
- `crates/gosling/src/providers/claude_code.rs:775-777,841-847` — `executes_tools_outside_gosling`; `stream(..., _tools)` discards tools
- `crates/gosling/src/agents/agent.rs:1732-1738` — working-dir restriction is a bolt-on bail, not the trait

Observed behavior:
- Claude Code / Codex / Gemini CLI / Cursor run tools in the vendor process. Gosling Auto acks their permission prompts. Gosling inspectors never see those tool names/args.

Expected boundary:
- Either Auto must not auto-ack vendor confirmations, or vendor-CLI sessions must not be Auto; tools-capability must be a distinct port (ARC-GSL-002).

Failure mechanism:
- Uniform `Provider::stream` plus Auto “no operator” policy applied to a provider that still prompts.

Impact:
- Auto + Claude Code is unattended execution of a second agent’s tools.

Operational impact: Workflow / process / irreversible / UI-visible / unsafe

Recommended mitigation:
- Do not auto-ack `ActionRequired` from `executes_tools_outside_gosling` providers in Auto; refuse the session or require Approve.
- Type-level split so CLI providers cannot silently drop tools (ARC-GSL-002).

Implementation assessment: workflow_protocol / M / tests / claude

### CAS-GSL-001: Imported untrusted history is still model authority

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Cascade

Evidence:
- `crates/gosling/src/session/session_manager/session_transfer.rs:53-90` — `history_trusted: false`; every message `with_imported_untrusted()`; mode forced Approve
- Consumers of the flag: ACP replay meta, UI badge, artifact discovery skip (`artifacts_storage.rs:85`)
- Non-consumers: agent reply / provider payload (no `imported_untrusted` filter)

Observed behavior:
- Import containment is UI + mode + working-dir restriction. The model still receives imported user/assistant text and tool results as ordinary conversation.

Expected boundary:
- Untrusted history labeled or stripped at the **model** boundary.

Failure mechanism:
- Advisory provenance never becomes an enforcing input gate.

Impact:
- State poisoning of the next tool loop from a file the product already marked untrusted.

Operational impact: Workflow / process / compensatable / UI-visible badge / unsafe if tools run

Recommended mitigation:
- Inject a durable system notice; strip or wrap imported `ToolResponse` bodies unless the operator opts in.

Implementation assessment: workflow_protocol / M / tests / claude

### LLM-GSL-001 / LLM-GSL-003: Auto explicit-grant class is too narrow

Severity: High  
Confidence: Confirmed (code property)  
Evidence basis: source-evidenced  
Domain: Security-LLM

Evidence:
- `crates/gosling/src/permission/tool_class.rs:70-76` — `requires_explicit_grant_in_auto` = code execution **or** write
- `crates/gosling/src/security/egress_inspector.rs:356-362` — `is_web_tool` is a small name list (`web_fetch`, `fetch`, `browser_navigate`, `http_request` + suffixes)
- Computercontroller mixed tools (`cache`, automation) are not all in the Auto deny set

Observed behavior:
- Auto denies ungranted shell/write. An MCP `http`/`request` tool that is not in `is_web_tool` is Auto-Allowed as “read-only.” `manage_extensions` is the same hole (LLM-GSL-004).

Expected boundary:
- Auto explicit-grant covers egress, extension management, and mixed-risk MCP god-tools, or unknown tools fail closed.

Recommended mitigation:
- Expand `requires_explicit_grant_in_auto` (or fail-closed unknown). Include HTTP family and `manage_extensions`.
- Test: Auto `web_fetch`/`http_request`/`manage_extensions` denied without AlwaysAllow.

Implementation assessment: local_guardrail / S / tests / codex

### CON-GSL-001: No cross-process session lease

Severity: High  
Confidence: Likely (missing lease Confirmed; compact-wipe manifestation not drilled)  
Evidence basis: source-evidenced / simulation-reasoned  
Domain: Concurrency

Evidence:
- `ui/desktop/src/main.ts:1376-1398` — each `createChat` can start its own `gosling serve`
- `crates/gosling/src/acp/server.rs` — `active_prompt_runs` is in-memory
- `crates/gosling/src/session/session_manager/message_storage.rs` — compact `DELETE FROM messages WHERE session_id = ?` then re-insert

Observed behavior:
- Prompt single-flight is process-local. Two Desktop windows (or Desktop + CLI) sharing `sessions.db` can both run a turn. Auto-compaction in one process can delete the other’s in-flight messages.

Expected boundary:
- Durable per-session lease; compact must not delete rows it did not snapshot.

Calibration: race manifestation capped **Likely**.

Implementation assessment: cross_process_coordination / M / tests / claude

### REL-GSL-001: Agent LRU busy-skip does not observe ACP in-flight turns

Severity: High if ≥ `DEFAULT_MAX_SESSION` concurrent chats; Medium on typical 1–2 window desktop  
Confidence: Confirmed (missing registration); dual-agent manifestation Likely  
Evidence basis: source-evidenced / simulation-reasoned  
Domain: Reliability

Evidence:
- `crates/gosling/src/execution/manager.rs:280-313,440-442` — skip-busy uses `is_session_busy` == `cancel_tokens.contains_key`; `DEFAULT_MAX_SESSION = 5`
- Production `try_register_cancel_token` caller is `orchestrator.rs:492` (plus tests), not ACP `reply`

Observed behavior:
- Opening a 6th ACP session can evict a session whose prompt is still running. Eviction skips `shutdown` if another Arc is held, then `get_or_create_agent` builds a second agent; last persist wins.

Expected boundary:
- Any in-flight turn (ACP or orchestrator) must pin the LRU entry.

Failure mechanism:
- Busy truth lives in ACP `active_run`; LRU consults a different map.

Operational impact: Service / process + DB / compensatable / silent / unsafe

Recommended mitigation:
- Register ACP cancel tokens with AgentManager, or have LRU query ACP active runs.
- Test: ACP prompt in flight + 6th session → original remains, no second agent.

Implementation assessment: cross_process_coordination / M / tests / claude

Non-goals: Do not raise `DEFAULT_MAX_SESSION` as the fix.

### NEG-GSL-005: Official remote server collapses single-operator

Severity: High if bound on a network; Low on loopback  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Negative-Space

Evidence:
- `documentation/docs/guides/remote-gosling-server.md` — `GOSLING_HOST=0.0.0.0`, shared `GOSLING_SERVER__SECRET_KEY`
- `crates/gosling-cli/src/cli.rs` — `gosling serve` refuses unauth non-loopback (stricter sibling)

Observed behavior:
- Documented `goslingd` deployment exposes the control plane. Auth is one shared secret with no per-client roles.

Expected boundary:
- Product claims local-first, or multi-client authz exists.

Impact:
- Full agent/tool/secret-store access for anyone with the key. Not an auth bypass; it is an actor-model hole.

Implementation assessment: external_service_semantics / L / docs+runtime / human-owner

## Non-Findings / Checked But Not Confirmed

Important seams checked and held in **current** source (parent-verified or cluster-quoted with line):

- **Inspector-error fail-closed** — `tool_inspection.rs:137-162` synthesizes `RequireApproval` even in Auto. Security/egress/adversary/working-dir set `auto_downgrades_require_approval() = false`.
- **Chat execution** — `agent.rs:2666-2712` skips remaining tools; `dispatch_app_tool_call` fail-closes if `approved` is empty (`1167-1173`); Chat inspector `continue` defaults to needs_approval (`permission_inspector.rs:81-83`).
- **Auto write/exec without grant** — `requires_explicit_grant_in_auto` denies ungranted shell/write/`computercontroller__automation_script`.
- **Subagent leftover approvals** — denied, not hung (`agent.rs:913-939`). Nested delegates refused.
- **Repo-committed capability policy cannot grant extensions** — held in delegation.
- **MCP cache / document path confinement** — canonicalize + prefix.
- **Plugin `git clone --` and `ext::` reject** — `plugins/mod.rs:292-330`.
- **Unauth `gosling serve` non-loopback** — refused.
- **config.yaml / secrets / workspaces.json RMW** — flock + UUID temp (config) / exclusive mutate (workspaces). Concurrent tests exist (not re-run).
- **Tool-op unique key / in_doubt not auto-replayed**.
- **Provider empty stdout** — OpenAI empty message is `ExecutionError` (WFG cluster).
- **Prompt launch ≠ completion** — Desktop `chatSessionController` waits for `prompt()` settle.
- **Default mode SmartApprove** — enum default; UI matches. Schema DEFAULT is the separate INV-GSL-002.
- **Inner MCP guest origin isolation** — ACP guest on its own port; gosling-server inner iframe omits `allow-same-origin`.
- **Workspace backend SoT** — handlers → WorkspaceService; renderer does not persist workspaces.json.
- **Forbidden OpenAPI desktop client** — `ui/desktop/src/api` does not exist; workspaces use `@repo-makeover/gosling-sdk`.
- **Keyring optionality** — feature + file fallback.
- **Goose config/data/keyring names** — `app_name: "gosling"`; keyring `"gosling"`.
- **Import artifact skip** — `imported_untrusted` prevents artifact minting.
- **ADR-0017 session-private dirs** — additive, this session only; live workspace refresh still intentionally absent (documented divergence, not a defect).
- **CMP scanner codes** — N/A; this repo is not an SSDF collector. Goose v1.47 guide is not a certification. RELEASE notes labeled candidate.
- **Electron window flags sampled** — `webSecurity` / `contextIsolation` / `sandbox` / `nodeIntegration: false`.

## Break-It Review

| Attack | Result |
|---|---|
| Inspector throws in Auto | **Survives** (synthesized RequireApproval) |
| CLI Auto + same inspector error | **Fails** WFG-GSL-004 |
| Permission persist fail via ACP | **Fails** WFG-GSL-002 |
| Always Allow all extension, persist fail | **Fails** WFG-GSL-001 |
| Auto `manage_extensions` Enable | **Fails** LLM-GSL-004 |
| Chat ACP `tools/call` | **Survives** (unapproved rejected) |
| Chat model still tool-calls | **Hits** WFG-GSL-005 (advertise + success skip) |
| Absolute assistant `.txt` preview | **Fails** IOP-GSL-001 |
| Workspace JSON `workspaces: []` | **Fails** DAT-GSL-001 |
| Two `gosling serve` + compact | **Likely fail** CON-GSL-001 (not drilled) |
| Two concurrent `config.yaml` writers | **Survives** (flock) |
| Two concurrent `permission.yaml` writers | **Likely fail** CON-GSL-002 |
| Import poisoned tool result then reply | **Fails** CAS-GSL-001 |
| `../` MCP cache path | **Survives** |
| `git clone ext::` / `--upload-pack` | **Survives** |
| `--host 0.0.0.0 --dangerously-unauthenticated` | **Survives** (`gosling serve`) |
| Official remote guide `0.0.0.0` + secret | **Breaks** single-operator assumption |
| Empty provider 200 / missing `[DONE]` | **Survives** (WFG cluster) |
| Ink approve truncated args | **Fails** WFG-GSL-006 (static) |

## Recommended Patch Order

1. WFG-GSL-002 — ACP must not `Ok` persist failure  
2. WFG-GSL-001 — persist bulk grant before resolving the live request  
3. LLM-GSL-004 / LLM-GSL-001 / LLM-GSL-003 — Auto explicit-grant class (extensions, HTTP, mixed MCP)  
4. WFG-GSL-004 — CLI non-interactive Deny/abort on any confirmation  
5. IOP-GSL-001 — root-constrain artifact grants and discovery  
6. DAT-GSL-001 — fail-closed on same-schema workspace validation failure  
7. AOC-GSL-001 + ARC-GSL-002 — vendor-CLI Auto and tools-capability split  
8. CAS-GSL-001 — model-boundary handling of `imported_untrusted`  
9. CON-GSL-002 / CON-GSL-003 — permission flock; UUID temps  
10. REL-GSL-001 — pin ACP active runs in AgentManager LRU  
11. FSR-GSL-002 — persist configured/failed extensions, not only survivors  
12. REL-GSL-002 / RES-GSL-001 — host default timeouts for shell and computercontroller  
13. REC-GSL-001 — atomic backend PID registry write  
14. CON-GSL-001 — session lease (after design)  
15. IAPI-GSL-001 — generate SessionMeta; kill `as` cast  
16. Docs: CMP-GSL-001/003, PATH-GSL-001/NEG-GSL-005, AID-GSL-001/002  
17. Remaining Medium/Low in the table  

Do not start with god-object extraction (ARC-GSL-001) until the High persist/approval holes are closed.

## Regression Test Strategy

| Test | Purpose | Finding |
|---|---|---|
| Persist-fail `tools/permissions/set` → ACP error; UI not “Always allowed” | False success | WFG-GSL-002 |
| Bulk allow: pending check → persist → resolve; persist error leaves tool blocked | Consume-then-write | WFG-GSL-001 |
| Auto + subagent `manage_extensions` Enable does not `add_extension` | Confused deputy | LLM-GSL-004 |
| Auto `http_request` / unclassified MCP HTTP denied without AlwaysAllow | Class gap | LLM-GSL-001 |
| CLI Auto + inspector error: tool not dispatched | Headless fail-open | WFG-GSL-004 |
| Assistant absolute `.txt` outside workspace cannot pass artifact access | Preview grant | IOP-GSL-001 |
| Workspace JSON `workspaces: []` does not reinitialize | Recovery wipe | DAT-GSL-001 |
| Auto + Claude Code confirmation is not auto-acked (or session refused) | Vendor CLI | AOC-GSL-001 |
| After import, provider payload contains untrusted marker / no raw imported tool results | Model authority | CAS-GSL-001 |
| Two processes, same session `/prompt`: one fails closed | Lease | CON-GSL-001 |
| Two processes each add a distinct NeverAllow; both keys on disk | Permission flock | CON-GSL-002 |
| Chat: provider `tools` empty; UI status not success | Chat truth | WFG-GSL-005 |
| Unix chmod failure aborts session pool | Confidentiality | NEG-GSL-006 |
| Guest iframe cannot read `window.parent.location.hash` | Origin | NEG-GSL-008 |

## Deferred Risks

- Full `audit-security` / OWASP / Node / repo-triage / vuln-harness — **not finished**. Do not treat this report as a complete SEC clearance.
- Reliability / failsafe family folded in late; memory findings remain Potential-only (no heap snapshots).
- CON-GSL-001 compact-wipe — needs a two-process drill (`requires-authorized-drill`).
- IOP-GSL-005 zip-bomb — Likely until measured.
- NEG-GSL-008 outer `allow-same-origin` — Plausible until a guest-hash read is attempted.
- MCP `append()` as ACP user prompt — Likely, parent not fully traced.
- `rmcp` mid-session 401 refresh — not traced into the dependency crate.
- ACP local-unauthenticated: library file-link and import duplication severity jump if ACP is bound beyond the user’s desktop (NEG-GSL-005).
- Performance profile — deferred (no metric).

## Skill Escalation

| Finding | Primary | Secondary | Why |
|---|---|---|---|
| WFG-GSL-002 | Workflow-GUI | State-Transition, Security | Success DTO without persist |
| WFG-GSL-001 | Workflow-GUI | State-Transition | Approval consumed before grant |
| LLM-GSL-004 | Security-LLM | Cascade, Orchestration | Auto child enables MCP |
| WFG-GSL-004 | Workflow-GUI | Failsafe | Inspector fail-closed → CLI allow |
| IOP-GSL-001 | Input-Output-Path | Security, Cascade | LLM text → FS read |
| DAT-GSL-001 | Data-Integrity | Failsafe, Operator-signal | Silent reinit |
| AOC-GSL-001 | Orchestration | Security, Architecture | Tools outside inspectors |
| ARC-GSL-002 | Architecture | Security | Flattened provider port |
| CAS-GSL-001 | Cascade | Security-LLM, Negative-Space | Import → model |
| CON-GSL-001 | Concurrency | Temporal, Workflow-GUI | Compact + second backend |
| NEG-GSL-005 | Negative-Space | Security | Multi-client control plane |
| IAPI-GSL-001 | Architecture (IAPI) | Workflow-GUI | Silent field drop |
| CMP-GSL-001 | Compliance-Posture | Workflow-GUI | Operators read a guarantee |

Incomplete required lenses: escalate a **follow-up SEC** pass (OWASP/Node/repo posture) before any “comprehensive security clearance” claim. REL/FSR landed in this fold-in.

## Repo-state reconciliation (this pass only)

| Source | State |
|---|---|
| Working tree | Dirty: `docs/INDEX.md`, this report, unused playtest spec (untouched) |
| Branch | `main` `82e676be3` |
| This audit | Open High findings listed above; no code repairs in this run |
| `docs/TODO.md` | Prior 2026-08-15 leftovers remain; this pass prepends a new section |
| Playtest | Explicitly not run |

Next evidence-backed action: patch WFG-GSL-002 then WFG-GSL-001, then Auto explicit-grant (LLM-GSL-004/001/003), then CLI Auto (WFG-GSL-004).

## Validation Limits

- Static only. No `cargo test`, Desktop, CLI live run, two-window compact, hostile MCP guest, or `0.0.0.0` bind.
- Oracle integrity: no in-process suite used as production proof. Concurrent config/workspace tests were **read**, not re-run.
- `audit-security` (plus OWASP/Node/repo posture/triage) **did not complete**. Absence of those findings is not a SEC clearance.
- Goose live payloads were not fetched. Contrast/a11y not measured. Gate 5 (device) not reviewed.
- Forbidden trees were not used as defect evidence. This report does not inherit 2026-08-15 IDs except where current comments still name them in source.
- Commit hash taken from `git rev-parse` at preflight.

## Final Confidence

**Medium** overall: High for parent-confirmed persist/approval/grant/Auto-class defects; Low for incomplete SEC/REL surfaces and unreproduced races.

Do not describe this run as a complete 13-lens clearance. Describe it as a clean independent audit of the completed lenses, with SEC unfinished.
