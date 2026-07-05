# Gosling Audit — Architecture & Seam Lens

Lens: `audit-architecture-seam` (domain ARC). Authority: **audit-only / read-only**
(only this file written). Builds on `docs/cloud/00-orientation.md`. Method:
`audit_method.md v3.0` + v3.1 calibration addendum. Evidence discipline per
`evidence_discipline.md` — Confirmed requires a quoted `file:line` actually read.

Effort budget: ~30–45 tool calls; **~22 used**. Sampling was boundary-first:
crate dependency edges, the provider abstraction, the global-config seam, the
top-5 largest core modules, and the Rust↔CLI↔server↔Electron seam. Everything
else is `Not Reviewed` (see Validation Limits) — absence of review is reported,
not a clean bill.

---

## 1. Seam / boundary inventory

Crate graph (workspace, `crates/*/Cargo.toml`), edges are `path` deps:

```
gosling-sdk-types      (leaf, shared wire types)
gosling-acp-macros     (leaf, proc-macros)
gosling-providers  ->  gosling-sdk-types            (leaf-ish; NO gosling dep)
gosling-mcp            (leaf; NO gosling dep)
gosling            ->  gosling-providers, gosling-sdk-types, gosling-acp-macros
                       (dev-only: gosling-mcp, gosling-test-support)
gosling-cli        ->  gosling, gosling-mcp, gosling-providers
gosling-server     ->  gosling, gosling-mcp, gosling-providers
ui/desktop (Electron)  --ACP-over-HTTP-->  goslingd (gosling-server)
```

Direction check: all crate edges point toward `gosling-providers` /
`gosling-sdk-types` (would-be leaves). **No crate-level cycle.** But the *domain*
does not sit at the bottom — the provider crate does (see ARC-GSL-001).

| Module | Responsibility | Owns | Depends on | Boundary contract | Abstraction fidelity | Coupling risk |
|---|---|---|---|---|---|---|
| `gosling-providers` (crate) | **provider trait + core conversation domain types** | `Provider` trait, `Message`, `Conversation`, `Usage`, canonical model registry | sdk-types | crate boundary | mixed — carries domain, not just providers | **High** (mislabeled as leaf) |
| `gosling` core (46 `pub mod`) | agent loop, session, config, security, permission, ~21 providers, acp | almost everything | providers crate | none — all modules `pub` | low (no facade) | **High** |
| `agents/agent.rs` (3858 LOC) | orchestration hub | Agent struct (18 fields) | 15 distinct crate modules | struct-internal | orchestrator (justified fan-out) but state-heavy | **High** |
| `config/base.rs` (2698 LOC) | params + secrets + keyring + yaml + workspace | `Config` global singleton | keyring, fs | `Config::global()` | mechanism+store mixed | **High** (256 call sites) |
| `session/session_manager.rs` (3727 LOC) | session CRUD/import/export/naming/search | sessions | storage, provider (naming) | builder + `instance()` | mostly cohesive | Medium |
| `acp/*` (server 3923 + provider.rs ~2000 LOC) | ACP front-end + ACP-subprocess bridge | ACP sessions | agents (one-way) | wraps `Agent` | facade over agents; `AcpProvider` flattens | Medium/High |
| CLI / server | front-ends | entry points | reach into ~13–18 core modules each | direct module access | none | **High** |

---

## 2. Findings

### ARC-GSL-001: The "providers" crate owns the core conversation domain (inverted ownership / mislabeled boundary)

Severity: High
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `crates/gosling-providers/src/conversation/message.rs:771` — `pub struct Message`
  (the central conversation type) is defined **in the providers crate**, alongside
  `token_usage.rs`, `tool_result_serde.rs`, and the canonical model registry.
- `crates/gosling/src/lib.rs:12-14` — the core crate does not own `conversation`;
  it re-exports it: `pub mod conversation { pub use gosling_providers::conversation::*; }`.
- Consumers reach **past** the core re-export straight into the provider crate:
  `crates/gosling-cli/src/session/mod.rs`, `crates/gosling/src/agents/agent.rs`,
  `crates/gosling/src/session/session_manager.rs`, `crates/gosling/src/context_mgmt/mod.rs`
  all `use gosling_providers::conversation…` directly (32 core sites + CLI/server).

Observed behavior:
- The domain nucleus (`Message`, `Conversation`, `Usage`) lives in the crate named
  `gosling-providers`, which the whole workspace treats as a leaf.

