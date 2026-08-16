# Gosling Audit — Architecture, Invariants, Negative Space

**Date:** 2026-08-15  
**Target:** `/Users/eric/Work/vscode/forked/gosling`  
**HEAD:** `073d19428509ea6eb317924b1856a1fe7e9002c8` (`refine XAI auth settings and OAuth handling`)  
**Authority:** `read_only` — source was not modified. This file is the assigned report.  
**Lenses:** `audit-architecture-seam` (ARC-001..025), `audit-invariant-sync` (INV-001..015), `audit-negative-space` (NEG-001..015)  
**Method:** `audit_method.md` v3.2 + evidence discipline + confidence calibration. Historical `docs/cloud/audit-*.md` reports were seeds only; every claim below was re-read at HEAD.

The supplied prompt is treated as a draft. The intended mission is preserved: audit god objects, provider adapters, ACP/UI seams, permission lists vs UI/CLI, Goose catalogs, subagent Auto override, and untrusted-repo composition. Review was expanded to adjacent seams implied by those surfaces (Config singleton, generated-vs-hand types, scanner predicates, project skills vs plugin trust).

---

## Executive Verdict

No new Critical defect was confirmed from static review. The load-bearing safety hole that remains is **compositional**, not a missing single guard: subagents and orchestrator-managed agents still start in `GoslingMode::Auto`, so approving *delegation* still grants unsupervised tool execution unless the operator previously pinned a per-tool `NeverAllow`/`AskBefore`. That interacts with **untrusted-repo inputs that are not called "plugins"** — project skills and `AGENTS.md` / `.goslinghints` load without the `trust_project` gate that now correctly blocks repo-shipped plugins.

Several 2026-07 findings have been repaired at HEAD and are **not** re-raised: MCP `read_only_hint` can no longer auto-approve; the command-injection scanner is no longer name-`shell`-only; Auto honors explicit user permissions and fail-closes unanswerable subagent approvals; imported history is marked `imported_untrusted`; project plugins stay untrusted until `gosling plugin trust`.

The structural picture is worse than the July seam report on size: `session_manager.rs` is now **8740** lines (Session + storage + import + artifacts + tests in one file), `agent.rs` is **5332**, `extension_manager.rs` is **4096+**, `config/base.rs` is **3199+**. Those are confirmed god objects. Provider adapters remain a split-brain extraction (`Provider` in `gosling-providers`, `ProviderDef` + most impls in core). Desktop still hand-copies ACP enums with no parity test.

**Merge/use:** do not pause local single-user use. Pause any claim that "Approve/SmartApprove is the human-in-the-loop boundary" for delegated work or untrusted checkouts. Patching is recommended now for NEG-GSL-001 and NEG-GSL-002; architecture findings are structural debt, not a ship-stop.

---

## Scope

- Repository / branch / commit: gosling `main` @ `073d19428509ea6eb317924b1856a1fe7e9002c8`
- Prompt / orientation: `docs/cloud/2026-08-15-orientation.md`; assigned focus seams listed above
- Skills: ARC, INV, NEG (this combined report)
- Files/directories inspected (deep-read or targeted): `crates/gosling/src/{lib,agents,session,permission,security,config,providers,plugins,hooks,skills,hints,acp,sources}.rs` and named submodules; `crates/gosling-providers/src/{base,gosling_mode,permission,thinking,conversation/message}.rs`; `crates/gosling-sdk-types/src/custom_requests.rs`; `crates/gosling-cli/src/{cli,commands/configure}.rs`; `ui/desktop/src/{types,acp,components/settings}`; `documentation/{GOOSE_COMPATIBILITY.md,src/utils/goose-compat.ts,scripts/goose-compat.js}`; `docs/{architecture.md,INDEX.md,adr/0002,0004}`; `SECURITY.md`; `AGENTS.md`
- Commands/tests run: none (read_only; this agent had no shell). Oracle-integrity fresh-process check **not performed**.
- Effort budget: ~90 tool calls / ~70 files sampled. Deep-walk prioritized god objects, permission/subagent, Goose, skills/plugins/hooks, provider traits, ACP/UI types. Remainder marked Not Reviewed.
- Constraints: no source mutation; no target execution; historical findings re-verified, not copied.

---

## Draft Prompt Assessment

- **Intended mission:** combined architecture + invariant-sync + negative-space audit of gosling's orchestration core and its trust seams.
- **Under-specified:** whether Goose catalogs are a product path or docs-only; whether "permission lists" means stored `PermissionLevel` or confirmation `Permission`.
- **Over-narrow risk:** treating plugin trust as the whole untrusted-repo story (skills + hints bypass it).
- **Added angles:** producer/consumer of ACP enums; sibling `is_shell_tool*` predicates; sanctioned Goose dual-stack vs undocumented dual provider traits.
- **Assumptions challenged:** "Approve mode gates every tool"; "untrusted repo is handled by plugin trust"; "providers crate is just adapters."

---

## Surface Inventory

| Surface | Actor | Input/Trigger | State/Output | Boundary | Reviewed |
|---|---|---|---|---|---|
| `Agent` loop | user / ACP / subagent | messages, tool calls, mode | tool exec, session writes | permission inspectors | Yes |
| `SessionManager` / `SessionStorage` | Agent, ACP, CLI | CRUD, import/export, artifacts | `sessions.db` | store owns schema | Yes (API + import; not every SQL path) |
| `ExtensionManager` | Agent | MCP/stdio/docker/platform ext | tools, resources, dispatch | MCP host | Yes (lifecycle + dispatch; not every transport) |
| Provider adapters | Agent | model stream | tokens / delegated CLI | `Provider` trait | Yes (trait + ACP/CLI impls sampled) |
| ACP HTTP / desktop | Electron renderer | JSON-RPC custom methods | UI state | ADR-0004 / generated SDK | Yes (types + permission mapping) |
| Permission YAML + UI/CLI | operator | tool name + level | `permission.yaml` | `PermissionManager` | Yes |
| Goose catalogs | docs site visitor; `gosling mcp install --from-goose` | remote JSON / Goose yaml | normalized catalog / local ext | compat adapter | Yes (docs + CLI; desktop does not fetch) |
| Subagent / orchestrator | parent agent | `summon` / orchestrator tools | child `SessionType::SubAgent` + Auto | comment-deferred confirmation forward | Yes |
| Untrusted repo | checkout + session cwd | plugins, skills, hooks, `AGENTS.md` | MCP/hooks/skills/hints | `trust_project` for plugins only | Yes |
| Security scanner | tool inspector | tool name + args | allow / ask / deny | `SECURITY_PROMPT_ENABLED` default true | Yes |

---

## Seam Inventory (ARC)

| Module | Layer | Responsibility | Owns | Depends On | Boundary Contract | Abstraction Fidelity | Coupling Risk | Generated/Vendored? |
|---|---|---|---|---|---|---|---|---|
| `gosling-sdk-types` | domain contract | ACP DTOs, workspace, permission levels | wire types | serde/schemars | generated schema | high | Low | hand + generator input |
| `gosling-providers` | **inferred mixed** — named adapters, actually domain+adapters | `Provider`, `Message`, `GoslingMode` | conversation model + 4 HTTP impls | sdk-types | crate leaf | mixed | **High** | hand |
| `gosling` core | application + domain + adapters | agent, session, config, 20+ providers, ACP host | almost everything | providers, sdk-types | 49 `pub mod`s, no facade | low | **High** | hand |
| `agents/agent.rs` (5332) | application orchestrator | turn loop, mode, tools, hooks, steering | `Agent` 18+ fields | 15+ crate modules | struct-internal | orchestrator + hosted state | **High** | hand |
| `session/session_manager.rs` (8740) | adapter + persistence | Session DTO, storage, import, artifacts, naming, tests | `sessions.db` | sqlx, providers (naming) | `SessionManager` facade over in-file `SessionStorage` | file-level god | **High** | hand |
| `agents/extension_manager.rs` (4096+) | infrastructure | MCP lifecycle, docker, OAuth, tool cache, dispatch, resources | live extensions | rmcp, Config | methods on one struct | mixed | **High** | hand |
| `config/base.rs` (3199+) | infrastructure | YAML + env + keyring + caches | `config.yaml` / secrets | fs, keyring | `Config::global()` | mechanism+store | **High** | hand |
| `acp/*` | interface | ACP server + `AcpProvider` subprocess | sessions via Agent | agents one-way | wraps Agent; flattens CLI agents | medium | Medium/High | hand |
| `ui/desktop` | UI | Electron + React | presentation | ACP HTTP, local `src/types` | AGENTS.md forbids generated OpenAPI | hand-copied enums | Medium | generated SDK unused by desktop |
| `documentation/src/utils/goose-compat.ts` + `scripts/goose-compat.js` | docs adapter | Goose → gosling catalog | none (docs) | remote Goose JSON | documented fallback | two live copies | Medium | hand |
| `vendor/v8` | vendored | excluded from god-object ranking | — | — | — | — | — | vendored |

