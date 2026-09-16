# 2026-09-16 — Architecture-drift and low-complexity defect repair

## Task and authority

The user authorized fixing "all issues that don't require human feedback" from an
architecture-drift audit and a follow-on low-complexity defect hunt, then auditing
those fixes and patching what that surfaced. Read order completed: `AGENTS.md`
(`GEMINI.md` absent — see follow-ups), `README.md`, `docs/INDEX.md`,
`.architecture/{README,components,invariants}.yaml`, `.giles/repo.yaml`, and recent
`docs/logs/session/` entries. Skills used: `audit-architecture-drift` (discovery,
read-only) and `repair-defect-patchset` (this repair, Gates 0-9).

Baseline: `main` @ `28c17b7a1`, working tree clean. The prior V8-helper repair from
this session had already landed as `28c17b7a1` (committed by a concurrent session
against this checkout).

## Finding dispositions

Eight findings entered Gate 1. Two did not survive validation and were **not** patched.

| ID | Finding | Disposition |
|---|---|---|
| F1 | `ARC-001`/`ARC-002` deleted instead of retired when `gosling-server` was removed | fixed |
| F2 | `GEMINI.md` required by `AGENTS.md` but never committed | deferred — needs human decision |
| F3 | ADR-0012 references the removed `gosling-server` crate | fixed |
| F4 | `.giles/` advisory metadata records a crashed 2026-07-07 run | recorded — not agent-owned |
| F5 | CI `cargo test` masks failures (no `--no-fail-fast`) | fixed |
| F6 | `acp_provider_test.rs:98` ignore reason was factually wrong | fixed |
| F7 | "no coverage for delete-then-prompt -> `ResourceNotFound`" | **not-a-defect** |
| F8 | "ignored test missing from the TODO ledger" | **not-a-defect** |

F7 and F8 rested on the false premise in F6's comment. `run_prompt_error` is generic
over `Connection`; `acp_server_test.rs:837` runs it un-ignored on `AcpServerConnection`,
and that test passes. The behaviour is covered where `delete_session` actually lives, so
there is no coverage gap and no actionable debt to ledger. Recording them as
not-a-defect rather than manufacturing a ledger entry.

## Changes

- `.architecture/invariants.yaml`: restored `ARC-001` and `ARC-002` as `status: retired`
  with `retired_in: 9f470b5b9` and a `retired_note`, matching the `ARC-005/006/007`
  precedent. Rule, scope, and check text are verbatim from `aeb9f1af2` — the registry
  README requires the entry to persist so the id is never reused and the rule stays
  discoverable. Registry id continuity is now unbroken (`ARC-001`..`ARC-013`).
- `.github/workflows/ci.yml`: added `--no-fail-fast` to both `cargo test` invocations
  and latched the exit status so a failure in the first suite no longer prevents the
  scenario suite from running. Overall pass/fail semantics are unchanged.
- `crates/gosling/tests/acp_provider_test.rs`: corrected the `#[ignore]` reason on
  `test_prompt_error_session_not_found`. It claimed "ensure_session lazy-creates
  sessions so deleted ones reappear"; `ensure_session` exists nowhere in the repository
  and the test actually fails at its first line with `not implemented for
  AcpProviderConnection`. Now carries the accurate reason already used by its sibling at
  line 30.
- `docs/adr/0012-shell-domain-adapter-topology.md`: dated amendment in the status block
  noting `gosling-server` was removed in `9f470b5b`. Body unchanged, following the
  `reports/ADR-2026-07-12-tagteam-workflow.md` precedent for annotating rather than
  rewriting a decision record.

## Validation

- `scripts/test-with-rusty-v8-cache.sh` — pass.
- `cargo test -p gosling --test acp_server_test test_prompt_error_session_not_found` —
  1 passed. Confirms deleted sessions do not reappear and F7 is not a gap.
- `cargo test -p gosling --test acp_provider_test test_prompt_error_session_not_found
  -- --ignored` — fails at `delete_session` with `not implemented for
  AcpProviderConnection`, which is the evidence behind the F6 correction.
- `python3 -c "import yaml; yaml.safe_load(...)"` on `ci.yml` and `invariants.yaml` — parse clean.
- Registry path audit: every `components.yaml` path and every active-invariant `scope`
  path resolves; retired entries skipped per the registry README.
- `docs/INDEX.md`: 69 link targets, 0 missing.
- `grep -R "GILES:DOCS-GOVERNANCE:START" -n AGENTS.md` — 2 occurrences, intact.

Not run: full `cargo test`, `cargo clippy`, and the Desktop suite. No Rust or TypeScript
behaviour changed in this pass — the only Rust edit is an `#[ignore]` attribute string —
so the targeted tests above are the relevant coverage. This is stated as a scope limit,
not a claim of full-suite green.

## Self-audit of these repairs (Gates 8-9)

Auditing this patch set against itself surfaced two defects in the repair work, both
fixed before this log was finalised:

- **`retired_in` hash format.** The restored entries used a 9-character short hash while
  the existing `ARC-005/006/007` entries use 8. Normalised to `9f470b5b` (verified
  unambiguous) to preserve the file's established convention.
