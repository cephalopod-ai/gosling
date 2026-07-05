# Audit Lens — Security Repo Posture (RSP)

Lens: `audit-security-repo-posture` (Variant B, tri-perspective). Authority:
**audit-only / read-only**. Builds on `docs/cloud/00-orientation.md`. Finding IDs
`POSTURE-GSL-NNN`. Evidence discipline per `evidence_discipline.md`: every
`Confirmed` quotes a `file:line` actually read; severity is independent of
confidence.

Effort budget: ~25–40 tool calls (used ~16). Scope: repository-level shipped-product
posture — SECURITY.md, disclosure path, dependency policy (`deny.toml`,
cargo-deny, `Cargo.lock`), supply-chain / provenance, secret-scanning,
release signing, `.github/` workflow trust chain. Code-level enforcement of the
agent's runtime safety controls (permission gating, prompt-injection inspectors)
is **out of scope for this lens** and escalated to the security-code / security-llm
lenses.

---

## Posture verdict

**Partial → leaning credible.** gosling inherits a genuinely above-average
open-source supply-chain baseline from its `goose` upstream: SLSA build-provenance
attestations, Sigstore/OIDC code signing, npm trusted publishing with
`--provenance`, OpenSSF Scorecard, Dependabot across three ecosystems, a committed
`Cargo.lock`, a real cargo-deny advisory gate, mostly SHA-pinned actions, and a
working private vulnerability-disclosure path. These are real, enforced controls,
not just prose.

The gaps are consistent and matter for a **code-executing agent shipped as a
`curl | bash` binary**: (1) the provenance the release pipeline produces is never
verified at install time; (2) there is no secret scanning / push protection; (3)
the dependency gate is advisory-only (no license, source-registry, or ban policy)
while minor+patch dependency bumps auto-merge unattended; (4) action pinning is
mixed. None are Critical in isolation; together they mean the *strongest* link
(provenance generation) is not connected to the *user-facing* link (install), and
the automated dependency intake has no human in the loop.

---

## Category inventory (8-category RSP matrix)

| Category | Status | Basis |
|---|---|---|
| RSP-SCR Secrets | **partial** | No live creds committed (held); no secret scanning (fail, GSL-001); committed `ui/desktop/.env` outside ignore (GSL-006); no rotation procedure |
| RSP-DEP Dependencies | **partial** | `Cargo.lock` + Dependabot present (held); cargo-deny advisory-only, no bans/licenses/sources (GSL-003) |
| RSP-WFL Workflows | **pass (minor)** | Least-privilege `permissions:` present; `pull_request_target` jobs do not checkout+exec untrusted code (held); unattended auto-merge (GSL-004) |
| RSP-AUT Automation | **partial** | Majority SHA-pinned (held); several mutable-tag third-party actions (GSL-005) |
| RSP-BRN Branches | **not_observable / partial** | `CODEOWNERS` present but single maintainer (GSL-007); branch protection is platform-only, not verified here |
| RSP-RUN Runners | **pass / n_a** | All jobs on GitHub-hosted `ubuntu-latest`/`macos-latest`/`windows-latest`; no self-hosted runner exposed to PR code |
| RSP-ART Artifacts | **partial** | Strong provenance/signing at build (held) but not verified at install (GSL-002) |
| RSP-ALT Alerts | **partial** | `SECURITY.md` disclosure path (held); Scorecard SARIF only, no SAST/CodeQL (GSL-007) |

---

## Findings

### POSTURE-GSL-001: No secret scanning or push protection in CI/repo config

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `.github/workflows/` — full listing inspected; no `gitleaks`, `trufflehog`,
  `detect-secrets`, or GitHub secret-scanning step in any of the 39 workflows.
  Grep for `gitleaks|trufflehog|secret.?scan|detect-secrets` matched only
  `publish-docker.yml`, `release.yml`, `canary.yml` — all on `secrets.*`
  interpolation, none a scanner.
- `.github/workflows/ci.yml:1-237` — CI runs fmt, build/test, clippy, schema,
  desktop lint; no secret-scan job.
- `ui/desktop/.env` is tracked (see GSL-006), showing `.env` files can enter the
  tree — the exact surface push-protection defends.

Observed behavior:
- Nothing in-repo detects a committed API key, provider token, or private key
  before or after it lands on `main`. Detection depends entirely on org-level
  GitHub secret-scanning that is not observable from source.

