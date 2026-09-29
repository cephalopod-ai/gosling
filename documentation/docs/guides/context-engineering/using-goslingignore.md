---
title: Protecting Sensitive Files
sidebar_label: Protecting Sensitive Files
sidebar_position: 9
---


:::warning `.goslingignore` is not enforced
gosling does not read `.goslingignore` files, does not fall back to `.gitignore` to block tool access,
and does not protect `.env` or `secrets.*` files by default. A `.goslingignore` file has no effect on the
Developer extension's `shell`, `write`, `edit`, or `tree` tools: gosling can read, change, or delete the
files it lists, and their contents can be sent to your model provider. Earlier versions of this page
described an ignore-file access control that is no longer part of gosling.
:::

Use the controls below instead. gosling checks them before a tool call runs.

## Controls that protect files

| Control | What it does | Where to set it |
|---------|--------------|-----------------|
| [Permission mode](/docs/guides/managing-tools/gosling-permissions) | **Manual Approval** asks before every tool call, so nothing reads or changes a file without your approval. **Smart Approval** asks for calls it cannot classify as read-only. | `/mode approve` in the CLI, the mode selector in Desktop, or `GOSLING_MODE` |
| [Tool permissions](/docs/guides/managing-tools/tool-permissions) | **Never Allow** blocks a tool (for example `shell`, `write`, or `edit`) in every mode. **Ask Before** requires approval for it in Manual and Smart Approval. | Desktop mode settings, or `gosling configure` → gosling settings → Tool Permission |
| **Restrict tools to working directories** | A tool call that touches a path outside the session's working directories needs your approval. | Desktop working directories menu, per session. Imported sessions start with it on |
| [Workspace](/docs/guides/workspaces) folder policy | Folders marked **Read only** cannot be modified by gosling's tools. | Desktop workspace settings |

The working-directory restriction is a guardrail, not a sandbox. It checks the paths a tool call names,
including the folders a shell command changes into, but it cannot see files that a command reaches
through variables, scripts, or the programs it runs. When a file must never reach the model, keep it
outside the folders gosling works in, for example in your operating system's keychain or a secrets
manager, and use Manual Approval for sessions that handle sensitive material.

## What `.gitignore` still affects

- The Developer extension's `tree` tool leaves git-ignored and hidden entries out of its listing.
- Files referenced with `@` inside context files such as `AGENTS.md` or `.goslinghints` are not
  imported when git ignores them.
- Context files in directories that git ignores are not loaded as nested hints when a tool touches
  those directories.

Neither behavior stops a tool call from reading or changing an ignored file.
