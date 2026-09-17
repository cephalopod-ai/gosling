# Evidence, instruction admission, and execution authority: integration handoff

Date: 2026-09-16. Source checkout `main` at `1a166751c` plus uncommitted changes
from this work. Status: implemented in source and validated locally. Not
committed, released, or installed.

Decision record: [ADR-0023](../../adr/0023-evidence-instruction-authority-boundary.md).
Invariant: ARC-014 in `.architecture/invariants.yaml`.

## 1. Contract version and source

- **Upstream contract: `evidence-authority-boundary` version `1.0`**, agent-skills
  commit `759f43a` (`000_common/engagement-base/evidence_authority_boundary.md`,
  `docs/evidence-authority-boundary-integration.md`, fixtures
  `_registry/fixtures/evidence-authority-boundary-v1.json`). It landed while this
  work was in progress and was read afterwards, read-only. Gosling's design was
  not derived from it, so alignment was checked after the fact.
- **Aligned runtime semantics** (Gosling integration obligations, section 5 of the
  upstream page): discovery and loading are candidates, and admission binds a
  revision digest (section 2 here); summaries keep the untrusted marker; recalled
  approvals are not current; execution is authorized per action at the host
  gate; failures gate the action with a named reason. The admitted-procedure
  ceiling follows the published authority ladder.
- **Not implemented:** Gosling does not emit or validate the machine `boundary`
  companion document (`_registry/schema/boundary.schema.json`) and does not
  declare the `evidence_authority_boundary_enforcement` consumer capability.
  It does not claim to pass the upstream validator. The upstream obligation to
  place retrieved content in a labeled evidence channel is met only partially:
  Gosling already wraps imported untrusted history and Session History results
  as untrusted evidence, but ordinary tool results are not relabeled.
- `scenarios.json` maps upstream `EAB-*` ids to the Gosling runtime tests that
  exercise the analogous host behavior (`eabEquivalents`).
- The catalog wire format is unchanged: skill-catalog schema v1. The only new
  interpretation is of the optional `contentHash` when it starts with `sha256:`,
  documented in the schema description and the catalog guide.
- The adapter boundary is `crates/gosling/src/skills/admission.rs`
  (`map_authority_label`, `verify_declared_content_hash`,
  `SkillAdmission::for_skill`).

## 2. Host-owned types and entrypoints

| Item | Location | Notes |
| --- | --- | --- |
| `SkillSourceKind`, `SkillOrigin`, `CatalogDescriptorFacts` | `skills/admission.rs` | Set by the discovery adapter (`discover_skills_with_origin`), never by frontmatter |
| `SkillAdmission` | `skills/admission.rs` | Fields private; built only by `for_skill` / `for_supporting_file` inside the crate |
| `AuthorityCeiling` | `skills/admission.rs` | `unrestricted` < `non_mutating` < `human_approval_required`; unparseable stored value = most restrictive |
| `ActiveSkillCeiling`, `SkillCeilingDenied` | `skills/admission.rs` | Error code `skill_authority_ceiling` |
| Admission entrypoints | `SkillsClient::call_tool` (`load_skill`), `Agent::handle_skill_command` (slash command) | Persist before returning text |
| Storage | `session_manager/skill_admission_storage.rs` | `record_skill_admission`, `inherit_skill_admissions`, `active_skill_ceiling`, `enforce_skill_scope_in_tx` |
| Authorization entrypoints | `SkillAuthorityInspector` (routing to approval); `authorize_and_begin_tool_operation(..., SkillScopeGate)` (fenced gate); `ExtensionManager::dispatch_tool_call` (nested gate) | |
| Verified read-only identities | `interaction_policy::is_verified_non_mutating` | Platform `skills` load/find/refresh, `session_history` search/read, `developer` tree/read_image |

Metadata ownership: the discovery adapter owns source kind and catalog facts;
the skills host owns loaded-byte digests; the session store owns scope; the user
(through the client confirmation for a request id) owns per-call approvals.
Nothing the model, a tool, a memory, or a document returns is read into these.

## 3. Label mapping

