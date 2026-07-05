# Documentation Style Guide

## Brand Guidelines

**IMPORTANT**: The product name "gosling" should ALWAYS be written in lowercase "g" in all documentation, blog posts, and any content within this documentation directory.

- ✅ Correct: "gosling", "using gosling", "gosling provides"
- ❌ Incorrect: "Gosling", "using Gosling", "Gosling provides"

This is a brand guideline that must be strictly followed.

## Context

This rule applies to:
- All markdown files in `/docs/`
- All blog posts in `/blog/`
- README files
- Configuration files with user-facing text
- Any other documentation content

When editing or creating content in this documentation directory, always ensure "gosling" uses a lowercase "g".

## Goose Catalog Compatibility

The documentation site deliberately references Goose's AAIF-maintained extension and skills catalogs as a compatibility source for discovery. Keep this as an explicit adapter, not an accidental rename artifact.

- See `documentation/GOOSE_COMPATIBILITY.md` before changing extension or skills discovery.
- Use `documentation/scripts/goose-compat.js` for deterministic onboarding from Goose catalog JSON into gosling-compatible JSON.
- Preserve provenance fields such as `sourceCatalog`, `sourceCatalogUrl`, and `compatibilityNote`.
