# Mistral Comprehensive Audit — gosling — 12 October 2026

| Field | Value |
|---|---|
| Report | `mistral-audit-comprehensive-12OCT2026.md` (repo root) |
| Auditor | Mistral Vibe (lead) + 7 subagent audit groups |
| Target | `gosling` @ `29d31f7072f8a5d369c363444fdc4532b9bd95cb` ("Merge pull request #83 from cephalopod-ai/release/1.5.0"), branch `main`, clean tree |
| Date | 2026-10-12 |
| Mode | READ-ONLY static audit. Nothing executed: no builds, tests, network calls, servers, or installs. Playtest explicitly excluded by operator. |
| Skills pack | `/Users/eric/Work/vscode/agent-skills/` — shared contracts in `000_common/audit-base/` (report_template.md, finding_format.md, severity_matrix.md, confidence_calibration.md v3.1, cross_lens_escalation.md v3.1, evidence_discipline.md, routing_guide.md v3.2, audit_method.md); lenses + standalones in `010_audit/` |
| Skills applied | 13 lenses (all applicable) + 21 of 37 standalones (11 not-applicable with reasons, 5 catalog-excluded) — see Coverage Matrix |
| Findings | 45 findings (3 High, 10 Medium, 30 Low, 2 Info) + 4 observations (OBS) + 1 falsified subagent claim recorded as a non-finding |
| Prior audits consulted | in-repo IDs (SEC-GOS-*, CON/REL/DAT/MEM/RES/ARCN/RST/RSP-GSL-*, GSL-PT-*) — collision-checked; prior-cycle fixes verified present, not re-filed |

## Executive Verdict

Gosling's core permission pipeline, session storage, ACP transport, and Electron sandbox are genuinely hardened — most README/SECURITY hardening claims verified in code — but this audit confirms **three High findings that warrant action before the next release and before any further fork PR merges**:

1. **SEC-GSL-001** — saved website-login passwords are substituted into *any* model-proposed tool call with no destination-origin check, contradicting the docs. Combined with the inspector prompt showing the login's own URL rather than the destination, this is a ready-made indirect-prompt-injection exfiltration primitive against the user's saved credentials.
2. **SEC-GSL-201** — the Electron renderer can grant itself read/write/delete access to arbitrary host directories (including `~/.ssh`, `~/.aws`) through a single IPC call; the "guards" the code comments claim are only a filesystem-root and whole-home-dir check.
3. **SECN-GSL-201** — `pr-smoke-test.yml` exposes 10 org LLM/Databricks secrets to PR-head code execution on same-repo branches, gated only against forks. **Recommend rotating those secrets now** regardless of patch timing, and gating the workflow before further merges.

Beyond the Highs: 10 Medium findings (LLM-judge and delegation approval gaps, MCP response spill files, Peekaboo auto-install from a mutable brew tap, output revisions surviving session deletion, silent `.gitignore` mutation, three CI supply-chain exposures, and OAuth logout that silently fails), 30 Low, 2 Info, and 4 observations. Findings are predominantly **Confirmed at the code-property level** (source-evidenced); runtime manifestations (exfiltration chains, races, OOM, install behavior) are capped at **Likely/Plausible** per `confidence_calibration.md` because nothing was executed. All 13 applicable lenses and 21 applicable standalones were applied; the playtest standalone was excluded per the operator's explicit instruction. Patching is recommended now, in the order given in Recommended Patch Order.

## Scope

- **Repository/project / branch / commit:** `/Users/eric/Work/vscode/forked/gosling`, `main` @ `29d31f7072f8a5d369c363444fdc4532b9bd95cb` (workspace version 1.5.0), clean tree at start.
- **Prompt or session log reviewed:** operator request — "do a full audit screening (of applicable) using agent skills. Don't do playtest but do conduct audits of any and all skills that apply to this repo. write a report mistral-audit-comprehensive-12OCT2026.md to repo and skill standards." Prior in-repo audit docs under `docs/cloud/`, `docs/polish/`, and prior session logs were used as context and for ID-collision checks.
- **Skills (lenses) invoked:** Security-code (Rust), Security-LLM, Security-Node/TS, Security-repo-triage, Security-repo-posture; Input/Output-Path (dataflow IOP); State-Transition (STT); Data-Integrity (DAT); Concurrency (CON); Cascade (CAS); Temporal (TMP); Reliability (REL); Architecture-Seam (ARC); Workflow-GUI (WFG); Negative-Space (NEG); Compliance-Posture (CMP); Invariant-Sync (arch group). Standalones: agent-orchestration-code, architecture-drift, architecture-nodejs, contract-internalapi, dataflow-pipeline-graph, deadcode-cleanup, dependency-criticality, failsafe-readiness, mcp-server, memory-lifecycle, operator-signal, pipeline-externalapi, recovery-idempotency, repo-path-consistency, repo-state-reconciliation, resource-lifecycle, security-code, security-llm, security-nodejs, security-repo-posture, security-repo-triage. Not applied: playtest-app (operator exclusion), security-owasp, contract-crossrepo, optimization-opportunities, testsuite-economics, llm-config-drift, performance-profile, security-vuln-harness, design-webapp, design-handheld, multiagent-consensus (not applicable, reasons in Coverage Matrix). Catalog-excluded: equation-sourcebase, security-supabase, flutter-ios, graphdb-design, go-repo-hardening.
- **Files/directories inspected:** `crates/gosling` (agents, permission, security, session/session_manager, acp, config, workspace, providers, tracing, skills, plugins), `crates/gosling-mcp` (computercontroller, peekaboo, autovisualiser, docx), `crates/gosling-providers` (retry, OAuth modules, HTTP), `crates/gosling-cli`, `crates/gosling-sdk-types`, `ui/desktop/src` (main IPC, preload, renderer components, goslingServe, registries), `ui/text/src` (TUI components), `.github/workflows` (all 15), `docs/` + `documentation/` (architecture, INTENT, guides, ADRs, automation), `AGENTS.md`, `README.md`, `justfile`, `.giles/`, `governance/`, `reports/`.
- **Commands/tests run:** none that mutate or execute the project. Read-only: file reads, `grep`/`rg` sweeps, `git log`/`git status`, `sed` slices of large files. No `cargo`, `pnpm`, network, servers, or test runners were invoked (read-only constraint; AGENTS.md says run builds/tests only when asked).
- **Effort budget (per-lens) and what it bought:** 7 subagent groups, each scoped to a lens cluster with targeted slices of large files (session_manager.rs 7342 lines, agents/agent/ ~12.6k lines, config/base.rs, working_dir_scope_inspector.rs 2670 lines, otel/otlp.rs read in slices). Lead re-verified every High/Medium citation line-by-line and spot-checked Lows. This bought full-lens coverage of the security, state, integrity, reliability, architecture, workflow, concurrency, compliance surfaces; deep coverage of screen-control MCP extensions, OAuth modules, and all CI workflows.
- **Constraints:** read-only audit (no code changes, no builds, no tests, no network); playtest excluded by operator; report to repo root per operator; session log + INDEX entry per AGENTS.md documentation rules.

## Draft Prompt Assessment

- **Intended mission:** a full screening audit of every agent skill that applies to this repo, excluding playtest, with a report written to the repo at skill standards.
- **Explicit scope and command restrictions:** honored. Read-only throughout; no playtest-app skill; no runtime execution of any kind.
- **Assumptions challenged (and falsified):**
  - "Docs match code" — falsified: SEC-GSL-001 (website-logins doc claims origin-scoped substitution; code does none), SEC-GSL-003 (remote-server doc claims `/health`/`/status` require the token; they do not), ARC-GSL-001/002/004 and CMP-GSL-002 (stale doc tables, README version claims, undocumented schema v38, un-annotated removed-feature plan).
  - "The permission store persists denials" — falsified: `ToolPermissionStore` is dead code (ARC-GSL-005); the live mechanism is `config/permission.rs` `PermissionManager`.
  - "CI gates keep secrets away from PR-head code" — falsified: SECN-GSL-201, SEC-GSL-204, SECN-GSL-205, SECN-GSL-202/203/204 family.
  - "Provider retry is uncapped" — falsified by lead verification: `Retry-After` is capped at 3600 s (`crates/gosling-providers/src/http_status.rs:43,74`). Recorded as a non-finding, not a finding.
- **Angles applied within scope:** all 13 applicable lenses across 7 subagent groups (sec-llm, sec-node, data-io, data-flow, arch, ops, rel), each with held non-findings, break-it notes, and cross-lens stubs; lead merge with dedup/renumber and citation re-verification.
- **Adjacent work proposed but not performed:** runtime break-it harness for the concurrency/state findings; GitHub-side settings review (branch protection, environments, secret rotation status — needs API access); renderer-lens Playwright pass on the desktop markdown sink and MCP-app proxy guest content; deep per-provider secret-handling audit of `gosling-providers`; Nostr relay transport audit; docs-claims verification sweep (two docs-vs-code contradictions found suggests a systematic pattern).

## Surface Inventory

| Surface | Actor | Input/Trigger | State/Output | Boundary | Reviewed |
|---|---|---|---|---|---|
| Rust agent loop (`crates/gosling/src/agents/`) | model proposals, tool calls, hooks | session state, side effects, turns | permission/inspector pipeline, interaction_policy | Yes |
| Permission & inspector chain (`permission/`, `security/`) | model tool calls | approval/deny decisions, scanner verdicts | interaction_policy, inspector order, fail-closed | Yes |
| Website logins (`website_logins.rs`, inspector) | model tool calls bound to a login | password substitution into calls | destination-origin binding (missing) | Yes |
| Session storage (`session/`, `session_manager/`, migrations v1–38) | agent, ACP, CLI | persistent records, leases, plans, revisions | single-owner fs2 lock, schema versioning | Yes |
| ACP server (`acp/`, transport, custom_dispatch) | ACP clients (desktop, TUI, editors) | 145 custom requests, notifications | token auth, loopback default, action-bound dispatch | Yes |
| MCP extensions (`crates/gosling-mcp/`) | model tool calls | host actions: screen control, shell, files, docs | extension config, tool_class, malware check | Yes (screen-control deep) |
| Electron desktop (`ui/desktop/src/`) | renderer, main, preload | IPC, fs, processes, settings | contextIsolation, IPC allowlist, granted roots | Yes |
| Ink TUI (`ui/text/src/`) | operator + ACP client child | terminal rendering, prompts | fixed-grid Ink layout rules (AGENTS.md) | Yes (sampled) |
| CI/CD (`.github/workflows/`, 15 workflows) | PR/push/cron/schedule events | secrets, tokens, deploys, docs site | fork gates, action pinning, permissions | Yes (all) |
| Docs & governance metadata (`docs/`, `documentation/`, `.giles/`, `governance/`, `reports/`) | readers, agents, Giles | claims, indexes, advisory findings | source-groundedness, freshness | Yes |
| Providers (`crates/gosling-providers/`) | agent requests | HTTPS to LLM APIs, OAuth tokens | retry policy, auth headers, token caches | Sampled (retry, OAuth, HTTP status) |
| Skills/extensions discovery & admission | agent runtime | local + external Goose/AAIF catalogs | content-hash binding, ceilings, OSV check | Yes |
| OAuth flows (`providers/*_oauth.rs`, `chatgpt_codex.rs`) | user login/logout | token caches on disk | loopback bind, state check, 0o600 | Yes |

## Boundary Map

| Surface | Intended Boundary | Enforced At | Status |
|---|---|---|---|
| Model tool calls → host effects | permission pipeline + inspectors + working-dir scope | `tool_dispatch.rs`, inspector chain, `working_dir_scope_inspector.rs` | Held with gaps (SEC-GSL-001, LLM-GSL-001, LLM-GSL-004) |
| Renderer → host filesystem | IPC allowlist + per-session granted roots | `fileIpc.ts` `assertRendererFileAccess` | **Broken** (SEC-GSL-201) |
| CI PR-head code → org secrets | no secrets on untrusted code; fork/collaborator gates | workflow `if:` gates | **Broken** (SECN-GSL-201, SEC-GSL-204, SECN-GSL-205) |
| Saved website-login passwords → matching origin only | destination binding at substitution | `website_logins.rs` (no check exists) | **Missing** (SEC-GSL-001) |
| Session delete → data removal | cascade delete of session-owned rows | `session_crud.rs` | Partial (DAT-GSL-004: `output_revisions` survives) |
| Remote ACP access → token auth | bearer token, loopback default | `acp/transport/mod.rs`, `auth.rs` | Partial (SEC-GSL-003: `/health`, `/status` unauthenticated) |
| Extension/tool install → reviewed, pinned artifacts | admission, OSV malware check, ceilings | `extension_manager.rs`, `extension_malware_check.rs` | Partial (IOP-GSL-005: Peekaboo from mutable tap) |
| Docs claims → match source | doc review, stewardship gates | `docs/polish/` stewardship | Drift (ARC-GSL-001/002/004, CMP-GSL-001/002, SEC-GSL-001/003) |
| Config/secrets at rest → owner-only | 0o600/0o700, flock RMW | `config/base.rs`, `workspace/store.rs` | Held (OBS-GSL-001: missing dir fsync noted) |
| Subagent delegation → confined, bounded | working_dir confinement, turn caps | `delegate_config.rs`, `orchestrator.rs` | Partial (LLM-GSL-004, ARC-GSL-006) |

## Findings Table

45 findings + 4 observations. Patch Priority: **P1** = High (patch now), **P2** = Medium (guardrails next), **P3** = Low with code/workflow impact, **P4** = Low doc-only, **P5** = Info/observations. OBS rows are observations, not findings. Dedup/renumber decisions are documented in Appendix A.

| ID | Severity | Confidence | Evidence Basis | Domain | Title | Patch Priority | Blast Radius | Complexity | Cost | Nominal Agent |
|---|---|---|---|---|---|---|---|---|---|---|
| SEC-GSL-001 | High | Confirmed | source-evidenced (exfil chain simulation-reasoned) | Security | Saved website-login passwords substituted into any model-proposed tool call; no destination-origin check; contradicts docs | P1 | Cross-system | workflow_protocol | S | codex |
| SEC-GSL-201 | High | Confirmed | source-evidenced | Security | Renderer can self-grant read/write/delete on arbitrary directories incl. `~/.ssh`, `~/.aws` | P1 | Local | operator_ux | S | codex |
| SECN-GSL-201 | High | Confirmed | source-evidenced | Security (repo posture) | `pr-smoke-test.yml` exposes 10 org LLM/Databricks secrets to PR-head code; fork gate only | P1 | Cross-system | governance_decision | S | codex + human-owner |
| LLM-GSL-001 | Medium | Likely | simulation-reasoned | Security (LLM) | SmartApprove LLM judge fed full model args; name gate covers only known-dangerous names | P2 | Local | workflow_protocol | S | codex |
| LLM-GSL-004 | Medium | Likely | simulation-reasoned | Security (LLM) | `delegate` grant model-chosen and unbounded without role policy; Auto subagent; no depth cap | P2 | Local | workflow_protocol | M | codex + human-owner |
| IOP-GSL-001 | Medium | Confirmed (property) / Likely (accumulation) | source-evidenced | Input-Output-Path | MCP large responses spilled to `$TMPDIR/gosling_mcp_responses/`, never cleaned, no fsync | P2 | Local | persistence_recovery | XS | codex |
| IOP-GSL-005 | Medium | Confirmed | source-evidenced | Input-Output-Path (sec Security) | `computer_control` auto-installs Peekaboo from mutable brew tap on first model-proposed call | P2 | Local | external_service_semantics | XS–S | codex |
| DAT-GSL-004 | Medium | Confirmed | source-evidenced | Data-Integrity | `delete_session` omits `output_revisions`; output contents survive explicit deletion | P2 | Local | persistence_recovery | S | codex + human-owner |
| ARC-GSL-003 | Medium | Confirmed | source-evidenced | Architecture | Silent `.gitignore` append on output-folder creation; undisclosed, undocumented, duplicated with divergent checks, non-atomic RMW | P2 | Repo | operator_ux | S | other-agent (full-stack) |
| SECN-GSL-202 | Medium | Plausible | simulation-reasoned | Security (repo posture) | `docs-update-cli-ref.yml` runs LLM agent in CI with Anthropic key + persisted git creds + `contents:write`, fed repo-derived text | P2 | Cross-system | governance_decision | S | codex |
| SECN-GSL-203 | Medium | Confirmed | source-evidenced | Security (repo posture) | `create-version-bump-pr.yaml` passes 6 LLM keys into unpinned `cargo update --workspace` + `cargo run` | P2 | Cross-system | governance_decision | XS | codex |
| SECN-GSL-204 | Medium | Confirmed | source-evidenced | Security (repo posture) | Hourly cron clones unpinned `block/Agent-Skills` and publishes manifest + zips to production docs site | P2 | Cross-system | governance_decision | S | codex |
| REL-GSL-001 | Medium | Confirmed | source-evidenced | Reliability | OAuth token removal silently fails; `cleanup()` returns `Ok` unconditionally | P2 | Local | local_guardrail | XS | codex |
| SEC-GSL-002 | Low | Confirmed | source-evidenced | Security | Scanner warn logs embed 300-char tool previews + full arg JSON | P3 | Local | local_guardrail | XS | codex |
| SEC-GSL-003 | Low | Confirmed | source-evidenced | Security | ACP `/health` `/status` unauthenticated; doc claims token required | P3 | Service | local_guardrail | XS | codex |
| LLM-GSL-002 | Low | Confirmed | source-evidenced | Security (LLM) | Opt-in ML classifier POSTs full tool content + user messages, no redaction | P3 | Cross-system | local_guardrail | XS | codex |
| LLM-GSL-003 | Low | Confirmed | source-evidenced | Security (LLM) | Opt-in Langfuse/OTLP export full messages, no redaction or content-scope warning | P3 | Cross-system | local_guardrail | XS | codex |
| STT-GSL-001 | Low | Confirmed | source-evidenced | State-Transition | `update_handoff_status` has no from-status precondition | P3 | Local | local_guardrail | XS | codex |
| STT-GSL-002 | Low | Plausible | simulation-reasoned | State-Transition | Archive gate narrow; archived sessions mutable; CLI auto-unarchives on resume | P3 | Local | workflow_protocol | S | codex |
| CON-GSL-001 | Low | Likely | simulation-reasoned | Concurrency | `backendProcessRegistry.ts` cross-process RMW; in-process queue only | P3 | Local | cross_process_coordination | S | codex |
| DAT-GSL-002 | Low | Confirmed | source-evidenced | Data-Integrity | `output_revisions` path-keyed, no session FK; cross-session disclosure + version chaining (absorbs TMP-GSL-001) | P3 | Local | persistence_recovery | S | codex |
| IOP-GSL-002 | Low | Confirmed | source-evidenced | Input-Output-Path | `on_link_shell_library_file` accepts any absolute host path | P3 | Local | local_guardrail | XS | codex |
| IOP-GSL-003 | Low | Confirmed | source-evidenced | Input-Output-Path | Legacy `.jsonl` import silently drops malformed lines / truncates | P3 | Local | local_guardrail | XS | codex |
| IOP-GSL-004 | Low | Plausible | simulation-reasoned | Input-Output-Path | Office archive validation checks declared sizes only; unbounded decompression (zip-bomb DoS) | P3 | Local | workflow_protocol | S | codex |
| DAT-GSL-001 | Low | Confirmed (ordering) / Likely (phantom) | source-evidenced | Data-Integrity | Output-revision DB commit precedes file rename; phantom "latest" after crash | P3 | Local | persistence_recovery | S | codex |
| DAT-GSL-003 | Low | Confirmed | source-evidenced | Data-Integrity | Malformed workspace store silently resets to defaults (credential profiles dropped) | P3 | Local | local_guardrail | XS | codex |
| SEC-GSL-202 | Low | Confirmed | source-evidenced | Security (repo posture) | `quarantine.yml` `pull_request_target` shell interpolation of PR login (latent; charset-constrained) | P3 | Cross-system | local_guardrail | XS | codex |
| SECN-GSL-205 | Low | Confirmed | source-evidenced | Security (repo posture) | `pr-website-preview.yml` unpinned `wrangler@latest` + CF/INKEEP secrets in PR-head env | P3 | Cross-system | local_guardrail | XS | codex |
| SEC-GSL-203 | Low | Confirmed | source-evidenced | Security (repo posture) | `release.yml` `secrets:inherit` + `id-token:write` on branch runs | P3 | Cross-system | governance_decision | XS–S | codex |
| SEC-GSL-204 | Low | Confirmed | source-evidenced | Security (repo posture) | `update-release-pr.yaml` executes PR-head repo code with `pull-requests:write` token, bot-author gate | P3 | Cross-system | governance_decision | XS | codex |
| ARC-GSL-001 | Low | Confirmed | source-evidenced | Architecture | `architecture.md` module table names 3 nonexistent modules, omits 2 | P4 | Repo | local_guardrail | XS | claude |
| ARC-GSL-002 | Low | Confirmed | source-evidenced | Architecture | README claims 1.4.0 current/verified; source is 1.5.0; release notes stop at v1.4.0 | P4 | Repo | local_guardrail | XS | claude |
| ARC-GSL-005 | Low | Confirmed | source-evidenced | Architecture (sec Negative-Space) | `ToolPermissionStore` dead code (zero callers); `save()` no fsync, no 0o600 (absorbs NEG-GSL-002) | P3 | Local | local_guardrail | XS | codex (human-owner for pub-API removal) |
| STT-GSL-003 | Low | Confirmed | source-evidenced | State-Transition | Migration v24 silently clears `restrict_tools_to_working_dirs` for all workspace sessions | P3 | Local | persistence_recovery | XS–S | codex + human-owner |
| ARC-GSL-006 | Low | Confirmed | source-evidenced | Architecture | Orchestrator `start_agent` accepts any absolute `working_dir` + Auto; `view_session`/`send_message` any session | P3 | Local | workflow_protocol | S | codex |
| CMP-GSL-001 | Low | Confirmed | source-evidenced | Compliance-Posture | `.giles/` + `governance/` metadata stale ~2.5 months (91 open findings, crashed flag, foreign repo_path) | P4 | Repo | governance_decision | XS–M | human-owner |
| CMP-GSL-002 | Low | Confirmed | source-evidenced | Compliance-Posture | Tagteam plan doc still says implemented/in-progress though feature removed 2026-08-27 | P4 | Repo | local_guardrail | XS | codex |
| WFG-GSL-001 | Low | Likely | simulation-reasoned | Workflow-GUI | DiffViewer header/footer untruncated; overflow on narrow terminals | P3 | Local | local_guardrail | XS | codex |
| WFG-GSL-002 | Low | Likely | simulation-reasoned | Workflow-GUI | PermissionPrompt height budget under-reserves with many options/rawInput | P3 | Local | local_guardrail | XS–S | codex |
| WFG-GSL-003 | Low | Plausible | simulation-reasoned | Workflow-GUI | Message edit truncates backend before UI sync; lost response = permanent divergence | P3 | Local | workflow_protocol | S | codex |
| FSR-GSL-001 | Low | Confirmed | source-evidenced | Failsafe (sec Security) | Extension tempdirs with keyring secrets + inline-Python source cleaned only on graceful shutdown; no sweep | P3 | Local | persistence_recovery | S | codex |
| FSR-GSL-002 | Low | Confirmed | source-evidenced | Failsafe (sec Reliability) | Plugin staging sweep never removes on non-unix; unix removal failures silent | P3 | Local | local_guardrail | S | codex |
| FSR-GSL-003 | Low | Confirmed (property) / Likely (OOM) | source-evidenced | Failsafe (sec Reliability) | ui/text markdown `parseCache` unbounded module-level Map | P3 | Local | local_guardrail | XS | codex |
| ARC-GSL-004 | Info | Confirmed | source-evidenced | Architecture | sessions.db schema v38 vs docs cite v36; v38 undocumented, no ADR | P5 | Repo | local_guardrail | XS | claude |
| NEG-GSL-001 | Info | Confirmed | source-evidenced | Negative-Space | ui/text `App.initialPrompt` dead prop; four dependent branches unreachable | P5 | Local | local_guardrail | XS | codex |
| OBS-GSL-001 | Obs | Confirmed | source-evidenced | Reliability | `write_file_atomic` + registry `writeRecords`: no dir fsync after rename | P5 | Local | local_guardrail | XS | codex |
| OBS-GSL-002 | Obs | Confirmed | source-evidenced | Reliability | `findAvailablePort` listen-0/close/reuse TOCTOU | P5 | Local | local_guardrail | XS | codex |
| OBS-GSL-003 | Obs | Confirmed | source-evidenced | Reliability | `delete_session` removes DB rows before in-memory agent is stopped | P5 | Local | workflow_protocol | XS | codex |
| OBS-GSL-004 | Obs | Confirmed | source-evidenced | Security (policy) | stop/pre/post-tool hooks bypass `interaction_policy` check | P5 | Local | workflow_protocol | S | codex |

