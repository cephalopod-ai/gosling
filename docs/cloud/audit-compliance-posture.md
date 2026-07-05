# Compliance / Posture Audit — Honest Posture / Claim-vs-Evidence Lens

Lens: `audit-compliance-posture` (skill v3.1, domain CMP) · Target: `gosling` @ `/home/user/gosling`
Authority: **audit-only / read-only**. Only this file was written.
Builds on `docs/cloud/00-orientation.md`. Effort: ~22 tool calls (within the ~30–45 budget).

> The supplied prompt is treated as a draft. I preserved the intended mission (does the repo
> OVERCLAIM or MISSTATE its own posture?) and adapted the CMP taxonomy: gosling is **not** a
> compliance-reporting tool (no SSDF collector, SARIF, release gate, or JSON+Markdown posture
> outputs), so several CMP codes are N/A (recorded below). The applicable core is claim-vs-evidence
> in the repo's own governance surface — README claims, LICENSE/Apache-2.0 attribution, SECURITY.md
> posture, provenance/version accuracy, and disclaimers.

---

## 1. Surface inventory (posture-bearing artifacts)

| Artifact | Role | Read |
|---|---|---|
| `README.md` | Public claims: provenance, footprint/perf table, "what's new", disclaimers | full |
| `LICENSE` | Apache-2.0 text + retained copyright | head/appendix/tail |
| `UPSTREAM.md` | Fork provenance + preservation policy | full |
| `SECURITY.md` | Security posture / reporting | full |
| `deny.toml` | Advisory-handling posture (cargo-deny) | full |
| `MAINTAINERS.md` | Governance | full |
| `Cargo.toml` (workspace) | Crate/CLI version = `1.40.0` | version block |
| `ui/desktop/package.json` | Desktop app version = `0.0.1` | version |
| `ui/desktop/src/main.ts` | About dialog / tray version + provenance strings | lines 734, 2695–2701 |
| `documentation/STRUCTURE_COMPLIANCE.md`, `DOCUMENTATION_INVENTORY.md` | The fork's own rebrand/compliance ledger | grep |
| `documentation/blog/2026-07-04-welcome-to-gosling/index.md` | Public provenance restatement | grep |

## 2. Boundary map (what honest posture requires here)

- **Provenance accuracy**: one authoritative version identity; credited originator == retained copyright holder; links resolve to the credited party.
- **Claim scoping**: measured claims carry method/date/caveats; equivalence claims carry evidence.
- **Attribution (Apache-2.0 §4)**: retain copyright + license; reproduce upstream NOTICE if one existed.
- **Advisory honesty**: security/compliance prose must not assert enforcement/certification/guarantee beyond code.
- **Disclaimer correctness**: "not affiliated/endorsed" present where a fork of a named project is claimed.

## 3. Posture inventory table

| Claim / Control | Evidence Source | Evidence Grade | Mapping Confidence | Gap Language | Enforcement? |
|---|---|---|---|---|---|
| gosling version = "v0.0.1" | `README.md:23,54` | contradicted by `Cargo.toml:11` (1.40.0) | High | ambiguous | none |
| gosling version = "v1.40.0" | `README.md:31`, `Cargo.toml:11` | strong (build metadata) | High | — | none |
| goose baseline = v1.38 | `README.md:23`, `UPSTREAM.md:12` | asserted | Med | — | none |
| goose comparator = v1.41.0 | `README.md:31` | pinned commit (present in repo) | High | — | none |
| Footprint/perf deltas | `README.md:31–43` | **well-caveated / measured** | High | scoped | none |
| "Core functionality unchanged" | `README.md:43` | **no evidence cited** | High | absolute | none |
| goose originator = AAIF | `README.md:23`, `UPSTREAM.md:3` | contradicts `LICENSE` (Block, Inc.) | Med | — | none |
| Apache-2.0 attribution | `LICENSE` (Block, Inc. 2024) | retained; no NOTICE file | Med | absent-NOTICE | none |
| Security posture | `SECURITY.md` | honest advisory | High | advisory | none |
| Advisory handling | `deny.toml:11–13` | honest (documented ignore) | High | advisory | none |
| Not-affiliated disclaimer | `README.md:23`, `UPSTREAM.md:3,5` | present & correct | High | — | none |

---

## 4. Findings

