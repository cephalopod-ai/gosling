# Audit-Repair Campaign — Routed Items & Residual-Risk Backlog

**Date:** 2026-07-10
**Companion to:** `reports/2026-07-10-audit-skills-pack-report.md`
**Status:** merged to `main` as PR #19; reconciled at `6ab24704ea2b0698700ac1af88bd2cf0a74a0850`

This backlog holds the findings the `repair-defect-campaign` deliberately **did not
patch in-line**, because they are architectural rewrites, deliberate design tradeoffs,
or maintainer/security-posture decisions. Each should be triaged on its own — several
warrant a dedicated PR. (This file is the content for a follow-up GitHub issue; the
repository has GitHub Issues disabled, so this file is the canonical ledger.)

---

## Confirmed open work not deliberately deferred — reconciliation 2026-07-10

**Confirmation basis:** current `main` at `6ab24704`, current source, and the
GitHub Actions runs for that SHA. GitHub Issues are disabled for
`repo-makeover/gosling`, so this document is the canonical open-work ledger.
"Open" below means either a present source TODO with a concrete next action or a
current CI failure with an evidenced root cause. The routed security and
architecture work in sections A–G remains deliberately deferred and is not
duplicated in this section.

### Confirmed defects

| ID | Priority | Confirmed state | Next action |
| --- | --- | --- | --- |
| CI-001 | P0 | Resolved in `95d3c5ecc`: public GPT-5.3-Codex reasoning/context contracts and provider test expectations were reconciled; focused tests and the full `gosling` library suite passed. | Rerun required GitHub checks after the campaign PR is opened. |
| CI-002 | P0 | Resolved in `58f3c6afd`: removed the unreachable sanitizer that made schema generation fail under `-D warnings`; the generator and strict clippy passed. | Rerun the required GitHub schema check on the campaign PR. |
| CI-003 | P1 | Resolved in `8157ef494`: source and all 15 locale catalogs are synchronized; strict i18n and desktop lint checks passed. PR CI later exposed three stale desktop test contracts (code-runtime default, explicit managed-profile save, and compacted ACP load metadata); their assertions now match the existing product behavior. | None beyond normal PR checks. |
| CI-004 | P1 | Repaired in `db883f9d9`: absent Anthropic credentials now produce an explicit prerequisite skip and local preflight failure instead of false live-test failures. | A maintainer must add `ANTHROPIC_API_KEY` to exercise live compaction. |
| CI-005 | P1 | Repaired in `db883f9d9`: Claude Code smoke coverage is explicitly opt-in and requires a usable authenticated CLI. | A maintainer may set `RUN_CLAUDE_CODE_SMOKE=true` on a suitable runner. |
| CI-006 | P1 | Repaired in `dc00488a5`: Docker release compilation uses a lower-disk Thin-LTO profile and final-image-only Buildx cache export. | Manually dispatch or merge the workflow to verify two-architecture publishing on a hosted runner. |
| CI-007 | P1 | Repaired in `9b69f52a5`: both Pages workflows make deployment an explicit, visible opt-in. | To publish, enable Pages with Actions and set `ENABLE_GITHUB_PAGES=true`; otherwise no further action is needed. |
| DEF-001 | P2 | Resolved in `66996bdbb`: diagnostics search now matches/highlights text, navigates JSON-tree matches, and clears state correctly. | None beyond normal PR checks. |
| DEF-002 | P1 | Open and routed: ACP data-root isolation still depends on process-global configuration, paths, and request logging. A local patch would race across agents. | Use `repair-source-modularization` to inject per-agent configuration/state and request-log dependencies, then add concurrent multi-root tests. |
| DEF-003 | P2 | Open and routed: external ACP providers own one eagerly-created native session and cannot restore the native session associated with a loaded Gosling session. | Use `repair-source-modularization` to persist/map native session ids and make provider activation select native `session/load`; then unignore the four behavior tests. |

### Confirmed source TODOs and maintenance work

