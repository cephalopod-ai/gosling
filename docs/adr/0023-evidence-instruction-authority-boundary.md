# ADR-0023: Evidence, instruction admission, and execution authority

Date: 2026-09-16
Status: Implemented in source; locally validated; not released
Related: ADR-0019, ADR-0020, ADR-0021, ADR-0022, ARC-011, ARC-014

## Context

An agent reads documents, tool results, memories, summaries, and skills. Any of
them can contain text that looks like an instruction, an approval, or a grant.
If a transformation (summary, compaction, handoff, delegation) drops the
content's status, a later path can mistake it for current authorization.

Gosling already kept most authority out of content. Tool grants live in the
global permission store, the authorization mode and folder grants in session
columns, and plan approval in the plan lifecycle. Nothing parses summaries,
checkpoints, or tool output into those records. Two gaps remained:

- Skills were discovered, loaded, and rendered without a host-side record of
  which revision was admitted or from where. A catalog's declared `execution`
  authority and `contentHash` were carried as properties but never enforced or
  verified. A repository `SKILL.md` could take over a configured catalog id,
  and with it shed the catalog's declared restriction.
- Some continuity paths let imported or derived content keep or lose status
  silently. Import kept file-supplied Deep Research host paths that turn
  completion copies files between. Compaction dropped the imported-untrusted
  marker from summaries of imported history.

The design used the published agent-skills authority ladder
(`_registry/authority-levels.yaml` version 1). The agent-skills
`evidence-authority-boundary` contract version 1.0 landed while this work was in
progress; its host obligations were checked afterwards and are consistent with
this decision. Gosling does not emit that contract's machine companion document.

## Decision

**Discovery is not admission.** Search, routing, descriptions, and a file named
`SKILL.md` grant nothing. Admission happens only when the host loads a skill
for use, through `load_skill` (model) or a skill slash command (user).

**Admission is host-owned and revision-bound.** `SkillAdmission` can be
constructed only in the skills module. It records the source kind the discovery
adapter established (configured catalog, project, user, plugin, built-in), the
catalog id and declared version, SHA-256 of the exact `SKILL.md` bytes that were
rendered, the declared-hash status, the declared authority label and how it
mapped, and the declared approval terms. Frontmatter cannot supply catalog
facts. A catalog `contentHash` of the form `sha256:<64 lowercase hex>` must
match the loaded bytes or admission is refused; other forms are recorded as
unverifiable rather than guessed. A local skill that shadows a configured
catalog id is refused as ambiguous. `SKILL.md` resolving outside its directory
is refused. Supporting files are recorded as reference reads and add no ceiling.

**Labels map explicitly.** `read_only` and `plan_only` map to a non-mutating
ceiling; `destructive_admin` to a human-approval ceiling. `test_only`,
`low_risk_repair`, and `governed_repair` add no host restriction, because Gosling
cannot verify "tests only" or "low risk" for shell or editor tools; existing
permissions apply and the admission section says so. An absent label is legacy
metadata and neither restricts nor grants. An unrecognized label maps to the
non-mutating ceiling. Frontmatter labels on non-catalog skills are honored
because they can only narrow.

**Admissions are scoped to a host turn.** The durable `skill_admissions` table
(migration 37) keys each admission to the session turn lease current when it was
recorded. The ceiling therefore persists through the turn (including loading
other skills, refreshes, and mid-turn compaction) and ends when the lease is
released. An unrelated later turn is not restricted. Admission is recorded
before the admitted text is returned, so the restriction exists whenever the
model can read the guidance. Outside a turn nothing is recorded and the rendered
section says no restriction applies. A delegate (summon `delegate`, orchestrator
`start_agent`) receives a copy of the parent's active restrictive admissions for
the delegated session's lifetime. Admissions are never exported, imported,
copied, forked, or handed off.

**Admission never raises permissions.** A ceiling only removes the ability to run
unverified calls without explicit per-call approval. Enforcement reuses the
existing layers:

1. A mandatory `skill_authority` inspector routes each unverified call to an
   approval prompt. Auto mode cannot downgrade it, a saved tool-wide grant cannot
   satisfy it, the prompt refuses "Always allow", and a subagent turns it into a
   denial.
2. The tool-operation begin transaction re-reads the active ceiling for every new
   operation. A call that is neither a verified read-only host tool nor approved
   by the user for that exact request id is denied with
   `skill_authority_ceiling`. This is the linearization point: an admission
   committed before it applies, one committed after it applies only to later
   operations. Completed replays and in-doubt results are unaffected. If another
   process has taken over the session's turn lease, the superseded turn's
   conversation operations are refused here instead of being checked against
   the new turn's scope.
3. Code-mode nested dispatch, which has no approval route, refuses unverified
   calls under a ceiling.

The verified read-only set is identity-based and in-process only: the skills
tools, Session History search/read, and developer `tree`/`read_image`. Shell,
file writes, and code execution are excluded because their effects cannot be
verified from arguments. An MCP server exposing a same-named tool does not
qualify.

**Arguments cannot imitate host sections.** Invocation argument lines that start
with `#` are escaped so they cannot render as `# Loaded Skill` or `## Host
Admission`. This is presentation hygiene; enforcement reads the durable record,
not rendered text.

**Provider-owned runtimes are refused, not trusted.** A restricted skill selected
by slash command is refused when the provider executes tools outside Gosling.
Such providers are not offered Gosling tools, so `load_skill` is unavailable to
them. Delegates on those providers already run in Chat mode.

**Continuity keeps status.** Import removes Deep Research state as well as
enabled extensions and prompt extras. A compaction summary whose inputs include
imported untrusted messages keeps `imported_untrusted`.

## Safety boundary and residual risk

The enforced claim is narrow: during a turn with an admitted restrictive skill,
no unverified tool call begins through Gosling's model-native, frontend,
app-direct, agent-direct, or code-mode nested paths without an explicit user
approval of that exact request; and no content, summary, checkpoint, import, or
tool result writes a grant, mode, plan decision, or admission.

It does not cover:

- tools run by provider-owned runtimes, remote MCP servers, or external engines
  after the outer call is authorized;
- what an approved shell or code call does internally;
- semantic prompt injection within broad permissions, for example Auto mode with
  no restrictive skill admitted;
- a local administrator who can edit both `sessions.db` and the permission store.
  A stored ceiling that fails to parse is treated as most restrictive, but a
  digest in the same database is not tamper evidence.

Catalog `requiresHumanApprovalFor` terms are recorded and shown, not enforced:
Gosling has no verified mapping from terms such as `code_changes` to tool calls.

## Consequences

Read-only and plan-only catalog skills now prompt for shell and file edits even
in Auto mode, and their delegates cannot run those calls. That is the intended
cost of an enforceable ceiling. Operators who need autonomous edits should use a
skill whose declared authority permits modification. The schema gains one
table; old databases migrate forward and legacy catalogs without labels or
hashes keep their prior behavior.