**Declared architecture** exists (`docs/architecture.md`, ADR-0001..0014). Workspace/credential layering is declared; the provider/agent/session core is only partially covered. Boundaries used below that are not in an ADR are marked **inferred**.

---

## Boundary Map

| Surface | Intended Boundary | Enforced At | Status |
|---|---|---|---|
| Workspace CRUD / secrets | domain in Rust; UI presents | `WorkspaceService` + `ConfigResolutionScope` (ADR-0002) | Held (sampled) |
| ACP desktop types | generated SDK (ADR-0004) vs hand `src/types` (AGENTS.md) | **conflict of two accepted rules** | Drift-prone |
| Tool permission (stored) | `PermissionManager` | `permission_inspector.rs` + YAML | Held for explicit entries |
| Tool permission (session mode) | `GoslingMode` | inspector + Auto shortcut | Auto nullifies unset tools |
| Subagent authority | should not exceed parent | **not enforced** — hardcoded Auto | Finding NEG-GSL-001 |
| Project plugins / hooks / plugin MCP | explicit `trust_project` | `plugins/discovery.rs` | Held |
| Project skills / `AGENTS.md` | none declared | auto-load from cwd | Finding NEG-GSL-002 |
| Provider swap | uniform `Provider` | capability flags added (`manages_own_context`, `executes_tools_outside_gosling`) | Partial — flattening remains |
| Goose catalog | docs fallback + CLI import | `GOOSE_COMPATIBILITY.md` | Sanctioned; dual JS/TS copies |
| Imported transcript | untrusted history | `imported_untrusted` + prompt | Held for history; cwd still live |
| Security scanner | prompt-injection backstop | default **on** (`unwrap_or(true)`) | Held vs July; extract still `command`-first |

---

## Invariant-Sync Inventory

| Invariant (the fact) | Ground-Truth Source | Copies | Must Match / May Differ | Handling Class | Drift Guard | Delta |
|---|---|---|---|---|---|---|
| GoslingMode values | `gosling-providers/src/gosling_mode.rs:24-34` | `ui/desktop/src/types/session.ts:5`; `ModeSelectionItem.tsx:48-69` | must match | display / wire | none | aligned today |
| SessionType | `session_manager.rs:87-95` | `ui/desktop/src/types/session.ts:26` | must match | wire | none | aligned |
| ThinkingEffort | `thinking.rs:278-285` (now includes Ultra) | `ui/desktop/src/types/providers.ts:3` | must match | wire | none | aligned |
| MessageContent variants | `message.rs:271-282` | `ui/desktop/src/types/message.ts:177-187` | must match | wire | none | aligned |
| ActionRequiredData | `message.rs:199-218` | `message.ts:108-127` | must match | wire | none | aligned |
| SystemNotificationType | `message.rs:253-257` | `message.ts:96` | must match | wire | none | aligned |
| ProviderType | `providers/base.rs:19-24` | `ui/desktop/src/types/providers.ts:1` | must match | wire | none | aligned |
| Confirmation Permission | `gosling-providers/src/permission.rs:6-12` | `ui/desktop/src/types/permissions.ts:1` | must match | one-shot decision | none | aligned |
| Stored PermissionLevel | `config/permission.rs:19-23` | SDK `ToolPermissionLevel` generated; UI modal literals `PermissionModal.tsx:93-95`; CLI `configure.rs:1739-1767` | must match | persist | compiler on Rust `From`; **no** UI/CLI↔enum test | aligned today |
| Permission ↔ PermissionDecision | `acp/common.rs:31-53` | ACP option ids | must map | degrade toward less authority | exhaustive `From` + tests | held |
| DictationProvider | `dictation/providers.rs:16-20` | request doc `custom_requests.rs:1793`; generated `types.gen.ts:2322` | must match | reject unknown | none | **doc has phantom `local`** |
| Built-in slash commands | `execute_commands.rs:19-57` `COMMANDS` | dispatch `match` `:126-140`; test vector `slash_command.rs:57-62` | must match | advertise vs handle | names-list test only | aligned; no handler guard |
| "Is a shell/command tool" | no single registry | `scanner.rs:518-527`; `egress_inspector.rs:285-294`; `working_dir_scope_inspector.rs:437-440`; adversary `DEFAULT_TOOLS` `:14` | must-match **class** (command-executing) | scan / egress / cwd-scope | none | **divergent predicates** |
| Goose excluded skill ids | `GOOSE_COMPATIBILITY.md` | `goose-compat.ts:9-12`; `goose-compat.js:8` | must match | filter | script self-test only; **no TS↔JS test** | aligned today |
| Declarative providers | `include_dir!` of JSON | — | n/a | register | directory scan | single source |
| ACP session modes | `GoslingMode::VARIANTS` | response builder | n/a | advertise | strum | single source (Rust) |
| sessions.db columns | SQL in `SessionStorage` | `Session` serde fields | must match | persist | migrations in-file | not field-diffed (sampled) |

---

## Assumption Ledger (NEG)

| # | Assumption | Where | What relies on it | If false | Evidence status |
|---|---|---|---|---|---|
| A1 | Parent approval mode gates every tool the product runs | inspector + UI mode picker | human-in-the-loop | Subagent/orchestrator Auto | Confirmed — still true |
| A2 | Untrusted repo cannot run code/hooks without `plugin trust` | `plugins/discovery.rs` comments SEC-GSL-101 | plugin/hook/MCP from `.agents/plugins` | Skills + AGENTS.md + hints still load | Confirmed |
| A3 | A tool named something other than `shell` is not a command sink | historical scanner | injection scan | Partially repaired; extract still prefers `command` | Residual |
| A4 | `read_only_hint` is honest | historical inspector | auto-approve | **Repaired** — hint can only tighten | Held |
| A5 | Scanner is an optional extra | July report (`unwrap_or(false)`) | defense in depth | Now `unwrap_or(true)` at `security/mod.rs:52-54` | Held (default on) |
| A6 | Single local operator | CLI/desktop/serve all share Config + PermissionManager singletons | no authz between surfaces | `gosling serve` + desktop + CLI are multiple actors on one store | Plausible (NEG-013) |
| A7 | `GOSLING_SHELL` is POSIX-shaped | `shell.rs` + `patterns.rs` | regex scanner | Non-POSIX shell changes semantics | Plausible |
| A8 | Goose catalog is advisory docs | `GOOSE_COMPATIBILITY.md` | discovery UX | Live fetch of upstream JSON is an unmodeled input | Speculative for product; Confirmed for docs site |
| A9 | Approving `summon` is not approving the child's tools | summon comment | confirmation UX | Hidden actor | Confirmed |

---

## Findings Table

| ID | Severity | Confidence | Evidence Basis | Domain | Title | Patch Priority | Blast Radius | Complexity | Cost | Nominal Agent |
|---|---|---|---|---|---|---|---|---|---|---|
| NEG-GSL-001 | High | Confirmed | source-evidenced | Negative-Space | Subagent/orchestrator hard-codes Auto | 1 | Service | cross_process_coordination | L | claude |
| NEG-GSL-002 | High | Confirmed | source-evidenced | Negative-Space | Untrusted-repo skills+hints bypass plugin trust | 2 | Workflow | workflow_protocol | M | claude |
| ARC-GSL-001 | Medium | Confirmed | source-evidenced | Architecture | Three god objects grew since July | 4 | Repo | workflow_protocol | L | claude |
| ARC-GSL-002 | High | Confirmed | source-evidenced | Architecture | Providers crate owns the conversation domain | 5 | Repo | workflow_protocol | L | claude |
| ARC-GSL-003 | Medium | Confirmed | source-evidenced | Architecture | `Config::global()` ambient coupling | 6 | Repo | cross_process_coordination | XL | claude |
| ARC-GSL-004 | Medium | Confirmed | source-evidenced | Architecture | ACP/CLI agents flattened behind `Provider` | 3 | Workflow | external_service_semantics | L | multi-agent |
| ARC-GSL-005 | Low | Confirmed | source-evidenced | Architecture | Dual Goose-compat implementations | 8 | Repo | local_guardrail | S | codex |
| ARC-GSL-006 | Medium | Confirmed | source-evidenced | Architecture | Core crate exposes every module | 7 | Repo | workflow_protocol | L | claude |
| INV-GSL-001 | Medium | Confirmed | source-evidenced | Invariant Sync | Rust↔desktop enums have no drift guard | 4 | Workflow | local_guardrail | S | codex |
| INV-GSL-002 | Low | Confirmed | source-evidenced | Invariant Sync | Dictation doc advertises phantom `local` | 9 | Local | local_guardrail | XS | codex |
| INV-GSL-003 | Low | Confirmed | source-evidenced | Invariant Sync | Slash `COMMANDS` vs dispatch `match` | 10 | Local | local_guardrail | S | codex |
| INV-GSL-004 | Medium | Confirmed | source-evidenced | Invariant Sync | Three `is_shell_tool*` predicates | 3 | Workflow | local_guardrail | S | codex |
| NEG-GSL-003 | Low | Plausible | source-evidenced | Negative-Space | `GOSLING_SHELL` is unmodeled scanner input | 11 | Local | local_guardrail | XS | codex |