Expected boundary:
- Dependencies point toward the domain. Core conversation types are the domain and
  should live in a domain/core crate (or `gosling` itself); the provider layer should
  depend on them, not define them.

Failure mechanism:
- The extraction that carved out `gosling-providers` pulled the shared conversation
  types down with the provider code (they are tightly used together), so the crate
  became "provider adapters + domain model" under a provider name.

Break-it angle:
- Try to swap or delete the provider layer to build a "lighter goose": you cannot,
  because deleting `gosling-providers` also deletes `Message`/`Conversation` and the
  canonical model registry that session, context_mgmt, and the ACP layer all depend on.
  The name promises an optional adapter; the contents are load-bearing domain.

Impact:
- A remixer cannot reason about layering from crate names; the provider seam is not
  actually separable from the domain. Refactors to provider code risk the domain model.

Operational impact:
- Blast radius: Repo
- Side-effect class: none (structural)
- Reversibility: compensatable (move types to a `gosling-core`/domain crate)
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes:
- ARC-GSL-002 (partial extraction), ARC-GSL-006 (no facade so the leak is visible everywhere).

Recommended mitigation:
- Remediation pattern: extract domain crate. Move `conversation`, `token_usage`,
  `canonical`, `model` into a `gosling-domain` (or fold back into `gosling`); make
  `gosling-providers` depend on it and keep only provider adapters.
- Local guardrail: a workspace lint / `cargo-deny`-style check that
  `gosling-providers` exports no type re-used as domain identity.
- Behavior test: assert `gosling-providers` compiles with only provider adapters and
  no `struct Message`.

Implementation assessment:
- Complexity: workflow_protocol (crate reshaping)
- Cost: L
- Cost drivers: modules, many import sites (32+), Cargo.lock
- Nominal implementation agent: claude
- Rationale: mechanical but wide; touches every `gosling_providers::conversation` import.

Validation:
- After the move, `rg "gosling_providers::conversation"` returns only provider-internal hits.

Non-goals:
- Do not rename the provider crate or merge providers back into core in this slice.

---

### ARC-GSL-002: Provider extraction is partial — two provider traits, 21 concrete impls left in core, re-export shims blur the boundary

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- Two traits model "a provider": `crates/gosling-providers/src/base.rs:395` `pub trait Provider`
  (runtime streaming) and `crates/gosling/src/providers/base.rs:30`
  `pub trait ProviderDef: ProviderDescriptor` (env construction) — the second lives in
  a *different crate* from the first.
- `crates/gosling/src/providers/base.rs:16` `pub use gosling_providers::base::*;` — core
  re-exports the crate's provider surface and adds to it.
- Concrete impls are split across the boundary: 4 `impl Provider for …` in
  `gosling-providers/src/` (anthropic/openai/ollama/openai_compatible) vs **21** in
  `crates/gosling/src/providers/` (bedrock, databricks, google, codex, chatgpt_codex,
  claude_code, gcpvertexai, azure, litellm, …).
- `crates/gosling/src/providers/mod.rs:3-8,17-19,50-58` — modules are re-export shims:
  `pub mod anthropic { pub use gosling_providers::anthropic::*; }`,
  `pub mod ollama { pub use gosling_providers::ollama::*; }`,
  `pub mod http_status { … }`, etc., interleaved with real core modules.

Observed behavior:
- Which crate a provider lives in appears arbitrary (anthropic/openai extracted;
  bedrock/databricks/google/codex left behind), and the two-trait split means the
  construction contract (`ProviderDef`) and runtime contract (`Provider`) live on
  opposite sides of a crate boundary joined by `pub use` shims.

Expected boundary:
- A provider abstraction should be one coherent contract in one place; a crate split
  should carve a clean subset, not a partial one stitched with re-exports.

Failure mechanism:
- The extraction stopped partway: only providers with no core-crate dependencies moved;
  the rest depend on `crate::config::ExtensionConfig` / core auth and could not follow,
  so `ProviderDef` (which references `ExtensionConfig`, `base.rs:11,33`) had to stay in core.

Break-it angle:
- Add a new provider: an implementer must satisfy `ProviderDef` (core) + `Provider`
  (crate) + `ProviderDescriptor` (crate) and decide which crate it belongs to based on
  whether it needs `ExtensionConfig` — an implementation detail, not a design axis.

