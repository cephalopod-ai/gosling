# Gosling Exhaustive Audit — Master Report

**Date:** 2026-08-15  
**Target:** `/Users/eric/Work/vscode/forked/gosling` @ `073d19428509ea6eb317924b1856a1fe7e9002c8` (`main`, clean)  
**Authority:** audit-only / read-only for static lenses; `test_only` for playtest. Source was not modified.  
**Catalog:** `search_skills` audit-suite / exhaustive. Consensus overlay was not requested.

This report merges the 2026-08-15 lens reports and the live 110-card playtest. Per-lens files hold full inventories, non-findings, and validation limits. Historical `docs/cloud/99-master-report.md` (2026-07-05) is **not** current.

## 1. Scope and method

Required lenses run: architecture-seam, dataflow-cascade, compliance-posture, dataflow-concurrency, dataflow-integrity, dataflow-input-output, invariant-sync, negative-space, reliability, security, dataflow-state-transition, dataflow-temporal, workflow-gui.

Standalone applied: security-code, security-owasp, security-llm, security-vuln-harness, security-nodejs, security-repo-posture, security-repo-triage, deadcode-cleanup, pipeline-externalapi, dependency-criticality, failsafe-readiness, contract-internalapi, contract-crossrepo (limited), architecture-nodejs (limited), performance-profile, memory-lifecycle, resource-lifecycle, operator-signal, dataflow-pipeline-graph, recovery-idempotency, design-webapp, playtest-app, mcp-server (limited), agent-orchestration-code, repo-state-reconciliation, repo-path-consistency, architecture-drift (limited, ADR/docs as intent).

Excluded as N/A: `audit-graphdb-design` (marked not applicable), `audit-equation-sourcebase`, `audit-security-supabase`, `audit-flutter-ios`, `audit-go-repo-hardening`.

Playtest: existing `docs/test_scenarios/` library, all 110 cards, disposable `GOSLING_PATH_ROOT`, loopback OpenAI-compatible oracle. Desktop cards blocked (no GUI driver).

## 2. Ship-readiness verdict

At this HEAD the July 2025/2026 “default Auto + scanner off + plugin auto-trust” cluster is **largely stale**. Interactive default is **SmartApprove**; prompt-injection scanning defaults on; project plugins require trust; secrets writes are atomic; missing-session diagnostics fail closed; invalid typed settings warn.

The remaining ship-gating theme is **Auto islands plus operator-truth gaps**:

1. Subagents, plan-act, and headless paths still run in Auto and auto-approve tools that lack an explicit NeverAllow/AskBefore. Egress `RequireApproval` is downgraded there.
2. Repo `AGENTS.md` / hints still enter the prompt as additional instructions without a workspace-trust gate (plugins were tightened; instructions were not).
3. ACP still accepts the shared secret as `?token=`; the live MCP-app guest trusts a client-supplied CSP (goslingd does not).
4. Live playtest: a broken MCP stdio extension can hang `gosling run` with empty streams; non-TTY `session remove`/`--fork` dies with `not connected`; `gosling acp` can exit 0 with no handshake bytes.

Recommendation: treat Cluster A (Auto-child agency) and Cluster B (ACP token/CSP) as the current ship-gating pair. Playtest hangs and non-TTY session mutations are high-leverage CLI fixes.

## 3. Coverage matrix

| Skill | Verdict | Report |
|---|---|---|
| audit-security + code + owasp | Applied | `2026-08-15-audit-security.md` |
| audit-security-llm + vuln-harness | Applied | `2026-08-15-audit-security-llm.md` |
| audit-dataflow-cascade/concurrency/integrity | Applied | `2026-08-15-audit-dataflow-core.md` |
| audit-dataflow-io/state/temporal | Applied | `2026-08-15-audit-dataflow-io-state-temporal.md` |
| audit-architecture-seam + invariant-sync + negative-space | Applied | `2026-08-15-audit-architecture-invariants.md` |
| audit-reliability + failsafe + recovery + dependency + operator-signal | Applied | `2026-08-15-audit-reliability-failsafe.md` |
| audit-workflow-gui + design-webapp + compliance-posture | Applied | `2026-08-15-audit-workflow-gui-design.md` |
| audit-agent-orchestration + mcp-server + externalapi + contracts | Applied (mcp/crossrepo limited) | `2026-08-15-audit-orchestration-contracts.md` |
| audit-security-nodejs + architecture-nodejs + memory + resource + perf | Applied (nodejs limited) | `2026-08-15-audit-nodejs-lifecycle-perf.md` |
| audit-repo-posture/triage + deadcode + pipeline-graph + drift + repo-state + path-consistency | Applied (drift limited) | `2026-08-15-audit-repo-posture-state.md` |
| audit-playtest-app | Applied live, 110/110 | `2026-08-15-live-all-scenarios-playtest.md` |
| audit-graphdb-design | N/A | marked not applicable |

## 4. Cross-lens clusters

### CLUSTER A — Auto islands and confused-deputy agency *(ship-gating)*

Independent lenses: security, security-llm, negative-space, orchestration, pipeline-graph.

- Interactive default is SmartApprove (`gosling_mode` / config docs).
- Subagents, orchestrator-managed agents, plan-act, and some headless paths still force or require `GoslingMode::Auto`.
- In Auto, tools without explicit user NeverAllow/AskBefore are allowed; egress `RequireApproval` downgrades to Allow.
- A parent `delegate` that grants `developer` therefore yields write/shell without a further action-bound gate.
- SmartApprove still treats an LLM “read-only” verdict as Allow for unknown tools (SEC-GOS-013 / LLM-GSL-006).

### CLUSTER B — ACP / serve trust *(ship-gating if the port is reachable)*

