# Documentation stewardship report

## Focused refresh — 2026-09-29

The current 1.4.0 source and verified local macOS arm64 build are reflected in the root/Desktop
READMEs, installation/update manuals, affected Desktop guides, release notes, and navigation.
The [session record](../logs/session/2026-09-29-gui-install-documentation.md) owns the file inventory,
source evidence, validation results, and scoped gate dispositions for this refresh.

Normal installed GUI/backend launch passed. PKG-GSL-002 now needs verification of clean shutdown
and fresh-profile startup rather than repeating the old Keychain blocker as a current observation.
Native fault/recovery/picker/notification scenarios and release publication gates remain open.
The focused active mirror retains 21 rows; no full backlog re-audit, log move, architecture change,
or Giles compliance promotion occurred. The earlier full stewardship report follows unchanged.

Validation passed: the full documentation build exported 174 pages, all 16 documentation tests
and typecheck passed, local links/anchors and seven rendered pages passed, and formatting,
whitespace, and unchanged governance markers were checked. Runtime/native verification gaps
remain separate from this completed documentation refresh. Changes are commit-ready; no commit,
push, or deployment was requested or performed.

## Historical full pass — 2026-08-27

Date: 2026-08-27

## Gate results

| Gate | Result |
|---|---|
| 0 — authority and safety | Read `AGENTS.md`, README, indexes, architecture/intent, Giles advisory metadata, manifests, CI, and recent relevant session logs before editing. |
| 1 — conventions and structure | Repository-declared layout governs; seven Tier A items are compliant/repaired, one Tier B Giles item and one Tier C rebucketing item are routed. |
| 2 — inventory | `documentation-inventory.md` maps canonical, current, stale, and historical surfaces. |
| 3 — logs | Raw session records were preserved; `docs/logs/README.md` documents the established flat convention. No historical log was moved or rewritten. |
| 4 — TODOs | Active mirror now has 12 unresolved rows; completed evidence remains in the canonical backlog and logs. |
| 5 — spec and architecture | Existing architecture, ADR, intent, and shell-contract surfaces were retained; the stale SessionManager schema label was corrected to v30. |
| 6 — setup, manual, README | Existing 165-page user manual remains canonical. README is under 300 lines, links deeper setup/docs, includes its existing implementation diagram, and now has exactly one MCP configuration block. |
| 7 — validation | Rust, Desktop, and documentation lanes pass; details and limitations are in `test-ledger.md`. |
| 8 — handoff | This report, the code-polish report, and the dated session log provide commit-ready evidence. |

## Installation truth

The existing installation manual covers the Desktop and CLI setup paths. It was
source-reviewed, and the repository build passed on this macOS checkout; the
run did not perform a clean install, package installation, account setup, Windows/Linux
execution, or published-artifact verification. Dependency completeness remains
anchored in Cargo and package lockfiles rather than a hand-maintained package
list. The README MCP command was verified against the CLI documentation and
source shape but not executed because it downloads a package and changes user
configuration.

## Created and updated

- Created the documentation inventory, structure-compliance report, log
  convention README, this report, and a dated session record.
- Updated the README, docs index, architecture label, active TODO mirror, test
  ledger, and existing polish evidence.
- Archived or moved logs: none.

## Remaining risks and next action

There are 12 active ledger rows, including four P0 remote/release gates. Giles
metadata remains advisory after its recorded failed scan; Docusaurus advisories
remain blocked upstream; cross-platform and publication evidence require
external systems. The next safe maintenance action is a fresh active-ledger
reconciliation after remote CI or dependency state changes.
