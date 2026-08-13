# Context compaction fail-safe plan

Status: implemented; validation evidence is recorded in the associated session log.

## Problem statement

Gosling can wait too long to compact a ChatGPT Codex session, then construct a
single summarization request that is larger than either the model context window
or the provider's `instructions` field. Two observed failures are in scope:

- an in-band Responses event with code `context_length_exceeded` was flattened
  into a generic request failure, preventing compaction recovery;
- GPT-5.5 rejected a 1,703,084-character `instructions` value because the field
  limit is 1,048,576 characters.

Desktop can compound the problem by displaying a public canonical model limit
instead of the active provider route's effective limit.

## Patch sequence

### C0 — Typed provider failures

- Classify in-band `context_length_exceeded` events as
  `ProviderError::ContextLengthExceeded`.
- Treat provider rejections for an oversized prompt or `instructions` field as
  context-size failures.
- Extract the provider message and code for user-facing errors instead of Rust
  debug-rendering the JSON object.

### C1 — Provider-route context capability

- Query the authenticated ChatGPT Codex `/models` route and cache its model
  capabilities for the provider instance.
- Calculate the usable limit from `context_window` and
  `effective_context_window_percent`.
- Use a conservative built-in route fallback when discovery is unavailable.
- Keep user configuration overrides explicit; do not replace route limits with
  public API limits for the separate ChatGPT subscription route.

### C2 — Bounded hierarchical compaction

- Keep compaction instructions fixed-size and send transcript material as
  bounded user input.
- Preflight chunks against both a character ceiling and a conservative fraction
  of the provider's effective context limit.
- Split oversized histories chronologically, summarize each chunk, and reduce
  the summaries until one final continuation summary remains.
- If the provider still reports a context overflow, split that unit and retry
  until the minimum safe unit is reached.
- When history must be pruned, remove matched tool request/response content as
  an atomic pair. Never leave a request whose response was deliberately removed.
- Split a single pathological message into explicitly numbered bounded segments
  so one tool result cannot make recovery impossible.

### C3 — User-visible context and recovery

- Prefer provider-route model metadata over canonical public-model metadata in
  the Desktop context indicator.
- Label the displayed token count as the most recent model request, not the
  freshly constructed compaction input.
- Return a concise recovery message if even the minimum bounded compaction unit
  is rejected. Preserve the original session and recommend a new session rather
  than implying authentication or database damage.

### C4 — Verification and documentation

- Add regression tests for both Responses stream event shapes and clean error
  text.
- Test live-model capability parsing, effective limits, and fallback behavior.
- Test large histories, single oversized messages, multi-pass reduction,
  ordered output, matched tool-pair pruning, and terminal failure.
- Test provider-first Desktop limit selection and recovery copy.
- Update context-management documentation, scenario coverage, and the session
  log.

## Native Responses compaction boundary

The public Responses API and the ChatGPT Codex route expose
`/responses/compact`, but its output is opaque provider state that must be
passed forward unchanged. Gosling sessions are provider-neutral and support
model/provider switching; their message schema currently has no contract for
persisting, replaying, exporting, or migrating opaque compacted response items.

This repair therefore does not serialize native compacted output into an
ordinary Gosling message. Native compaction should be added only with a
versioned provider-state persistence contract and compatibility behavior for
provider switching. The bounded hierarchical path is the fail-safe for every
current provider.

## Acceptance criteria

- The supplied 499-message, tool-heavy shape cannot produce an `instructions`
  value derived from the full transcript.
- Every compaction provider request stays below the configured character and
  token budgets before it is sent.
- A `context_length_exceeded` stream event reaches the recovery branch as a
  typed context error.
- A ChatGPT Codex model limit comes from its route catalog when available and
  from a conservative route-specific fallback otherwise.
- Desktop does not show the public GPT-5.4 limit for the ChatGPT Codex route.
- Failed compaction leaves the original conversation unchanged.
- Successful manual or automatic compaction produces one valid continuation
  context and aggregates usage across all summarization requests.

## Observed recovery evidence

Before this patch, the same preserved session failed on the ChatGPT Codex route
with both `context_length_exceeded` and a 1,703,084-character `instructions`
rejection. Switching the session to xAI Grok 4.6 allowed `/compact` to complete;
switching back to GPT then worked. This demonstrates that the persisted session
was healthy and that failed compaction did not destroy its cross-provider recovery
path. PN-04 preserves this sequence as a regression scenario.
