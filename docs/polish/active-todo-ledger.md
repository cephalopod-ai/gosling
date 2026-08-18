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
