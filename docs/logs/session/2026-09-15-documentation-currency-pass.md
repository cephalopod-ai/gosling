# 2026-09-15 — Documentation currency pass

## Task and authority

Bring the repository's documentation up to date with the source that landed after the 2026-09-13
release-documentation refresh, check the README's feature and validation claims against the code,
hold other documentation to repo standards including its Mermaid diagrams, and remove temporary
files left behind by earlier runs.

Read order completed: `AGENTS.md` (`GEMINI.md` absent), `README.md`, `docs/INDEX.md`, the affected
architecture, ADR, guide, and governance documents, `.architecture/README.md`, and the
2026-09-13 through 2026-09-15 session logs. This was a documentation and registry pass; no runtime
code, test, schema, or scenario file was changed.

## Drift found and repaired

| Finding | Evidence | Disposition |
| --- | --- | --- |
| Host-enforced planning (ADR-0020, schema v35), Context History (ADR-0021, schema v36), and the Recall Brief action (ADR-0022) were absent from `docs/architecture.md` | `crates/gosling/src/session/session_manager/migrations.rs:816-817`, `plans.rs`, `compaction_history_storage.rs`, `recall_brief/` | Added three source-grounded sections, including a plan-lifecycle state diagram |
| `docs/architecture.md` labelled the live store `SessionManager v33` | `CURRENT_SCHEMA_VERSION = 36` in `session_manager.rs:68` | Corrected the diagram node to v36 and named plans and compaction history |
| `.architecture/components.yaml` had no owner entry for the compaction-history ledger, although the registry requires registration for new ownership and dependency boundaries | `.architecture/README.md` update rule; ADR-0021 | Added `core.context_compaction_history` with its storage, ACP, CLI, and Desktop paths |
| README described the candidate without planning or Context History | README feature narrative | Added a source-candidate paragraph, a Context History mention in the context/memory bullet, and a quick link |
| README's validation section implied 127-card coverage for the whole candidate | `docs/cloud/2026-09-13-live-all-scenarios-playtest.md` predates these features | Stated that planning, Context History, and Recall Brief carry focused automated evidence only |
| v1.2.5 release notes omitted Context History and the Recall Brief and understated the planning boundary | `git log c5cddc428..0dc8cfa0d`, feature session logs | Added highlights, a Context History section, schema/config compatibility notes, and dated validation evidence |
| `gosling doctor`, `gosling review`, `gosling tui`, `gosling session import`, `session export --nostr/--relay`, and the `/model`, `/status`, `/edit` slash commands were undocumented | `crates/gosling-cli/src/cli.rs`, `commands/doctor.rs`, `session/input.rs` | Documented each against the current source |
| `docs/build/shell-productization/audits/ds7-acceptance.md` pointed at `../../architecture/default-shell-template.md`, which does not exist | repo-local link check | Corrected to `../../../architecture/…` |
| Indexes did not list the planning or Context History session logs | `docs/INDEX.md`, `documentation/INDEX.md` | Added entries and refreshed the candidate-scope note |

Claims were taken from source rather than from earlier documentation: the plan states and their
transitions come from `plans.rs` and `plan_storage.rs`, the import trust boundary from
`session_transfer.rs`, the doctor exit behavior from `commands/doctor.rs`, and the Context History
policy surface from `acp/server/compaction_history.rs` and `custom_requests.rs`.

## Temporary files removed

- `tmp/_to_delete_gos-ui-src.tgz`, `tmp/_to_delete_shell_gui.tgz`, and `tmp/_to_delete_gui_extract/`
  (4.9 MB, untracked, ignored). These were the 2026-08-18 shell-GUI extraction copies; every
  extracted file now exists in the repository at `ui/desktop/src/shell-ui/`, `ui/desktop/shell.html`,
  and `fixtures/shell-consumers/default-shell-template/`. The empty `tmp/` directory was removed.
- Stray `.DS_Store` files at the repository root and under `ui/`, `ui/desktop/`, and `documentation/`.

Deliberately retained: `generated/today-audit/` is referenced by tracked reports
(`docs/cloud/2026-09-08-evening-audit-repair.md`), and `logs/` holds July audit and playtest notes.
Neither is regenerable from the repository, so removing them would destroy referenced evidence.

## Validation

- Repo-local Markdown link check over 339 files: the two repaired or pre-existing failures are gone;
  the only remaining unresolved links live in an untracked session log.
- Mermaid: all seven diagram blocks in the repository, including the new plan-lifecycle state
  diagram, parse cleanly with `mermaid.parse` (mermaid 11 under jsdom, run from a scratch directory).
- `.architecture/components.yaml` parses as YAML after the new component entry.
- `git diff --check` passed; the AGENTS governance-marker check passed; `GEMINI.md` is absent, so its
  marker check does not apply.
- Documentation production build: see the result recorded in `docs/polish/test-ledger.md` for this
  date.
- No Rust build, test, or Clippy run was performed or is claimed; no source file was touched.

## Risks and follow-ups

- `docs/cloud/2026-09-08-evening-audit-repair.md` links to reports under `generated/today-audit/`,
  which `.gitignore` excludes. The cited evidence is local-only; decide whether to commit those
  reports or mark the links as machine-local.
- The `.gitignore` allowlist for `docs/logs/session/` stops at 2026-09-08, so every later log —
  including this one — needs `git add -f` to be tracked.
- ADR-0021 now has a component entry but no `ARC-` invariant, while planning and the Recall Brief
  have ARC-011/012 and ARC-013. Decide whether the export, sharing, and telemetry exclusions deserve
  their own review gate.
- `documentation/docs/guides/goose-comparison.md` remains pinned to gosling v1.2.3 against Goose
  v1.49.0 as of 2026-09-08. It is dated evidence, so it was not rewritten, but its CLI row no longer
  lists the current command surface; refresh it at the next upstream check.
