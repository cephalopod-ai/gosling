# Documentation inventory

Date: 2026-09-15

| Path | Type | Status | Owner/Authority | Last evidence | Action |
|---|---|---|---|---|---|
| `README.md` | Landing page | current | Repository maintainers | 2026-09-15 planning, Context History, and Recall Brief refresh | Keep concise and route procedures outward. |
| `AGENTS.md` | Operating contract | canonical | Repository maintainers | 2026-08-27 authority read | Preserve required clauses literally. |
| `docs/INDEX.md` | Engineering-doc index | canonical | Repository maintainers | 2026-09-15 link review and session-log additions | Keep current as repo-local surfaces change. |
| `documentation/INDEX.md` | User-doc index | canonical | Repository maintainers | 2026-09-15 candidate-scope refresh | Retain as the Docusaurus manual entry point. |
| `documentation/docs/` | User manual | current | Source, tests, and maintainers | 2026-09-15 CLI-surface reconciliation against `crates/gosling-cli` | Review when commands, providers, setup, or support change. |
| `docs/architecture.md`, `docs/architecture/`, `docs/adr/` | Architecture and decisions | current | Accepted ADRs and source | 2026-09-15 plan/Context History/Recall Brief sections and schema v36 correction | Distinguish current, intended, and historical claims. |
| `docs/TODO.md` | Repository backlog | canonical | Repository maintainers | 2026-09-13 release/install reconciliation | Keep as the source of truth; mirror only active items. |
| `docs/polish/` | Governance evidence | current | Dated run evidence | 2026-09-15 | Refresh ledgers without rewriting historical reports. |
| `docs/logs/session/` | Session records | historical | Original run evidence | Through 2026-09-15 | Retain flat dated files; do not use as current truth without reconciliation. |
| `docs/cloud/` and `reports/` | Audits and campaigns | historical plus dated addenda | Original audit evidence | Through 2026-09-13 | Preserve original campaign claims, append later integration evidence, and route unresolved findings into current ledgers. |
| `.architecture/*.yaml` | Architecture registry | current | Registry rules in `.architecture/README.md` | 2026-09-15 Context History component registration | Register new ownership, dependency, or privilege boundaries in the same change. |
| `.giles/*.yaml` | Generated governance metadata | stale | Advisory under `AGENTS.md` | July 2026 failed audit | Do not promote or edit until a fresh successful scan. |

No duplicate user manual, specification, or architecture tree was created. The
existing repository-specific layout takes precedence over generic stewardship
directory defaults.
