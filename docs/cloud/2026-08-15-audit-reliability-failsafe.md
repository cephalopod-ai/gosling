# Gosling Reliability + Failsafe Family Audit — 2026-08-15

**Date:** 2026-08-15  
**Target:** `/Users/eric/Work/vscode/forked/gosling`  
**HEAD:** `073d19428509ea6eb317924b1856a1fe7e9002c8` (`refine XAI auth settings and OAuth handling`)  
**Authority:** `read_only` — source not modified. Static traces only; no kill/network/corruption drills.  
**Lenses:** `audit-reliability` (REL-001..015) + failsafe family umbrella `audit-failsafe-readiness` (FSR-001..016) with siblings `audit-recovery-idempotency` (REC-001..012), `audit-dependency-criticality` (DEP-001..014), `audit-operator-signal` (SIG-001..013).  
**Draft-prompt note:** The supplied prompt is treated as a draft. The intended mission (provider retries/timeouts, doctor/info honesty, mid-turn crash, secrets atomicity, inspector fail-open, missing provider, health/readiness, cancellation) is preserved and expanded to the adjacent seams those mechanisms share (slash `/doctor` mutation, sibling `/status` implementations, Auto-mode inspector downgrade, `RetryConfig::new` vs default).

Historical `docs/cloud/audit-{reliability,failsafe-readiness,recovery-idempotency,dependency-criticality,operator-signal}.md` reports are **seeds only**. Every verdict below was re-read at this HEAD.

**Budget / stop:** Deep-read the named focus surfaces plus their producer/consumer twins (~55 files). Stop condition: every inventory mnemonic is a finding or an explicit non-finding. `goslingd` HTTP surface walked at `/status` only; Ink UI, OAuth token-refresh under loss, and every provider adapter beyond retry/timeout were sampled.

---

## 1. Surface inventory

| Surface | Entrypoint | State mutation | Operator-facing signal |
|---|---|---|---|
| CLI `gosling doctor` | `crates/gosling-cli/src/commands/doctor.rs` | none | stdout + always exit 0 |
| CLI `gosling info` / `info --check` | `crates/gosling-cli/src/commands/info.rs` | none | stdout; `--check` live-probes provider and exits nonzero |
| Slash `/doctor` | `crates/gosling/src/agents/execute_commands.rs` → `crates/gosling/src/doctor.rs` | **global provider/model write** | chat message after possible auto-switch |
| `gosling serve` (desktop backend) | `crates/gosling-cli/src/cli.rs` `handle_serve_command` → `acp/transport::create_router` | ACP/session/MCP | `/health`, `/status` |
| `goslingd` HTTP host | `crates/gosling-server` | session/agent/reply | `/status` only (probes session store) |
| Desktop readiness | `ui/desktop/src/backendStatus.ts` | none | treats `/status` 200 + `/acp` 406 as ready |
| Provider retry/timeout | `gosling-providers/src/{retry,http_status}.rs`, per-adapter `retry_config()` | outbound HTTP | warn logs + eventual `ProviderError` |
| Agent reply / cancel | `agents/agent.rs`, `gosling-server/src/routes/reply_service.rs`, CLI `session/mod.rs` | messages + `tool_operations` | cancel/error events |
| Tool inspection | `tool_inspection.rs`, security/egress/adversary/permission inspectors | approval/deny | approval prompt or Auto allow |
| Secrets / config writes | `config/base.rs` `write_secrets_file`, `save_values` | `secrets.yaml` / `config.yaml` | file + keyring fallback |
| MCP client / children | `agents/mcp_client.rs`, `subprocess.rs` | child processes | spawn/timeout errors |

---

## 2. Boundary map

| Surface | Intended boundary | Observed at HEAD |
|---|---|---|
| Serve liveness vs readiness | `/health` = process up; `/status` = ready for work | **Both** `/health` and `/status` are the same static `"ok"` on `gosling serve` |
| `goslingd` readiness | `/status` reflects session-store reachability | Held — probes `SessionManager::healthy()` |
| CLI doctor | "Check that your Gosling setup is working" | Local dump only; never probes provider; always 0 |
| `info --check` | pre-flight verifier | Held — live complete + classified errors + nonzero exit |
| Slash `/doctor` | diagnose; refuse or ask before changing defaults | Live-tests, then **writes** a new global provider/model |
| Inspector error | fail-closed (do not execute unjudged tools) | Held — synthesized `RequireApproval` |
| Auto mode | advisory findings may downgrade; hard security/working-dir/adversary must not | Held for those three; **egress still auto-downgrades** |
| Mid-turn crash | do not re-dispatch a started tool | Held — `tool_operations` + `in_doubt` + recover-on-reply |
| Secrets write | atomic replace | Held — temp + `sync_all` + rename under flock |
| Provider retry | bounded, transient-only by default | Default held; Bedrock/GCP/Databricks still retry 4xx |
| Missing provider | `fail_visible` / `fail_closed` | Held on session builder; planner/configure still `.expect()` |
| Cancellation | cancel token + child cleanup | Held for SIGINT/SIGTERM on serve and CLI turn; macOS hard-kill residual |

---

## 3. Failure-mode inventory (REL)

| Op | Dependency | Failure modes | Timeout? | Retry | Atomic recovery? | Health/signal |
|---|---|---|---|---|---|---|
| Provider complete/stream | LLM HTTP | 401/429/5xx/empty/malformed/hang | yes (e.g. OpenAI 600s; Google delay clamped 3600s) | cap 3 + backoff/jitter; default `transient_only` | n/a (read) | warn per retry; classified `ProviderError` |
| `gosling serve` ready | process, auth, session DB, provider | DB down, provider missing, inspector fail | desktop probe 1s / 30s budget | poll `/status` | n/a | **static 200** (REL-GSL-010) |
| `goslingd` `/status` | session SQLite | store unreachable | query | none | n/a | 503 `"degraded: session store unreachable"` |
| CLI doctor | none actually probed | misconfigured/broken provider | n/a | n/a | n/a | **"setup is working" + exit 0** (REL-GSL-011) |
| Slash `/doctor` | every registered provider | configured provider down | provider timeout | implicit via complete() | writes global config | auto-switch message (FSR-GSL-012) |
| Tool dispatch | MCP/shell/provider tools | crash after start | MCP `DEFAULT_EXTENSION_TIMEOUT` 300s | **no auto-retry** if `in_doubt` | `tool_operations` | in-doubt error to model |
| Secrets write | filesystem | kill mid-write | n/a | n/a | temp+fsync+rename | next read `NotFound` / parse err |
| Inspector | ML endpoint / inspector impl | `Err` or classifier outage | classifier HTTP | none | n/a | fail-closed approval; init fallback log-only |

---

## 4. Failsafe inventory

| Workflow | Assumption/Dependency | Failure trigger | Target safe state | Timeout/Abort | Cleanup | Signal | Recovery |
|---|---|---|---|---|---|---|---|
| `gosling serve` start | bind + secret | missing secret | `fail_closed` | n/a | n/a | bail names env | n/a |
| Desktop wait-for-backend | `/status` means ready | session store / provider down | `fail_visible` | 30s / 1s probe | none | healthcheck_* events | retry until deadline |
| CLI doctor | "working setup" | broken provider | `fail_visible` | none | none | local dump + exit 0 | `info --check` exists but unlinked |
| `info --check` | configured provider | auth/net/model | `fail_closed` | provider timeout | none | classified + hint + nonzero | configure |
| Slash `/doctor` | configured provider | 401/404/5xx | `fail_visible` or `fail_manual_hold` | provider timeout | none | chat + **silent global write** | persist different provider |
| Agent turn | provider + tools | cancel / crash | `fail_resumable` / `fail_idempotent` | cancel token; MCP 300s | `ToolOperationGuard` → `in_doubt` | cancelled/error reason | recover on next `reply` |
| Tool inspection | inspectors run | inspector `Err` | `fail_closed` / `fail_manual_hold` | n/a | n/a | RequireApproval | human |
| Secrets save | writable config dir | kill mid-write | `fail_idempotent` | n/a | leftover `.tmp` | next-read error | old file remains |
| Planner / configure tools | provider set | unset | `fail_visible` | n/a | none | **panic via expect** | configure |
| MCP child | parent alive | SIGKILL parent (macOS) | `fail_visible` + reap | kill_on_drop | Linux PDEATHSIG | none on orphan | residual RR |