---

## Detailed Findings

### NEG-GSL-001: Subagent and orchestrator agents still run Auto — delegation is unsupervised execution

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Negative-Space

Evidence:
- `crates/gosling/src/agents/platform_extensions/summon.rs:1097-1118` — comment still says subagents "must use Auto until `get_agent_messages` forwards `ActionRequired`"; `AgentConfig::new(..., GoslingMode::Auto, ...)` and `create_session(..., SessionType::SubAgent, GoslingMode::Auto)`.
- `crates/gosling/src/agents/platform_extensions/orchestrator.rs:415-427` — orchestrator-managed agents are the "same Auto+SubAgent combination" so `redirect_unapprovable_subagent_requests` can run.
- `crates/gosling/src/permission/permission_inspector.rs:162-166` — if no explicit user permission, `GoslingMode::Auto` → `InspectionAction::Allow` / `"Auto mode - all tools approved"`.
- `crates/gosling/src/agents/agent.rs:904-930` — unanswerable `needs_approval` on a SubAgent is **denied** (hang path repaired), not forwarded to the parent.
- `crates/gosling/src/agents/subagent_handler.rs:232-248` — the child loop consumes `AgentEvent::Message` and never surfaces confirmation to a parent.

Observed behavior:
- Parent may be in Approve or SmartApprove. `summon` / orchestrator still spawn a child that auto-approves every tool the user has not previously listed in `permission.yaml`. Security/egress/adversary `RequireApproval` is fail-closed to deny (not hang). Unmarked `shell`, `automation_script`, MCP tools execute.

Expected boundary:
- Delegating a task must not grant more authority than the parent session. Either forward `ActionRequired` to the parent (the deferred fix in the comment) or require an explicit, non-cacheable "this child will run unsupervised" confirmation.