### CMP-GSL-001: gosling has three conflicting version identities in one release surface

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- `README.md:23` — "gosling **v0.0.1** is a fork of goose **v1.38**".
- `README.md:31` — benchmark compares "goose v1.41.0 (commit `181cbbe`) and **gosling v1.40.0** (commit `5b7d039`)".
- `README.md:54` — "Help → About shows that this is **Gosling v0.0.1**, a fork of goose v1.38."
- `Cargo.toml:11` — workspace/CLI `version = "1.40.0"`.
- `ui/desktop/package.json:4` — `"version": "0.0.1"`.
- `ui/desktop/src/main.ts:734` — About credits `Gosling v${app.getVersion()}` (Electron app version → `0.0.1`).

Observed behavior:
- The same README labels gosling both **v0.0.1** (Provenance/About) and **v1.40.0** (footprint benchmark). The two are real, divergent build identities: the Rust CLI/crates ship as `1.40.0`; the Electron desktop app ships as `0.0.1`. A user asking "what gosling version am I on?" gets `1.40.0` from the CLI and `0.0.1` from the desktop About dialog. The upstream anchor is equally split: provenance says forked from goose **v1.38**, but the benchmark comparator is goose **v1.41.0** — i.e. the comparison is *not* against the version gosling forked from.

Expected boundary:
- One authoritative version per artifact, and a README that does not present two different numbers as "the gosling version" without explaining they name different components. (CMP-001 Framework/Version Ambiguity, adapted.)

Failure mechanism:
- The CLI kept goose's version lineage (goose reached 1.41.0; gosling sits at 1.40.0), while the desktop app and the marketing narrative were reset to a fresh `0.0.1`. Neither the README nor UPSTREAM.md reconciles the two numbering schemes.

Break-it angle:
- A downstream consumer pinning "gosling v0.0.1" from the README will not match the CLI's `1.40.0` in `--version`; a reproducer following "fork of goose v1.38" cannot reproduce the benchmark, which was run against goose v1.41.0.

Impact:
- Provenance/version confusion; unreliable version pinning; weakened reproducibility of the very claims the README makes.

Operational impact:
- Blast radius: Repo (public claim surface). Side-effect class: user-visible. Reversibility: reversible. Operator visibility: UI-visible (About shows 0.0.1 while CLI shows 1.40.0). Rerun safety: safe.

Adjacent failure modes:
- Feeds CMP-GSL-002 (the "unchanged" claim spans yet a third code state).

Recommended mitigation:
- Remediation pattern: single-source-of-truth versioning + explicit component labels.
- Minimal repair: in `README.md`, state the CLI and desktop versions distinctly (e.g. "gosling CLI 1.40.0 / desktop 0.0.1") and say plainly which version the benchmark used; label the fork point ("forked from goose v1.38; benchmarked against goose v1.41.0").
- Local guardrail: a docs check asserting README version strings match `Cargo.toml`/`package.json`.
- Behavior test: assert `README` contains the exact `Cargo.toml` and `package.json` version tokens.

Implementation assessment:
- Complexity: operator_ux. Cost: XS. Cost drivers: docs. Nominal agent: claude. Rationale: pure documentation reconciliation, no code behavior change.

Validation:
- Test: grep README version tokens and diff against `Cargo.toml:11` / `ui/desktop/package.json:4`; both must be represented and correctly attributed to their component.

Non-goals:
- Do not renumber the crates or the app; only reconcile the prose.

---

### CMP-GSL-002: "Core functionality unchanged" asserted as fact with no cited evidence, across mismatched versions

Severity: Medium
Confidence: Confirmed (on wording)
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- `README.md:43` — "**Core agent/session/MCP functionality is unchanged between the two.** Actual LLM conversation/tool-calling throughput wasn't benchmarked (no provider configured…) and isn't expected to differ…".
- `README.md:31` — "the two" = **goose v1.41.0** vs **gosling v1.40.0** (different code states; gosling forked at v1.38 per `README.md:23`).

Observed behavior:
- The footprint section makes a flat behavioral-equivalence claim ("functionality is unchanged") between two artifacts that are different versions of different codebases, and cites no parity evidence (no shared test-suite result, no behavioral diff). Notably, the *adjacent* sentence about throughput **is** properly hedged ("wasn't benchmarked", "isn't expected to differ") — the asymmetry shows the author knew how to caveat but stated functional equivalence as fact.

Expected boundary:
- An equivalence claim must be scoped to what evidence supports (CMP-003 Evidence Overclaim). The measured footprint numbers are fine; "functionality unchanged" is a testable assertion that needs a test or a hedge.

