# Active TODO ledger

docs/TODO.md is the canonical backlog. This is a staging mirror of the
release-relevant items observed in this run. Rows closed during the run remain
as repair evidence; older completed or struck-through entries are not copied.

| ID | Status | Priority | Area | Item | Source | Exit criteria |
|---|---|---|---|---|---|---|
| TODO-20260817-001 | closed | P0 | Release | Align all source, lockfile, generated, Desktop, About, and runtime versions for the intended release. | docs/TODO.md | Workspace, lockfile, Desktop, generated API, packaged app, and CLI evidence report 1.1.0. |
| TODO-20260817-002 | open | P0 | Release | Complete the release checklist source, Desktop, signing, checksum, scenario, and clean-install gates. | RELEASE_CHECKLIST.md | Every required checkbox has observed evidence. |
| TODO-20260817-003 | closed | P1 | Repository identity | Resolve the committed upstream identity versus the cephalopod-ai/gosling remote. | posture audit | Origin, UPSTREAM.md, README, release stewardship record, current links, and repository-gated workflows designate cephalopod-ai/gosling; historical evidence and the separately governed npm scope are preserved. |
| TODO-20260817-004 | closed | P1 | Documentation | Repair the documentation TypeScript/Docusaurus failures. | documentation typecheck | Typecheck, 16 tests, and the production site build pass. |
| TODO-20260817-005 | partial | P1 | Release hygiene | Run approved secret/history and dependency-audit tooling. | security redaction ledger | Documentation npm audit is recorded; dedicated secret/history and Rust dependency scanners remain unavailable. |
| TODO-20260827-001 | partial | P0 | CI / shell | Restore shell consumer conformance and cross-platform assertion portability. | CI run 33120050894; shell profile tests | 67 shell-profile tests and profile checks pass locally; the next remote revision must confirm Windows/macOS/Linux runners. |
| TODO-20260827-002 | partial | P0 | Windows Rust | Remove cfg-specific warnings rejected by the Windows `-D warnings` build. | shell package run 33120050956 | Imports, arguments, and helpers are scoped to the platforms that use them; host fmt/check pass, with remote Windows confirmation pending. |
| TODO-20260827-003 | blocked-upstream | P1 | Documentation dependencies | Clear the remaining Docusaurus build-chain advisories. | RSP-GSL-004 | A compatible Docusaurus/webpack dependency chain clears the current 25 transitive advisories. |
| TODO-20260826-008 | closed | P0 | Desktop FS | Artifact routing IPC must not grant dirs/files outside renderer grants. | SEC-GSL-001 | Renderer routing is root-constrained; focused artifact-access tests pass. |
| TODO-20260826-009 | closed | P0 | Extensions | Enforce `GOSLING_ALLOWLIST` in Rust on every add-extension sink. | SEC-GSL-002 | Exact command/argv enforcement covers runtime, ACP/HTTP, and CLI sinks; focused Rust/UI tests pass. |
| TODO-20260826-010 | closed | P0 | Shell | Protocol-filter shell `openExternal`. | SECN-GSL-001 | Shared URL guard blocks `file:`, `javascript:`, and `data:`; focused tests pass. |
| TODO-20260826-001 | closed | P0 | Permissions | ACP `tools/permissions/set` must error when persist fails. | WFG-GSL-002 | Persist-fail ACP regression returns an error and rolls memory back. |
| TODO-20260826-002 | closed | P0 | Permissions | Always Allow all extension tools must persist before resolving the live request. | WFG-GSL-001 | Desktop regression verifies persist-before-resolve and leaves failure pending. |
| TODO-20260826-003 | closed | P0 | Auto/subagent | Auto must not Allow `manage_extensions` without an explicit user grant. | LLM-GSL-004 | Auto denies ungranted extension management through shared tool classification. |
| TODO-20260826-004 | closed | P0 | CLI | Headless Auto must Deny/abort inspector-failure confirmations. | WFG-GSL-004 | Focused CLI regression verifies cancellation without dispatch. |
| TODO-20260826-005 | closed | P0 | Artifacts | Assistant absolute document paths must not become preview grants. | IOP-GSL-001 | Focused Rust/UI tests reject assistant paths outside authorized roots. |
| TODO-20260826-006 | closed | P0 | Workspaces | Same-schema workspace validation failure must not wipe to Default. | DAT-GSL-001 | Focused store regressions preserve invalid same-schema bytes and return an error. |
| TODO-20260826-007 | closed | P1 | Reliability | Pin ACP in-flight turns in AgentManager LRU so a 6th session cannot evict a running agent. | REL-GSL-001 | Focused ACP/LRU regression keeps the active agent and evicts the idle entry. |