---

## 5. Recovery & idempotency map

| Operation | Side-effect class | Worst interruption | On-rerun | Idempotency class | Compensation | Safe state |
|---|---|---|---|---|---|---|
| `write_secrets_file` | file | kill after truncate of **temp** (dest untouched) | old secrets still present | `naturally-idempotent` | flock + temp+rename | `fail_idempotent` — **held** |
| `save_values` | file | kill mid-temp | old config remains | `naturally-idempotent` (UUID temp + lock) | flock | `fail_idempotent` — **held** |
| `add_message` | DB | kill mid-tx | WAL rollback | single-statement tx | WAL | `fail_rollback` — **held** |
| Tool dispatch | process / external API / file | kill after `state='started'` | **replay blocked**; `in_doubt` synthesized | `keyed` (`session_id`+`tool_request_id`) | recover_tool_operations | `fail_resumable` — **held** |
| Provider HTTP | network | timeout after server applied (rare for chat) | retry may re-POST | `non-idempotent-unprotected` (chat complete is usually safe) | retry budget | `fail_degraded` |
| Slash `/doctor` provider swap | file + session | after `set_active_provider` | new provider is canonical | `non-idempotent-unprotected` | none (no confirm / no rollback) | should be `fail_manual_hold` |
| Config load of corrupt YAML | file | corrupt file | read skips with warning; write refuses | n/a | original preserved on write | `fail_visible` — **held** |

### Write-path atomicity (four questions)

| Write | Idiom | Half-state | Who detects? | Who repairs? |
|---|---|---|---|---|
| `write_secrets_file` | temp + `sync_all` + `rename` | leftover `*.tmp`; dest intact | next reader of dest | operator deletes `.tmp`; dest never truncated |
| `save_values` | UUID temp + flock + `sync_all` + `rename` | leftover unique temp | dest reader | same |
| `add_message` | SQLite `BEGIN IMMEDIATE` | nothing committed | WAL | rollback |
| `begin_tool_operation` | INSERT `started` then dispatch | `started` row, no result | recover on next `reply`/`load_session`/`resume` | `in_doubt` response, `retryable: false` |
| Slash `set_active_provider` | config write (atomic file) | new provider persisted | next session | manual `gosling configure` |

### Crash-point enumeration — conversation-bound tool call

| Point | State | Rerun | Class |
|---|---|---|---|
| C0 before persist ToolRequest | nothing | re-asks model | clean |
| C1 after ToolRequest persisted, before `begin_tool_operation` | request in messages | `begin` requires checkpoint; new `started` | clean / first dispatch |
| C2 after `started` insert, before side effect | durable start | `InDoubt` — **not** re-dispatched | resumable (blocked) |
| C3 after side effect, before `completed` | external effect + `started` | `InDoubt`; effect may have happened | residue / unknown external; **no duplicate dispatch** |
| C4 after `completed`, before response message | result stored | `Replay` stored result | clean |
| C5 after response persisted | done | no-op | clean |

Worst class today is C3 **external residue without duplicate dispatch** — residual, not the historical replay finding.

---

## 6. Dependency criticality register

| Dependency | Class | Consumers | Absence behavior | Detected? | Detection time | Is SPF | Alternate | Safe state | Owner decision |
|---|---|---|---|---|---|---|---|---|---|
| Selected LLM provider (runtime) | DEP-008 | every turn, `info --check` | `refuse-clear` at first-use | exception | first-use | yes (intentional) | none for outage | `fail_visible` | RR-001 — no failover |
| Provider unset in config | DEP-002 | builder, planner, configure-tools | builder `refuse-clear`; planner **`crash`** | exception / panic | first-use | no (refusal exists on main path) | configure | `fail_visible` | fix planner `.expect` |
| Session SQLite | DEP-007 | serve, goslingd, resume | goslingd `refuse-clear` 503; **serve `/status` `degrade-silent`** | goslingd health; serve none | goslingd first-use; serve **never** via `/status` | yes for serve readiness | none | `fail_visible` | wire serve `/status` to `healthy()` |
| OS keyring | DEP-003 | secrets | `degrade-honest` → `secrets.yaml` 0600 | error-string match | first-use | no | file fallback | `fail_degraded` | none |
| MCP stdio binary | DEP-010 | extensions | `refuse-clear` (spawn + stderr) | exception | first-use | no | disable extension | `fail_visible` | none |
| ML classifier endpoint | DEP-008/009 | security inspector (opt-in) | init: `degrade-silent` to patterns; runtime: `degrade-honest` (approval) | warn / approval | init never-to-operator; runtime first-use | no (pattern fallback) | patterns | `fail_degraded` | announce init fallback |
| Tool inspectors as a set | DEP-004 | tool gate | `refuse-clear` on `Err` | error + approval | mid-operation | no | synthesized RequireApproval | `fail_closed` | none — **held** |
| `gosling` serve binary | DEP-010 | desktop | `refuse-clear` | fs + 30s readiness | startup | no | error names paths | `fail_visible` | none |
| Bind port | DEP-005 | serve | `refuse-clear` (bind error) | exception | startup | no | desktop port 0 | `fail_visible` | none |
| `git` / `bash` | DEP-010 | plugins / shell | git refuse; bash → `sh` | first-use | first-use | no | `sh` / skip feature | `fail_visible` / `fail_degraded` | none |
| Human approval (Approve/SmartApprove) | DEP-013 | tools | headless refuses | exception | first-use | no | Auto mode (explicit) | `fail_closed` | none |
| Quota / 429 | DEP-012 | providers | bounded retry + classified error | exception | mid-operation | no | backoff / wait | `fail_visible` | none |

---

## 7. Detection & signal map

| Failure event | Detection method | Surface | Audience | Visibility | TTD | Content | Next safe action |
|---|---|---|---|---|---|---|---|
| Session store down (`gosling serve`) | `none` on `/status` | static `"ok"` | desktop / LB | silent | days-to-never via health | `"ok"` | none — **REL-GSL-010** |
| Session store down (`goslingd`) | `health_check` | 503 body | operator / LB | UI-visible | immediate | names store | restart / fix DB path |
| Broken provider + `gosling doctor` | `none` | "local diagnostics complete" + exit 0 | operator | silent-as-success | never via doctor | provider name listed | `info --check` not mentioned — **REL-GSL-011** |
| Broken provider + `info --check` | `exception` | stdout + exit != 0 | operator / CI | obvious | immediate | Auth/Check + hint | `gosling configure` |
| Slash `/doctor` provider outage | `exception` then mutation | chat | end user | UI-visible **after** write | immediate | "switched to X/Y" | re-configure — **FSR-GSL-012** |
| Inspector `Err` | `log` + approval | action_required | end user | UI-visible | immediate | inspector name + error | approve/deny |
| Classifier **init** fail | `log` | tracing warn | log reader | log-only | hours | error chain | none interactive |
| Classifier **runtime** degrade | `exception`→approval | action_required | end user | UI-visible | immediate | "Security Review Required" | approve/deny |
| Mid-turn crash | recover on next reply | in-doubt tool result | model + later user | inferred | next turn | `retryable: false` | human verify external state |
| Planner missing provider | `exception` (panic) | stderr backtrace | operator | obvious | immediate | expect message | configure |
| MCP `list_tools` fail | `exception` | `Failed to enumerate…` | caller | logged | first-use | per-extension errors | fix extension |
| 429 | `exception` + retry warn | logs then error | log + user | logged | minutes (bounded) | RateLimitExceeded | wait / check quota |

### Observability-gap ranking

