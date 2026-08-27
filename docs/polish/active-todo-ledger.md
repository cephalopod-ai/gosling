# Active TODO ledger

docs/TODO.md is the canonical backlog. This is a staging mirror of the
release-relevant items observed in this run. Rows closed during the run remain
as repair evidence; older completed or struck-through entries are not copied.

| ID | Status | Priority | Area | Item | Source | Exit criteria |
|---|---|---|---|---|---|---|
| TODO-20260817-001 | open | P0 | Release | Align all source, lockfile, generated, Desktop, About, and runtime versions for the intended release. | docs/TODO.md | Reviewed release change reports one consistent version. |
| TODO-20260817-002 | open | P0 | Release | Complete the release checklist source, Desktop, signing, checksum, scenario, and clean-install gates. | RELEASE_CHECKLIST.md | Every required checkbox has observed evidence. |
| TODO-20260817-003 | needs-decision | P1 | Repository identity | Resolve the committed upstream identity versus the cephalopod-ai/gosling remote. | posture audit | Owner designates canonical repository and updates dependent policy deliberately. |
| TODO-20260817-004 | open | P1 | Documentation | Repair the documentation TypeScript/Docusaurus failures. | documentation typecheck | pnpm run typecheck passes. |
| TODO-20260817-005 | needs-verification | P1 | Release hygiene | Run approved secret/history and dependency-audit tooling. | security redaction ledger | Tool output is reviewed and recorded. |
| TODO-20260826-008 | closed | P0 | Desktop FS | Artifact routing IPC must not grant dirs/files outside renderer grants. | SEC-GSL-001 | Renderer routing is root-constrained; focused artifact-access tests pass. |
| TODO-20260826-009 | closed | P0 | Extensions | Enforce `GOSLING_ALLOWLIST` in Rust on every add-extension sink. | SEC-GSL-002 | Exact command/argv enforcement covers runtime, ACP/HTTP, and CLI sinks; focused Rust/UI tests pass. |
| TODO-20260826-010 | closed | P0 | Shell | Protocol-filter shell `openExternal`. | SECN-GSL-001 | Shared URL guard blocks `file:`, `javascript:`, and `data:`; focused tests pass. |
| TODO-20260826-001 | closed | P0 | Permissions | ACP `tools/permissions/set` must error when persist fails. | WFG-GSL-002 | Persist-fail ACP regression returns an error and rolls memory back. |
| TODO-20260826-002 | closed | P0 | Permissions | Always Allow all extension tools must persist before resolving the live request. | WFG-GSL-001 | Desktop regression verifies persist-before-resolve and leaves failure pending. |
| TODO-20260826-003 | closed | P0 | Auto/subagent | Auto must not Allow `manage_extensions` without an explicit user grant. | LLM-GSL-004 | Auto denies ungranted extension management through shared tool classification. |
| TODO-20260826-004 | closed | P0 | CLI | Headless Auto must Deny/abort inspector-failure confirmations. | WFG-GSL-004 | Focused CLI regression verifies cancellation without dispatch. |
| TODO-20260826-005 | closed | P0 | Artifacts | Assistant absolute document paths must not become preview grants. | IOP-GSL-001 | Focused Rust/UI tests reject assistant paths outside authorized roots. |
| TODO-20260826-006 | closed | P0 | Workspaces | Same-schema workspace validation failure must not wipe to Default. | DAT-GSL-001 | Focused store regressions preserve invalid same-schema bytes and return an error. |
| TODO-20260826-007 | open | P1 | Reliability | Pin ACP in-flight turns in AgentManager LRU so a 6th session cannot evict a running agent. | REL-GSL-001 | ACP prompt in flight + 6th session keeps the original agent. |
