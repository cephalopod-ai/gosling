# Planning evidence — Gosling shared shell productization

Date: 2026-08-12
Evidence scope: planning artifacts only; no runtime/code/package/release behavior was tested or claimed
Target baseline: `/Users/eric/Work/vscode/forked/gosling`, `main` at `8627dc31a`

## EVD-PLAN-001 — Repository baseline and preservation

Commands run from the Gosling repository:

```bash
git status --short --branch
git rev-parse --show-toplevel
git log -1 --oneline --decorate
```

Observed:

- branch was `main...origin/main`;
- worktree was clean before plan edits;
- target root was `/Users/eric/Work/vscode/forked/gosling`;
- HEAD/origin main was `8627dc31a`, merge of PR #46.

Supports: plan baseline, Gate 0 resume instructions, preservation of existing user work.

## EVD-PLAN-002 — Existing shell and Desktop/release surfaces

Static inspections/searches covered:

- `docs/architecture/shell-foundation.md`;
- `crates/gosling-sdk-types/src/shell.rs`;
- `crates/gosling/src/acp/shell.rs` and shell validation/handlers;
- CLI provisioning and spawned runtime tests;
- `ui/desktop/src/shellHost.ts`, `goslingServe.ts`, `main.ts`, and `preload.ts`;
- shell presentation primitives;
- `ui/desktop/forge.config.ts` and package scripts;
- platform Desktop bundle and release workflows;
- `scripts/with-rusty-v8-cache.sh`, `vendor/v8/Cargo.toml`, and CI Rust test job.

Material observations:

- `createMinimalShellHost` exists but search found no production call site;
- existing shell UI files are primitives, not a complete runtime application;
- actual shell package assets/updater feeds/package artifacts/packaged Electron E2E were documented as deferred;
- Forge currently composes only selected identity fields from environment variables while several resources/metadata remain Gosling-specific;
- Linux CI invokes Cargo directly; an existing trusted V8 archive helper is present but not used by that test job;
- `ui/desktop/src/main.ts` was 3,399 lines at baseline and the repo already tracks modularization as backlog.

Supports: SHP-DEF-001–008, file/module plan, Gate ordering, SHP-RSK-001/003/006/010/018.

## EVD-PLAN-003 — Documentation structural validation

Commands/checks:

```bash
git diff --check
wc -l docs/build/shell-productization/*.md \
  docs/build/shell-productization/audits/*.md
grep -n 'GILES:DOCS-GOVERNANCE:START' AGENTS.md
```

Custom Python checks also:

- counted Markdown fences and rejected odd/unbalanced counts;
- resolved every relative Markdown link from the new package, docs index, TODO, and local session record;
- asserted `SHP-REQ-001` through `SHP-REQ-032` are present in both plan and matrix with no extras/gaps;
- asserted `SHP-INV-001` through `SHP-INV-012` are present;
- searched for unresolved template placeholders.

Observed:

- `git diff --check` passed;
- all fences balanced across 11 checked documents;
- all relative targets existed;
- all 32 requirement IDs and all 12 invariant IDs were complete;
- no unresolved template placeholders remained;
- the main execution plan was 1,027 lines, under the planning skill's 1,200-line document limit;
- the required Giles docs-governance marker remained present.

Supports: planning package structural safety and internal traceability consistency.

## EVD-PLAN-004 — Plan review

The plan was checked against the `plan-prototype-build` controller and the `plan-architecture-invariants` P3 checklist. The review challenged requirement completeness, dependency/reversibility order, trust boundaries, evidence realism, release authority, safe parallelism, and gate decisions.

Observed disposition is recorded in `../audits/plan-review.md`. Improvements added:

- SHP-INV-001–012 checkable invariant registry;
- capability/intent map;
- safe parallelism and same-file conflict restrictions;
- relative gate complexity/cost drivers;
- GO/PATCH/REPLAN/BLOCKED/STOP gate protocol;
- explicit non-self-certification and final audit handoff.

Supports: plan readiness for Gate 0 handoff only.

## EVD-PLAN-005 — Ignored local session record

`git check-ignore -v docs/logs/session/2026-08-12-shell-productization-plan.md` reported the repository's `.gitignore` `logs/` rule. The session record was created locally per governance guidance but is not part of the tracked diff. The durable tracked planning record is this namespaced plan package and its `build-state.md`.

Supports: honest artifact-placement/readback status; no claim that the ignored log will synchronize.

## Evidence limitations

- Untracked plan files do not appear in ordinary `git diff --stat`; their existence/content/line counts were checked directly.
- No Rust, SDK, Desktop, package, workflow, signing, release, or updater behavior changed, so their test suites were not run for this documentation-only task.
- The Linux V8 failure evidence includes prior GitHub run inspection plus current static workflow/helper inspection; a fresh clean reproduction is deliberately Gate 0 work.
- This planning review does not certify future implementation conformance.