| Label | Ceiling | Reason |
| --- | --- | --- |
| absent | unrestricted | legacy descriptor; not a grant |
| `read_only`, `plan_only` (also `-` separators) | non_mutating | may_modify false |
| `test_only`, `low_risk_repair`, `governed_repair` | unrestricted | Gosling cannot verify these scopes for shell/editor tools; existing permissions apply |
| `destructive_admin` | human_approval_required | ladder requires explicit human approval |
| anything else, including case variants | non_mutating (`unrecognized`) | unknown never widens |

`requiresHumanApprovalFor` is recorded and displayed, not enforced.

## 4. Execution coverage matrix

Status key: **Enforced** = Gosling denies or requires per-call user approval
before the first side effect, with a test. **Existing** = pre-existing gate this
work relies on. **External** = outside Gosling's control. **Unsupported** =
not provided.

| Path | Evidence cannot grant | Skill ceiling | First side-effect gate | Evidence |
| --- | --- | --- | --- | --- |
| Model-native hosted MCP / built-in / platform tools | Existing (grants only from permission store) | Enforced: inspector prompt + begin-transaction gate | ledger begin, before hooks and dispatch | EIA-REF-001/002, EIA-SKILL-CEIL-001, EIA-RACE-001 |
| User-confirmed call | n/a | Allowed for that exact request id only | ledger binds id to checkpointed payload | EIA-SKILL-CEIL-001, EIA-RACE-001 |
| Frontend tool emission | Existing | Enforced: denied at begin, nothing emitted | ledger begin | EIA-FRONTEND-001 |
| App-direct (`dispatch_app_tool_call`, Recall Brief) | Existing (no prompt path) | Enforced: inspector makes it "requires approval" → refused; begin gate as backstop | inspection, then ledger begin | EIA-APPDIRECT-001 |
| Agent-direct (`dispatch_tool_call`) | Existing | Enforced at ledger begin | ledger begin | EIA-RACE-001 (same gate) |
| Superseded turn after lease takeover | Existing lease fence (heartbeat) | Enforced: unverified, unapproved conversation operations refused | ledger begin | EIA-RACE-001 |
| Code-mode nested calls | Existing gap: nested calls skip inspection | Enforced: unverified nested calls refused under a ceiling | `ExtensionManager::dispatch_tool_call` | EIA-NESTED-001 |
| PreToolUse / command hooks | Hooks can only deny; no argument rewrite | Hooks run after the gate | ledger begin precedes hooks | ARC-011 tests (existing) |
| Summon `delegate`, orchestrator `start_agent` | Existing | Enforced: child inherits active restrictive admissions; subagent cannot escalate | child ledger begin / subagent redirect | EIA-DELEG-001 |
| Manual compaction | Existing; now keeps imported-untrusted on summaries | Unaffected (separate table) | n/a | EIA-COMPACT-001/002 |
| Auto compaction | Existing | Unaffected (turn lease persists through the turn) | n/a | Not separately tested in this change |
| Provider/model handoff | Existing (checkpoint never parsed into grants; ADR-0019 atomicity) | Admissions not transferred | n/a | existing `acp_handoff_session_test` |
| Restart | Existing | Turn ends; delegated-session rows persist | n/a | EIA-RESTART-001 |
| Copy / fork / export / import | Existing plan staleness; import now drops Deep Research paths | Not transferred | n/a | EIA-HANDOFF-002 |
| Session History recovery | Existing redaction and "untrusted evidence" notice | Tools are verified read-only | n/a | existing session history tests |
| Recall Brief / Santa | Existing (ADR-0022) | Unchanged | app-direct path | `tests/recall_brief.rs` 13/13 |
| Provider-owned runtimes (claude_code, gemini_cli, cursor_agent, antigravity, ACP provider) | External | Unsupported: restricted slash-command entry refused; inner tools not constrained | external process | EIA-PROVIDER-001 |
| Remote MCP / external engine internals | External | Unsupported beyond the outer call | external | none |
| Shell/code internals after approval | External to the approval | Unsupported | external | none |

## 5. Persistence, compaction, handoff, compatibility

- Migration 37 adds `skill_admissions`. Old databases migrate forward; no data
  is rewritten. No SDK/ACP types changed, so no generated client changes.
- Scope: a row is active while its `turn_lease_id` equals the session's current
  `session_turn_leases.lease_id`, or while it is a `delegated_session` row.
  Lease release ends the turn scope. After a lease takeover by another process,
  the superseded turn's new conversation operations that would need the ceiling
  check are refused in the begin transaction rather than evaluated against the
  new turn's (empty) scope; its heartbeat then cancels it.
