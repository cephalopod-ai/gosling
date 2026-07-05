# Gosling Repair Campaign — Session Log & Disposition Ledger

Follow-up to the stress-test audit (`99-master-report.md`). Executed with
`20_repair/repair-defect-campaign`. Authority: patch-authorized on branch
`claude/gosling-stress-test-audit-jwhooa` (audit PR #4 already merged to `main`;
this is a **fresh change** restarted from the merged `main`).

## Scope decision (honest)

The user asked to "fix all findings." The campaign skill itself **excludes**
feature work, architectural rewrites, and human-owner/policy/legal decisions, and
directs that those be dispositioned with reasons rather than blind-fixed.
Combined with the "no misleading results / mission-critical" mandate and the fact
that this environment **cannot run the app or its provider/UI integration tests**
(`cargo build -p gosling-cli` fails on a network-gated static-lib download; pnpm
install unverified), the campaign fixed **every eligible defect that is a genuine
bug, localized, and compile+test-verifiable in the core `gosling` crate**, and
dispositioned the rest with concrete recommendations.

## Gate 0 — posture & baseline

- Git: branch reset onto merged `main` (`e1898d5`); audit docs preserved.
- Toolchain: hermit cargo 1.92 available. `cargo check -p gosling --features
  nostr` works (the network block is specific to `gosling-cli`'s static-lib dep).
- **Baseline defect discovered:** `cargo check -p gosling` with default features
  (`default = []`) was **red** — `acp/server/manage_sessions.rs:282` called the
  `#[cfg(feature="nostr")]` module unconditionally. The shipped CLI/server enable
  `nostr`, so this was latent. Fixed in Stage 1.
- Verification harness for the campaign: `cargo check/test -p gosling --features
  nostr` (matches the shipped product's feature set).

## Stages executed (all compile + test verified)

| Stage | Commit | Defects fixed | Validation |
|---|---|---|---|
| 1 — Secrets & build integrity | `0dd40e8` | Atomic `write_secrets_file` (temp+sync+rename, mirrors `save_values`) so a crash can't lose all API keys/tokens; gate `is_nostr_session_link` so the default-feature build compiles | default build now green (was red); `--features nostr` green; `config::base` 54 tests pass |
| 2 — Provider reliability | `e90bf3e` | Clamp Google 429 `retryDelay` to 3600s (anti-freeze); bound the Bedrock mantle (600s) and GCP metadata (10s) HTTP clients that had no timeout | `--features nostr` green; `providers::utils` retry-delay tests pass incl. new clamp regression test |
| 3 — Untrusted-input exec guard | `d0b057d` | `git clone --` end-of-options terminator in plugin install (a `source` starting with `-` could inject a git option) | `--features nostr` green |
| 4 — Fail-closed safety control | `b1b39fd` | `ToolInspectionManager::inspect_tools` now synthesizes `RequireApproval` when an inspector errors, instead of dropping its verdict (in Auto mode the dropped restriction let an unjudged tool run) | `--features nostr` green; new `test_inspector_failure_fails_closed` passes; clippy-clean |

**Gate 9 final regression:** `cargo test -p gosling --features nostr --lib` =
**1282 passed / 0 failed**. `cargo clippy -p gosling --features nostr --lib` clean
on all changed production files. `cargo fmt` applied.

New regression tests added: `test_parse_google_retry_delay_clamps_hostile_value`,
`test_inspector_failure_fails_closed`.

## What each fix maps to in the audit

- Stage 1 secrets → master-report Cluster C (concurrency/integrity/recovery all
  found the non-atomic write). Stage 1 build fix → a defect the audit missed;
  found by the campaign's Gate-0 compile baseline.
- Stage 2 → Cluster D (reliability REL-GSL-001; pipeline-externalapi EXT-GSL-003).
- Stage 3 → Cluster B (input-output IOP git-clone).
- Stage 4 → Cluster A fail-open inspector (FSR-GSL-001, DEP-GSL-002, PGR, SIG).

## Disposition ledger — NOT fixed this campaign (with reasons)

These are real findings deliberately **not** blind-patched. Each is either a
product-policy/human-owner decision, a UI/feature change, an architectural
rewrite, or a change whose regression surface can't be verified in this
environment. Recommended owner/route given for each.

### Deferred — product-policy / owner decision (flipping a default is not a bug-fix)
- **Default `GoslingMode = Auto`** (auto-approve everything). Changing the product's
  default trust posture is a policy call for the maintainers, not a silent patch.
  Recommend: ship `SmartApprove` (or `Approve`) as the default. → human-owner.
- **Prompt-injection scanner default-off** and **`EgressInspector` telemetry-only
  (always Allow)**. Decide whether these are *controls* (make them enforce +
  default-on + scan tool/MCP results, the real injection ingress) or *telemetry*
  (stop presenting them as mitigations). → human-owner + a follow-up repair once
  the policy is set.
- **Retry `transient_only=false` default** (retries non-retryable 4xx). An existing
  test pins current behavior, i.e. it is intentional-and-tested; flipping it is a
  retry-semantics decision. → human-owner. (Stage 2 fixed the unbounded-delay half,
  which is unambiguously a bug.)

### Routed — UI / cross-surface (needs the app running to verify; route to repair-design-webapp)
- **TUI auto-approves `options[0]`=AllowAlways** (WFG-GSL-001); **ACP app tool
  entrypoint bypasses the permission gate** (CTR-GSL-001); **desktop approval UI**
  (destructive `Always Allow` styled like `Allow Once`, args hidden, no
  `aria-live`) (WEB-GSL-001/002/003); **desktop fake-success / dropped error**
  (WFG-GSL-002/003); **Electron CSP `unsafe-inline` + unconfined fs IPC**
  (SECN-GSL-001/003). These touch TS/Electron code that can't be typecheck/run-
  verified here and carry real UX/breakage risk. → `repair-design-webapp` /
  `repair-defect-nodejs` with a live build.

### Deferred — needs an integration/drill environment to change safely
- **MCP stdio subprocess inherits full env** (leaks provider keys). The safe fix is
  a targeted allowlist/denylist (the Docker path `env_clear`s), but clearing env
  wrong breaks legitimate extensions and can't be integration-tested here. →
  follow-up with an extension test harness.
- **Untrusted-repo auto-enabled project plugins/hooks/`.mcp.json`** (Cluster B core):
  needs a *workspace-trust prompt* — a small feature, not a one-line guard. →
  design + implement with the maintainers.
- **Mid-turn crash replays tool side effects** (REC-GSL-001): needs an atomic turn
  boundary / idempotency key — a data-path change best done with crash-drill
  verification. → dedicated repair with drills.
- **Full HTTP-client timeout sweep** (~18 remaining bare `reqwest::Client::new()`
  in xai_oauth/oauth_device_flow/oauth): a shared default-timeout client is the
  right fix but touches auth flows that can't be exercised here. → follow-up.

### Routed — architecture (STOP per skill; route to repair-source-modularization)
- Inverted domain ownership (`Message` in the providers crate), `agent.rs` god
  orchestrator (3.8K LOC), 256-site config singleton, cross-language type-parity
  gate. These are architectural rewrites the campaign skill explicitly excludes. →
  `repair-source-modularization` / architecture work.

### Deferred — human-owner / legal / docs
- README version identity contradiction (v0.0.1 vs v1.40.0) and the **AAIF vs
  "Copyright 2024 Block, Inc."** attribution contradiction: which value is correct
  is a product/legal decision, unsafe to guess. → human-owner.

## Final status

`completed_verified` for the in-scope eligible-defect set (Stages 1–4, all
compile+test verified, 1282/0). Remaining audit findings are **dispositioned**
above (deferred / routed / human-owner) with reasons and recommended next
actions — not silently skipped. The highest-leverage next step is a maintainer
decision on the Cluster A defaults, after which the enforce-the-controls follow-up
becomes a mechanical, testable change.
