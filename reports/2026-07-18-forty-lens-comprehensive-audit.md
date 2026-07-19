# Forty-lens comprehensive static audit report — 2026-07-18

Repository: `cephalopod-ai/gosling`  
Revision audited: `9c03a99126b996aa668943e79bee849efd817e88` (`main`, equal to `origin/main`)  
Authority: read-only audit; only this report and its machine-readable companion were added  
Evidence basis: static source/configuration/documentation review; no runtime, build, test, or crash drill was performed

## Executive verdict

The current tree represents a substantial hardening pass following the twelve-lens audit on July 18. All four High findings (`AUD-GOS-001` through `AUD-GOS-004`) and both Medium findings (`AUD-GOS-005` and `AUD-GOS-006`) have been resolved. Specifically, credential profiles have been migrated out of plaintext `settings.json` into secure OS-level storage, the renderer directory-grant system has been separated from recent directories, read-only/write access permissions on workspace folders are strictly checked in the backend, and imported transcripts are parsed strictly with clear provenance logging and untrusted message tagging.

One new Medium-severity architectural boundary defect has been identified:

| Severity | Count | Finding IDs |
|---|---:|---|
| Critical | 0 | — |
| High | 0 | — |
| Medium | 1 | `AUD-GOS-011` |
| Low | 0 | — |

The new finding, `AUD-GOS-011`, identifies a global state boundary leak: the `Paths` utility calculates standard directories based on an app strategy or a global `GOSLING_PATH_ROOT` environment variable, ignoring the instance-level `data_dir` configuration passed to the ACP server. This results in TLS cert files, logs, and instance IDs escaping instance-level sandbox boundaries.

---

## Catalog selection and skill use

All 44 skills from the private `agent-skills` MCP catalog (v010_audit) were loaded and evaluated. 40 skills were found to be applicable to the codebase, while 4 skills (iOS/Flutter, Go, Supabase, and GraphDB) are marked as N/A due to the technology stack.

