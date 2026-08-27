# Security / redaction ledger

Security-shaped findings are reported only. No secret was removed, no credential
was rotated, and no history was rewritten in this run.

| ID | Severity | Type | Location | Evidence | Recommended handling | Status |
|---|---|---|---|---|---|---|
| SEC-20260817-001 | medium | PII / local path | committed MCP replay fixtures and historical audit/session records | Static scan found real local macOS path footprints, including a replay fixture; no runtime source path was found. | Owner decides whether public history should be scrubbed; remove or redact in a reviewed change, then consider history rewrite only with explicit approval. | routed |
| SEC-20260817-002 | low | scanner coverage | working tree and history | gitleaks, trufflehog, cargo-audit, and cargo-deny were unavailable. Pattern fallbacks found no confirmed live credential. | Run an approved secret/history scanner before public release. | open |
| SEC-20260817-003 | reviewed | tracked configuration | ui/desktop/.env | Four keys were inspected without exposing values; all are non-secret development settings and none is credential-shaped. | Keep under review if new keys are added. | reviewed |
| SEC-20260827-001 | high | transitive dependency advisories | documentation/package-lock.json | `npm audit --package-lock-only` reports 25 advisories (19 high, 6 moderate), rooted in `image-size`, `serialize-javascript`, and `uuid` through `sockjs`. The lockfile update removed all critical advisories, and `npm audit fix` reports no non-breaking complete repair. | Track Docusaurus/webpack upstream releases and update when a compatible fixed chain exists. | blocked-upstream |

## Notes

- Workflow secrets references are GitHub secret references, not plaintext credentials.
- Large tracked binaries and media were reviewed separately as intentional package/site assets.
- The dependency-audit count is lockfile evidence, not proof that the vulnerable
  parser/serializer paths are reachable with attacker-controlled input in the
  published static site.