1. Serve `/status` green-while-broken — required `obvious`, actual `silent`. Gap 3. Driving axis: **detectability** + desktop shared-fate.  
2. CLI doctor false-success — required `logged`+actionable, actual `silent` (exit 0). Gap 2. Driving axis: **operator-deception**.  
3. `/doctor` auto-switch — detection exists, **confirmation** missing. Driving axis: **reversibility** of global config.  
4. Classifier init fallback — required `logged` interactive, actual `log-only`. Gap 1. Low.

### Log/alert rubric (sampled failure paths)

| Path | Structured fields | Severity honesty | Actionability (5Q) |
|---|---|---|---|
| Inspector fail-closed `error!` | inspector_name, error | honest ERROR | 4/5 — missing "still broken?" |
| Provider retry `warn!` | attempt, error Debug | honest | 3/5 — no next action |
| CLI `info --check` | classified labels + hint | honest | **5/5** |
| CLI doctor | none | N/A | 1/5 |
| Serve `/status` | none | **dishonest success** | 0/5 |

No in-repo alert-routing config (PagerDuty/etc.). Audience for most logs is whoever tails the desktop/CLI log — `log-only` unless the UI surfaces it.

---

## 8. Findings table

| ID | Title | Sev | Conf | Inventory |
|---|---|---|---|---|
| REL-GSL-010 | `gosling serve` `/health` and `/status` are the same static `"ok"`; desktop treats that as ready | High | Confirmed | REL-002, SIG-002, FSR-012, DEP-007 |
| FSR-GSL-012 | Slash `/doctor` auto-persists a different global provider/model when the configured one fails | High | Confirmed (path); switch manifestation Likely | FSR-003, FSR-015, DEP-008, DEP-014, SIG-010 |
| REL-GSL-011 | CLI `gosling doctor` claims to check the setup, never probes the provider, always exits 0 | Medium | Confirmed | REL-002, REL-011, SIG-010, FSR-012 |
| REL-GSL-012 | Bedrock / GCP Vertex / Databricks `RetryConfig::new()` still retries deterministic 4xx | Medium | Confirmed | REL-004, FSR-011, REL-014 |
| REL-GSL-005 | `McpClient` `assert!` on cross-session reuse can panic a live tool path | Medium | Confirmed (assert); panic manifestation Likely | REL-007, FSR-016 |
| FSR-GSL-005 | Planner + configure-tools still `.expect()` on missing provider/model | Low | Confirmed | FSR-001, FSR-004, DEP-002, SIG-001 |
| SIG-GSL-005 | ML classifier **init** failure still falls back to pattern-only with log-only signal | Low | Confirmed | REL-003, SIG-006, FSR-012, DEP-008 |

---

## 9. Detailed findings

### REL-GSL-010: `gosling serve` `/health` and `/status` cannot observe a down session store (or anything else)

Severity: High  
Confidence: Confirmed (static-200 is a code property). Runtime "desktop proceeds while DB is dead" is Likely unless drilled.  
Evidence basis: source-evidenced  
Domain: Reliability (also Failsafe / Operator Signal)

Evidence:
- `crates/gosling/src/acp/transport/mod.rs:216-218` — `async fn health() -> &'static str { "ok" }`
- `crates/gosling/src/acp/transport/mod.rs:239-241` — **both** `/health` and `/status` are `get(health)`
- `ui/desktop/src/backendStatus.ts:68-78` — `response.ok` on `/status` plus `/acp` 406 ⇒ `healthcheck_success`
- Contrast (sibling implementation): `crates/gosling-server/src/routes/status.rs:17-25` — `/status` calls `session_manager().healthy()` and returns 503 `"degraded: session store unreachable"`
- Contrast: `crates/gosling/src/session/session_manager.rs:628-635` — `healthy()` is documented as the readiness probe and runs `SELECT 1`

Observed behavior:
- Desktop readiness and any LB that hits `gosling serve` `/status` receive HTTP 200 `"ok"` as soon as the process is bound. The handler cannot observe session-store, provider, or inspector state. `goslingd` already implements the honest probe; serve does not use it.

Expected boundary:
- `fail_visible` / honest readiness: `/health` may be process-up; `/status` must fail closed (503 + named dependency) when the session store is unreachable. Safe state: `fail_visible`. Scenario SC-DEG; observed `degraded-lying`.

Failure mechanism:
- Auxiliary router aliases readiness onto a liveness stub. Desktop's producer (`statusUrl`) and serve's consumer (`/status`) were implemented as if `/status` meant ready; only `goslingd` actually probes.

Break-it angle:
- Start `gosling serve` against an unreadable/locked session DB (or after moving the data dir). Desktop `checkBackendStatus` still returns true once `/acp` answers 406.

Impact:
- Operator and desktop believe the backend is ready while the first real turn fails mid-operation. Shared-fate: every desktop session on that serve.

Operational impact:
- Blast radius: Service. Side-effect class: user-visible. Reversibility: reversible. Operator visibility: silent (health). Rerun safety: safe.

Adjacent failure modes:
- REL-GSL-011 (another green-while-broken operator probe). Provider-down is also invisible on `/status` (residual RR-001).

Recommended mitigation:
- Remediation patterns: `honest_health_check`, `dependency_health_probe`.
- Minimal repair: point serve `/status` at `SessionManager::healthy()` (same contract as `goslingd`); keep `/health` as liveness if desired.
- Local guardrail: desktop readiness must treat non-`"ok"` / 503 as not ready (already would, if the body/status changed).
- Behavior test: unreachable session store ⇒ `/status` 503 with the degraded phrase; `/health` may stay 200.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Cost drivers: tests. Nominal agent: codex.
- Rationale: the probe function already exists; serve just does not call it.

Validation:
- Test asserts 503 + body when `healthy()` fails; 200 when the store answers `SELECT 1`.

Non-goals:
- Do not add a live provider probe to `/status` in this slice (that is a separate, slower readiness tier).

Resilience mapping:
- Phase: withstand. Objective(s): understand, constrain. Safe state: fail_visible.

Failure analysis (FMECA row):
- Failure mode / Likely cause / Operational phase: readiness reports healthy / static handler / startup+normal_run
- Local effect / Workflow effect / System-or-operator effect: 200 `"ok"` / desktop proceeds / first turn dies on DB
- Detection method / Detection latency / Operator visible: none (via health) / delayed / false
- Compensating provision: none on serve; `goslingd` has the probe unused by this path

Criticality:
- Likelihood: plausible (moved/unwritable data dir, first-run race). Detectability: silent.

Single point of failure:
- is_spf: yes (session store for serve readiness). missing_alternate: true. redundancy_or_fallback: null. required_owner_decision: none (wire existing probe).

---

### FSR-GSL-012: Slash `/doctor` auto-writes a new global provider/model when the configured one fails

Severity: High  
Confidence: Confirmed for the mutation path. Live "switched because of a 429" manifestation is Likely unless reproduced.  
Evidence basis: source-evidenced  
Domain: Failsafe

Evidence:
- `crates/gosling/src/agents/execute_commands.rs:132` — `"doctor" => Ok(Some(crate::doctor::run(...)))`
- `crates/gosling/src/doctor.rs:71-90` — configured provider fails `try_create_and_test` → `try_other_models` → `save_and_set`
- `crates/gosling/src/doctor.rs:97-107` — then `try_other_providers` over the **entire registry** → `save_and_set`
- `crates/gosling/src/doctor.rs:138-148` — `save_and_set` calls `set_active_provider` (global config) and `agent.update_provider`
- `crates/gosling/src/doctor.rs:151-166` — "working" means one `complete()` of `"Say 'hello' and nothing else."`

Observed behavior:
- A user (or model) running `/doctor` because "something seems off" can have a transient 5xx/429/wrong-model on the configured pair treated as a license to persist a different model on the same provider, then a **different provider's default model**, as the new global default. There is no confirmation gate.

