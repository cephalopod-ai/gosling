# Gosling Stress-Test Audit — Master Report

**Engagement:** full multi-lens audit of `gosling` using the `agent-skills/10_audit`
catalog. **Authority:** audit-only / read-only (no source modified; only
`docs/cloud/` written). **Date:** 2026-07-05. **Target:** `gosling` @
`claude/gosling-stress-test-audit-jwhooa` (fork of goose v1.38).

This report **merges, de-duplicates, and severity-ranks** the findings from 34
independent lens reports in this directory. Read the per-lens files for full
evidence, non-findings, and validation limits; this file is the consolidated view.

---

## 1. Scope & method

- **35 audit skills exercised** (≥28 requested): 33 run in full + 2 run with
  explicit limits (`playtest-app`, `contract-crossrepo`). `multiagent-consensus`
  is realized as the cross-lens convergence + lead verification in §4/§6.
- **3 skills excluded as N/A** (recorded in `00-orientation.md`):
  `audit-flutter-ios`, `audit-security-supabase`, `audit-equation-sourcebase` —
  no corresponding surface exists in the repo.
- Each lens obeyed the shared **evidence-discipline contract**: `Confirmed`
  requires a quoted `file:line`; races/OOM/crash-recovery capped at
  `simulation-reasoned`/`requires-authorized-drill` unless traced; severity scored
  independently of confidence; explicit non-findings and validation limits
  required.
- **Nothing was built or run live.** `cargo build` fails in this environment (a
  dependency build-script needs a network download blocked by policy — see
  `audit-playtest-app.md`). Every finding is therefore static / source-evidenced
  or simulation-reasoned. Runtime manifestations that need a kill/network/crash
  drill are labelled as such and are **not** claimed Confirmed.

## 2. Ship-readiness verdict (honest, calibrated)

