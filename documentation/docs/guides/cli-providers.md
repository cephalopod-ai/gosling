---
sidebar_position: 8
title: CLI Providers
sidebar_label: CLI Providers
description: Use Claude Code, Codex, Cursor Agent, Gemini CLI, or Antigravity subscriptions in gosling
---

# CLI Providers

:::warning Deprecated — Use ACP Providers
The Claude Code (`claude-code`), Codex (`codex`), and Gemini CLI (`gemini-cli`) providers are deprecated. Use the [ACP providers](/docs/guides/acp-providers) (`claude-acp`, `codex-acp`) instead, which support gosling extensions via MCP and use the standardized Agent Client Protocol. For Gemini, use `Google Gemini (API Key)` with `GOOGLE_API_KEY`. CLI providers are kept for backward compatibility only.
:::

gosling can make use of pass-through providers that integrate with existing CLI tools from Anthropic, OpenAI, Cursor, and Google. These providers allow you to use your existing Claude Code, Codex, Cursor Agent, and Google Gemini CLI subscriptions through gosling's interface, adding session management, persistence, and workflow integration capabilities to these tools.

:::warning Limitations
These providers don’t fully support all gosling features, may have platform or capability limitations, and can sometimes require advanced debugging if issues arise. They’re included here purely as a convenience.
:::

## Why Use CLI Providers?

CLI providers are useful if you:

- already have a Claude Code, Codex, Cursor, or Google Gemini CLI subscription and want to use it through gosling instead of paying per token
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
- Automatic filtering of gosling extensions from system prompts (since Claude Code has its own tool ecosystem)
- Streaming JSON (NDJSON) protocol for persistent, multi-turn sessions

**Requirements:**
- Claude CLI tool installed and configured
- Active Claude Code subscription
- CLI tool authenticated with your Anthropic account

### OpenAI Codex

The Codex provider integrates with OpenAI's [Codex CLI tool](https://developers.openai.com/codex/cli), allowing you to use OpenAI models through your existing ChatGPT Plus/Pro subscription or API credits.

**Features:**
- Uses OpenAI's GPT-5 series models (gpt-5.6-sol, gpt-5.6-terra, gpt-5.6-luna, gpt-5.5, gpt-5.4, gpt-5.4-mini, gpt-5.3-codex-spark)
- Configurable reasoning effort levels (`none`, `low`, `medium`, `high`, `xhigh`, `max`, `ultra`; the ceiling varies per model)
- Optional skills support for enhanced capabilities
- JSON output parsing for structured responses
- Automatic filtering of gosling extensions from system prompts

**Requirements:**
- Codex CLI tool installed (`npm i -g @openai/codex` or `brew install --cask codex`)
- Active ChatGPT Plus/Pro subscription or OpenAI API credits
- CLI tool authenticated with your OpenAI account
- By default, Codex requires running from a git repository. Set `CODEX_SKIP_GIT_CHECK=true` to bypass this requirement

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
### OpenAI Codex

1. **Install Codex CLI Tool**

   Install the Codex CLI using npm or Homebrew:
   ```bash
   npm i -g @openai/codex
   # or
   brew install --cask codex
   ```

2. **Authenticate with OpenAI**

   Run `codex` and follow the authentication prompts. You can use your ChatGPT account or API key.

