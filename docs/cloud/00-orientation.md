# Gosling Audit — Shared Orientation & Surface Inventory

Produced by the audit lead pass (Phase 1 Orientation + Phase 2 Surface Inventory
of `audit_method.md v3.0`). Every lens report in this directory builds on this
document instead of re-deriving it. Authority for the whole engagement:
**audit-only / read-only**. No source files were modified; only reports under
`docs/cloud/` were written.

## 1. What gosling is

- A general-purpose **AI agent framework**, written in Rust, forked from
  `goose` v1.38 (see `README.md`, `UPSTREAM.md`). Ships as:
  - a **CLI** (`crates/gosling-cli`, entry `src/main.rs`),
  - an **Electron desktop app** (`ui/desktop`, entry `src/main.ts`),
  - a **server / API** (`crates/gosling-server`),
  - an **SDK** (`crates/gosling-sdk`, `crates/gosling-sdk-types`, `ui/sdk`),
  - a **text/TUI** front end (`ui/text`, Ink/React).
- Talks to **15+ LLM providers** (`crates/gosling-providers`, `crates/gosling/src/providers`)
  via API keys, OAuth, and **ACP** (Agent Client Protocol) subprocess bridges to
  Claude/ChatGPT/Gemini/Codex/Copilot/Cursor CLIs.
- Connects to **MCP extensions** (`crates/gosling-mcp`, `crates/gosling/src/agents/mcp_client.rs`)
  — spawns subprocesses / connects to remote MCP servers.
- Runs an **autonomous tool-using agent loop** with subagents
  (`crates/gosling/src/agents/`), permission gating, and prompt-injection /
  egress security inspection.

## 2. Scale (line counts, approx.)

| Component | LOC |
|---|---|
| `crates/gosling` (core) | ~123.7K Rust |
| `crates/gosling-providers` | ~23.7K Rust |
| `crates/gosling-cli` | ~20.1K Rust |
| `crates/gosling-mcp` | ~7.3K Rust |
| `crates/gosling-server` | ~6.1K Rust |
| other crates | ~5K Rust |
| `ui/**` (desktop/text/sdk) | ~75.4K TS/TSX |

Total is large enough that every lens **must** budget effort and sample explicitly
per `audit_method.md` §"Effort Budgeting And Stop Conditions". Absence of review is
a reported fact (Validation Limits), never a clean bill.

## 3. Core crate module map (`crates/gosling/src/`)

- `agents/` — agent loop, subagents, MCP client, tool execution, tool
  confirmation router, extension manager, **extension_malware_check.rs**,
  container, large_response_handler, platform_extensions.
- `security/` — `security_inspector.rs`, `adversary_inspector.rs`,
  `egress_inspector.rs`, `scanner.rs` (`PromptInjectionScanner`),
  `classification_client.rs`, `patterns.rs`. The app's own prompt-injection /
  data-exfil defenses.
- `permission/` — `permission_judge.rs`, `permission_inspector.rs`,
  `permission_store.rs`, mode-based tool gating.
- `execution/` — `manager.rs` (task/turn execution).
- `oauth/` — device/callback OAuth flows, `persist.rs`, `oauth_callback.html`.
- `providers/` — provider trait impls, auth (aws/gcp/azure/databricks),
  declarative providers, formats, inventory.
- `config/` — `base.rs`, `paths.rs`, `permission.rs`, `providers.rs`, `tls.rs`,
  `extensions.rs`, `migrations.rs`, `experiments.rs`, signup flows.
- `session/` — `session_manager.rs`, persistence, `chat_history_search.rs`,
  `import_formats/`, `nostr_share.rs`, `extension_data.rs`, `legacy.rs`.
- `context_mgmt/` — context-window management / truncation / summarization.
- `hooks/`, `plugins/`, `skills/`, `slash_commands/`, `prompts/`, `hints/`,
  `dictation/`, `checks/`, `otel/`, `tracing/`, `acp/`.

## 4. Trust boundaries & actors (for every lens)

- **User** (operator) → CLI flags, TUI, desktop UI, config files, slash commands.
- **LLM provider** (semi-trusted, remote) → model output = **untrusted input**;
  can emit tool calls, text, URLs, file paths.
- **MCP extensions / ACP subprocesses** (third-party code) → tool results are
  **attacker-influenceable content**; the framework spawns and trusts them.
- **Retrieved / tool / web / file content** entering the context window =
  **untrusted**; the data-vs-instructions boundary lives in `security/`.
- **Local machine** — the agent can run shell, edit files, spawn processes,
  make network calls. Blast radius = the user's workstation (per `SECURITY.md`).