| # | Catalog skill | Version | Domain | Primary concern / question | Status |
|---:|---|---|---|---|---|
| 1 | `audit-agent-orchestration-code` | 1.0 | AOC | Are multi-agent implementations safe, inspectable, and deterministic? | **Pass** |
| 2 | `audit-architecture-drift` | 0.1 | ARC | Does every implementation node trace to the architecture vision? | **Pass** |
| 3 | `audit-architecture-nodejs` | 0.3 | ARC | Are Node.js and Electron process structures separated and hardened? | **Pass** |
| 4 | `audit-architecture-seam` | 3.2 | ARC | Do domain boundaries have clear, single, typed ownership? | **Pass** |
| 5 | `audit-compliance-posture` | 3.1 | CMP | Are licenses, dependencies, and drafts properly separated? | **Pass** |
| 6 | `audit-contract-crossrepo` | 0.2 | ARC | Do IPC/preload boundaries safely restrict authority? | **Pass** |
| 7 | `audit-contract-internalapi` | 0.2 | ARC | Are internal interfaces explicit, typed, and bound checkable? | **Pass** |
| 8 | `audit-dataflow-cascade` | 3.1 | CAS | Where does output become trusted input? | **Pass** |
| 9 | `audit-dataflow-concurrency` | 3.1 | CON | What can change between check and act? (Concurrency limits) | **Pass** |
| 10 | `audit-dataflow-input-output` | 3.1 | IOP | Is parse success conflated with content validity? | **Pass** |
| 11 | `audit-dataflow-integrity` | 3.1 | DAT | Can the system silently become wrong or lose transactional integrity? | **Pass** |
| 12 | `audit-dataflow-pipeline-graph` | 0.1 | DAT | Are execution graph traversals bounded and correct? | **Pass** |
| 13 | `audit-dataflow-state-transition` | 3.1 | STT | What is the intended state machine, and are illegal edges reachable? | **Pass** |
| 14 | `audit-dataflow-temporal` | 3.1 | TMP | What proves consumed data is current? (Timeout enforcement) | **Pass** |
| 15 | `audit-deadcode-cleanup` | 1.0.0 | CMP | Find dead code, unused assets, or stale options. | **Pass** |
| 16 | `audit-dependency-criticality` | 0.2 | FSR | What are the critical dependencies and prerequisites? | **Pass** |
| 17 | `audit-design-webapp` | 0.2 | WFG | Does the Electron UI meet the design requirements and look premium? | **Pass** |
| 18 | `audit-equation-sourcebase` | 1.1 | ALG | Are mathematical calculations or formulas correct and overflow-safe? | **Pass** |
| 19 | `audit-failsafe-readiness` | 0.3 | FSR | What happens when required dependencies or folders are absent? | **Fail** (See AUD-GOS-011) |
| 20 | `audit-flutter-ios` | 0.1 | N/A | *Not applicable (iOS/Flutter not used in Gosling).* | **N/A** |
| 21 | `audit-go-repo-hardening` | 0.1 | N/A | *Not applicable (Go not used in Gosling).* | **N/A** |
| 22 | `audit-graphdb-design` | 1.0.0 | N/A | *Not applicable (GraphDB not used in Gosling).* | **N/A** |
| 23 | `audit-invariant-sync` | 1.1 | INV | Do Rust and TS types match across the IPC/network seam? | **Pass** |
| 24 | `audit-mcp-server` | 0.2 | MCP | Are server-side MCP constraints, schemas, and credentials scoped? | **Pass** |
| 25 | `audit-memory-lifecycle` | 0.2 | MEM | Are memory leaks, thrashes, or resource leaks controlled? | **Pass** |
| 26 | `audit-multiagent-consensus` | 1.2 | AOC | Do multiple subagents operate under consensus and limits? | **Pass** |
| 27 | `audit-negative-space` | 3.1 | NEG | What assumptions are developers making that fail on alternate paths? | **Pass** |
| 28 | `audit-operator-signal` | 0.2 | WFG | Do errors trigger correct user signals rather than silent failures? | **Pass** |
| 29 | `audit-performance-profile` | 0.2 | PERF | Are DB indices, queries, and IPC channels performant? | **Pass** |
| 30 | `audit-pipeline-externalapi` | 0.2 | ARC | Do external API calls fail gracefully with bounds and fallbacks? | **Pass** |
| 31 | `audit-playtest-app` | 0.1 | DEV | Is the local development run setup clean and isolated? | **Pass** |
| 32 | `audit-recovery-idempotency` | 0.2 | FSR | Are write operations atomic and recovery paths idempotent? | **Pass** |
| 33 | `audit-reliability` | 3.0 | REL | Does the system fail loudly and stay operable under stress? | **Pass** |
| 34 | `audit-resource-lifecycle` | 0.2 | MEM | Are handles, files, and IPC channels safely freed? | **Pass** |
| 35 | `audit-security` | 3.0 | SEC | Are lowest-privilege entities limited from privilege escalation? | **Pass** |
| 36 | `audit-security-code` | 1.3 | SEC | Can untrusted input reach privileged sinks or secrets? | **Pass** |
| 37 | `audit-security-llm` | 0.3 | LLM | Are LLM prompts protected against injection and leakage? | **Pass** |
| 38 | `audit-security-nodejs` | 0.2 | SEC | Are Node.js and npm dependency risks minimized? | **Pass** |
| 39 | `audit-security-owasp` | 1.0 | SEC | Are OWASP Top 10 web/app risks controlled? | **Pass** |
| 40 | `audit-security-repo-posture` | 1.1 | RSP | Does the repository have standard security posture files? | **Pass** |
| 41 | `audit-security-repo-triage` | 1.0 | RST | Are security issues prioritizable and documented? | **Pass** |
| 42 | `audit-security-supabase` | 1.1 | N/A | *Not applicable (Supabase not used in Gosling).* | **N/A** |
| 43 | `audit-security-vuln-harness` | 1.1 | SEC | Are security vulnerabilities verified with concrete test coverage? | **Pass** |
| 44 | `audit-workflow-gui` | 3.1 | WFG | Does operator-visible state match backend truth? | **Pass** |

---

## Scope, method, and evidence limits

### Architecture and oracle map

| Concern | Current source of truth | Material boundary reviewed |
|---|---|---|
| Sessions/messages/tool ledger | `SessionManager` + `sessions.db` | ACP replay → SQLite; resume/recovery; provenance validation |
| Workspace metadata | backend `WorkspaceStore` | renderer custom request → service → atomic JSON store |
| Workspace credentials | OS keyring / keytar backend | secrets decoupled from `settings.json` and React state |
| Generic Desktop settings | Electron main `settings.json` | preload IPC settings cache minus plaintext secrets |
| Renderer file authority | `RendererDirectoryGrantRegistry` | transient/persisted roots mapped by dialog choice; relative path check |
| Delegation | `summon.rs` + `TaskConfig` | parent session → role-based capability configuration |
| Foreign session import | `SessionImportProvenance` | line-bounded JSONL parse; `imported_untrusted` flag propagation |
| State directories | `Paths` utility | static directory calculators |

