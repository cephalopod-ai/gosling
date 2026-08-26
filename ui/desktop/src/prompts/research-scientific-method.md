---
name: research-scientific-method
description: "Apply disciplined, reproducible scientific reasoning throughout an active research investigation — ML experiments, physical modeling/simulation, data analysis, or any other hypothesis-driven inquiry. Frames a falsifiable question and null-model plan before anything runs, searches prior findings so a renamed idea is not mistaken for a new one, designs controls and a pre-registered comparison plan, captures execution provenance, separates measured observation from interpretation, caps confidence at what evidence actually supports, preserves negative results and unexplained residuals as first-class record, and corrects conclusions by superseding rather than silently rewriting prior entries. Detects a repository's existing research-discipline tooling and defers to it before proposing anything new. Invoke when framing a hypothesis, designing an experiment, evaluating whether a result is real, logging a finding, closing a research branch, or auditing whether a research program's own reasoning has been consistent."
version: 0.1
---

# Scientific Method and Research Discipline

## Mission

Keep an active research investigation honest across sessions, agents, and
months of work: every claim traces to evidence, every discriminative claim has
been compared against a null, every conclusion states what would have falsified
it, and every correction is a new record rather than a quietly edited one. This
is a process discipline applied *while research happens*, not a report produced
after it is done.

It is domain-general. The same failure modes recur whether the object under
study is a transformer's internals, a candidate physical theory, a pipeline's
throughput, or a dataset's statistics: an unfalsifiable hypothesis, a
comparison with no baseline, a measurement quietly merged with its
interpretation, a dead end that gets tried again next quarter because nobody
wrote down that it was already closed.

## When to invoke

- Framing a new research question or hypothesis before designing an experiment.
- Designing an experiment, simulation, or analysis that will be used to decide
  whether something is true.
- Evaluating whether an observed effect is real (is there a null model? a
  baseline? an architecture- or condition-matched control?).
- Logging a finding, especially a positive or "surprising" one.
- Deciding whether an idea has already been tried in this program.
- Closing a research branch, retracting a claim, or correcting a prior result.
- Auditing an existing research program's own findings, ledger, or claim graph
  for consistency (evidence vs. confidence, orphaned discriminative claims,
  silently overwritten history).
- Bootstrapping lightweight research bookkeeping in a repository that has none.

Not for: writing the investigation's code (`repair-*`, `plan-*` skills), running
a general code review (`code-review`), or producing publication-ready figures
(`graphics-figures-scientific` — invoke it *after* this skill has decided what
the figure should honestly claim).

## Relationship to sibling skills (binding)

Three catalog skills touch "research." They own different phases and never
substitute for one another:

- **`research-scientific-method` (this skill)** owns the *ongoing internal
  discipline* of a live investigation: framing, null models, controls,
  provenance, evidence logging, negative-result preservation, correction
  hygiene. It runs continuously, from the first hypothesis through the last
  correction.
- **`research-priorart-landscape`** owns whether the mechanism *already exists
  outside this program* — external literature and patent claims. Its "search
  the literature" is about the outside world. This skill's "search prior work"
  (Phase 2 below) is about *this program's own* closed branches and findings —
  a much cheaper, mandatory-first check that the priorart skill does not
  perform.
- **`research-paper-review`** owns the *after-the-fact adversarial audit* of a
  finished manuscript: does the written paper's argument, math, and statistics
  hold up. It presumes the investigation is over and a draft exists. This
  skill is what should have been true *while the investigation was running* so
  that a later paper review has something sound to review.
- **`acquisition-test-plan`** (600_acquisition) is the defense/DoD-specific
  analog for formal test-and-evaluation campaigns bound to an acquisition
  framework (DoDI 5000.89, event-authority gates, sponsor decision packages).
  Route there when the request is explicitly a defense T&E campaign; route
  here for general-purpose scientific investigation with no acquisition
  framework attached.