Failure mechanism:
- Measurement (binary size, cold-start) is conflated with functional parity; the strong word "unchanged" is applied to un-benchmarked behavior across three distinct code states (goose 1.38 fork point, gosling 1.40.0, goose 1.41.0 comparator).

Break-it angle:
- gosling drops the `recipe`, `schedule`, `gateway`, and `local-models` subcommands (stated one sentence earlier at `README.md:43`) — so functionality is demonstrably *not* wholly unchanged; the sentence relies on the reader interpreting "core" narrowly and un-provably.

Impact:
- Adopters may assume drop-in behavioral parity with goose and skip their own validation.

Operational impact:
- Blast radius: Repo. Side-effect class: user-visible. Reversibility: reversible. Operator visibility: UI-visible. Rerun safety: safe.

Adjacent failure modes:
- CMP-GSL-001 (version mismatch amplifies the ambiguity of "the two").

Recommended mitigation:
- Minimal repair: rescope to evidence — e.g. "the retained agent/session/MCP **code paths** are carried over from the goose v1.38 baseline; behavioral parity was not tested," matching the tone already used for throughput.
- Local guardrail: none needed; wording change.
- Behavior test: n/a (docs); optionally add a note pointing to the subcommands actually removed.

Implementation assessment:
- Complexity: operator_ux. Cost: XS. Cost drivers: docs. Nominal agent: claude. Rationale: single-sentence rescope.

Validation:
- Reviewer check: the sentence no longer asserts un-evidenced equivalence; it states carried-over code + untested behavior.

Non-goals:
- Do not remove the (well-caveated) footprint table.

---

### CMP-GSL-003: Credited originator (AAIF) contradicts the retained Apache-2.0 copyright holder (Block, Inc.)

Severity: Medium
Confidence: Confirmed (internal contradiction) / external truth Not Confirmed
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- `LICENSE` (appendix) — "**Copyright 2024 Block, Inc.**" (the retained upstream copyright notice).
- `README.md:23` — "the open source AI agent from the **[Agentic AI Foundation (AAIF)](https://aaif.io/)** at the Linux Foundation … All credit … goes to the goose project".
- `UPSTREAM.md:3` — "goose … originally **created by the Agentic AI Foundation (AAIF)**"; repo link `https://github.com/aaif-goose/goose` (`UPSTREAM.md:10`).
- `documentation/blog/2026-07-04-welcome-to-gosling/index.md:7` — same AAIF attribution.

Observed behavior:
- Every human-readable provenance statement credits goose to "Agentic AI Foundation (AAIF)" and links `aaif-goose/goose` / `aaif.io`, while the Apache-2.0 attribution actually preserved in the repo names a **different entity, Block, Inc. (2024)**. The actual copyright holder in the license file is never named anywhere in the prose.

Expected boundary:
- The credited originator, the linked source, and the retained Apache-2.0 copyright notice must name the same party (or the prose must explain a donation/transfer). Attribution accuracy is a claim-vs-evidence obligation (CMP-001 authority-ambiguity + Apache-2.0 §4 attribution).

Failure mechanism:
- The retained license copyright (Block, Inc.) was preserved correctly per §4, but the narrative substitutes an unrelated "AAIF" as originator without reconciling it to the copyright holder or explaining a transfer.

Break-it angle:
- A licensee doing attribution due-diligence sees "Block, Inc." in `LICENSE` but is told the project is "from AAIF"; they cannot determine whom to attribute or whether `aaif-goose/goose`/`aaif.io` are the authoritative upstream.

Impact:
- Misstated provenance; possible incorrect attribution of a third party's copyrighted work to an entity that does not hold the copyright.

Operational impact:
- Blast radius: Repo (legal/attribution surface). Side-effect class: user-visible. Reversibility: reversible. Operator visibility: silent (only visible on careful reconciliation). Rerun safety: safe.

Adjacent failure modes:
- CMP-GSL-004 (NOTICE absence), CMP-GSL-005 (scoped "resolved" claim).

Recommended mitigation:
- Minimal repair: reconcile the originator. Either (a) credit the actual `LICENSE` copyright holder (Block, Inc.) and correct the upstream link, or (b) if a Block→AAIF transfer/donation is real, state it explicitly and update the `LICENSE` copyright per the transfer.
- Local guardrail: a docs check that the copyright entity named in `LICENSE` also appears in `UPSTREAM.md`.
- Behavior test: assert the `LICENSE` copyright holder string is present in the provenance doc.

