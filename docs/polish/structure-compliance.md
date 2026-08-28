# Documentation structure compliance

Date: 2026-08-27

Governing authority is the repository-declared layout in `AGENTS.md` and
`docs/INDEX.md`. Giles is present but explicitly advisory until a fresh scan or
promotion, so its alternative monthly-bucket layout was not imposed.

| ID | Rule / Giles code | Location | Status | Severity | Tier | Recommended action | Authority needed |
|---|---|---|---|---|---|---|---|
| DOC-STRUCT-001 | Root README remains concise | `README.md` | compliant | info | A | Keep detailed guidance in the manual. | none |
| DOC-STRUCT-002 | Canonical user guidance exists | `documentation/docs/` | compliant | info | A | Retain existing installation, quickstart, provider, extension, architecture, and troubleshooting pages. | none |
| DOC-STRUCT-003 | One pasteable MCP configuration block | `README.md` | compliant | info | A | Keep alternatives and trust guidance in the linked extension manual. | none |
| DOC-STRUCT-004 | Index distinct documentation surfaces | `docs/INDEX.md` | compliant | low | A | Links added for logs and stewardship evidence. | execute authority granted |
| DOC-STRUCT-005 | Document session evidence retention | `docs/logs/README.md` | compliant | low | A | Preserve the flat tracked session-log convention. | execute authority granted |
| DOC-STRUCT-006 | Active ledger contains unresolved work only | `docs/polish/active-todo-ledger.md` | compliant | medium | A | Reconcile from `docs/TODO.md` on future runs. | execute authority granted |
| DOC-STRUCT-007 | Required stewardship evidence has a repo-local home | `docs/polish/` | compliant | low | A | Inventory and compliance artifacts added under the existing evidence directory. | execute authority granted |
| DOC-STRUCT-008 | Giles metadata is authoritative | `.giles/*.yaml` | drift | medium | B | Repair Giles and run a fresh scan before any promotion or metadata edit. | external tool / maintainer |
| DOC-STRUCT-009 | Rebucket historical logs and add monthly summaries | `docs/logs/session/` | drift | low | C | Scope separately only if maintainers replace the established flat convention. | maintainer architecture |

No Tier B reorganization or Tier C archive migration was performed. Historical
logs, audits, and reports remain in place to preserve provenance and links.