- Compaction and handoff do not touch the table. Restart ends turn scope.
- Copy, fork, export, import, and new-session handoff do not copy the table.
  Session deletion removes its rows.
- Catalogs without labels or hashes behave as before, except that a local skill
  shadowing a configured catalog id is no longer loadable under that id.
- Import now removes `deep_research.v1` extension state (file-supplied host
  paths); other kept extension state is unchanged.

## 6. Shared fixtures

`crates/gosling/tests/fixtures/authority_boundary/`:

- `authority_label_mapping.json`: `EIA-LABEL-001`..`011`, consumed by
  `skills::admission::tests::authority_label_mapping_matches_portable_fixture`.
- `scenarios.json`: scenario ids `EIA-*`, the invariant, expected outcomes, the
  test that proves each, and explicit not-covered paths. No private catalog
  content, skill names, or paths are included; all skills are synthetic `eia-*`.

## 7. Expected adapter behavior elsewhere

**agent-skills** (contract owner): publish a versioned contract that states the
label semantics above (or replaces them), the `sha256:` content-hash form, and
the rule that discovery, routing scores, and descriptions never grant authority.
Reuse the `EIA-*` ids for portable fixtures. Do not describe
`requiresHumanApprovalFor` as enforced by Gosling.

**Cuttlefish** (orchestration): when it delegates into Gosling, pass only the task
and an explicit skill selection; do not pass serialized grants or approval
claims, which Gosling ignores. Treat Gosling results as evidence and proposals.
Where Cuttlefish controls execution itself, it needs its own ceiling
enforcement; Gosling's covers only Gosling's dispatch.

**Muninn** (evidence): memory results remain untrusted evidence. A Muninn record
that says an action was approved does not change Gosling authority (EIA-REF-*).
Gosling writes no memory from these paths.

## 8. What Gosling proves, what depends on others, residual risk

Proved by deterministic tests here (mock providers, inert fixtures, real
dispatch and persistence): the enforced rows in section 4.

Depends on external runtimes: anything a provider-owned runtime, remote MCP
server, or external engine does inside an authorized call.

Admission trust policy: Gosling admits a skill the model selects when the
discovery policy found it, and that policy includes project directories
(`.agents/skills`, `.gosling/skills`, `.claude/skills`) of the working directory by
default. The upstream contract calls a model's `load_skill` a candidate until the
host admits it; in Gosling the host admits it immediately under that discovery
policy. Admission can only narrow authority, so a hostile repository skill cannot
widen permissions, but it can steer the model within them. A policy that
requires operator selection before admitting project skills is not implemented.

Residual semantic risk: within broad permissions (Auto mode, saved grants, an
unrestricted or modifying-authority skill) a model can still be persuaded by
content to choose a harmful permitted action. Provenance labels and delimiters
are presentation aids, not a security boundary. These tests show host behavior
with scripted models; they are not evidence about any live model.

## 9. Validation (this change)

Local session log: `docs/logs/session/2026-09-16-evidence-instruction-authority.md`
(gitignored). Final run after all edits:

- `cargo fmt --all -- --check`: clean. `git diff --check`: clean.
- `cargo clippy --all-targets -- -D warnings`: clean.
- `cargo check -p gosling-cli --tests`: clean.
- `cargo test -p gosling --lib`: 2017 passed, 3 failed, 4 ignored. The 3 failures
  are `agents::prompt_manager` snapshot tests that read this machine's customized
  prompt; they failed before the change too (baseline 1985 passed, 5 failed; the
  two extra baseline failures in `reply_parts` did not recur).
- Every `crates/gosling/tests/*.rs` binary except `agent`: all passed (includes
  `authority_boundary_restart` 2/2, `recall_brief` 13/13, `acp_server_test` 47,
  `permission_audit_regressions` 27, handoff/fork/compaction suites).
- `--test agent` was run once earlier: 24 passed, 1 failed
  (`test_batch_summarization_preserves_all_summaries`). That test uses the global
  session store and writes `GOSLING_TOOL_CALL_CUTOFF: 2` into the operator's real
  config, so it was not rerun and its failure is not attributed to this change.
- Not run: Desktop typecheck/tests (no Desktop source changed), live providers,
  ACP schema regeneration (no SDK types changed).