| ID | Scope | Confirmed state | Next action |
| --- | --- | --- | --- |
| TODO-001 | Tests | `OpenAiFixture` only models Chat Completions, leaving Responses-routed models without end-to-end coverage. | Add a Responses API fixture and move the affected ACP tests to it. |
| TODO-002 | Desktop configuration | Extension timeout and `nameToKey` are duplicated in TypeScript and Rust, so they can drift. | Share the contract or add cross-language contract tests. |
| TODO-003 | Provider data | Recommended models come only from the bundled canonical registry, and disabled context-limit probing leaves freshness to static data. | Decide and implement a bounded refresh/probe strategy, with an offline fallback. |
| TODO-004 | Vertex AI | MaaS requests always use Google format even though publisher-specific formats are unknown. | Add publisher-to-format selection and compatibility tests. |
| TODO-005 | Observability | The temporary OTLP temporality helper is still present although its upstream prerequisite, OpenTelemetry Rust PR #3351, merged on 2026-02-17. | Verify the supported dependency release, remove the workaround if no longer needed, and test metrics export. |
| TODO-006 | Desktop permission UI | The UI injects a synthetic `platform` extension instead of representing it as a real extension. | Model it at the extension boundary or make the distinction explicit in the UI contract. |
| TODO-007 | ACP migration | `goslingd` retains a desktop ACP bridge until `gosling serve` provides equivalent initialization and platform identity. | Complete the direct-launch migration, verify desktop ACP behavior, then remove the bridge. |
| TODO-008 | Orchestration | Subagents cannot select a fast versus standard model tier. | Add and validate the optional `model_tier` parameter. |
| TODO-009 | Desktop cleanup | `DEFAULT_CHAT_TITLE` is marked as obsolete in `ChatContext`. | Confirm callers no longer require it and remove or retain it without the stale TODO. |
| TODO-010 | Database tooling | The rollback helper cannot generate inverses for unsupported migration statements and emits a manual TODO before returning failure. | Expand supported reversible SQL or require explicit rollback SQL in migrations. |

### Deliberately deferred, not counted above

Sections A–G and the deferred findings in
`reports/2026-07-09-defect-audit-and-repair.md` retain their deliberate-deferment
status. The cargo-deny failure is included there as section F: current CI now
reports `quick-xml 0.39.4` advisories `RUSTSEC-2026-0194` and
`RUSTSEC-2026-0195` through `bat -> plist`. It remains a maintainer dependency
decision, not a newly reclassified active item.

---

## A. Permission model — per-argument authorization (SECURITY, design)
**Findings:** NEG-002 / STT-001 / LLM-AGY-001, STT-002, NEG-001, NEG-004, NEG-005.
**Why routed:** changes the default `SmartApprove` security model, not a local guardrail.
**Scope:** `permission_inspector.rs`, `permission_judge.rs`, `config/permission.rs`,
`tool_monitor.rs`.
**Direction:**
- Classify per-call **including arguments**, not tool name alone; never cache a blanket
  `AlwaysAllow` for a tool whose args can carry a sink (url/path/body/command).
- Scope `AlwaysAllow` grants to session or working-dir instead of a single global key.
- Stop treating MCP-server-declared `read_only_hint` as a security decision.
- Namespace permission keys by extension/server identity (unprefixed-tool collision).
- Give the repetition loop-guard a safe default cap.
- Needs a migration story for existing `permission.yaml` entries + negative tests
  (hostile annotation, mislabeled-write tool).

## B. Turn atomicity — persist-before-side-effect (RELIABILITY/DATA, design)
**Findings:** pipeline REL-GOS-001, recovery-idempotency turn-write, STT-003, DAT-GOS-002/003.
**Why routed:** persistence redesign touching `agent.rs` (4217 LOC — above the 2000-line
in-stage-split ceiling, so any fix here is smallest-safe-only) and `session_manager.rs`.
**Direction:**
- Persist the assistant `tool_request` **before** dispatching the tool; dedupe on
  `tool_call_id` at resume so a crash mid-turn can't re-run a non-idempotent side effect.
- Wrap a turn's messages in a single transactional append (no orphan `tool_request`).
- On cancel, synthesize a `cancelled` tool_response for every unanswered request id.
- Carry `restrict_tools_to_working_dirs` + `provider_name`/`model_config` on session
  copy/import; preserve tool-error `code`/`data` in `tool_result_serde`.
- Route a dedicated `repair-source-modularization` pass for `agent.rs` first.

## C. Subagent authority (SECURITY, design)
**Finding:** AOC-001 (delegated subagents hardcode `GoslingMode::Auto`).
**Why routed:** the code comment notes approval-requiring modes would *hang* subagents;
the fix is to forward subagent `ActionRequired` to the parent confirmation channel, which
is a control-flow change, not a flag flip. Also AOC-019 (default least-authority toolset).

