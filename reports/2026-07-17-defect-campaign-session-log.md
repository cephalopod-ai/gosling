# Defect-repair campaign session log — 2026-07-17

Agent/model: Codex / GPT-5 family. Repository: `cephalopod-ai/gosling`
(local `origin` still uses the redirecting `repo-makeover/gosling` URL).

## Gate 0 — orientation, safety, and git posture

- Starting repair baseline: branch
  `claude/gosling-defect-repair-followup-6093` at `cdf2634ae`.
- Audit checkpoint: committed and remote-verified at
  `cdf2634ae97979c3f8fda438b89004a038e924f8` before repair skill load.
- Worktree: clean; no unrelated changes.
- Git/remote: available; audit push explicitly authorized and complete. Repair
  stages use local commits; no further push is assumed.
- Repo instructions: `AGENTS.md` read. Rust changes require `cargo fmt`. Cargo
  build, test, and Clippy commands are reserved for an explicit user build/test
  request. UI typecheck/test commands exist but are not baseline evidence.
- Baseline validation: unknown/not run by policy. Static source audit and
  `git diff --check` are available substitutes; regression tests will be added
  but recorded as unexecuted unless authorization changes.
- Documentation convention: dated reports and campaign logs under `reports/`,
  durable backlog/status summary in `docs/TODO.md`.
- Protected deferred work: Tagteam workflow activation, earlier audit ledger
  deferrals not present in the frozen inventory, ORCH-002, quick-xml advisories.
- Repo file-size rule: none beyond the loaded repair skill. The skill's
  <=1000 / 1001-1999 / >=2000 modularization rule governs this campaign.

## Gate 1 — inventory

- Finding source: exhaustive static audit checkpoint at
  `reports/2026-07-17-exhaustive-defect-audit-checkpoint.md` using multi-agent
  consensus plus architecture, dataflow, reliability, security, LLM, MCP,
  contract, lifecycle, Node/Electron, GUI, and external-API lenses.
- Counts: 34 in-scope defects; 16 security/P0-or-equivalent high-impact entries,
  17 P1 correctness/reliability/data-integrity entries, 1 P2 frontend defect.
  Complexity: 12 high/medium-high, 12 medium, 10 low.
- Candidate disposition and complete touch sets are recorded in
  `reports/2026-07-17-defect-campaign-plan.md`.
- AUD-031 is reopened by the user's all-findings instruction but retains a
  mandatory stop checkpoint if it requires a broad architectural rewrite.

## Gate 2 — locality grouping

- Thirteen ordered groups cover every in-scope finding exactly once.
- Same-file affinity joins SmartApprove, import, provider outcome, MCP
  pagination, and state-transition defects; same-data-path affinity joins
  lifecycle and desktop-boundary defects.
- Planned in-band modularizations: developer shell process-group seam,
  Codex event-result interpretation seam, and review worker-launch seam.
  Claude/Codex/route files will trigger additional extraction only if their
  planned local edits expand into multiple substantial functions.
- Files >=2000 lines routed for dedicated modularization: `session_manager.rs`,
  `acp/server.rs`, `agent.rs`, `extension_manager.rs`, `summon.rs`, `main.ts`.
- Commit boundary: one local commit per verified group, with hashes appended
  below. Remote synchronization is not repeated without authorization.

## Stage results

### Stage 1 — credential persistence and OAuth input

- Defects: AUD-011 and AUD-033 fixed.
- Changes: ChatGPT Codex, GitHub Copilot, and Kimi token caches now use the
  repository's owner-only atomic secret writer. Databricks workspace/OIDC URL
  parsing now returns contextual errors instead of panicking.
- Regression guardrails added: Unix mode assertions for each cache and an
  invalid Databricks host test.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: `git diff --check` passed. Regression tests were not
  executed under the repository's build/test authorization rule.
- Adversarial review: verified parent-directory creation remains supplied by
  the shared helper, existing-file replacement receives 0600 through atomic
  rename, invalid input performs no request, and cache clear/load contracts are
  unchanged. No additional finding.
- Change review: scoped to four provider files plus this log; no unrelated
  formatting or protected deferred work.
- Commit: `ae9264458`.

### Stage 2 — SmartApprove authority

- Defects: AUD-003 and AUD-004 fixed.
- Changes: MCP `readOnlyHint=true` is advisory and can no longer populate
  `AlwaysAllow`; `readOnlyHint=false` still tightens policy. LLM read-only
  results authorize only the concrete call and are no longer persisted by
  tool name. Negative decisions remain safely cached as `AskBefore`, and old
  positive cache entries are reclassified instead of trusted.
- Regression guardrails: annotation tests now query the actual tool name and
  cover an innocuously named hostile tool; legacy positive, negative, and deny
  cache behavior is explicit. Permission-judge coverage continues to assert
  that concrete arguments enter classification.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: `git diff --check` passed. Tests were not executed under
  the repository authorization rule.
