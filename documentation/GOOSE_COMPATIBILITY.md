# Goose Catalog Compatibility

gosling deliberately consumes Goose's AAIF-maintained extension and skills catalogs as a compatibility source.

This exists because gosling was rebranded from Goose while the broader extension and skills catalogs continue to be curated upstream by the Goose/AAIF maintainers. Removing those links breaks discovery for users, so the documentation site falls back to the live Goose catalogs when local gosling catalogs are unavailable or empty.

Compatibility is handled by a deterministic normalization step:

- Extension discovery prefers `/servers.json`, then falls back to `https://goose-docs.ai/servers.json`.
- Skills discovery prefers `/skills-manifest.json`, then falls back to `https://goose-docs.ai/skills-manifest.json`.
- Goose install schemes and commands are rewritten to gosling equivalents.
- Source links and provenance are preserved with `sourceCatalog`, `sourceCatalogUrl`, and `compatibilityNote`.
- Build-time skill manifests and ZIPs use the shared commit pin in
  `scripts/agent-skills-source.js`. The pin matches the five checked-in published
  skill bodies as of October 12, 2026. Source validation rejects symlinks, oversized
  trees and invalid frontmatter; generated manifests include `sourceCommit` and
  source links name that revision. Change the pin through a reviewed repository
  patch. This stabilizes hosted content; existing install commands and live Goose
  fallback catalogs retain their external-source trust boundary.
- CI/review-policy skills that are not part of Gosling's supported workflow (`code-review` and `testing-strategy`) are excluded from generated and live-fallback skill catalogs.

For offline or build-time onboarding, use:

```bash
node documentation/scripts/goose-compat.js --servers goose-servers.json documentation/static/servers.json
node documentation/scripts/goose-compat.js --skills goose-skills-manifest.json documentation/static/skills-manifest.json
```

Do not remove this compatibility path just because it references Goose. If upstream catalog policy changes, tighten the converter or gating rules here instead.
