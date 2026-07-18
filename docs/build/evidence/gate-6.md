# Gate 6 evidence — security, concurrency, and LLM boundaries

Date: 2026-07-18

## Audit closure

- `audit-security` v3.0: all SEC-001 through SEC-015 dispositions recorded.
- `audit-dataflow-concurrency` v3.1: all CON-001 through CON-018 dispositions recorded with a
  state/artifact concurrency inventory and real concurrent regressions.
- `audit-security-llm` v0.3: all LLM-001 through LLM-014 dispositions recorded with the
  workspace context trust path, agency boundary, consumption, and supply-chain overlays.
- Five findings were repaired: ACP diagnostic credential leakage, secret-shaped unknown field
  preservation, cross-process secure/profile lost updates, instruction-like workspace context,
  and unbounded model-visible metadata.

Canonical report:
`docs/build/audits/gate-6-security-concurrency-llm-audit.md`

Machine-readable findings:
`docs/build/audits/gate-6-security-concurrency-llm-findings.json`

## Commands and results

| Command | Result |
| ------- | ------ |
| `source bin/activate-hermit && cd ui/sdk && pnpm run build:ts` | pass |
| `source bin/activate-hermit && cd ui/sdk && pnpm test` | pass: 6 tests |
| `source bin/activate-hermit && cd ui/sdk && pnpm run typecheck:test` | pass |
| `cargo test -p gosling secret_mutations_across_config_instances_do_not_drop_updates --lib` | pass: 1 |
| `cargo test -p gosling workspace --lib` | pass: 26 |
| `cargo check -p gosling` | pass |
| `cargo fmt --check` | pass |
| `jq empty docs/build/audits/gate-6-security-concurrency-llm-findings.json` | pass |
| `git diff --check` | pass |

## Gate decision

Gate 6 passes. The complete Workspaces delta has explicit security/concurrency/LLM inventory
coverage; credential values remain on the secure-storage side of the renderer/session/model
boundaries; and every recorded finding has a bounded patch plus regression evidence.