## D. Extension-failure isolation (RELIABILITY, tradeoff)
**Finding:** CAS-GS-001 (one MCP extension's `list_tools` failure aborts the whole turn).
**Why routed:** current fail-visible behavior is **intentional**, asserted by a test
(`extension_manager.rs:2579`). Changing to per-extension drop-with-warning is a product
tradeoff for the maintainers. CAS-GS-002 (cache the degraded result) can ride along.

## E. `gosling serve` security defaults (SECURITY, CLI-behavior)
**Findings:** SEC-SRV-001 (unauth + non-loopback bind = networked shell-capable agent),
SEC-SRV-002 (plaintext HTTP default leaks the secret on non-loopback).
**Why routed:** guarded today by an explicit `--dangerously-unauthenticated` flag;
hardening (refuse unauth unless host is loopback; default TLS on / fail-closed on
non-loopback plaintext; enforce origin on all `/acp` methods, not just WS) changes
documented CLI behavior and deserves a maintainer sign-off + changelog.

## F. Supply-chain advisory — `quick-xml` via `bat` (SECURITY, dependency)
**Finding:** cargo-deny fails on `quick-xml v0.39.4` (RUSTSEC OOM advisory) pulled by
`bat 0.26.1 → plist 1.9.0 → gosling-cli`.
**Why routed:** `plist 1.9.0` pins `quick-xml ^0.39`, so the clean fix (`>= 0.41`)
requires bumping **`bat`** to a release using a newer `plist` — a behavior-bearing
dependency bump that can't be built in this sandbox (v8-goose binary download is
proxy-blocked). Alternative is a `deny.toml` advisory-ignore, which is a security-posture
decision (matches the existing accepted-risk `RUSTSEC-2023-0071` rsa ignore). Maintainer
call. Also RSP-201 (widen cargo-deny to bans/licenses/sources) and RSP-202 (tighten
Dependabot minor auto-merge) live here.

## G. Desktop & server security/signals — sandbox-unverifiable (bounded, deferred)
Attempted under "server + desktop, static-only"; the safe subset landed in stage 7
(contrast A11Y-GOS-001, export error handling WFG-GOS-002). The rest are deferred
because they can't be built/linted or run in this sandbox and carry real risk:
- **SEC-N-001 `open-in-chrome` cmd.exe injection** (`ui/desktop/src/main.ts`): Windows
  path `spawn('cmd.exe', ['/c','start','','chrome', url])` splits URLs on cmd
  metacharacters (`&|^%`) — a correctness bug for legit multi-param URLs *and* an
  injection vector. A correct fix needs either a Chrome-locator + `execFile` or a switch
  to `shell.openExternal` (default browser, a product decision), and can't be tested on
  Windows here.
- **SEC-ACP-003 ACP `?token=` query auth** (`crates/gosling/src/acp/transport/auth.rs`):
  the desktop client authenticates via `ws://…/acp?token=…`, so the query path can't be
  removed without first moving clients to header auth (or restricting query-token to WS
  upgrades) — a coordinated change.
- **LLM-EXF-001 markdown-image exfiltration** (`MarkdownContent.tsx`/`csp.ts`): blocking
  external `<img>` in model output closes the exfil channel but stops legitimate image
  rendering — a CSP/UX product decision.
- **SEC-N-002 will-navigate guard, A11Y-GOS-002/003/004** (BaseModal semantics, form
  aria-required/role=alert, unlabeled header controls): additive and low-risk, but land in
  the desktop lint gate which is currently red for an undiagnosable reason (GitHub app
  de-authorized, no local node toolchain) — deferred until the lint gate can be run.
- **Server signals** (`gosling-server`): honest `/status` (FSR-SRV-001), threading the real
  loop-exit reason into `exit_type`/`Finish.reason` (FSR-SRV-002), and error-cause
  propagation instead of bare 500/404 (FSR-SRV-003). The crate can't link locally
  (proxy-blocked v8-goose download), so unverified Rust here risks an uncatchable compile
  error that would add CI red — deferred to a session that can build it.

---

## Pre-existing CI failures (NOT from this campaign; maintainer action)
Surfaced while triaging PR #19; predate this branch:
- **Build and Test Rust** — `providers::codex::tests::test_known_model_context_limits`
  and a chatgpt_codex reasoning-effort test assert stale gpt-5.x context numbers. Needs a
  decision on the correct values (code vs test).
- **Compaction Tests, Smoke Tests, Check Generated Schemas** — red on the base SHA before
  this branch existed.
- The `deny`/`machete` gates are path-filtered and were *skipped* on docs-only commits;
  any Cargo-touching PR re-runs them and hits the latent `quick-xml` advisory (see F).

## Environment note
`gosling-cli` and `gosling-server` transitively pull `v8-goose`, whose build script
downloads prebuilt binaries that this sandbox's egress proxy blocks (HTTP 403). Changes
in those two crates are reference-verified statically but their final link is left to CI.
