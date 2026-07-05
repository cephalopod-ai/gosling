# Gosling — Repository & CI/CD Security Triage (Lens: audit-security-repo-triage)

Authority: **audit-only / read-only**. No repo or platform state mutated. No secret values reproduced.
Builds on `docs/cloud/00-orientation.md`. Finding IDs: `TRIAGE-GSL-NNN`.
Skill order followed: (1) secrets → (2) workflow permissions → (3) dangerous triggers →
(4) third-party actions → (5) script injection → (6) deps → (8) publish path. Trust-boundary
core (1–5) received most of the budget, as the skill requires.

## Effort budget

~22 tool calls, strictly risk-ordered. Full read of `.github/workflows/*` structure (39 workflows),
the four `issue_comment`-triggered build workflows, both `pull_request_target` workflows, custom
composite actions, `dependabot.yml`, `CODEOWNERS`, and a repo-wide secret pattern sweep. Deep
line-by-line read was spent on the highest-risk workflows; lower-risk trusted-context workflows
(release/publish/schedule) were trigger-and-permission sampled, not fully read. See Validation Limits.

---

## Highest-risk path (lead)

**Fork/unauthorized PR comment → PR-head code build with cloud OIDC (`id-token: write`) available,
because two of four comment-triggered build workflows discard their authorization gate**
(`TRIAGE-GSL-001`). This is the single shortest credible path from untrusted input (a PR comment
from anyone) to attacker-controlled code executing in a job that can mint AWS/Azure OIDC tokens.

---

## Findings

### TRIAGE-GSL-001: Comment-build workflows (Windows/Intel) hardcode the authorization output, bypassing the write-access gate their siblings enforce

Severity: High (Critical if reachable)
Confidence: Likely (structural); exploitability Plausible pending `github/command` semantics
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `.github/workflows/pr-comment-bundle-windows.yml:36` — job output `continue: 'true'` (hardcoded literal).
- `.github/workflows/pr-comment-bundle-intel.yml:33` — job output `continue: 'true'` (hardcoded literal).
- Trigger is untrusted: `pr-comment-bundle-windows.yml:4-6` / `:30-32`
  `issue_comment: types:[created]` + `contains(github.event.comment.body, '.bundle-windows')`.
- Top-level `permissions: id-token: write` at `pr-comment-bundle-windows.yml:15-19` (AWS OIDC), and the
  called reusable build workflow checks out and builds PR head: `:64-71` `uses: ./.github/workflows/bundle-desktop-windows.yml with: ref: ${{ needs.trigger-on-command.outputs.head_sha }}`.
- Contrast — siblings gate on a real permission check:
  `.github/workflows/pr-comment-bundle.yml:36` `continue: ${{ steps.security_check.outputs.authorized }}`
  fed by `getCollaboratorPermissionLevel` at `pr-comment-bundle.yml:58`; identical real gate in
  `.github/workflows/pr-comment-build-cli.yml:35` + `:57`.
- The SECURITY headers in the gated siblings (`pr-comment-bundle.yml:4-6`, `pr-comment-build-cli.yml:3-5`)
  explicitly cite `GHSA-4h72-4h3w-4587` / `GHSA-mqm8-hhf6-wvjq` — the maintainers deemed the built-in
  `github/command` gate insufficient and added an explicit check, but did **not** apply it to the
  Windows and Intel variants.

Observed behavior:
- On `.bundle-windows` / `.bundle-intel` comments, the trigger job runs `github/command@…v2.0.3`
  (`pr-comment-bundle-windows.yml:42-48`) which performs its own commenter-permission check, but the job
  then emits a **constant** `continue: 'true'` regardless of that check's result, and the downstream
  build job gates only on `needs.trigger-on-command.outputs.continue == 'true'` (`:67`).

Expected boundary:
- The authorization decision (commenter has `write`/`maintain`/`admin`) must gate whether PR-head code is
  built with privileged OIDC tokens — exactly as the ARM/CLI siblings implement.

Failure mechanism:
- The workflow delegates authorization entirely to `github/command`'s internal behavior, then throws away
  its result by hardcoding the gating output. If `github/command@v2.0.3` signals denial via
  `steps.command.outputs.continue == 'false'` and exits 0 (its documented IssueOps pattern) rather than
  hard-failing the step, an unauthorized commenter's PR head is built with `id-token: write` in scope.

Break-it angle:
- Fork PR author comments `.bundle-windows`; permission check denies but does not fail the step; job still
  outputs `continue:'true'`; `bundle-desktop-windows.yml` builds attacker code where an OIDC token can be
  requested and exfiltrated, or malicious build scripts run in a job holding cloud federation.

