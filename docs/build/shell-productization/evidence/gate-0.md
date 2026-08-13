# Gate 0 evidence — orientation and live-state correction

Date: 2026-08-12
Branch: `codex/shell-productization`
Baseline: `8627dc31a8dd6155c7cbbf00c450a549e1860f68`
Decision: **GO** to Gate 1 after the planning/Gate 0 checkpoint

## Goal and scope

Re-verify the planning baseline, current repository instructions, toolchains, architecture, validation commands, V8 mechanism, and current CI failure before making runtime changes. No domain-shell or Electron feature implementation belongs in this gate.

## Repository and governance readback

Observed commands:

```bash
git status --short --branch --untracked-files=all
git log -1 --oneline --decorate
cat AGENTS.md
cat CLAUDE.md
sed -n '1,220p' README.md
sed -n '1,220p' docs/INTENT.md
cat .giles/repo.yaml
git worktree list
```

Observed results:

- The repository was still at merge commit `8627dc31a`; only the prior planning package was modified/untracked.
- `AGENTS.md` requires `source bin/activate-hermit`, inspection before edits, `cargo fmt`, relevant tests, and Clippy before merge.
- The Giles profile applies existing repository conventions; this campaign must not replace the repository's authoritative workspace intent artifacts.
- The implementation branch `codex/shell-productization` did not exist and was created without overwriting the planning changes.
- Other Gosling worktrees exist but do not own this branch. Same-file parallel edits are prohibited by the plan.

## Toolchain correction

Direct shell commands resolved to Homebrew Rust 1.97.1. After:

```bash
source bin/activate-hermit
rustc --version
cargo --version
rustc -vV
```

observed Rust/Cargo 1.92.0, matching `rust-toolchain.toml`. All authoritative validation for this campaign must run in the Hermit environment.

## V8 helper readback and local cold/warm probe

Inspected:

- `scripts/with-rusty-v8-cache.sh`;
- `vendor/v8/Cargo.toml`;
- the fetched `v8-goose-145.0.2/build.rs`;
- `.github/workflows/ci.yml`;
- `Cargo.lock`.

Observed contract:

- the wrapper package version is `v8` 145.0.0 and the implementation dependency is `v8-goose` 145.0.2;
- `v8-goose` honors `RUSTY_V8_ARCHIVE` and emits native link flags;
- the helper selects/checks assets by wrapper version, target, profile, and trusted compressed SHA-256;
- the CI Rust test job invokes Cargo directly and does not guarantee a prepared archive independent of restored Cargo artifacts.

The following isolated-cache probe was run under Hermit:

```bash
cache_dir="$(mktemp -d /tmp/gosling-v8-gate0.XXXXXX)"
GOSLING_V8_CACHE_DIR="$cache_dir" scripts/with-rusty-v8-cache.sh --prepare
GOSLING_V8_CACHE_DIR="$cache_dir" scripts/with-rusty-v8-cache.sh --prepare
find "$cache_dir" -name '*.a' -type f -exec shasum -a 256 {} \;
rm -rf "$cache_dir"
```

Both cold and warm preparation completed. On this host the helper seeded its isolated cache from the already-built 126 MiB local release archive, then reused it. This proves local cache/seed mechanics, not the Linux download path.

## CI evidence and corrected diagnosis

Historical failed runs directly confirm the V8 defect:

- run `31642034573`, SHA `006e604b4`: `v8-goose v145.0.2` failed with `could not find native static library rusty_v8`;
- run `31659795858`, SHA `772f19889`: the same missing native library failure.

Current merged-main run `31660173759`, SHA `8627dc31a`, did **not** fail at V8. It compiled and ran Rust tests, then failed `scenario_tests::scenarios::tests::test_weather_tool` because the Anthropic replay hash `112127de…` was absent. This supersedes the stale planning statement that current main always fails before tests.

The V8 defect is therefore a nondeterministic/restored-cache integration gap: CI can reach tests when the cache happens to contain the native object, but no explicit, checksum-bound preparation step guarantees it. Gate 1 remains required. The unrelated replay failure is recorded separately and is not silently included in the V8 patch.

## Gate 0 validation inventory

Authoritative commands identified:

- Rust: `cargo fmt --check`, focused `cargo test -p <crate>`, and `cargo clippy --all-targets -- -D warnings` through Hermit;
- SDK: `just check-acp-schema`, `ui/sdk` tests/typecheck when canonical DTOs change;
- Desktop: `pnpm run lint:check`, `pnpm run typecheck`, `pnpm run test:run` from `ui/desktop` through Hermit;
- package/release: existing Forge and platform workflow commands, to be narrowed by later gates;
- docs: `git diff --check`, relative-link/fence/ID checks already captured in planning evidence.

No `docs/testing/test-ledger.yaml` exists at this baseline, so Gate 1 shell tests do not require a ledger update under a repository-specific rule.

## Gate decision

**GO** to Gate 1. Gate 0 has current evidence, a corrected diagnosis, an isolated branch, an exact command inventory, and no invented implementation path.

Residual blockers:

- a fresh Linux cold-download/helper run requires CI or a disposable Linux environment;
- current main has an unrelated deterministic replay-data failure that may keep the whole Rust job red after V8 succeeds;
- production release topology, signing credentials, and updater predecessor evidence remain later human gates.