Expected boundary:
- `fail_manual_hold` (or `fail_visible` report-only): diagnose, name working alternatives, do not mutate defaults without an explicit operator accept. Scenario SC-USR/SC-DEP; observed `unsafe-continue`.

Failure mechanism:
- Diagnostic workflow reuses the same `complete()` used for health as a promotion criterion, then writes through the same path as `gosling configure`.

Break-it angle:
- Configure provider A / model M; make A return 429 or 404 for M; run `/doctor`. Config now points at another model or another provider. Subsequent sessions inherit it.

Impact:
- Capability, cost, data-residency, and tool-inspection assumptions change under the operator. A rate-limit can look like "doctor fixed it."

Operational impact:
- Blast radius: Repo (global config). Side-effect class: file + user-visible. Reversibility: compensatable. Operator visibility: UI-visible after the fact. Rerun safety: unsafe (may switch again).

Adjacent failure modes:
- REL-GSL-011 (CLI doctor is the opposite lie — no check at all). DEP-014 (unvalidated diagnostic path performs a destructive config write).

Recommended mitigation:
- Remediation patterns: `owner_decision_gate`, `fail_closed_refusal`, `operator_action_message`.
- Minimal repair: `/doctor` reports the working alternative and the command to apply it; do not call `save_and_set` unless the user confirms.
- Behavior test: failing configured provider ⇒ no `set_active_provider`; message names the next command.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Cost drivers: tests, operator_training. Nominal agent: codex.

Validation:
- Assert config file provider/model unchanged after `/doctor` against a failing mock; assert the message lists the candidate.

Non-goals:
- Do not remove the live probe; `info --check` already owns CI-style verification.

Resilience mapping:
- Phase: adapt. Objective(s): prevent_avoid, understand. Safe state: fail_manual_hold.

Failure analysis (FMECA row):
- Failure mode / Likely cause / Operational phase: diagnostic mutates defaults / probe success used as authorization / configure+recovery
- Local effect / Workflow effect / System-or-operator effect: `set_active_provider` / all new sessions use new model / operator believes doctor "fixed" setup
- Detection method / Detection latency / Operator visible: user_report of the chat line / immediate / true
- Compensating provision: message text; no rollback

Criticality:
- Likelihood: plausible (`/doctor` is the documented "something seems off" command). Detectability: logged (chat), not gated.

Single point of failure:
- is_spf: no (the write is the defect, not the missing alternate). missing_alternate: false. redundancy_or_fallback: tries other providers — that *is* the hazard. required_owner_decision: whether auto-failover is a product feature.

---

### REL-GSL-011: CLI `gosling doctor` advertises a setup check and always reports local completion

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Reliability (also Operator Signal)

Evidence:
- `crates/gosling-cli/src/cli.rs:703-704` — `about = "Check that your Gosling setup is working"`
- `crates/gosling-cli/src/commands/doctor.rs:7-17` — prints `render_report(...)` and `Ok(())` with no probe
- `crates/gosling-cli/src/commands/doctor.rs:20-31` — `Status: local diagnostics complete`; provider/model are `get_*.ok()` strings or `"not configured"`
- Contrast: `crates/gosling-cli/src/commands/info.rs:163-270` — `--check` live-completes, classifies auth vs other, hints `gosling configure`, **nonzero exit**
- `crates/gosling-cli/src/commands/info.rs:265-267` — comment: automation should use `gosling info --check` as the pre-flight verifier
- Playtest cards still pair doctor with resilience (`docs/test_scenarios/16-provider-and-network-resilience.md`, `01-first-run-and-lifecycle.md`)

Observed behavior:
- `gosling doctor` with a configured-but-dead provider prints `Provider: <name>` and `Status: local diagnostics complete` and exits 0. The help string says the setup was checked.

Expected boundary:
- `fail_visible`: either (a) doctor performs the same live check as `info --check` and exits nonzero, or (b) help/status name the command that actually checks and doctor exits 0 only as a dump. Scenario SC-DEG; observed `degraded-lying` (help + exit 0 vs unprobed provider).

Failure mechanism:
- After GSL-PLAY-007, CLI doctor was narrowed to a finite local dump, but clap `about` and operator habit still treat it as the health command. The live check moved without updating the contract.

Break-it angle:
- Unset or break the provider; run `gosling doctor`. Exit 0, "complete". Run `gosling info --check`. Nonzero, classified failure.

Impact:
- CI/scripts/humans that run `doctor` as the install verifier get a false pass. `info --check` is honest but not what `--help` points at.

Operational impact:
- Blast radius: Workflow. Side-effect class: user-visible. Reversibility: reversible. Operator visibility: silent (false success). Rerun safety: safe.

Adjacent failure modes:
- REL-GSL-010 (same honesty class on HTTP). FSR-GSL-012 (the *other* doctor is too eager).

Recommended mitigation:
- Remediation patterns: `false_success_guard`, `operator_action_message`, `honest_health_check`.
- Minimal repair: change `about` + status line to "local diagnostics only; run `gosling info --check` to verify the provider"; or make doctor call `check_provider` and propagate the error.
- Behavior test: broken provider ⇒ either nonzero doctor or status/help that cannot be read as success.

Implementation assessment:
- Complexity: operator_ux. Cost: XS. Cost drivers: tests, docs. Nominal agent: gpt.

Validation:
- Assert `--help` / status text and, if doctor probes, exit code + "Auth:"/"Check:" labels.

Non-goals:
- Do not restore the old interactive `/doctor`-to-model CLI behavior.

Resilience mapping:
- Phase: anticipate. Objective(s): understand. Safe state: fail_visible.

Failure analysis (FMECA row):
- Failure mode / Likely cause / Operational phase: success exit without probe / contract not updated after dump-only repair / configure
- Local effect / Workflow effect / System-or-operator effect: exit 0 / install scripts pass / broken provider discovered later
- Detection method / Detection latency / Operator visible: none / never via doctor / false
- Compensating provision: `info --check` exists, unlinked from doctor help

Criticality:
- Likelihood: likely (first-run / docs still say doctor). Detectability: silent.

---

### REL-GSL-012: Bedrock, GCP Vertex, and Databricks retry deterministic 4xx

Severity: Medium  
Confidence: Confirmed (config property). Extra-latency manifestation Likely.  
Evidence basis: source-evidenced  
Domain: Reliability

Evidence:
- `crates/gosling-providers/src/retry.rs:28-36` — `Default` now sets `transient_only: true` (historical REL-GSL-004 **default** is fixed)
- `crates/gosling-providers/src/retry.rs:41-54` — `RetryConfig::new(...)` still sets `transient_only: false`
- `crates/gosling-providers/src/retry.rs:99-107` — `RequestFailed` is retried unless `transient_only` or a thinking-block marker
- Callers of `RetryConfig::new` **without** `.transient_only()`:
  - `crates/gosling/src/providers/bedrock.rs:219-224`
  - `crates/gosling/src/providers/gcpvertexai.rs:224-229`
  - `crates/gosling/src/providers/databricks.rs:180-185`
  - `crates/gosling/src/providers/databricks_v2.rs:159-164`
- Contrast: `crates/gosling-providers/src/ollama.rs:383-390` — same constructor **then** `.transient_only()`
- Tests in `retry.rs:266-271` only cover `Default`

Observed behavior:
- A 400 "model not found" / immutable-thinking-unrelated client error on Bedrock/GCP/Databricks is retried up to `max_retries` with exponential backoff (Bedrock interval cap 120s; GCP 320s). Default OpenAI-compatible path correctly refuses.

Expected boundary:
- `fail_visible` after the first deterministic 4xx. Safe state: `fail_visible`. Scenario SC-NET; observed `unsafe-continue` (retry storm on a doomed request).

Failure mechanism:
- Constructor predates the default-policy flip and was not migrated; only Ollama was updated.

Break-it angle:
- Point Bedrock/GCP/Databricks at a 400 mock; observe 1 + max_retries attempts.

Impact:
- Added latency/cost; slower fail for a typo'd model. Not a correctness fork.

