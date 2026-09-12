---
sidebar_position: 8
title: CLI Providers
sidebar_label: CLI Providers
description: Use Claude Code, Cursor Agent, Gemini CLI, or Antigravity subscriptions in gosling
---

# CLI Providers

:::warning Deprecated — Use ACP Providers
The remaining direct CLI providers are deprecated. Use the [ACP providers](/docs/guides/acp-providers) (`claude-acp`, `codex-acp`) when available, since they support gosling extensions through the standardized Agent Client Protocol. The legacy direct `codex` provider was removed in `v1.2.5`; use `codex-acp` or `chatgpt_codex`. For Gemini, use `Google Gemini (API Key)` with `GOOGLE_API_KEY`.
:::

gosling can use pass-through providers that integrate with existing CLI tools from Anthropic, Cursor, and Google. These providers add gosling's session management, persistence, and workflow surfaces around the external agent.

:::warning Limitations
These providers don’t fully support all gosling features, may have platform or capability limitations, and can sometimes require advanced debugging if issues arise. They’re included here purely as a convenience.
:::

## Why Use CLI Providers?

CLI providers are useful if you:

- already have a Claude Code, Cursor, or Google Gemini CLI subscription and want to use it through gosling instead of paying per token
- need session persistence to save, resume, and export conversation history
- prefer unified commands across different AI providers
- want to [use multiple models together](#combining-with-planner-models) in your tasks

### Benefits

#### Session Management
- **Persistent conversations**: Save and resume sessions across restarts
- **Export capabilities**: Export conversation history and artifacts
- **Session organization**: Manage multiple conversation threads

#### Workflow Integration  
- **Hybrid configurations**: Combine with planning mode and model-specific workflows

#### Interface Consistency
- **Unified commands**: Use the same `gosling session` interface across all providers
- **Consistent configuration**: Manage all providers through gosling's configuration system

:::warning Extensions
CLI providers do **not** give you access to gosling's extension ecosystem (MCP servers, third-party integrations, etc.). They use their own built-in tools to prevent conflicts. If you need gosling's extensions, use standard [API providers](/docs/getting-started/providers#available-providers) instead.
:::


## Available CLI Providers

### Claude Code

The Claude Code provider integrates with Anthropic's [Claude CLI tool](https://claude.ai/cli), allowing you to use Claude models through your existing Claude Code subscription.

**Features:**
- Uses Claude's latest models
- 200,000 token context limit
- Automatic filtering of gosling extensions from system prompts (since Claude Code has its own tool ecosystem); stdio and streamable-http extensions are handed to the CLI as MCP servers
- Streaming JSON (NDJSON) protocol for persistent, multi-turn sessions
- Clarifying questions from Claude Code appear as a form in gosling, in every mode
- Terminal CLI errors are shown as provider failures instead of completing as an empty response

**Requirements:**
- Claude CLI tool installed and configured
- Active Claude Code subscription
- CLI tool authenticated with your Anthropic account

### Cursor Agent

The Cursor provider integrates with Cursor's [CLI agent](https://docs.cursor.com/en/cli/installation), providing access to through your existing subscription.

**Features:**

- integrates with Cursor Agent CLI coding tasks.
- ideal for code-related workflows and file interactions.

**Requirements:**

- cursor-agent tool installed and configured.
- CLI tool authenticated.

### Gemini CLI

The Gemini CLI provider integrates with Google's [Gemini CLI tool](https://ai.google.dev/gemini-api/docs), providing access to Gemini models through your Google AI subscription.

**Features:**
- 1,000,000 token context limit

**Requirements:**
- Gemini CLI tool installed and configured
- CLI tool authenticated with your Google account

### Antigravity

The Antigravity provider drives Google's agentic coding CLI (`agy`) headless over its
`stream-json` protocol, reusing the Google sign-in the Antigravity CLI or IDE already holds.

**Features:**
- Gemini 3.x, Claude 4.6, and GPT-OSS models served through one Antigravity sign-in
- 1,048,576 token context limit on the Gemini models
- One persistent CLI process per session, so Antigravity keeps its own conversation state and prompt cache across turns

**Requirements:**
- `agy` installed and on your PATH
- CLI signed in with your Google account
- gosling mode set to `auto` (see the limitation below)

:::warning Auto mode only
Antigravity's headless mode has no channel for handing an approval back to the caller —
a tool its own `toolPermission` setting will not clear is soft-denied rather than surfaced.
gosling therefore accepts this provider only in `auto` mode and refuses `approve`,
`smart-approve`, and `chat` with an explicit error rather than silently degrading them.
:::

## Setup Instructions

### Claude Code

1. **Install Claude CLI Tool**
   
   Follow the [installation instructions for Claude Code](https://docs.anthropic.com/en/docs/claude-code/overview) to install and configure the Claude CLI tool.

2. **Authenticate with Claude**
   
   Ensure your Claude CLI is authenticated and working

3. **Configure gosling**
   
   Set the provider environment variable:
   ```bash
   export GOSLING_PROVIDER=claude-code
   ```
   
   Or configure through the gosling CLI using `gosling configure`:

   ```bash
   ┌   gosling-configure 
   │
   ◇  What would you like to configure?
   │  Configure Providers 
   │
   ◇  Which model provider should we use?
   │  Claude Code 
   │
   ◇  Model fetch complete
   │
   ◇  Enter a model from that provider:
   │  default
   ```
### Cursor Agent

1. **Install Cursor agent Tool**

   Follow the [installation instructions for Cursor Agent](https://docs.cursor.com/en/cli/installation) to install and configure the cursor agent tool.

2. **Authenticate with Cursor**

   Ensure your Cursor Agent is authenticated and working

3. **Configure gosling**

   Set the provider environment variable:

   ```bash
   export GOSLING_PROVIDER=cursor-agent
   ```

   Or configure through the gosling CLI using `gosling configure`:

   ```bash
   ┌   gosling-configure
   │
   ◇  What would you like to configure?
   │  Configure Providers
   │
   ◇  Which model provider should we use?
   │  Cursor Agent
   │
   ◇  Model fetch complete
   │
   ◇  Enter a model from that provider:
   │  default
   ```

### Gemini CLI

1. **Install Gemini CLI Tool**
   
   Follow the [installation instructions for Gemini CLI](https://blog.google/technology/developers/introducing-gemini-cli-open-source-ai-agent/) to install and configure the Gemini CLI tool.

2. **Authenticate with Google**
   
   Ensure your Gemini CLI is authenticated and working.

3. **Configure gosling**
   
   Set the provider environment variable:
   ```bash
   export GOSLING_PROVIDER=gemini-cli
   ```
   
   Or configure through the gosling CLI using `gosling configure`:

   ```bash
   ┌   gosling-configure 
   │
   ◇  What would you like to configure?
   │  Configure Providers 
   │
   ◇  Which model provider should we use?
   │  Gemini CLI 
   │
   ◇  Model fetch complete
   │
   ◇  Enter a model from that provider:
   │  default
   ```

### Antigravity

1. **Install the Antigravity CLI**

   `agy` ships with the Antigravity IDE and the Antigravity VS Code extension. Run
   `agy install` once to put it on your PATH and configure shell settings.

2. **Sign in**

   Run `agy` in a terminal, complete the Google sign-in, then exit. The credential is
   cached under `~/.gemini/`, and gosling's headless spawns reuse it — gosling cannot
   complete the sign-in itself, because Antigravity's interactive OAuth needs a
   controlling terminal.

   Confirm headless access works before configuring gosling:

   ```bash
   agy models
   ```

   It should list models without prompting. If it does not, gosling reports an
   authentication error pointing back at this step.

3. **Configure gosling**

   ```bash
   export GOSLING_PROVIDER=antigravity
   export GOSLING_MODEL=gemini-3.1-pro-high
   export GOSLING_MODE=auto
   ```

4. **Trust the workspace**

   Antigravity gates access on its own `trustedWorkspaces` list. Open the directory once
   in the Antigravity IDE or interactive CLI and accept the trust prompt, or set
   `allowNonWorkspaceAccess` in `~/.gemini/antigravity-cli/settings.json`. gosling does not
   modify that file.

## Usage Examples

### Basic Usage

Once configured, you can start a gosling session using these providers just like any others:

```bash
gosling session
```

### Combining with Planner Models

CLI providers also work well with planning mode when you want one model for strategy and another for execution:

```bash
# Use Claude Code for execution, OpenAI for planning
export GOSLING_PROVIDER=claude-code
export GOSLING_MODEL=default
export GOSLING_PLANNER_PROVIDER=openai
export GOSLING_PLANNER_MODEL=gpt-4o

gosling session
```

## Configuration Options

### Claude Code Configuration

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `GOSLING_PROVIDER` | Set to `claude-code` to use this provider | None |
| `GOSLING_MODEL` | Model to use. gosling offers the models the installed `claude` CLI advertises: `claude-opus-5`, `claude-sonnet-5`, `claude-fable-5-1` or `claude-fable-5` (whichever your CLI serves), and `claude-haiku-4-5` | `default` |
| `CLAUDE_CODE_COMMAND` | Path to the Claude CLI command | `claude` |

**Known Models:**

gosling asks the installed `claude` CLI which models it serves and offers the current names it recognizes (`claude-opus-5`, `claude-sonnet-5`, `claude-fable-5-1` or `claude-fable-5`, `claude-haiku-4-5`). The selected name is passed to the CLI with `--model`; `default` leaves the CLI's own default in place.

**Permission Modes (`GOSLING_MODE`):**

Every mode runs the CLI with `--permission-prompt-tool stdio`, so its permission requests arrive on gosling's control channel instead of being decided inside the CLI.

| Mode | Claude Code Flags | Behavior |
|------|-------------------|----------|
| `auto` | `--permission-prompt-tool stdio` | Tool requests are approved automatically without a prompt |
| `smart-approve` | `--permission-prompt-tool stdio` | Tool requests are shown as allow/deny prompts in gosling |
| `approve` | `--permission-prompt-tool stdio` | Tool requests are shown as allow/deny prompts in gosling |
| `chat` | `--permission-mode plan --permission-prompt-tool stdio` | The CLI stays read-only; any tool that would change something is denied without a prompt |

**Clarifying questions:**

When Claude Code asks a question (its `AskUserQuestion` tool), gosling shows the question as a form in every mode, including `auto` and `chat`: one dropdown per single-choice question and checkboxes for multi-select questions. Your choices are returned to the CLI as the answers. If you decline, dismiss, or leave the form unanswered for five minutes, the CLI is told that no answer arrived so the model states its assumption or repeats the question in text rather than reporting that you ignored it.

:::tip Approve Mode Integration
When using `approve` or `smart_approve` mode with Claude Code, gosling routes Claude Code's permission prompts through gosling's confirmation interface. This means:

- **Sensitive operations** (file writes, shell commands, etc.) trigger approval prompts in gosling
- **You review and approve/deny** directly in the gosling CLI or Desktop interface
- **Denied operations** are communicated back to Claude Code, which adapts accordingly

This provides a consistent permission experience across all gosling providers while leveraging Claude Code's built-in safety checks.

Example with approve mode:
```bash
GOSLING_PROVIDER=claude-code GOSLING_MODE=approve gosling session
```
:::

### Cursor Agent Configuration

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `GOSLING_PROVIDER` | Set to `cursor-agent` to use this provider | None |
| `CURSOR_AGENT_COMMAND` | Path to the Cursor Agent command | `cursor-agent` |

### Gemini CLI Configuration

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `GOSLING_PROVIDER` | Set to `gemini-cli` to use this provider | None |
| `GEMINI_CLI_COMMAND` | Path to the Gemini CLI command | `gemini` |

### Antigravity Configuration

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `GOSLING_PROVIDER` | Set to `antigravity` to use this provider | None |
| `ANTIGRAVITY_COMMAND` | Path to the Antigravity CLI command | `agy` |

Models come from `agy models` at runtime, so the list follows your account rather than a
list baked into gosling.

| gosling mode | Antigravity flag | Behavior |
|-------------|------------------|----------|
| `auto` | `--dangerously-skip-permissions` | Antigravity auto-approves its own tool calls |
| `smart-approve`, `approve`, `chat` | (unsupported) | Rejected with an explicit error — Antigravity cannot route approvals headless |

## How It Works

### System Prompt Filtering

The CLI providers automatically filter out gosling's extension information from system prompts since these CLI tools have their own tool ecosystems. This prevents conflicts and ensures clean interaction with the underlying CLI tools.

### Message Translation

- **Claude Code**: Converts gosling messages to text content blocks with role prefixes (Human:/Assistant:), similar to Gemini CLI
- **Cursor Agent**: Converts gosling messages to Cursor's JSON message format, handling tool calls and responses appropriately
- **Gemini CLI**: Converts messages to simple text prompts with role prefixes (Human:/Assistant:)
- **Antigravity**: Sends one `{"event":"user",...}` NDJSON line per turn to a persistent `agy` process; the system prompt is folded into the first turn because the CLI has no system-prompt flag

### Response Processing

- **Claude Code**: Parses streaming JSON responses to extract text content and usage information
- **Cursor Agent**: Parses JSON responses to extract text content and usage information
- **Gemini CLI**: Processes plain text responses from the CLI tool
- **Antigravity**: Streams `agent_response` text deltas from its `step_update` events and reads usage from the closing `result` event

## Error Handling

CLI providers depend on external tools, so ensure:

- CLI tools are properly installed and in your PATH
- Authentication is maintained and valid
- Subscription limits are not exceeded


---

CLI providers offer a way to use existing AI tool subscriptions through gosling's interface, adding session management and workflow integration capabilities. They're particularly valuable for users with existing CLI subscriptions who want unified session management.
