# 2026-09-29 — GUI reinstall and documentation refresh

## Task and scope

The operator requested a GUI rebuild/reinstall, followed by updates to documents, README, and
manuals. The documentation pass uses `governance-doc-stewardship` 2.3 in Execute mode at its
documentation-only, low-risk ceiling. Target: `/Users/eric/Work/vscode/forked/gosling`, branch
`main`, initially clean at `b69804da55019d787e6797e297cf2d7007f71e57`.

This is a focused refresh for the current 1.4.0 source, Desktop repairs, and local installation.
Historical comparisons and audits retain their original dates. No runtime, dependency, permission,
architecture, release-publication, or product-decision changes are authorized by this docs pass.
Codex has local shell/file/Git access; no independent review is required by the selected skill.
Independent reads/checks run concurrently; edits precede validation of their final state. This
tracked log is the durable checkpoint. Involvement is standard, with no unresolved choice needed
for these reversible documentation edits.

## GUI build and install evidence

The preceding authorized install task ran from the same clean source revision:

```sh
source bin/activate-hermit
DEVELOPER_DIR=/Library/Developer/CommandLineTools just package-ui
cargo fmt --check
```

- Release Rust backend, SDK/i18n, production renderer/main/preload, Electron Forge arm64 packaging,
  and local ad-hoc signing passed. No version bump was performed: source and package report 1.4.0.
- Packaged and installed bundles passed `codesign --verify --deep --strict`. The release and bundled
  backends matched; version output was `gosling 1.4.0`.
- The installed bundle is `/Applications/Gosling.app`. Production-bundle checks found the backend
  identity, native folder-selection, corrected completion notice, and plaintext-storage markers.
- The installed GUI and embedded backend remained running. Native accessibility readback showed
  the installed renderer with navigation, conversation, composer, and Outputs pane. An initial
  capture failure was transient; selecting the installed app again returned the rendered UI.
- The prior bundle and a machine-local receipt remain under
  `~/.local/share/gosling/install-backups/gui-reinstall-20260929-r7xdgezc/`. The receipt reports
  `installed_and_launched`. User data and the standalone global CLI were not replaced.

| Installed resource | SHA-256 |
|---|---|
| `Contents/Resources/app.asar` | `5078021c5ada03b05a55756005626929e738fa8dc3c4553b5b02bdcf4fb6eb84` |
| `Contents/Resources/bin/gosling` | `658427d7564c1bd98332ef6adc076ff3c2daec4e9c1be3fd066a5011d608e66c` |

This proves local build, install, and normal launch. It does not prove a Developer ID signature,
notarization, clean-account installation, clean shutdown, updater operation, or the remaining native
fault/recovery/picker/notification scenarios. The September 13 Keychain startup blocker did not
recur in this launch; its root cause was not established. PKG-GSL-002 remains partially verified.

## Documentation evidence and routing

Read order: `AGENTS.md`, `GEMINI.md`, root README, `docs/INDEX.md`, relevant architecture, manuals,
release/build instructions, stewardship/log conventions, advisory `.giles/*.yaml`, then recent
September 27/29 session and playtest records. Also read `documentation/AGENTS.md` before site edits.
The July Giles scan remains stale advisory evidence with a recorded indexing crash; this pass
makes no compliance claim and changes no agent contract or Giles metadata.

Source checks include `Cargo.toml`, `ui/desktop/package.json`, `justfile`, `forge.config.ts`, the
V8 build wrapper, backend status/identity/storage, directory grants, Research Library settings,
credential notices, and completion-notification handling. Lockfiles remain the dependency records.

| README section | Original lines | Treatment | Canonical detail |
|---|---:|---|---|
| Root README, all sections | 256 | Keep existing structure; correct current version and validation; add concise repair summary | 1.4.0 notes, installation/update manuals, test ledger |
| Dated Goose comparison and earlier feature history | Within root README | Keep historical versions and attribution | Existing comparison and release-note archive |
| Desktop README | 110 | Correct setup/bundling commands; document tested local packaging and verification | Existing installation/update manuals, release process |
| Documentation README | 52 | Retain site development/publishing role | `documentation/INDEX.md` |

The existing flat session-log layout, user manual tree, architecture/ADRs, and maintenance contract
satisfy those artifact roles. Monthly rebucketing, a new specification, new diagrams, and a full
backlog or whole-tree structure audit are outside this focused refresh. Gates 0–2 and 6–8 apply;
Gate 3 records this dated log, Gate 4 reconciles only install/release evidence, and Gate 5 preserves
the current architecture rather than inventing a new decision. No logs are moved or deleted.

