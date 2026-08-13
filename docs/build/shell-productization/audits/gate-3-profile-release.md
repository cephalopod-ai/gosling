# Gate 3 audit — profile, configuration, path, and release identity

Date: 2026-08-12
Scope: Gate 3 resolver, fixtures, Forge projection, generated manifest, scripts, and CI wiring
Verdict: **GO after implementation corrections**

## Findings and dispositions

| ID | Severity | Finding | Disposition |
| --- | --- | --- | --- |
| SHP-G3-AUD-001 | high | Initial collision logic compared values across unrelated identity fields, so one profile ID equaling another profile namespace would fail while same-field intent was obscured | Fixed: ten separately named same-field maps with normalized case where platform IDs require it; exhaustive per-field and cross-field regression |
| SHP-G3-AUD-002 | high | Initial asset code treated `iconBase` as a file, contradicting the frozen stem contract | Fixed: target-specific `.icns`, `.ico`, `.png`, and `.svg` inventory from a contained stem |
| SHP-G3-AUD-003 | high | Initial Forge integration could consume Windows signing variables for a non-publishable fixture | Fixed: one source-derived `signingAllowed` guard disables macOS signing/notarization and Windows signing; hostile environment regression passes |
| SHP-G3-AUD-004 | high | A Forge-selected profile initially embedded source profile data but not the generated manifest required for artifact traceability | Fixed: Forge projection generates and embeds canonical `profile.json` and `manifest.json` from ignored `build/` output |
| SHP-G3-AUD-005 | medium | JSON.parse alone silently accepts duplicate keys | Fixed: dependency-free duplicate-key parser used for profile and provisioning with non-reflective diagnostics |
| SHP-G3-AUD-006 | medium | Presence/extension checks alone could accept malformed or wrong-dimension icon content | Fixed: bounded header/structure and square-dimension checks for PNG/ICO/ICNS/SVG; malformed/dimension regressions pass |
| SHP-G3-AUD-007 | medium | A CLI missing-profile error could leak the caller-supplied path, including secret-shaped content | Fixed: `resolveProfiles` routes through the redacted resolver error; CLI negative test asserts no path reflection |
| SHP-G3-AUD-008 | medium | Initial output resolution interpreted a relative `--output` against caller CWD and concurrent writers shared one temporary filename | Fixed: relative output resolves against repository root and randomized atomic temporary names avoid caller/process ambiguity |
| SHP-G3-AUD-009 | medium | Cross-architecture Forge selection initially used host architecture instead of `ELECTRON_ARCH` | Fixed: projection honors `ELECTRON_ARCH`; macOS x64 regression passes |
| SHP-G3-AUD-010 | medium | Refactoring the Flatpak icon to one `iconPng` value would have changed default Gosling from `icon-512.png` to `icon.png` | Fixed: separate `iconFlatpak512` preserves default parity while profiles use their validated PNG |

## Security and negative-space checks

- Profile input cannot select runtime settings, credentials, providers/models, extensions/skills,
  denied methods, domain adapters, prompts, actions, payloads, or handoffs.
- Paths cannot be absolute, traverse, use backslashes/NULs, leave approved roots, resolve through an
  external symlink, or substitute a wrong file type.
- Fixture A/B source audit rejects named domain products/semantics and release activation fields.
- Fixture environment variables cannot enable publisher/updater/signing/notarization.
- Production release destination allowlist is intentionally empty; no production activation is
  possible in this gate.
- Generated outputs contain safe identity, profile hash, target/schema/core revision, and no source
  paths or secret/profile payload beyond the canonical reviewed profile resource.
- Default full-Gosling name, executable, protocol, resources, publisher, icons, and signing behavior
  are asserted by a Forge configuration regression.

## Residual risks

- Package-level readback, manifest tamper detection, updater feed isolation, and actual installed
  coexistence need Gate 6/7 artifacts.
- Dedicated shell Vite entries and minimal preload/process ownership need Gate 4.
- Profile asset inspection is intentionally a narrow structural validator, not a full image decoder;
  target packagers remain the final format authority.
- No production destination/signing policy can be exercised until a human authorizes and configures
  one.

No P0/P1 Gate 3 implementation finding remains open after the listed corrections. Broader
requirements stay `built`, not final-revision `verified`, until their package/runtime gates pass.