Expected boundary:
- A code-executing agent that integrates 15+ providers with API keys and OAuth
  tokens should gate secret introduction (pre-commit hook, CI scan, or documented
  reliance on GitHub push protection).

Failure mechanism:
- No scanner is wired; the control is absent, not merely unenforced.

Break-it angle:
- A contributor pastes a live provider key into a test fixture or `.env`; it
  merges through the advisory-only checks and is published in the git history and
  every downstream clone/tarball.

Impact:
- Credential exposure with irreversible git-history persistence.

Operational impact:
- Blast radius: Repo / Cross-system (leaked provider creds)
- Side-effect class: external API / user-visible
- Reversibility: irreversible (git history + published artifacts)
- Operator visibility: silent
- Rerun safety: n/a

Adjacent failure modes:
- GSL-006 (committed `.env` outside ignore).

Recommended mitigation:
- Remediation pattern: pipeline-gate + documented reliance.
- Minimal repair: add a `gitleaks`/`trufflehog` job to `ci.yml` on
  `pull_request` + `push`, or document + require org push-protection in SECURITY.md.
- Behavior test: seed a dummy `AKIA...`-shaped string in a throwaway branch and
  assert the scan job fails.

Implementation assessment:
- Complexity: workflow_protocol; Cost: S; Cost drivers: modules, tests
- Nominal agent: codex

Validation:
- CI job fails on a planted fake secret; passes on clean tree.

Non-goals:
- Rotating any existing secret (out of authority).

---

### POSTURE-GSL-002: Build provenance produced but never verified at install (`curl | bash`)

Severity: Medium
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `.github/workflows/release.yml:125-137` — `actions/attest-build-provenance`
  attests `gosling-*.tar.*`, `*.deb`, `*.rpm`, `*.flatpak`, `download_cli.sh`.
- `.github/workflows/publish-docker.yml:67-72` — image attestation pushed to
  registry. `publish-npm.yml:182` — `pnpm publish ... --provenance`.
- `download_cli.sh:14` documents install as
  `curl -fsSL https://github.com/repo-makeover/gosling/releases/download/stable/download_cli.sh | bash`.
- `download_cli.sh:216-274` — downloads the tarball via `curl -sLf` and untars it
  directly. Grep for `attestation|cosign|gh attestation|verify|checksum|sha256|slsa`
  over `download_cli.sh` returned **no matches**. No signature/checksum/attestation
  check occurs before extraction and install.

Observed behavior:
- The strongest supply-chain control (SLSA provenance) is generated at release but
  the end-user install path consumes artifacts with zero verification.

Expected boundary:
- If provenance/signing is the claimed supply-chain posture, the install path
  should verify it (`gh attestation verify`, cosign, or at minimum a published
  checksum comparison) before executing the downloaded binary.

Failure mechanism:
- Provenance is a producer-side artifact; the consumer-side link is missing, so
  the guarantee is aspirational for anyone using the documented installer.

Break-it angle:
- A registry/release-asset compromise, an MITM on a misconfigured proxy, or a
  malicious mirror serves a tampered tarball; `download_cli.sh` installs and runs
  it with no attestation check. The agent then has shell/file/network authority on
  the user's machine (per `SECURITY.md`).

Impact:
- Arbitrary code execution on the operator's workstation via a tampered install,
  despite provenance existing that would have caught it.

Operational impact:
- Blast radius: Cross-system (user workstation)
- Side-effect class: process / network
- Reversibility: irreversible
- Operator visibility: silent
- Rerun safety: unsafe

Adjacent failure modes:
- GSL-005 (mutable action tags in the pipeline that produces these artifacts).

Recommended mitigation:
- Remediation pattern: provenance-verification-at-consume.
- Minimal repair: publish per-artifact SHA-256 sums and have `download_cli.sh`
  verify them; ideally add `gh attestation verify "$FILE" --repo repo-makeover/gosling`.
- Behavior test: flip one byte of a downloaded artifact in a test harness and
  assert the installer aborts.

Implementation assessment:
- Complexity: workflow_protocol; Cost: M; Cost drivers: modules, tests, docs, runtime_verification
- Nominal agent: codex

Validation:
- Installer refuses a checksum/attestation mismatch; accepts a genuine release.

Non-goals:
- Changing the signing pipeline itself (already present).