Impact:
- High cognitive load for the "easy-to-remix" goal; the provider seam is not a clean
  plug point. Re-export shims hide where a symbol truly lives.

Operational impact:
- Blast radius: Repo
- Side-effect class: none (structural)
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes: ARC-GSL-001, ARC-GSL-004.

Recommended mitigation:
- Remediation pattern: consolidate the abstraction. Either move all provider impls +
  both traits into `gosling-providers` (requires ARC-GSL-001's domain extraction so
  `ExtensionConfig`/config lives below providers), or keep all providers in core and
  reduce `gosling-providers` to shared wire/format helpers. Pick one side.
- Behavior test: assert a single crate defines the full provider contract (grep: no
  `pub trait *Provider*` split across crates).

Implementation assessment:
- Complexity: workflow_protocol
- Cost: L
- Cost drivers: modules, 25 impls, feature flags (`aws-providers`)
- Nominal implementation agent: claude
- Rationale: depends on ARC-GSL-001; broad but mechanical.

Non-goals: do not change provider runtime behavior.

---

### ARC-GSL-003: Global mutable config singleton (`Config::global()`) is pervasive hidden coupling

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `crates/gosling/src/config/base.rs:200` `static GLOBAL_CONFIG: OnceCell<Config>` and
  `:447` `pub fn global() -> &'static Config { GLOBAL_CONFIG.get_or_init(Config::default) }`.
- `Config` holds interior-mutable caches: `base.rs:157-159` `guard: Mutex<()>`,
  `secrets_cache: Arc<Mutex<Option<…>>>`, `param_cache: Mutex<Option<ConfigSnapshot>>`.
- **256** call sites of `Config::global()`: 193 in `crates/gosling`, 50 in
  `crates/gosling-cli`, 13 in `crates/gosling-server` (grep count).
- The same module also owns secrets/keyring: `base.rs:925 get_secret`, `:1015 set_secret`,
  `:1052 delete_secret`, `keyring` referenced 86×.

Observed behavior:
- Any module, at any layer, reads/writes process-global config + secrets by calling a
  static singleton, with no injected dependency.

Expected boundary:
- Configuration and secret access should be an injected capability (a handle passed to
  constructors), so the dependency is explicit and testable/substitutable.

Failure mechanism:
- Convenience singleton pattern inherited from upstream; every consumer reaches the
  global directly rather than receiving a config handle.

Break-it angle:
- Two tests (or two agent instances) sharing one process share one mutable global
  config + secrets cache; test isolation and any future multi-tenant/embedded use are
  compromised. A remixer cannot swap config sources without touching 256 sites.

Impact:
- Untracked coupling across the whole tree; the config module + its secret store are a
  hub every layer depends on invisibly. Blocks the "easy-to-remix / lighter" goal.

Operational impact:
- Blast radius: Repo
- Side-effect class: file (yaml) / process (keyring), user-visible via secrets
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes: concurrency (shared cache), ARC-GSL-006.

Recommended mitigation:
- Remediation pattern: dependency injection at the seams. Pass an `Arc<Config>` into
  `Agent`, `SessionManager`, ACP server constructors; keep `global()` only as the CLI
  composition-root default. Split secret access into its own capability from param access.
- Behavior test: a test constructs an `Agent` with an isolated `Config` and asserts it
  never reads `GLOBAL_CONFIG`.

Implementation assessment:
- Complexity: cross_process_coordination (wide threading of a handle)
- Cost: XL
- Cost drivers: 256 sites, tests
- Nominal implementation agent: claude
- Rationale: very wide; better done incrementally at high-value seams first.

Non-goals: do not change the on-disk config/secret format.

---

### ARC-GSL-004: ACP subprocess agents are flattened behind the stateless `Provider` trait

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `crates/gosling/src/acp/provider.rs:428` `impl Provider for AcpProvider` — a bridge that
  spawns/connects to an external CLI agent (`use std::process::Stdio`, JSON-RPC over ACP,
  `AcpProvider::connect`) implements the **same** `Provider` trait as HTTP providers.
- `crates/gosling/src/providers/claude_acp.rs:41-42` `impl ProviderDef … type Provider = AcpProvider;`
  (same for `codex_acp.rs:40`, `copilot_acp.rs:44`).
