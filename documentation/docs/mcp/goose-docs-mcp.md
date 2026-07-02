---
title: goose Docs Extension

description: Add goose Docs MCP Server as a goose Extension
---

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';
import CLIExtensionInstructions from '@site/src/components/CLIExtensionInstructions';
import GooseDesktopInstaller from '@site/src/components/GooseDesktopInstaller';


This tutorial covers how to add the [goose Docs MCP Server](https://github.com/idosal/git-mcp) as a goose extension to enable goose to answer questions about itself.

:::tip Quick Install
<Tabs groupId="interface">
  <TabItem value="ui" label="goose Desktop" default>
  [Launch the installer](goose://extension?cmd=npx&arg=mcp-remote&arg=https%3A%2F%2Fblock.gitmcp.io%2Fgoose%2F&id=goose-docs&name=goose%20Docs&description=gitmcp%20for%20goose%20documentation)
  </TabItem>
  <TabItem value="cli" label="goose CLI">
  **Command**
  ```sh
  npx mcp-remote https://block.gitmcp.io/goose/
  ```
  </TabItem>
</Tabs>
:::

## Configuration

<Tabs groupId="interface">
  <TabItem value="ui" label="goose Desktop" default>
    <GooseDesktopInstaller
      extensionId="goose-docs"
      extensionName="goose Docs"
      description="GitMCP for goose documentation"
      command="npx"
      args={["mcp-remote", "https://block.gitmcp.io/goose/"]}
      cliCommand="npx mcp-remote https://block.gitmcp.io/goose/"
      timeout={300}
      note="Note that you'll need Node.js installed on your system to run this command, as it uses npx."
    />
 </TabItem>
  <TabItem value="cli" label="goose CLI">
      <CLIExtensionInstructions
        name="goose Docs"
        description="GitMCP for goose documentation"
        command="npx mcp-remote https://block.gitmcp.io/goose/"
        timeout={300}
      />
  </TabItem>
</Tabs>


## Example Usage
GitMCP allows you to convert any Git repository into a goose extension. The goose Docs MCP Server is a great example of this, as it provides a way for goose to answer questions about itself.

Let's ask goose to research its own documentation and write a setup guide for us! It will use the goose Docs MCP Server to find the relevant information.


### goose Prompt

```
hey goose, my development team is new to goose and I want to help them get set up with extensions. Can you research the goose documentation to understand:

1. How extensions are configured (config file, CLI, and Desktop)
2. Which built-in extensions are enabled by default
3. How to add an external MCP server as an extension

Then write a concise setup guide as a reusable skill (a SKILL.md file) that teaches goose to walk a new teammate through enabling the extensions we use.
```


### goose Output

:::note Desktop

I researched the goose documentation using the goose Docs extension and wrote a reusable setup guide as a skill. Here's a summary of what I found and created:

#### 📋 Documentation Research

- **Configuring extensions**: Extensions are managed in `~/.config/goose/config.yaml`, via `goose configure` in the CLI, or from the Extensions page in goose Desktop.
- **Built-in platform extensions**: Extension Manager, Summon, and Todo are enabled by default. Others like Chat Recall and Code Mode can be toggled on as needed.
- **External MCP servers**: Any MCP server can be added as an extension by providing the command (stdio) or URL (http) plus a name and timeout.

#### 🛠️ Generated Skill

I saved the guide as `.agents/skills/team-extension-setup/SKILL.md` so goose can load it whenever a teammate asks for help getting set up.

```markdown
---
name: team-extension-setup
description: Walk a new teammate through enabling our standard goose extensions
---

# Team Extension Setup

Guide the user through this checklist, one step at a time:

1. Verify goose is installed and run `goose configure`
2. Confirm the default platform extensions are enabled (Extension Manager, Summon, Todo)
3. Enable the Developer extension for file and shell access
4. Add our team MCP servers as extensions:
   - GitHub MCP server (requires GITHUB_TOKEN)
   - goose Docs: `npx mcp-remote https://block.gitmcp.io/goose/`
5. Restart the session and ask goose to list enabled extensions to verify

If a step fails, check `~/.config/goose/config.yaml` for typos and confirm
required environment variables are set before retrying.
```

Your teammates can now say "use the team-extension-setup skill" in any session and goose will walk them through the whole setup!
:::
