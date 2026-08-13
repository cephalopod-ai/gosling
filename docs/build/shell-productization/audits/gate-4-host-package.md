# Gate 4 checkpoint audit — host, package, and integrity boundary

Date: 2026-08-13
Scope: shared shell identity/lifecycle/IPC/ACP/bootstrap/resources/diagnostics/handoff and local
package/readback path
Verdict: **CONTINUE Gate 4 after corrections**

## Findings and dispositions

| ID             | Severity | Finding                                                                                                                                        | Disposition                                                                                                                                                                                        |
| -------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| SHP-G4-AUD-001 | critical | A package could copy ignored, stale `ui/desktop/src/bin/gosling` while the generated manifest claimed the current Git revision                 | Fixed: tracked wrapper builds one exact host-target CLI artifact, stages it, packages it, and rejects any packaged-binary hash mismatch; positive and tamper tests plus real package readback pass |
| SHP-G4-AUD-002 | high     | Fixture macOS bundle IDs contained underscores that Electron Packager silently stripped, so package metadata could not equal the profile       | Fixed: fixture IDs use hyphens; profile validation rejects characters outside Packager's stable alphanumeric/dot/hyphen set; real plist readback now equals the profile exactly                    |
| SHP-G4-AUD-003 | high     | A custom Vite renderer root emitted the shell renderer outside Forge's package staging tree                                                    | Fixed before checkpoint: root-level `shell.html` and dedicated renderer input produce `.vite/renderer/shell_window/shell.html` inside `app.asar`; verifier requires it                             |
| SHP-G4-AUD-004 | high     | Initial package inspection did not prove that the broad Desktop preload was absent                                                             | Fixed: verifier requires dedicated `shell-preload.js`, forbids broad `preload.js` and `main_window`, and scans preload/renderer files for backend URL/secret and broad-authority sentinels         |
| SHP-G4-AUD-005 | medium   | Package resources could be internally consistent but not equal the selected source profile/provisioning/current checkout                       | Fixed: verifier resolves the approved profile again and compares exact canonical profile, complete generated manifest, provisioning bytes, target, revision, and source state                      |
| SHP-G4-AUD-006 | medium   | The package wrapper accepted target pairs that could cause Cargo cross-compilation and Forge packaging without a proven runnable host artifact | Fixed: local wrapper accepts only the current host platform/architecture; cross-platform package construction remains explicit CI/Gate 7 work                                                      |
| SHP-G4-AUD-007 | medium   | Bootstrap test harness used six explicit `any` types, preventing a zero-warning focused lint checkpoint                                        | Fixed: harness listeners/IPC use unknowns and adapter-derived types; focused shell ESLint passes with zero warnings                                                                                |
| SHP-G4-AUD-008 | medium   | Asar's process cache could hide mutations in verifier tamper tests                                                                             | Fixed: verifier uncaches the archive before every list/read pass; broad-preload and sentinel tamper tests fail closed                                                                              |

## Boundary observations

- Renderer/preload receive lifecycle, diagnostics, handoff, and allowlisted external-open operations
  only; backend URL, generated secret, profile/provisioning paths, runtime namespace, filesystem,
  provider/settings, updater, and release authority remain in main or absent.
- Compatibility is main-owned and runs before any session seam. Current code does not create a
  session, so no compatibility failure can create one through this path.
- Package wrapper rejects publishable, update-enabled, or signing-required profiles. It clears
  release-signing environment inputs and invokes Forge package only; it has no make/publish/upload
  path.
- The macOS arm64 package has an ad-hoc signature because the existing Forge fuses plugin rewrites
  Electron and restores the mandatory local signature. It has no team identifier and is not a
  release signature, notarization, or publication action.
- Diagnostics tests cover secret/path/URL redaction, circular input, bounds, explicit gesture,
  atomic owner-private writes, overwrite refusal, and temporary-file cleanup.
- Handoff preparation is canonical server output retained exactly and consumed once after explicit
  confirmation. Full Desktop receiving is still absent and is not represented as complete.

## Residual findings

- Session create/resume and deterministic neutral probe are required before Gate 4 can close.
- Full Desktop handoff receiving must be routed through a focused module without adding domain or
  lifecycle behavior to oversized `main.ts`.
- Renderer recovery/accessibility, packaged restart/coexistence, and cross-platform package
  readback remain later gates.
- Repository-wide format check has an unrelated 62-file baseline; touched files pass the focused
  check.

No unresolved defect in this checkpoint permits stale binary/profile substitution, broad preload
packaging, updater inclusion, or transformed macOS bundle identity to pass readback.