- The flattened contract: `Provider::stream(model, system, messages, tools)`
  (`gosling-providers/src/base.rs:412`) assumes the *caller* owns the conversation and
  tool set. But `AcpProvider` construction takes `extension_configs_to_mcp_servers` and
  `gosling_mode` (`claude_acp.rs:7,75-91`) — the ACP agent runs its **own** session,
  permission prompts, and tool loop.
- Adaptation cost signal: `acp/provider.rs` is ~2000 LOC to make one "provider" fit,
  including its own `get_context_limit` override capturing a remote session's context size
  (`:433`, tested at `:1737`).

Observed behavior:
- A subprocess agent that maintains its own session/tools/permissions is presented to the
  core as an interchangeable completion `Provider`.

Expected boundary:
- Per `audit-architecture-seam` ARC-008: capability differences (stateful session vs
  stateless completion, self-owned tool loop vs caller-owned) should be modeled, not
  flattened into a false-uniform `stream(messages, tools)`.

Failure mechanism:
- To reuse the agent loop, ACP CLIs were adapted to the existing `Provider` trait rather
  than given a distinct "delegated agent" seam.

Break-it angle:
- Swap an HTTP provider for an ACP one behind the uniform interface: `tools`/`system`
  passed by the caller may be re-derived or ignored by the external agent, and the
  external agent's own permission prompts run outside the core `tool_confirmation_router`
  — a security-relevant divergence hidden by the shared trait (escalate to Security).

Impact:
- Divergent execution/permission semantics are indistinguishable at the call site; the
  abstraction hides that one "provider" runs its own tool-executing agent.

Operational impact:
- Blast radius: Workflow (per-session), Security-adjacent
- Side-effect class: process (subprocess), network
- Reversibility: n/a (design)
- Operator visibility: log-only
- Rerun safety: unknown

Adjacent failure modes: permission bypass via delegated agent (SEC lens), ARC-GSL-002.

Recommended mitigation:
- Remediation pattern: model the capability. Introduce a `DelegatedAgent` seam (or a
  capability flag `owns_session/owns_tool_loop` on the metadata) distinct from stateless
  `Provider`; route permission/tool events through the core router explicitly.
- Behavior test: assert an ACP provider's tool executions surface through the core
  `tool_confirmation_router`, not only the subprocess.

Implementation assessment:
- Complexity: external_service_semantics
- Cost: L
- Cost drivers: modules, runtime_verification, security review
- Nominal implementation agent: multi-agent
- Rationale: crosses architecture + security + reliability.

Non-goals: do not rewrite the ACP transport.

---

### ARC-GSL-005: `agents/agent.rs` is a god orchestrator (3858 LOC, 18-field mutable struct, reaches 15 modules)

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `crates/gosling/src/agents/agent.rs` — 3858 LOC, 47 `pub fn`; the `Agent` struct
  (`:228-251`) carries 18 fields spanning unrelated responsibility classes: `provider`,
  `extension_manager`, `frontend_extensions/tools/instructions`, `prompt_manager`,
  `subdirectory_hint_tracker`, `tool_confirmation_router`, tool-result channels,
  `tool_inspection_manager`, `hook_manager`, plus loose session state `container`,
  `goal`, `grind`, `pending_steers` (each its own `Mutex`).
- Fan-out: `agent.rs` imports 15 distinct `crate::` subsystems (agents, permission,
  session, security, config, providers, conversation, tool_monitor, tool_inspection,
  plugins, mcp_utils, hints, context_mgmt, action_required_manager, utils) — grep of
  `use crate::` prefixes.

Observed behavior:
- One file mixes provider streaming, tool execution wiring, hook dispatch, extension
  management, prompt assembly, permission routing, and ad-hoc conversational state
  (goal/grind/steers) in a single struct with 4 independent inner mutexes.

Expected boundary:
- An orchestrator may fan out, but per ARC-001 it should delegate to owned submodules,
  not host mutable domain state (`goal`, `grind`, `pending_steers`) and every subsystem's
  wiring in one type.

Failure mechanism:
- Growth accretion: the agent loop is the natural attractor; features were added as
  fields/methods rather than extracted collaborators.

Break-it angle:
- Any change to steering, goals, or tool wiring forces editing the same 3858-LOC file
  under lock-ordering constraints (4 mutexes) — merge-conflict and deadlock surface for
  the "easy-to-remix" goal.

Impact:
- High change-coupling; the single most central file resists safe modification.

Operational impact:
- Blast radius: Repo
- Side-effect class: none (structural)
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes: concurrency (4 inner mutexes / lock ordering), ARC-GSL-003.

