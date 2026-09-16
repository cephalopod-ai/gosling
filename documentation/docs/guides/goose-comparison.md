---
title: Goose and gosling feature comparison
description: Dated source comparison of Goose v1.50.1 and the gosling v1.2.5 source, including shared features and implementation differences.
---

# Goose and gosling feature comparison

Last checked: **2026-09-16**.

- **Goose:** [v1.50.1][goose-release], published 2026-09-14, was the latest
  non-prerelease returned by GitHub's release API. Source was checked out at
  `881f96c00d618ab6fca9b2aaa3a0abf07673cc2c`; all upstream source links below pin
  that commit. Later `main` commits are outside this comparison.
- **gosling:** the `main` working tree at `274955f7b`, whose `Cargo.toml` and
  `ui/desktop/package.json` declare `1.2.5`. The `v1.2.5` tag (`4c3dbdf32`) predates
  this source by more than 100 commits, so this identifies source behavior, not a
  packaged release. See the [v1.2.5 notes](../release-notes/v1.2.5.md).

This is a source review of the listed features. It does not establish identical
behavior across providers, MCP servers, operating systems, or packaged builds.
The [v1.47 compatibility record](goose-v1-47-compatibility.md) preserves the older
three-feature import; use this page for the current comparison. The previous
snapshot compared Goose v1.49.0 with gosling v1.2.3 on 2026-09-08.

## Shared capabilities and differences

The gosling paths identify files in this source tree. Upstream links identify the
corresponding evidence in the pinned Goose release.