- Shared secret accepted as `?token=` and constructed that way by Desktop (SEC-GOS-001).
- Live ACP MCP-app guest trusts client-supplied CSP; goslingd builds CSP server-side (SEC-GOS-002).
- Unauthenticated serve (`--dangerously-unauthenticated`) does not force loopback (SEC-GOS-012).
- MCP App iframe can receive the backend secret under a scriptable sandbox (SECN-GSL-001).

### CLUSTER C — Untrusted-repo composition *(High)*

Plugin auto-enable is no longer the July story. Remaining: repo `AGENTS.md` / hints load as additional instructions without the plugin trust gate (NEG-GSL-002 / LLM-GSL-004). Repo-committed agent files can request parent extensions for an Auto child (AOC-GOS-004).

### CLUSTER D — Durability / concurrency *(High/Medium)*

Historical non-atomic secrets write is **held** (atomic temp+rename). Remaining:

- Compacted resume injects a summary without a freshness gate (DAT-GSL-001, High).
- Cross-process recover can mark a live peer tool `in_doubt` (CON-GSL-001, High).
- Config RMW is process-local (CON-GSL-002).
- `GOSLING_PATH_ROOT` does not isolate plugin settings / `plugins_dir` the same way as config/data (RPC-GSL-001/002, TMP-GOS-002/003).

### CLUSTER E — Operator truth *(High/Medium)*

- Settings Default Mode can show Autonomous while backend default is SmartApprove (WFG-GOS-001).
- Desktop drops tool `error` strings (WFG-GOS-003).
- TUI still offers Allow-always on security prompts (WFG-GOS-006).
- CLI `/doctor` can auto-write a new global provider when the configured one fails (FSR-GSL-012).
- `gosling doctor` reports “local diagnostics complete” even when that is not a live provider proof (REL-GSL-011).
- `gosling serve` `/health` and `/status` always return static `"ok"` and cannot see a down session store (REL-GSL-010, High).
- Stale `docs/cloud` audits still assert default Auto as current (CMP-GOS-001).

### CLUSTER F — Live playtest defects *(observed)*

| ID | Sev | Cards | Mechanism |
|---|---|---|---|
| GSL-PLAY-2026-004 | High | EX-03 | Broken MCP stdio hangs `run` with empty streams |
| GSL-PLAY-2026-005 | Medium | SE-01, AC-02 | Non-TTY remove/fork: `not connected` after announcing the mutation |
| GSL-PLAY-2026-006 | Medium | HS-03, AP-05 | `gosling acp` initialize → exit 0, empty stdout |

2026-08-12 repairs still hold live: missing diagnostics fail closed (AC-04); invalid typed settings warn (LC-04).

## 5. Severity tally (deduplicated mechanisms)

| Severity | Distinct mechanisms | Notes |
|---|---|---|
| Critical | 0 | No confirmed unauthenticated remote RCE on default loopback |
| High | ~16 | Auto islands, ACP token/CSP, compacted resume, recover liveness, playtest MCP hang, tagteam argv / false “no findings” |
| Medium | ~40 | Path isolation, retries, WS origin, export redaction, GUI truth, repo posture |
| Low / Info | ~25 | Dead docs, version 0.1.0 vs 1.0.0 claims, PostHog hard-off |

Counts are mechanisms, not raw finding IDs. See per-lens files for the full set.

## 6. Playtest scoreboard

**58 Pass · 5 Fail · 47 Blocked · 0 N/A = 110.**

Blocked is dominated by Desktop (WS/DT/ST) and fixture-heavy resilience/ACP matrices. That is incomplete coverage, not a clean bill for those cards.

## 7. Held / stale vs July master

Do **not** treat the following July items as current without re-reading HEAD:

- Default mode Auto — now SmartApprove for interactive.
- Scanner default-off — now default-on (still advisory / never Deny).
- Project plugins auto-enabled — now require `plugin trust`.
- Non-atomic secrets write — atomic temp+rename present.
- Missing-session diagnostics success bundle — fail-closed (live AC-04).

## 8. Patch order (recommended)

1. Stop Auto-child paths from auto-approving write/shell/egress, or require an action-bound parent grant that names the tool+args.
2. Remove `?token=` from ACP URLs; bind guest CSP server-side on the live serve path (match goslingd).
3. Fail closed and name spawn errors when an MCP stdio server exits immediately (playtest EX-03).
4. Make `session remove` / `--fork` scriptable or refuse before printing “will be removed”.
5. Make `gosling acp` speak or fail on initialize; never exit 0 with empty stdout.
6. Add a freshness/version gate on compacted-resume summaries; don’t mark a live peer tool `in_doubt`.
7. Align Settings “Default Mode” copy with SmartApprove; stop TUI Allow-always on security prompts.
8. Isolate plugin/agent dirs under `GOSLING_PATH_ROOT` / `RuntimePaths`.

## 9. Validation limits

- Static lenses did not run `cargo test` / clippy (operator did not request a build of findings).
- Races, OOM, and multi-writer outcomes stay Likely/Plausible unless a test or playtest reproduced them.
- Playtest oracle is not a real model; instruction-loading and tool-loop cards are only as strong as filesystem/CLI evidence.
- Desktop was not playtested.
- `audit-security-vuln-harness` was used as an exploitability ladder, not a six-phase multi-agent hunt.
- `audit-architecture-drift` ran without a formal invariant registry.

## 10. Artifacts

- Orientation: `docs/cloud/2026-08-15-orientation.md`
- Per-lens reports: `docs/cloud/2026-08-15-audit-*.md`
- Playtest: `docs/cloud/2026-08-15-live-all-scenarios-playtest.md`
- Session log: `docs/logs/session/2026-08-15-exhaustive-audit.md`
- Live evidence (not in git): `/tmp/gosling-playtest-20260815/evidence`
