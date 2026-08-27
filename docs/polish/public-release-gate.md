# Public release gate

Date: 2026-08-27
Verdict: **not ready**

This is an advisory staging assessment, not publication authorization.

## Blockers

1. The required fresh-clone drill and approved secret/history scanners were not
   completed. Rust advisories and unused dependencies are clean under current
   `cargo-deny` and `cargo-machete`; the documentation npm audit retains 25
   transitive advisories with no compatible full fix.
2. The public default branch has no active protection; the discovered ruleset
   remains disabled as of 2026-08-27.
3. Current source repairs pass the complete local Rust, Desktop, documentation,
   dependency, and workflow-structure gates, but require a pushed revision and
   green remote CI to become revision-bound evidence.
4. The release checklist installed-artifact, signing, checksum, scenario,
   clean-install, tag, publication, verification, and announcement gates remain
   incomplete.

## GHR coverage

| Code | Status | Evidence |
|---|---|---|
| GHR-001 | not-verifiable | Pattern fallback found no confirmed live credential; dedicated scanner unavailable. |
| GHR-002 | not-verifiable | History heuristic found no confirmed credential; dedicated history scanner unavailable. |
| GHR-003 | finding | Security ledger SEC-20260817-001. |
| GHR-004 | pass | Static endpoint scan found no confirmed private endpoint outside fixtures/examples. |
| GHR-005 | pass with note | Intentional large runtime/media assets reviewed; largest tracked file is about 47 MB. |
| GHR-006 | pass | ui/desktop/.env contains reviewed non-secret development settings. |
| GHR-007 | pass | README and release surfaces identify cephalopod-ai/gosling and v1.1.0 consistently. |
| GHR-008 | skipped | Fresh-clone drill not run; verdict capped. |
| GHR-009 | pass | Root, Cargo, and Desktop license declarations align on Apache-2.0. |
| GHR-010 | finding | Public repository lacks CODE_OF_CONDUCT.md. |
| GHR-011 | pass | Catch-all .github/CODEOWNERS route exists. |
| GHR-012 | pass with note | Current repository links and repository-gated workflows now use cephalopod-ai/gosling; historical evidence and the separately governed npm package scope remain unchanged. |
| GHR-013 | pass | No tracked build/cache junk found outside reviewed intentional assets. |
| GHR-014 | pass | No case-colliding tracked paths found. |
| GHR-015 | finding | .gitattributes exists but only classifies snapshots; it has no line-ending normalization policy. |
| GHR-016 | pass | Executable files reviewed as scripts or packaged runtime inputs; no symlinks reported. |
| GHR-017 | pass | No Windows-reserved or overlong tracked path found in the static check. |
| GHR-018 | pass | No tracked backup/conflict artifact found; local Hermit cache is ignored. |
| GHR-019 | partial | Local source validation is green and BuildNotify explicitly succeeds as a no-op when its optional webhook is absent; no remote run exists for this patch revision. |
| GHR-020 | skipped | Fresh-environment build depends on the skipped clone drill. |
| GHR-021 | pass with note | Full gosling suite passed; ignored ACP-provider tests remain visible. |
| GHR-022 | pass with note | Current `cargo-deny check advisories` and `cargo-machete` pass; dedicated secret/history scanners remain separate. |
| GHR-023 | pass | PR and issue templates exist. |
| GHR-024 | finding | GitHub API reports main unprotected and ruleset disabled. |
| GHR-025 | pass | Dependabot and other repository-gated workflows now target cephalopod-ai/gosling. |
| GHR-026 | pass | Static scan found third-party action references pinned to full SHAs. |
| GHR-027 | pass | Only pull_request_target path is Dependabot-only and does not check out PR code. |
| GHR-028 | pass | No plaintext credential found in workflow review; secret references use GitHub secret contexts. |
| GHR-029 | finding | Historical local-path footprint carried from GHR-003. |
| GHR-030 | pass with note | Public repo has a description and topics; homepage is unset. |

## Next action

The release owner must enable and verify branch protection, run a clean-clone
drill plus approved secret/history scans, obtain green remote CI for the local
repairs, address or explicitly accept the upstream documentation advisories,
and complete the installed-artifact, signing, checksum, scenario, tag, and
publication sections of `RELEASE_CHECKLIST.md` before requesting publication.