## Detailed Findings

Full `finding_format.md` schema for High and Medium findings; compact same-fields blocks for Low/Info/Obs.

### SEC-GSL-001: Saved website-login passwords substituted into any model-proposed tool call; no destination-origin check

Severity: High
Confidence: Confirmed (code property source-evidenced; end-to-end exfiltration chain simulation-reasoned, hence Likely as a runtime outcome)
Evidence basis: source-evidenced (chain: simulation-reasoned)
Domain: Security

Evidence:
- `crates/gosling/src/website_logins.rs:301-339` — `insert_passwords` substitutes saved passwords into tool calls; no destination-origin comparison anywhere in the substitution path
- `crates/gosling/src/agents/agent/tool_dispatch.rs:440,529-550` — the only gate on dispatch is `DispatchOrigin::ModelNative`, not destination
- `crates/gosling/src/permission/website_login_inspector.rs:44-58` — approval prompt shows the login's own URL, not the destination URL of the call; `:104` — auto-downgrades `RequireApproval`, i.e. silent in Auto mode
- `documentation/docs/guides/website-logins.md:37-44` — doc claims passwords are only filled for the matching site
- `README.md:70-74` — README hardening claim about website logins

Observed behavior:
- Any model-proposed tool call that references a bound website login receives the saved password via substitution regardless of the call's destination (URL, host, command, or file path).
- The permission prompt for such calls displays the login's registered URL rather than the actual destination, so the operator approves against the wrong target.
- In Auto mode the website-login inspector downgrades approval requirements, making substitution silent.

Expected boundary:
- Per the docs, a saved password is substituted only when the tool call's destination origin matches the login's origin; the approval prompt shows the real destination.

Failure mechanism:
- `insert_passwords` performs template substitution without resolving or comparing the call's destination origin; dispatch gating checks only who proposed the call (model vs user), not where it points; the inspector renders the login's URL instead of the destination.

Break-it angle:
- Indirect prompt injection: content the model reads (web page, file, MCP response, doc) instructs the model to invoke a shell/HTTP/file tool referencing the bound login with an attacker-controlled destination. The password is substituted and exfiltrated. Silent in Auto mode; otherwise log-only. Malformed/duplicate/stale variants do not weaken the mechanism — it is unconditional.

Impact:
- Exfiltration of saved website credentials (usernames/passwords) to attacker-controlled destinations — cross-system, irreversible.
- Operator deception at approval time: the prompt shows a benign, familiar URL.

Operational impact:
- Blast radius: Cross-system
- Side-effect class: network (credential exfiltration); user-visible deception
- Reversibility: irreversible
- Operator visibility: log-only (silent in Auto)
- Rerun safety: unsafe (each rerun can re-exfiltrate)

Adjacent failure modes:
- SEC-GSL-002 (scanner warn logs embed tool-call args — can capture substituted passwords)
- LLM-GSL-001 (SmartApprove judge sees full model args, can wave substitution calls through)
- SEC-GSL-003 (docs-vs-code contradiction pattern)

Recommended mitigation:
- Remediation patterns: destination-binding for credential substitution; display-destination-at-approval.
- Minimal repair: in `insert_passwords`, resolve the tool call's destination origin and require equality with the login's origin before substituting; on mismatch, refuse substitution and require explicit approval showing the destination.
- Local guardrail: `website_login_inspector` renders the actual destination origin of the call; never auto-approve substitutions in Auto without a destination match.
- Behavior test: model-proposed call referencing a login with a mismatched destination → no substitution, prompt shows destination; matched destination → substitution proceeds.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: S
- Cost drivers: `website_logins.rs`, `tool_dispatch.rs`, inspector, tests, doc alignment
- Nominal implementation agent: codex
- Rationale: cross-cutting but well-specified change across substitution, dispatch, and inspector with clear static test hooks.

Validation:
- Unit test: mismatched destination → substitution refused.
- Integration test: inspector prompt contains the destination URL, not only the login URL.
- Doc claim (`website-logins.md:37-44`) updated to match implemented behavior.

Non-goals:
- Redesigning login storage format or encryption at rest.
- Reordering the global inspector chain.

### SEC-GSL-201: Renderer can self-grant read/write/delete access to arbitrary host directories

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `ui/desktop/src/main/rendererIpc.ts:157-188` — `grantSessionDirectories` handler; comment claims guards keep a bad caller from broad grants
- `ui/desktop/src/utils/rendererDirectoryGrants.ts:57-70` — `isOverlyBroadRoot` rejects only the filesystem root and paths containing the home dir as a whole
- `ui/desktop/src/preload.ts:237-238` — exposed on `window.electron`
- `ui/desktop/src/main/fileIpc.ts:258-260,537-539,548-550` — `readFile`/`writeFile`/`deleteFile` enforce `assertRendererFileAccess`; granted roots subsume read+write+delete
- `ui/desktop/src/components/BaseChat.tsx:253` — legitimate renderer caller

Observed behavior:
- Any code running in the renderer can call `grantSessionDirectories` with arbitrary absolute paths. `isOverlyBroadRoot` rejects only `/` and the home directory itself, so `~/.ssh`, `~/.aws`, `~/.gnupg`, `/etc` all pass. The resulting grant authorizes read, write, and delete through `fileIpc.ts`.

Expected boundary:
- Directory grants beyond the workspace require user mediation (native directory picker or explicit per-path confirmation); sensitive locations are denied by policy. The renderer is a rendering context, not a trust principal for filesystem scope.

Failure mechanism:
- The IPC handler trusts the renderer as caller and applies only a path-shape check. Nothing binds the grant to a user gesture or a user-visible confirmation, and the sensitive-path list is absent.

Break-it angle:
- Renderer compromise (markdown/XSS sink — grep-sampled only, escalated as a stub; or a malicious dependency) calls `grantSessionDirectories(['~/.ssh', '~/.aws'])` → reads keys, writes `authorized_keys` or cloud credentials, exfiltrates. Silent; no user prompt; no log beyond normal IPC traffic. Concurrent/duplicate grants accumulate. Direct renderer console access is the "malformed/direct bypass" variant.

Impact:
- Arbitrary host file read/write/delete from the renderer context, including credential stores (`~/.ssh`, `~/.aws`, browser credential stores readable by the user).

Operational impact:
- Blast radius: Local
- Side-effect class: file
- Reversibility: irreversible for exfiltration; compensatable for writes
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- rel stub: `grantSessionDirectories` failure swallowed at `BaseChat.tsx:253` (`.catch(() => {})`)
- MCP-app proxy guest-content sandboxing is the inverse surface (stub routed to renderer lens)
- SEC-GSL-001 compounds impact (stolen passwords flow into tool calls)

Recommended mitigation:
- Remediation patterns: user-mediation for privilege grant; sensitive-path deny-by-default.
- Minimal repair: route grants through a user-visible confirmation (or native directory picker) in main; extend `isOverlyBroadRoot` with a sensitive-directory list (`~/.ssh`, `~/.aws`, `~/.gnupg`, `/etc`, `/usr`, system credential stores).
- Local guardrail: deny-by-default for paths outside the active workspace and previously user-confirmed roots.
- Behavior test: renderer call granting `~/.ssh` → denied or requires confirmed user gesture; grant of a workspace subdir → allowed without friction.

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Cost drivers: `rendererIpc.ts`, `rendererDirectoryGrants.ts`, `fileIpc.ts`, confirmation UI, tests
- Nominal implementation agent: codex
- Rationale: contained to Electron main/preload/renderer with a clear user-mediation UX pattern.

Validation:
- Unit tests: `isOverlyBroadRoot` rejects `~/.ssh`, `~/.aws`, `/etc`.
- Integration test: grant of a sensitive path requires a user confirmation round-trip.
- Sweep: no other IPC handler exposes filesystem scope to the renderer.

Non-goals:
- Full renderer XSS hunt (separate renderer-lens pass with Playwright — proposed, not performed).
- Redesigning `fileIpc.ts` authorization model.

### SECN-GSL-201: `pr-smoke-test.yml` exposes 10 org LLM/Databricks secrets to PR-head code execution

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security (repo posture)

Evidence:
- `.github/workflows/pr-smoke-test.yml:1-8` — triggers on `pull_request` targeting `main`
- `:25` — fork gate only (same-repo PRs pass)
- `:38,143-155,161,187` — `ref` set to the PR merge ref; 10 provider secrets placed in env; job runs test scripts and a build of PR-head code

Observed behavior:
- Any same-repo pull request executes its head code in a job whose environment contains 10 org LLM API keys and Databricks credentials. Fork PRs are gated; same-repo PRs (any branch in this repo, i.e. any collaborator or compromised collaborator account) are not.

Expected boundary:
- Secrets are never available to jobs that execute PR-head code. Live smoke tests run on `push` to `main`, or behind an environment/label approval that precedes checkout of untrusted code.

Failure mechanism:
- `pull_request` trigger + `secrets` in env + checkout of the PR merge ref; the only gate is the fork check at :25. No environment protection, no label gate, no allowlist of actors.

Break-it angle:
- A malicious or compromised collaborator pushes a PR whose test step exfiltrates the 10 secrets (print env, curl to attacker host). Cross-system blast: org LLM keys (spend/abuse) and Databricks credentials. Direct-API variant: none needed — the workflow itself is the bypass. Concurrent variant: parallel PRs all receive the secrets.

Impact:
- Org secret exposure: 10 LLM provider keys + Databricks credentials readable by arbitrary same-repo PR code.

Operational impact:
- Blast radius: Cross-system
- Side-effect class: external API / secret exposure
- Reversibility: irreversible once leaked
- Operator visibility: silent (may appear in CI logs)
- Rerun safety: unsafe

Adjacent failure modes:
- SECN-GSL-202, SECN-GSL-203, SECN-GSL-204, SECN-GSL-205, SEC-GSL-203, SEC-GSL-204 — same repo-posture family.

Recommended mitigation:
- Remediation patterns: trusted-workflow gate (environment protection or label gate before checkout); secrets on push-only jobs.
- Minimal repair: require an environment approval or a `ci-ok` label before the secret-bearing job runs, or move live smoke tests to `push` on `main` (pattern already present in `pr-comment-build-cli.yml`).
- Local guardrail: no `secrets.*` in any job that checks out a `pull_request` merge ref.
- Behavior test: same-repo PR without approval → secret-bearing job skipped; fork PR → skipped.
- Immediate (independent of code patch): **rotate the 10 exposed secrets** — exposure window is unbounded while the workflow is unchanged.

Implementation assessment:
- Complexity: governance_decision
- Cost: S
- Cost drivers: workflow yaml, GitHub-side environment/label setup, secret rotation, runbook
- Nominal implementation agent: codex (workflow edit) + human-owner (GitHub environment configuration + rotation authority)
- Rationale: half workflow code, half GitHub-side configuration and rotation decisions that require owner authority.

Validation:
- Dry-run a same-repo PR; verify the secret-bearing job does not run without approval.
- GitHub-side: confirm the 10 secrets are rotated and old values revoked (needs API/console access — Not Reviewed here).

Non-goals:
- Auditing branch protection / environments / secret inventory (needs GitHub API access — listed in Deferred Risks).
- Redesigning the smoke-test suite itself.

### LLM-GSL-001: SmartApprove LLM judge is fed full model-generated arguments; name gate covers only known-dangerous names

Severity: Medium
Confidence: Likely (structural, not traced end-to-end)
Evidence basis: simulation-reasoned
Domain: Security (LLM)

Evidence:
- `crates/gosling/src/permission/permission_judge.rs:92-133` — `create_check_messages` serializes the full model-generated arguments into the judge prompt; `:154-193` judge flow
- `crates/gosling/src/permission/permission_inspector.rs:200-224` — `can_auto_approve` → `Allow`; `:217-219` comment cites SEC-GOS-013 / LLM-GSL-006
- `crates/gosling/src/agents/agent/tool_class.rs:118-124` — `has_recognized_side_effects` name allowlist gates only known-dangerous tool names

Observed behavior:
- In SmartApprove mode, auto-approval delegates to an LLM judge whose input includes the full model-generated arguments. The pre-gate (`has_recognized_side_effects`) only recognizes a fixed allowlist of dangerous tool names; any side-effecting call with an unrecognized name reaches the judge, and the judge can be steered by the arguments themselves (which are model-controlled, hence attacker-influenceable via prompt injection).

Expected boundary:
- Auto-approval decisions are made on sanitized/summarized call metadata, or the judge is not fed raw attacker-influenceable content; recognized-side-effects gating is not the only defense.

Failure mechanism:
- The judge prompt embeds raw model output (tool name + full argument JSON). Prompt injection in conversation content can shape those arguments to argue for approval; the name allowlist is a denylist-by-omission.

Break-it angle:
- Injection payload in a file/web page instructs the model to call an unrecognized-name tool (e.g. a shell wrapper with a benign-looking name) with arguments that include text like "operator pre-approved this". Judge sees convincing context → auto-approves. Concurrent variant: multiple calls in one turn each judged independently.

Impact:
- Approval bypass for side-effecting calls in SmartApprove/Auto modes; attacker-influenced auto-approval.

Operational impact:
- Blast radius: Local
- Side-effect class: process/file/network per tool
- Reversibility: compensatable to irreversible depending on tool
- Operator visibility: log-only
- Rerun safety: unsafe

Adjacent failure modes:
- SEC-GSL-001 (password substitution calls flow through the same judge), LLM-GSL-004 (delegation grants), SEC-GSL-002 (judge inputs may also land in logs)

Recommended mitigation:
- Remediation patterns: untrusted-input isolation for judges; defense-in-depth gating.
- Minimal repair: feed the judge a structured summary (tool name, destination, side-effect class) rather than raw argument JSON; treat model-generated text in arguments as untrusted and quote/escape it.
- Local guardrail: auto-approval requires both name-gate pass AND judge pass on sanitized input; never judge raw args.
- Behavior test: call with injection-flavored arguments and unrecognized name → not auto-approved.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: S
- Cost drivers: `permission_judge.rs`, `permission_inspector.rs`, `tool_class.rs`, tests
- Nominal implementation agent: codex
- Rationale: contained to the permission pipeline with clear prompt-construction hooks.

Validation:
- Regression test: judge receives summarized/sanitized input; injection-flavored args do not flip the verdict.
- Static assertion: judge prompt template contains no raw argument interpolation.

Non-goals:
- Replacing the LLM judge with a rules engine (product decision).
- Changing SmartApprove's user-facing semantics.

### LLM-GSL-004: `delegate` grant is model-chosen and unbounded without role policy; subagent runs in Auto; no delegation-depth cap

Severity: Medium
Confidence: Likely (structural, not traced)
Evidence basis: simulation-reasoned
Domain: Security (LLM)

Evidence:
- `crates/gosling/src/platform_extensions/summon/source_discovery.rs:69-130` — comment admits "the *model* chose it"; warns `delegate_extensions_unbounded`; refs LLM-GSL-010
- `crates/gosling/src/platform_extensions/summon/delegate_config.rs:37-64` — `delegate_mode` → Auto; `:55-63` parent must be Auto
- `crates/gosling/src/agents/subagent_task_config.rs:9-10` — 25-turn cap per subagent

Observed behavior:
- When no role policy constrains it, which extension(s) a delegate subagent may use is chosen by the model, and the grant is unbounded. The subagent runs in Autonomous (Auto) mode — no per-call approval — and there is no cap on delegation depth.

Expected boundary:
- Delegation scope (extensions, depth, mode) is policy-bounded: role policy or operator config defines the grant; Auto mode for subagents is explicit and depth-limited.

Failure mechanism:
- Source discovery leaves extension selection to the model; delegate config defaults the subagent to Auto; nothing counts delegation depth.

Break-it angle:
- Injection content steers the model to delegate with a maximally-scoped grant, chaining Auto subagents (depth unbounded) that inherit broad tool access without further approval. Stale variant: a previously-approved narrow role silently widened by a model-chosen grant.

Impact:
- Privilege amplification inside a session: model-chosen, unbounded, unattended (Auto) execution scope.

Operational impact:
- Blast radius: Local
- Side-effect class: process/file/network per granted extension
- Reversibility: compensatable to irreversible depending on tools
- Operator visibility: log-only
- Rerun safety: unsafe

Adjacent failure modes:
- LLM-GSL-001 (Auto approval inside subagents), ARC-GSL-006 (orchestrator surface), IOP-GSL-005 (Auto + model-proposed install)

Recommended mitigation:
- Remediation patterns: policy-bounded delegation; explicit Auto opt-in; depth cap.
- Minimal repair: require a role policy (or explicit operator setting) for delegate extension grants; cap delegation depth (e.g. 1–2); require non-Auto mode or scoped approval for subagents by default.
- Local guardrail: warn + downgrade when a delegate grant exceeds the parent's own effective scope.
- Behavior test: no role policy → delegate grant denied or clamped to parent scope; depth > cap → refused.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: `source_discovery.rs`, `delegate_config.rs`, `subagent_task_config.rs`, policy plumbing, tests, docs
- Nominal implementation agent: codex + human-owner (Auto-by-default semantics are a product policy decision)
- Rationale: touches delegation policy and product-default semantics; owner sign-off needed on defaults.

Validation:
- Regression tests for grant clamping, depth cap, and mode default.
- Doc update for delegate semantics.

Non-goals:
- Redesigning the summon/delegate extension architecture.
- Removing model-chosen discovery entirely (product decision).

### IOP-GSL-001: MCP large responses spilled to `$TMPDIR/gosling_mcp_responses/`, never cleaned, no fsync