Gosling is a **well-engineered agent framework** — the async/subprocess
lifecycle, the SQLite session store (WAL + `BEGIN IMMEDIATE`), the ACP typed
protocol with a real CI drift-gate, the provider retry/backoff plumbing, and the
Electron Fuses/`contextIsolation` hardening are all above average, and many
lenses returned strong non-findings. **The blocker for "mission-critical /
ship-ready" is not code quality — it is the default security posture and the
truthfulness of the approval UI**, plus two durability/execution defects. In the
threat model gosling itself documents (`SECURITY.md`: *"gosling may follow
commands found embedded in content"*), the controls that are supposed to contain
that risk are, in the default configuration, **off, advisory-only, fail-open,
self-certifiable, or auto-approved on every surface.**

Good news: the fixes are mostly **small and known** — in almost every case the
correct pattern already exists elsewhere in the same file/repo (atomic
`config.yaml` write next to the non-atomic secrets write; `ollama`'s
`transient_only(true)`; the Docker path's `env_clear`; the CLI's fail-closed
non-interactive gate that the TUI lacks). This is a **posture-and-defaults**
problem far more than a rewrite.

**Recommendation:** treat Cluster A (default-permissive controls), Cluster B
(untrusted-repo code execution), and finding SEC-GSL-001 / the secrets-write bug
(Cluster C) as **ship-gating**. The rest is a prioritized hardening backlog.

## 3. Severity tally (material findings, post-dedup)

| Severity | Count (distinct root causes) | Notes |
|---|---|---|
| **Critical** | 0 | No single Confirmed reachable-catastrophe finding; the Critical *risk* is the **compounding** of Cluster A, which no one lens rates Critical alone. |
| **High** | 12 | Concentrated in Clusters A, B, C. |
| **Medium** | ~34 | Reliability, external-API, integrity, UI-truth, Electron edges. |
| **Low / Info** | ~40 | Hardening, claims accuracy, maintainability. |

(≈150 raw findings across 34 lenses collapse to the clusters below; counts are of
distinct mechanisms, not raw findings.)

## 4. Cross-lens convergence — the signal that matters

The audit's strongest output is **where independent lenses collided on the same
mechanism**. Convergence = high confidence. The dominant theme (Cluster A) was
found independently by **eight** lenses and then **lead-verified against source**.

### CLUSTER A — Default security posture is non-enforcing *(SHIP-GATING)*

The human-in-the-loop / prompt-injection defense is defeated on every surface in
default config. **Lead-verified** items are marked ✔.

| Mechanism | Evidence (lead-verified) | Found by lenses |
|---|---|---|
| Default `GoslingMode` = **Auto** ⇒ every tool auto-approved, no gate ✔ | `gosling_mode.rs:25-27` `#[default] Auto`; `agent.rs:314` `unwrap_or_default()`; `permission_inspector.rs:152` `Auto => Allow`; headless force-sets Auto `session/mod.rs:1063` | security-llm, pipeline-graph, security, negative-space, workflow-gui |
| Prompt-injection scanner **default-off**, and even on only scans the literal `shell` tool's args + user msgs — **never tool/MCP/web results** (the actual injection ingress) ✔ | `security/mod.rs:53-54` `unwrap_or(false)`; `scanner.rs:394` matches name `"shell"` only | security-llm, security, pipeline-graph, negative-space, reliability, operator-signal, dependency |
| `EgressInspector` is **telemetry-only** — always returns `Allow`, confidence 0.0, after "network egress detected" ✔ | `egress_inspector.rs:369-383` | security-llm, security, pipeline-graph, operator-signal, failsafe, dependency |
| Tool inspectors **fail open** — an inspector `Err` is logged and its verdict dropped; loop returns `Ok` ✔ | `tool_inspection.rs:107-114` | pipeline-graph, failsafe, operator-signal, dependency, reliability |
| `read_only_hint: true` from a **third-party** tool auto-approves it, even in the stricter Approve mode | `permission_inspector.rs:38-53,164-169` | security-llm, state-transition, negative-space |
| SmartApprove caches `AlwaysAllow` keyed by **tool name only**, judged on names-not-args, persisted to disk | `permission_judge.rs:90-116`, `permission.rs:183` | security-llm, state-transition |
| **Subagents forced to Auto** — operator's mode nullified for delegated work | `summon.rs:976-996` (comment admits workaround) | negative-space |
| **TUI silently auto-approves** every call by selecting `options[0]` = `AllowAlways` | `ui/text/src/tui.tsx:763-773,1315-1325`; `acp/server.rs:1891` orders AllowAlways first | workflow-gui |
| **ACP app tool entrypoint** calls raw `ExtensionManager::dispatch_tool_call`, bypassing permission-inspector + PreToolUse hooks | `acp/server/tools.rs:97` | contract-internalapi |
| Desktop approval UI: destructive **`Always Allow` styled identically to `Allow Once`**; command args hidden/truncated before approval; no screen-reader announce | `ToolApprovalButtons.tsx:138-157`, `ToolCallWithResponse.tsx:780` | design-webapp |

**Why this is the headline:** in a default install, the *only* runtime boundary in
front of shell/write/MCP execution is the permission gate, and that gate is Auto
(open) by default, bypassable via self-declared `read_only_hint` when tightened,
nullified for subagents, and silently answered "AllowAlways" by the TUI. The
prompt-injection and egress "controls" `SECURITY.md` alludes to do not enforce.

### CLUSTER B — Untrusted-repo → local code execution *(SHIP-GATING)*

Opening/running gosling inside an attacker-controlled repository — a *normal*
coding-agent workflow — yields code execution with no workspace-trust prompt.

| Mechanism | Evidence | Lenses |
|---|---|---|
| Project-scoped plugins/hooks under `<cwd>/.agents/plugins/**` are **auto-discovered and auto-enabled** (`is_enabled` defaults true); a `SessionStart` hook runs `sh -c <command>` on first turn with no consent | `plugins/discovery.rs:52-147`, `agent.rs:371,1455-1463`, `hooks/mod.rs:594-598` | vuln-harness |
| Project `.mcp.json` auto-loaded and its `command`+`args` spawned at connect | `cli/session/builder.rs:414-419`, `extension_manager.rs:1104` | vuln-harness |
| OSV "malware check" in front of that spawn is **fail-open & narrow** (npx/uvx only, first arg only, any network error ⇒ allow) | `extension_malware_check.rs:48-56,211-233` | vuln-harness, security, security-code, state-transition, dependency |
| Plugin install `git clone <source>` lacks `--` end-of-options guard; a `source` beginning `-` becomes a git option ⇒ local exec | `plugins/mod.rs:292-301` | input-output |

### CLUSTER C — Secret handling & durability *(SEC-GSL-001 + secrets-write are SHIP-GATING)*

| Mechanism | Evidence (lead-verified ✔) | Lenses |
|---|---|---|
| **Non-atomic secrets write** — `truncate(true)` + `write_all`, no temp/rename/fsync; a crash/kill/ENOSPC mid-write **irreversibly loses all API keys + OAuth tokens**. The sibling `config.yaml` path *is* atomic. ✔ | `config/base.rs:42-62` (vs atomic `save_values:658-683`) | concurrency, integrity, recovery |
| Local **MCP stdio subprocess inherits goslingd's full env** — third-party extension code reads all provider API keys and the server secret (RCE-equivalent). The Docker path already `env_clear`s — proof the fix is feasible. | `extension_manager.rs:1104-1105` | security |
| Master server `secret_key` transmitted as a **`?secret=` URL query param** (lands in logs/history) | `mcp_app_proxy.rs:25-26,161,259` | security-code |
| Cross-process config/secret writes race (in-process `Mutex` only; CLI+desktop+server share `~/.config/gosling`) → lost token refreshes | `config/base.rs`, `permission_store.rs` | concurrency, integrity |

### CLUSTER D — Reliability & external-API robustness

- **Retry non-retryable 4xx by default** (`RetryConfig.transient_only=false`;
  a repo test pins that a 400 is retried) — 4× load + delay for requests that can
  never succeed. `ollama` uses the correct `.transient_only(true)`.
  [`retry.rs:99-108,28-38`] — cascade, reliability, pipeline-externalapi.
- **Bedrock nests a 6-retry budget over the AWS SDK's own retries** → ~18× metered
  calls worst case [`bedrock.rs:122-123`] — pipeline-externalapi.
- **Missing HTTP timeouts** on the Bedrock mantle path, GCP metadata, and OAuth
  token clients (`reqwest::Client::new()`) — a silent-open endpoint hangs the turn
  [`bedrock.rs:173`, `gcpauth.rs:244`] — pipeline-externalapi, reliability.
- **Uncapped Google `retryDelay`** parse — `"999999999s"` freezes the turn
  [`providers/utils.rs:79-102`] — reliability.
- **`grind` nudge self-feeds** — re-injects "keep working" every no-tool turn,
  bounded only by `max_turns=1000` [`agent.rs:2610-2624`] — cascade.
- **Stream truncation accepted as complete** — a stream closing without `[DONE]`
  or `finish_reason=length` is committed as a full message [`base.rs:323-372`] —
  pipeline-externalapi.

### CLUSTER E — Crash / recovery integrity

- **Mid-turn crash replays tool side effects** — tool responses are persisted only
  *after* execution; on resume `fix_conversation` strips the orphan and the model
  re-issues the same (irreversible) call, no idempotency key
  [`agent.rs:2078,2221-2299,2680`] — recovery.
- **Corrupt config ⇒ silent "start fresh" ⇒ next write persists only the new key**
  = total silent config loss [`config/base.rs:527-548`] — integrity.
- Session import spans 3 un-enclosed transactions → partial sessions on interrupt
  [`session_manager.rs:1939-2006`] — integrity.

### CLUSTER F — Operator truth / signal

- **Desktop "fake success"** — a tool with no response after streaming renders a
  green success dot (code-commented "workaround") [`ToolCallWithResponse.tsx:509-520`];
  the error string is dropped on the error variant [`:532-535`] — workflow-gui.
- **CLI logging is file-only** (`console:false`) so every `tracing` security/degradation
  event reaches no interactive operator [`gosling-cli/src/logging.rs:19`] — operator-signal.
- Unknown slash command is **silently sent to the LLM as a billable prompt**
  [`input.rs:182-187`] — playtest.

### CLUSTER G — Electron / renderer trust boundary

- **CSP ships `script-src 'unsafe-inline'`** while the renderer holds a live backend
  secret and can reach the local agent over `ws://127.0.0.1:*` — any renderer HTML
  injection ⇒ full local-agent control + exfil [`csp.ts:65-80`, `main.ts:1894`] — security-nodejs.
- **Unconfined fs IPC** — `read/write/delete-file/list-files` accept arbitrary
  renderer paths, no root confinement, no symlink reject [`main.ts:2203-2294`] — security-nodejs.
- `setPermissionRequestHandler` grants **every** permission unconditionally
  [`main.ts:2362-2371`]; `open-external` uses a denylist not an allowlist — security-nodejs.

### CLUSTER H — Architecture / contracts (the "lighter, remixable" goal)

- **Inverted domain ownership** — `Message`/`Conversation`/`Usage` + the model
  registry live in the *leaf* `gosling-providers` crate; you cannot separate the
  provider layer from the domain [`conversation/message.rs:771`] — architecture-seam.
- `agent.rs` is a **3,858-LOC god orchestrator** (18-field struct, 4 mutexes,
  reaches 15 modules) — architecture-seam.
- **Global mutable config+secrets singleton** called at **256 sites** across 3
  crates [`config/base.rs:447`] — architecture-seam.
- **Cross-language type duplication with no parity gate** — 9 enum/union pairs
  hand-mirrored Rust↔`ui/desktop/src/types` (`AGENTS.md` forbids consuming the
  generated types); aligned today, unenforced tomorrow — invariant-sync.
- **SDK supply drift** — `desktop` links the workspace SDK, `text` pins a registry
  version → `text` can run stale generated types against the current binary
  [`pnpm-lock.yaml:406-408`] — architecture-nodejs.

### CLUSTER I — Compliance / claims accuracy

- **Three version identities** in one README (v0.0.1 vs v1.40.0) matching real
  divergent builds; goose anchor split (fork of v1.38, benchmarked vs v1.41.0)
  [`README.md:23,31,54`] — compliance-posture, performance-profile.
- **Attribution contradiction** — docs credit "AAIF"; the retained Apache-2.0
  notice says "Copyright 2024 Block, Inc." (needs human/legal resolution) —
  compliance-posture.
- README cold-start claim rests on `--version`/`doctor` toy commands that skip
  real agent init; sub-10ms on a 117MB binary reads as *warm* not cold —
  performance-profile.
- `EXTRACTION_PLAN.md` is partially stale (describes already-deleted modules as
  present) — deadcode-cleanup. (The dropped-feature *removal itself* is genuinely
  clean — a strong non-finding.)

### CLUSTER J — Resource / memory (bounded)

- macOS orphans MCP/ACP children on hard parent SIGKILL (no PDEATHSIG equivalent;
  self-documented `subprocess.rs:54-56`); Linux path is test-proven — resource, failsafe.
- Unbounded `DIGEST_CACHE` (`summarizer/mod.rs:147`, opt-in) and per-turn full read
  of an uncapped `memories.jsonl` (`agent.rs:1772`, `memory.rs:116`) — memory-lifecycle.

## 5. Recommended patch order

1. **Flip the defaults / close the gates (Cluster A + B + C-core).** Highest risk
   per unit effort; most fixes are one-liners with the correct pattern already
   in-repo:
   - Make secrets write atomic (reuse `save_values`' temp+rename+fsync). *[XS]*
   - `env_clear` local MCP stdio spawns like the Docker path. *[S]*
   - Synthesize a **fail-closed `RequireApproval`** when an inspector errors,
     instead of dropping its verdict. *[S]*
   - Add a **workspace-trust prompt** before auto-enabling project-scoped
     plugins/hooks/`.mcp.json`. *[M]*
   - Add `--` to the plugin `git clone`. *[XS]*
   - Route the ACP app tool entrypoint through the shared permission gate + add a
     cross-entrypoint parity test. *[S]*
   - Fix the TUI to honor the real mode / fail closed non-interactively (match the
     CLI). *[S]*
   - Stop trusting third-party `read_only_hint`; key smart-approve cache on args
     not just name. *[M]*
2. **Reconsider the "advisory-by-design" controls (owner decision).** Decide
   whether `EgressInspector` and the prompt-injection scanner are *controls* or
   *telemetry*; if controls, make them enforce and default-on, and scan tool/MCP
   results (the real ingress). Otherwise stop presenting them as mitigations.
3. **Approval-UI truthfulness.** Re-rank desktop approval buttons, show full args,
   add `aria-live`/focus, remove desktop "fake success". *[S each]*
4. **Reliability defaults.** `transient_only=true` default; disable Bedrock SDK
   retries under the wrapper; add timeouts to the 3 unbounded clients; clamp the
   Google retryDelay. *[S each]*
5. **Durability/recovery.** Atomic turn boundary or idempotency key for tool
   side effects; enclose session import in one transaction; fail-closed on corrupt
   config instead of silent reset.
6. **Compliance clean-up (human-owner).** Reconcile version identities and the
   AAIF/Block attribution; refresh `EXTRACTION_PLAN.md`.
7. **Maintainability backlog** (supports the "remixable" goal): domain-ownership
   inversion, `agent.rs` decomposition, config-singleton DI, cross-language type
   parity test.

## 6. Residual-risk register (needs a drill or an owner decision — NOT closed here)

| ID | Item | Why open | Owner |
|---|---|---|---|
| RR-1 | Runtime manifestation of Cluster A (a tool actually executing ungated under injection) | static-confirmed mechanism; not reproduced (no live build) | drill |
| RR-2 | `github/command@v2` denial semantics behind `continue:'true'` in Windows/Intel PR-bundle workflows (unauthorized cloud-OIDC build) | action internals reasoned, not read; branch protection is platform-only | security + human |
| RR-3 | Non-atomic-secrets & mid-turn-replay actual data loss | needs a kill/crash drill to promote from Likely | drill |
| RR-4 | Single-provider-per-session outage has no failover | intentional SPF | human-owner |
| RR-5 | AAIF vs Block, Inc. true upstream identity | can't fetch upstream in this run | human/legal |
| RR-6 | macOS subprocess orphan census; RES-GSL-001/004 timing | needs `ps`/`lsof` on macOS | drill |
| RR-7 | Install-time provenance (SLSA/sigstore produced at release, **not verified** by `download_cli.sh`) | tamper-install RCE; needs branch-protection + install-verify decision | human-owner |

## 7. Validation limits (whole engagement)

- **No live execution.** No `cargo build`/`cargo test`/`clippy`, no app run, no
  provider calls, no crash/network/kill drills. Every "runtime hang / fail-open /
  double-exec / OOM" is simulation-reasoned, not observed.
- **Sampling.** ~183K Rust + 75K TS LOC exceeds full-read budget; each lens
  deep-read a prioritized boundary sample and recorded unreviewed areas. Notably
  thin coverage (flagged across lenses, recommended follow-ups): the **ACP
  subprocess providers** (`claude_code.rs`, `codex.rs`, `gemini_cli.rs`,
  `cursor_agent.rs`) and their external-credential/`--dangerously-skip-permissions`
  mapping; `gosling-server` REST/authz surface; `mcp_app_proxy` guest HTML;
  `context_mgmt` internals; `ui/text` beyond the approval path; several provider
  bindings (databricks/vertex/sagemaker bodies).
- **No git history / platform settings.** Secret-scanning history, branch
  protection, required-checks, and default `GITHUB_TOKEN` scope are GitHub platform
  state not observable from source — they gate the true severity of RR-2/RR-7.
- **Consensus.** The headline cluster is corroborated by ≥3 independent lenses and
  lead-verified against source (§4). Single-lens findings are labelled as such in
  their reports and should be independently confirmed before remediation where the
  confidence is below Confirmed.

## 8. Per-lens index

All 34 reports live beside this file as `audit-<lens>.md`. Highest-signal reads:
`audit-security-llm.md`, `audit-security.md`, `audit-dataflow-pipeline-graph.md`,
`audit-negative-space.md`, `audit-failsafe-readiness.md`, `audit-workflow-gui.md`,
`audit-security-vuln-harness.md`, `audit-dataflow-integrity.md`,
`audit-design-webapp.md`, `audit-pipeline-externalapi.md`,
`audit-security-repo-triage.md`, `audit-compliance-posture.md`. Orientation and
applicability matrix: `00-orientation.md`. Engagement timeline: `session-log.md`.
