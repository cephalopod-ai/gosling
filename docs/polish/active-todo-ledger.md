# Active TODO ledger

docs/TODO.md is the canonical backlog. This is a staging mirror of the
release-relevant open items observed in this run; it intentionally does not
copy completed or struck-through entries.

| ID | Status | Priority | Area | Item | Source | Exit criteria |
|---|---|---|---|---|---|---|
| TODO-20260817-001 | open | P0 | Release | Align all source, lockfile, generated, Desktop, About, and runtime versions for the intended release. | docs/TODO.md | Reviewed release change reports one consistent version. |
| TODO-20260817-002 | open | P0 | Release | Complete the release checklist source, Desktop, signing, checksum, scenario, and clean-install gates. | RELEASE_CHECKLIST.md | Every required checkbox has observed evidence. |
| TODO-20260817-003 | needs-decision | P1 | Repository identity | Resolve the committed upstream identity versus the cephalopod-ai/gosling remote. | posture audit | Owner designates canonical repository and updates dependent policy deliberately. |
| TODO-20260817-004 | open | P1 | Documentation | Repair the documentation TypeScript/Docusaurus failures. | documentation typecheck | pnpm run typecheck passes. |
| TODO-20260817-005 | needs-verification | P1 | Release hygiene | Run approved secret/history and dependency-audit tooling. | security redaction ledger | Tool output is reviewed and recorded. |
| TODO-20260826-001 | open | P0 | Permissions | ACP `tools/permissions/set` must error when persist fails. | WFG-GSL-002 | Persist-fail ACP call is not `Ok`; UI does not show a lasting grant. |
| TODO-20260826-002 | open | P0 | Permissions | Always Allow all extension tools must persist before resolving the live request. | WFG-GSL-001 | Persist error leaves the pending tool blocked. |
| TODO-20260826-003 | open | P0 | Auto/subagent | Auto must not Allow `manage_extensions` without an explicit user grant. | LLM-GSL-004 | Auto subagent Enable does not start MCP. |
| TODO-20260826-004 | open | P0 | CLI | Headless Auto must Deny/abort inspector-failure confirmations. | WFG-GSL-004 | CLI Auto + inspector error does not dispatch the tool. |
| TODO-20260826-005 | open | P0 | Artifacts | Assistant absolute document paths must not become preview grants. | IOP-GSL-001 | Path outside workspace roots fails `assertArtifactFileAccess`. |
| TODO-20260826-006 | open | P0 | Workspaces | Same-schema workspace validation failure must not wipe to Default. | DAT-GSL-001 | `workspaces: []` leaves the original file in place and errors. |
