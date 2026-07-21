# documentation index

This file is the durable map for the repository's documentation surface.

## authority and governance

- [repository instructions](../AGENTS.md)
- [architecture registry](../.architecture/README.md)
- [documentation style guide](./AGENTS.md)
- [root overview](../README.md)
- [docs-site workflow](./README.md)
- [structure compliance](./STRUCTURE_COMPLIANCE.md)
- [documentation inventory](./DOCUMENTATION_INVENTORY.md)

## user-facing docs

- [quickstart](./docs/quickstart.md)
- [getting started](./docs/getting-started/)
- [guides](./docs/guides/)
- [workspaces guide](./docs/guides/workspaces.md)
- [troubleshooting](./docs/troubleshooting/)
- [v1.0.0 release notes](./docs/release-notes/v1.0.0.md)
- [release-note archive](./docs/release-notes/)
- [tutorials](./docs/tutorials/)
- [experimental](./docs/experimental/)
- [mcp catalog](./docs/mcp/)
- [architecture docs](./docs/gosling-architecture/)

## site content and publishing

- [blog](./blog/README.md)
- [automation](./automation/README.md)
- [sidebar config](./sidebars.ts)
- [docusaurus config](./docusaurus.config.ts)

## release and validation

- [release process](../RELEASE.md)
- [v1.0.0 release checklist](../RELEASE_CHECKLIST.md)
- [current engineering TODO](../docs/TODO.md)
- [110-card live playtest and repair closure](../docs/cloud/2026-07-20-live-all-scenarios-playtest.md)
- [test scenario cards](../docs/test_scenarios/)

## stewardship notes

- The canonical documentation root is `documentation/`. This repo does not use a parallel top-level `docs/` governance tree.
- Root `README.md` is the product entry point; `documentation/README.md` is the docs-site build and publishing guide.
- Session-share deep links are documented with the `gosling://` scheme only. Legacy `goose://` share-link compatibility is not part of the current docs contract.
- Durable documentation governance artifacts currently live in this directory as point-in-time records rather than a full log/archive program.
- v1.0.0 documentation is prepared, but version alignment, validation, tagging, signing, publication, and updater promotion remain maintainer-owned release gates.

## open follow-ups

- Consolidate existing scenario-card and campaign evidence into a durable test ledger without replacing the source reports.
- Add a scoped documentation TODO ledger after separating documentation work from code TODO noise.
- Decide whether `.dory/` remains local-only operational state or should feed durable monthly summaries under a future log policy.