Failure mechanism:
- Hidden actor (NEG-002) + assumption collapse (A1). Two safe intentions (don't hang; gate tools) compose into unsupervised execution. The July hang is fixed; the authority leak is not.

Break-it angle:
- Parent in Approve. Prompt-injected content induces `summon` with "run `curl … | sh`". Child Auto-allows `shell` unless the user had pre-set NeverAllow on that exact tool name.

Impact:
- The product's primary safety control is nullified for delegated work. Blast radius: local workstation, arbitrary tools.

Operational impact:
- Blast radius: Service
- Side-effect class: process / file / network
- Reversibility: irreversible
- Operator visibility: silent (child tool calls are notifications, not approvals)
- Rerun safety: unsafe

Adjacent failure modes:
- NEG-GSL-002 (repo skills + Auto child)
- ARC-GSL-004 (ACP CLI providers also map Auto → `AllowOnce` at `acp/provider.rs:1611-1616`)
- INV-GSL-004 (scanner coverage of the child's command tools)

Recommended mitigation:
- Remediation patterns: forward confirmation; inherit parent mode.
- Minimal repair: inherit parent `GoslingMode` and route child `ActionRequired` onto the parent's `tool_confirmation_router`. Until that exists, deny (don't allow) unmarked tools in `SessionType::SubAgent` the same way AskBefore is already denied in Auto.
- Local guardrail: fail the `summon` tool unless the parent is already Auto, *or* require a one-shot confirmation that names the child task.
- Behavior test: parent Approve + delegated `shell` → no execution without a delivered parent confirmation (or a deny).

Implementation assessment:
- Complexity: cross_process_coordination
- Cost: L
- Cost drivers: modules, tests, runtime_verification
- Nominal implementation agent: claude
- Rationale: confirmation plumbing across agent instances; security-adjacent.

Validation:
- Integration test: Approve parent + `summon` shell → denied or parent prompt, never silent exec.
- Regression: existing Auto NeverAllow/AskBefore tests in `permission_inspector.rs:312-365` stay green.

Non-goals:
- Do not switch children to Approve without wiring the confirmation path (reintroduces the hang the comment avoids).

---

### NEG-GSL-002: Untrusted-repo composition — skills and hints load without the plugin trust gate

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Negative-Space

Evidence:
- Plugin trust (the *declared* untrusted-repo control): `crates/gosling/src/plugins/discovery.rs:90-97,114-132,169-175` — project plugins are "never enabled or marked `trusted` from repo-shipped content"; only `trust_project` (CLI `gosling plugin trust`) flips `trusted`.
- Hooks only load from `discover_enabled_plugins` (`hooks/mod.rs:239`) — so hooks **are** gated.
- Plugin MCP servers same (`plugins/mcp_servers.rs:32-34`).
- Skills are **not** gated: `skills/mod.rs:304-311` always scans `<cwd>/.agents/skills`, `.gosling/skills`, `.claude/skills`; `discover_skills` (`:501-508`) returns them with no `trusted` bit.
- `sources.rs:870-874` lists those skills for ACP/slash/load.
- Hints: `hints/load_hints.rs:10-17,20-32` default-loads `.goslinghints` and `AGENTS.md` from the working tree (and subdirs visited by tool args, `:51-77`).
- Import composition: imported messages are tagged untrusted (`session_manager.rs:4828-4832`) and a system prompt warns (`acp/server.rs:1404-1408`), but the session still uses a live `working_dir` from which skills/hints load.
- Combined with NEG-GSL-001: a child Auto agent inherits parent extensions (`orchestrator.rs:438`) and runs in that cwd.

Observed behavior:
- Checking out an untrusted repo and opening a session there will **not** auto-run repo plugins/hooks (held). It **will** inject repo `AGENTS.md` / `.goslinghints` into context and advertise/load repo skills as first-class sources and slash commands.

Expected boundary:
- Anything that executes or steers from repo-authored files should share one trust bit. Skills and context files are unmodeled input (NEG-003) that compose with Auto children (NEG-004).

Failure mechanism:
- Cross-boundary composition. The SEC-GSL-101 repair closed the plugin/hook auto-exec hole and left the instruction/skill channel open. Repo prose becomes policy.

Break-it angle:
- Malicious `AGENTS.md` + `.agents/skills/pwn/SKILL.md` in a cloned repo. User starts a chat (or imports a transcript into that cwd). Agent loads the skill; if a subagent is spawned, Auto executes whatever the skill instructs.

Impact:
- Instruction-level compromise of an untrusted checkout without `plugin trust`. Not the same as silent hook exec, but enough to drive tools once the user sends any message (or a parent delegates).

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible (prompt) then process/file if tools run
- Reversibility: compensatable (don't open the repo) / irreversible once tools run
- Operator visibility: silent (skills appear as normal sources)
- Rerun safety: unsafe

Adjacent failure modes:
- NEG-GSL-001
- NEG-011 (model/provider output plus repo skill)
- ARC-010 (UI has no "untrusted checkout" affordance — stub to workflow-gui)

Recommended mitigation:
- Remediation patterns: one trust bit for project-scoped *instruction* surfaces.
- Minimal repair: treat project skill dirs and context files as untrusted unless `trust_project` (or a sibling `trust_project_sources`) has been called; surface the same warning used for plugins.
- Local guardrail: do not register project skills as slash commands until trusted.
- Behavior test: untrusted project `SKILL.md` is listed as pending, not loaded into the agent prompt; after `gosling plugin trust` (or explicit skills-trust) it loads.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: M
- Cost drivers: modules, tests, operator_training
- Nominal implementation agent: claude
- Rationale: policy decision + discovery change; must not break intentional project skills for trusted repos.

Validation:
- Test: untrusted cwd skill does not appear in `discover_skills` enabled set.
- Test: trusted project still discovers `.agents/skills`.

Non-goals:
- Do not disable global `~/.agents/skills`.
- Do not treat `AGENTS.md` in *this* gosling repo as hostile; the finding is the missing *runtime* trust bit.

---

### ARC-GSL-001: God objects — `agent.rs`, `session_manager.rs`, `extension_manager.rs`

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture

Evidence:
- `crates/gosling/src/agents/agent.rs` — **5332** lines (July report: 3858). `Agent` at `:318-342` holds provider, mode watch, `extension_manager`, frontend tools, `prompt_manager`, hint tracker, confirmation + frontend-result routers, `tool_inspection_manager`, `hook_manager`, plus `container`/`goal`/`grind`/`pending_steers` mutexes. `Agent::new` (`:400-410`) constructs from `Config::global()` + `SessionManager::instance()` + `PermissionManager::instance()`.
- `crates/gosling/src/session/session_manager.rs` — **8740** lines. One file defines `SessionType` (`:87`), `SessionManager` (`:513`), **and** `SessionStorage` (`:1141`) plus import/export (`:4787-4835`), artifacts, naming, search, and tests. `SessionManager` is a thin facade (`:611-653`) over in-file storage that still owns unrelated responsibility classes.
- `crates/gosling/src/agents/extension_manager.rs` — **4096+** lines. `ExtensionManager` (`:334-345`) owns live MCP clients, docker processes, tools cache, capabilities, **and** `dispatch_tool_call` (`:2470`), resource listing, prompt listing, `search_available_extensions` (`:2689`), OAuth fallback tests, pagination guards.
- `config/base.rs` is **3199+** and is the ambient hub (see ARC-GSL-003), not re-counted as a fourth finding.

Observed behavior:
- The three named hubs still accrete features. Agent grew ~1.5k lines since the July seam audit.

Expected boundary:
- ARC-001: one module, one responsibility class; orchestrators delegate. Persistence, MCP lifecycle, and the turn loop should not share a file with their tests *and* their satellite domains.

Failure mechanism:
- Growth attractor. Every new session/MCP/tool concern lands in the file that already imports the dependencies.

Break-it angle:
- Any lock-order or import-cycle change in session storage, extension dispatch, or agent steering edits the same multi-thousand-line files; review cannot hold the whole type.

Impact:
- Change-coupling and untestable cores (ARC-020): `Agent::new` cannot be constructed without global Config + singleton session store.

Operational impact:
- Blast radius: Repo
- Side-effect class: none (structural)
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- ARC-GSL-003, ARC-GSL-006
- Concurrency (multiple inner mutexes) — escalate, do not deep-audit here

Recommended mitigation:
- Extract `SessionStorage` (+ tests) from `session_manager.rs`; extract MCP transport/lifecycle from `ExtensionManager`; keep `Agent` as a wiring facade over already-split `tool_execution.rs` / `execute_commands.rs` / `reply_parts.rs`.
- Guardrail: file-size budget CI on these three paths.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: L
- Cost drivers: modules, tests
- Nominal implementation agent: claude

Validation:
- `SessionStorage` compiles as its own module; `Agent` constructs in tests with fakes only.

Non-goals:
- Do not change session schema or MCP protocol in the extract.

---

### ARC-GSL-002: The providers crate still owns the conversation domain (inverted ownership)

Severity: High  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture

Evidence:
- `crates/gosling/src/lib.rs:12-14` — `pub mod conversation { pub use gosling_providers::conversation::*; }`.
- `crates/gosling-providers/src/conversation/message.rs:271` — `pub enum MessageContent` (domain nucleus) lives in the adapters crate.
- `crates/gosling/src/providers/base.rs:16,30` — `pub use gosling_providers::base::*;` then a **second** trait `ProviderDef` in core that constructs `type Provider: Provider`.
- `crates/gosling/src/providers/mod.rs:3-8,54-62` — re-export shims (`anthropic`, `ollama`, `openai`, `http_status`) interleaved with in-core impls (`claude_code`, `bedrock`, `cursor_agent`, …).
- `impl Provider for` appears in `gosling-providers` (openai/anthropic/ollama/openai_compatible) and ~20 times under `crates/gosling/src/providers/`.

Observed behavior:
- Deleting or swapping the "provider adapters" crate deletes `Message` / `GoslingMode` / `ThinkingEffort`. Most concrete providers never moved.

Expected boundary:
- Dependencies point toward the domain. Adapters depend on `Message`; they do not define it.

Failure mechanism:
- Partial extraction: only impls without `ExtensionConfig` could move; the name `gosling-providers` now lies about optionality (ARC-012).

Break-it angle:
- "Lighter goose" remix that drops providers cannot compile session, ACP, or context_mgmt.

Impact:
- Layering is unreadable from crate names; provider refactors risk the domain model.

Operational impact:
- Blast radius: Repo
- Side-effect class: none
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- ARC-GSL-004, ARC-GSL-006, INV-GSL-001 (desktop copies types that live in the wrong crate)

Recommended mitigation:
- Move `conversation`, `gosling_mode`, `thinking`, `permission` into a domain crate (or back into `gosling`); leave only HTTP adapters in `gosling-providers`.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: L
- Cost drivers: modules, many import sites
- Nominal implementation agent: claude

Validation:
- `rg "gosling_providers::conversation"` only hits provider-internal files.

Non-goals:
- Do not merge all providers back into core in the same slice.

---

### ARC-GSL-003: `Config::global()` is pervasive hidden / ambient coupling

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture

Evidence:
- `crates/gosling/src/config/base.rs:537-549` — `pub fn global() -> &'static Config` leaks a `Box` into a process-wide `HashMap<PathBuf, &'static Config>` keyed by `Paths::config_dir()`.
- Grep of `Config::global()` at HEAD: **at least 187** call sites across `gosling`, `gosling-cli`, `gosling-server` (CLI configure/session, ACP server, security, workspace credentials, provider inventory, plugins, skills catalogs, …).
- Interior mutability: `guard`, `secrets_cache`, `param_cache` (`base.rs:258-263`, construction `:566-572`).
- ADR-0002 documents that providers still "read logical config keys from process-global `Config`" and layers a task-local `ConfigResolutionScope` on top rather than injecting a handle.
- `Agent::new` (`agent.rs:400-410`) and `discover_enabled_plugins` (`discovery.rs:54-55`) and `discover_skills` catalogs (`catalog.rs:95-98`) all reach the singleton.

Observed behavior:
- Any layer reads/writes process-global config and secrets. Tests and multiple front-ends in one process share one map (now per `config_dir`, still process-global).

Expected boundary:
- Config is an injected capability. `global()` is a composition-root default only.

Failure mechanism:
- Convenience singleton. ADR-0002's scope adapter acknowledges the coupling and works around it.

Break-it angle:
- Two agents / tests in one process; one `set_param` is visible to the other. Future embed/multi-tenant cannot isolate secrets.

Impact:
- Invisible hub; blocks remix; secret cache invalidation becomes a cross-cutting ritual (existing ACP tests exist *because* of this).

Operational impact:
- Blast radius: Repo
- Side-effect class: file / process (keyring)
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- ARC-018/019/020 (same singleton)
- Concurrency / security (secret cache) — stub

Recommended mitigation:
- Thread `Arc<Config>` from CLI/server composition roots; keep `global()` as last-resort default.

Implementation assessment:
- Complexity: cross_process_coordination
- Cost: XL
- Cost drivers: 187+ sites, tests
- Nominal implementation agent: claude

Validation:
- Test constructs `Agent` with an isolated `Config` and never touches the leaked map.

Non-goals:
- Do not change on-disk YAML/keyring format.

---

### ARC-GSL-004: ACP/CLI subprocess agents are flattened behind `Provider`

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture

Evidence:
- `crates/gosling/src/acp/provider.rs:428` — `impl Provider for AcpProvider` (spawns stdio JSON-RPC agent).
- Same trait as HTTP completions. Capability flags were added: `Provider::manages_own_context` / `executes_tools_outside_gosling` (`gosling-providers/src/base.rs:718-737`) — Claude Code/Codex/Gemini CLI/Cursor set both true (`claude_code.rs:771-777`).
- `Agent::ensure_provider_ready` (`agent.rs:1723-1730`) refuses `executes_tools_outside_gosling` when the session restricts tools to working dirs — a **local** honesty patch, not a separate seam.
- `permission_decision_from_mode` (`acp/provider.rs:1611-1616`) maps `GoslingMode::Auto` → `PermissionDecision::AllowOnce` for the *subprocess* permission prompt.
- `ProviderDef` stays in core because construction needs `ExtensionConfig` (`providers/base.rs:30-46`).

Observed behavior:
- Callers still `stream(messages, tools)` at a type that may ignore those tools and run its own session, permissions, and shell. Flags exist; the trait is still one type.

Expected boundary:
- ARC-007/008: stateful delegated agents are not substitutable completions. Model `DelegatedAgent` (or require the flags at the type level, not default-false methods).

Failure mechanism:
- Reuse of the agent loop forced a fake adapter.

Break-it angle:
- Swap OpenAI → Claude Code behind `dyn Provider`: caller-supplied tools and Gosling inspectors are not the execution path; working-dir restrict is the only hard stop.

Impact:
- Divergent permission/tool semantics hidden at the call site (security-adjacent).

Operational impact:
- Blast radius: Workflow
- Side-effect class: process
- Reversibility: n/a (design)
- Operator visibility: log-only
- Rerun safety: unknown

Adjacent failure modes:
- NEG-GSL-001 (Auto mapped to AllowOnce on the ACP side too)
- Security lens for subprocess tool loop

Recommended mitigation:
- Split the trait or make `executes_tools_outside_gosling` a constructor-level kind that the agent loop matches on exhaustively.

Implementation assessment:
- Complexity: external_service_semantics
- Cost: L
- Cost drivers: modules, runtime_verification
- Nominal implementation agent: multi-agent

Validation:
- Type-level test: agent loop path for `executes_tools_outside_gosling == true` does not call `dispatch_tool_call`.

Non-goals:
- Do not rewrite ACP transport.

---

### ARC-GSL-005: Goose compatibility is a sanctioned dual stack with two live implementations

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture

Evidence:
- Sanction: `documentation/GOOSE_COMPATIBILITY.md:1-22` — fallback to `https://goose-docs.ai/servers.json` and `skills-manifest.json` is intentional; excluded ids `code-review`, `testing-strategy`.
- Copy 1: `documentation/src/utils/goose-compat.ts:4-12,62-99,157-158` (Docusaurus runtime fetch in `mcp-servers.ts:17-34`, `skills.ts:90-123`).
- Copy 2: `documentation/scripts/goose-compat.js:4-8` (build-time `--servers` / `--skills` / `--self-test`).
- Product desktop does **not** fetch Goose (`ui/desktop` grep has only marketing "fork of goose").
- Product CLI has a **third** Goose path: `gosling mcp install --from-goose` (`gosling-cli/src/cli.rs:663-672`) importing Goose `config.yaml` (not the AAIF catalog).

Observed behavior:
- Two hand-maintained normalizers must stay identical. A third, different Goose surface (local config import) shares the name.

Expected boundary:
- One implementation, or generated JS from TS. Sanctioned dual *product* stacks are a non-finding (ARC-022 deny if documented + selector). Here the *docs* dual copy is undocumented as two sources.

Failure mechanism:
- Overbuilt compatibility (ARC-015) plus duplicate parallel abstraction (ARC-022) at the adapter, not the product.

Break-it angle:
- Add an excluded skill id in `.ts` only; the static `skills-manifest.json` build still ships it.

Impact:
- Docs catalog drift; not a runtime RCE. CLI `--from-goose` is a separate trust decision (operator-initiated).

Operational impact:
- Blast radius: Repo
- Side-effect class: none (docs) / file (CLI import)
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- INV-GSL-001-class (two copies, no TS↔JS guard)
- NEG-012 (upstream catalog policy change)

Recommended mitigation:
- Make `goose-compat.js` require/build from the TS module, or add a fixture test that both exclude sets and rewrite rules are equal.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Cost drivers: tests
- Nominal implementation agent: codex

Validation:
- Test: `GOOSE_EXCLUDED_SKILL_IDS` and rewrite samples equal across JS and TS.

Non-goals:
- Do not remove the Goose fallback (`GOOSE_COMPATIBILITY.md` forbids that).

---

### ARC-GSL-006: `gosling` has no public facade — 49 `pub mod`s are the API

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture

Evidence:
- `crates/gosling/src/lib.rs:4-49` — every top-level module is `pub` (`acp`, `agents`, `config`, `providers`, `session`, `security`, `permission`, …). No `pub(crate)` core, no `gosling::api`.
- CLI and server import internal paths directly (`Config::global`, `SessionManager`, provider modules).

Observed behavior:
- Internal layout is an accidental frozen surface (ARC-013). Any extract for ARC-GSL-001/002 breaks front-ends.

Expected boundary:
- A library with three front-ends exposes a curated API.

Failure mechanism:
- Monolith modules were made `pub` as CLI needed them.

Break-it angle:
- Move `session::SessionStorage` out of `session_manager.rs` and every `gosling::session::…` import churns.

Impact:
- Refactor tax; "easy-to-remix" blocked.

Operational impact:
- Blast radius: Repo
- Side-effect class: none
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Recommended mitigation:
- Incremental `pub(crate)` + `gosling::api` facade; start with `utils`, `subprocess`, `token_counter`.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: L
- Nominal implementation agent: claude

Validation:
- Compile-fence in CLI/server against the facade only.

Non-goals:
- Do not hide ACP server modules the desktop host needs in this slice.

---

### INV-GSL-001: Rust↔desktop wire enums are hand-copied with no drift guard

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Invariant Sync

Evidence:
- Ground truth: `GoslingMode` `gosling_mode.rs:24-34`; `SessionType` `session_manager.rs:87-95`; `ThinkingEffort` `thinking.rs:278-285`; `MessageContent` `message.rs:271-282`; confirmation `Permission` `permission.rs:6-12`.
- Hand copies: `ui/desktop/src/types/session.ts:5,26`; `types/providers.ts:1,3`; `types/message.ts:96,108-127,177-187`; `types/permissions.ts:1`; third GoslingMode copy `ModeSelectionItem.tsx:48-69`.
- ADR-0004 rejected local TS workspace interfaces; `AGENTS.md` still forbids desktop imports from `ui/desktop/src/api` generated OpenAPI. Generated ACP SDK exists (`ui/sdk/src/generated/types.gen.ts`) and desktop *does* import `ToolPermissionLevel` from `@repo-makeover/gosling-sdk` (`acp/permissions.ts:1-4`) — **inconsistent**: stored permissions are generated; session/message enums are not.
- No desktop test asserts parity against `acp-schema.json` or `types.gen.ts`.

Observed behavior:
- Values match *today* (including new `ThinkingEffort::Ultra` on both sides). Nothing fails if Rust adds a `MessageContent` variant and TS does not.

Expected boundary:
- INV-007/009: one source or a parametrized guard.

Failure mechanism:
- Two accepted policies fight (ADR-0004 generate vs AGENTS.md don't import generated OpenAPI). Desktop invented a third path (hand types + selective SDK imports).

Break-it angle:
- Add `MessageContent::Reasoning`; desktop union has no arm; block dropped.

Impact:
- UI truth gap on the next enum extension. Not current data corruption.

Operational impact:
- Blast radius: Workflow
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- INV-GSL-002 (already drifted doc)
- ARC-013 frozen-surface / IAPI stub

Recommended mitigation:
- Parity test: generated SDK enum members == `ui/desktop/src/types` unions; derive `all_gosling_modes` from one `as const` array.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Nominal implementation agent: codex

Validation:
- Vitest: set-equal generated `GoslingMode` / `SessionType` / `MessageContent` tags vs hand types.

Non-goals:
- Do not re-enable `src/api` OpenAPI client imports.

---

### INV-GSL-002: Dictation request documents a phantom `local` provider

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Invariant Sync

Evidence:
- Enum: `crates/gosling/src/dictation/providers.rs:16-20` — `OpenAI | ElevenLabs | Groq`.
- Doc + generated echo: `crates/gosling-sdk-types/src/custom_requests.rs:1793` — `/// Provider to use: "openai", "groq", "elevenlabs", or "local"`; `ui/sdk/src/generated/types.gen.ts:2322` copies the comment.
- Field is `pub provider: String` (`:1794`), not the enum.

Observed behavior:
- Authoritative copy is the enum; consumed copy is a free `String` whose advertised set includes `local`.

Expected boundary:
- Typed field or doc generated from the enum.

Failure mechanism:
- INV-011/013: doc is treated as the contract; generator faithfully ships the lie.

Break-it angle:
- Client sends `provider: "local"`; deserialize-to-enum at the handler rejects.

Impact:
- Misleading ACP contract; request fails closed (no exec).

Operational impact:
- Blast radius: Local
- Side-effect class: none
- Reversibility: reversible
- Operator visibility: log-only
- Rerun safety: safe

Recommended mitigation:
- Type the field as `DictationProvider`; delete `"local"`.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Nominal implementation agent: codex

Validation:
- `"local"` rejected; three enum variants round-trip.

Non-goals:
- Do not add a local dictation backend.

---

### INV-GSL-003: Slash-command `COMMANDS` table and dispatch `match` are parallel copies

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Invariant Sync

Evidence:
- Advertise: `execute_commands.rs:19-57` (`prompts, prompt, compact, clear, skills, doctor, goal, grind, status`).
- Dispatch: `:126-140` same nine names + `_ => handle_skill_command`.
- Guard: `slash_command.rs:50-63` hard-codes the **same** nine names (a third copy). Nothing asserts each advertised name has a dedicated arm.

Observed behavior:
- Aligned today. Add-to-table-forget-match falls through to skill lookup.

Expected boundary:
- Table *is* dispatch, or a test that every `COMMANDS` name is a non-fallthrough arm.

Failure mechanism:
- INV-007/010.

Break-it angle:
- Add `review` to `COMMANDS` and to the test vector; omit the match arm → advertised built-in runs as a skill.

Impact:
- Mis-routed command; user-visible, reversible.

Operational impact:
- Blast radius: Local
- Side-effect class: user-visible
- Reversibility: reversible
- Operator visibility: silent
- Rerun safety: safe

Recommended mitigation:
- Handler pointer on `CommandDef`; test parametrized over `COMMANDS`.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Nominal implementation agent: codex

Validation:
- Every `COMMANDS` name does not hit the skill catch-all.

Non-goals:
- Do not redesign skill slash commands.

---

### INV-GSL-004: "Command-executing tool" is three hand-maintained predicates

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Invariant Sync

Evidence:
- Scanner: `security/scanner.rs:518-527` — exact names `shell|bash|execute_command|run_command|terminal` plus `__` suffixes. `should_scan_tool_call` (`:529-543`) also scans if args have `command|cmd|script|input|url|uri|endpoint`.
- Egress: `egress_inspector.rs:285-294` — **same name list**, no arg-key fallback.
- Working-dir scope: `working_dir_scope_inspector.rs:437-440` — `name.contains("shell"|"command"|"terminal")` (broader; matches `automation_script` via neither `shell` nor `command` nor `terminal` — **misses** `computercontroller__automation_script`).
- Adversary: `adversary_inspector.rs:14` — `DEFAULT_TOOLS = ["shell", "computercontroller__automation_script"]`.
- `automation_script` args use `script` not `command` (`gosling-mcp/.../computercontroller/mod.rs:149-155`). Scanner will still *scan* via the `script` key; extract prefers `command` (`scanner.rs:497-503`) and otherwise dumps JSON. Egress and cwd-scope use name-only predicates that **do not** include `automation_script`.

Observed behavior:
- July's "only the literal name `shell`" hole is mostly closed for the scanner. The **set** of command tools is still replicated and already disagrees on `automation_script` / `computer_control`.

Expected boundary:
- One `is_command_tool` / `extract_command_text` used by scanner, egress, cwd-scope, adversary.

Failure mechanism:
- INV-001/010. Adding a new command MCP tool requires N edits; the acting path (cwd-scope) is the narrowest on this name.

Break-it angle:
- `computercontroller__automation_script` in a working-dir-restricted session: cwd inspector may not treat it as a shell tool; scanner sees `script` and scans; egress may not classify it as shell.

Impact:
- Inconsistent security policy per inspector. Residual injection/path-escape depends on which inspector is the backstop.

Operational impact:
- Blast radius: Workflow
- Side-effect class: process
- Reversibility: irreversible if missed
- Operator visibility: log-only
- Rerun safety: unknown

Adjacent failure modes:
- NEG-GSL-001 (Auto child + non-shell command tool)
- Security lens

Recommended mitigation:
- Single predicate module; include `automation_script` / `computer_control` and prefixed forms; extract `command` then `script` then `cmd`.

Implementation assessment:
- Complexity: local_guardrail
- Cost: S
- Nominal implementation agent: codex

Validation:
- Parametrized test: the four sites classify the same fixture tool list identically.

Non-goals:
- Do not change default scanner enablement here.

---

### NEG-GSL-003: `GOSLING_SHELL` is unmodeled input to a POSIX pattern scanner

Severity: Low  
Confidence: Plausible  
Evidence basis: source-evidenced  
Domain: Negative-Space

Evidence:
- `agents/platform_extensions/developer/shell.rs:88-92,132-142` — `GOSLING_SHELL` is "passed through as-is"; default is bash-or-sh, not `$SHELL`.
- Scanner patterns remain POSIX/PowerShell-shaped (not re-read line-by-line this pass; residual of July NEG-GSL-004).

Observed behavior:
- Operator-selected interpreter can express the same intent in a form the regex set misses.

Expected boundary:
- Treat the resolved shell as scanner input, or document POSIX-only coverage.

Failure mechanism:
- NEG-003 unmodeled input. Operator is mostly trusted, so severity stays Low.

Break-it angle:
- `GOSLING_SHELL=nu` + a nu-idiom download-exec string.

Impact:
- Narrow hardening gap.

Operational impact:
- Blast radius: Local
- Side-effect class: process
- Reversibility: irreversible
- Operator visibility: log-only
- Rerun safety: unknown

Recommended mitigation:
- Attach resolved shell flavor to findings; select pattern pack by flavor.

Implementation assessment:
- Complexity: local_guardrail
- Cost: XS
- Nominal implementation agent: codex

Validation:
- Documented out-of-scope flavors **or** a fixture that flags equivalent intent under `nu`.

Non-goals:
- Do not sandbox `GOSLING_SHELL`.

---

## Inventory Disposition

### ARC-001..025

| Code | Result |
|---|---|
| ARC-001 God Object | **Finding** ARC-GSL-001 |
| ARC-002 Boundary Violation | **Finding** ARC-GSL-002 (domain in providers crate); workspace UI→ACP direction **held** (`docs/architecture.md:9-35`) |
| ARC-003 Hidden Coupling | **Finding** ARC-GSL-003 |
| ARC-004 Wrong Ownership | **Finding** ARC-GSL-002 (`Message` owned by adapters); permission-mode decision owned by inspector **held** |
| ARC-005 Circular Dependency | **Non-finding** — `gosling-providers` has no `gosling` dep; `agents/` has no `use crate::acp`; `acp/` imports `agents` one-way. Intra-crate file cycles not graph-tooled. |
| ARC-006 Leaky Abstraction | **Finding-adjacent** ARC-GSL-004 — callers must know `manages_own_context` / `executes_tools_outside_gosling`. Default methods hide the leak until a flag is set. |
| ARC-007 Fake Adapter | **Finding** ARC-GSL-004 (`AcpProvider`) |
| ARC-008 Provider Contract Flattening | **Finding** ARC-GSL-004 |
| ARC-009 Policy Mixed With Mechanism | **Non-finding as material** — Auto allow is policy in the inspector (right layer). Residual: `permission_decision_from_mode` inside `acp/provider.rs` (mechanism mapping policy) folded into ARC-GSL-004. |
| ARC-010 UI Owns Domain Rule | **Non-finding (sampled)** — workspace validation is invoked as `validateWorkspace` against the backend (`WorkspaceEditorDialog.test.tsx`); stored permissions apply via ACP `on_get_tools` (`acp/server/tools.rs:24-39`). Desktop mode list is display-only (INV-GSL-001). Not a sole-enforcement UI rule. |
| ARC-011 Collector Executes | **N/A — role absent** — no passive collector/scanner component that is declared observe-only. `PromptInjectionScanner` is an active inspector, not a collector. |
| ARC-012 Optional Integration Hard Dependency | **Finding-adjacent** ARC-GSL-002 (providers crate not optional). Goose remote catalog is truly optional (empty → `[]`). `system-keyring` has a file fallback (`base.rs:518-522`). |
| ARC-013 Frozen Surface Drift | **Finding** INV-GSL-001 / ARC-GSL-006 (accidental frozen internals + hand desktop types). `acp-schema.json` generator exists (`bin/generate_acp_schema.rs`); regen-diff **not run**. |
| ARC-014 Cross-Layer Mutation | **Non-finding (sampled)** — ACP mutates via `SessionManager` / `Agent`, not SQL from the transport. Desktop does not write `sessions.db`. |
| ARC-015 Overbuilt Compatibility | **Non-finding (sanctioned)** for Goose docs fallback (`GOOSE_COMPATIBILITY.md`). **Finding** ARC-GSL-005 only for the undocumented JS+TS twin. Session `import_formats/` + `legacy.rs` **Not Reviewed** for dead-weight (deadcode lens). |
| ARC-016 Shared Data Store Without Owner | **Non-finding** — `sessions.db` writes go through `SessionStorage`; `permission.yaml` through `PermissionManager`; `config.yaml` through `Config`. Multiple *callers*, one owner each. |
| ARC-017 Implicit Event Contract | **Non-finding (sampled)** — `AgentEvent` is a typed Rust enum (`agent.rs:345-350`). MCP `ServerNotification` is rmcp-typed. Desktop ACP notifications use generated/custom request types. No undeclared bus. Residual: tool notification payload fields — IAPI lens. |
| ARC-018 Ambient Config Coupling | **Finding** ARC-GSL-003 (`Config::global` + scattered `GOSLING_*` / `SECURITY_*` / `OPENAI_*` reads). Confirm signal: ≫3 sites, process-global defaults. |
| ARC-019 Initialization-Order Coupling | **Likely / folded into ARC-GSL-003** — `global()` first caller wins the leaked `Config` for that `config_dir`. `Agent::new` vs tests depend on who initialized Paths. Not a classic import-time cycle. |
| ARC-020 Untestable Core | **Finding** ARC-GSL-001 — `Agent::new` performs IO-capable singleton init. `with_config` is the test seam and is used; default path is not IO-free. |
| ARC-021 Vendor Type Leakage | **Likely, not separately raised** — `rmcp::model::{Tool, CallToolRequestParams, CallToolResult}` appear throughout `Agent`, `ExtensionManager`, session import. Anti-corruption is incomplete. Cap: inferred (no ADR forbids rmcp in domain). Route remainder to IAPI. |
| ARC-022 Duplicate Parallel Abstraction | **Finding** ARC-GSL-002 (two Provider traits) + ARC-GSL-005 (two goose-compat modules). |
| ARC-023 Reflection/Monkey-Patch | **N/A — role absent** — no `monkeypatch` / `type(x)=` / runtime trait object replacement across crates found. Rust has no monkey-patch idiom in this tree. |
| ARC-024 Generated-Surface Hand Edit | **Not Confirmed** — `ui/sdk/src/generated/*.gen.ts` marked `@hey-api/openapi-ts`; no hand-edit inside generated files was spotted. Desktop hand types are *beside* generated output, not edits of it (INV-GSL-001). Regen-diff not run. |
| ARC-025 Cross-Cutting Reimplemented | **Finding** INV-GSL-004 (`is_shell_tool*`) + permission option lists in CLI/UI (aligned today). Logging/authz not fully sampled. |

### INV-001..015

| Code | Result |
|---|---|
| INV-001 Replicated Membership Set | **Finding** INV-GSL-001, INV-GSL-003, INV-GSL-004, ARC-GSL-005 |
| INV-002 Canonical Source Bypassed | **Finding** INV-GSL-001 — generated SDK exists; desktop re-declares most enums. Exception: `ToolPermissionLevel` *is* consumed from the SDK. |
| INV-003 Schema/Code Inventory Drift | **Not Reviewed** — `CURRENT_SCHEMA_VERSION = 26` (`session_manager.rs:33`) vs SQL in `SessionStorage`; no field-by-field diff this pass. |
| INV-004 Enum/Constraint/UI Value Drift | **Finding** INV-GSL-001 (no UI constraint); stored vs confirmation permission are **different enums** (see INV-015). |
| INV-005 Serialize/Deserialize Asymmetry | **Non-finding (sampled)** — session export is `serde_json::to_string_pretty(&session)` and import `from_str` after format convert (`session_manager.rs:4787-4803`). Residual: foreign import formats (`import_formats/`) not field-diffed. |
| INV-006 Export/Import Schema Mismatch | **Non-finding (sampled)** — import strips enabled-extension state (`:4809-4812`), stamps provenance, marks messages untrusted. Deliberate narrowing, not a silent drop of the conversation body. |
| INV-007 Drift Guard Missing | **Finding** INV-GSL-001, INV-GSL-003, INV-GSL-004 |
| INV-008 Handling Class Omitted | **Non-finding** — plugin discovery states handling (disable vs trust). Permission lists encode handling (`AlwaysAllow`/`AskBefore`/`NeverAllow`). Shell-tool copies do **not** encode handling (INV-GSL-004). |
| INV-009 Unenforced Must-Match | **Finding** INV-GSL-001 |
| INV-010 Silent Add-Site Gap | **Finding** INV-GSL-001 / INV-GSL-003 / INV-GSL-004 |
| INV-011 Authoritative Copy Not Consumed | **Finding** INV-GSL-002 (enum not used as the request field); INV-GSL-001 (generated SDK not used for session/message types) |
| INV-012 Permission/Guard Table Drift | **Non-finding today** — UI modal (`PermissionModal.tsx:93-95`) and CLI (`configure.rs:1739-1767`) list `always_allow|ask_before|never_allow`, matching `PermissionLevel` / generated `ToolPermissionLevel`. No extra UI-only action. Confirmation vocabulary is a different fact (INV-015). |
| INV-013 Migration/Model Drift | **Not Reviewed** — session schema v26 migrations live in `SessionStorage`; not diffed. |
| INV-014 Narrowed Subset Copy | **Non-finding (classified)** — import removes enabled-extension snapshot (handling: skip/re-resolve). Smart-approve cache is a subset of tools the user has seen. |
| INV-015 Divergence Class Unclassified | **Non-finding after classification** — `Permission` (one-shot: allow_once/always_deny/…) vs `PermissionLevel` (stored: ask_before/never_allow) vs `PermissionDecision` (ACP: AllowAlways/RejectOnce) are **intentionally different** with `From` impls + tests (`acp/common.rs:31-53`). Desktop maps `always_allow` → ACP `allow_always` (`permissionRequests.ts:97-109`). Documented by type names; not drift. |

### NEG-001..015

| Code | Result |
|---|---|
| NEG-001 Impossible State Possible | **Finding** NEG-GSL-001 — parent Approve + child Auto is a reachable "impossible" (child more privileged than parent). |
| NEG-002 Hidden Actor | **Finding** NEG-GSL-001 — subagent/orchestrator; also `gosling serve` + CLI on one `PermissionManager` (A6, not separately raised). |
| NEG-003 Unmodeled Input | **Finding** NEG-GSL-002 (repo files), NEG-GSL-003 (shell env), Goose live JSON (docs). |
| NEG-004 Cross-Boundary Composition | **Finding** NEG-GSL-002 (+ Auto child). Plugin trust × skills is the composition. |
| NEG-005 Assumption Collapse | **Finding** A1 / NEG-GSL-001 |
| NEG-006 Rare Timing Window | **Not Confirmed** — confirmation `request_id` overwrite still a robustness issue (July); ids are gosling-generated. Not re-traced. |
| NEG-007 Catastrophic Low Probability | **Not Confirmed** — no new unrecoverable path beyond ordinary tool exec. Session import is transactional (`session_manager.rs:4837-4839` comment). |
| NEG-008 Negative Test Missing | **Finding-adjacent** NEG-GSL-002 — plugin trust has tests (`discovery.rs:376+`); no equivalent "untrusted project skill must not load" test was found in `skills/mod.rs` discovery. |
| NEG-009 Safety Bypassed By Alternate Path | **Finding** NEG-GSL-001 (summon/orchestrator vs parent mode). ACP `AcpProvider` Auto→AllowOnce is another path (ARC-GSL-004). |
| NEG-010 Human/Operator Misuse | **Plausible** — `gosling plugin trust` is explicit; operators may believe that covers skills. Folded into NEG-GSL-002. |
| NEG-011 Model/Provider Output Trusted | **Likely / stub to security-llm** — tool names/args are model-authored; inspectors exist. Auto trusts them by design. Not re-litigated beyond Auto. |
| NEG-012 Future Integration Breaks Invariant | **Speculative** — public multi-user on `gosling serve` would share Config + PermissionManager (A6). Enabling code (`gosling-server`) exists; no authz between actors. Cap Speculative for full multi-user; **Plausible** for "two local processes." |
| NEG-013 Local-First Assumption Fails | **Plausible** — desktop + `gosling serve` + CLI are already multiple surfaces on one store. Not confirmed as an exploit. Escalate to security. |
| NEG-014 Compliance Language Over-Trusted | **Non-finding vs July** — `SECURITY.md` is cautionary, not a certification claim. Scanner default is now **on** (`security/mod.rs:52-54`), closer to the doc than July. Residual: "foundational models include baseline protections" is vendor language, not a gosling control. |
| NEG-015 Recovery Mechanism Causes Damage | **Non-finding** — subagent unapprovable requests are denied (`agent.rs:911-929`), not auto-allowed. Import refuses duplicate/changed source (`session_manager.rs:45-52,962-967`). Plugin "restore" does not re-trust from repo settings (`discovery.rs:114-117`). |

---

## Non-Findings / Checked But Not Confirmed

- **`read_only_hint` cannot grant Auto-exec.** `PermissionManager::apply_tool_annotations` (`config/permission.rs:142-160`) only records `read_only_hint == Some(false)` as SmartApprove `AskBefore`. Test `hostile_read_only_hint_does_not_bypass_approval` (`permission_inspector.rs:400-428`). July NEG-GSL-002 is **repaired**.
- **Scanner is not name-`shell`-only.** `is_shell_tool_name` + `should_scan_tool_call` (`scanner.rs:518-543`). July NEG-GSL-001 is **mostly repaired**; residual is INV-GSL-004.
- **Prompt-injection scanner defaults on.** `SECURITY_PROMPT_ENABLED` `unwrap_or(true)` (`security/mod.rs:52-54`). July A4 is **stale**.
- **Auto honors explicit user permissions.** NeverAllow denies; AskBefore denies rather than hangs (`permission_inspector.rs:133-160,312-365`).
- **Project plugins cannot self-trust.** `discovery.rs:114-117,169-175` + tests around `:376`.
- **Imported history is not treated as local approval.** `imported_untrusted` (`message.rs:685-687,776-777`); replay prompt (`acp/server.rs:1404-1408`); artifact backfill skips untrusted (`session_manager.rs:4226` grep hit).
- **Working-dir restrict refuses out-of-band CLI providers.** `ensure_provider_ready` (`agent.rs:1723-1730`).
- **ACP permission option mapping degrades toward less authority.** `map_permission_response` (`acp/common.rs:67-80`); unknown option id → Cancel (`:60`).
- **No crate-level import cycle** (ARC-005).
- **Desktop does not fetch Goose catalogs** (product path is CLI `--from-goose` + docs site only).
- **Declarative provider JSON is single-sourced** via `include_dir!`.
- **Workspace UI does not own validation** (ARC-010 held, sampled).
- **`sessions.db` has a single write owner** (`SessionStorage`).

---

## Break-It Review

| Probe (static) | Result |
|---|---|
| Parent Approve + `summon` shell | Child Auto-allows unmarked tools (NEG-GSL-001) |
| Untrusted `.agents/plugins` | Held — not enabled without `trust_project` |
| Untrusted `.agents/skills` + `AGENTS.md` | Loads (NEG-GSL-002) |
| Import transcript into that cwd | History untrusted; cwd skills/hints still live (composition) |
| MCP `readOnlyHint: true` | Held — cannot auto-allow |
| Command tool named `automation_script` | Scanner likely sees `script` key; cwd/egress predicates disagree (INV-GSL-004) |
| Swap HTTP provider for `AcpProvider` | Flattened; tools/permissions diverge (ARC-GSL-004) |
| Add `GoslingMode` variant, skip TS | Compiles; mode picker omits it (INV-GSL-001) |
| `provider: "local"` dictation | Doc allows; enum/handler reject (INV-GSL-002) |
| Remove `gosling-providers` crate | Domain types disappear (ARC-GSL-002) |
| Two processes (CLI + serve) on one Config | Shared YAML/keyring (NEG-013 Plausible) |
| `GOSLING_SHELL=nu` | Scanner still POSIX (NEG-GSL-003) |
| Forge ACP permission option id | Cancel (held) |
| Duplicate confirmation `request_id` | Not re-traced (NEG-006 Not Reviewed) |

---

## Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| NEG-GSL-001 | Negative-Space | Security / agent-orchestration / Workflow-GUI | Hidden actor + approval UX lies if UI implies parent mode applies to children |
| NEG-GSL-002 | Negative-Space | Security-LLM / Security | Repo instruction injection; prompt-injection from files |
| ARC-GSL-004 | Architecture | Security | Subprocess tool loop outside inspectors |
| ARC-GSL-003 | Architecture | Concurrency / Security | Global secret cache |
| ARC-GSL-001 | Architecture | Concurrency | Multi-mutex god objects |
| INV-GSL-001 | Invariant Sync | Contract-Internal-API / Workflow-GUI | Wire contract + UI truth |
| INV-GSL-004 | Invariant Sync | Security | Command-tool membership is a security set |
| ARC-GSL-002/006 | Architecture | IAPI / deadcode | Facade + extraction |
| NEG-013 / A6 | Negative-Space | Security | Multi-surface local store |
| Vendor rmcp types | ARC-021 stub | IAPI | Domain signatures |
| Session SQL vs `Session` | INV-003 Not Reviewed | Data Integrity | Schema drift |
| `import_formats/` / `legacy.rs` | ARC-015 Not Reviewed | deadcode | Shim sunset |

---

## Recommended Patch Order

1. **NEG-GSL-001** — inherit parent mode or deny unmarked tools in SubAgent; then forward confirmations.
2. **NEG-GSL-002** — one trust bit for project skills + context files.
3. **INV-GSL-004** — unify command-tool predicate (cheap, security-adjacent).
4. **INV-GSL-001** — desktop enum parity test.
5. **ARC-GSL-004** — type-level delegated-agent kind (after 1, so Auto mapping is honest).
6. **INV-GSL-002 / INV-GSL-003 / ARC-GSL-005** — small drift guards.
7. **ARC-GSL-001 / 002 / 003 / 006** — structural extracts; do not block 1–4.

---

## Regression Test Strategy

| Test | Purpose | Finding |
|---|---|---|
| Parent Approve + summon `shell` → no silent exec | Authority follows delegation | NEG-GSL-001 |
| Existing Auto NeverAllow/AskBefore tests remain | Don't regress July repair | NEG-GSL-001 |
| Untrusted project `SKILL.md` not prompt-injected | Trust bit covers skills | NEG-GSL-002 |
| Plugin trust tests still pass | Don't break SEC-GSL-101 | NEG-GSL-002 |
| Shared fixture list classified equally by scanner/egress/cwd/adversary | Predicate unity | INV-GSL-004 |
| Generated SDK enums == desktop hand unions | Drift guard | INV-GSL-001 |
| `DictationProvider` rejects `local` | Doc/type match | INV-GSL-002 |
| Every `COMMANDS` name has a dedicated arm | Dispatch sync | INV-GSL-003 |
| JS vs TS Goose exclude sets | Dual-stack sync | ARC-GSL-005 |
| `Agent::with_config` constructs with fakes only | Untestable-core guard | ARC-GSL-001 |

---

## Deferred Risks

- Full multi-user authz on `gosling serve` (NEG-012/013) — product policy.
- `Config::global` injection (ARC-GSL-003) — XL, incremental.
- Crate split for conversation types (ARC-GSL-002) — depends on facade (ARC-GSL-006).
- Session schema v26 vs SQL (INV-003/013) — needs a dedicated integrity pass.
- Regen-diff of `ui/sdk/src/generated` vs `acp-schema.json` (ARC-024).
- Confirmation-router duplicate ids (NEG-006).
- Electron renderer domain rules beyond workspace/permissions (workflow-gui / architecture-nodejs).

---

## Validation Limits

- **No execution.** No `cargo test`, no playtest, no generator run. Runtime consequences (whether a given ACP CLI ignores `tools`, whether a live Goose catalog is empty) stay **Likely** per `confidence_calibration.md`.
- **No graph tool.** Cycle non-finding is import-direction grep + crate manifests, not `cargo-modules`. Intra-`gosling` file cycles **Not Reviewed**.
- **No shell** in this agent — `git status --short` was not run. Only this report path should have been written.
- **Oracle integrity:** not applicable as a passing-suite claim; no suite was used as evidence.
- **Not Reviewed (ranked leftover):** full `acp/provider.rs` (~2k LOC beyond mode mapping); every provider impl; `session/legacy.rs` + all `import_formats/*`; `config/migrations.rs`; Ink `ui/text`; Electron main/preload (nodejs lens); `tagteam/`; OAuth callback as hidden actor; `large_response_handler.rs`; full `custom_requests.rs` ↔ `types.gen.ts` field diff; SQL schema vs `Session` fields.
- **Generated/vendored excluded from god-object ranking:** `ui/sdk/src/generated/*`, `vendor/v8`.
- **Dynamic wiring:** MCP tools and provider inventory are runtime registries. "No edge found" does not clear those seams; dependent non-findings are capped.

---

## Final Confidence

**Medium-High** on structural and permission/subagent claims (quoted current source). **Medium** on composition exploitability (not executed). **Low** on session SQL drift and generated-SDK freshness (not diffed).

Historical July findings that are **no longer current:** NEG-GSL-001 scanner-name-only (repaired), NEG-GSL-002 `read_only_hint` auto-allow (repaired), scanner-default-off (now on), plugin auto-enable (repaired). Do not merge those IDs forward without the new IDs in this report.
