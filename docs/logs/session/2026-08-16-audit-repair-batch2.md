# 2026-08-16 — Audit repair, second batch (5 High/Medium defects)

**Branch:** `repair/audit-2026-08-16-batch2` off `main` @ `17fd4a621`
**Skill:** `repair-defect-campaign` (governed_repair)
**Scope:** five remaining High/Medium findings from the 2026-08-15 audit,
selected from the backlog in `docs/TODO.md`.

## Gate 0 — baseline

Working tree clean, `main` in sync with origin. `cargo clippy --all-targets`
0 issues. One pre-existing failure,
`context_mgmt::summarizer::tests::defaults_to_off`, unchanged throughout.

## Gate 1 — inventory

| ID | Domain | Pri | Cx | Touch set |
|---|---|---|---|---|
| STT-GOS-001 | security/correctness | P1 | med | `agents/agent.rs` reply loop; tool dispatch × GoslingMode |
| STT-GOS-005 | security | P2 | low | `config/permission.rs` persist/mutators; permission config file |
| SEC-GOS-012 | security | P1 | low | `gosling-cli/src/cli.rs` serve bind; network bind address |
| ARCN-GSL-001 | correctness/security | P2 | low | `ui/desktop/src/main.ts` CSP handler; lease registry keying |
| SECN-GSL-002 | security/reliability | P2 | low | `ui/desktop/src/main.ts` allowlist fetch; external HTTP |

All five re-verified open against source before editing.

## Gate 2 — grouping and modularization

- **Group A** — STT-GOS-001 + STT-GOS-005. Grouped by data path, not file:
  both are "a security decision silently does not take effect".
- **Group B** — SEC-GOS-012 (serve bind).
- **Group C** — ARCN-GSL-001 + SECN-GSL-002, same file (`main.ts`).

Modularization rule applied: `agent.rs` (5334), `cli.rs` (2534) and `main.ts`
(3410) are all >= 2000 lines, so each was patched minimally and its split stays
**routed** to a dedicated `repair-source-modularization` pass rather than
attempted mid-repair. `permission.rs` (478) was patched in place.

## Results

### Group A — `886c8df8b`

STT-GOS-001: the `frontend_requests` execution loop sat above the
`GoslingMode::Chat` branch, which only skipped `remaining_requests` — so Chat
mode still executed frontend tools. Moved into the `else`, returning the same
`CHAT_MODE_TOOL_SKIPPED_RESPONSE`, preserving the existing carve-out that an
unparseable call surfaces its parse error rather than a successful skip.

STT-GOS-005: `persist` swallowed write failures. It and the `update_*`
mutators now return the error. Making it visible surfaced every site dropping
it; each was handled deliberately — runtime and ACP paths log a security event
naming tool and level, `configure.rs` tells the operator via the cliclack idiom
already used there, and the SmartApprove cache tightening explicitly ignores it
(losing that only costs a recompute), commented at the site.

Two new tests pin the persist-failure contract by denying writes to the config
directory.

**Residual, stated in the commit:** STT-GOS-001 is verified structurally and by
compile — `handle_frontend_tool_request` is unreachable in Chat mode — not by a
runtime test. It sits in a deeply nested async stream with no existing harness.

### Groups B and C — `6a02881fb`

SEC-GOS-012: `--dangerously-unauthenticated` now refuses a non-loopback bind
instead of warning about it. Verified live in all three combinations:
unauth+`0.0.0.0` refused; authenticated+`0.0.0.0` serves (200);
unauth+`127.0.0.1` serves (200).

ARCN-GSL-001: the CSP handler looked up the ACP lease with a *webContents* id
while leases are keyed by `BrowserWindow.id`. The lookup essentially never
matched, so the CSP omitted the local ACP origin and the renderer's own backend
was blocked by policy. Now resolves the window first.

SECN-GSL-002: the allowlist fetch gates what may execute but had no scheme
check, timeout, or size cap. Requires `https`, bounded by an AbortController,
oversized bodies refused before parsing.

## Validation

`cargo clippy --all-targets` 0 issues; gosling lib 1673 passed / 1 pre-existing
failure; gosling-cli 245; desktop `lint:check` clean (tsc, eslint
`--max-warnings 0`, i18n across 15 locales) and 801 tests across 110 files.

## Record closure

`docs/TODO.md` updated: all five marked closed with commit pointers.
