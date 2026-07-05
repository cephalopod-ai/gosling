# Gosling Stress-Test Audit — Session Log

Chronological log of the audit engagement. All timestamps approximate
(2026-07-05). Authority: audit-only / read-only. Branch:
`claude/gosling-stress-test-audit-jwhooa`.

## Objective (from operator)

Run a thorough audit of `gosling` using **every applicable skill** in the
`agent-skills` audit directory (≥28 expected applicable). Stress-test the repo in
every way. It is a **mission-critical baseline** for real productivity work — it
must be reliable, and results must **not be misleading**. Audit + session logs to
`./docs/cloud/`. (Explicitly out of scope: giles / dory guidance.)

## Timeline

1. **Orientation.** Located both repos (`/home/user/gosling`,
   `/home/user/agent-skills`). Read the shared audit contract
   (`00_common/audit-base/`: audit_method v3.0, evidence_discipline,
   severity_matrix, finding_format). Surveyed gosling: ~183K Rust + ~75K TS LOC;
   Rust core + CLI + Electron desktop + server + SDK + MCP; 15+ LLM providers;
   rich security/permission/oauth/execution surface. Wrote
   `00-orientation.md` (surface inventory, trust boundaries, prioritized
   high-value surfaces, applicability matrix).

2. **Applicability decision.** Of 38 audit skills: **3 excluded N/A**
   (`flutter-ios`, `security-supabase`, `equation-sourcebase` — no surface);
   **2 run with limits** (`playtest-app` static — app won't build here;
   `contract-crossrepo` scoped to internal cross-language seams); **33 run in
   full**. `multiagent-consensus` realized as cross-lens convergence + lead
   verification rather than a separate code lens. Total exercised: **35**.

3. **Fan-out.** Dispatched 34 parallel read-only lens subagents in 5 family
   waves (security ×7, dataflow ×9, reliability/failsafe ×7, architecture/
   contract/perf ×7, UI/compliance ×4). Each was bound to: read its SKILL.md +
   the shared base, build on the orientation doc, budget effort with explicit
   sampling, obey evidence discipline (Confirmed ⇒ quoted file:line; races/OOM/
   crash capped at simulation-reasoned), and write one report to
   `docs/cloud/audit-<lens>.md` plus return a calibrated summary.

4. **Collection & incremental checkpointing.** As each lens completed, its report
   was committed and pushed (the execution container is ephemeral, so work was
   pushed continuously to avoid loss). Opened **PR #4** (docs-only) and subscribed
   to its activity. All 34 reports landed (14,793 lines total).

5. **Consensus / verification pass.** The dominant theme — *default security
   posture is non-enforcing* — was asserted by 8 independent lenses; one
   (`pipeline-graph`) rated the "default = Auto" linchpin only *Likely* because
   onboarding might set a stricter mode. The lead independently read the cited
   code and **confirmed** five load-bearing mechanisms against source:
   - `gosling_mode.rs:25-27` — `#[default]` is on `Auto`.
   - `agent.rs:314` — mode read via `unwrap_or_default()`; onboarding does **not**
     default to a stricter mode (the mode dialog is separate and lists Auto first;
     headless force-sets Auto at `session/mod.rs:1063`).
   - `permission_inspector.rs:152` — `Auto => Allow` (no gate).
   - `egress_inspector.rs:369-383` — always `Allow`, confidence 0.0.
   - `security/mod.rs:53-54` — prompt-injection scanner defaults `false`.
   - `config/base.rs:42-62` — secrets write is non-atomic (vs the atomic
     `save_values:658-683`).
   - `tool_inspection.rs:107-114` — inspector errors are swallowed (fail-open).
   All confirmed as described. The "onboarding might make it safe" caveat was
   thereby **retired** — the finding is Confirmed, not Likely.

6. **Merge.** De-duplicated ~150 raw findings into 10 cross-lens clusters,
   severity-ranked, with a recommended patch order and a residual-risk register
   for items needing a drill or an owner decision. Wrote `99-master-report.md`.

## Key outcomes

- **Ship-gating:** Cluster A (default-permissive / non-enforcing controls on every
  surface), Cluster B (untrusted-repo → local code execution via auto-enabled
  project plugins/hooks), Cluster C (non-atomic secrets write = irreversible key
  loss; MCP subprocess env-inheritance leaks secrets).
- **Reassuring non-findings:** SQLite session store (WAL + `BEGIN IMMEDIATE`),
  subprocess lifecycle (Linux PDEATHSIG, test-proven), ACP typed protocol with a
  real CI drift-gate, Electron Fuses + `contextIsolation`, provider retry/backoff
  plumbing, clean dropped-feature removal. Gosling is well-built; the gap is
  posture and defaults, not craftsmanship.
- **Honesty guardrails held:** nothing built or run; every runtime-manifestation
  claim is simulation-reasoned or flagged `requires-authorized-drill`; convergence
  across independent lenses + lead source-verification is the confidence basis.

## Deliverables in docs/cloud/

- `00-orientation.md` — surface inventory + applicability matrix.
- `audit-<lens>.md` ×34 — per-lens reports (findings, non-findings, limits).
- `99-master-report.md` — merged, ranked, de-duplicated master view + patch order
  + residual-risk register.
- `session-log.md` — this file.

## Follow-ups the operator should commission (needing a live environment)

- A build+run pass in an unrestricted environment to drive the approval flow and
  **promote Cluster A from source-confirmed to runtime-observed** (RR-1/RR-3).
- Deep-read of the **ACP subprocess providers** and their
  `--dangerously-skip-permissions` / `bypassPermissions` mapping (flagged thin by
  several lenses).
- GitHub **branch-protection / required-checks / secret-scanning** API queries to
  settle RR-2 (PR-bundle workflow authz) and RR-7 (install-time provenance).