Operational impact:
- Blast radius: Workflow. Side-effect class: network. Reversibility: reversible. Operator visibility: log-only. Rerun safety: safe.

Adjacent failure modes:
- Historical REL-GSL-004. No circuit breaker (acceptable at 3 retries).

Recommended mitigation:
- Remediation patterns: `retry_budget`.
- Minimal repair: `.transient_only()` on those four `retry_config()`s, or default `RetryConfig::new` to `true`.
- Behavior test: copy `default_config_skips_request_failed` against each adapter's `retry_config()`.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: tests. Nominal agent: codex.

Validation:
- `should_retry(RequestFailed(400), adapter.retry_config()) == false`.

Non-goals:
- Do not change max_retries / interval numbers in this slice.

Resilience mapping:
- Phase: withstand. Objective(s): constrain. Safe state: fail_visible.

Failure analysis (FMECA row):
- Failure mode / Likely cause / Operational phase: 4xx retried / constructor default left false / normal_run
- Local effect / Workflow effect / System-or-operator effect: extra HTTP / turn delayed / operator waits
- Detection method / Detection latency / Operator visible: log / immediate if tailing / false
- Compensating provision: `max_retries` cap + thinking-block denylist

Criticality:
- Likelihood: likely on those providers after a bad model name. Detectability: logged.

---

### REL-GSL-005: `McpClient` `assert!` on a second session id can panic the tool path

Severity: Medium  
Confidence: Confirmed for the `assert!`. Panic manifestation Likely (needs two session ids on one client).  
Evidence basis: source-evidenced  
Domain: Reliability

Evidence:
- `crates/gosling/src/agents/mcp_client.rs:233-239` — `assert!(slot.as_deref().is_none_or(|s| s == session_id), "McpClient received requests from different sessions")`
- Same file still uses `.expect("active_tool_calls mutex poisoned")` at `:182`, `:277`, `:300` (poison → panic)

Observed behavior:
- A programming / reuse invariant is enforced with `assert!` on the hot request path. If an extension client is shared across session ids, the process panics instead of returning `ErrorData`.

Expected boundary:
- `fail_visible`: return a structured MCP/internal error; do not abort the process. Scenario SC-INT/SC-USR; observed `unsafe-continue` (panic residue).

Failure mechanism:
- Debug invariant left as `assert!` rather than `return Err`.

Break-it angle:
- Reuse one `GoslingClient` for two session ids (subagent / extension leak).

Impact:
- Process death mid-turn. `tool_operations` should mark `in_doubt` on drop if the runtime is still up enough to spawn; a panic during unwind is messier.

Operational impact:
- Blast radius: Service. Side-effect class: process. Reversibility: compensatable. Operator visibility: obvious (crash). Rerun safety: unknown (in_doubt should block replay).

Adjacent failure modes:
- FSR-GSL-004 (hard kill orphans children). Mid-turn recovery otherwise held.

Recommended mitigation:
- Remediation patterns: `fail_closed_refusal`.
- Minimal repair: replace `assert!` with `Err(ErrorData::internal(...))`.
- Behavior test: second session id returns error, process lives.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: tests. Nominal agent: codex.

Validation:
- Unit test on `set_session_id` / request path.

Non-goals:
- Do not redesign session-scoped MCP clients here.

Resilience mapping:
- Phase: withstand. Objective(s): constrain. Safe state: fail_visible.

Failure analysis (FMECA row):
- Failure mode / Likely cause / Operational phase: panic / assert on reuse / normal_run
- Local effect / Workflow effect / System-or-operator effect: abort / turn dies / desktop reconnects
- Detection method / Detection latency / Operator visible: exception / immediate / true
- Compensating provision: none in-process

Criticality:
- Likelihood: unlikely (needs client reuse). Detectability: obvious.

---

### FSR-GSL-005: Planner and configure-tools still panic on missing provider/model

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Failsafe

Evidence:
- `crates/gosling-cli/src/session/mod.rs:2293-2309` — `get_reasoner()` `.expect("No provider configured. Run 'gosling configure' first")` and model twin; callers at `:721`, `:903` (`RunMode::Plan`)
- `crates/gosling-cli/src/commands/configure.rs:1632-1638` — `configure_tool_permissions_dialog` same `.expect`
- Contrast: `crates/gosling-cli/src/session/builder.rs:244`, `:559` — `output::render_error("No provider configured. Run 'gosling configure' first.")`
- Contrast: `crates/gosling/src/agents/agent.rs:934-937` — `provider()` returns `Err(anyhow!("Provider not set"))`

Observed behavior:
- Plan mode and the tools-permission dialog abort with a Rust panic/backtrace instead of the builder's `fail_visible` message. No side effects precede the expect (dialog has not yet created the session when provider is read).

Expected boundary:
- `fail_visible`: nonzero / dialog error with the same configure hint. Scenario SC-DEP; observed `safe-stop` ungracefully (`crash`).

Failure mechanism:
- Twin implementations of "provider required" were not migrated when the builder path was.

Break-it angle:
- Unconfigured `gosling` → plan mode or configure → tool permissions.

Impact:
- Noisy first-run crash; same information as a clean error.

Operational impact:
- Blast radius: Local. Side-effect class: none. Reversibility: reversible. Operator visibility: UI-visible. Rerun safety: safe.

Adjacent failure modes: none material.

Recommended mitigation:
- Remediation patterns: `startup_preflight`, `operator_action_message`.
- Minimal repair: `ok_or_else` + `bail!` / `render_error`, matching builder.
- Behavior test: unconfigured plan/configure-tools ⇒ no panic, message contains `gosling configure`.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Cost drivers: tests. Nominal agent: codex.

Validation:
- Exit/result + message; `should_panic` tests must not be the oracle.

Non-goals:
- Do not centralize all config validation.

Resilience mapping:
- Phase: anticipate. Objective(s): prevent_avoid, understand. Safe state: fail_visible.

Failure analysis (FMECA row):
- Failure mode / Likely cause / Operational phase: panic / `.expect` / configure
- Local effect / Workflow effect / System-or-operator effect: abort / command dies / backtrace
- Detection method / Detection latency / Operator visible: exception / immediate / true
- Compensating provision: panic text includes the hint

Criticality:
- Likelihood: likely on first-run plan. Detectability: obvious.

---

### SIG-GSL-005: ML classifier init failure still degrades to pattern-only with no interactive signal

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Failsafe

Evidence:
- `crates/gosling/src/security/mod.rs:112-128` — `with_ml_detection()` `Err` → `tracing::warn!` + `PromptInjectionScanner::new()` (pattern-only)
- Contrast (runtime, repaired): `crates/gosling/src/security/mod.rs:159-186` — `analysis_result.degraded` ⇒ `should_ask_user: true` + structured warn
- Contrast (init of *both* classifiers): `crates/gosling/src/security/scanner.rs:84-88` — `bail!` if ML enabled and **no** classifier inits; the SecurityManager catch still swallows that into patterns
- Security inspector does **not** auto-downgrade (`security_inspector.rs:61-66`)

Observed behavior:
- If the operator enabled ML scanning and the client cannot be constructed, every subsequent scan is pattern-only. The interactive user is not told at init time. A later *runtime* classifier error **does** force approval.

Expected boundary:
- `fail_degraded` with an announced contract, or `fail_closed` if ML was explicitly required. Scenario SC-DEG; observed `degraded-lying` at init (control appears enabled).

Failure mechanism:
- Init is treated as optional even after the operator set the ML flags; runtime was later fail-closed, init was not.

Break-it angle:
- Enable ML flags with a bad endpoint; start a session; first tool is pattern-scanned only, no approval.

Impact:
- Weaker injection detection than configured, until a runtime error happens.

Operational impact:
- Blast radius: Workflow. Side-effect class: none (until a malicious tool is allowed). Reversibility: reversible. Operator visibility: log-only. Rerun safety: safe.

Adjacent failure modes:
- Historical REL-GSL-002/003 (runtime half is held). Inspector-error fail-open is held.