- **Session log was silently untrackable.** `.gitignore:18` ignores
  `docs/logs/session/*` and re-admits each log with an explicit `!` negation. Writing
  this log without adding its negation line would have left it invisible to git — the
  mandated evidence artefact, absent from the commit. Negation added.

Verified as non-issues: nothing parses `.architecture/invariants.yaml` programmatically
(the `input.architecture` grep hits are unrelated Electron packaging properties), and no
script parses ADR headers, so the ADR-0012 amendment line cannot break a consumer.

The `ci.yml` change was the highest-risk edit, so its exit semantics were simulated
directly under `bash -e` across all four pass/fail combinations: both suites always run,
and the step exits non-zero if and only if at least one suite failed. Overall pass/fail
behaviour is unchanged from before the patch; only failure *visibility* improves.

## Risks and follow-ups

- **`GEMINI.md` (F2) needs a human decision and is deliberately unpatched.** `AGENTS.md`
  names it in the required read order (line 161), in the authority rules (lines 170-171),
  and in a validation command that greps it (line 203). `git log --all` shows the file
  has never existed, so that command can only fail. Both available fixes are governance
  choices an agent should not make alone: creating `GEMINI.md` means authoring
  Gemini-specific guidance and a `GILES:GEMINI-DOCS-GOVERNANCE` marker, and removing the
  four references means deleting repo-specific constraints — which this repo's own
  documentation-patch rules forbid without an explicit request. Previously noted in
  `docs/logs/session/2026-09-08-recent-audit-repairs.md` and still open.
- **`.giles/` metadata (F4) is stale and not agent-owned.** `compliance_status.yaml` is
  from 2026-07-07 with `execution_status: crashed` and 0/9 actions complete;
  `patch_todo.yaml` holds 13 items predating both the Tagteam and `gosling-server`
  removals. `AGENTS.md` designates these advisory mirrors (`canonical: false`) and
  forbids converting them into compliance claims, so refreshing them requires a Giles
  scan, not an edit here. Recorded so a reader does not mistake a crashed July run for
  current posture.
- **The session-log allowlist convention is undocumented.** `AGENTS.md` mandates a log
  under `docs/logs/session/` but does not mention that `.gitignore` ignores that
  directory and re-admits each file by name. An agent that follows `AGENTS.md` literally
  produces an untracked log and reports evidence that never lands. Worth adding one line
  to the `AGENTS.md` logging section; not done here because editing the operating
  contract is a governance change the user should approve.

- **`ARC-` id namespace collision.** The registry uses bare `ARC-NNN` for repo
  invariants while the audit skill pack uses `ARC-001..025` as seam mnemonics; both
  appear across `docs/cloud/`. With `ARC-001`/`ARC-002` restored the registry is
  self-consistent, but a bare `ARC-010` in prose remains ambiguous. Namespacing the
  registry ids would resolve it and is a larger change than this patch set.

## Addendum — two follow-ups closed the same day

The user authorised closing the two `AGENTS.md`-related follow-ups recorded above.
Appended rather than edited into the sections above, per `docs/logs/README.md`
("do not rewrite historical results to match current state").

Investigating them changed the approach to both. `AGENTS.md:151-217` is a
Giles-owned template block (`<!-- GILES:DOCS-GOVERNANCE:START -->` ... `:END`)
containing the required read order, authority rules, documentation-patch rules,
validation expectations, **and** the logging section. The repository's own rules end
at line 149. Both follow-ups therefore pointed at fleet-managed text, and the
`GEMINI.md` references are template boilerplate assuming every fleet repo ships a
Gemini adapter — this one ships `CLAUDE.md` instead.

- **Session-log allowlist — documented in `docs/logs/README.md`, not `AGENTS.md`.**
  The original follow-up proposed adding a line to the `AGENTS.md` logging section.
  That section sits inside the Giles block, where repo-local `.gitignore` mechanics
  do not belong and may not survive a rescan. `docs/logs/README.md` is repo-owned,
  tracked, and already the canonical home for session-log conventions, so the
  allowlist rule went there.
- **`GEMINI.md` created as an adapter.** Removing the references would have edited
  the Giles block and deleted declared constraints, which this repo's
  documentation-patch rules forbid without an explicit request. Creating the file
  touches nothing fleet-managed and makes `AGENTS.md`'s own mandated validation
  command satisfiable instead of permanently failing. It carries the
  `GILES:GEMINI-DOCS-GOVERNANCE` marker pair, states the deference relationship
  `AGENTS.md:171` already declares, and records honestly that no Gemini-specific
  deviations currently apply. It is deliberately not indexed in `docs/INDEX.md`,
  matching the existing treatment of `CLAUDE.md` — that index lists the canonical
  contract, not adapters.

Both mandated greps now pass: `GILES:DOCS-GOVERNANCE:START` in `AGENTS.md` (2) and
`GILES:GEMINI-DOCS-GOVERNANCE:START` in `GEMINI.md` (1). `AGENTS.md` itself remains
unmodified.

Still open and not agent-owned: the root cause is that the fleet template names
`GEMINI.md` where this repo has `CLAUDE.md`. Pointing the template at the adapter a
repo actually ships would fix this fleet-wide rather than per-repo, and needs a Giles
change rather than an edit here.
