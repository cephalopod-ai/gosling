# Shared shell product profiles

Focused shell packaging is driven by one reviewed JSON profile. The profile owns product/build
identity and distribution policy; the referenced Rust provisioning document remains the
runtime/session/policy authority. Profiles cannot contain credentials, secrets, workspaces,
providers, models, extension/skill selection, denied methods, domain payloads, prompts, or actions.

The binding schema and security boundaries are in
[`architecture/shell-productization-contracts.md`](architecture/shell-productization-contracts.md)
and [ADR-0007](adr/0007-shell-product-profile.md).

## Approved roots

Source-controlled profiles and all referenced provisioning/assets must remain under one of:

- `shell-products/` for future operator-approved non-fixture profiles;
- `fixtures/shell-products/` for neutral, permanently non-publishable test profiles.

A profile file is named `product-profile.json`. Paths inside it are repository-relative with `/`
separators. Absolute paths, traversal, backslashes, NULs, missing/wrong-type files, unapproved
roots, and symlink escapes fail before Forge starts. Required target icons are inspected as
PNG/ICO/ICNS/SVG rather than trusted by extension alone.

## Check and resolve

Activate the repository environment, then run commands from `ui/desktop`:

```bash
source ../../bin/activate-hermit
pnpm run shell:check-profile check ../../fixtures/shell-products/fixture-a/product-profile.json
pnpm run shell:check-profiles
pnpm run shell:resolve-profile resolve ../../fixtures/shell-products/fixture-a/product-profile.json --target macos-arm64
pnpm run shell:test-profile
```

Resolution writes deterministic canonical `profile.json` and `manifest.json` files under
`build/shell-profiles/<product-id>/<target>/`. `build/` is ignored. The manifest records the
profile hash, exact checkout revision, target, architecture, schema versions, required methods,
and safe product identity. A publishable manifest additionally requires a clean checkout.

To load a reviewed profile through Forge:

```bash
GOSLING_SHELL_PROFILE=../../fixtures/shell-products/fixture-a/product-profile.json pnpm exec electron-forge package
```

The fixture profiles are only packaging/configuration fixtures at this gate; the dedicated shell
main/preload/renderer entry arrives in the next implementation gate. Using a fixture profile cannot
enable updater publication, macOS signing/notarization, or Windows signing, even if credential
environment variables are present.

## Add a profile

1. Copy the strict field shape from a neutral fixture without copying its identity values.
2. Give every identity-sensitive field a globally unique value across approved roots.
3. Add a matching provisioning document whose `identity.id`, `displayName`, and `version` exactly
   equal the product fields.
4. Add every icon required by `assets.requiredTargets` under the declared asset root.
5. Keep `publishable: false`, updater disabled, no destination, and `signingPolicy: none` unless a
   separate operator-approved production release decision supplies identifiers, destination, and
   signing authority.
6. Run `shell:test-profile`, `shell:check-profiles`, Desktop typecheck, and Forge load validation.

Production release destinations are intentionally not configured. Adding one is a human-governed
release decision, not a profile-only change. Do not implement domain behavior in this shared
profile/fixture layer.