Implementation assessment:
- Complexity: governance_decision. Cost: S. Cost drivers: docs, external verification of upstream identity. Nominal agent: human-owner. Rationale: requires resolving the real upstream identity/ownership before rewording — not an autonomous edit.

Validation:
- Reviewer check: credited originator == `LICENSE` copyright holder, or an explicit, sourced transfer note exists.

Non-goals:
- Do not alter the retained `LICENSE` text without an authoritative basis.

Validation limit:
- Which of "Block, Inc." vs "AAIF" is factually correct, and whether `aaif-goose/goose`/`aaif.io` resolve to the real upstream, was **not** verified (no upstream fetch in this read-only pass). The *internal* contradiction is Confirmed; the *external* resolution is out of scope.

---

### CMP-GSL-004: No NOTICE file for an Apache-2.0 fork (advisory attribution gap)

Severity: Low
Confidence: Plausible
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- Repo-wide search for `NOTICE*` / `THIRD*PARTY*` / `*ATTRIB*` (excluding `.git`, `target`, `node_modules`) returned **no NOTICE file**; only `LICENSE` and `crates/gosling-mcp/licenses` (six JS-library `.license` files) exist.
- `LICENSE` retains "Copyright 2024 Block, Inc." and the full Apache-2.0 text.
- `UPSTREAM.md:15–18` — "Preserve upstream copyright notices, license text, attribution".

Observed behavior:
- Apache-2.0 §4(d) requires a derivative work to carry a readable copy of the attribution notices in the original work's NOTICE file **if one existed**. gosling ships no NOTICE.

Expected boundary:
- If upstream goose distributed a NOTICE, the fork must reproduce its attribution notices. (Worded as absence, **not** as a proven violation, per CMP-004.)

Failure mechanism:
- A NOTICE, if it existed upstream, was not carried into the fork; cannot be confirmed without the upstream.

Break-it angle:
- If goose's NOTICE contained third-party attributions, redistributing gosling binaries without them would be a §4(d) gap.

Impact:
- Possible incomplete Apache-2.0 attribution in redistributed artifacts.

Operational impact:
- Blast radius: Repo. Side-effect class: none (source). Reversibility: reversible. Operator visibility: silent. Rerun safety: safe.

Recommended mitigation:
- Minimal repair: fetch upstream goose's NOTICE (if any) and reproduce it; if none existed, record that fact in `UPSTREAM.md` so the absence is deliberate and documented.

Implementation assessment:
- Complexity: governance_decision. Cost: S. Cost drivers: external verification, docs. Nominal agent: human-owner. Rationale: depends on upstream artifact not present in this repo.

Validation:
- Check: either a NOTICE exists matching upstream, or `UPSTREAM.md` states upstream had none.

Non-goals:
- Do not fabricate a NOTICE; only reproduce a real upstream one.

Validation limit: whether goose shipped a NOTICE was **not** verified (no upstream fetch). This is worded as an *absence to resolve*, not a confirmed noncompliance.

---

### CMP-GSL-005: "Resolved / zero hits" rebrand claim is scoped to `documentation/docs/**` but read as repo-wide

Severity: Low
Confidence: Confirmed (on wording)
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- `documentation/STRUCTURE_COMPLIANCE.md:48` — "swept `documentation/docs/**` … 'AAIF', 'Agentic AI Foundation', 'LF Projects', 'Linux Foundation' … **zero hits** — this item is **resolved**."
- Yet AAIF/Linux-Foundation attribution persists at `README.md:23`, `UPSTREAM.md:3,10`, and `documentation/blog/2026-07-04-welcome-to-gosling/index.md:7` — all **outside** the swept `documentation/docs/**` path.

Observed behavior:
- The compliance ledger marks the AAIF-affiliation cleanup "resolved" with "zero hits", but the sweep covered only one subtree; three top-level provenance surfaces still carry the AAIF/Linux-Foundation language.

Expected boundary:
- A "resolved / zero hits" posture statement must scope its coverage so it is not read as repo-wide (CMP-003 evidence-scope; the closest analog to evidence-grade-inflation for a self-attested remediation).

Failure mechanism:
- The sweep scope (`documentation/docs/**`) is narrower than the claim's apparent reach; README/UPSTREAM/blog were not in scope.

