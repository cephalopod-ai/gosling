---
sidebar_position: 20
title: gosling Permission Modes
sidebar_label: gosling Permissions
---

import Tabs from '@theme/Tabs';
import TabItem from '@theme/TabItem';
import { PanelLeft, Tornado } from 'lucide-react';

gosling’s permission mode controls the default treatment of tool calls. It is
one layer in the authorization pipeline: saved tool policy, security checks,
network-egress checks, policy hooks, workspace restrictions, and any active
host-enforced planning boundary can still deny a request or require review.

<details>
  <summary>Permission Modes Video Walkthrough</summary>
  <iframe
  class="aspect-ratio"
  src="https://www.youtube.com/embed/bMVFFnPS_Uk"
  title="gosling Permission Modes Explained"
  frameBorder="0"
  allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
  allowFullScreen
  ></iframe>
</details>

## Permission Modes

| Mode | Description | Best For |
|------|-------------|----------|
| **Completely Autonomous** | The ordinary permission layer does not prompt for unknown or `Ask Before` tools. Saved `Never Allow` rules and stricter security, egress, hook, workspace, and planning decisions still apply. | Users who accept tool execution without routine confirmation |
| **Manual Approval** | gosling asks for confirmation before each ordinary tool call. A confirmation cannot override a denial from a stricter policy layer. | Users who want to review every ordinary tool call |
| **Smart Approval** | gosling may approve a tool only when its read-only classifier and host-side side-effect checks agree; uncertain, sensitive, or explicitly configured tools require approval or are denied. | Users who want low-risk reads automated while retaining review gates |
| **Chat Only** | gosling does not publish gosling-hosted tools to the model. | Users who want a conversational session without gosling-hosted automation |

:::warning
`Autonomous Mode` is the default for new gosling sessions. It is not a promise
that every request will execute: the stricter checks described above remain in
force.
:::

:::caution
Some CLI-backed providers manage and execute their own tools outside gosling.
gosling cannot apply its normal tool-inspection pipeline to those provider-owned
actions. Chat Only therefore suppresses gosling-hosted tools but cannot by
itself make an externally managed provider read-only. Use that provider's own
permission controls, or choose a provider whose tools are executed by gosling.
:::

## Configuring gosling mode

Here's how to configure:

<Tabs groupId="interface">
  <TabItem value="ui" label="gosling Desktop" default>

    You can change modes before or during a session and it will take effect immediately.

     <Tabs groupId="method">
      <TabItem value="session" label="In Session" default>

      Click the <Tornado className="inline" size={16} /> mode button from the bottom menu. 
      </TabItem>
      <TabItem value="settings" label="From Settings">
        1. Click the <PanelLeft className="inline" size={16} /> button on the top-left to open the sidebar.
        2. Click the `Settings` button on the sidebar.
        3. Click `Chat`.
        4. Under `Mode`, choose the mode you'd like.
      </TabItem>
    </Tabs>   
  </TabItem>
  <TabItem value="cli" label="gosling CLI">

    <Tabs groupId="method">
      <TabItem value="session" label="In Session" default>
        To change modes mid-session, use the `/mode` command.

        * Autonomous: `/mode auto`
        * Smart Approve: `/mode smart_approve`
        * Approve: `/mode approve`
        * Chat: `/mode chat`     
      </TabItem>
      <TabItem value="settings" label="From Settings">
        1. Run the following command:

        ```sh
        gosling configure
        ```

        2. Select `gosling settings` from the menu and press Enter.

        ```sh
        ┌ gosling-configure
        │
        ◆ What would you like to configure?
        | ○ Configure Providers
        | ○ Add Extension
        | ○ Toggle Extensions
        | ○ Remove Extension
        // highlight-start
        | ● gosling settings (Set the gosling mode, Tool Output, Tool Permissions, Experiment and more)
        // highlight-end
        └
        ```

        3. Choose `gosling mode` from the menu and press Enter.

        ```sh
        ┌   gosling-configure
        │
        ◇  What would you like to configure?
        │  gosling settings 
        │
        ◆  What setting would you like to configure?
        // highlight-start
        │  ● gosling mode (Configure gosling mode)
        // highlight-end
        │  ○ Router Tool Selection Strategy 
        │  ○ Tool Permission 
        │  ○ Tool Output 
        │  ○ Max Turns 
        │  ○ Toggle Experiment 
        └
        ```

        4.  Choose the gosling mode you would like to configure.

        ```sh
        ┌   gosling-configure
        │
        ◇  What would you like to configure?
        │  gosling settings
        │
        ◇  What setting would you like to configure?
        │  gosling mode
        │
        ◆  Which gosling mode would you like to configure?
        // highlight-start
        │  ● Auto Mode (Full file modification, extension usage, edit, create and delete files freely)
        // highlight-end
        |  ○ Approve Mode
        |  ○ Smart Approve Mode    
        |  ○ Chat Mode
        |
        └  Set to Auto Mode - full file modification enabled
        ```     
      </TabItem>
    </Tabs>
  </TabItem>
</Tabs>

:::info
In Manual Approval and Smart Approval modes, a tool request that needs your
decision appears with Allow and Deny controls. Manual Approval asks for every
ordinary tool call. Smart Approval uses a model-assisted read-only judgment,
but that judgment cannot grant tools that gosling recognizes as having side
effects. Tool annotations may force more review; they cannot grant execution.
:::

## Host-enforced planning

Host-enforced planning is separate from the four permission modes. While a
session has an open plan, gosling publishes a fixed set of bounded read and plan
lifecycle capabilities and checks the same capability again immediately before
dispatch. Ordinary permission grants, tool names, annotations, extensions, and
provider output cannot widen this set. Shell, network, file mutation,
delegation, frontend tools, and direct app tool calls are unavailable during the
planning turn.

Providers that execute tools outside gosling cannot enter host-enforced
planning. Approval applies only to the exact persisted plan generation,
revision, content hash, conversation source, and workspace scope that you
reviewed. Approving a plan records the decision; implementation requires a
separate explicit action.

## CLI Provider Permission Integration

When using [CLI providers](/docs/guides/cli-providers) like Claude Code, gosling integrates with the provider's native permission system. In approve mode, permission requests from Claude Code are routed through gosling's confirmation interface, giving you a unified experience.

For example, with Claude Code in approve mode:
- Claude Code detects sensitive operations (file writes, shell commands, tool calls)
- The permission prompt appears in gosling's interface (CLI or Desktop)
- Your allow/deny decision is sent back to Claude Code
- Claude Code proceeds or adapts based on your response

This integration uses the same mechanism as the official Claude Agent SDKs, ensuring compatibility and consistent behavior.

Claude Code's clarifying questions travel over the same channel but are not approvals: gosling shows them as a question form in every mode, including auto and chat, and sends your choices back as the answers. An unanswered form (declined, dismissed, or left for five minutes) tells Claude Code that no answer arrived, so it does not proceed as if you had been asked and stayed silent.

See [CLI Providers - Claude Code Configuration](/docs/guides/cli-providers#claude-code-configuration) for setup details.