## Files changed

- `README.md`, `ui/desktop/README.md`: current source/install status, build prerequisites and actual
  package commands, embedded versus standalone backend, verification and reinstall navigation.
- `RELEASE.md`, `RELEASE_CHECKLIST.md`: align documentation to 1.4.0 source without checking release
  gates or asserting publication. Historical tags retain their dated provenance.
- `documentation/docs/getting-started/installation.md`, `guides/updating-gosling.md`: installation
  evidence, local reinstall/rollback procedure, and preservation of user data.
- `documentation/docs/guides/workspaces.md`, `guides/sessions/in-session-actions.md`,
  `guides/environment-variables.md`, `troubleshooting/known-issues.md`: corrected storage/access
  descriptions, backend state/reconnect, preview/composer behavior, notifications, Research Library
  isolation, and the repaired provider-managed compaction-countdown status.
- `documentation/docs/release-notes/v1.4.0.md`: new source/local-build notes with validation limits.
- `docs/INDEX.md`, `documentation/INDEX.md`, both existing documentation inventories: current
  navigation and focused evidence dates.
- `docs/TODO.md`, `docs/polish/active-todo-ledger.md`, `test-ledger.md`,
  `documentation-stewardship-report.md`, `structure-compliance.md`: reconcile install evidence,
  document validation, preserve open native/product/release gates and historical reports.
- This session log and its `.gitignore` negation: make the install receipt summary and documentation
  evidence durable and trackable. No raw log was archived, moved, or deleted.

## Documentation validation

Baseline: `source bin/activate-hermit && cd documentation && npm test` passed all 16 tests before
edits. Final checks ran on the documentation diff over `b69804da5`:

| Check | Result and scope |
|---|---|
| `source ../bin/activate-hermit && npm run build` in `documentation/` | Passed the complete wrapper, including both external catalog-generation steps and Docusaurus; exported 174 Markdown pages. No generation bypass was needed. |
| `source ../bin/activate-hermit && npm test && npm run typecheck` in `documentation/` | 16 tests passed; TypeScript passed. |
| Local Markdown link/anchor scan over changed and new Markdown | 375 local links and 81 anchors passed across 21 Markdown files, including repository-relative links, local targets of GitHub `blob/main` links, and Docusaurus `/docs/` targets. |
| Rendered HTML inspection | All seven changed site manuals/release pages contain the expected new prose and heading anchors, including the 1.4.0 note and local-reinstall section. |
| `source bin/activate-hermit && cargo fmt --check` at repository root | Passed; no Rust source changed. |
| `git diff --check`, governance-marker checks, scoped manifest/lockfile diff | Passed; agent contracts, source, manifests, and lockfiles remain unchanged. Only docs and the log-tracking negation changed. |
| README routing/size and tracking | Root 261 lines, Desktop 161; existing architecture diagram and MCP configuration command block retained. New note/log indexed; log explicitly unignored. |
| `pnpm list --depth Infinity --json` in `ui/desktop` | Passed and returned valid JSON; validates the documented dependency-enumeration option without changing dependencies. |

An initial build invocation used `source bin/activate-hermit` from `documentation/` and failed
before starting the build. Correcting the activation path to `../bin/activate-hermit` produced the
passing full run above. The docs package has no installed Mermaid parser; the existing README
diagram was unchanged and was not re-parsed. The MCP configuration block, Cargo tree command,
Linux/Windows paths, and provider setup were source/help-reviewed, not exercised as live
install/configuration actions in this documentation pass. No external-link/publication readback,
runtime suite, or deployment is claimed.

## Closure and remaining work

Documentation refresh complete and commit-ready. No commit or push was requested or performed.
The focused active mirror retains 21 rows (13 P0/P1); this is not a new full-backlog assessment.
PKG-GSL-002 now records normal-launch success with fresh-profile and clean-shutdown verification
still open. G210/G213/G215/G216 native checks, product decisions, translated fallback messages,
relative-root recovery, and release gates retain their existing disposition.

Next maintenance action: record the native scenario and clean-shutdown results when those checks
are run, then reconcile their exact gates. A future change to manifests, build recipes, packaging,
or credential/backend behavior should revisit the corresponding installation/manual sections.