A single research program typically uses this skill throughout, then
`research-priorart-landscape` before committing to a direction, then
`research-paper-review` before submission. None of the three revises another's
verdict: prior-art status never overturns an experimental conclusion, and a
paper-review finding never retroactively edits the evidence log — it can only
prompt a new, superseding entry (Phase 6).

## Detect before you build (binding)

A repository doing serious research may already have its own disciplined
system — a claim graph with mechanically enforced invariants, a status/promotion
model, a provenance ledger, a `do_not_repeat` registry, a CLI that wraps
experiment runs. **Find it before proposing anything.** Look for:

- A `research/`, `experiments/`, or similarly named directory with a ledger,
  status model, or provenance subdirectory.
- `AGENTS.md`, `CLAUDE.md`, `CHARTER.md`, or a `*_STATUS_MODEL.md` /
  `*_PROVENANCE_RULES.md` describing enforced invariants, a status taxonomy,
  or a re-entry protocol.
- A CLI or module (`*.provenance.cli`, `*evaluator*`, `*ledger*`) that
  validates, seeds, or queries a claim graph.
- Existing YAML/JSON ledgers of hypotheses, findings, decisions, or closed
  branches.

If found: **use it.** Follow its status taxonomy, its required record fields,
its CLI, and its own stricter rules in preference to anything below — this
skill's job in that repository is to make sure the existing discipline is
actually followed (Phase 2 discovery, Phase 5 evidence hygiene, Phase 6
correction hygiene still apply as a checklist), not to introduce a competing
scheme. Report what you found and are deferring to.

If nothing comparable exists: bootstrap the minimal ledger described near the
end of this file, scaled to the size of the investigation — a single-file
findings log for a small analysis, the full five-file ledger for a
multi-month program. Say explicitly that you are bootstrapping it and why.

## The epistemic invariants (binding)

These hold regardless of domain or whether a bespoke system exists. Treat
"binding" as: violating one is a defect to fix, not a style preference.

| | Invariant | Why it is enforced, not merely advised |
|---|---|---|
| **R1** | **Falsifiability first.** Before running anything, write down what result would make you abandon the hypothesis. If you cannot state it, it is not a test yet — it is data collection in search of a story. | Post-hoc "this is roughly what I expected" is unfalsifiable by construction; the criterion has to exist before the outcome is known. |
| **R2** | **Null or baseline mandatory.** A comparative or discriminative claim with no null model, control, or baseline is **unmeasured**, not weak evidence. Report it as `unmeasured`, not as a supported finding with low confidence. | An observable that has never been compared to chance, to an untreated condition, or to a matched null carries no information about whether it distinguishes anything. Confident conclusions have been built and believed for months on statistics that, once nulled, turned out to be *less* extreme than an unstructured baseline. The failure is invisible from inside the result; it is only visible next to a null. |
| **R3** | **Observation/interpretation separation.** Record the measured quantity and the current reading of it as two distinct fields, never one sentence. | Once a measurement and its interpretation are fused, the interpretation cannot be revised without appearing to revise the measurement — so people revise both, or neither, and the record silently becomes narrative instead of data. |
| **R4** | **Provenance and a confidence ceiling.** Every claim states where it came from (experiment, derivation, external source, prior session, AI-generated draft) and the source class caps the confidence that claim may hold. Confidence is never allowed to exceed what the evidence strength and provenance class actually support. | A claim without a source cannot be re-checked. A confidence level detached from evidence strength is an opinion wearing a number. |
| **R5** | **Prior-work discovery before new work.** Search this program's own findings and closed-branch record before proposing a new mechanism, metric, framing, or experimental design. A new name for an old idea is not a new hypothesis. | The single most expensive, most common failure in ongoing research programs is re-deriving something already tried and closed under a different vocabulary. |
| **R6** | **Append-only correction.** When a conclusion changes, add a new record that supersedes the old one and states why; never edit or delete the old one in place. | A silent edit destroys the ability to tell what was believed and when, and hides exactly the trajectory (confidently wrong, then corrected) that later readers most need to see to avoid repeating it. |
| **R7** | **Negative results and residuals are first-class.** Preserve failed hypotheses, closed branches (with the condition that would reopen them), and observations that don't fit the current model. Do not delete them, merge them into a vague "didn't pan out" bucket, or quietly drop them from the write-up. | A closed branch without a stated reopening condition gets silently retried. An unexplained residual that gets discarded because it's inconvenient is often the most informative thing in the dataset. |
| **R8** | **Independent-provenance convergence.** Two results produced by the same script, the same run, or the same underlying data source are one observation reported twice, not corroboration. Convergence requires support from genuinely independent provenance. | Apparent agreement between non-independent measurements is the most common way overconfidence enters a research program; it looks exactly like replication from the inside. |
| **R9** | **AI-generated content gate.** Content that this agent (or any prior model session) produced or substantially shaped — an analogy, a derivation, a proposed mechanism, a summary of results — is labeled with that provenance and capped at low confidence until independently re-derived, re-run, or checked by a human or an independent method. | Model output is fluent and can be wrong in ways that read as confident. An idea's fluency is not evidence for it; capping confidence at the source is the same rule as R4 applied to the case where the agent itself is the source. |