Impact:
- Potential arbitrary code execution on a CI runner that can mint AWS (Windows path) / cloud OIDC tokens
  and holds `pull-requests: write`. Blast radius reaches cloud signing/deploy identity, not just the repo.

Operational impact:
- Blast radius: Cross-system (CI → cloud OIDC). Side-effect class: process/network/external API.
  Reversibility: irreversible if a token is exfiltrated (rotate). Operator visibility: log-only.
  Rerun safety: unsafe.

Adjacent failure modes:
- Same class as TRIAGE-GSL-004 (implicit token scope) if `github/command`'s gate is the only line of defense.

Recommended mitigation:
- Minimal repair: mirror the siblings — set `continue: ${{ steps.<check>.outputs.authorized }}` driven by a
  `getCollaboratorPermissionLevel` github-script step, in both Windows and Intel workflows.
- Local guardrail: never emit a constant authorization output; the gate value must be data-flow-derived.
- Behavior test: simulate an `issue_comment` from a non-collaborator login and assert the `bundle-*` job is
  skipped (job `if` evaluates false) and no OIDC step runs.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Cost drivers: two workflow edits + a CI assertion.
  Nominal agent: codex. Rationale: localized, mirrors an existing in-repo pattern.

Validation:
- Assert the boundary (skipped job for unauthorized commenter), not the presence of a string.

Non-goals:
- Do not touch the ARM/CLI workflows (already gated) or the publish path.

---

### TRIAGE-GSL-002: Mutable-tag third-party actions on privileged paths (inconsistent with repo's own SHA-pinning)

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `.github/workflows/bundle-desktop-windows.yml:87` `uses: Jimver/cuda-toolkit@v0.2.35` — third-party
  action, mutable tag, runs inside the Windows bundle build that is reachable with `id-token: write`.
- `.github/workflows/pr-comment-build-cli.yml:132` `uses: peter-evans/create-or-update-comment@v5` —
  third-party, mutable tag, in a job holding `pull-requests: write`. The **same** action is SHA-pinned in
  `.github/workflows/pr-comment-bundle.yml:181` (`@e8674b0…  # v5.0.0`) — inconsistent hardening.
- GitHub-owned actions also tag-pinned (lower risk, dependabot-covered): `pr-smoke-test.yml:99,133,219`
  `actions/setup-*@v6`; `scorecard.yml:76` `github/codeql-action/upload-sarif@v4`; `autoclose:13`
  `actions/stale@v9`; `cargo-deny.yml:25` `actions/checkout@v7`.

Observed behavior:
- A mutable tag lets the upstream owner (or a tag-hijack) change the code a privileged job executes.

Expected boundary:
- Third-party automation on privileged paths should be pinned to a full-length commit SHA (the repo's own
  convention for most actions).

Failure mechanism / Break-it angle:
- Compromise or retag of `Jimver/cuda-toolkit` or `peter-evans/create-or-update-comment` silently alters CI
  behavior on the next run; the cuda-toolkit case executes in a build with OIDC in scope.

Impact:
- Supply-chain code execution on privileged runners; the `create-or-update-comment` case can also forge
  maintainer-looking PR comments.

Operational impact:
- Blast radius: Service→Cross-system (cuda path). Reversibility: compensatable (re-pin). Visibility: silent.
  Rerun safety: unsafe on a poisoned tag.

Recommended mitigation:
- Pin all third-party `uses:` to full SHAs with a `# vX.Y.Z` comment. Prioritize `Jimver/cuda-toolkit`
  (OIDC-adjacent) and `peter-evans/create-or-update-comment@v5` in build-cli.
- Behavior test: a CI lint (e.g. a small `grep` gate or `zizmor`/actionlint) failing any non-`./`
  third-party `uses:` that is not a 40-hex SHA.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Nominal agent: codex.

Non-goals:
- GitHub-owned `actions/*` tag pins may stay (dependabot-managed) if a policy explicitly allows them.

---

### TRIAGE-GSL-003: Dependabot auto-approve+merge covers all ecosystems (incl. github-actions) behind a single opaque CODEOWNER

Severity: Low (Medium if the actions ecosystem is included in auto-merge in practice)
Confidence: Likely
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `.github/workflows/dependabot-auto-merge.yml:21-34` auto-`approve` + auto-`merge --auto` for
  `version-update:semver-patch` and `…-minor`, gated only on `pull_request_target` +
  `github.event.pull_request.user.login == 'dependabot[bot]' && github.repository == 'repo-makeover/gosling'`
  (`:13`).
- `.github/dependabot.yml:29-33` enables the `github-actions` ecosystem, so action-version bumps arrive as
  dependabot PRs eligible for the same auto-merge.
- `.github/CODEOWNERS:2` `* @e3742526` — a single reviewer owns everything; auto-approve substitutes for
  human review on minor/patch.

