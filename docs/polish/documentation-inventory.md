# Documentation inventory

Date: 2026-09-15

Focused refresh: 2026-09-29 for the 1.4.0 source, Desktop repair manuals, and local GUI install.
Other surfaces retain their earlier evidence dates; this is not a new repository-wide audit.

| Path | Type | Status | Owner/Authority | Last evidence | Action |
|---|---|---|---|---|---|
| `README.md` | Landing page | current | Repository maintainers | 2026-09-29 source version, repair summary, latest campaign and installed launch evidence | Keep concise and route procedures outward. |
| `AGENTS.md` | Operating contract | canonical | Repository maintainers | 2026-08-27 authority read | Preserve required clauses literally. |
| `docs/INDEX.md` | Engineering-doc index | canonical | Repository maintainers | 2026-09-29 install log and current build-note links | Keep current as repo-local surfaces change. |
| `documentation/INDEX.md` | User-doc index | canonical | Repository maintainers | 2026-09-29 1.4.0 notes and current repair/install evidence | Retain as the Docusaurus manual entry point. |
| `documentation/docs/` | User manual | current in focused scope | Source, tests, and maintainers | 2026-09-29 installation/update, workspace, notifications, backend and isolation guidance | Other manuals retain their existing evidence; review when their behavior changes. |
| `ui/desktop/README.md` | Developer setup | current in focused scope | Manifests, justfile, Forge and install evidence | 2026-09-29 arm64 package/reinstall; Linux path correction and removed-script cleanup | Recheck prerequisites and platform commands when build configuration changes. |
| `RELEASE.md`, `RELEASE_CHECKLIST.md` | Release process and gates | current source baseline | Release owner | 2026-09-29 1.4.0 alignment; checkboxes remain open | Local build/launch does not close publication gates. |
| `docs/architecture.md`, `docs/architecture/`, `docs/adr/` | Architecture and decisions | current | Accepted ADRs and source | 2026-09-15 plan/Context History/Recall Brief sections and schema v36 correction | Distinguish current, intended, and historical claims. |
| `docs/TODO.md` | Repository backlog | canonical | Repository maintainers | 2026-09-29 focused repair/install reconciliation | Keep as the source of truth; mirror only active items. |
| `docs/polish/` | Governance evidence | dated | Dated run evidence | 2026-09-29 focused ledger/inventory/report refresh | Refresh ledgers without rewriting historical reports. |
| `docs/logs/session/` | Session records | historical | Original run evidence | Through 2026-09-29 | Retain flat dated files; do not use as current truth without reconciliation. |
| `docs/cloud/` and `reports/` | Audits and campaigns | historical plus dated addenda | Original audit evidence | September 27 campaign and September 29 repair addendum | Preserve original card outcomes; normal install is not a full replay. |
| `.architecture/*.yaml` | Architecture registry | current | Registry rules in `.architecture/README.md` | 2026-09-15 Context History component registration | Register new ownership, dependency, or privilege boundaries in the same change. |
| `.giles/*.yaml` | Generated governance metadata | stale | Advisory under `AGENTS.md` | July 2026 failed audit | Do not promote or edit until a fresh successful scan. |

No duplicate user manual, specification, or architecture tree was created. The
existing repository-specific layout takes precedence over generic stewardship
directory defaults.
