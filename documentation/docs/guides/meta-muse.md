---
title: Meta Muse
description: Use Meta Muse with an API key or your signed-in Muse Code CLI
---

# Meta Muse

Gosling offers two Meta providers in Desktop's provider settings and in
`gosling configure`:

| Provider | ID | Authentication | Tools and permissions |
| --- | --- | --- | --- |
| Meta Muse (API) | `meta` | `MODEL_API_KEY` | Gosling tools, extensions and approval modes |
| Meta Muse Code (Subscription/CLI) | `muse-code` | Muse Code's saved login | Muse tools; Gosling Auto mode required |

Both default to `muse-spark-1.3` and declare a 1,048,576-token model context.
`muse-wonder-1.0` is also listed; availability depends on your Meta account and plan.

## API key

Create an API key at [Meta's developer portal](https://dev.meta.ai), select
**Meta Muse (API)**, and enter it as `MODEL_API_KEY`. For a terminal session:

```sh
export MODEL_API_KEY='your-api-key'
export GOSLING_PROVIDER=meta
export GOSLING_MODEL=muse-spark-1.3
gosling session
```

The provider uses Meta's documented
[Chat Completions endpoint](https://dev.meta.ai/docs/protocols/chat-completions)
at `https://api.meta.ai/v1/chat/completions`. Streaming text, function tools,
conversation history and usage accounting use Gosling's existing API engine.
Chat Completions does not preserve encrypted reasoning across turns.

The starter list includes Muse Wonder 1.0, Muse Spark 1.3, 1.2 and 1.1, and the
1.3/1.2 Contributor variants. It is a static list; a model ID can also be entered
manually. Contributor models have different data-use terms; see Meta's
[pricing modes](https://dev.meta.ai/docs/pricing-rate-limits).

Thinking effort maps to Meta's supported levels. **Off** selects `minimal`
because these hosted models reject `none`. **Max** and **Ultra** select `max`
for Wonder 1.0 and standard Spark 1.3, and `xhigh` for the other starter models.
An explicit `reasoning_effort` request parameter takes precedence.

API usage is billed by Meta separately from a Muse Code subscription.

## Muse Code subscription

1. Install [Muse Code](https://dev.meta.ai/docs/muse-code) and check `muse --version`.
2. Run `muse login` in a terminal and complete browser sign-in for your subscribed account.
3. Confirm Muse itself can use your subscription in the working folder.
4. Select **Meta Muse Code (Subscription/CLI)** in Gosling and choose **Auto** mode.

For terminal use:

```sh
export GOSLING_PROVIDER=muse-code
export GOSLING_MODEL=muse-spark-1.3
export GOSLING_MODE=auto
gosling session
```

Set `MUSE_CODE_COMMAND` to the Muse executable path if it is not found. The
wrapper launches `muse exec --provider meta --json` in the Gosling session's
working directory, passes the prompt through a private temporary file, and
waits for both a completed run event and a successful process exit. The final
answer appears when the run completes. Dropping the response stream terminates
the owned process; on Unix its process group is also terminated.

The wrapper removes `META_API_KEY` and `MODEL_API_KEY` from the child environment
to avoid an exported API key overriding browser sign-in. Meta's
[credential precedence](https://dev.meta.ai/docs/muse-code/auth) still applies
to Muse's own stored credentials: a stored CLI API key takes priority over
browser sign-in. Verify billing in Muse; Gosling does not read or rewrite its
credential store.

### Wrapper limits

- **Auto mode only.** Headless Muse cannot relay approval prompts into Gosling.
  The wrapper disables Muse approval prompts and does not disable its sandbox.
  Approve, Smart Approve and Chat mode changes return an explicit error.
- **Muse owns tools.** Gosling extensions are not forwarded. Configure MCP servers
  and other tools in Muse, or use `meta` for Gosling-managed tools. Host-enforced
  planning restrictions still apply to this external tool runtime.
- **Text input.** Image attachments return an explicit error; use `meta` for them.
- **History replay.** Each turn starts a new Muse process with Gosling's text and
  tool history. Native Muse session resume is not provided. The wrapper requests
  no Muse session log; Gosling keeps its own session history.
- **Usage.** The wrapper does not infer token counts from undocumented events.
  Muse manages subscription limits and reports run failures through the provider.
- **Compatibility.** Event schema version 1 was checked with Muse Code 1.4.4.
  Malformed events, unsupported schemas, failed or cancelled runs, missing or
  duplicate terminal results, empty answers and nonzero exits return errors.

See [Meta's headless documentation](https://dev.meta.ai/docs/muse-code/extending)
for the underlying CLI interface.
