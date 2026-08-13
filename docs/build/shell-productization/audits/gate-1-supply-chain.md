# Gate 1 audit — V8 CI supply chain and reproducibility

Date: 2026-08-12
Scope: `.github/workflows/ci.yml`, `scripts/with-rusty-v8-cache.sh`, `scripts/test-with-rusty-v8-cache.sh`
Lens: focused repository/CI security triage plus adversarial reproducibility review
Authority: read-only audit followed by separately authorized in-scope patching

## Surface and boundary inventory

- Inputs: vendored wrapper version, Rust host/requested target, debug flag, feature suffix, cache path, optional operator seed archive, immutable upstream release asset.
- External boundary: HTTPS download from `github.com/denoland/rusty_v8/releases`.
- Integrity boundary: source-controlled compressed SHA-256 table, gzip validation, static archive validation, cache digest sidecar.
- CI boundary: untrusted pull-request code executes with top-level `contents: read`; no secrets or write permissions are provided by this job.
- Consumer boundary: one `RUSTY_V8_ARCHIVE` path is inherited by three existing Cargo commands.

## Finding

### SHP-G1-AUD-001 — Cargo cache restoration could bypass trusted upstream checksum

- Severity: high
- Confidence: confirmed
- Evidence basis: source-evidenced
- Initial evidence: the first draft restored `Swatinem/rust-cache` before invoking helper preparation, while `scripts/with-rusty-v8-cache.sh` accepted a valid host archive under `target/{debug,release}/gn_out/obj` and generated a new sidecar for it.
- Mechanism: a restored Cargo cache could supply the native archive before the helper's checksum-verified download path. Archive syntax and a self-generated digest detect corruption after acceptance, but do not establish upstream provenance.
- Impact: the CI fix could appear checksum-bound while still trusting a cache-restored native object, preserving the original nondeterminism and weakening supply-chain integrity.
- Disposition: fixed.
- Patch: reordered the job to run the helper regression suite and verified prepare step before `rust-cache`; the fresh `${RUNNER_TEMP}` path is then exported through `GITHUB_ENV`. CI has no host `target/**` archive available at prepare time.
- Regression evidence: workflow-order assertion proves `helper test → verified prepare → rust-cache → unchanged Cargo tests`; real Linux x86_64 asset download matched the pinned compressed hash and yielded a valid archive.

## Checked seams that held

- Workflow permissions remain `contents: read`; the changed job receives no write token or release secret.
- Triggers remain ordinary `push`, `pull_request`, `merge_group`, and manual dispatch; there is no `pull_request_target` or privileged cross-workflow execution.
- Every third-party action in the changed workflow remains pinned to a full commit SHA.
- No GitHub event text or workflow input is interpolated into shell commands.
- V8 URL components derive from the source-controlled wrapper version, a closed target checksum table, and internally selected profile, not PR metadata.
- `curl` uses HTTPS, `--fail`, bounded connect timeout, retries, and verifies downloaded bytes before decompression.
- Unsupported targets fail before network execution because no checksum is recorded.
- CI uses a fresh runner-temp cache and does not add a persistent Actions cache for the archive.
- The regression harness covers valid seed, warm hit, sidecar loss, corruption, invalid seed, cache-under-target, unsupported target, network failure, checksum mismatch, concurrency, debug selection, and environment propagation.
- Changed-file secret-pattern scan found no credential/private-key pattern.

## Validation limits

- Branch protection and repository environment settings are platform-side and were not needed for this read-only PR job.
- No push was authorized, so the changed workflow itself has not run on GitHub.
- A Docker client exists locally but no daemon/runtime is available; fresh Linux container execution could not be run.
- The real Linux x86_64 upstream asset was downloaded and verified from macOS, but that is not equivalent to Linux Cargo linkage.
- Existing unrelated workflow/release surfaces were outside this focused Gate 1 audit.

## Verdict

**SURVIVED after patch, locally.** No open finding remains in the Gate 1 diff. Remote Linux CI execution is still required before SHP-REQ-014 can be marked verified.