Severity: Medium
Confidence: Confirmed (write property source-evidenced); Likely (accumulation/secret residue as runtime outcome)
Evidence basis: source-evidenced
Domain: Input-Output-Path

Evidence:
- `crates/gosling/src/agents/large_response_handler.rs:122-137` — spill path `$TMPDIR/gosling_mcp_responses/mcp_response_<ts>.txt`; direct write, no `O_EXCL`, no fsync
- `crates/gosling/src/agents/large_response_handler.rs:163-171` — directory handling; `remove_file` appears only in `#[cfg(test)]` code
- Contrast: `plugins/mod.rs:547-577` shows the repo's own staging-sweep pattern that is absent here

Observed behavior:
- Oversized MCP tool responses are written to temp files under `$TMPDIR/gosling_mcp_responses/`. Nothing ever deletes them (removal exists only in tests). Writes are direct (no temp+fsync+rename), so a crash mid-write leaves a partial file.

Expected boundary:
- Spill files are scoped to the turn/session that created them, removed when the turn completes (or on next startup sweep), written atomically, and created owner-only.

Failure mechanism:
- No cleanup ownership: the handler creates files but no code path removes them outside tests; no sweep exists (grep-verified).

Break-it angle:
- Long-running sessions accumulate response files in `$TMPDIR` (disk growth; multi-worker/shared-host `/tmp` residue). Tool responses can contain secrets/credentials from tool output → residue at rest in a shared temp dir. Concurrent turns writing same-named files (timestamp-keyed) can interleave. Partial writes on crash are never reconciled.

Impact:
- Unbounded temp-disk growth; potential secret residue in shared temp directories; no recovery path for partial writes.

Operational impact:
- Blast radius: Local
- Side-effect class: file
- Reversibility: compensatable (delete files)
- Operator visibility: silent
- Rerun safety: safe (but accumulation worsens)

Adjacent failure modes:
- FSR-GSL-001 (same temp-residue family: extension tempdirs), FSR-GSL-002 (sweep pattern missing), OBS-GSL-001 (no dir fsync family)

Recommended mitigation:
- Remediation patterns: resource-lifecycle ownership; startup sweep; atomic write.
- Minimal repair: delete the spill file when the turn/session that created it completes; add a startup sweep of `gosling_mcp_responses/` older than N hours; write via temp+fsync+rename with 0o600.
- Local guardrail: cap per-session spill count/size.
- Behavior test: after turn completion the spill file is gone; after simulated crash, startup sweep removes stale files.

Implementation assessment:
- Complexity: persistence_recovery
- Cost: XS
- Cost drivers: `large_response_handler.rs`, lifecycle hook point, tests
- Nominal implementation agent: codex
- Rationale: small, contained change with an existing in-repo sweep pattern to mirror.

Validation:
- Test: turn completes → spill file removed; startup sweep removes orphaned files.
- Static check: `remove_file`/sweep referenced outside `#[cfg(test)]`.

Non-goals:
- Changing the spill-to-disk design itself (memory caps are a separate decision).

### IOP-GSL-005: `computer_control` auto-installs Peekaboo from a mutable Homebrew tap on first model-proposed call

Severity: Medium
Confidence: Confirmed (install path source-evidenced; actual install behavior Likely end-to-end — macOS-only, extension disabled by default)
Evidence basis: source-evidenced
Domain: Input-Output-Path (secondary Security)

Evidence:
- `crates/gosling-mcp/src/peekaboo/mod.rs:7` — `BREW_FORMULA` constant; `:47-56` — `auto_install_peekaboo` runs `brew install`
- `crates/gosling-mcp/src/computercontroller/mod.rs:1042` — doc comment; `:1061-1069` — tool definition with `destructive_hint` + `open_world_hint`; `:1149-1177` — `ensure_peekaboo` → auto-install; `:1211-1215` — `peekaboo_impl` calls `ensure_peekaboo`
- `built-in-extensions.json:12-15`, `bundled-extensions.json:14-18` — `enabled: false` (opt-in)
- `crates/gosling/src/agents/agent/tool_class.rs:29-32` — code-execution class; `lib.rs:27-29,66-71` — macOS cfg, builtin

Observed behavior:
- The first model-proposed `computer_control` call triggers `ensure_peekaboo`, which runs `brew install` of the Peekaboo formula from the mutable third-party tap `steipete/tap/peekaboo`. No version pin, no checksum, no distinct user consent for the install; in Auto mode there is no prompt at all.

Expected boundary:
- Installing third-party software is a distinct, user-consented, pinned action (version + checksum), never triggered implicitly by a model tool call.

Failure mechanism:
- The extension treats a missing binary as a self-healing condition and installs from a mutable upstream source at first use, inside the tool-call path.

Break-it angle:
- A compromised or malicious tap update ships a trojaned Peekaboo; the next model-proposed screenshot/click installs and runs it with the user's privileges. Injection angle: content steers the model to call `computer_control` early, triggering the install. No pin/checksum means the supply chain is live-mutable. Auto mode = no prompt.

Impact:
- Arbitrary third-party code installation and execution on the host from a mutable source, triggered by model behavior.

Operational impact:
- Blast radius: Local
- Side-effect class: process + network + file
- Reversibility: compensatable
- Operator visibility: log-only
- Rerun safety: safe

Adjacent failure modes:
- IOP-GSL-001 (model-proposed actions with host side effects), LLM-GSL-004 (Auto amplification), SECN-GSL-204 (same mutable-upstream pattern in CI)

Recommended mitigation:
- Remediation patterns: pinned supply chain; explicit consent for installs.
- Minimal repair: pin formula version + verify checksum after install (or bundle the binary); require explicit user consent before any `brew install`; on missing binary, fail with instructions instead of auto-installing.
- Local guardrail: never install from a mutable tap inside a tool-call path; log and refuse.
- Behavior test: missing binary → no `brew install` without user consent; consented install → pinned version verified.

Implementation assessment:
- Complexity: external_service_semantics
- Cost: XS–S
- Cost drivers: `peekaboo/mod.rs`, `computercontroller/mod.rs`, consent UX, tests (macOS-gated)
- Nominal implementation agent: codex
- Rationale: small surface, but consent UX touches operator flow.

Validation:
- Test (macOS CI or mocked brew): first call does not invoke `brew install` absent consent.
- Static check: no `brew install` on the tool-call path.

Non-goals:
- Auditing Peekaboo itself (third-party project).
- Redesigning the computercontroller extension set.

### DAT-GSL-004: `delete_session` omits `output_revisions`; output contents survive explicit session deletion

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Data-Integrity

Evidence:
- `crates/gosling/src/session/session_manager/session_crud.rs:583-627` — `delete_session` deletes session rows but never touches `output_revisions`
- grep: no `DELETE FROM output_revisions` anywhere in the codebase; no purge ACP method (`custom_dispatch.rs:760-787`); no secure-delete/VACUUM under `session/`
- `crates/gosling/src/session/output_revisions_storage.rs:42-49` — table is path-keyed with content BLOBs; `session/output_revisions.rs:41-61` — `is_output_document` captures full file bytes; caps 8 MiB × 1000 revisions/path
- Contrast: `session_artifacts` IS cascaded (schema FK `ON DELETE CASCADE`, `pool_lifecycle.rs:85`)

Observed behavior:
- Deleting a session removes its metadata and cascades most child tables, but `output_revisions` rows — including full output-document content BLOBs — survive. Deletion reports success silently.

Expected boundary:
- Explicit session deletion removes all session-owned data, or the retention is documented and user-visible (with a purge path).

Failure mechanism:
- `delete_session` simply omits the `output_revisions` table; the table has no session FK (`DAT-GSL-002`), so even a cascade would not have reached it.

Break-it angle:
- User deletes a session believing its outputs (which may contain secrets the model wrote to files: tokens, keys, personal data) are gone; the content remains in `sessions.db` (+ WAL + any backups) indefinitely. Direct-API variant: `session/delete` ACP request exhibits the same behavior. Stale variant: restored/re-imported sessions resurrect old revisions for the same paths (`DAT-GSL-002` chaining).

Impact:
- Deletion-intent violation: sensitive output content persists at rest after explicit deletion; silent false success.

Operational impact:
- Blast radius: Local (sessions.db + WAL + backups)
- Side-effect class: DB
- Reversibility: irreversible in practice (rows never removed)
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- DAT-GSL-002 (path-keyed table enables cross-session chaining), TMP-GSL-001 (merged into DAT-GSL-002), DAT-GSL-001 (ordering), OBS-GSL-003 (delete ordering)

Recommended mitigation:
- Remediation patterns: deletion completeness; documented retention with purge path.
- Minimal repair: delete `output_revisions` rows for the session's registered paths in `delete_session` (requires path scoping — see DAT-GSL-002 for the FK gap); add a `secure_delete`/VACUUM option; surface retention in the delete confirmation UI.
- Local guardrail: post-delete assertion that no rows reference the deleted session's paths; log count of retained rows if retention is chosen.
- Behavior test: delete session → `SELECT COUNT(*) FROM output_revisions` for its paths == 0 (or documented retention with purge).

Implementation assessment:
- Complexity: persistence_recovery
- Cost: S
- Cost drivers: `session_crud.rs`, `output_revisions_storage.rs`, migration/FK decision (DAT-GSL-002), UI copy, tests
- Nominal implementation agent: codex + human-owner (delete-vs-retention is a product/policy sign-off)
- Rationale: small code change but the retention semantics need owner decision; interacts with the path-keyed design finding.

Validation:
- Integration test: create outputs → delete session → assert revision rows and content gone (or retention documented + purge works).
- Migration test if a backfill purge is added.

Non-goals:
- Redesigning `output_revisions` keying in this slice (tracked as DAT-GSL-002).
- Secure-deletion of the whole DB file.

### ARC-GSL-003: Silent `.gitignore` append on output-folder creation; undisclosed, undocumented, duplicated with divergent checks, non-atomic RMW

Severity: Medium
Confidence: Confirmed (code property); Plausible (RMW race and ignore-check divergence on specific fixtures)
Evidence basis: source-evidenced
Domain: Architecture (INV-010 tension; cross-layer duplication)

Evidence:
- `crates/gosling/src/workspace/service.rs:897-935` — `ensure_gitignored_if_in_repo`: doc comment, `:920` `read_to_string`, `:930` `fs::write` (non-atomic), `:931-935` `warn!` only on failure, `:906-910` ignore-crate check
- `crates/gosling/src/workspace/service.rs:475-498` — `create_output_folder` calls it at `:496`; `:1569-1587` — test asserts `.gitignore == "/outputs/\n"`
- `ui/desktop/src/main/settingsIpc.ts:21-24` (mirror comment), `:41` (`writeFile`), `:110/:169` (call sites); `settingsIpc.test.ts:72-109`
- `ui/desktop/src/main/gitIpc.ts:89-97` — `git check-ignore` with divergent semantics
- `ui/desktop/src/components/WorkspaceEditorDialog.tsx:1096-1100`, `ArtifactRouterContext.tsx:191-197` — confirmation copy omits the `.gitignore` mutation
- `docs/INTENT.md:51` (INV-010); `documentation/docs/guides/using-goslingignore.md:33-40` (read-side only)

Observed behavior:
- Creating a workspace output folder silently appends an entry to the user's `.gitignore`. The UI confirmation does not disclose this; INTENT/docs do not document it; the ignore logic is duplicated in Rust and Electron main with divergent checks; the Rust write is a non-atomic read-modify-write with warn-only failure.

Expected boundary:
- Mutating a user's VCS config is either disclosed and confirmed, or not done; a single canonical implementation decides ignore behavior; writes are atomic.

Failure mechanism:
- Side effect hidden inside a create-folder operation; two implementations drift (ignore-crate semantics vs `git check-ignore`); `fs::write` RMW can lose concurrent edits and is not fsynced.

Break-it angle:
- Concurrent folder creations (two sessions, or desktop + CLI) race the RMW → lost `.gitignore` entries or duplicated lines. Divergent checks: a path one implementation ignores and the other doesn't → outputs committed to git (secret/artifact leak into VCS) or over-ignoring. Operator surprise: unrequested VCS config mutation. Downstream misuse: `.gitignore` used as an authority signal by other tooling.

Impact:
- Undisclosed mutation of user VCS config; potential VCS leakage of outputs or lost user entries; cross-layer drift.

Operational impact:
- Blast radius: Repo
- Side-effect class: file
- Reversibility: compensatable
- Operator visibility: log-only
- Rerun safety: safe

Adjacent failure modes:
- OBS-GSL-001 (no dir fsync after write), DAT-GSL-003 (silent store reset family — silent config mutation), ARC-GSL-001 (doc drift)

Recommended mitigation:
- Remediation patterns: disclose side effects at confirmation; single-source ignore logic; atomic RMW.
- Minimal repair: add the `.gitignore` mutation to the confirmation copy; implement one canonical ignore check shared by Rust and TS (or a single IPC call from renderer to the Rust service); write via temp+fsync+rename.
- Local guardrail: fail-visible (not `warn!`) when the append fails; dedupe before append.
- Behavior test: confirmation copy mentions `.gitignore`; concurrent creates → no lost/duplicated entries; both layers agree on a fixture set.

Implementation assessment:
- Complexity: operator_ux
- Cost: S
- Cost drivers: `workspace/service.rs`, `settingsIpc.ts`, `gitIpc.ts`, two UI components, INTENT/docs, tests
- Nominal implementation agent: other-agent (full-stack: Rust + Electron + UI copy)
- Rationale: spans Rust service, Electron main, and renderer copy — needs one owner across the stack.

Validation:
- UI copy test; concurrency test for the RMW; parity test between Rust and TS ignore checks.

Non-goals:
- Removing the auto-ignore feature (product decision).
- Reworking goslingignore semantics.

### SECN-GSL-202: `docs-update-cli-ref.yml` runs an LLM agent in CI with an Anthropic key, persisted git credentials, and `contents:write`, fed repo-derived text

Severity: Medium
Confidence: Plausible (simulation-reasoned over the workflow and prompt pipeline)
Evidence basis: simulation-reasoned
Domain: Security (repo posture)

Evidence:
- `.github/workflows/docs-update-cli-ref.yml:9-16,38-43,47,195-196,213` — LLM agent step with `ANTHROPIC_API_KEY`, git credential persisted (no `persist-credentials: false`), `contents:write` + `pull-requests:write`
- `documentation/automation/cli-command-tracking/prompts/update-cli-commands.system.md` — agent prompt; inputs derive from merged PR content (CLI help text, repo-derived text)

Observed behavior:
- A workflow runs a gosling LLM agent in CI with an Anthropic API key in env, a persisted git credential, and write permission on repo contents and PRs. The agent is fed text derived from the repository (including content authored in merged PRs).

Expected boundary:
- CI agents with write tokens and API keys are not fed untrusted/attacker-influenceable text; credentials are scoped (contents:write only where needed, `persist-credentials: false` where possible); injection channel is treated as hostile.

Failure mechanism:
- PR-authored text (commit messages, CLI help strings, file contents) flows into the agent's prompt; the agent has a key + token + write permissions; a prompt-injection payload in merged content could steer the agent to commit/push attacker-controlled changes or exfiltrate the key.

Break-it angle:
- Injection via a merged PR's CLI help text or doc snippet → agent writes attacker-controlled content to `main` (or a branch) with `contents:write`; `ANTHROPIC_API_KEY` exfiltration via the same channel. Downstream: repo content becomes authority for docs and possibly other automation (cascade).

Impact:
- Prompt-injection-to-write-channel in CI: poisoned commits, secret exposure.

Operational impact:
- Blast radius: Cross-system
- Side-effect class: external API / VCS write
- Reversibility: compensatable (revert) but trust damage persists
- Operator visibility: log-only
- Rerun safety: unsafe

Adjacent failure modes:
- SECN-GSL-201/203/204 (CI secret/token family), SEC-GSL-204 (bot-author gate weakness)

Recommended mitigation:
- Remediation patterns: untrusted-input isolation for CI agents; least-privilege tokens; no persistent credentials.
- Minimal repair: `persist-credentials: false` + push via a scoped token or PR; sanitize/quarantine repo-derived input (strip instruction-like content); scope permissions to a docs path; human review of agent-produced diffs before merge.
- Local guardrail: agent output lands on a branch + PR, never direct to `main`.
- Behavior test: workflow run with injection-flavored input → no direct write to protected branch; diff requires review.

Implementation assessment:
- Complexity: governance_decision
- Cost: S
- Cost drivers: workflow yaml, prompt pipeline, permission scoping, review step
- Nominal implementation agent: codex
- Rationale: workflow + automation-pipeline change, contained but policy-laden.

Validation:
- Dry-run with hostile input fixture; assert no direct main write.
- Permission audit: token scopes minimal.

Non-goals:
- Removing the docs automation (product decision).

### SECN-GSL-203: `create-version-bump-pr.yaml` passes six LLM API keys into unpinned `cargo update --workspace` + `cargo run`

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security (repo posture)

Evidence:
- `.github/workflows/create-version-bump-pr.yaml:6-13` — six LLM API keys in env; `:29-31,44-49,51,61` — steps run `cargo update --workspace` (unpinned) and `cargo run` via justfile
- `justfile:272-273` — version is deterministic; `:297-303,313-323` — the update/run recipes
- `crates/.../build_canonical_models.rs:33,328-342` — fetches models.dev only; keys are unnecessary for the version-bump computation
- `.github/workflows/minor-release.yaml:60-67` — sibling context

Observed behavior:
- A workflow executes `cargo update --workspace` (resolves to latest semver-compatible versions of all dependencies, i.e. arbitrary newly-published crate code) and `cargo run` of repo code, with six LLM API keys in the environment. The keys are unnecessary for the deterministic version bump.

Expected boundary:
- Dependency resolution and code execution in CI happen without production secrets in env; updates are pinned/reviewed (lockfile diff review) before execution.

Failure mechanism:
- `cargo update --workspace` pulls whatever is latest at run time; `cargo run` executes build scripts of those crates; secrets are present in env and can be exfiltrated by any compromised/newly-published dependency's build script.

Break-it angle:
- A typosquatted or compromised crate version published between lockfile state and workflow run executes its build script with six LLM keys in env → exfiltration. Cross-system blast: org LLM spend/keys. The deterministic version bump makes the keys pure liability.

Impact:
- Six org LLM API keys exposed to arbitrary mutable dependency code execution.

Operational impact:
- Blast radius: Cross-system
- Side-effect class: external API / secret exposure
- Reversibility: irreversible once leaked
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- SECN-GSL-201 (secrets in PR-triggered code), SECN-GSL-204 (unpinned external clone), SECN-GSL-205 (unpinned npx)

Recommended mitigation:
- Remediation patterns: least-privilege CI env; pinned/reviewed dependency mutation.
- Minimal repair: remove the six keys from the workflow env (they are unnecessary — version is deterministic); gate `cargo update` behind a reviewed lockfile-diff PR.
- Local guardrail: CI lint rule — no `secrets.*` in jobs running `cargo update`/`cargo run` of unpinned resolution.
- Behavior test: workflow env contains zero `secrets.*` references.

Implementation assessment:
- Complexity: governance_decision
- Cost: XS
- Cost drivers: workflow yaml only
- Nominal implementation agent: codex
- Rationale: single-file change; the justfile recipes already compute the version without keys.

Validation:
- Workflow lint/dry-run: no secrets in the update/run job env.
- Confirm version-bump job still succeeds without keys (dry-run).

Non-goals:
- Pinning the entire dependency tree repo-wide (separate effort).

### SECN-GSL-204: Hourly cron clones unpinned `block/Agent-Skills` and publishes manifest + zips to the production docs site

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security (repo posture)

Evidence:
- `.github/workflows/rebuild-skills-marketplace.yml:10-11` — hourly `schedule` cron; `:36-42` — clone of `block/Agent-Skills` default branch, unpinned; `:82` — publish step
- `documentation/package.json:8` — dependency/workspace context
- `documentation/scripts/generate-skills-manifest.js:19,48,23,351` — manifest generation from the cloned content

Observed behavior:
- Every hour, CI clones the default branch of the external `block/Agent-Skills` repository at whatever commit is current, generates a skills manifest and zips from it, and publishes to the production docs site — with no pin, no review, and no content validation gate.

Expected boundary:
- Production-published content derives from a pinned, reviewed revision; automation has a human or automated review gate before publish.

Failure mechanism:
- Mutable upstream (default branch HEAD) → generation → publish, with no pin or review in between.

Break-it angle:
- A compromised upstream repo (or a malicious PR merged upstream) changes a skill zip or manifest → within an hour it is live on the production docs site. Consumers (agents reading the marketplace, humans installing skills) receive the payload. Cascade: docs-site content becomes authority for skill installation.

Impact:
- Unreviewed external content published to production hourly; supply-chain channel into skill consumers.

Operational impact:
- Blast radius: Cross-system (docs-site consumers / agents reading the marketplace)
- Side-effect class: external API / published content
- Reversibility: compensatable (revert publish) but consumers may already have fetched
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- SECN-GSL-203 (unpinned resolution family), Goose/AAIF catalog fallback at runtime (stub from sec-node: supply-chain sibling surface)

