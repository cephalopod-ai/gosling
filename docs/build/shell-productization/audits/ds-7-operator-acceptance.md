# DS-7 operator acceptance record

Date: 2026-08-18

Decision: **ACCEPTED — generic Default Shell GUI design is authorized. Gates 1 and 2 of the
`plan-webapp-design` workflow may begin.**

Accepted revision: `240ab751585afc03c68a710f8be10ea891ab168f` (merged `main`; identical source tree
to the corrective candidate `259935f01b1fbf0bcffcb17f21a01a7f9c2548fc`)

Recorded against branch head: `437d7bd7d7866356ddd3eb6feb0c32b52b4e8528`

Authority: ADR-0014, [`../../../architecture/default-shell-template.md`](../../../architecture/default-shell-template.md),
and [`default-shell-pre-gui-corrective-audit.md`](default-shell-pre-gui-corrective-audit.md).

## What this record does

The 2026-08-15 corrective audit closed every DS-1–DS-6 requirement and issued a technical GO
recommendation. It explicitly did not record operator acceptance, and it named that decision as the
only remaining DS-7 condition. This document is that decision.

## What is authorized

- `plan-webapp-design` Gate 1 — product and workflow design for the generic Default Shell.
- `plan-webapp-design` Gate 2 — front-end handoff specification for the same.

Both gates are design-only. They produce documents and reviewable non-shipping artifacts. They add
no renderer source, no host source, no new IPC channel, no new ACP method, and no new capability.

## What remains unauthorized

- `plan-webapp-design` Gates 3–6 (build, integrate, validate, release). Gate 3 has its own entry
  condition, recorded below as SHP-DEF-053.
- Every named project shell — DAWES, math, Project ABC, Physics/CST, or any other. The
  named-shell start policy in `build-state.md` is unchanged: named implementation begins only after
  milestone M5, when the reusable generic GUI and its conformance workflow have passed.
- R6–R8: packaged restart/coexistence harnesses, cross-platform workflow coverage, and
  signing/notarization/release/publication.
- Any widening of the v1 capability envelope in `default-shell-template.md`. A design that needs a
  capability the envelope excludes is a plan-change request, not a design detail.

## Accepted evidence

Reproduced from the corrective audit rather than re-run for this record:

| Check                                            | Result                                                                              |
| ------------------------------------------------ | ----------------------------------------------------------------------------------- |
| Exact-source Rust workspace                      | pass, system-keyring feature enabled, no disable override                           |
| Exact-source Desktop suite                       | pass 795/795                                                                        |
| Fixture profile resolution                       | all resolve the corrective commit with `sourceClean:true`                            |
| macOS arm64 package/readback                     | profile hash `830f6143…82950`, backend hash `76b812b5…3d6574`                        |
| Packaged renderer-to-backend startup replay      | `ready` in 23 ms, only `core:session` exposed, stop in 6 ms, backend exit code 0     |
| Packaged close/restart cleanup                   | no surviving shell backend, empty product-local process registry                     |
| Full-Gosling-plus-two-shell coexistence          | pass on the supported host                                                           |
| Exact-head CI and merged-main CI rerun           | green (one unrelated i18n lock-test race passed on rerun)                            |
| Main Live Provider Tests                         | green                                                                               |
| Open critical/high Default Shell findings        | none                                                                                |

## Conditions attached to this acceptance

1. **Revision drift is acknowledged, not waived.** `main` has advanced 76 commits from the accepted
   revision to `437d7bd7d`, and `6634ece38` ("fix(acp): carry the ACP secret in the WebSocket
   subprotocol, not the URL") touched `ui/desktop/src/shell/acpRuntime.ts`,
   `ui/desktop/src/shell/runtimeController.ts`, and `ui/desktop/src/shellHost.test.ts`. Gates 1–2
   are design-only and cannot be invalidated by that change, so they proceed. Gate 3 must not start
   until the DS-7 check battery is reproduced on a current clean revision with current CI. Tracked
   as SHP-DEF-053.
2. **Gemini OAuth remains a known provider defect.** The corrective audit recorded an
   `Internal error` on Gemini OAuth configuration and did not investigate it. Credential-selection
   design may proceed, but no claim of a polished credential/relink experience is permitted until
   the `docs/TODO.md` item is closed.
3. **Design must not invent surface.** Every operation, event, field, failure code, and bound in the
   Gate 1 and Gate 2 documents must trace to committed source. Anything the renderer needs and the
   host does not expose is recorded as a gap for Gate 3, not assumed into existence.

## Next gate

`plan-webapp-design` Gate 3 (build). Entry conditions: SHP-DEF-053 closed, Gate 1 and Gate 2
accepted, and no new critical or high finding open against the Default Shell foundation.
