# Making a Release

gosling releases are built and published by GitHub Actions from version tags. Preparing documentation or pushing a release branch does not publish a release.

## Current release target

The current release target is **v1.0.0**.

Release theme:

- independent, release-ready product identity and attribution;
- workspace-scoped Desktop chats and secure credential profiles;
- Desktop startup, shutdown, packaged connectivity, and native windowing reliability;
- session, CLI, ACP, MCP, context, and memory hardening;
- security improvements around permissions, secrets, paths, plugins, providers, and workflows;
- a rigorous scenario-card, audit, repair, and documentation evidence trail.

The user-facing summary is in [the v1.0.0 release notes](documentation/docs/release-notes/v1.0.0.md). The maintainer gate is [RELEASE_CHECKLIST.md](RELEASE_CHECKLIST.md).

## Required version alignment

Before tagging `v1.0.0`, update and review every version-bearing surface, including:

- `Cargo.toml` workspace package version;
- workspace package entries in `Cargo.lock`;
- `ui/desktop/package.json` and the applicable pnpm lockfile entries;
- `ui/desktop/openapi.json` `info.version` and generated SDK metadata;
- packaged Desktop metadata and About/version output;
- README and documentation release notes.

At the 2026-07-20 documentation-preparation pass, the Rust workspace and Desktop package still reported `0.1.0`. Do not create `v1.0.0` until the checklist confirms all runtime and package surfaces report `1.0.0`.

## Automated release path

1. Run the [minor release workflow](https://github.com/repo-makeover/gosling/actions/workflows/minor-release.yaml) manually, or use its scheduled version-bump PR, if it matches the intended target.
2. Review and merge the version-bump PR into `main`.
3. Use the generated `release/<version>` branch and release PR for QA and release-only corrections.
4. Complete every required item in `RELEASE_CHECKLIST.md`, including installed artifacts on supported platforms.
5. Create and push the final `v1.0.0` tag only from the reviewed release commit.
6. Confirm `release.yml` completes and the GitHub release contains the expected signed artifacts, checksums, install scripts, and notes.
7. Perform the post-release checks before promoting updater behavior or announcing availability.

`release.yml` is currently tag-limited to `v1.*` releases. The previously inherited automatic patch-branch creation and tag-triggered release-PR cleanup workflows were intentionally retired. Patch releases therefore require an explicit reviewed branch/PR and tag; do not rely on an automatic next-patch branch.

## Tagging

Use the exact reviewed release commit. Replace `<release-commit>` only after the checklist is complete:

```bash
git tag -a v1.0.0 <release-commit> -m "gosling v1.0.0"
git push origin v1.0.0
```

Do not move or recreate a published tag to repair an artifact. Fix forward with a new patch version.

## Release boundary

- Documentation may be merged before the tag, but install links continue to resolve to the latest published artifact.
- Historical audit and release notes remain point-in-time evidence and are not rewritten to make a release look green.
- A successful source test suite is not a substitute for installed Desktop, signing, updater, and clean-machine checks.
- The release owner, not documentation automation, approves signing, tagging, publication, and announcement.