Recommended mitigation:
- Remediation pattern: extract collaborators. Move conversational state (goal/grind/
  pending_steers) into a `SteeringState` type; split provider-turn execution from
  extension/tool wiring. Note `impl Agent` is already partly split
  (`execute_commands.rs`, `tool_execution.rs`, `reply_parts.rs`) — continue that.
- Behavior test: none per-se; guardrail is a size/responsibility budget check.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: L
- Cost drivers: modules, tests, concurrency care
- Nominal implementation agent: claude
- Rationale: broad context-heavy refactor of the hottest file.

Non-goals: do not change agent-loop semantics.

---

### ARC-GSL-006: No crate-level public API — all 46 core modules are `pub`, so CLI/server/desktop couple to internals

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Architecture

Evidence:
- `crates/gosling/src/lib.rs:4-46` — 46 top-level `pub mod` declarations; effectively the
  entire internal module tree is the crate's public surface (no `pub(crate)` facade, no
  curated `pub use` API root).
- Front-ends reach broadly into internals: CLI imports 18 distinct `gosling::*` modules
  (`config` 28×, `agents` 18×, `providers` 12×, `conversation` 12×, `session` 11×,
  `acp`, `permission`, `skills`, `subprocess`, `source_roots`, `model_config`, …);
  server imports 13 distinct modules (grep of `use gosling::` prefixes).

Observed behavior:
- Every consumer binds directly to the core's internal module layout; there is no stable,
  narrow contract between `gosling` and its front-ends.

Expected boundary:
- A library consumed by multiple front-ends should expose a curated API and keep internal
  structure `pub(crate)`, so internals can be refactored without breaking consumers
  (frozen-surface stability, ARC-013 direction).

Failure mechanism:
- Fork-of-goose monolith: modules were made `pub` to satisfy CLI/server needs one at a
  time; no facade was ever introduced.

Break-it angle:
- Rename or restructure any core module (e.g. move `providers::` per ARC-GSL-002) and the
  CLI + server break, because they import the internal path directly. The "lighter,
  easy-to-remix" goal is blocked: internal layout is a frozen public contract by accident.

Impact:
- Refactor cost is repo-wide; no seam protects internals; the crate boundary carries no
  contract.

Operational impact:
- Blast radius: Repo
- Side-effect class: none (structural)
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: safe

Adjacent failure modes: ARC-GSL-001 (leak visible everywhere), ARC-GSL-002.

Recommended mitigation:
- Remediation pattern: introduce a facade. Add a curated `gosling::api` (or top-level
  `pub use`) surface; demote internal modules to `pub(crate)` incrementally, starting
  with the lowest-churn ones (`utils`, `subprocess`, `token_counter`).
- Behavior test: a compile-fence test in CLI/server that imports only the facade.

Implementation assessment:
- Complexity: workflow_protocol
- Cost: L
- Cost drivers: modules, CLI+server import churn
- Nominal implementation agent: claude
- Rationale: wide, coordinated with ARC-GSL-002.

Non-goals: do not hide modules the SDK/server genuinely need in this slice.

---

## 3. Inventory coverage (ARC-001..015)

| Code | Result | Basis |
|---|---|---|
| ARC-001 God Object | **Finding** ARC-GSL-005 (agent.rs); config/base.rs + session_manager.rs borderline (large but cohesive) | source |
| ARC-002 Boundary Violation | **Finding** ARC-GSL-001 (domain in provider crate) | source |
| ARC-003 Hidden Coupling | **Finding** ARC-GSL-003 (`Config::global` singleton) | source |
| ARC-004 Wrong Ownership | **Finding** ARC-GSL-001 (Message owned by providers crate) | source |
| ARC-005 Circular Dependency | **Non-finding** — no crate cycle; `acp→agents` one-way, `agents` does not import `acp` | source |
| ARC-006 Leaky Abstraction | **Finding** ARC-GSL-002 / ARC-GSL-006; provider-trait downcast leak = **non-finding** (downcasts are on error types only) | source |
| ARC-007 Fake Adapter | folded into ARC-GSL-004 (ACP bridge) | source |
| ARC-008 Provider Contract Flattening | **Finding** ARC-GSL-004 | source |
| ARC-009 Policy Mixed w/ Mechanism | **Minor / not raised as material** — `model_config.rs:66,150` special-cases the OpenAI provider by name; narrow | source |
| ARC-010 UI Owns Domain Rule | **Not Reviewed** (UI/desktop TS not deep-read) — deferred to workflow-gui/design-webapp lenses | — |
| ARC-011 Collector Executes | **N/A** — no passive-collector component in this framework | — |
| ARC-012 Optional Integration Hard Dep | partially ARC-GSL-001 (provider crate is not optional; carries domain) | source |
| ARC-013 Frozen Surface Drift | **Finding-adjacent** ARC-GSL-006 (internal layout is an accidental frozen contract) | source |
| ARC-014 Cross-Layer Mutation | **Non-finding (sampled)** — ACP server mutates via `Agent`/`SessionManager` methods, not direct persistence writes (`acp/server.rs` calls `agent.*`, `session.*`) | source |
| ARC-015 Overbuilt Compatibility | **Not Reviewed** — `session/legacy.rs`, `import_formats/`, `config/migrations.rs` exist but not audited for dead-weight | — |

