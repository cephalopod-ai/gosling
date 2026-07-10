# Audit-Repair Campaign — Routed Items & Residual-Risk Backlog

**Date:** 2026-07-10
**Companion to:** `reports/2026-07-10-audit-skills-pack-report.md`
**Branch:** `claude/audit-skills-report-abcnsz` (PR #19)

This backlog holds the findings the `repair-defect-campaign` deliberately **did not
patch in-line**, because they are architectural rewrites, deliberate design tradeoffs,
or maintainer/security-posture decisions. Each should be triaged on its own — several
warrant a dedicated PR. (This file is the content for a follow-up GitHub issue; the
issue itself is pending GitHub-app re-authorization.)

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
