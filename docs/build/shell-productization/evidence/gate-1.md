# Gate 1 evidence — verified V8 archive provisioning

Date: 2026-08-12
Branch: `codex/shell-productization`
Decision: **BLOCKED for remote verification; built and locally validated**

## Implemented slice

- Added `scripts/test-with-rusty-v8-cache.sh`, a network-independent black-box helper regression suite.
- Tightened cache-hit acceptance: the archive must be valid, have a sidecar, and match the sidecar digest both before and after lock acquisition.
- Added CI helper tests and an explicit prepare step before Cargo cache restoration.
- Exported one fresh `${RUNNER_TEMP}` archive path through `GITHUB_ENV` for all existing Rust test commands.
- Did not add a persistent V8 Actions cache or alter any Cargo test selection.

## Root cause

Historical Linux failures show that restored Cargo state can compile `v8-goose` metadata without a usable native static library. The CI job had no deterministic archive-provisioning step, so success depended on incidental cache contents. The existing helper is the correct boundary; Gate 1 integrates and tests it rather than adding a second downloader.

## Regression suite

Command:

```bash
source bin/activate-hermit
bash -n scripts/with-rusty-v8-cache.sh scripts/test-with-rusty-v8-cache.sh
scripts/test-with-rusty-v8-cache.sh
```

Observed:

```text
Testing V8 helper with disposable archives...
PASS: V8 helper cache, integrity, target, failure, lock, profile, and propagation behavior
```

The suite exercises:

- valid seed and digest sidecar;
- warm cache without rewrite;
- corrupt archive/digest repair;
- missing sidecar repair;
- invalid seed rejection;
- cache-under-Cargo-target rejection;
- unsupported target rejection before download;
- simulated network failure;
- simulated checksum mismatch;
- four concurrent preparers using one cache;
- `RUSTY_V8_ARCHIVE` environment propagation;
- debug profile archive naming.

## Real upstream Linux asset evidence

The exact x86_64 Linux asset was fetched from the helper's source-controlled URL. Its compressed SHA-256 was:

```text
7215753c0c78d141f752d7b993794bae07e18a1dfd466dcaa84fa64e76bacac1
```

This exactly matched the source-controlled trusted value. After decompression:

```text
size=169586332
sha256=62c711690504482da39e2d58e351fb22725d730151609c342757a35ad1ab7060
```

`gzip -t` and `ar -t` passed. Running the helper wrapper with a no-op probe for `--target x86_64-unknown-linux-gnu` propagated that path, and a second run reused the same archive.

## Workflow validation

- Ruby YAML parsing of `.github/workflows/ci.yml`: passed.
- Custom order assertion: `helper regression → fresh verified prepare → rust-cache → unchanged Cargo test commands`: passed.
- All actions in the workflow remain full-SHA pinned.
- Workflow permissions remain read-only.
- `git diff --check`: passed.

## Focused Rust evidence

Command under Hermit with a helper-prepared host archive:

```bash
export GOSLING_V8_CACHE_DIR="$(mktemp -d /tmp/gosling-v8-gate1-final.XXXXXX)"
export RUSTY_V8_ARCHIVE="$(scripts/with-rusty-v8-cache.sh --prepare)"
cargo test -p gosling --lib acp::shell -- --nocapture
```

Observed: 4 shell/validation tests passed, 0 failed, 1,578 filtered out. The first clean compile completed through the V8-dependent Gosling graph; a final rerun completed in under one second.

## Audit/patch cycle

The supply-chain audit found that preparing after `rust-cache` could seed from restored `target/**`, bypassing the pinned upstream compressed checksum. The prepare step was moved before cache restoration, then all helper, workflow, and Rust checks were rerun. See `../audits/gate-1-supply-chain.md`.

## Blocked evidence

- Docker CLI is installed, but no Docker daemon/runtime is present and Docker Desktop is not installed; a disposable Linux container could not run.
- No push/PR was authorized, so the new job has not run on a GitHub Linux runner.
- Two clean remote runs and actual job-level “reaches tests through prepare step” evidence remain required.
- Current merged-main run has an unrelated missing Anthropic weather replay that may keep the overall Rust job red after V8 succeeds.

## Status truth

Gate 1 code is **built and locally validated**, not remotely verified. SHP-REQ-014 moves to `built`; it must not become `verified` until fresh Linux CI evidence exists. This does not block Gate 2 contract work, which the plan explicitly permits to proceed independently, but it remains a Gate 8 acceptance blocker.