Observed behavior:
- Patch/minor dependency bumps (npm, cargo, and github-actions) can be approved and merged with no human in
  the loop.

Expected boundary:
- Automated merges of dependencies that can alter build/CI behavior (github-actions) should require human
  review, or auto-merge should be scoped to exclude the actions ecosystem.

Failure mechanism / Break-it angle:
- A malicious "patch" release of a monitored action or library merges to `main` unattended; combined with
  TRIAGE-GSL-002's mutable tags this widens the unattended supply-chain surface. The trigger gate itself is
  sound (`dependabot[bot]` login is GitHub-set, not spoofable; repo pinned), so this is a policy/blast-radius
  concern, not a trigger bypass.

Impact:
- Unreviewed supply-chain change to the default branch. Blast radius: Repo→Service.

Recommended mitigation:
- Exclude `github-actions` (and optionally cargo) from auto-merge, or require the update to pass a pinned-SHA
  and provenance check before `--auto` merge. Behavior test: a dependabot actions PR is not auto-merged
  without a passing review gate.

Implementation assessment:
- Complexity: governance_decision. Cost: S. Nominal agent: human-owner (policy) then codex.

---

### TRIAGE-GSL-004: 16 workflows declare no top-level `permissions:` (no least-privilege token scope)

Severity: Low / Info
Confidence: Confirmed (missing declaration); actual granted scope not_observable
Evidence basis: source-evidenced
Domain: Security

Evidence:
- No `^permissions:` block in: `build-cli.yml`, `bundle-desktop{,-intel,-linux,-manual}.yml`,
  `cargo-deny.yml`, `cargo-machete.yml`, `deploy-docs-and-extensions.yml`, `pr-smoke-test.yml`,
  `pr-website-preview.yml`, `rebuild-skills-marketplace.yml`, `release-branches.yml`, `take.yml`,
  `update-hacktoberfest-leaderboard.yml`, `check-release-pr.yaml`, `autoclose`.
- Mitigating context: several are reusable workflows called with explicit caller permissions
  (`bundle-desktop*.yml`, `build-cli.yml`); a few set job-level scope (`take.yml:11-12` `issues: write`,
  `autoclose:10`). The PR-triggered ones (`pr-website-preview.yml:4` `pull_request`, `pr-smoke-test.yml:2`
  `pull_request`) receive GitHub's forced read-only token for fork PRs, bounding the exposure.

Observed behavior:
- These workflows inherit the repository/organization default `GITHUB_TOKEN` scope rather than pinning
  least privilege.

Expected boundary:
- Every workflow should set an explicit minimal top-level `permissions:` (defense-in-depth), even when the
  effective default is read-only.

Failure mechanism:
- If the repo/org default token scope is "read and write," non-fork-triggered runs (push/schedule/dispatch)
  execute with broad write ability they do not need.

Impact:
- Over-broad token on trusted-context runs. Blast radius: Repo. The actual default is a platform setting
  **not observable** from repo files.

Recommended mitigation:
- Add explicit top-level `permissions:` (default `contents: read`, widened per job) to each listed workflow;
  confirm the org default token scope is read-only. Behavior test: actionlint/zizmor rule requiring an
  explicit top-level permissions block.

Implementation assessment:
- Complexity: workflow_protocol. Cost: S. Nominal agent: codex.

---

### TRIAGE-GSL-005: `quarantine.yml` performs an unused `pull_request_target` checkout (latent footgun)

Severity: Low / Info
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `.github/workflows/quarantine.yml:4` `pull_request_target` (runs with base-repo secrets + write token),
  `:8-10` `pull-requests: write` + `issues: write`, `:16-17` `actions/checkout@…v7.0.0` with no `ref`.
- The checked-out tree is never built or executed; the job only reads
  `github.event.pull_request.user.login` (`:21`) and compares against `vars.QUARANTINED_USERS` (`:24`).

Observed behavior:
- Under `pull_request_target` the no-ref checkout resolves to the trusted base branch, so today the checkout
  is harmless but pointless.

Expected boundary:
- A `pull_request_target` job that does not need PR source should not check out at all; if it ever must, it
  must never build/run PR head.

Failure mechanism / Break-it angle:
- The interpolation `PR_AUTHOR="${{ github.event.pull_request.user.login }}"` (`:21`) is low injection risk
  (GitHub login charset is constrained), but the standing checkout is a trap: a future edit that adds a build
  or `npm install` step in this privileged context becomes an immediate `pull_request_target` RCE.

Impact:
- None today; latent high-severity footgun. Blast radius: Local now, Repo/Cross-system if a build is added.

Recommended mitigation:
- Remove the checkout step (unused), and pass `github.event.pull_request.user.login` via `env:` rather than
  inline interpolation for hygiene. Behavior test: workflow succeeds without the checkout.

