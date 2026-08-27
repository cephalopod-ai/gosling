# gosling Extension Allowlist

The allowlist feature provides a security mechanism for controlling which MCP commands can be used by gosling. 
By default, gosling will let you run any MCP via any command, which isn't always desired.

## How It Works

1. When enabled, gosling will only allow execution of commands that match entries in the allowlist
2. Commands not in the allowlist will be rejected with an error message
3. The allowlist is fetched from a URL specified by the `GOSLING_ALLOWLIST` environment variable and cached while running.

## Setup

Set the `GOSLING_ALLOWLIST` environment variable to the URL of your allowlist YAML file:

```bash
export GOSLING_ALLOWLIST=https://example.com/gosling-allowlist.yaml
```

If this environment variable is not set, no allowlist restrictions will be applied (all commands will be allowed).

## Bypassing the Allowlist

In certain development or testing scenarios, you may need to bypass the allowlist restrictions. You can do this by setting the `GOSLING_ALLOWLIST_BYPASS` environment variable to `true`:

```bash
export GOSLING_ALLOWLIST_BYPASS=true
```


When this environment variable is set to `true` (case-insensitive), the Rust
execution and configuration sinks bypass the allowlist. Use it only in an
isolated development or test environment.

## Allowlist File Format

The allowlist file should be a YAML file with the following structure:

```yaml
extensions:
  - id: extension-id-1
    command: command-name-1
  - id: extension-id-2
    command: command-name-2
```

Example:

```yaml
extensions:
  - id: slack
    command: uvx mcp_slack
  - id: github
    command: uvx mcp_github
  - id: jira
    command: uvx mcp_jira
```

The command must be the full command and argument vector used to launch the MCP
(environment variables are not part of the comparison). Additional arguments
are rejected.