Recommended mitigation:
- Remediation patterns: pinned upstream; review gate before publish.
- Minimal repair: pin the clone to a reviewed commit SHA (renovate-style bump PRs); add a content sanity gate (schema validation, size caps, malware scan) before publish.
- Local guardrail: publish job refuses to run if the clone ref is not the pinned SHA.
- Behavior test: workflow fails when upstream HEAD ≠ pinned SHA.

Implementation assessment:
- Complexity: governance_decision
- Cost: S
- Cost drivers: workflow yaml, manifest script validation, bump automation
- Nominal implementation agent: codex
- Rationale: workflow + script validation change; publish gating is policy.

Validation:
- Dry-run with unpinned ref → publish step skipped/fails.
- Manifest schema validation test with hostile fixture.

Non-goals:
- Auditing `block/Agent-Skills` itself (third-party repo).

### REL-GSL-001: OAuth token removal silently fails; `cleanup()` returns `Ok` unconditionally

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Reliability (secondary Security)

Evidence:
- `crates/gosling-providers/src/xai_oauth.rs:259-261` — `let _ = std::fs::remove_file(&self.cache_path);` in `clear()`; `:800-803` — `cleanup()` returns `Ok(())` unconditionally; `:242-248` — `load`/`has_token` still see the file after a failed removal
- `crates/gosling-providers/src/chatgpt_codex.rs:446-448,1028-1031` — identical pattern
- `crates/gosling-providers/src/init.rs:151-156` — registry wiring
- `crates/gosling-cli/src/commands/configure.rs:2298-2302` — warn-only, then outro "Removed custom provider"
- Correct pattern for contrast: `githubcopilot.rs:245-251`, `kimicode.rs:122-128,154-156`

Observed behavior:
- When OAuth token file removal fails (permissions, AV lock, race), the error is discarded (`let _ =`), `cleanup()` reports success, and the CLI prints "Removed custom provider". The stale token file remains on disk and remains loadable.

Expected boundary:
- Logout/removal either removes the credential or reports failure; the operator is never told "removed" while a live token remains at rest.

Failure mechanism:
- `remove_file` result is ignored and `cleanup()` has no error path for it; callers treat `Ok` as removal.

Break-it angle:
- Malformed/hostile environment (read-only cache dir, file locked by AV/indexer, path race) → silent retention of a valid OAuth token after the user explicitly logged out. False success causes action on invalid state (user believes credential is gone). Stale state persists across restarts.

Impact:
- Stale OAuth credentials survive explicit logout; user-visible false success; credential residue at rest.

Operational impact:
- Blast radius: Local
- Side-effect class: file (stale secret at rest) + user-visible false success
- Reversibility: compensatable
- Operator visibility: silent (false success shown)
- Rerun safety: safe

Adjacent failure modes:
- FSR-GSL-001 (temp/secret residue family), SEC-GSL-002 (log hygiene), OBS-GSL-001 (fsync family)

Recommended mitigation:
- Remediation patterns: status truthfulness; error propagation.
- Minimal repair: propagate `remove_file` errors through `clear()`/`cleanup()`; on failure, surface "token file could not be removed (path)" and keep the provider in a not-removed state. Mirror the `githubcopilot.rs`/`kimicode.rs` pattern.
- Local guardrail: after cleanup, verify the cache path no longer exists; if it does, report failure.
- Behavior test: removal failure (chmod read-only dir) → `cleanup()` returns Err and CLI does not print success.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Cost drivers: `xai_oauth.rs`, `chatgpt_codex.rs`, `configure.rs`, tests
- Nominal implementation agent: codex
- Rationale: two call sites plus CLI copy; existing correct pattern in-repo to mirror.

Validation:
- Test: failing `remove_file` → `Err` surfaced; success path unchanged.
- Manual check: logout with locked file → honest message.

Non-goals:
- Redesigning OAuth token storage.
- Keychain migration.

### SEC-GSL-002: Scanner warn logs embed 300-char tool previews and full argument JSON

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Security
- **Evidence:** `crates/gosling/src/security/scanner.rs:451-453,458-473` (`build_explanation`, `safe_truncate` 300); `crates/gosling/src/security/mod.rs:186,192,225` (`sanitized_explanation` = newline flatten only, `tracing::warn!`). Contrast `mod.rs:23-40` (keys-only, SEC-GOS-009).
- **Observed / Expected / Mechanism:** Scanner warnings log up to 300 chars of tool-call content plus full argument JSON. Expected: logs carry tool name/class only (INV-003 spirit, `docs/INTENT.md:44`). Mechanism: explanation builder embeds content; sanitizer flattens newlines only.
- **Break-it / Impact:** Secrets pasted into commands/args (incl. SEC-GSL-001 substituted passwords) persist in log files (0o700 dir, but still residue; shipped in diagnostics).
- **Operational impact:** Blast: Local | Side-effect: log residue | Reversibility: compensatable | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: log redaction. Minimal repair: truncate to tool name + side-effect class; never log raw args. Guardrail: sanitizer strips arg JSON. Behavior test: warn log for a secret-bearing call contains no secret substring.
- **Implementation:** local_guardrail | XS | drivers: `scanner.rs`, `security/mod.rs`, tests | agent: codex | rationale: contained to explanation builder.
- **Validation / Non-goals:** Assert log output excludes arg payload. Non-goal: redesigning the scanner.

### SEC-GSL-003: ACP `/health` and `/status` routes are unauthenticated; doc claims token required

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Security
- **Evidence:** `crates/gosling/src/acp/transport/mod.rs:281-308` (aux routes merged with only a CORS layer), `:260-277` (handlers return static strings); `documentation/docs/guides/remote-gosling-server.md:36` (claims token required); `acp/auth.rs:8-13` (constant-time compare exists, not applied to aux routes).
- **Observed / Expected / Mechanism:** `/health` and `/status` answer without the bearer token. Expected: doc claim holds — token required — or doc is corrected. Mechanism: aux router merged without the auth layer.
- **Break-it / Impact:** On a non-loopback deployment, unauthenticated parties can probe liveness/status (fingerprinting, uptime, readiness signal for attacks). Bounded: handlers return static strings.
- **Operational impact:** Blast: Service | Side-effect: information disclosure | Reversibility: reversible | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: uniform auth on all routes; docs-truthfulness. Minimal repair: apply the token check to aux routes (or bind aux to loopback); align the doc. Guardrail: route-layer test asserts 401 without token. Behavior test: `GET /health` without token → 401 when auth enabled.
- **Implementation:** local_guardrail | XS | drivers: `transport/mod.rs`, doc line, tests | agent: codex | rationale: one-layer change.
- **Validation / Non-goals:** HTTP-level test with and without token. Non-goal: redesigning ACP auth.

### LLM-GSL-002: Opt-in ML classifier POSTs full tool content and user messages to an external endpoint, no redaction

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Security (LLM)
- **Evidence:** `crates/gosling/src/security/classification_client.rs:141-160`; `crates/gosling/src/security/scanner.rs:106-119,497-515` (`extract_tool_content` sends full command/args JSON; `:189-199,110-119` default off, env override exists).
- **Observed / Expected / Mechanism:** With the classifier enabled, full tool-call content and user messages go to an external classification endpoint. Expected: redaction or explicit content-scope warning before enabling.
- **Break-it / Impact:** Sensitive content (credentials in commands, personal data in messages) leaves the host to a third-party endpoint; opt-in but undisclosed in scope.
- **Operational impact:** Blast: Cross-system | Side-effect: network egress of content | Reversibility: irreversible | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: egress redaction; disclosure at enable-time. Minimal repair: redact/strip args and user text before POST, or require an explicit acknowledgment of content scope. Guardrail: debug logs carry `has_token` only (already). Behavior test: classifier request body contains no raw user text.
- **Implementation:** local_guardrail | XS | drivers: `classification_client.rs`, enable-flow copy, tests | agent: codex | rationale: contained to the client.
- **Validation / Non-goals:** Mock-endpoint test asserting redacted body. Non-goal: removing the classifier feature.

### LLM-GSL-003: Opt-in Langfuse/OTLP telemetry exports full messages and span fields, no redaction or content-scope warning

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Security (LLM)
- **Evidence:** `crates/gosling/src/tracing/logging.rs:103-105`; `crates/gosling/src/agents/agent/reply_entry.rs:153-156` (full user message as `trace_input`), `:172-175`; `reply_stream.rs:1622` (`trace_output`); `tracing/observation_layer.rs:196-280,351-371` (no redaction); `tracing/langfuse_layer.rs:170-191`; `otel/otlp.rs:226-253,417-460,555-572`.
- **Observed / Expected / Mechanism:** With Langfuse/OTLP enabled, full user messages, assistant output, and span fields export to the telemetry backend. Expected: documented content scope, redaction option, warning at enable time.
- **Break-it / Impact:** Conversation content (potentially secrets pasted by users) ships to a third-party observability backend; opt-in but undisclosed in scope.
- **Operational impact:** Blast: Cross-system | Side-effect: network egress of content | Reversibility: irreversible | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: telemetry redaction; disclosure. Minimal repair: add a redaction layer or an explicit content-scope warning at enable; document what is exported. Guardrail: config validation warns on enable. Behavior test: exported span excludes message bodies when redaction on.
- **Implementation:** local_guardrail | XS | drivers: tracing layers, config UX, docs | agent: codex | rationale: contained to tracing config + layers.
- **Validation / Non-goals:** OTLP collector test asserting field redaction. Non-goal: removing telemetry.

### STT-GSL-001: `update_handoff_status` has no from-status precondition

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed (structural) / source-evidenced / State-Transition
- **Evidence:** `crates/gosling/src/session/handoff_storage.rs:118-155` — `UPDATE` keyed only by `snapshot_id`, no `AND status = ?`, no `rows_affected` check; sibling transitions carry preconditions.
- **Observed / Expected / Mechanism:** Any caller can overwrite a handoff snapshot's status regardless of its current status (e.g. completed → in-progress). Expected: transitions guarded by from-status, like siblings.
- **Break-it / Impact:** Status integrity broken under direct API use or future racy callers; no current in-repo racy caller found (contained today).
- **Operational impact:** Blast: Local | Side-effect: DB | Reversibility: compensatable | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: preconditioned state transition; idempotent terminal handling. Minimal repair: add `AND status = ?` + `rows_affected` check; map no-rows to a typed conflict. Guardrail: terminal states reject further transitions. Behavior test: transition from wrong status → conflict error, no row change.
- **Implementation:** local_guardrail | XS | drivers: `handoff_storage.rs`, tests | agent: codex | rationale: single-query change.
- **Validation / Non-goals:** State-machine test over all transitions. Non-goal: redesigning handoff lifecycle.

### STT-GSL-002: Archive gate is narrow; archived sessions remain mutable; CLI auto-unarchives on resume

- **Severity / Confidence / Evidence basis / Domain:** Low / Plausible / simulation-reasoned / State-Transition
- **Evidence:** `crates/gosling/src/acp/server/prompt_execution.rs:317-335`, `active_runs.rs:501`, `plan_storage.rs:1813` (`ensure_session_not_archived` — the only archive checks); `crates/gosling-cli/src/session/builder.rs:451-468` (CLI auto-unarchives on resume); `manage_sessions.rs:751-775`. ADR-0020 claims read-only plan state (satisfied); broader freeze expectation undocumented.
- **Observed / Expected / Mechanism:** "Archived" blocks only new prompts/plans; rename, truncate, system-prompt edits, output-revision restore, and delete still mutate archived sessions; the CLI silently un-archives on resume. Expected: archive = freeze (or documented narrow meaning).
- **Break-it / Impact:** Operator believes archive preserves a session; later operations mutate it; CLI resume silently reverses the archive. Misunderstood terminal state.
- **Operational impact:** Blast: Local | Side-effect: DB | Reversibility: compensatable | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: terminal-state freeze; truthful status semantics. Minimal repair: extend the archive check to mutating handlers, or document the narrow meaning and rename the concept; make CLI resume ask before un-archiving. Guardrail: mutation handlers share one `ensure_session_not_archived`. Behavior test: rename/truncate/restore on archived session → refused (or documented allowed).
- **Implementation:** workflow_protocol | S | drivers: ACP handlers, CLI builder, docs, tests | agent: codex | rationale: semantics decision + several call sites.
- **Validation / Non-goals:** Matrix test of mutations against archived sessions. Non-goal: full session immutability redesign.

### CON-GSL-001: `backendProcessRegistry.ts` cross-process read-modify-write has no cross-process lock

- **Severity / Confidence / Evidence basis / Domain:** Low / Likely / simulation-reasoned / Concurrency
- **Evidence:** `ui/desktop/src/backendProcessRegistry.ts:93-110` (in-process promise queue only), `:65-91` (`writeRecords` temp+rename, no fsync); file comment admits two Electron instances can share `userData`.
- **Observed / Expected / Mechanism:** Two Electron processes sharing `userData` can interleave registry reads/writes; the in-process queue serializes only within one process. Expected: cross-process mutual exclusion for the shared file (the Rust config path has one: `config/base.rs:856-895` flock).
- **Break-it / Impact:** Lost update → a stale record survives a sweep or a live record is dropped → missed orphan cleanup or false orphan. Contained by PID-reuse command-line + parent-liveness checks.
- **Operational impact:** Blast: Local | Side-effect: file | Reversibility: compensatable | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: cross-process lock; last-writer-wins with merge. Minimal repair: flock-style lock mirroring `config/base.rs:856-895`; merge sweep results through the update queue. Guardrail: re-read under lock before write. Behavior test: two-writer interleaving → no lost record.
- **Implementation:** cross_process_coordination | S | drivers: `backendProcessRegistry.ts`, lock helper, tests | agent: codex | rationale: mirrors an existing in-repo lock pattern.
- **Validation / Non-goals:** Two-process test or fault-injected interleaving. Non-goal: registry redesign.

### DAT-GSL-002: `output_revisions` is path-keyed with no session FK; cross-session disclosure and version chaining (absorbs TMP-GSL-001)

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Data-Integrity
- **Evidence:** `crates/gosling/src/session/output_revisions_storage.rs:43-49` (DDL: path-keyed, no `session_id` FK), `:414-452` (`list_output_revisions` discloses contributor metadata for any path); `crates/gosling-sdk-types/src/custom_requests.rs:1352-1360`; `session_crud.rs:583-627` (delete never touches it). TMP-GSL-001 (same table, same gap) merged here as an alias — see Appendix A.
- **Observed / Expected / Mechanism:** Revisions are keyed by output path, not session. `list_output_revisions` returns other sessions' contributor metadata for a path; session B can restore content authored under deleted session A for the same path (artifact-registration gate limits to own registered paths). Expected: revisions scoped to their owning session (FK + cascade).
- **Break-it / Impact:** Cross-session information disclosure (who produced which revision, when); version chaining across sessions; unbounded growth on delete (DAT-GSL-004 interaction).
- **Operational impact:** Blast: Local | Side-effect: DB | Reversibility: compensatable | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: referential integrity; scoped queries. Minimal repair: add `session_id` FK with cascade; scope `list_output_revisions` to the requesting session. Guardrail: migration backfills or orphans with a documented policy. Behavior test: session B listing session A's path → empty.
- **Implementation:** persistence_recovery | S | drivers: schema migration, storage queries, SDK types, tests | agent: codex | rationale: schema change + migration; moderate breadth.
- **Validation / Non-goals:** Migration test; cross-session isolation test. Non-goal: merging with DAT-GSL-004's delete semantics (separate decision).

### IOP-GSL-002: `on_link_shell_library_file` accepts any absolute host path

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Input-Output-Path
- **Evidence:** `crates/gosling/src/acp/server/shell_handlers.rs:362-379`; `crates/gosling/src/acp/server/shell_library_compilation.rs:155-236,200-236` — no root containment on the linked file path.
- **Observed / Expected / Mechanism:** The ACP shell-library link handler accepts any absolute host path and links/compiles it into the agent context. Expected: containment to workspace/session-owned directories (contrast: output-revision restore canonicalizes + `O_NOFOLLOW`, `output_revisions_storage.rs:414-757`).
- **Break-it / Impact:** Arbitrary host-file read into agent context (e.g. `/etc/passwd`, `~/.ssh/id_rsa`, cloud creds) via a single ACP request; content then flows into prompts (injection + exfiltration surface).
- **Operational impact:** Blast: Local | Side-effect: file read | Reversibility: irreversible (read) | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: path containment; allowlist roots. Minimal repair: confine to workspace + session dirs; reject symlinks/canonical escapes. Guardrail: reuse the canonical-path + `O_NOFOLLOW` pattern from output-revision restore. Behavior test: link `/etc/passwd` → refused.
- **Implementation:** local_guardrail | XS | drivers: `shell_handlers.rs`, `shell_library_compilation.rs`, tests | agent: codex | rationale: contained handler change with in-repo pattern to copy.
- **Validation / Non-goals:** Path-escape test suite. Non-goal: redesigning shell library management.

### IOP-GSL-003: Legacy `.jsonl` import silently drops malformed lines and truncates on IO error

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Input-Output-Path
- **Evidence:** `crates/gosling/src/session/legacy.rs:91-99` (per-line parse, malformed lines skipped), `:11`; `crates/gosling/src/session/session_manager/legacy_import.rs:65-95`; strict contrast: `import_formats/mod.rs:112-122`.
- **Observed / Expected / Mechanism:** Importing a legacy `.jsonl` session silently skips malformed lines and truncates the session on IO error. Expected: strict parse with a reported error/warning listing dropped content (the strict importer already exists as contrast).
- **Break-it / Impact:** Partial import presented as success; user believes full history imported; silent data loss at a trust boundary (import).
- **Operational impact:** Blast: Local | Side-effect: DB | Reversibility: irreversible (dropped lines) | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: strict import with accounting. Minimal repair: count and report dropped lines; fail or warn-visibly on truncation. Guardrail: import summary includes `lines_dropped`. Behavior test: file with N malformed lines → import reports N, imports the rest.
- **Implementation:** local_guardrail | XS | drivers: `legacy.rs`, `legacy_import.rs`, CLI copy, tests | agent: codex | rationale: contained to the legacy importer.
- **Validation / Non-goals:** Fixture with malformed lines → reported count matches. Non-goal: migrating off `.jsonl` support.

### IOP-GSL-004: Office archive validation checks declared sizes only; decompression is unbounded (zip-bomb DoS)

- **Severity / Confidence / Evidence basis / Domain:** Low / Plausible / simulation-reasoned / Input-Output-Path
- **Evidence:** `crates/gosling/src/acp/server/shell_library_formats.rs:163-192` (validation caps DECLARED uncompressed sizes), `:7-8,163-171`; actual decompression in office_oxide is unbounded.
- **Observed / Expected / Mechanism:** The handler validates the declared uncompressed sizes inside the office archive but the decompression path enforces no actual-bytes cap. Expected: enforce a hard cap on bytes actually decompressed.
- **Break-it / Impact:** A zip-bomb office document linked via the shell-library path decompresses without bound → disk/memory exhaustion (DoS). Resource-exhaustion manifestation is Likely-capped (not measured).
- **Operational impact:** Blast: Local | Side-effect: resource exhaustion | Reversibility: compensatable | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: enforced resource caps at the consuming boundary. Minimal repair: wrap decompression with a hard byte cap (declared-size check retained as a fast pre-filter). Guardrail: per-request decompression budget. Behavior test: bomb fixture → abort at cap, error surfaced.
- **Implementation:** workflow_protocol | S | drivers: `shell_library_formats.rs`, oxide wrapper, tests | agent: codex | rationale: needs a byte-counting wrapper around the decompressor.
- **Validation / Non-goals:** Fixture bomb aborts at cap. Non-goal: auditing office_oxide itself.

### DAT-GSL-001: Output-revision DB commit precedes the file rename; phantom "latest" revision after crash

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed (ordering source-evidenced) / Likely (phantom manifestation) / source-evidenced / Data-Integrity
- **Evidence:** `crates/gosling/src/session/output_revisions_storage.rs:322` (DB commit), `:382-392` (file rename after), `:626-628`.
- **Observed / Expected / Mechanism:** The revision row is committed before the content file is renamed into place; a crash between commit and rename leaves a "latest" revision whose file does not exist. Expected: file durable before the row points at it (or reconciliation on startup).
- **Break-it / Impact:** Crash between the two steps → phantom latest; restore of latest fails or serves stale content; operator sees a revision that was never fully written.
- **Operational impact:** Blast: Local | Side-effect: DB + file | Reversibility: compensatable | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: write-ahead ordering; startup reconciliation. Minimal repair: rename file first, fsync, then commit row; add a startup check for rows whose files are missing. Guardrail: restore path already has phantom-write guard — extend it to missing files. Behavior test: simulated crash between steps → startup reconciliation removes/flags the row.
- **Implementation:** persistence_recovery | S | drivers: `output_revisions_storage.rs`, startup path, tests | agent: codex | rationale: ordering change + reconciliation hook; crash testing needed.
- **Validation / Non-goals:** Fault-injection test for the ordering. Non-goal: redesigning revision storage.