- Adversarial review: verified explicit user `AlwaysAllow` remains authoritative,
  extension management remains approval-gated, legacy smart-positive entries
  cannot bypass classification, and classifier failure tightens to approval.
- Change review: three permission files plus this log; obsolete name heuristics
  and misleading cache comments removed.
- Commit: `dac101bcf`.

### Stage 3 — session import trust boundary

- Defects: AUD-001 and AUD-002 fixed.
- Changes: native import removes the versioned enabled-extension state before
  persistence, preventing imported Stdio/InlinePython/builtin/platform configs
  from becoming automatic resume-time authority. The working-directory
  restriction flag is now applied explicitly during import.
- Regression guardrail: the export/import round-trip fixture includes a Stdio
  payload and safe Todo state; it asserts executable state is quarantined,
  safe state survives, and the restriction remains enabled.
- Modularization: `session_manager.rs` is >=2000 lines, so the patch is local
  and the file remains routed for dedicated modularization.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: `git diff --check` passed; tests not executed by policy.
- Adversarial review: verified all executable-capable imported variants share
  the removed versioned key, non-executable extension state is preserved, and
  trusted local config remains the fallback after quarantine.
- Change review: two session files plus this log, no schema or unrelated import
  behavior changed.
- Commit: `a52862231`.

### Stage 4 — extension secret and process-environment boundaries

- Defects: AUD-009 and AUD-010 fixed.
- Changes: configured environment values are rehydrated only when a client
  echoes the exact stored Stdio command/arguments or HTTP URI/headers/socket,
  preventing same-name endpoint substitution. Inline Python now receives the
  same allowlisted child environment as Stdio extensions instead of inheriting
  the server's full process environment.
- Regression guardrails: Stdio and HTTP endpoint-redirection cases cover each
  identity field, and a Unix subprocess test verifies an inherited secret is
  removed by the shared minimal-environment policy.
- Modularization: `acp/server.rs` and `extension_manager.rs` are >=2000 lines,
  so both patches remain local and both files stay routed for dedicated
  modularization.
- Formatting: `source bin/activate-hermit && cargo fmt` passed.
- Static verification: `git diff --check` passed; tests not executed by policy.
- Adversarial review: verified client-supplied values still win on an exact
  destination, mismatched transport variants cannot cross-match, configured
  HTTP headers are part of destination identity, and the Inline Python launch
  adds no environment after applying the allowlist.
- Change review: two implementation files plus this log; extension transport
  and lifecycle behavior are otherwise unchanged.
- Commit: `f69a00d87`.

### Stage 5 — desktop content, filesystem, and transport boundaries

- Defects: AUD-012, AUD-013, and AUD-032 fixed.
- Changes: untrusted Markdown images render as inert alt-text placeholders and
  the renderer CSP no longer permits arbitrary HTTPS image loads. Renderer IPC
  file operations canonicalize existing paths and the nearest existing parent
  for prospective paths, reject symlink escapes and dangling symlink ancestors,
  and store artifact-picker grants canonically. The ACP WebSocket receive path
  now has per-message, aggregate-character, and message-count bounds, respects
  ReadableStream backpressure, and closes oversized peers with code 1009.
- Regression guardrails: Markdown rendering and CSP tests cover remote-image
  suppression; path tests cover legitimate existing/missing paths, `..`-prefixed
  names, existing/missing symlink escapes, and dangling links; WebSocket tests
  cover queue and single-message overflow.
- Modularization: the focused canonical-path helper is isolated for testing;
  `main.ts` remains >=2000 lines and routed for dedicated modularization rather
  than undergoing a broad split in this campaign.
- Formatting: `source bin/activate-hermit && cargo fmt` and targeted Prettier
  formatting passed.
- Static verification: `pnpm exec tsc --noEmit` and `git diff --check` passed.
  The package-script wrapper could not run because the environment has pnpm
  10.6.4 while the manifest requires >=10.30.0; the same local compiler was
  invoked directly through `pnpm exec`.
- Tests: four targeted Vitest files passed, 51 tests total.
- Adversarial review: verified canonical paths, not attacker-controlled lexical
  aliases, reach filesystem APIs; dangling links fail closed; stale invalid
  approved roots cannot deny unrelated valid roots; the receive buffer is
  bounded by both item count and encoded size; and only one parsed message can
  enter the ReadableStream queue per pull.
- Change review: desktop boundary files, focused test helpers, and this log;
  generated API code and unrelated renderer behavior are untouched.
- Commit: pending.

## Campaign closeout

- Final regression/adversarial walkthrough: pending.
- Documentation refresh: plan and session log created; `docs/TODO.md` pending.
- Residual risks/follow-up: pending.
- Final status: `partially_completed_groups_remaining`.