Recommended mitigation:
- Remediation patterns: `degraded_mode_contract`, `degraded_status_signal`.
- Minimal repair: surface one session-level warning / doctor/`info` line when ML was requested and the scanner is pattern-only.
- Behavior test: bad classifier config ⇒ interactive/status signal, not only `warn!`.

Implementation assessment:
- Complexity: operator_ux. Cost: S. Cost drivers: tests. Nominal agent: gpt.

Validation:
- Assert a user-visible or `info --check` field `scanner=pattern-only (ML requested)`.

Non-goals:
- Do not disable pattern fallback.

Resilience mapping:
- Phase: anticipate. Objective(s): understand, continue. Safe state: fail_degraded.

Failure analysis (FMECA row):
- Failure mode / Likely cause / Operational phase: silent pattern fallback / catch at get_or_init / startup
- Local effect / Workflow effect / System-or-operator effect: no ML / weaker gate / operator thinks ML is on
- Detection method / Detection latency / Operator visible: log / delayed / false
- Compensating provision: runtime degrade now requires approval

Criticality:
- Likelihood: plausible (bad endpoint/token). Detectability: logged.

---

## 10. Non-findings (re-verified held)

Coverage mnemonics in **bold** were historical findings and are **closed at this HEAD**.

| Inventory | Verdict | Evidence |
|---|---|---|
| **REL-001** startup hidden | Held for `gosling serve` missing secret (`cli.rs:1502-1505` bail). Partial via REL-GSL-010 (process-up ≠ ready). | quoted above |
| **REL-002 / SIG-002** false healthy | **Finding** on serve (REL-GSL-010) and CLI doctor (REL-GSL-011). **Held** on `goslingd` `/status` (`status.rs:17-25`) and `info --check` (`info.rs:163-270`). | |
| **REL-003** silent degrade | **Partial** — SIG-GSL-005 init only. Runtime classifier degrade held (`security/mod.rs:159-186`). `list_extensions` empty-on-err (`agent.rs:1663-1670`) is log-only Low, not raised. | |
| **REL-004 / FSR-011** retry storm | **Held** for default: cap 3, backoff, jitter, `transient_only: true`, auth retry exactly 1 (`retry.rs:28-36`, `:203-222`, `:65-80`). **Finding** REL-GSL-012 on three adapters. Google `retryDelay` **clamped to 3600s** (`providers/utils.rs:99-105`, test `:529-541`). HTTP `Retry-After` clamped (`http_status.rs:32-69`). | |
| **REL-005 / REL-013** unbounded work | Held — `DEFAULT_MAX_GRIND_NUDGES=50`, `MAX_TURNS_MESSAGE`, MCP pagination caps (`extension_manager.rs:83-96`), default max_turns 1000. | |
| **REL-006 / FSR-007** missing timeout | Held on walked waits: OpenAI 600s, n_ctx probe 5s (`openai.rs:613-617`), MCP `await_response` uses extension timeout (default 300s), handshake timeout (`mcp_client.rs:647-651`), close 5s, reply heartbeat/cancel (`reply_service.rs:267-310`). | |
| **REL-007 / REC-001..007 / FSR-014 mid-turn crash** | **Held** — `tool_operations` start/complete/in_doubt/recover (`session_manager.rs:3611-3980`, `agent.rs:1216-1238`, recover on `reply` `:1748-1751`, ACP load `:227-230`, goslingd resume `:249-251`). Historical REC-GSL-001 **closed**. | |
| **REL-008 / REC-001 secrets atomicity** | **Held** — `write_secrets_file` temp + `sync_all` + rename (`base.rs:45-69`); `mutate_secrets` + flock (`:1205-1214`, `:1182-1202`). Historical REC-GSL-002 **closed**. `save_values` UUID temp + lock (`:776-828`). | |
| **REL-009** misclassified provider errors | Held — `http_status` + `ProviderError` kinds; `info --check` splits Auth vs Check (`info.rs:239-261`); doctor slash `describe_error` (`doctor.rs:244-270`). | |
| **REL-010** swallowed | Held on inspector errors (now fail-closed). ApiResponse parse **held** — empty vs malformed distinguished (`api_client.rs:221-232`). Historical REL-GSL-006 **closed**. | |
| **REL-011** partial success | Held for `handle_response_google_compat` requiring JSON (`utils.rs:133-134`). Doctor CLI is the exception (REL-GSL-011). | |
| **REL-012 / FSR-006** cleanup | Held for drop-kill + Linux PDEATHSIG (`subprocess.rs:52-69`). CLI cancel persists cancelled tool response (`session/mod.rs:1222-1238`). | |
| **REL-014** fragile default | Default retry **held**. `RetryConfig::new` still fragile (REL-GSL-012). | |
| **REL-015 / SIG-001** operator signal | Held on `info --check`. Failed on doctor help / serve `/status`. | |
| **FSR-001 / DEP-002** missing provider | Held on builder + `Agent::provider()`. Failed on planner expect (FSR-GSL-005). | |
| **FSR-002** misconfig | Held — typed-ish YAML; write refuses corrupt (`base.rs:647-662`, test `:2042-2057`); unknown keys not schema-rejected (sampled; no destructive default found). | |
| **FSR-003 / FSR-010** user error | Held on headless Approve/SmartApprove refuse (`session/mod.rs:1201-1214`). `/doctor` mutation is FSR-GSL-012. | |
| **FSR-004** startup refuse | Held for missing serve secret. | |
| **FSR-005 / FSR-015** cancel / abort | Held — CLI `ctrl_c` cancels token (`session/mod.rs:1156-1161`, `:1361-1369`); serve SIGINT/SIGTERM (`signal.rs:6-26`, `cli.rs:1555-1588`); reply loop distinct `Cancelled` (`reply_service.rs:27-50`, `:270-276`). | |
| **FSR-008 / FSR-016** intermittent / agent-provider | Held — classified errors + bounded retry. No cross-provider outage failover (RR-001, intentional). | |
| **FSR-009 / SC-COR** corrupt state | Held for config write (refuse). Session WAL. Secrets parse error returns `Err`; non-object YAML becomes `{}` (`base.rs:1308-1316`) — sampled, not raised (needs a subsequent write to clobber). | |
| **FSR-012 inspector fail-open** | **Held** — `inspect_tools` Err synthesizes `RequireApproval` even in Auto (`tool_inspection.rs:130-154`, test `:567-597`). Historical FSR-GSL-001 / DEP-GSL-002 / SIG-GSL-002 **closed**. Security/adversary/working-dir set `auto_downgrades_require_approval = false`. | |
| **FSR-013** notification | Partial — see SIG map. No paging product. | |
| **REC-003** tx boundary | Held per message / tool op. Multi-message turn is still N txs (historical REC-GSL-003) — acceptable now that tools are keyed. | |
| **REC-004 / REC-010** tool idempotency | Held — digest + `tool_request_id` key; replay vs in_doubt. | |
| **REC-008** locks | Held — flock released on drop; config save lock separate from extensions lock. | |
| **REC-009** migrations | Session/schema via sqlx; no user-facing forward-only migration advertised. Not a product migration engine. | |
| **REC-011** exactly-once claim | No "exactly-once" claim found on tool dispatch; in_doubt is honest at-least-once-with-hold. | |
| **REC-012** provenance | Session message ids + tool operation ids stable across recover. Not an artifact-hash pipeline. | |
| **DEP-001** files | Missing config → empty + warning; missing secrets → empty map. Honest enough. | |
| **DEP-003** credentials | Keyring fallback held. `info --check` classifies auth. | |
| **DEP-005 / DEP-010** port / CLIs | Held (bind error; spawn error). | |
| **DEP-006** queue | N/A — no broker. | |
| **DEP-011** schema/client | ACP generated types; drift not a failsafe walk here. | |
| **DEP-012** quota | 429 classified + Retry-After clamped. | |
| **DEP-013** approval | Held (headless refuse; Auto explicit). | |
| **SIG-003 / 004 / 005** progress / heartbeat | Held on reply `Ping` + 500ms poll (`reply_service.rs:261-299`); CLI thinking/spinners. No process-level watchdog beyond that — acceptable for interactive. | |
| **SIG-007** correlation | Session id on spans (`agent.rs:1736`); not a distributed trace. Sampled. | |
| **SIG-008** structured logs | Security events have fields. Provider retries are Debug prose. | |
| **SIG-009** reason propagation | Held on `info --check` and inspector approval. | |
| **SIG-011** alerting | N/A — no alert router in repo. | |
| **SIG-012** runbook hint | Held on `info --check` / builder. Missing on doctor CLI. | |
| **SIG-013** silent skip | Historical list_tools skip **closed** — `fetch_all_tools` fails the batch (`extension_manager.rs:2108-2157`). | |
| **FSR-GSL-003 egress** | Improved: outbound now `RequireApproval` (`egress_inspector.rs:395-399`). In Auto it still auto-downgrades (default `true`). Treated as **residual product contract**, not a new High finding. | |
| **FSR-GSL-004 macOS orphan** | Still true (`subprocess.rs:56-57`). Residual RR-FSR-01. Desktop `wait_for_process_exit` covers **goslingd←Electron**, not MCP grandchildren of a SIGKILL'd serve. | |