### DAT-GSL-003: Malformed workspace store silently resets to defaults

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Data-Integrity
- **Evidence:** `crates/gosling/src/workspace/store.rs:197-224` (parse failure → defaults), `:283-318` (write path), `:83`; invariants held at `:336-338` (0o700), `:366,377,395-397` (0o600).
- **Observed / Expected / Mechanism:** A malformed workspace store file resets the workspace configuration to defaults with only a log line — dropping credential profiles and settings. Expected: fail-visible with backup/quarantine of the malformed file (contrast: config base has richer handling).
- **Break-it / Impact:** Corrupt/truncated store (crash mid-write, disk issue, manual edit) → silent loss of workspace config incl. credential profiles; user re-onboards or, worse, a reset profile points elsewhere.
- **Operational impact:** Blast: Local | Side-effect: file/DB-equivalent config | Reversibility: compensatable (restore from backup) | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: fail-visible config load; quarantine-and-recover. Minimal repair: on parse failure, move the malformed file aside (`.corrupt-<ts>`) and surface a UI-visible error before writing defaults. Guardrail: never overwrite the malformed file. Behavior test: malformed store → file preserved as `.corrupt-*`, error surfaced, defaults not silently persisted over it.
- **Implementation:** local_guardrail | XS | drivers: `store.rs`, UI surface, tests | agent: codex | rationale: contained to the store loader.
- **Validation / Non-goals:** Corrupt-fixture test. Non-goal: store schema redesign.

### SEC-GSL-202: `quarantine.yml` `pull_request_target` interpolates PR login into a `run:` shell

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Security (repo posture)
- **Evidence:** `.github/workflows/quarantine.yml:4,8-9` (`pull_request_target` trigger), `:21,:24` (`github.event.pull_request.user.login` and `vars.QUARANTINED_USERS` interpolated into `run:` shell), `:31`.
- **Observed / Expected / Mechanism:** PR-controlled values are interpolated into a shell script in a `pull_request_target` workflow. Expected: env-var indirection, never `${{ }}` into `run:`.
- **Break-it / Impact:** Latent injection: charset/admin-constrained today (GitHub login charset is safe; `vars.QUARANTINED_USERS` is admin-controlled), so not currently exploitable — filed as an anti-pattern with a one-line fix, not an active vuln.
- **Operational impact:** Blast: Cross-system (latent) | Side-effect: shell injection (latent) | Reversibility: n/a | Visibility: n/a | Rerun: unsafe if constraints change
- **Mitigation:** patterns: env-var indirection for all interpolations. Minimal repair: pass values via `env:` and reference `$VAR` in the script. Guardrail: CI lint bans `${{ }}` inside `run:`. Behavior test: lint rule fires on the current file until fixed.
- **Implementation:** local_guardrail | XS | drivers: `quarantine.yml`, lint config | agent: codex | rationale: one-file change.
- **Validation / Non-goals:** Lint pass. Non-goal: none.

### SECN-GSL-205: `pr-website-preview.yml` runs unpinned `wrangler@latest` with Cloudflare/INKEEP secrets in PR-head env

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Security (repo posture)
- **Evidence:** `.github/workflows/pr-website-preview.yml:20` (same-repo gate), `:27-28` (`CLOUDFLARE_API_TOKEN` in env), `:44-49` (`npm ci` of PR-head code with CF/INKEEP secrets), `:70` (`npx --yes wrangler@latest`, unpinned).
- **Observed / Expected / Mechanism:** The preview workflow installs PR-head dependencies and runs whatever `wrangler@latest` resolves to at run time, with Cloudflare/INKEEP secrets in env. Expected: pinned tool versions; secrets withheld from PR-head install steps.
- **Break-it / Impact:** A compromised `wrangler` release (or a malicious PR-head `package.json` postinstall) runs with CF/INKEEP secrets. Same-repo gate limits actors to collaborators, but the tool pin is absent.
- **Operational impact:** Blast: Cross-system | Side-effect: external API / secret exposure | Reversibility: irreversible once leaked | Visibility: silent | Rerun: unsafe
- **Mitigation:** patterns: pinned tools; least-privilege env. Minimal repair: pin `wrangler` to the lockfile version (`npx wrangler@<pinned>` or a package.json devDependency); move secrets to a deploy step that does not run PR-head install scripts (`--ignore-scripts` for the install step). Guardrail: CI lint for `npx --yes <pkg>@latest`. Behavior test: workflow diff shows pinned wrangler.
- **Implementation:** local_guardrail | XS | drivers: workflow yaml, package.json, lint | agent: codex | rationale: small workflow change.
- **Validation / Non-goals:** Dry-run; lint rule. Non-goal: removing the preview workflow.

### SEC-GSL-203: `release.yml` grants `secrets:inherit` and caller-level `id-token:write` on branch runs

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Security (repo posture)
- **Evidence:** `.github/workflows/release.yml:5-11` (`secrets:inherit`), `:55-72` (caller-level `id-token:write`), `:85-92`; inner gating holds (defense-in-depth erosion).
- **Observed / Expected / Mechanism:** The reusable release workflow receives all secrets and a caller-level OIDC token on non-tag `release/*` branch runs, relying on inner conditions to gate actual use. Expected: least-privilege at the workflow-call boundary; secrets only where used.
- **Break-it / Impact:** Any future caller (or a compromised inner step) on a branch run has org secrets + OIDC token available; today inner gating holds, so this is posture erosion, not an active exposure.
- **Operational impact:** Blast: Cross-system | Side-effect: secret/token availability | Reversibility: n/a | Visibility: silent | Rerun: unsafe if inner gating regresses
- **Mitigation:** patterns: least-privilege workflow calls. Minimal repair: replace `secrets:inherit` with an explicit secret list; scope `id-token:write` to the tag path only. Guardrail: CI lint for `secrets:inherit`. Behavior test: branch-run job env contains only the explicit list.
- **Implementation:** governance_decision | XS–S | drivers: `release.yml`, callers, lint | agent: codex | rationale: small but touches release plumbing; verify callers.
- **Validation / Non-goals:** Dry-run branch vs tag; confirm needed secrets still present. Non-goal: restructuring release automation.