---

### POSTURE-GSL-003: Dependency gate is advisory-only — no license, source-registry, or ban policy

Severity: Low (Medium for a shipped, code-executing product)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- `deny.toml:1-13` — only an `[advisories]` section. `unmaintained = "none"` and
  `unsound = "none"` explicitly disable those checks; there is no `[bans]`,
  `[licenses]`, or `[sources]` section.
- `.github/workflows/cargo-deny.yml:29` — `command: check advisories` only. Bans
  (dependency-confusion / duplicate / disallowed crates), license compliance, and
  source-registry allowlisting are never run.
- `deny.toml:11-13` — `RUSTSEC-2023-0071` (rsa Marvin timing side-channel via
  jsonwebtoken) is ignored with a documented rationale (retained risk, acceptable
  but tracked).

Observed behavior:
- The only enforced dependency control is "known-vulnerable + yanked." No policy
  prevents a dependency from an unexpected registry (dependency confusion), a
  GPL/incompatible license slipping in, or an unmaintained crate.

Expected boundary:
- A product shipping signed binaries and 15+ provider integrations should pin
  crate sources to crates.io (`[sources]`) and enforce a license allowlist, so the
  supply chain is governed, not just scanned for existing CVEs.

Failure mechanism:
- Scope of the gate was narrowed to emulate cargo-audit; the broader governance
  surfaces are simply not evaluated.

Break-it angle:
- A transitive dependency is repointed (via a compromised or typosquatted
  registry entry) to a source not on crates.io; nothing in the gate flags it.

Impact:
- Silent introduction of a hostile or non-compliant dependency; blast radius is
  the whole binary since deps run in-process.

Operational impact:
- Blast radius: Repo / Cross-system
- Side-effect class: process
- Reversibility: compensatable
- Operator visibility: silent
- Rerun safety: n/a

Adjacent failure modes:
- GSL-004 (unattended merge of dependency bumps).

Recommended mitigation:
- Remediation pattern: policy-gate-expansion.
- Minimal repair: add `[sources]` (allow crates.io only) and `[licenses]`
  (allowlist) to `deny.toml`; change `cargo-deny.yml` to `check advisories bans licenses sources`.
- Behavior test: add a fixture dep with a disallowed license / git source and
  assert cargo-deny fails.

Implementation assessment:
- Complexity: governance_decision; Cost: S; Cost drivers: modules, governance_decision
- Nominal agent: human-owner (license allowlist is a policy decision)

Validation:
- cargo-deny fails on a planted non-allowlisted source/license; passes clean.

Non-goals:
- Removing the documented `RUSTSEC-2023-0071` ignore (justified retained risk).

---

### POSTURE-GSL-004: Dependabot minor + patch bumps auto-approved and auto-merged unattended

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `.github/workflows/dependabot-auto-merge.yml:3-4` — trigger is
  `pull_request_target`; `:6-8` grants `contents: write` + `pull-requests: write`.
- `:21-34` — for `version-update:semver-patch` **and** `version-update:semver-minor`
  the workflow runs `gh pr review --approve` then `gh pr merge --auto --merge`.
- Gated on `github.event.pull_request.user.login == 'dependabot[bot]'` (`:13`).

Observed behavior:
- Minor (not just patch) dependency updates across cargo/npm/actions are approved
  and merged with no human review, subject only to whatever required status checks
  branch protection enforces (not observable here).

Expected boundary:
- For an agent that executes code, minor version bumps (which can carry behavior
  changes and, in a compromised-maintainer scenario, malicious code) warrant a
  human gate or at least a hard dependency on full CI + provenance before merge.

Failure mechanism:
- Auto-merge scope includes `semver-minor`; the safety of the merge rests entirely
  on unverified branch-protection required-checks.

Break-it angle:
- A compromised upstream package publishes a malicious minor release; Dependabot
  opens the PR, this workflow approves and auto-merges it. If branch protection
  does not require the full test/deny suite as *required* checks, it lands on
  `main` and flows into the next canary/release build.

Impact:
- Unattended ingestion of a potentially malicious dependency into a signed release.

Operational impact:
- Blast radius: Repo / Cross-system
- Side-effect class: process
- Reversibility: compensatable (revert) but may ship in canary first
- Operator visibility: log-only
- Rerun safety: n/a