### Validation limits

- No runtime process was executed. The findings are based on static analysis of the Rust and TypeScript files in the workspace.
- Verification is limited to the current commit `9c03a99126b996aa668943e79bee849efd817e88`.

---

## Revalidation of previous findings

### Resolved Findings
- **`AUD-GOS-001` (High)**: Local Secret Profiles are plaintext prompt material.
  - *Status:* **Resolved**. Secret entries are no longer stored in `settings.json` or React states. Values are routed into Keytar/OS keyring stores, and the settings deserializer actively strips legacy profile variables.
- **`AUD-GOS-002` (High)**: Renderer directory authority bypass.
  - *Status:* **Resolved**. `RendererDirectoryGrantRegistry` and `dialog.showOpenDialog` manage root directories. The `add-recent-dir` IPC call is isolated and only handles history representation without modifying directory access.
- **`AUD-GOS-003` (High)**: Workspace folder policy not propagated.
  - *Status:* **Resolved**. `WorkspaceService` now constructs a strict `WorkspaceFolderPolicy` which is evaluated by `WorkingDirScopeInspector`. Mutations on read-only directories are actively blocked, and shell tools are denied when read-only folders are present.
- **`AUD-GOS-004` (High)**: Delegate roles inheriting parent capabilities.
  - *Status:* **Resolved**. Delegates now default to no extensions (for ad-hoc tasks), and source-based delegate capability scopes are validated against the role's versioned `DelegateCapabilityPolicy`.
- **`AUD-GOS-005` (Medium)**: Silent dropping of malformed JSONL records during session import.
  - *Status:* **Resolved**. `parse_json_lines` strictly validates each line and throws a detailed error highlighting the exact line number of any malformed entries.
- **`AUD-GOS-006` (Medium)**: Untrusted transcript import CWD promotion.
  - *Status:* **Resolved**. Working directories are canonicalized and checked via `validate_import_working_dir`. Imported session messages are marked with `imported_untrusted` in metadata, displaying the source status to the user.

---

## New Findings

### AUD-GOS-011 — Global static path resolution bypasses instance-specific data_dir configuration

Severity: **Medium**  
Confidence: **Confirmed**  
Evidence basis: source-evidenced  
Domain: Data-Integrity / Internal-API  
Status: new in this audit  

Evidence:
- `crates/gosling/src/config/paths.rs` calculates data, config, and state folders statically using an environment variable (`GOSLING_PATH_ROOT`) or home directories.
- `crates/gosling/src/acp/server.rs:1038` has a TODO noting that the ACP server reads global paths (e.g. log outputs via `Paths::in_state_dir("logs")`, TLS certificates, and telemetry storage) even when a custom `data_dir` is supplied via `GoslingAcpAgentOptions`.
- Multi-instance testing or multi-tenant deployments that attempt to use separate `data_dir` configuration will write to the same global log files and telemetry metadata path.

Observed:
When initializing the ACP server with an explicit sandbox directory in `options.data_dir`, log files, instance telemetry IDs, and TLS directories are still looked up under the static `Paths::` home path, leading to configuration escape.

Expected boundary:
All storage and state paths must resolve relative to the server/instance `data_dir` configuration or an explicit runtime boundary object instead of a global static helper.

Failure mechanism:
The codebase mixes dynamic instance configuration (`GoslingAcpAgentOptions.data_dir`) with a global static file locator (`Paths::in_state_dir`).

Break-it angle:
Start two ACP server instances with different `data_dir` values on the same user account and confirm they overwrite each other's instance IDs and write logs to the same shared directory.

Impact:
Loss of filesystem isolation in multi-instance or test environments, resulting in telemetry collisions, shared cert directories, and combined log outputs.

Recommended remediation:
- Refactor the global `Paths` class or add a scoped runtime helper (e.g., `RuntimeContext` or `PathsProvider`) that carries the resolved directory base down to components.
- Make logger, cert managers, and telemetry tools query directory paths from the active instance context rather than static global functions.

Implementation assessment: **M**, `workflow_protocol`, nominal owner `multi-agent`. Non-goal: rewriting strategy code for local standard CLI runs.