### SEC-GSL-204: `update-release-pr.yaml` executes PR-head repo code with a `pull-requests:write` token, gated on bot authorship

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Security (repo posture)
- **Evidence:** `.github/workflows/update-release-pr.yaml:3-8` (trigger), `:13` (`just get-prior-version` — repo code execution), `:18` (PR authorship gate), `:36-40` (`pull-requests:write` `GH_TOKEN`), `:62-67`.
- **Observed / Expected / Mechanism:** The workflow runs repo code (`just get-prior-version`) from the PR head with a `pull-requests:write` token; the gate is only "is the PR authored by the expected bot". Expected: code execution from PR heads is not combined with write tokens, or the gate verifies the branch is the expected release/* branch from the expected repo.
- **Break-it / Impact:** Pushing commits to a bot-opened `release/*` branch keeps the bot as author → arbitrary code runs with the write token; a collaborator can push to such a branch. The authorship check is not a branch/repo check.
- **Operational impact:** Blast: Cross-system | Side-effect: VCS write / code execution | Reversibility: compensatable | Visibility: silent | Rerun: unsafe
- **Mitigation:** patterns: trusted-workflow gate; verify ref not just author. Minimal repair: gate on `github.event.pull_request.head.repo.full_name == github.repository && head.ref` matches `release/*` AND author is the bot; prefer `pull_request_target` with base checkout + pinned action, or run code only on `push` to `release/*`. Guardrail: CI lint for PR-triggered repo-code execution with write tokens. Behavior test: PR from a collaborator branch → job skipped.
- **Implementation:** governance_decision | XS | drivers: workflow yaml, lint | agent: codex | rationale: one-file gating fix.
- **Validation / Non-goals:** Dry-run PRs from collaborator and bot branches. Non-goal: none.

### ARC-GSL-001: `architecture.md` module table names three nonexistent modules and omits two

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Architecture
- **Evidence:** `docs/architecture.md:82,86,88` (names `workspace::model`, `workspace::context`, `acp::server::workspaces`); `crates/gosling/src/workspace/mod.rs:1-6`, `workspace/store.rs:20-22`, `crates/gosling-sdk-types/src/workspace.rs:296-298`, `workspace/service.rs:627`, `acp/server.rs:152-154` (actual modules; `bootstrap`, `planning_access` omitted).
- **Observed / Expected / Mechanism:** The module-contract table in architecture.md does not match the source tree. Expected: doc table matches `mod.rs` declarations.
- **Break-it / Impact:** Doc drift misleads contributors and future audits; seam analysis built on the table misidentifies ownership.
- **Operational impact:** Blast: Repo | Side-effect: none (doc) | Reversibility: reversible | Visibility: UI-visible (doc) | Rerun: safe
- **Mitigation:** patterns: docs-match-source. Minimal repair: correct the table to match `mod.rs`; add a doc-freshness check to stewardship. Guardrail: stewardship gate diffs module lists. Behavior test: every table row resolves to a real module path.
- **Implementation:** local_guardrail | XS | drivers: `architecture.md`, stewardship script | agent: claude | rationale: doc-only.
- **Validation / Non-goals:** Script check or manual diff vs `mod.rs`. Non-goal: restructuring the doc.

### ARC-GSL-002: README claims 1.4.0 current/verified; source is 1.5.0; release notes stop at v1.4.0

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Architecture
- **Evidence:** `README.md:76,79,186,242` (1.4.0 current/verified claims); `Cargo.toml:11-13` (1.5.0), `ui/desktop/package.json:4` (1.5.0); git `8bdf783e7` bumped manifests only; `79d25e527` docs alignment was for 1.4.0; release notes stop at v1.4.0.
- **Observed / Expected / Mechanism:** README's install/verify instructions reference 1.4.0 while the source tree is 1.5.0 and no 1.5.0 release notes exist. Expected: README version claims track the release.
- **Break-it / Impact:** Users install/verify against stale version claims; support and audit confusion; release-process gap (notes not written for 1.5.0).
- **Operational impact:** Blast: Repo | Side-effect: none (doc) | Reversibility: reversible | Visibility: UI-visible | Rerun: safe
- **Mitigation:** patterns: release-doc sync. Minimal repair: update README claims to 1.5.0 (or mark 1.5.0 as unreleased-in-docs) and add v1.5.0 release notes. Guardrail: release checklist includes README version sync. Behavior test: README version == workspace version at release.
- **Implementation:** local_guardrail | XS | drivers: README, release notes, checklist | agent: claude | rationale: doc-only.
- **Validation / Non-goals:** Version-string check in release checklist. Non-goal: cutting the release.

### ARC-GSL-005: `ToolPermissionStore` is dead code; `save()` has no fsync and no 0o600 (absorbs NEG-GSL-002)

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Architecture (secondary Negative-Space)
- **Evidence:** `crates/gosling/src/permission/permission_store.rs:1-148` (zero callers; `:63-76` `save()` temp+rename, no fsync, no 0o600); `crates/gosling/src/permission/mod.rs:14` (re-export only; `lib.rs:29` makes it `pub` so no dead-code lint); `docs/cloud/audit-dataflow-integrity.md:217,314,362-364` (prior audit treats it as live). Live denial mechanism verified working: `config/permission.rs` `PermissionManager`. NEG-GSL-002 merged here as an alias — see Appendix A.
- **Observed / Expected / Mechanism:** A public permission-denial store type exists with no callers; if revived, `save()` would write tool-call arguments (from `tool_request.to_readable_string()`, `:100,:17`) world-readable (umask default) without fsync. Expected: dead public API removed, or the prior audit doc corrected.
- **Break-it / Impact:** Future contributor revives the "obvious" store and silently reintroduces world-readable denial records; audit trail confusion (doc says live, code says dead).
- **Operational impact:** Blast: Local (latent) | Side-effect: file (if revived) | Reversibility: reversible (delete) | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: deadcode cleanup; audit-doc correction. Minimal repair: delete `permission_store.rs` (or fix perms + fsync and wire it in — not recommended); update `docs/cloud/audit-dataflow-integrity.md` to mark the store dead. Guardrail: clippy/udeps gate on public dead API. Behavior test: grep zero references post-removal; doc no longer claims it is live.
- **Implementation:** local_guardrail | XS | drivers: `permission_store.rs`, `mod.rs`, prior audit doc | agent: codex (human-owner for pub-API removal decision) | rationale: removal of a `pub` item is a (minor) API decision.
- **Validation / Non-goals:** Build + grep verification. Non-goal: redesigning denial persistence (PermissionManager works).

### STT-GSL-003: Migration v24 silently clears `restrict_tools_to_working_dirs` for all workspace sessions

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / State-Transition
- **Evidence:** `crates/gosling/src/session/session_manager/migrations.rs:711-721` (v24 clears the flag for all workspace sessions); `:418-433` (v18 added column `DEFAULT FALSE`); `schema.rs:83`; `session_manager.rs:72` (`CURRENT_SCHEMA_VERSION = 38`); `working_dir_scope_inspector.rs:14-18` (flag off → out-of-scope reads pass); ACP toggle `manage_sessions.rs:271-297`.
- **Observed / Expected / Mechanism:** Upgrading the schema to v24 silently turns off working-directory restriction for every workspace session, with no user notification. Expected: security-relevant migration either preserves the flag or is announced/requires opt-in.
- **Break-it / Impact:** After upgrade, sessions that had confinement lose it silently; tool calls can read/write outside working dirs (the inspector passes when the flag is off). Silent security-posture downgrade at upgrade time.
- **Operational impact:** Blast: Local | Side-effect: DB | Reversibility: compensatable (re-enable per session) | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: security-preserving migrations; upgrade notices. Minimal repair: v24 preserves the flag (or sets it to the session's prior value); add an upgrade notice listing the behavior change. Guardrail: migration review checklist item for security-flag changes. Behavior test: session with flag on → still on after v24.
- **Implementation:** persistence_recovery | XS (notice) to S (semantics) | drivers: `migrations.rs`, upgrade notice, tests | agent: codex + human-owner if semantics change | rationale: semantics of the flag are a product decision.
- **Validation / Non-goals:** Migration test: flag preserved across v24. Non-goal: redesigning the inspector.

### ARC-GSL-006: Orchestrator `start_agent` accepts any absolute `working_dir` in Auto mode; `view_session`/`send_message` reach any session

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Architecture
- **Evidence:** `crates/gosling/src/acp/server/orchestrator.rs:382-429` (`start_agent`: only canonicalize+`is_dir` at `:409-413`, `GoslingMode::Auto` at `:423`, `create_session` `:427-429`); `:250-267` (`view_session` reads any session incl. full conversation); `:462-478` (`send_message` drives any session); contrast `delegate_config.rs:286-314` (delegate working_dir confinement); `mod.rs:161-170` (orchestrator opt-in: `default_enabled: false`, `hidden: true`).
- **Observed / Expected / Mechanism:** The opt-in orchestrator surface starts agents in any absolute directory in Auto mode and can view/drive any session — unlike the delegate path, which confines `working_dir`. Expected: same confinement discipline as delegation, or documented as an operator-only power surface.
- **Break-it / Impact:** With the orchestrator enabled, any ACP client can spawn Auto agents outside the workspace and read/drive other sessions' conversations (cross-session disclosure + unconfined execution).
- **Operational impact:** Blast: Local | Side-effect: process + DB read | Reversibility: compensatable | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: confinement parity; documented power surface. Minimal repair: confine `start_agent` `working_dir` like `delegate_config.rs:286-314`; scope `view_session`/`send_message` to owned sessions or require explicit operator grant. Guardrail: orchestrator stays opt-in + hidden until confinement lands. Behavior test: `start_agent` with `/etc` working_dir → refused; `view_session` on foreign session → refused.
- **Implementation:** workflow_protocol | S | drivers: `orchestrator.rs`, `delegate_config.rs` pattern, docs, tests | agent: codex | rationale: mirrors an existing confinement pattern.
- **Validation / Non-goals:** Confinement + scoping tests. Non-goal: removing the orchestrator.

### CMP-GSL-001: `.giles/` + `governance/` advisory metadata is stale (~2.5 months); foreign `repo_path` in run log

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Compliance-Posture
- **Evidence:** `.giles/audit_report.yaml:5` (`generated_at` 2026-07-07, 91 open findings, `audit_structure_crashed: true`); `.giles/patch_todo.yaml:5,16-27` (`status: active`, 9 open actions); `.giles/run_log.jsonl:7-9` (entries 2026-08-03 with foreign `repo_path` `/sessions/rcw-01wdrvhoww7uw2czde9glyfw/mnt/vscode/forked/gosling`); last git touch `8d729903e` 2026-08-03; HEAD `29d31f707` 2026-10-12. Nothing marks staleness.
- **Observed / Expected / Mechanism:** Advisory governance metadata is ~2.5 months stale relative to HEAD, includes a crashed scan flag and 91 open findings, and its run log records a foreign `repo_path` — with no staleness marker. Expected: advisory metadata either refreshed or visibly marked stale; never silently authoritative (AGENTS.md: `.giles/*.yaml` are advisory mirrors).
- **Break-it / Impact:** Consumers (agents, Giles scans) may treat stale findings as current; foreign path leaks infrastructure layout; crashed flag unaddressed.
- **Operational impact:** Blast: Repo | Side-effect: none (metadata) | Reversibility: reversible | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: freshness markers; advisory labeling. Minimal repair: add a generated-at/staleness banner; re-run the scan or mark the report superseded; correct or annotate the foreign `repo_path`. Guardrail: agents must not treat `.giles` as authority (already AGENTS.md policy — enforce in tooling). Behavior test: staleness banner present when `generated_at` is older than N days.
- **Implementation:** governance_decision | XS (banner) to M (re-scan) | drivers: `.giles/*`, `governance/*`, scan tooling | agent: human-owner | rationale: governance decision; re-scan is an owner action.
- **Validation / Non-goals:** Banner present; scan re-run recorded. Non-goal: promoting advisory metadata to authority.

### CMP-GSL-002: Tagteam plan doc still claims implemented/in-progress though the feature was removed 2026-08-27

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Compliance-Posture
- **Evidence:** `reports/2026-07-12-tagteam-future-integration-plan.md:3-4` ("Phase 1 implemented… Phase 2 in progress"); commits `602ae43c2`, `2e4f22fb5` (removal 2026-08-27); `migrations.rs:753-789` (v30 DROPs tagteam tables, deliberate + regression test `session_manager.rs:5410`); doc last touched `93c10b945` 2026-07-14, never annotated.
- **Observed / Expected / Mechanism:** A planning report describes a removed feature as implemented/in-progress, with no annotation. Expected: removed-feature docs are annotated or moved to a historical section.
- **Break-it / Impact:** Readers plan against a nonexistent feature; migration v30's DROP looks contradictory without the annotation.
- **Operational impact:** Blast: Repo | Side-effect: none (doc) | Reversibility: reversible | Visibility: UI-visible | Rerun: safe
- **Mitigation:** patterns: doc lifecycle annotation. Minimal repair: prepend a "Superseded/removed 2026-08-27" banner with commit links. Guardrail: stewardship sweep for docs referencing dropped features. Behavior test: doc carries the banner.
- **Implementation:** local_guardrail | XS | drivers: one report file, stewardship sweep | agent: codex | rationale: doc-only.
- **Validation / Non-goals:** Banner present. Non-goal: deleting historical reports.

### WFG-GSL-001: DiffViewer header/footer untruncated; overflow on narrow terminals

- **Severity / Confidence / Evidence basis / Domain:** Low / Likely / simulation-reasoned / Workflow-GUI
- **Evidence:** `ui/text/src/components/DiffViewer.tsx:13-16` (`PAD_X/PAD_Y/HEADER_LINES/FOOTER_LINES`), `:65-68` (`innerWidth`/`viewportHeight` math), `:129-143` (header Box, two untruncated `Text`), `:193-212` (footer, 14 untruncated `Text`, 55-cell hint); `tui.tsx:1031` (`safeTermWidth` min 40), `:1292-1300` (full-terminal overlay). Ink no-clip semantics per AGENTS.md (Ink-Text/Ink-Overflow rules). Same finding as the arch group's routed stub.
- **Observed / Expected / Mechanism:** The fixed-height overlay reserves one line each for header/footer but neither is truncated; both exceed `innerWidth` on narrow terminals (footer hint 55 cells vs `innerWidth` 36–54 at widths 40–58). Expected: header/footer fit the computed inner width (truncate/pre-truncate per AGENTS.md Ink rules).
- **Break-it / Impact:** Overflow bleeds into neighboring cells and breaks the diff layout on narrow terminals. Runtime manifestation Likely-capped (not executed).
- **Operational impact:** Blast: Local | Side-effect: UI corruption | Reversibility: reversible | Visibility: UI-visible | Rerun: safe
- **Mitigation:** patterns: fit-content-to-container (AGENTS.md Ink-Text/Ink-Layout rules). Minimal repair: truncate header/footer strings to `innerWidth` before render. Guardrail: TUI lint for untruncated Text in fixed-height boxes. Behavior test: render at width 40 → header/footer cell counts ≤ `innerWidth`.
- **Implementation:** local_guardrail | XS | drivers: `DiffViewer.tsx`, tests/snapshot | agent: codex | rationale: contained component fix.
- **Validation / Non-goals:** Width-sweep render test. Non-goal: redesigning the diff overlay.

### WFG-GSL-002: PermissionPrompt height budget under-reserves with many options/rawInput

- **Severity / Confidence / Evidence basis / Domain:** Low / Likely / simulation-reasoned / Workflow-GUI (secondary Security)
- **Evidence:** `ui/text/src/components/PermissionPrompt.tsx:69` (`boxHeight` min 10), `:74-85` (baseLines/showFooter/showOptionsMargin/payloadLineBudget math), `:120-176` (render); `tui.tsx:1279-1287,1032`. Worked example: 7 options on a 10-row terminal → 11 lines in a 10-line box; `payloadLineBudget` clamped to min 1.
- **Observed / Expected / Mechanism:** The prompt's height budget under-reserves when options/rawInput exceed the box; content overflows the fixed-height prompt on short terminals. Expected: content fits or scrolls within the reserved box (AGENTS.md Ink-HeightBudget rule).
- **Break-it / Impact:** Trust-boundary UI renders corrupt on short terminals; options remain selectable — usability defect, not an approval bypass (verified: selection semantics unchanged).
- **Operational impact:** Blast: Local | Side-effect: UI corruption | Reversibility: reversible | Visibility: UI-visible | Rerun: safe
- **Mitigation:** patterns: audit-the-line-count (AGENTS.md Ink-HeightBudget). Minimal repair: recompute the budget including every option row and clamp options shown with a scroll indicator. Guardrail: height-budget unit test per option count. Behavior test: 7 options on 10 rows → all chrome fits, no overflow.
- **Implementation:** local_guardrail | XS–S | drivers: `PermissionPrompt.tsx`, tests | agent: codex | rationale: contained component fix with a worked example.
- **Validation / Non-goals:** Row-count sweep test. Non-goal: redesigning the prompt.

### WFG-GSL-003: Message edit truncates the backend conversation before the UI is synced; a lost response leaves permanent divergence

- **Severity / Confidence / Evidence basis / Domain:** Low / Plausible / simulation-reasoned / Workflow-GUI (secondary Data-Integrity)
- **Evidence:** `ui/desktop/src/chatSessionController.ts:556-585` (`updateMessage`: `acpTruncateSessionConversation` → `setMessages` → `submitMessage`; catch sets `Idle` and rethrows); `ui/desktop/src/sessions.ts:558-564` (thin ACP wrapper). Adjacent: `forkSessionWithEditedMessage` (`chatSessionController.ts:128-143`) has the same lost-response ambiguity.
- **Observed / Expected / Mechanism:** An in-place edit truncates the backend conversation first, then syncs the UI and resubmits. If the truncate response is lost (connection drop), the UI and backend diverge permanently until reload. Expected: sync-then-mutate or a reconcilable divergence path.
- **Break-it / Impact:** Partial-failure state divergence on a trust-boundary action (editing history); the operator sees a state the backend does not have.
- **Operational impact:** Blast: Local | Side-effect: state divergence | Reversibility: compensatable (reload) | Visibility: error surfaced on catch, but divergence itself silent | Rerun: unsafe (rerun re-truncates)
- **Mitigation:** patterns: reconcile-on-reconnect; idempotent mutation. Minimal repair: after reconnect/retry, reconcile UI from backend truth (`getSession` messages) before further edits; or truncate via the fork path. Guardrail: edit action disabled while a truncate is unacknowledged. Behavior test: drop the truncate response → next sync restores backend truth.
- **Implementation:** workflow_protocol | S | drivers: `chatSessionController.ts`, `sessions.ts`, reconnect path, tests | agent: codex | rationale: needs a reconciliation path across the ACP client.
- **Validation / Non-goals:** Fault-injected lost-response test. Non-goal: redesigning edit semantics.

### FSR-GSL-001: Extension tempdirs holding keyring secrets and inline-Python source are cleaned only on graceful shutdown; no startup sweep

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Failsafe (secondary Security)
- **Evidence:** `crates/gosling/src/agents/extension_manager.rs:195` (`TempDir` on `Extension`, dropped only on drop), `:715-722` (tempdir + `write_docker_env_file`; comment: env can contain keyring secrets), `:764-766` (inline-Python source write); `crates/gosling/src/agents/extension_manager/child_process.rs:88-140` (`write_docker_env_file`: 0o600 unix, no fsync, direct write). No sweep reference exists (grep). Contrast: `plugins/mod.rs:547-577` sweep pattern.
- **Observed / Expected / Mechanism:** Docker-extension tempdirs can contain keyring secrets (via `--env-file`) and inline-Python extension source. They are removed only on graceful shutdown (`TempDir` drop); a crash/SIGKILL leaves them in `$TMPDIR` with no startup sweep. Expected: crash-resilient cleanup (startup sweep keyed to a marker/owner).
- **Break-it / Impact:** Secret-bearing residue accumulates in a shared temp dir after crashes; local secret exposure window persists until reboot/tmp cleanup. Resilience mapping: phase recover/anticipate; objectives constrain/reconstitute; current safe state fail_silent vs ideal fail_quarantined. FMECA: plausible/silent, no compensating provision.
- **Operational impact:** Blast: Local | Side-effect: file (secret residue) | Reversibility: compensatable (delete) | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: startup sweep; crash-safe cleanup. Minimal repair: on extension-manager startup, sweep `gosling` tempdirs older than N hours not owned by a live PID (mirror `plugins/mod.rs:547-577`); fsync the env file. Guardrail: env files always 0o600 (already unix) + no secrets in dir names. Behavior test: kill -9 a running extension → next startup removes its tempdir.
- **Implementation:** persistence_recovery | S | drivers: `extension_manager.rs`, `child_process.rs`, sweep helper, tests | agent: codex | rationale: mirrors an existing in-repo sweep pattern; crash test needed.
- **Validation / Non-goals:** Kill-test + startup sweep test. Non-goal: moving secrets out of env files (bigger change).

### FSR-GSL-002: Plugin staging-dir sweep never removes abandoned dirs on non-unix; unix removal failures are silent

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed / source-evidenced / Failsafe (secondary Reliability)
- **Evidence:** `crates/gosling/src/plugins/mod.rs:552-577` (unix `let _ = fs::remove_dir_all(...)` — failures ignored), `:579-583` (`#[cfg(not(unix))] fn staging_owner_is_running(_pid) -> bool { true }` — stub returns true, so non-unix never sweeps), `:547-577`.
- **Observed / Expected / Mechanism:** The plugin staging sweep treats every directory as owned on non-unix (stub returns `true`), so abandoned staging dirs are never removed on Windows; on unix, removal failures are silently ignored. Expected: liveness check on all platforms; removal failures logged and retried.
- **Break-it / Impact:** Unbounded disk growth in the plugin root on Windows per killed install; silent failure hides the leak. FMECA: likely/silent on Windows.
- **Operational impact:** Blast: Local | Side-effect: disk growth | Reversibility: compensatable | Visibility: silent | Rerun: safe (accumulation worsens)
- **Mitigation:** patterns: cross-platform liveness check; visible cleanup failure. Minimal repair: implement a Windows owner check (e.g. lock-file held open by the owner process); log + count removal failures. Guardrail: sweep reports removed/failed counts. Behavior test: abandoned dir on each platform → removed or logged.
- **Implementation:** local_guardrail | S | drivers: `plugins/mod.rs`, platform owner check, tests | agent: codex | rationale: contained to the sweep; Windows path needs a real owner check.
- **Validation / Non-goals:** Windows + unix sweep tests. Non-goal: redesigning plugin staging.

### FSR-GSL-003: ui/text markdown `parseCache` is an unbounded module-level Map

- **Severity / Confidence / Evidence basis / Domain:** Low / Confirmed (property) / Likely (OOM manifestation) / source-evidenced / Failsafe (secondary Reliability)
- **Evidence:** `ui/text/src/markdown.tsx:22` (module-level `Map`), `:24-42` (cache insert, no eviction); `ui/text/src/components/ContentRenderers.tsx:92-93` (`cacheKey` per chunk). Contrast: desktop store bounded (`MAX_RETAINED_SESSIONS = 50`, `chatSessionStore.ts:231-322` — prior MEM-GSL-003 fix verified present).
- **Observed / Expected / Mechanism:** Every markdown chunk's source + rendered lines are retained for the process lifetime with no eviction — a ~2–3x multiplier on conversation-text memory. Expected: bounded cache (LRU) or per-render memo. OOM manifestation Likely-capped (not measured).
- **Break-it / Impact:** Long sessions in the TUI grow memory without bound → eventual OOM/kill. Desktop is bounded; the gap is TUI-only.
- **Operational impact:** Blast: Local | Side-effect: memory growth | Reversibility: compensatable (restart) | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: bounded cache. Minimal repair: LRU with a small cap (e.g. 200 entries) or `WeakRef`-keyed memo. Guardrail: cache-size metric in debug output. Behavior test: render 10k chunks → cache size ≤ cap.
- **Implementation:** local_guardrail | XS | drivers: `markdown.tsx`, tests | agent: codex | rationale: contained one-file change.
- **Validation / Non-goals:** Bound test. Non-goal: TUI conversation retention policy (separate design decision).

### ARC-GSL-004: sessions.db schema is v38; docs cite v36; v38 undocumented, no ADR

- **Severity / Confidence / Evidence basis / Domain:** Info / Confirmed / source-evidenced / Architecture
- **Evidence:** `crates/gosling/src/session/session_manager/session_manager.rs:72` (`CURRENT_SCHEMA_VERSION = 38`); `migrations.rs:832-836` (v36/37/38), `:753-789` (v30 tagteam DROP — separate); `session_crud.rs:66`, `schema.rs:467-507` (v38 `session_id_high_water`); `docs/architecture.md:32,267` (cite v36).
- **Observed / Expected / Mechanism:** Docs cite schema v36 while the code is at v38; the v38 high-water-mark feature is undocumented and has no ADR. Expected: architecture doc tracks schema version; new features get ADRs.
- **Break-it / Impact:** Doc drift; auditors/operators misinformed about migration state.
- **Operational impact:** Blast: Repo | Side-effect: none (doc) | Reversibility: reversible | Visibility: UI-visible | Rerun: safe
- **Mitigation:** patterns: doc-freshness; ADR discipline. Minimal repair: update the doc to v38 and add a short ADR or doc section for the high-water mark. Behavior test: doc version == `CURRENT_SCHEMA_VERSION`.
- **Implementation:** local_guardrail | XS | drivers: `architecture.md`, ADR | agent: claude | rationale: doc-only.
- **Validation / Non-goals:** Version-string check. Non-goal: none.

### NEG-GSL-001: ui/text `App.initialPrompt` is a dead prop; four dependent branches unreachable

- **Severity / Confidence / Evidence basis / Domain:** Info / Confirmed / source-evidenced / Negative-Space
- **Evidence:** `ui/text/src/tui.tsx:1583-1592` (`--text` routes to `runTextMode` and returns before render; `:1590-1592` passes `cli.flags.text` which is falsy there), `:798-800`, `:883`, `:1039`, `:1188` (four dependent branches unreachable).
- **Observed / Expected / Mechanism:** `initialPrompt` is always `undefined` in the TUI render path, making four branches dead. Expected: dead prop removed or the routing fixed so the prop is meaningful.
- **Break-it / Impact:** Dead code + unreachable branches mislead future edits; negative-space risk if routing changes later.
- **Operational impact:** Blast: Local | Side-effect: none | Reversibility: reversible | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: deadcode cleanup. Minimal repair: remove the prop and the four branches (or fix routing to pass the prompt). Behavior test: grep zero references post-removal.
- **Implementation:** local_guardrail | XS | drivers: `tui.tsx` | agent: codex | rationale: contained.
- **Validation / Non-goals:** Typecheck + grep. Non-goal: redesigning TUI routing.

### OBS-GSL-001: `write_file_atomic` and registry `writeRecords` lack a directory fsync after rename

- **Severity / Confidence / Evidence basis / Domain:** Obs / Confirmed / source-evidenced / Reliability
- **Evidence:** `crates/gosling/src/config/base.rs:83-95` (`write_file_atomic`: file fsync + rename, no dir fsync); `ui/desktop/src/backendProcessRegistry.ts:65-91` (`writeRecords`: temp+rename, no fsync at all).
- **Observed / Expected / Mechanism:** Atomic-replace writes do not fsync the containing directory, so the rename itself may not be durable across power loss (POSIX). Expected: dir fsync after rename for durability claims.
- **Break-it / Impact:** Crash/power-loss can lose the rename → stale config/registry content despite "atomic" writes. Observation: durability gap, bounded impact (next write heals config; registry heals via liveness checks).
- **Operational impact:** Blast: Local | Side-effect: file durability | Reversibility: compensatable | Visibility: silent | Rerun: safe
- **Mitigation:** patterns: durable rename. Minimal repair: fsync the parent dir after rename in `write_file_atomic`; add file+dir fsync to `writeRecords`. Behavior test: fault-injection at the fsync layer (mock) asserting the dir-fsync call.
- **Implementation:** local_guardrail | XS | drivers: `base.rs`, `backendProcessRegistry.ts` | agent: codex | rationale: two small additions.
- **Validation / Non-goals:** Mocked-fs test asserting fsync sequence. Non-goal: redesigning atomic-write helpers.

### OBS-GSL-002: `findAvailablePort` listen-0/close/reuse is a TOCTOU window

- **Severity / Confidence / Evidence basis / Domain:** Obs / Confirmed / source-evidenced / Reliability
- **Evidence:** `ui/desktop/src/goslingServe.ts:120-132` (bind port 0 → read assigned port → close → reuse).
- **Observed / Expected / Mechanism:** The helper picks a free port by binding `:0`, closing, and reusing the number; another process can claim it in between. Expected: pass the listener through (bind once, hand the socket/port to the child) or accept + retry on `EADDRINUSE`.
- **Break-it / Impact:** Rare port collision at backend startup → child fails to bind; bounded (retry exists at a higher level? verify at patch time).
- **Operational impact:** Blast: Local | Side-effect: startup failure (rare) | Reversibility: reversible | Visibility: error at startup | Rerun: safe
- **Mitigation:** patterns: bind-once handoff; retry on `EADDRINUSE`. Minimal repair: retry loop around child startup on bind failure. Behavior test: occupy the returned port → helper/startup retries with a new port.
- **Implementation:** local_guardrail | XS | drivers: `goslingServe.ts` | agent: codex | rationale: contained.
- **Validation / Non-goals:** Collision-injection test. Non-goal: redesigning serve lifecycle.

### OBS-GSL-003: `delete_session` removes DB rows before the in-memory agent is stopped

- **Severity / Confidence / Evidence basis / Domain:** Obs / Confirmed / source-evidenced / Reliability
- **Evidence:** `crates/gosling/src/session/session_manager/session_crud.rs:583-627` (DB delete) vs `crates/gosling-cli/src/commands/manage_sessions.rs:380-385` (in-memory agent stop ordering).
- **Observed / Expected / Mechanism:** The DB deletion precedes stopping the in-memory agent for the session; if the stop fails or the process is mid-turn, the agent can write to a deleted session (error-visible) or resurrect state. Expected: stop/quiesce first, then delete.
- **Break-it / Impact:** Ordering smell with error-visible failure; bounded today (writes to a deleted session surface errors).
- **Operational impact:** Blast: Local | Side-effect: DB | Reversibility: compensatable | Visibility: error-visible | Rerun: safe
- **Mitigation:** patterns: quiesce-before-delete. Minimal repair: stop the in-memory agent, then delete rows. Behavior test: delete during active turn → agent stopped before rows removed.
- **Implementation:** workflow_protocol | XS | drivers: `session_crud.rs`, `manage_sessions.rs` | agent: codex | rationale: reordering with a test.
- **Validation / Non-goals:** Active-turn delete test. Non-goal: redesigning session lifecycle.

### OBS-GSL-004: stop/pre/post-tool hooks bypass the `interaction_policy` check

- **Severity / Confidence / Evidence basis / Domain:** Obs / Confirmed / source-evidenced / Security (policy)
- **Evidence:** `crates/gosling/src/agents/agent/hooks.rs:46-81` (`emit_hook` gated by interaction_policy) vs `:93-103,105-112,196-207,232-272,312-333` (`emit_stop_hook`, `emit_stop_hook_blocking`, `with_post_tool_hook` — ungated).
- **Observed / Expected / Mechanism:** Only the generic `emit_hook` consults `interaction_policy`; the stop and post-tool hook emitters run regardless of the session's interaction policy. Expected: policy check on all hook emissions (or documented exemption).
- **Break-it / Impact:** A session under a restrictive interaction policy still fires stop/post-tool hooks → policy bypass for hook-driven side effects (notifications, scripts). Bounded: hooks are operator-configured, payload goes via stdin (not interpolated — verified).
- **Operational impact:** Blast: Local | Side-effect: process (hook execution) | Reversibility: compensatable | Visibility: log-only | Rerun: safe
- **Mitigation:** patterns: uniform policy enforcement. Minimal repair: route stop/post-tool emitters through the same policy check. Guardrail: policy test covers every emitter. Behavior test: restrictive policy → no stop/post-tool hook fires.
- **Implementation:** workflow_protocol | S | drivers: `hooks.rs`, policy, tests | agent: codex | rationale: single-file gating change.
- **Validation / Non-goals:** Policy-matrix test over emitters. Non-goal: redesigning hooks.

## Non-Findings / Checked But Not Confirmed

Important seams checked and held, each with the line that makes them safe. Consolidated from all 7 subagent groups; lead spot-verified.

### Security (Rust core + LLM) — held

1. Fail-closed tool inspection: `agents/tool_inspection.rs:107-163` (`Err` → `RequireApproval` per request; the Auto downgrade loop touches only the `Ok` branch). Lead independently re-verified `:125-170`.
2. 0o600 secret files: `config/base.rs:48-76` (+ `:65,695,868,922,1314`), `config/permission.rs:156`.
3. 0o700 session DB dir: `session/session_manager/pool_lifecycle.rs:28-45` (`:36,:64`).
4. `git clone --` guard + transport-helper rejection: `plugins/mod.rs:474-485` (`:480`), `:449-467` (`ext::` guard).
5. Persisted permission denials fail-closed: `config/permission.rs:44-66,327-345`.
6. PostHog telemetry disabled by default: `tracing/posthog.rs:23-27`.
7. Nostr share redaction: `acp/server/manage_sessions.rs` ~`:835`; `cli.rs:626-630,2199`; `session/redaction.rs:11-30,57-79`.
8. Session export file perms: `write_owner_only_output_file`.
9. LLM request log dir 0o700: `providers/utils.rs:265-281`; `gosling-cli/src/logging.rs:14`; `cli.rs:2785`.
10. Skill admission: content-hash binding, catalog-facts authority, ceilings enforced (`skills/admission.rs`; `tool_dispatch.rs:288-296`).
11. Action-bound dispatch integrity: `session_manager/tool_operations.rs:73-190`; `interaction_policy.rs:302-380+`.
12. ACP serve auth posture: `cli.rs:1904-1912` (token required), `:1958-1971` (unauth refused off-loopback), `:1023-1027` (loopback default); `acp/auth.rs:8-13,15-29`; `transport/mod.rs:130-165`.
13. MCP app proxy hardened: `mcp_app_proxy.rs:21-36` (guest CSP), `:295-300` (const-time compare), `:431-460` (loopback, single-use tokens, TTL 300 s / 64 entries / 16 MB cap).
14. OSV extension malware check fail-closed: `agents/extension_malware_check.rs:60-91,226-280`; `extension_manager.rs:703,761`.
15. Project-plugin trust gate: `plugins/discovery.rs` (settings.json can only disable; `trust_project` is an explicit user action).
16. Workspace store invariants: `workspace/store.rs:336-338` (0o700), `:366,377,395-397` (0o600); `service.rs:1040-1070` (`reject_secret_shaped_value`); `validation.rs:8-31`.
17. Scoped-credential destination binding: `authentication.rs:75-92` (blake3 destination hash); `extension_manager.rs:437-452` (refuses changed destination); `config/extensions.rs:27-36` (namespaced env secrets).
18. Secret-in-log sweep: clean except SEC-GSL-002.
19. Session diagnostics redaction: `session/diagnostics.rs:307-308,397,424,444`; tests `:584-686`.
20. Handoff snapshot redaction: `session/handoff.rs:175-231`; `export_bundle.rs:41-98`.
21. Large tool responses: `agents/large_response_handler.rs:117-160` (0o700, symlink rejection), `:100-107` (untrusted-content banner) — cleanup gap filed as IOP-GSL-001.
22. Mandatory inspectors in Auto (non-downgrading): `security_inspector.rs:62`, `adversary_inspector.rs:390`, `skill_authority_inspector.rs:36`, `working_dir_scope_inspector.rs:186`. `EgressInspector` intentionally downgrades.
23. Working-dir scope inspector: canonicalization + symlink rejection; opt-in per session (design).
24. Hooks: payload goes to stdin, not interpolated; `emit_blocking` fails open (documented). Policy gap filed as OBS-GSL-004.
25. OAuth flows: loopback bind, state validated, first-callback-wins, escaping; auth URL printed to stderr (intentional). Removal failure filed as REL-GSL-001.
26. ACP client-side fs/shell wrappers: editor is the trust principal (by design).
27. Electron shell hardening: `main.ts:1255-1261`, `main/windowChrome.ts:80-83` (`webSecurity`, no `nodeIntegration`, `contextIsolation`, `sandbox`). Renderer self-grant filed as SEC-GSL-201.
28. Session-history extension scoped to own lineage; no RAG/vector store in repo (LLM-008 N/A).
29. Consumption caps: `DEFAULT_MAX_TURNS=1000` (`agent.rs:83`, `reply_stream.rs:140-143`), grind cap (`agent.rs:85-90`), subagent 25 turns, hook timeout 30 s (`hooks/mod.rs:39`), Langfuse batch 4096, `OUTPUT_LIMIT_BYTES`. No $ budget cap; no delegation-depth cap (→ LLM-GSL-004).
30. Elicitation/action-required: session-bound claim, timeouts fail closed (`action_required_manager.rs:1-330`).
31. Classification client debug log carries `has_token` only.

### Security (Node/TS + repo posture) — held

- Electron hardening held across the board: `webPreferences`, Fuses, CSP, preload shape, shell IPC shapes/caps, git IPC, file IPC containment (except SEC-GSL-201), settings allowlist, session import/archive, ACP secret transport via WS subprotocol + `ct_eq`, TLS TOFU pin, updater semver gating, localStorage prefs only, redaction helpers, `randomBytes(32)`.
- SECN-001..022 mostly non-finding: no `eval`/dynamic import/SQL/JWT/cookies; argv-array spawns; allowlist fetch; lockfiles + dependabot + pnpm allowBuilds; OIDC publish.
- RST/RSP held: all workflow action uses SHA-pinned; dependabot-auto-merge is the safe pattern; publish-npm/docker hardened; canary documented; quarantine checks out base.
- AGENTS.md rule check satisfied: `ui/desktop/src/api` absent at HEAD (removed `b83b194da`); `@hey-api/openapi-ts` only in `ui/sdk` with a CI freshness gate.
- `web_scrape` SSRF guard solid: `computercontroller/mod.rs:1816-1831,1898-1990`; residual DNS-rebinding TOCTOU documented at `:1907-1910` (stub, Appendix B).
- Extension child processes held: `child_process.rs:17-84` (`env_clear` + allowlist).
- Delegation/summon held except LLM-GSL-004: no nested delegation, Auto-only parent, timeouts, semaphore, untrusted repo agent-file policies dropped, working_dir confined, max_turns bounded.
- Autovisualiser render handlers: pure data → `structuredContent`.
- No commented-out blocks / `todo!()` outside tests (single TODO marker `canonical.rs:35`).
- Current docs path consistency held (30+ references resolve); hardcoded user paths only in test fixtures/historical docs; `Paths` centralized (`config/paths.rs:8-45`).
- Recipe/schedule/gateway absence consistent with README claims.

### Data I/O + integrity — held

- Traversal, redaction egress, formula export (no CSV writer; XLSX writes literal strings), import strictness (strict importer `import_formats/mod.rs:112-122`), dedup, provenance, migrations v1..38 (ladder single-tx), CLI/ACP/UI parity.
- Output-revision read authorization held: per-path registration, batch cap 500, optimistic concurrency, 8 MiB cap, `O_NOFOLLOW`, phantom-write guard (`output_revisions_storage.rs:414-757`).
- `delete_session` DOES cascade `session_artifacts` (FK `ON DELETE CASCADE`, `schema.rs:380`, `pool_lifecycle.rs:85`).
- Config-file migrations non-destructive with idempotence tests.
- `docx_tool.rs:125` write path CLEARED: `resolve_document_path` confines (`mod.rs:1842-1883`; tests reject `/etc/passwd` and traversal).

### State/concurrency/temporal/cascade — held (26 checks, highlights)

- Lease optimistic acquire (`session_leases.rs` full); turn closure triggers (`turn_closure.rs`); `SessionOperationGate` (`active_runs.rs`); plan CAS transitions (`plan_storage.rs:547-1027`); stale-clone plan import (`plan_storage.rs:1198-1420`); plan source-hash cache triggers; tool-operation ledger (`tool_operations.rs` full: begin/replay/in-doubt/complete CAS/recover live-peer-aware/cancel_undispatched); import dedup atomicity (`session_transfer.rs:180-200`, provenance in `extension_data`); import authority stripping (`session_transfer.rs` full: auth key/extensions/extras stripped, `history_trusted=false`, restrict+Approve forced); migration ladder single-tx (`migrations.rs`); config write path flock RMW (`base.rs:640-960,1023-1160`); `PermissionManager` sidecar lock (`config/permission.rs`); elicitation manager; guarded output restore; delete-session cascade (only `output_revisions` escapes → DAT-GSL-004); session CRUD guards (`session_crud.rs:1-460`); message storage; provider retry core (`retry.rs` full: `transient_only` forced); extension manager lifecycle; tools-cache versioning (`tool_catalog.rs:99-122,166-169`); desktop lease registry; `goslingServe` lifecycle (SIGTERM group → 5 s grace → SIGKILL → 10 s deadline); `sessionRecoveryRegistry.ts`; hooks steering; CLI builder (`builder.rs:372-468`); timestamps (SQLite `CURRENT_TIMESTAMP` UTC display-only; lease `unix_timestamp` `SystemTime`-based, fence-mitigated).
- Prior in-code findings observed FIXED (not re-filed): CON-GSL-001/REL-GSL-006 (`tool_operations.rs:405-425`), CON-GSL-002 (`base.rs:803-830`), CAS-GSL-001/REL-GSL-012 (`retry.rs:44-60`), DAT-GSL-005 (`pool_lifecycle.rs`), SEC-GOS-001 (`goslingServe.ts`).

### Architecture/invariant-sync — held (20+ checks)

- ACP contract parity: 145 `#[request]` in `gosling-sdk-types` == 145 `#[custom_method]` in `custom_dispatch.rs`; 144 `_gosling/*` + `session/delete` standard; 2 notifications; `acp-meta.json` (145) matches; `ui/sdk/src/generated` in sync; drift guard `check-acp-schema` (`justfile:161-165`); generator `generate_acp_schema.rs`.
- Desktop uses `@repo-makeover/gosling-sdk`; no `ui/desktop/src/api`; no OpenAPI imports (AGENTS.md rule held).
- Electron renderer isolation held (`main.ts:1254-1261` etc.); fs-touching main modules not reachable from renderer (type-only imports).
- Ink TUI sampled vs AGENTS.md rules — held (5 components sampled; gaps filed as WFG-GSL-001/002, FSR-GSL-003).
- INV-001..009 cleared with citations (`service.rs:1040`, `:425-438`, `:397-423`, `:507-583`; `store.rs` temp+rename/0o700/0o600/schema_version; `WorkspaceContext` localStorage; `normalize_workspace_path`; migrations v1→38). INV-010 letter holds; rationale tension filed as ARC-GSL-003.
- Seam catalog cleared: agent↔workspace seam, domain purity (no sqlx/keyring/etc in `workspace/*.rs`), sessions.db single-owner fs2 lock (`pool_lifecycle.rs:25,52-86`), README security claims verified, 0 broken relative doc links.

### Reliability / failsafe family — held

- 11 README/SECURITY hardening claims verified in code (0o600 secrets, 0o700 session dir, permission denials, atomic writes, compaction, doctor, memories, provider transitions, ACP auth, git clone hardening, fail-closed inspection). SECURITY.md advisory-only. No overclaim.
- Failsafe readiness scorecard: session storage Strong; provider HTTP Strong; ACP transport Strong; extension processes Strong w/ gap (FSR-GSL-001); plugin install Strong w/ gap (FSR-GSL-002); desktop UI state Strong (WFG-GSL-003 gap); TUI adequate w/ gaps (WFG-GSL-001/002, FSR-GSL-003); temp files adequate w/ gaps; spawned processes Strong.
- DEP inventory: keyring (High, file fallback), sessions.db (High, WAL+lock+leases), provider endpoints (High, retry), ACP transport (High), node/npx/uvx/jbang shims (Medium), docker CLI (Medium), git binary (Medium), network egress (High, `PermissionManager` `egress_domain`), Cargo pins recent; lodash `^4.18.1` unverifiable offline (stub to DEP maintenance).
- REC inventory: workspace store sound; message storage sound; output revisions sound w/ DAT-GSL-001 gap; MCP spill IOP-GSL-001; plugin install sound w/ FSR-GSL-002; extension tempdirs gap FSR-GSL-001; OAuth removal gap REL-GSL-001; turn closure/cancellation sound.
- SIG inventory: tracing warn/error good; provider retry notices surfaced; backend lifecycle good; healthcheck good; desktop errors good; gaps = REL-GSL-001, FSR-GSL-001/002.
- Prior-cycle fixes verified present: REL-GSL-005/006, MEM-GSL-003, RES-GSL-002, DAT-GSL-005, SEC-GOS-001.

### Falsified subagent claim (recorded, not filed)

- **"Provider `Retry-After` honored uncapped"** (data-flow observation (c), `retry.rs:224-246`): FALSIFIED by lead verification. `Retry-After` is capped at 3600 s: `crates/gosling-providers/src/http_status.rs:43` (`MAX_RETRY_AFTER_SECS`), `:74` (`secs.min(...)`), bounded additionally by `max_retries`. Not a finding.

## Break-It Review

How each main workflow was attacked (malformed, cross-scope, duplicate, concurrent, stale, direct API/CLI bypass, partial failure, downstream misuse) and what survived. HELD = boundary held under the attack; BREAKS = filed finding.

| Workflow | Attack(s) applied | Result |
|---|---|---|
| Turn lease / mutual exclusion | concurrent turns on one session; stale lease | HELD (`session_leases.rs`) |
| Plan approval gate | direct API bypass; stale clone import | HELD (`plan_storage.rs:547-1027,1198-1420`) |
| Tool dispatch | duplicate/replayed dispatch; in-doubt recovery | HELD (`tool_operations.rs:73-190`) |
| Session import | hostile import (authority transfer, dedup) | HELD (`session_transfer.rs`) |
| Config write | two writers (lost update) | HELD (`base.rs` flock RMW) |
| Permission denial persistence | deny then rerun; cross-process | HELD (`config/permission.rs`) |
| Migration ladder | replay; partial application | HELD (`migrations.rs` single-tx) |
| Provider retry | flaky endpoint; retry storm; Retry-After abuse | HELD (`retry.rs` `transient_only`; cap verified at `http_status.rs:43,74` — uncapped claim falsified) |
| Output restore | resurrect rejected plan/permission state; path escape | HELD (file bytes only; canonical path, `O_NOFOLLOW`, 8 MiB cap) |
| Session deletion | delete then inspect DB; cascade completeness | BREAKS (DAT-GSL-004 / DAT-GSL-002: `output_revisions` survives, path-keyed) |
| Handoff status update | direct overwrite from wrong status | BREAKS (STT-GSL-001: no from-status precondition) |
| Archive semantics | mutate archived session; resume | BREAKS-partial (STT-GSL-002: narrow gate; CLI auto-unarchive) |
| Backend process registry | two Electron instances RMW | BREAKS (CON-GSL-001: in-process queue only) |
| Renderer fs grant | self-grant sensitive dirs; duplicate grants | BREAKS (SEC-GSL-201) |
| CI secret exposure | same-repo PR exfiltration; bot-branch push | BREAKS (SECN-GSL-201, SEC-GSL-204) |
| Credential substitution | injection-steered call to attacker destination | BREAKS (SEC-GSL-001) |
| Extension/tool install | model-proposed first call triggers install | BREAKS (IOP-GSL-005: mutable tap, no consent) |
| MCP response spill | long-running session; crash mid-write | BREAKS (IOP-GSL-001: never cleaned, no fsync) |
| Workspace creation | concurrent creates; user edits `.gitignore` | BREAKS (ARC-GSL-003: non-atomic RMW, divergent checks) |
| OAuth logout | removal failure under lock/AV | BREAKS (REL-GSL-001: silent false success) |
| TUI rendering | narrow terminal; many permission options | BREAKS (WFG-GSL-001/002: overflow, not clipped by Ink) |
| Message edit | lost truncate response | BREAKS (WFG-GSL-003: permanent divergence) |

## Recommended Patch Order

1. **High, now (before next release / further fork-PR merges):**
   - **SEC-GSL-001** — destination-origin binding for website-login password substitution + inspector shows real destination. This is the credential-exfiltration primitive; patch first.
   - **SEC-GSL-201** — user-mediation + sensitive-dir deny list for renderer directory grants.
   - **SECN-GSL-201** — gate `pr-smoke-test.yml` (environment/label approval or move live tests to `push`), **and rotate the 10 exposed secrets immediately** — rotation is independent of the code patch and the exposure window is unbounded while the workflow is unchanged. Review the other workflows for the same secret set (SECN-GSL-203 drops 6 keys trivially).
2. **Medium guardrails (next patch batch):**
   - Scope/validation guardrails: **IOP-GSL-002** (path containment), **IOP-GSL-005** (no auto-install from mutable tap; consent + pin), **DAT-GSL-004** (delete completeness for `output_revisions`), **REL-GSL-001** (honest OAuth removal), **LLM-GSL-004** (policy-bounded delegation + depth cap), **LLM-GSL-001** (sanitize judge input), **ARC-GSL-006** (orchestrator confinement parity), **ARC-GSL-003** (disclose `.gitignore` mutation; single ignore implementation; atomic write).
   - CI posture: **SECN-GSL-203** (drop 6 keys — XS), **SECN-GSL-204** (pin + review gate), **SECN-GSL-202** (isolate CI agent input; no persistent creds; review-gated output), **IOP-GSL-001** (spill-file cleanup + sweep).
3. **Regression tests** for every P1/P2 fix (see Regression Test Strategy).
4. **Low reliability/status-truthfulness batch:** STT-GSL-001, STT-GSL-002, STT-GSL-003, CON-GSL-001, DAT-GSL-001/002/003, IOP-GSL-003/004, REL-adjacent Lows (SEC-GSL-002/003, LLM-GSL-002/003, SEC-GSL-202/203/204, SECN-GSL-205, ARC-GSL-005/006), WFG-GSL-001/002/003, FSR-GSL-001/002/003, OBS-GSL-001..004.
5. **Documentation/status updates:** ARC-GSL-001/002/004, CMP-GSL-001/002, NEG-GSL-001, plus the doc corrections implied by SEC-GSL-001/003 (`website-logins.md`, `remote-gosling-server.md`).

## Regression Test Strategy

| Test | Purpose | Finding |
|---|---|---|
| Mismatched-destination substitution refused; prompt shows destination | assert the origin-binding boundary | SEC-GSL-001 |
| Renderer grant of `~/.ssh`/`~/.aws`/`/etc` denied or requires user gesture | assert sensitive-path deny + mediation | SEC-GSL-201 |
| Same-repo PR without approval → secret-bearing job skipped; fork PR skipped | assert CI gating | SECN-GSL-201 |
| Judge receives sanitized summary; injection-flavored args do not flip verdict | assert judge input isolation | LLM-GSL-001 |
| Delegate without role policy → clamped/denied; depth > cap → refused | assert delegation bounds | LLM-GSL-004 |
| Turn completes → spill file removed; crash → startup sweep removes orphans | assert spill lifecycle | IOP-GSL-001 |
| Missing Peekaboo + no consent → no `brew install` on tool-call path | assert no implicit install | IOP-GSL-005 |
| Delete session → `output_revisions` rows for its paths gone (or documented retention + purge) | assert deletion completeness | DAT-GSL-004 |
| Confirmation copy mentions `.gitignore`; concurrent creates → no lost/duplicate entries; Rust/TS parity | assert disclosure + atomicity + parity | ARC-GSL-003 |
| CI agent with hostile repo-derived input → no direct protected-branch write | assert untrusted-input isolation | SECN-GSL-202 |
| Version-bump job env contains zero `secrets.*` | assert least-privilege env | SECN-GSL-203 |
| Unpinned upstream ref → publish step fails | assert pin gate | SECN-GSL-204 |
| `remove_file` failure → `cleanup()` returns `Err`; CLI shows no false success | assert status truthfulness | REL-GSL-001 |
| Warn logs for secret-bearing calls contain no secret substring | assert log redaction | SEC-GSL-002 |
| `GET /health` without token → 401 when auth enabled | assert uniform route auth | SEC-GSL-003 |
| Classifier/OTLP export bodies exclude raw user text (redaction on) | assert egress redaction | LLM-GSL-002/003 |
| Handoff transition from wrong status → conflict, no row change | assert preconditioned transition | STT-GSL-001 |
| Mutation matrix against archived sessions matches documented semantics | assert freeze semantics | STT-GSL-002 |
| Two-writer registry interleaving → no lost record | assert cross-process RMW | CON-GSL-001 |
| Session B listing session A's path → empty; delete cascades revisions | assert scoping + integrity | DAT-GSL-002/004 |
| Link `/etc/passwd` → refused; malformed `.jsonl` → dropped-line count reported; bomb fixture aborts at cap | assert IOP boundaries | IOP-GSL-002/003/004 |
| Crash between revision commit and rename → startup reconciliation | assert ordering + recovery | DAT-GSL-001 |
| Malformed workspace store → `.corrupt-*` preserved, error surfaced | assert fail-visible config | DAT-GSL-003 |
| Lint: no `${{ }}` in `run:`; no `secrets:inherit`; no `npx --yes pkg@latest`; no secrets in `cargo update` jobs | assert CI hygiene | SEC-GSL-202/203/204, SECN-GSL-205 |
| Doc checks: module table resolves; README version == workspace version; schema version == `CURRENT_SCHEMA_VERSION` | assert docs-match-source | ARC-GSL-001/002/004 |
| Orchestrator `start_agent` outside confinement → refused; foreign `view_session` → refused | assert confinement parity | ARC-GSL-006 |
| TUI width sweep: DiffViewer and PermissionPrompt content ≤ container | assert Ink fit rules | WFG-GSL-001/002 |
| Lost truncate response → next sync restores backend truth | assert reconcilable divergence | WFG-GSL-003 |
| kill -9 extension → next startup sweeps its tempdir; Windows abandoned staging dir removed or logged | assert crash-safe cleanup | FSR-GSL-001/002 |
| Render 10k markdown chunks → cache size ≤ cap | assert bounded memory | FSR-GSL-003 |
| Migration v24 preserves `restrict_tools_to_working_dirs` | assert security-preserving migration | STT-GSL-003 |

## Deferred Risks

Plausible/speculative items not recommended for the immediate patch, plus surfaces outside this run's authority:

- **GitHub-side settings Not Reviewed** (needs API access): branch protection rules, environment protection rules, the full secret inventory and rotation status, collaborator/membership list. SECN-GSL-201's rotation recommendation assumes owner action here.
- **Secret rotation for SECN-GSL-201's 10 secrets**: urgent but a human-owner action; the workflow patch does not rotate anything by itself.
- **Renderer XSS / markdown sink** (sec-llm stub LLM-014 sink side): only grep-sampled; needs a renderer-lens Playwright pass. It is the natural exploit partner for SEC-GSL-201 — prioritize after the P1s.
- **MCP-app proxy guest-content sandboxing**: inverse of SEC-GSL-201; dedicated renderer-lens pass proposed.
- **Per-provider secret handling in `gosling-providers`**: sampled only (retry, OAuth, HTTP status audited; the rest not).
- **`gosling-mcp` screen-control extensions** (computercontroller beyond the Peekaboo path, autovisualiser): partially audited; `automation_script` executes agent-supplied scripts by design (`computercontroller/mod.rs:802-900`, destructive/open-world hints) — product-surface decision, extension disabled by default.
- **Nostr relay transport**: redaction verified; transport not audited.
- **ui/text Ink TUI**: sampled (5 components + the three filed WFG/FSR findings); a full component sweep is a separate pass.
- **pnpm-workspace `allowBuilds` semantics**: unverifiable offline (sec-node validation limit).
- **lodash `^4.18.1`** (`ui/desktop/package.json`): version unverifiable offline; stubbed to DEP maintenance.
- **CSEC-074 / deep ReDoS screening**: screening-depth only (sec-node validation limit).
- **`.hermit` vendored crates**: excluded from sweeps; `cargo-deny` lacks bans/licenses checks (sec-node stub).
- **`configReadAll` returns plaintext config values to the renderer** (masked only when `is_secret`): if users store provider keys as plain config values they reach the renderer — Info posture note for a follow-up secrets-handling pass (sec-node stub).
- **Runtime Goose/AAIF catalog fallback** (skills/extensions discovery): supply-chain sibling surface; SECN-GSL-204 covers the docs-site manifest only (AGENTS.md: deterministic Goose compatibility adapter — see `documentation/GOOSE_COMPATIBILITY.md`).
- **docs/build — 37 tracked generated files** and the checked-in Mach-O binary `ui/desktop/src/bin/gosling`: repo-hygiene notes (CMP), not patched here.
- **TMP-009 / CAS-015** (mixed-era report rendering, alert noise): out of budget for the data-flow pass; candidates for a round-2 if wanted.
- **No $ budget cap** on LLM spend (turn caps exist; cost caps don't): product decision, noted by sec-llm.

## Validation Limits

- **Nothing was executed.** No `cargo build/test/clippy/fmt`, no `pnpm`/vitest/typecheck, no network calls, no servers, no installs. This was a read-only static audit per the operator's constraint (AGENTS.md: run builds/tests only when asked). The report is therefore strong on code properties and deliberately capped on runtime manifestations.
- **Confidence calibration applied** (`confidence_calibration.md` v3.1): static code properties marked Confirmed; races, OOM, install behavior, exfiltration chains, and other runtime-dependent outcomes capped at Likely/Plausible. No finding claims `runtime-observed` or `test-reproduced` evidence.
- **Lead verification**: every High/Medium finding's citations were re-read and confirmed line-by-line. One subagent observation was **falsified** and excluded (Retry-After uncapped → actually capped at 3600 s, `http_status.rs:43,74`) — recorded under Non-Findings.
- **Large files read in targeted slices**: `session_manager.rs` (7342 lines), `agents/agent/` (~12.6k lines), `config/base.rs`, `working_dir_scope_inspector.rs` (2670 lines), `otel/otlp.rs` (1001 lines), `ui/desktop` renderer markdown sink sampled only, `ui/text` not fully audited, `gosling-providers` sampled, `gosling-mcp` screen tools partially deep, Nostr transport not audited, ADRs 0010–0023 skimmed by grep.
- **Path corrections applied during merge**: canonical paths are `crates/gosling/src/session/session_manager/` (session_crud, migrations, pool_lifecycle, legacy_import), `crates/gosling/src/session/legacy.rs`, `crates/gosling/src/agents/agent/hooks.rs`. Earlier drafts that cited shorter paths were normalized.
- **Prior-ID collisions checked** (grep-based) against SEC-GSL-001..005/101..103/901..902, SECN-GSL-001..007, SEC-GOS-001..013, RES/ARCN/MEM/RST/RSP-GSL-*, GSL-PT-*; this report's ID series (SEC-GSL-2xx, SECN-GSL-2xx, and the domain series) avoids them. Prior-cycle fixes verified present and not re-filed (see Non-Findings).
- **GitHub-side settings, secret rotation status, and branch protection**: Not Reviewed (no API access).
- **Playtest**: excluded per operator instruction; no UI was driven, screenshotted, or exercised.

## Final Confidence

- **Static/code-property findings: High confidence.** All 45 findings are source-evidenced at the property level; High/Medium citations were re-verified by the lead. Held non-findings carry the line that makes them safe.
- **Runtime manifestations: Medium confidence.** Exfiltration chains (SEC-GSL-001), races (CON-GSL-001, ARC-GSL-003 RMW), OOM (FSR-GSL-003), install behavior (IOP-GSL-005), and accumulation effects (IOP-GSL-001, FSR-GSL-002) are reasoned, not reproduced — per the calibration ceiling they stay Likely/Plausible.
- **Coverage: High for the applied lenses** (all 13 applicable lenses + 21 applicable standalones executed by 7 subagent groups); **Medium overall** because the playtest was excluded, GitHub-side settings were not reviewed, and several surfaces (providers beyond retry/OAuth, Nostr transport, full TUI, renderer markdown sink) were sampled rather than exhaustively audited.

## Skill Escalation

Per `cross_lens_escalation.md` v3.1 — secondary lenses so patches add the correct guardrails.

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| SEC-GSL-001 | Security | Data-Integrity (Cascade) | Substituted credentials flow into tool args, logs (SEC-GSL-002), and judge input (LLM-GSL-001); exfil output becomes downstream evidence |
| SEC-GSL-201 | Security | Workflow-GUI | Grant UX/confirmation; pairs with renderer markdown sink (deferred Playwright pass) |
| SECN-GSL-201 | Security (repo posture) | Compliance-Posture | Secret-rotation + environment policy are governance actions |
| LLM-GSL-001 | Security (LLM) | Reliability | Judge availability/latency affects approval truthfulness |
| LLM-GSL-004 | Security (LLM) | Architecture-Seam | Delegation policy spans summon/delegate/orchestrator seams |
| IOP-GSL-001 | Input-Output-Path | Failsafe (resource-lifecycle) | Temp-file lifecycle ownership; same family as FSR-GSL-001/002 |
| IOP-GSL-005 | Input-Output-Path | Security | Mutable-tap install is a supply-chain/privilege issue; Auto mode removes consent |
| DAT-GSL-004 | Data-Integrity | Compliance-Posture | Deletion/retention posture; interacts with DAT-GSL-002 keying |
| ARC-GSL-003 | Architecture | Workflow-GUI + Reliability | Disclosure at confirmation (UI truthfulness); atomic RMW (reliability) |
| SECN-GSL-202 | Security (repo posture) | Cascade | Agent output becomes repo/docs authority |
| SECN-GSL-203 | Security (repo posture) | Reliability | Unpinned resolution also breaks reproducible builds |
| SECN-GSL-204 | Security (repo posture) | Cascade | Published manifest becomes authority for skill consumers |
| REL-GSL-001 | Reliability | Security | Stale OAuth token at rest is a secret-residue issue |
| SEC-GSL-002 | Security | Compliance-Posture | Log hygiene vs INV-003; diagnostics shipping |
| SEC-GSL-003 | Security | Compliance-Posture | Doc claims a control that is missing |
| LLM-GSL-002 | Security (LLM) | Compliance-Posture | Undisclosed egress scope at opt-in |
| LLM-GSL-003 | Security (LLM) | Compliance-Posture | Undisclosed telemetry scope at opt-in |
| STT-GSL-001 | State-Transition | Concurrency | Precondition matters once racy callers exist |
| STT-GSL-002 | State-Transition | Workflow-GUI | CLI auto-unarchive is a status-truthfulness issue |
| CON-GSL-001 | Concurrency | Reliability | Lost updates degrade orphan cleanup |
| DAT-GSL-002 | Data-Integrity | Temporal | Version chaining across sessions is a freshness/provenance issue |
| IOP-GSL-002 | Input-Output-Path | Security | Arbitrary host-file read into prompts (injection + exfil surface) |
| IOP-GSL-003 | Input-Output-Path | Data-Integrity | Silent truncation at an import boundary |
| IOP-GSL-004 | Input-Output-Path | Reliability | Resource-exhaustion DoS |
| DAT-GSL-001 | Data-Integrity | Reliability | Crash-ordering recovery |
| DAT-GSL-003 | Data-Integrity | Workflow-GUI | Silent reset is a status-truthfulness issue |
| SEC-GSL-202 | Security (repo posture) | Compliance-Posture | Latent anti-pattern; lint-guardrail |
| SECN-GSL-205 | Security (repo posture) | Reliability | Unpinned tool also breaks reproducibility |
| SEC-GSL-203 | Security (repo posture) | Compliance-Posture | Defense-in-depth erosion in release plumbing |
| SEC-GSL-204 | Security (repo posture) | Compliance-Posture | Gate-quality issue (author vs branch check) |
| ARC-GSL-001 | Architecture-Seam | Compliance-Posture | Doc drift in the module contract |
| ARC-GSL-002 | Architecture-Seam | Compliance-Posture | Version-claim drift; release-process gap |
| ARC-GSL-005 | Architecture-Seam | Negative-Space + Security | Dead store would write world-readable records if revived |
| STT-GSL-003 | State-Transition | Security | Silent security-flag downgrade at migration |
| ARC-GSL-006 | Architecture-Seam | Security | Unconfined Auto agents + cross-session reads |
| CMP-GSL-001 | Compliance-Posture | Temporal | Staleness/freshness of advisory metadata |
| CMP-GSL-002 | Compliance-Posture | Negative-Space | Doc describes a removed feature as live |
| WFG-GSL-001 | Workflow-GUI | Reliability | Layout corruption under narrow terminals |
| WFG-GSL-002 | Workflow-GUI | Security | Trust-boundary prompt rendering integrity |
| WFG-GSL-003 | Workflow-GUI | Data-Integrity | UI/backend divergence after partial failure |
| FSR-GSL-001 | Failsafe | Security | Secret residue in shared temp dirs |
| FSR-GSL-002 | Failsafe | Reliability | Unbounded disk growth |
| FSR-GSL-003 | Failsafe | Reliability | Unbounded memory growth |
| ARC-GSL-004 | Architecture-Seam | Compliance-Posture | Undocumented schema version |
| NEG-GSL-001 | Negative-Space | Reliability | Dead prop + unreachable branches |
| OBS-GSL-001 | Reliability | Data-Integrity | Durability of atomic writes |
| OBS-GSL-002 | Reliability | Concurrency | Port-reuse race window |
| OBS-GSL-003 | Reliability | State-Transition | Delete ordering vs agent stop |
| OBS-GSL-004 | Security (policy) | State-Transition | Policy check skipped on hook lifecycle transitions |

## Coverage Matrix

### Lenses (13/13 applicable applied)

| Lens | Applied by | Result |
|---|---|---|
| Security — code (Rust) | sec-llm group (security-code + parts) | SEC-GSL-001/002/003; hardening claims verified |
| Security — LLM | sec-llm group (security-llm) | LLM-GSL-001/002/003/004; LLM-008/009 N/A (no RAG/training); LLM-002 Not Reviewed (model-dependent) |
| Security — Node/TS | sec-node group (security-nodejs) | SEC-GSL-201; SECN-001..022 mostly held |
| Security — repo triage + posture | sec-node group (security-repo-triage, security-repo-posture) | SECN-GSL-201..205, SEC-GSL-202/203/204; RST/RSP held |
| Input-Output-Path (dataflow IOP) | data-io + ops groups | IOP-GSL-001..005 |
| State-Transition | data-flow + ops groups | STT-GSL-001/002/003 |
| Data-Integrity | data-io + ops groups | DAT-GSL-001..004 |
| Reliability | rel group | REL-GSL-001 + failsafe family |
| Architecture-Seam | arch + ops groups | ARC-GSL-001..006 |
| Workflow-GUI | rel group | WFG-GSL-001/002/003 |
| Concurrency | data-flow group | CON-GSL-001 |
| Cascade | data-flow group | cleared (no unrepaired cascade found beyond filed findings) |
| Temporal | data-flow group | merged (TMP-GSL-001 → DAT-GSL-002 alias) |
| Negative-Space | rel group | NEG-GSL-001; NEG-GSL-002 → ARC-GSL-005 alias |
| Compliance-Posture | rel + ops groups | CMP-GSL-001/002; zero-finding lens for README/SECURITY claims |
| Invariant-Sync | arch group | INV-001..010 verified; ARC-GSL-003 (INV-010 tension) |

### Standalones (21 applied / 11 not applicable / 5 catalog-excluded, of 37)

| Standalone | Status | Notes |
|---|---|---|
| security-code | applied | sec-node + sec-llm groups |
| security-llm | applied | sec-llm group |
| security-nodejs | applied | sec-node group |
| security-repo-triage | applied | sec-node group (RST x8) |
| security-repo-posture | applied | sec-node + ops groups (RSP x8; CMP) |
| agent-orchestration-code | applied | ops group (orchestrator → ARC-GSL-006; delegation → LLM-GSL-004) |
| architecture-drift | applied | arch group (ARC-GSL-001/002/004) |
| architecture-nodejs | applied | arch group (desktop/TUI seams held) |
| contract-internalapi | applied | arch group (ACP parity 145/145 held) |
| dataflow-pipeline-graph | applied | ops group |
| deadcode-cleanup | applied | ops + rel groups (ARC-GSL-005; NEG-GSL-001; 14 `#[allow(dead_code)]` sites reviewed) |
| dependency-criticality | applied | rel group (DEP inventory) |
| failsafe-readiness | applied | rel group (FSR scorecard + FSR-GSL-001/002/003) |
| mcp-server | applied | ops group (mcp_app_proxy held; computercontroller → IOP-GSL-005) |
| memory-lifecycle | applied | rel group (FSR-GSL-003; desktop bounded) |
| operator-signal | applied | rel group (SIG inventory) |
| pipeline-externalapi | applied | ops group (provider HTTP/retry held) |
| recovery-idempotency | applied | rel group (REC inventory) + data-flow (tool ledger held) |
| repo-path-consistency | applied | ops group (30+ refs resolve; `.giles` foreign path → CMP-GSL-001) |
| repo-state-reconciliation | applied | ops group (git-tracked generated artifacts noted) |
| resource-lifecycle | applied | rel group (FSR-GSL-001/002; child processes sound) |
| playtest-app | **not applied** | operator exclusion (explicit) |
| security-owasp | not applicable | no web-server attack-surface claim beyond ACP (covered by security-code) |
| contract-crossrepo | not applicable | single-repo audit; ACP contract checked internally instead |
| optimization-opportunities | not applicable | audit screening, not a perf engagement |
| testsuite-economics | not applicable | no test-suite changes proposed in a read-only audit |
| llm-config-drift | not applicable | LLM-002 model-dependent (Not Reviewed); no config-drift claim to audit |
| performance-profile | not applicable | no profiling in a read-only audit |
| security-vuln-harness | not applicable | requires authorized drills (not authorized this run) |
| design-webapp | not applicable | no web UI design work in scope |
| design-handheld | not applicable | no handheld UI in scope |
| multiagent-consensus | not applicable | single-lead merge used; no consensus round needed |
| equation-sourcebase | catalog-excluded | not applicable to this repo's domain |
| security-supabase | catalog-excluded | no Supabase in this repo |
| flutter-ios | catalog-excluded | no Flutter/iOS in this repo |
| graphdb-design | catalog-excluded | no graph DB in this repo |
| go-repo-hardening | catalog-excluded | not a Go repo |

## Appendix A — Subagent roster, budgets, and merge decisions

| Group | Lenses/standalones owned | Findings emitted | Budget notes |
|---|---|---|---|
| audit-sec-llm | security-code (Rust), security-llm, SEC lens | 7 (SEC-GSL-001/002/003, LLM-GSL-001/002/003/004) | targeted slices of large files; 31 held non-findings; runtime capped |
| audit-sec-node | security-nodejs, security-code (TS), security-repo-triage, security-repo-posture | 9 (SEC-GSL-201..204, SECN-GSL-201..205) | all 15 workflows read; Electron surface swept |
| audit-data-io | dataflow-input-output, dataflow-integrity | 7 (IOP-GSL-001..004, DAT-GSL-001..003) | storage modules via DDL/FK greps + targeted reads |
| audit-data-flow | State-Transition, Concurrency, Cascade, Temporal | 4 (STT-GSL-001/002, CON-GSL-001, TMP-GSL-001) + 5 info observations | 26 held checks; break-it review; Retry-After claim later falsified by lead |
| audit-arch | architecture-seam, architecture-drift, contract-internalapi, architecture-nodejs, invariant-sync | 4 (ARC-GSL-001..004) + routed stubs | 20+ cleared checks; ACP parity verified 145/145 |
| audit-ops | mcp-server, agent-orchestration-code, pipeline-externalapi, deadcode-cleanup, repo-state-reconciliation, repo-path-consistency, dataflow-pipeline-graph | 7 (IOP-GSL-005, ARC-GSL-005/006 [renumbered], STT-GSL-003, DAT-GSL-004, CMP-GSL-001/002) | 18 held non-findings |
| audit-rel | reliability, workflow-gui, negative-space, compliance-posture, failsafe family (DEP/REC/SIG, memory-lifecycle, resource-lifecycle) | 9 (REL-GSL-001, WFG-GSL-001/002/003, NEG-GSL-001/002, FSR-GSL-001/002/003) | CMP lens zero findings; 11 hardening claims verified |

**Merge / dedup decisions (lead):**

- **TMP-GSL-001 → DAT-GSL-002** (alias): same `output_revisions` path-keyed table, same gap (no session FK; never purged on delete). Reported once as DAT-GSL-002; the deletion-retention consequence is filed separately as DAT-GSL-004 (distinct mechanism: deletion-intent violation vs path-keyed design/cross-session disclosure).
- **NEG-GSL-002 → ARC-GSL-005** (alias): both flag the dead `ToolPermissionStore`. Reported once as ARC-GSL-005 (deadcode-cleanup owns it); the latent 0o600/fsync note from NEG-GSL-002 is folded in.
- **ops ARC-GSL-001/002 renumbered → ARC-GSL-005/006**: ops initially used ARC-GSL-001 (dead store) and ARC-GSL-002 (orchestrator), colliding with arch group's ARC-GSL-001 (module table) and ARC-GSL-002 (README version). Renumbered to ARC-GSL-005/006 in the merged report.
- **arch "DiffViewer stub" = WFG-GSL-001**: same finding (ui/text DiffViewer overflow); filed once under the WFG lens.
- **Subagent ID series chosen to avoid prior-audit collisions** (SEC-GSL-2xx, SECN-GSL-2xx); verified by grep against prior in-repo audit IDs.

## Appendix B — Cross-lens stubs and dispositions

| Stub (origin group) | Disposition |
|---|---|
| Renderer XSS / markdown sink (sec-llm, LLM-014 sink side) | grep-sampled only → deferred renderer-lens Playwright pass (Deferred Risks); exploit partner for SEC-GSL-201 |
| Per-provider secret handling (sec-llm) | sampled only → Deferred Risks |
| gosling-mcp screen-control extensions (sec-llm) | computercontroller audited via ops (IOP-GSL-005 filed); `automation_script` agent-supplied scripts noted as product surface (Deferred Risks) |
| Nostr relay transport (sec-llm) | redaction verified; transport → Deferred Risks |
| ui/text Ink TUI (sec-llm) | audited via rel group (WFG-GSL-001/002, FSR-GSL-003, NEG-GSL-001); full sweep deferred |
| Docs-vs-code contradiction sweep (sec-llm) | two instances found (SEC-GSL-001, SEC-GSL-003); systematic sweep proposed (Draft Prompt Assessment) |
| `configReadAll` plaintext values to renderer (sec-node) | Info posture note → Deferred Risks (secrets-handling pass) |
| Runtime Goose/AAIF catalog fallback (sec-node) | supply-chain sibling surface → Deferred Risks; AGENTS.md Goose-compat adapter noted |
| MCP-app proxy guest-content sandboxing (sec-node) | inverse of SEC-GSL-201 → deferred renderer-lens pass |
| Branch protection / environments / rotation (sec-node) | Not Reviewed (GitHub API) → Deferred Risks |
| `.hermit` vendored; cargo-deny bans/licenses (sec-node) | Deferred Risks |
| migration v23 flips `restrict_tools_to_working_dirs=FALSE` (data-io) | filed as STT-GSL-003 |
| migration v30 DROPs tagteam tables (data-io) | resolved as deliberate (documented + regression test); doc staleness filed as CMP-GSL-002 |
| peekaboo auto `brew install` (data-io) | filed as IOP-GSL-005 |
| computercontroller `automation_script` (data-io, arch) | product surface, extension disabled by default → Deferred Risks |
| `docx_tool.rs:125` caller-supplied write path (data-io, arch) | CLEARED: `resolve_document_path` confines (`mod.rs:1842-1883`); tests reject `/etc/passwd` + traversal |
| output_revisions content BLOBs = secret residue (data-io) | folded into DAT-GSL-004 |
| ToolPermissionStore dead code (data-flow) | filed as ARC-GSL-005 (absorbs NEG-GSL-002) |
| Hook policy gap (data-flow) | filed as OBS-GSL-004 |
| Desktop↔backend secret plumbing (data-flow) | reviewed under sec-node (WS subprotocol + `ct_eq` held) |
| Output-revision cross-session chaining (data-flow) | filed as DAT-GSL-002 (absorbs TMP-GSL-001); ADR-0018 clarification suggested |
| INV-002 remaining ACP mutation handlers (data-flow) | sampled only; round-2 STT if wanted |
| Ink DiffViewer overflow (arch) | filed as WFG-GSL-001 |
| git-tracked generated artifacts; Mach-O binary (arch) | CMP notes → Deferred Risks |
| web_scrape DNS-rebinding residual (ops) | documented at `computercontroller/mod.rs:1907-1910`; not filed (documented residual, SSRF guard otherwise solid) |
| Peekaboo install security dimension (ops) | filed as IOP-GSL-005 (severity adjudicated Medium, macOS-only + disabled-by-default) |
| large_response_handler write-failure fallback inlines full content (ops) | same cleanup-ownership family as IOP-GSL-001; noted in its mitigation |
| `grantSessionDirectories` swallowed catch (rel, `BaseChat.tsx:253`) | folded into SEC-GSL-201 adjacent modes |
| Cleanup-deadline expiry leaves registry record (rel, `goslingServe.ts:738-742`) | adjacent to CON-GSL-001; noted |
| `provider.rs` `unwrap()` sites (rel) | assumption encoding; not filed (NEG stub) |
| `setSessionRecoveryActive`/`setWakelockActive` warn-only (rel) | not filed (bounded, error surfaced) |
| lodash `^4.18.1` (rel) | Deferred Risks (DEP maintenance) |

---

*End of report. Generated 2026-10-12 by Mistral Vibe (lead) with 7 subagent audit groups, per the agent-skills audit pack (`000_common/audit-base/` contracts, `010_audit/` lenses and standalones). Read-only audit; playtest excluded per operator. Session log: `docs/logs/session/2026-10-12-mistral-comprehensive-audit.md`.*


## Repair disposition addendum — 2026-10-12

The original audit above is retained as historical evidence. Current per-ID repairs, contract-based counterfindings, remaining work and validation limits are tracked in [the Mistral repair record](docs/logs/session/2026-10-12-mistral-findings-repair.md). The operator selected static-only validation and per-call saved-password approval with disclosure of external-tool trust. Source edits and static checks do not establish live CI execution, credential rotation or installed-app verification.
