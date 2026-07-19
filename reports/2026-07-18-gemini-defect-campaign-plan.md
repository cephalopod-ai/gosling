# Gemini defect-repair campaign plan — 2026-07-18

**Status:** campaign complete; four confirmed localized defects repaired, seven
stale/incorrect claims closed with evidence, and one architectural residual routed.  
**Skill:** `repair-defect-campaign` from the private `agent-skills` catalog.  
**Findings source:** the user-supplied consolidated Gemini report, current source,
and the repository's existing 2026-07-18 audit evidence.  
**Repository state:** `main` at `5f5a3acde8a671199e5d384df2fddd9a8aa35066`,
identical to `origin/main`, with a clean worktree before this campaign.  
**Git policy:** no commit or remote mutation was requested for this repair pass.

## Gate 0 — orientation and baseline

- Read `AGENTS.md`, `CLAUDE.md`, `CONTRIBUTING.md`, `SECURITY.md`,
  `docs/TODO.md`, and `documentation/GOOSE_COMPATIBILITY.md`.
- Preserved Gosling naming, paths, configuration, and desktop type-boundary rules.
- Baseline targeted Rust tests passed for concurrent extension-state merges,
  lock-upgrade contention, concurrent session creation, working-directory scope,
  provider HTTP classification, and the Rust/TypeScript provider-type drift guard.
- The desktop suite passed at baseline: 73 files and 516 tests.

## Gate 1 — frozen inventory

| ID | Gemini candidate | Current disposition | Evidence |
| --- | --- | --- | --- |
| GEM-001 | Windows `open-in-chrome` command injection | Confirmed | Renderer IPC reaches `cmd.exe /c start` with an attacker-controlled HTTP(S) URL. URL-scheme validation does not make shell metacharacters inert. |
| GEM-002 | Persisted renderer directory-grant bypass | Verified not a defect | Persisted roots are main-process-only (`webContentsId == 0`); renderer grants are transient and scoped to the selecting `webContents`. Tests cover restart and cross-renderer denial. |
| GEM-003 | Working-directory scope key bypass | Confirmed | Only a fixed top-level key list is inspected. Common aliases and nested argument objects are ignored. |
| GEM-004 | Delegates skip security/egress inspection in Auto mode | Verified not a defect | Delegates use Auto because approvals cannot be forwarded, but Auto still runs all inspectors. Security, egress, and working-directory inspectors opt out of Auto downgrades; a regression test proves denying inspectors run in Auto. |
| GEM-005 | ACP instance `data_dir` bypassed by global paths | Confirmed architectural residual; routed | This is the previously recorded `AUD-GOS-011` / `DEF-002`. Session/workspace storage honors injected roots, while process-global configuration/path consumers cannot provide concurrent per-agent isolation. A correct repair needs scoped dependency injection across a >2,000-line server lifecycle and is routed to source modularization. |
| GEM-006 | `BEGIN IMMEDIATE` deadlocks extension-state merges | Verified not a defect | SQLite serializes writers under WAL/busy-timeout. Existing concurrent-writer and lock-upgrade tests pass; the latter proves deferred transactions fail where `BEGIN IMMEDIATE` succeeds. |
| GEM-007 | Session-directory creation race | Verified not a defect | `create_dir_all` is idempotent, failures surface on pool use, and two independent concurrent-creation regression tests pass. |
| GEM-008 | Eight shell-output slots collide | Confirmed | The ninth truncated result reuses the first file even sequentially; concurrent calls can overwrite referenced output. |
| GEM-009 | Deleting a session orphans SQLite connections | Verified not a defect | Sessions share one `SessionStorage` pool. Deletion owns no per-session connection; closing the shared pool would break unrelated sessions. Transactions commit before return. |
| GEM-010 | Provider adapters lack error classification | Confirmed after adversarial re-review | The cited path is wrong and OpenAI itself uses the shared classifier, but several other model-list adapters collapse typed transport errors into `RequestFailed` or parse error bodies without first classifying HTTP status. `ProviderError::from(reqwest::Error)` also leaves status errors untyped. |
| GEM-011 | Large session imports have no size cap | Verified stale | Main-process and Rust import paths both enforce a 16 MiB preflight and bounded read; the renderer also rejects oversized selections. |
| GEM-012 | Rust/TypeScript SDK enum drift | Verified stale | Generated ACP types have a checked generation target and CI job; the hand-mirrored desktop provider enum has an explicit Rust drift test, which passes. |

## Gate 2 — repair groups

### Group 1 — desktop external-URL IPC

- Defect: GEM-001.
- Touch set: `ui/desktop/src/main.ts`; desktop validation.
- Repair: retain the IPC contract but route the validated URL through Electron's
  non-shell `shell.openExternal` boundary. Remove the platform command launch.
- Adversarial cases: shell metacharacters, non-web protocols, malformed values,
  and asynchronous open failures.
- Large-file decision: `main.ts` exceeds 2,000 lines, so this is a smallest-possible
  local replacement; structural IPC extraction is outside this stage.

### Group 2 — structured path discovery

- Defect: GEM-003.
- Touch set: `crates/gosling/src/permission/working_dir_scope_inspector.rs`.
- Repair: recursively inspect nested argument values, recognize a bounded set of
  path-semantic aliases, retain explicit path-shape detection, and preserve the
  specialized shell parser.
- Adversarial cases: nested objects/arrays, `target`/`output`/`directory`/`cwd`,
  parent traversal, absolute paths under unknown keys, and non-path content.
- Large-file decision: 865 lines; patch in place.

### Group 3 — durable shell-output references

- Defect: GEM-008.
- Touch set: `crates/gosling/src/agents/platform_extensions/developer/shell.rs`.
- Repair: use a monotonic per-tool call identifier instead of wrapping eight
  reusable slots. Temporary-directory lifetime still bounds cleanup.
- Adversarial cases: more than eight calls, concurrent allocation, stdout/stderr
  separation, and preservation of prior truncated content.
- Large-file decision: 1,232 lines, but the change is local and does not heavily
  edit the module; no modularization.

### Group 4 — provider error classification

- Defect: GEM-010.
- Touch set: the shared `ProviderError` reqwest conversion and provider model-list
  adapters that bypass or erase the shared HTTP classifier.
- Repair: classify status-bearing reqwest errors centrally and preserve typed
  transport/status errors through model discovery. Run non-successful responses
  through the existing body-aware HTTP classifier before JSON success parsing.
- Adversarial cases: 401/403 authentication, 429 rate limits, 5xx server failures,
  network failures, model-endpoint 404 fallback behavior, and invalid success JSON.
- Large-file decision: changes remain narrow in each adapter; no heavily edited
  >2,000-line module is involved.

### Routed residual

GEM-005 remains routed to `repair-source-modularization`, consistent with the prior
`DEF-002` campaign decision. A partial global override would create cross-agent races
and falsely claim isolation while provider/configuration state still leaks.

## Closeout requirements

Completed as recorded in
[`2026-07-18-gemini-defect-campaign-session-log.md`](2026-07-18-gemini-defect-campaign-session-log.md).
The routed residual is not claimed as repaired.
