# Defect ledger — Gosling shared shell productization

Every audit finding or implementation defect enters this ledger and receives one disposition: `open`, `fixed`, `verified-not-a-defect`, `deferred-with-risk`, `blocked-by-input`, or `blocked-by-tooling`. A fixed defect requires a regression test or an explicit not-testable reason.

## Baseline known gaps

| ID | Source | Finding | Severity | Root-cause status | Planned gate | Disposition / evidence |
| --- | --- | --- | --- | --- | --- | --- |
| SHP-DEF-001 | PR #46 deferral/static inspection | `createMinimalShellHost` has no production Electron call site; no shared shell application bootstraps it | high | Known implementation gap, not yet a defect in merged foundation | Gate 4 | open |
| SHP-DEF-002 | PR #46 deferral | No actual packaged Electron renderer-to-backend shell smoke test exists | high | Known validation gap | Gate 6 | open |
| SHP-DEF-003 | GitHub CI runs `31642034573` / `31659795858` | Linux `Build and Test Rust Project` can fail before Gosling tests because restored Cargo state lacks native `rusty_v8` | high | Existing helper is not wired to CI; version/build-script contract confirmed and isolated local seed/cache path passes | Gates 0–1 | open; current merged-main run reached tests, so failure is nondeterministic rather than universal |
| SHP-DEF-004 | PR #46 deferral/static package inspection | Shell-specific icons, updater feeds, complete package artifacts, and release-profile inputs are absent | medium | Deliberately deferred distribution work | Gates 3/7 | open |
| SHP-DEF-005 | Static inspection | Current Forge configuration derives only product name, protocol, and package ID from independent environment values; other product identity/assets remain Gosling-specific | high | No canonical product-profile resolver yet | Gates 2–3/7 | open |
| SHP-DEF-006 | Static inspection | `ShellFrame`/`ShellStatus` do not provide a shared runtime state/recovery/diagnostic/handoff application | medium | Deliberately minimal foundation | Gate 5 | open |
| SHP-DEF-007 | Static inspection | No observed installed coexistence test covers full Gosling plus multiple shell identities | high | Acceptance harness absent | Gate 6 | open |
| SHP-DEF-008 | Static inspection | No explicit bundled-core/profile/provisioning/handoff compatibility gate exists in Electron startup | high | Compatibility policy not yet frozen | Gates 2/4 | open |

These entries are not claims that implementation is broken beyond its documented current scope. They convert agreed remaining work and known CI failure into trackable closure items.

## Findings discovered during execution

| ID | Source | Finding | Severity | Root-cause status | Planned gate | Disposition / evidence |
| --- | --- | --- | --- | --- | --- | --- |
| SHP-DEF-009 | Gate 0 GitHub run `31660173759` | Current merged-main Rust CI fails `test_weather_tool` because an Anthropic replay hash is missing after V8 compilation succeeds | medium | Replay fixture drift outside the V8/productization slice; no root-cause repair attempted here | External/follow-up; Gate 8 acceptance blocker if still present | deferred-with-risk; exact log in `evidence/gate-0.md` |
| SHP-DEF-010 | Gate 0 toolchain probe | Direct shell resolved Homebrew Rust 1.97.1 instead of repository-pinned Rust 1.92 | medium | Hermit was not activated in the invoking shell | All implementation gates | fixed operationally: every authoritative command starts with `source bin/activate-hermit`; no code test required |

## New entry template

### SHP-DEF-NNN — title

- Discovered at gate / audit:
- Requirement(s):
- Severity:
- Symptom and reproduction:
- Root cause:
- Security/data/process/release impact:
- Disposition:
- Patch/files:
- Regression test or not-testable reason:
- Validation/evidence:
- Residual risk:
