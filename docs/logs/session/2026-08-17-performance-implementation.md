# 2026-08-17 — Performance recommendation implementation

## Task

Implement the remaining `PERF-GSL-001` through `PERF-GSL-004` recommendations.

## Changes

- `README.md`: historical command timings no longer claim to be cold-start or
  ready-to-prompt measurements.
- `ui/desktop/tests/e2e/performance.spec.ts`: replaced provider-dependent,
  single-run prompt timing with an opt-in renderer-readiness harness. It uses a
  fresh Electron process per sample and reports p50/p95 for at least five runs.
- `crates/gosling/src/agents/moim.rs` and `crates/gosling/src/context_mgmt/mod.rs`:
  avoid full conversation clones unless MOIM injection or tool-pair summarization
  actually needs an owned conversation.
- `crates/gosling/src/security/patterns.rs`: use `RegexSet` to avoid running each
  individual regex against inputs that cannot match it.

## Record status

- `PERF-GSL-001`: open → closed; README claim is now accurately scoped.
- `PERF-GSL-002`: open → closed; reproducible, opt-in measurement harness added.
- `PERF-GSL-003`: open → partial; avoidable clones removed, but full-history
  tokenization and session reload need a dedicated turn-loop profile.
- `PERF-GSL-004`: partial → closed; `RegexSet` prefilter added without changing
  detection semantics or truncating scanner input.

## Validation

```sh
source bin/activate-hermit && cargo test -p gosling security::patterns --lib
source bin/activate-hermit && cargo test -p gosling agents::moim --lib
cd ui/desktop && source ../../bin/activate-hermit && pnpm exec prettier --check tests/e2e/performance.spec.ts
cd ui/desktop && source ../../bin/activate-hermit && pnpm run typecheck
cd ui/desktop && source ../../bin/activate-hermit && pnpm exec playwright test tests/e2e/performance.spec.ts --list
```

All listed checks passed. The opt-in benchmark itself was not run because it
launches ten fresh Electron instances; no performance number is claimed here.

## Follow-up

Profile a long, tool-heavy turn loop before changing the remaining
`PERF-GSL-003` tokenization and session-reload paths.