Adjacent failure modes:
- GSL-003 (no source/ban gate to catch a hostile dep), GSL-002 (auto-merged code
  is then signed and shipped unverified downstream).

Recommended mitigation:
- Remediation pattern: narrow-automation-scope + required-checks.
- Minimal repair: restrict auto-merge to `semver-patch`; require the full CI +
  cargo-deny suite as branch-protection required checks before `--auto` can land.
- Behavior test: open a minor-bump PR and assert it does NOT auto-merge.

Implementation assessment:
- Complexity: governance_decision; Cost: S; Cost drivers: governance_decision
- Nominal agent: human-owner

Validation:
- Minor bump requires manual approval; patch bump auto-merges only after required
  checks pass.

Non-goals:
- Disabling Dependabot itself.

---

### POSTURE-GSL-005: Mixed action pinning — third-party actions referenced by mutable tag

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- Majority of `uses:` are SHA-pinned (e.g. `actions/checkout@9c091bb...` in
  `ci.yml:29`, `EmbarkStudios/cargo-deny-action@bb137d7...` in `cargo-deny.yml:27`).
- But mutable-tag refs remain, including third-party (non-GitHub) actions:
  - `.github/workflows/cargo-deny.yml:25` — `actions/checkout@v7`
  - `.github/workflows/scorecard.yml:76` — `github/codeql-action/upload-sarif@v4`
  - `.github/workflows/pr-comment-build-cli.yml:132` — `peter-evans/create-or-update-comment@v5`
  - `.github/workflows/bundle-desktop-windows.yml:87` — `Jimver/cuda-toolkit@v0.2.35`
  - `.github/workflows/pr-smoke-test.yml:99,133,219` — `actions/setup-node@v6`, `actions/setup-python@v6`
  - `.github/workflows/autoclose:13` — `actions/stale@v9`

Observed behavior:
- The repo's stated posture (and Scorecard's Pinned-Dependencies check) is
  SHA-pinning, but several actions — including two non-GitHub-owned ones
  (`peter-evans/*`, `Jimver/*`) — resolve to mutable tags.

Expected boundary:
- All external actions pinned to full-length commit SHAs so a tag repoint cannot
  silently change CI/release behavior.

Failure mechanism:
- Inconsistent pinning; the mutable tags are the weakest link, especially in
  release-adjacent (`bundle-desktop-windows`) and comment-writing workflows.

Break-it angle:
- A compromised `peter-evans/create-or-update-comment` or `Jimver/cuda-toolkit`
  tag repoints to malicious code that runs in a workflow holding a write token.

Impact:
- Supply-chain execution in CI with whatever token scope the job carries.

Operational impact:
- Blast radius: Repo
- Side-effect class: process
- Reversibility: compensatable
- Operator visibility: log-only
- Rerun safety: n/a

Recommended mitigation:
- Remediation pattern: uniform-SHA-pinning (Dependabot `github-actions` already
  configured — `dependabot.yml:29-33` — will keep SHAs fresh).
- Minimal repair: pin the listed refs to commit SHAs.
- Behavior test: a lint (e.g. `zizmor` / ratchet) that fails on tag refs.

Implementation assessment:
- Complexity: workflow_protocol; Cost: S; Cost drivers: modules
- Nominal agent: codex

Validation:
- Pin-lint passes; no `@vN` refs remain for third-party actions.

Non-goals:
- Re-pinning reusable local `./.github/workflows/*` calls (internal, not mutable).

---

### POSTURE-GSL-006: Committed `ui/desktop/.env` outside `.gitignore` scope; no credential-rotation procedure

Severity: Low
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Security

Evidence:
- `git ls-files` shows `ui/desktop/.env` is **tracked**; `git check-ignore` reports
  it is NOT ignored.
- `.gitignore:65` ignores `/.env` (repo root only) — the subdirectory `.env` is not
  covered.
- Content of `ui/desktop/.env` is non-secret config only
  (`VITE_START_EMBEDDED_SERVER=yes`, `GOSLING_PROVIDER__TYPE=openai`,
  `...__HOST`, `...__MODEL=gpt-4o`) — **no live credential present** (RSP-SCR-001
  holds).
- `SECURITY.md:1-16` documents a disclosure path but no credential-rotation /
  exposed-secret incident procedure (RSP-SCR-005 gap).

