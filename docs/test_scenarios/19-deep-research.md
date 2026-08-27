# 19 — Deep Research

Deep Research must distinguish a prompt-only investigation from one seeded
with Initial Inputs, and multi-model orchestration must fail once instead of
spending turns repairing an invalid delegate payload. Use deterministic test
providers and protocol/tool-call capture; never infer these results from the
final prose alone.

---

### DR-01 — Prompt-only research does not discover workspace files
- Goal: start Deep Research with no Initial Inputs and receive a report without the agent treating workspace files as an implicit research corpus.
- Category: empty state / happy path / files
- Preconditions: Desktop running from the target build with a disposable `GOSLING_PATH_ROOT`; configured deterministic or sandbox provider; required research extensions enabled; disposable workspace containing uniquely named bait files and access/tool-call telemetry; empty session Outputs and Research Library folders.
- Steps:
  1. Open New Research and leave Initial Inputs empty: attach no files, images, reports, links, or notes.
  2. Select Solo mode and submit a short external research prompt that does not mention the workspace or any local file, such as `Compare two documented approaches to bounded retry handling and cite the sources.`
  3. Observe the activity/tool-call stream until the turn completes or reaches the 120-second provider deadline; record every filesystem, search, and extension call.
  4. Inspect the final response, session Outputs, and Research Library entries.
- Expected: the session starts without requiring Initial Inputs; no workspace list/search/read call occurs and no bait filename or content enters model context; the agent treats file-oriented Phase 0 and internal-program analysis as not applicable while external evidence collection may proceed; it never claims local files were reviewed; a completed report appears in both session Outputs and the Research Library, or a bounded actionable error names the prerequisite that prevented completion.
- Observe: hidden automatic workspace inspection before the first visible activity; attempts to find an “initial-input corpus”; file-access permission prompts; false claims about reviewed files; duplicate or mismatched report artifacts between Outputs and Library.
- Variations: repeat with an empty workspace; repeat with many tempting source files; after the first turn, explicitly attach one file and verify only the new turn may inspect that supplied input.

### DR-02 — Invalid delegate source fails once without retry churn
- Goal: a malformed Deep Research delegate request fails at the payload boundary once, does not start the wrong agent, and does not consume repeated turns trying alternate `source` values.
- Category: invalid input / recovery / concurrency
- Preconditions: Desktop running with a disposable `GOSLING_PATH_ROOT`; Dual research mode backed by deterministic lead and delegate test models; required research extensions enabled; Summon/delegate protocol capture that records exact arguments, launch count, spawned delegate sessions, elapsed time, and token/turn usage; no Initial Inputs.
- Steps:
  1. Start a prompt-only Dual research session and confirm the normal first-pass delegate request contains `instructions`, the selected `provider` and `model`, and `async: true`, with the `source` key omitted entirely.
  2. In a fresh cloned fixture, configure the scripted lead response to emit exactly one otherwise valid delegate call with `source: ""`.
  3. Observe the validation result and all later delegate calls for that roster seat for 30 seconds or until the lead reaches a terminal response.
  4. Repeat from another fresh clone with one non-empty unknown named source, such as `source: "missing-researcher"`.
- Expected: the normal request launches one delegate for the configured seat; the empty source is rejected before a delegate starts; the unknown named source returns one clear not-found failure; after either rejection the lead records that exact seat as failed, continues or terminates explicitly in degraded mode, and does not retry with empty, null, omitted, renamed, or alternate source values; no orphan delegate session is created and the turn reaches a terminal state within the normal deadline.
- Observe: repeated `delegating` activity, changing explanations that claim the same payload is now different, a copy-through `source` key after saying it was omitted, growing token/turn use after the first deterministic validation error, wrong-provider substitution, silent success, or a spinner with no terminal state.
- Variations: use `source: null` if the protocol fixture permits schema-invalid JSON; run Trio mode with one valid seat and one invalid seat and verify the valid result survives while only the invalid seat is degraded.