## Operating modes

- **Frame** — turn a loose research idea into a falsifiable question with a
  stated null-model plan, before design begins.
- **Design** — turn a framed question into an experiment, simulation, or
  analysis with controls, a comparison plan, and pre-registered success/failure
  criteria.
- **Log** — turn a completed run into evidence-log entries that keep
  observation and interpretation apart and carry provenance and a confidence
  ceiling.
- **Correct** — supersede a prior conclusion without erasing it.
- **Audit** — read an existing program's ledger/claim graph and report where
  R1–R9 are violated (unmeasured claims presented as findings, confidence
  exceeding evidence, silently edited history, closed branches with no
  reopening condition), without changing the program's substantive
  conclusions.

A single invocation is usually one mode. State which mode is active.

## Procedure

### Phase 0 — Detect existing tooling

Run the checks under "Detect before you build" above. State in the first
response what was found (a named existing system, or nothing) and which path
this session follows.

### Phase 1 — Frame the question

1. State the research question in one falsifiable sentence — not "investigate
   X" but "does X exceed Y under condition Z".
2. Write the **falsification criterion**: the specific result that would make
   you abandon the hypothesis. If none can be written, the question is not yet
   ready for an experiment; return to framing.
3. Name the **null model or baseline** this claim will be compared against,
   before running anything. If the claim is about something being
   "training-created," "learned," "caused by the treatment," or similar, name
   the matched-but-untreated control specifically (e.g., an architecture-matched
   untrained model, a shuffled/permuted-label run, a treatment-free arm) — a
   generic or unrelated null is not a substitute.
4. Note competing explanations: at least one alternative account of the same
   observation that the experiment should be able to distinguish from the
   hypothesis under test.

### Phase 2 — Prior-work discovery (mandatory, before designing anything)

1. Search the program's own findings and closed-branch record (the existing
   ledger if Phase 0 found one; otherwise the minimal ledger below) for the
   question, the measurement, and its synonyms — the same object often arrives
   under a different name (e.g., "coupling" / "interaction" / "dependence").
2. Check adjacent, not just identical, prior work: an experiment that measured
   a related quantity for a different reason may have already answered part of
   the question, and its controls are usually still binding.
3. If a closed branch matches, read what would reopen it. If the current idea
   meets that condition, say so explicitly in the new experiment record. If it
   does not, this is the closed branch — do something else, or state
   explicitly that this is intentional replication.
4. If the idea is materially distinct from a closed branch, name the specific
   difference (measurement, control, or scope) that makes it a different
   question, not merely a renamed one.
5. Document the search itself, not just its result: "nothing found" is a claim
   about the search and must record what was searched and how — an absent
   search record is indistinguishable from a search that was never performed.