Observed behavior:
- A committed `.env` sets a precedent and the ignore rule does not cover
  subdirectory `.env` files, so a developer adding `OPENAI_API_KEY=` to
  `ui/desktop/.env` would commit it by default (compounded by GSL-001: no scan).

Expected boundary:
- `.env` files ignored repo-wide (`**/.env` or `.env`), plus a documented rotation
  step for the inevitable exposed-key event.

Failure mechanism:
- Root-anchored ignore pattern + an already-tracked `.env` in a subdir.

Break-it angle:
- Contributor appends a real provider key to the existing tracked file; git stages
  it silently; no scan catches it (GSL-001).

Impact:
- Latent credential-leak vector; today the file is clean.

Operational impact:
- Blast radius: Repo; Side-effect class: file; Reversibility: irreversible (history);
  Operator visibility: silent; Rerun safety: n/a

Recommended mitigation:
- Remediation pattern: ignore-hardening + runbook.
- Minimal repair: change `.gitignore` to `.env` / `**/.env`; convert the tracked
  file to `ui/desktop/.env.example`; add a 3-line rotation procedure to SECURITY.md.
- Behavior test: `git check-ignore ui/desktop/.env` returns a match after the fix.

Implementation assessment:
- Complexity: local_guardrail; Cost: XS; Cost drivers: docs
- Nominal agent: codex

Validation:
- New `.env` anywhere in the tree is ignored; example file remains tracked.

Non-goals:
- Removing the file from history (no live secret; not warranted).

---

### POSTURE-GSL-007: No SAST/code-scanning; single-maintainer CODEOWNERS; honest-but-user-delegated agent threat model

Severity: Info (Low for governance)
Confidence: Confirmed
Evidence basis: source-evidenced
Domain: Compliance-Posture

Evidence:
- No CodeQL/Semgrep/SAST workflow exists (`.github/workflows` listing; grep for
  `codeql|sast|semgrep` matched none as a scanning job). Scorecard
  (`scorecard.yml`) uploads its own SARIF but is a posture scanner, not code SAST.
- `.github/CODEOWNERS:2` — `* @e3742526`; `MAINTAINERS.md:1-3` lists a single core
  maintainer. All review authority + bus factor rests on one account.
- `SECURITY.md:1-13` — threat model is explicit and honest: it states the agent
  "can run code and take actions on your computer," acknowledges prompt injection
  ("gosling may follow commands found embedded in content even if those commands
  conflict with the task"), and **delegates** mitigation to the user (dedicated
  VM/container, review generated code, require human confirmation for significant
  actions, review MCP extensions).
- Disclosure path is real: GitHub private vulnerability reporting
  (`SECURITY.md:13-15`).

Observed behavior:
- The stated posture matches reality (advisory, user-shouldered) — this is a
  non-finding on honesty. But "require human confirmation for significant actions"
  is asserted as user guidance while the *code enforcement* of that confirmation
  lives in `permission/` and `agents/tool_confirmation_router.rs` — not verified by
  this lens (see Skill Escalation).

Expected boundary:
- For a security-sensitive agent: a SAST gate on the Rust/TS code, and review
  authority not concentrated in one account.

Failure mechanism:
- SAST absent; ownership concentrated.

Impact:
- Vulnerable code changes are caught only by clippy + human review by a single
  owner; a compromised/unavailable maintainer account is a governance SPOF.

Operational impact:
- Blast radius: Repo; Side-effect class: none; Reversibility: n/a;
  Operator visibility: n/a; Rerun safety: n/a

Recommended mitigation:
- Remediation pattern: add-SAST + broaden-ownership.
- Minimal repair: add a CodeQL workflow for `rust` + `javascript-typescript`; add a
  second maintainer / CODEOWNERS reviewer for `crates/gosling/src/security/`,
  `permission/`, and `.github/`.
- Behavior test: CodeQL runs on PR and surfaces to the code-scanning dashboard.

Implementation assessment:
- Complexity: governance_decision; Cost: M; Cost drivers: modules, governance_decision
- Nominal agent: human-owner

Validation:
- SAST job present and required; ≥2 owners on security-sensitive paths.

Non-goals:
- Rewriting SECURITY.md's (accurate) threat model.

---

## Non-findings (checked and held)