Mitigating context (not a defect by itself):
- The item DOCSTEW-20260703-010 was specifically about *governance material falsely implying LF/AAIF governance*; README/UPSTREAM frame AAIF as *attribution* and explicitly disclaim affiliation (`README.md:23`, `UPSTREAM.md:5`). So the residual mentions are arguably in-scope-acceptable attribution, not the governance overclaim that was fixed. This keeps the finding at Low — it is a scoping-clarity issue, not a false remediation.

Impact:
- A reader treating "resolved" as repo-wide may believe all AAIF references were removed; they were not (see CMP-GSL-003 for why the residual ones also contradict the LICENSE).

Operational impact:
- Blast radius: Repo (internal ledger). Side-effect class: none. Reversibility: reversible. Operator visibility: log-only (internal doc). Rerun safety: safe.

Recommended mitigation:
- Minimal repair: annotate the "resolved" row with its exact sweep scope (`documentation/docs/**`) and cross-reference the still-present README/UPSTREAM/blog attributions (deliberately retained as attribution).

Implementation assessment:
- Complexity: operator_ux. Cost: XS. Cost drivers: docs. Nominal agent: claude. Rationale: one-line scope annotation.

Validation:
- Reviewer check: the ledger row names its sweep path and does not imply repo-wide removal.

Non-goals:
- Do not remove the legitimate attribution from README/UPSTREAM.

---

## 5. Non-findings (checked and held)

- **Footprint / performance table is adequately caveated** (`README.md:31–43`). It pins a **date** (2026-07-04), **both commits** (`181cbbe`, `5b7d039` — the gosling commit `5b7d039` is present in this repo via `git cat-file`), **same host**, **matched Cargo feature flags**, explicitly notes the `code-mode`/`v8-goose` exclusion is **symmetric**, and states throughput **was not benchmarked**. This is calibrated measurement language, not absolute marketing. **Not** an overclaim (CMP-003) and **not** stale evidence (CMP-015) — the comparator commits are pinned. The only defect in this section is the un-evidenced *functionality* sentence (CMP-GSL-002), not the numbers.
- **SECURITY.md is honest advisory posture** (`SECURITY.md:1–15`). It uses `[!CAUTION]`, pushes mitigation onto the user, explicitly acknowledges prompt injection ("gosling may follow commands found embedded in content"), and routes reports to the repo Security tab. No certification, enforcement, or guarantee language; no draft-treated-as-final. It also makes **no** claim about the code's own security inspectors, so there is no advisory-gap-as-guarantee overclaim (CMP-005/CMP-010 held).
- **Not-affiliated / not-endorsed disclaimers present and correct** (`README.md:23`, `UPSTREAM.md:3,5`). A fork of a named project correctly disclaims endorsement/affiliation.
- **License badge matches LICENSE** — Apache-2.0 badge (`README.md:10–12`) matches the Apache-2.0 `LICENSE`. No license-mismatch.
- **`deny.toml` advisory handling is honest** (`deny.toml:11–13`). The single ignored advisory (`RUSTSEC-2023-0071`, rsa Marvin-attack timing sidechannel) is documented with rationale ("no safe upgrade available, via jsonwebtoken"), not silently suppressed. Yanked crates set to `deny`. Good posture; not hidden-residue.
- **Dropping crates does not create a new license obligation.** Removing the local-inference stack (candle/llama.cpp/MLX/HF, 148 crates per `README.md:43`) only *removes* attribution obligations for those deps; it cannot add any. No stale attribution pointing at removed deps was found (the only third-party `.license` files, `crates/gosling-mcp/licenses/*`, are for still-bundled JS viz libraries).
- **Residual goose brand marks in the desktop UI are minimal** — case-insensitive `goose` (excluding `gosling`) appears in only 3 files under `ui/desktop/src` (`settings.test.ts` legacy `externalGoosed` key; `main.ts:2701` provenance string). The README claim that "goose branding has been replaced … across the desktop app" (`README.md:47`) is largely supported. Completeness of the rebrand is deferred to the `workflow-gui` lens; only spot-checked here.

## 6. CMP codes recorded N/A (with reason)