Agent-specific failsafe rows (required):

| Case | Class | Note |
|---|---|---|
| Model provider unavailable | `safe-stop` / `fail_visible` | first-use error; no failover (RR-001) |
| Wrong model | `safe-stop` | 4xx; Bedrock/GCP/Databricks may retry (REL-GSL-012) |
| Context too large | `safe-stop` | `ContextLengthExceeded` classified |
| Tool call timeout | `safe-stop` | MCP timeout → error; operation in_doubt if started |
| Tool result malformed | `safe-stop` | parse error fed back once |
| Agent loop terminate | `safe-stop` | max turns / grind nudges |
| Repeat destructive command | `fail_manual_hold` unless Auto | inspectors; Auto downgrades advisory only |
| CLI subprocess hang | `safe-stop` | kill_on_drop + timeout |
| Invalid JSON from model | `safe-stop` | provider parse errors |
| Fallback model semantic drift | **`unsafe-continue`** | only via `/doctor` auto-switch (FSR-GSL-012) |
| Rate limit partial state | `fail_visible` | turn fails after bounded retry |
| User interrupt mid-write | `fail_resumable` | cancel + in_doubt / cancelled response |

---

## 11. Scenario records (static-trace)

```yaml
- scenario: SC-DEG serve /status static-ok
  injection: static-trace
  predicted_safe_state: fail_visible
  observed_behavior: degraded-lying
  evidence: [acp/transport/mod.rs:216-241, backendStatus.ts:68-78, status.rs:17-25]
  residue: none
  disposition: REL-GSL-010

- scenario: SC-DEG CLI doctor
  injection: static-trace
  predicted_safe_state: fail_visible
  observed_behavior: degraded-lying
  evidence: [cli.rs:703-704, doctor.rs:7-31, info.rs:163-270]
  residue: none
  disposition: REL-GSL-011

- scenario: SC-DEP/SC-USR slash /doctor auto-switch
  injection: static-trace
  predicted_safe_state: fail_manual_hold
  observed_behavior: unsafe-continue
  evidence: [execute_commands.rs:132, doctor.rs:71-148]
  residue: mutated global provider/model
  disposition: FSR-GSL-012

- scenario: SC-NET 4xx on Bedrock/GCP/Databricks
  injection: static-trace
  predicted_safe_state: fail_visible
  observed_behavior: unsafe-continue
  evidence: [retry.rs:41-54, bedrock.rs:219-224, gcpvertexai.rs:224-229, databricks.rs:180-185]
  residue: extra HTTP
  disposition: REL-GSL-012

- scenario: SC-DEP planner missing provider
  injection: static-trace
  predicted_safe_state: fail_visible
  observed_behavior: safe-stop  # but crash presentation
  evidence: [session/mod.rs:2297-2309, configure.rs:1632-1638]
  residue: none
  disposition: FSR-GSL-005

- scenario: SC-INT mid-turn kill after tool start
  injection: static-trace
  predicted_safe_state: fail_resumable
  observed_behavior: safe-stop
  evidence: [session_manager.rs:3650-3657, 3746-3768, 3943-3950, agent.rs:1226-1237]
  residue: in_doubt row; no automatic re-dispatch
  disposition: non-finding

- scenario: SC-INT secrets write
  injection: static-trace
  predicted_safe_state: fail_idempotent
  observed_behavior: safe-stop
  evidence: [base.rs:45-69, 1182-1214]
  residue: possible leftover .tmp
  disposition: non-finding

- scenario: SC-DEP inspector Err
  injection: static-trace
  predicted_safe_state: fail_closed
  observed_behavior: safe-stop
  evidence: [tool_inspection.rs:130-154, 567-597]
  residue: none
  disposition: non-finding

- scenario: SC-NET Google hostile retryDelay
  injection: static-trace
  predicted_safe_state: fail_visible
  observed_behavior: degraded-honest  # clamped to 1h then retry budget
  evidence: [providers/utils.rs:99-105, http_status.rs:32-69, retry.rs:233-250]
  residue: up to 3 * 3600s worst case if a provider still supplied 1h thrice
  disposition: non-finding (historical REL-GSL-001 closed)

- scenario: SC-INT SIGINT during CLI turn
  injection: static-trace
  predicted_safe_state: fail_visible
  observed_behavior: safe-stop
  evidence: [session/mod.rs:1156-1161, 1361-1369, reply_service.rs:270-276]
  residue: cancelled/in_doubt tool rows
  disposition: non-finding
```

---

## 12. Readiness scorecard

| Subsystem | Families traced | Worst class | Det | Con | Rec | Sig | Grade | Driving evidence |
|---|---|---|---|---|---|---|---|---|
| `gosling serve` + desktop ready | CFG DEP NET STL DEG | degraded-lying (SC-DEG) | 0 | 2 | 2 | 0 | **not-ready** | `/status` = static ok |
| `goslingd` HTTP `/status` | DEP DEG | degraded-honest | 2 | 2 | 2 | 2 | **conditional** | honest 503; no provider probe (ok for this archetype) |
| CLI doctor | USR CFG DEP DEG | degraded-lying | 0 | 2 | 2 | 0 | **not-ready** | help vs dump |
| CLI `info --check` | DEP NET | safe-stop | 2 | 2 | 2 | 2 | **ready** (static cap 2) | classified + nonzero |
| Slash `/doctor` | USR DEP DEG | unsafe-continue | 2 | 0 | 1 | 2 | **not-ready** | `save_and_set` |
| Agent reply + tools | INT NET STL | safe-stop | 2 | 2 | 2 | 2 | **conditional** | ready if single-writer + recover-on-reply; C3 external residue remains |
| Tool inspection | DEP DEG | safe-stop | 2 | 2 | 2 | 2 | **conditional** | ready if Auto operators accept advisory egress downgrade |
| Secrets/config IO | INT COR | safe-stop | 2 | 2 | 2 | 1 | **conditional** | atomic write held; init ML signal weak |
| Planner / configure-tools | DEP CFG | safe-stop (crash) | 2 | 2 | 2 | 1 | **conditional** | refuse happens via panic |

Attention order (worst end effect, then reversibility, then detectability): slash `/doctor` mutation → serve false-ready → CLI doctor false-success → 4xx retries → MCP assert → planner expect → classifier init signal.

---

## 13. Break-it review

Executed as **static traces only** (authority `read_only`; no authorized drill).