- **On-disk state** — sessions, config, OAuth tokens/keyring, permission store,
  plugin/extension registries. Persistence integrity + secret handling matter.

## 5. High-value surfaces (prioritized per audit_method §Prioritization)

1. **Permission / tool-confirmation gating** (`permission/`,
   `agents/tool_confirmation_router.rs`, `agents/tool_execution.rs`) — the
   code-level boundary that must stand in front of destructive tool calls.
2. **Security inspectors** (`security/`) — prompt-injection scanner, egress
   inspector, adversary inspector, classification client. These ARE the claimed
   safety controls; audit whether they hold and whether they're bypassable.
3. **Secret / credential handling** — OAuth tokens, provider API keys, keyring
   (`oauth/`, provider auth files, `config/`).
4. **MCP / extension / subprocess spawning** — command construction, argument
   handling, malware check, lifecycle (`agents/mcp_client.rs`,
   `agents/extension_*.rs`, `execute_commands.rs`).
5. **Session persistence & import** (`session/`, `import_formats/`) —
   integrity, provenance, injection via imported/shared sessions (`nostr_share`).
6. **Provider request/response pipeline** (`providers/`) — external-API contract,
   auth scope, retries, rate limits, failure tolerance, streaming.
7. **Context management** (`context_mgmt/`) — truncation/summarization correctness,
   secret leakage across context, unbounded growth.
8. **Desktop/TUI operator truth** (`ui/desktop`, `ui/text`) — fake success,
   stale display, destructive-action ambiguity, CLI/API/UI mismatch.

## 6. Known environment / claims to verify honestly

- `README.md` makes **performance & footprint claims** vs goose (binary size,
  cold-start, package count, build time) — in scope for `audit-performance-profile`
  and `audit-compliance-posture` (claim vs evidence).
- `SECURITY.md` states prompt-injection risk is acknowledged and pushes mitigation
  onto the user ("gosling may follow commands found embedded in content"). Lenses
  must judge the *actual* code controls against this stated posture, not the prose.
- Fork provenance (goose v1.38) means some findings may be inherited upstream;
  note provenance but score by present-code mechanism.

## 7. Applicability matrix (which of the 38 audit skills run)

**Not applicable (excluded, with reason):**
- `audit-flutter-ios` — no Flutter/Dart/iOS code in the repo. N/A.
- `audit-security-supabase` — no Supabase project, schema, RLS, or client. N/A.
- `audit-equation-sourcebase` — no equation/data-sourcebase (raw→staging→gold)
  data stack. N/A.

**Deferred / partial (run with explicit limits):**
- `audit-playtest-app` — requires building & running the Electron/CLI app;
  full release build + provider credentials are heavy/unavailable in this
  environment. Run as a static UX/flow readiness review with a clear
  "not executed live" Validation Limit.
- `audit-contract-crossrepo` — gosling is largely single-repo, but the
  `gosling-sdk-types` ↔ `ui/sdk` and Rust↔Electron ACP/JSON contract is a real
  producer↔consumer seam; run scoped to those internal cross-language contracts.

**Applicable lenses (run in full):** the remaining 33 skills — the security
family (security, security-code, security-llm, security-nodejs,
security-repo-posture, security-repo-triage, security-vuln-harness), the dataflow
family (cascade, concurrency, input-output, integrity, pipeline-graph,
state-transition, temporal), invariant-sync, negative-space, reliability,
failsafe-readiness, recovery-idempotency, resource-lifecycle, memory-lifecycle,
operator-signal, dependency-criticality, performance-profile, pipeline-externalapi,
deadcode-cleanup, architecture-seam, architecture-nodejs, contract-internalapi,
workflow-gui, design-webapp, compliance-posture, multiagent-consensus.

This yields **≥33 applicable + 2 partial = 35 skills exercised**, comfortably
above the 28 floor, with 3 honestly excluded as inapplicable.

## 8. Reporting contract (all lenses)

- Obey `evidence_discipline.md`: Confirmed requires a quoted `file:line` actually
  read; otherwise cap at Plausible. Severity is independent of confidence.
- Use the `finding_format.md` block for every material finding, with an ID
  prefixed by the lens domain.
- Include **explicit non-findings** (seams checked and held) and a
  **Validation Limits** section (what was NOT reviewed).
- No generic advice: every recommendation ties to a mechanism + a proving test.
- Because this repo is a **mission-critical baseline**, do **not** inflate
  confidence or severity. A calibrated "Plausible / needs drill" beats a
  confident-but-unproven "Critical".
