# gosling Desktop App

Native desktop app for gosling built with [Electron](https://www.electronjs.org/) and [ReactJS](https://react.dev/).

For end-user installation and updates, use the [installation manual](../../documentation/docs/getting-started/installation.md)
and [update guide](../../documentation/docs/guides/updating-gosling.md). This page covers contributor
builds. Source version: **1.4.0**; local macOS arm64 packaging, reinstall, and normal launch were
verified on 2026-09-29. Other platforms and release distribution were not tested in that run.

# Building and running

gosling uses [Hermit](https://github.com/cashapp/hermit) to manage dependencies, so you will need to have it installed and activated.

```
git clone git@github.com:cephalopod-ai/gosling.git
cd gosling
source ./bin/activate-hermit
just run-ui
```

Run `just` recipes from the repository root. `just run-ui` builds the release Rust backend, copies
it into `ui/desktop/src/bin/`, installs UI dependencies, and starts Electron. Stop the development
process with `Ctrl+C` after finishing active work. `just run-ui-only` reuses the staged backend;
use it only when that backend already matches the source you intend to run.

| Prerequisite | Evidence |
|---|---|
| Rust 1.91.1 minimum | Required by workspace `rust-version` in [`Cargo.toml`](../../Cargo.toml). |
| Node `^24.10.0`, pnpm `>=10.30.0` | Required by [`package.json`](package.json); activate the repository's Hermit environment. |
| Electron 41.10.3 | Pinned by the Desktop package; installed with UI dependencies. |
| macOS command-line developer tools | Needed by the native build and signing tools; `/Library/Developer/CommandLineTools` was used for the verified local build. |
| Network and package caches | Cargo/pnpm, Electron, and the V8 wrapper may fetch build inputs. Provider accounts and optional MCP runtimes are configured separately. |

[`Cargo.lock`](../../Cargo.lock) and [`ui/pnpm-lock.yaml`](../pnpm-lock.yaml) record the resolved
dependency graphs; enumerate them with `cargo tree --locked` at the root and
`pnpm list --depth Infinity` in `ui/desktop`. The SDK and localization build steps run through the
existing package scripts. No external database service is required for the default local backend.

## Platform-specific build requirements

### Linux

For building on Linux distributions, you'll need additional system dependencies:

**Debian/Ubuntu:**

```bash
sudo apt install dpkg fakeroot
```

**Arch/Manjaro:**

```bash
sudo pacman -S dpkg fakeroot
```

**Fedora/RHEL:**

```bash
sudo dnf install dpkg-dev fakeroot
```

# Building notes

This is an electron forge app, using vite and react.js. The `gosling` backend (`gosling serve`) runs as multi process binaries on each window/tab similar to chrome.

## Localization catalogs

Run `pnpm i18n:extract` after changing desktop messages. Existing message changes and removals stop for explicit locale review; after resolving each locale, acknowledge them with `pnpm i18n:sync -- --accept-source-changes`.

Synchronization retains replaced files under `.i18n-sync-recovery/`. After reviewing a successful synchronization, run `pnpm i18n:recovery:clean` to remove only successful recovery transactions. Rolled-back, conflicted, malformed, and incomplete transactions are never removed by that command.

On macOS, the installed backend is `/Applications/Gosling.app/Contents/Resources/bin/gosling`;
source builds stage it at `ui/desktop/src/bin/gosling`. Updating a standalone CLI does not update
this embedded backend. Repackage and reinstall the GUI after backend changes; editing a signed
bundle in place invalidates its signature.

## Building for different platforms

### Local macOS arm64 package

From the repository root on Apple Silicon:

```sh
source bin/activate-hermit
DEVELOPER_DIR=/Library/Developer/CommandLineTools just package-ui
```

Use the `DEVELOPER_DIR` override when those command-line tools are installed; otherwise select a
working Xcode developer directory. The existing V8 wrapper diagnoses an unusable `ar` rather than
treating it as a corrupt download.

`just package-ui` rebuilds the backend and GUI, then ad-hoc signs
`ui/desktop/out/Gosling-darwin-arm64/Gosling.app` with the repository entitlements. This recipe is
specific to the default arm64 app and does not copy it into `/Applications`.

Verify before installation:

```sh
codesign --verify --deep --strict ui/desktop/out/Gosling-darwin-arm64/Gosling.app
ui/desktop/out/Gosling-darwin-arm64/Gosling.app/Contents/Resources/bin/gosling --version
shasum -a 256 target/release/gosling ui/desktop/out/Gosling-darwin-arm64/Gosling.app/Contents/Resources/bin/gosling
```

The backend version should match `Cargo.toml` and `package.json`, and the two backend hashes should
match. Follow the [local reinstall procedure](../../documentation/docs/guides/updating-gosling.md#reinstalling-a-local-macos-build)
to retain a rollback bundle, replace the stopped app, and verify the installed GUI. The
[September 29 install record](../../docs/logs/session/2026-09-29-gui-install-documentation.md#gui-build-and-install-evidence)
contains the tested revision, hashes, and remaining validation limits.

### macOS distribution bundles

`pnpm run bundle:default` and `pnpm run bundle:intel` are the existing arm64 and Intel bundle
scripts in `ui/desktop`. Signing/notarization is conditional on the `APPLE_TEAM_ID`, `APPLE_ID`,
`APPLE_ID_PASSWORD`, and optional `KEYCHAIN_PATH` configuration in [`forge.config.ts`](forge.config.ts).
A local ad-hoc signature is not a distributable Developer ID signature or notarization result.
Use the [release process](../../RELEASE.md) and [release checklist](../../RELEASE_CHECKLIST.md) for
publication. For custom products, follow [Custom Distributions](../../CUSTOM_DISTROS.md);
`bundle:preconfigured` is no longer a package script.

### Linux

For Linux builds, first ensure you have the required system dependencies installed (see above), then:

1. From the repository root, build and stage the Rust backend:

```bash
source bin/activate-hermit
just release-binary
```

2. Enter the Desktop package and install dependencies:

```bash
cd ui/desktop
pnpm install
```

3. Build the application:

```bash
# For ZIP distribution (works on all Linux distributions)
pnpm run make --targets=@electron-forge/maker-zip

# For DEB package (Debian/Ubuntu)
pnpm run make --targets=@electron-forge/maker-deb

# For Flatpak (requires flatpak and flatpak-builder)
pnpm run make --targets=@electron-forge/maker-flatpak
```

Forge reports the output paths under `ui/desktop/out/` and `out/make/`. These Linux package paths
were source-reviewed, not executed during the September 29 macOS build. See the
[Linux build guide](../../BUILDING_LINUX.md) for native dependencies and platform details.

### Windows

On a Windows host, use `just run-ui-windows` from the repository root to build/stage the MSVC
backend and launch the GUI. Distribution builds use
[bundle-desktop-windows.yml](../../.github/workflows/bundle-desktop-windows.yml). This path was not
executed in the September 29 macOS validation.