3. **Configure gosling**

   Set the provider environment variable:
   ```bash
   export GOSLING_PROVIDER=codex
   ```

   Or configure through the gosling CLI using `gosling configure`:

   ```bash
   ┌   gosling-configure
   │
   ◇  What would you like to configure?
   │  Configure Providers
   │
   ◇  Which model provider should we use?
   │  OpenAI Codex CLI
   │
   ◇  Model fetch complete
   │
   ◇  Enter a model from that provider:
   │  gpt-5.6-sol
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

The following models are recognized and passed to the Claude CLI via the `--model` flag. If `GOSLING_MODEL` is set to a value not in this list, no model flag is passed and Claude Code uses its default:

- `default` (opus)
- `sonnet`
- `haiku`

**Permission Modes (`GOSLING_MODE`):**

| Mode | Claude Code Flag | Behavior |
|------|------------------|----------|
| `auto` | `--dangerously-skip-permissions` | Bypasses all permission prompts |
| `smart-approve` | `--permission-prompt-tool stdio` | Routes permission checks through the control protocol (prompts as needed) |
| `approve` | `--permission-prompt-tool stdio` | Routes permission checks through the control protocol (prompts as needed) |
| `chat` | (none) | Default Claude Code behavior |

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

### OpenAI Codex Configuration

| Environment Variable | Description | Default |
|---------------------|-------------|---------|
| `GOSLING_PROVIDER` | Set to `codex` to use this provider | None |
| `GOSLING_MODEL` | Model to use (only known models are passed to CLI) | `gpt-5.6-sol` |
| `CODEX_COMMAND` | Path to the Codex CLI command | `codex` |
| `CODEX_REASONING_EFFORT` | Reasoning effort level: `none`, `low`, `medium`, `high`, `xhigh`, `max`, or `ultra`. gosling lowers a level the selected model does not support to its ceiling | `high` |
| `CODEX_ENABLE_SKILLS` | Enable Codex skills: `true` or `false` | `true` |
| `CODEX_SKIP_GIT_CHECK` | Skip git repository requirement: `true` or `false` | `false` |

**Known Models:**

The following models are recognized and passed to the Codex CLI via the `-m` flag. If `GOSLING_MODEL` is set to a value not in this list, no model flag is passed and Codex uses its default:

- `gpt-5.6-sol` (258K effective context)
- `gpt-5.6-terra` (258K effective context)
- `gpt-5.6-luna` (258K effective context)
- `gpt-5.5` (258K effective context)
- `gpt-5.4` (258K effective context)
- `gpt-5.4-mini` (258K effective context)
- `gpt-5.3-codex-spark` (121K effective context)

:::note Model availability follows your Codex account
This list mirrors the catalog the Codex CLI fetches for a ChatGPT account. Models retired from that catalog are rejected by the backend with an HTTP 400 even though the CLI still accepts the flag, so gosling only offers what the catalog currently serves. To use a model outside this list, run `codex -m <model_name>` directly or configure it in Codex's `config.toml`. See the [Codex CLI documentation](https://developers.openai.com/codex/cli) for details.
:::

**Permission Modes (`GOSLING_MODE`):**

| Mode | Codex Flag | Behavior |
|------|------------|----------|
| `auto` | `--yolo` | Bypasses all approvals and sandbox restrictions |
| `smart-approve` | `--full-auto` | Workspace-write sandbox, approvals only on failure |
| `approve` | (none) | Interactive approvals (Codex default behavior) |
| `chat` | `--sandbox read-only` | Read-only sandbox mode |

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

- **Claude Code**: Converts gosling messages to text content blocks with role prefixes (Human:/Assistant:), similar to Codex and Gemini CLI
- **Codex**: Converts messages to simple text prompts with role prefixes (Human:/Assistant:), similar to Gemini CLI
- **Cursor Agent**: Converts gosling messages to Cursor's JSON message format, handling tool calls and responses appropriately
- **Gemini CLI**: Converts messages to simple text prompts with role prefixes (Human:/Assistant:)
- **Antigravity**: Sends one `{"event":"user",...}` NDJSON line per turn to a persistent `agy` process; the system prompt is folded into the first turn because the CLI has no system-prompt flag

### Response Processing

- **Claude Code**: Parses streaming JSON responses to extract text content and usage information
- **Codex**: Parses newline-delimited JSON events to extract text content and usage information
- **Cursor Agent**: Parses JSON responses to extract text content and usage information
- **Gemini CLI**: Processes plain text responses from the CLI tool
- **Antigravity**: Streams `agent_response` text deltas from its `step_update` events and reads usage from the closing `result` event

## Error Handling

CLI providers depend on external tools, so ensure:

- CLI tools are properly installed and in your PATH
- Authentication is maintained and valid
- Subscription limits are not exceeded
- For Codex: you're in a git repository, or set `CODEX_SKIP_GIT_CHECK=true`


---

CLI providers offer a way to use existing AI tool subscriptions through gosling's interface, adding session management and workflow integration capabilities. They're particularly valuable for users with existing CLI subscriptions who want unified session management.