| Attack | Result |
|---|---|
| Kill dependency + read `/status` | serve still `"ok"` (REL-GSL-010); goslingd would 503 |
| Empty provider stdout as success | Held — JSON required / `info --check` fails |
| 429 / hostile Retry-After | Held — clamped; retry budget |
| Crash mid tool | Held — no automatic re-dispatch |
| Crash mid secrets write | Held — dest not truncated |
| Unbounded input | Sampled held (pagination / turns) |
| Swallow path | Inspector Err held; doctor/status are the honesty holes |
| Retry storm on 4xx | Default held; Bedrock/GCP/Databricks not |
| Missing provider | Builder held; planner panics |
| Inspector Err | Fail-closed held |
| SIGINT turn | Token + cancelled/in_doubt held |
| `/doctor` while provider 429 | Would persist another provider (FSR-GSL-012) |

Oracle-integrity note: no test suite was used as a non-finding oracle for backup/restore. Held items are source properties, not "tests pass."

---

## 14. Residual risk register

| ID | Finding | Retained risk | Required control | Control present | Safe state | Owner | Review by |
|---|---|---|---|---|---|---|---|
| RR-001 | DEP-008 | Single selected LLM; outage has no failover | bounded_wait + fail_visible turn + rerun hint | **partial** (retry/timeout yes; `/status` no; no failover) | fail_visible | human-owner | only if product wants failover (not `/doctor` silent swap) |
| RR-FSR-01 | FSR-GSL-004 | macOS SIGKILL of parent can orphan MCP/provider children | kill_on_drop + Linux PDEATHSIG; desktop parent-wait for serve | **partial** | fail_visible | human-owner | if macOS supervisor is funded |
| RR-FSR-02 | — | SQLite `synchronous=Normal` last-commit window | WAL (no torn-page corruption) | true | fail_resumable | human-owner | null (documented tradeoff) |
| RR-FSR-03 | egress Auto | Auto mode still downgrades egress `RequireApproval` to Allow | document Auto = advisory egress; or set `auto_downgrades=false` | **partial** (Approve modes gate) | fail_degraded | human-owner | product decision observe vs enforce |
| RR-FSR-04 | C3 tool crash | External side effect may have happened; marked in_doubt | no auto-retry + operator text | true | fail_manual_hold | human-owner | null |

---

## 15. Skill escalation

| Finding | Primary | Secondary | Why |
|---|---|---|---|
| REL-GSL-010 | Reliability | Workflow-GUI, Operator-Signal | Desktop readiness widget believes `/status` |
| FSR-GSL-012 | Failsafe | Security, State-Transition, Cascade | Unconfirmed provider swap changes trust/cost; output becomes new default |
| REL-GSL-011 | Reliability | Workflow-GUI | CLI help/status truthfulness |
| REL-GSL-012 | Reliability | Cascade | Retry amplification on doomed 4xx |
| REL-GSL-005 | Reliability | Concurrency | Panic under unexpected session reuse |
| FSR-GSL-005 | Failsafe | Operator-Signal | Panic vs actionable refusal |
| SIG-GSL-005 | Operator-Signal | Security | Safety control silently weaker |

---

## 16. Bounded patch order

1. **REL-GSL-010** — serve `/status` → `SessionManager::healthy()` (XS/S, `codex`). Unblocks honest desktop readiness.  
2. **FSR-GSL-012** — `/doctor` report-only unless confirmed (S, `codex`). Stops global config mutation.  
3. **REL-GSL-011** — doctor help/status or live check (XS, `gpt`).  
4. **REL-GSL-012** — `.transient_only()` on Bedrock/GCP/Databricks (XS, `codex`).  
5. **REL-GSL-005** — `assert!` → `Err` (XS, `codex`).  
6. **FSR-GSL-005** — `.expect` → `bail!` (XS, `codex`).  
7. **SIG-GSL-005** — announce ML init fallback (S, `gpt`).

Do not implement `/doctor` auto-failover as the "fix" for RR-001.

---

## 17. Regression / guardrail tests (recommend, do not write)

- `status_returns_503_when_session_store_unreachable` — assert body, not just code.  
- `desktop_readiness_fails_on_status_503` (if the probe is wired).  
- `slash_doctor_does_not_call_set_active_provider_when_configured_provider_fails`.  
- `gosling_doctor_help_or_exit_not_success_when_provider_dead`.  
- `info_check_nonzero_on_auth_and_network` (likely exists; keep).  
- `bedrock_retry_config_skips_request_failed` (+ gcp, databricks, databricks_v2).  
- `mcp_client_second_session_id_returns_error`.  
- `get_reasoner_unconfigured_is_err_not_panic`.  
- `ml_init_failure_sets_degraded_flag_visible_to_info_or_session`.  
- Existing: `test_inspector_failure_fails_closed`, `test_parse_google_retry_delay_clamps_hostile_value`, tool-operation recover tests, `test_corrupt_config_skipped_on_read`.

---

## 18. Validation limits

- Static review only. No `gosling serve` / doctor / kill / closed-port drills in this run.  
- Manifestation of hang/panic/orphan/torn-file capped per `confidence_calibration.md`.  
- Not fully walked: every provider `stream()` after first event; OAuth refresh under loss; Ink TUI cancel; every `getenv` default; goslingd routes other than `/status`.  
- `gosling-server` `/status` honesty is **held**; it is **not** what desktop `gosling serve` uses.  
- Historical reports were not copied forward as current verdicts.

---

## 19. Inventory completion (every code)

### REL-001..015
001 partial (serve ready), 002 **REL-GSL-010/011**, 003 **SIG-GSL-005**, 004 **REL-GSL-012** (default held), 005 held, 006 held, 007 **REL-GSL-005** (mid-turn recover held), 008 held, 009 held, 010 held (inspector), 011 **REL-GSL-011**, 012 held, 013 held, 014 **REL-GSL-012**, 015 **REL-GSL-010/011**.

### FSR-001..016
001 **FSR-GSL-005** + held main path, 002 held, 003 **FSR-GSL-012**, 004 **FSR-GSL-005**, 005 held, 006 residual macOS, 007 held, 008 held, 009 held, 010 held, 011 **REL-GSL-012**, 012 **REL-GSL-010/011** + inspector held, 013 map above, 014 held, 015 **FSR-GSL-012**, 016 **REL-GSL-005** + held tool recover.

### REC-001..012
001 held, 002 held (`.tmp` residue only), 003 held, 004 held for tools, 005 held (keyed), 006 **REL-GSL-012** (4xx layer), 007 held, 008 held, 009 N/A-sampled, 010 held, 011 held (honest in_doubt), 012 sampled held.

### DEP-001..014
001 held, 002 **FSR-GSL-005**, 003 held, 004 held (inspector), 005 held, 006 N/A, 007 **REL-GSL-010**, 008 RR-001 + **FSR-GSL-012**, 009 SIG-GSL-005, 010 held, 011 not deep-walked, 012 held, 013 held, 014 **FSR-GSL-012**.

### SIG-001..013
001 **FSR-GSL-005** / doctor, 002 **REL-GSL-010**, 003 held, 004 held (reply ping), 005 held, 006 **SIG-GSL-005**, 007 sampled, 008 sampled, 009 held on info/inspector, 010 **REL-GSL-011** + **FSR-GSL-012**, 011 N/A, 012 missing on doctor, 013 list_tools held.

---

## Finding IDs + severities + path

| ID | Severity | Path |
|---|---|---|
| REL-GSL-010 | High | `docs/cloud/2026-08-15-audit-reliability-failsafe.md` |
| FSR-GSL-012 | High | `docs/cloud/2026-08-15-audit-reliability-failsafe.md` |
| REL-GSL-011 | Medium | `docs/cloud/2026-08-15-audit-reliability-failsafe.md` |
| REL-GSL-012 | Medium | `docs/cloud/2026-08-15-audit-reliability-failsafe.md` |
| REL-GSL-005 | Medium | `docs/cloud/2026-08-15-audit-reliability-failsafe.md` |
| FSR-GSL-005 | Low | `docs/cloud/2026-08-15-audit-reliability-failsafe.md` |
| SIG-GSL-005 | Low | `docs/cloud/2026-08-15-audit-reliability-failsafe.md` |