Implementation assessment:
- Complexity: local_guardrail. Cost: XS. Nominal agent: codex.

---

## Non-findings (checked and held)

- **No live secrets committed.** The only private-key/token pattern hits are dummy test fixtures and a doc
  example, not credentials:
  - `crates/gosling-providers/src/api_client.rs:480-544` — `PKCS8_RSA_KEY`/`PKCS1_RSA_KEY`/`SEC1_EC_KEY`
    test constants in a `#[cfg(test)]` module.
  - `crates/gosling/src/providers/gcpauth.rs:628-629` — `// This is a generated test credential`, mock
    service account.
  - `documentation/.../prompts/multi-project-security-audit.json:8` — `AKIA1234567890ABCDEF` doc example.
  - `ui/desktop/.env` — non-secret config only (`VITE_START_EMBEDDED_SERVER`, provider host/model).
  - `services/ask-ai-bot/.env.example` — placeholders (`DISCORD_TOKEN=unset`, `ANTHROPIC_API_KEY=sk-1234`).
  Note: repository-history and platform secret-scanning were not run (Validation Limits).
- **ARM/CLI comment-build gate holds.** `pr-comment-bundle.yml` / `pr-comment-build-cli.yml` verify commenter
  write access via `getCollaboratorPermissionLevel` **before** any checkout, pin all actions to SHAs, and
  pass `github.event.*` via `env:` (not inline in `run:`) — see `pr-comment-bundle.yml:40-83, 86-100`.
- **Publish/release path is not reachable from untrusted input.** `publish-npm.yml:3-5` `workflow_dispatch`
  only (npm trusted publishing via OIDC); `publish-docker.yml:3-7` and `release.yml:2-8` on `push` tags/branches;
  `canary.yml` on push; `rebuild-skills-marketplace.yml` on schedule/dispatch. No `pull_request`/`issue_comment`
  → publish edge found.
- **`dependabot-auto-merge` trigger gate is sound** — gated on non-spoofable `dependabot[bot]` login and repo
  identity (`:13`); no checkout/build of PR code. (Blast-radius concern captured in TRIAGE-GSL-003.)
- **Apple codesign composite action handles secrets safely** — `.github/actions/apple-codesign/action.yml:23-53`
  passes cert/password via `env:`, uses a random keychain password, and deletes the decoded `.p12`.
- **Script-injection sinks are constrained** — repo-wide grep for `github.event.*` in `run:`/script blocks
  yielded only `create-release-branch.yaml:34` (`base.ref`) and `quarantine.yml:21` (`user.login`), both
  constrained charsets in trusted or bounded contexts; no free-text `comment.body`/`issue.title` reaches a
  shell.
- **Supply-chain inventory present** — `dependabot.yml` monitors npm, cargo, and github-actions; Scorecard
  (`scorecard.yml`, `read-all`) and CodeQL SARIF upload are wired.

## Cross-lens escalations

- **audit-security-repo-posture / audit-pipeline-externalapi:** confirm `github/command@v2.0.3` denial
  semantics (fails vs. `continue=false`) to resolve TRIAGE-GSL-001 exploitability, and audit the AWS/Azure
  OIDC trust policy / role scope reachable from `bundle-desktop-windows.yml` and `bundle-desktop-intel.yml`.
- **audit-compliance-posture:** the `MAINTAINERS.md`-referenced single opaque CODEOWNER (`@e3742526`) plus
  dependabot auto-merge is a review-governance gap worth a posture note.
- **audit-dependency-criticality:** `Jimver/cuda-toolkit` and `peter-evans/create-or-update-comment` are
  privileged-path single points of supply-chain failure (TRIAGE-GSL-002).

## Validation Limits (what was NOT reviewed)

- Git history and platform secret-scanning **not run** (`max_depth: fast`); only the working tree was swept.
  Secret exposure in history is therefore `not_observable`.
- Actual repo/org **default `GITHUB_TOKEN` scope** and **branch protection / rulesets** are platform settings —
  `not_observable` from files (bears on TRIAGE-GSL-004).
- `github/command@v2.0.3` internal permission/exit semantics were **not** read from source — the pivot for
  TRIAGE-GSL-001's confidence; reasoned from documented IssueOps behavior, hence capped at Likely/Plausible.
- Trusted-context release/publish workflows (`release.yml`, `publish-docker.yml`, `canary.yml`,
  `minor-/patch-release.yaml`, `deploy-docs-and-extensions.yml`) were trigger-and-permission sampled, not
  line-by-line audited for injection within their own steps.
- `services/ask-ai-bot/` Dockerfile/runtime and `scripts/pr-review-mcp.py` were not deep-read (no CI reference
  to the script found in `.github/`).
