# Repo health report

- Repo: gosling
- Branch: main
- Remote: https://github.com/cephalopod-ai/gosling.git
- Date: 2026-08-17
- Run mode: staging
- Dirty state: clean at baseline; this run adds bounded polish and report files.
- Audience: public GitHub repository.

## Baseline

| Aspect | Finding |
|---|---|
| Primary languages | Rust and TypeScript |
| Build commands | cargo build; Desktop and documentation commands use Hermit-managed pnpm |
| Test commands | cargo test -p gosling; Desktop Vitest |
| Lint/type commands | cargo fmt --check, cargo clippy --all-targets -- -D warnings, Desktop TypeScript |
| CI present | GitHub Actions workflows are present; current CI run was in progress at inspection |
| License present | Root Apache-2.0 license; Cargo and Desktop manifest align |
| Governance/authority files | AGENTS.md, CONTRIBUTING.md, RELEASE.md, RELEASE_CHECKLIST.md, and docs/INDEX.md |

## Identity

| File | Present | Status | Notes |
|---|---|---|---|
| README | yes | condition | It accurately says the current source build is 0.1.0, but its release links use the inherited upstream identity while this remote has published v1.0.0 and v1.0.1. The repository posture audit already records this ownership conflict. |
| LICENSE | yes | pass | Apache-2.0 aligns with Cargo and the Desktop package. |
| CONTRIBUTING | yes | pass | Development setup and review expectations match the repository tooling. |
| CODE_OF_CONDUCT | no | finding | Missing for a public contribution surface. |
| SECURITY | yes | pass | Present. |
| CODEOWNERS | yes | pass | .github/CODEOWNERS provides a catch-all maintainer route. |

## Structure hygiene

| Path | Issue | Disposition |
|---|---|---|
| ui/desktop/src/platform/windows/bin/uv.exe | Large tracked runtime binary, about 47 MB. | Documented as intentional packaging input; retain. |
| documentation/static/ | Large media assets. | Documented site assets; retain. |
| .hermit/ | Local dependency cache contains backup files. | Ignored and untracked; retain locally. |
| Repository root | .gitattributes only classifies snapshots; it has no line-ending normalization policy. | Advisory portability follow-up; do not add a global normalization policy without maintainer review. |

## Summary

- Strengths: clean Rust formatting and linting, full core suite green, Desktop typecheck and targeted OAuth tests green, pinned third-party actions, and existing collaboration templates.
- Gaps: public branch protection is disabled, documentation typecheck fails, fresh-clone validation was not run, dependency-audit tooling is unavailable, and the source/release identity is unresolved.
- Blockers: release checklist remains entirely open; the remote already publishes later versions than the local source; no release action is safe from this checkout.