### Phase 3 — Experimental design

1. Specify controls: the null model / baseline from Phase 1, and any standing
   controls this program has already validated (reuse them; do not
   re-implement a second version of a control that already exists).
2. Specify the comparison/statistical plan *before* looking at results:
   what test or comparison will be used, the multiple-comparisons policy if
   more than one comparison is planned, and any train/validation split or
   held-out data the plan depends on.
3. Specify what counts as success, failure, and inconclusive — in advance.
   "We'll know it when we see it" is not a plan.
4. Specify what is held fixed (seeds, versions, configuration) so the run is
   reproducible, and what varies.
5. Estimate cost against expected information gain. A cheap experiment with an
   unfavorable prior (most likely to further subtract from the claim base) can
   still be the right thing to run next if the alternative is an unmeasured
   claim sitting on the books.

### Phase 4 — Execution discipline

1. Capture run provenance: what was run, on what commit/version, with what
   seed/configuration, and whether the working tree was clean. Best-effort
   capture beats none — a broken recorder must never be allowed to stop the
   actual research, but an exit-clean run with no captured provenance at all is
   itself a defect to flag, not a silently accepted gap.
2. Record deviations from the pre-registered plan as they happen, not
   reconstructed afterward. A deviation is not misconduct; an unrecorded one
   is.
3. Record warnings, partial failures, and anything unexpected while it is
   fresh, even if it seems irrelevant to the current question — Phase 6/7 is
   where irrelevance gets decided, not execution time.

### Phase 5 — Evidence logging

For each finding, log these as separate fields — never collapse them into one
narrative sentence:

- `observation`: the measured quantity, machine-readable, no adjectives.
- `interpretation`: what it currently seems to mean — explicitly revisable.
- `status`: `proposed` / `active` / `supported` / `rejected` / `unmeasured` /
  `superseded` / `open`.
- `evidence_strength`: roughly, none → anecdotal → single run → replicated →
  replicated with independent provenance (R8).
- `confidence`: capped by `evidence_strength` and by provenance class (R4, R9)
  — never set confidence by how compelling the story feels.
- `provenance`: experiment/run id, source, or session that produced this.
- `controls`: the null/baseline actually used.
- `falsification`: what would overturn this specific finding (carried from
  Phase 1, refined if the experiment sharpened it).
- `known_limitations`: what the finding does not establish.
- A discriminative claim with `evidence_strength: none` (no null attached)
  gets `status: unmeasured` — never a supported status with a hedge in prose.

Preserve negative results and residuals explicitly (R7): a rejected hypothesis
is logged with the same rigor as a supported one, including what specifically
falsified it and what should not be repeated without new evidence. An
unexplained observation that survives a real attempt at explanation is
preserved as a residual, not discarded for lack of a story.

### Phase 6 — Corrections

1. Never edit a prior finding or delete a prior record to fix it.
2. Add a new record with `supersedes: [old-id]`; set the old record's status
   to `superseded` (or `rejected` / `retracted`) and point it at the new one.
3. If the correction reveals a methodological error (not just a changed
   reading), add a record naming: what was wrong, which prior claims it
   invalidates, the corrected method, the corrected result, and the new
   control or check that prevents it recurring silently next time. A
   correction record missing the last two is incomplete.

### Phase 7 — Reporting

Report per the involvement contract below, and always distinguish three
registers explicitly rather than blending them: **known** (supported by
evidence meeting its confidence ceiling), **believed but not yet measured**
(a claim currently `unmeasured` or resting on a null that hasn't been run —
say so plainly, do not round up to "known"), and **open** (a stated question
with no attempted answer yet). A status report that cannot tell these apart is
the same failure mode as R2/R3 at the reporting layer.

## Minimal ledger (only when Phase 0 found nothing to defer to)

