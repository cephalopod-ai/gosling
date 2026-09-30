---
sidebar_position: 6
title: Updating gosling
sidebar_label: Updating gosling
---

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';
import { DesktopAutoUpdateSteps } from '@site/src/components/DesktopAutoUpdateSteps';
import MacDesktopInstallButtons from '@site/src/components/MacDesktopInstallButtons';
import WindowsDesktopInstallButtons from '@site/src/components/WindowsDesktopInstallButtons';
import LinuxDesktopInstallButtons from '@site/src/components/LinuxDesktopInstallButtons';

The gosling CLI and desktop apps are under active and continuous development. To get the newest features and fixes, you should periodically update your gosling client using the following instructions.

:::info Check the version you receive
The current source/local-build version is `v1.4.0` as of 2026-09-29; see the
[build notes](../release-notes/v1.4.0.md). Published channels may carry an earlier
version until their release artifacts are uploaded. Read the notes for the
[published release](https://github.com/cephalopod-ai/gosling/releases/latest), then
run `gosling --version` for the CLI and check **Help > About** in Desktop after updating.
:::

gosling uses its own config, data, session database, keyring service, deep-link scheme, and app identity. Updating gosling must not overwrite or migrate an installed goose application implicitly.

Desktop includes its own backend binary. Replacing the GUI does not update the standalone CLI on
your `PATH`, and updating that CLI does not replace the backend inside the GUI. Verify each one
separately. An ordinary update does not require deleting sessions, configuration, or credentials.

<Tabs>
  <TabItem value="mac" label="macOS" default>
    <Tabs groupId="interface">
      <TabItem value="ui" label="gosling Desktop" default>
        Update gosling to the latest stable version.

        <DesktopAutoUpdateSteps />
        
        **To manually download and install updates:**
        1. <MacDesktopInstallButtons/>
        2. Finish or cancel active work, then quit gosling Desktop completely
        3. Unzip the download and retain a copy of the previous app for rollback
        4. Drag the extracted `Gosling.app` file to the `Applications` folder to replace the app bundle
        5. Launch gosling Desktop and verify **Help > About** and that the chat interface loads

      </TabItem>
      <TabItem value="cli" label="gosling CLI">
        You can update gosling by running:

        ```sh
        gosling update
        ```

        Additional [options](/docs/guides/gosling-cli-commands#update-options):
        
        ```sh
        # Update to latest canary (development) version
        gosling update --canary

        # Update and reconfigure settings
        gosling update --reconfigure
        ```

        Or you can run the [installation](/docs/getting-started/installation) script again:

        ```sh
        curl -fsSL https://github.com/cephalopod-ai/gosling/releases/download/stable/download_cli.sh | CONFIGURE=false bash
        ```

        To check your current gosling version, use the following command:

        ```sh
        gosling --version
        ```
      </TabItem>
    </Tabs>
  </TabItem>

  <TabItem value="linux" label="Linux">
    <Tabs groupId="interface">
      <TabItem value="ui" label="gosling Desktop" default>
        Update gosling to the latest stable version.

        <DesktopAutoUpdateSteps />
        
        **To manually download and install updates:**
        1. <LinuxDesktopInstallButtons/>

        #### For Debian/Ubuntu-based distributions
        2. In a terminal, navigate to the downloaded DEB file
        3. Run `sudo dpkg -i (filename).deb`
        4. Launch gosling from the app menu
      </TabItem>
      <TabItem value="cli" label="gosling CLI">
        You can update gosling by running:

        ```sh
        gosling update
        ```

        Additional [options](/docs/guides/gosling-cli-commands#update-options):
        
        ```sh
        # Update to latest canary (development) version
        gosling update --canary

        # Update and reconfigure settings
        gosling update --reconfigure
        ```

        Or you can run the [installation](/docs/getting-started/installation) script again:

        ```sh
        curl -fsSL https://github.com/cephalopod-ai/gosling/releases/download/stable/download_cli.sh | CONFIGURE=false bash
        ```

        To check your current gosling version, use the following command:

        ```sh
        gosling --version
        ```
      </TabItem>
    </Tabs>
  </TabItem>

  <TabItem value="windows" label="Windows">
    <Tabs groupId="interface">
      <TabItem value="ui" label="gosling Desktop" default>
        Update gosling to the latest stable version.

        <DesktopAutoUpdateSteps />
        
        **To manually download and install updates:**
        1. <WindowsDesktopInstallButtons/>
        2. Unzip the downloaded zip file
        3. Run the executable file to launch the gosling Desktop app
      </TabItem>
      <TabItem value="cli" label="gosling CLI">
        You can update gosling by running:

        ```sh
        gosling update
        ```

        Additional [options](/docs/guides/gosling-cli-commands#update-options):
        
        ```sh
        # Update to latest canary (development) version
        gosling update --canary

        # Update and reconfigure settings
        gosling update --reconfigure
        ```

        Or you can run the [installation](/docs/getting-started/installation) script again in **Git Bash**, **MSYS2**, or **PowerShell** to update the gosling CLI natively on Windows:

        ```bash
        curl -fsSL https://github.com/cephalopod-ai/gosling/releases/download/stable/download_cli.sh | CONFIGURE=false bash
        ```
        
        To check your current gosling version, use the following command:

        ```sh
        gosling --version
        ```        

        <details>
        <summary>Update via Windows Subsystem for Linux (WSL)</summary>

        To update your WSL installation, use `gosling update` or run the installation script again via WSL:

        ```sh
        curl -fsSL https://github.com/cephalopod-ai/gosling/releases/download/stable/download_cli.sh | CONFIGURE=false bash
        ```

       </details>
      </TabItem>
    </Tabs>
  </TabItem>
</Tabs>

## Reinstalling a local macOS build

Build from the repository root with the [Desktop build guide](https://github.com/cephalopod-ai/gosling/blob/main/ui/desktop/README.md#local-macos-arm64-package).
The `just package-ui` recipe rebuilds both the Rust backend and GUI and produces
`ui/desktop/out/Gosling-darwin-arm64/Gosling.app`. It does not install that bundle automatically.

1. Verify the packaged bundle and embedded backend using the checks in the build guide.
2. Finish or cancel active work and quit every gosling window before replacing the installed app.
3. Move the old `/Applications/Gosling.app` to a backup location, then copy the new app into
   `/Applications`. Preserve your existing configuration, credentials, sessions, and Research Library.
4. Open the installed app. Check the version and confirm the navigation, chat composer, and Outputs
   pane render. A successful version command alone does not establish that the GUI started.

If replacement fails before launch, restore the saved app bundle. An app-bundle backup does not
back up or roll back user data; keep a separate data backup before an upgrade that changes storage.
The September 29 reinstall verified normal launch on macOS arm64, with the prior app retained.
Clean shutdown, recovery scenarios, notarization, and updater behavior were separate, unrun checks.

:::info Updating in CI/CD
If you're running gosling in CI or other non-interactive environments, pin a specific version with `GOSLING_VERSION` for reproducible installs. See [CI/CD Environments](/docs/tutorials/cicd) for a complete example and usage details.
:::
