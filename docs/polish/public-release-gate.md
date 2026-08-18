# Public release gate

Date: 2026-08-17
Verdict: **not ready**

This is an advisory staging assessment, not publication authorization.

## Blockers

1. The remote has released v1.0.0 and v1.0.1; this checkout still declares source version 0.1.0, and the release checklist still describes original v1.0.0 alignment work as open.
2. Documentation typecheck fails.
3. The required fresh-clone drill and dependency/security audit tools were not completed.
4. The public default branch has no active protection; the discovered ruleset is disabled.
5. The release checklist installed-artifact, signing, checksum, scenario, and clean-install gates are uncompleted.

## GHR coverage

| Code | Status | Evidence |
|---|---|---|
| GHR-001 | not-verifiable | Pattern fallback found no confirmed live credential; dedicated scanner unavailable. |
| GHR-002 | not-verifiable | History heuristic found no confirmed credential; dedicated history scanner unavailable. |
| GHR-003 | finding | Security ledger SEC-20260817-001. |
| GHR-004 | pass | Static endpoint scan found no confirmed private endpoint outside fixtures/examples. |
| GHR-005 | pass with note | Intentional large runtime/media assets reviewed; largest tracked file is about 47 MB. |
| GHR-006 | pass | ui/desktop/.env contains reviewed non-secret development settings. |
| GHR-007 | finding | README is otherwise complete, but source/release identity is unresolved. |
| GHR-008 | skipped | Fresh-clone drill not run; verdict capped. |
| GHR-009 | pass | Root, Cargo, and Desktop license declarations align on Apache-2.0. |
| GHR-010 | finding | Public repository lacks CODE_OF_CONDUCT.md. |
| GHR-011 | pass | Catch-all .github/CODEOWNERS route exists. |
| GHR-012 | finding | Inherited upstream links versus current remote require owner resolution. |
| GHR-013 | pass | No tracked build/cache junk found outside reviewed intentional assets. |
| GHR-014 | pass | No case-colliding tracked paths found. |
| GHR-015 | finding | .gitattributes exists but only classifies snapshots; it has no line-ending normalization policy. |
| GHR-016 | pass | Executable files reviewed as scripts or packaged runtime inputs; no symlinks reported. |
| GHR-017 | pass | No Windows-reserved or overlong tracked path found in the static check. |
| GHR-018 | pass | No tracked backup/conflict artifact found; local Hermit cache is ignored. |
| GHR-019 | finding | CI was in progress and a recent BuildNotify run failed. |
| GHR-020 | skipped | Fresh-environment build depends on the skipped clone drill. |
| GHR-021 | pass with note | Full gosling suite passed; ignored ACP-provider tests remain visible. |
| GHR-022 | not-verifiable | cargo-audit and cargo-deny unavailable. |
| GHR-023 | pass | PR and issue templates exist. |
| GHR-024 | finding | GitHub API reports main unprotected and ruleset disabled. |
| GHR-025 | finding | Dependabot auto-merge is gated to another repository identity. |
| GHR-026 | pass | Static scan found third-party action references pinned to full SHAs. |
| GHR-027 | pass | Only pull_request_target path is Dependabot-only and does not check out PR code. |
| GHR-028 | pass | No plaintext credential found in workflow review; secret references use GitHub secret contexts. |
| GHR-029 | finding | Historical local-path footprint carried from GHR-003. |
| GHR-030 | pass with note | Public repo has a description and topics; homepage is unset. |

## Next action

The release owner should select the canonical release identity and reconcile the
version/release history first. Then repair the documentation typecheck, enable
and verify branch protection, run a clean-clone drill plus approved security and
dependency audits, and complete RELEASE_CHECKLIST.md before requesting a
publication run.
