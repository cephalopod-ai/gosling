# Active TODO ledger

Date: 2026-09-13

[`docs/TODO.md`](../TODO.md) is the canonical backlog. This mirror contains
only items that are still active, partial, blocked, or externally gated.
Completed work remains in the canonical backlog and session logs.

| ID | Status | Priority | Area | Source | Opened | Last evidence | Exit criteria |
|---|---|---|---|---|---|---|---|
| AUD-DAT-001 | needs-decision | P0 | Context privacy | `docs/TODO.md` | 2026-09-13 | 2026-09-13 | Approve workspace-memory scope and migration semantics, then prove isolation and intended recall compatibility. |
| GSL-PT-20260913-006 | needs-design | P3 | Credential lifecycle | `docs/TODO.md` | 2026-09-13 | 2026-09-13 | Record secret ownership/shared references and prove extension removal cannot delete another consumer's credential. |
| AUD-DAT-010 | needs-decision | P3 | Session schema | `docs/TODO.md` | 2026-09-13 | 2026-09-13 | Approve a legacy-column migration and prove fresh/upgraded schema parity. |
| AUD-DAT-012 | needs-evidence | P3 | Workspace concurrency | `docs/TODO.md` | 2026-09-13 | 2026-09-13 | Measure lock contention and approve/test an async boundary without weakening ADR-0001 cross-process locking. |
| ARC-GSL-002 | needs-decision | P1 | Provider architecture | `docs/TODO.md` | 2026-08-15 | 2026-08-27 | Move conversation-domain ownership only through an approved cross-crate architecture change. |
| ARC-GSL-003/004/005 | needs-decision | P1 | Architecture | `docs/TODO.md` | 2026-08-26 | 2026-08-27 | Complete the provider-port, MCP dependency, and process-global-state architecture pass. |
| CMP-GSL-004 | blocked | P1 | Governance | `docs/TODO.md` | 2026-08-26 | 2026-08-27 | Run a successful fresh Giles scan before promoting or changing advisory `.giles` evidence. |
| RSP-GSL-002/003 | blocked | P1 | Supply chain | `docs/TODO.md` | 2026-08-15 | 2026-08-27 | Validate secret scanning and the remaining `cargo-deny` policy sections with approved tooling. |
| RSP-GSL-004 | blocked | P1 | Documentation dependencies | `docs/TODO.md` | 2026-08-27 | 2026-08-27 | Upgrade to a compatible Docusaurus/webpack chain that clears the recorded transitive advisories. |
| CI-GSL-001 | needs-verification | P0 | Shell CI | `docs/TODO.md` | 2026-08-27 | 2026-08-27 | Confirm the locally passing shell conformance changes on Windows, macOS, and Linux runners. |
| CI-GSL-002 | needs-verification | P0 | Windows Rust | `docs/TODO.md` | 2026-08-27 | 2026-08-27 | Confirm the cfg-scoped warning repair on the authoritative Windows runner. |
| PKG-GSL-002 | blocked | P0 | Installed Desktop | `docs/TODO.md` | 2026-09-13 | 2026-09-13 | Resolve or safely bypass the macOS Keychain startup wait, then prove the installed app creates a renderer/backend window and quits cleanly. |
| GILES-GSL-001 | blocked | P2 | Giles | `docs/TODO.md` | 2026-07-20 | 2026-08-27 | Resolve the Giles uniqueness-constraint failure in the external tool. |
| RELEASE-GSL-001 | needs-verification | P0 | Release | `RELEASE_CHECKLIST.md` | 2026-08-23 | 2026-09-13 | Complete every remaining source, GUI, distributable signing/notarization, checksum, scenario, updater, clean-install, and repository-readiness gate. |
| RELEASE-GSL-002 | blocked | P0 | Publication | `docs/TODO.md` | 2026-08-23 | 2026-09-13 | Sign, tag, publish, verify, and announce `v1.2.5` after all release gates pass. |
