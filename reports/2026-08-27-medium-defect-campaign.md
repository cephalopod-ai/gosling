# Medium defect campaign — final report

## Executive verdict

All Medium findings from the 2026-08-26 clean independent audit now have an
explicit disposition. Twenty-two source findings are repaired, controlled, or
verified against the current tree; ten require an operator/product/security/
architecture decision. Two additional live ACP defects reported during the
campaign were repaired, release-packaged, reinstalled, and exercised in the
installed GUI. No open item is being presented as fixed without evidence.

## Completed work

| Campaign slice | Findings |
|---|---|
| First five | FSR-GSL-002, REL-GSL-002, RES-GSL-001, REC-GSL-001, SECN-GSL-002 |
| Second five | CAS-GSL-001, WFG-GSL-005, NEG-GSL-006, CMP-GSL-001, CMP-GSL-003 |
| Third five | REL-GSL-001, CON-GSL-003, IOP-GSL-002, DAT-GSL-003, WFG-GSL-006 |
| Existing closure reconciled | CON-GSL-002 |
| Completion slice | EAPI-GSL-001, WEB-GSL-001, IOP-GSL-005, AID-GSL-001, XREPO-GSL-001, RST-GSL-001 |
| Live ACP incidents | ACP-GSL-001, ACP-GSL-002 |

The first three slices are detailed in
[`docs/logs/session/2026-08-27-medium-defect-campaign.md`](../docs/logs/session/2026-08-27-medium-defect-campaign.md).
The completion slice added bounded HTTP/update I/O, non-color tool-state
semantics, schema-documentation parity, a Goose converter drift gate, and
workflow-permission reconciliation.

ACP-GSL-001 was SQLite pool starvation: concurrent `BEGIN IMMEDIATE` writers
held pool connections while waiting for the single SQLite write lock. A shared
writer-admission gate now serializes admission before checkout. ACP-GSL-002 was
mode incompatibility: providers that execute tools outside Gosling are now
normalized to Manual/Approve across new, resumed, and provider-switched ACP
sessions, while explicit Auto remains denied.

## Installed application evidence

- `just package-ui` completed and produced an ad-hoc signed arm64 application.
- Packaged and installed backend SHA-256:
  `0f8e91202ad0d48872d8a391a032007bb20438e6f545b9f1d710c3b04fc865bc`.
- `/Applications/Gosling.app` was replaced and signature-verified. The staged
  old bundle was permanently deleted after verification, as authorized.
- No other Gosling backup/archive app, ZIP, or DMG was found in Applications,
  Downloads, or Desktop.
- Installed Solo with Claude Code displayed Manual mode and returned
  `ACP_SOLO_OK`.
- Installed Dual Deep Research persisted, loaded 11 extensions, ran, and
  completed without the original pool timeout.

## Operator-input register

| Finding | Decision required |
|---|---|
| SEC-GSL-003 | Preserve documented remote MCP-app deployments or require loopback-only peers. |
| RSP-GSL-001 | Choose an advisory exception/removal path and provide `cargo-deny` validation. |
| DAT-GSL-002 | Decide whether deleting a workspace may remove project library state still referenced by pinned sessions. |
| NEG-GSL-001 | Define MCP App actor visibility, transcript semantics, and permissions. |
| PATH-GSL-001 | Choose Gosling isolation or shared Goose `~/.agents` discovery. |
| ARC-GSL-003/004/005 | Approve a cross-crate/provider/process-global architecture migration. |
| INV-GSL-001 | Define what authority an imported provider/model/workspace snapshot may reactivate. |
| CMP-GSL-004 | Run a fresh Giles scan and decide whether advisory YAML should become promoted evidence. |
| ACP-GSL-003 | Decide whether external-tool providers are valid Deep Research delegates; if so, design a secure approval-capable subagent channel and enforce ad-hoc Summon arguments. |

ACP-GSL-003 was observed during the installed Dual smoke: the lead supplied
`claude-code/claude-opus-5` as a Summon `source` despite instructions to omit
`source`, and the seat failed as `Source ... not found`. Treating that string as
an ad-hoc source automatically would only reveal the deeper mode conflict:
subagents currently require Auto, while external-tool providers are forbidden
in Auto. The campaign therefore did not weaken the security guard or claim the
seat works.

## Validation

- Focused regressions passed for SQLite writer admission, ACP provider-mode
  normalization, streamable-HTTP timeout, and updater download/extraction
  bounds.
- Desktop Vitest: 132 files / 1,061 tests passed; typecheck passed; changed
  status files pass Prettier.
- Documentation tests: 16/16 passed, including Goose converter parity.
- Release package, code signature, embedded-binary hash, reinstall, launch, and
  installed Solo/Dual ACP boundary probes passed.
- `cargo fmt --all -- --check`, `cargo check` for `gosling` and `gosling-cli`,
  and warning-denying Clippy for both packages/all targets passed.
- `git diff --check` and the required `AGENTS.md` governance-marker check
  passed. `GEMINI.md` is absent, so its optional marker check does not apply.
- Documentation typecheck remains partially validated because unrelated
  pre-existing Docusaurus/JSX errors persist in untouched files.

## Repository-state note

The campaign did not invoke Git commit, push, fetch, merge, or rebase. A separate
repository process advanced `main`/`origin/main` through the implementation and
record commits while the campaign was running, ending at observed
`3fbd0a7bd`. Final review is against that tree plus the remaining report update
listed by `git status` at handoff.