Scale to the investigation. A short analysis may need only a single
`findings.md`/`findings.yaml` with the Phase 5 fields. A multi-session program
benefits from splitting by record type, e.g. under `research/ledger/`:

- `hypotheses.yaml` — one entry per framed question: statement, falsification
  criterion, null-model plan, status, provenance.
- `experiments.yaml` — one entry per run: objective, hypothesis link,
  controls, statistical plan, status (`planned` → `running` → `completed`),
  deviations.
- `findings.yaml` — Phase 5 fields per finding, linked to the experiment that
  produced it.
- `do_not_repeat.yaml` — closed approaches: what was tried, why it's closed,
  what was learned, and the explicit condition that would reopen it.
- `decisions.yaml` — methodological decisions a later session might otherwise
  reverse without knowing why: what was decided, the rationale, alternatives
  considered, and the consequence.

Put the bootstrap under `research/` (or the target repo's existing top-level
convention for non-code artifacts) so it stays visibly separate from
production code. State clearly, on creation, that this is a lightweight
starting point and name the fuller reference model it is scaled down from if
the operator wants to grow into it later.

## Operator involvement

Binding: `../../000_common/engagement-base/involvement_contract.md`.
Preflight: `../../000_common/engagement-base/preflight_checklist.md`.

1. Take the involvement level from the invocation envelope; infer it from
   request sophistication when absent, and record the inference as an
   assumption.
2. If the operator wants low involvement but cannot evaluate the result, apply
   guarded autopilot (§3): lower the authority ceiling, do not raise the
   question count, and say that you did.
3. Stop at every hard gate regardless of involvement level — status
   promotions, branch closures, and bootstrapping a competing ledger into a
   repository that already has one are hard gates (see `skill.yaml`
   `requires_human_approval_for`).
4. Report per `../../000_common/engagement-base/plain_language_reporting.md`
   when involvement is guided or guarded autopilot applied.

## Catalog-wide execution resilience

For independent, multi-step, batch, delegated, or long-running work, the
preflight execution-shape gate is binding:

- attempt safe concurrency for independent operations when the runtime permits;
- divide work into bounded, durable, independently verifiable checkpoints so a
  lost session or reboot resumes from completed units — one ledger entry, one
  experiment record, or one phase of Phase 2 discovery is a natural unit;
- prefer incremental or resume-capable operations over long monolithic
  processes that restart from the beginning;
- record the dependency, side effect, or tool limit when execution must remain
  sequential or monolithic (e.g., a multi-hour training run genuinely cannot
  be checkpointed mid-epoch without the harness supporting it — say so rather
  than pretending otherwise).

## Hard constraints

- Never present an unmeasured, unnulled discriminative claim as a supported
  finding (R2).
- Never merge observation and interpretation into one field or one sentence
  when logging evidence (R3).
- Never edit or delete a prior finding, decision, or ledger record to correct
  it — supersede it (R6).
- Never delete, bury, or silently reclassify a negative result or an
  unexplained residual (R7).
- Never promote a claim's confidence above the ceiling its evidence strength
  and provenance class support (R4, R9).
- Never propose a new ledger/status scheme in a repository whose existing
  research-discipline tooling (Phase 0) already covers the same ground.
- Do not expose secrets.
- Do not perform destructive cleanup without explicit request.
- Do not preserve private chain-of-thought in logged records — preserve
  decisions, evidence, commands, and concise rationale.

## Completion criteria

- Phase 0 detection result is stated explicitly (found tooling and deferred
  to it, or bootstrapped the minimal ledger and said so).
- Every discriminative claim produced or reviewed this session has a stated
  status consistent with R2 (supported claims have a null; everything else is
  `unmeasured`, not hedged-but-supported).
- Any correction made is an append (Phase 6), not an edit, and both old and
  new records are identified.
- Negative results and residuals encountered this session are preserved, not
  dropped from the record.
- Unresolved items, assumptions, and anything needing an operator decision are
  stated explicitly — a partial run reports `partial`, never `completed`.