| Capability | Goose v1.50.1 | gosling v1.2.5 source |
|---|---|---|
| Agent and MCP | Chat/tool execution and configurable MCP extensions. [Extension configuration][goose-extension]. | Chat/tool execution and MCP extensions in `crates/gosling/src/agents/`. Catalog discovery intentionally retains the AAIF compatibility adapter; support varies by extension and transport. |
| Cloud and local-service providers | Configurable providers, including [Ollama][goose-ollama]. | Configurable providers, including Ollama in `crates/gosling/src/providers/ollama_def.rs`. Provider counts do not imply identical authentication, model catalogs, or feature support. |
| Integrated local inference | [llama.cpp/GGUF runtime with optional MLX][goose-local]. v1.50.0 removed the managed model registry; Hugging Face discovery and cache management sit behind the `hf-hub` feature. | No bundled inference-runtime crate. A separately managed service such as Ollama can run models locally. See [provider setup](../getting-started/providers.md#local-llms). |
| CLI workflows | [Command registration][goose-cli] includes `session`, `run`, `acp`, `serve`, `mcp`, `skills`, `plugin`, `term`, `review`, and `completion`, plus `recipe`, `schedule`, and `gateway`. `local-models` is behind the default `local-inference` feature; `roam` is behind the non-default `roaming` feature. | `crates/gosling-cli/src/cli.rs` registers the shared command names plus `project`/`projects`, `secret`, `shell-validate`, and the feature-gated `tui`. It does not register `recipe`, `schedule`, `gateway`, `local-models`, or `roam`. This is a command-surface comparison, not flag compatibility. |
| Conversation context | [Context-management crate][goose-context] provides compaction and structured summaries. | `crates/gosling/src/context_mgmt/` implements compaction, summarization, budget/selection logic, and memory retrieval. Goose is not missing context management. |
| Fast-model routing | Removed in v1.50.0 ([release notes][goose-release-v150]); no `fast_model` configuration remains in the pinned source. | `crates/gosling/src/model_config.rs` resolves a fast model from `GOSLING_FAST_MODEL`, then the provider default, then the main model, for session naming, compaction, and tool summaries. |
| Peer-to-peer agent access | [`goose-roaming`][goose-roaming] provides an iroh-based transport so a remote ACP client can drive an agent without port-forwarding. It is an optional build feature. | No equivalent crate or command. Remote clients use the ACP server started by `gosling serve`. |
| Durable memory | [Memory MCP extension][goose-memory] supports local/global memory categories and retrieval. | `crates/gosling/src/context_mgmt/memory.rs` provides `FileMemorySource` for retrieved facts. These are different storage and retrieval contracts, not interchangeable memory formats. |
| Git branch menu | [Desktop branch indicator][goose-git] displays the branch and can switch it. | `ui/desktop/src/components/bottom_menu/GitBranchIndicator.tsx` provides branch display and local switching; `ui/desktop/src/main/gitIpc.ts` checks the renderer's directory grant. |
| Pre-registered OAuth clients | [Streamable HTTP configuration][goose-extension] accepts `client_id`, `client_secret_key`, and `scopes`. | `crates/gosling/src/agents/extension.rs` supports those fields. The client secret is resolved from the extension environment or secret store. Shared field names do not establish identical OAuth flows. |
| Recent model choices | [Recent-model helper][goose-recent] retains up to five model/provider pairs. | `ui/desktop/src/utils/recentModels.ts` retains up to five successful model/provider choices for the Desktop picker. |
| Tool lifecycle hooks | [Hook contract][goose-hooks] includes `PreToolUseResult`, `tool_call_id`, and `on_failure` handling for pre-tool hooks. | `crates/gosling/src/hooks/mod.rs` has earlier hook events, including `PreToolUse` and `PostToolUseFailure`, but lacks those newer fields/events. Do not assume a current Goose hook configuration transfers unchanged. |
| Coexistence | [Goose paths][goose-paths] use the `goose` application name and shared `~/.agents` paths. | `crates/gosling/src/config/paths.rs` uses `gosling` for product-owned state while retaining shared `~/.agents` interoperability. Separate app identities enable coexistence; it is not a feature one app alone can lack. |

## Security behavior checked

| Control | Goose v1.50.1 | gosling v1.2.5 source |
|---|---|---|
| An enabled inspector returns an error | The [inspection manager][goose-inspection] logs the error and continues to other inspectors without adding a fallback verdict. | `crates/gosling/src/tool_inspection.rs` adds `RequireApproval` for every affected request, including in Autonomous mode. |
| A tool is stored in both allow and deny lists within a policy category | [Permission lookup][goose-permission] checks `never_allow` first. | `crates/gosling/src/config/permission.rs` also checks `never_allow` first. This protection is shared. |

The inspector row describes one error path. A missing inspector verdict does not
prove that every tool executes: other permissions, hooks, and visibility controls
still apply. Goose v1.50.0 also contains security fixes, including enforced subagent
platform guards and permission revocations preserved across managers, documented in
its [release notes][goose-release-v150]. Neither project is assigned an overall
security rating here.

The old README's generic "Path Sandbox Enforcement: Weak/Yes" and MCP `cache`
claims have been removed. gosling's current bundled MCP command registry,
`crates/gosling-mcp/src/mcp_server_runner.rs`, does not expose that cache command.
Current Desktop file access instead follows the artifact authorization boundary
described in [Workspaces](workspaces.md#product-outputs-and-exports).

## gosling Desktop outputs

gosling's Desktop workspaces bind working folders, output destinations, and
credential profiles to sessions. The session Outputs inventory retains file
provenance separately from preview tabs. Its display is controlled by **Settings
→ App → Output files** and, in `v1.2.2`, the remembered **Hide repository files**
switch. The switch is off by default and can hide source/project files as well as
documents located inside a repository. It changes the list and count, not the
files or stored inventory.

The reviewed Goose Desktop/session source did not contain an equivalent to this
session Outputs inventory and repository-file switch. This observation does not
mean Goose cannot create files or display file references. The gosling
implementation is in `ui/desktop/src/components/artifacts/ArtifactPane.tsx`,
`ui/desktop/src/utils/artifactRepository.ts`, and
`ui/desktop/src/main/fileIpc.ts`. See the [Outputs guide](workspaces.md#filter-repository-files)
for exact filter behavior and unavailable-file handling.

The `v1.2.3` source update added filesystem timestamps to Outputs and Library rows, workspace
readiness indicators, output contributor/model attribution with saved revisions, and a separate
**Copy contents** action for complete UTF-8 text files. These additions are documented in the
[Outputs guide](workspaces.md#output-agents-and-revision-history); they do not change the pinned
Goose evidence or establish runtime parity with Goose.

## Refreshing this comparison

1. Check [Goose's latest stable release](https://github.com/aaif-goose/goose/releases/latest)
   and resolve its tag to a commit. Record the release publication date separately
   from the source commit date; exclude canary/prerelease builds unless explicitly compared.
2. Read both projects' command registrations, configuration contracts, and feature
   implementations. Release notes identify candidates to check, not proof that
   gosling has or lacks them. Recheck every negative claim.
3. Record gosling's manifest version and whether it is a working tree, tag, or
   published artifact. Update this page and the README matrix together. Preserve
   versioned release notes and import logs as historical evidence.
4. Update the documentation index/inventory and regenerate the docs map. Run the
   docs tests and Docusaurus content build; record source-review and runtime-test
   limits separately in the session log.

[goose-release]: https://github.com/aaif-goose/goose/releases/tag/v1.50.1
[goose-release-v150]: https://github.com/aaif-goose/goose/releases/tag/v1.50.0
[goose-roaming]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose-roaming/README.md
[goose-cli]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose-cli/src/cli.rs
[goose-extension]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose/src/agents/extension.rs
[goose-ollama]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose/src/providers/ollama_def.rs
[goose-local]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose-local-inference/README.md
[goose-context]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose-context-management/README.md
[goose-memory]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose-mcp/src/memory/mod.rs
[goose-git]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/ui/desktop/src/components/GitBranchIndicator.tsx
[goose-recent]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/ui/desktop/src/utils/recentModels.ts
[goose-hooks]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose/src/hooks/mod.rs
[goose-paths]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose/src/config/paths.rs
[goose-inspection]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose/src/tool_inspection.rs
[goose-permission]: https://github.com/aaif-goose/goose/blob/881f96c00d618ab6fca9b2aaa3a0abf07673cc2c/crates/goose/src/config/permission.rs