## 4. Non-findings (checked and held)

- **No crate-level circular dependency.** `gosling-providers` and `gosling-mcp` declare no
  `gosling` dependency (`crates/gosling-providers/Cargo.toml`, `crates/gosling-mcp/Cargo.toml`);
  all edges point down. `acp` imports `agents` (8 files) but `agents/` contains no
  `use crate::acp` — one-way.
- **Provider trait not leaked via concrete-type switching.** The only `downcast*` calls in
  `providers/`, `agents/`, `acp/` are on error types (`ProviderError`, `ErrorData`,
  `StreamableHttpError`), e.g. `providers/google.rs:204`, `agents/extension_manager.rs:463`;
  no caller downcasts to a concrete provider. Callers use the `Provider` surface.
- **Desktop↔core seam is a clean ACP-over-HTTP boundary.** `ui/desktop` talks to the Rust
  core via the ACP HTTP endpoint with a token (`ui/desktop/src/backendStatus.ts`,
  `.../acp/url`, `startGoslingServe`); no `ui/desktop/src` file imports the forbidden
  generated `src/api` client (grep clean), honoring the AGENTS.md rule.
- **ACP server is a real facade, not a re-implemented loop.** `acp/server.rs` delegates to
  `Agent` (`:965 agent.provider()`, `:1103 agent.clone()`, `:1211 register_acp_session`)
  rather than duplicating the agent loop.

## 5. Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| ARC-GSL-004 | Architecture | **Security** | ACP subprocess agents run their own tool loop/permission prompts outside the core `tool_confirmation_router`; flattening hides the divergent permission path. |
| ARC-GSL-003 | Architecture | **Concurrency / Security** | Global mutable secrets+param cache shared across agents/tests in-process; also a secret-handling surface. |
| ARC-GSL-005 | Architecture | **Concurrency** | Four independent inner `Mutex`es on one struct → lock-ordering / deadlock surface. |
| ARC-GSL-001 / -002 / -006 | Architecture | **Contract-Internal-API** | Crate/module boundaries carry no contract; `gosling-sdk-types`↔consumers and internal module surface are the real producer/consumer seams. |

## 6. Validation Limits (Not Reviewed)

- **UI/desktop (TS/TSX ~75K LOC) not deep-read** beyond the core-seam check — ARC-010
  (UI owns domain rule) and ARC-014 in the renderer are out of scope here; defer to
  `audit-workflow-gui` / `audit-design-webapp`.
- **No static cycle tool run** (`cargo-modules` / `cargo tree --duplicates` not executed);
  cycle non-finding is from import-direction grep + Cargo manifests, not a graph tool —
  capped by that method (Confirmed at crate level; intra-module cycles within core not
  exhaustively traced).
- **ARC-015 overbuilt compatibility not assessed** — `session/legacy.rs`,
  `session/import_formats/`, `config/migrations.rs` inventoried but not diffed against live
  needs; recommend `audit-deadcode-cleanup`.
- **`config/base.rs` and `session_manager.rs` god-object judgment is by size + fn count +
  responsibility scan**, not a full read; rated borderline, not raised as separate findings.
- **Runtime consequences of ARC-GSL-004** (whether caller-supplied tools are actually
  ignored by a given ACP CLI) are `Confirmed` structurally but not runtime-reproduced;
  the security consequence is `Likely`, pending a live ACP-provider tool-call trace.
