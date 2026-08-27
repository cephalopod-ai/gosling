# documentation TODO

This scoped ledger tracks documentation-site work. Engineering defects and
release execution remain canonical in [`docs/TODO.md`](../docs/TODO.md).

## Open

- [ ] **RSP-GSL-004** — update the Docusaurus dependency chain when compatible
  releases clear the 25 transitive advisories reported by
  `npm audit --package-lock-only` (19 high, 6 moderate). The current roots are
  `image-size`, `serialize-javascript`, and `uuid` through `sockjs`; the current
  npm lockfile has no non-breaking automated fix.
- [ ] Decide whether `.dory/` remains local-only operational state or feeds a
  future durable summary. This is a repository-governance choice, not a docs
  build defect.

## Closed on 2026-08-27

- [x] Align Docusaurus runtime/type packages and restore passing typecheck,
  tests, and production build.
- [x] Repair the v1.1.0 release-note link that failed the broken-link build gate.
- [x] Link the existing engineering test ledger from the documentation index.