- **CMP-002 Draft-Treated-As-Final**, **CMP-006 Wrong Control Mapping**, **CMP-007 Evidence-Grade Inflation**, **CMP-008 Report Format Drift**, **CMP-009 Collector Scope Violation**, **CMP-011 Tool-Output-As-Ground-Truth**, **CMP-013 Profile Misapplied**, **CMP-014 Release-Gate Semantics Drift** — N/A: gosling is an agent framework, not a compliance/posture engine. It has no framework-grading collector, no evidence→control mapping rules, no SARIF/JSON+Markdown posture reports, and no release-gate that consumes advisory findings. No surface exists to exhibit these codes.
- **CMP-010 Certification Language Without Authority** — checked (`README.md`, `SECURITY.md`) for "certified/attest/compliant with/guarantee"; none beyond the correct Apache-2.0 badge. Held.
- **CMP-015 Stale Compliance Evidence** — checked the benchmark's dated, commit-pinned evidence; anchor commit present in-repo. Not stale. Held.
- Applicable codes exercised: **CMP-001** (→ CMP-GSL-001, and provenance angle of CMP-GSL-003), **CMP-003** (→ CMP-GSL-002, CMP-GSL-005), **CMP-004** discipline applied (CMP-GSL-004 worded as absence), **CMP-012** policy/practice flavor (CMP-GSL-003 attribution).

## 7. Break-it review (per skill checklist)

- *Remove repo-surface evidence, check "absent" vs "noncompliant":* applied to the NOTICE gap — worded as absence (CMP-GSL-004), never as violation.
- *Feed a draft artifact graded as final:* no framework-grading engine exists → N/A.
- *Advisory gap gates a release:* no release gate consumes advisory posture → held.
- *Diff JSON vs Markdown:* no dual-format posture report exists → N/A.
- *Weak signal graded strong:* the "functionality unchanged" sentence is the closest analog (measurement generalized to behavioral equivalence) — flagged CMP-GSL-002.
- *Version/authority ambiguity:* actively attacked — found three version identities (CMP-GSL-001) and an originator↔copyright contradiction (CMP-GSL-003).

## 8. Patch order (highest value first)

1. **CMP-GSL-003** (attribution accuracy — legal surface; needs human-owner to resolve upstream identity).
2. **CMP-GSL-001** (version reconciliation — reproducibility + operator truth).
3. **CMP-GSL-002** (rescope the "unchanged" sentence).
4. **CMP-GSL-004** (resolve/document NOTICE).
5. **CMP-GSL-005** (annotate ledger scope).

## 9. Regression / guardrail tests

- Docs test: README version tokens ⊇ {`Cargo.toml` version, `package.json` version}, each labeled by component (guards CMP-GSL-001).
- Docs test: `LICENSE` copyright-holder string appears in `UPSTREAM.md` (guards CMP-GSL-003).
- Attribution check: NOTICE present, or `UPSTREAM.md` records upstream had none (guards CMP-GSL-004).
- Lint: reject un-hedged behavioral-equivalence phrasing in `README.md` footprint section (guards CMP-GSL-002) — or manual reviewer check.

## 10. Validation Limits (what was NOT reviewed)

- **No upstream fetch.** Whether goose's real originator is Block, Inc. or "AAIF", whether `aaif-goose/goose`/`aaif.io` are authentic, whether goose ships a NOTICE, and whether commits `181cbbe`/goose-side are real were **not** verified. CMP-GSL-003 and -004 rest on *internal* repo contradictions only; external truth is out of scope for this read-only pass.
- **Benchmark numbers not reproduced.** The footprint/perf table was assessed for *caveat quality*, not re-measured (release builds are heavy and the environment blocks the `v8-goose` download). Its numeric accuracy is unverified.
- **Docs sampled, not exhaustive.** Focused on root governance surface (README, LICENSE, UPSTREAM, SECURITY, deny.toml, MAINTAINERS) plus the rebrand ledgers and welcome blog. `documentation/docs/**` content beyond grep, `CONTRIBUTING.md` body, and per-crate license headers were not deep-read.
- **Branding completeness deferred.** Full residual-goose-mark inventory belongs to the `workflow-gui`/branding lens; only a 3-file spot-check was done.
- **`crates/gosling-mcp/licenses/` not line-audited** beyond confirming the six files are for still-bundled JS libraries.

---

Skill Escalation: CMP-GSL-003 (attribution/originator contradiction) should be cross-checked by the audit lead against any `dependency-criticality` / repo-posture finding on upstream identity, and requires a **human-owner** decision (legal/provenance) before any patch. No race/OOM/timeout claims are made in this lens.
