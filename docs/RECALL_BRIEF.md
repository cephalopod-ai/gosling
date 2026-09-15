# Recall Brief (ACP)

Recall Brief is an explicit report action for a Muninn MCP extension already
enrolled in a gosling session. It reports what returned memory records say and
keeps remembered beliefs separate from facts about the world. The current
source exposes a typed ACP request; the CLI and Desktop do not yet provide a
dedicated button or command for it.

Send `_gosling/unstable/session/recall/brief` through an admitted ACP
connection for the session:

```json
{
  "sessionId": "<current-session-id>",
  "extensionName": "muninn",
  "query": "What did I say about Santa Claus?",
  "facets": ["belief at age seven", "belief at age eight"],
  "selectors": {"storeId": "personal"}
}
```

`extensionName` selects one enrolled Stdio or HTTP MCP extension; it is not a
URL or a source of trust. `facets`, `cursor`, and `selectors` are optional.
Only selectors you supply are sent to Muninn. Supported selectors include
artifact kind, conversation/repository fields, a relative repository path
prefix, retrieval context, recorded-time `since`/`until`, and store ID. The
tool call uses gosling's normal permission and tool-inspection controls.
An open host-enforced plan denies this action before it contacts Muninn.

If retrieval succeeds, the response contains `status`, `providerName`,
`modelName`, typed `findings`, `sourceEvidence`, `unresolved`, an optional
`receipt`, a rendered Markdown report, and a `notice`. Source evidence has
short returned excerpts and pinned `muninn://memory/` citation URIs with exact
revisions. `status` is one of `synthesized`, `evidence_only`, `empty`,
`partial`, or `unavailable`. A partial result means a returned hit could not
be bound to an authorized exact revision or Muninn reported failed retrieval
lanes. A receipt can disclose empty or failed facets, unsearched lanes, a continuation, or a
generation change; missing receipt fields mean unknown, not zero.

Selected memory excerpts may contain private chat history. This explicit
action sends the bounded selected excerpts to the session's compatible
provider/model for a candidate report, and the response names them. A model
failure or an invalid citation leaves the exact returned excerpts in an
evidence-only report with `Inference: None`. No Recall Brief writes or promotes
memory. A pinned record proves only that the returned revision exists; its
autobiographical content and the model's paraphrase are not independently
verified. Ordinary streamed chat replies are outside this report gate.