- **RSP-SCR-001 no live credentials committed** — `ui/desktop/.env` is the only
  tracked `.env` and contains config only, no key (content quoted in GSL-006). The
  private-key regex hits in `crates/gosling-providers/src/api_client.rs` and
  `crates/gosling/src/providers/gcpauth.rs` are GCP service-account key *handling
  code*, not committed keys (not deep-read; see Validation Limits).
- **RSP-SCR-003 CI secrets via platform store** — workflows use
  `secrets.GITHUB_TOKEN` and OIDC (`id-token: write`), no hardcoded tokens
  (`release.yml:13-19`, `publish-npm.yml:12`).
- **RSP-WFL-001 least privilege** — `ci.yml:15-16` and `scorecard.yml:18`
  (`permissions: read-all`) set restrictive top-level tokens; write scopes are
  job-scoped in release/publish workflows.
- **RSP-WFL-002 `pull_request_target` isolation** — the two `pull_request_target`
  workflows (`quarantine.yml`, `dependabot-auto-merge.yml`) do **not** checkout and
  execute PR-authored code; they operate on metadata/`gh` CLI only. Quarantine
  interpolates `github.event.pull_request.user.login` into bash but GitHub logins
  are a constrained charset — held (low residual).
- **RSP-DEP-002 lockfile** — `Cargo.lock` (285 KB) committed; CI uses
  `cargo check --workspace --locked` (`ci.yml:142`) and `pnpm install --frozen-lockfile`.
- **RSP-DEP-003 update automation** — Dependabot across npm/cargo/github-actions
  weekly (`dependabot.yml`).
- **RSP-ART-001/002/003 provenance & signing** — SLSA attestation + Sigstore OIDC
  signing for CLI, desktop, Docker, and npm (`release.yml:125-137`,
  `publish-docker.yml:67-72`, `publish-npm.yml:182`). Strong — the gap is
  verification at *install* (GSL-002), not production.
- **RSP-ALT-001 disclosure path** — `SECURITY.md` points to GitHub private
  vulnerability reporting; present and actionable.
- **Cargo-deny/Scorecard org gating** — `if: github.repository == 'repo-makeover/gosling'`
  correctly scopes these to the canonical repo; not a defect.

---

## Skill Escalation

| Finding | Primary Lens | Secondary Lens | Why |
|---|---|---|---|
| GSL-002 install verification | Security (RSP) | Input/Output-Path | Installer parses/extracts an untrusted tarball with no integrity check |
| GSL-004 auto-merge minor | Security (RSP) | Cascade | Auto-merged dep flows into signed canary/release build |
| GSL-003 dep gate scope | Compliance-Posture | Security | Dependency-confusion / license risk enters an in-process supply chain |
| GSL-007 "require human confirmation" claim | Compliance-Posture | Security-Code / Security-LLM | Enforcement of the confirmation claim lives in `permission/` + `tool_confirmation_router.rs` — verify the code actually gates destructive tool calls |
| GSL-001 no secret scanning | Security (RSP) | Temporal | Leaked secret persists irreversibly in git history |

---

## Validation Limits (not reviewed)

- **Branch protection / rulesets (RSP-BRN-001/002/004)** — platform-only; not
  observable from source. Not verified whether required status checks gate
  `main`/`release/*`, whether force-push/deletion are blocked, or whether the
  GSL-004 auto-merge is backstopped by required checks. `scorecard.yml` implies
  the Branch-Protection check runs, but its result is not in-repo. **next action:**
  query GitHub branch-protection API for `main` and `release/*`.
- **Org-level GitHub secret scanning / push protection** — not observable; GSL-001
  assumes it is not relied upon because nothing documents it.
- **Reusable workflow bodies** — `build-cli.yml`, `bundle-desktop*.yml`,
  `publish-npm.yml` beyond the provenance/publish lines, and the reusable Windows
  signing path were not deep-read; only the calling `release.yml`/`canary.yml`
  and the provenance/publish steps were inspected.
- **Private-key regex hits** in `api_client.rs` / `gcpauth.rs` were not opened;
  classified as key-handling code by filename/module, not confirmed line-by-line.
- **`oidc-proxy/`, `services/`, `vendor/`** subtrees not inspected for posture.
- **Runtime**: nothing executed (audit-only). Provenance verification behavior,
  branch-protection enforcement, and auto-merge outcomes are static-only; per
  `confidence_calibration.md` their *runtime manifestation* is Likely, not
  independently reproduced.
