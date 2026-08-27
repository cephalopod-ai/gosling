# all remaining TODOs repair campaign

Date: 2026-08-27  
Branch: `codex/repair-all-remaining-todos`  
Baseline: `48946f41585c1d34456614bf0ccba0f948852e6e`

## Outcome

The campaign repaired every source-confirmed item in the canonical TODO and
active release ledgers that did not require a product, security, architecture,
release, external-service, or platform-owner decision.

Completed work:

- restored documentation typecheck, tests, and production build;
- aligned Docusaurus runtime/type dependencies and removed all critical npm
  audit advisories;
- repaired the broken v1.1.0 release-checklist link;
- reconciled stale version, identity, path, provider, chat-scroll, CLI-usage,
  and performance records;
- updated current repository URLs, updater/release metadata, OIDC policy, and
  repository-gated workflows to `cephalopod-ai/gosling` while preserving
  historical evidence, npm package scopes, frozen bundle identities, and the
  pinned legacy container digest;
- repaired shell consumer extension-capability validation and cross-platform
  test portability;
- cfg-scoped Windows-only Rust warnings discovered in current CI;
- added the scoped documentation TODO and refreshed release, security, and
  validation ledgers.

## Remaining unique actionable records

There are **40 unique remaining records** across the canonical engineering TODO
and documentation TODO after removing mirrors and the explicitly closed
`SECN-GSL-001` warning:

| Class | Count | Disposition |
|---|---:|---|
| Product/security/data decisions | 8 | Needs maintainer/architecture input. |
| Architecture, external-tool, dependency, and profile prerequisites | 8 | Routed or blocked on an unavailable/future prerequisite. |
| Remote cross-platform confirmation | 2 | Local repairs pass; next published revision must rerun CI. |
| Release/external repository gates | 3 | Maintainer-owned signing, publication, branch protection, webhook, and release evidence. |
| Tagteam feature program | 16 | Explicit deferred feature horizon. |
| Source modularization | 1 | Routed because files exceed the repair campaign's 2,000-line split ceiling. |
| Session Handoff feature backlog | 1 | Feature work, not a defect patch. |
| `.dory/` governance | 1 | Needs a repository-retention policy decision. |

The two open rows in `documentation/TODO.md` are one mirror of `RSP-GSL-004`
and one additional `.dory/` governance decision; only the latter increases the
unique count.

## Exact deferrals requiring input or external state

- Workspace deletion semantics, MCP App actor/authority, imported snapshot
  authority, Deep Research delegate seats, remote goslingd posture, session
  leasing, and MCP guest loopback policy require explicit product/security or
  architecture decisions.
- Provider/domain-crate/dependency/global-state migrations require a governed
  architecture pass.
- `cargo-deny`, `cargo-audit`, Giles, gitleaks, and trufflehog are unavailable;
  no unvalidated CI policy was invented.
- Docusaurus/webpack must publish a compatible chain that clears the remaining
  25 transitive npm advisories.
- PERF-GSL-003 requires the specified wall-time profile before further change.
- Tagteam, Session Handoff, and large-file modularization are separately routed
  feature/refactor programs.
- Release publication, signing, checksums, clean-install acceptance, branch
  protection, BuildNotify secret/no-secret behavior, and remote CI reruns are
  maintainer/external gates.
- `.dory/` retention is a repository-governance decision.

## Validation

- Rust format, build, targeted crate suites, and all-target Clippy passed.
- Desktop typecheck and 1,069 tests passed.
- Shell profile/conformance passed 67 tests and three profile checks.
- Documentation typecheck, 16 tests, and production build passed.
- OIDC proxy passed 11 tests.
- Windows cross-compilation was only partial because the macOS host lacks a
  Windows C SDK; real Windows CI remains required.

No push, merge, tag, release, deployment, branch-protection change, webhook
change, signing action, or publication was performed.
